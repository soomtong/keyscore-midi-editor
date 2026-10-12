//! S0-T7 적대적 리뷰 3회차의 깨뜨리기 테스트. 공개 API만 쓴다.

use keyscore_core::transform::{self, Grid, NoteRef, PitchRange, QuantizeSettings};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Note, Pitch, Project, Tick, TimeSignature,
    Track, TrackColor, TrackId, Velocity,
};
use proptest::prelude::*;

fn note(start: u64, length: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::new(1),
    )
    .unwrap()
}

fn new_track() -> Track {
    Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap())
}

fn clip_of(p: &Project, t: TrackId, c: ClipId) -> &Clip {
    p.track(t).unwrap().clip(c).unwrap()
}

fn tails(clip: &Clip) -> usize {
    clip.notes()
        .iter()
        .filter(|n| n.end() > clip.length())
        .count()
}

/// 길이 `length`의 클립 하나(분할 없음, 꼬리 없음)와, 있으면 `neighbor_at`에서 시작하는 이웃 클립.
fn doc_one_clip(
    length: u64,
    notes: &[Note],
    neighbor_at: Option<u64>,
) -> (Document, TrackId, ClipId) {
    let mut p = Project::new();
    let mut t = new_track();
    let tid = t.id();
    let mut clip = Clip::new(Tick::ZERO, length).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    let c = t.insert_clip(clip).unwrap();
    if let Some(at) = neighbor_at {
        t.insert_clip(Clip::new(Tick::new(at), 3840).unwrap())
            .unwrap();
    }
    p.push_track(t).unwrap();
    (Document::new(p, usize::MAX).unwrap(), tid, c)
}

// ---------------------------------------------------------------------------
// 공개 Edit::ReplaceNotes
// ---------------------------------------------------------------------------

/// 꼬리 허용은 「pair의 from이 편집 직전 길이를 넘는가」로 판정한다. 그런데 from은 앞 pair가 막
/// 만든 값일 수 있다. 일반 노트를 늘린 구간(편집 직전 길이 너머, 새 길이 안)으로 옮긴 뒤, 그 값을
/// 다음 pair의 from으로 쓰면 「원래 꼬리」로 판정되어 새 길이를 넘는 꼬리가 된다. 분할 없이 꼬리가
/// 생기고 꼬리 수가 는다(README·`clip.rs` 불변식 「꼬리의 개수는 분할로만 는다」).
#[test]
fn review_r3_replace_notes_chain_turns_plain_note_into_tail() {
    let plain = note(0, 100, 60);
    let (mut doc, t, c) = doc_one_clip(3840, std::slice::from_ref(&plain), None);
    assert_eq!(tails(clip_of(doc.project(), t, c)), 0);
    // 1단계: 3800..4800. 편집 직전 길이 3840은 넘지만 새 길이 7680 안이라 일반 노트로 허용된다.
    let mid = note(3800, 1000, 60);
    // 2단계: from = mid(끝 4800 > 3840)라 「원래 꼬리」로 판정된다. 끝 3800 + 100000 > 7680.
    let tail = note(3800, 100_000, 60);
    let r = doc.apply(
        vec![Edit::ReplaceNotes {
            track: t,
            clip: c,
            length: 7680,
            pairs: vec![(plain, mid.clone()), (mid, tail)],
        }],
        None,
    );
    let clip = clip_of(doc.project(), t, c);
    assert!(
        r.is_err() && tails(clip) == 0,
        "분할 없이 일반 노트가 꼬리가 됐다: r = {r:?}, 길이 {}, 노트 {:?}",
        clip.length(),
        clip.notes()
    );
}

