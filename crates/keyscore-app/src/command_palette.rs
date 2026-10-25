//! 명령 팔레트(UI-2). gpui-component `Command` 위에 명령 표의 줄을 올린다.
//!
//! 줄에는 Action을 달지 않는다. 컴포넌트가 확정 때 Action을 직접 디스패치하면 실행 함수(`run_action`,
//! 결정 C)를 거치지 않기 때문이다. 대신 확정한 줄의 Action을 [`PaletteEvent::Run`]으로 내보내고, 창이
//! 팔레트를 닫고 문서로 포커스를 돌린 뒤 실행 함수로 실행한다. 단축키 표시도 컴포넌트에 맡기지 않고
//! 문서 포커스 기준으로 찾는다. 팔레트를 연 자리에서 그 키가 무엇을 하는지 보여야 하기 때문이다.

use gpui_kit::component::command::{Command, CommandItem, CommandState};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::{ActiveTheme as _, Disableable as _};
use gpui_kit::{
    Action, AppContext as _, AsKeystroke as _, Context, Entity, EventEmitter, FocusHandle,
    Focusable, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window, div,
    px,
};

use crate::actions::{mode, palette};
use crate::commands::ex::{self, ExCommand};
use crate::commands::{self, PaletteEntry};

/// 팔레트가 창에 알리는 일.
pub enum PaletteEvent {
    /// 이 Action을 실행해 달라(팔레트는 닫힌다).
    Run(Box<dyn Action>),
    /// 이 ex 명령을 실행해 달라(결정 A12, 팔레트는 닫힌다).
    Ex(ExCommand),
    /// 닫아 달라.
    Close,
}

/// ex 줄의 요소 id(시험이 찾는다).
const EX_ROW_ID: &str = "palette-row-ex";

/// 열린 팔레트 하나.
pub struct CommandPalette {
    state: Entity<CommandState>,
    entries: Vec<PaletteEntry>,
    /// 지금 질의로 거른 줄 번호(순위순). 거르기와 순위는 컴포넌트가 아니라 여기서 한다
    /// (`commands::filter_entries`): 메뉴 이름을 그대로 치면 그 명령이 맨 위에 와야 한다.
    visible: Vec<usize>,
    /// 단축키를 찾는 기준(문서의 포커스).
    target: FocusHandle,
    /// 마지막 질의. 컴포넌트는 확정하면서 질의를 비우므로 확정 처리기는 이것을 읽는다(ex 명령줄).
    query_text: String,
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl CommandPalette {
    pub fn new(target: FocusHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| CommandState::new(window, cx));
        let entries = commands::palette_entries(cx);
        let visible = (0..entries.len()).collect();
        CommandPalette {
            state,
            entries,
            visible,
            target,
            query_text: String::new(),
        }
    }

    /// 지금 질의.
    #[must_use]
    pub fn query(&self, cx: &gpui_kit::App) -> String {
        self.state.read(cx).query(cx).to_string()
    }

