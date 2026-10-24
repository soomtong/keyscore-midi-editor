//! S3-T1 적대적 리뷰 1회차 시험. 구현 파일은 고치지 않고 경계와 계획 항목을 깨뜨리려 한다.
//!
//! 순수 시험은 `scene::build`·`Viewport`·`PianoRollState`로, UI 시험은 `tests/pianoroll.rs`의 도우미 패턴을
//! 따른다. 실패하는 시험은 실패한 채로 둔다(그것이 산출물이다).
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::scene::{self, LineKind, Scene, SceneInput};
use keyscore_app::pianoroll::{
    Cursor, CursorMove, DEFAULT_PX_PER_TICK, MIN_PX_PER_TICK, MaxLenCache, PianoRollState, ROWS,
    Selection, Viewport,
};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, Key, MeterMap, Note, Pitch, Project, Tick, TimeSignature, Track, TrackColor,
    Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

// ---------------------------------------------------------------- 도우미

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv31-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext, project: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
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

fn pitch(value: u8) -> Pitch {
    Pitch::new(value).unwrap()
}

fn clip(start: u64, length: u64, notes: &[Note]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    clip
}

fn track(clips: Vec<Clip>) -> Track {
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    for c in clips {
        track.insert_clip(c).unwrap();
    }
    track
}

fn project_with(track: Track) -> Project {
    let mut project = Project::new();
    project.push_track(track).unwrap();
    project
}

fn viewport(
    first_tick: f64,
    px_per_tick: f64,
    width: f32,
    first_row: f32,
    height: f32,
) -> Viewport {
    Viewport {
        first_tick,
        px_per_tick,
        width,
        first_row,
        row_height: 12.0,
        height,
    }
}

fn build_with(
    track: Option<&Track>,
    meter: &MeterMap,
    cursor: Cursor,
    playhead: Option<Tick>,
    vp: Viewport,
) -> Scene {
    let mut cache = MaxLenCache::default();
    cache.refresh(track, 0);
    let current = track.and_then(|t| {
        t.clips()
            .find(|(_, c)| c.start().get() <= cursor.tick.get() && cursor.tick.get() < c.end())
            .map(|(id, _)| id)
    });
    let selection = Selection::default();
    let mut scene = Scene::default();
    scene::build(
        &SceneInput {
            track,
            current,
            cache: &cache,
            meter,
            key: Key::None,
            cursor,
            step_ticks: QUARTER,
            selection: &selection,
            playhead,
            viewport: vp,
        },
        &mut scene,
    );
    scene
}

fn c4_at(tick: u64) -> Cursor {
    Cursor {
        tick: Tick::new(tick),
        pitch: pitch(60),
    }
}

fn cursor_visible(vp: &Viewport, cursor: Cursor, step: u64) -> bool {
    let tick = cursor.tick.get() as f64;
    let row = Viewport::row_of(cursor.pitch);
    tick >= vp.first_tick
        && (tick + step as f64 <= vp.last_tick() + 0.001 || tick == vp.first_tick)
        && row >= vp.first_row
        && row + 1.0 <= vp.first_row + vp.height / vp.row_height + 0.001
}

fn cursor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Cursor {
    cx.read(|cx| view.read(cx).piano_roll().cursor())
}

