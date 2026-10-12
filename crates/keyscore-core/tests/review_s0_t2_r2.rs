//! S0-T2 적대적 리뷰(2회차)의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! `docs/reports/s0-t2/review-2.md` 3절의 시도와 짝을 이룬다. 모두 깨뜨리지 못한 시도이며,
//! 결정 Q2·Q3과 `ClipId` 전역 발급의 회귀 테스트로 남긴다.

use std::collections::HashSet;
use std::thread;

use keyscore_core::{
    Channel, Clip, ClipId, ModelError, PitchOffset, Tick, Track, TrackColor, Velocity,
};
use proptest::prelude::*;

fn new_track() -> Track {
    Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap())
}

/// MIDI 2.0 명세(UMP and MIDI 2.0 Protocol, 부록 「Translation」)의 일반형 scaleUp을
/// 구현과 독립적으로 옮긴 것. 비트 수를 인자로 받아 구현의 7→16 고정 상수와 다른 경로로 계산한다.
fn spec_scale_up(src: u32, src_bits: u32, dst_bits: u32) -> u32 {
    let scale_bits = dst_bits - src_bits;
    let mut out = src << scale_bits;
    let center = 1 << (src_bits - 1);
    if src <= center {
        return out;
    }
    let repeat_bits = src_bits - 1;
    let repeat_mask = (1 << repeat_bits) - 1;
    let mut repeat = src & repeat_mask;
    if scale_bits > repeat_bits {
        repeat <<= scale_bits - repeat_bits;
    } else {
        repeat >>= repeat_bits - scale_bits;
    }
    while repeat != 0 {
        out |= repeat;
        repeat >>= repeat_bits;
    }
    out
}

/// 명세의 scaleDown은 단순 오른쪽 시프트다.
fn spec_scale_down(src: u32, src_bits: u32, dst_bits: u32) -> u32 {
    src >> (src_bits - dst_bits)
}

/// 시도 1(못 깨뜨림): 7→16 변환이 0–127 전체에서 명세의 scaleUp과 같고, 단조 증가하며,
/// 양 끝(0, 127)과 중앙(64)이 0, 0xFFFF, 0x8000으로 간다.
#[test]
fn velocity_from_midi1_matches_spec_scale_up_for_every_7bit_value() {
    let mut prev: Option<u16> = None;
    for x in 0..=127u8 {
        let got = Velocity::from_midi1(x).unwrap().get();
        let want = spec_scale_up(u32::from(x), 7, 16);
        assert_eq!(u32::from(got), want, "from_midi1({x})");
        if let Some(p) = prev {
            assert!(got > p, "단조 증가가 아니다: {x}");
        }
        prev = Some(got);
    }
    assert_eq!(Velocity::from_midi1(0).unwrap().get(), 0);
    assert_eq!(Velocity::from_midi1(64).unwrap().get(), 0x8000);
    assert_eq!(Velocity::from_midi1(127).unwrap().get(), 0xFFFF);
    for x in 128..=255u8 {
        assert_eq!(
            Velocity::from_midi1(x),
            Err(ModelError::Midi1VelocityOutOfRange(x))
        );
    }
}

/// 시도 2(못 깨뜨림): 16→7 변환이 u16 전체에서 명세의 scaleDown(상위 7비트)과 같다.
/// 하위 비트 절삭(`& 0x7F`)이면 여기서 깨진다.
#[test]
fn velocity_to_midi1_matches_spec_scale_down_for_every_16bit_value() {
    for v in 0..=u16::MAX {
        let got = Velocity::new(v).to_midi1();
        assert_eq!(
            u32::from(got),
            spec_scale_down(u32::from(v), 16, 7),
            "to_midi1({v:#06x})"
        );
    }
}

/// 시도 3(못 깨뜨림): 피치 오프셋 경계가 결정 Q3(반음 = 2^25 단위, ±12반음 포함)과 정확히 맞는다.
#[test]
fn pitch_offset_bounds_match_decision_q3_exactly() {
    assert_eq!(PitchOffset::SEMITONE, 33_554_432);
    assert_eq!(PitchOffset::MAX, 402_653_184);
    let max = PitchOffset::MAX;
    for ok in [0, 1, -1, max, -max, max - 1, -max + 1] {
        assert_eq!(PitchOffset::new(ok).map(PitchOffset::units), Ok(ok), "{ok}");
    }
    for bad in [
        max + 1,
        -max - 1,
        i32::MAX,
        i32::MIN,
        i32::MIN + 1,
        i32::MAX - 1,
    ] {
        assert_eq!(
            PitchOffset::new(bad),
            Err(ModelError::PitchOffsetOutOfRange(bad)),
            "{bad}"
        );
    }
}

