//! S3-T1 피아노롤 Element(REQ 5.3, 5.7, 5.8, UI-3, UI-4). 장면 계산은 순수 함수로, 커서·줌·뷰포트 따라가기는
//! 헤드리스 창에서, 재생 헤드는 같은 프로세스의 CoreMIDI 엔진으로 본다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::scene::{self, LineKind, Scene, SceneInput, Shade};
use keyscore_app::pianoroll::{
    Cursor, DEFAULT_PX_PER_TICK, MAX_PX_PER_TICK, MaxLenCache, Selection, Viewport, ZOOM_FACTOR,
};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, Key, MeterMap, Note, Pitch, PitchClass, Project, Tick, TimeSignature, Track,
    TrackColor, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-pr-{}-{name}", std::process::id()));
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

/// 클립 A(0–4마디): C4 D4 E4 … 끝에 F4. 클립 B(4마디부터 1마디): G4.
fn track_with_two_clips() -> Track {
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track
        .insert_clip(clip(
            0,
            4 * BAR,
            &[
                note(0, 960, 60),
                note(960, 960, 62),
                note(1920, 480, 64),
                note(4 * BAR - 960, 960, 65),
            ],
        ))
        .unwrap();
    track
        .insert_clip(clip(4 * BAR, BAR, &[note(0, 960, 67)]))
        .unwrap();
    track
}

fn project_two_clips() -> Project {
    let mut project = Project::new();
    project.push_track(track_with_two_clips()).unwrap();
    project
}

fn viewport(first_tick: f64, px_per_tick: f64, width: f32, top_pitch: u8, height: f32) -> Viewport {
    Viewport {
        first_tick,
        px_per_tick,
        width,
        first_row: Viewport::row_of(pitch(top_pitch)),
        row_height: 12.0,
        height,
    }
}

fn build(track: &Track, current: Option<usize>, key: Key, meter: &MeterMap, vp: Viewport) -> Scene {
    let mut cache = MaxLenCache::default();
    cache.refresh(Some(track), 0);
    let current = current.map(|i| track.clips().nth(i).unwrap().0);
    let selection = Selection::default();
    let mut scene = Scene::default();
    scene::build(
        &SceneInput {
            track: Some(track),
            current,
            cache: &cache,
            meter,
            key,
            cursor: Cursor {
                tick: Tick::new(1920),
                pitch: pitch(60),
            },
            step_ticks: QUARTER,
            selection: &selection,
            playhead: Some(Tick::new(3000)),
            viewport: vp,
        },
        &mut scene,
    );
    scene
}

fn cursor_visible(vp: &Viewport, cursor: Cursor, step: u64) -> bool {
    let tick = cursor.tick.get() as f64;
    let row = Viewport::row_of(cursor.pitch);
    tick >= vp.first_tick
        && tick + step as f64 <= vp.last_tick() + 0.001
        && row >= vp.first_row
        && row + 1.0 <= vp.first_row + vp.height / vp.row_height + 0.001
}

fn cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (u64, u8) {
    cx.read(|cx| {
        let c = view.read(cx).piano_roll().cursor();
        (c.tick.get(), c.pitch.get())
    })
}