/// 같은 구멍을 두 편집이 아니라 한 묶음 안의 두 노트로: 노트 A를 B의 자리(늘린 구간)로 옮기는 pair
/// 뒤에 그 값을 from으로 쓰는 pair. 같은 값의 노트가 클립에 원래 있었는지와 무관하게 통과한다.
#[test]
fn review_r3_replace_notes_chain_order_matters() {
    let a = note(0, 100, 60);
    let (mut doc, t, c) = doc_one_clip(3840, std::slice::from_ref(&a), None);
    let mid = note(3839, 2, 60); // 끝 3841: 편집 직전 길이 3840을 1틱 넘는다.
    let tail = note(3839, u32::MAX, 60);
    // 순서를 바꾼 묶음(mid → tail을 먼저)은 from이 없으므로 NoteNotFound여야 한다.
    let reversed = doc.apply(
        vec![Edit::ReplaceNotes {
            track: t,
            clip: c,
            length: 3841,
            pairs: vec![(mid.clone(), tail.clone()), (a.clone(), mid.clone())],
        }],
        None,
    );
    assert_eq!(reversed, Err(ModelError::NoteNotFound));
    let r = doc.apply(
        vec![Edit::ReplaceNotes {
            track: t,
            clip: c,
            length: 3841,
            pairs: vec![(a, mid.clone()), (mid, tail)],
        }],
        None,
    );
    assert!(
        r.is_err(),
        "pair 순서에 따라 일반 노트가 꼬리가 된다: {:?}",
        clip_of(doc.project(), t, c).notes()
    );
}

/// 꼬리가 아닌 노트를 새 길이 밖으로, 시작을 새 길이 밖(= 길이)으로: 둘 다 거부, 상태·undo 불변.
#[test]
fn review_r3_replace_notes_rejects_plain_overhang_and_outside_start() {
    let a = note(0, 100, 60);
    let (mut doc, t, c) = doc_one_clip(3840, std::slice::from_ref(&a), None);
    let before = doc.project().clone();
    for to in [note(7600, 100, 60), note(7680, 1, 60), note(3840, 1, 60)] {
        let length = if to.start().get() == 3840 { 3840 } else { 7680 };
        let r = doc.apply(
            vec![Edit::ReplaceNotes {
                track: t,
                clip: c,
                length,
                pairs: vec![(a.clone(), to.clone())],
            }],
            None,
        );
        assert!(
            matches!(r, Err(ModelError::NoteOutsideClip { .. })),
            "{to:?}: {r:?}"
        );
        assert_eq!(doc.project(), &before);
        assert_eq!(doc.undo_len(), 0);
    }
}

/// 길이 줄이기, 0 길이, 맞닿은 이웃까지 늘리기(허용), 이웃을 1틱 넘기(거부), u64 끝.
#[test]
fn review_r3_replace_notes_length_rules() {
    let (mut doc, t, c) = doc_one_clip(3840, &[note(0, 100, 60)], Some(7680));
    let before = doc.project().clone();
    for (length, ok) in [(3839, false), (0, false), (7681, false), (7680, true)] {
        let r = doc.apply(
            vec![Edit::ReplaceNotes {
                track: t,
                clip: c,
                length,
                pairs: vec![],
            }],
            None,
        );
        assert_eq!(r.is_ok(), ok, "length {length}: {r:?}");
        if !ok {
            assert_eq!(doc.project(), &before);
        }
    }
    assert_eq!(clip_of(doc.project(), t, c).length(), 7680);
    assert!(doc.undo().unwrap());
    assert_eq!(doc.project(), &before);

    // u64 끝: 시작 u64::MAX − 3840인 클립을 u64를 넘게 늘리기, 꼬리 끝이 u64를 넘기.
    let mut p = Project::new();
    let mut tr = new_track();
    let tid = tr.id();
    let start = u64::MAX - 3840;
    let mut clip = Clip::new(Tick::new(start), 3840).unwrap();
    clip.insert_note(note(0, 100, 60)).unwrap();
    let cid = tr.insert_clip(clip).unwrap();
    p.push_track(tr).unwrap();
    let mut doc = Document::new(p, usize::MAX).unwrap();
    let before = doc.project().clone();
    let r = doc.apply(
        vec![Edit::ReplaceNotes {
            track: tid,
            clip: cid,
            length: 3841,
            pairs: vec![],
        }],
        None,
    );
    assert!(r.is_err());
    let r = doc.apply(
        vec![Edit::ReplaceNotes {
            track: tid,
            clip: cid,
            length: 3840,
            pairs: vec![(note(0, 100, 60), note(3800, 41, 60))],
        }],
        None,
    );
    assert!(r.is_err(), "{r:?}");
    assert_eq!(doc.project(), &before);
}

