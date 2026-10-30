//! S3-T4c 배치 뷰의 Step 위치 표시(결정 `docs/decisions/2026-10-17-mode-entry-view.md` 이월, REQ 5.10·5.11)와
//! S3-T4b 넘김(`n n` 표시). 시험 목록은 `docs/reports/s3-t4c/plan.md`.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::app_state::AppSettings;
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-arr-step-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

/// 트랙 「Lead」 하나, 클립들(시작, 길이, 이름).
fn project(clips: &[(u64, u64, Option<&str>)]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    for &(s, l, name) in clips {
        let mut clip = Clip::new(Tick::new(s), l).unwrap();
        clip.insert_note(
            Note::new(
                Tick::ZERO,
                240,
                Pitch::new(72).unwrap(),
                Velocity::from_midi1(100).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        if let Some(name) = name {
            clip.set_name(name);
        }
        track.insert_clip(clip).unwrap();
    }
    project.push_track(track).unwrap();
    project
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

/// 배치 뷰(창의 처음 뷰), Normal, 계이름 레이아웃.
fn open(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = KeyLayout::Solfege));
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    (handle, view)
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn cursor_text(cx: &mut TestAppContext, handle: AnyWindowHandle) -> Option<String> {
    label(cx, handle, "arrangement-step-cursor")
}

fn clip_text(cx: &mut TestAppContext, handle: AnyWindowHandle) -> Option<String> {
    label(cx, handle, "arrangement-step-clip")
}

// ---------- 10–17. 배치 뷰 Step 표시 ----------

#[gpui_kit::test]
fn step_in_arrangement_shows_track_and_cursor(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "cursor", project(&[]));
    set_cursor(cx, handle, &view, BAR + QUARTER);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert_eq!(cursor_text(cx, handle).as_deref(), Some("Step: Lead 2.2.0"));
    // 텍스트 상태와 같은 값을 공개 API로도 읽는다
    let shown = cx.read(|cx| view.read(cx).arrangement_step(cx)).unwrap();
    assert_eq!(shown.cursor, "Step: Lead 2.2.0");
}

#[gpui_kit::test]
fn step_in_arrangement_inside_clip_names_clip(cx: &mut TestAppContext) {
    let (handle, view) = open(
        cx,
        "inside",
        project(&[(0, BAR, Some("후렴")), (2 * BAR, BAR, None)]),
    );
    set_cursor(cx, handle, &view, QUARTER);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert_eq!(clip_text(cx, handle).as_deref(), Some("클립 「후렴」 안"));
    // 이름 없는 클립은 시작 위치로 가리킨다
    set_cursor(cx, handle, &view, 2 * BAR + QUARTER);
    assert_eq!(clip_text(cx, handle).as_deref(), Some("클립 3.1.0 안"));
}

#[gpui_kit::test]
fn step_in_arrangement_at_clip_end_says_extend(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "end", project(&[(0, BAR, Some("A"))]));
    set_cursor(cx, handle, &view, BAR);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert_eq!(
        clip_text(cx, handle).as_deref(),
        Some("클립 「A」 끝에 이어 쓴다")
    );
}

#[gpui_kit::test]
fn step_in_arrangement_outside_clip_shows_new_clip_bars(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "outside", project(&[]));
    set_cursor(cx, handle, &view, 2 * BAR + 3 * QUARTER);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert_eq!(
        clip_text(cx, handle).as_deref(),
        Some("클립 밖: 입력하면 마디 3–3에 새 클립")
    );
    // 2분음표는 마디 4로 넘는다. 클립은 마디 단위로 그 마디 끝까지다(REQ 5.10)
    press(cx, handle, "2");
    assert_eq!(
        clip_text(cx, handle).as_deref(),
        Some("클립 밖: 입력하면 마디 3–4에 새 클립")
    );
}

#[gpui_kit::test]
fn step_in_arrangement_follows_input(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "follows", project(&[]));
    set_cursor(cx, handle, &view, 2 * BAR);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    press(cx, handle, "c enter");
    assert_eq!(cursor_text(cx, handle).as_deref(), Some("Step: Lead 3.2.0"));
    assert_eq!(clip_text(cx, handle).as_deref(), Some("클립 3.1.0 안"));
}

#[gpui_kit::test]
fn step_in_arrangement_blocked_by_next_clip_says_why(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "blocked", project(&[(BAR + 480, BAR, None)]));
    set_cursor(cx, handle, &view, BAR);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    let text = clip_text(cx, handle).unwrap();
    assert!(text.starts_with("클립 밖: 여기서는 쓸 수 없다"), "{text}");
}

#[gpui_kit::test]
fn normal_in_arrangement_has_no_step_display(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "normal", project(&[]));
    assert_eq!(cursor_text(cx, handle), None);
    assert_eq!(cx.read(|cx| view.read(cx).arrangement_step(cx)), None);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert!(cursor_text(cx, handle).is_some());
    press(cx, handle, "escape");
    assert_eq!(cursor_text(cx, handle), None);
    assert_eq!(clip_text(cx, handle), None);
}

#[gpui_kit::test]
fn step_in_piano_roll_has_no_arrangement_display(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "pianoroll", project(&[]));
    event(cx, handle, &view, ModeEvent::ToggleView);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert_eq!(cursor_text(cx, handle), None);
    assert_eq!(clip_text(cx, handle), None);
}