fn vp(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Viewport {
    cx.read(|cx| view.read(cx).piano_roll_viewport())
}

fn main_view_bounds(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    cx.update_window(handle, |_, window, _| window.find("main-view").bounds())
        .unwrap()
}

// ---------------------------------------------------------------- 순수: 장면 경계

/// 박자표가 마디 중간(틱 2400, 4/4의 셋째 박 중간)에서 3/4로 바뀌는 SMF형 맵. `MeterMap`은 짧은 마디를
/// 한 마디로 세므로 마디 선은 0·2400·5280·8160, 첫 마디의 박 선은 960·1920에서 끊겨야 한다.
#[test]
fn scene_mid_bar_meter_change_cuts_beats_and_starts_new_bar() {
    let mut meter = MeterMap::new(TimeSignature::COMMON);
    meter.set(Tick::new(2400), TimeSignature::new(3, 4).unwrap());
    let t = track(vec![clip(0, 4 * BAR, &[note(0, 960, 60)])]);
    let scene = build_with(
        Some(&t),
        &meter,
        c4_at(0),
        None,
        viewport(0.0, 0.1, 1000.0, 55.0, 100.0),
    );
    let bars: Vec<u64> = scene
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Bar)
        .map(|l| l.tick.get())
        .collect();
    assert_eq!(bars, vec![0, 2400, 5280, 8160]);
    let beats: Vec<u64> = scene
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Beat)
        .map(|l| l.tick.get())
        .collect();
    assert_eq!(
        beats,
        vec![960, 1920, 3360, 4320, 6240, 7200, 9120],
        "짧은 마디의 셋째 박(2880)은 다음 마디 시작(2400) 뒤라 없다"
    );
    let ticks: Vec<u64> = scene.lines.iter().map(|l| l.tick.get()).collect();
    assert!(
        ticks.windows(2).all(|w| w[0] < w[1]),
        "틱 오름차순 {ticks:?}"
    );
}

/// 커서와 뷰포트가 u64 끝에 있어도 패닉 없이 장면이 나온다(`tick_u` 포화, `bar_start` 넘침 → `None`).
#[test]
fn scene_at_tick_max_does_not_panic() {
    let meter = MeterMap::default();
    let t = track(vec![clip(0, 4 * BAR, &[note(0, 960, 60)])]);
    let first = u64::MAX as f64 - 100_000.0;
    let scene = build_with(
        Some(&t),
        &meter,
        c4_at(u64::MAX),
        Some(Tick::new(u64::MAX)),
        viewport(first, 0.1, 1000.0, 55.0, 100.0),
    );
    assert!(scene.notes.is_empty());
    assert!(scene.lines.iter().all(|l| l.x >= 0.0 && l.x < 1000.0));
    let ticks: Vec<u64> = scene.lines.iter().map(|l| l.tick.get()).collect();
    assert!(ticks.windows(2).all(|w| w[0] < w[1]), "{ticks:?}");
    // 뷰포트 첫 틱이 u64 범위를 넘어도(스크롤 공개 API로 가능) 패닉하지 않는다
    let scene = build_with(
        Some(&t),
        &meter,
        c4_at(0),
        None,
        viewport(1.0e30, 0.1, 1000.0, 55.0, 100.0),
    );
    assert!(scene.notes.is_empty() && scene.cursor.is_none());
}

/// 음높이 0·127 노트는 가장 아래·위 행에 그려지고 행 배경도 128줄 모두 나온다.
#[test]
fn scene_draws_pitch_0_and_127_in_outer_rows() {
    let meter = MeterMap::default();
    let t = track(vec![clip(
        0,
        BAR,
        &[note(0, 960, 127), note(960, 960, 0), note(1920, 960, 64)],
    )]);
    let vp = viewport(0.0, 0.1, 1000.0, 0.0, 128.0 * 12.0);
    let scene = build_with(Some(&t), &meter, c4_at(0), None, vp);
    assert_eq!(scene.rows.len(), 128);
    assert_eq!(scene.rows[0].pitch.get(), 127);
    assert_eq!(scene.rows[127].pitch.get(), 0);
    assert_eq!(scene.notes.len(), 3, "{:?}", scene.notes);
    let top = scene.notes.iter().find(|n| n.rect.x == 0.0).unwrap();
    assert_eq!(top.rect.y, 0.0);
    let bottom = scene.notes.iter().find(|n| n.rect.x == 96.0).unwrap();
    assert_eq!(bottom.rect.y, 127.0 * 12.0);
    // 위쪽 행이 반만 보이면(소수 first_row) 127 노트는 y가 음수지만 여전히 들어간다
    let half = viewport(0.0, 0.1, 1000.0, 0.5, 100.0);
    let scene = build_with(Some(&t), &meter, c4_at(0), None, half);
    assert!(
        scene.notes.iter().any(|n| n.rect.y == -6.0),
        "{:?}",
        scene.notes
    );
    // 아래 끝: 행 127만 반 보이는 뷰포트에서도 음높이 0이 들어간다
    let tail = viewport(0.0, 0.1, 1000.0, 126.5, 12.0);
    let scene = build_with(Some(&t), &meter, c4_at(0), None, tail);
    assert_eq!(scene.notes.len(), 1, "{:?}", scene.notes);
    assert_eq!(scene.rows.last().map(|r| r.pitch.get()), Some(0));
}

