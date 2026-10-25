//! 명령 표(UI-2, UI-11, KM-13, 7.6). 메뉴바와 명령 팔레트의 유일한 원천이다.
//!
//! Action마다 표시 이름(팔레트 이름 = 메뉴 항목 이름)과 자리를 하나 둔다. 자리는 메뉴이거나, 메뉴에 두지
//! 않는다고 명시한 「다른 곳」(눈에 보이는 자리를 적는다)이다. 설명은 Action 문서 주석을 쓴다.

pub mod ex;
mod table;

use gpui_kit::component::kbd::Kbd;
use gpui_kit::{Action, App, KeyContext, Keystroke, Menu, MenuItem, SharedString};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::actions::file::OpenRecent;

/// 메뉴바의 메뉴. 7.6의 일곱 메뉴 앞에 macOS 앱 메뉴를 둔다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MenuId {
    App,
    File,
    Edit,
    View,
    Track,
    Transport,
    Ai,
    Help,
}

impl MenuId {
    /// 메뉴바 순서.
    pub const ALL: [MenuId; 8] = [
        MenuId::App,
        MenuId::File,
        MenuId::Edit,
        MenuId::View,
        MenuId::Track,
        MenuId::Transport,
        MenuId::Ai,
        MenuId::Help,
    ];

    /// 메뉴 제목.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            MenuId::App => "KeyScore",
            MenuId::File => "파일",
            MenuId::Edit => "편집",
            MenuId::View => "보기",
            MenuId::Track => "트랙",
            MenuId::Transport => "재생",
            MenuId::Ai => "AI",
            MenuId::Help => "도움말",
        }
    }
}

/// 명령의 자리.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// 메뉴바. `group`이 바뀌는 곳에 구분선을 넣고, 메뉴 안 순서는 `order`다.
    Menu { menu: MenuId, group: u8, order: u16 },
    /// 메뉴에 두지 않는다. 눈에 보이는 화면 면과 그 안의 자리(UI-11). 키만으로 된 자리는 없다.
    Elsewhere {
        surface: Surface,
        detail: &'static str,
    },
}

/// 메뉴가 아닌 화면 면(UI-11). 「키」는 화면 면이 아니므로 없다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Surface {
    PianoRoll,
    Arrangement,
    TrackList,
    Inspector,
    Transport,
    StatusBar,
    KeyboardView,
    SequenceLanes,
    LaneHeader,
    AiPanel,
    Settings,
    Palette,
    Dialog,
    KeymapOverlay,
    SearchBar,
}

impl Surface {
    /// 화면 면 이름.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Surface::PianoRoll => "피아노롤",
            Surface::Arrangement => "배치 뷰",
            Surface::TrackList => "트랙 목록",
            Surface::Inspector => "트랙 인스펙터",
            Surface::Transport => "트랜스포트",
            Surface::StatusBar => "상태줄",
            Surface::KeyboardView => "건반 뷰",
            Surface::SequenceLanes => "Sequence 레인 뷰",
            Surface::LaneHeader => "컨트롤러 레인",
            Surface::AiPanel => "AI 패널",
            Surface::Settings => "설정",
            Surface::Palette => "명령 팔레트",
            Surface::Dialog => "대화상자",
            Surface::KeymapOverlay => "키맵 오버레이",
            Surface::SearchBar => "검색 막대",
        }
    }
}

/// 인자 있는 Action의 유한한 값 하나. 메뉴에서는 하위 메뉴 항목, 팔레트에서는 한 줄이다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant {
    label: &'static str,
    /// `cx.build_action`에 넘기는 JSON
    args: &'static str,
}

impl Variant {
    const fn new(label: &'static str, args: &'static str) -> Self {
        Variant { label, args }
    }

    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.label
    }

    #[must_use]
    pub const fn args(&self) -> &'static str {
        self.args
    }
}

/// 명령 표의 한 줄.
#[derive(Clone, Copy, Debug)]
pub struct CommandSpec {
    name: &'static str,
    label: &'static str,
    place: Place,
    variants: &'static [Variant],
}

impl CommandSpec {
    const fn menu(
        name: &'static str,
        label: &'static str,
        menu: MenuId,
        group: u8,
        order: u16,
        variants: &'static [Variant],
    ) -> Self {
        CommandSpec {
            name,
            label,
            place: Place::Menu { menu, group, order },
            variants,
        }
    }

