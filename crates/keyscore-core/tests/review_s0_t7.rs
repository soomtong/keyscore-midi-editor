//! S0-T7 적대적 리뷰 1회차의 깨뜨리기 테스트. 공개 API만 쓴다.

use std::panic::{AssertUnwindSafe, catch_unwind};

use keyscore_core::transform::{self, Grid, NoteRef, PitchRange, QuantizeSettings};
use keyscore_core::{
    Channel, Clip, ClipId, CoalesceKey, Document, Edit, ModelError, Note, Pitch, PitchOffset,
    Project, Tick, TimeSignature, Track, TrackColor, TrackId, Velocity,
};
use proptest::prelude::*;

const GRIDS: [Grid; 5] = [
    Grid::Quarter,
    Grid::Eighth,
    Grid::Sixteenth,
    Grid::ThirtySecond,
    Grid::SixtyFourth,
];

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

fn refs_of(p: &Project, t: TrackId, c: ClipId) -> Vec<NoteRef> {
    p.track(t)
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

fn starts(p: &Project, t: TrackId, c: ClipId) -> Vec<u64> {
    let mut v: Vec<u64> = p
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .notes()
        .iter()
        .map(|n| n.start().get())
        .collect();
    v.sort_unstable();
    v
}

/// 박자표 변화 목록과 클립 하나(시작 0)를 가진 문서.
fn doc_with_meter(
    changes: &[(u64, u8, u16)],
    clip_len: u64,
    notes: &[Note],
) -> (Document, TrackId, ClipId) {
    let mut p = Project::new();
    for &(at, n, d) in changes {
        p.meter_map_mut()
            .set(Tick::new(at), TimeSignature::new(n, d).unwrap());
    }
    let mut t = new_track();
    let tid = t.id();
    let mut clip = Clip::new(Tick::ZERO, clip_len).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    let c = t.insert_clip(clip).unwrap();
    p.push_track(t).unwrap();
    (Document::new(p, usize::MAX).unwrap(), tid, c)
}

/// 독립 기준 모델: 노트가 속한 마디 시작부터 잰 모든 그리드 점(홀수 칸은 스윙만큼 뒤로)과 다음
/// 마디 시작 가운데 가장 가까운 점. 같으면 앞쪽. 강도는 이동 크기를 내림해 원래 쪽(0 쪽)으로 자른다.
fn reference_quantize(p: &Project, abs: u64, grid: u64, swing: u64, strength: u64) -> u64 {
    let m = p.meter_map();
    let bar_no = m.position(Tick::new(abs)).bar();
    let bar = m.bar_start(bar_no).unwrap().get();
    let next = m.bar_start(bar_no + 1).unwrap().get();
    let mut points = vec![next];
    let mut k = 0u64;
    while bar + k * grid < next {
        let plain = bar + k * grid;
        points.push(if k % 2 == 1 {
            plain + grid * swing / 100
        } else {
            plain
        });
        k += 1;
    }
    let target = points
        .into_iter()
        .min_by(|a, b| a.abs_diff(abs).cmp(&b.abs_diff(abs)).then(a.cmp(b)))
        .unwrap();
    let mag = target.abs_diff(abs) * strength / 100;
    if target >= abs { abs + mag } else { abs - mag }
}

fn meter_sig() -> impl Strategy<Value = (u8, u16)> {
    (
        1..=15u8,
        prop::sample::select(vec![1u16, 2, 4, 8, 16, 32, 64, 128, 256]),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 스윙·셋잇단·강도·마디 중간 박자표 변화·분모 256을 섞어 기준 모델과 비교한다.
    #[test]
    fn review_quantize_matches_reference_model(
        first in meter_sig(),
        later in prop::collection::vec((1..30_000u64, meter_sig()), 0..3),
        raw_notes in prop::collection::vec((0..40_000u64, 1..500u32), 1..10),
        g in 0..5usize, triplet in any::<bool>(), strength in 0..=100u8, swing in 0..=75u8,
    ) {
        let mut changes = vec![(0, first.0, first.1)];
        changes.extend(later.iter().map(|&(at, (n, d))| (at, n, d)));
        let notes: Vec<Note> = raw_notes.iter().map(|&(s, l)| note(s, l, 60)).collect();
        let (mut d, t, c) = doc_with_meter(&changes, 200_000, &notes);
        let s = QuantizeSettings::new(GRIDS[g], triplet, strength, swing).unwrap();
        let mut want: Vec<u64> = notes
            .iter()
            .map(|n| reference_quantize(d.project(), n.start().get(), s.grid_ticks(), u64::from(swing), u64::from(strength)))
            .collect();
        want.sort_unstable();
        let edits = transform::quantize(d.project(), &refs_of(d.project(), t, c), &s).unwrap();
        d.apply(edits, None).unwrap();
        prop_assert_eq!(starts(d.project(), t, c), want);
    }

    /// 공개 편집(`ExtendClip` 포함)을 임의 순서·묶음으로 적용해도 불변식이 지켜지고,
    /// 모두 되돌리면 처음과 같고, 모두 다시 하면 끝과 같다.
    #[test]
    fn review_extend_clip_mixed_with_other_edits_undo_redo(
        ops in prop::collection::vec((0..5u8, 0..12_000u64, 1..4_000u32, 0..3u64), 1..25),
    ) {
        let mut p = Project::new();
        let mut t = new_track();
        let tid = t.id();
        t.insert_clip(Clip::new(Tick::ZERO, 3840).unwrap()).unwrap();
        t.insert_clip(Clip::new(Tick::new(11_520), 3840).unwrap()).unwrap();
        p.push_track(t).unwrap();
        let mut d = Document::new(p, usize::MAX).unwrap();
        let initial = d.project().clone();
        let mut applied = 0usize;
        for (kind, x, len, key) in ops {
            let track = d.project().track(tid).unwrap();
            let clips: Vec<(ClipId, Clip)> = track.clips().map(|(id, c)| (id, c.clone())).collect();
            let (id, clip) = clips[usize::try_from(x).unwrap() % clips.len()].clone();
            let edit = match kind {
                0 => Edit::ExtendClip { track: tid, id, length: clip.length() + x % 4000 },
                1 => Edit::InsertNote { track: tid, clip: id, note: note(x % clip.length(), len, 60) },
                2 => match clip.notes().first() {
                    Some(n) => Edit::RemoveNote { track: tid, clip: id, note: n.clone() },
                    None => continue,
                },
                3 => Edit::SplitClip { track: tid, id, at: Tick::new(clip.start().get() + x % clip.length()) },
                _ => Edit::MoveClip { track: tid, id, to: Tick::new(x * 2) },
            };
            let coalesce = if key == 0 { None } else { Some(CoalesceKey(key)) };
            let before = d.project().clone();
            let undo_before = d.undo_len();
            if d.apply(vec![edit], coalesce).is_ok() {
                applied += 1;
            } else {
                prop_assert_eq!(d.project(), &before);
                prop_assert_eq!(d.undo_len(), undo_before);
            }
            // 불변식: 클립 겹침 없음, 노트 시작은 클립 안.
            let track = d.project().track(tid).unwrap();
            let mut spans: Vec<(u64, u64)> = track.clips().map(|(_, c)| (c.start().get(), c.end())).collect();
            spans.sort_unstable();
            for w in spans.windows(2) {
                prop_assert!(w[0].1 <= w[1].0);
            }
            for (_, c) in track.clips() {
                for n in c.notes() {
                    prop_assert!(n.start().get() < c.length());
                }
            }
        }
        let last = d.project().clone();
        while d.undo().unwrap() {}
        prop_assert_eq!(d.project(), &initial);
        while d.redo().unwrap() {}
        prop_assert_eq!(d.project(), &last);
        let _ = applied;
    }
}

#[test]
fn review_quantize_strength_rounds_toward_original_both_directions() {
    let (mut d, t, c) = doc_with_meter(&[], 7680, &[note(961, 10, 60), note(1923, 10, 62)]);
    let s = QuantizeSettings::new(Grid::Quarter, false, 50, 0).unwrap();
    let edits = transform::quantize(d.project(), &refs_of(d.project(), t, c), &s).unwrap();
    d.apply(edits, None).unwrap();
    // 961 → 목표 960(−1): 50%는 −0.5 → 0으로 잘려 그대로. 1923 → 목표 1920(−3): −1.5 → −1.
    assert_eq!(starts(d.project(), t, c), vec![961, 1922]);
    let (mut d, t, c) = doc_with_meter(&[], 7680, &[note(959, 10, 60)]);
    let edits = transform::quantize(d.project(), &refs_of(d.project(), t, c), &s).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(starts(d.project(), t, c), vec![959]);
}

#[test]
fn review_quantize_tie_and_short_bar_swing() {
    // 1/4 그리드 정중앙(480)은 앞쪽(0)으로.
    let (mut d, t, c) = doc_with_meter(&[], 7680, &[note(480, 10, 60)]);
    let s = QuantizeSettings::new(Grid::Quarter, false, 100, 0).unwrap();
    let edits = transform::quantize(d.project(), &refs_of(d.project(), t, c), &s).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(starts(d.project(), t, c), vec![0]);
    // 7/16 마디(1680틱)에 1/8 그리드·스윙 75: 마지막 홀수 칸(1440)의 스윙 점 1800은 마디선 너머다.
    // 결과는 다음 마디 시작(1680)을 넘지 않아야 한다.
    let notes: Vec<Note> = (1400..1680).step_by(7).map(|s| note(s, 1, 60)).collect();
    let (mut d, t, c) = doc_with_meter(&[(0, 7, 16)], 7680, &notes);
    let s = QuantizeSettings::new(Grid::Eighth, false, 100, 75).unwrap();
    let edits = transform::quantize(d.project(), &refs_of(d.project(), t, c), &s).unwrap();
    d.apply(edits, None).unwrap();
    assert!(starts(d.project(), t, c).iter().all(|&s| s <= 1680));
}

/// u64 끝 근처 클립에서 모든 연산이 패닉 없이 `Ok`/`Err`를 낸다.
#[test]
fn review_ops_near_u64_end_do_not_panic() {
    for meter in [(4u8, 4u16), (1, 256), (15, 1)] {
        let mut project = Project::new();
        project
            .meter_map_mut()
            .set(Tick::ZERO, TimeSignature::new(meter.0, meter.1).unwrap());
        let mut track = new_track();
        let tid = track.id();
        let start = u64::MAX - 5000;
        let mut clip = Clip::new(Tick::new(start), 4999).unwrap();
        for s in [0u64, 1, 2000, 4000, 4990] {
            clip.insert_note(note(s, 9, 60)).unwrap();
        }
        let cid = track.insert_clip(clip).unwrap();
        project.push_track(track).unwrap();
        let p = project;
        let refs = refs_of(&p, tid, cid);
        for g in GRIDS {
            for triplet in [false, true] {
                let settings = QuantizeSettings::new(g, triplet, 100, 75).unwrap();
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    transform::quantize(&p, &refs, &settings)
                }));
                assert!(outcome.is_ok(), "quantize panicked: {meter:?} {g:?}");
            }
        }
        for delta in [i64::MAX, i64::MIN, 10, -10] {
            let r = catch_unwind(AssertUnwindSafe(|| transform::shift(&p, &refs, delta)));
            assert!(r.is_ok());
        }
        for (n, dd) in [(u32::MAX, 1), (1, u32::MAX), (3, 1)] {
            let r = catch_unwind(AssertUnwindSafe(|| transform::scale(&p, &refs, n, dd)));
            assert!(r.is_ok());
        }
    }
}

