//! 선택한 노트에 대한 일괄 연산: 퀀타이즈(EDT-2), 조옮김·시간 이동·길이 스케일(EDT-3).
//!
//! 함수는 프로젝트를 바꾸지 않고 [`Edit`] 묶음을 돌려준다. 호출자가 [`crate::Document::apply`]로
//! 한 undo 단위로 적용한다. 규칙은 `docs/decisions/2026-10-12-quantize-and-transform-rules.md`를
//! 따른다: 퀀타이즈는 시작만 옮기고, 그리드는 곡의 마디 기준이며, 결과가 클립 끝을 넘으면 클립을
//! 마디 단위로 늘리고(다음 클립과 겹치면 거부), 클립 시작 앞으로 나가면 거부한다.

use crate::{ClipId, Edit, ModelError, Note, Pitch, Project, TrackId};

/// 선택 항목 하나. 모델 불변식을 담는 타입이 아니라 「무엇을 고를지」를 담는 값이다. 잘못된
/// 참조는 연산에서 거부된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRef {
    pub track: TrackId,
    pub clip: ClipId,
    pub note: Note,
}

/// 퀀타이즈 그리드(EDT-2: 1/4–1/64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grid {
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
}

/// 퀀타이즈 설정. 강도 0–100%, 스윙 0–75%.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantizeSettings {
    grid: Grid,
    triplet: bool,
    strength: u8,
    swing: u8,
}

impl QuantizeSettings {
    /// # Errors
    ///
    /// 강도가 100을, 스윙이 75를 넘으면 [`ModelError::QuantizeSettingOutOfRange`].
    pub fn new(grid: Grid, triplet: bool, strength: u8, swing: u8) -> Result<Self, ModelError> {
        todo!("{grid:?} {triplet} {strength} {swing}")
    }

    /// 그리드 한 칸의 틱 수. 셋잇단이면 2/3.
    #[must_use]
    pub fn grid_ticks(&self) -> u64 {
        todo!()
    }
}

/// 조옮김 결과를 접어 넣을 음역. 최소 한 옥타브라서 언제나 접을 수 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PitchRange {
    low: Pitch,
    high: Pitch,
}

impl PitchRange {
    /// # Errors
    ///
    /// `high − low < 11`이면(한 옥타브 미만) [`ModelError::PitchRangeTooNarrow`].
    pub fn new(low: Pitch, high: Pitch) -> Result<Self, ModelError> {
        todo!("{low:?} {high:?}")
    }

    /// MIDI 전체 음역 0–127.
    #[must_use]
    pub fn full() -> Self {
        todo!()
    }
}

/// 퀀타이즈한다(EDT-2). 시작만 옮기고 길이는 유지한다.
///
/// # Errors
///
/// 참조가 맞지 않거나, 결과가 클립 시작 앞으로 나가거나, 클립을 늘려야 하는데 다음 클립과 겹치면 `Err`.
pub fn quantize(
    project: &Project,
    notes: &[NoteRef],
    settings: &QuantizeSettings,
) -> Result<Vec<Edit>, ModelError> {
    todo!("{} {} {settings:?}", project.tracks().len(), notes.len())
}

/// 조옮김한다(EDT-3). 음역 밖이면 옥타브 단위로 접는다.
///
/// # Errors
///
/// 참조가 맞지 않거나, 접어도 절대 피치(음높이 + 오프셋)가 MIDI 2.0 범위에 들지 않으면 `Err`.
pub fn transpose(
    project: &Project,
    notes: &[NoteRef],
    semitones: i32,
    range: PitchRange,
) -> Result<Vec<Edit>, ModelError> {
    todo!(
        "{} {} {semitones} {range:?}",
        project.tracks().len(),
        notes.len()
    )
}

