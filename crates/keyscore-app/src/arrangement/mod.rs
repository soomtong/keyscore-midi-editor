//! 배치 뷰(S4-T1, REQ 5.10, 5.11, 7.1, UI-3, UI-4). 트랙×시간 격자에 클립 블록을 그리는 자체 `Element`와 그 뷰포트·
//! 선택 상태.
//!
//! 커서는 창에 하나다: 틱은 피아노롤 커서의 틱이고 트랙은 창의 현재 트랙이다(계획 2절, REQ 5.11의 시간 유지 규칙).
//! 이 모듈은 뷰포트(가로 배율·스크롤, 트랙 행 스크롤)와 클립 선택만 갖는다. 장면 계산(`scene`)은 GPUI와 무관한
//! 순수 함수이고, `element`는 그 결과를 `paint_layer` 하나 안에서 그린다(`docs/spikes/sp2-element-instancing.md`
//! 분기 A).

pub mod element;
pub mod scene;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use keyscore_core::{ClipId, Project, Tick, Track, TrackId};

pub use crate::pianoroll::{DEFAULT_PX_PER_TICK, MAX_PX_PER_TICK, MIN_PX_PER_TICK, ZOOM_FACTOR};
use crate::pianoroll::{px_f32, tick_f, tick_u};

/// 트랙 행 높이. REQ에 없는 세부 값이라 S8 테마와 함께 설정으로 뺄지 본다.
pub const DEFAULT_ROW_HEIGHT: f32 = 28.0;
/// 한 박이 이보다 좁으면 박 선을 그리지 않는다(배치 뷰는 피아노롤보다 훨씬 넓은 범위를 보인다).
pub const MIN_BEAT_PX: f32 = 8.0;
/// 블록이 이보다 좁으면 이름·가사 글을 만들지 않는다.
pub const MIN_LABEL_PX: f32 = 24.0;

/// 보이는 범위. 행은 트랙 순서(위가 첫 트랙)다.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// 왼쪽 끝 틱.
    pub first_tick: f64,
    pub px_per_tick: f64,
    pub width: f32,
    /// 위쪽 끝 행(소수 가능).
    pub first_row: f32,
    pub row_height: f32,
    pub height: f32,
}

impl Viewport {
    /// 틱의 x 좌표(뷰 기준).
    #[must_use]
    pub fn x_of(&self, tick: f64) -> f32 {
        px_f32((tick - self.first_tick) * self.px_per_tick)
    }

    /// 행의 y 좌표(뷰 기준).
    #[must_use]
    pub fn y_of(&self, row: f32) -> f32 {
        (row - self.first_row) * self.row_height
    }

    /// 오른쪽 끝 틱(배타).
    #[must_use]
    pub fn last_tick(&self) -> f64 {
        self.first_tick + f64::from(self.width) / self.px_per_tick
    }

    /// 보이는 행 수(소수 가능).
    #[must_use]
    pub fn rows_visible(&self) -> f32 {
        if self.row_height > 0.0 {
            self.height / self.row_height
        } else {
            0.0
        }
    }

    /// 뷰 기준 좌표의 칸: (트랙 행, `step_ticks`로 내린 틱). 뷰포트 밖이거나 트랙 행(`rows`개) 밖이면 `None`.
    #[must_use]
    pub fn cell_at(&self, x: f32, y: f32, step_ticks: u64, rows: usize) -> Option<(usize, Tick)> {
        if x < 0.0 || y < 0.0 || x >= self.width || y >= self.height {
            return None;
        }
        let tick = tick_u(self.first_tick + f64::from(x) / self.px_per_tick);
        let row = (self.first_row + y / self.row_height).floor();
        if row < 0.0 {
            return None;
        }
        // 음수는 위에서 걸렀고 행 수는 usize 범위 안이다.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let row = row as usize;
        if row >= rows {
            return None;
        }
        let step = step_ticks.max(1);
        Some((row, Tick::new(tick / step * step)))
    }
}

/// 클립 선택(REQ 5.10 「`Shift+이동`으로 클립 선택」). 트랙마다 클립 id 집합.
///
/// 선택은 존재하는 클립만 가리키거나 다음 문서 변경 때 걸러진다(`retain_existing`). 이 Task는 전체 선택·해제만
/// 채우고 `Shift+이동` 확장은 S4-T2다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipSelection {
    clips: BTreeMap<TrackId, BTreeSet<ClipId>>,
}