fn vp(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Viewport {
    cx.read(|cx| view.read(cx).piano_roll_viewport())
}

// ---------- 1. 장면: 현재 클립과 이웃 클립 ----------

#[test]
fn scene_shows_current_clip_notes_and_dims_neighbors() {
    let track = track_with_two_clips();
    let meter = MeterMap::default();
    // 20,000틱 × 음높이 72…39가 보인다.
    let scene = build(
        &track,
        Some(0),
        Key::None,
        &meter,
        viewport(0.0, 0.1, 2000.0, 72, 400.0),
    );
    assert_eq!(scene.notes.len(), 5, "{:?}", scene.notes);
    let own: Vec<_> = scene.notes.iter().filter(|n| !n.dim).collect();
    let dim: Vec<_> = scene.notes.iter().filter(|n| n.dim).collect();
    assert_eq!(own.len(), 4);
    assert_eq!(dim.len(), 1);
    // C4(60)은 행 67, 위 행이 55(72)이므로 y = 12 × 12
    let c4 = own.iter().find(|n| n.rect.x == 0.0).expect("C4");
    assert_eq!(c4.rect.y, 144.0);
    assert_eq!(c4.rect.w, 95.0, "길이 960틱 × 0.1 − 1 px 틈");
    assert_eq!(c4.rect.h, 11.0, "행 높이 − 1 px 틈");
    assert_eq!(c4.velocity, 100);
    assert!(!c4.selected);
    // 클립 B의 G4는 트랙 기준 15,360틱
    assert_eq!(dim[0].rect.x, 1536.0);
    assert_eq!(dim[0].rect.y, (67.0 - 7.0 - 55.0) * 12.0);
    // 현재 클립이 B이면 흐림이 뒤집힌다
    let scene = build(
        &track,
        Some(1),
        Key::None,
        &meter,
        viewport(0.0, 0.1, 2000.0, 72, 400.0),
    );
    assert_eq!(scene.notes.iter().filter(|n| n.dim).count(), 4);
    // 현재 클립이 없으면 모두 흐리다
    let scene = build(
        &track,
        None,
        Key::None,
        &meter,
        viewport(0.0, 0.1, 2000.0, 72, 400.0),
    );
    assert_eq!(scene.notes.iter().filter(|n| n.dim).count(), 5);
}

// ---------- 2. 장면: 화면 밖 노트는 정렬 탐색으로 건너뛴다 ----------

#[test]
fn scene_skips_offscreen_notes_with_sorted_search() {
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut notes = Vec::new();
    // 첫 노트는 아주 길어 왼쪽 경계에 걸린다(최대 길이 캐시가 없으면 빠진다)
    notes.push(note(0, 2_000_000, 30));
    for i in 0..10_000u64 {
        notes.push(note(i * 120, 240, 24 + (i % 72) as u8));
    }
    track.insert_clip(clip(0, 3_000_000, &notes)).unwrap();
    let meter = MeterMap::default();
    // 600,000–612,000틱, 모든 행
    let vp = viewport(600_000.0, 0.1, 1200.0, 127, 128.0 * 12.0);
    let scene = build(&track, Some(0), Key::None, &meter, vp);
    let expected: Vec<&Note> = notes
        .iter()
        .filter(|n| {
            let start = n.start().get() as f64;
            let end = n.end() as f64;
            end > vp.first_tick && start < vp.last_tick()
        })
        .collect();
    assert_eq!(scene.notes.len(), expected.len());
    assert!(
        scene.notes.iter().any(|n| n.rect.x < 0.0),
        "긴 노트는 왼쪽 밖에서 시작한다"
    );
    // 행 범위 밖은 빠진다: 음높이 60–72만 보이는 뷰포트
    let narrow = viewport(0.0, 0.1, 1200.0, 72, 13.0 * 12.0);
    let scene = build(&track, Some(0), Key::None, &meter, narrow);
    let expected = notes
        .iter()
        .filter(|n| {
            let p = n.pitch().get();
            (60..=72).contains(&p) && (n.start().get() as f64) < narrow.last_tick()
        })
        .count();
    assert_eq!(scene.notes.len(), expected);
}

// ---------- 3. 장면: 행 명암 ----------

#[test]
fn scene_rows_shade_in_key_pitches() {
    let f_major = Key::Major(PitchClass::new(5).unwrap());
    let a_minor = Key::Minor(PitchClass::new(9).unwrap());
    for (key, p, shade) in [
        (f_major, 60, Shade::Light),
        (f_major, 61, Shade::Dark),
        (f_major, 64, Shade::Light),
        (f_major, 65, Shade::Light),
        (f_major, 66, Shade::Dark),
        (f_major, 70, Shade::Light),
        (f_major, 71, Shade::Dark),
        (a_minor, 60, Shade::Light),
        (a_minor, 61, Shade::Dark),
        (a_minor, 67, Shade::Light),
        (a_minor, 68, Shade::Dark),
        (Key::None, 60, Shade::Light),
        (Key::None, 61, Shade::Dark),
        (Key::None, 64, Shade::Light),
        (Key::None, 65, Shade::Light),
    ] {
        assert_eq!(scene::row_shade(key, pitch(p)), shade, "{key:?} {p}");
    }
    let track = track_with_two_clips();
    let meter = MeterMap::default();
    // 음높이 72…61(12행)이 보인다
    let scene = build(
        &track,
        Some(0),
        f_major,
        &meter,
        viewport(0.0, 0.1, 100.0, 72, 144.0),
    );
    let pitches: Vec<u8> = scene.rows.iter().map(|r| r.pitch.get()).collect();
    assert_eq!(pitches, (61..=72).rev().collect::<Vec<_>>());
    let top = &scene.rows[0];
    assert_eq!(top.rect.y, 0.0);
    assert_eq!(top.rect.h, 12.0);
    assert_eq!(top.rect.w, 100.0);
    assert_eq!(top.shade, Shade::Light, "C5는 F장조 안");
    assert_eq!(scene.rows[1].shade, Shade::Dark, "B4는 F장조 밖");
}

// ---------- 4. 장면: 격자 선 ----------

#[test]
fn scene_grid_lines_follow_meter_map() {
    let mut meter = MeterMap::new(TimeSignature::COMMON);
    meter.set(Tick::new(2 * BAR), TimeSignature::new(3, 4).unwrap());
    let track = track_with_two_clips();
    // 0–12,000틱
    let scene = build(
        &track,
        Some(0),
        Key::None,
        &meter,
        viewport(0.0, 0.1, 1200.0, 72, 100.0),
    );
    let bars: Vec<u64> = scene
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Bar)
        .map(|l| l.tick.get())
        .collect();
    assert_eq!(bars, vec![0, 3840, 7680, 10560]);
    let beats: Vec<u64> = scene
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Beat)
        .map(|l| l.tick.get())
        .collect();
    assert_eq!(
        beats,
        vec![960, 1920, 2880, 4800, 5760, 6720, 8640, 9600, 11520]
    );
    let xs: Vec<f32> = scene.lines.iter().map(|l| l.x).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "x 오름차순");
    assert_eq!(scene.lines[1].x, 96.0);
    // 왼쪽 끝이 마디 중간이면 그 앞 선은 없다
    let scene = build(
        &track,
        Some(0),
        Key::None,
        &meter,
        viewport(5000.0, 0.1, 500.0, 72, 100.0),
    );
    let ticks: Vec<u64> = scene.lines.iter().map(|l| l.tick.get()).collect();
    assert_eq!(ticks, vec![5760, 6720, 7680, 8640, 9600]);
}