/// 뷰포트 크기 0·배율 0이면 빈 장면이고 `cell_at`은 `None`.
#[test]
fn scene_zero_viewport_is_empty_and_cell_at_is_none() {
    let meter = MeterMap::default();
    let t = track(vec![clip(0, BAR, &[note(0, 960, 60)])]);
    for vp in [
        viewport(0.0, 0.1, 0.0, 55.0, 100.0),
        viewport(0.0, 0.1, 100.0, 55.0, 0.0),
        viewport(0.0, 0.0, 100.0, 55.0, 100.0),
    ] {
        let scene = build_with(Some(&t), &meter, c4_at(0), Some(Tick::ZERO), vp);
        assert_eq!(scene, Scene::default(), "{vp:?}");
        // 배율 0은 줌 하한 때문에 공개 API로 못 만든다. 크기 0만 `cell_at`을 본다
        if vp.px_per_tick > 0.0 {
            assert!(vp.cell_at(0.0, 0.0).is_none(), "{vp:?}");
        }
    }
}

/// 트랙이 없거나(프로젝트에 트랙 없음) 클립이 없는 트랙이면 격자만 있고 노트는 없다.
#[test]
fn scene_without_track_or_clips_has_grid_but_no_notes() {
    let meter = MeterMap::default();
    // 행 60–68.3이 보인다(C4는 행 67)
    let vp = viewport(0.0, 0.1, 1000.0, 60.0, 100.0);
    let scene = build_with(None, &meter, c4_at(0), None, vp);
    assert!(!scene.rows.is_empty() && !scene.lines.is_empty());
    assert!(scene.notes.is_empty());
    assert!(scene.cursor.is_some());
    let empty = track(vec![]);
    let scene = build_with(Some(&empty), &meter, c4_at(0), None, vp);
    assert!(scene.notes.is_empty());
    assert!(scene.cursor.is_some());
}

/// 같은 음높이의 긴 노트가 뒤 노트와 겹쳐도(겹침은 모델이 허용) 둘 다 그려지고 왼쪽 경계에 걸린 긴 노트가
/// 빠지지 않는다.
#[test]
fn scene_overlapping_long_notes_both_drawn_across_left_edge() {
    let meter = MeterMap::default();
    let t = track(vec![clip(
        0,
        8 * BAR,
        &[
            note(0, 6 * BAR as u32, 60),
            note(960, 960, 60),
            note(5 * BAR, 960, 60),
        ],
    )]);
    // 4마디 반부터 보인다: 긴 노트(0–6마디)와 5마디 노트만
    let vp = viewport(4.5 * BAR as f64, 0.1, 400.0, 55.0, 200.0);
    let scene = build_with(Some(&t), &meter, c4_at(0), None, vp);
    assert_eq!(scene.notes.len(), 2, "{:?}", scene.notes);
    assert!(scene.notes.iter().any(|n| n.rect.x < 0.0));
}

/// 캐시 키가 (revision, TrackId)뿐이라 revision이 그대로인 채 트랙이 바뀌면 긴 노트가 빠진다. 지금은
/// `ProjectDocument`가 `project_mut`을 내지 않아 도달하지 않지만 캐시 계약이 이것임을 적어 둔다.
#[test]
fn max_len_cache_same_revision_hides_note_lengthened_later() {
    let meter = MeterMap::default();
    let mut t = track(vec![clip(0, 8 * BAR, &[note(0, 960, 60)])]);
    let mut cache = MaxLenCache::default();
    cache.refresh(Some(&t), 7);
    let id = t.clips().next().unwrap().0;
    t.insert_note(id, note(0, 6 * BAR as u32, 62)).unwrap();
    cache.refresh(Some(&t), 7);
    let vp = viewport(4.5 * BAR as f64, 0.1, 400.0, 55.0, 200.0);
    let selection = Selection::default();
    let mut scene = Scene::default();
    scene::build(
        &SceneInput {
            track: Some(&t),
            current: Some(id),
            cache: &cache,
            meter: &meter,
            key: Key::None,
            cursor: c4_at(0),
            step_ticks: QUARTER,
            selection: &selection,
            playhead: None,
            viewport: vp,
        },
        &mut scene,
    );
    assert!(
        scene.notes.is_empty(),
        "낡은 캐시(최대 길이 960)로는 6마디 노트를 건너뛴다: {:?}",
        scene.notes
    );
    cache.refresh(Some(&t), 8);
    scene::build(
        &SceneInput {
            track: Some(&t),
            current: Some(id),
            cache: &cache,
            meter: &meter,
            key: Key::None,
            cursor: c4_at(0),
            step_ticks: QUARTER,
            selection: &selection,
            playhead: None,
            viewport: vp,
        },
        &mut scene,
    );
    assert_eq!(scene.notes.len(), 1);
}