impl ClipSelection {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clips.values().all(BTreeSet::is_empty)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.clips.values().map(BTreeSet::len).sum()
    }

    #[must_use]
    pub fn contains(&self, track: TrackId, clip: ClipId) -> bool {
        self.clips.get(&track).is_some_and(|s| s.contains(&clip))
    }

    pub fn iter(&self) -> impl Iterator<Item = (TrackId, ClipId)> + '_ {
        self.clips
            .iter()
            .flat_map(|(t, s)| s.iter().map(move |c| (*t, *c)))
    }

    pub fn insert(&mut self, track: TrackId, clip: ClipId) {
        self.clips.entry(track).or_default().insert(clip);
    }

    /// 모든 트랙의 클립 전부(`Cmd+A`).
    pub fn set_all(&mut self, project: &Project) {
        self.clips.clear();
        for t in project.tracks() {
            let ids: BTreeSet<ClipId> = t.clips().map(|(id, _)| id).collect();
            if !ids.is_empty() {
                self.clips.insert(t.id(), ids);
            }
        }
    }

    pub fn clear(&mut self) {
        self.clips.clear();
    }

    /// 문서가 바뀐 뒤: 더 없는 트랙·클립을 뺀다. 바뀌었으면 참.
    pub fn retain_existing(&mut self, project: &Project) -> bool {
        let before = self.len();
        self.clips.retain(|track, ids| {
            let Some(t) = project.track(*track) else {
                return false;
            };
            ids.retain(|id| t.clip(*id).is_some());
            !ids.is_empty()
        });
        self.len() != before
    }
}

/// 현재 트랙에서 `tick` 앞의 가장 가까운 클립 경계(클립 시작·끝, 그리고 0틱). 없으면 `None`.
#[must_use]
pub fn clip_boundary_before(track: &Track, tick: Tick) -> Option<Tick> {
    let tick = tick.get();
    let mut best: Option<u64> = (tick > 0).then_some(0);
    for (_, c) in track.clips() {
        if c.start().get() >= tick {
            break;
        }
        best = best.max(Some(c.start().get()));
        if c.end() < tick {
            best = best.max(Some(c.end()));
        }
    }
    best.map(Tick::new)
}

/// 현재 트랙에서 `tick` 뒤의 가장 가까운 클립 경계(클립 시작·끝). 없으면 `None`.
#[must_use]
pub fn clip_boundary_after(track: &Track, tick: Tick) -> Option<Tick> {
    let tick = tick.get();
    track
        .clips_ending_after(tick)
        .flat_map(|(_, c)| [c.start().get(), c.end()])
        .filter(|&b| b > tick)
        .min()
        .map(Tick::new)
}

/// 창 하나의 배치 뷰 상태. `Element`와 공유하므로 `Rc<RefCell>`이다.
#[derive(Clone)]
pub struct ArrangementState {
    inner: Rc<RefCell<Inner>>,
}

/// 커서 칸의 자리: 틱, 스텝 너비, 트랙 행(트랙이 없으면 `None`). 커서는 창의 것이라 호출자가 넘긴다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub tick: Tick,
    pub step_ticks: u64,
    pub row: Option<usize>,
}

pub(crate) struct Inner {
    pub(crate) first_tick: f64,
    pub(crate) px_per_tick: f64,
    pub(crate) first_row: f32,
    pub(crate) row_height: f32,
    /// 마지막으로 그린 크기. 그리기 전에는 0.
    pub(crate) width: f32,
    pub(crate) height: f32,
    /// 마지막으로 그릴 때의 트랙 수(세로 스크롤 범위).
    pub(crate) rows: usize,
    pub(crate) selection: ClipSelection,
    /// 선택을 마지막으로 문서에 대조한 `revision`.
    pub(crate) revision: Option<u64>,
    pub(crate) scene: scene::Scene,
    /// 마지막 `paint`가 그린 클립 블록 수(시험용).
    pub(crate) painted: usize,
    /// 크기를 알기 전에 온 뷰포트 요청. 처음 크기를 알 때 적용한다.
    pending: Option<(Follow, Cell)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Follow {
    /// 커서 칸이 보이게만(키 이동).
    Visible,
    /// 커서 칸을 가운데로(배치 뷰 진입, `view::CenterCursor`).
    Center,
}

impl Inner {
    fn viewport(&self) -> Viewport {
        Viewport {
            first_tick: self.first_tick,
            px_per_tick: self.px_per_tick,
            width: self.width,
            first_row: self.first_row,
            row_height: self.row_height,
            height: self.height,
        }
    }