/// 시간 이동한다(EDT-3). 노트는 자기 클립 안에서만 움직인다.
///
/// # Errors
///
/// [`quantize`]와 같다.
pub fn shift(
    project: &Project,
    notes: &[NoteRef],
    delta_ticks: i64,
) -> Result<Vec<Edit>, ModelError> {
    todo!("{} {} {delta_ticks}", project.tracks().len(), notes.len())
}

/// 길이 스케일(EDT-3). 선택의 가장 이른 절대 시작을 기준으로 시작 간격과 길이를 함께
/// `numerator / denominator`배 한다. 길이는 최소 1틱이다.
///
/// # Errors
///
/// 비율이 0이면 [`ModelError::ScaleRatioInvalid`]. 그 밖은 [`quantize`]와 같다.
pub fn scale(
    project: &Project,
    notes: &[NoteRef],
    numerator: u32,
    denominator: u32,
) -> Result<Vec<Edit>, ModelError> {
    todo!(
        "{} {} {numerator}/{denominator}",
        project.tracks().len(),
        notes.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Channel, Clip, Document, PitchOffset, Tick, TimeSignature, Track, TrackColor, Velocity,
    };

    fn note(start: u64, length: u32, pitch: u8) -> Note {
        Note::new(
            Tick::new(start),
            length,
            Pitch::new(pitch).unwrap(),
            Velocity::new(1000),
        )
        .unwrap()
    }

    /// 트랙 하나에 클립 둘: a(시작 `a_start`, 길이 3840), b(10 마디 뒤).
    fn doc_with(a_start: u64, notes: &[Note]) -> (Document, TrackId, ClipId, ClipId) {
        let mut p = Project::new();
        let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
        let tid = t.id();
        let mut clip = Clip::new(Tick::new(a_start), 3840).unwrap();
        for n in notes {
            clip.insert_note(n.clone()).unwrap();
        }
        let a = t.insert_clip(clip).unwrap();
        let b = t
            .insert_clip(Clip::new(Tick::new(10 * 3840), 3840).unwrap())
            .unwrap();
        p.push_track(t).unwrap();
        (Document::new(p, usize::MAX).unwrap(), tid, a, b)
    }

    fn refs(d: &Document, t: TrackId, c: ClipId) -> Vec<NoteRef> {
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

    fn starts(d: &Document, t: TrackId, c: ClipId) -> Vec<u64> {
        d.project()
            .track(t)
            .unwrap()
            .clip(c)
            .unwrap()
            .notes()
            .iter()
            .map(|n| n.start().get())
            .collect()
    }

    fn q(grid: Grid, triplet: bool, strength: u8, swing: u8) -> QuantizeSettings {
        QuantizeSettings::new(grid, triplet, strength, swing).unwrap()
    }

    #[test]
    fn quantize_settings_validate_and_grid_ticks() {
        assert_eq!(q(Grid::Quarter, false, 100, 0).grid_ticks(), 960);
        assert_eq!(q(Grid::Eighth, true, 100, 0).grid_ticks(), 320);
        assert_eq!(q(Grid::SixtyFourth, true, 100, 0).grid_ticks(), 40);
        assert!(QuantizeSettings::new(Grid::Quarter, false, 101, 0).is_err());
        assert!(QuantizeSettings::new(Grid::Quarter, false, 100, 76).is_err());
        assert!(QuantizeSettings::new(Grid::Quarter, false, 0, 75).is_ok());
    }

    #[test]
    fn quantize_snaps_to_bar_relative_grid() {
        // 클립이 박에서 어긋난 자리(100틱)에 있어도 곡의 마디 그리드에 맞는다.
        let (mut d, t, a, _) = doc_with(
            100,
            &[note(0, 10, 60), note(400, 10, 62), note(900, 10, 64)],
        );
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Quarter, false, 100, 0),
        )
        .unwrap_err();
        // 절대 100 → 0은 클립 시작 앞이라 거부된다.
        assert_eq!(edits, ModelError::NoteBeforeClipStart);
        let (mut d, t, a, _) = doc_with(100, &[note(400, 10, 62), note(900, 10, 64)]);
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Quarter, false, 100, 0),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        // 절대 500 → 960, 1000 → 960 (클립 기준 860).
        assert_eq!(starts(&d, t, a), vec![860, 860]);
    }

    #[test]
    fn quantize_bar_end_uses_next_bar_start() {
        // 7/8 마디(3360틱)에 4분음표 그리드: 마디 안 점은 0·960·1920·2880, 그다음은 다음 마디 시작 3360.
        let (mut d, t, a, _) = doc_with(0, &[note(3200, 10, 60)]);
        let mut p = d.into_project();
        p.meter_map_mut()
            .set(Tick::ZERO, TimeSignature::new(7, 8).unwrap());
        let mut d = Document::new(p, usize::MAX).unwrap();
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Quarter, false, 100, 0),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![3360]);
    }

    #[test]
    fn quantize_strength_and_swing_values() {
        let (mut d, t, a, _) = doc_with(0, &[note(100, 10, 60)]);
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Quarter, false, 50, 0),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![50], "강도 50%: 100 → 0의 절반");

        // 8분음표 그리드에 스윙 50%: 홀수 칸(480)은 480 + 240 = 720.
        let (mut d, t, a, _) = doc_with(0, &[note(700, 10, 60), note(1000, 10, 62)]);
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Eighth, false, 100, 50),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![720, 960]);

        // 스윙 75%: 홀수 칸 480 → 840.
        let (mut d, t, a, _) = doc_with(0, &[note(800, 10, 60)]);
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Eighth, false, 100, 75),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![840]);
    }

    #[test]
    fn quantize_triplet_grid() {
        // 8분음표 셋잇단 그리드 320틱: 650 → 640.
        let (mut d, t, a, _) = doc_with(0, &[note(650, 10, 60)]);
        let edits = quantize(d.project(), &refs(&d, t, a), &q(Grid::Eighth, true, 100, 0)).unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![640]);
    }

    #[test]
    fn transpose_folds_into_range() {
        let (mut d, t, a, _) = doc_with(0, &[note(0, 10, 120), note(10, 10, 5)]);
        let edits = transpose(d.project(), &refs(&d, t, a), 12, PitchRange::full()).unwrap();
        d.apply(edits, None).unwrap();
        let pitches: Vec<u8> = d
            .project()
            .track(t)
            .unwrap()
            .clip(a)
            .unwrap()
            .notes()
            .iter()
            .map(|n| n.pitch().get())
            .collect();
        // 120 + 12 = 132 → 120(한 옥타브 접음), 5 + 12 = 17.
        assert_eq!(pitches, vec![120, 17]);

        let range = PitchRange::new(Pitch::new(60).unwrap(), Pitch::new(71).unwrap()).unwrap();
        let (mut d, t, a, _) = doc_with(0, &[note(0, 10, 60)]);
        let edits = transpose(d.project(), &refs(&d, t, a), 25, range).unwrap();
        d.apply(edits, None).unwrap();
        // 60 + 25 = 85 → 73 → 61.
        assert_eq!(
            d.project().track(t).unwrap().clip(a).unwrap().notes()[0]
                .pitch()
                .get(),
            61
        );

        assert!(PitchRange::new(Pitch::new(60).unwrap(), Pitch::new(70).unwrap()).is_err());

        // 오프셋 +반음 1개를 가진 127은 128반음이라 범위 밖. 접어서 들어가야 한다.
        let bent = note(0, 10, 115)
            .with_offset(PitchOffset::new(PitchOffset::SEMITONE).unwrap())
            .unwrap();
        let (mut d, t, a, _) = doc_with(0, &[bent]);
        let edits = transpose(d.project(), &refs(&d, t, a), 12, PitchRange::full()).unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(
            d.project().track(t).unwrap().clip(a).unwrap().notes()[0]
                .pitch()
                .get(),
            115
        );
    }

    #[test]
    fn shift_past_end_extends_clip_by_bars() {
        let (mut d, t, a, _) = doc_with(0, &[note(3000, 480, 60)]);
        let edits = shift(d.project(), &refs(&d, t, a), 1000).unwrap();
        d.apply(edits, None).unwrap();
        let clip = d.project().track(t).unwrap().clip(a).unwrap();
        // 끝 4480 > 3840 → 다음 마디 끝 7680까지 늘린다.
        assert_eq!((clip.length(), starts(&d, t, a)), (7680, vec![4000]));
        assert!(d.undo().unwrap());
        assert_eq!(
            d.project().track(t).unwrap().clip(a).unwrap().length(),
            3840
        );
    }

    #[test]
    fn shift_before_start_is_rejected() {
        let (d, t, a, _) = doc_with(0, &[note(100, 10, 60)]);
        assert_eq!(
            shift(d.project(), &refs(&d, t, a), -101).unwrap_err(),
            ModelError::NoteBeforeClipStart
        );
        assert!(shift(d.project(), &refs(&d, t, a), -100).is_ok());
    }

    #[test]
    fn extend_into_next_clip_is_rejected() {
        // 클립 b는 10마디 뒤. 노트를 9마디 넘게 밀면 a가 b와 겹친다.
        let (d, t, a, _) = doc_with(0, &[note(0, 10, 60)]);
        assert!(matches!(
            shift(d.project(), &refs(&d, t, a), 10 * 3840).unwrap_err(),
            ModelError::ClipOverlap { .. }
        ));
    }

    #[test]
    fn scale_doubles_spacing_and_length() {
        let (mut d, t, a, _) = doc_with(0, &[note(480, 240, 60), note(960, 240, 62)]);
        let edits = scale(d.project(), &refs(&d, t, a), 2, 1).unwrap();
        d.apply(edits, None).unwrap();
        let notes = d
            .project()
            .track(t)
            .unwrap()
            .clip(a)
            .unwrap()
            .notes()
            .to_vec();
        // 기준은 가장 이른 시작 480: 480 → 480, 960 → 1440. 길이 240 → 480.
        let got: Vec<(u64, u32)> = notes
            .iter()
            .map(|n| (n.start().get(), n.length()))
            .collect();
        assert_eq!(got, vec![(480, 480), (1440, 480)]);

        let (mut d, t, a, _) = doc_with(0, &[note(0, 1, 60), note(1001, 3, 62)]);
        let edits = scale(d.project(), &refs(&d, t, a), 1, 2).unwrap();
        d.apply(edits, None).unwrap();
        let got: Vec<(u64, u32)> = d
            .project()
            .track(t)
            .unwrap()
            .clip(a)
            .unwrap()
            .notes()
            .iter()
            .map(|n| (n.start().get(), n.length()))
            .collect();
        // 반올림(0.5는 올림): 1001/2 = 500.5 → 501, 3/2 = 1.5 → 2, 1/2 = 0.5 → 1(최소 1).
        assert_eq!(got, vec![(0, 1), (501, 2)]);
        assert!(scale(d.project(), &[], 0, 1).is_err());
        assert!(scale(d.project(), &[], 1, 0).is_err());
    }

    #[test]
    fn stale_reference_is_rejected_on_apply() {
        let (mut d, t, a, _) = doc_with(0, &[note(100, 10, 60)]);
        let stale = vec![NoteRef {
            track: t,
            clip: a,
            note: note(5, 10, 60),
        }];
        let edits = shift(d.project(), &stale, 10);
        // 계산 단계나 적용 단계 중 하나에서 거부되고 문서는 그대로다.
        let before = d.project().clone();
        if let Ok(edits) = edits {
            assert!(d.apply(edits, None).is_err());
        }
        assert_eq!(d.project(), &before);
    }
}
