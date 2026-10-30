//! Step 입력 상태(S3-T4a, REQ 5.1, 5.6). GPUI와 무관한 값과 계산이다. 창 처리기는 `step_input.rs`에 있다.
//! 규칙은 `docs/decisions/2026-10-29-s3-t4-step-input-details.md`.

use keyscore_core::{ClipId, Note, Pitch, Tick, TrackId};

use crate::actions::args::{KeyLayout, LengthCode};
use crate::pianoroll::edit::length_ticks;

/// 스티키 길이: 길이 코드와 점 여부.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepLength {
    pub code: LengthCode,
    pub dotted: bool,
}

impl StepLength {
    #[must_use]
    pub const fn plain(code: LengthCode) -> Self {
        StepLength {
            code,
            dotted: false,
        }
    }
}

/// 한 스텝의 틱. 셋잇단은 2/3, `Shift+음`은 절반. 모든 조합이 정수다(가장 짧은 점64분 셋잇단 절반 = 30).
#[must_use]
pub fn step_ticks(length: StepLength, triplet: bool, half: bool) -> u32 {
    let mut ticks = length_ticks(length.code, length.dotted);
    if triplet {
        ticks = ticks * 2 / 3;
    }
    if half {
        ticks /= 2;
    }
    ticks
}

/// 상태줄의 길이 표기: 4분 `♩`, 8분 `♪`, 그 밖은 분수. 점음표는 뒤에 `.`.
#[must_use]
pub fn length_label(length: StepLength) -> String {
    let base = match length.code {
        LengthCode::Whole => "1/1",
        LengthCode::Half => "1/2",
        LengthCode::Quarter => "♩",
        LengthCode::Eighth => "♪",
        LengthCode::Sixteenth => "1/16",
        LengthCode::ThirtySecond => "1/32",
        LengthCode::SixtyFourth => "1/64",
    };
    if length.dotted {
        format!("{base}.")
    } else {
        base.to_owned()
    }
}

/// 길이 코드의 숫자 키(상태줄의 조합 표시).
#[must_use]
pub const fn code_digit(code: LengthCode) -> char {
    match code {
        LengthCode::Whole => '1',
        LengthCode::Half => '2',
        LengthCode::Quarter => '3',
        LengthCode::Eighth => '4',
        LengthCode::Sixteenth => '5',
        LengthCode::ThirtySecond => '6',
        LengthCode::SixtyFourth => '7',
    }
}

/// MML `l` 뒤 분모(`1` `2` `4` `8` `16` `32` `64`)와 점의 길이. 그 밖의 값이면 `None`(S3-T4b 결정 4절).
#[must_use]
pub fn mml_length(digits: &str, dotted: bool) -> Option<StepLength> {
    let _ = (digits, dotted);
    todo!()
}

/// 옥타브 상한(과학적 음높이 표기 한 자리, REQ 5.6).
pub const OCTAVE_MAX: u8 = 9;

/// 옥타브 `octave`의 C에서 `semitone`만큼 떨어진 음높이(C4 = 60). 0–127 밖이면 `None`.
#[must_use]
pub fn pitch_of(octave: u8, semitone: i16) -> Option<Pitch> {
    let value = (i16::from(octave) + 1) * 12 + semitone;
    u8::try_from(value).ok().and_then(|v| Pitch::new(v).ok())
}

/// 반음 자리의 음이름(조합 표시). 올림표로 적는다.
#[must_use]
pub fn semitone_name(semitone: i16) -> &'static str {
    const NAMES: [&str; 12] = [
        "c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b",
    ];
    NAMES[usize::try_from(semitone.rem_euclid(12)).unwrap_or(0)]
}

