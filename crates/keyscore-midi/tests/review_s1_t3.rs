//! S1-T3 적대적 리뷰 1회차(`docs/reports/s1-t3/review-1.md`)의 테스트.
//!
//! 실패하는 테스트가 깨뜨린 사례이고, 통과하는 테스트는 경계를 고정한다. `CoreMIDI`를 쓰는 테스트는
//! `coremidi` crate 클라이언트보다 먼저 `MidiClient::new`를 부른다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::{Bank, Channel, Pitch, Program, Velocity};
use keyscore_midi::{
    Group, HostTime, Midi1Message, MidiClient, MidiError, MidiPort, bank_select, bend_to_14bit,
    cc_to_7bit, continue_, control_change, note_off, note_on, panic_messages, pitch_bend,
    program_change, song_position, source_names, start, stop, timing_clock, velocity_to_7bit,
};

fn ch(n: u8) -> Channel {
    Channel::new(n).unwrap()
}

fn pitch(n: u8) -> Pitch {
    Pitch::new(n).unwrap()
}

/// 32비트 범위를 고르게 훑는 표본. 끝값과 시프트 경계 바로 앞뒤를 포함한다.
fn u32_samples() -> Vec<u32> {
    let mut v: Vec<u32> = (0..=u32::MAX).step_by(65_521).collect();
    for k in 0..32 {
        let b = 1u32 << k;
        v.extend([b - 1, b, b.wrapping_add(1), u32::MAX - b]);
    }
    v.push(u32::MAX);
    v
}

#[test]
fn velocity_to_7bit_every_u16_matches_top_bits_and_core_to_midi1() {
    for v in 0..=u16::MAX {
        let got = velocity_to_7bit(v);
        assert_eq!(u16::from(got), v >> 9, "v={v:#x}");
        assert_eq!(got, Velocity::new(v).to_midi1(), "core와 다름: v={v:#x}");
    }
}

#[test]
fn note_on_every_u16_velocity_never_sends_zero_and_is_monotonic() {
    let mut prev = 0u8;
    for v in 0..=u16::MAX {
        let b = note_on(ch(1), pitch(60), Velocity::new(v)).bytes()[2];
        assert!((1..=127).contains(&b), "v={v:#x} -> {b}");
        assert_eq!(b, velocity_to_7bit(v).max(1), "v={v:#x}");
        assert!(b >= prev, "단조 증가가 아니다: v={v:#x}");
        prev = b;
    }
}

#[test]
fn note_off_every_u16_release_velocity_keeps_zero_and_uses_status_8n() {
    for v in 0..=u16::MAX {
        let b = note_off(ch(16), pitch(0), Velocity::new(v))
            .bytes()
            .to_vec();
        assert_eq!(b, vec![0x8F, 0, velocity_to_7bit(v)], "v={v:#x}");
    }
}

#[test]
fn cc_to_7bit_low_bits_set_do_not_leak_into_result() {
    // 하위 비트 절삭(`v & 0x7F`)이었다면 127이 나왔을 값들이다.
    for v in [0x7F, 0x01FF_FFFF, 0x0000_FFFF] {
        assert_eq!(cc_to_7bit(v), 0, "v={v:#x}");
    }
    for v in u32_samples() {
        assert_eq!(u32::from(cc_to_7bit(v)), v >> 25, "v={v:#x}");
    }
}

#[test]
fn pitch_bend_every_14bit_value_encodes_lsb_then_msb_ignoring_low_18_bits() {
    for x in 0u32..0x4000 {
        let expected = [
            0xE0,
            u8::try_from(x & 0x7F).unwrap(),
            u8::try_from(x >> 7).unwrap(),
        ];
        assert_eq!(pitch_bend(ch(1), x << 18).bytes(), &expected, "x={x:#x}");
        assert_eq!(
            pitch_bend(ch(1), (x << 18) | 0x3_FFFF).bytes(),
            &expected,
            "x={x:#x} 하위 18비트가 결과를 바꿨다"
        );
    }
    for v in u32_samples() {
        assert_eq!(u32::from(bend_to_14bit(v)), v >> 18, "v={v:#x}");
    }
}

#[test]
fn bank_select_every_bank_recombines_to_same_value() {
    for v in 0..=Bank::MAX {
        let [msb, lsb] = bank_select(ch(7), Bank::new(v).unwrap());
        let (m, l) = (msb.bytes(), lsb.bytes());
        assert_eq!((m[0], m[1], l[0], l[1]), (0xB6, 0, 0xB6, 32), "v={v}");
        assert_eq!((u16::from(m[2]) << 7) | u16::from(l[2]), v, "v={v}");
    }
}

#[test]
fn control_change_bank_controllers_match_bank_select_encoding() {
    // 리뷰어에게 2: CC0·CC32를 control_change로 보내도 bank_select와 같은 바이트다.
    let [msb, lsb] = bank_select(ch(2), Bank::new(0x1234).unwrap());
    assert_eq!(control_change(ch(2), 0, 0x24 << 25).unwrap(), msb);
    assert_eq!(control_change(ch(2), 32, 0x34 << 25).unwrap(), lsb);
}

