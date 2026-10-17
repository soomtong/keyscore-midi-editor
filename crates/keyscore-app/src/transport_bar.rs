//! 트랜스포트 바(S2-T5b, 7.1 상단, TRN-1·2·4, 7.6).
//!
//! 버튼은 해당 Action을 실행 함수로 낸다(결정 C). 템포·박자표 필드와 조성·포트 선택은 문서 편집이고 각각
//! undo 한 단위다. TRN-4에 따라 템포·박자표·조성은 곡 전체에 하나(틱 0)다.

use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _};
use gpui_kit::{
    Action, AppContext as _, ClickEvent, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Pixels, Point, StatefulInteractiveElement as _,
    Styled as _, Subscription, TestSupportExt as _, Window, div, px,
};
use keyscore_core::{Edit, OutputPort, Tempo, Tick, TrackProp};
use keyscore_midi::HostTime;

use crate::actions::{ai, mode, transport};
use crate::dispatch::run_action;
use crate::engine::TransportState;
use crate::tracks::{
    format_meter, format_position, format_tempo, key_choices, key_label, parse_meter, parse_tempo,
    port_label,
};
use crate::window::{PopupKind, ProjectWindow};

/// 탭 템포: 이만큼 쉬면 새로 센다.
const TAP_RESET: Duration = Duration::from_secs(2);
/// 탭 템포: 평균에 쓰는 최근 탭 수.
const TAP_WINDOW: usize = 8;

/// 트랜스포트 바의 입력 상태.
pub(crate) struct TransportBar {
    pub(crate) tempo: Entity<InputState>,
    pub(crate) meter: Entity<InputState>,
    /// 최근 탭 시각.
    taps: Vec<HostTime>,
    /// 마지막으로 연 포트 드롭다운의 선택지.
    pub(crate) port_choices: Vec<OutputPort>,
    /// 필드마다 마지막으로 앱이 넣은 글자. 지금 글자가 이것과 다르면 사람이 고친 필드다. 고친 필드만
    /// 문서가 바뀌어도 덮지 않고, 고치지 않은 필드는 Enter로 편집을 만들지 않는다(리뷰 2 지적 3·5).
    /// 입력란의 변경 사건은 값을 넣은 뒤에도 늦게 올 수 있어 사건으로 세지 않는다.
    shown: [String; 2],
    _subscriptions: [Subscription; 2],
}

/// 트랜스포트 바의 입력란.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Tempo,
    Meter,
}

impl Field {
    const ALL: [Field; 2] = [Field::Tempo, Field::Meter];

    const fn index(self) -> usize {
        match self {
            Field::Tempo => 0,
            Field::Meter => 1,
        }
    }
}

impl TransportBar {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ProjectWindow>) -> Self {
        let tempo = cx.new(|cx| InputState::new(window, cx));
        let meter = cx.new(|cx| InputState::new(window, cx));
        let subscriptions = [
            cx.subscribe_in(&tempo, window, |this, _, event: &InputEvent, window, cx| {
                this.field_event(Field::Tempo, event, window, cx);
            }),
            cx.subscribe_in(&meter, window, |this, _, event: &InputEvent, window, cx| {
                this.field_event(Field::Meter, event, window, cx);
            }),
        ];
        TransportBar {
            tempo,
            meter,
            taps: Vec::new(),
            port_choices: Vec::new(),
            shown: [String::new(), String::new()],
            _subscriptions: subscriptions,
        }
    }
}

impl ProjectWindow {
    pub(crate) fn field_state(&self, field: Field) -> &Entity<InputState> {
        match field {
            Field::Tempo => &self.transport_bar.tempo,
            Field::Meter => &self.transport_bar.meter,
        }
    }

    /// 문서의 지금 값을 필드 표기로.
    fn field_document_text(&self, field: Field, cx: &gpui_kit::App) -> String {
        let project = self.document.read(cx).project();
        match field {
            Field::Tempo => format_tempo(project.tempo_map().tempo_at(Tick::ZERO)),
            Field::Meter => format_meter(project.meter_map().signature_at(Tick::ZERO)),
        }
    }

    /// 필드를 문서 값으로 되돌리고 「고치지 않음」으로 둔다.
    fn reset_field(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.field_document_text(field, cx);
        self.field_state(field)
            .clone()
            .update(cx, |state, cx| state.set_value(text.clone(), window, cx));
        self.transport_bar.shown[field.index()] = text;
    }

