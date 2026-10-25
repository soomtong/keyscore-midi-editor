//! S3-T3a 적대적 리뷰 2(core). 리뷰 1 지적 2의 수정(`clip_for_notes`의 새 시작·끝 규칙)을 다시 깨뜨려 보고, 같은
//! 규칙이 `insert_notes`(있던 클립 늘리기)에는 없는 것을 본다(`docs/reports/s3-t3a/review-2.md`).

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

fn inserted(edit: &Edit) -> &Clip {
    let Edit::InsertClip { clip, .. } = edit else {
        panic!("{edit:?}");
    };
    clip
}

// ---------- 지적: 있던 클립을 늘릴 때는 마디 중간에서 시작하는 다음 클립 앞에서 멈추지 않는다 ----------

/// 클립 A [0, 1마디), 다음 클립 B는 마디 중간(5000)에서 시작한다. A 안 3000에 1박 노트(끝 3960)를 넣으면 A를
/// 3960 이상으로 늘려야 한다. 빈 자리 [3840, 5000)에 들어가는데도 마디 끝(7680)까지 늘리려다 B와 겹쳐 거부된다.
/// 리뷰 1 지적 2의 수정은 새 클립(`clip_for_notes`)에만 「다음 클립이 노트 끝과 마디 끝 사이에서 시작하면 끝은 그
/// 시작」을 넣었다. REQ 7.5 「노트는 클립 경계를 넘으면 클립을 늘리고」.
#[test]
fn insert_notes_fits_before_mid_bar_next_clip() {
    let (mut d, t, ids) = doc(&[(0, BAR), (5000, BAR)], None);
    let edits = insert_notes(d.project(), t, ids[0], &[note(3000, 960, 60)]);
    let edits = edits.expect("빈 자리에 들어가는 노트가 거부되었다");
    d.apply(edits, None).unwrap();
    let a = d.project().track(t).unwrap().clip(ids[0]).unwrap();
    assert!(
        a.end() <= 5000 && a.end() >= 3960,
        "A: {}..{}",
        a.start().get(),
        a.end()
    );
}

/// 대조(통과): 같은 노트를 A 끝 틱(3840, 틈)에 두면 `clip_for_notes`는 [3840, 5000)의 새 클립을 만든다. 한 틱 앞
/// (A 안)은 거부되고 A 끝은 받아들여진다.
#[test]
fn clip_for_notes_at_clip_end_fits_before_mid_bar_next_clip() {
    let (d, t, _) = doc(&[(0, BAR), (5000, BAR)], None);
    let edit = clip_for_notes(d.project(), t, &[note(BAR, 960, 60)], Tick::new(BAR)).unwrap();
    let clip = inserted(&edit);
    assert_eq!((clip.start().get(), clip.end()), (BAR, 5000));
}

// ---------- 리뷰 1 지적 2 수정의 경계 ----------

/// 자리 틱이 앞 클립 끝(마디 중간 5760)과 같은 틱이면 새 클립은 그 틱에서 시작한다(앞 클립과 맞닿음, 겹침 없음).
#[test]
fn clip_for_notes_at_prev_mid_bar_end_tick_starts_there() {
    let (mut d, t, _) = doc(&[(0, BAR + 1920)], None);
    let at = BAR + 1920;
    let edit = clip_for_notes(d.project(), t, &[note(at, 960, 60)], Tick::new(at)).unwrap();
    let clip = inserted(&edit);
    assert_eq!((clip.start().get(), clip.end()), (at, 2 * BAR));
    assert_eq!(clip.notes()[0].start().get(), 0);
    d.apply(vec![edit], None).unwrap();
}

/// 앞 클립 끝이 마디 시작과 같으면(마디 경계) 시작은 그 마디 시작이다(`e > bar_start` 경계).
#[test]
fn clip_for_notes_prev_end_on_bar_line_starts_at_bar() {
    let (d, t, _) = doc(&[(0, BAR)], None);
    let edit = clip_for_notes(
        d.project(),
        t,
        &[note(BAR + 960, 960, 60)],
        Tick::new(BAR + 960),
    )
    .unwrap();
    assert_eq!(inserted(&edit).start().get(), BAR);
}

