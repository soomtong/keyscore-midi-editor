//! S3-T3a: 붙여넣기·복제가 쓰는 `transform::insert_notes`(클립에 넣기, 마디 단위 연장)와 `clip_for_notes`(틈에
//! 새 클립). 연장 규칙은 일괄 연산과 같다(REQ 5.10).

use keyscore_core::transform::{clip_for_notes, insert_notes};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Note, Pitch, Project, Tick, TimeSignature,
    Track, TrackColor, TrackId, Velocity,
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

/// 트랙 하나에 클립들(시작, 길이). 박자표는 0에서 4/4, `meter`가 있으면 그 틱에서 3/4.
fn doc(clips: &[(u64, u64)], meter: Option<u64>) -> (Document, TrackId, Vec<ClipId>) {
    let mut p = Project::new();
    if let Some(at) = meter {
        p.meter_map_mut()
            .set(Tick::new(at), TimeSignature::new(3, 4).unwrap());
    }
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let tid = t.id();
    let ids = clips
        .iter()
        .map(|&(s, l)| t.insert_clip(Clip::new(Tick::new(s), l).unwrap()).unwrap())
        .collect();
    p.push_track(t).unwrap();
    (Document::new(p, usize::MAX).unwrap(), tid, ids)
}

fn clip_of(d: &Document, t: TrackId, c: ClipId) -> Clip {
    d.project().track(t).unwrap().clip(c).unwrap().clone()
}

#[test]
fn insert_notes_extends_clip_by_bars_and_rejects_overlap() {
    let (mut d, t, ids) = doc(&[(0, BAR), (4 * BAR, BAR)], None);
    let c = ids[0];

    // 끝 안: InsertNote만
    let edits = insert_notes(d.project(), t, c, &[note(0, 960, 60), note(960, 960, 62)]).unwrap();
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|e| matches!(e, Edit::InsertNote { .. })));

    // 끝 넘김: 마디 단위로 먼저 늘린다(끝 4800 → 2마디)
    let edits = insert_notes(d.project(), t, c, &[note(3840, 960, 64)]).unwrap();
    assert!(matches!(
        edits[0],
        Edit::ExtendClip { length, .. } if length == 2 * BAR
    ));
    d.apply(edits, None).unwrap();
    assert_eq!(clip_of(&d, t, c).length(), 2 * BAR);
    assert_eq!(clip_of(&d, t, c).notes().len(), 1);

    // 마디 끝에 딱 맞으면 그 길이
    let edits = insert_notes(d.project(), t, c, &[note(2 * BAR, 960, 64)]).unwrap();
    assert!(matches!(
        edits[0],
        Edit::ExtendClip { length, .. } if length == 3 * BAR
    ));

    // 다음 클립(4마디)과 겹치면 거부
    assert!(matches!(
        insert_notes(d.project(), t, c, &[note(4 * BAR, 10, 64)]),
        Err(ModelError::ClipOverlap { .. })
    ));
    // 빈 목록은 빈 묶음
    assert!(insert_notes(d.project(), t, c, &[]).unwrap().is_empty());
}

#[test]
fn insert_notes_extends_to_bar_after_meter_change() {
    // 1마디 뒤 3/4: 두 번째 마디부터 2880틱
    let (d, t, ids) = doc(&[(0, BAR)], Some(BAR));
    let edits = insert_notes(d.project(), t, ids[0], &[note(BAR, 10, 60)]).unwrap();
    assert!(matches!(
        edits[0],
        Edit::ExtendClip { length, .. } if length == BAR + 2880
    ));
}

