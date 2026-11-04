//! S4-T1 배치 뷰 Element(REQ 5.10, 5.11, 7.1, UI-3, UI-4). 장면 계산은 순수 함수로, 뷰 전환·커서·줌·선택은
//! 헤드리스 창에서 본다. 시험 목록은 `docs/reports/s4-t1/plan.md`.
//!
//! 기대값은 정수 틱·행에서 정확히 계산되므로 부동소수 비교와 캐스트를 그대로 쓴다.
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::collections::BTreeSet;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Entity, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};
use keyscore_app::actions::{cursor, transport, view};
use keyscore_app::arrangement::scene::{self, LineKind, Scene, SceneInput, StepGhost};
use keyscore_app::arrangement::{
    ArrangementState, ClipSelection, DEFAULT_PX_PER_TICK, DEFAULT_ROW_HEIGHT, MIN_BEAT_PX,
    MIN_LABEL_PX, Viewport, ZOOM_FACTOR, clip_boundary_after, clip_boundary_before,
};
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Edit, MeterMap, Note, Pitch, Project, Tick, TimeSignature, Track,
    TrackColor, TrackId, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

// ---------- 도우미 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-arr-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 클립 하나: (시작, 길이, 이름). 노트 하나(C5, 16분음표)를 넣어 비어 있지 않게 한다.
fn clip(start: u64, len: u64, name: Option<&str>) -> Clip {
    let mut clip = Clip::new(Tick::new(start), len).unwrap();
    clip.insert_note(note(0, 240, 72)).unwrap();
    if let Some(name) = name {
        clip.set_name(name);
    }
    clip
}

fn track(name: &str, color: u8, clips: &[(u64, u64, Option<&str>)]) -> Track {
    let mut track = Track::new(
        name,
        Channel::new(1).unwrap(),
        TrackColor::new(color).unwrap(),
    );
    for &(s, l, n) in clips {
        track.insert_clip(clip(s, l, n)).unwrap();
    }
    track
}

fn project(tracks: Vec<Track>) -> Project {
    let mut project = Project::new();
    for t in tracks {
        project.push_track(t).unwrap();
    }
    project
}

/// 트랙 셋: T0에 클립 둘(마디 1, 마디 3 「후렴」), T1에 클립 하나(마디 2–3, 색 5), T2는 비어 있다.
fn three_tracks() -> Project {
    let mut t1 = track("T1", 1, &[]);
    let mut c = clip(BAR, 2 * BAR, None);
    c.set_color(Some(TrackColor::new(5).unwrap()));
    t1.insert_clip(c).unwrap();
    project(vec![
        track("T0", 0, &[(0, BAR, None), (2 * BAR, BAR, Some("후렴"))]),
        t1,
        track("T2", 2, &[]),
    ])
}

fn vp(first_tick: f64, px_per_tick: f64, width: f32, first_row: f32, height: f32) -> Viewport {
    Viewport {
        first_tick,
        px_per_tick,
        width,
        first_row,
        row_height: DEFAULT_ROW_HEIGHT,
        height,
    }
}

struct Build<'a> {
    project: &'a Project,
    current: Option<usize>,
    cursor: u64,
    step: u64,
    selection: &'a ClipSelection,
    ghost: Option<StepGhost>,
    playhead: Option<u64>,
    vp: Viewport,
}

fn build(b: &Build<'_>) -> Scene {
    let mut out = Scene::default();
    scene::build(
        &SceneInput {
            project: b.project,
            current_track: b.current.map(|i| b.project.tracks()[i].id()),
            cursor: Tick::new(b.cursor),
            step_ticks: b.step,
            selection: b.selection,
            ghost: b.ghost,
            playhead: b.playhead.map(Tick::new),
            viewport: b.vp,
        },
        &mut out,
    );
    out
}

fn ids(project: &Project, track: usize) -> (TrackId, Vec<ClipId>) {
    let t = &project.tracks()[track];
    (t.id(), t.clips().map(|(id, _)| id).collect())
}

fn open(cx: &mut TestAppContext, project: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(view_of(cx, &view), View::Arrangement, "창은 배치 뷰로 연다");
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(handle, key);
        cx.run_until_parked();
        render(cx, handle);
    }
}

fn event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    e: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| this.apply_mode_event(e, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn view_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> View {
    cx.read(|cx| view.read(cx).mode_state().view())
}

fn cursor_tick(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> u64 {
    cx.read(|cx| view.read(cx).piano_roll().cursor().tick.get())
}

fn set_cursor(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    tick: u64,
) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(60).unwrap(),
        });
    });
    cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
    cx.run_until_parked();
    render(cx, handle);
}

