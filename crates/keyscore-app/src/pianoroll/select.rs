//! 선택과 노트 단위 이동의 순수 계산(S3-T2, REQ 5.7). GPUI와 무관하다.
//!
//! 커서 틱은 트랙 기준(절대) 틱이고 클립 노트의 시작은 클립 기준이다. 모든 함수가 그 변환을 안에서 한다.

use std::collections::BTreeSet;

use keyscore_core::{Clip, ClipId, MeterMap, Note, Tick, Track};

use super::Cursor;

/// `a`와 `b` 칸을 모두 포함하는 직사각형(시간 `[min, max + step)`, 음높이 양끝 포함)에 걸치는 `clip`의 노트.
/// 「걸친다」는 노트 구간 `[start, end)`가 시간 범위와 겹치는 것이다(7.4 러버밴드와 같은 기준).
#[must_use]
pub fn region_notes(clip: &Clip, a: Cursor, b: Cursor, step: u64) -> BTreeSet<Note> {
    let _ = (clip, a, b, step);
    todo!("S3-T2")
}

/// `at`이 든 마디 `[마디 시작, 다음 마디 시작)`에 걸치는 `clip`의 노트.
#[must_use]
pub fn bar_notes(clip: &Clip, meter: &MeterMap, at: Tick) -> BTreeSet<Note> {
    let _ = (clip, meter, at);
    todo!("S3-T2")
}

/// 커서 위의 노트: 커서 틱을 품고(`start <= tick < end`) 음높이가 같은 노트. 여럿이면 가장 이른 것.
#[must_use]
pub fn note_under(clip: &Clip, cursor: Cursor) -> Option<Note> {
    let _ = (clip, cursor);
    todo!("S3-T2")
}

/// 트랙의 모든 클립 노트를 (절대 시작, 음높이) 순으로 놓았을 때 커서 칸 다음 노트의 (클립, 칸).
#[must_use]
pub fn next_note(track: &Track, cursor: Cursor) -> Option<(ClipId, Cursor)> {
    let _ = (track, cursor);
    todo!("S3-T2")
}

/// [`next_note`]의 반대.
#[must_use]
pub fn prev_note(track: &Track, cursor: Cursor) -> Option<(ClipId, Cursor)> {
    let _ = (track, cursor);
    todo!("S3-T2")
}

/// 커서 음높이의 다음 노트(시작이 커서 틱보다 뒤). vim `f`.
#[must_use]
pub fn next_same_pitch(track: &Track, cursor: Cursor) -> Option<Cursor> {
    let _ = (track, cursor);
    todo!("S3-T2")
}

/// 커서 음높이의 이전 노트(시작이 커서 틱보다 앞). vim `F`.
#[must_use]
pub fn prev_same_pitch(track: &Track, cursor: Cursor) -> Option<Cursor> {
    let _ = (track, cursor);
    todo!("S3-T2")
}

/// `tick` 뒤에서 시작하는 첫 클립의 시작.
#[must_use]
pub fn next_clip_start(track: &Track, tick: Tick) -> Option<Tick> {
    let _ = (track, tick);
    todo!("S3-T2")
}

/// `tick` 앞에서 시작하는 마지막 클립의 시작.
#[must_use]
pub fn prev_clip_start(track: &Track, tick: Tick) -> Option<Tick> {
    let _ = (track, tick);
    todo!("S3-T2")
}

/// 클립의 마지막 스텝 칸(절대 틱): `start + floor((len − 1) / step) × step`.
#[must_use]
pub fn clip_last_step(clip: &Clip, step: u64) -> Tick {
    let _ = (clip, step);
    todo!("S3-T2")
}