/// 확정을 기다리는 음 하나(상태줄 `5c_`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Composing {
    /// 음 키를 누를 때의 현재 트랙. 확정은 이 트랙에 쓴다.
    pub(crate) track: TrackId,
    /// 옥타브의 C에서 떨어진 반음(건반형 윗줄은 12 이상, 변화표로 음수일 수 있다).
    pub(crate) semitone: i16,
    pub(crate) octave: u8,
    /// `Ctrl+음`: 전진하지 않는다.
    pub(crate) chord: bool,
    /// `Shift+음`: 절반 길이, 이 음 하나.
    pub(crate) half: bool,
    /// `n` 접두: 셋잇단, 이 음 하나.
    pub(crate) triplet: bool,
    /// 상태줄에 보일 음이름과 변화표(친 그대로: `c`, `c-`, 건반형은 음이름 `f#`).
    pub(crate) shown: String,
    /// 누를 때 들려준 음높이. 확정 때 다르면 다시 들려준다.
    pub(crate) sounded: Option<Pitch>,
    /// 대기 시간 작업과 맞추는 세대 번호.
    pub(crate) generation: u64,
}

impl Composing {
    #[must_use]
    pub(crate) fn pitch(&self) -> Option<Pitch> {
        pitch_of(self.octave, self.semitone)
    }
}

/// MML 접두의 종류(계이름 레이아웃, S3-T4b 결정 4절).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MmlKind {
    /// `o<숫자>`
    Octave,
    /// `l<숫자>[.]`
    Length,
}

/// 숫자를 기다리는 MML 접두(상태줄 `l16_`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MmlInput {
    pub(crate) kind: MmlKind,
    pub(crate) digits: String,
    pub(crate) dotted: bool,
}

impl MmlInput {
    /// 상태줄 표시(`o`, `l16.`).
    #[must_use]
    pub(crate) fn shown(&self) -> String {
        let mut s = String::from(match self.kind {
            MmlKind::Octave => "o",
            MmlKind::Length => "l",
        });
        s.push_str(&self.digits);
        if self.dotted {
            s.push('.');
        }
        s
    }
}

/// 열린 화음: 첫 화음 음의 자리와 스텝 길이, 지금까지 넣은 노트(클립 기준).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenChord {
    pub(crate) track: TrackId,
    pub(crate) clip: ClipId,
    pub(crate) at: Tick,
    pub(crate) len: u64,
    pub(crate) notes: Vec<Note>,
}

/// 직전 스텝(타이 대상): 확정한 음 또는 닫은 화음의 노트(클립 기준)와 그 스텝의 끝(절대 틱).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LastStep {
    pub(crate) track: TrackId,
    pub(crate) clip: ClipId,
    pub(crate) notes: Vec<Note>,
    pub(crate) end: Tick,
}

/// 창 하나의 Step 상태. Step을 나갔다 들어와도 길이·옥타브는 남는다.
#[derive(Clone, Debug)]
pub(crate) struct StepState {
    /// 음 키 레이아웃(설정). 그릴 때 읽어 둔다.
    pub(crate) layout: KeyLayout,
    pub(crate) length: StepLength,
    pub(crate) octave: u8,
    /// `n`을 눌렀고 아직 음·쉼표가 오지 않았다.
    pub(crate) triplet_next: bool,
    /// 마지막 확정 뒤 누른 길이 접두(`5`, `n`). 조합 표시에만 쓴다.
    pub(crate) prefix: String,
    pub(crate) composing: Option<Composing>,
    /// 숫자를 기다리는 MML 접두. 조합과 함께 있지 않다.
    pub(crate) mml: Option<MmlInput>,
    /// 덮어쓰기 Step(vim `R`): 밀지 않고 스텝 범위의 노트를 바꾼다. Step 안에서만 참이다.
    pub(crate) overwrite: bool,
    pub(crate) chord: Option<OpenChord>,
    pub(crate) last: Option<LastStep>,
    /// 지나온 스텝(시작, 끝, 절대 틱)의 자취. `Backspace`·`Shift+Tab`은 커서가 맨 위 스텝의 끝에 있으면 그
    /// 스텝의 시작으로 간다(절반·셋잇단으로 친 스텝도 그 길이만큼, 리뷰 1 지적 1). 아니면 현재 길이 한 스텝.
    pub(crate) trail: Vec<(Tick, Tick)>,
    /// 자취가 맞는 (트랙, 문서 revision). 다른 트랙이거나 Step 밖의 편집·undo로 문서가 바뀌면 자취는 낡아 버린다
    /// (리뷰 2 지적 1). Step 자신의 편집은 열쇠를 이어 간다(`rekey`).
    pub(crate) trail_key: Option<(Option<TrackId>, u64)>,
    pub(crate) generation: u64,
}

