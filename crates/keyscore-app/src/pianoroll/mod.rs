//! 피아노롤 뷰(S3-T1, REQ 5.3, 5.7, UI-3, UI-4). 커서·뷰포트 상태와 자체 `Element`.
//!
//! 장면 계산(`scene`)은 GPUI와 무관한 순수 함수이고, `element`는 그 결과를 `paint_layer` 하나 안에서
//! 그린다(`docs/spikes/sp2-element-instancing.md` 분기 A).

pub mod edit;
pub mod element;
pub mod keys;
pub mod mouse;
pub mod ops;
pub mod scene;
pub mod select;
pub mod vim;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use keyscore_core::{Clip, ClipId, MeterMap, Note, Pitch, Tick, Track, TrackId};

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
/// 한 스텝의 기본값(4분음표, REQ 5.6). Step 모드의 길이 코드가 생기면(S3-T4) 그 값을 쓴다.
pub const DEFAULT_STEP_TICKS: u64 = 960;

/// 틱을 f64로. 2^53 틱(120 BPM으로 수십만 년)까지는 정확하고, 그 위는 픽셀 계산에만 쓰므로 정밀도 손실이
/// 보이지 않는다.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub(crate) fn tick_f(tick: u64) -> f64 {
    tick as f64
}

/// 0 이상의 f64 틱을 u64로. 음수는 0, u64 범위 밖은 포화한다(`as`의 규칙).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub(crate) fn tick_u(tick: f64) -> u64 {
    tick.max(0.0).floor() as u64
}

/// 0–128 안으로 자른 행 번호를 정수로.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub(crate) fn row_u8(row: f32) -> u8 {
    row.clamp(0.0, f32::from(ROWS)) as u8
}

/// f64 픽셀 값을 그리기용 f32로(1 px 아래 차이는 보이지 않는다).
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub(crate) fn px_f32(value: f64) -> f32 {
    value as f32
}

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
        // f64로 계산해 큰 틱에서도 1 px 안에서 맞고, 그리기는 f32면 충분하다.
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

    /// 뷰 기준 좌표의 칸(틱은 스텝에 맞추지 않는다). 뷰포트 밖이면 `None`.
    #[must_use]
    pub fn cell_at(&self, x: f32, y: f32) -> Option<Cursor> {
        if x < 0.0 || y < 0.0 || x >= self.width || y >= self.height {
            return None;
        }
        let tick = (self.first_tick + f64::from(x) / self.px_per_tick).floor();
        let row = (self.first_row + y / self.row_height).floor();
        if tick < 0.0 || row < 0.0 || row >= f32::from(ROWS) {
            return None;
        }
        let pitch = Pitch::new(ROWS - 1 - row_u8(row)).ok()?;
        Some(Cursor {
            tick: Tick::new(tick_u(tick)),
            pitch,
        })
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

/// `cursor`를 `delta`만큼 옮긴 칸(`step`은 한 스텝 틱). 틱은 0 아래로, 음높이는 0–127 밖으로 가지 않는다.
#[must_use]
pub fn moved(cursor: Cursor, delta: CursorMove, step: u64, meter: &MeterMap) -> Cursor {
    let Cursor { tick, pitch } = cursor;
    let pitch_by = |semitones: i16| {
        let value = (i16::from(pitch.get()) + semitones).clamp(0, i16::from(ROWS - 1));
        // clamp로 0–127 안이다.
        u8::try_from(value)
            .ok()
            .and_then(|v| Pitch::new(v).ok())
            .unwrap_or(pitch)
    };
    match delta {
        CursorMove::StepBack => Cursor {
            tick: Tick::new(tick.get().saturating_sub(step)),
            pitch,
        },
        CursorMove::StepForward => Cursor {
            tick: tick.checked_add(step).unwrap_or(tick),
            pitch,
        },
        CursorMove::SemitoneDown => Cursor {
            tick,
            pitch: pitch_by(-1),
        },
        CursorMove::SemitoneUp => Cursor {
            tick,
            pitch: pitch_by(1),
        },
        CursorMove::OctaveDown => Cursor {
            tick,
            pitch: pitch_by(-12),
        },
        CursorMove::OctaveUp => Cursor {
            tick,
            pitch: pitch_by(12),
        },
        CursorMove::BarBack => {
            let bar = meter.position(tick).bar();
            let start = meter.bar_start(bar).unwrap_or(Tick::ZERO);
            // 마디 중간이면 그 마디 시작, 시작이면 앞 마디 시작(5.7 「한 마디 뒤로」)
            let target = if tick > start {
                start
            } else {
                meter.bar_start(bar.saturating_sub(1)).unwrap_or(Tick::ZERO)
            };
            Cursor {
                tick: target,
                pitch,
            }
        }
        CursorMove::BarForward => {
            let bar = meter.position(tick).bar();
            Cursor {
                tick: meter.bar_start(bar + 1).unwrap_or(tick),
                pitch,
            }
        }
    }
}

/// 선택(REQ 5.3 「선택 → 동작」, 5.7). 한 클립의 실제 `Note` 값 집합.
///
/// 노트는 그 (트랙, 클립)에 있는 값이거나 다음 문서 변경 때 걸러진다(`retain_existing`). 비어 있으면
/// 트랙·클립도 없다. `Shift+이동`의 기준 칸은 선택이 비어도 남아야 해서(빈 칸을 지나 노트까지 넓히기)
/// [`PianoRollState::anchor`]가 따로 든다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    track: Option<TrackId>,
    clip: Option<ClipId>,
    notes: BTreeSet<Note>,
}

