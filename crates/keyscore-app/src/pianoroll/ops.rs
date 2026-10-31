//! 선택 노트의 빠른 조작(S3-T2, REQ 5.7 `Alt`·`Alt+Shift`·`Alt+Ctrl` 행)을 `Edit`로 계산한다.
//!
//! 규칙은 S0-T7 일괄 연산과 같다(`keyscore_core::transform::map_notes`): 클립 끝을 넘으면 마디 단위로 클립을
//! 늘리고(다음 클립과 겹치면 거부), 클립 시작 앞은 거부, 꼬리 노트는 시작만 클립 안이면 된다. 음높이가
//! 0–127을 벗어나면 거부한다(옥타브 접기는 쓰지 않는다). 전부 아니면 전무다.

use keyscore_core::transform::{NoteRef, map_notes};
use keyscore_core::{ClipId, Edit, ModelError, Note, Pitch, Project, Tick, TrackId};

/// 5.7 표의 `Alt` 조작.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteOp {
    /// 한 스텝 앞당김(`Alt+h`)
    NudgeEarlier,
    /// 한 스텝 미룸(`Alt+l`)
    NudgeLater,
    /// 한 마디 앞당김(`Alt+Ctrl+h`)
    NudgeBarEarlier,
    /// 한 마디 미룸(`Alt+Ctrl+l`)
    NudgeBarLater,
    /// 반음 올림(`Alt+k`, `+`)
    TransposeUp,
    /// 반음 내림(`Alt+j`, `-`)
    TransposeDown,
    /// 한 옥타브 올림(`Alt+Shift+k`)
    OctaveUp,
    /// 한 옥타브 내림(`Alt+Shift+j`)
    OctaveDown,
    /// 끝을 한 스텝 늘림(`Alt+Shift+l`, `>`)
    Lengthen,
    /// 끝을 한 스텝 줄임(`Alt+Shift+h`, `<`)
    Shorten,
}

/// 되돌리기 묶음 키의 상위 비트. 다른 묶음 키(S3-T3 이후)와 겹치지 않게 조작 종류마다 자리를 둔다.
const COALESCE_BASE: u64 = 0x5337_0000;

impl NoteOp {
    pub const ALL: [NoteOp; 10] = [
        NoteOp::NudgeEarlier,
        NoteOp::NudgeLater,
        NoteOp::NudgeBarEarlier,
        NoteOp::NudgeBarLater,
        NoteOp::TransposeUp,
        NoteOp::TransposeDown,
        NoteOp::OctaveUp,
        NoteOp::OctaveDown,
        NoteOp::Lengthen,
        NoteOp::Shorten,
    ];

    /// 되돌리기 묶음 키(5.7 「같은 조작의 연속은 한 단위」). 조작마다 다르다.
    #[must_use]
    pub fn coalesce_key(self) -> u64 {
        let index = NoteOp::ALL
            .iter()
            .position(|op| *op == self)
            .unwrap_or_default();
        COALESCE_BASE + index as u64
    }

    /// 상태줄 문구에 쓰는 이름.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            NoteOp::NudgeEarlier => "한 스텝 앞당김",
            NoteOp::NudgeLater => "한 스텝 미룸",
            NoteOp::NudgeBarEarlier => "한 마디 앞당김",
            NoteOp::NudgeBarLater => "한 마디 미룸",
            NoteOp::TransposeUp => "반음 올림",
            NoteOp::TransposeDown => "반음 내림",
            NoteOp::OctaveUp => "옥타브 올림",
            NoteOp::OctaveDown => "옥타브 내림",
            NoteOp::Lengthen => "늘림",
            NoteOp::Shorten => "줄임",
        }
    }

    /// 이 조작의 일반 변환. `step`은 한 스텝의 틱(0이면 1).
    #[must_use]
    pub fn shift(self, step: u64) -> NoteShift {
        let _ = step;
        todo!("S3-T6b")
    }

    /// 시간을 옮기는 조작인가(커서 노트에 적용하면 커서 틱이 따라간다).
    #[must_use]
    pub const fn moves_time(self) -> bool {
        matches!(
            self,
            NoteOp::NudgeEarlier
                | NoteOp::NudgeLater
                | NoteOp::NudgeBarEarlier
                | NoteOp::NudgeBarLater
        )
    }

    /// 음높이를 바꾸는 조작인가(커서 노트에 적용하면 커서 음높이가 따라간다).
    #[must_use]
    pub const fn moves_pitch(self) -> bool {
        matches!(
            self,
            NoteOp::TransposeUp | NoteOp::TransposeDown | NoteOp::OctaveUp | NoteOp::OctaveDown
        )
    }
}

