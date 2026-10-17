//! 명령 표(UI-2, UI-11, KM-13, 7.6). 메뉴바와 명령 팔레트의 유일한 원천이다.
//!
//! Action마다 표시 이름(팔레트 이름 = 메뉴 항목 이름)과 자리를 하나 둔다. 자리는 메뉴이거나, 메뉴에 두지
//! 않는다고 명시한 「다른 곳」(눈에 보이는 자리를 적는다)이다. 설명은 Action 문서 주석을 쓴다.

mod table;

use gpui_kit::{Action, App, Menu, MenuItem, SharedString};

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
    /// 메뉴에 두지 않는다. 눈에 보이는 자리(UI-11).
    Elsewhere(&'static str),
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
        place: &'static str,
        variants: &'static [Variant],
    ) -> Self {
        CommandSpec {
            name,
            label,
            place: Place::Elsewhere(place),
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
                entries.push(PaletteEntry {
                    name: spec.name,
                    label: variant.label.into(),
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

/// 메뉴 항목 하나. 변형이 있으면 하위 메뉴, 인자가 열려 있으면 빈(꺼진) 하위 메뉴다(최근 파일은 S2-T7).
fn menu_item(spec: &CommandSpec, cx: &App) -> MenuItem {
    let action_item = |name: &'static str, action: Box<dyn Action>| MenuItem::Action {
        name: name.into(),
        action,
        os_action: None,
        checked: false,
        disabled: false,
    };
    if spec.variants.is_empty() {
        return match cx.build_action(spec.name, None) {
            Ok(action) => action_item(spec.label, action),
            Err(_) => MenuItem::submenu(Menu {
                name: spec.label.into(),
                items: Vec::new(),
                disabled: true,
            }),
        };
    }
    MenuItem::submenu(Menu {
        name: spec.label.into(),
        items: spec
            .variants
            .iter()
            .filter_map(|v| build_variant(spec.name, v, cx).map(|a| action_item(v.label, a)))
            .collect(),
        disabled: false,
    })
}
