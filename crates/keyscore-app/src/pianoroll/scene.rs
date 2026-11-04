//! 피아노롤 장면: 뷰포트 안의 사각형·선 목록. GPUI를 모르는 순수 계산이라 벤치·단위 시험이 바로 돈다.

use keyscore_core::{ClipId, Key, MeterMap, Pitch, Tick, Track};

use super::{Cursor, MaxLenCache, ROWS, Selection, Viewport, px_f32, row_u8, tick_f, tick_u};

/// 뷰 기준 사각형(px).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub(crate) fn overlaps(&self, width: f32, height: f32) -> bool {
        self.x + self.w > 0.0 && self.x < width && self.y + self.h > 0.0 && self.y < height
    }
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
    /// 러버밴드 직사각형(S3-T6a). 장면 계산이 아니라 창의 마우스 조작이 채운다.
    pub band: Option<Rect>,
    /// 끄는 중인 이동·조절의 고스트(S3-T6b, MS-1). 창의 마우스 조작이 채운다.
    pub ghosts: Vec<Rect>,
    /// 고스트의 계획이 거부됐다(거부 색으로 그린다).
    pub ghost_blocked: bool,
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

/// 장조·자연 단조의 음계 간격(비트: 반음 n이 켜짐).
const MAJOR: u16 = 0b1010_1011_0101;
const MINOR: u16 = 0b0101_1010_1101;

/// 행의 명암. 조성 안(장조·자연 단조 음계)이면 `Light`, 조성이 없으면 흰건반이 `Light`.
#[must_use]
pub fn row_shade(key: Key, pitch: Pitch) -> Shade {
    let pc = pitch.get() % 12;
    let (root, scale) = match key {
        Key::None => (0, MAJOR),
        Key::Major(root) => (root.get(), MAJOR),
        Key::Minor(root) => (root.get(), MINOR),
    };
    let degree = (pc + 12 - root) % 12;
    if scale & (1 << degree) != 0 {
        Shade::Light
    } else {
        Shade::Dark
    }
}

/// 장면을 `out`에 채운다. 이전 내용은 버린다(버퍼 재사용).
pub fn build(input: &SceneInput<'_>, out: &mut Scene) {
    out.rows.clear();
    out.lines.clear();
    out.notes.clear();
    out.cursor = None;
    out.playhead = None;
    let vp = input.viewport;
    if vp.width <= 0.0 || vp.height <= 0.0 || vp.px_per_tick <= 0.0 || vp.row_height <= 0.0 {
        return;
    }
    build_rows(input, out);
    build_lines(input, out);
    build_notes(input, out);
    let step = vp.x_of(vp.first_tick + tick_f(input.step_ticks));
    let cursor = Rect {
        x: vp.x_of(tick_f(input.cursor.tick.get())),
        y: vp.y_of(Viewport::row_of(input.cursor.pitch)),
        w: step,
        h: vp.row_height,
    };
    if cursor.overlaps(vp.width, vp.height) {
        out.cursor = Some(cursor);
    }
    if let Some(playhead) = input.playhead {
        let x = vp.x_of(tick_f(playhead.get()));
        if x >= 0.0 && x < vp.width {
            out.playhead = Some(x);
        }
    }
}

fn build_rows(input: &SceneInput<'_>, out: &mut Scene) {
    let vp = input.viewport;
    // 드럼 트랙의 음높이는 악기 종류라 조성 명암을 그리지 않는다(결정 `2026-10-31-key-shading-off-and-drums.md` 2)
    let key = if input.track.is_some_and(Track::is_drum) {
        Key::None
    } else {
        input.key
    };
    let first = row_u8(vp.first_row.floor());
    let last = row_u8((vp.first_row + vp.rows_visible()).ceil());
    for row in first..last {
        let Ok(pitch) = Pitch::new(ROWS - 1 - row) else {
            continue;
        };
        out.rows.push(RowBand {
            pitch,
            rect: Rect {
                x: 0.0,
                y: vp.y_of(f32::from(row)),
                w: vp.width,
                h: vp.row_height,
            },
            shade: row_shade(key, pitch),
        });
    }
}

fn build_lines(input: &SceneInput<'_>, out: &mut Scene) {
    let vp = input.viewport;
    let last_tick = vp.last_tick();
    let first_tick = vp.first_tick.max(0.0);
    let mut bar = input.meter.position(Tick::new(tick_u(first_tick))).bar();
    while let Some(start) = input.meter.bar_start(bar) {
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
        let sig = input.meter.signature_at(start);
        // 박자표가 마디 중간에 바뀌면(SMF) 그 앞 마디는 짧다. 다음 마디 시작에서 박 선을 끊는다.
        let next = input.meter.bar_start(bar + 1);
        for beat in 1..u64::from(sig.numerator()) {
            let Some(tick) = start.checked_add(beat * sig.beat_ticks()).ok() else {
                break;
            };
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
        if next.is_none() {
            break;
        }
        bar += 1;
    }
}

fn build_notes(input: &SceneInput<'_>, out: &mut Scene) {
    let Some(track) = input.track else {
        return;
    };
    let vp = input.viewport;
    let last_tick = vp.last_tick();
    let rows_visible = vp.rows_visible();
    let note_h = (vp.row_height - 1.0).max(1.0);
    for (id, clip) in track.clips() {
        let clip_start = tick_f(clip.start().get());
        if tick_f(clip.end()) <= vp.first_tick || clip_start >= last_tick {
            continue;
        }
        let dim = input.current != Some(id);
        let max_len = f64::from(input.cache.get(id));
        let local_first = vp.first_tick - clip_start;
        let local_last = last_tick - clip_start;
        let notes = clip.notes();
        // 노트는 시작 틱 순(모델 불변식). 끝이 왼쪽 끝 앞인 노트는 최대 길이로 건너뛴다.
        let lo = notes.partition_point(|n| tick_f(n.start().get()) + max_len <= local_first);
        for note in &notes[lo..] {
            let start = tick_f(note.start().get());
            if start >= local_last {
                break;
            }
            if tick_f(note.end()) <= local_first {
                continue;
            }
            let row = Viewport::row_of(note.pitch());
            if row + 1.0 <= vp.first_row || row >= vp.first_row + rows_visible {
                continue;
            }
            let w = px_f32((f64::from(note.length()) * vp.px_per_tick - 1.0).max(1.0));
            out.notes.push(NoteRect {
                rect: Rect {
                    x: vp.x_of(clip_start + start),
                    y: vp.y_of(row),
                    w,
                    h: note_h,
                },
                velocity: note.velocity().to_midi1(),
                dim,
                selected: !dim && input.selection.contains(id, note),
            });
        }
    }
}
