//! S0-T7 적대적 리뷰 4회차의 깨뜨리기 테스트. 공개 API만 쓴다.
//!
//! 대상은 동시 치환으로 바뀐 `Edit::ReplaceNotes`(`Clip::replace_notes`)다.

use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Note, Pitch, Project, Tick, Track,
    TrackColor, TrackId, Velocity,
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

fn clip_of(doc: &Document, t: TrackId, c: ClipId) -> &Clip {
    doc.project().track(t).unwrap().clip(c).unwrap()
}

fn tails(clip: &Clip) -> usize {
    clip.notes()
        .iter()
        .filter(|n| n.end() > clip.length())
        .count()
}

/// 길이 7680 클립에 `plain`(끝 ≤ 3840)과 `tailing`(3840을 가로지름)을 넣고 3840에서 분할한 뒤 오른쪽을
/// 지운다. 왼쪽(0..3840)에는 `tailing`이 분할로 생긴 꼬리로 남고, 오른쪽이 비어 늘릴 수 있다.
fn doc_with_tails(plain: &[Note], tailing: &[Note]) -> (Document, TrackId, ClipId) {
    let mut p = Project::new();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let tid = t.id();
    let mut clip = Clip::new(Tick::ZERO, 7680).unwrap();
    for n in plain.iter().chain(tailing) {
        clip.insert_note(n.clone()).unwrap();
    }
    let c = t.insert_clip(clip).unwrap();
    let right = t.split_clip(c, Tick::new(3840)).unwrap();
    t.remove_clip(right).unwrap();
    p.push_track(t).unwrap();
    let doc = Document::new(p, usize::MAX).unwrap();
    assert_eq!(clip_of(&doc, tid, c).length(), 3840);
    assert_eq!(tails(clip_of(&doc, tid, c)), tailing.len());
    (doc, tid, c)
}

fn replace(
    doc: &mut Document,
    t: TrackId,
    c: ClipId,
    length: u64,
    pairs: Vec<(Note, Note)>,
) -> Result<(), ModelError> {
    doc.apply(
        vec![Edit::ReplaceNotes {
            track: t,
            clip: c,
            length,
            pairs,
        }],
        None,
    )
}

/// 맞바꾸기: 꼬리 T를 일반 노트 P의 값으로, P를 T의 값으로. 꼬리 허용은 from(P)에 붙으므로 P의
/// 자리에서 꼬리가 나올 수 없다. 길이를 늘려 P의 새 값이 넘는 경우도 거부. 상태·undo 불변.
#[test]
fn review_r4_swap_cannot_turn_plain_into_tail() {
    let p = note(0, 100, 60);
    let tl = note(3000, 2000, 62); // 끝 5000 > 3840
    let (mut doc, t, c) = doc_with_tails(std::slice::from_ref(&p), std::slice::from_ref(&tl));
    let before = doc.project().clone();

    // 정확한 맞바꾸기, 같은 길이.
    let r = replace(
        &mut doc,
        t,
        c,
        3840,
        vec![(tl.clone(), p.clone()), (p.clone(), tl.clone())],
    );
    assert!(
        matches!(r, Err(ModelError::NoteOutsideClip { .. })),
        "{r:?}"
    );
    assert_eq!(doc.project(), &before);
    assert_eq!(doc.undo_len(), 0);

    // 늘리면서 맞바꾸고 P를 새 길이 밖으로. pair 순서를 바꿔도 같다.
    let far = note(3000, 100_000, 62);
    for pairs in [
        vec![(tl.clone(), p.clone()), (p.clone(), far.clone())],
        vec![(p.clone(), far.clone()), (tl.clone(), p.clone())],
    ] {
        let r = replace(&mut doc, t, c, 7680, pairs);
        assert!(
            matches!(r, Err(ModelError::NoteOutsideClip { .. })),
            "{r:?}"
        );
        assert_eq!(doc.project(), &before);
        assert_eq!(doc.undo_len(), 0);
    }

    // 늘린 길이 안에 들어오는 맞바꾸기(T값의 끝 5000 ≤ 7680)는 허용, 꼬리 수는 1 → 0.
    replace(
        &mut doc,
        t,
        c,
        7680,
        vec![(tl.clone(), p.clone()), (p.clone(), tl.clone())],
    )
    .unwrap();
    assert_eq!(tails(clip_of(&doc, t, c)), 0);
    assert!(doc.undo().unwrap());
    assert_eq!(doc.project(), &before);
    assert_eq!(tails(clip_of(&doc, t, c)), 1);
}

