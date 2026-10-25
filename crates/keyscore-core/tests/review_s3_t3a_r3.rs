//! S3-T3a 적대적 리뷰 3(core). 리뷰 2 지적 1의 수정(`insert_notes`의 연장 끝을 마디 중간 다음 클립 시작에서 멈춤)을
//! 꼬리 노트, 박자표 변화, 다음 클립 여럿, 마디에 맞지 않는 클립에서 다시 깨뜨려 본다
//! (`docs/reports/s3-t3a/review-3.md`).

use keyscore_core::transform::insert_notes;
use keyscore_core::{
    Channel, Clip, ClipId, Document, Note, Pitch, Project, Tick, TimeSignature, Track, TrackColor,
    TrackId, Velocity,
};
use proptest::prelude::*;

const BAR: u64 = 3840;
const BAR34: u64 = 2880;

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

fn clip_of(d: &Document, t: TrackId, c: ClipId) -> Clip {
    d.project().track(t).unwrap().clip(c).unwrap().clone()
}

/// `x` 이상인 첫 마디선. 4/4로 시작해 `meter`(마디선 위)부터 3/4.
fn bar_line_at_or_after(x: u64, meter: Option<u64>) -> u64 {
    match meter {
        Some(m) if x > m => m + (x - m).div_ceil(BAR34) * BAR34,
        _ => x.div_ceil(BAR) * BAR,
    }
}

/// 숨김 꼬리가 있는 클립 A [0, 1마디)(노트 3000, 1920틱, 끝 4920)와 B를 `b_start`에 둔 문서. A는 [0, 2마디)를
/// 1마디에서 나눈 왼쪽이다(결정 Q1). 오른쪽은 지우고 B를 따로 넣는다.
fn tail_doc(b_start: u64) -> (Document, TrackId, ClipId) {
    let mut p = Project::new();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let tid = t.id();
    let mut whole = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    whole.insert_note(note(3000, 1920, 60)).unwrap();
    let a = t.insert_clip(whole).unwrap();
    let right = t.split_clip(a, Tick::new(BAR)).unwrap();
    t.remove_clip(right).unwrap();
    t.insert_clip(Clip::new(Tick::new(b_start), BAR).unwrap())
        .unwrap();
    p.push_track(t).unwrap();
    (Document::new(p, usize::MAX).unwrap(), tid, a)
}

// ---------- 꼬리 노트 ----------

/// 숨김 꼬리(끝 4920)가 있는 A에 3500(끝 4460) 노트를 넣으면 A는 다음 클립 B(6000) 시작까지 늘어난다. 꼬리는 늘린 A
/// 안에 든다.
#[test]
fn insert_notes_with_hidden_tail_extends_to_mid_bar_next_clip() {
    let (mut d, t, a) = tail_doc(6000);
    let edits = insert_notes(d.project(), t, a, &[note(3500, 960, 62)]).unwrap();
    d.apply(edits, None).unwrap();
    let c = clip_of(&d, t, a);
    assert_eq!(c.length(), 6000);
    assert_eq!(c.notes().len(), 2);
}

/// B가 꼬리 구간 안(4500, 꼬리 끝 4920 앞)에서 시작해도 새 노트(끝 4460)는 B 앞에 들어간다. A는 4500까지 늘고
/// 꼬리는 그대로 숨김이다.
#[test]
fn insert_notes_with_hidden_tail_over_next_clip_stops_at_next_start() {
    let (mut d, t, a) = tail_doc(4500);
    let edits = insert_notes(d.project(), t, a, &[note(3500, 960, 62)]).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(clip_of(&d, t, a).length(), 4500);
    // 되돌리면 길이도 돌아온다
    d.undo().unwrap();
    assert_eq!(clip_of(&d, t, a).length(), BAR);
}

/// 꼬리 노트 자체를 폭만큼 뒤로 복제한 사본(4920..6840)은 B(6000)에 닿아 거부된다.
#[test]
fn insert_notes_tail_copy_reaching_next_clip_is_rejected() {
    let (d, t, a) = tail_doc(6000);
    assert!(insert_notes(d.project(), t, a, &[note(4920, 1920, 60)]).is_err());
}

// ---------- 다음 클립 여럿, 박자표, 마디에 맞지 않는 클립 ----------

/// 다음 클립이 둘(5000, 6000)이면 가까운 쪽에서 멈춘다. 노트가 가까운 쪽에 닿으면 먼 쪽이 비어도 거부한다.
#[test]
fn insert_notes_stops_at_nearest_of_several_next_clips() {
    let (mut d, t, ids) = doc(&[(0, BAR), (6000, 500), (5000, 500)], None);
    let edits = insert_notes(d.project(), t, ids[0], &[note(3000, 960, 60)]).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(clip_of(&d, t, ids[0]).length(), 5000);
    let (d, t, ids) = doc(&[(0, BAR), (6000, 500), (5000, 500)], None);
    assert!(insert_notes(d.project(), t, ids[0], &[note(3000, 2100, 60)]).is_err());
}