// ---------- 5. 장면: 커서 칸과 재생 헤드 ----------

#[test]
fn scene_playhead_and_cursor_positions() {
    let track = track_with_two_clips();
    let meter = MeterMap::default();
    let scene = build(
        &track,
        Some(0),
        Key::None,
        &meter,
        viewport(0.0, 0.1, 2000.0, 72, 400.0),
    );
    let cursor = scene.cursor.expect("커서는 뷰포트 안");
    assert_eq!(
        (cursor.x, cursor.y, cursor.w, cursor.h),
        (192.0, 144.0, 96.0, 12.0)
    );
    assert_eq!(scene.playhead, Some(300.0));
    // 뷰포트 밖
    let scene = build(
        &track,
        Some(0),
        Key::None,
        &meter,
        viewport(5000.0, 0.1, 500.0, 72, 400.0),
    );
    assert_eq!(scene.cursor, None);
    assert_eq!(scene.playhead, None);
    // 세로로 밖이어도 커서는 없다
    let scene = build(
        &track,
        Some(0),
        Key::None,
        &meter,
        viewport(0.0, 0.1, 2000.0, 40, 100.0),
    );
    assert_eq!(scene.cursor, None);
}

// ---------- 6. 커서 키 ----------

#[gpui_kit::test]
fn cursor_keys_move_by_step_semitone_bar_octave(cx: &mut TestAppContext) {
    start(cx, "cursor-keys");
    let (handle, view) = open(cx, project_two_clips());
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    assert_eq!(cursor(cx, &view), (0, 60));
    press(cx, handle, "l");
    assert_eq!(cursor(cx, &view), (960, 60));
    press(cx, handle, "right");
    assert_eq!(cursor(cx, &view), (1920, 60));
    press(cx, handle, "h h h");
    assert_eq!(cursor(cx, &view), (0, 60), "0틱 아래로 가지 않는다");
    press(cx, handle, "k");
    assert_eq!(cursor(cx, &view), (0, 61));
    press(cx, handle, "j j down");
    assert_eq!(cursor(cx, &view), (0, 58));
    press(cx, handle, "ctrl-l ctrl-l");
    assert_eq!(cursor(cx, &view), (2 * BAR, 58));
    press(cx, handle, "l ctrl-h");
    assert_eq!(
        cursor(cx, &view),
        (2 * BAR, 58),
        "마디 중간에서 뒤로는 그 마디 시작"
    );
    press(cx, handle, "ctrl-h");
    assert_eq!(cursor(cx, &view), (BAR, 58));
    press(cx, handle, "ctrl-k");
    assert_eq!(cursor(cx, &view), (BAR, 70));
    press(cx, handle, "]");
    assert_eq!(cursor(cx, &view), (BAR, 82));
    press(cx, handle, "ctrl-j ctrl-down");
    assert_eq!(cursor(cx, &view), (BAR, 58));
    press(
        cx,
        handle,
        "ctrl-k ctrl-k ctrl-k ctrl-k ctrl-k ctrl-k ctrl-k",
    );
    assert_eq!(cursor(cx, &view), (BAR, 127), "127 위로 가지 않는다");
    press(cx, handle, "k");
    assert_eq!(cursor(cx, &view), (BAR, 127));
    press(cx, handle, "[ [ [ [ [ [ [ [ [ [ [ [");
    assert_eq!(cursor(cx, &view), (BAR, 0), "0 아래로 가지 않는다");
}