/// 큰 반음 수(±1000, i32 끝)에서도 패닉 없이 음역 안·같은 음이름으로 접는다.
#[test]
fn review_transpose_huge_semitones() {
    let (d, t, c) = doc_with_meter(&[], 3840, &[note(0, 10, 0), note(10, 10, 127)]);
    let refs = refs_of(d.project(), t, c);
    for semis in [1000, -1000, i32::MIN, i32::MAX - 200, i32::MAX] {
        let r = catch_unwind(AssertUnwindSafe(|| {
            transform::transpose(d.project(), &refs, semis, PitchRange::full())
        }));
        assert!(r.is_ok(), "transpose({semis}) panicked");
    }
}

#[test]
fn review_transpose_exact_octave_range_folds_every_pitch() {
    let low = Pitch::new(60).unwrap();
    let high = Pitch::new(71).unwrap();
    let range = PitchRange::new(low, high).unwrap();
    let notes: Vec<Note> = (0..=127u8).map(|p| note(u64::from(p), 1, p)).collect();
    let (d, t, c) = doc_with_meter(&[], 3840, &notes);
    let refs = refs_of(d.project(), t, c);
    for semis in (-1000..=1000).step_by(37) {
        let edits = transform::transpose(d.project(), &refs, semis, range).unwrap();
        let mut d2 = d.clone();
        d2.apply(edits, None).unwrap();
        for n in d2.project().track(t).unwrap().clip(c).unwrap().notes() {
            let orig = i32::try_from(n.start().get()).unwrap();
            let got = i32::from(n.pitch().get());
            assert!((60..=71).contains(&got));
            assert_eq!((orig + semis - got).rem_euclid(12), 0);
        }
    }
}

