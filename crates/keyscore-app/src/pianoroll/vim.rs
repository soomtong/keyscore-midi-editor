//! vim 프리셋의 오퍼레이터·모션·텍스트 객체·비주얼 선택 계산(S3-T3c, REQ 5.8.1). GPUI와 무관한 순수 함수다.
//! 규칙은 `docs/reports/s3-t3c/plan.md` 「끝의 정의」, 결정은 `docs/decisions/2026-10-26-s3-t3c-vim-operators.md`.
//!
//! 노트는 (절대 시작 틱, 음높이) 순으로 늘어놓은 글처럼 다룬다(읽기 순서, 결정 1 A). 커서 틱은 트랙 기준(절대)
//! 틱이고 클립 노트의 시작은 클립 기준이다. 모든 함수가 그 변환을 안에서 한다.

use std::collections::BTreeSet;

use keyscore_core::{Clip, MeterMap, Note, Pitch, Tick, Track};

use super::select::{self as sel, bar_notes, note_under, region_notes};
use super::{Cursor, CursorMove, moved};

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
    /// 범위의 끝 처리(계획 4절 표).
    #[must_use]
    pub fn reach(self) -> Reach {
        match self {
            Motion::Left
            | Motion::Right
            | Motion::PrevBar
            | Motion::NextBar
            | Motion::NextNote
            | Motion::PrevNote
            | Motion::NextBeat
            | Motion::PrevBeat
            | Motion::NextClip
            | Motion::PrevClip => Reach::Exclusive,
            Motion::NoteEnd
            | Motion::LastNoteEndInBar
            | Motion::NextSamePitch
            | Motion::PrevSamePitch => Reach::Inclusive,
            Motion::ClipStart => Reach::FromClipStart,
            Motion::ClipEnd => Reach::ToClipEnd,
            Motion::Up | Motion::Down | Motion::OctaveUp | Motion::OctaveDown => Reach::Pitch,
        }
    }

    /// 문서 없이 커서만으로 정해지는 이동(S3-T1의 스텝·반음·마디·옥타브).
    #[must_use]
    pub fn as_cursor_move(self) -> Option<CursorMove> {
        Some(match self {
            Motion::Left => CursorMove::StepBack,
            Motion::Right => CursorMove::StepForward,
            Motion::Up => CursorMove::SemitoneUp,
            Motion::Down => CursorMove::SemitoneDown,
            Motion::OctaveUp => CursorMove::OctaveUp,
            Motion::OctaveDown => CursorMove::OctaveDown,
            Motion::PrevBar => CursorMove::BarBack,
            Motion::NextBar => CursorMove::BarForward,
            _ => return None,
        })
    }

    /// 카운트를 쓰는가(`g g` `G`는 받되 쓰지 않는다).
    #[must_use]
    pub fn counts(self) -> bool {
        !matches!(self, Motion::ClipStart | Motion::ClipEnd)
    }
}

/// `tick`을 품은 트랙의 클립.
fn clip_at(track: &Track, tick: Tick) -> Option<&Clip> {
    track
        .clips()
        .map(|(_, c)| c)
        .find(|c| c.start() <= tick && tick.get() < c.end())
}

/// 노트의 마지막 칸(절대 틱): 시작 + ⌊(길이 − 1) / 스텝⌋ × 스텝. 클립의 마지막 스텝 칸과 같은 셈이다.
fn last_cell(clip: &Clip, note: &Note, step: u64) -> Cursor {
    let step = step.max(1);
    let cells = u64::from(note.length()).saturating_sub(1) / step;
    Cursor {
        tick: Tick::new(
            clip.start()
                .get()
                .saturating_add(note.start().get())
                .saturating_add(cells.saturating_mul(step)),
        ),
        pitch: note.pitch(),
    }
}

/// 칸 `at`에서 시작하는 클립의 노트(같은 자리에 여럿이면 첫째).
fn note_starting_at(clip: &Clip, at: Cursor) -> Option<&Note> {
    let rel = at.tick.get().checked_sub(clip.start().get())?;
    clip.notes()
        .iter()
        .find(|n| n.start().get() == rel && n.pitch() == at.pitch)
}

fn key(c: Cursor) -> (u64, u8) {
    (c.tick.get(), c.pitch.get())
}

fn note_key(clip: &Clip, note: &Note) -> (u64, u8) {
    (
        clip.start().get().saturating_add(note.start().get()),
        note.pitch().get(),
    )
}

/// 박 시작(절대 틱): 박자표 변화 지점부터 센 박 격자.
fn beat_start(meter: &MeterMap, at: Tick) -> u64 {
    at.get() - meter.position(at).tick()
}

/// `e`: 커서 노트의 마지막 칸. 커서 노트가 없거나 이미 그 칸 이후면 읽기 순서 다음 노트의 마지막 칸.
fn note_end(track: &Track, step: u64, from: Cursor) -> Option<Cursor> {
    if let Some(clip) = clip_at(track, from.tick)
        && let Some(n) = note_under(clip, from)
    {
        let end = last_cell(clip, &n, step);
        if end.tick > from.tick {
            return Some(end);
        }
    }
    let (clip_id, at) = sel::next_note(track, from)?;
    let clip = track.clip(clip_id)?;
    let n = note_starting_at(clip, at)?;
    Some(last_cell(clip, n, step))
}

