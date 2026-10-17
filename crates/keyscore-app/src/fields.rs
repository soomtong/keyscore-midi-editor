//! 창의 한 줄 입력란(트랜스포트의 템포·박자표, 인스펙터의 이름·뱅크·기본 벨로시티).
//!
//! 규칙은 모든 입력란이 같다(S2-T5b 리뷰 1–3, S2-T5c).
//! - 앱이 마지막으로 넣은 글자(`shown`)와 지금 글자가 다르면 「사람이 고친 필드」다.
//! - Enter는 고친 필드만 확정한다. 값이 문서와 같으면 편집을 만들지 않는다.
//! - 수식키 없는 `Esc`와 창 안의 다른 곳으로 포커스가 옮겨 가는 것은 취소다.
//! - 창이 비활성이 되어 포커스를 잃는 것은 취소가 아니다(macOS 관례: 다른 앱에 다녀와도 글자가 남는다).
//! - 문서가 바뀌면 고치지 않은 필드는 문서 값을 따라가고, 고친 필드는 글자를 두고 기준만 옮긴다.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AppContext as _, Context, Entity, Focusable as _, KeyDownEvent, Subscription, Window,
};
use keyscore_core::{Tick, TrackId};

use crate::tracks::{format_meter, format_tempo};
use crate::window::ProjectWindow;

/// 창의 입력란.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Tempo,
    Meter,
    Name,
    Bank,
    Velocity,
}

impl Field {
    pub(crate) const ALL: [Field; 5] = [
        Field::Tempo,
        Field::Meter,
        Field::Name,
        Field::Bank,
        Field::Velocity,
    ];

    /// 현재 트랙을 보이는 필드(인스펙터).
    pub(crate) const fn follows_track(self) -> bool {
        matches!(self, Field::Name | Field::Bank | Field::Velocity)
    }

    const fn index(self) -> usize {
        match self {
            Field::Tempo => 0,
            Field::Meter => 1,
            Field::Name => 2,
            Field::Bank => 3,
            Field::Velocity => 4,
        }
    }
}

/// 입력란 상태.
pub(crate) struct Fields {
    states: [Entity<InputState>; 5],
    /// 필드마다 마지막으로 앱이 넣은 글자. 입력란의 변경 사건은 값을 넣은 뒤에도 늦게 올 수 있어 사건으로
    /// 세지 않는다.
    shown: [String; 5],
    /// 인스펙터 필드가 지금 보이는 트랙. 현재 트랙이 바뀌면 인스펙터 필드를 새 트랙 값으로 맞춘다.
    track: Option<TrackId>,
    _subscriptions: Vec<Subscription>,
}

impl Fields {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ProjectWindow>) -> Self {
        let states = Field::ALL.map(|_| cx.new(|cx| InputState::new(window, cx)));
        let subscriptions = Field::ALL
            .iter()
            .zip(&states)
            .map(|(&field, state)| {
                cx.subscribe_in(
                    state,
                    window,
                    move |this, _, event: &InputEvent, window, cx| {
                        this.field_event(field, event, window, cx);
                    },
                )
            })
            .collect();
        Fields {
            states,
            shown: Default::default(),
            track: None,
            _subscriptions: subscriptions,
        }
    }
}

impl ProjectWindow {
    pub(crate) fn field_state(&self, field: Field) -> &Entity<InputState> {
        &self.fields.states[field.index()]
    }

    /// 문서의 지금 값을 필드 표기로.
    fn field_document_text(&self, field: Field, cx: &gpui_kit::App) -> String {
        let project = self.document.read(cx).project();
        let track = self
            .current_track()
            .and_then(|id| project.tracks().iter().find(|t| t.id() == id));
        match field {
            Field::Tempo => format_tempo(project.tempo_map().tempo_at(Tick::ZERO)),
            Field::Meter => format_meter(project.meter_map().signature_at(Tick::ZERO)),
            Field::Name => track.map(|t| t.name().to_owned()).unwrap_or_default(),
            Field::Bank => track
                .and_then(keyscore_core::Track::bank)
                .map(|b| b.get().to_string())
                .unwrap_or_default(),
            Field::Velocity => track
                .map(|t| t.default_velocity().to_midi1().to_string())
                .unwrap_or_default(),
        }
    }

    /// 필드를 문서 값으로 되돌리고 「고치지 않음」으로 둔다.
    fn reset_field(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.field_document_text(field, cx);
        self.field_state(field)
            .clone()
            .update(cx, |state, cx| state.set_value(text.clone(), window, cx));
        self.fields.shown[field.index()] = text;
    }

    /// 사람이 고친 필드인가.
    fn field_dirty(&self, field: Field, cx: &gpui_kit::App) -> bool {
        self.field_state(field).read(cx).value().as_ref() != self.fields.shown[field.index()]
    }

    /// 모든 필드를 문서 값으로(창을 열 때).
    pub(crate) fn refresh_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fields.track = self.current_track();
        for field in Field::ALL {
            self.reset_field(field, window, cx);
        }
    }

    /// 현재 트랙이 바뀌었으면 인스펙터 필드를 새 트랙 값으로 맞춘다(고치던 글자는 버린다: 다른 트랙의
    /// 값으로 확정되면 안 된다).
    pub(crate) fn follow_current_track(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fields.track == self.current_track() {
            return;
        }
        self.fields.track = self.current_track();
        for field in Field::ALL.into_iter().filter(|f| f.follows_track()) {
            self.reset_field(field, window, cx);
        }
    }

    /// 문서가 바뀌었다(undo, 탭 템포, Action). 고치지 않은 필드를 문서 값으로 맞춘다. 고친 필드는 확정·취소할
    /// 때까지 글자를 건드리지 않고 기준만 새 문서 값으로 옮긴다. 옛 기준과 같은 글자를 친 확정이 「고치지
    /// 않음」으로 버려지지 않게 하기 위해서다(S2-T5b 리뷰 3 제안 4).
    pub(crate) fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_current_track(window, cx);
        for field in Field::ALL {
            if self.field_dirty(field, cx) {
                self.fields.shown[field.index()] = self.field_document_text(field, cx);
            } else {
                self.reset_field(field, window, cx);
            }
        }
    }

    /// 창의 입력란에 포커스가 있는가(모드가 바뀔 때 고치던 입력을 버리려고 본다).
    pub(crate) fn field_focused(&self, window: &Window, cx: &gpui_kit::App) -> bool {
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
    pub(crate) fn field_key_down(
        &mut self,
        field: Field,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = &event.keystroke;
        if k.key == "escape" && !k.modifiers.modified() {
            self.reset_field(field, window, cx);
            self.focus_document(window, cx);
            cx.stop_propagation();
        }
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
                    let edit = if field.follows_track() {
                        self.inspector_field_edit(field, &text, cx)
                    } else {
                        self.transport_field_edit(field, &text, cx)
                    };
                    match edit {
                        Ok(Some(edit)) => {
                            self.edit_document(vec![edit], cx);
                        }
                        Ok(None) => {}
                        Err(message) => self.say(message, cx),
                    }
                }
                self.reset_field(field, window, cx);
                self.focus_document(window, cx);
            }
            // 창이 비활성이 되어 잃은 포커스는 돌아오면 그대로 이어 친다(S2-T5b 리뷰 3 제안 5).
            InputEvent::Blur if window.is_window_active() => self.reset_field(field, window, cx),
            InputEvent::Blur | InputEvent::Change | InputEvent::Focus => {}
        }
    }
}