/// 같은 값 노트가 셋일 때 같은 from을 여러 번: 셋까지 허용, 넷은 `NoteNotFound`. undo/redo 왕복.
#[test]
fn review_r3_replace_notes_duplicate_from_and_round_trip() {
    let n = note(10, 10, 60);
    let mut p = Project::new();
    let mut tr = new_track();
    let tid = tr.id();
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for _ in 0..3 {
        clip.insert_note(n.clone()).unwrap();
    }
    let cid = tr.insert_clip(clip).unwrap();
    p.push_track(tr).unwrap();
    let mut doc = Document::new(p, usize::MAX).unwrap();
    let before = doc.project().clone();
    let four = vec![
        (n.clone(), note(20, 10, 60)),
        (n.clone(), note(30, 10, 60)),
        (n.clone(), note(40, 10, 60)),
        (n.clone(), note(50, 10, 60)),
    ];
    let r = doc.apply(
        vec![Edit::ReplaceNotes {
            track: tid,
            clip: cid,
            length: 3840,
            pairs: four.clone(),
        }],
        None,
    );
    assert_eq!(r, Err(ModelError::NoteNotFound));
    assert_eq!(doc.project(), &before);
    doc.apply(
        vec![Edit::ReplaceNotes {
            track: tid,
            clip: cid,
            length: 7680,
            pairs: four[..3].to_vec(),
        }],
        None,
    )
    .unwrap();
    let after = doc.project().clone();
    let starts: Vec<u64> = clip_of(&after, tid, cid)
        .notes()
        .iter()
        .map(|n| n.start().get())
        .collect();
    assert_eq!(starts, vec![20, 30, 40]);
    for _ in 0..3 {
        assert!(doc.undo().unwrap());
        assert_eq!(doc.project(), &before);
        assert!(doc.redo().unwrap());
        assert_eq!(doc.project(), &after);
    }
}

/// 빈 pairs와 같은 길이: 아무것도 바꾸지 않는 편집. 적용·undo가 상태를 바꾸지 않는다.
#[test]
fn review_r3_replace_notes_empty_is_noop() {
    let (mut doc, t, c) = doc_one_clip(3840, &[note(0, 100, 60)], None);
    let before = doc.project().clone();
    doc.apply(
        vec![Edit::ReplaceNotes {
            track: t,
            clip: c,
            length: 3840,
            pairs: vec![],
        }],
        None,
    )
    .unwrap();
    assert_eq!(doc.project(), &before);
    assert!(doc.undo().unwrap());
    assert_eq!(doc.project(), &before);
}

/// 모르는 트랙·클립, 다른 클립의 노트를 from으로.
#[test]
fn review_r3_replace_notes_rejects_unknown_targets() {
    let a = note(0, 100, 60);
    let (mut doc, t, c) = doc_one_clip(3840, std::slice::from_ref(&a), Some(7680));
    let other = doc
        .project()
        .track(t)
        .unwrap()
        .clips()
        .map(|(id, _)| id)
        .find(|&id| id != c)
        .unwrap();
    let before = doc.project().clone();
    let foreign_track = new_track().id();
    for (track, clip) in [(foreign_track, c), (t, other)] {
        let r = doc.apply(
            vec![Edit::ReplaceNotes {
                track,
                clip,
                length: 3840,
                pairs: vec![(a.clone(), note(10, 10, 60))],
            }],
            None,
        );
        assert!(r.is_err());
        assert_eq!(doc.project(), &before);
    }
}

