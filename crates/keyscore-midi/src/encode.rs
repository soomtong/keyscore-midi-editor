//! 모델 값을 MIDI 1.0 메시지로 인코딩한다(REQ M2-1, SEQ-4, CTL-4, OUT-5, OUT-6).
//!
//! 다운스케일은 MIDI 2.0 UMP 명세의 변환 규칙(오른쪽 시프트)을 따른다. 하위 비트만 남기는
//! 절삭은 큰 값이 작은 값으로 접히므로 쓰지 않는다. 송신 경로에서 쓰이므로 할당·락·패닉이 없다.

use keyscore_core::{Bank, Channel, Pitch, Program, Velocity};

use crate::MidiError;

/// 1–3바이트짜리 MIDI 1.0 메시지. 고정 크기라 힙 할당이 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Midi1Message {
    bytes: [u8; 3],
    len: u8,
}

/// 상태 바이트 하위 니블로 쓰는 채널 번호(0–15).
fn channel_nibble(ch: Channel) -> u8 {
    // Channel은 1–16을 보장한다. 0이 되는 입력은 없지만 뺄셈이 넘치지 않게 포화시킨다.
    ch.get().saturating_sub(1) & 0x0F
}

/// 값이 이미 7비트 안이라는 전제로 `u8`로 바꾼다. 시프트가 범위를 보장하므로 `unwrap_or`는
/// 닿지 않는 분기이고, 패닉 대신 최댓값으로 막는다.
fn to_7bit(v: u32) -> u8 {
    u8::try_from(v & 0x7F).unwrap_or(0x7F)
}

fn short(status: u8, d1: u8, d2: u8, len: u8) -> Midi1Message {
    Midi1Message {
        bytes: [status, d1, d2],
        len,
    }
}

fn channel_msg(status: u8, ch: Channel, d1: u8, d2: u8, len: u8) -> Midi1Message {
    short(status | channel_nibble(ch), d1, d2, len)
}

fn cc(ch: Channel, controller: u8, value: u8) -> Midi1Message {
    channel_msg(0xB0, ch, controller & 0x7F, value & 0x7F, 3)
}

fn realtime(status: u8) -> Midi1Message {
    short(status, 0, 0, 1)
}

impl Midi1Message {
    /// SMF에 쓰는 MIDI 1.0 바이트. 상태 바이트는 항상 포함한다(running status 없음).
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    /// `MidiPort::send`에 넘길 UMP 한 워드. 채널 보이스는 타입 0x2, 시스템은 타입 0x1이다.
    /// 그룹은 하위 4비트만 쓴다(0–15).
    #[must_use]
    pub fn ump(&self, group: u8) -> u32 {
        let b = &self.bytes;
        // 1바이트 메시지도 시스템 타입 워드에 data1·data2를 0으로 채운다.
        let status = u32::from(b[0]);
        let data = if self.len >= 3 {
            (u32::from(b[1]) << 8) | u32::from(b[2])
        } else if self.len == 2 {
            u32::from(b[1]) << 8
        } else {
            0
        };
        let message_type = if b[0] >= 0xF0 { 0x1 } else { 0x2 };
        (message_type << 28) | (u32::from(group & 0x0F) << 24) | (status << 16) | data
    }
}

/// 16비트 벨로시티를 7비트로 내린다(`v >> 9`). 0이 나올 수 있다.
#[must_use]
pub fn velocity_to_7bit(v: u16) -> u8 {
    to_7bit(u32::from(v) >> 9)
}

/// 32비트 컨트롤러 값을 7비트로 내린다(`v >> 25`).
#[must_use]
pub fn cc_to_7bit(v: u32) -> u8 {
    to_7bit(v >> 25)
}

/// 32비트 피치 벤드를 14비트로 내린다(`v >> 18`). 가운데 `0x8000_0000`은 `0x2000`이다.
#[must_use]
pub fn bend_to_14bit(v: u32) -> u16 {
    u16::try_from(v >> 18).unwrap_or(0x3FFF)
}

/// Note On. 내린 벨로시티가 0이면 1로 보낸다(MIDI 1.0의 벨로시티 0 Note On은 Note Off다).
#[must_use]
pub fn note_on(ch: Channel, pitch: Pitch, velocity: Velocity) -> Midi1Message {
    channel_msg(
        0x90,
        ch,
        pitch.get(),
        velocity_to_7bit(velocity.get()).max(1),
        3,
    )
}

/// Note Off. 릴리스 벨로시티는 `v >> 9` 그대로 보낸다.
#[must_use]
pub fn note_off(ch: Channel, pitch: Pitch, release: Velocity) -> Midi1Message {
    channel_msg(0x80, ch, pitch.get(), velocity_to_7bit(release.get()), 3)
}

#[must_use]
pub fn program_change(ch: Channel, program: Program) -> Midi1Message {
    channel_msg(0xC0, ch, program.get() & 0x7F, 0, 2)
}

/// CC0(MSB), CC32(LSB) 순서.
#[must_use]
pub fn bank_select(ch: Channel, bank: Bank) -> [Midi1Message; 2] {
    let v = bank.get();
    [
        cc(ch, 0, to_7bit(u32::from(v >> 7))),
        cc(ch, 32, to_7bit(u32::from(v))),
    ]
}

