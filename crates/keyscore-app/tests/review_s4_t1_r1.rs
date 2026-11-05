//! S4-T1 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s4-t1/review-1.md` 3절).
//!
//! `brk_` 접두는 코드를 읽고 「실패할 것」이라고 본 시험이다(근거는 리뷰 문서). 나머지는 경계값·불변식 대조 시험이다.
//! 리뷰어 세션(Linux)은 앱 crate를 실행하지 못해 교차 타입 검사(`--target aarch64-apple-darwin`)까지만 했다.
//! 실행은 CI(macos-15)·사람 Mac 몫이다.
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Entity, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};
use keyscore_app::actions::{cursor, view};
use keyscore_app::arrangement::scene::{self, Scene, SceneInput, StepGhost};
use keyscore_app::arrangement::{
    ArrangementState, ClipSelection, DEFAULT_PX_PER_TICK, DEFAULT_ROW_HEIGHT, MAX_PX_PER_TICK,
    MIN_PX_PER_TICK, Viewport,
};
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

// ---------- 도우미(tests/arrangement.rs와 같은 꼴) ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-arr-r1-{}-{name}", std::process::id()));
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

fn clip(start: u64, len: u64) -> Clip {
    let mut clip = Clip::new(Tick::new(start), len).unwrap();
    clip.insert_note(note(0, 240, 72)).unwrap();
    clip
}