/// 사슬 to → from: 중간 값이 원래 목록에 없으면 `NoteNotFound`(3회차 구멍), 있으면 원래 노트로
/// 읽혀 통과한다. 결과는 pair 순서와 무관하다. 꼬리 허용은 사슬을 따라 옮겨 가지 않는다.
#[test]
fn review_r4_chain_through_value_present_or_absent() {
    let a = note(0, 100, 60);
    let b = note(500, 100, 60);
    let tl = note(3000, 2000, 62);
    let (mut doc, t, c) = doc_with_tails(&[a.clone(), b.clone()], std::slice::from_ref(&tl));
    let before = doc.project().clone();

    // 중간 값 mid는 원래 없다.
    let mid = note(3800, 1000, 60);
    let res = replace(
        &mut doc,
        t,
        c,
        7680,
        vec![
            (a.clone(), mid.clone()),
            (mid.clone(), note(3800, 100_000, 60)),
        ],
    );
    assert_eq!(res, Err(ModelError::NoteNotFound));
    assert_eq!(doc.project(), &before);

    // 꼬리 T의 to를 일반 노트 a의 값으로, a를 넘는 노트로: a는 원래 일반 노트라 거부.
    let res = replace(
        &mut doc,
        t,
        c,
        7680,
        vec![(tl.clone(), a.clone()), (a.clone(), note(0, 100_000, 60))],
    );
    assert!(
        matches!(res, Err(ModelError::NoteOutsideClip { .. })),
        "{res:?}"
    );
    assert_eq!(doc.project(), &before);

    // 중간 값 b가 원래 있다: a → b, b → c. 두 순서 모두 결과 {b, c, T}.
    let cc = note(1000, 100, 60);
    let mut results = Vec::new();
    for pairs in [
        vec![(a.clone(), b.clone()), (b.clone(), cc.clone())],
        vec![(b.clone(), cc.clone()), (a.clone(), b.clone())],
    ] {
        replace(&mut doc, t, c, 3840, pairs).unwrap();
        results.push(clip_of(&doc, t, c).notes().to_vec());
        assert_eq!(tails(clip_of(&doc, t, c)), 1);
        assert!(doc.undo().unwrap());
        assert_eq!(doc.project(), &before);
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0], vec![b.clone(), cc, tl.clone()]);
}

/// 같은 값 from 여럿: 개수까지 허용, 초과 거부(상태 불변). from == to는 꼬리·일반 모두 무변화.
#[test]
fn review_r4_duplicate_from_and_identity() {
    let p = note(0, 100, 60);
    let tl = note(3000, 2000, 62);
    let (mut doc, t, c) = doc_with_tails(&[p.clone(), p.clone()], &[tl.clone(), tl.clone()]);
    let before = doc.project().clone();

    let r = replace(&mut doc, t, c, 3840, vec![(p.clone(), p.clone()); 3]);
    assert_eq!(r, Err(ModelError::NoteNotFound));
    assert_eq!(doc.project(), &before);
    assert_eq!(doc.undo_len(), 0);

    // 꼬리 둘을 하나는 그대로, 하나는 더 길게. 일반 노트 둘은 그대로.
    let longer = note(3000, 50_000, 62);
    replace(
        &mut doc,
        t,
        c,
        3840,
        vec![
            (tl.clone(), tl.clone()),
            (tl.clone(), longer.clone()),
            (p.clone(), p.clone()),
            (p.clone(), p.clone()),
        ],
    )
    .unwrap();
    assert_eq!(
        clip_of(&doc, t, c).notes(),
        &[p.clone(), p.clone(), tl.clone(), longer][..]
    );
    assert_eq!(tails(clip_of(&doc, t, c)), 2);
    // 꼬리 세 번째는 없다.
    let r = replace(&mut doc, t, c, 3840, vec![(tl.clone(), tl.clone()); 2]);
    assert_eq!(r, Err(ModelError::NoteNotFound));
    assert!(doc.undo().unwrap());
    assert_eq!(doc.project(), &before);
    assert!(doc.redo().unwrap());
    assert!(doc.undo().unwrap());
    assert_eq!(doc.project(), &before);
}

