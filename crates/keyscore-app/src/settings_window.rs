//! 설정 창(S2-T6a): 구역 목록과 키맵 편집기(KM-2·3·4·8·9·11·12, 7.3).
//!
//! 설정 창은 앱에 하나인 별도 창이다. 프로젝트 창의 모드 키 컨텍스트와 섞이지 않는다. 키 컨텍스트는
//! `Settings`(창) > `Keymap`(키맵 구역) > `Editing`(목록), 시트는 `Settings > Dialog`다.
//!
//! 편집은 사용자 레이어(`keymap.json`)만 고친다. 쓰기 전 내용은 `.bak`으로 남기고 곧바로 다시 건다
//! (`keymap::save_user`). 사용자 파일에 오류가 있으면 고쳐 쓰지 않는다(손 편집을 덮지 않는다).

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants, Toggle};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable,
    Global, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    PathPromptOptions, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, TestSupportExt as _, TitlebarOptions, WeakEntity, Window, WindowOptions,
    div, prelude::FluentBuilder as _, px,
};

use crate::actions::args::{PresetName, SettingsSection};
use crate::actions::{dialog, keymap as km, settings};
use crate::keymap::conflict::{Conflict, ConflictKind};
use crate::keymap::source::{ActionRef, Layer};
use crate::keymap::user::{Effective, UserKeymap, binding_conflicts, effective};
use crate::keymap::{self, Preset, canonical_keys};

/// press-to-bind의 시퀀스 확정 대기(KM-5의 시퀀스 대기와 같다).
const CAPTURE_SETTLE: Duration = Duration::from_secs(1);

/// 왼쪽 구역 목록의 순서.
const SECTIONS: [(SettingsSection, &str, &str); 5] = [
    (SettingsSection::Input, "input", "입력"),
    (SettingsSection::MidiOutput, "midi-output", "MIDI 출력"),
    (SettingsSection::Metronome, "metronome", "메트로놈"),
    (SettingsSection::Keymap, "keymap", "키맵"),
    (SettingsSection::Ai, "ai", "AI"),
];

/// 열린 설정 창.
struct OpenSettings(Option<(AnyWindowHandle, WeakEntity<SettingsWindow>)>);

impl Global for OpenSettings {}

/// 키맵 목록의 한 줄.
struct Row {
    name: &'static str,
    title: SharedString,
    description: SharedString,
    keys: Vec<String>,
}

/// 열린 시트.
enum Sheet {
    /// 새 바인딩이 다른 Action과 부딪힌다(KM-4).
    Bind {
        action: String,
        context: Option<String>,
        keys: String,
        conflicts: Vec<Effective>,
    },
    /// 가져온 키맵 안의 충돌(KM-9).
    Import {
        user: UserKeymap,
        conflicts: Vec<Conflict>,
    },
}

/// 진행 중인 키 캡처.
struct Capture {
    keys: Vec<String>,
    /// 마지막 키 뒤의 확정 대기. 새 키가 오면 바꿔 끼워 앞 대기를 버린다.
    settle: Option<Task<()>>,
}

/// 설정 창.
pub struct SettingsWindow {
    section: SettingsSection,
    /// 창 루트의 포커스(키맵 밖 구역).
    focus: FocusHandle,
    /// 키맵 목록의 포커스(`Keymap > Editing`).
    list_focus: FocusHandle,
    /// 시트의 포커스(`Dialog`).
    sheet_focus: FocusHandle,
    search: Entity<InputState>,
    query: String,
    /// 고른 Action 이름. 거르기가 바뀌어도 같은 Action을 가리킨다.
    selected: Option<&'static str>,
    capture: Option<Capture>,
    sheet: Option<Sheet>,
    message: Option<String>,
    /// 검색란에 포커스가 있는가(입력란 사건으로 센다).
    query_focused: bool,
    _subscriptions: Vec<Subscription>,
}