    const fn elsewhere(
        name: &'static str,
        label: &'static str,
        surface: Surface,
        detail: &'static str,
        variants: &'static [Variant],
    ) -> Self {
        CommandSpec {
            name,
            label,
            place: Place::Elsewhere { surface, detail },
            variants,
        }
    }

    /// Action 이름(`edit::Undo`).
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// 표시 이름. 팔레트와 메뉴가 같은 이름을 쓴다.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.label
    }

    #[must_use]
    pub const fn place(&self) -> Place {
        self.place
    }

    #[must_use]
    pub const fn variants(&self) -> &'static [Variant] {
        self.variants
    }
}

/// 명령 표 전체.
#[must_use]
pub fn specs() -> &'static [CommandSpec] {
    table::SPECS
}

/// 이름으로 찾는다.
#[must_use]
pub fn spec(name: &str) -> Option<&'static CommandSpec> {
    specs().iter().find(|s| s.name == name)
}

/// 값이 열린 인자가 필요해 팔레트·메뉴에서 바로 실행할 수 없는 Action의 설명 꼬리.
const NEEDS_ARGUMENT: &str = "인자 필요: 팔레트에서 인자를 받는 것은 `:` 명령줄 결정(A12) 뒤";

/// 변형의 Action. 변형 JSON `null`은 인자 없음이다.
fn build_variant(name: &str, variant: &Variant, cx: &App) -> Option<Box<dyn Action>> {
    let args: serde_json::Value = serde_json::from_str(variant.args).ok()?;
    let args = (!args.is_null()).then_some(args);
    cx.build_action(name, args).ok()
}

/// 팔레트의 한 줄.
pub struct PaletteEntry {
    name: &'static str,
    label: SharedString,
    description: SharedString,
    /// 실행할 Action. 값이 열린 인자가 필요해 팔레트에서 실행할 수 없으면 `None`이다.
    action: Option<Box<dyn Action>>,
}

impl PaletteEntry {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    #[must_use]
    pub fn description(&self) -> &SharedString {
        &self.description
    }

    #[must_use]
    pub fn action(&self) -> Option<&dyn Action> {
        self.action.as_deref()
    }
}

/// 팔레트 줄 전체. Action마다 한 줄이고, 변형이 있으면 변형마다 한 줄이다.
#[must_use]
pub fn palette_entries(cx: &App) -> Vec<PaletteEntry> {
    let docs = cx.action_documentation();
    let mut entries = Vec::with_capacity(specs().len());
    for spec in specs() {
        let doc = docs.get(spec.name).copied().unwrap_or_default();
        // 최근 파일은 표에 고정된 변형이 아니라 지금 목록이다(S2-T7a).
        if spec.name == RECENT {
            let recent = crate::files::recent_labels(cx);
            // 목록이 비어도 팔레트에서 찾을 수는 있어야 한다(UI-11). 실행할 것이 없는 줄로 둔다.
            if recent.is_empty() {
                entries.push(PaletteEntry {
                    name: spec.name,
                    label: spec.label.into(),
                    description: format!("{doc} (목록이 비었다)").into(),
                    action: None,
                });
            }
            for (label, path) in recent {
                entries.push(PaletteEntry {
                    name: spec.name,
                    label: format!("{}: {label}", spec.label).into(),
                    description: path.clone().into(),
                    action: Some(Box::new(OpenRecent(path))),
                });
            }
            continue;
        }
        if spec.variants.is_empty() {
            let action = cx.build_action(spec.name, None).ok();
            let description = if action.is_some() {
                doc.into()
            } else {
                format!("{doc} ({NEEDS_ARGUMENT})").into()
            };
            entries.push(PaletteEntry {
                name: spec.name,
                label: spec.label.into(),
                description,
                action,
            });
        } else {
            for variant in spec.variants {
                // 메뉴 하위 항목의 이름(「다크 테마」)은 그대로 쓰고, 메뉴 밖 변형은 앞에 명령 이름을
                // 붙인다(「Step 길이: 4분음표」). 팔레트 이름이 겹치면 사람이 고를 수 없다.
                let label: SharedString = match spec.place {
                    Place::Menu { .. } => variant.label.into(),
                    Place::Elsewhere { .. } => format!("{}: {}", spec.label, variant.label).into(),
                };
                entries.push(PaletteEntry {
                    name: spec.name,
                    label,
                    description: doc.into(),
                    action: build_variant(spec.name, variant, cx),
                });
            }
        }
    }
    entries
}