/// `E`: 커서 마디에서 시작하는 현재 클립 노트 가운데 읽기 순서 마지막 노트의 마지막 칸. 그 칸이 커서 칸보다
/// 뒤가 아니면 다음 마디들에서(현재 클립 끝까지) 같은 것을 찾는다.
fn last_note_end_in_bar(
    track: &Track,
    meter: &MeterMap,
    step: u64,
    from: Cursor,
) -> Option<Cursor> {
    let clip = clip_at(track, from.tick)?;
    let mut bar = meter.position(from.tick).bar();
    loop {
        let lo = meter.bar_start(bar)?.get();
        if lo >= clip.end() {
            return None;
        }
        let hi = meter.bar_start(bar + 1).map_or(u64::MAX, Tick::get);
        let last = clip
            .notes()
            .iter()
            .filter(|n| (lo..hi).contains(&note_key(clip, n).0))
            .max_by_key(|n| note_key(clip, n));
        if let Some(n) = last {
            let end = last_cell(clip, n, step);
            if key(end) > key(from) {
                return Some(end);
            }
        }
        bar += 1;
    }
}

/// 모션의 목적지 한 번. 없으면 `None`. 더 갈 수 없는 이동(틱 0에서 `h`)은 같은 칸을 준다.
#[must_use]
pub fn motion_target(
    track: &Track,
    meter: &MeterMap,
    step: u64,
    from: Cursor,
    motion: Motion,
) -> Option<Cursor> {
    if let Some(delta) = motion.as_cursor_move() {
        return Some(moved(from, delta, step, meter));
    }
    let at_tick = |tick: Tick| Cursor {
        tick,
        pitch: from.pitch,
    };
    match motion {
        Motion::Left
        | Motion::Right
        | Motion::Up
        | Motion::Down
        | Motion::OctaveUp
        | Motion::OctaveDown
        | Motion::PrevBar
        | Motion::NextBar => None,
        Motion::NextNote => sel::next_note(track, from).map(|(_, c)| c),
        Motion::PrevNote => sel::prev_note(track, from).map(|(_, c)| c),
        Motion::NextBeat => {
            let next = beat_start(meter, from.tick)
                .saturating_add(meter.signature_at(from.tick).beat_ticks());
            // 박자표가 바뀌는 틱에서 새 박이 시작한다
            let next = meter
                .next_change_after(from.tick)
                .map_or(next, |change| next.min(change.get()));
            Some(at_tick(Tick::new(next)))
        }
        Motion::PrevBeat => {
            let start = beat_start(meter, from.tick);
            let target = if start < from.tick.get() {
                start
            } else {
                beat_start(meter, Tick::new(from.tick.get().checked_sub(1)?))
            };
            Some(at_tick(Tick::new(target)))
        }
        Motion::NoteEnd => note_end(track, step, from),
        Motion::LastNoteEndInBar => last_note_end_in_bar(track, meter, step, from),
        Motion::NextSamePitch => sel::next_same_pitch(track, from),
        Motion::PrevSamePitch => sel::prev_same_pitch(track, from),
        Motion::NextClip => sel::next_clip_start(track, from.tick).map(at_tick),
        Motion::PrevClip => sel::prev_clip_start(track, from.tick).map(at_tick),
        Motion::ClipStart => clip_at(track, from.tick).map(|c| at_tick(c.start())),
        Motion::ClipEnd => clip_at(track, from.tick).map(|c| at_tick(sel::clip_last_step(c, step))),
    }
}

/// 칸의 읽기 순서 자리: 칸 위 노트가 있으면 그 노트의 (절대 시작, 음높이), 없으면 칸 자체.
#[must_use]
pub fn reading_key(clip: &Clip, cell: Cursor) -> (u64, u8) {
    note_under(clip, cell).map_or(key(cell), |n| note_key(clip, &n))
}

/// 읽기 순서 키가 `keep`을 만족하는 `clip`의 노트.
fn notes_by_key(clip: &Clip, keep: impl Fn((u64, u8)) -> bool) -> BTreeSet<Note> {
    clip.notes()
        .iter()
        .filter(|n| keep(note_key(clip, n)))
        .cloned()
        .collect()
}

/// 커서 칸 `from`에서 목적지 `to`까지의 모션 범위에 드는 `clip`의 노트. 두 끝은 읽기 순서 자리다.
#[must_use]
pub fn motion_notes(clip: &Clip, from: Cursor, to: Cursor, reach: Reach) -> BTreeSet<Note> {
    let a = reading_key(clip, from);
    let b = reading_key(clip, to);
    let (lo, hi) = (a.min(b), a.max(b));
    match reach {
        Reach::Exclusive => notes_by_key(clip, |k| lo <= k && k < hi),
        Reach::Inclusive => notes_by_key(clip, |k| lo <= k && k <= hi),
        Reach::FromClipStart => notes_by_key(clip, |k| k <= a),
        Reach::ToClipEnd => notes_by_key(clip, |k| k >= a),
        Reach::Pitch => BTreeSet::new(),
    }
}

/// 텍스트 객체의 노트(결정 2 A: 노트만).
#[must_use]
pub fn object_notes(
    clip: &Clip,
    meter: &MeterMap,
    cursor: Cursor,
    object: TextObject,
) -> BTreeSet<Note> {
    match object {
        TextObject::InnerNote => note_under(clip, cursor).into_iter().collect(),
        TextObject::InnerBar => bar_notes(clip, meter, cursor.tick),
        TextObject::AroundClip => clip.notes().iter().cloned().collect(),
    }
}

/// 비주얼 선택의 노트(결정 3 A).
#[must_use]
pub fn visual_notes(
    clip: &Clip,
    anchor: Cursor,
    cursor: Cursor,
    step: u64,
    kind: VisualKind,
) -> BTreeSet<Note> {
    match kind {
        VisualKind::Note => motion_notes(clip, anchor, cursor, Reach::Inclusive),
        VisualKind::Time => region_notes(
            clip,
            Cursor {
                pitch: Pitch::MIN,
                ..anchor
            },
            Cursor {
                pitch: Pitch::MAX,
                ..cursor
            },
            step,
        ),
        VisualKind::Block => region_notes(clip, anchor, cursor, step),
    }
}