/// `ExtendClip`, `ReplaceNote`, `ReplaceNotes`, `InsertNote`를 섞은 히스토리를 끝까지 undo하면 처음,
/// 끝까지 redo하면 마지막 상태.
#[test]
fn review_r3_mixed_history_round_trips() {
    let a = note(0, 100, 60);
    let b = note(1000, 100, 62);
    let (mut doc, t, c) = doc_one_clip(3840, &[a.clone(), b.clone()], None);
    let start = doc.project().clone();
    let steps: Vec<Vec<Edit>> = vec![
        vec![Edit::ExtendClip {
            track: t,
            id: c,
            length: 7680,
        }],
        vec![Edit::ReplaceNotes {
            track: t,
            clip: c,
            length: 11520,
            pairs: vec![(a.clone(), note(9000, 100, 60)), (b.clone(), a.clone())],
        }],
        vec![Edit::ReplaceNote {
            track: t,
            clip: c,
            from: a.clone(),
            to: note(5, 5, 61),
        }],
        vec![
            Edit::InsertNote {
                track: t,
                clip: c,
                note: note(11000, 500, 70),
            },
            Edit::ReplaceNotes {
                track: t,
                clip: c,
                length: 15360,
                pairs: vec![(note(11000, 500, 70), note(15000, 300, 70))],
            },
        ],
    ];
    let mut states = vec![start.clone()];
    for s in steps {
        doc.apply(s, None).unwrap();
        doc.seal();
        states.push(doc.project().clone());
    }
    for _ in 0..2 {
        for i in (0..states.len() - 1).rev() {
            assert!(doc.undo().unwrap());
            assert_eq!(doc.project(), &states[i], "undo to {i}");
        }
        for (i, s) in states.iter().enumerate().skip(1) {
            assert!(doc.redo().unwrap());
            assert_eq!(doc.project(), s, "redo to {i}");
        }
    }
}

// ---------------------------------------------------------------------------
// 일괄 연산
// ---------------------------------------------------------------------------

/// 리뷰 1 테스트(`review_stale_transform_after_extend_is_atomic`)가 원래 보던 「편집 사이의」 원자성:
/// 두 클립에 걸친 묶음에서 첫 `ReplaceNotes`(늘리기 포함)는 성공하고 둘째가 낡아 실패한다.
#[test]
fn review_r3_stale_batch_across_two_clips_is_atomic() {
    let mut p = Project::new();
    let mut tr = new_track();
    let tid = tr.id();
    let mut c1 = Clip::new(Tick::ZERO, 3840).unwrap();
    c1.insert_note(note(3000, 500, 60)).unwrap();
    let mut c2 = Clip::new(Tick::new(20_000), 3840).unwrap();
    c2.insert_note(note(10, 10, 62)).unwrap();
    let a = tr.insert_clip(c1).unwrap();
    let b = tr.insert_clip(c2).unwrap();
    p.push_track(tr).unwrap();
    let mut doc = Document::new(p, usize::MAX).unwrap();
    let sel = vec![
        NoteRef {
            track: tid,
            clip: a,
            note: note(3000, 500, 60),
        },
        NoteRef {
            track: tid,
            clip: b,
            note: note(10, 10, 62),
        },
    ];
    let edits = transform::shift(doc.project(), &sel, 500).unwrap();
    assert_eq!(edits.len(), 2);
    doc.apply(
        vec![Edit::RemoveNote {
            track: tid,
            clip: b,
            note: note(10, 10, 62),
        }],
        None,
    )
    .unwrap();
    let before = doc.project().clone();
    let undo_len = doc.undo_len();
    assert_eq!(doc.apply(edits, None), Err(ModelError::NoteNotFound));
    assert_eq!(doc.project(), &before);
    assert_eq!(clip_of(doc.project(), tid, a).length(), 3840);
    assert_eq!(doc.undo_len(), undo_len);
}

