//! 트랜스포트 템포·박자표 필드의 끌어서 조정(S3-T6c, REQ 7.6 「클릭해 입력, 끌어서 조정」). 규칙은
//! `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절, 구현 단위는 `docs/reports/s3-t6c/plan.md`.
//!
//! 순수 계산(단계 수, 템포·박자표 값)은 여기의 `pub` 함수이고, 창 처리기는 아래 `impl ProjectWindow`다. 끌어서
//! 고른 값은 필드 확정과 같은 편집(`Edit::SetTempo`·`Edit::SetMeter`, 틱 0)으로 뗄 때 한 번 들어간다.

use std::ops::RangeInclusive;

use keyscore_core::{Tempo, TimeSignature};

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

/// 위로 끈 거리(px, 위가 양수) → 단계 수. 4 px마다, 0 쪽으로 내림.
#[must_use]
pub fn drag_steps(up_px: f32) -> i32 {
    let _ = up_px;
    todo!()
}

/// 템포 → 10분의 1 BPM(`format_tempo`와 같은 반올림).
#[must_use]
pub fn tempo_tenths(tempo: Tempo) -> u32 {
    let _ = tempo;
    todo!()
}

/// 10분의 1 BPM → 템포(`parse_tempo`와 같은 반올림). 0이면 `None`.
#[must_use]
pub fn tempo_from_tenths(tenths: u32) -> Option<Tempo> {
    let _ = tenths;
    todo!()
}

/// 기준(10분의 1 BPM)에서 단계만큼 옮긴 값. 미세면 한 단계 0.1 BPM, 아니면 1 BPM. 10.0–999.9 BPM에서 멈춘다.
#[must_use]
pub fn dragged_tempo(base_tenths: u32, steps: i32, fine: bool) -> u32 {
    let _ = (base_tenths, steps, fine);
    todo!()
}

/// 기준 박자표에서 단계만큼 옮긴 박자표. 분자는 1씩(1–99), 분모는 두 배·절반(1–32).
#[must_use]
pub fn dragged_meter(base: TimeSignature, part: MeterPart, steps: i32) -> TimeSignature {
    let _ = (base, part, steps);
    todo!()
}

/// 필드 왼쪽에서 잰 x와 필드 폭 → 바꿀 쪽. 가운데보다 왼쪽이면 분자.
#[must_use]
pub fn meter_part_at(x: f32, width: f32) -> MeterPart {
    let _ = (x, width);
    todo!()
}

impl crate::window::ProjectWindow {
    /// 템포 필드가 지금 보이는 글자(시험용).
    #[must_use]
    pub fn tempo_field_text(&self, cx: &gpui_kit::App) -> String {
        let _ = cx;
        todo!()
    }

    /// 박자표 필드가 지금 보이는 글자(시험용).
    #[must_use]
    pub fn meter_field_text(&self, cx: &gpui_kit::App) -> String {
        let _ = cx;
        todo!()
    }

    /// 트랜스포트 필드 끌기(문턱을 넘은)가 진행 중인가(시험용).
    #[must_use]
    pub fn transport_drag_active(&self) -> bool {
        todo!()
    }
}
