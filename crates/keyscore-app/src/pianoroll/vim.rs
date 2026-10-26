//! vim 프리셋의 오퍼레이터·모션·텍스트 객체·비주얼 선택 계산(S3-T3c, REQ 5.8.1). GPUI와 무관한 순수 함수다.
//! 규칙은 `docs/reports/s3-t3c/plan.md` 「끝의 정의」, 결정은 `docs/decisions/2026-10-26-s3-t3c-vim-operators.md`.
//!
//! 노트는 (절대 시작 틱, 음높이) 순으로 늘어놓은 글처럼 다룬다(읽기 순서, 결정 1 A). 커서 틱은 트랙 기준(절대)
//! 틱이고 클립 노트의 시작은 클립 기준이다. 모든 함수가 그 변환을 안에서 한다.

use std::collections::BTreeSet;

use keyscore_core::{Clip, MeterMap, Note, Track};

use super::Cursor;

/// 오퍼레이터(`d` `y` `c`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operator {
    Delete,
    Yank,
    Change,
}

/// 오퍼레이터와 결합하는 커서 이동(계획 4절 표).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    Left,
    Right,
    PrevBar,
    NextBar,
    NextNote,
    PrevNote,
    NextBeat,
    PrevBeat,
    NoteEnd,
    LastNoteEndInBar,
    NextSamePitch,
    PrevSamePitch,
    NextClip,
    PrevClip,
    ClipStart,
    ClipEnd,
    Up,
    Down,
    OctaveUp,
    OctaveDown,
}

/// 모션 범위의 끝 처리.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reach {
    /// [앞 자리, 뒤 자리)
    Exclusive,
    /// [앞 자리, 뒤 자리]
    Inclusive,
    /// 클립 처음부터 커서 자리까지(포함)
    FromClipStart,
    /// 커서 자리부터 클립 끝까지
    ToClipEnd,
    /// 음높이 모션: 오퍼레이터와 결합하지 않는다
    Pitch,
}

/// 텍스트 객체(`i n` `i b` `a c`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextObject {
    InnerNote,
    InnerBar,
    AroundClip,
}

/// 비주얼 선택의 종류(`v` `V` `Ctrl+v`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VisualKind {
    /// 읽기 순서(vim charwise)
    Note,
    /// 시간 범위의 모든 음높이(vim linewise)
    Time,
    /// 음높이 × 시간 직사각형
    Block,
}

impl Motion {
    /// 범위의 끝 처리.
    #[must_use]
    pub fn reach(self) -> Reach {
        todo!()
    }

    /// 카운트를 쓰는가(`g g` `G`는 받되 쓰지 않는다).
    #[must_use]
    pub fn counts(self) -> bool {
        todo!()
    }
}

/// 모션의 목적지 한 번. 없으면 `None`.
#[must_use]
pub fn motion_target(
    track: &Track,
    meter: &MeterMap,
    step: u64,
    from: Cursor,
    motion: Motion,
) -> Option<Cursor> {
    let _ = (track, meter, step, from, motion);
    todo!()
}

/// 칸의 읽기 순서 자리: 칸 위 노트가 있으면 그 노트의 (절대 시작, 음높이), 없으면 칸 자체.
#[must_use]
pub fn reading_key(clip: &Clip, cell: Cursor) -> (u64, u8) {
    let _ = (clip, cell);
    todo!()
}

/// 커서 칸 `from`에서 목적지 `to`까지의 모션 범위에 드는 `clip`의 노트.
#[must_use]
pub fn motion_notes(clip: &Clip, from: Cursor, to: Cursor, reach: Reach) -> BTreeSet<Note> {
    let _ = (clip, from, to, reach);
    todo!()
}

/// 텍스트 객체의 노트.
#[must_use]
pub fn object_notes(
    clip: &Clip,
    meter: &MeterMap,
    cursor: Cursor,
    object: TextObject,
) -> BTreeSet<Note> {
    let _ = (clip, meter, cursor, object);
    todo!()
}

/// 비주얼 선택의 노트.
#[must_use]
pub fn visual_notes(
    clip: &Clip,
    anchor: Cursor,
    cursor: Cursor,
    step: u64,
    kind: VisualKind,
) -> BTreeSet<Note> {
    let _ = (clip, anchor, cursor, step, kind);
    todo!()
}
