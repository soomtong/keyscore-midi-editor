//! 선택 노트의 빠른 조작(S3-T2, REQ 5.7 `Alt`·`Alt+Shift`·`Alt+Ctrl` 행)을 `Edit`로 계산한다.
//!
//! 규칙은 S0-T7 일괄 연산과 같다(`keyscore_core::transform::map_notes`): 클립 끝을 넘으면 마디 단위로 클립을
//! 늘리고(다음 클립과 겹치면 거부), 클립 시작 앞은 거부, 꼬리 노트는 시작만 클립 안이면 된다. 음높이가
//! 0–127을 벗어나면 거부한다(옥타브 접기는 쓰지 않는다). 전부 아니면 전무다.

use keyscore_core::{ClipId, Edit, ModelError, Note, Project, TrackId};

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
        todo!("S3-T2")
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
    let _ = (project, track, clip, notes, op, step);
    todo!("S3-T2")
}