/// 명령 표로 만든 메뉴바.
#[must_use]
pub fn menus(cx: &App) -> Vec<Menu> {
    MenuId::ALL
        .into_iter()
        .map(|id| {
            let mut placed: Vec<(u8, u16, &CommandSpec)> = specs()
                .iter()
                .filter_map(|s| match s.place {
                    Place::Menu { menu, group, order } if menu == id => Some((group, order, s)),
                    _ => None,
                })
                .collect();
            placed.sort_by_key(|(group, order, _)| (*group, *order));
            let mut items = Vec::with_capacity(placed.len() + 4);
            let mut last_group = None;
            for (group, _, spec) in placed {
                if last_group.is_some_and(|g| g != group) {
                    items.push(MenuItem::separator());
                }
                last_group = Some(group);
                items.push(menu_item(spec, cx));
            }
            Menu {
                name: id.title().into(),
                items,
                disabled: false,
            }
        })
        .collect()
}

/// 메뉴 항목 하나. 변형이 있으면 하위 메뉴, 인자가 열려 있으면 꺼진 빈 하위 메뉴다(최근 파일은 S2-T7).
/// 인자 없음(`null`) 변형이 있으면 그것만 바로 항목으로 둔다(설정, macOS 관례). 나머지 변형은 팔레트에만 있다.
fn menu_item(spec: &CommandSpec, cx: &App) -> MenuItem {
    if spec.name == RECENT {
        return recent_menu(spec, cx);
    }
    if let Some(plain) = spec.variants.iter().find(|v| v.args == "null") {
        return action_item(plain.label, spec.name, None, cx);
    }
    if spec.variants.is_empty() {
        return if cx.build_action(spec.name, None).is_ok() {
            action_item(spec.label, spec.name, None, cx)
        } else {
            MenuItem::submenu(Menu {
                name: spec.label.into(),
                items: Vec::new(),
                disabled: true,
            })
        };
    }
    MenuItem::submenu(Menu {
        name: spec.label.into(),
        items: spec
            .variants
            .iter()
            .map(|v| {
                let args = serde_json::from_str(v.args).ok();
                action_item(v.label, spec.name, args, cx)
            })
            .collect(),
        disabled: false,
    })
}

/// 「최근 파일」 Action 이름.
const RECENT: &str = "file::OpenRecent";

/// 최근 파일 하위 메뉴: 파일마다 한 항목(키 없음). 목록이 비면 꺼진다.
fn recent_menu(spec: &CommandSpec, cx: &App) -> MenuItem {
    let items: Vec<MenuItem> = crate::files::recent_labels(cx)
        .into_iter()
        .map(|(label, path)| MenuItem::Action {
            name: label.into(),
            action: Box::new(OpenRecent(path)),
            os_action: None,
            checked: false,
            disabled: false,
        })
        .collect();
    MenuItem::submenu(Menu {
        name: spec.label.into(),
        disabled: items.is_empty(),
        items,
    })
}

/// Action 항목.
///
/// 제목 뒤 키는 사람이 실제로 누를 키다: Normal 모드(배치 뷰·피아노롤)에서 다른 바인딩에 가려지지 않은
/// 가장 우선하는 바인딩이고, 키 시퀀스(`z z`)도 된다(리뷰 3 지적 1·2). GPUI가 굽는 메뉴 키 등가물은 그와
/// 다를 수 있다(`find_or_first`는 가려짐을 보지 않는다). 그래서
/// - 키 등가물이 글자 입력과 겹치면(`is_bare`) [`RunCommand`]로 감싸 키 등가물을 없앤다.
/// - 키 등가물이 실제로 누를 키와 같으면 제목에 붙이지 않는다(macOS가 오른쪽에 그린다).
/// - 그 밖에는 제목 뒤에 실제로 누를 키를 붙인다.
fn action_item(
    label: &'static str,
    name: &'static str,
    args: Option<serde_json::Value>,
    cx: &App,
) -> MenuItem {
    let Ok(action) = cx.build_action(name, args.clone()) else {
        return MenuItem::submenu(Menu {
            name: label.into(),
            items: Vec::new(),
            disabled: true,
        });
    };
    let native = native_key_equivalent(action.as_ref(), cx);
    // 글자 입력과 겹치거나, 어느 모드에서 눌러도 이 명령이 아닌(다른 바인딩에 가려진) 키 등가물은 굽지
    // 않는다(리뷰 4 지적 1: 사용자 키맵이 `cmd-j`를 다른 명령에 걸면 「합치기 ⌘J」가 거짓이 된다).
    let wrap = native.as_ref().is_some_and(|key| {
        is_bare(key) || !runs_somewhere(std::slice::from_ref(key), action.as_ref(), cx)
    });
    let shown = effective_keys(action.as_ref(), cx);
    // 감싸면 키 등가물이 없어지므로 macOS가 그려 주는 키도 없다
    let native_is_shown = !wrap
        && match (&native, &shown) {
            (Some(native), Some(shown)) => shown.len() == 1 && shown[0] == *native,
            _ => false,
        };
    let title: SharedString = match &shown {
        Some(keys) if !native_is_shown => format!(
            "{label}{MENU_KEY_SEPARATOR}{}",
            keys.iter().map(Kbd::format).collect::<Vec<_>>().join(" ")
        )
        .into(),
        _ => label.into(),
    };
    let action: Box<dyn Action> = if wrap {
        Box::new(RunCommand {
            name: name.to_owned(),
            args,
        })
    } else {
        action
    };
    MenuItem::Action {
        name: title,
        action,
        os_action: None,
        checked: false,
        disabled: false,
    }
}