// ---------------------------------------------------------------- 순수: 커서 상태

/// u64 끝에서 스텝·마디 앞으로는 제자리, 뒤로는 돌아온다.
#[test]
fn state_moves_at_tick_max_do_not_overflow() {
    let meter = MeterMap::default();
    let state = PianoRollState::new();
    state.set_cursor(Cursor {
        tick: Tick::new(u64::MAX),
        pitch: pitch(127),
    });
    state.move_cursor(CursorMove::StepForward, &meter);
    assert_eq!(state.cursor().tick.get(), u64::MAX);
    state.move_cursor(CursorMove::BarForward, &meter);
    assert_eq!(
        state.cursor().tick.get(),
        u64::MAX,
        "다음 마디가 없으면 제자리"
    );
    state.move_cursor(CursorMove::OctaveUp, &meter);
    assert_eq!(state.cursor().pitch.get(), 127);
    state.move_cursor(CursorMove::BarBack, &meter);
    let bar_start = meter
        .bar_start(meter.position(Tick::new(u64::MAX)).bar())
        .unwrap();
    assert_eq!(state.cursor().tick, bar_start);
    state.move_cursor(CursorMove::StepBack, &meter);
    assert_eq!(state.cursor().tick.get(), bar_start.get() - QUARTER);
    // 크기를 모를 때의 ensure/center는 무시되고, zoom은 커서 x를 지키느라 첫 틱을 옮기지만 패닉은 없다
    state.ensure_cursor_visible();
    state.center_cursor();
    state.zoom(2.0);
    state.zoom_reset();
    let first = state.viewport().first_tick;
    assert!(first.is_finite() && first >= 0.0, "{first}");
}

/// 마디 중간 박자표 변화 앞뒤로 `Ctrl+h`/`Ctrl+l`이 짧은 마디를 한 마디로 센다.
#[test]
fn state_bar_moves_follow_mid_bar_meter_change() {
    let mut meter = MeterMap::new(TimeSignature::COMMON);
    meter.set(Tick::new(2400), TimeSignature::new(3, 4).unwrap());
    let state = PianoRollState::new();
    state.set_cursor(c4_at(2500));
    state.move_cursor(CursorMove::BarBack, &meter);
    assert_eq!(
        state.cursor().tick.get(),
        2400,
        "마디 중간이면 그 마디 시작"
    );
    state.move_cursor(CursorMove::BarBack, &meter);
    assert_eq!(state.cursor().tick.get(), 0);
    state.move_cursor(CursorMove::BarBack, &meter);
    assert_eq!(state.cursor().tick.get(), 0, "0 아래로 가지 않는다");
    state.move_cursor(CursorMove::BarForward, &meter);
    assert_eq!(state.cursor().tick.get(), 2400);
    state.move_cursor(CursorMove::BarForward, &meter);
    assert_eq!(state.cursor().tick.get(), 2400 + 3 * QUARTER);
}