#[test]
fn review_transpose_offset_note_folds_by_absolute_pitch() {
    // 음높이 100 + 오프셋 11.5반음(절대 111.5). +20이면 131.5라 한 옥타브 접혀 108(+11.5 = 119.5).
    let n = note(0, 10, 100)
        .with_offset(PitchOffset::new(PitchOffset::SEMITONE * 23 / 2).unwrap())
        .unwrap();
    let (mut d, t, c) = doc_with_meter(&[], 3840, std::slice::from_ref(&n));
    let edits = transform::transpose(
        d.project(),
        &refs_of(d.project(), t, c),
        20,
        PitchRange::full(),
    )
    .unwrap();
    d.apply(edits, None).unwrap();
    let got = &d.project().track(t).unwrap().clip(c).unwrap().notes()[0];
    assert_eq!(got.pitch().get(), 108);
    assert_eq!(got.offset(), n.offset());
}

/// 분할로 생긴 숨김 꼬리 노트가 든 왼쪽 클립(오른쪽 클립과 맞닿음).
fn split_fixture() -> (Document, TrackId, ClipId, ClipId) {
    let mut p = Project::new();
    let mut t = new_track();
    let tid = t.id();
    let mut clip = Clip::new(Tick::ZERO, 7680).unwrap();
    clip.insert_note(note(0, 480, 60)).unwrap();
    clip.insert_note(note(1920, 3840, 64)).unwrap();
    let left = t.insert_clip(clip).unwrap();
    let right = t.split_clip(left, Tick::new(3840)).unwrap();
    p.push_track(t).unwrap();
    // 전제: 왼쪽 클립에 꼬리 노트(끝 5760 > 길이 3840)가 남았다.
    let lc = p.track(tid).unwrap().clip(left).unwrap();
    assert_eq!(lc.length(), 3840);
    assert!(lc.notes().iter().any(|n| n.end() > lc.length()));
    (Document::new(p, usize::MAX).unwrap(), tid, left, right)
}

