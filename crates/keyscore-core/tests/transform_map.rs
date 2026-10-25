//! S3-T2: `transform::map_notes`(일괄 연산의 `plan`을 공개한 것)가 규칙을 바꾸지 않는다. 같은 이동을
//! `map_notes`와 `shift`로 만들면 같은 편집이 나오거나 둘 다 같은 이유로 거부된다.

use keyscore_core::transform::{self, NoteRef};
use keyscore_core::{
    Channel, Clip, ClipId, Document, ModelError, Note, Pitch, Project, Tick, TimeSignature, Track,
    TrackColor, TrackId, Velocity,
};
use proptest::prelude::*;

fn doc(clip_start: u64, notes: &[(u64, u32, u8)], meter: (u8, u16)) -> (Document, TrackId, ClipId) {
    let mut p = Project::new();
    p.meter_map_mut()
        .set(Tick::ZERO, TimeSignature::new(meter.0, meter.1).unwrap());
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let tid = t.id();
    let mut clip = Clip::new(Tick::new(clip_start), 40_000).unwrap();
    for &(s, l, p) in notes {
        let _ = clip.insert_note(
            Note::new(Tick::new(s), l, Pitch::new(p).unwrap(), Velocity::new(1)).unwrap(),
        );
    }
    let id = t.insert_clip(clip).unwrap();
    p.push_track(t).unwrap();
    (Document::new(p, usize::MAX).unwrap(), tid, id)
}

fn all(d: &Document, t: TrackId, c: ClipId) -> Vec<NoteRef> {
    d.project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .notes()
        .iter()
        .map(|n| NoteRef {
            track: t,
            clip: c,
            note: n.clone(),
        })
        .collect()
}

fn notes_strategy() -> impl Strategy<Value = Vec<(u64, u32, u8)>> {
    prop::collection::vec((0u64..30_000, 1u32..5000, 0u8..128), 0..12)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// `map_notes`로 만든 시간 이동 = `shift`. 클립 시작 앞 거부, 마디 단위 연장, 꼬리 규칙이 모두 같다.
    #[test]
    fn map_notes_matches_shift(
        notes in notes_strategy(),
        clip_start in 0u64..20_000,
        delta in -40_000i64..40_000,
        meter in (1u8..8, prop::sample::select(vec![4u16, 8])),
    ) {
        let (d, t, c) = doc(clip_start, &notes, meter);
        let sel = all(&d, t, c);
        let expected = transform::shift(d.project(), &sel, delta);
        let actual = transform::map_notes(d.project(), &sel, |_, clip, note| {
            let abs = i128::from(clip.start().get()) + i128::from(note.start().get()) + i128::from(delta);
            let rel = abs - i128::from(clip.start().get());
            if rel < 0 {
                return Err(ModelError::NoteBeforeClipStart);
            }
            let rel = u64::try_from(rel).map_err(|_| ModelError::TickOverflow)?;
            note.with_start(Tick::new(rel))
        });
        prop_assert_eq!(actual, expected);
    }

    /// `map_notes`로 만든 조옮김(음역 안) = `transpose`. 음역 밖은 `transpose`가 접고 `map_notes` 쪽은 거부하므로
    /// 접히지 않는 입력만 비교한다.
    #[test]
    fn map_notes_matches_transpose_inside_range(
        notes in notes_strategy(),
        semitones in -24i32..24,
    ) {
        let (d, t, c) = doc(0, &notes, (4, 4));
        let sel: Vec<NoteRef> = all(&d, t, c)
            .into_iter()
            .filter(|r| (0..=127).contains(&(i32::from(r.note.pitch().get()) + semitones)))
            .collect();
        let expected = transform::transpose(d.project(), &sel, semitones, transform::PitchRange::full());
        let actual = transform::map_notes(d.project(), &sel, |_, _, note| {
            let p = u8::try_from(i32::from(note.pitch().get()) + semitones)
                .map_err(|_| ModelError::PitchOutOfRange(u8::MAX))?;
            note.with_pitch(Pitch::new(p)?)
        });
        prop_assert_eq!(actual, expected);
    }
}

/// 같은 노트를 두 번 고르면 계산 단계에서 거부된다(`plan`과 같은 규칙).
#[test]
fn map_notes_rejects_duplicate_picks() {
    let (d, t, c) = doc(0, &[(0, 960, 60)], (4, 4));
    let mut sel = all(&d, t, c);
    sel.push(sel[0].clone());
    let result = transform::map_notes(d.project(), &sel, |_, _, n| n.with_length(480));
    assert!(
        matches!(result, Err(ModelError::NoteNotFound)),
        "{result:?}"
    );
}

/// 길이 바꾸기: 끝이 클립을 넘으면 마디 단위로 늘린 길이가 편집에 담긴다.
#[test]
fn map_notes_extends_clip_by_bars_when_length_grows() {
    let mut p = Project::new();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let tid = t.id();
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    clip.insert_note(
        Note::new(
            Tick::new(2880),
            960,
            Pitch::new(60).unwrap(),
            Velocity::new(1),
        )
        .unwrap(),
    )
    .unwrap();
    let id = t.insert_clip(clip).unwrap();
    p.push_track(t).unwrap();
    let d = Document::new(p, usize::MAX).unwrap();
    let sel = all(&d, tid, id);
    let edits = transform::map_notes(d.project(), &sel, |_, _, n| n.with_length(1920)).unwrap();
    assert_eq!(edits.len(), 1);
    match &edits[0] {
        keyscore_core::Edit::ReplaceNotes { length, pairs, .. } => {
            assert_eq!(*length, 7680, "끝 4800을 덮는 마디 경계");
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].1.length(), 1920);
        }
        other => panic!("{other:?}"),
    }
}
