//! S0-T2 적대적 리뷰(3회차)의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! `docs/reports/s0-t2/review-3.md` 3절과 짝을 이룬다. 2회차 이후 바뀐 `move_clip`
//! (제자리 `set_start` 뒤 재정렬)을 기준 모델과 비교한다. 모두 깨뜨리지 못한 시도이며 회귀로 남긴다.

use keyscore_core::{
    Channel, Clip, ClipId, ModelError, Note, Pitch, Tick, Track, TrackColor, Velocity,
};
use proptest::prelude::*;

fn new_track() -> Track {
    Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap())
}

fn note(start: u64, length: u32) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(60).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn layout(track: &Track) -> Vec<(u64, u64)> {
    track
        .clips()
        .map(|(_, c)| (c.start().get(), c.length()))
        .collect()
}

fn assert_sorted_disjoint(track: &Track) {
    let l = layout(track);
    for w in l.windows(2) {
        assert!(w[0].0 + w[0].1 <= w[1].0, "정렬·겹침 위반: {l:?}");
    }
}

/// 시도 1: 클립 셋 [0,10) [20,30) [40,50) 각각을 모든 시작 위치(자기 자리, 맨 앞, 맨 뒤, 사이,
/// 경계에 맞닿는 곳)로 옮기고 기준 모델과 비교한다. 식별자·노트가 유지되어야 한다.
#[test]
fn move_clip_every_position_matches_reference_model() {
    for which in 0..3usize {
        for target in 0..=60u64 {
            let mut track = new_track();
            let ids: Vec<ClipId> = [0u64, 20, 40]
                .iter()
                .map(|&s| {
                    let id = track
                        .insert_clip(Clip::new(Tick::new(s), 10).unwrap())
                        .unwrap();
                    track.insert_note(id, note(1, 3)).unwrap();
                    id
                })
                .collect();
            let before = track.clone();
            let mut model: Vec<(u64, u64)> = vec![(0, 10), (20, 10), (40, 10)];
            let r = track.move_clip(ids[which], Tick::new(target));
            let others: Vec<(u64, u64)> = model
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != which)
                .map(|(_, c)| *c)
                .collect();
            let overlaps = others
                .iter()
                .any(|&(s, l)| s < target + 10 && target < s + l);
            if overlaps {
                assert!(
                    matches!(r, Err(ModelError::ClipOverlap { .. })),
                    "which={which} t={target}"
                );
                assert_eq!(
                    track, before,
                    "실패했는데 상태가 바뀜 which={which} t={target}"
                );
            } else {
                r.unwrap();
                model[which].0 = target;
                model.sort_unstable();
                assert_eq!(layout(&track), model, "which={which} t={target}");
                let c = track.clip(ids[which]).unwrap();
                assert_eq!(c.start().get(), target);
                assert_eq!(c.notes().len(), 1, "노트 유실");
                assert_eq!(track.clips().count(), 3);
            }
            assert_sorted_disjoint(&track);
        }
    }
}

/// 시도 2: 오버플로 실패(끝이 u64 초과, 숨김 꼬리 초과)는 어느 위치의 클립이든 상태를 바꾸지 않는다.
#[test]
fn move_clip_overflow_failure_keeps_track_identical_for_every_index() {
    for which in 0..3usize {
        let mut track = new_track();
        let ids: Vec<ClipId> = [0u64, 200, 400]
            .iter()
            .map(|&s| {
                track
                    .insert_clip(Clip::new(Tick::new(s), 100).unwrap())
                    .unwrap()
            })
            .collect();
        track.insert_note(ids[which], note(0, 100)).unwrap();
        track
            .split_clip(
                ids[which],
                Tick::new(if which == 0 {
                    50
                } else {
                    [0, 200, 400][which] + 50
                }),
            )
            .unwrap();
        let before = track.clone();
        // 길이는 들어가지만 숨김 꼬리가 넘치는 위치와, 길이 자체가 넘치는 위치
        for target in [u64::MAX - 50, u64::MAX - 49, u64::MAX] {
            let r = track.move_clip(ids[which], Tick::new(target));
            if r.is_err() {
                assert_eq!(track, before, "which={which} target={target}");
            }
        }
        assert_eq!(track.clips().count(), 4);
    }
}

#[derive(Debug, Clone)]
enum Step {
    Move { pick: usize, to: u64 },
    Insert { start: u64, len: u64 },
}

proptest! {
    /// 시도 3: 무작위 이동·삽입 열을 기준 모델(Vec 정렬)과 단계마다 비교한다.
    #[test]
    fn move_and_insert_sequences_match_reference_model(
        steps in prop::collection::vec(
            prop_oneof![
                (any::<usize>(), 0u64..300).prop_map(|(pick, to)| Step::Move { pick, to }),
                (0u64..300, 1u64..40).prop_map(|(start, len)| Step::Insert { start, len }),
            ],
            0..60,
        )
    ) {
        let mut track = new_track();
        let mut model: Vec<(ClipId, u64, u64)> = Vec::new();
        for s in steps {
            match s {
                Step::Insert { start, len } => {
                    let free = !model.iter().any(|&(_, s0, l0)| s0 < start + len && start < s0 + l0);
                    let r = track.insert_clip(Clip::new(Tick::new(start), len).unwrap());
                    prop_assert_eq!(r.is_ok(), free);
                    if let Ok(id) = r { model.push((id, start, len)); }
                }
                Step::Move { pick, to } => {
                    if model.is_empty() { continue; }
                    let i = pick % model.len();
                    let (id, _, len) = model[i];
                    let free = !model.iter().enumerate().any(|(j, &(_, s0, l0))| j != i && s0 < to + len && to < s0 + l0);
                    let before = track.clone();
                    let r = track.move_clip(id, Tick::new(to));
                    prop_assert_eq!(r.is_ok(), free);
                    if r.is_ok() { model[i].1 = to; } else { prop_assert_eq!(&track, &before); }
                }
            }
            model.sort_by_key(|&(_, s, _)| s);
            let want: Vec<(ClipId, u64, u64)> = model.clone();
            let got: Vec<(ClipId, u64, u64)> =
                track.clips().map(|(id, c)| (id, c.start().get(), c.length())).collect();
            prop_assert_eq!(got, want);
        }
    }
}