/// 설정 창을 연다. 이미 있으면 앞으로 가져오고 구역만 바꾼다. 구역이 없으면 키맵이다.
pub fn open(cx: &mut App, section: Option<SettingsSection>) {
    if let Some((handle, view)) = current(cx) {
        let _ = handle.update(cx, |_, window, cx| {
            window.activate_window();
            view.update(cx, |this, cx| {
                if let Some(section) = section {
                    this.set_section(section, window, cx);
                }
            });
        });
        return;
    }
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("설정".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| {
            let mut this = SettingsWindow::new(window, cx);
            this.set_section(section.unwrap_or(SettingsSection::Keymap), window, cx);
            this
        })
    });
    if let Ok((handle, view)) = opened {
        cx.set_global(OpenSettings(Some((handle, view.downgrade()))));
    }
}

/// 열린 설정 창.
#[must_use]
pub fn current(cx: &App) -> Option<(AnyWindowHandle, Entity<SettingsWindow>)> {
    let (handle, view) = cx.try_global::<OpenSettings>()?.0.as_ref()?;
    Some((*handle, view.upgrade()?))
}

/// 앱 전역 처리기(`settings::Open`, `keymap::Edit`). 프로젝트 창 어디서든 연다.
pub(crate) fn register(cx: &mut App) {
    cx.on_action(|action: &settings::Open, cx| open(cx, action.0.0));
    cx.on_action(|_: &km::Edit, cx| open(cx, Some(SettingsSection::Keymap)));
}