// ---------- 7. 뷰포트가 커서를 따라간다 ----------

#[gpui_kit::test]
fn cursor_moves_scroll_viewport_to_follow(cx: &mut TestAppContext) {
    start(cx, "follow");
    let (handle, view) = open(cx, project_two_clips());
    press(cx, handle, "enter");
    let first = vp(cx, &view);
    assert!(first.width > 0.0 && first.height > 0.0, "{first:?}");
    assert_eq!(first.first_tick, 0.0);
    assert_eq!(first.px_per_tick, DEFAULT_PX_PER_TICK);
    let c = cx.read(|cx| view.read(cx).piano_roll().cursor());
    assert!(cursor_visible(&first, c, QUARTER), "{first:?} {c:?}");
    // 오른쪽 끝을 넘을 때까지 마디 단위로
    let bars_per_screen =
        (f64::from(first.width) / (BAR as f64 * first.px_per_tick)).ceil() as usize;
    for _ in 0..=bars_per_screen {
        press(cx, handle, "ctrl-l");
    }
    let moved = vp(cx, &view);
    let c = cx.read(|cx| view.read(cx).piano_roll().cursor());
    assert!(moved.first_tick > 0.0, "{moved:?}");
    assert!(cursor_visible(&moved, c, QUARTER), "{moved:?} {c:?}");
    // 돌아오면 왼쪽으로도 따라간다
    for _ in 0..=bars_per_screen {
        press(cx, handle, "ctrl-h");
    }
    let back = vp(cx, &view);
    assert_eq!(back.first_tick, 0.0);
    // 위로 끝까지
    for _ in 0..7 {
        press(cx, handle, "ctrl-k");
    }
    let top = vp(cx, &view);
    let c = cx.read(|cx| view.read(cx).piano_roll().cursor());
    assert_eq!(c.pitch.get(), 127);
    assert_eq!(top.first_row, 0.0, "{top:?}");
    assert!(cursor_visible(&top, c, QUARTER));
    // 아래로 끝까지
    for _ in 0..12 {
        press(cx, handle, "ctrl-j");
    }
    let bottom = vp(cx, &view);
    let c = cx.read(|cx| view.read(cx).piano_roll().cursor());
    assert_eq!(c.pitch.get(), 0);
    assert!(cursor_visible(&bottom, c, QUARTER), "{bottom:?}");
    assert!(bottom.first_row > top.first_row);
    // 스크롤 함수: 오른쪽·아래 양수, 왼쪽 끝은 0
    cx.read(|cx| view.read(cx).piano_roll().scroll_by(100.0, -24.0));
    let scrolled = vp(cx, &view);
    assert_eq!(
        scrolled.first_tick,
        bottom.first_tick + 100.0 / bottom.px_per_tick
    );
    assert_eq!(scrolled.first_row, bottom.first_row - 2.0);
    cx.read(|cx| view.read(cx).piano_roll().scroll_by(-1.0e9, 0.0));
    assert_eq!(vp(cx, &view).first_tick, 0.0);
}

