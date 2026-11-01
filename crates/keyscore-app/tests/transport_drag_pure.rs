//! S3-T6c 트랜스포트 끌기의 순수 계산. 결정 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절 4.

use keyscore_app::tracks::{format_tempo, parse_tempo};
use keyscore_app::transport_drag::{
    MeterPart, drag_steps, dragged_meter, dragged_tempo, meter_part_at, tempo_from_tenths,
    tempo_tenths,
};
use keyscore_core::TimeSignature;

fn sig(n: u8, d: u16) -> TimeSignature {
    TimeSignature::new(n, d).unwrap()
}

#[test]
fn drag_steps_counts_four_px_toward_zero() {
    assert_eq!(drag_steps(0.0), 0);
    assert_eq!(drag_steps(3.9), 0);
    assert_eq!(drag_steps(4.0), 1);
    assert_eq!(drag_steps(7.0), 1);
    assert_eq!(drag_steps(8.0), 2);
    assert_eq!(drag_steps(-3.9), 0);
    assert_eq!(drag_steps(-4.0), -1);
    assert_eq!(drag_steps(-7.0), -1);
    assert_eq!(drag_steps(-40.0), -10);
}

#[test]
fn dragged_tempo_adds_steps_and_clamps() {
    // 1 BPM 단계, 소수 자리 유지
    assert_eq!(dragged_tempo(1200, 10, false), 1300);
    assert_eq!(dragged_tempo(1205, 2, false), 1225);
    assert_eq!(dragged_tempo(1205, -3, false), 1175);
    // 0.1 BPM 단계
    assert_eq!(dragged_tempo(1200, 10, true), 1210);
    assert_eq!(dragged_tempo(1200, -1, true), 1199);
    // 10.0–999.9에서 멈춤
    assert_eq!(dragged_tempo(1200, -10_000, false), 100);
    assert_eq!(dragged_tempo(105, -1, false), 100);
    assert_eq!(dragged_tempo(9990, 5, false), 9999);
    assert_eq!(dragged_tempo(1200, i32::MAX, false), 9999);
    assert_eq!(dragged_tempo(1200, i32::MIN, true), 100);
}

/// 10.0–999.9 모든 값이 필드 표기와 같은 반올림으로 왕복한다(전수).
#[test]
fn tempo_tenths_round_trips_with_field_format() {
    for tenths in 100..=9999_u32 {
        let tempo = tempo_from_tenths(tenths).unwrap();
        assert_eq!(tempo_tenths(tempo), tenths, "{tenths}");
        let text = format!("{}.{}", tenths / 10, tenths % 10);
        assert_eq!(format_tempo(tempo), text);
        assert_eq!(parse_tempo(&text).unwrap(), tempo);
    }
    assert_eq!(tempo_from_tenths(0), None);
}

#[test]
fn dragged_meter_changes_numerator_and_denominator() {
    let n = MeterPart::Numerator;
    let d = MeterPart::Denominator;
    assert_eq!(dragged_meter(sig(4, 4), n, 1), sig(5, 4));
    assert_eq!(dragged_meter(sig(4, 4), n, -2), sig(2, 4));
    assert_eq!(dragged_meter(sig(4, 4), n, -10), sig(1, 4));
    assert_eq!(dragged_meter(sig(4, 4), n, 200), sig(99, 4));
    assert_eq!(dragged_meter(sig(4, 4), d, 1), sig(4, 8));
    assert_eq!(dragged_meter(sig(4, 4), d, 2), sig(4, 16));
    assert_eq!(dragged_meter(sig(4, 4), d, -1), sig(4, 2));
    assert_eq!(dragged_meter(sig(4, 4), d, -5), sig(4, 1));
    assert_eq!(dragged_meter(sig(4, 4), d, 9), sig(4, 32));
    assert_eq!(dragged_meter(sig(6, 8), d, 0), sig(6, 8));
    // 끌기 범위 밖의 기준: 바깥으로는 움직이지 않고, 안쪽 한 단계에 범위 끝으로 들어온다
    assert_eq!(dragged_meter(sig(120, 4), n, 1), sig(120, 4));
    assert_eq!(dragged_meter(sig(120, 4), n, -1), sig(99, 4));
    assert_eq!(dragged_meter(sig(120, 4), n, -3), sig(97, 4));
    assert_eq!(dragged_meter(sig(4, 128), d, 1), sig(4, 128));
    assert_eq!(dragged_meter(sig(4, 128), d, -1), sig(4, 32));
    assert_eq!(dragged_meter(sig(4, 128), d, -2), sig(4, 16));
    assert_eq!(dragged_meter(sig(4, 4), n, i32::MIN), sig(1, 4));
    assert_eq!(dragged_meter(sig(4, 4), d, i32::MAX), sig(4, 32));
}

#[test]
fn meter_part_splits_at_middle() {
    assert_eq!(meter_part_at(0.0, 56.0), MeterPart::Numerator);
    assert_eq!(meter_part_at(27.9, 56.0), MeterPart::Numerator);
    assert_eq!(meter_part_at(28.0, 56.0), MeterPart::Denominator);
    assert_eq!(meter_part_at(55.0, 56.0), MeterPart::Denominator);
}