impl Selection {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.notes.len()
    }

    #[must_use]
    pub fn track(&self) -> Option<TrackId> {
        self.track
    }

    #[must_use]
    pub fn clip(&self) -> Option<ClipId> {
        self.clip
    }

    /// `clip`의 이 노트가 선택에 있는가. 다른 클립의 같은 값은 아니다.
    #[must_use]
    pub fn contains(&self, clip: ClipId, note: &Note) -> bool {
        self.clip == Some(clip) && self.notes.contains(note)
    }

    pub fn notes(&self) -> impl Iterator<Item = &Note> {
        self.notes.iter()
    }

    /// 선택을 `notes`로 바꾼다. 비면 `clear`와 같다.
    pub fn set(&mut self, track: TrackId, clip: ClipId, notes: BTreeSet<Note>) {
        if notes.is_empty() {
            self.clear();
            return;
        }
        self.track = Some(track);
        self.clip = Some(clip);
        self.notes = notes;
    }

    pub fn clear(&mut self) {
        *self = Selection::default();
    }

    /// 문서가 바뀐 뒤: `clip`(선택의 클립, 없으면 `None`)에 더 없는 노트를 뺀다. 비면 `clear`. 바뀌었으면 참.
    pub fn retain_existing(&mut self, clip: Option<&Clip>) -> bool {
        let before = self.notes.len();
        match clip {
            Some(clip) => self.notes.retain(|n| clip.notes().binary_search(n).is_ok()),
            None => self.notes.clear(),
        }
        if self.notes.is_empty() {
            let had = before > 0;
            self.clear();
            return had;
        }
        self.notes.len() != before
    }
}

/// vim 마크(`m a`, `' a`): 트랙과 칸. 창마다 a–z.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    pub track: TrackId,
    pub cursor: Cursor,
}

/// 마지막 `f`/`F` 탐색의 방향(`;` `,` 반복).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindDirection {
    Forward,
    Backward,
}