    /// 질의 입력에 포커스를 준다.
    pub fn focus_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }

    /// 질의를 `text`로 둔다(`:` 명령줄 접두, A12).
    pub fn set_query_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.to_owned();
        self.state.update(cx, |state, cx| {
            state.set_query(SharedString::from(text.clone()), window, cx);
        });
        self.set_query(&text, cx);
    }

    /// 지금 질의가 ex 명령줄이면 읽은 결과.
    fn ex(&self) -> Option<Result<ExCommand, ex::ExError>> {
        ex::is_command_line(&self.query_text).then(|| ex::parse(&self.query_text))
    }

    /// 확정한 줄. 실행할 수 없는 줄은 컴포넌트가 확정하지 않는다(꺼진 줄).
    fn confirm(&mut self, row: usize, cx: &mut Context<Self>) {
        if let Some(parsed) = self.ex() {
            if let Ok(cmd) = parsed {
                cx.emit(PaletteEvent::Ex(cmd));
            }
            return;
        }
        let entry = self.visible.get(row).and_then(|ix| self.entries.get(*ix));
        if let Some(action) = entry.and_then(PaletteEntry::action) {
            cx.emit(PaletteEvent::Run(action.boxed_clone()));
        }
    }

    /// ex 명령줄 한 줄: 읽힌 명령의 설명, 또는 실행 불가 줄로 이유.
    fn ex_item(parsed: &Result<ExCommand, ex::ExError>, muted: Hsla) -> CommandItem {
        let (label, disabled) = match parsed {
            Ok(cmd) => (cmd.label(), false),
            Err(e) => (format!("실행할 수 없다: {e}"), true),
        };
        let shown = SharedString::from(label);
        CommandItem::new()
            .label(shown.clone())
            .disabled(disabled)
            .child(move |_, _| {
                div()
                    .id(EX_ROW_ID)
                    .aria_label(shown.clone())
                    .test_support()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_none().child(shown.clone()))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(muted)
                            .child("ex 명령(:w :q :e :new :set)"),
                    )
            })
    }

    /// `Tab`: 선택한 줄의 이름으로 질의를 채운다(5.11 「자동완성 확정」).
    fn complete(&mut self, _: &palette::Complete, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.state.read(cx).selected_index().map(|ix| ix.row);
        let Some(label) = selected
            .and_then(|row| self.visible.get(row))
            .and_then(|ix| self.entries.get(*ix))
            .map(|e| e.label().clone())
        else {
            return;
        };
        self.state
            .update(cx, |state, cx| state.set_query(label, window, cx));
    }

    /// 질의가 바뀌었다. 컴포넌트는 거르지 않고(`filterable(false)`) 강조를 첫 줄로 되돌린다. `:` 접두면
    /// Action 줄은 없고 ex 줄 하나다(A12).
    fn set_query(&mut self, query: &str, cx: &mut Context<Self>) {
        query.clone_into(&mut self.query_text);
        self.visible = if ex::is_command_line(query) {
            Vec::new()
        } else {
            commands::filter_entries(&self.entries, query)
        };
        cx.notify();
    }

    /// 한 줄의 단축키 표시. 여러 키 시퀀스는 공백으로 잇는다.
    fn shortcut(&self, action: &dyn Action, window: &Window) -> Option<String> {
        let binding = window.highest_precedence_binding_for_action_in(action, &self.target)?;
        Some(
            binding
                .keystrokes()
                .iter()
                .map(|k| Kbd::format(k.as_keystroke()))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

impl Focusable for CommandPalette {
    fn focus_handle(&self, cx: &gpui_kit::App) -> FocusHandle {
        self.state.focus_handle(cx)
    }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let ex_row = self.ex().map(|parsed| Self::ex_item(&parsed, muted));
        let items: Vec<CommandItem> = self
            .visible
            .iter()
            .map(|ix| &self.entries[*ix])
            .map(|entry| {
                let name = entry.name();
                let label = entry.label().clone();
                let description = entry.description().clone();
                let shortcut = entry.action().and_then(|a| self.shortcut(a, window));
                CommandItem::new()
                    .label(label.clone())
                    .keywords([name.into(), description.clone()])
                    .disabled(entry.action().is_none())
                    .child(move |_, _| {
                        div()
                            .id(format!("palette-row-{name}"))
                            .aria_label(label.clone())
                            .test_support()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_none().child(label.clone()))
                            .child(
                                div()
                                    .id(format!("palette-desc-{name}"))
                                    .aria_label(description.clone())
                                    .test_support()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(description.clone()),
                            )
                            .children(shortcut.clone().map(|keys| {
                                div()
                                    .id(format!("palette-key-{name}"))
                                    .aria_label(keys.clone())
                                    .test_support()
                                    .flex_none()
                                    .text_xs()
                                    .child(keys)
                            }))
                    })
            })
            .chain(ex_row)
            .collect();
        let this = cx.entity().downgrade();
        let on_cancel = this.clone();
        let on_query = this.clone();
        div()
            .id("command-palette")
            .test_support()
            .key_context("Palette")
            // 팔레트 밖(문서, 다른 패널)을 누르면 닫는다. 문서가 포커스를 가져간 뒤에도 팔레트가 남으면
            // Esc로도 닫을 수 없다(리뷰 1 지적 5).
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(PaletteEvent::Close)))
            .on_action(cx.listener(|_, _: &palette::Close, _, cx| cx.emit(PaletteEvent::Close)))
            .on_action(cx.listener(Self::complete))
            // `Shift+Tab`: 이전 후보(컴포넌트의 위로 이동과 같다)
            .on_action(|_: &palette::CompletePrev, window, cx| {
                window.dispatch_action(Box::new(gpui_kit::base::actions::SelectUp), cx);
            })
            // 팔레트가 열린 동안에는 팔레트 뒤의 모드를 바꾸지 않는다(리뷰 1 제안 1). 처리기가 전파를
            // 멈추므로 창의 모드 처리기에 닿지 않는다.
            .on_action(|_: &mode::Cycle, _, _| {})
            .on_action(|_: &mode::CycleBack, _, _| {})
            .w(px(560.))
            .child(
                Command::new(&self.state)
                    .items(items)
                    .filterable(false)
                    .on_query(move |query, _, cx| {
                        let _ = on_query.update(cx, |palette, cx| palette.set_query(query, cx));
                    })
                    .placeholder("명령 이름이나 Action 이름(edit::Undo)")
                    .on_confirm(move |ix, _, cx| {
                        let _ = this.update(cx, |palette, cx| palette.confirm(ix.row, cx));
                    })
                    .on_cancel(move |_, cx| {
                        let _ = on_cancel.update(cx, |_, cx| cx.emit(PaletteEvent::Close));
                    }),
            )
    }
}
