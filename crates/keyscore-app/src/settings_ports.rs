//! 설정 > MIDI 출력의 가상 포트(S2-T6d, OUT-1·2, 7.6).
//!
//! 편집할 포트를 선택 상자로 고르고, 이름 입력란에서 이름을 바꾼다(Enter 확정, Esc 취소). 추가·삭제는 버튼이다.
//! 이름 바꾸기·추가·삭제는 모두 Action(`midi::RenameVirtualPort`·`AddVirtualPort`·`RemoveVirtualPort`)을 거쳐
//! 앱 전역 처리기가 한다. 처리기는 이 구역에서 고른 포트를 대상으로 한다(설정 창이 없으면 이름 바꾸기는
//! 기본 포트, 삭제는 대상 없음).

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::IndexPath;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::searchable_list::SearchableVec;
use gpui_kit::component::select::SelectEvent;
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, TestSupportExt as _, Window, div,
};

use keyscore_core::OutputPort;

use crate::actions::midi;
use crate::app_state::AppSettings;
use crate::engine::{self, PortEditError};
use crate::settings_tabs::{Choice, action_button, choice, choice_items, row};
use crate::settings_window::{self, SettingsWindow};

/// MIDI 출력 구역의 상태.
pub(crate) struct PortsTab {
    select: Entity<Choice>,
    /// 선택 상자 항목(앱 가상 포트 이름, 기본 포트 먼저)
    names: Vec<String>,
    /// 편집할 포트
    selected: Option<String>,
    name: Entity<InputState>,
    /// 이름 입력란에 마지막으로 넣은 글자(고른 포트의 이름)
    name_shown: String,
    add: FocusHandle,
    remove: FocusHandle,
    test: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl PortsTab {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<SettingsWindow>) -> Self {
        let select = choice([], window, cx);
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("포트 이름"));
        let subscriptions = vec![
            cx.subscribe_in(
                &select,
                window,
                |this, state, event: &SelectEvent<SearchableVec<SharedString>>, window, cx| {
                    let SelectEvent::Confirm(Some(_)) = event else {
                        return;
                    };
                    let Some(row) = state.read(cx).selected_index(cx).map(|i| i.row) else {
                        return;
                    };
                    let tab = this.ports_tab_mut();
                    if let Some(name) = tab.names.get(row).cloned() {
                        tab.selected = Some(name);
                        tab.sync_name(window, cx);
                    }
                    cx.notify();
                },
            ),
            cx.subscribe_in(
                &name,
                window,
                |this, state, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        let text = state.read(cx).value().to_string();
                        if Some(&text) != this.ports_tab().selected.as_ref() {
                            window.dispatch_action(Box::new(midi::RenameVirtualPort(text)), cx);
                        }
                    }
                },
            ),
        ];
        PortsTab {
            select,
            names: Vec::new(),
            selected: None,
            name,
            name_shown: String::new(),
            add: cx.focus_handle().tab_stop(true),
            remove: cx.focus_handle().tab_stop(true),
            test: cx.focus_handle().tab_stop(true),
            _subscriptions: subscriptions,
        }
    }

    /// 편집할 포트(시험용 API가 쓴다).
    pub(crate) fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub(crate) fn name_text(&self, cx: &App) -> String {
        self.name.read(cx).value().to_string()
    }

    /// 포커스를 가진 컨트롤의 요소 id.
    pub(crate) fn focused(&self, window: &Window, cx: &App) -> Option<&'static str> {
        if self.test.is_focused(window) {
            Some("settings-port-test")
        } else if self.add.is_focused(window) {
            Some("settings-port-add")
        } else if self.remove.is_focused(window) {
            Some("settings-port-remove")
        } else if self
            .name
            .read(cx)
            .focus_handle(cx)
            .contains_focused(window, cx)
        {
            Some("settings-port-name")
        } else if self
            .select
            .read(cx)
            .focus_handle(cx)
            .contains_focused(window, cx)
        {
            Some("settings-port-select")
        } else {
            None
        }
    }

    pub(crate) fn focus_first(&self, window: &mut Window, cx: &mut App) {
        let handle = self.select.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    pub(crate) fn focus_name(&self, window: &mut Window, cx: &mut App) {
        let handle = self.name.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    /// 이 포트를 고른다(추가 뒤, 이름을 바꾼 뒤).
    pub(crate) fn select_port(&mut self, name: String) {
        self.selected = Some(name);
    }

    /// 이름 입력란을 고른 포트의 이름으로 되돌린다.
    fn sync_name(&mut self, window: &mut Window, cx: &mut App) {
        let text = self.selected.clone().unwrap_or_default();
        self.name
            .update(cx, |s, cx| s.set_value(text.clone(), window, cx));
        self.name_shown = text;
    }

    /// 선택 상자와 이름 입력란을 엔진의 포트 목록에 맞춘다. 그릴 때 부른다.
    fn sync(&mut self, window: &mut Window, cx: &mut App) {
        let names = engine::virtual_port_names(cx);
        if names != self.names {
            let items = choice_items(names.iter().cloned());
            self.select
                .update(cx, |s, cx| s.set_items(items, window, cx));
            self.names = names;
            // 지금 고른 포트가 없어졌으면(지움) 기본 포트로
            if !self
                .selected
                .as_ref()
                .is_some_and(|s| self.names.contains(s))
            {
                self.selected = self.names.first().cloned();
            }
            let row = self
                .selected
                .as_ref()
                .and_then(|s| self.names.iter().position(|n| n == s));
            self.select.update(cx, |s, cx| {
                s.set_selected_index(row.map(IndexPath::new), window, cx);
            });
        }
        if self.selected.is_none() {
            self.selected = self.names.first().cloned();
        }
        if self.selected.as_deref() != Some(self.name_shown.as_str()) {
            let row = self
                .selected
                .as_ref()
                .and_then(|s| self.names.iter().position(|n| n == s));
            self.select.update(cx, |s, cx| {
                s.set_selected_index(row.map(IndexPath::new), window, cx);
            });
            self.sync_name(window, cx);
        }
    }
}

impl SettingsWindow {
    /// MIDI 출력 구역.
    pub(crate) fn render_midi_output(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.ports_tab_mut().sync(window, cx);
        let tab = self.ports_tab();
        let selected = tab.selected.clone().unwrap_or_default();
        let is_main = tab.names.first() == Some(&selected);
        let list = tab.names.join(", ");
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("settings-ports-list")
                    .aria_label(format!("가상 포트: {list}"))
                    .test_support()
                    .child(format!("가상 포트 {}개: {list}", tab.names.len())),
            )
            .child(row(
                "편집할 포트",
                "settings-port-select",
                format!("편집할 포트: {selected}"),
                &tab.select,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(gpui_kit::px(120.)).child("이름"))
                    .child(
                        div()
                            .id("settings-port-name")
                            .aria_label(format!("이름: {}", tab.name_text(cx)))
                            .test_support()
                            .w(gpui_kit::px(260.))
                            // 수식키 없는 Esc는 친 글자를 버리고 지금 이름으로 되돌린다(A13 통상의 취소)
                            .capture_key_down(cx.listener(
                                |this, event: &KeyDownEvent, window, cx| {
                                    let k = &event.keystroke;
                                    if k.key == "escape" && !k.modifiers.modified() {
                                        this.ports_tab_mut().sync_name(window, cx);
                                        cx.stop_propagation();
                                    }
                                },
                            ))
                            .child(Input::new(&tab.name)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(action_button(
                        "settings-port-add",
                        "가상 포트 추가",
                        &tab.add,
                        false,
                        || Box::new(midi::AddVirtualPort),
                        window,
                        cx,
                    ))
                    .child(action_button(
                        "settings-port-remove",
                        "삭제",
                        &tab.remove,
                        is_main,
                        || Box::new(midi::RemoveVirtualPort),
                        window,
                        cx,
                    )),
            )
            .child(div().text_color(cx.theme().muted_foreground).child(
                "외부 신디는 이 이름의 MIDI 입력을 고른다. 이름을 바꿔도 연결은 유지되고, 열린 프로젝트의 \
                 트랙도 새 이름을 따른다. 기본 포트는 지울 수 없다(최대 16개).",
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(action_button(
                        "settings-port-test",
                        "테스트 노트 보내기",
                        &tab.test,
                        false,
                        || Box::new(midi::SendTestNote),
                        window,
                        cx,
                    ))
                    .child(div().text_color(cx.theme().muted_foreground).child(
                        "고른 포트로 채널 1의 가운데 C를 보낸다. 신디에서 들리는지 확인한다(PRE-3).",
                    )),
            )
            .child(port_status(&tab.names, cx))
            .child(render_clock_list(cx))
    }

    /// 설정 창에서 낸 `midi::SendTestNote`: 고른 가상 포트로 테스트 노트를 보낸다(프로젝트 창에서는 현재
    /// 트랙으로 보낸다).
    pub(crate) fn ports_test_note(
        &mut self,
        _: &midi::SendTestNote,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .ports_tab()
            .selected()
            .map_or_else(|| engine::virtual_port_name(cx), str::to_owned);
        let message = match engine::send_port_test_note(cx, &target) {
            Ok(()) => format!("테스트 노트를 보냈다: {target} ch1. 신디에서 들리는지 확인한다"),
            Err(reason) => reason,
        };
        self.set_message(message);
        cx.notify();
    }
}

/// 포트 상태(PRE-2). 가상 포트에 리스너가 붙었는지는 공개 API로 알 수 없어(SP-4) 앱이 아는 것만 보인다.
fn port_status(names: &[String], cx: &App) -> impl IntoElement {
    let last = match engine::last_test_note(cx) {
        Some((at, port)) => {
            let ago = at.elapsed().map_or(0, |d| d.as_secs());
            format!("「{port}」, {ago}초 전")
        }
        None => "없음".to_owned(),
    };
    let main = names.first().cloned().unwrap_or_default();
    let text = format!(
        "가상 포트 {}개 활성 · 마지막 테스트 노트: {last} · 들리지 않으면 신디의 MIDI 입력에서 「{main}」을 고른다",
        names.len()
    );
    div()
        .id("settings-port-status")
        .aria_label(text.clone())
        .test_support()
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

/// 포트별 MIDI Clock(OUT-5): 포트 선택지마다 체크 상자 하나. 켜 둔 포트가 지금 없으면 그것도 보인다.
fn render_clock_list(cx: &App) -> impl IntoElement {
    let main = engine::virtual_port_name(cx);
    let on = crate::transport::clock_settings(cx);
    let mut ports = engine::port_choices(cx);
    for port in &on {
        if !ports.contains(port) {
            ports.push(port.clone());
        }
    }
    let rows = ports.into_iter().map(|port| {
        let name = match &port {
            OutputPort::Virtual => main.clone(),
            OutputPort::Named(n) => n.clone(),
        };
        let checked = on.contains(&port);
        let id = format!("settings-clock-{name}");
        div()
            .id(SharedString::from(id.clone()))
            .aria_label(format!("{name}: {}", if checked { "켬" } else { "끔" }))
            .test_support()
            .child(
                Checkbox::new(SharedString::from(format!("{id}-box")))
                    .label(name)
                    .checked(checked)
                    .on_click(move |_, _, cx| toggle_clock(cx, &port)),
            )
    });
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child("MIDI Clock(24 ppqn, Start/Stop/Continue/SPP)")
        .children(rows)
}

/// 그 포트의 클럭을 켜거나 끈다. 재생 중이면 곧바로 반영한다.
fn toggle_clock(cx: &mut App, port: &OutputPort) {
    AppSettings::update(cx, |s| {
        let clock = &mut s.midi_output.clock;
        if let Some(i) = clock.iter().position(|p| p == port) {
            clock.remove(i);
        } else {
            clock.push(port.clone());
        }
    });
    engine::clock_changed(cx);
    cx.refresh_windows();
}

/// 처리기의 결과를 설정 창(열려 있으면) 또는 열린 프로젝트 창에 알린다.
fn report(cx: &mut App, message: String) {
    if let Some((_, view)) = settings_window::current(cx) {
        view.update(cx, |this, cx| {
            this.set_message(message);
            cx.notify();
        });
    } else {
        crate::window::say_everywhere(cx, &message);
    }
}

/// 설정 창에서 고른 포트.
fn selected_in_settings(cx: &App) -> Option<String> {
    let (_, view) = settings_window::current(cx)?;
    view.read(cx).ports_tab().selected().map(str::to_owned)
}

fn describe(e: &PortEditError) -> String {
    match e {
        PortEditError::Midi(keyscore_midi::MidiError::InvalidName) => {
            "포트 이름으로 쓸 수 없다(빈 이름, 제어 문자 등)".to_owned()
        }
        PortEditError::Midi(keyscore_midi::MidiError::DuplicateName(n)) => {
            format!("「{n}」은 다른 가상 포트와 이름이 같다")
        }
        PortEditError::Midi(keyscore_midi::MidiError::TooManyPorts) => {
            format!("가상 포트는 {}개까지다", keyscore_midi::MAX_VIRTUAL_OUTPUTS)
        }
        other => other.to_string(),
    }
}

/// 앱 전역 처리기(`midi::AddVirtualPort`·`RenameVirtualPort`·`RemoveVirtualPort`).
pub(crate) fn register(cx: &mut App) {
    cx.on_action(
        |_: &midi::AddVirtualPort, cx| match engine::add_virtual_port(cx) {
            Ok(name) => {
                report(cx, format!("가상 포트 「{name}」을 만들었다"));
                // 설정 창이 열려 있으면 그 포트를 고르고 이름 입력란으로 간다. 설정 창 안에서 낸 Action이면 그 창을
                // 갱신하는 중이라 지금은 창을 빌릴 수 없다. 뒤로 미룬다.
                if let Some((handle, view)) = settings_window::current(cx) {
                    view.update(cx, |this, _| this.ports_tab_mut().select_port(name));
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            view.update(cx, |this, cx| {
                                this.ports_tab_mut().sync(window, cx);
                                this.ports_tab().focus_name(window, cx);
                                cx.notify();
                            });
                        });
                    });
                }
            }
            Err(e) => report(cx, format!("가상 포트를 만들지 못했다: {}", describe(&e))),
        },
    );
    cx.on_action(|action: &midi::RenameVirtualPort, cx| {
        let target = selected_in_settings(cx).unwrap_or_else(|| engine::virtual_port_name(cx));
        match engine::rename_virtual_port(cx, &target, &action.0) {
            Ok(name) => {
                if let Some((_, view)) = settings_window::current(cx) {
                    view.update(cx, |this, _| this.ports_tab_mut().select_port(name.clone()));
                }
                report(
                    cx,
                    format!("가상 포트 「{target}」의 이름을 「{name}」으로 바꿨다"),
                );
            }
            Err(e) => report(
                cx,
                format!(
                    "가상 포트 「{target}」의 이름을 바꾸지 못했다: {}",
                    describe(&e)
                ),
            ),
        }
    });
    cx.on_action(|_: &midi::RemoveVirtualPort, cx| {
        let Some(target) = selected_in_settings(cx) else {
            settings_window::open(cx, Some(crate::actions::args::SettingsSection::MidiOutput));
            report(cx, "설정 > MIDI 출력에서 지울 포트를 고른다".to_owned());
            return;
        };
        match engine::remove_virtual_port(cx, &target) {
            Ok(()) => report(cx, format!("가상 포트 「{target}」을 지웠다")),
            Err(e) => report(
                cx,
                format!("가상 포트 「{target}」을 지우지 못했다: {}", describe(&e)),
            ),
        }
    });
}
