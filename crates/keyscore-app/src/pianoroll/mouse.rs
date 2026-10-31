//! 피아노롤 마우스 선택의 순수 계산(S3-T6a, REQ 7.4). GPUI와 무관하다. 규칙은
//! `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 3절.

use std::collections::BTreeSet;

use keyscore_core::{Clip, ClipId, MeterMap, Note, Pitch, Tick, Track, TrackId};

use super::scene::Rect;
use super::select::notes_in;
use super::{Cursor, ROWS, Selection, Viewport, row_u8, tick_u};

/// 끌기로 보는 최소 이동(px). macOS 끌기 문턱과 같은 크기다.
pub const DRAG_THRESHOLD: f32 = 3.0;

/// 러버밴드가 덮는 범위. 틱은 절대 `[lo, hi)`, 음높이는 양끝 포함.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub lo: Tick,
    pub hi: Tick,
    pub low: Pitch,
    pub high: Pitch,
}

/// 내용 위의 한 점: 절대 틱(소수)과 행(소수, 위가 0). 러버밴드의 누른 점은 이렇게 들어 스크롤·줌 뒤에도
/// 같은 내용에 붙어 있다(리뷰 1 지적 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub tick: f64,
    pub row: f32,
}

/// 뷰 기준 점의 내용 위치. 점은 뷰포트 안으로 자른다(아래·오른쪽 끝은 보이는 마지막 칸 안).
#[must_use]
pub fn anchor_at(viewport: &Viewport, at: (f32, f32)) -> Anchor {
    let x = at.0.clamp(0.0, viewport.width.max(0.0));
    let last_row = (viewport.first_row + viewport.rows_visible()).ceil() - 1.0;
    let row = if viewport.row_height > 0.0 {
        viewport.first_row + at.1.clamp(0.0, viewport.height.max(0.0)) / viewport.row_height
    } else {
        viewport.first_row
    };
    Anchor {
        tick: viewport.first_tick + f64::from(x) / viewport.px_per_tick,
        row: row.min(last_row + 0.999),
    }
}

/// 두 내용 위치의 러버밴드 범위. 틱은 `[내림, 올림)`이고 폭이 0이면(세로로만 끈 경우) 한 틱으로 넓힌다
/// (화면에서 가로지른 노트를 고르게, 리뷰 1 제안 5). 행은 양끝 포함.
#[must_use]
pub fn band_between(a: Anchor, b: Anchor) -> Option<Band> {
    let hi = tick_u(a.tick.max(b.tick).ceil()).max(tick_u(a.tick.min(b.tick)).saturating_add(1));
    // 틱 상한에서는 아래로 한 틱(리뷰 3 제안 2)
    let lo = tick_u(a.tick.min(b.tick)).min(hi.saturating_sub(1));
    let row = |r: f32| r.floor().clamp(0.0, f32::from(ROWS - 1));
    let (top, bottom) = (row(a.row).min(row(b.row)), row(a.row).max(row(b.row)));
    let pitch_of = |row: f32| Pitch::new(ROWS - 1 - row_u8(row)).ok();
    Some(Band {
        lo: Tick::new(lo),
        hi: Tick::new(hi),
        low: pitch_of(bottom)?,
        high: pitch_of(top)?,
    })
}

/// 뷰 기준 두 점의 러버밴드 범위. 점은 뷰포트 안으로 자른다. 뷰포트가 비었으면 `None`.
#[must_use]
pub fn band(viewport: &Viewport, a: (f32, f32), b: (f32, f32)) -> Option<Band> {
    if viewport.width <= 0.0 || viewport.height <= 0.0 || viewport.row_height <= 0.0 {
        return None;
    }
    band_between(anchor_at(viewport, a), anchor_at(viewport, b))
}

/// `clip`에서 `band` 안의 노트.
#[must_use]
pub fn band_notes(clip: &Clip, band: Band) -> BTreeSet<Note> {
    notes_in(clip, band.lo.get(), band.hi.get(), band.low, band.high)
}

/// 클립이 `[lo, hi)`에 걸치는가.
fn overlaps(clip: &Clip, lo: Tick, hi: Tick) -> bool {
    clip.start() < hi && lo.get() < clip.end()
}

/// `tick`을 품은 클립.
fn clip_at(track: &Track, tick: Tick) -> Option<ClipId> {
    track
        .clips()
        .find(|(_, c)| c.start() <= tick && tick.get() < c.end())
        .map(|(id, _)| id)
}