/// 시도 4(못 깨뜨림): 여러 스레드가 동시에 클립을 넣어도 식별자가 겹치지 않는다.
/// 전역 카운터를 공유하는 병렬 테스트 환경과 같은 조건이다.
#[test]
fn clip_ids_are_unique_across_threads() {
    let handles: Vec<_> = (0..8)
        .map(|_| {
            thread::spawn(|| {
                let mut t = new_track();
                let mut ids = Vec::new();
                for i in 0..500u64 {
                    let id = t
                        .insert_clip(Clip::new(Tick::new(i * 10), 10).unwrap())
                        .unwrap();
                    ids.push(id);
                    if i % 3 == 0 {
                        ids.push(t.split_clip(id, Tick::new(i * 10 + 5)).unwrap());
                    }
                }
                ids
            })
        })
        .collect();
    let mut seen = HashSet::new();
    for h in handles {
        for id in h.join().unwrap() {
            assert!(seen.insert(id), "중복 식별자 {id:?}");
        }
    }
}

#[derive(Debug, Clone)]
enum Op {
    Insert {
        to_b: bool,
        start: u64,
        length: u64,
    },
    Split {
        to_b: bool,
        pick: usize,
        frac: u64,
    },
    /// 한쪽 트랙에서 꺼낸 클립을 다른 쪽(또는 같은 쪽)에 다시 넣는다.
    Transfer {
        from_b: bool,
        to_b: bool,
        pick: usize,
    },
    /// B를 A의 복제로 바꾼다. 이후 두 트랙은 같은 식별자를 공유한다.
    CloneAIntoB,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (any::<bool>(), 0..5_000u64, 1..500u64)
            .prop_map(|(to_b, start, length)| Op::Insert { to_b, start, length }),
        2 => (any::<bool>(), any::<usize>(), any::<u64>())
            .prop_map(|(to_b, pick, frac)| Op::Split { to_b, pick, frac }),
        3 => (any::<bool>(), any::<bool>(), any::<usize>())
            .prop_map(|(from_b, to_b, pick)| Op::Transfer { from_b, to_b, pick }),
        1 => Just(Op::CloneAIntoB),
    ]
}

fn pick(track: &Track, i: usize) -> Option<ClipId> {
    let ids: Vec<_> = track.clips().map(|(id, _)| id).collect();
    (!ids.is_empty()).then(|| ids[i % ids.len()])
}

fn ids_unique(track: &Track) -> bool {
    let ids: Vec<ClipId> = track.clips().map(|(id, _)| id).collect();
    ids.iter().collect::<HashSet<_>>().len() == ids.len()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 시도 5(못 깨뜨림): 복제된 트랙 사이에서 클립을 꺼내 옮기고, 같은 트랙에 다시 넣고,
    /// 분할해도 한 트랙 안에 같은 `ClipId`가 둘 생기지 않는다. 다시 넣은 클립은 새 식별자를 받는다.
    #[test]
    fn clip_transfer_between_cloned_tracks_never_duplicates_id_within_track(
        ops in prop::collection::vec(op(), 0..80)
    ) {
        let mut a = new_track();
        let mut b = new_track();
        for op in &ops {
            match *op {
                Op::Insert { to_b, start, length } => {
                    let t = if to_b { &mut b } else { &mut a };
                    let _ = t.insert_clip(Clip::new(Tick::new(start), length).unwrap());
                }
                Op::Split { to_b, pick: p, frac } => {
                    let t = if to_b { &mut b } else { &mut a };
                    if let Some(id) = pick(t, p) {
                        let c = t.clip(id).unwrap();
                        let at = c.start().get() + frac % c.length();
                        let _ = t.split_clip(id, Tick::new(at));
                    }
                }
                Op::Transfer { from_b, to_b, pick: p } => {
                    let src = if from_b { &mut b } else { &mut a };
                    if let Some(id) = pick(src, p) {
                        let clip = src.remove_clip(id).unwrap();
                        let dst = if to_b { &mut b } else { &mut a };
                        if let Ok(new_id) = dst.insert_clip(clip) {
                            prop_assert_ne!(new_id, id, "다시 넣은 클립이 옛 식별자를 받았다");
                        }
                    }
                }
                Op::CloneAIntoB => b = a.clone(),
            }
            prop_assert!(ids_unique(&a), "{:?} 뒤 A에 중복 식별자", op);
            prop_assert!(ids_unique(&b), "{:?} 뒤 B에 중복 식별자", op);
        }
    }
}
