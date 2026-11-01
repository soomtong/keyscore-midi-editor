//! 트랜스포트 템포·박자표 필드의 끌어서 조정(S3-T6c, REQ 7.6 「클릭해 입력, 끌어서 조정」). 규칙은
//! `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절, 구현 단위는 `docs/reports/s3-t6c/plan.md`.
//!
//! 순수 계산(단계 수, 템포·박자표 값)은 여기의 `pub` 함수이고, 창 처리기는 아래 `impl ProjectWindow`다. 끌어서
//! 고른 값은 필드 확정과 같은 편집(`Edit::SetTempo`·`Edit::SetMeter`, 틱 0)으로 뗄 때 한 번 들어간다.

use std::cell::Cell;
use std::ops::RangeInclusive;
use std::rc::Rc;

use gpui_kit::{
    Bounds, Context, DispatchPhase, Focusable as _, IntoElement, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Styled as _, Window, canvas,
};
use keyscore_core::{Edit, Tempo, Tick, TimeSignature};

use crate::fields::Field;
use crate::pianoroll::mouse::DRAG_THRESHOLD;
use crate::tracks::format_meter;
use crate::window::ProjectWindow;

/// 한 단계에 해당하는 세로 거리(px).
pub const DRAG_STEP_PX: f32 = 4.0;
/// 끌어서 고를 수 있는 분자 범위. 그 밖은 입력란에 친다.
pub const NUMERATOR_RANGE: RangeInclusive<u8> = 1..=99;
/// 끌어서 고를 수 있는 가장 큰 분모.
pub const DENOMINATOR_MAX: u16 = 32;

/// 박자표 필드에서 끌기가 바꾸는 쪽.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterPart {
    Numerator,
    Denominator,
}

/// 필드 표기의 범위(10.0–999.9 BPM, 10분의 1 단위). `tracks::parse_tempo`와 같다.
const TEMPO_TENTHS: RangeInclusive<i64> = 100..=9999;

/// 위로 끈 거리(px, 위가 양수) → 단계 수. 4 px마다, 0 쪽으로 내림.
#[must_use]
pub fn drag_steps(up_px: f32) -> i32 {
    // `as`는 범위 밖에서 포화한다(창 밖 먼 거리도 안전하다)
    #[allow(clippy::cast_possible_truncation)]
    let steps = (up_px / DRAG_STEP_PX).trunc() as i32;
    steps
}

/// 템포 → 10분의 1 BPM(`format_tempo`와 같은 반올림).
#[must_use]
pub fn tempo_tenths(tempo: Tempo) -> u32 {
    let us = u64::from(tempo.micros_per_quarter());
    u32::try_from((600_000_000 + us / 2) / us).unwrap_or(u32::MAX)
}

/// 10분의 1 BPM → 템포(`parse_tempo`와 같은 반올림). 템포로 담을 수 없으면 `None`.
#[must_use]
pub fn tempo_from_tenths(tenths: u32) -> Option<Tempo> {
    if tenths == 0 {
        return None;
    }
    let us = (600_000_000 + u64::from(tenths) / 2) / u64::from(tenths);
    Tempo::from_micros_per_quarter(u32::try_from(us).ok()?).ok()
}

/// 범위 `[lo, hi]`에서 기준을 `steps` 단계(한 단계 `unit`) 옮긴다. 기준이 범위 밖이면 바깥으로는 움직이지 않고,
/// 안쪽 첫 단계가 범위 끝이다(결정 7절 4). 범위 안에서는 끝에서 멈춘다.
fn step_within(base: i64, steps: i64, unit: i64, lo: i64, hi: i64) -> i64 {
    let go = |from: i64, n: i64| from.saturating_add(n.saturating_mul(unit)).clamp(lo, hi);
    if base > hi {
        if steps < 0 { go(hi, steps + 1) } else { base }
    } else if base < lo {
        if steps > 0 { go(lo, steps - 1) } else { base }
    } else {
        go(base, steps)
    }
}

/// 기준(10분의 1 BPM)에서 단계만큼 옮긴 값. 미세면 한 단계 0.1 BPM, 아니면 1 BPM. 10.0–999.9 BPM에서 멈춘다.
#[must_use]
pub fn dragged_tempo(base_tenths: u32, steps: i32, fine: bool) -> u32 {
    let unit = if fine { 1 } else { 10 };
    let value = step_within(
        i64::from(base_tenths),
        i64::from(steps),
        unit,
        *TEMPO_TENTHS.start(),
        *TEMPO_TENTHS.end(),
    );
    u32::try_from(value).unwrap_or(base_tenths)
}

