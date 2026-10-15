//! S1-T3 적대적 리뷰 3회차 테스트. 대상은 리뷰 2 대응 커밋(`2ea6827`, `c02cc19`, `679e16a`,
//! `2488a22`)이다.
//!
//! 깨뜨리지 못한 경계를 통과 테스트로 남긴다. 가상 포트를 쓰지 않는다.

use keyscore_core::{Channel, Pitch, Velocity};
use keyscore_midi::{
    Group, MidiError, bend_to_14bit, control_change, note_on, pitch_bend, song_position,
};

/// 리뷰 2 제안 3 대응: 넓힌 문서가 말하는 인코더 쪽 세 경우가 모두 바로 그 경계에서 갈리고,
/// 같은 변형·같은 표시 문자열로 나온다. 표시 문자열은 더 이상 길이 불일치만 말하지 않는다.
#[test]
fn invalid_message_every_encoder_boundary_splits_exactly_and_display_is_generic() {
    let c = Channel::new(1).unwrap();

    assert!(Group::try_from(15).is_ok());
    assert!(control_change(c, 119, 0).is_ok());
    assert!(song_position(0x3FFF).is_ok());

    let errors = [
        Group::try_from(16).unwrap_err(),
        control_change(c, 120, 0).unwrap_err(),
        song_position(0x4000).unwrap_err(),
    ];
    for e in errors {
        assert_eq!(e, MidiError::InvalidMessage);
        let shown = e.to_string();
        assert!(!shown.is_empty());
        assert!(
            !shown.contains("길이"),
            "표시 문자열이 길이 불일치만 말한다: {shown}"
        );
    }
}

/// 리뷰 2 지적 1 대응(S6-T1 이월)의 전제: 인코더의 모델 벤드 값은 가운데 `0x8000_0000`인 부호 없는
/// 32비트이고, 14비트 값을 `<< 18`로 올리면 `bend_to_14bit`가 그대로 되돌린다. 레인 최댓값 +8191에
/// 해당하는 14비트 0x3FFF가 최댓값이고, +8192는 14비트로 표현할 수 없다.
#[test]
fn bend_every_14bit_value_round_trips_through_32bit_and_center_is_8000_0000() {
    for b in 0u16..0x4000 {
        assert_eq!(bend_to_14bit(u32::from(b) << 18), b, "b={b}");
    }
    assert_eq!(0x2000u32 << 18, 0x8000_0000);
    assert_eq!(bend_to_14bit(u32::MAX), 0x3FFF);

    let c = Channel::new(1).unwrap();
    assert_eq!(pitch_bend(c, 0x8000_0000).bytes(), &[0xE0, 0x00, 0x40]);
    assert_eq!(pitch_bend(c, u32::MAX).bytes(), &[0xE0, 0x7F, 0x7F]);
}

/// 리뷰 2 제안 2 대응(S1-T6 이월)의 전제: 인코더는 벨로시티 0인 Note On을 만들지 않는다. 따라서
/// 가져올 때 `9n kk 00`을 Note Off로 읽는 규칙이 우리 내보내기 결과를 바꾸지 않는다.
#[test]
fn note_on_any_velocity_never_encodes_data2_zero() {
    let c = Channel::new(10).unwrap();
    let p = Pitch::new(60).unwrap();
    for v in 0..=u16::MAX {
        let msg = note_on(c, p, Velocity::new(v));
        let bytes = msg.bytes();
        assert_eq!(bytes[0], 0x99);
        assert_ne!(bytes[2], 0, "v={v}");
    }
}