impl SettingsWindow {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Action 이름·설명 검색"));
        // 캡처 중의 키는 바인딩(`r` 복원, `backspace` 지우기, 전역 `Cmd` 단축키)보다 먼저 받아야 한다. 요소의
        // 키 리스너는 Action 디스패치 뒤라 늦다. 이 창의 키만 본다.
        let me = cx.entity().downgrade();
        let settings_window = window.window_handle();
        let intercept = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != settings_window {
                return;
            }
            let taken = me
                .update(cx, |this, cx| {
                    this.capture_keystroke(&event.keystroke, window, cx)
                })
                .unwrap_or(false);
            if taken {
                cx.stop_propagation();
            }
        });
        let mut subscriptions = vec![intercept];
        subscriptions.push(cx.subscribe_in(
            &search,
            window,
            |this, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    this.query = state.read(cx).value().to_string();
                    this.selected = this.rows(cx).first().map(|r| r.name);
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.focus_list(window, cx),
                InputEvent::Focus => this.query_focused = true,
                InputEvent::Blur => this.query_focused = false,
            },
        ));
        SettingsWindow {
            section: SettingsSection::Keymap,
            focus: cx.focus_handle(),
            list_focus: cx.focus_handle(),
            sheet_focus: cx.focus_handle(),
            search,
            query: String::new(),
            selected: None,
            capture: None,
            sheet: None,
            message: None,
            query_focused: false,
            _subscriptions: subscriptions,
        }
    }

    #[must_use]
    pub fn section(&self) -> SettingsSection {
        self.section
    }

    /// 키맵 목록에서 고른 Action 이름.
    #[must_use]
    pub fn selected_action(&self, cx: &App) -> Option<String> {
        let rows = self.rows(cx);
        self.selected
            .filter(|s| rows.iter().any(|r| r.name == *s))
            .or_else(|| rows.first().map(|r| r.name))
            .map(str::to_owned)
    }

    #[must_use]
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    #[must_use]
    pub fn sheet_open(&self) -> bool {
        self.sheet.is_some()
    }

    /// 편집기가 마지막으로 남긴 알림.
    #[must_use]
    pub fn message(&self) -> Option<String> {
        self.message.clone()
    }

    /// 검색란에 포커스가 있는가(테스트용).
    #[must_use]
    pub fn search_focused_for_test(&self) -> bool {
        self.query_focused
    }

    fn set_section(
        &mut self,
        section: SettingsSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.section = section;
        self.cancel_capture(cx);
        if section == SettingsSection::Keymap {
            self.focus_list(window, cx);
        } else {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn step_section(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let i = SECTIONS
            .iter()
            .position(|(s, _, _)| *s == self.section)
            .unwrap_or(0);
        let n = SECTIONS.len().cast_signed();
        let next = (i.cast_signed() + delta).rem_euclid(n).cast_unsigned();
        self.set_section(SECTIONS[next].0, window, cx);
    }

    fn focus_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.list_focus, cx);
        cx.notify();
    }

    /// 거른 목록. 명령 표의 모든 Action(KM-3 「Action 목록」)과 지금 걸린 키.
    fn rows(&self, cx: &App) -> Vec<Row> {
        let (layers, _) = keymap::current_layers(cx);
        let eff = effective(&layers);
        let docs = cx.action_documentation();
        let query = self.query.trim().to_lowercase();
        crate::commands::specs()
            .iter()
            .map(|spec| {
                let name = spec.name();
                Row {
                    name,
                    title: spec.label().to_owned().into(),
                    description: docs.get(name).copied().unwrap_or_default().into(),
                    keys: eff
                        .iter()
                        .filter(|e| e.name == name)
                        .map(|e| match &e.context {
                            Some(c) => format!("{} ({c})", e.keys),
                            None => e.keys.clone(),
                        })
                        .collect(),
                }
            })
            .filter(|row| {
                query.is_empty()
                    || row.name.to_lowercase().contains(&query)
                    || row.title.to_lowercase().contains(&query)
                    || row.description.to_lowercase().contains(&query)
            })
            .collect()
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        if rows.is_empty() {
            return;
        }
        let i = self
            .selected
            .and_then(|s| rows.iter().position(|r| r.name == s))
            .unwrap_or(0);
        let next = (i.cast_signed() + delta).clamp(0, rows.len().cast_signed() - 1);
        self.selected = Some(rows[next.cast_unsigned()].name);
        cx.notify();
    }

    // ---------- 캡처(KM-3·8) ----------

    fn start_capture(&mut self, cx: &mut Context<Self>) {
        if self.selected_action(cx).is_none() || self.sheet.is_some() {
            return;
        }
        self.capture = Some(Capture {
            keys: Vec::new(),
            settle: None,
        });
        self.message = Some("키를 누른다(시퀀스 가능, 1초 쉬면 확정, Esc는 취소)".to_owned());
        cx.notify();
    }

    fn cancel_capture(&mut self, cx: &mut Context<Self>) {
        if self.capture.take().is_some() {
            self.message = Some("키 캡처를 취소했다".to_owned());
            cx.notify();
        }
    }

    /// 캡처 중의 키는 바인딩으로 디스패치하지 않고 여기서 받는다(앱의 키 가로채기, 모든 디스패치보다 먼저).
    /// 수식키 없는 `escape`는 취소다(KM-8). 가로챘으면 `true`.
    fn capture_keystroke(
        &mut self,
        k: &gpui_kit::Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.capture.is_none() {
            return false;
        }
        if k.key == "escape" && !k.modifiers.modified() {
            self.cancel_capture(cx);
            return true;
        }
        let stroke = canonical_keys(&k.unparse());
        let settle = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CAPTURE_SETTLE).await;
            let _ = this.update_in(cx, Self::finish_capture);
        });
        if let Some(capture) = &mut self.capture {
            capture.keys.push(stroke);
            capture.settle = Some(settle);
        }
        cx.notify();
        true
    }

    fn finish_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(capture) = self.capture.take() else {
            return;
        };
        let Some(action) = self.selected_action(cx) else {
            return;
        };
        if capture.keys.is_empty() {
            return;
        }
        let keys = capture.keys.join(" ");
        let (layers, user) = keymap::current_layers(cx);
        if let Err(why) = user {
            self.message = Some(format!("keymap.json을 고쳐 쓰지 않는다: {why}"));
            cx.notify();
            return;
        }
        let eff = effective(&layers);
        // 대상 컨텍스트: 그 Action의 지금(없으면 프리셋의) 첫 바인딩의 컨텍스트, 없으면 전역
        let context = eff
            .iter()
            .find(|e| e.name == action)
            .map(|e| e.context.clone())
            .or_else(|| {
                layers
                    .preset
                    .iter()
                    .find(|b| matches!(&b.action, ActionRef::Named { name, .. } if *name == action))
                    .map(|b| b.context.clone())
            })
            .flatten();
        let conflicts = binding_conflicts(&eff, context.as_deref(), &keys, &action);
        if conflicts.is_empty() {
            self.bind(&action, context.as_deref(), &keys, cx);
        } else {
            self.sheet = Some(Sheet::Bind {
                action,
                context,
                keys,
                conflicts,
            });
            window.focus(&self.sheet_focus, cx);
        }
        cx.notify();
    }

    /// 새 바인딩을 사용자 레이어에 넣고 저장한다. 표준 단축키를 덮으면 알린다(KM-12).
    fn bind(&mut self, action: &str, context: Option<&str>, keys: &str, cx: &mut Context<Self>) {
        let (layers, user) = keymap::current_layers(cx);
        let mut user = match user {
            Ok(user) => user,
            Err(why) => {
                self.message = Some(format!("keymap.json을 고쳐 쓰지 않는다: {why}"));
                return;
            }
        };
        let eff = effective(&layers);
        // 그 자리를 가진 다른 Action(충돌 시트의 「새 바인딩 유지」)은 키를 잃는다. 접두 충돌의 상대는 지운다.
        for other in binding_conflicts(&eff, context, keys, action) {
            if canonical_keys(&other.keys) != canonical_keys(keys) {
                user.clear_place(other.context.as_deref(), &other.keys, other.layer);
            }
        }
        user.bind(action, context, keys, &eff);
        let standard = layers
            .standard
            .iter()
            .any(|b| canonical_keys(&b.keys) == canonical_keys(keys));
        self.message = Some(match keymap::save_user(cx, &user) {
            Ok(()) if standard => {
                format!(
                    "{action}에 {keys}를 걸었다. 경고: 표준 단축키를 덮는다(메뉴 단축키가 바뀐다)"
                )
            }
            Ok(()) => format!("{action}에 {keys}를 걸었다"),
            Err(e) => e,
        });
    }

    fn edit_user(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut UserKeymap, &[Effective], &crate::keymap::LayerSet),
    ) -> bool {
        let (layers, user) = keymap::current_layers(cx);
        let mut user = match user {
            Ok(user) => user,
            Err(why) => {
                self.message = Some(format!("keymap.json을 고쳐 쓰지 않는다: {why}"));
                cx.notify();
                return false;
            }
        };
        let eff = effective(&layers);
        edit(&mut user, &eff, &layers);
        let result = keymap::save_user(cx, &user);
        if let Err(e) = &result {
            self.message = Some(e.clone());
        }
        cx.notify();
        result.is_ok()
    }

    // ---------- 시트 ----------

    fn confirm_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.sheet.take() {
            Some(Sheet::Bind {
                action,
                context,
                keys,
                ..
            }) => self.bind(&action, context.as_deref(), &keys, cx),
            Some(Sheet::Import { mut user, .. }) => {
                user.keep_later_of_conflicts();
                self.message = Some(match keymap::save_user(cx, &user) {
                    Ok(()) => "키맵을 가져왔다(충돌마다 뒤 항목을 남겼다)".to_owned(),
                    Err(e) => e,
                });
            }
            None => return,
        }
        self.focus_list(window, cx);
    }

    fn cancel_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sheet.take().is_some() {
            self.message = Some("취소했다".to_owned());
            self.focus_list(window, cx);
        }
    }

    // ---------- 내보내기·가져오기(KM-9) ----------

    fn export(&mut self, _: &km::Export, window: &mut Window, cx: &mut Context<Self>) {
        // 지난 결과 알림을 지운다. 대화상자를 닫으면(취소) 아무것도 남지 않는다
        self.message = None;
        let dir = keymap::user_path(cx)
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let chosen = cx.prompt_for_new_path(&dir, Some("keymap.json"));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.message = Some(match export_to(cx, &path) {
                    Ok(()) => format!("키맵을 내보냈다: {}", path.display()),
                    Err(e) => e,
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn import(&mut self, _: &km::Import, window: &mut Window, cx: &mut Context<Self>) {
        self.message = None;
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("가져오기".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| this.import_from(&path, window, cx));
        })
        .detach();
    }

    /// 파일을 읽고, 충돌이 있으면 적용 전에 시트로 묻는다. 없으면 바로 적용한다.
    fn import_from(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let parsed = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| {
                UserKeymap::parse(&text)
                    .map_err(|errors| errors.first().map_or_else(String::new, keymap::describe))
            });
        let user = match parsed {
            Ok(user) => user,
            Err(why) => {
                self.message = Some(format!("키맵을 가져오지 못했다({}): {why}", path.display()));
                cx.notify();
                return;
            }
        };
        let conflicts = crate::keymap::conflict::conflicts(user.bindings());
        if conflicts.is_empty() {
            self.message = Some(match keymap::save_user(cx, &user) {
                Ok(()) => "키맵을 가져왔다".to_owned(),
                Err(e) => e,
            });
        } else {
            self.sheet = Some(Sheet::Import { user, conflicts });
            window.focus(&self.sheet_focus, cx);
        }
        cx.notify();
    }

    // ---------- 그리기 ----------

    fn render_sheet(&self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let sheet = self.sheet.as_ref()?;
        let (text, confirm) = match sheet {
            Sheet::Bind {
                action,
                context,
                keys,
                conflicts,
            } => {
                let others: Vec<String> = conflicts
                    .iter()
                    .map(|c| {
                        let kind = if canonical_keys(&c.keys) == canonical_keys(keys) {
                            "같은 키"
                        } else {
                            "접두 충돌"
                        };
                        format!("{} {} ({kind})", c.keys, c.action)
                    })
                    .collect();
                (
                    format!(
                        "{keys}를 {action}에 걸면{} 다음과 부딪힌다: {}. 새 바인딩을 유지하면 그쪽 키를 지운다.",
                        context
                            .as_deref()
                            .map(|c| format!("({c})"))
                            .unwrap_or_default(),
                        others.join(", ")
                    ),
                    "새 바인딩 유지",
                )
            }
            Sheet::Import { conflicts, .. } => {
                let lines: Vec<String> = conflicts
                    .iter()
                    .map(|c| {
                        let kind = match c.kind {
                            ConflictKind::SameKeys => "같은 키",
                            ConflictKind::Prefix => "접두 충돌",
                        };
                        format!(
                            "{} {} ↔ {} {} ({kind})",
                            c.first.0, c.first.1, c.second.0, c.second.1
                        )
                    })
                    .collect();
                (
                    format!(
                        "가져올 키맵에 충돌이 {}개 있다: {}. 적용하면 충돌마다 파일에서 뒤 항목만 남긴다.",
                        conflicts.len(),
                        lines.join("; ")
                    ),
                    "뒤 항목 남기고 적용",
                )
            }
        };
        Some(
            div()
                .key_context("Dialog")
                .track_focus(&self.sheet_focus)
                .on_action(cx.listener(|this, _: &dialog::Confirm, window, cx| {
                    this.confirm_sheet(window, cx);
                }))
                .on_action(cx.listener(|this, _: &dialog::Cancel, window, cx| {
                    this.cancel_sheet(window, cx);
                }))
                .p_3()
                .gap_2()
                .flex()
                .flex_col()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .child(
                    div()
                        .id("keymap-sheet")
                        .aria_label(text.clone())
                        .test_support()
                        .child(text),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("sheet-confirm")
                                .label(confirm)
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.confirm_sheet(window, cx);
                                })),
                        )
                        .child(Button::new("sheet-cancel").label("취소").ghost().on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.cancel_sheet(window, cx);
                            }),
                        )),
                )
                .into_any_element(),
        )
    }

    #[allow(
        clippy::too_many_lines,
        reason = "키맵 구역의 위 줄·검색란·목록을 화면 순서대로 한곳에 둔다"
    )]
    fn render_keymap(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let options = keymap::options(cx);
        let rows = self.rows(cx);
        let selected = self.selected_action(cx);
        let capturing = self.capture.as_ref().map(|c| c.keys.join(" "));
        let preset_button = |id: &'static str, label: &'static str, preset: Preset| {
            let on = options.preset == preset;
            Toggle::new(id)
                .label(label)
                .checked(on)
                .on_click(move |_, window, cx| {
                    let name = match preset {
                        Preset::Default => PresetName::Default,
                        Preset::Vim => PresetName::Vim,
                    };
                    window.dispatch_action(Box::new(km::SelectPreset(name)), cx);
                })
        };
        let list = rows.iter().enumerate().map(|(i, row)| {
            let is_selected = selected.as_deref() == Some(row.name);
            let keys = if is_selected && let Some(k) = &capturing {
                format!("캡처 중: {k}")
            } else if row.keys.is_empty() {
                "(없음)".to_owned()
            } else {
                row.keys.join(", ")
            };
            let label = format!("{} {} {}", row.title, row.name, keys);
            let name = row.name;
            div()
                .id(("keymap-row", i))
                .aria_label(label)
                .test_support()
                .flex()
                .gap_3()
                .px_2()
                .when(is_selected, |d| d.bg(cx.theme().accent))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.selected = Some(name);
                    this.focus_list(window, cx);
                }))
                .child(div().w(px(180.)).child(row.title.clone()))
                .child(
                    div()
                        .w(px(200.))
                        .text_color(cx.theme().muted_foreground)
                        .child(row.name),
                )
                .child(div().flex_1().child(keys))
        });
        div()
            .key_context("Keymap")
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(preset_button(
                        "keymap-preset-default",
                        "기본",
                        Preset::Default,
                    ))
                    .child(preset_button("keymap-preset-vim", "vim", Preset::Vim))
                    .child(
                        Toggle::new("keymap-swap")
                            .label("수식키 교환")
                            .checked(options.swap_alt_cmd)
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(km::SwapModifiers), cx);
                            }),
                    )
                    .child(
                        Button::new("keymap-export")
                            .label("내보내기")
                            .ghost()
                            .on_click(|_: &ClickEvent, window, cx| {
                                window.dispatch_action(Box::new(km::Export), cx);
                            }),
                    )
                    .child(
                        Button::new("keymap-import")
                            .label("가져오기")
                            .ghost()
                            .on_click(|_: &ClickEvent, window, cx| {
                                window.dispatch_action(Box::new(km::Import), cx);
                            }),
                    ),
            )
            .child(
                div()
                    .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                        let k = &e.keystroke;
                        let plain = !k.modifiers.modified();
                        if plain && k.key == "escape" {
                            // 검색어를 비우고 목록으로(A13 통상의 취소)
                            this.search.update(cx, |s, cx| s.set_value("", window, cx));
                            this.query.clear();
                            this.focus_list(window, cx);
                            cx.stop_propagation();
                        } else if plain && k.key == "down" {
                            this.focus_list(window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(Input::new(&self.search).id("keymap-search")),
            )
            .child(
                div()
                    .id("keymap-list")
                    .key_context("Editing")
                    .track_focus(&self.list_focus)
                    .on_action(cx.listener(|this, _: &km::SelectNext, _, cx| {
                        this.move_selection(1, cx);
                    }))
                    .on_action(cx.listener(|this, _: &km::SelectPrev, _, cx| {
                        this.move_selection(-1, cx);
                    }))
                    .on_action(cx.listener(|this, _: &km::FocusSearch, window, cx| {
                        this.search.update(cx, |s, cx| {
                            s.focus(window, cx);
                            s.select_all(window, cx);
                        });
                        cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &km::StartCapture, _, cx| {
                        this.start_capture(cx);
                    }))
                    .on_action(cx.listener(|this, _: &km::CancelCapture, _, cx| {
                        this.cancel_capture(cx);
                    }))
                    .on_action(cx.listener(|this, _: &km::ClearBinding, _, cx| {
                        if let Some(action) = this.selected_action(cx)
                            && this.edit_user(cx, |user, eff, _| user.clear(&action, eff))
                        {
                            this.message = Some(format!("{action}의 키를 지웠다"));
                        }
                    }))
                    .on_action(cx.listener(|this, _: &km::RestoreDefault, _, cx| {
                        if let Some(action) = this.selected_action(cx)
                            && this.edit_user(cx, |user, _, layers| {
                                user.restore(&action, &layers.preset);
                            })
                        {
                            this.message = Some(format!("{action}을 프리셋 기본값으로 되돌렸다"));
                        }
                    }))
                    .flex_1()
                    .overflow_y_scroll()
                    .border_1()
                    .border_color(cx.theme().border)
                    .children(list),
            )
            .into_any_element()
    }
}

/// 지금 사용자 레이어를 `path`에 JSON 파일 하나로 쓴다(KM-9).
///
/// # Errors
///
/// 사용자 파일을 읽을 수 없거나 쓰지 못한 이유.
pub fn export_to(cx: &App, path: &Path) -> Result<(), String> {
    let (_, user) = keymap::current_layers(cx);
    let user = user?;
    std::fs::write(path, user.to_json()).map_err(|e| format!("내보내지 못했다: {e}"))
}

impl UserKeymap {
    /// 그 자리(컨텍스트·키)의 실효 바인딩을 지운다. 사용자 항목이면 빼고, 아래 레이어 것이면 `null`로 가린다.
    pub(crate) fn clear_place(&mut self, context: Option<&str>, keys: &str, layer: Layer) {
        let eff = [Effective {
            context: context.map(str::to_owned),
            keys: keys.to_owned(),
            action: String::new(),
            name: String::new(),
            args: None,
            layer,
        }];
        self.clear("", &eff);
    }
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let section = self.section;
        let sidebar: Vec<Button> = SECTIONS
            .iter()
            .map(|&(s, id, label)| {
                Button::new(SharedString::from(format!("settings-section-{id}")))
                    .label(label)
                    .when(s != section, ButtonVariants::ghost)
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.set_section(s, window, cx);
                    }))
            })
            .collect();
        let body = if section == SettingsSection::Keymap {
            self.render_keymap(cx).into_any_element()
        } else {
            let label = SECTIONS
                .iter()
                .find(|(s, _, _)| *s == section)
                .map_or("", |(_, _, l)| *l);
            div()
                .id("settings-placeholder")
                .aria_label(format!("{label}: S2-T6c에서 채운다"))
                .test_support()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{label} 설정은 S2-T6c에서 채운다"))
                .into_any_element()
        };
        let message = self.message.clone().unwrap_or_default();
        let sheet = self.render_sheet(cx);
        div()
            .id("settings-window")
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &settings::NextSection, window, cx| {
                this.step_section(1, window, cx);
            }))
            .on_action(cx.listener(|this, _: &settings::PrevSection, window, cx| {
                this.step_section(-1, window, cx);
            }))
            .on_action(cx.listener(|_, action: &km::SelectPreset, _, cx| {
                let preset = match action.0 {
                    PresetName::Default => Preset::Default,
                    PresetName::Vim => Preset::Vim,
                };
                keymap::set_preset(cx, preset);
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &km::SwapModifiers, _, cx| {
                let on = !keymap::options(cx).swap_alt_cmd;
                keymap::set_swap_alt_cmd(cx, on);
                cx.notify();
            }))
            .on_action(cx.listener(Self::export))
            .on_action(cx.listener(Self::import))
            .size_full()
            .flex()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .child(
                div()
                    .w(px(140.))
                    .flex_none()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .children(sidebar),
            )
            .child(
                div()
                    .flex_1()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().flex_1().min_h_0().child(body))
                    .children(sheet)
                    .child(
                        div()
                            .id("settings-message")
                            .aria_label(message.clone())
                            .test_support()
                            .text_color(cx.theme().muted_foreground)
                            .child(message),
                    ),
            )
    }
}