/// 기준 박자표에서 단계만큼 옮긴 박자표. 분자는 1씩(1–99), 분모는 두 배·절반(1–32).
#[must_use]
pub fn dragged_meter(base: TimeSignature, part: MeterPart, steps: i32) -> TimeSignature {
    let steps = i64::from(steps);
    let moved = match part {
        MeterPart::Numerator => {
            let n = step_within(
                i64::from(base.numerator()),
                steps,
                1,
                i64::from(*NUMERATOR_RANGE.start()),
                i64::from(*NUMERATOR_RANGE.end()),
            );
            TimeSignature::new(
                u8::try_from(n).unwrap_or(base.numerator()),
                base.denominator(),
            )
        }
        MeterPart::Denominator => {
            // 분모는 2의 거듭제곱이라 지수로 센다
            let k = step_within(
                i64::from(base.denominator().trailing_zeros()),
                steps,
                1,
                0,
                i64::from(DENOMINATOR_MAX.trailing_zeros()),
            );
            let d = u32::try_from(k).ok().and_then(|k| 1_u16.checked_shl(k));
            TimeSignature::new(base.numerator(), d.unwrap_or(base.denominator()))
        }
    };
    moved.unwrap_or(base)
}

/// 필드 왼쪽에서 잰 x와 필드 폭 → 바꿀 쪽. 가운데보다 왼쪽이면 분자.
#[must_use]
pub fn meter_part_at(x: f32, width: f32) -> MeterPart {
    if x < width / 2.0 {
        MeterPart::Numerator
    } else {
        MeterPart::Denominator
    }
}

/// 끌어서 고르는 값.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragValue {
    /// 10분의 1 BPM.
    Tempo(u32),
    Meter(TimeSignature),
}

impl DragValue {
    fn text(self) -> String {
        match self {
            DragValue::Tempo(t) => format!("{}.{}", t / 10, t % 10),
            DragValue::Meter(m) => format_meter(m),
        }
    }
}

/// 필드 끌기(누름부터 뗌까지).
pub(crate) struct FieldDrag {
    field: Field,
    part: MeterPart,
    /// 누른 점(창 좌표).
    press: Point<Pixels>,
    /// 문턱(3 px)을 넘었는가. 넘지 않고 떼면 클릭이다.
    passed: bool,
    /// 누를 때의 값. 뗄 때 이 값과 같으면 편집하지 않는다.
    start: DragValue,
    /// 기준점의 y와 기준 값. `Cmd`가 바뀌면 그 자리로 옮긴다(7절 4).
    base_y: Pixels,
    base: DragValue,
    /// 마지막 움직임의 y.
    last_y: Pixels,
    /// 미세 조정(`Cmd`, 템포만).
    fine: bool,
    value: DragValue,
}

/// 필드 상자. 그리기 때 기록해 누른 x가 박자표의 어느 쪽인지 가른다.
pub(crate) type FieldBox = Rc<Cell<Option<Bounds<Pixels>>>>;