/// 메뉴 항목의 키를 확인할 창의 키 컨텍스트 경로. 모드 FSM의 모든 상태가 만드는 경로다
/// (`mode::ModeState::context_path`). 「가사 분배 ⌘V」처럼 한 모드에서만 맞는 키도 그 모드에서는 사실이다.
fn menu_context_stacks() -> Vec<Vec<KeyContext>> {
    let mut paths: Vec<&'static [&'static str]> = crate::mode::ModeState::all()
        .map(crate::mode::ModeState::context_path)
        .collect();
    paths.sort_unstable();
    paths.dedup();
    paths
        .into_iter()
        // 고정 식별자라 늘 읽힌다
        .map(|path| {
            path.iter()
                .filter_map(|c| KeyContext::parse(c).ok())
                .collect()
        })
        .collect()
}

/// 어느 모드 경로에서든 이 키(시퀀스)를 누르면 이 Action이 나오는가.
fn runs_somewhere(keys: &[Keystroke], action: &dyn Action, cx: &App) -> bool {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    menu_context_stacks().iter().any(|stack| {
        let (found, pending) = keymap.bindings_for_input(keys, stack);
        !pending && found.first().is_some_and(|b| b.action().partial_eq(action))
    })
}

/// 사람이 실제로 누를 키: 우선순위가 높은 바인딩부터, 어느 모드 경로에서든 그 키를 누르면 이 Action이
/// 나오는 첫 바인딩. 컨텍스트 없는 바인딩도 같은 검사를 거친다(더 깊은 컨텍스트가 가릴 수 있다).
#[must_use]
pub fn effective_keys(action: &dyn Action, cx: &App) -> Option<Vec<Keystroke>> {
    let candidates: Vec<Vec<Keystroke>> = {
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        keymap
            .bindings_for_action(action)
            .map(|b| {
                b.keystrokes()
                    .iter()
                    .map(|k| gpui_kit::AsKeystroke::as_keystroke(k).clone())
                    .collect()
            })
            .collect()
    };
    candidates
        .into_iter()
        .rev()
        .find(|keys| runs_somewhere(keys, action, cx))
}

/// 감싼 메뉴 항목의 제목에서 이름과 키를 나누는 문자열.
pub const MENU_KEY_SEPARATOR: &str = "    ";