/// 클립마다 가장 긴 노트 길이. 보이는 노트의 왼쪽 경계를 이진 탐색할 때 쓴다. 문서 `revision`과 트랙이
/// 같으면 다시 세지 않는다.
///
/// 계약: 문서는 `apply`·`undo`·`redo`로만 바뀌고 그때마다 `revision`이 오른다. `revision`을 올리지 않고
/// 프로젝트를 바꾸는 API를 `ProjectDocument`에 더하면 이 캐시가 낡아 긴 노트가 화면에서 빠진다(리뷰 1 제안 5).
#[derive(Debug, Default)]
pub struct MaxLenCache {
    key: Option<(u64, TrackId)>,
    /// `ClipId` 순.
    entries: Vec<(ClipId, u32)>,
}

impl MaxLenCache {
    /// `revision`이나 트랙이 바뀌었으면 트랙의 클립마다 다시 센다.
    pub fn refresh(&mut self, track: Option<&Track>, revision: u64) {
        let Some(track) = track else {
            self.key = None;
            self.entries.clear();
            return;
        };
        let key = (revision, track.id());
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.entries.clear();
        self.entries.extend(track.clips().map(|(id, clip)| {
            let max = clip.notes().iter().map(Note::length).max().unwrap_or(0);
            (id, max)
        }));
        self.entries.sort_unstable_by_key(|(id, _)| *id);
    }

    /// 그 클립의 최대 노트 길이. 모르는 클립은 0.
    #[must_use]
    pub fn get(&self, clip: ClipId) -> u32 {
        self.entries
            .binary_search_by_key(&clip, |(id, _)| *id)
            .map_or(0, |i| self.entries[i].1)
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
    /// `Shift+이동`의 기준 칸(절대 틱). 첫 `Shift+이동` 때 그 전 커서 칸으로 잡고, 수식키 없는 이동·선택
    /// 해제·`b`류 선택이 지운다(S3-T2).
    pub(crate) anchor: Option<Cursor>,
    /// vim 비주얼 선택의 종류(S3-T3c). 있으면 `anchor`도 있다(`set_anchor(None)`이 함께 지운다).
    pub(crate) visual: Option<vim::VisualKind>,
    /// vim 마크 a–z(S3-T2).
    pub(crate) marks: BTreeMap<char, Mark>,
    /// 마지막 `f`/`F` 방향(S3-T2).
    pub(crate) last_find: Option<FindDirection>,
    pub(crate) cache: MaxLenCache,
    pub(crate) scene: scene::Scene,
    /// 마지막 `paint`가 그린 노트 수(시험용).
    pub(crate) painted: usize,
    /// 크기를 알기 전에 온 뷰포트 요청. 처음 크기를 알 때 적용한다(리뷰 1 지적 1).
    pending: Option<Follow>,
    /// 진행 중인 마우스 조작(S3-T6a). 누를 때 생기고 뗄 때 없어진다.
    pub(crate) gesture: Option<mouse::Gesture>,
    /// 「마지막 선택」: 클릭·`Cmd+클릭`으로 마지막에 선택에 넣은 노트(`Shift+클릭`의 기준, S3-T6a).
    pub(crate) last_clicked: Option<(ClipId, Note)>,
}

/// 크기를 알기 전에 미뤄 둔 뷰포트 요청.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Follow {
    /// 커서 칸이 보이게만(키 이동).
    Visible,
    /// 커서 칸을 가운데로(피아노롤 진입, 리뷰 2 제안 1).
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

    /// 커서 칸이 뷰포트 안에 있는가(칸이 뷰포트보다 넓으면 왼쪽 끝이 보이면 된다).
    fn cursor_visible(&self) -> bool {
        let vp = self.viewport();
        let tick = tick_f(self.cursor.tick.get());
        let step = tick_f(self.step_ticks);
        let row = Viewport::row_of(self.cursor.pitch);
        tick >= vp.first_tick
            && (tick + step <= vp.last_tick() || tick <= vp.first_tick)
            && row >= self.first_row
            && row + 1.0 <= self.first_row + vp.rows_visible()
    }

