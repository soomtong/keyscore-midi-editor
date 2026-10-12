//! 일괄 연산의 속성 기반 테스트(S0-T7 게이트). 공개 API만 쓴다.

use keyscore_core::transform::{self, Grid, NoteRef, PitchRange, QuantizeSettings};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Note, Pitch, Project, Tick, TimeSignature, Track, TrackColor,
    TrackId, Velocity,
};
use proptest::prelude::*;

const GRIDS: [Grid; 5] = [
    Grid::Quarter,
    Grid::Eighth,
    Grid::Sixteenth,
    Grid::ThirtySecond,
    Grid::SixtyFourth,
];

/// 클립 하나(시작 `clip_start`, 길이 넉넉히)와 노트들. 박자표는 `meter`.
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
    prop::collection::vec((0..20_000u64, 1..2_000u32, 0..=127u8), 0..12)
}

fn meter_strategy() -> impl Strategy<Value = (u8, u16)> {
    (1..=12u8, prop::sample::select(vec![2u16, 4, 8, 16]))
}

/// 독립 계산: 절대 위치 `abs`가 속한 마디의 시작부터 잰 가장 가까운 그리드 점(스윙 없음).
/// 마디 끝이 그리드의 배수가 아니면 다음 마디 시작도 후보다. 같으면 앞쪽.
fn nearest_plain_grid(p: &Project, abs: u64, grid: u64) -> u64 {
    let pos = p.meter_map().position(Tick::new(abs));
    let bar = p.meter_map().bar_start(pos.bar()).unwrap().get();
    let next = p.meter_map().bar_start(pos.bar() + 1).unwrap().get();
    let k = (abs - bar) / grid;
    let mut best = bar + k * grid;
    for cand in [bar + (k + 1) * grid, next] {
        if cand <= next && cand.abs_diff(abs) < best.abs_diff(abs) {
            best = cand;
        }
    }
    best
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn quantize_full_strength_is_idempotent(
        notes in notes_strategy(), clip_start in 0..5_000u64, meter in meter_strategy(),
        g in 0..5usize, triplet in any::<bool>(), swing in 0..=75u8,
    ) {
        let (mut d, t, c) = doc(clip_start, &notes, meter);
        let s = QuantizeSettings::new(GRIDS[g], triplet, 100, swing).unwrap();
        let Ok(edits) = transform::quantize(d.project(), &all(&d, t, c), &s) else { return Ok(()) };
        d.apply(edits, None).unwrap();
        let once = d.project().clone();
        let edits = transform::quantize(d.project(), &all(&d, t, c), &s).unwrap();
        d.apply(edits, None).unwrap();
        prop_assert_eq!(d.project(), &once);
    }

    #[test]
    fn quantize_swing_zero_equals_plain_grid(
        notes in notes_strategy(), clip_start in 0..5_000u64, meter in meter_strategy(), g in 0..5usize, triplet in any::<bool>(),
    ) {
        let (mut d, t, c) = doc(clip_start, &notes, meter);
        let s = QuantizeSettings::new(GRIDS[g], triplet, 100, 0).unwrap();
        let before: Vec<Note> = d.project().track(t).unwrap().clip(c).unwrap().notes().to_vec();
        let Ok(edits) = transform::quantize(d.project(), &all(&d, t, c), &s) else { return Ok(()) };
        let mut expected: Vec<u64> = before
            .iter()
            .map(|n| nearest_plain_grid(d.project(), clip_start + n.start().get(), s.grid_ticks()) - clip_start)
            .collect();
        expected.sort_unstable();
        d.apply(edits, None).unwrap();
        let mut got: Vec<u64> = d.project().track(t).unwrap().clip(c).unwrap().notes().iter().map(|n| n.start().get()).collect();
        got.sort_unstable();
        prop_assert_eq!(got, expected);
    }

    #[test]
    fn transpose_by_octave_inside_range_is_exact(notes in notes_strategy(), up in any::<bool>()) {
        let (mut d, t, c) = doc(0, &notes, (4, 4));
        let delta = if up { 12 } else { -12 };
        let before: Vec<u8> = d.project().track(t).unwrap().clip(c).unwrap().notes().iter().map(|n| n.pitch().get()).collect();
        let edits = transform::transpose(d.project(), &all(&d, t, c), delta, PitchRange::full()).unwrap();
        d.apply(edits, None).unwrap();
        let mut got: Vec<(u64, u8)> = d.project().track(t).unwrap().clip(c).unwrap().notes().iter().map(|n| (n.start().get(), n.pitch().get())).collect();
        got.sort_unstable();
        let mut want: Vec<(u64, u8)> = d.project().track(t).unwrap().clip(c).unwrap().notes().iter().map(|n| n.start().get()).zip(before.iter().map(|&p| {
            let moved = i32::from(p) + delta;
            // 음역 안이면 정확히 ±12, 밖이면 한 옥타브 접혀 원래 자리.
            if (0..=127).contains(&moved) { u8::try_from(moved).unwrap() } else { p }
        })).collect();
        want.sort_unstable();
        // 시작 순서로 짝지은 비교가 노트 재정렬에 흔들리지 않도록 다중집합으로 본다.
        let mut gp: Vec<u8> = got.iter().map(|x| x.1).collect();
        let mut wp: Vec<u8> = want.iter().map(|x| x.1).collect();
        gp.sort_unstable();
        wp.sort_unstable();
        prop_assert_eq!(gp, wp);
    }

    #[test]
    fn transform_edits_apply_as_one_undo_unit(
        notes in notes_strategy(), clip_start in 0..5_000u64, op in 0..4u8,
        amount in -3_000i64..3_000, g in 0..5usize, num in 1..4u32, den in 1..4u32,
    ) {
        let (mut d, t, c) = doc(clip_start, &notes, (4, 4));
        let before = d.project().clone();
        let refs = all(&d, t, c);
        let edits = match op {
            0 => transform::quantize(d.project(), &refs, &QuantizeSettings::new(GRIDS[g], false, 100, 0).unwrap()),
            1 => transform::transpose(d.project(), &refs, i32::try_from(amount / 100).unwrap(), PitchRange::full()),
            2 => transform::shift(d.project(), &refs, amount),
            _ => transform::scale(d.project(), &refs, num, den),
        };
        let Ok(edits) = edits else { return Ok(()) };
        d.apply(edits, None).unwrap();
        prop_assert!(d.undo_len() <= 1);
        // 결과의 불변식: 노트 시작은 클립 안, 클립은 겹치지 않는다(모델이 보장하지만 경로를 확인한다).
        let track = d.project().track(t).unwrap();
        for (_, clip) in track.clips() {
            for n in clip.notes() {
                prop_assert!(n.start().get() < clip.length());
            }
        }
        if d.undo_len() == 1 {
            prop_assert!(d.undo().unwrap());
        }
        prop_assert_eq!(d.project(), &before);
    }
}