/// 팔레트 거르기와 순위. 맞는 줄의 번호를 순위대로 준다. 빈 질의는 모든 줄을 준다(실행할 수 있는 줄
/// 먼저, 그 안은 표 순서).
///
/// 실행할 수 있는 줄이 늘 먼저이고(꺼진 줄은 맨 뒤), 그 안의 순위는 이름이 같음 → 이름이 질의로 시작 →
/// 이름에 질의가 있음 → Action 이름이 같음 → Action 이름에 있음 → 설명에 있음이다. 메뉴 이름을 그대로
/// 치면 그 명령이 맨 위에 와야 하고(7.6 「팔레트에서 같은 이름으로 검색된다」), 첫 줄이 늘 실행할 수 있어야
/// 컴포넌트가 질의가 바뀔 때 강조를 첫 줄로 되돌려도 Enter가 엉뚱한 줄을 고르지 않는다(리뷰 2 지적 1).
#[must_use]
pub fn filter_entries(entries: &[PaletteEntry], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut scored: Vec<(bool, u8, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(ix, e)| {
            let label = e.label.to_lowercase();
            let name = e.name.to_lowercase();
            // 빈 질의는 모든 줄이 같은 순위(표 순서)다
            let rank = if query.is_empty() || label == query {
                0
            } else if label.starts_with(&query) {
                1
            } else if label.contains(&query) {
                2
            } else if name == query {
                3
            } else if name.contains(&query) {
                4
            } else if e.description.to_lowercase().contains(&query) {
                5
            } else {
                return None;
            };
            Some((e.action.is_none(), rank, ix))
        })
        .collect();
    scored.sort_unstable();
    scored.into_iter().map(|(_, _, ix)| ix).collect()
}

/// 메뉴 항목이 맨 키를 키 등가물로 갖게 될 때 대신 거는 Action.
///
/// GPUI macOS 메뉴는 Action의 바인딩 가운데 컨텍스트 없는 것을, 없으면 첫 바인딩을 키 등가물로 굽는다
/// (`gpui-pre-macos` `create_menu_item`). 모드 컨텍스트의 맨 키(`v`, `x`)가 그렇게 구워지면, GPUI가
/// 처리하지 않은 글자(팔레트 입력란에 친 `v`)를 `AppKit`이 메뉴로 넘겨 메뉴가 실행된다. 그런 항목은 이
/// Action으로 감싸 키 등가물을 없애고 제목 뒤에 키를 보인다. 실행은 실행 함수(`run_action`)를 거친다.
// GPUI `Action` 파생이 만드는 메서드의 unsafe 때문에 걸리는 lint다(actions.rs와 같은 이유).
#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Clone, PartialEq, Debug, Deserialize, JsonSchema, gpui_kit::Action)]
#[action(namespace = menu)]
#[serde(deny_unknown_fields)]
pub struct RunCommand {
    /// 실제 Action 이름
    pub name: String,
    /// 실제 Action 인자
    pub args: Option<serde_json::Value>,
}

/// 메뉴 항목이 실제로 실행하는 Action. 감싼 항목이면 풀어서 준다.
#[must_use]
pub fn menu_target(action: &dyn Action, cx: &App) -> Option<Box<dyn Action>> {
    match action.as_any().downcast_ref::<RunCommand>() {
        Some(run) => cx.build_action(&run.name, run.args.clone()).ok(),
        None => Some(action.boxed_clone()),
    }
}

/// GPUI가 이 Action의 메뉴 키 등가물로 고를 키(`create_menu_item`과 같은 규칙). 한 키가 아니면 없다.
#[must_use]
pub fn native_key_equivalent(action: &dyn Action, cx: &App) -> Option<Keystroke> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let default_context = {
        let mut workspace = KeyContext::new_with_defaults();
        workspace.add("Workspace");
        let mut pane = KeyContext::new_with_defaults();
        pane.add("Pane");
        let mut editor = KeyContext::new_with_defaults();
        editor.add("Editor");
        pane.extend(&editor);
        workspace.extend(&pane);
        vec![workspace]
    };
    let bindings: Vec<_> = keymap.bindings_for_action(action).collect();
    let chosen = bindings
        .iter()
        .find(|b| b.predicate().is_none_or(|p| p.eval(&default_context)))
        .or_else(|| bindings.first())?;
    match chosen.keystrokes() {
        [one] => Some(gpui_kit::AsKeystroke::as_keystroke(one).clone()),
        _ => None,
    }
}

/// 글자 입력과 겹치는 키. GPUI는 키 등가물에 Cmd·Ctrl·Alt·Shift만 싣고 fn은 버린다
/// (`create_menu_item`). 그래서 fn은 없는 것으로 보고, Alt(Option+글자는 특수 문자 입력)와 Shift만
/// 있는 키도 글자 입력과 겹친다고 본다(리뷰 2 지적 3, 제안 4). 기능 키(F1–F24)는 겹치지 않는다.
#[must_use]
pub fn is_bare(key: &Keystroke) -> bool {
    let m = key.modifiers;
    let function_key = key.key.len() > 1
        && key.key.starts_with('f')
        && key.key[1..].chars().all(|c| c.is_ascii_digit());
    !(m.platform || m.control || function_key)
}
