//! S0-T2 적대적 리뷰(1회차)의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! `docs/reports/s0-t2/review-1.md` 3절의 시도와 짝을 이룬다. 깨뜨리는 데 성공한 테스트와
//! 회귀로 남길 가치가 있는 시도를 함께 둔다.

use keyscore_core::{
    Channel, Clip, ClipId, ModelError, Note, Pitch, Tick, Track, TrackColor, Velocity,
};
use proptest::prelude::*;

fn new_track() -> Track {
    Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap())
}

fn note(start: u64, length: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 시도 1(깨짐): 분할로 생긴 숨김 꼬리를 가진 클립을 u64 끝 근처로 옮기면
/// 노트의 절대 끝 틱(클립 시작 + 노트 끝)이 u64로 표현되지 않는다.
/// `move_clip`은 클립 길이만 보고 꼬리를 보지 않는다. 재생(S1)·내보내기(S0-T6)·
/// 트림으로 꼬리를 되살리는 일(S4-T2, 결정 Q1)이 이 값을 계산하는 순간 넘친다.
#[test]
fn move_clip_with_hidden_tail_near_u64_max_keeps_absolute_note_end_in_u64() {
    let mut track = new_track();
    let id = track
        .insert_clip(Clip::new(Tick::ZERO, 100).unwrap())
        .unwrap();
    track.insert_note(id, note(0, 100, 60)).unwrap();
    // 왼쪽 [0, 50)에 노트 [0, 100)이 숨김 꼬리로 남는다.
    track.split_clip(id, Tick::new(50)).unwrap();
    // 클립 끝 = (MAX - 50) + 50 = MAX이므로 클립 불변식은 통과한다.
    let moved = track.move_clip(id, Tick::new(u64::MAX - 50));
    for (_, clip) in track.clips() {
        for n in clip.notes() {
            assert!(
                clip.start().get().checked_add(n.end()).is_some(),
                "move_clip 결과 {moved:?}: 클립 시작 {} + 노트 끝 {}가 u64를 넘는다",
                clip.start().get(),
                n.end()
            );
        }
    }
}

/// 시도 2(못 깨뜨림): u64 끝 경계에서의 이동은 오버플로를 `Err`로 돌려주고 상태를 바꾸지 않는다.
#[test]
fn move_clip_past_u64_max_is_rejected_and_state_unchanged() {
    let mut track = new_track();
    let id = track
        .insert_clip(Clip::new(Tick::ZERO, 10).unwrap())
        .unwrap();
    let before = track.clone();
    assert_eq!(
        track.move_clip(id, Tick::new(u64::MAX - 9)),
        Err(ModelError::TickOverflow)
    );
    assert_eq!(track, before);
    // 정확히 끝에 닿는 것은 된다.
    track.move_clip(id, Tick::new(u64::MAX - 10)).unwrap();
    assert_eq!(track.clip(id).unwrap().end(), u64::MAX);
}

/// 시도 3(못 깨뜨림): 숨김 꼬리 노트를 같은 클립에 다시 넣거나, 꼬리를 가진 클립을
/// 빼서 다른 트랙에 넣은 뒤 노트를 넣어도 `insert_note`는 끝까지 클립 안만 받는다.
#[test]
fn hidden_tail_cannot_be_created_by_insert_note_after_split() {
    let mut a = new_track();
    let id = a.insert_clip(Clip::new(Tick::ZERO, 100).unwrap()).unwrap();
    a.insert_note(id, note(10, 90, 60)).unwrap();
    a.split_clip(id, Tick::new(50)).unwrap();
    let tail = a.clip(id).unwrap().notes()[0].clone();
    assert!(tail.end() > a.clip(id).unwrap().length());
    let before = a.clone();
    assert!(matches!(
        a.insert_note(id, tail.clone()),
        Err(ModelError::NoteOutsideClip { .. })
    ));
    assert_eq!(a, before);

    let mut moved = a.remove_clip(id).unwrap();
    assert!(matches!(
        moved.insert_note(note(40, 20, 62)),
        Err(ModelError::NoteOutsideClip { .. })
    ));
    let mut b = new_track();
    let bid = b.insert_clip(moved).unwrap();
    assert!(b.insert_note(bid, note(40, 20, 62)).is_err());
}

/// 시도 4: 리뷰 1 당시 `ClipId`는 트랙마다 0부터 발급되어 다른 트랙의 식별자가 이 트랙의
/// 엉뚱한 클립을 가리켰다. 제안 4를 받아 식별자를 전역으로 유일하게 발급하므로 이제 지워지지 않는다.
#[test]
fn clip_id_from_another_track_addresses_unrelated_clip() {
    let mut a = new_track();
    let mut b = new_track();
    let id_a = a.insert_clip(Clip::new(Tick::ZERO, 10).unwrap()).unwrap();
    let _id_b = b
        .insert_clip(Clip::new(Tick::new(1000), 10).unwrap())
        .unwrap();
    let removed = b.remove_clip(id_a);
    assert_eq!(removed.map(|c| c.start().get()), None);
}

#[derive(Debug, Clone)]
enum Op {
    Insert {
        start: u64,
        length: u64,
    },
    Move {
        pick: usize,
        start: u64,
    },
    Split {
        pick: usize,
        frac: u64,
    },
    Note {
        pick: usize,
        start: u64,
        length: u32,
    },
}

/// u64 끝 근처에 몰리도록 고른 틱. 기존 속성 테스트는 `0..20_000`만 본다.
fn edge_tick() -> impl Strategy<Value = u64> {
    prop_oneof![
        0..1_000u64,
        (0..1_000u64).prop_map(|d| u64::MAX - d),
        any::<u64>(),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (edge_tick(), prop_oneof![1..500u64, any::<u64>()])
            .prop_map(|(start, length)| Op::Insert { start, length }),
        (any::<usize>(), edge_tick()).prop_map(|(pick, start)| Op::Move { pick, start }),
        (any::<usize>(), any::<u64>()).prop_map(|(pick, frac)| Op::Split { pick, frac }),
        (any::<usize>(), 0..500u64, 1..500u32).prop_map(|(pick, start, length)| Op::Note {
            pick,
            start,
            length
        }),
    ]
}

fn pick(track: &Track, i: usize) -> Option<ClipId> {
    let ids: Vec<_> = track.clips().map(|(id, _)| id).collect();
    (!ids.is_empty()).then(|| ids[i % ids.len()])
}

fn apply(track: &mut Track, op: &Op) -> Result<(), ModelError> {
    match *op {
        Op::Insert { start, length } => track
            .insert_clip(Clip::new(Tick::new(start), length)?)
            .map(|_| ()),
        Op::Move { pick: p, start } => match pick(track, p) {
            Some(id) => track.move_clip(id, Tick::new(start)),
            None => Ok(()),
        },
        Op::Split { pick: p, frac } => match pick(track, p) {
            Some(id) => {
                let c = track.clip(id).unwrap();
                let at = c.start().get() + frac % c.length();
                track.split_clip(id, Tick::new(at)).map(|_| ())
            }
            None => Ok(()),
        },
        Op::Note {
            pick: p,
            start,
            length,
        } => match pick(track, p) {
            Some(id) => track.insert_note(id, note(start, length, 60)),
            None => Ok(()),
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// 시도 5(못 깨뜨림): u64 끝 경계에 몰린 수열에서도 겹침 없음, 노트 시작은 클립 안,
    /// 클립 끝 오버플로 없음, 실패한 연산은 상태 불변이 성립한다.
    #[test]
    fn edge_tick_ops_preserve_invariants_and_failed_ops_leave_state(ops in prop::collection::vec(op(), 0..60)) {
        let mut track = new_track();
        for op in &ops {
            let before = track.clone();
            let result = apply(&mut track, op);
            if result.is_err() {
                prop_assert_eq!(&track, &before, "실패한 {:?}가 상태를 바꿨다", op);
            }
            let clips: Vec<&Clip> = track.clips().map(|(_, c)| c).collect();
            for c in &clips {
                prop_assert!(c.length() >= 1);
                prop_assert!(c.start().get().checked_add(c.length()).is_some());
                for n in c.notes() {
                    prop_assert!(n.start().get() < c.length(), "{:?} 뒤 노트 시작이 클립 밖", op);
                }
            }
            for w in clips.windows(2) {
                prop_assert!(w[0].end() <= w[1].start().get(), "{:?} 뒤 겹침", op);
            }
        }
    }
}
