//! 검색 막대와 결과 이동(S3-T7, REQ 5.8 `Cmd+F`, 5.8.1 `/`·`n`·`N`, 7.5).
//!
//! 막대는 프로젝트 창 안의 한 줄 패널이다. 팔레트·퀀타이즈 시트처럼 모드 키 컨텍스트 겹 바깥에 그리고, 열린
//! 동안 포커스는 막대의 입력란에 있다. 그래서 문서 키는 동작하지 않는다. 배경은 막지 않는다.
//! 순수 계산은 `search.rs`, 규칙은 결정 `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md` 1–5절.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EntityInputHandler as _, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _, Window,
    deferred, div, px,
};

use crate::actions::search;
use crate::mode::Mode;
use crate::pianoroll::Cursor;
use crate::search::{Direction, Position, Query, find_hits, step};
use crate::window::ProjectWindow;

/// 열린 검색 막대.
pub(crate) struct SearchBar {
    input: Entity<InputState>,
    /// 지금 검색어의 결과 수 글(결정 1절 5). 글자가 바뀔 때 다시 센다.
    status: String,
    _events: Subscription,
    /// 창이 활성인 동안 포커스가 막대 밖으로 나가면 닫는다(결정 1절 7).
    _focus_out: Subscription,
}

/// 검색어의 결과 수 글.
fn count_text(project: &keyscore_core::Project, text: &str) -> String {
    match Query::parse(text) {
        Ok(query) => match find_hits(project, &query).len() {
            0 => "결과 없음".to_owned(),
            n => format!("결과 {n}개"),
        },
        Err(crate::search::QueryError::Empty) => String::new(),
        Err(e) => e.message().to_owned(),
    }
}

impl ProjectWindow {
    /// 검색 막대가 열려 있는가.
    #[must_use]
    pub fn search_bar_open(&self) -> bool {
        self.search_bar.is_some()
    }

    /// 막대의 결과 수 글. 막대가 닫혀 있으면 `None`.
    #[must_use]
    pub fn search_status(&self) -> Option<String> {
        self.search_bar.as_ref().map(|b| b.status.clone())
    }

    /// 지난 검색어(`Enter`·버튼으로 확정한 것).
    #[must_use]
    pub fn last_search(&self) -> Option<String> {
        self.last_search.clone()
    }

    /// 검색은 Normal 모드에서만 한다(결정 1절 2). 아니면 메시지를 남기고 거짓.
    fn search_allowed(&mut self, cx: &mut Context<Self>) -> bool {
        if self.mode_state().mode() == Mode::Normal {
            true
        } else {
            self.say("검색은 Normal 모드에서 한다", cx);
            false
        }
    }