/// 시간을 바꾸지 않는 조옮김이 꼬리 노트 때문에 클립을 늘리려다 오른쪽 클립과 겹쳐 거부된다.
#[test]
fn review_transpose_selection_with_hidden_tail_next_to_split_neighbor() {
    let (mut d, t, left, _) = split_fixture();
    let refs = refs_of(d.project(), t, left);
    let edits = transform::transpose(d.project(), &refs, 1, PitchRange::full())
        .expect("조옮김은 시간을 바꾸지 않으므로 클립 경계와 무관해야 한다");
    d.apply(edits, None).unwrap();
    assert_eq!(
        d.project().track(t).unwrap().clip(left).unwrap().length(),
        3840
    );
}

/// 움직이지 않는 연산(시간 이동 0, 강도 0 퀀타이즈)도 같은 이유로 거부된다.
#[test]
fn review_noop_shift_and_zero_strength_quantize_with_hidden_tail() {
    let (d, t, left, _) = split_fixture();
    let refs = refs_of(d.project(), t, left);
    assert!(transform::shift(d.project(), &refs, 0).is_ok());
    let s = QuantizeSettings::new(Grid::Sixteenth, false, 0, 0).unwrap();
    assert!(transform::quantize(d.project(), &refs, &s).is_ok());
}