/// `cell_at` 경계: 마지막 행의 마지막 픽셀은 음높이 0, 행 128부터는 `None`, 소수 `first_row`는 내림.
#[test]
fn cell_at_bottom_row_and_fractional_first_row() {
    let full = viewport(0.0, 0.1, 100.0, 0.0, 128.0 * 12.0);
    assert_eq!(
        full.cell_at(0.0, 128.0 * 12.0 - 0.5).map(|c| c.pitch.get()),
        Some(0)
    );
    assert!(full.cell_at(0.0, 128.0 * 12.0).is_none());
    // 보이는 범위가 행 128을 넘는 뷰포트(세로가 128행보다 큼)는 행 밖에서 None
    let tall = viewport(0.0, 0.1, 100.0, 0.0, 200.0 * 12.0);
    assert!(tall.cell_at(0.0, 150.0 * 12.0).is_none());
    assert_eq!(
        tall.cell_at(0.0, 127.9 * 12.0).map(|c| c.pitch.get()),
        Some(0)
    );
    let half = viewport(0.0, 0.1, 100.0, 0.5, 100.0);
    assert_eq!(half.cell_at(0.0, 0.0).map(|c| c.pitch.get()), Some(127));
    assert_eq!(half.cell_at(0.0, 6.0).map(|c| c.pitch.get()), Some(126));
    // 가로: 마지막 픽셀은 안, 너비는 밖. 틱은 내림
    assert_eq!(full.cell_at(99.9, 0.0).map(|c| c.tick.get()), Some(999));
    assert!(full.cell_at(100.0, 0.0).is_none());
}

// ---------------------------------------------------------------- UI

/// 계획 2 「피아노롤로 들어갈 때 ... 첫 클립 시작으로 옮긴다」와 「커서가 뷰포트 밖으로 가면 뷰포트가
/// 따라간다」를 합치면, 먼 클립으로 들어간 직후 커서가 보여야 한다. 피아노롤이 한 번도 그려지지 않은
/// 상태에서는 크기를 몰라 `ensure_cursor_visible`이 무시되고, 첫 그리기는 세로만 가운데로 맞춘다.
#[gpui_kit::test]
fn entering_piano_roll_from_far_clip_scrolls_viewport_to_cursor(cx: &mut TestAppContext) {
    start(cx, "far-enter");
    let far = 50 * BAR;
    let (handle, view) = open(
        cx,
        project_with(track(vec![clip(far, BAR, &[note(0, 960, 60)])])),
    );
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    let c = cursor_of(cx, &view);
    assert_eq!((c.tick.get(), c.pitch.get()), (far, 60));
    let v = vp(cx, &view);
    assert!(v.width > 0.0 && v.height > 0.0, "{v:?}");
    assert!(
        cursor_visible(&v, c, QUARTER),
        "들어간 직후 커서가 뷰포트 밖: {v:?} {c:?}"
    );
}

/// 같은 상황에서 커서 키를 한 번 누르면 따라온다(키 경로 자체는 맞다는 대조).
#[gpui_kit::test]
fn cursor_key_after_far_entry_brings_cursor_into_view(cx: &mut TestAppContext) {
    start(cx, "far-enter-key");
    let far = 50 * BAR;
    let (handle, view) = open(
        cx,
        project_with(track(vec![clip(far, BAR, &[note(0, 960, 60)])])),
    );
    press(cx, handle, "enter l");
    let c = cursor_of(cx, &view);
    assert_eq!(c.tick.get(), far + QUARTER);
    let v = vp(cx, &view);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
}

/// `v`로 들어가도 같은 규칙(계획 2: `clip::Open`, `view::Toggle`).
#[gpui_kit::test]
fn toggling_view_into_far_clip_scrolls_viewport_to_cursor(cx: &mut TestAppContext) {
    start(cx, "far-toggle");
    let far = 50 * BAR;
    let (handle, view) = open(
        cx,
        project_with(track(vec![clip(far, BAR, &[note(0, 960, 60)])])),
    );
    press(cx, handle, "v");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    let c = cursor_of(cx, &view);
    assert_eq!(c.tick.get(), far);
    let v = vp(cx, &view);
    assert!(
        cursor_visible(&v, c, QUARTER),
        "들어간 직후 커서가 뷰포트 밖: {v:?} {c:?}"
    );
}