/// 2마디부터 3/4. 앞 클립이 3/4 마디 안(8680)에서 끝나고 자리 틱이 같은 마디(9180)면 [8680, 10560).
#[test]
fn clip_for_notes_mid_bar_prev_end_after_meter_change() {
    let (d, t, _) = doc(&[(0, 2 * BAR + 1000)], Some(2 * BAR));
    let at = 2 * BAR + 1500;
    let edit = clip_for_notes(d.project(), t, &[note(at, 500, 60)], Tick::new(at)).unwrap();
    let clip = inserted(&edit);
    assert_eq!(
        (clip.start().get(), clip.end()),
        (2 * BAR + 1000, 2 * BAR + 2880)
    );
}

/// 노트가 자리 틱보다 앞(앞 클립 끝 뒤, 같은 마디)이면 받아들이고, 앞 클립 끝보다 앞이면 `NoteBeforeClipStart`.
#[test]
fn clip_for_notes_note_before_at_uses_prev_end_as_floor() {
    let (d, t, _) = doc(&[(0, 1000)], None);
    let edit = clip_for_notes(d.project(), t, &[note(2000, 960, 60)], Tick::new(3000)).unwrap();
    assert_eq!(inserted(&edit).start().get(), 1000);
    assert!(matches!(
        clip_for_notes(d.project(), t, &[note(500, 960, 60)], Tick::new(3000)),
        Err(keyscore_core::ModelError::NoteBeforeClipStart)
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// 완전성: 자리 틱이 틈 [lo, hi)에 있고 노트가 모두 그 틈 안(시작 ≥ 자리 틱, 끝 ≤ hi)이면 `clip_for_notes`는
    /// `Ok`이고, 새 클립은 lo 이상에서 시작해 자리 틱을 품고 hi를 넘지 않으며 적용이 성공한다. 클립은 마디 중간
    /// 끝·시작을 섞는다(16분음표 단위).
    #[test]
    fn clip_for_notes_accepts_notes_that_fit_the_gap(
        clips in prop::collection::vec((0u64..40, 1u64..48), 0..5),
        meter in prop::option::of(1u64..8),
        at in 0u64..14 * BAR,
        notes in prop::collection::vec((0u64..BAR, 1u32..2 * 960, 0u8..128), 1..5),
    ) {
        let mut placed = Vec::new();
        let mut cursor = 0;
        for (gap, len) in clips {
            let s = cursor + gap * 240;
            placed.push((s, len * 240));
            cursor = s + len * 240;
        }
        prop_assume!(!placed.iter().any(|&(s, l)| s <= at && at < s + l));
        let lo = placed.iter().map(|&(s, l)| s + l).filter(|&e| e <= at).max().unwrap_or(0);
        let hi = placed.iter().map(|&(s, _)| s).filter(|&s| s > at).min().unwrap_or(u64::MAX);
        let notes: Vec<Note> = notes.iter().map(|&(s, l, p)| note(at + s, l, p)).collect();
        prop_assume!(notes.iter().all(|n| n.end() <= hi));
        let (mut d, t, _) = doc(&placed, meter.map(|m| m * BAR));
        let edit = clip_for_notes(d.project(), t, &notes, Tick::new(at));
        prop_assert!(edit.is_ok(), "{:?} at {} gap [{}, {}): {:?}", placed, at, lo, hi, edit);
        let edit = edit.unwrap();
        let (cs, ce) = {
            let c = inserted(&edit);
            (c.start().get(), c.end())
        };
        prop_assert!(cs >= lo && cs <= at && ce <= hi, "clip [{}, {}) gap [{}, {})", cs, ce, lo, hi);
        d.apply(vec![edit], None).unwrap();
    }
}