/// 이웃이 없으면 조옮김만으로 클립이 늘어 숨김 꼬리가 드러난다(REQ 5.10 비파괴 트림과 어긋남).
#[test]
fn review_transpose_does_not_resize_clip_with_hidden_tail() {
    let (d, t, left, right) = split_fixture();
    let mut p = d.into_project();
    let idx = p.track_index(t).unwrap();
    p.track_mut(idx).unwrap().remove_clip(right).unwrap();
    let mut d = Document::new(p, usize::MAX).unwrap();
    let edits = transform::transpose(
        d.project(),
        &refs_of(d.project(), t, left),
        1,
        PitchRange::full(),
    )
    .unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(
        d.project().track(t).unwrap().clip(left).unwrap().length(),
        3840
    );
}

/// 같은 노트를 두 번 고른 선택: 연산은 `Ok` 묶음을 내지만 적용에서 실패한다.
#[test]
fn review_duplicate_ref_in_selection() {
    let (mut d, t, c) = doc_with_meter(&[], 3840, &[note(10, 10, 60)]);
    let mut refs = refs_of(d.project(), t, c);
    refs.push(refs[0].clone());
    let before = d.project().clone();
    match transform::shift(d.project(), &refs, 100) {
        Err(_) => {}
        Ok(edits) => {
            let r = d.apply(edits, None);
            assert_eq!(d.project(), &before);
            assert!(r.is_ok(), "연산이 Ok로 낸 묶음이 적용에서 실패한다: {r:?}");
        }
    }
}

/// 같은 값의 노트가 둘인 클립에서 하나만 고르면 하나만 움직인다.
#[test]
fn review_identical_notes_only_selected_one_moves() {
    let (mut d, t, c) = doc_with_meter(&[], 3840, &[note(10, 10, 60), note(10, 10, 60)]);
    let refs = vec![refs_of(d.project(), t, c)[0].clone()];
    let edits = transform::shift(d.project(), &refs, 100).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(starts(d.project(), t, c), vec![10, 110]);
    assert!(d.undo().unwrap());
    assert_eq!(starts(d.project(), t, c), vec![10, 10]);
}

/// 여러 트랙·여러 클립 선택: 한 번에 적용되고 undo 한 번으로 돌아온다. 한 클립이 거부되면 전체 거부.
#[test]
fn review_multi_track_multi_clip_selection() {
    let mut p = Project::new();
    let mut ids = Vec::new();
    for _ in 0..2 {
        let mut t = new_track();
        let tid = t.id();
        let mut a = Clip::new(Tick::ZERO, 3840).unwrap();
        a.insert_note(note(100, 100, 60)).unwrap();
        let mut b = Clip::new(Tick::new(7680), 3840).unwrap();
        b.insert_note(note(3700, 100, 62)).unwrap();
        let a = t.insert_clip(a).unwrap();
        let b = t.insert_clip(b).unwrap();
        p.push_track(t).unwrap();
        ids.push((tid, a, b));
    }
    let mut d = Document::new(p, usize::MAX).unwrap();
    let before = d.project().clone();
    let refs: Vec<NoteRef> = ids
        .iter()
        .flat_map(|&(t, a, b)| {
            let mut v = refs_of(d.project(), t, a);
            v.extend(refs_of(d.project(), t, b));
            v
        })
        .collect();
    let edits = transform::shift(d.project(), &refs, 1000).unwrap();
    d.apply(edits, None).unwrap();
    for &(t, a, b) in &ids {
        assert_eq!(starts(d.project(), t, a), vec![1100]);
        assert_eq!(starts(d.project(), t, b), vec![4700]);
        assert_eq!(
            d.project().track(t).unwrap().clip(b).unwrap().length(),
            7680
        );
    }
    assert_eq!(d.undo_len(), 1);
    assert!(d.undo().unwrap());
    assert_eq!(d.project(), &before);
    // 첫 클립의 노트가 클립 앞으로 나가면 다른 트랙 것까지 전체 거부.
    assert_eq!(
        transform::shift(d.project(), &refs, -150),
        Err(ModelError::NoteBeforeClipStart)
    );
}