/// 일반 변환의 시간 이동. 마디 단위는 노트마다 그 자리의 마디 길이라 틱 수 하나로 적을 수 없다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeShift {
    /// 틱 수(음수면 앞당김).
    Ticks(i64),
    /// 한 마디 앞당김: 시작 직전 틱이 든 마디의 길이만큼(`Alt+Ctrl+h`).
    BarEarlier,
    /// 한 마디 미룸: 시작이 든 마디의 길이만큼(`Alt+Ctrl+l`).
    BarLater,
}

/// 노트의 일반 변환(결정 `2026-10-31-s3-t6-mouse-editing.md` 6절). 5.7 `Alt` 조작과 마우스 드래그가 함께 쓴다.
/// 적용 순서는 시간 → 시작(끝 고정) → 끝(시작 고정) → 음높이이고, 결과 길이가 1틱 미만이면 그 노트는 그대로다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteShift {
    /// 시작·끝을 함께 옮긴다.
    pub time: TimeShift,
    /// 반음 수.
    pub pitch: i32,
    /// 시작만 옮긴다(끝 고정).
    pub start: i64,
    /// 끝만 옮긴다(시작 고정).
    pub end: i64,
}

impl NoteShift {
    /// 아무것도 바꾸지 않는 변환.
    pub const IDENTITY: NoteShift = NoteShift {
        time: TimeShift::Ticks(0),
        pitch: 0,
        start: 0,
        end: 0,
    };

    /// 아무것도 바꾸지 않는가.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        *self == NoteShift::IDENTITY
    }
}

/// 계산 실패.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpError {
    #[error("{0}")]
    Model(#[from] ModelError),
    #[error("음높이가 0–127을 벗어난다")]
    PitchOutOfRange,
}

/// 계산 결과: 적용할 편집과 (원래, 바뀐) 노트 짝.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub edits: Vec<Edit>,
    pub moved: Vec<(Note, Note)>,
}

/// `abs`(절대 틱)가 든 마디의 길이. 박자표 변화가 마디 중간에 있으면 그 짧은 마디의 길이다.
fn bar_length_at(project: &Project, abs: u64) -> u64 {
    let meter = project.meter_map();
    let bar = meter.position(Tick::new(abs)).bar();
    let start = meter.bar_start(bar).map_or(0, Tick::get);
    let next = meter.bar_start(bar + 1).map_or(u64::MAX, Tick::get);
    next.saturating_sub(start).max(1)
}

