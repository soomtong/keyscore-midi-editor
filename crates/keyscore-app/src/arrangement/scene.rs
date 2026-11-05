//! 배치 뷰 장면: 뷰포트 안의 행 띠·격자선·클립 블록·커서·고스트·재생 헤드. GPUI를 모르는 순수 계산이라 벤치·단위
//! 시험이 바로 돈다.

use keyscore_core::{Clip, ClipId, MeterMap, Project, Tick, TrackColor, TrackId};

use super::{ClipSelection, MIN_BEAT_PX, MIN_LABEL_PX, Viewport};
pub use crate::pianoroll::scene::{GridLine, LineKind, Rect};
use crate::pianoroll::{px_f32, tick_f, tick_u};

/// 트랙 행 하나의 배경 띠.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowBand {
    /// 트랙 순서(0부터).
    pub index: usize,
    pub rect: Rect,
    /// 현재 트랙(커서가 있는 행).
    pub current: bool,
}

/// 클립 블록 하나.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipBlock {
    pub track: TrackId,
    pub clip: ClipId,
    pub rect: Rect,
    /// 클립 색, 없으면 트랙 색(`Clip::color`가 `None`이면 트랙 색을 따른다).
    pub color: TrackColor,
    pub selected: bool,
    /// 이름. 블록이 `MIN_LABEL_PX`보다 좁으면 만들지 않는다.
    pub name: Option<String>,
    /// 첫 마디(클립 시작부터 한 마디) 노트의 가사를 공백으로 이은 것(REQ 5.10 계획: 흐리게).
    pub lyric: Option<String>,
}

/// Step 모드에서 입력하면 생길 클립(배치 뷰 Step 표시, 결정 `2026-10-17-mode-entry-view.md` 이월).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepGhost {
    pub start: Tick,
    /// 끝 틱(배타).
    pub end: u64,
    /// 다음 클립에 막혀 쓸 수 없다(거부 색으로 그린다).
    pub blocked: bool,
}

/// 장면에 든 고스트 블록.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ghost {
    pub rect: Rect,
    pub blocked: bool,
}

/// 한 프레임의 장면. 버퍼는 프레임마다 재사용한다.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub rows: Vec<RowBand>,
    pub lines: Vec<GridLine>,
    pub clips: Vec<ClipBlock>,
    /// 커서 칸(현재 트랙 행, 스텝 너비). 트랙이 없거나 뷰포트 밖이면 `None`.
    pub cursor: Option<Rect>,
    pub ghost: Option<Ghost>,
    /// 재생 헤드의 x. 뷰포트 밖이면 `None`.
    pub playhead: Option<f32>,
}

/// 장면 입력.
pub struct SceneInput<'a> {
    pub project: &'a Project,
    pub current_track: Option<TrackId>,
    /// 커서 틱(창의 커서, 피아노롤과 같다).
    pub cursor: Tick,
    pub step_ticks: u64,
    pub selection: &'a ClipSelection,
    pub ghost: Option<StepGhost>,
    pub playhead: Option<Tick>,
    pub viewport: Viewport,
}

/// 장면을 `out`에 채운다. 이전 내용은 버린다(버퍼 재사용).
pub fn build(input: &SceneInput<'_>, out: &mut Scene) {
    out.rows.clear();
    out.lines.clear();
    out.clips.clear();
    out.cursor = None;
    out.ghost = None;
    out.playhead = None;
    let vp = input.viewport;
    if vp.width <= 0.0 || vp.height <= 0.0 || vp.px_per_tick <= 0.0 || vp.row_height <= 0.0 {
        return;
    }
    let tracks = input.project.tracks();
    let current_row = input
        .current_track
        .and_then(|id| tracks.iter().position(|t| t.id() == id));
    let (first_row, last_row) = visible_rows(vp, tracks.len());
    let block_h = (vp.row_height - 1.0).max(1.0);
    for (row, track) in tracks.iter().enumerate().take(last_row).skip(first_row) {
        let y = vp.y_of(row_f32(row));
        out.rows.push(RowBand {
            index: row,
            rect: Rect {
                x: 0.0,
                y,
                w: vp.width,
                h: vp.row_height,
            },
            current: current_row == Some(row),
        });
        build_clips(input, track, y, block_h, out);
    }
    build_lines(input.project.meter_map(), vp, out);
    if let Some(row) = current_row {
        let cursor = Rect {
            x: vp.x_of(tick_f(input.cursor.get())),
            y: vp.y_of(row_f32(row)),
            w: px_f32(tick_f(input.step_ticks) * vp.px_per_tick),
            h: vp.row_height,
        };
        if cursor.overlaps(vp.width, vp.height) {
            out.cursor = Some(cursor);
        }
        if let Some(g) = input.ghost {
            let rect = block_rect(vp, g.start.get(), g.end, vp.y_of(row_f32(row)), block_h);
            if rect.overlaps(vp.width, vp.height) {
                out.ghost = Some(Ghost {
                    rect,
                    blocked: g.blocked,
                });
            }
        }
    }
    if let Some(playhead) = input.playhead {
        let x = vp.x_of(tick_f(playhead.get()));
        if x >= 0.0 && x < vp.width {
            out.playhead = Some(x);
        }
    }
}

