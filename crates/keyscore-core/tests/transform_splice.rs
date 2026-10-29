//! S3-T4a: Step 삽입이 쓰는 `transform::splice`(커서 뒤 노트 밀기·당기기 + 지우기·바꾸기·넣기, 마디 단위 연장).
//! 규칙은 `docs/decisions/2026-10-29-s3-t4-step-input-details.md` 1절.

use keyscore_core::transform::{Splice, splice};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Note, Pitch, Project, Tick, Track,
    TrackColor, TrackId, Velocity,
};
use proptest::prelude::*;

const BAR: u64 = 3840;

fn note(start: u64, length: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 트랙 하나에 클립들(시작, 길이, 노트). 박자표는 4/4.
fn doc(clips: &[(u64, u64, Vec<Note>)]) -> (Document, TrackId, Vec<ClipId>) {
    let mut p = Project::new();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let tid = t.id();
    let ids = clips
        .iter()
        .map(|(s, l, notes)| {
            let mut c = Clip::new(Tick::new(*s), *l).unwrap();
            for n in notes {
                c.insert_note(n.clone()).unwrap();
            }
            t.insert_clip(c).unwrap()
        })
        .collect();
    p.push_track(t).unwrap();
    (Document::new(p, usize::MAX).unwrap(), tid, ids)
}

fn triples(d: &Document, t: TrackId, c: ClipId) -> Vec<(u64, u32, u8)> {
    d.project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .notes()
        .iter()
        .map(|n| (n.start().get(), n.length(), n.pitch().get()))
        .collect()
}

fn length(d: &Document, t: TrackId, c: ClipId) -> u64 {
    d.project().track(t).unwrap().clip(c).unwrap().length()
}

fn insert_at(at: u64, by: i64, insert: Vec<Note>) -> Splice {
    Splice {
        at,
        by,
        insert,
        ..Splice::default()
    }
}

#[test]
fn splice_insert_pushes_notes_at_or_after_at() {
    let notes = vec![note(0, 960, 60), note(960, 960, 62), note(1920, 960, 64)];
    let (mut d, t, ids) = doc(&[(0, BAR, notes)]);
    let edits = splice(
        d.project(),
        t,
        ids[0],
        &insert_at(960, 480, vec![note(960, 480, 67)]),
    )
    .unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(
        triples(&d, t, ids[0]),
        vec![
            (0, 960, 60),
            (960, 480, 67),
            (1440, 960, 62),
            (2400, 960, 64)
        ]
    );
    assert_eq!(length(&d, t, ids[0]), BAR);
}

#[test]
fn splice_does_not_move_note_spanning_at() {
    let (mut d, t, ids) = doc(&[(0, BAR, vec![note(0, 1920, 60)])]);
    let edits = splice(
        d.project(),
        t,
        ids[0],
        &insert_at(960, 960, vec![note(960, 960, 62)]),
    )
    .unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(triples(&d, t, ids[0]), vec![(0, 1920, 60), (960, 960, 62)]);
}

#[test]
fn splice_extends_clip_by_bars_when_pushed_past_end() {
    let (mut d, t, ids) = doc(&[(0, BAR, vec![note(2880, 960, 60)])]);
    let edits = splice(
        d.project(),
        t,
        ids[0],
        &insert_at(2880, 960, vec![note(2880, 960, 62)]),
    )
    .unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(length(&d, t, ids[0]), 2 * BAR);
    assert_eq!(
        triples(&d, t, ids[0]),
        vec![(2880, 960, 62), (3840, 960, 60)]
    );
}

#[test]
fn splice_extends_for_inserted_note_at_clip_end() {
    let (mut d, t, ids) = doc(&[(0, BAR, vec![])]);
    let edits = splice(
        d.project(),
        t,
        ids[0],
        &insert_at(BAR, 960, vec![note(BAR, 960, 60)]),
    )
    .unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(length(&d, t, ids[0]), 2 * BAR);
    assert_eq!(triples(&d, t, ids[0]), vec![(BAR, 960, 60)]);
}

#[test]
fn splice_stops_extension_at_next_clip_and_rejects_touching_it() {
    // 다음 클립이 1.5마디에서 시작: 노트가 그 앞에서 끝나면 그 시작까지만 늘린다.
    let (d, t, ids) = doc(&[(0, BAR, vec![]), (BAR + 1920, BAR, vec![])]);
    let edits = splice(
        d.project(),
        t,
        ids[0],
        &insert_at(BAR, 960, vec![note(BAR, 960, 60)]),
    )
    .unwrap();
    assert!(edits.iter().any(|e| matches!(
        e,
        Edit::ExtendClip { length, .. } | Edit::ReplaceNotes { length, .. } if *length == BAR + 1920
    )));
    // 노트가 다음 클립에 닿으면 거부
    let err = splice(
        d.project(),
        t,
        ids[0],
        &insert_at(BAR, 1921, vec![note(BAR, 1921, 60)]),
    )
    .unwrap_err();
    assert!(matches!(err, ModelError::ClipOverlap { .. }));
}

#[test]
fn splice_pull_removes_step_and_pulls_later_notes() {
    let notes = vec![note(0, 960, 60), note(960, 960, 62), note(1920, 960, 64)];
    let (mut d, t, ids) = doc(&[(0, BAR, notes)]);
    let s = Splice {
        at: 1920,
        by: -960,
        remove: vec![note(960, 960, 62)],
        ..Splice::default()
    };
    let edits = splice(d.project(), t, ids[0], &s).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(triples(&d, t, ids[0]), vec![(0, 960, 60), (960, 960, 64)]);
    assert_eq!(length(&d, t, ids[0]), BAR, "당겨도 클립은 줄지 않는다");
}

#[test]
fn splice_pull_before_clip_start_is_rejected() {
    let (d, t, ids) = doc(&[(0, BAR, vec![note(480, 960, 60)])]);
    let err = splice(
        d.project(),
        t,
        ids[0],
        &Splice {
            at: 480,
            by: -960,
            ..Splice::default()
        },
    )
    .unwrap_err();
    assert_eq!(err, ModelError::NoteBeforeClipStart);
}

#[test]
fn splice_replace_extends_and_pushes() {
    // 타이: 직전 노트를 늘리고 뒤 노트를 민다
    let notes = vec![note(0, 960, 60), note(960, 960, 62)];
    let (mut d, t, ids) = doc(&[(0, BAR, notes)]);
    let s = Splice {
        at: 960,
        by: 960,
        replace: vec![(note(0, 960, 60), note(0, 1920, 60))],
        ..Splice::default()
    };
    let edits = splice(d.project(), t, ids[0], &s).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(triples(&d, t, ids[0]), vec![(0, 1920, 60), (1920, 960, 62)]);
}

#[test]
fn splice_rejects_missing_remove_or_replace() {
    let (d, t, ids) = doc(&[(0, BAR, vec![note(0, 960, 60)])]);
    let s = Splice {
        remove: vec![note(0, 960, 61)],
        ..Splice::default()
    };
    assert_eq!(
        splice(d.project(), t, ids[0], &s).unwrap_err(),
        ModelError::NoteNotFound
    );
    let s = Splice {
        replace: vec![(note(0, 480, 60), note(0, 960, 60))],
        ..Splice::default()
    };
    assert_eq!(
        splice(d.project(), t, ids[0], &s).unwrap_err(),
        ModelError::NoteNotFound
    );
    // 다른 문서의 클립 id는 이 트랙에 없다
    let (_, _, other) = doc(&[(0, BAR, vec![])]);
    assert_eq!(
        splice(d.project(), t, other[0], &Splice::default()).unwrap_err(),
        ModelError::UnknownClip
    );
}

#[test]
fn splice_keeps_hidden_tail_as_tail() {
    // 분할로 생긴 꼬리(끝 > 클립 길이)는 시작만 클립 안이면 된다
    let (mut d, t, ids) = doc(&[(0, 2 * BAR, vec![note(BAR + 960, 1920, 60)])]);
    d.apply(
        vec![Edit::SplitClip {
            track: t,
            id: ids[0],
            at: Tick::new(BAR + 1920),
        }],
        None,
    )
    .unwrap();
    let left = ids[0];
    assert_eq!(length(&d, t, left), BAR + 1920);
    let edits = splice(d.project(), t, left, &insert_at(BAR, 480, vec![])).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(triples(&d, t, left), vec![(BAR + 1440, 1920, 60)]);
    assert_eq!(
        length(&d, t, left),
        BAR + 1920,
        "꼬리 끝 때문에 늘리지 않는다"
    );
}

proptest! {
    #[test]
    fn splice_applies_and_keeps_invariants(
        starts in prop::collection::vec((0u64..BAR * 2, 1u32..2000, 40u8..90), 0..12),
        at in 0u64..BAR * 2,
        by in -1000i64..3000,
        insert_len in 0u32..2000,
    ) {
        let notes: Vec<Note> = starts.iter().map(|&(s, l, p)| note(s, l, p)).collect();
        let (mut d, t, ids) = doc(&[(0, BAR * 3, notes.clone())]);
        let mut sorted = notes;
        sorted.sort_by_key(|n| (n.start().get(), n.length(), n.pitch().get()));
        let insert = if insert_len > 0 { vec![note(at, insert_len, 72)] } else { vec![] };
        let s = Splice { at, by, insert: insert.clone(), ..Splice::default() };
        match splice(d.project(), t, ids[0], &s) {
            Ok(edits) => {
                d.apply(edits, None).unwrap();
                let after = triples(&d, t, ids[0]);
                let mut expected: Vec<(u64, u32, u8)> = sorted
                    .iter()
                    .map(|n| {
                        let st = n.start().get();
                        let st = if st >= at { st.checked_add_signed(by).unwrap() } else { st };
                        (st, n.length(), n.pitch().get())
                    })
                    .chain(insert.iter().map(|n| (n.start().get(), n.length(), n.pitch().get())))
                    .collect();
                expected.sort_unstable();
                let mut got = after;
                got.sort_unstable();
                prop_assert_eq!(got, expected);
                let clip = d.project().track(t).unwrap().clip(ids[0]).unwrap();
                prop_assert!(clip.notes().iter().all(|n| n.end() <= clip.length()));
            }
            Err(e) => {
                // 거부는 클립 시작 앞으로 당길 때뿐이다(다음 클립이 없다)
                prop_assert_eq!(e, ModelError::NoteBeforeClipStart);
                prop_assert!(by < 0);
            }
        }
    }
}

#[test]
fn splice_reach_extends_clip_without_notes() {
    // 클립 끝을 넘는 쉼표(S3-T4a 리뷰 1 지적 2): 노트 없이 마디 단위로 늘린다
    let (mut d, t, ids) = doc(&[(0, BAR, vec![]), (2 * BAR + 960, BAR, vec![])]);
    let s = Splice {
        at: 2880,
        by: 1920,
        reach: 2880 + 1920,
        ..Splice::default()
    };
    let edits = splice(d.project(), t, ids[0], &s).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(length(&d, t, ids[0]), 2 * BAR);
    // 다음 클립 시작까지만
    let s = Splice {
        reach: 2 * BAR + 480,
        ..Splice::default()
    };
    let edits = splice(d.project(), t, ids[0], &s).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(length(&d, t, ids[0]), 2 * BAR + 960);
    // 다음 클립에 닿으면 거부
    let s = Splice {
        reach: 2 * BAR + 961,
        ..Splice::default()
    };
    assert!(matches!(
        splice(d.project(), t, ids[0], &s),
        Err(ModelError::ClipOverlap { .. })
    ));
}