#[test]
fn song_position_every_14bit_value_round_trips_and_16384_is_error() {
    for x in 0u16..0x4000 {
        let m = song_position(x).unwrap();
        let b = m.bytes();
        assert_eq!(b[0], 0xF2);
        assert_eq!((u16::from(b[2]) << 7) | u16::from(b[1]), x);
        assert_eq!(
            m.ump(Group::ZERO),
            0x10F2_0000 | (u32::from(b[1]) << 8) | u32::from(b[2])
        );
    }
    for x in 0x4000..=u16::MAX {
        assert_eq!(song_position(x), Err(MidiError::InvalidMessage), "x={x}");
    }
}

#[test]
fn every_channel_maps_to_status_nibble_zero_to_fifteen() {
    for n in 1..=16u8 {
        let c = ch(n);
        let nib = n - 1;
        let msgs = [
            (note_off(c, pitch(1), Velocity::new(0)), 0x80),
            (note_on(c, pitch(1), Velocity::new(0)), 0x90),
            (control_change(c, 1, 0).unwrap(), 0xB0),
            (program_change(c, Program::new(0).unwrap()), 0xC0),
            (pitch_bend(c, 0), 0xE0),
        ];
        for (m, status) in msgs {
            assert_eq!(m.bytes()[0], status | nib, "ch={n}");
            assert_eq!(
                m.ump(Group::ZERO) >> 16,
                0x2000 | u32::from(status | nib),
                "ch={n}"
            );
        }
        for m in panic_messages(c) {
            assert_eq!(m.bytes()[0], 0xB0 | nib, "ch={n}");
        }
    }
}

#[test]
fn ump_layout_matches_spec_for_two_byte_and_realtime_messages() {
    // 타입 0x2: [mt|group][status][data1][data2], 쓰지 않는 data2는 0.
    let pc = program_change(ch(3), Program::new(0x55).unwrap());
    assert_eq!(pc.ump(Group::try_from(0xA).unwrap()), 0x2AC2_5500);
    // 타입 0x1: 리얼타임은 data1·data2가 0.
    assert_eq!(start().ump(Group::try_from(15).unwrap()), 0x1FFA_0000);
    assert_eq!(continue_().ump(Group::ZERO), 0x10FB_0000);
    assert_eq!(stop().ump(Group::try_from(1).unwrap()), 0x11FC_0000);
    assert_eq!(timing_clock().bytes(), &[0xF8]);
}

#[test]
fn ump_group_out_of_range_does_not_alias_valid_group() {
    // 깨뜨린 사례: 그룹 16은 그룹 0, 그룹 17은 그룹 1과 같은 워드가 됐다. 상위 비트 절삭으로
    // 범위 밖 값이 조용히 다른 그룹으로 갔으므로, 이제 타입이 생성 시점에 거부한다.
    for g in 16..=u8::MAX {
        assert_eq!(
            Group::try_from(g),
            Err(MidiError::InvalidMessage),
            "group={g}"
        );
    }
}

#[test]
fn panic_messages_every_channel_sends_cc120_cc123_cc64_zero_in_order() {
    for n in 1..=16u8 {
        let got: Vec<Vec<u8>> = panic_messages(ch(n))
            .iter()
            .map(|m| m.bytes().to_vec())
            .collect();
        let s = 0xB0 | (n - 1);
        assert_eq!(got, vec![vec![s, 120, 0], vec![s, 123, 0], vec![s, 64, 0]]);
    }
}

fn find_source(name: &str) -> coremidi::Source {
    let start = Instant::now();
    loop {
        if source_names().iter().any(|n| n == name)
            && let Some(s) = coremidi::Source::from_name(name)
        {
            return s;
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "소스를 찾지 못했다"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn every_message_kind_reaches_receiver_with_same_bytes_through_coremidi() {
    let client = MidiClient::new("ks-review-t3").unwrap();
    let name = format!("KS rv3 {} all", std::process::id());
    let port = client.create_virtual_output(&name).unwrap();

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let receiver = coremidi::Client::new("ks-review-t3-receiver").unwrap();
    let input = receiver
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
            }
        })
        .unwrap();
    input.connect_source(&find_source(&name)).unwrap();

    let c = ch(16);
    let mut msgs: Vec<Midi1Message> = vec![
        note_on(c, pitch(127), Velocity::new(1)),
        note_off(c, pitch(127), Velocity::new(0)),
        program_change(c, Program::new(127).unwrap()),
        control_change(c, 119, u32::MAX).unwrap(),
        pitch_bend(c, 0x8000_0000),
        pitch_bend(c, u32::MAX),
        song_position(16383).unwrap(),
        song_position(0).unwrap(),
        start(),
        continue_(),
        stop(),
        timing_clock(),
    ];
    msgs.extend(bank_select(c, Bank::new(Bank::MAX).unwrap()));
    for m in msgs {
        port.send(HostTime::now(), &[m.ump(Group::ZERO)]).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            m.bytes(),
            "ump={:#010x}",
            m.ump(Group::ZERO)
        );
    }
}