/// 자취 상한. 넘으면 오래된 것부터 버린다(그 앞은 현재 길이로 돌아간다).
const TRAIL_MAX: usize = 1024;

impl StepState {
    pub(crate) fn new(default_length: LengthCode) -> Self {
        StepState {
            layout: KeyLayout::Piano,
            length: StepLength::plain(default_length),
            octave: 4,
            triplet_next: false,
            prefix: String::new(),
            composing: None,
            mml: None,
            overwrite: false,
            chord: None,
            last: None,
            trail: Vec::new(),
            trail_key: None,
            generation: 0,
        }
    }

    /// 열쇠가 지금(`key`)과 다르면 자취를 버린다.
    fn check_key(&mut self, key: (Option<TrackId>, u64)) {
        if self.trail_key != Some(key) {
            self.trail.clear();
            self.trail_key = Some(key);
        }
    }

    /// 자취를 버린다(Step을 나갈 때).
    pub(crate) fn clear_trail(&mut self) {
        self.trail.clear();
        self.trail_key = None;
    }

    /// Step 자신이 문서를 한 번 바꿨다(revision `before` → `after`): 맞던 자취는 그대로 맞다.
    pub(crate) fn rekey(&mut self, track: Option<TrackId>, before: u64, after: u64) {
        if self.trail_key == Some((track, before)) {
            self.trail_key = Some((track, after));
        }
    }

    /// 스텝 하나를 지나왔다. `key`는 지금 (트랙, revision).
    pub(crate) fn push_step(&mut self, key: (Option<TrackId>, u64), start: Tick, end: Tick) {
        self.check_key(key);
        if self.trail.len() == TRAIL_MAX {
            self.trail.remove(0);
        }
        self.trail.push((start, end));
    }

    /// 타이로 맨 위 스텝(끝이 `end`)이 `to`까지 늘었다.
    pub(crate) fn extend_step(&mut self, key: (Option<TrackId>, u64), end: Tick, to: Tick) {
        self.check_key(key);
        match self.trail.last_mut() {
            Some(top) if top.1 == end => top.1 = to,
            _ => self.push_step(key, end, to),
        }
    }

    /// 한 스텝 뒤의 틱: 커서가 맨 위 스텝의 끝이면 그 스텝을 꺼내 시작으로, 아니면 `plain`만큼(0에서 멈춤).
    pub(crate) fn step_back_from(
        &mut self,
        key: (Option<TrackId>, u64),
        cursor: Tick,
        plain: u64,
    ) -> Tick {
        self.check_key(key);
        match self.trail.last() {
            Some(&(start, end)) if end == cursor => {
                self.trail.pop();
                start
            }
            _ => Tick::new(cursor.get().saturating_sub(plain)),
        }
    }

    /// 상태줄 모드 표시: `STEP ♩ oct4`, 조합 중이면 `STEP 1/16 oct4 5c_`, 덮어쓰기면 `STEP R ♩ oct4`, MML 대기면
    /// `STEP ♩ oct4 l16_`.
    #[must_use]
    pub(crate) fn label(&self) -> String {
        let mode = if self.overwrite { "STEP R" } else { "STEP" };
        let mut label = format!("{mode} {} oct{}", length_label(self.length), self.octave);
        if let Some(shown) = self.composition() {
            label.push(' ');
            label.push_str(&shown);
        }
        label
    }

    /// 조합 표시(`5c_`) 또는 MML 대기 표시(`l16_`).
    #[must_use]
    pub(crate) fn composition(&self) -> Option<String> {
        if let Some(c) = &self.composing {
            return Some(format!("{}{}_", self.prefix, c.shown));
        }
        self.mml.as_ref().map(|m| format!("{}_", m.shown()))
    }
}

