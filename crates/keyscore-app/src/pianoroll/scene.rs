//! 피아노롤 장면: 뷰포트 안의 사각형·선 목록. GPUI를 모르는 순수 계산이라 벤치·단위 시험이 바로 돈다.

use keyscore_core::{ClipId, Key, MeterMap, Pitch, Tick, Track};

use super::{Cursor, MaxLenCache, Selection, Viewport};

/// 뷰 기준 사각형(px).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// 행 배경의 명암. 조성이 있으면 음계 안·밖, 없으면 흰건반·검은건반.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shade {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowBand {
    pub pitch: Pitch,
    pub rect: Rect,
    pub shade: Shade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Bar,
    Beat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLine {
    pub x: f32,
    pub tick: Tick,
    pub kind: LineKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteRect {
    pub rect: Rect,
    /// MIDI 1.0 벨로시티(명도에 쓴다).
    pub velocity: u8,
    /// 현재 클립이 아닌 이웃 클립의 노트(흐리게, 편집 불가).
    pub dim: bool,
    pub selected: bool,
}

/// 한 프레임의 장면. 버퍼는 프레임마다 재사용한다.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub rows: Vec<RowBand>,
    pub lines: Vec<GridLine>,
    pub notes: Vec<NoteRect>,
    pub cursor: Option<Rect>,
    /// 재생 헤드의 x. 뷰포트 밖이면 `None`.
    pub playhead: Option<f32>,
}

/// 장면 입력.
pub struct SceneInput<'a> {
    pub track: Option<&'a Track>,
    pub current: Option<ClipId>,
    pub cache: &'a MaxLenCache,
    pub meter: &'a MeterMap,
    pub key: Key,
    pub cursor: Cursor,
    pub step_ticks: u64,
    pub selection: &'a Selection,
    pub playhead: Option<Tick>,
    pub viewport: Viewport,
}

/// 행의 명암. 조성 안(장조·자연 단조 음계)이면 `Light`, 조성이 없으면 흰건반이 `Light`.
#[must_use]
pub fn row_shade(key: Key, pitch: Pitch) -> Shade {
    todo!()
}

/// 장면을 `out`에 채운다. 이전 내용은 버린다(버퍼 재사용).
pub fn build(input: &SceneInput<'_>, out: &mut Scene) {
    todo!()
}