    /// 크기를 알기 전이면 처음 크기를 알 때로 미룬다(피아노롤에 처음 들어가는 프레임).
    fn ensure_cursor_visible(&mut self) {
        if self.width <= 0.0 || self.height <= 0.0 {
            self.pending = Some(Follow::Visible);
            return;
        }
        self.pending = None;
        let vp = self.viewport();
        let tick = tick_f(self.cursor.tick.get());
        let step = tick_f(self.step_ticks);
        if tick < vp.first_tick {
            self.first_tick = tick;
        } else if tick + step > vp.last_tick() {
            // 칸이 뷰포트보다 넓으면(큰 줌) 왼쪽 끝을 맞춘다.
            let fit = tick + step - f64::from(vp.width) / vp.px_per_tick;
            self.first_tick = fit.min(tick).max(0.0);
        }
        let row = Viewport::row_of(self.cursor.pitch);
        let visible = vp.rows_visible();
        if row < self.first_row {
            self.first_row = row;
        } else if row + 1.0 > self.first_row + visible {
            self.first_row = row + 1.0 - visible;
        }
        self.clamp_rows();
    }

    fn clamp_rows(&mut self) {
        let max_first = (f32::from(ROWS) - self.viewport().rows_visible()).max(0.0);
        self.first_row = self.first_row.clamp(0.0, max_first);
    }

    /// 크기를 알게 된 처음에 커서 행을 세로 가운데로. 그 전까지는 C4 근처를 보인다.
    pub(crate) fn set_size(&mut self, width: f32, height: f32) {
        let first = self.width <= 0.0 || self.height <= 0.0;
        self.width = width;
        self.height = height;
        if first && width > 0.0 && height > 0.0 {
            self.center_rows();
            match self.pending.take() {
                Some(Follow::Visible) => self.ensure_cursor_visible(),
                Some(Follow::Center) => self.center_cursor(),
                None => {}
            }
        }
        self.clamp_rows();
    }

    /// 커서 칸을 가운데로. 칸이 뷰포트보다 넓으면 왼쪽 끝을 맞춘다.
    fn center_cursor(&mut self) {
        if self.width <= 0.0 || self.height <= 0.0 {
            self.pending = Some(Follow::Center);
            return;
        }
        self.pending = None;
        let tick = tick_f(self.cursor.tick.get());
        let half = f64::from(self.width) / self.px_per_tick / 2.0;
        self.first_tick = (tick + tick_f(self.step_ticks) / 2.0 - half).max(0.0);
        self.center_rows();
        self.ensure_cursor_visible();
    }

    fn center_rows(&mut self) {
        let row = Viewport::row_of(self.cursor.pitch);
        self.first_row = row + 0.5 - self.viewport().rows_visible() / 2.0;
        self.clamp_rows();
    }
}

impl Default for PianoRollState {
    fn default() -> Self {
        Self::new()
    }
}

impl PianoRollState {
    /// 커서는 틱 0·C4, 뷰포트는 C4가 가운데 오도록.
    ///
    /// # Panics
    ///
    /// 없다. C4(60)는 늘 유효한 음높이다.
    #[must_use]
    pub fn new() -> Self {
        let cursor = Cursor {
            tick: Tick::ZERO,
            pitch: Pitch::new(60).expect("C4"),
        };
        PianoRollState {
            inner: Rc::new(RefCell::new(Inner {
                cursor,
                first_tick: 0.0,
                px_per_tick: DEFAULT_PX_PER_TICK,
                // 크기를 알기 전의 임시값: C4가 16행 아래
                first_row: Viewport::row_of(cursor.pitch) - 16.0,
                row_height: DEFAULT_ROW_HEIGHT,
                width: 0.0,
                height: 0.0,
                step_ticks: DEFAULT_STEP_TICKS,
                selection: Selection::default(),
                anchor: None,
                visual: None,
                marks: BTreeMap::new(),
                last_find: None,
                cache: MaxLenCache::default(),
                scene: scene::Scene::default(),
                painted: 0,
                pending: None,
                gesture: None,
                last_clicked: None,
            })),
        }
    }