    fn sized(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }

    /// 커서 칸이 뷰포트 안에 있는가(칸이 뷰포트보다 넓으면 왼쪽 끝이 보이면 된다).
    fn cell_visible(&self, cell: Cell) -> bool {
        let vp = self.viewport();
        let tick = tick_f(cell.tick.get());
        let step = tick_f(cell.step_ticks);
        let row_ok = cell.row.is_none_or(|row| {
            let row = row_f32(row);
            row >= self.first_row && row + 1.0 <= self.first_row + vp.rows_visible()
        });
        tick >= vp.first_tick && (tick + step <= vp.last_tick() || tick <= vp.first_tick) && row_ok
    }

    fn ensure_visible(&mut self, cell: Cell) {
        if !self.sized() {
            self.pending = Some((Follow::Visible, cell));
            return;
        }
        self.pending = None;
        let vp = self.viewport();
        let tick = tick_f(cell.tick.get());
        let step = tick_f(cell.step_ticks);
        if tick < vp.first_tick {
            self.first_tick = tick;
        } else if tick + step > vp.last_tick() {
            // 칸이 뷰포트보다 넓으면(큰 줌) 왼쪽 끝을 맞춘다.
            let fit = tick + step - f64::from(vp.width) / vp.px_per_tick;
            self.first_tick = fit.min(tick).max(0.0);
        }
        if let Some(row) = cell.row {
            let row = row_f32(row);
            let visible = vp.rows_visible();
            if row < self.first_row {
                self.first_row = row;
            } else if row + 1.0 > self.first_row + visible {
                self.first_row = row + 1.0 - visible;
            }
        }
        self.clamp_rows();
    }

    fn center(&mut self, cell: Cell) {
        if !self.sized() {
            self.pending = Some((Follow::Center, cell));
            return;
        }
        self.pending = None;
        let tick = tick_f(cell.tick.get());
        let half = f64::from(self.width) / self.px_per_tick / 2.0;
        self.first_tick = (tick + tick_f(cell.step_ticks) / 2.0 - half).max(0.0);
        if let Some(row) = cell.row {
            self.first_row = row_f32(row) + 0.5 - self.viewport().rows_visible() / 2.0;
        }
        self.clamp_rows();
        self.ensure_visible(cell);
    }

    /// 위쪽 끝은 0행, 아래쪽은 마지막 트랙 행이 뷰포트 아래에 닿는 곳까지.
    fn clamp_rows(&mut self) {
        let max_first = (row_f32(self.rows) - self.viewport().rows_visible()).max(0.0);
        self.first_row = self.first_row.clamp(0.0, max_first);
    }

    /// 그리기 직전: 크기와 트랙 수. 처음 크기를 알면 미뤄 둔 뷰포트 요청을 적용한다.
    pub(crate) fn set_size(&mut self, width: f32, height: f32, rows: usize) {
        let first = !self.sized();
        self.width = width;
        self.height = height;
        self.rows = rows;
        if first && self.sized() {
            match self.pending.take() {
                Some((Follow::Visible, cell)) => self.ensure_visible(cell),
                Some((Follow::Center, cell)) => self.center(cell),
                None => {}
            }
        }
        self.clamp_rows();
    }
}

/// 트랙 행 번호를 f32로(트랙 수는 2^24보다 훨씬 작다).
#[allow(clippy::cast_precision_loss)]
fn row_f32(row: usize) -> f32 {
    row as f32
}

impl Default for ArrangementState {
    fn default() -> Self {
        Self::new()
    }
}

impl ArrangementState {
    /// 뷰포트는 0틱·첫 트랙부터, 기본 배율.
    #[must_use]
    pub fn new() -> Self {
        ArrangementState {
            inner: Rc::new(RefCell::new(Inner {
                first_tick: 0.0,
                px_per_tick: DEFAULT_PX_PER_TICK,
                first_row: 0.0,
                row_height: DEFAULT_ROW_HEIGHT,
                width: 0.0,
                height: 0.0,
                rows: 0,
                selection: ClipSelection::default(),
                revision: None,
                scene: scene::Scene::default(),
                painted: 0,
                pending: None,
            })),
        }
    }