/// `notes`(한 클립의 노트)에 `op`를 적용하는 편집. 바뀌는 노트가 없으면 `Ok(None)`.
///
/// # Errors
///
/// 노트가 클립에 없거나, 클립 시작 앞으로 나가거나, 클립을 늘려야 하는데 다음 클립과 겹치거나, 음높이가
/// 범위를 벗어나면 `Err`. 하나라도 실패하면 전체가 실패다.
pub fn plan_note_op(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
    op: NoteOp,
    step: u64,
) -> Result<Option<Planned>, OpError> {
    let step = step.max(1);
    let clip_ref = project
        .track(track)
        .ok_or(ModelError::UnknownTrack)?
        .clip(clip)
        .ok_or(ModelError::UnknownClip)?;
    let clip_start = clip_ref.start().get();
    // 바뀌지 않는 노트(한 스텝 이하를 줄이기)는 뺀다. 계산 전에 걸러야 「변화 없음」을 알 수 있다.
    let targets: Vec<&Note> = notes
        .iter()
        .filter(|n| op != NoteOp::Shorten || u64::from(n.length()) > step)
        .collect();
    if targets.is_empty() {
        return Ok(None);
    }
    // 음높이는 전부 먼저 검사한다(하나라도 밖이면 전무).
    let semitones: i32 = match op {
        NoteOp::TransposeUp => 1,
        NoteOp::TransposeDown => -1,
        NoteOp::OctaveUp => 12,
        NoteOp::OctaveDown => -12,
        _ => 0,
    };
    if semitones != 0
        && targets
            .iter()
            .any(|n| !(0..=127).contains(&(i32::from(n.pitch().get()) + semitones)))
    {
        return Err(OpError::PitchOutOfRange);
    }
    let refs: Vec<NoteRef> = targets
        .iter()
        .map(|n| NoteRef {
            track,
            clip,
            note: (*n).clone(),
        })
        .collect();
    let mut moved = Vec::with_capacity(refs.len());
    let edits = map_notes(project, &refs, |_, _, note| {
        let abs = clip_start.saturating_add(note.start().get());
        let new = match op {
            NoteOp::NudgeEarlier => shift_start(note, abs, clip_start, -i128::from(step))?,
            NoteOp::NudgeLater => shift_start(note, abs, clip_start, i128::from(step))?,
            NoteOp::NudgeBarEarlier => {
                // 앞 마디로: 시작 직전 틱이 든 마디의 길이만큼
                let len = bar_length_at(project, abs.saturating_sub(1));
                shift_start(note, abs, clip_start, -i128::from(len))?
            }
            NoteOp::NudgeBarLater => {
                let len = bar_length_at(project, abs);
                shift_start(note, abs, clip_start, i128::from(len))?
            }
            NoteOp::TransposeUp | NoteOp::TransposeDown | NoteOp::OctaveUp | NoteOp::OctaveDown => {
                let p = u8::try_from(i32::from(note.pitch().get()) + semitones)
                    .map_err(|_| ModelError::PitchOutOfRange(u8::MAX))?;
                note.with_pitch(Pitch::new(p)?)?
            }
            NoteOp::Lengthen => {
                let len = u64::from(note.length())
                    .checked_add(step)
                    .and_then(|l| u32::try_from(l).ok())
                    .ok_or(ModelError::TickOverflow)?;
                note.with_length(len)?
            }
            NoteOp::Shorten => {
                let len = u32::try_from(u64::from(note.length()) - step)
                    .map_err(|_| ModelError::TickOverflow)?;
                note.with_length(len)?
            }
        };
        moved.push((note.clone(), new.clone()));
        Ok(new)
    })?;
    Ok(Some(Planned { edits, moved }))
}

/// 절대 시작 `abs`를 `delta`만큼 옮긴 노트. 클립 시작 앞이면 거부.
fn shift_start(note: &Note, abs: u64, clip_start: u64, delta: i128) -> Result<Note, ModelError> {
    let rel = i128::from(abs) + delta - i128::from(clip_start);
    if rel < 0 {
        return Err(ModelError::NoteBeforeClipStart);
    }
    let rel = u64::try_from(rel).map_err(|_| ModelError::TickOverflow)?;
    note.with_start(Tick::new(rel))
}

/// `notes`(한 클립의 노트)에 일반 변환 `shift`를 적용하는 편집(5.7 `Alt` 조작, 마우스 드래그). 바뀌는 노트가
/// 없으면 `Ok(None)`.
///
/// # Errors
///
/// [`plan_note_op`]와 같다.
pub fn plan_shift(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
    shift: NoteShift,
) -> Result<Option<Planned>, OpError> {
    let _ = (project, track, clip, notes, shift);
    todo!("S3-T6b")
}

/// `notes`를 그대로 두고 `shift`를 적용한 사본을 넣는 편집(`Alt+드래그` 복제). 사본은 붙여넣기·`Cmd+D`와 같은
/// `insert_notes` 규칙으로 넣는다. `moved`는 (원래, 사본) 짝이다. 바뀌는 노트가 없으면 `Ok(None)`.
///
/// # Errors
///
/// 사본이 클립 시작 앞이거나 음높이가 범위를 벗어나거나 다음 클립과 겹치면 `Err`.
pub fn plan_shift_copy(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
    shift: NoteShift,
) -> Result<Option<Planned>, OpError> {
    let _ = (project, track, clip, notes, shift);
    todo!("S3-T6b")
}