    /// 사람이 고친 필드인가.
    fn field_dirty(&self, field: Field, cx: &gpui_kit::App) -> bool {
        self.field_state(field).read(cx).value().as_ref() != self.transport_bar.shown[field.index()]
    }

    /// 모든 필드를 문서 값으로(창을 열 때).
    pub(crate) fn refresh_transport_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for field in Field::ALL {
            self.reset_field(field, window, cx);
        }
    }

    /// 문서가 바뀌었다(undo, 탭 템포, `SetTempo`). 사람이 고치지 않은 필드를 문서 값으로 맞춘다. 고친 필드는
    /// 확정·취소할 때까지 글자를 건드리지 않는다(리뷰 1 지적 2, 리뷰 2 지적 3, 제안 2: 필드마다 따로 본다).
    /// 고친 필드의 기준은 새 문서 값으로 옮긴다. 옛 기준과 같은 글자를 친 확정이 「고치지 않음」으로
    /// 버려지지 않게 하기 위해서다(리뷰 3 제안 4).
    pub(crate) fn sync_transport_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for field in Field::ALL {
            if self.field_dirty(field, cx) {
                self.transport_bar.shown[field.index()] = self.field_document_text(field, cx);
            } else {
                self.reset_field(field, window, cx);
            }
        }
    }

    /// 트랜스포트 입력란에 포커스가 있는가(모드가 바뀔 때 고치던 입력을 버리려고 본다).
    pub(crate) fn transport_field_focused(&self, window: &Window, cx: &gpui_kit::App) -> bool {
        Field::ALL.iter().any(|f| {
            self.field_state(*f)
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        })
    }

    /// 고치던 필드를 모두 취소한다(모드가 바뀌어 문서로 돌아갈 때).
    pub(crate) fn cancel_open_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for field in Field::ALL {
            self.reset_field(field, window, cx);
        }
    }

    /// 입력란의 키를 입력란보다 먼저 본다. 수식키 없는 `Esc`만 가로채 입력을 취소한다(gpui-base 입력란은
    /// `Esc`에서 포커스를 놓지 않는다).
    fn field_key_down(
        &mut self,
        field: Field,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = &event.keystroke;
        if k.key == "escape" && !k.modifiers.modified() {
            self.cancel_field(field, window, cx);
            cx.stop_propagation();
        }
    }

    /// `Esc`: 입력을 취소하고 문서로 돌아간다(A13 「통상의 취소」, 리뷰 2 지적 4).
    pub(crate) fn cancel_field(
        &mut self,
        field: Field,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reset_field(field, window, cx);
        self.focus_document(window, cx);
    }

    fn field_event(
        &mut self,
        field: Field,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::PressEnter { .. } => {
                if self.field_dirty(field, cx) {
                    let text = self.field_state(field).read(cx).value().to_string();
                    self.commit_field(field, &text, cx);
                }
                self.reset_field(field, window, cx);
                self.focus_document(window, cx);
            }
            InputEvent::Blur => self.reset_field(field, window, cx),
            InputEvent::Change | InputEvent::Focus => {}
        }
    }

    /// 고친 필드를 확정한다. 값이 문서와 같으면 편집을 만들지 않는다(빈 undo 단위가 생기지 않게, 리뷰 2
    /// 지적 5).
    fn commit_field(&mut self, field: Field, text: &str, cx: &mut Context<Self>) {
        let project = self.document.read(cx).project();
        let edit = match field {
            Field::Tempo => parse_tempo(text).map(|tempo| {
                (tempo != project.tempo_map().tempo_at(Tick::ZERO)).then_some(Edit::SetTempo {
                    at: Tick::ZERO,
                    tempo: Some(tempo),
                })
            }),
            Field::Meter => parse_meter(text).map(|signature| {
                (signature != project.meter_map().signature_at(Tick::ZERO)).then_some(
                    Edit::SetMeter {
                        at: Tick::ZERO,
                        signature: Some(signature),
                    },
                )
            }),
        };
        match edit {
            Ok(Some(edit)) => {
                self.edit_document(vec![edit], cx);
            }
            Ok(None) => {}
            Err(message) => self.say(message, cx),
        }
    }

    fn set_song_tempo(&mut self, tempo: Tempo, cx: &mut Context<Self>) {
        if self
            .document
            .read(cx)
            .project()
            .tempo_map()
            .tempo_at(Tick::ZERO)
            == tempo
        {
            return;
        }
        self.edit_document(
            vec![Edit::SetTempo {
                at: Tick::ZERO,
                tempo: Some(tempo),
            }],
            cx,
        );
    }

    /// `transport::SetTempo`: 값이 있으면 그 템포로, 없으면 템포 필드에 포커스.
    pub(crate) fn transport_set_tempo(
        &mut self,
        action: &transport::SetTempo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(bpm) = action.0.0 {
            match parse_tempo(&format!("{:.1}", bpm.0)) {
                Ok(tempo) => self.set_song_tempo(tempo, cx),
                Err(message) => self.say(message, cx),
            }
        } else {
            // 필드에 들어가 내용을 전체 선택한다. 바로 새 값을 칠 수 있다.
            let tempo = self.transport_bar.tempo.clone();
            tempo.update(cx, |state, cx| {
                state.focus(window, cx);
                state.select_all(window, cx);
            });
        }
    }

    /// `transport::TapTempo`: 최근 탭 간격의 평균으로 템포를 정한다.
    pub(crate) fn transport_tap_tempo(
        &mut self,
        _: &transport::TapTempo,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = HostTime::now();
        let taps = &mut self.transport_bar.taps;
        if taps
            .last()
            .is_some_and(|last| now.duration_since(*last) > TAP_RESET)
        {
            taps.clear();
        }
        taps.push(now);
        if taps.len() > TAP_WINDOW {
            taps.remove(0);
        }
        if taps.len() < 2 {
            return;
        }
        let span = now.duration_since(taps[0]);
        let intervals = u32::try_from(taps.len() - 1).unwrap_or(1);
        let average = span / intervals;
        let bpm = 60.0 / average.as_secs_f64();
        match parse_tempo(&format!("{bpm:.1}")) {
            Ok(tempo) => self.set_song_tempo(tempo, cx),
            Err(message) => self.say(message, cx),
        }
    }

    /// 조성 메뉴(TRN-4).
    fn open_key_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.document.read(cx).project().key();
        let me = cx.entity().downgrade();
        self.open_popup(PopupKind::Key, at, window, cx, move |mut menu| {
            for key in key_choices() {
                let me = me.clone();
                menu = menu.item(
                    gpui_kit::component::menu::PopupMenuItem::new(key_label(key))
                        .checked(key == current)
                        .on_click(move |_, _, cx| {
                            let _ = me.update(cx, |this, cx| {
                                // 같은 값을 다시 고르면 되돌리기 단계를 만들지 않는다(리뷰 3 지적 3)
                                if this.document.read(cx).project().key() != key {
                                    this.edit_document(vec![Edit::SetKey(key)], cx);
                                }
                            });
                        }),
                );
            }
            menu
        });
    }

    /// 출력 포트 메뉴: 현재 트랙의 포트(7.6 「트랜스포트 오른쪽 포트 드롭다운」).
    fn open_port_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(track) = self.current_track() else {
            return;
        };
        let current = self
            .document
            .read(cx)
            .project()
            .tracks()
            .iter()
            .find(|t| t.id() == track)
            .map(|t| t.port().clone());
        let mut choices = vec![OutputPort::Virtual];
        choices.extend(
            keyscore_midi::destinations()
                .into_iter()
                .map(|d| OutputPort::Named(d.name().to_owned())),
        );
        // 지금 포트가 목록에 없으면(포트가 사라짐) 그대로 보이게 넣는다
        if let Some(port) = current.clone().filter(|p| !choices.contains(p)) {
            choices.push(port);
        }
        self.transport_bar.port_choices.clone_from(&choices);
        let virtual_name = crate::engine::virtual_port_name(cx);
        let me = cx.entity().downgrade();
        self.open_popup(PopupKind::Port, at, window, cx, move |mut menu| {
            for port in choices {
                let me = me.clone();
                let checked = current.as_ref() == Some(&port);
                let label = port_label(&port, &virtual_name);
                menu = menu.item(
                    gpui_kit::component::menu::PopupMenuItem::new(label)
                        .checked(checked)
                        .on_click(move |_, _, cx| {
                            let port = port.clone();
                            let _ = me.update(cx, |this, cx| {
                                // 같은 포트를 다시 고르면 되돌리기 단계를 만들지 않는다(리뷰 3 지적 3)
                                let same = this
                                    .document
                                    .read(cx)
                                    .project()
                                    .tracks()
                                    .iter()
                                    .any(|t| t.id() == track && *t.port() == port);
                                if same {
                                    return;
                                }
                                this.edit_document(
                                    vec![Edit::SetTrackProp {
                                        track,
                                        prop: TrackProp::Port(port),
                                    }],
                                    cx,
                                );
                            });
                        }),
                );
            }
            menu
        });
    }

    // 트랜스포트 바의 항목 순서(7.1)를 한곳에서 읽히게 나누지 않는다.
    #[allow(clippy::too_many_lines)]
    /// 트랜스포트 바. 왼쪽부터 재생·정지·녹음, 템포, 박자표, 조성, 위치, 메트로놈·탭, `AI✦`, 출력 포트.
    pub(crate) fn render_transport_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let project = self.document.read(cx).project();
        let key = key_label(project.key());
        let position = format_position(project, self.transport.position());
        let current_port = self.current_track().and_then(|id| {
            project
                .tracks()
                .iter()
                .find(|t| t.id() == id)
                .map(|t| t.port().clone())
        });
        let virtual_name = crate::engine::virtual_port_name(cx);
        let port = current_port
            .as_ref()
            .map_or_else(|| "트랙 없음".to_owned(), |p| port_label(p, &virtual_name));
        let playing = self.transport.state == TransportState::Playing;
        if playing {
            // 재생 중 위치 표시를 계속 갱신한다(UI-4는 피아노롤 재생 헤드와 함께 S3).
            window.request_animation_frame();
        }
        // 버튼을 누른 뒤 포커스를 문서로 돌린다. 트랜스포트 바는 모드 키 컨텍스트 밖이라 포커스가 버튼에
        // 남으면 모드 키가 먹지 않는다(리뷰 2 지적 1).
        let doc = self.document_focus();
        let action_button = |id: &'static str, label: &'static str, action: Box<dyn Action>| {
            let doc = doc.clone();
            Button::new(id)
                .label(label)
                .ghost()
                .on_click(move |_: &ClickEvent, window, cx| {
                    run_action(action.boxed_clone(), window, cx);
                    window.focus(&doc, cx);
                })
        };
        div()
            .id("transport")
            .test_support()
            .size_full()
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .text_sm()
            .child(action_button(
                "transport-play",
                "▶",
                Box::new(transport::Play),
            ))
            .child(action_button(
                "transport-stop",
                "■",
                Box::new(transport::Stop),
            ))
            // 패닉(OUT-6, 7.6 「트랜스포트의 ⏹! 버튼」)
            .child(action_button(
                "transport-panic",
                "⏹!",
                Box::new(transport::Panic),
            ))
            .child(
                // 녹음(Sequence 모드)은 S5까지 꺼짐
                action_button("transport-record", "⏺", Box::new(mode::Sequence)).disabled(true),
            )
            .child(
                div()
                    .w(px(72.))
                    .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                        this.field_key_down(Field::Tempo, e, window, cx);
                    }))
                    .child(Input::new(&self.transport_bar.tempo).id("transport-tempo")),
            )
            .child("BPM")
            .child(
                div()
                    .w(px(56.))
                    .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                        this.field_key_down(Field::Meter, e, window, cx);
                    }))
                    .child(Input::new(&self.transport_bar.meter).id("transport-meter")),
            )
            .child(
                Button::new("transport-key")
                    .label(key.clone())
                    .accessibility_label(key)
                    .ghost()
                    .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                        this.open_key_menu(event.position(), window, cx);
                    })),
            )
            .child(
                div()
                    .id("transport-position")
                    .aria_label(position.clone())
                    .test_support()
                    .min_w(px(72.))
                    .text_color(cx.theme().muted_foreground)
                    .child(position),
            )
            .child(
                Toggle::new("transport-metronome")
                    .label("메트로놈")
                    .checked(self.transport.metronome)
                    .on_click({
                        let doc = doc.clone();
                        move |_, window, cx| {
                            run_action(Box::new(transport::ToggleMetronome), window, cx);
                            window.focus(&doc, cx);
                        }
                    }),
            )
            .child(action_button(
                "transport-tap",
                "탭",
                Box::new(transport::TapTempo),
            ))
            .child(
                // AI 어시스트는 S7까지 꺼짐
                action_button("transport-ai", "AI✦", Box::new(ai::ToggleAssist)).disabled(true),
            )
            .child(div().flex_1())
            .child(
                Button::new("transport-port")
                    .label(port.clone())
                    .accessibility_label(port)
                    .ghost()
                    .disabled(current_port.is_none())
                    .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                        this.open_port_menu(event.position(), window, cx);
                    })),
            )
    }
}