/// 1마디부터 3/4. A [0, 1마디), B는 3/4 마디 안(5840)에서 시작한다. 3000의 1박 노트(끝 3960)로 A는 5840까지,
/// B가 3/4 마디 끝(6720) 뒤라면 6720까지.
#[test]
fn insert_notes_after_meter_change_stops_at_mid_bar_next_clip() {
    let (mut d, t, ids) = doc(&[(0, BAR), (5840, 500)], Some(BAR));
    let edits = insert_notes(d.project(), t, ids[0], &[note(3000, 960, 60)]).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(clip_of(&d, t, ids[0]).length(), 5840);
    let (mut d, t, ids) = doc(&[(0, BAR), (6720, 500)], Some(BAR));
    let edits = insert_notes(d.project(), t, ids[0], &[note(3000, 960, 60)]).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(clip_of(&d, t, ids[0]).length(), BAR + BAR34);
}

/// 마디에 맞지 않는 클립 A [960, 3840)(1박에서 시작, 길이 2880)에 끝 4000의 노트: 다음 클립 4500 앞에서 멈춘다.
/// 노트 끝이 정확히 다음 클립 시작이면 그 틱까지(맞닿음).
#[test]
fn insert_notes_unaligned_clip_stops_at_next_and_touching_end_is_ok() {
    let (mut d, t, ids) = doc(&[(960, 2880), (4500, 500)], None);
    let edits = insert_notes(d.project(), t, ids[0], &[note(2800, 240, 60)]).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(clip_of(&d, t, ids[0]).end(), 4500);
    let (mut d, t, ids) = doc(&[(960, 2880), (4500, 500)], None);
    let edits = insert_notes(d.project(), t, ids[0], &[note(2800, 740, 60)]).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(clip_of(&d, t, ids[0]).end(), 4500);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// 클립 A(시작·길이 임의, 16분음표 단위), 다음 클립 0–3개(마디 중간 포함), 마디선 위 3/4 변화, 노트 1–6개.
    /// 기대: 노트 끝이 A 끝 이하면 길이 그대로, 아니면 다음 클립 가운데 첫 시작 이하일 때만 `Ok`이고 늘린 끝은
    /// min(그 시작, 노트 끝 이상 첫 마디선). 다음 클립에 닿으면 `Err`. `Ok`면 적용이 성공한다.
    #[test]
    fn insert_notes_extends_exactly_to_min_of_bar_line_and_next_clip(
        a_start in 0u64..16,
        a_len in 1u64..24,
        gaps in prop::collection::vec((0u64..40, 1u64..12), 0..3),
        meter_bar in prop::option::of(1u64..6),
        notes in prop::collection::vec((0u64..40, 1u32..4000, 0u8..128), 1..6),
    ) {
        let a_start = a_start * 240;
        let a_len = a_len * 240;
        let meter = meter_bar.map(|b| b * BAR);
        let mut clips = vec![(a_start, a_len)];
        let mut cursor = a_start + a_len;
        for &(gap, len) in &gaps {
            let s = cursor + gap * 120;
            clips.push((s, len * 240));
            cursor = s + len * 240;
        }
        let (mut d, t, ids) = doc(&clips, meter);
        let notes: Vec<Note> = notes
            .iter()
            .map(|&(s, l, p)| note((s * 240) % a_len, l, p))
            .collect();
        let end_rel = notes.iter().map(Note::end).max().unwrap();
        let end_abs = a_start + end_rel;
        let first_next = clips.get(1).map(|c| c.0);
        let result = insert_notes(d.project(), t, ids[0], &notes);
        if end_rel <= a_len {
            let edits = result.unwrap();
            d.apply(edits, None).unwrap();
            prop_assert_eq!(clip_of(&d, t, ids[0]).length(), a_len);
        } else if first_next.is_some_and(|n| end_abs > n) {
            prop_assert!(result.is_err(), "{result:?}");
        } else {
            let bar = bar_line_at_or_after(end_abs, meter);
            let stop = first_next.map_or(bar, |n| n.min(bar));
            let edits = result.unwrap();
            d.apply(edits, None).unwrap();
            let c = clip_of(&d, t, ids[0]);
            prop_assert_eq!(c.end(), stop);
            prop_assert_eq!(c.notes().len(), notes.len());
        }
    }
}

/// 박자표가 마디 중간(5000)에서 바뀌면 [3840, 5000)은 짧은 마디다. 3000의 1박 노트(끝 3960)로 A는 그 짧은 마디 끝
/// (5000)까지 늘어난다. 같은 자리에 다음 클립이 있어도(5000) 맞닿아 받아들인다.
#[test]
fn insert_notes_short_bar_before_mid_bar_meter_change() {
    for next in [None, Some(5000)] {
        let mut clips = vec![(0, BAR)];
        clips.extend(next.map(|s| (s, 500)));
        let (mut d, t, ids) = doc(&clips, Some(5000));
        let edits = insert_notes(d.project(), t, ids[0], &[note(3000, 960, 60)]).unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(clip_of(&d, t, ids[0]).length(), 5000, "next {next:?}");
    }
}