/// 줌 아웃 하한에서 멈추고 커서는 보인다. 되돌리면 기본 배율.
#[gpui_kit::test]
fn zoom_out_stops_at_floor_and_keeps_cursor_visible(cx: &mut TestAppContext) {
    start(cx, "zoom-floor");
    let (handle, view) = open(
        cx,
        project_with(track(vec![clip(0, 4 * BAR, &[note(0, 960, 60)])])),
    );
    press(cx, handle, "enter ctrl-l ctrl-l ctrl-l");
    for _ in 0..60 {
        press(cx, handle, "cmd--");
    }
    let v = vp(cx, &view);
    assert_eq!(v.px_per_tick, MIN_PX_PER_TICK, "{v:?}");
    let c = cursor_of(cx, &view);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
    press(cx, handle, "cmd-0");
    let v = vp(cx, &view);
    assert_eq!(v.px_per_tick, DEFAULT_PX_PER_TICK);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
}

fn wheel(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: gpui_kit::Point<gpui_kit::Pixels>,
    dy: f32,
    shift: bool,
) {
    let mut vcx = gpui_kit::VisualTestContext::from_window(handle, cx);
    vcx.simulate_event(gpui_kit::ScrollWheelEvent {
        position: at,
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(dy))),
        modifiers: gpui_kit::Modifiers {
            shift,
            ..gpui_kit::Modifiers::none()
        },
        touch_phase: gpui_kit::TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
}

/// 키를 누른 뒤 포인터를 움직이지 않고 휠을 굴리면(트랙패드에서 흔한 순서) 스크롤돼야 한다. 휠 처리기가
/// `Hitbox::is_hovered`를 쓰는데, GPUI는 `ScrollWheel`에서 입력 모달리티를 바꾸지 않아 키보드 모달리티가
/// 남아 있으면 `is_hovered`가 거짓이다(`Div`는 그래서 `should_handle_scroll`을 쓴다).
#[gpui_kit::test]
fn wheel_after_keyboard_input_without_pointer_move_scrolls(cx: &mut TestAppContext) {
    start(cx, "wheel-after-keys");
    let (handle, view) = open(
        cx,
        project_with(track(vec![clip(0, 4 * BAR, &[note(0, 960, 60)])])),
    );
    press(cx, handle, "enter");
    let before = vp(cx, &view);
    let at = main_view_bounds(cx, handle).center();
    wheel(cx, handle, at, -120.0, false);
    let down = vp(cx, &view);
    assert!(
        (down.first_row - (before.first_row + 10.0)).abs() < 0.01,
        "키 입력 뒤 첫 휠이 무시된다: {before:?} → {down:?}"
    );
}

/// 포인터를 움직인 뒤의 휠: 세로 델타는 행, `Shift`+휠은 가로, 왼쪽 끝은 0틱. 자연 스크롤이라 내용이 위로
/// 가면(델타 음수) 뷰포트 첫 행이 는다.
#[gpui_kit::test]
fn wheel_after_pointer_move_scrolls_rows_and_shift_wheel_scrolls_ticks(cx: &mut TestAppContext) {
    start(cx, "wheel-after-move");
    let (handle, view) = open(
        cx,
        project_with(track(vec![clip(0, 4 * BAR, &[note(0, 960, 60)])])),
    );
    press(cx, handle, "enter");
    let before = vp(cx, &view);
    let at = main_view_bounds(cx, handle).center();
    gpui_kit::VisualTestContext::from_window(handle, cx).simulate_mouse_move(
        at,
        None,
        gpui_kit::Modifiers::none(),
    );
    cx.run_until_parked();
    render(cx, handle);
    wheel(cx, handle, at, -120.0, false);
    let down = vp(cx, &view);
    assert_eq!(
        down.first_tick, before.first_tick,
        "세로 휠은 가로를 안 건드린다"
    );
    assert!(
        (down.first_row - (before.first_row + 10.0)).abs() < 0.01
            || down.first_row == f32::from(ROWS) - down.height / down.row_height,
        "내용이 위로 가면 첫 행이 는다: {before:?} → {down:?}"
    );
    wheel(cx, handle, at, -120.0, true);
    let right = vp(cx, &view);
    assert_eq!(
        right.first_row, down.first_row,
        "Shift+휠은 세로를 안 건드린다"
    );
    assert!(
        (right.first_tick - (down.first_tick + 120.0 / down.px_per_tick)).abs() < 0.01,
        "{down:?} → {right:?}"
    );
    wheel(cx, handle, at, 1.0e6, true);
    assert_eq!(vp(cx, &view).first_tick, 0.0, "왼쪽 끝은 0틱");
}