/// 시험·접근성이 읽는 Step 상태.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepStatus {
    pub length: StepLength,
    pub octave: u8,
    /// 조합 중이면 상태줄의 조합 표시(`5c_`). MML 접두 대기도 같은 자리에 보인다(`l16_`).
    pub composing: Option<String>,
    pub chord_open: bool,
    /// 덮어쓰기 Step(vim `R`)
    pub overwrite: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_ticks_combine_dot_triplet_half_as_integers() {
        let q = StepLength::plain(LengthCode::Quarter);
        assert_eq!(step_ticks(q, false, false), 960);
        assert_eq!(step_ticks(q, true, false), 640);
        assert_eq!(step_ticks(q, false, true), 480);
        let e = StepLength::plain(LengthCode::Eighth);
        assert_eq!(step_ticks(e, true, false), 320);
        let dotted64 = StepLength {
            code: LengthCode::SixtyFourth,
            dotted: true,
        };
        assert_eq!(step_ticks(dotted64, false, false), 90);
        assert_eq!(step_ticks(dotted64, false, true), 45);
        assert_eq!(step_ticks(dotted64, true, true), 30);
        let s64 = StepLength::plain(LengthCode::SixtyFourth);
        assert_eq!(step_ticks(s64, true, true), 20);
    }

    #[test]
    fn mml_length_reads_denominators_and_dot() {
        let cases = [
            ("1", LengthCode::Whole),
            ("2", LengthCode::Half),
            ("4", LengthCode::Quarter),
            ("8", LengthCode::Eighth),
            ("16", LengthCode::Sixteenth),
            ("32", LengthCode::ThirtySecond),
            ("64", LengthCode::SixtyFourth),
        ];
        for (digits, code) in cases {
            assert_eq!(mml_length(digits, false), Some(StepLength::plain(code)));
            assert_eq!(
                mml_length(digits, true),
                Some(StepLength { code, dotted: true })
            );
        }
        for bad in ["", "0", "3", "6", "12", "128", "016", "164"] {
            assert_eq!(mml_length(bad, false), None, "{bad}");
        }
    }

    #[test]
    fn length_label_uses_note_glyphs_and_fractions() {
        assert_eq!(length_label(StepLength::plain(LengthCode::Quarter)), "♩");
        assert_eq!(length_label(StepLength::plain(LengthCode::Eighth)), "♪");
        assert_eq!(
            length_label(StepLength::plain(LengthCode::Sixteenth)),
            "1/16"
        );
        let dq = StepLength {
            code: LengthCode::Quarter,
            dotted: true,
        };
        assert_eq!(length_label(dq), "♩.");
    }

    #[test]
    fn pitch_of_uses_scientific_octaves_and_bounds() {
        assert_eq!(pitch_of(4, 0).map(Pitch::get), Some(60));
        assert_eq!(pitch_of(4, -1).map(Pitch::get), Some(59));
        assert_eq!(pitch_of(0, -13), None);
        assert_eq!(pitch_of(9, 7).map(Pitch::get), Some(127));
        assert_eq!(pitch_of(9, 8), None);
    }

    #[test]
    fn label_shows_composition_with_prefix() {
        let mut s = StepState::new(LengthCode::Quarter);
        assert_eq!(s.label(), "STEP ♩ oct4");
        s.length = StepLength::plain(LengthCode::Sixteenth);
        s.prefix = "5".to_owned();
        s.composing = Some(Composing {
            track: keyscore_core::Track::new(
                "T",
                keyscore_core::Channel::new(1).unwrap(),
                keyscore_core::TrackColor::new(0).unwrap(),
            )
            .id(),
            semitone: 0,
            shown: "c".to_owned(),
            octave: 4,
            chord: false,
            half: false,
            triplet: false,
            sounded: None,
            generation: 1,
        });
        assert_eq!(s.label(), "STEP 1/16 oct4 5c_");
    }
}
