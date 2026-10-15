//! S1-T3 적대적 리뷰 2회차 테스트. 대상은 리뷰 1 지적 1의 수정(`Group` newtype)이다.
//!
//! 깨뜨리지 못한 경계를 통과 테스트로 남긴다. 가상 포트를 쓰지 않는다.

use keyscore_core::{Bank, Channel, Pitch, Program, Velocity};
use keyscore_midi::{
    Group, Midi1Message, MidiError, bank_select, continue_, control_change, note_off, note_on,
    panic_messages, pitch_bend, program_change, song_position, start, stop, timing_clock,
};

fn every_kind() -> Vec<Midi1Message> {
    let c = Channel::new(16).unwrap();
    let p = Pitch::new(127).unwrap();
    let mut v = vec![
        note_on(c, p, Velocity::new(0)),
        note_on(c, p, Velocity::new(u16::MAX)),
        note_off(c, p, Velocity::new(0)),
        program_change(c, Program::new(127).unwrap()),
        control_change(c, 119, u32::MAX).unwrap(),
        pitch_bend(c, u32::MAX),
        pitch_bend(c, 0),
        song_position(16383).unwrap(),
        song_position(0).unwrap(),
        timing_clock(),
        start(),
        continue_(),
        stop(),
    ];
    v.extend(bank_select(c, Bank::new(Bank::MAX).unwrap()));
    v.extend(panic_messages(c));
    v
}

#[test]
fn group_try_from_every_u8_accepts_only_zero_to_fifteen_and_get_round_trips() {
    for g in 0..=u8::MAX {
        match Group::try_from(g) {
            Ok(group) => {
                assert!(g < 16, "group={g}가 받아들여졌다");
                assert_eq!(group.get(), g);
            }
            Err(e) => {
                assert!(g >= 16, "group={g}가 거부됐다");
                assert_eq!(e, MidiError::InvalidMessage);
            }
        }
    }
}

#[test]
fn group_zero_constant_equals_try_from_zero_and_is_smallest() {
    assert_eq!(Group::try_from(0), Ok(Group::ZERO));
    assert_eq!(Group::ZERO.get(), 0);
    for g in 1..16u8 {
        assert!(Group::ZERO < Group::try_from(g).unwrap());
    }
}

#[test]
fn ump_every_group_sets_only_group_nibble_and_rest_matches_bytes() {
    for m in every_kind() {
        let base = m.ump(Group::ZERO);
        let b = m.bytes();
        // 그룹 0 워드를 바이트에서 다시 읽는다. 쓰지 않는 데이터 바이트는 0이다.
        let expected_type = if b[0] >= 0xF0 { 0x1 } else { 0x2 };
        assert_eq!(base >> 28, expected_type, "{b:02X?}");
        assert_eq!((base >> 24) & 0xF, 0, "{b:02X?}");
        assert_eq!((base >> 16) & 0xFF, u32::from(b[0]), "{b:02X?}");
        let d1 = b.get(1).copied().map_or(0, u32::from);
        let d2 = b.get(2).copied().map_or(0, u32::from);
        assert_eq!(base & 0xFFFF, (d1 << 8) | d2, "{b:02X?}");
        for g in 0..16u8 {
            let w = m.ump(Group::try_from(g).unwrap());
            assert_eq!(w, base | (u32::from(g) << 24), "group={g} {b:02X?}");
        }
    }
}

#[test]
fn ump_sixteen_groups_give_sixteen_distinct_words() {
    for m in every_kind() {
        let mut words: Vec<u32> = (0..16u8)
            .map(|g| m.ump(Group::try_from(g).unwrap()))
            .collect();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), 16, "{:02X?}", m.bytes());
    }
}