/// 러버밴드의 클립: `preferred` → 누른 틱의 클립 → 지금 틱의 클립 → 시간 범위에 걸치는 첫 클립(결정 3절 5).
/// 어느 후보든 시간 범위에 걸칠 때만 고른다(점이 뷰포트 밖이라 잘린 범위 밖의 클립을 고르지 않는다). 걸치는
/// 클립이 없으면 `None`.
#[must_use]
pub fn band_clip(
    track: &Track,
    press: Tick,
    now: Tick,
    band: Band,
    preferred: Option<ClipId>,
) -> Option<ClipId> {
    let hits = |id: ClipId| {
        track
            .clip(id)
            .is_some_and(|c| overlaps(c, band.lo, band.hi))
    };
    preferred
        .filter(|id| hits(*id))
        .or_else(|| clip_at(track, press).filter(|id| hits(*id)))
        .or_else(|| clip_at(track, now).filter(|id| hits(*id)))
        .or_else(|| {
            track
                .clips()
                .find(|(_, c)| overlaps(c, band.lo, band.hi))
                .map(|(id, _)| id)
        })
}

/// `[from, to)`(절대 틱)에 걸치는 `clip`의 모든 노트(음높이 무관).
#[must_use]
pub fn time_range_notes(clip: &Clip, from: Tick, to: Tick) -> BTreeSet<Note> {
    notes_in(clip, from.get(), to.get(), Pitch::MIN, Pitch::MAX)
}

/// `tick`을 격자로 내린다. 격자의 기준은 그 틱이 든 마디의 시작이다. 격자가 0이면 그대로.
#[must_use]
pub fn snap_floor(meter: &MeterMap, tick: Tick, grid: u64) -> Tick {
    if grid == 0 {
        return tick;
    }
    let bar = meter.position(tick).bar();
    let start = meter.bar_start(bar).map_or(0, Tick::get);
    let offset = tick.get().saturating_sub(start);
    Tick::new(start + offset / grid * grid)
}

/// 두 점 사이가 문턱 이상인가.
#[must_use]
pub fn past_threshold(a: (f32, f32), b: (f32, f32)) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    dx * dx + dy * dy >= DRAG_THRESHOLD * DRAG_THRESHOLD
}

/// `notes`(그 클립의 노트) 가운데 시작이 가장 이르고, 같으면 높은 음인 노트의 절대 칸.
#[must_use]
pub fn first_cell(clip: &Clip, notes: &BTreeSet<Note>) -> Option<Cursor> {
    let first = notes
        .iter()
        .min_by_key(|n| (n.start(), std::cmp::Reverse(n.pitch())))?;
    Some(Cursor {
        tick: Tick::new(clip.start().get().saturating_add(first.start().get())),
        pitch: first.pitch(),
    })
}

/// 진행 중인 마우스 조작(S3-T6a). 누름 하나에 하나이고 뗄 때 없어진다.
#[derive(Clone, Debug)]
pub(crate) enum Gesture {
    /// 노트 위 누름. `collapse`가 있으면 끌지 않고 뗄 때 선택을 그 노트 하나로 줄인다(이미 선택된 노트를 누름).
    Note {
        origin: (f32, f32),
        collapse: Option<(TrackId, ClipId, Note)>,
    },
    /// 빈 곳 누름(아직 끌지 않음). 끌면 러버밴드가 된다. `origin`은 문턱을 재는 화면 점, `anchor`는 그 내용 위치.
    Empty {
        origin: (f32, f32),
        anchor: Anchor,
        add: bool,
        before: Selection,
    },
    /// 러버밴드. 누른 점은 내용 위치(`anchor`), 지금 점은 화면 점(`now`, 포인터는 스크롤해도 화면에 머문다).
    Band {
        anchor: Anchor,
        now: (f32, f32),
        add: bool,
        before: Selection,
    },
    /// `Esc`나 문서 변경으로 취소됨. 뗄 때까지 움직임을 무시한다.
    Cancelled,
}

impl Gesture {
    /// 취소되지 않은 조작인가.
    #[must_use]
    pub(crate) fn is_live(&self) -> bool {
        !matches!(self, Gesture::Cancelled)
    }

    /// 그릴 러버밴드 직사각형(뷰 기준 px, 뷰포트 안으로 자름).
    #[must_use]
    pub(crate) fn band_rect(&self, viewport: &Viewport) -> Option<Rect> {
        match self {
            Gesture::Band { anchor, now, .. } => {
                let origin = (viewport.x_of(anchor.tick), viewport.y_of(anchor.row));
                Some(clipped_rect(origin, *now, viewport.width, viewport.height))
            }
            _ => None,
        }
    }
}

/// 두 점을 뷰포트(`width`×`height`) 안으로 자른 직사각형.
#[must_use]
pub(crate) fn clipped_rect(a: (f32, f32), b: (f32, f32), width: f32, height: f32) -> Rect {
    let x = |v: f32| v.clamp(0.0, width.max(0.0));
    let y = |v: f32| v.clamp(0.0, height.max(0.0));
    let (x0, x1) = (x(a.0).min(x(b.0)), x(a.0).max(x(b.0)));
    let (y0, y1) = (y(a.1).min(y(b.1)), y(a.1).max(y(b.1)));
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}