// ---------------------------------------------------------------------------
// 속성: 일괄 연산이 Ok로 낸 묶음은 언제나 적용되고, 꼬리 수는 늘지 않으며, undo/redo가 왕복한다.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Op {
    Quantize(Grid, bool, u8, u8),
    Transpose(i32),
    Shift(i64),
    Scale(u32, u32),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let grid = prop_oneof![
        Just(Grid::Quarter),
        Just(Grid::Eighth),
        Just(Grid::Sixteenth),
        Just(Grid::ThirtySecond),
        Just(Grid::SixtyFourth),
    ];
    prop_oneof![
        (grid, any::<bool>(), 0u8..=100, 0u8..=75)
            .prop_map(|(g, t, s, w)| Op::Quantize(g, t, s, w)),
        (-300i32..300).prop_map(Op::Transpose),
        prop_oneof![-9000i64..9000, Just(0i64), -2000i64..2000, 0i64..20000].prop_map(Op::Shift),
        (1u32..5, 1u32..5).prop_map(|(n, d)| Op::Scale(n, d)),
    ]
}

/// 트랙 하나의 구성: 첫 클립 시작, 분할 전 길이, 분할점(상대), 오른쪽을 남길지, 그 뒤 이웃 클립 간격,
/// 노트(시작, 길이, 음높이).
type TrackSpec = (u64, u64, u64, bool, Option<u64>, Vec<(u64, u32, u8)>);

fn track_strategy() -> impl Strategy<Value = TrackSpec> {
    (
        0u64..8000,
        1000u64..12000,
        0u64..=100,
        prop::bool::weighted(0.3),
        prop::option::weighted(0.5, prop_oneof![0u64..100, 0u64..30000]),
        prop::collection::vec((0u64..12000, 1u32..6000, 0u8..128), 0..8),
    )
}

fn build(meter_at: Option<u64>, specs: &[TrackSpec]) -> Option<(Project, Vec<NoteRef>)> {
    let mut p = Project::new();
    if let Some(at) = meter_at {
        // 마디 중간 박자표 변화(5/8)를 포함할 수 있다.
        p.meter_map_mut()
            .set(Tick::new(at), TimeSignature::new(5, 8).unwrap());
    }
    let mut all = Vec::new();
    for (start, len, split_pct, keep_right, gap, notes) in specs {
        let mut t = new_track();
        let tid = t.id();
        let mut clip = Clip::new(Tick::new(*start), *len).ok()?;
        for &(s, l, pitch) in notes {
            let n = note(s % len, l, pitch);
            if n.end() <= *len {
                clip.insert_note(n).ok()?;
            }
        }
        let left = t.insert_clip(clip).ok()?;
        let at = len * split_pct / 100;
        let mut ids = vec![left];
        let mut end = start + len;
        if at > 0 && at < *len {
            let right = t.split_clip(left, Tick::new(start + at)).ok()?;
            if *keep_right {
                ids.push(right);
            } else {
                t.remove_clip(right);
                end = start + at;
            }
        }
        if let Some(g) = gap {
            let mut n = Clip::new(Tick::new(end + g), 3840).ok()?;
            n.insert_note(note(0, 100, 60)).ok()?;
            ids.push(t.insert_clip(n).ok()?);
        }
        for id in ids {
            for n in t.clip(id)?.notes() {
                all.push(NoteRef {
                    track: tid,
                    clip: id,
                    note: n.clone(),
                });
            }
        }
        p.push_track(t).ok()?;
    }
    Some((p, all))
}

fn tail_counts(p: &Project) -> Vec<usize> {
    p.tracks()
        .iter()
        .flat_map(|t| t.clips().map(|(_, c)| tails(c)).collect::<Vec<_>>())
        .collect()
}

