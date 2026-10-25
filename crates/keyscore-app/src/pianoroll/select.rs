//! 선택과 노트 단위 이동의 순수 계산(S3-T2, REQ 5.7). GPUI와 무관하다.
//!
//! 커서 틱은 트랙 기준(절대) 틱이고 클립 노트의 시작은 클립 기준이다. 모든 함수가 그 변환을 안에서 한다.

use std::collections::BTreeSet;

use keyscore_core::{Clip, ClipId, MeterMap, Note, Pitch, Tick, Track};

use super::Cursor;

/// `[lo, hi)` 절대 틱 범위와 `[low, high]` 음높이 범위에 걸치는 `clip`의 노트.
fn notes_in(clip: &Clip, lo: u64, hi: u64, low: Pitch, high: Pitch) -> BTreeSet<Note> {
    let start = clip.start().get();
    // 클립 기준으로 바꾼다. 범위가 클립 앞에서 끝나면 빈 집합이다.
    let (lo, hi) = (lo.saturating_sub(start), hi.saturating_sub(start));
    if hi <= lo {
        return BTreeSet::new();
    }
    // 노트는 시작 순이라 시작이 `hi` 뒤면 그 뒤는 모두 밖이다(걸침은 끝으로 보므로 앞쪽은 전부 본다).
    clip.notes()
        .iter()
        .take_while(|n| n.start().get() < hi)
        .filter(|n| n.end() > lo && (low..=high).contains(&n.pitch()))
        .cloned()
        .collect()
}

/// `a`와 `b` 칸을 모두 포함하는 직사각형(시간 `[min, max + step)`, 음높이 양끝 포함)에 걸치는 `clip`의 노트.
/// 「걸친다」는 노트 구간 `[start, end)`가 시간 범위와 겹치는 것이다(7.4 러버밴드와 같은 기준).
#[must_use]
pub fn region_notes(clip: &Clip, a: Cursor, b: Cursor, step: u64) -> BTreeSet<Note> {
    let lo = a.tick.min(b.tick).get();
    let hi = a.tick.max(b.tick).get().saturating_add(step.max(1));
    notes_in(clip, lo, hi, a.pitch.min(b.pitch), a.pitch.max(b.pitch))
}

/// `at`이 든 마디 `[마디 시작, 다음 마디 시작)`에 걸치는 `clip`의 노트.
#[must_use]
pub fn bar_notes(clip: &Clip, meter: &MeterMap, at: Tick) -> BTreeSet<Note> {
    let bar = meter.position(at).bar();
    let lo = meter.bar_start(bar).unwrap_or(Tick::ZERO).get();
    let hi = meter.bar_start(bar + 1).map_or(u64::MAX, Tick::get);
    notes_in(clip, lo, hi, Pitch::MIN, Pitch::MAX)
}

/// 커서 위의 노트: 커서 틱을 품고(`start <= tick < end`) 음높이가 같은 노트. 여럿이면 가장 이른 것.
#[must_use]
pub fn note_under(clip: &Clip, cursor: Cursor) -> Option<Note> {
    let tick = cursor.tick.get().checked_sub(clip.start().get())?;
    clip.notes()
        .iter()
        .take_while(|n| n.start().get() <= tick)
        .find(|n| n.end() > tick && n.pitch() == cursor.pitch)
        .cloned()
}

/// 트랙의 노트를 (절대 시작, 음높이) 순으로 훑는다. 클립은 시작 순이고 겹치지 않으므로 클립 순서대로
/// 이어 붙이면 절대 시작 순이다.
fn walk(track: &Track) -> impl DoubleEndedIterator<Item = (ClipId, Cursor)> + '_ {
    // `Track::clips`는 양방향 반복자가 아니라 클립 참조를 한 번 모은다(클립 수는 적다).
    let clips: Vec<(ClipId, &Clip)> = track.clips().collect();
    clips.into_iter().flat_map(|(id, clip)| {
        let base = clip.start().get();
        clip.notes().iter().map(move |n| {
            (
                id,
                Cursor {
                    tick: Tick::new(base.saturating_add(n.start().get())),
                    pitch: n.pitch(),
                },
            )
        })
    })
}

fn key(c: Cursor) -> (u64, u8) {
    (c.tick.get(), c.pitch.get())
}

/// 트랙의 모든 클립 노트를 (절대 시작, 음높이) 순으로 놓았을 때 커서 칸 다음 노트의 (클립, 칸).
#[must_use]
pub fn next_note(track: &Track, cursor: Cursor) -> Option<(ClipId, Cursor)> {
    walk(track).find(|(_, c)| key(*c) > key(cursor))
}

/// [`next_note`]의 반대.
#[must_use]
pub fn prev_note(track: &Track, cursor: Cursor) -> Option<(ClipId, Cursor)> {
    walk(track).rev().find(|(_, c)| key(*c) < key(cursor))
}

/// 커서 음높이의 다음 노트(시작이 커서 틱보다 뒤). vim `f`.
#[must_use]
pub fn next_same_pitch(track: &Track, cursor: Cursor) -> Option<Cursor> {
    walk(track)
        .map(|(_, c)| c)
        .find(|c| c.pitch == cursor.pitch && c.tick > cursor.tick)
}

/// 커서 음높이의 이전 노트(시작이 커서 틱보다 앞). vim `F`.
#[must_use]
pub fn prev_same_pitch(track: &Track, cursor: Cursor) -> Option<Cursor> {
    walk(track)
        .rev()
        .map(|(_, c)| c)
        .find(|c| c.pitch == cursor.pitch && c.tick < cursor.tick)
}

/// `tick` 뒤에서 시작하는 첫 클립의 시작.
#[must_use]
pub fn next_clip_start(track: &Track, tick: Tick) -> Option<Tick> {
    track
        .clips()
        .map(|(_, c)| c.start())
        .find(|start| *start > tick)
}

/// `tick` 앞에서 끝나는 마지막 클립의 시작(REQ 5.8 「이전 클립으로」). 커서가 클립 중간이면 그 클립이 아니라
/// 앞 클립이다(리뷰 1 제안 7). 커서가 클립 시작에 있으면 그 클립의 끝은 `tick`보다 뒤라 역시 앞 클립이다.
#[must_use]
pub fn prev_clip_start(track: &Track, tick: Tick) -> Option<Tick> {
    track
        .clips()
        .filter(|(_, c)| c.end() <= tick.get())
        .map(|(_, c)| c.start())
        .last()
}

/// 클립의 마지막 스텝 칸(절대 틱): `start + floor((len − 1) / step) × step`.
#[must_use]
pub fn clip_last_step(clip: &Clip, step: u64) -> Tick {
    let step = step.max(1);
    let cells = clip.length().saturating_sub(1) / step;
    Tick::new(
        clip.start()
            .get()
            .saturating_add(cells.saturating_mul(step)),
    )
}