/// # Errors
///
/// 컨트롤러 번호가 120 이상이면(채널 모드 메시지) `InvalidMessage`.
pub fn control_change(
    ch: Channel,
    controller: u8,
    value32: u32,
) -> Result<Midi1Message, MidiError> {
    if controller >= 120 {
        return Err(MidiError::InvalidMessage);
    }
    Ok(cc(ch, controller, cc_to_7bit(value32)))
}

/// 바이트 순서는 `E0 lsb msb`다.
#[must_use]
pub fn pitch_bend(ch: Channel, value32: u32) -> Midi1Message {
    let v = bend_to_14bit(value32);
    channel_msg(
        0xE0,
        ch,
        to_7bit(u32::from(v)),
        to_7bit(u32::from(v >> 7)),
        3,
    )
}

#[must_use]
pub fn timing_clock() -> Midi1Message {
    realtime(0xF8)
}

#[must_use]
pub fn start() -> Midi1Message {
    realtime(0xFA)
}

#[must_use]
pub fn continue_() -> Midi1Message {
    realtime(0xFB)
}

#[must_use]
pub fn stop() -> Midi1Message {
    realtime(0xFC)
}

/// # Errors
///
/// 14비트(16384) 이상이면 `InvalidMessage`.
pub fn song_position(sixteenths: u16) -> Result<Midi1Message, MidiError> {
    if sixteenths >= 0x4000 {
        return Err(MidiError::InvalidMessage);
    }
    Ok(short(
        0xF2,
        to_7bit(u32::from(sixteenths)),
        to_7bit(u32::from(sixteenths >> 7)),
        3,
    ))
}