fn track(name: &str, clips: &[(u64, u64)]) -> Track {
    let mut track = Track::new(name, Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    for &(s, l) in clips {
        track.insert_clip(clip(s, l)).unwrap();
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

/// 트랙 `n`개(첫 트랙만 `clips`).
fn many_tracks(n: usize, clips: &[(u64, u64)]) -> Project {
    let mut tracks = vec![track("T0", clips)];
    for i in 1..n {
        tracks.push(track(&format!("T{i}"), &[]));
    }
    project(tracks)
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: Box<dyn gpui_kit::Action>) {
    cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
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

fn has_selection(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_state().has_selection())
}

fn clip_selection(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> ClipSelection {
    cx.read(|cx| view.read(cx).clip_selection())
}

fn bounds_of(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    id: &'static str,
) -> gpui_kit::Bounds<Pixels> {
    cx.update_window(handle, |_, window, _| window.find(id).bounds())
        .unwrap()
}

/// `main-view`(패널 크기)의 4 px 여백 안쪽이 Element다(tests/arrangement.rs와 같다).
const MAIN_PAD: f32 = 4.0;

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

/// `Inner::cell_visible`과 같은 규칙(mod.rs:244-253): 틱은 왼쪽 끝 이상이고 칸 오른쪽이 뷰포트 안(칸이 뷰포트보다
/// 넓으면 왼쪽 끝 맞춤), 행은 위아래 안.
fn cell_visible(v: Viewport, tick: u64, step: u64, row: usize) -> bool {
    let (tick, step, row) = (tick as f64, step as f64, row as f32);
    tick >= v.first_tick
        && (tick + step <= v.last_tick() || tick <= v.first_tick)
        && row >= v.first_row
        && row + 1.0 <= v.first_row + v.rows_visible() + 0.01
}

// ---------- 시도 1. Step 고스트: 커서가 u64 끝 근처(마지막 마디) ----------

/// `step_ghost`(`step_input.rs`)의 `Err` 분기는 `bar_start(bar + 1).map_or(at + bar_ticks, …)`로 끝을 잡는다.
/// `map_or`의 기본값은 늘 계산되므로 `at + bar_ticks`가 u64를 넘으면(커서가 끝에서 3840틱 안) debug에서 패닉,
/// release에서는 감겨 `end < start`가 되고 `scene::block_rect`의 `end - start`가 다시 감긴다. 커서는 클립 밖이라
/// `Note::new(at, 960)`의 `checked_add`가 `TickOverflow`를 돌려 이 분기로 들어온다.
#[gpui_kit::test]
fn brk_step_ghost_near_u64_end_does_not_overflow(cx: &mut TestAppContext) {
    start(cx, "ghost-u64");
    let (handle, view) = open(cx, project(vec![track("Lead", &[(0, BAR)])]));
    set_cursor(cx, handle, &view, u64::MAX - 100);
    // Step 진입 → render_arrangement → step_ghost. 여기서 debug는 오버플로 패닉.
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    let ghost = cx.read(|cx| view.read(cx).step_ghost(cx));
    if let Some(g) = ghost {
        assert!(
            g.end >= g.start.get(),
            "고스트 끝이 시작보다 앞: {g:?} (u64 감김)"
        );
    }
    // 장면도 패닉 없이 만들어진다
    let _ = arr(cx, &view).scene();
}

// ---------- 시도 2. 새 트랙이 뷰포트 아래에 생길 때 따라가기 ----------

/// `set_current_track`(`window.rs`)이 배치 뷰에서 `ensure_visible`을 부르지만 `Inner::clamp_rows`는 마지막
/// 그리기 때의 트랙 수(`Inner::rows`)로 자른다. `track::New`는 문서에 트랙을 넣은 직후(아직 그리기 전) 새 트랙을
/// 현재 트랙으로 삼으므로 `rows`가 하나 적다. 아래쪽 끝 `max_first = rows − visible`에 걸려 새 행(번호 `rows`)은
/// 뷰포트 바로 아래에 남는다. 트랙 수가 보이는 행 수보다 많을 때만 드러난다.
#[gpui_kit::test]
fn brk_new_track_beyond_viewport_bottom_is_revealed(cx: &mut TestAppContext) {
    start(cx, "new-track-follow");
    let (handle, view) = open(cx, many_tracks(40, &[(0, BAR)]));
    let v0 = arr_vp(cx, &view);
    assert!(
        v0.rows_visible() < 40.0,
        "{v0:?}: 창이 트랙 40개보다 낮아야 한다"
    );
    press(cx, handle, "ctrl-j");
    assert_eq!(current_index(cx, &view), Some(39));
    let v1 = arr_vp(cx, &view);
    assert!(
        cell_visible(v1, 0, QUARTER, 39),
        "{v1:?}: 마지막 트랙 행이 보인다"
    );
    press(cx, handle, "cmd-t");
    assert_eq!(current_index(cx, &view), Some(40), "새 트랙이 현재 트랙");
    let v2 = arr_vp(cx, &view);
    assert!(
        cell_visible(v2, 0, QUARTER, 40),
        "{v2:?}: 새 트랙(행 40)이 뷰포트 안이어야 한다"
    );
}

// ---------- 시도 3. REQ 5.11: 배치 뷰 Tab은 시간을 유지 ----------

#[gpui_kit::test]
fn tab_in_arrangement_keeps_cursor_tick(cx: &mut TestAppContext) {
    start(cx, "tab-tick");
    let (handle, view) = open(cx, many_tracks(3, &[(0, BAR), (3 * BAR, BAR)]));
    set_cursor(cx, handle, &view, 3 * BAR + 480);
    for (key, want) in [("tab", 1), ("tab", 2), ("tab", 2), ("shift-tab", 1)] {
        press(cx, handle, key);
        assert_eq!(current_index(cx, &view), Some(want), "{key}");
        assert_eq!(cursor_tick(cx, &view), 3 * BAR + 480, "{key}: 틱은 그대로");
    }
}

// ---------- 시도 4. 트랙 0개 ----------

#[gpui_kit::test]
fn empty_project_arrangement_keys_and_mouse_do_not_panic(cx: &mut TestAppContext) {
    start(cx, "empty");
    let (handle, view) = open(cx, Project::new());
    assert_eq!(current_index(cx, &view), None);
    press(
        cx,
        handle,
        "h l j k ctrl-j ctrl-k ctrl-h ctrl-l cmd-a escape cmd-= cmd-- cmd-0 tab shift-tab",
    );
    run(cx, handle, Box::new(view::CenterCursor));
    run(cx, handle, Box::new(cursor::SetMark("a".into())));
    run(cx, handle, Box::new(cursor::JumpMark("a".into())));
    let center = bounds_of(cx, handle, "main-view").center();
    mouse(cx, handle, center, MouseButton::Left);
    mouse(cx, handle, center, MouseButton::Right);
    assert!(!cx.read(|cx| view.read(cx).context_menu_open()));
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert!(cx.read(|cx| view.read(cx).step_ghost(cx)).is_none());
    event(cx, handle, &view, ModeEvent::ToNormal);
    let s = arr(cx, &view).scene();
    assert!(s.rows.is_empty());
    assert_eq!(s.cursor, None, "트랙이 없으면 커서 칸도 없다");
    assert!(clip_selection(cx, &view).is_empty());
    assert!(!has_selection(cx, &view));
    assert_eq!(view_of(cx, &view), View::Arrangement);
}

// ---------- 시도 5. 줌 상한·하한과 커서 칸 ----------

#[gpui_kit::test]
fn zoom_keys_clamp_at_bounds_and_keep_cursor_cell_visible(cx: &mut TestAppContext) {
    start(cx, "zoom-bounds");
    let (handle, view) = open(cx, many_tracks(3, &[(0, BAR), (2 * BAR, BAR)]));
    set_cursor(cx, handle, &view, 2 * BAR);
    for _ in 0..40 {
        press(cx, handle, "cmd-=");
    }
    let v = arr_vp(cx, &view);
    assert_eq!(v.px_per_tick, MAX_PX_PER_TICK, "상한에서 멈춘다");
    assert!(cell_visible(v, 2 * BAR, QUARTER, 0), "{v:?}");
    for _ in 0..60 {
        press(cx, handle, "cmd--");
    }
    let v = arr_vp(cx, &view);
    assert_eq!(v.px_per_tick, MIN_PX_PER_TICK, "하한에서 멈춘다");
    assert!(cell_visible(v, 2 * BAR, QUARTER, 0), "{v:?}");
    press(cx, handle, "cmd-0");
    let v = arr_vp(cx, &view);
    assert_eq!(v.px_per_tick, DEFAULT_PX_PER_TICK);
    assert!(cell_visible(v, 2 * BAR, QUARTER, 0), "{v:?}");
    // 줌·스크롤이 뷰포트를 음수로 보내지 않는다
    assert!(v.first_tick >= 0.0 && v.first_row >= 0.0, "{v:?}");
}

// ---------- 시도 6. 먼 커서·마지막 트랙에서 배치 뷰로 들어올 때 ----------

/// S3-T1 리뷰 1 지적 1의 배치 뷰 판: 피아노롤에 있는 동안 커서가 멀리(가로 100마디, 세로 마지막 트랙) 갔다가
/// `v`로 돌아오면 커서 칸이 보여야 한다(`reveal_on_entry`).
#[gpui_kit::test]
fn entering_arrangement_with_far_cursor_and_last_track_reveals_cell(cx: &mut TestAppContext) {
    start(cx, "far-entry");
    let (handle, view) = open(cx, many_tracks(40, &[(100 * BAR, BAR)]));
    press(cx, handle, "v");
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    let before = arr_vp(cx, &view);
    set_cursor(cx, handle, &view, 100 * BAR);
    run(cx, handle, Box::new(cursor::LastTrack));
    assert_eq!(current_index(cx, &view), Some(39));
    assert_eq!(
        arr_vp(cx, &view),
        before,
        "피아노롤에 있는 동안 배치 뷰 뷰포트는 움직이지 않는다"
    );
    press(cx, handle, "v");
    assert_eq!(view_of(cx, &view), View::Arrangement);
    let v = arr_vp(cx, &view);
    assert!(
        cell_visible(v, 100 * BAR, QUARTER, 39),
        "{v:?}: 들어올 때 커서 칸(100마디, 행 39)이 보여야 한다"
    );
}

// ---------- 시도 7. 클립 선택과 FSM 플래그: Step 모드 왕복·뷰 전환 ----------

#[gpui_kit::test]
fn clip_selection_and_mode_flag_stay_consistent_through_step_mode(cx: &mut TestAppContext) {
    start(cx, "selection-step");
    let (handle, view) = open(cx, many_tracks(2, &[(0, BAR), (2 * BAR, BAR)]));
    let consistent = |cx: &mut TestAppContext| {
        let has = has_selection(cx, &view);
        let clips = clip_selection(cx, &view);
        let notes = cx.read(|cx| view.read(cx).selection().is_empty());
        assert_eq!(has, !clips.is_empty(), "FSM 플래그 = 배치 뷰 선택 유무");
        assert!(notes, "배치 뷰에서 노트 선택은 비어 있다");
    };
    press(cx, handle, "cmd-a");
    assert_eq!(clip_selection(cx, &view).len(), 2);
    consistent(cx);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    consistent(cx);
    event(cx, handle, &view, ModeEvent::ToNormal);
    consistent(cx);
    // 커서 이동은 선택을 지우지 않고 플래그도 그대로
    press(cx, handle, "l j");
    consistent(cx);
    // 문서 변경(새 트랙)도 선택을 그대로 둔다
    press(cx, handle, "cmd-t");
    consistent(cx);
    // `w`(가사 모드)는 피아노롤로 옮기므로 뷰 전환 규칙대로 비운다
    press(cx, handle, "escape");
    assert!(!has_selection(cx, &view));
    press(cx, handle, "cmd-a");
    press(cx, handle, "enter");
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    assert!(clip_selection(cx, &view).is_empty());
    assert!(!has_selection(cx, &view));
}

// ---------- 시도 8. 피아노롤의 커서 이동은 배치 뷰 뷰포트를 건드리지 않는다 ----------

#[gpui_kit::test]
fn cursor_moves_in_piano_roll_do_not_touch_arrangement_viewport(cx: &mut TestAppContext) {
    start(cx, "pr-isolation");
    let (handle, view) = open(cx, many_tracks(3, &[(0, 2 * BAR), (50 * BAR, BAR)]));
    let v0 = arr_vp(cx, &view);
    press(cx, handle, "v");
    press(
        cx,
        handle,
        "shift-n ctrl-l ctrl-l cmd-right cmd-down cmd-up",
    );
    run(cx, handle, Box::new(cursor::SetMark("b".into())));
    run(cx, handle, Box::new(cursor::JumpMark("b".into())));
    press(cx, handle, "cmd-= cmd-=");
    let v1 = arr_vp(cx, &view);
    assert_eq!(
        v0, v1,
        "피아노롤 뷰의 이동·줌은 배치 뷰 상태를 바꾸지 않는다"
    );
}

// ---------- 시도 9. 맞닿은 클립의 경계 ----------

#[gpui_kit::test]
fn clip_boundary_keys_with_adjacent_clips(cx: &mut TestAppContext) {
    start(cx, "adjacent");
    let (handle, view) = open(cx, many_tracks(1, &[(BAR, BAR), (2 * BAR, BAR)]));
    for (key, want) in [
        ("l", BAR),
        ("l", 2 * BAR),
        ("l", 3 * BAR),
        ("l", 3 * BAR),
        ("h", 2 * BAR),
        ("h", BAR),
        ("h", 0),
        ("h", 0),
    ] {
        press(cx, handle, key);
        assert_eq!(cursor_tick(cx, &view), want, "{key}");
    }
    // 클립 한가운데서는 그 클립의 시작·끝
    set_cursor(cx, handle, &view, BAR + 1);
    press(cx, handle, "h");
    assert_eq!(cursor_tick(cx, &view), BAR);
    set_cursor(cx, handle, &view, BAR + 1);
    press(cx, handle, "l");
    assert_eq!(cursor_tick(cx, &view), 2 * BAR);
}

// ---------- 시도 10. 트랙 행 밖 클릭·우클릭 ----------

#[gpui_kit::test]
fn clicks_outside_track_rows_do_nothing(cx: &mut TestAppContext) {
    start(cx, "outside-rows");
    let (handle, view) = open(cx, many_tracks(3, &[(0, BAR)]));
    set_cursor(cx, handle, &view, BAR);
    press(cx, handle, "j");
    assert_eq!(current_index(cx, &view), Some(1));
    // 행 3은 트랙이 없다(0·1·2뿐)
    let below = at_cell(cx, handle, &view, 3, 2 * BAR);
    mouse(cx, handle, below, MouseButton::Left);
    assert_eq!(
        current_index(cx, &view),
        Some(1),
        "빈 자리 클릭은 트랙을 바꾸지 않는다"
    );
    assert_eq!(
        cursor_tick(cx, &view),
        BAR,
        "빈 자리 클릭은 커서를 옮기지 않는다"
    );
    mouse(cx, handle, below, MouseButton::Right);
    assert!(
        !cx.read(|cx| view.read(cx).context_menu_open()),
        "빈 자리 우클릭은 메뉴를 열지 않는다"
    );
    assert_eq!(current_index(cx, &view), Some(1));
    assert_eq!(cursor_tick(cx, &view), BAR);
}

// ---------- 시도 11. 장면 계산: 틱 u64 끝·거대한 뷰포트 ----------

#[test]
fn scene_at_tick_max_and_huge_viewport_does_not_panic() {
    let mut t = track("T", &[(0, BAR)]);
    // u64 끝에 닿는 클립도 모델상 유효하다
    t.insert_clip(Clip::new(Tick::new(u64::MAX - BAR), BAR).unwrap())
        .unwrap();
    let p = project(vec![t]);
    let current: Option<TrackId> = Some(p.tracks()[0].id());
    let sel = ClipSelection::default();
    let mut out = Scene::default();
    for (first_tick, cursor, playhead) in [
        (0.0, u64::MAX, Some(u64::MAX)),
        (
            (u64::MAX - 100_000) as f64,
            u64::MAX - 1,
            Some(u64::MAX - 1),
        ),
        (1e30, 0, None),
        (f64::from(f32::MAX), u64::MAX, Some(0)),
    ] {
        for ppt in [MIN_PX_PER_TICK, DEFAULT_PX_PER_TICK, MAX_PX_PER_TICK] {
            scene::build(
                &SceneInput {
                    project: &p,
                    current_track: current,
                    cursor: Tick::new(cursor),
                    step_ticks: QUARTER,
                    selection: &sel,
                    ghost: Some(StepGhost {
                        start: Tick::new(u64::MAX - BAR),
                        end: u64::MAX,
                        blocked: true,
                    }),
                    playhead: playhead.map(Tick::new),
                    viewport: Viewport {
                        first_tick,
                        px_per_tick: ppt,
                        width: 1600.0,
                        first_row: 0.0,
                        row_height: DEFAULT_ROW_HEIGHT,
                        height: 300.0,
                    },
                },
                &mut out,
            );
            assert!(
                out.lines
                    .windows(2)
                    .all(|w| w[0].tick.get() <= w[1].tick.get()),
                "격자선 틱은 단조 증가"
            );
        }
    }
    // 크기 0·배율 0은 빈 장면
    scene::build(
        &SceneInput {
            project: &p,
            current_track: current,
            cursor: Tick::ZERO,
            step_ticks: QUARTER,
            selection: &sel,
            ghost: None,
            playhead: Some(Tick::ZERO),
            viewport: Viewport {
                first_tick: 0.0,
                px_per_tick: 0.0,
                width: 0.0,
                first_row: 0.0,
                row_height: DEFAULT_ROW_HEIGHT,
                height: 0.0,
            },
        },
        &mut out,
    );
    assert!(out.rows.is_empty() && out.clips.is_empty() && out.lines.is_empty());
    assert_eq!((out.cursor, out.playhead, out.ghost), (None, None, None));
}

// ---------- 시도 12. 뷰포트 상태 API의 경계: 크기를 알기 전 ----------

#[test]
fn state_api_before_first_paint_does_not_panic_and_keeps_bounds() {
    let state = ArrangementState::new();
    let cell = keyscore_app::arrangement::Cell {
        tick: Tick::new(u64::MAX),
        step_ticks: u64::MAX,
        row: Some(usize::MAX),
    };
    state.ensure_visible(cell);
    state.center(cell);
    state.reveal_on_entry(cell);
    state.zoom(1e300, cell);
    state.zoom(0.0, cell);
    state.zoom_at(f64::INFINITY, 0.0);
    state.zoom_at(0.0, f32::MAX);
    state.scroll_by(f32::MAX, f32::MAX);
    state.scroll_by(-f32::MAX, -f32::MAX);
    state.zoom_reset(cell);
    let v = state.viewport();
    assert!(
        v.px_per_tick >= MIN_PX_PER_TICK && v.px_per_tick <= MAX_PX_PER_TICK,
        "{v:?}"
    );
    assert!(v.first_tick >= 0.0 && v.first_tick.is_finite(), "{v:?}");
    assert!(v.first_row >= 0.0 && v.first_row.is_finite(), "{v:?}");
    assert_eq!(
        v.cell_at(0.0, 0.0, QUARTER, 1),
        None,
        "크기를 모르면 칸도 없다"
    );
}