    /// `search::Open`: 막대를 연다. 이미 열려 있으면 글자를 전부 골라 포커스만 돌린다.
    pub(crate) fn search_open(
        &mut self,
        _: &search::Open,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.search_allowed(cx) {
            return;
        }
        // 팝업은 하나만 띄운다
        self.palette = None;
        self.overlay = None;
        self.popup = None;
        self.quantize_sheet = None;
        self.note_info = None;
        let reopened = self.search_bar.is_some();
        if !reopened {
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("c#4, 트랙 이름, 가사"));
            let events = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
                // `Enter`는 `search_key_down`이 먼저 받는다(그 주석)
                if matches!(event, InputEvent::Change) {
                    this.search_text_changed(cx);
                }
            });
            let focus = input.focus_handle(cx);
            // 창이 비활성이 되어 잃는 포커스는 닫지 않는다(입력란과 같은 macOS 관례, `fields.rs`)
            let focus_out = cx.on_focus_out(&focus, window, |this, _, window, cx| {
                if window.is_window_active() {
                    this.close_search_bar(window, cx);
                }
            });
            let status = count_text(
                self.document.read(cx).project(),
                self.last_search.as_deref().unwrap_or_default(),
            );
            self.search_bar = Some(SearchBar {
                input,
                status,
                _events: events,
                _focus_out: focus_out,
            });
        }
        let initial = self.last_search.clone().unwrap_or_default();
        if let Some(bar) = &self.search_bar {
            bar.input.update(cx, |state, cx| {
                // 지난 검색어를 채운다(결정 1절 3). 이미 열린 막대면 친 글자를 그대로 둔다
                if !reopened {
                    state.set_value(initial, window, cx);
                }
                state.focus(window, cx);
                state.select_all(window, cx);
            });
        }
        cx.notify();
    }

    fn search_text_changed(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &self.search_bar else {
            return;
        };
        let text = bar.input.read(cx).value().to_string();
        let status = count_text(self.document.read(cx).project(), &text);
        if let Some(bar) = &mut self.search_bar {
            bar.status = status;
        }
        cx.notify();
    }

    /// 문서가 바뀌면 막대의 결과 수를 다시 센다.
    pub(crate) fn search_document_changed(&mut self, cx: &mut Context<Self>) {
        if self.search_bar.is_some() {
            self.search_text_changed(cx);
        }
    }

    /// 막대를 닫는다. 지난 검색어는 바꾸지 않는다. 포커스가 막대에 있었으면 문서로 돌린다.
    pub(crate) fn close_search_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bar) = self.search_bar.take() else {
            return;
        };
        if bar.input.focus_handle(cx).contains_focused(window, cx) || window.focused(cx).is_none() {
            self.focus_document(window, cx);
        }
        cx.notify();
    }

    /// 막대의 글자를 지난 검색어로 두고 결과로 옮긴다. `close`면 막대를 닫는다(`Enter`), 아니면 둔다(버튼).
    /// 빈 검색어는 닫기만 한다(결정 1절 6).
    fn search_confirm(
        &mut self,
        direction: Direction,
        close: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bar) = &self.search_bar else {
            return;
        };
        let text = bar.input.read(cx).value().trim().to_owned();
        if close {
            self.close_search_bar(window, cx);
        }
        if text.is_empty() {
            return;
        }
        self.last_search = Some(text.clone());
        self.search_jump(&text, direction, cx);
    }

    /// `text`로 찾아 `direction`의 결과로 커서를 옮긴다(결정 3·4절).
    fn search_jump(&mut self, text: &str, direction: Direction, cx: &mut Context<Self>) {
        let query = match Query::parse(text) {
            Ok(query) => query,
            Err(e) => {
                self.say(format!("검색 {text}: {}", e.message()), cx);
                return;
            }
        };
        let (hits, found) = {
            let doc = self.document.read(cx);
            let project = doc.project();
            let hits = find_hits(project, &query);
            let cursor = self.pianoroll.cursor();
            let at = Position {
                track_index: self
                    .current_track()
                    .and_then(|id| project.track_index(id))
                    .unwrap_or(0),
                tick: cursor.tick,
                pitch: cursor.pitch,
            };
            let found = step(project, &hits, at, direction);
            (hits, found)
        };
        // 검색 이동은 오퍼레이터의 모션이 아니다. 대기를 끝내고 이동만 한다(결정 4절 4)
        self.clear_operator(cx);
        let Some(found) = found else {
            self.say(format!("검색 {text}: 결과 없음"), cx);
            return;
        };
        let hit = hits[found.index];
        self.seal_history(cx);
        if self.current_track() != Some(hit.track) {
            self.set_current_track(Some(hit.track), cx);
        }
        let pitch = if hit.has_note {
            hit.pitch
        } else {
            self.pianoroll.cursor().pitch
        };
        self.jump_cursor(
            Cursor {
                tick: hit.tick,
                pitch,
            },
            cx,
        );
        let mut message = format!("검색 {text}: {}/{}", found.index + 1, hits.len());
        if found.wrapped {
            message.push_str(match direction {
                Direction::Forward => " · 끝에서 처음으로 돌아왔다",
                Direction::Backward => " · 처음에서 끝으로 돌아왔다",
            });
        }
        self.say(message, cx);
    }

    /// `search::Next`·`search::Prev`: 지난 검색어로 이동한다(결정 5절).
    fn search_repeat(&mut self, direction: Direction, cx: &mut Context<Self>) {
        if !self.search_allowed(cx) {
            return;
        }
        let Some(text) = self.last_search.clone() else {
            self.clear_operator(cx);
            self.say("검색어가 없다", cx);
            return;
        };
        self.search_jump(&text, direction, cx);
    }

    pub(crate) fn search_next(&mut self, _: &search::Next, _: &mut Window, cx: &mut Context<Self>) {
        self.search_repeat(Direction::Forward, cx);
    }

    pub(crate) fn search_prev(&mut self, _: &search::Prev, _: &mut Window, cx: &mut Context<Self>) {
        self.search_repeat(Direction::Backward, cx);
    }

    /// 입력란의 키를 입력란보다 먼저 본다(결정 1절 6).
    /// - 수식키 없는 `Esc`는 막대를 닫는다(gpui-base 입력란은 `Esc`에서 포커스를 놓지 않는다, `fields.rs`).
    /// - `Enter`·`Shift+Enter`는 여기서 확정한다. 입력란의 `Enter` 처리기는 사건을 낸 뒤 키를 전파해 「처리 안 된
    ///   키」로 남기고, 그러면 줄바꿈 글자가 입력으로 들어가 골라 둔 글자(다시 연 막대의 지난 검색어)를 지운다.
    ///   사건 구독은 그 뒤에 돌아 빈 글자를 읽는다.
    ///
    /// 조합 중인 글자(IME)가 있으면 키는 입력기의 것이라 건드리지 않는다.
    fn search_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = &event.keystroke;
        let m = k.modifiers;
        let composing = self.search_bar.as_ref().is_some_and(|bar| {
            bar.input
                .update(cx, |state, cx| state.marked_text_range(window, cx))
                .is_some()
        });
        if composing {
            return;
        }
        if k.key == "escape" && !m.modified() {
            self.close_search_bar(window, cx);
            cx.stop_propagation();
        } else if k.key == "enter" && !m.control && !m.alt && !m.platform && !m.function {
            let direction = if m.shift {
                Direction::Backward
            } else {
                Direction::Forward
            };
            self.search_confirm(direction, true, window, cx);
            cx.stop_propagation();
        }
    }

    /// 열린 막대. 창 루트의 자식으로, 주 뷰 위 오른쪽에 겹쳐 그린다.
    pub(crate) fn render_search_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let bar = self.search_bar.as_ref()?;
        let status = SharedString::from(bar.status.clone());
        Some(deferred(
            div()
                .id("search-bar")
                .aria_label("검색")
                .test_support()
                .key_context("SearchBar")
                .absolute()
                .top(px(44.))
                .right(px(16.))
                .w(px(360.))
                .p_1()
                .flex()
                .items_center()
                .gap_1()
                .text_sm()
                .bg(cx.theme().popover)
                .border_1()
                .border_color(cx.theme().border)
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                    this.search_key_down(e, window, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&bar.input).id("search-input")),
                )
                .child(
                    div()
                        .id("search-status")
                        .aria_label(status.clone())
                        .test_support()
                        .w(px(80.))
                        .text_color(cx.theme().muted_foreground)
                        .child(status),
                )
                .child(
                    Button::new("search-prev")
                        .label("‹")
                        .ghost()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.search_confirm(Direction::Backward, false, window, cx);
                        })),
                )
                .child(
                    Button::new("search-next")
                        .label("›")
                        .ghost()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.search_confirm(Direction::Forward, false, window, cx);
                        })),
                )
                .child(
                    Button::new("search-close")
                        .label("×")
                        .ghost()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.close_search_bar(window, cx);
                        })),
                ),
        ))
    }
}