    #[must_use]
    pub fn viewport(&self) -> Viewport {
        self.inner.borrow().viewport()
    }

    #[must_use]
    pub fn selection(&self) -> ClipSelection {
        self.inner.borrow().selection.clone()
    }

    /// 선택을 바꾼다(창은 `ProjectWindow::set_clip_selection`으로 모드 FSM까지 맞춘다).
    pub fn set_selection(&self, selection: ClipSelection) {
        self.inner.borrow_mut().selection = selection;
    }

    /// 마지막으로 만든 장면(시험용 복제본).
    #[must_use]
    pub fn scene(&self) -> scene::Scene {
        self.inner.borrow().scene.clone()
    }

    /// 마지막 `paint`가 그린 클립 블록 수.
    #[must_use]
    pub fn painted_clips(&self) -> usize {
        self.inner.borrow().painted
    }

    /// 커서 칸이 보이도록 뷰포트를 옮긴다. 크기를 아직 모르면 처음 크기를 알 때 한다.
    pub fn ensure_visible(&self, cell: Cell) {
        self.inner.borrow_mut().ensure_visible(cell);
    }

    /// 커서 칸을 뷰포트 가운데로. 크기를 모르면 처음 크기를 알 때 한다.
    pub fn center(&self, cell: Cell) {
        self.inner.borrow_mut().center(cell);
    }

    /// 배치 뷰에 들어올 때: 커서가 보이면 스크롤 자리를 지키고, 안 보이면 커서를 가운데로(S3-T1 리뷰 2 제안 1·2와
    /// 같은 규칙).
    pub fn reveal_on_entry(&self, cell: Cell) {
        let mut inner = self.inner.borrow_mut();
        if !inner.sized() {
            inner.pending = Some((Follow::Center, cell));
        } else if !inner.cell_visible(cell) {
            inner.center(cell);
        }
    }

    /// 커서 칸의 왼쪽 x를 유지하며 배율을 `factor`배 한다(상한·하한 안). 크기를 모르면 배율만 바꾼다.
    pub fn zoom(&self, factor: f64, cell: Cell) {
        let mut inner = self.inner.borrow_mut();
        let old = inner.px_per_tick;
        let new = (old * factor).clamp(MIN_PX_PER_TICK, MAX_PX_PER_TICK);
        if (new - old).abs() < f64::EPSILON {
            return;
        }
        if !inner.sized() {
            inner.px_per_tick = new;
            return;
        }
        let tick = tick_f(cell.tick.get());
        let x = (tick - inner.first_tick) * old;
        inner.px_per_tick = new;
        inner.first_tick = (tick - x / new).max(0.0);
        inner.ensure_visible(cell);
    }

    pub fn zoom_reset(&self, cell: Cell) {
        let mut inner = self.inner.borrow_mut();
        inner.px_per_tick = DEFAULT_PX_PER_TICK;
        inner.ensure_visible(cell);
    }

    /// 포인터 x(뷰 기준) 아래의 틱을 제자리에 두고 가로 배율을 `factor`배 한다(상한·하한 안, `Cmd+휠`·핀치).
    /// 커서를 따라가지 않는다.
    pub fn zoom_at(&self, factor: f64, x: f32) {
        let mut inner = self.inner.borrow_mut();
        let old = inner.px_per_tick;
        let new = (old * factor).clamp(MIN_PX_PER_TICK, MAX_PX_PER_TICK);
        if !new.is_finite() || (new - old).abs() < f64::EPSILON {
            return;
        }
        let tick = inner.first_tick + f64::from(x) / old;
        inner.px_per_tick = new;
        inner.first_tick = (tick - f64::from(x) / new).max(0.0);
    }

    /// 픽셀만큼 스크롤(양수 = 오른쪽·아래). 왼쪽 끝은 0틱, 위아래는 트랙 행 범위 안.
    pub fn scroll_by(&self, dx: f32, dy: f32) {
        let mut inner = self.inner.borrow_mut();
        inner.first_tick = (inner.first_tick + f64::from(dx) / inner.px_per_tick).max(0.0);
        inner.first_row += dy / inner.row_height;
        inner.clamp_rows();
    }

    pub(crate) fn inner(&self) -> &Rc<RefCell<Inner>> {
        &self.inner
    }
}
