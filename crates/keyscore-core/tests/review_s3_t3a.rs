//! S3-T3a 적대적 리뷰 1(core). `clip_for_notes`의 박자표 변화·속성, `insert_notes`의 마디에 맞지 않는 클립 시작.

use keyscore_core::transform::{clip_for_notes, insert_notes};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, Note, Pitch, Project, Tick, TimeSignature, Track,
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

/// 2마디부터 3/4(2880틱 마디). 2마디 + 1000틱에 붙이면 클립은 [7680, 10560).
#[test]
fn clip_for_notes_after_meter_change_uses_that_bar() {
    let (d, t, _) = doc(&[], Some(2 * BAR));
    let edit = clip_for_notes(
        d.project(),
        t,
        &[note(2 * BAR + 1000, 960, 60)],
        Tick::new(2 * BAR + 1000),
    )
    .unwrap();
    let Edit::InsertClip { clip, .. } = &edit else {
        panic!("{edit:?}");
    };
    assert_eq!((clip.start().get(), clip.length()), (2 * BAR, 2880));
    assert_eq!(clip.notes()[0].start().get(), 1000);
}

/// 마디에 맞지 않는 시작(1박)의 클립을 늘리면 끝은 절대 마디 경계다(S0-T7 규칙).
#[test]
fn insert_notes_into_unaligned_clip_extends_to_absolute_bar_end() {
    let (d, t, ids) = doc(&[(960, BAR)], None);
    let edits = insert_notes(d.project(), t, ids[0], &[note(BAR, 100, 60)]).unwrap();
    assert!(matches!(
        edits[0],
        Edit::ExtendClip { length, .. } if length == 2 * BAR - 960
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// `clip_for_notes`가 `Ok`면 적용이 성공하고, 새 클립은 다른 클립과 겹치지 않으며 노트가 모두 그 안이다.
    #[test]
    fn clip_for_notes_ok_applies_without_overlap(
        clips in prop::collection::vec((0u64..40, 1u64..3 * BAR), 0..4),
        meter in prop::option::of(1u64..8),
        at in 0u64..12 * BAR,
        notes in prop::collection::vec((0u64..2 * BAR, 1u32..3 * 960, 0u8..128), 1..6),
    ) {
        // 겹치지 않게 1/4박 단위 시작으로 앞에서부터 쌓는다
        let mut placed = Vec::new();
        let mut cursor = 0;
        for (gap, len) in clips {
            let s = cursor + gap * 240;
            placed.push((s, len));
            cursor = s + len;
        }
        let (mut d, t, _) = doc(&placed, meter.map(|m| m * BAR));
        let notes: Vec<Note> = notes.iter().map(|&(s, l, p)| note(at + s, l, p)).collect();
        if let Ok(edit) = clip_for_notes(d.project(), t, &notes, Tick::new(at)) {
            d.apply(vec![edit], None).unwrap();
            let track = d.project().track(t).unwrap();
            let all: Vec<&Clip> = track.clips().map(|(_, c)| c).collect();
            for (i, a) in all.iter().enumerate() {
                for b in &all[i + 1..] {
                    prop_assert!(a.end() <= b.start().get() || b.end() <= a.start().get());
                }
                prop_assert!(a.notes().iter().all(|n| n.end() <= a.length()));
            }
            prop_assert_eq!(track.clips().count(), placed.len() + 1);
        }
    }
}