fn current_index(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<usize> {
    cx.read(|cx| {
        let this = view.read(cx);
        let id = this.current_track()?;
        this.document()
            .read(cx)
            .project()
            .tracks()
            .iter()
            .position(|t| t.id() == id)
    })
}

fn arr(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> ArrangementState {
    cx.read(|cx| view.read(cx).arrangement().clone())
}

fn arr_vp(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Viewport {
    arr(cx, view).viewport()
}

fn bounds_of(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    id: &'static str,
) -> gpui_kit::Bounds<Pixels> {
    cx.update_window(handle, |_, window, _| window.find(id).bounds())
        .unwrap()
}

/// `main-view`(패널 크기)의 4 px 여백 안쪽이 Element다.
const MAIN_PAD: f32 = 4.0;

/// 배치 뷰의 (트랙 행, 틱) 자리의 창 좌표(칸 안쪽).
fn at_cell(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    row: usize,
    tick: u64,
) -> Point<Pixels> {
    let main = bounds_of(cx, handle, "main-view");
    let v = arr_vp(cx, view);
    let x = MAIN_PAD + v.x_of(tick as f64) + 2.0;
    let y = MAIN_PAD + v.y_of(row as f32) + 4.0;
    main.origin + point(px(x), px(y))
}

fn mouse(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, button: MouseButton) {
    let mut vcx = VisualTestContext::from_window(handle, cx);
    // 포인터를 먼저 옮겨 히트박스의 hover가 선다(실제 입력 순서와 같다)
    vcx.simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: None,
        modifiers: Modifiers::default(),
    });
    vcx.simulate_event(MouseDownEvent {
        button,
        position: at,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    vcx.simulate_event(MouseUpEvent {
        button,
        position: at,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn wheel(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    dy: f32,
    modifiers: Modifiers,
) {
    let mut vcx = VisualTestContext::from_window(handle, cx);
    vcx.simulate_event(gpui_kit::ScrollWheelEvent {
        position: at,
        delta: gpui_kit::ScrollDelta::Pixels(point(px(0.0), px(dy))),
        modifiers,
        touch_phase: gpui_kit::TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: Box<dyn gpui_kit::Action>) {
    cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

// ---------- 1. 클립 블록의 자리와 색 ----------

#[test]
fn scene_lays_out_clip_blocks_by_track_row_and_tick() {
    let p = three_tracks();
    let sel = ClipSelection::default();
    let s = build(&Build {
        project: &p,
        current: Some(1),
        cursor: 0,
        step: QUARTER,
        selection: &sel,
        ghost: None,
        playhead: None,
        vp: vp(0.0, 0.1, 1600.0, 0.0, 300.0),
    });
    let (t0, c0) = ids(&p, 0);
    let (t1, c1) = ids(&p, 1);
    let blocks: Vec<_> = s
        .clips
        .iter()
        .map(|b| {
            (
                b.track,
                b.clip,
                b.rect.x,
                b.rect.y,
                b.rect.w,
                b.rect.h,
                b.color.index(),
            )
        })
        .collect();
    let h = DEFAULT_ROW_HEIGHT - 1.0;
    assert_eq!(
        blocks,
        vec![
            (t0, c0[0], 0.0, 0.0, BAR as f32 * 0.1 - 1.0, h, 0),
            (
                t0,
                c0[1],
                2.0 * BAR as f32 * 0.1,
                0.0,
                BAR as f32 * 0.1 - 1.0,
                h,
                0
            ),
            // 클립 색이 있으면 그것, 없으면 트랙 색
            (
                t1,
                c1[0],
                BAR as f32 * 0.1,
                DEFAULT_ROW_HEIGHT,
                2.0 * BAR as f32 * 0.1 - 1.0,
                h,
                5
            ),
        ]
    );
    // 행 띠는 트랙마다 하나, 현재 트랙 표시
    let rows: Vec<_> = s
        .rows
        .iter()
        .map(|r| (r.index, r.rect.y, r.current))
        .collect();
    assert_eq!(
        rows,
        vec![
            (0, 0.0, false),
            (1, DEFAULT_ROW_HEIGHT, true),
            (2, 2.0 * DEFAULT_ROW_HEIGHT, false)
        ]
    );
    assert!(s.clips.iter().all(|b| !b.selected));
}

// ---------- 2. 보이는 클립만, 정렬 탐색 = 전수 검사 ----------

fn big_project(tracks: usize, clips: usize) -> Project {
    project(
        (0..tracks)
            .map(|t| {
                let mut track = Track::new(
                    format!("T{t}"),
                    Channel::new(1).unwrap(),
                    TrackColor::new(u8::try_from(t % 12).unwrap()).unwrap(),
                );
                for i in 0..clips as u64 {
                    track.insert_clip(clip(i * BAR, BAR, None)).unwrap();
                }
                track
            })
            .collect(),
    )
}

#[test]
fn scene_skips_offscreen_clips_with_sorted_search() {
    let p = big_project(64, 200);
    let sel = ClipSelection::default();
    // 50–60마디, 10–20행
    let v = vp(
        50.0 * BAR as f64,
        0.1,
        10.0 * BAR as f32 * 0.1,
        10.0,
        10.0 * DEFAULT_ROW_HEIGHT,
    );
    let s = build(&Build {
        project: &p,
        current: Some(0),
        cursor: 0,
        step: QUARTER,
        selection: &sel,
        ghost: None,
        playhead: None,
        vp: v,
    });
    let got: BTreeSet<(TrackId, ClipId)> = s.clips.iter().map(|b| (b.track, b.clip)).collect();
    let mut want = BTreeSet::new();
    for (row, t) in p.tracks().iter().enumerate() {
        let row = row as f32;
        if row + 1.0 <= v.first_row || row >= v.first_row + v.rows_visible() {
            continue;
        }
        for (id, c) in t.clips() {
            if (c.end() as f64) > v.first_tick && (c.start().get() as f64) < v.last_tick() {
                want.insert((t.id(), id));
            }
        }
    }
    assert_eq!(got.len(), 10 * 10, "행 10개 × 마디 10개");
    assert_eq!(got, want);
    // 왼쪽 경계에 걸친 클립: 49.5마디부터 보면 49번 클립도 든다
    let s = build(&Build {
        project: &p,
        current: Some(0),
        cursor: 0,
        step: QUARTER,
        selection: &sel,
        ghost: None,
        playhead: None,
        vp: vp(
            49.5 * BAR as f64,
            0.1,
            10.0 * BAR as f32 * 0.1,
            10.0,
            DEFAULT_ROW_HEIGHT,
        ),
    });
    let (_, row10) = ids(&p, 10);
    assert!(s.clips.iter().any(|b| b.clip == row10[49]));
    assert!(s.clips.iter().all(|b| b.clip != row10[48]));
    assert_eq!(s.clips.len(), 11);
}

// ---------- 3. 라벨: 이름과 첫 마디 가사만 ----------

#[test]
fn scene_labels_use_name_and_first_bar_lyric_only() {
    let mut c = Clip::new(Tick::new(BAR), 3 * BAR).unwrap();
    c.set_name("후렴");
    c.insert_note(note(0, 480, 60).with_lyric("사")).unwrap();
    c.insert_note(note(480, 480, 62).with_lyric("랑")).unwrap();
    c.insert_note(note(960, 480, 64)).unwrap();
    c.insert_note(note(BAR, 480, 65).with_lyric("해")).unwrap();
    let meter = MeterMap::default();
    assert_eq!(scene::first_bar_lyric(&c, &meter).as_deref(), Some("사 랑"));
    // 3/4에서는 첫 마디가 세 박이라 2880틱 뒤의 가사는 빠진다
    let mut c34 = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    c34.insert_note(note(0, 480, 60).with_lyric("가")).unwrap();
    c34.insert_note(note(2880, 480, 60).with_lyric("나"))
        .unwrap();
    let meter34 = MeterMap::new(TimeSignature::new(3, 4).unwrap());
    assert_eq!(
        scene::first_bar_lyric(&c34, &meter34).as_deref(),
        Some("가")
    );
    assert_eq!(
        scene::first_bar_lyric(&c34, &meter).as_deref(),
        Some("가 나")
    );
    let plain = Clip::new(Tick::ZERO, BAR).unwrap();
    assert_eq!(scene::first_bar_lyric(&plain, &meter), None);

    let mut t = track("T0", 0, &[(0, BAR, None)]);
    t.insert_clip(c).unwrap();
    let p = project(vec![t]);
    let sel = ClipSelection::default();
    let s = build(&Build {
        project: &p,
        current: Some(0),
        cursor: 0,
        step: QUARTER,
        selection: &sel,
        ghost: None,
        playhead: None,
        vp: vp(0.0, 0.1, 1600.0, 0.0, 100.0),
    });
    let labels: Vec<_> = s
        .clips
        .iter()
        .map(|b| (b.name.as_deref(), b.lyric.as_deref()))
        .collect();
    assert_eq!(labels, vec![(None, None), (Some("후렴"), Some("사 랑"))]);
    // 블록이 좁으면(라벨 최소 폭 미만) 글을 만들지 않는다
    let narrow = build(&Build {
        project: &p,
        current: Some(0),
        cursor: 0,
        step: QUARTER,
        selection: &sel,
        ghost: None,
        playhead: None,
        vp: vp(
            0.0,
            (f64::from(MIN_LABEL_PX) - 2.0) / (3.0 * BAR as f64),
            1600.0,
            0.0,
            100.0,
        ),
    });
    assert_eq!(narrow.clips.len(), 2);
    assert!(
        narrow
            .clips
            .iter()
            .all(|b| b.name.is_none() && b.lyric.is_none())
    );
}

// ---------- 4. 격자선: 박자표 맵, 촘촘하면 박 선 없음 ----------

#[test]
fn scene_grid_lines_follow_meter_map_and_hide_dense_beats() {
    let mut p = three_tracks();
    *p.meter_map_mut() = MeterMap::from_changes(
        TimeSignature::new(3, 4).unwrap(),
        [(Tick::new(2 * 2880), TimeSignature::new(4, 4).unwrap())],
    );
    let sel = ClipSelection::default();
    let s = build(&Build {
        project: &p,
        current: Some(0),
        cursor: 0,
        step: QUARTER,
        selection: &sel,
        ghost: None,
        playhead: None,
        vp: vp(0.0, 0.1, 1600.0, 0.0, 100.0),
    });
    let bars: Vec<u64> = s
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Bar)
        .map(|l| l.tick.get())
        .collect();
    assert_eq!(bars, vec![0, 2880, 5760, 9600, 13440]);
    let beats: Vec<u64> = s
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Beat)
        .map(|l| l.tick.get())
        .collect();
    assert_eq!(
        beats,
        vec![
            960, 1920, 3840, 4800, 6720, 7680, 8640, 10560, 11520, 12480, 14400, 15360
        ]
    );
    assert!(s.lines.iter().all(|l| l.x == l.tick.get() as f32 * 0.1));
    // 한 박이 MIN_BEAT_PX보다 좁으면 박 선을 그리지 않는다
    let dense = build(&Build {
        project: &p,
        current: Some(0),
        cursor: 0,
        step: QUARTER,
        selection: &sel,
        ghost: None,
        playhead: None,
        vp: vp(
            0.0,
            (f64::from(MIN_BEAT_PX) - 1.0) / 960.0,
            1600.0,
            0.0,
            100.0,
        ),
    });
    assert!(dense.lines.iter().all(|l| l.kind == LineKind::Bar));
    assert!(dense.lines.len() > 10);
}

// ---------- 5. 커서·선택·고스트·재생 헤드 ----------

#[test]
fn scene_cursor_selection_ghost_playhead_positions() {
    let p = three_tracks();
    let (t0, c0) = ids(&p, 0);
    let mut sel = ClipSelection::default();
    sel.insert(t0, c0[1]);
    let s = build(&Build {
        project: &p,
        current: Some(1),
        cursor: 960,
        step: 480,
        selection: &sel,
        ghost: Some(StepGhost {
            start: Tick::new(2 * BAR),
            end: 3 * BAR,
            blocked: false,
        }),
        playhead: Some(1920),
        vp: vp(0.0, 0.1, 1600.0, 0.0, 300.0),
    });
    let cursor = s.cursor.unwrap();
    assert_eq!(
        (cursor.x, cursor.y, cursor.w, cursor.h),
        (96.0, DEFAULT_ROW_HEIGHT, 48.0, DEFAULT_ROW_HEIGHT)
    );
    let selected: Vec<_> = s
        .clips
        .iter()
        .filter(|b| b.selected)
        .map(|b| b.clip)
        .collect();
    assert_eq!(selected, vec![c0[1]]);
    let ghost = s.ghost.unwrap();
    assert_eq!(
        (ghost.rect.x, ghost.rect.y, ghost.rect.w, ghost.blocked),
        (768.0, DEFAULT_ROW_HEIGHT, 384.0 - 1.0, false)
    );
    assert_eq!(s.playhead, Some(192.0));
    // 뷰포트 밖: 재생 헤드·커서 없음, 막힌 고스트는 표시
    let s = build(&Build {
        project: &p,
        current: Some(1),
        cursor: 960,
        step: 480,
        selection: &sel,
        ghost: Some(StepGhost {
            start: Tick::new(BAR),
            end: 2 * BAR,
            blocked: true,
        }),
        playhead: Some(100 * BAR),
        vp: vp(50.0 * BAR as f64, 0.1, 1600.0, 0.0, 300.0),
    });
    assert_eq!(s.playhead, None);
    assert_eq!(s.cursor, None);
    assert_eq!(s.ghost, None, "고스트도 뷰포트 밖");
    let s = build(&Build {
        project: &p,
        current: Some(1),
        cursor: 960,
        step: 480,
        selection: &sel,
        ghost: Some(StepGhost {
            start: Tick::new(BAR),
            end: 2 * BAR,
            blocked: true,
        }),
        playhead: None,
        vp: vp(0.0, 0.1, 1600.0, 0.0, 300.0),
    });
    assert!(s.ghost.unwrap().blocked);
    // 현재 트랙이 없으면 커서 칸도 없다
    let s = build(&Build {
        project: &p,
        current: None,
        cursor: 960,
        step: 480,
        selection: &sel,
        ghost: None,
        playhead: None,
        vp: vp(0.0, 0.1, 1600.0, 0.0, 300.0),
    });
    assert_eq!(s.cursor, None);
}

// ---------- 6. 클립 경계 ----------

#[test]
fn clip_boundaries_on_current_track() {
    let t = track("T", 0, &[(BAR, BAR, None), (3 * BAR, BAR, None)]);
    let before = |tick: u64| clip_boundary_before(&t, Tick::new(tick)).map(Tick::get);
    let after = |tick: u64| clip_boundary_after(&t, Tick::new(tick)).map(Tick::get);
    assert_eq!(before(0), None);
    assert_eq!(after(0), Some(BAR));
    assert_eq!(before(BAR), Some(0));
    assert_eq!(after(BAR), Some(2 * BAR));
    assert_eq!(before(BAR + BAR / 2), Some(BAR));
    assert_eq!(after(2 * BAR), Some(3 * BAR));
    assert_eq!(after(4 * BAR), None);
    assert_eq!(before(5 * BAR), Some(4 * BAR));
    // 클립이 없는 트랙: 0틱만 경계
    let empty = track("E", 0, &[]);
    assert_eq!(
        clip_boundary_before(&empty, Tick::new(100)),
        Some(Tick::ZERO)
    );
    assert_eq!(clip_boundary_before(&empty, Tick::ZERO), None);
    assert_eq!(clip_boundary_after(&empty, Tick::new(100)), None);
}

// ---------- 7. v / Enter / Esc 전이 ----------

#[gpui_kit::test]
fn v_toggles_views_enter_opens_clip_and_escape_keeps_view(cx: &mut TestAppContext) {
    start(cx, "views");
    let (handle, view) = open(cx, three_tracks());
    press(cx, handle, "v");
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    press(cx, handle, "v");
    assert_eq!(view_of(cx, &view), View::Arrangement);
    press(cx, handle, "enter");
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    // 나올 때도 `v`다(REQ 5.10). `Esc`는 뷰를 바꾸지 않는다
    press(cx, handle, "escape");
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    press(cx, handle, "v");
    assert_eq!(view_of(cx, &view), View::Arrangement);
    // 배치 뷰로 올 때 커서 칸이 보인다
    let v = arr_vp(cx, &view);
    let tick = cursor_tick(cx, &view) as f64;
    assert!(v.width > 0.0 && v.height > 0.0, "{v:?}");
    assert!(
        tick >= v.first_tick && tick + 960.0 <= v.last_tick(),
        "{v:?} {tick}"
    );
}

// ---------- 8. 배치 뷰 커서 키 ----------

#[gpui_kit::test]
fn arrangement_cursor_keys_move_by_clip_boundary_track_and_bar(cx: &mut TestAppContext) {
    start(cx, "cursor-keys");
    let p = project(vec![
        track("T0", 0, &[(BAR, BAR, None), (3 * BAR, BAR, None)]),
        track("T1", 1, &[]),
        track("T2", 2, &[]),
    ]);
    let (handle, view) = open(cx, p);
    assert_eq!(current_index(cx, &view), Some(0));
    assert_eq!(cursor_tick(cx, &view), 0);
    for (key, want) in [
        ("l", BAR),
        ("l", 2 * BAR),
        ("l", 3 * BAR),
        ("l", 4 * BAR),
        ("l", 4 * BAR),
        ("h", 3 * BAR),
        ("ctrl-l", 4 * BAR),
        ("ctrl-h", 3 * BAR),
        ("h", 2 * BAR),
        ("h", BAR),
        ("h", 0),
        ("h", 0),
    ] {
        press(cx, handle, key);
        assert_eq!(cursor_tick(cx, &view), want, "{key}");
    }
    set_cursor(cx, handle, &view, 2 * BAR);
    for (key, want) in [
        ("j", 1),
        ("j", 2),
        ("j", 2),
        ("k", 1),
        ("ctrl-k", 0),
        ("ctrl-j", 2),
        ("k", 1),
    ] {
        press(cx, handle, key);
        assert_eq!(current_index(cx, &view), Some(want), "{key}");
        assert_eq!(cursor_tick(cx, &view), 2 * BAR, "{key}: 틱은 그대로");
    }
}

// ---------- 9. 커서를 따라 뷰포트가 움직인다 ----------

#[gpui_kit::test]
fn cursor_moves_scroll_arrangement_viewport_to_follow(cx: &mut TestAppContext) {
    start(cx, "follow");
    let mut tracks = vec![track("T0", 0, &[(100 * BAR, BAR, None)])];
    for i in 1..40 {
        tracks.push(track(&format!("T{i}"), 1, &[]));
    }
    let (handle, view) = open(cx, project(tracks));
    let v0 = arr_vp(cx, &view);
    assert_eq!(v0.first_tick, 0.0);
    assert_eq!(v0.first_row, 0.0);
    assert!(
        v0.rows_visible() < 40.0,
        "{v0:?}: 창이 트랙 40개보다 낮아야 세로 따라가기를 본다"
    );
    press(cx, handle, "l");
    assert_eq!(cursor_tick(cx, &view), 100 * BAR);
    let v1 = arr_vp(cx, &view);
    assert!(v1.first_tick > 0.0, "{v1:?}");
    let tick = (100 * BAR) as f64;
    assert!(
        tick >= v1.first_tick && tick + 960.0 <= v1.last_tick(),
        "{v1:?}"
    );
    press(cx, handle, "ctrl-j");
    assert_eq!(current_index(cx, &view), Some(39));
    let v2 = arr_vp(cx, &view);
    assert!(v2.first_row > 0.0, "{v2:?}");
    assert!(
        39.0 >= v2.first_row && 40.0 <= v2.first_row + v2.rows_visible() + 0.01,
        "{v2:?}"
    );
    press(cx, handle, "ctrl-k");
    let v3 = arr_vp(cx, &view);
    assert_eq!(v3.first_row, 0.0, "{v3:?}");
}

// ---------- 10. 줌 키는 보이는 뷰에만 ----------

#[gpui_kit::test]
fn zoom_keys_apply_only_to_the_visible_view(cx: &mut TestAppContext) {
    start(cx, "zoom-split");
    let (handle, view) = open(cx, three_tracks());
    let pr =
        |cx: &mut TestAppContext| cx.read(|cx| view.read(cx).piano_roll().viewport().px_per_tick);
    assert_eq!(arr_vp(cx, &view).px_per_tick, DEFAULT_PX_PER_TICK);
    press(cx, handle, "cmd-=");
    assert_eq!(
        arr_vp(cx, &view).px_per_tick,
        DEFAULT_PX_PER_TICK * ZOOM_FACTOR
    );
    assert_eq!(pr(cx), DEFAULT_PX_PER_TICK, "피아노롤은 그대로");
    press(cx, handle, "cmd--");
    assert!((arr_vp(cx, &view).px_per_tick - DEFAULT_PX_PER_TICK).abs() < 1e-12);
    press(cx, handle, "cmd-=");
    press(cx, handle, "v");
    press(cx, handle, "cmd-=");
    assert_eq!(pr(cx), DEFAULT_PX_PER_TICK * ZOOM_FACTOR);
    assert_eq!(
        arr_vp(cx, &view).px_per_tick,
        DEFAULT_PX_PER_TICK * ZOOM_FACTOR,
        "배치 뷰는 그대로"
    );
    press(cx, handle, "cmd-0");
    assert_eq!(pr(cx), DEFAULT_PX_PER_TICK);
    assert_eq!(
        arr_vp(cx, &view).px_per_tick,
        DEFAULT_PX_PER_TICK * ZOOM_FACTOR
    );
    press(cx, handle, "v");
    press(cx, handle, "cmd-0");
    assert_eq!(arr_vp(cx, &view).px_per_tick, DEFAULT_PX_PER_TICK);
    // 가운데로(기본 프리셋에는 키가 없어 Action으로): 커서 칸이 가로 가운데 근처
    set_cursor(cx, handle, &view, 200 * BAR);
    run(cx, handle, Box::new(view::CenterCursor));
    let v = arr_vp(cx, &view);
    let tick = (200 * BAR) as f64;
    let center = v.first_tick + f64::from(v.width) / v.px_per_tick / 2.0;
    assert!(
        (tick + 480.0 - center).abs() < 960.0,
        "{v:?} center {center}"
    );
}

// ---------- 11. 전체 선택과 해제 ----------

#[gpui_kit::test]
fn select_all_in_arrangement_selects_every_clip_and_escape_clears(cx: &mut TestAppContext) {
    start(cx, "select-all");
    let (handle, view) = open(cx, three_tracks());
    let sel = |cx: &mut TestAppContext| cx.read(|cx| view.read(cx).clip_selection());
    let has = |cx: &mut TestAppContext| cx.read(|cx| view.read(cx).mode_state().has_selection());
    assert!(sel(cx).is_empty());
    press(cx, handle, "cmd-a");
    assert_eq!(sel(cx).len(), 3);
    assert!(has(cx), "컨텍스트에 Selection");
    let s = arr(cx, &view).scene();
    assert_eq!(
        s.clips.iter().filter(|b| b.selected).count(),
        3,
        "선택 클립을 그린다"
    );
    press(cx, handle, "escape");
    assert!(sel(cx).is_empty());
    assert!(!has(cx));
    // 피아노롤의 `cmd-a`는 노트 선택이고 클립 선택은 비어 있다
    press(cx, handle, "v");
    press(cx, handle, "cmd-a");
    assert!(sel(cx).is_empty());
    assert!(has(cx));
    assert!(!cx.read(|cx| view.read(cx).selection().is_empty()));
    // 뷰를 바꾸면 선택이 비워진다(FSM)
    press(cx, handle, "v");
    assert!(!has(cx));
    press(cx, handle, "cmd-a");
    assert_eq!(sel(cx).len(), 3);
    press(cx, handle, "v");
    assert!(sel(cx).is_empty(), "뷰 전환은 클립 선택도 비운다");
    assert!(!has(cx));
}

// ---------- 12. 선택은 문서를 따라간다 ----------

#[gpui_kit::test]
fn clip_selection_drops_removed_clips(cx: &mut TestAppContext) {
    start(cx, "selection-follows");
    let (handle, view) = open(cx, three_tracks());
    press(cx, handle, "cmd-a");
    assert_eq!(cx.read(|cx| view.read(cx).clip_selection().len()), 3);
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let (t0, c0) = ids(doc.project(), 0);
            doc.apply(vec![Edit::RemoveClip {
                track: t0,
                id: c0[0],
            }])
            .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    render(cx, handle);
    let sel = cx.read(|cx| view.read(cx).clip_selection());
    assert_eq!(sel.len(), 2);
    let (t0, c0) = cx.read(|cx| ids(view.read(cx).document().read(cx).project(), 0));
    assert!(sel.contains(t0, c0[0]), "남은 클립은 그대로");
    assert!(cx.read(|cx| view.read(cx).mode_state().has_selection()));
    // 전부 없어지면 선택 컨텍스트도 빠진다
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let edits: Vec<Edit> = doc
                .project()
                .tracks()
                .iter()
                .flat_map(|t| {
                    t.clips()
                        .map(move |(id, _)| Edit::RemoveClip { track: t.id(), id })
                })
                .collect();
            doc.apply(edits).unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    render(cx, handle);
    assert!(cx.read(|cx| view.read(cx).clip_selection().is_empty()));
    assert!(!cx.read(|cx| view.read(cx).mode_state().has_selection()));
}

// ---------- 13. 커서부터 재생은 두 뷰 모두 커서 틱 ----------

#[gpui_kit::test]
fn play_from_cursor_uses_cursor_tick_in_both_views(cx: &mut TestAppContext) {
    start(cx, "play-from");
    let (handle, view) = open(cx, three_tracks());
    set_cursor(cx, handle, &view, 2 * BAR + 960);
    let from = |cx: &mut TestAppContext| cx.read(|cx| view.read(cx).play_from_cursor_tick().get());
    assert_eq!(from(cx), 2 * BAR + 960);
    press(cx, handle, "v");
    assert_eq!(from(cx), 2 * BAR + 960);
}

// ---------- 14. 우클릭 마크 메뉴 ----------

#[gpui_kit::test]
fn right_click_in_arrangement_opens_mark_menu_and_sets_mark(cx: &mut TestAppContext) {
    start(cx, "right-click");
    let (handle, view) = open(cx, three_tracks());
    assert!(!cx.read(|cx| view.read(cx).context_menu_open()));
    let at = at_cell(cx, handle, &view, 1, 2 * BAR);
    mouse(cx, handle, at, MouseButton::Right);
    assert!(
        cx.read(|cx| view.read(cx).context_menu_open()),
        "마크 메뉴가 열린다"
    );
    assert_eq!(current_index(cx, &view), Some(1), "우클릭한 행의 트랙");
    assert_eq!(cursor_tick(cx, &view), 2 * BAR, "우클릭한 칸(스텝 내림)");
    press(cx, handle, "escape");
    assert!(!cx.read(|cx| view.read(cx).context_menu_open()));
    run(cx, handle, Box::new(cursor::SetMark("a".into())));
    let mark = cx
        .read(|cx| view.read(cx).piano_roll().mark('a'))
        .expect("마크 a");
    let (t1, _) = cx.read(|cx| ids(view.read(cx).document().read(cx).project(), 1));
    assert_eq!(mark.track, t1);
    assert_eq!(mark.cursor.tick.get(), 2 * BAR);
    // 다른 트랙으로 간 뒤 마크로 돌아온다
    press(cx, handle, "ctrl-k h");
    assert_eq!(current_index(cx, &view), Some(0));
    run(cx, handle, Box::new(cursor::JumpMark("a".into())));
    assert_eq!(current_index(cx, &view), Some(1));
    assert_eq!(cursor_tick(cx, &view), 2 * BAR);
}

// ---------- 15. Step 모드: 격자 위 커서와 고스트 ----------

#[gpui_kit::test]
fn step_in_arrangement_draws_cursor_and_ghost_on_grid(cx: &mut TestAppContext) {
    start(cx, "step-ghost");
    let (handle, view) = open(cx, project(vec![track("Lead", 0, &[(0, BAR, None)])]));
    set_cursor(cx, handle, &view, 2 * BAR);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    let ghost = cx
        .read(|cx| view.read(cx).step_ghost(cx))
        .expect("클립 밖이면 고스트");
    assert_eq!(
        (ghost.start.get(), ghost.end, ghost.blocked),
        (2 * BAR, 3 * BAR, false)
    );
    let s = arr(cx, &view).scene();
    let v = arr_vp(cx, &view);
    let g = s.ghost.expect("장면에 고스트");
    assert_eq!(g.rect.x, v.x_of((2 * BAR) as f64));
    assert_eq!(g.rect.w, (BAR as f64 * v.px_per_tick) as f32 - 1.0);
    assert!(!g.blocked);
    let c = s.cursor.expect("장면에 커서");
    assert_eq!(c.x, v.x_of((2 * BAR) as f64));
    assert_eq!(
        c.w,
        (960.0 * v.px_per_tick) as f32,
        "Step 길이(4분음표) 너비"
    );
    // 클립 안이면 고스트 없음
    set_cursor(cx, handle, &view, 480);
    assert!(cx.read(|cx| view.read(cx).step_ghost(cx)).is_none());
    assert!(arr(cx, &view).scene().ghost.is_none());
    // 기존 글 표시는 접근성 이름으로 남는다
    let label = cx
        .update_window(handle, |_, window, _| {
            window
                .try_find("arrangement-step-cursor")
                .and_then(|e| e.label().map(str::to_owned))
        })
        .unwrap();
    assert_eq!(label.as_deref(), Some("Step: Lead 1.1.480"));
    // Normal로 나오면 고스트도 없다
    event(cx, handle, &view, ModeEvent::ToNormal);
    set_cursor(cx, handle, &view, 2 * BAR);
    assert!(arr(cx, &view).scene().ghost.is_none());
}

/// 다음 클립에 막혀 쓸 수 없으면 고스트가 거부 색이다(`arrangement_step`의 「여기서는 쓸 수 없다」와 같은 조건).
#[gpui_kit::test]
fn step_ghost_is_blocked_when_next_clip_is_in_the_way(cx: &mut TestAppContext) {
    start(cx, "step-blocked");
    let (handle, view) = open(
        cx,
        project(vec![track(
            "Lead",
            0,
            &[(0, BAR, None), (BAR + 480, 480, None)],
        )]),
    );
    set_cursor(cx, handle, &view, BAR);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    let ghost = cx
        .read(|cx| view.read(cx).step_ghost(cx))
        .expect("막혀도 고스트");
    assert!(ghost.blocked, "{ghost:?}");
    assert_eq!(ghost.start.get(), BAR);
    assert!(arr(cx, &view).scene().ghost.is_some_and(|g| g.blocked));
}

// ---------- 16. 클릭과 휠 ----------

#[gpui_kit::test]
fn click_places_cursor_and_wheel_scrolls(cx: &mut TestAppContext) {
    start(cx, "click-wheel");
    let mut tracks = vec![track("T0", 0, &[(0, BAR, None)])];
    for i in 1..40 {
        tracks.push(track(&format!("T{i}"), 1, &[]));
    }
    let (handle, view) = open(cx, project(tracks));
    let at = at_cell(cx, handle, &view, 1, BAR + 1300);
    mouse(cx, handle, at, MouseButton::Left);
    assert_eq!(current_index(cx, &view), Some(1));
    assert_eq!(cursor_tick(cx, &view), BAR + 960, "스텝(4분음표)으로 내림");
    // 휠: 세로 스크롤(트랙 행), `Shift`+휠: 가로
    let center = bounds_of(cx, handle, "main-view").center();
    let v0 = arr_vp(cx, &view);
    wheel(cx, handle, center, -120.0, Modifiers::none());
    let v1 = arr_vp(cx, &view);
    assert!(
        (v1.first_row - (v0.first_row + 120.0 / DEFAULT_ROW_HEIGHT)).abs() < 0.01,
        "{v0:?} → {v1:?}"
    );
    assert_eq!(v1.first_tick, v0.first_tick);
    wheel(
        cx,
        handle,
        center,
        -120.0,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    let v2 = arr_vp(cx, &view);
    assert_eq!(v2.first_row, v1.first_row);
    assert!(
        (v2.first_tick - (v1.first_tick + 120.0 / v1.px_per_tick)).abs() < 0.01,
        "{v1:?} → {v2:?}"
    );
    // 왼쪽 끝과 위쪽 끝을 넘지 않는다
    wheel(
        cx,
        handle,
        center,
        10_000.0,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    wheel(cx, handle, center, 10_000.0, Modifiers::none());
    let v3 = arr_vp(cx, &view);
    assert_eq!((v3.first_tick, v3.first_row), (0.0, 0.0));
}

// ---------- 17. 보이는 클립을 모두 한 번씩 그린다 ----------

#[gpui_kit::test]
fn element_paints_every_visible_clip_once(cx: &mut TestAppContext) {
    start(cx, "paint");
    let (handle, view) = open(cx, three_tracks());
    render(cx, handle);
    let state = arr(cx, &view);
    let (painted, expected) = (state.painted_clips(), state.scene().clips.len());
    assert_eq!(expected, 3);
    assert_eq!(painted, expected);
}

// ---------- 18. 재생 헤드 ----------

#[gpui_kit::test]
fn playhead_follows_transport_in_arrangement(cx: &mut TestAppContext) {
    start(cx, "playhead");
    let options = EngineOptions {
        client_name: format!("ks-arr-{}", std::process::id()),
        port_name: format!("KS arr {}", std::process::id()),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let (handle, view) = open(cx, three_tracks());
    let playhead = |cx: &mut TestAppContext| arr(cx, &view).scene().playhead;
    assert_eq!(playhead(cx), None, "정지면 없음");
    run(cx, handle, Box::new(transport::Play));
    std::thread::sleep(std::time::Duration::from_millis(120));
    render(cx, handle);
    let playing = playhead(cx).expect("재생 중이면 지금 자리");
    assert!(playing > 0.0, "{playing}");
    run(cx, handle, Box::new(transport::Pause));
    let paused = playhead(cx).expect("일시정지 자리");
    assert!(paused >= playing, "{paused} {playing}");
    render(cx, handle);
    assert_eq!(playhead(cx), Some(paused), "일시정지면 그 자리에 머문다");
    run(cx, handle, Box::new(transport::Stop));
    assert_eq!(playhead(cx), None);
}

// ---------- 19. 트랙 64 × 클립 200 프레임 예산(release) ----------

#[gpui_kit::test]
fn frame_with_64x200_clips_fits_60fps_budget_in_release(cx: &mut TestAppContext) {
    start(cx, "frame-64x200");
    let (handle, view) = open(cx, big_project(64, 200));
    // 하한 배율로 가로 전부 보이게
    cx.read(|cx| view.read(cx).arrangement().zoom_at(1e-6, 0.0));
    render(cx, handle);
    render(cx, handle);
    let state = arr(cx, &view);
    let (painted, v) = (state.painted_clips(), state.viewport());
    assert!(v.last_tick() >= (200 * BAR) as f64, "{v:?}");
    let rows = v.rows_visible().floor() as usize;
    assert!(
        painted >= rows * 200,
        "보이는 행의 클립이 모두 그려진다: {painted} < {rows} × 200"
    );
    let mut samples = Vec::new();
    for _ in 0..20 {
        let t0 = std::time::Instant::now();
        render(cx, handle);
        samples.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    samples.sort_by(f64::total_cmp);
    let median = samples[samples.len() / 2];
    let max = samples[samples.len() - 1];
    println!("frame_64x200_clips: median {median:.2} ms, max {max:.2} ms, painted {painted}");
    if !cfg!(debug_assertions) {
        assert!(median < 16.7, "release 프레임 중앙값 {median:.2} ms");
        assert!(max < 16.7, "release 프레임 최대 {max:.2} ms(지터 회귀)");
    }
}

// ---------- 20. 클릭 자리 → 칸 ----------

#[test]
fn cell_at_maps_click_point_to_track_row_and_step() {
    let v = vp(1000.0, 0.1, 500.0, 1.0, 120.0);
    // x 150 → 틱 2500 → 스텝 960 내림 1920, y 30 → 행 2
    assert_eq!(v.cell_at(150.0, 30.0, 960, 5), Some((2, Tick::new(1920))));
    assert_eq!(v.cell_at(0.0, 0.0, 960, 5), Some((1, Tick::new(960))));
    assert_eq!(v.cell_at(0.0, 0.0, 1, 5), Some((1, Tick::new(1000))));
    // 트랙 행 밖, 뷰포트 밖
    assert_eq!(v.cell_at(10.0, 60.0, 960, 3), None);
    assert_eq!(v.cell_at(-1.0, 10.0, 960, 5), None);
    assert_eq!(v.cell_at(10.0, 120.0, 960, 5), None);
    assert_eq!(v.cell_at(500.0, 10.0, 960, 5), None);
    assert_eq!(v.cell_at(10.0, 10.0, 960, 0), None);
}

#[test]
fn clip_selection_set_all_and_retain_existing() {
    let p = three_tracks();
    let mut sel = ClipSelection::default();
    sel.set_all(&p);
    assert_eq!(sel.len(), 3);
    let (t0, c0) = ids(&p, 0);
    assert!(sel.contains(t0, c0[0]));
    assert!(
        !sel.retain_existing(&p),
        "없어진 클립이 없으면 바뀌지 않는다"
    );
    let mut smaller = three_tracks();
    let _ = smaller;
    // 다른 프로젝트(다른 id)에 대조하면 모두 빠진다
    smaller = project(vec![track("X", 0, &[])]);
    assert!(sel.retain_existing(&smaller));
    assert!(sel.is_empty());
    let all: Vec<(TrackId, ClipId)> = {
        let mut s = ClipSelection::default();
        s.set_all(&p);
        s.iter().collect()
    };
    assert_eq!(all.len(), 3);
}