#[test]
fn insert_notes_rejects_unknown_track_and_clip() {
    let (d, t, ids) = doc(&[(0, BAR)], None);
    let other = Track::new("X", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    assert_eq!(
        insert_notes(d.project(), other.id(), ids[0], &[note(0, 1, 60)]),
        Err(ModelError::UnknownTrack)
    );
    // 다른 문서의 클립 id는 이 트랙에 없다(id는 프로세스 안에서 새로 발급된다)
    let (_, _, ids2) = doc(&[(0, BAR)], None);
    assert_eq!(
        insert_notes(d.project(), t, ids2[0], &[note(0, 1, 60)]),
        Err(ModelError::UnknownClip)
    );
}

#[test]
fn clip_for_notes_spans_bars_and_rejects_overlap() {
    let (mut d, t, _) = doc(&[(0, BAR), (6 * BAR, BAR)], None);
    // 2마디 중간(8000)부터 노트 둘(절대 틱), 마지막 끝 9000 → 3마디 끝(11520)
    let notes = [note(8000, 500, 60), note(8500, 500, 62)];
    let edit = clip_for_notes(d.project(), t, &notes, Tick::new(8000)).unwrap();
    let Edit::InsertClip { clip, .. } = &edit else {
        panic!("InsertClip이어야 한다: {edit:?}");
    };
    assert_eq!(clip.start().get(), 2 * BAR);
    assert_eq!(clip.length(), BAR);
    assert_eq!(clip.notes()[0].start().get(), 8000 - 2 * BAR);
    d.apply(vec![edit], None).unwrap();
    assert_eq!(d.project().track(t).unwrap().clips().count(), 3);

    // 다음 클립(6마디)과 겹치면 거부
    assert!(matches!(
        clip_for_notes(
            d.project(),
            t,
            &[note(5 * BAR, 3841, 60)],
            Tick::new(5 * BAR)
        ),
        Err(ModelError::ClipOverlap { .. })
    ));
    // 앞 클립과 겹쳐도 거부(at이 앞 클립 안)
    assert!(matches!(
        clip_for_notes(d.project(), t, &[note(100, 10, 60)], Tick::new(100)),
        Err(ModelError::ClipOverlap { .. })
    ));
    // 노트가 at이 든 마디 시작 앞이면 거부
    assert_eq!(
        clip_for_notes(d.project(), t, &[note(4 * BAR, 10, 60)], Tick::new(5 * BAR)),
        Err(ModelError::NoteBeforeClipStart)
    );
    assert_eq!(
        clip_for_notes(d.project(), t, &[], Tick::new(5 * BAR)),
        Err(ModelError::NoteNotFound)
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// `insert_notes`가 `Ok`면 적용이 성공하고 모든 노트가 클립 안이며, 클립은 겹치지 않는다.
    #[test]
    fn insert_notes_preserves_invariants(
        length in 1u64..20_000,
        next in prop::option::of(1u64..40_000),
        notes in prop::collection::vec((0u64..30_000, 1u32..5000, 0u8..128), 0..10),
    ) {
        let mut clips = vec![(0, length)];
        if let Some(gap) = next {
            clips.push((length + gap, 1000));
        }
        let (mut d, t, ids) = doc(&clips, None);
        let notes: Vec<Note> = notes.iter().map(|&(s, l, p)| note(s, l, p)).collect();
        if let Ok(edits) = insert_notes(d.project(), t, ids[0], &notes) {
            d.apply(edits, None).unwrap();
            let clip = clip_of(&d, t, ids[0]);
            prop_assert_eq!(clip.notes().len(), notes.len());
            prop_assert!(clip.notes().iter().all(|n| n.end() <= clip.length()));
            prop_assert!(clip.length().is_multiple_of(BAR) || clip.length() == length);
        } else {
            // 거부는 다음 클립과 겹칠 때뿐이다
            let end = notes.iter().map(Note::end).max().unwrap_or(0);
            let bar_end = end.div_ceil(BAR) * BAR;
            prop_assert!(next.is_some_and(|gap| bar_end > length + gap));
        }
    }
}

#[test]
fn clip_for_notes_starts_after_prev_clip_and_stops_before_next_in_same_bar() {
    // 앞 클립 [0, 1마디 + 2박), 다음 클립 [2마디 + 3박, …): 1마디 + 3박에 노트 하나(1박)
    let (d, t, _) = doc(&[(0, BAR + 1920), (2 * BAR + 2880, BAR)], None);
    let edit = clip_for_notes(
        d.project(),
        t,
        &[note(BAR + 2880, 960, 60)],
        Tick::new(BAR + 2880),
    )
    .unwrap();
    let Edit::InsertClip { clip, .. } = &edit else {
        panic!("{edit:?}");
    };
    // 시작은 앞 클립 끝, 끝은 마디 끝(2마디)
    assert_eq!(
        (clip.start().get(), clip.length()),
        (BAR + 1920, BAR - 1920)
    );
    assert_eq!(clip.notes()[0].start().get(), 960);
    // 2마디 + 1박의 노트: 끝은 마디 끝(3마디)이 아니라 다음 클립 시작
    let edit = clip_for_notes(
        d.project(),
        t,
        &[note(2 * BAR + 960, 960, 60)],
        Tick::new(2 * BAR + 960),
    )
    .unwrap();
    let Edit::InsertClip { clip, .. } = &edit else {
        panic!("{edit:?}");
    };
    assert_eq!((clip.start().get(), clip.end()), (2 * BAR, 2 * BAR + 2880));
    // 노트가 다음 클립에 닿으면 거부
    assert!(matches!(
        clip_for_notes(
            d.project(),
            t,
            &[note(2 * BAR + 960, 2000, 60)],
            Tick::new(2 * BAR + 960)
        ),
        Err(ModelError::ClipOverlap { .. })
    ));
}