/// 필드 상자를 기록하는 빈 그림(필드를 감싼 상자에 겹쳐 둔다. 히트박스가 없어 누름을 가리지 않는다).
pub(crate) fn field_box_recorder(field_box: &FieldBox) -> impl IntoElement {
    let field_box = field_box.clone();
    canvas(
        |_, _, _| (),
        move |bounds, (), _, _| field_box.set(Some(bounds)),
    )
    // `top_0`·`left_0`이 없으면 절대 위치 요소가 정적 자리(입력란 아래)에 놓여 상자가 어긋난다
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl ProjectWindow {
    /// 끄는 동안 포인터가 필드·창 어디에 있든 움직임과 뗌을 받는 빈 그림. 피아노롤 Element처럼 창 전체 마우스
    /// 사건을 그리기 때마다 등록한다(div의 움직임 처리기는 포인터가 그 위에 있을 때만 불린다).
    ///
    /// 뗌 사건을 놓친 끌기(창 밖에서 놓음 등)가 다음 클릭의 뗌을 자기 뗌으로 받지 않게, 단추를 쥐지 않은 움직임이나
    /// 필드 밖의 새 누름이 오면 남은 끌기를 취소한다(리뷰 2 지적 1). 필드 안의 누름은 `transport_field_press`가
    /// 남은 끌기를 먼저 끝낸다.
    pub(crate) fn transport_drag_listener(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity().downgrade();
        let boxes = [
            self.transport_bar.tempo_box.clone(),
            self.transport_bar.meter_box.clone(),
        ];
        canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let on_down = me.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture
                        || boxes
                            .iter()
                            .any(|b| b.get().is_some_and(|b| b.contains(&event.position)))
                    {
                        return;
                    }
                    let _ = on_down.update(cx, |this, cx| this.cancel_transport_drag(window, cx));
                });
                let on_move = me.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    let _ = on_move.update(cx, |this, cx| {
                        if event.dragging() {
                            this.transport_drag_move(event.position, event.modifiers, window, cx);
                        } else {
                            this.cancel_transport_drag(window, cx);
                        }
                    });
                });
                let on_up = me.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                        return;
                    }
                    let _ = on_up.update(cx, |this, cx| {
                        // 뗀 자리가 마지막 움직임과 다를 수 있다(움직임 사건 없이 뗌). 그 자리로 한 번 더 계산한다.
                        this.transport_drag_move(event.position, event.modifiers, window, cx);
                        this.transport_drag_release(window, cx);
                    });
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    /// 필드 상자의 캡처 단계 누름(7절 2). 그 필드를 고치는 중이 아니면 끌기 후보를 만들고 입력란이 누름을 받지
    /// 않게 기본 동작을 막는다.
    pub(crate) fn transport_field_press(
        &mut self,
        field: Field,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 뗌을 놓친 남은 끌기는 새 누름으로 끝난다(리뷰 2 지적 1)
        self.cancel_transport_drag(window, cx);
        if event.button != MouseButton::Left {
            return;
        }
        let focus = self.field_state(field).read(cx).focus_handle(cx);
        if focus.is_focused(window) {
            return;
        }
        // 입력란(글자 커서·포커스)과 GPUI의 누름 포커스는 기본 동작이 막힌 누름을 받지 않는다. 전파는 멈추지
        // 않는다: 열린 팔레트·메뉴의 바깥 누름 닫기(캡처 단계)가 이 누름을 받아야 한다(리뷰 1 지적 1).
        window.prevent_default();
        // 클릭은 키 입력이 아니므로 되돌리기 묶음을 끊는다. 다른 입력란을 고치던 중이면 포커스를 잃어 취소된다.
        self.seal_history(cx);
        self.focus_document(window, cx);
        let project = self.document.read(cx).project();
        let (base, field_box) = match field {
            Field::Tempo => (
                DragValue::Tempo(tempo_tenths(project.tempo_map().tempo_at(Tick::ZERO))),
                &self.transport_bar.tempo_box,
            ),
            Field::Meter => (
                DragValue::Meter(project.meter_map().signature_at(Tick::ZERO)),
                &self.transport_bar.meter_box,
            ),
            _ => return,
        };
        let part = field_box.get().map_or(MeterPart::Numerator, |b| {
            meter_part_at(
                f32::from(event.position.x - b.origin.x),
                f32::from(b.size.width),
            )
        });
        self.transport_bar.drag = Some(FieldDrag {
            field,
            part,
            press: event.position,
            passed: false,
            start: base,
            base_y: event.position.y,
            base,
            last_y: event.position.y,
            fine: event.modifiers.platform,
            value: base,
        });
    }

    fn transport_drag_move(
        &mut self,
        at: Point<Pixels>,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.transport_field_entered(window, cx) {
            return;
        }
        let Some(drag) = self.transport_bar.drag.as_mut() else {
            return;
        };
        if !drag.passed {
            let d = at - drag.press;
            let (dx, dy) = (f32::from(d.x), f32::from(d.y));
            if dx * dx + dy * dy < DRAG_THRESHOLD * DRAG_THRESHOLD {
                return;
            }
            drag.passed = true;
        }
        let fine = drag.field == Field::Tempo && modifiers.platform;
        if fine != drag.fine {
            // 지난 자리의 값을 새 기준으로(단계 크기가 바뀌어도 값이 튀지 않게)
            drag.base = drag.value;
            drag.base_y = drag.last_y;
            drag.fine = fine;
        }
        drag.last_y = at.y;
        let steps = drag_steps(f32::from(drag.base_y - at.y));
        drag.value = match drag.base {
            DragValue::Tempo(t) => DragValue::Tempo(dragged_tempo(t, steps, drag.fine)),
            DragValue::Meter(m) => DragValue::Meter(dragged_meter(m, drag.part, steps)),
        };
        let (field, text) = (drag.field, drag.value.text());
        self.field_state(field)
            .clone()
            .update(cx, |state, cx| state.set_value(text, window, cx));
        cx.notify();
    }

    /// 끄는 중 그 필드가 포커스를 받았으면(`transport::SetTempo` 값 없음 등) 끌기를 놓는다. 포커스가 있는 필드는
    /// 고치는 중이라(7절 2) 친 글자를 움직임·뗌이 덮거나 버리지 않는다(리뷰 2 제안 2). 놓았으면 참.
    fn transport_field_entered(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let Some(field) = self.transport_bar.drag.as_ref().map(|d| d.field) else {
            return false;
        };
        if !self
            .field_state(field)
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
        {
            return false;
        }
        self.transport_bar.drag = None;
        cx.notify();
        true
    }

    fn transport_drag_release(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.transport_field_entered(window, cx) {
            return;
        }
        let Some(drag) = self.transport_bar.drag.take() else {
            return;
        };
        if !drag.passed {
            // 클릭: 필드에 들어가 내용을 전체 선택한다(7절 3, 「클릭해 입력」)
            self.field_state(drag.field)
                .clone()
                .update(cx, |state, cx| {
                    state.focus(window, cx);
                    state.select_all(window, cx);
                });
            return;
        }
        if drag.value != drag.start {
            let project = self.document.read(cx).project();
            let edit = match drag.value {
                DragValue::Tempo(t) => tempo_from_tenths(t)
                    .filter(|tempo| *tempo != project.tempo_map().tempo_at(Tick::ZERO))
                    .map(|tempo| Edit::SetTempo {
                        at: Tick::ZERO,
                        tempo: Some(tempo),
                    }),
                DragValue::Meter(signature) => (signature
                    != project.meter_map().signature_at(Tick::ZERO))
                .then_some(Edit::SetMeter {
                    at: Tick::ZERO,
                    signature: Some(signature),
                }),
            };
            if let Some(edit) = edit {
                self.edit_document(vec![edit], cx);
            }
        }
        self.reset_field(drag.field, window, cx);
        self.focus_document(window, cx);
        cx.notify();
    }

    /// 문서가 바뀌었다. 끄는 값(틱 0의 템포·박자표)이 누를 때와 달라졌으면 끌기를 취소한다(결정 7절 7). 기준이
    /// 낡은 끌기가 뗄 때 그 변경을 덮지 않게 하기 위해서다. 다른 편집이나 되돌릴 것이 없는 `Cmd+Z`는 끌기를
    /// 끊지 않는다(리뷰 1 제안 2).
    pub(crate) fn transport_drag_document_changed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.transport_bar.drag.as_ref() else {
            return;
        };
        let project = self.document.read(cx).project();
        let now = match drag.start {
            DragValue::Tempo(_) => {
                DragValue::Tempo(tempo_tenths(project.tempo_map().tempo_at(Tick::ZERO)))
            }
            DragValue::Meter(_) => DragValue::Meter(project.meter_map().signature_at(Tick::ZERO)),
        };
        if now != drag.start {
            self.cancel_transport_drag(window, cx);
        }
    }

    /// 진행 중 트랜스포트 끌기를 취소한다(`edit::CancelDrag`, 문서 변경, 7절 7). 필드는 문서 값으로 돌아간다.
    /// 취소했으면 참.
    pub(crate) fn cancel_transport_drag(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = self.transport_bar.drag.take() else {
            return false;
        };
        self.reset_field(drag.field, window, cx);
        cx.notify();
        true
    }

    /// 템포 필드가 지금 보이는 글자(시험용).
    #[must_use]
    pub fn tempo_field_text(&self, cx: &gpui_kit::App) -> String {
        self.field_state(Field::Tempo).read(cx).value().to_string()
    }

    /// 박자표 필드가 지금 보이는 글자(시험용).
    #[must_use]
    pub fn meter_field_text(&self, cx: &gpui_kit::App) -> String {
        self.field_state(Field::Meter).read(cx).value().to_string()
    }

    /// 트랜스포트 필드 끌기(문턱을 넘은)가 진행 중인가(시험용).
    #[must_use]
    pub fn transport_drag_active(&self) -> bool {
        self.transport_bar.drag.as_ref().is_some_and(|d| d.passed)
    }
}