    #[must_use]
    pub fn cursor(&self) -> Cursor {
        self.inner.borrow().cursor
    }

    #[must_use]
    pub fn viewport(&self) -> Viewport {
        self.inner.borrow().viewport()
    }

    #[must_use]
    pub fn step_ticks(&self) -> u64 {
        self.inner.borrow().step_ticks
    }

    #[must_use]
    pub fn selection(&self) -> Selection {
        self.inner.borrow().selection.clone()
    }

    /// 선택을 바꾼다(창은 `ProjectWindow::set_selection`으로 모드 FSM까지 맞춘다).
    pub fn set_selection(&self, selection: Selection) {
        self.inner.borrow_mut().selection = selection;
    }

    /// `Shift+이동`의 기준 칸.
    #[must_use]
    pub fn anchor(&self) -> Option<Cursor> {
        self.inner.borrow().anchor
    }

    /// 기준 칸을 바꾼다. 지우면 비주얼도 끝난다(비주얼은 기준 칸이 있어야 한다, S3-T3c).
    pub fn set_anchor(&self, anchor: Option<Cursor>) {
        let mut inner = self.inner.borrow_mut();
        inner.anchor = anchor;
        if anchor.is_none() {
            inner.visual = None;
        }
    }

    /// vim 비주얼 선택의 종류(S3-T3c).
    #[must_use]
    pub fn visual(&self) -> Option<vim::VisualKind> {
        self.inner.borrow().visual
    }

    /// 비주얼을 시작하거나 바꾸거나 끝낸다. 기준 칸이 없으면 비주얼을 둘 수 없어 무시한다.
    pub fn set_visual(&self, kind: Option<vim::VisualKind>) {
        let mut inner = self.inner.borrow_mut();
        if kind.is_none() || inner.anchor.is_some() {
            inner.visual = kind;
        }
    }

    pub fn set_mark(&self, name: char, mark: Mark) {
        self.inner.borrow_mut().marks.insert(name, mark);
    }

    #[must_use]
    pub fn mark(&self, name: char) -> Option<Mark> {
        self.inner.borrow().marks.get(&name).copied()
    }

    #[must_use]
    pub fn last_find(&self) -> Option<FindDirection> {
        self.inner.borrow().last_find
    }

    pub fn set_last_find(&self, direction: FindDirection) {
        self.inner.borrow_mut().last_find = Some(direction);
    }

    /// 마지막 `paint`가 그린 노트 수.
    #[must_use]
    pub fn painted_notes(&self) -> usize {
        self.inner.borrow().painted
    }

    pub fn set_cursor(&self, cursor: Cursor) {
        self.inner.borrow_mut().cursor = cursor;
    }

    /// 커서를 옮긴다. 틱은 0 아래로, 음높이는 0–127 밖으로 가지 않는다.
    pub fn move_cursor(&self, delta: CursorMove, meter: &MeterMap) {
        let mut inner = self.inner.borrow_mut();
        inner.cursor = moved(inner.cursor, delta, inner.step_ticks, meter);
    }

    /// 커서 칸이 보이도록 뷰포트를 옮긴다. 크기를 아직 모르면(그리기 전) 아무것도 하지 않는다.
    pub fn ensure_cursor_visible(&self) {
        self.inner.borrow_mut().ensure_cursor_visible();
    }

    /// 커서 칸의 왼쪽 x를 유지하며 배율을 `factor`배 한다(상한·하한 안). 크기를 모르면 배율만 바꾼다.
    pub fn zoom(&self, factor: f64) {
        let mut inner = self.inner.borrow_mut();
        let old = inner.px_per_tick;
        let new = (old * factor).clamp(MIN_PX_PER_TICK, MAX_PX_PER_TICK);
        if (new - old).abs() < f64::EPSILON {
            return;
        }
        if inner.width <= 0.0 || inner.height <= 0.0 {
            inner.px_per_tick = new;
            return;
        }
        let tick = tick_f(inner.cursor.tick.get());
        let x = (tick - inner.first_tick) * old;
        inner.px_per_tick = new;
        inner.first_tick = (tick - x / new).max(0.0);
        inner.ensure_cursor_visible();
    }