/// 클릭은 그 칸으로 커서를 옮기고 뷰포트는 움직이지 않는다(계획 3). 좌표는 요소 원점 기준이어야 한다.
#[gpui_kit::test]
fn click_moves_cursor_to_cell_under_pointer(cx: &mut TestAppContext) {
    start(cx, "click");
    let (handle, view) = open(
        cx,
        project_with(track(vec![clip(0, 4 * BAR, &[note(0, 960, 60)])])),
    );
    press(cx, handle, "enter");
    let before = vp(cx, &view);
    let bounds = main_view_bounds(cx, handle);
    let local = gpui_kit::point(gpui_kit::px(150.0), gpui_kit::px(30.0));
    let expected = before.cell_at(150.0, 30.0).expect("뷰포트 안");
    gpui_kit::VisualTestContext::from_window(handle, cx)
        .simulate_click(bounds.origin + local, gpui_kit::Modifiers::none());
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(cursor_of(cx, &view), expected);
    assert_eq!(vp(cx, &view), before, "클릭은 스크롤하지 않는다");
    // 오른쪽 아래 구석도 칸이다. 가장자리 1 px는 패널 분할 손잡이·테두리 히트박스가 가리므로(S2 레이아웃)
    // 12 px 안쪽을 누른다(가장자리 4 px 띠가 분할 손잡이 히트박스에 가린다)
    let edge = gpui_kit::point(
        bounds.size.width - gpui_kit::px(12.0),
        bounds.size.height - gpui_kit::px(12.0),
    );
    let expected = before
        .cell_at(f32::from(edge.x), f32::from(edge.y))
        .expect("뷰포트 안");
    gpui_kit::VisualTestContext::from_window(handle, cx)
        .simulate_click(bounds.origin + edge, gpui_kit::Modifiers::none());
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(cursor_of(cx, &view), expected);
}

/// 휠로 커서를 화면 밖에 두었다가 커서 키를 누르면 뷰포트가 커서로 돌아온다.
#[gpui_kit::test]
fn cursor_key_after_scrolling_away_returns_to_cursor(cx: &mut TestAppContext) {
    start(cx, "scroll-away");
    let (handle, view) = open(
        cx,
        project_with(track(vec![clip(0, 4 * BAR, &[note(0, 960, 60)])])),
    );
    press(cx, handle, "enter");
    cx.read(|cx| view.read(cx).piano_roll().scroll_by(50_000.0, 600.0));
    let away = vp(cx, &view);
    let c = cursor_of(cx, &view);
    assert!(!cursor_visible(&away, c, QUARTER), "{away:?}");
    press(cx, handle, "l");
    let back = vp(cx, &view);
    let c = cursor_of(cx, &view);
    assert!(cursor_visible(&back, c, QUARTER), "{back:?} {c:?}");
}

/// 트랙이 없는 프로젝트에서 모든 커서·줌 키가 패닉 없이 동작한다.
#[gpui_kit::test]
fn keys_on_project_without_tracks_do_not_panic(cx: &mut TestAppContext) {
    start(cx, "no-tracks");
    let (handle, view) = open(cx, Project::new());
    press(
        cx,
        handle,
        "enter l l ctrl-l k ctrl-k ] cmd-= cmd-= cmd-- cmd-0 ctrl-h h [ ctrl-j j",
    );
    // l l → 1920, ctrl-l → 3840, k ctrl-k ] → 85, ctrl-h → 0(마디 시작이면 앞 마디), h → 0, [ ctrl-j j → 60
    let c = cursor_of(cx, &view);
    assert_eq!((c.tick.get(), c.pitch.get()), (0, 60), "{c:?}");
    let v = vp(cx, &view);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(keyscore_app::actions::view::CenterCursor), cx);
    })
    .unwrap();
    render(cx, handle);
}