// ---------- 8. 줌 ----------

#[gpui_kit::test]
fn zoom_keys_scale_and_keep_cursor_visible(cx: &mut TestAppContext) {
    start(cx, "zoom");
    let (handle, view) = open(cx, project_two_clips());
    press(cx, handle, "enter");
    press(cx, handle, "ctrl-l ctrl-l");
    press(cx, handle, "cmd-=");
    let zoomed = vp(cx, &view);
    assert!((zoomed.px_per_tick - DEFAULT_PX_PER_TICK * ZOOM_FACTOR).abs() < 1e-9);
    let c = cx.read(|cx| view.read(cx).piano_roll().cursor());
    assert!(cursor_visible(&zoomed, c, QUARTER), "{zoomed:?}");
    press(cx, handle, "cmd-- cmd--");
    let out = vp(cx, &view);
    assert!((out.px_per_tick - DEFAULT_PX_PER_TICK / ZOOM_FACTOR).abs() < 1e-9);
    assert!(cursor_visible(&out, c, QUARTER), "{out:?}");
    press(cx, handle, "cmd-0");
    assert_eq!(vp(cx, &view).px_per_tick, DEFAULT_PX_PER_TICK);
    for _ in 0..40 {
        press(cx, handle, "cmd-=");
    }
    let max = vp(cx, &view);
    assert_eq!(max.px_per_tick, MAX_PX_PER_TICK, "상한에서 멈춘다");
    assert!(cursor_visible(&max, c, QUARTER), "{max:?}");
    // 가운데로
    cx.read(|cx| view.read(cx).piano_roll().scroll_by(5000.0, 300.0));
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(keyscore_app::actions::view::CenterCursor), cx);
    })
    .unwrap();
    render(cx, handle);
    let centered = vp(cx, &view);
    let cursor_x =
        centered.x_of(c.tick.get() as f64) + (QUARTER as f64 * centered.px_per_tick) as f32 / 2.0;
    assert!(
        (cursor_x - centered.width / 2.0).abs() < 1.0,
        "{centered:?} {cursor_x}"
    );
    let cursor_y = centered.y_of(Viewport::row_of(c.pitch)) + centered.row_height / 2.0;
    assert!(
        (cursor_y - centered.height / 2.0).abs() < 1.0,
        "{centered:?} {cursor_y}"
    );
}

// ---------- 9. 들어갈 때 커서를 클립 안에 ----------

#[gpui_kit::test]
fn entering_piano_roll_places_cursor_in_first_clip(cx: &mut TestAppContext) {
    start(cx, "enter-clip");
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track
        .insert_clip(clip(2 * BAR, BAR, &[note(0, 960, 60)]))
        .unwrap();
    project.push_track(track).unwrap();
    let (handle, view) = open(cx, project);
    press(cx, handle, "enter");
    assert_eq!(
        cursor(cx, &view),
        (2 * BAR, 60),
        "클립 밖이면 첫 클립 시작으로"
    );
    press(cx, handle, "l l");
    assert_eq!(cursor(cx, &view), (2 * BAR + 2 * QUARTER, 60));
    press(cx, handle, "v v");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    assert_eq!(
        cursor(cx, &view),
        (2 * BAR + 2 * QUARTER, 60),
        "클립 안이면 그대로"
    );
    // 클립이 없는 트랙은 그대로
    let (handle, view) = open(cx, Project::new());
    press(cx, handle, "enter");
    assert_eq!(cursor(cx, &view), (0, 60));
}

