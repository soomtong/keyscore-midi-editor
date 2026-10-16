//! S1-T6 적대적 리뷰 2: core `Track::check_free`를 이분 탐색으로 바꾼 뒤에도 겹침 판정과 불변식이
//! 전수 비교(모든 클립과 구간 교차)와 같은지 공개 API로 확인한다(리뷰 1 지적 3의 core 변경).

use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Project, Tick, Track, TrackColor,
};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Insert { start: u64, length: u64 },
    Move { pick: usize, to: u64 },
    Split { pick: usize, offset: u64 },
    Remove { pick: usize },
}

fn op() -> impl Strategy<Value = Op> {
    // 구간을 좁혀 겹침·맞닿음이 자주 나오게 한다.
    prop_oneof![
        4 => (0..400u64, 1..60u64).prop_map(|(start, length)| Op::Insert { start, length }),
        3 => (any::<usize>(), 0..400u64).prop_map(|(pick, to)| Op::Move { pick, to }),
        2 => (any::<usize>(), 0..60u64).prop_map(|(pick, offset)| Op::Split { pick, offset }),
        1 => any::<usize>().prop_map(|pick| Op::Remove { pick }),
    ]
}

fn spans(track: &Track) -> Vec<(ClipId, u64, u64)> {
    track
        .clips()
        .map(|(id, c)| (id, c.start().get(), c.end()))
        .collect()
}

/// 전수 비교: `ignore`를 뺀 어떤 클립과도 `[start, end)`가 겹치는가.
fn brute_overlap(spans: &[(ClipId, u64, u64)], start: u64, end: u64, ignore: &[ClipId]) -> bool {
    spans
        .iter()
        .filter(|(id, _, _)| !ignore.contains(id))
        .any(|&(_, s, e)| s < end && start < e)
}

fn check_invariant(track: &Track) -> Result<(), String> {
    let s = spans(track);
    for w in s.windows(2) {
        if w[0].1 > w[1].1 {
            return Err(format!("시작 순이 아니다: {w:?}"));
        }
        if w[0].2 > w[1].1 {
            return Err(format!("겹친다: {w:?}"));
        }
    }
    Ok(())
}

fn track() -> Track {
    Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 넣기·옮기기의 성공 여부가 전수 비교와 같고, 실패하면 트랙이 그대로이며, 모든 단계에서 클립은
    /// 시작 순·비겹침이다.
    #[test]
    fn check_free_matches_brute_force_for_random_track_ops(
        ops in prop::collection::vec(op(), 1..80),
    ) {
        let mut t = track();
        for op in ops {
            let before = spans(&t);
            let ids: Vec<ClipId> = before.iter().map(|s| s.0).collect();
            match op {
                Op::Insert { start, length } => {
                    let want_hit = brute_overlap(&before, start, start + length, &[]);
                    let got = t.insert_clip(Clip::new(Tick::new(start), length).unwrap());
                    match got {
                        Ok(_) => prop_assert!(!want_hit, "겹치는데 넣었다 {start}+{length} {before:?}"),
                        Err(ModelError::ClipOverlap { .. }) => {
                            prop_assert!(want_hit, "안 겹치는데 거부 {start}+{length} {before:?}");
                            prop_assert_eq!(spans(&t), before);
                        }
                        Err(e) => prop_assert!(false, "예상 밖 오류 {e:?}"),
                    }
                }
                Op::Move { pick, to } if !ids.is_empty() => {
                    let id = ids[pick % ids.len()];
                    let len = t.clip(id).unwrap().length();
                    let want_hit = brute_overlap(&before, to, to + len, &[id]);
                    let got = t.move_clip(id, Tick::new(to));
                    prop_assert_eq!(got.is_err(), want_hit, "옮기기 {:?} → {}", id, to);
                    if want_hit {
                        prop_assert_eq!(spans(&t), before);
                    }
                }
                Op::Split { pick, offset } if !ids.is_empty() => {
                    let id = ids[pick % ids.len()];
                    let (s, e) = { let c = t.clip(id).unwrap(); (c.start().get(), c.end()) };
                    let at = s + offset;
                    let got = t.split_clip(id, Tick::new(at));
                    prop_assert_eq!(got.is_ok(), at > s && at < e);
                }
                Op::Remove { pick } if !ids.is_empty() => {
                    prop_assert!(t.remove_clip(ids[pick % ids.len()]).is_some());
                }
                _ => {}
            }
            if let Err(why) = check_invariant(&t) {
                prop_assert!(false, "{}", why);
            }
        }
    }

    /// 분할을 undo하면 `unsplit_clip`이 `check_free(&original, &[left, right])`를 거친다. 이웃 클립이
    /// 맞닿아 있어도 되돌리기·다시 하기가 상태를 정확히 되살린다.
    #[test]
    fn split_undo_redo_with_adjacent_clips_restores_state(
        lens in prop::collection::vec(2..50u64, 1..30),
        splits in prop::collection::vec((any::<usize>(), 1..49u64), 1..30),
    ) {
        let mut p = Project::new();
        let mut t = track();
        let mut at = 0;
        for len in &lens {
            // 맞닿은 클립들: 겹침 판정의 경계(`end == start`)를 계속 밟는다.
            t.insert_clip(Clip::new(Tick::new(at), *len).unwrap()).unwrap();
            at += len;
        }
        let tid = t.id();
        p.push_track(t).unwrap();
        let mut d = Document::new(p, usize::MAX).unwrap();
        let mut states = vec![d.project().clone()];
        for (pick, offset) in splits {
            let tr = &d.project().tracks()[0];
            let ids: Vec<ClipId> = tr.clips().map(|(id, _)| id).collect();
            let id = ids[pick % ids.len()];
            let c = tr.clip(id).unwrap();
            if offset >= c.length() {
                continue;
            }
            let at = Tick::new(c.start().get() + offset);
            d.apply(vec![Edit::SplitClip { track: tid, id, at }], None).unwrap();
            if let Err(why) = check_invariant(&d.project().tracks()[0]) {
                prop_assert!(false, "{}", why);
            }
            states.push(d.project().clone());
        }
        for want in states.iter().rev().skip(1) {
            prop_assert!(d.undo().unwrap());
            prop_assert_eq!(d.project(), want);
        }
        for want in states.iter().skip(1) {
            prop_assert!(d.redo().unwrap());
            prop_assert_eq!(d.project(), want);
        }
    }
}