/// 리뷰 1 당시 결정(선택 전체 기준)에서는 두 클립에 걸친 반속이 언제나 거부되어 그 결과를 기록했다.
/// 결정 개정(클립마다 기준점) 뒤에는 각 클립이 자기 첫 노트를 기준으로 반속되어 성공한다.
#[test]
fn review_scale_half_across_two_clips_uses_per_clip_anchor() {
    let mut p = Project::new();
    let mut t = new_track();
    let tid = t.id();
    let mut a = Clip::new(Tick::ZERO, 3840).unwrap();
    a.insert_note(note(0, 480, 60)).unwrap();
    let mut b = Clip::new(Tick::new(7680), 3840).unwrap();
    b.insert_note(note(0, 480, 60)).unwrap();
    let a = t.insert_clip(a).unwrap();
    let b = t.insert_clip(b).unwrap();
    p.push_track(t).unwrap();
    let mut refs = refs_of(&p, tid, a);
    refs.extend(refs_of(&p, tid, b));
    let edits = transform::scale(&p, &refs, 1, 2).unwrap();
    let mut d = keyscore_core::Document::new(p, usize::MAX).unwrap();
    d.apply(edits, None).unwrap();
    for c in [a, b] {
        let n = &d.project().track(tid).unwrap().clip(c).unwrap().notes()[0];
        assert_eq!((n.start().get(), n.length()), (0, 240));
    }
}

/// 비율이 커서 결과가 u64를 넘으면 오버플로 오류여야 한다. 「클립 시작 앞」 오류는 원인을 숨긴다.
#[test]
fn review_scale_overflow_reports_tick_overflow() {
    let (d, t, c) = doc_with_meter(&[], 1 << 40, &[note(0, 1, 60), note(1 << 39, 1, 60)]);
    let r = transform::scale(d.project(), &refs_of(d.project(), t, c), u32::MAX, 1);
    assert_eq!(r, Err(ModelError::TickOverflow));
}

/// `ExtendClip`이 먼저 성공하고 뒤의 노트 삭제가 실패하는 묶음도 원자적이다.
#[test]
fn review_stale_transform_after_extend_is_atomic() {
    let (mut d, t, c) = doc_with_meter(&[], 3840, &[note(3000, 500, 60), note(10, 10, 62)]);
    let refs = refs_of(d.project(), t, c);
    let edits = transform::shift(d.project(), &refs, 500).unwrap();
    assert!(matches!(edits[0], Edit::ExtendClip { .. }));
    // 선택한 노트 하나를 먼저 지워 묶음을 낡게 만든다.
    d.apply(
        vec![Edit::RemoveNote {
            track: t,
            clip: c,
            note: refs[1].note.clone(),
        }],
        None,
    )
    .unwrap();
    let before = d.project().clone();
    let undo_len = d.undo_len();
    assert_eq!(d.apply(edits, None), Err(ModelError::NoteNotFound));
    assert_eq!(d.project(), &before);
    assert_eq!(d.undo_len(), undo_len);
}

/// 늘린 뒤 늘어난 구간에 노트를 넣고(같은 coalesce 키로 묶어) 되돌리고 다시 하기.
#[test]
fn review_extend_then_insert_in_extension_coalesced_undo() {
    let (mut d, t, c) = doc_with_meter(&[], 3840, &[]);
    let before = d.project().clone();
    let k = Some(CoalesceKey(7));
    d.apply(
        vec![Edit::ExtendClip {
            track: t,
            id: c,
            length: 7680,
        }],
        k,
    )
    .unwrap();
    d.apply(
        vec![Edit::InsertNote {
            track: t,
            clip: c,
            note: note(5000, 2000, 60),
        }],
        k,
    )
    .unwrap();
    d.apply(
        vec![Edit::ExtendClip {
            track: t,
            id: c,
            length: 11_520,
        }],
        None,
    )
    .unwrap();
    let after = d.project().clone();
    assert!(d.undo().unwrap());
    assert!(d.undo().unwrap());
    assert_eq!(d.project(), &before);
    assert!(d.redo().unwrap());
    assert!(d.redo().unwrap());
    assert_eq!(d.project(), &after);
}