// ---------- 10. 재생 헤드 ----------

#[gpui_kit::test]
fn playhead_follows_transport(cx: &mut TestAppContext) {
    start(cx, "playhead");
    let options = EngineOptions {
        client_name: format!("ks-pr-{}", std::process::id()),
        port_name: format!("KS pr {}", std::process::id()),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let (handle, view) = open(cx, project_two_clips());
    press(cx, handle, "enter");
    let playhead = |cx: &mut TestAppContext| cx.read(|cx| view.read(cx).piano_roll_playhead());
    assert_eq!(playhead(cx), None, "정지면 없음");
    let run = |cx: &mut TestAppContext, action: Box<dyn gpui_kit::Action>| {
        cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
            .unwrap();
        render(cx, handle);
    };
    run(cx, Box::new(transport::Play));
    std::thread::sleep(std::time::Duration::from_millis(120));
    render(cx, handle);
    let playing = playhead(cx).expect("재생 중이면 지금 자리");
    assert!(playing.get() > 0, "{playing:?}");
    run(cx, Box::new(transport::Pause));
    let paused = playhead(cx).expect("일시정지 자리");
    assert!(paused >= playing, "{paused:?} {playing:?}");
    render(cx, handle);
    assert_eq!(playhead(cx), Some(paused), "일시정지면 그 자리에 머문다");
    run(cx, Box::new(transport::Stop));
    assert_eq!(playhead(cx), None);
}

// ---------- 11. Element가 보이는 노트를 모두 한 번씩 그린다 ----------

#[gpui_kit::test]
fn element_paints_every_visible_note_once(cx: &mut TestAppContext) {
    start(cx, "paint");
    let (handle, view) = open(cx, project_two_clips());
    press(cx, handle, "enter");
    render(cx, handle);
    let (painted, expected) = cx.read(|cx| {
        let this = view.read(cx);
        let state = this.piano_roll();
        let doc = this.document().read(cx);
        let track = doc.project().tracks().first();
        let mut cache = MaxLenCache::default();
        cache.refresh(track, doc.revision());
        let mut scene = Scene::default();
        scene::build(
            &SceneInput {
                track,
                current: state.current_clip(track),
                cache: &cache,
                meter: doc.project().meter_map(),
                key: doc.project().key(),
                cursor: state.cursor(),
                step_ticks: state.step_ticks(),
                selection: &state.selection(),
                playhead: None,
                viewport: state.viewport(),
            },
            &mut scene,
        );
        (state.painted_notes(), scene.notes.len())
    });
    assert!(
        expected >= 4,
        "기본 뷰포트에 클립 A의 노트가 보인다: {expected}"
    );
    assert_eq!(painted, expected);
}

// ---------- 12. 클릭 자리 → 칸 ----------

#[test]
fn cell_at_maps_click_point_to_cursor() {
    let vp = viewport(1000.0, 0.1, 500.0, 72, 120.0);
    // x 150 → 틱 2500, y 30 → 행 57 → 음높이 70
    let c = vp.cell_at(150.0, 30.0).unwrap();
    assert_eq!((c.tick.get(), c.pitch.get()), (2500, 70));
    assert_eq!(vp.cell_at(0.0, 0.0).map(|c| c.pitch.get()), Some(72));
    assert_eq!(vp.cell_at(499.0, 119.0).map(|c| c.pitch.get()), Some(63));
    assert!(vp.cell_at(-1.0, 10.0).is_none());
    assert!(vp.cell_at(10.0, 120.0).is_none());
    assert!(vp.cell_at(500.0, 10.0).is_none());
    // 뷰포트 안에서 스텝 단위로 맞추는 일은 하지 않는다(5.7의 스텝은 키 이동 단위)
    assert_eq!(vp.cell_at(1.0, 0.0).unwrap().tick.get(), 1010);
}
