//! 피아노롤 뷰(S3-T1, REQ 5.3, 5.7, UI-3, UI-4). 커서·뷰포트 상태와 자체 `Element`.
//!
//! 장면 계산(`scene`)은 GPUI와 무관한 순수 함수이고, `element`는 그 결과를 `paint_layer` 하나 안에서
//! 그린다(`docs/spikes/sp2-element-instancing.md` 분기 A).

pub mod element;
pub mod scene;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use keyscore_core::{ClipId, MeterMap, Note, Pitch, Tick, Track};

/// 기본 배율: 4분음표(960틱) = 96 px.
pub const DEFAULT_PX_PER_TICK: f64 = 0.1;
/// 기본 행 높이.
pub const DEFAULT_ROW_HEIGHT: f32 = 12.0;
/// 줌 한 단계의 배수.
pub const ZOOM_FACTOR: f64 = 1.25;
/// 배율 하한·상한(px/틱). 하한은 한 화면에 약 20분, 상한은 틱 하나가 2 px.
pub const MIN_PX_PER_TICK: f64 = 0.000_5;
pub const MAX_PX_PER_TICK: f64 = 2.0;
/// 행 수(음높이 0–127).
pub const ROWS: u8 = 128;

/// 커서 한 칸: 트랙 기준 틱과 음높이.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub tick: Tick,
    pub pitch: Pitch,
}

/// 보이는 범위. 행 번호는 위가 0(음높이 127), 아래가 127(음높이 0)이다.
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
    /// 행 번호 ↔ 음높이.
    #[must_use]
    pub fn row_of(pitch: Pitch) -> f32 {
        f32::from(ROWS - 1 - pitch.get())
    }

    /// 틱의 x 좌표(뷰 기준).
    #[must_use]
    pub fn x_of(&self, tick: f64) -> f32 {
        todo!()
    }

    /// 행의 y 좌표(뷰 기준).
    #[must_use]
    pub fn y_of(&self, row: f32) -> f32 {
        todo!()
    }

    /// 오른쪽 끝 틱(배타).
    #[must_use]
    pub fn last_tick(&self) -> f64 {
        todo!()
    }

    /// 뷰 기준 좌표의 칸(틱은 스텝에 맞추지 않는다). 뷰포트 밖이면 `None`.
    #[must_use]
    pub fn cell_at(&self, x: f32, y: f32) -> Option<Cursor> {
        todo!()
    }
}

/// 커서 이동(5.7 수식키 없음·`Ctrl` 행).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorMove {
    StepBack,
    StepForward,
    SemitoneDown,
    SemitoneUp,
    BarBack,
    BarForward,
    OctaveDown,
    OctaveUp,
}

/// 현재 클립 안의 선택 노트. 내용은 S3-T2가 채운다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    notes: BTreeSet<Note>,
}

impl Selection {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    #[must_use]
    pub fn contains(&self, note: &Note) -> bool {
        self.notes.contains(note)
    }

    pub fn insert(&mut self, note: Note) -> bool {
        self.notes.insert(note)
    }

    pub fn clear(&mut self) {
        self.notes.clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.notes.len()
    }
}

/// 클립마다 가장 긴 노트 길이. 보이는 노트의 왼쪽 경계를 이진 탐색할 때 쓴다. 문서 `revision`마다 다시 센다.
#[derive(Debug, Default)]
pub struct MaxLenCache {
    revision: Option<u64>,
    entries: Vec<(ClipId, u32)>,
}

impl MaxLenCache {
    /// `revision`이 바뀌었으면 트랙의 클립마다 다시 센다.
    pub fn refresh(&mut self, track: Option<&Track>, revision: u64) {
        todo!()
    }

    /// 그 클립의 최대 노트 길이. 모르는 클립은 0.
    #[must_use]
    pub fn get(&self, clip: ClipId) -> u32 {
        todo!()
    }
}

/// 창 하나의 피아노롤 상태. `Element`와 공유하므로 `Rc<RefCell>`이다.
#[derive(Clone)]
pub struct PianoRollState {
    inner: Rc<RefCell<Inner>>,
}

pub(crate) struct Inner {
    pub(crate) cursor: Cursor,
    pub(crate) first_tick: f64,
    pub(crate) px_per_tick: f64,
    pub(crate) first_row: f32,
    pub(crate) row_height: f32,
    /// 마지막으로 그린 크기. 그리기 전에는 0.
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) step_ticks: u64,
    pub(crate) selection: Selection,
    pub(crate) cache: MaxLenCache,
    pub(crate) scene: scene::Scene,
    /// 마지막 `paint`가 그린 노트 수(시험용).
    pub(crate) painted: usize,
}

impl Default for PianoRollState {
    fn default() -> Self {
        Self::new()
    }
}

impl PianoRollState {
    /// 커서는 틱 0·C4, 뷰포트는 C4가 가운데 오도록.
    #[must_use]
    pub fn new() -> Self {
        todo!()
    }

    #[must_use]
    pub fn cursor(&self) -> Cursor {
        todo!()
    }

    #[must_use]
    pub fn viewport(&self) -> Viewport {
        todo!()
    }

    #[must_use]
    pub fn step_ticks(&self) -> u64 {
        todo!()
    }

    #[must_use]
    pub fn selection(&self) -> Selection {
        todo!()
    }

    /// 마지막 `paint`가 그린 노트 수.
    #[must_use]
    pub fn painted_notes(&self) -> usize {
        todo!()
    }

    pub fn set_cursor(&self, cursor: Cursor) {
        todo!()
    }

    /// 커서를 옮긴다. 틱은 0 아래로, 음높이는 0–127 밖으로 가지 않는다.
    pub fn move_cursor(&self, delta: CursorMove, meter: &MeterMap) {
        todo!()
    }

    /// 커서 칸이 보이도록 뷰포트를 옮긴다. 크기를 아직 모르면(그리기 전) 아무것도 하지 않는다.
    pub fn ensure_cursor_visible(&self) {
        todo!()
    }

    /// 커서 칸의 왼쪽 x를 유지하며 배율을 `factor`배 한다(상한·하한 안).
    pub fn zoom(&self, factor: f64) {
        todo!()
    }

    pub fn zoom_reset(&self) {
        todo!()
    }

    /// 커서 칸을 뷰포트 가운데로.
    pub fn center_cursor(&self) {
        todo!()
    }

    /// 픽셀만큼 스크롤(양수 = 오른쪽·아래). 왼쪽 끝은 0틱, 위아래는 행 범위 안.
    pub fn scroll_by(&self, dx: f32, dy: f32) {
        todo!()
    }

    /// 커서가 트랙의 어느 클립 안도 아니면 첫 클립 시작으로 옮긴다. 옮겼으면 참.
    pub fn place_cursor_in_clip(&self, track: Option<&Track>) -> bool {
        todo!()
    }

    /// 커서 틱을 품은 클립.
    #[must_use]
    pub fn current_clip(&self, track: Option<&Track>) -> Option<ClipId> {
        todo!()
    }

    pub(crate) fn inner(&self) -> &Rc<RefCell<Inner>> {
        &self.inner
    }
}