/// 둘째 루프(꼬리 끝 u64 넘침)에서 실패해도 첫 루프의 삭제가 반영되지 않는다.
#[test]
fn review_r4_late_failure_leaves_clip_untouched() {
    let mut p = Project::new();
    let mut tr = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let tid = tr.id();
    let start = u64::MAX - 7680;
    let mut clip = Clip::new(Tick::new(start), 7680).unwrap();
    let a = note(0, 100, 60);
    let tl = note(3000, 2000, 62);
    clip.insert_note(a.clone()).unwrap();
    clip.insert_note(tl.clone()).unwrap();
    let c = tr.insert_clip(clip).unwrap();
    let right = tr.split_clip(c, Tick::new(start + 3840)).unwrap();
    tr.remove_clip(right).unwrap();
    p.push_track(tr).unwrap();
    let mut doc = Document::new(p, usize::MAX).unwrap();
    let before = doc.project().clone();
    let r = replace(
        &mut doc,
        tid,
        c,
        3840,
        vec![(a, note(10, 100, 60)), (tl, note(3000, u32::MAX, 62))],
    );
    assert!(r.is_err(), "{r:?}");
    assert_eq!(doc.project(), &before);
    assert_eq!(doc.undo_len(), 0);
}

fn arb_note(max_start: u64) -> impl Strategy<Value = Note> {
    (0..max_start, prop_oneof![1u32..400, 1u32..6000], 60u8..63).prop_map(|(s, l, p)| note(s, l, p))
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 10000, ..ProptestConfig::default() })]

    /// 임의 pair(원래 노트 또는 임의 값을 from으로, 임의 to)와 임의 길이(줄이기 포함)에 대해:
    /// 실패면 상태·undo 불변, 성공이면 꼬리 수 비증가, pair 순서를 뒤집어도 같은 결과,
    /// undo가 정확히 원래로(꼬리 수 포함), redo가 같은 결과.
    #[test]
    fn review_r4_replace_notes_simultaneous_properties(
        plain in prop::collection::vec((0u64..3700, 1u32..140, 60u8..63), 0..5),
        tailing in prop::collection::vec((2000u64..3839, 2000u32..6000, 60u8..63), 0..4),
        picks in prop::collection::vec((any::<prop::sample::Index>(), prop::bool::weighted(0.9), arb_note(3700)), 0..6),
        tos in prop::collection::vec((prop_oneof![3 => arb_note(4200), 1 => arb_note(8000)], any::<prop::sample::Index>(), prop::bool::weighted(0.3)), 6),
        length in prop_oneof![Just(3840u64), 3840u64..9000, 3000u64..3840],
    ) {
        let plain: Vec<Note> = plain.into_iter().map(|(s, l, p)| note(s, l, p)).collect();
        let tailing: Vec<Note> = tailing
            .into_iter()
            .map(|(s, l, p)| note(s, l.min(u32::try_from(7680 - s).unwrap()), p))
            .collect();
        let (mut doc, t, c) = doc_with_tails(&plain, &tailing);
        let all: Vec<Note> = clip_of(&doc, t, c).notes().to_vec();
        // from: 원래 노트(대개), 임의 값, 또는 앞 pair의 to(사슬). to: 임의 값 또는 원래 노트의 값.
        let mut pairs: Vec<(Note, Note)> = Vec::new();
        for ((i, original, other), (to, j, to_original)) in picks.into_iter().zip(tos) {
            let from = match pairs.last() {
                Some((_, prev)) if i.index(4) == 0 => prev.clone(),
                _ if original && !all.is_empty() => i.get(&all).clone(),
                _ => other,
            };
            let to = if to_original && !all.is_empty() { j.get(&all).clone() } else { to };
            pairs.push((from, to));
        }
        let before = doc.project().clone();
        let tails_before = tails(clip_of(&doc, t, c));

        let r = replace(&mut doc, t, c, length, pairs.clone());
        if r.is_err() {
            prop_assert_eq!(doc.project(), &before);
            prop_assert_eq!(doc.undo_len(), 0);
            let mut rev = pairs.clone();
            rev.reverse();
            prop_assert!(replace(&mut doc, t, c, length, rev).is_err());
            return Ok(());
        }
        let after = doc.project().clone();
        let clip = clip_of(&doc, t, c);
        prop_assert!(tails(clip) <= tails_before, "꼬리 증가: {:?}", clip.notes());
        prop_assert!(clip.notes().iter().all(|n| n.start().get() < clip.length()));

        prop_assert!(doc.undo().unwrap());
        prop_assert_eq!(doc.project(), &before);
        prop_assert_eq!(tails(clip_of(&doc, t, c)), tails_before);

        let mut rev = pairs.clone();
        rev.reverse();
        replace(&mut doc, t, c, length, rev).unwrap();
        prop_assert_eq!(doc.project(), &after);
        prop_assert!(doc.undo().unwrap());
        prop_assert!(doc.redo().unwrap());
        prop_assert_eq!(doc.project(), &after);
    }
}