    pub fn zoom_reset(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.px_per_tick = DEFAULT_PX_PER_TICK;
        inner.ensure_cursor_visible();
    }

    /// 포인터 x(뷰 기준) 아래의 틱을 제자리에 두고 가로 배율을 `factor`배 한다(상한·하한 안, S3-T6a 마우스
    /// 줌). 커서를 따라가지 않는다.
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

    /// 진행 중 마우스 조작을 버린다(모드·뷰 전환).
    pub fn drop_gesture(&self) {
        self.inner.borrow_mut().gesture = None;
    }

    /// 진행 중인 마우스 조작이 있는가(취소된 조작은 아니다).
    #[must_use]
    pub fn gesture_active(&self) -> bool {
        self.inner
            .borrow()
            .gesture
            .as_ref()
            .is_some_and(mouse::Gesture::is_live)
    }

    /// 지금 그리는 러버밴드 직사각형(뷰 기준 px).
    #[must_use]
    pub fn band_rect(&self) -> Option<scene::Rect> {
        let inner = self.inner.borrow();
        let viewport = inner.viewport();
        inner.gesture.as_ref().and_then(|g| g.band_rect(&viewport))
    }

    /// 끄는 동안의 고스트 사각형(뷰 기준 px, MS-1). 시험용.
    #[must_use]
    pub fn ghost_rects(&self) -> Vec<scene::Rect> {
        todo!("S3-T6b")
    }

    /// 고스트의 계획이 거부됐는가(놓아도 편집하지 않는다).
    #[must_use]
    pub fn ghost_blocked(&self) -> bool {
        todo!("S3-T6b")
    }

    /// 커서 칸을 뷰포트 가운데로. 크기를 모르면 처음 크기를 알 때 한다.
    pub fn center_cursor(&self) {
        self.inner.borrow_mut().center_cursor();
    }

    /// 피아노롤에 들어갈 때: 커서가 보이면 스크롤 자리를 지키고, 안 보이면 커서를 가운데로(리뷰 2 제안 1·2).
    pub fn reveal_cursor_on_entry(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.width <= 0.0 || inner.height <= 0.0 {
            inner.pending = Some(Follow::Center);
        } else if !inner.cursor_visible() {
            inner.center_cursor();
        }
    }

    /// 픽셀만큼 스크롤(양수 = 오른쪽·아래). 왼쪽 끝은 0틱, 위아래는 행 범위 안.
    pub fn scroll_by(&self, dx: f32, dy: f32) {
        let mut inner = self.inner.borrow_mut();
        inner.first_tick = (inner.first_tick + f64::from(dx) / inner.px_per_tick).max(0.0);
        inner.first_row += dy / inner.row_height;
        inner.clamp_rows();
    }

    /// 커서가 트랙의 어느 클립 안도 아니면 첫 클립 시작으로 옮긴다. 옮겼으면 참.
    #[must_use]
    pub fn place_cursor_in_clip(&self, track: Option<&Track>) -> bool {
        if self.current_clip(track).is_some() {
            return false;
        }
        let Some(first) = track.and_then(|t| t.clips().map(|(_, c)| c.start()).min()) else {
            return false;
        };
        self.inner.borrow_mut().cursor.tick = first;
        true
    }

    /// 커서 틱을 품은 클립.
    #[must_use]
    pub fn current_clip(&self, track: Option<&Track>) -> Option<ClipId> {
        let tick = self.inner.borrow().cursor.tick.get();
        track?
            .clips()
            .find(|(_, c)| c.start().get() <= tick && tick < c.end())
            .map(|(id, _)| id)
    }

    pub(crate) fn inner(&self) -> &Rc<RefCell<Inner>> {
        &self.inner
    }
}