/// All Sound Off(CC120), All Notes Off(CC123), Sustain 0(CC64) 순서(OUT-6).
#[must_use]
pub fn panic_messages(ch: Channel) -> [Midi1Message; 3] {
    [cc(ch, 120, 0), cc(ch, 123, 0), cc(ch, 64, 0)]
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn ch(n: u8) -> Channel {
        Channel::new(n).unwrap()
    }

    fn pitch(n: u8) -> Pitch {
        Pitch::new(n).unwrap()
    }

    #[test]
    fn velocity_downscale_matches_midi2_translation_table() {
        let table: [(u16, u8); 8] = [
            (0, 0),
            (1, 0),
            (0x1FF, 0),
            (0x200, 1),
            (0x8000, 64),
            (0xC924, 100),
            (0xFDFF, 126),
            (0xFFFF, 127),
        ];
        for (v, expected) in table {
            assert_eq!(velocity_to_7bit(v), expected, "v={v:#x}");
        }
        // Note On은 0으로 나가지 않고, Note Off의 릴리스 벨로시티는 그대로 0이다.
        for v in [0u16, 1, 0x1FF] {
            assert_eq!(note_on(ch(1), pitch(60), Velocity::new(v)).bytes()[2], 1);
            assert_eq!(note_off(ch(1), pitch(60), Velocity::new(v)).bytes()[2], 0);
        }
        assert_eq!(
            note_on(ch(1), pitch(60), Velocity::new(0x200)).bytes()[2],
            1
        );
        assert_eq!(
            note_on(ch(1), pitch(60), Velocity::new(0xFFFF)).bytes()[2],
            127
        );
    }

    #[test]
    fn cc_and_bend_downscale_match_midi2_translation_table() {
        let cc: [(u32, u8); 7] = [
            (0, 0),
            (0x01FF_FFFF, 0),
            (0x0200_0000, 1),
            (0x7FFF_FFFF, 63),
            (0x8000_0000, 64),
            (0xFDFF_FFFF, 126),
            (0xFFFF_FFFF, 127),
        ];
        for (v, expected) in cc {
            assert_eq!(cc_to_7bit(v), expected, "v={v:#x}");
        }
        let bend: [(u32, u16); 6] = [
            (0, 0),
            (0x0003_FFFF, 0),
            (0x0004_0000, 1),
            (0x8000_0000, 0x2000),
            (0xFFFB_FFFF, 0x3FFE),
            (0xFFFF_FFFF, 0x3FFF),
        ];
        for (v, expected) in bend {
            assert_eq!(bend_to_14bit(v), expected, "v={v:#x}");
        }
        // 가운데 벤드는 E0 00 40.
        assert_eq!(pitch_bend(ch(1), 0x8000_0000).bytes(), &[0xE0, 0x00, 0x40]);
    }

    proptest! {
        #[test]
        fn upscale_then_downscale_round_trips_all_7bit_values(v in 0u8..=127) {
            let up = Velocity::from_midi1(v).unwrap();
            prop_assert_eq!(velocity_to_7bit(up.get()), v);
            let on = note_on(ch(1), pitch(60), up).bytes()[2];
            prop_assert_eq!(on, v.max(1));
        }

        #[test]
        fn ump_uses_only_types_1_and_2_with_matching_bytes(
            c in 1u8..=16, p in 0u8..=127, v in any::<u16>(), w in any::<u32>(),
            cc in 0u8..120, prog in 0u8..=127, bank in 0u16..=16383,
            spp in 0u16..16384, group in 0u8..16,
        ) {
            let mut msgs = vec![
                note_on(ch(c), pitch(p), Velocity::new(v)),
                note_off(ch(c), pitch(p), Velocity::new(v)),
                program_change(ch(c), Program::new(prog).unwrap()),
                control_change(ch(c), cc, w).unwrap(),
                pitch_bend(ch(c), w),
                timing_clock(), start(), continue_(), stop(),
                song_position(spp).unwrap(),
            ];
            msgs.extend(bank_select(ch(c), Bank::new(bank).unwrap()));
            msgs.extend(panic_messages(ch(c)));
            for m in msgs {
                let b = m.bytes();
                prop_assert!((1..=3).contains(&b.len()));
                prop_assert!(b[0] >= 0x80);
                prop_assert!(b[1..].iter().all(|&d| d < 0x80));
                let ump = m.ump(group);
                let expected_type = if b[0] >= 0xF0 { 0x1 } else { 0x2 };
                prop_assert_eq!(ump >> 28, expected_type);
                prop_assert_eq!((ump >> 24) & 0xF, u32::from(group));
                prop_assert_eq!(crate::ump_word_count(ump), 1);
                prop_assert_eq!((ump >> 16) & 0xFF, u32::from(b[0]));
                // 쓰지 않는 데이터 바이트는 0이다.
                let d1 = b.get(1).copied().unwrap_or(0);
                let d2 = b.get(2).copied().unwrap_or(0);
                prop_assert_eq!((ump >> 8) & 0xFF, u32::from(d1));
                prop_assert_eq!(ump & 0xFF, u32::from(d2));
            }
        }
    }

    #[test]
    fn note_and_controller_messages_encode_status_and_data() {
        assert_eq!(
            note_on(ch(1), pitch(60), Velocity::new(0xC924)).bytes(),
            &[0x90, 60, 100]
        );
        assert_eq!(
            note_off(ch(16), pitch(127), Velocity::new(0xFFFF)).bytes(),
            &[0x8F, 127, 127]
        );
        assert_eq!(
            note_on(ch(10), pitch(36), Velocity::new(0x8000)).bytes(),
            &[0x99, 36, 64]
        );
        assert_eq!(
            program_change(ch(3), Program::new(127).unwrap()).bytes(),
            &[0xC2, 127]
        );
        assert_eq!(
            control_change(ch(2), 7, 0xFFFF_FFFF).unwrap().bytes(),
            &[0xB1, 7, 127]
        );
        assert_eq!(
            control_change(ch(2), 119, 0).unwrap().bytes(),
            &[0xB1, 119, 0]
        );
        assert_eq!(pitch_bend(ch(16), 0xFFFF_FFFF).bytes(), &[0xEF, 0x7F, 0x7F]);
        // 14비트 0x1234 = msb 0x24, lsb 0x34.
        assert_eq!(pitch_bend(ch(1), 0x1234 << 18).bytes(), &[0xE0, 0x34, 0x24]);
        let [msb, lsb] = bank_select(ch(1), Bank::new(0x1234).unwrap());
        assert_eq!(msb.bytes(), &[0xB0, 0, 0x24]);
        assert_eq!(lsb.bytes(), &[0xB0, 32, 0x34]);
    }

    #[test]
    fn system_realtime_and_spp_encode() {
        assert_eq!(timing_clock().bytes(), &[0xF8]);
        assert_eq!(start().bytes(), &[0xFA]);
        assert_eq!(continue_().bytes(), &[0xFB]);
        assert_eq!(stop().bytes(), &[0xFC]);
        // 0x1234 = msb 0x24, lsb 0x34.
        assert_eq!(song_position(0x1234).unwrap().bytes(), &[0xF2, 0x34, 0x24]);
        assert_eq!(song_position(16383).unwrap().bytes(), &[0xF2, 0x7F, 0x7F]);
        assert_eq!(timing_clock().ump(3), 0x13F8_0000);
        assert_eq!(song_position(0x1234).unwrap().ump(0), 0x10F2_3424);
    }

    #[test]
    fn panic_messages_cover_all_sound_off_all_notes_off_sustain() {
        let m = panic_messages(ch(5));
        assert_eq!(m[0].bytes(), &[0xB4, 120, 0]);
        assert_eq!(m[1].bytes(), &[0xB4, 123, 0]);
        assert_eq!(m[2].bytes(), &[0xB4, 64, 0]);
    }

    #[test]
    fn invalid_controller_and_spp_are_errors() {
        for cc in 120..=127 {
            assert_eq!(
                control_change(ch(1), cc, 0),
                Err(MidiError::InvalidMessage),
                "cc={cc}"
            );
        }
        for cc in 128..=255 {
            assert_eq!(control_change(ch(1), cc, 0), Err(MidiError::InvalidMessage));
        }
        assert_eq!(song_position(16384), Err(MidiError::InvalidMessage));
        assert_eq!(song_position(u16::MAX), Err(MidiError::InvalidMessage));
    }
}
