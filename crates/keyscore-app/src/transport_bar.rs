//! 트랜스포트 바(S2-T5b, 7.1 상단, TRN-1·2·4, 7.6).
//!
//! 버튼은 해당 Action을 실행 함수로 낸다(결정 C). 템포·박자표 필드와 조성·포트 선택은 문서 편집이고 각각
//! undo 한 단위다. TRN-4에 따라 템포·박자표·조성은 곡 전체에 하나(틱 0)다. 템포·박자표 필드는 세로로 끌어
//! 조정한다(S3-T6c, `transport_drag.rs`).

use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle};
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, Disableable as _};
use gpui_kit::{
    Action, ClickEvent, Context, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseDownEvent, ParentElement as _, Pixels, Point, StatefulInteractiveElement as _,
    Styled as _, TestSupportExt as _, Window, div, px,
};
use keyscore_core::{Edit, OutputPort, Tempo, Tick, TrackProp};
use keyscore_midi::HostTime;

use crate::actions::{ai, mode, transport};
use crate::dispatch::run_action;
use crate::engine::TransportState;
use crate::fields::Field;
use crate::tracks::{
    format_position, key_choices, key_label, parse_meter, parse_tempo, port_label,
};
use crate::transport_drag::{FieldBox, FieldDrag, field_box_recorder};
use crate::window::{PopupKind, ProjectWindow};

/// 탭 템포: 이만큼 쉬면 새로 센다.
const TAP_RESET: Duration = Duration::from_secs(2);
/// 탭 템포: 평균에 쓰는 최근 탭 수.
const TAP_WINDOW: usize = 8;

/// 트랜스포트 바의 상태(입력란은 `fields`가 가진다).
pub(crate) struct TransportBar {
    /// 최근 탭 시각.
    taps: Vec<HostTime>,
    /// 마지막으로 연 포트 드롭다운의 선택지.
    pub(crate) port_choices: Vec<OutputPort>,
    /// 진행 중 필드 끌기(S3-T6c).
    pub(crate) drag: Option<FieldDrag>,
    /// 템포·박자표 필드 상자(그리기 때 기록).
    pub(crate) tempo_box: FieldBox,
    pub(crate) meter_box: FieldBox,
}

impl TransportBar {
    pub(crate) fn new() -> Self {
        TransportBar {
            taps: Vec::new(),
            port_choices: Vec::new(),
            drag: None,
            tempo_box: FieldBox::default(),
            meter_box: FieldBox::default(),
        }
    }
}

impl ProjectWindow {
    /// 템포·박자표 필드를 확정할 편집. 값이 문서와 같으면 `None`(빈 undo 단위가 생기지 않게, 리뷰 2
    /// 지적 5).
    pub(crate) fn transport_field_edit(
        &self,
        field: Field,
        text: &str,
        cx: &gpui_kit::App,
    ) -> Result<Option<Edit>, String> {
        let project = self.document.read(cx).project();
        match field {
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
            _ => Ok(None),
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
            let tempo = self.field_state(Field::Tempo).clone();
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
    pub(crate) fn open_port_menu(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        // 기본 가상 포트, 추가 가상 포트(S2-T6d), 목적지
        let mut choices = crate::engine::port_choices(cx);
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
            .relative()
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
                // 누르고 세로로 끌면 값 조정, 그대로 떼면 입력(7.6, S3-T6c)
                div()
                    .relative()
                    .w(px(72.))
                    .capture_any_mouse_down(cx.listener(|this, e: &MouseDownEvent, window, cx| {
                        this.transport_field_press(Field::Tempo, e, window, cx);
                    }))
                    .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                        this.field_key_down(Field::Tempo, e, window, cx);
                    }))
                    .child(Input::new(self.field_state(Field::Tempo)).id("transport-tempo"))
                    .child(field_box_recorder(&self.transport_bar.tempo_box)),
            )
            .child("BPM")
            .child(
                div()
                    .relative()
                    .w(px(56.))
                    .capture_any_mouse_down(cx.listener(|this, e: &MouseDownEvent, window, cx| {
                        this.transport_field_press(Field::Meter, e, window, cx);
                    }))
                    .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                        this.field_key_down(Field::Meter, e, window, cx);
                    }))
                    .child(Input::new(self.field_state(Field::Meter)).id("transport-meter"))
                    .child(field_box_recorder(&self.transport_bar.meter_box)),
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
            .child(self.transport_drag_listener(cx))
    }
}