/// 보이는 트랙 행 범위 `[first, last)`.
fn visible_rows(vp: Viewport, rows: usize) -> (usize, usize) {
    let first = row_usize(vp.first_row.floor());
    let last = row_usize((vp.first_row + vp.rows_visible()).ceil()).min(rows);
    (first.min(last), last)
}

/// 블록 사각형: 오른쪽에 1 px 틈(이웃 클립과 구분). `end < start`여도 넘치지 않는다(폭 1 px).
fn block_rect(vp: Viewport, start: u64, end: u64, y: f32, h: f32) -> Rect {
    let w = px_f32((tick_f(end.saturating_sub(start)) * vp.px_per_tick - 1.0).max(1.0));
    Rect {
        x: vp.x_of(tick_f(start)),
        y,
        w,
        h,
    }
}

fn build_clips(
    input: &SceneInput<'_>,
    track: &keyscore_core::Track,
    y: f32,
    h: f32,
    out: &mut Scene,
) {
    let vp = input.viewport;
    let last_tick = vp.last_tick();
    let meter = input.project.meter_map();
    // 클립은 시작 순이고 겹치지 않아 끝도 시작 순(모델 불변식). 왼쪽 끝 앞에서 끝나는 클립은 이진 탐색으로
    // 건너뛰고 오른쪽 끝에서 멈춘다.
    for (id, clip) in track.clips_ending_after(tick_u(vp.first_tick)) {
        if tick_f(clip.start().get()) >= last_tick {
            break;
        }
        let rect = block_rect(vp, clip.start().get(), clip.end(), y, h);
        let (name, lyric) = if rect.w >= MIN_LABEL_PX {
            (clip.name().map(str::to_owned), first_bar_lyric(clip, meter))
        } else {
            (None, None)
        };
        out.clips.push(ClipBlock {
            track: track.id(),
            clip: id,
            rect,
            color: clip.color().unwrap_or_else(|| track.color()),
            selected: input.selection.contains(track.id(), id),
            name,
            lyric,
        });
    }
}

/// 클립 첫 마디 노트의 가사를 공백으로 잇는다. 「첫 마디」는 클립 시작부터 박자표 맵의 그 마디 끝까지다(마디
/// 중간에서 시작하는 클립이면 한 마디보다 짧다. 리뷰 1 제안 1). 가사가 없으면 `None`.
#[must_use]
pub fn first_bar_lyric(clip: &Clip, meter: &MeterMap) -> Option<String> {
    let start = clip.start();
    let bar = meter.position(start).bar();
    let bar_end = meter
        .bar_start(bar + 1)
        .map_or(u64::MAX, |t| t.get().saturating_sub(start.get()));
    let mut out = String::new();
    for note in clip.notes() {
        if note.start().get() >= bar_end {
            break;
        }
        if let Some(lyric) = note.lyric() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(lyric);
        }
    }
    (!out.is_empty()).then_some(out)
}

fn build_lines(meter: &MeterMap, vp: Viewport, out: &mut Scene) {
    let last_tick = vp.last_tick();
    let first_tick = vp.first_tick.max(0.0);
    let mut bar = meter.position(Tick::new(tick_u(first_tick))).bar();
    while let Some(start) = meter.bar_start(bar) {
        let start_f = tick_f(start.get());
        if start_f >= last_tick {
            break;
        }
        if start_f >= first_tick {
            out.lines.push(GridLine {
                x: vp.x_of(start_f),
                tick: start,
                kind: LineKind::Bar,
            });
        }
        let sig = meter.signature_at(start);
        let next = meter.bar_start(bar + 1);
        // 박 선은 한 박이 MIN_BEAT_PX 이상일 때만(촘촘하면 격자가 면이 된다).
        if px_f32(tick_f(sig.beat_ticks()) * vp.px_per_tick) >= MIN_BEAT_PX {
            for beat in 1..u64::from(sig.numerator()) {
                let Ok(tick) = start.checked_add(beat * sig.beat_ticks()) else {
                    break;
                };
                // 박자표가 마디 중간에 바뀌면(SMF) 그 앞 마디는 짧다. 다음 마디 시작에서 박 선을 끊는다.
                if next.is_some_and(|n| tick >= n) {
                    break;
                }
                let beat_f = tick_f(tick.get());
                if beat_f >= last_tick {
                    break;
                }
                if beat_f >= first_tick {
                    out.lines.push(GridLine {
                        x: vp.x_of(beat_f),
                        tick,
                        kind: LineKind::Beat,
                    });
                }
            }
        }
        if next.is_none() {
            break;
        }
        bar += 1;
    }
}

/// 트랙 행 번호를 f32로(트랙 수는 2^24보다 훨씬 작다).
#[allow(clippy::cast_precision_loss)]
fn row_f32(row: usize) -> f32 {
    row as f32
}

/// 0 이상의 행 값을 usize로. 음수는 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn row_usize(row: f32) -> usize {
    row.max(0.0) as usize
}