fn check(
    meter_at: Option<u64>,
    specs: &[TrackSpec],
    mask: &[bool],
    op: &Op,
) -> Result<(), TestCaseError> {
    let Some((project, all)) = build(meter_at, specs) else {
        return Ok(());
    };
    let sel: Vec<NoteRef> = all
        .into_iter()
        .zip(mask.iter().cycle())
        .filter(|(_, m)| **m)
        .map(|(sel_ref, _)| sel_ref)
        .collect();
    let mut doc = Document::new(project, usize::MAX).unwrap();
    let before = doc.project().clone();
    let res = match *op {
        Op::Quantize(grid, triplet, strength, swing) => {
            let settings = QuantizeSettings::new(grid, triplet, strength, swing).unwrap();
            transform::quantize(doc.project(), &sel, &settings)
        }
        Op::Transpose(n) => transform::transpose(doc.project(), &sel, n, PitchRange::full()),
        Op::Shift(n) => transform::shift(doc.project(), &sel, n),
        Op::Scale(num, den) => transform::scale(doc.project(), &sel, num, den),
    };
    let Ok(edits) = res else {
        return Ok(());
    };
    // 빈 선택은 빈 묶음이고 히스토리에 남지 않는다.
    let empty = edits.is_empty();
    let applied = doc.apply(edits, None);
    prop_assert!(applied.is_ok(), "Ok 묶음이 적용에서 실패: {applied:?}");
    let after = doc.project().clone();
    // 꼬리 수는 늘지 않는다(클립 순서는 같다: 연산은 클립을 만들거나 지우지 않는다).
    for (old, new) in tail_counts(&before).iter().zip(tail_counts(&after)) {
        prop_assert!(new <= *old, "꼬리 수가 늘었다: {old} → {new}");
    }
    for (bt, at) in before.tracks().iter().zip(after.tracks().iter()) {
        let mut spans: Vec<(u64, u64)> = Vec::new();
        for ((_, bc), (_, ac)) in bt.clips().zip(at.clips()) {
            // 노트 시작은 클립 안.
            for n in ac.notes() {
                prop_assert!(n.start().get() < ac.length());
            }
            // 늘었다면 끝이 마디 경계(마디 중간 박자표 변화 포함).
            if ac.length() != bc.length() {
                prop_assert!(ac.length() > bc.length());
                let pos = after.meter_map().position(Tick::new(ac.end()));
                prop_assert!(
                    pos.beat() == 0 && pos.tick() == 0,
                    "늘린 끝 {}이 마디 경계가 아니다",
                    ac.end()
                );
            }
            spans.push((ac.start().get(), ac.end()));
        }
        // 같은 트랙 클립은 겹치지 않는다.
        spans.sort_unstable();
        for w in spans.windows(2) {
            prop_assert!(w[0].1 <= w[1].0);
        }
    }
    prop_assert_eq!(doc.undo().unwrap(), !empty);
    prop_assert_eq!(doc.project(), &before);
    prop_assert_eq!(doc.redo().unwrap(), !empty);
    prop_assert_eq!(doc.project(), &after);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn review_r3_transform_ok_batches_always_apply(
        meter_at in prop::option::of(1u64..20000),
        specs in prop::collection::vec(track_strategy(), 1..3),
        mask in prop::collection::vec(any::<bool>(), 40),
        op in op_strategy(),
    ) {
        check(meter_at, &specs, &mask, &op)?;
    }

    /// 클립이 늘어나는 경우에 집중: 오른쪽으로 미는 이동과 배속.
    #[test]
    fn review_r3_growing_transforms_always_apply(
        meter_at in prop::option::of(1u64..20000),
        specs in prop::collection::vec(track_strategy(), 1..3),
        mask in prop::collection::vec(any::<bool>(), 40),
        op in prop_oneof![
            (1i64..20000).prop_map(Op::Shift),
            (2u32..6, 1u32..3).prop_map(|(n, d)| Op::Scale(n, d)),
        ],
    ) {
        check(meter_at, &specs, &mask, &op)?;
    }
}
