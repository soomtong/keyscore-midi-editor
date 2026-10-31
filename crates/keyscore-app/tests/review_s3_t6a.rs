//! S3-T6a 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s3-t6a/review-1.md`). 픽스처는 `mouse_select.rs`에서
//! 복사했다(클립 A [0, 2마디), 틈, 클립 B [3, 4마디), 0.05 px/틱).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    dead_code
)]

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Entity, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent, TestAppContext,
    TouchPhase, VisualTestContext, point, px,
};
use keyscore_app::actions::args::LengthCode;
use keyscore_app::app_state::AppSettings;
use keyscore_app::clipboard;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::{Cursor, Viewport};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

type Triple = (u64, u32, u8);

// 클립 A [0, 2마디), 틈 [2, 3마디), 클립 B [3, 4마디). 클립 기준 시작.
const N1: Triple = (0, 960, 60);
const N2: Triple = (960, 960, 62);
const N3: Triple = (1920, 480, 64);
const N4: Triple = (3840, 960, 60);
const N5: Triple = (3840, 960, 67);
const N6: Triple = (5760, 960, 65);
const A_NOTES: [Triple; 6] = [N1, N2, N3, N4, N5, N6];
const B_START: u64 = 3 * BAR;
const B1: Triple = (0, 960, 60);
const B2: Triple = (960, 960, 64);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str, preset: Preset) {
    let dir = std::env::temp_dir().join(format!("ks-mouse-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-mouse-{name}", std::process::id()),
        );
        if preset == Preset::Vim {
            keymap::set_preset(cx, Preset::Vim);
        }
        // 더블클릭 생성의 길이 코드: 8분
        AppSettings::update(cx, |s| s.input.step_default_length = LengthCode::Eighth);
    });
}

fn note((start, len, pitch): Triple) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn clip(start: u64, length: u64, notes: &[Triple]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(note(*n)).unwrap();
    }
    clip
}

fn project() -> Project {
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip(0, 2 * BAR, &A_NOTES)).unwrap();
    track.insert_clip(clip(B_START, BAR, &[B1, B2])).unwrap();
    project.push_track(track).unwrap();
    project
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

/// 피아노롤에 들어가 커서를 C4·틱 0에 두고 가로 배율을 0.05 px/틱(4분 = 48 px)으로 줄인다. 그러면 클립 B 끝
/// (틱 15360)이 x 768에 온다.
fn setup(
    cx: &mut TestAppContext,
    name: &str,
    preset: Preset,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name, preset);
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    cx.update(|cx| view.read(cx).piano_roll().zoom(0.5));
    render(cx, handle);
    let vp = viewport(cx, &view);
    assert!((vp.px_per_tick - 0.05).abs() < 1e-9, "{vp:?}");
    assert!(vp.width >= 800.0, "창이 좁다: {vp:?}");
    (handle, view)
}

fn viewport(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Viewport {
    cx.read(|cx| view.read(cx).piano_roll().viewport())
}

fn main_view_bounds(cx: &mut TestAppContext, handle: AnyWindowHandle) -> gpui_kit::Bounds<Pixels> {
    cx.update_window(handle, |_, window, _| window.find("main-view").bounds())
        .unwrap()
}

/// 절대 틱·음높이 행 가운데의 창 좌표.
fn spot(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    tick: u64,
    pitch: u8,
) -> Point<Pixels> {
    let vp = viewport(cx, view);
    let x = ((tick as f64 - vp.first_tick) * vp.px_per_tick) as f32;
    let row = Viewport::row_of(Pitch::new(pitch).unwrap());
    let y = (row - vp.first_row + 0.5) * vp.row_height;
    assert!(
        (0.0..vp.width).contains(&x) && (0.0..vp.height).contains(&y),
        "뷰포트 밖: {tick} {pitch} {vp:?}"
    );
    main_view_bounds(cx, handle).origin + point(px(x), px(y))
}

fn cmd() -> Modifiers {
    Modifiers {
        platform: true,
        ..Modifiers::default()
    }
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::default()
    }
}

fn none() -> Modifiers {
    Modifiers::default()
}

fn down_n(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    modifiers: Modifiers,
    count: usize,
) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers,
        click_count: count,
        first_mouse: false,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn down(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, modifiers: Modifiers) {
    down_n(cx, handle, at, modifiers, 1);
}

fn drag_to(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    modifiers: Modifiers,
) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: Some(MouseButton::Left),
        modifiers,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn up_n(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    modifiers: Modifiers,
    count: usize,
) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers,
        click_count: count,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn up(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, modifiers: Modifiers) {
    up_n(cx, handle, at, modifiers, 1);
}

fn click(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    modifiers: Modifiers,
) {
    down(cx, handle, at, modifiers);
    up(cx, handle, at, modifiers);
}

fn double_click(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    modifiers: Modifiers,
) {
    click(cx, handle, at, modifiers);
    down_n(cx, handle, at, modifiers, 2);
    up_n(cx, handle, at, modifiers, 2);
}

fn triple(n: &Note) -> Triple {
    (n.start().get(), n.length(), n.pitch().get())
}

fn selection_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    cx.read(|cx| {
        let s = view.read(cx).selection();
        let mut v: Vec<_> = s.notes().map(triple).collect();
        v.sort_unstable();
        v
    })
}

fn sorted(mut v: Vec<Triple>) -> Vec<Triple> {
    v.sort_unstable();
    v
}

fn has_selection(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_state().has_selection())
}

fn cursor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Cursor {
    cx.read(|cx| view.read(cx).piano_roll().cursor())
}

/// 클립 `index`의 (시작, 길이, 노트).
fn clip_of(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    index: usize,
) -> (u64, u64, Vec<Triple>) {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        let (_, c) = track.clips().nth(index).unwrap();
        (
            c.start().get(),
            c.length(),
            c.notes().iter().map(triple).collect(),
        )
    })
}

fn clip_count(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| {
        view.read(cx).document().read(cx).project().tracks()[0]
            .clips()
            .count()
    })
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

fn band_rect(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).piano_roll().band_rect().is_some())
}

fn gesture_active(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).piano_roll().gesture_active())
}

/// 노트 안쪽 한 점(시작 + 100틱).
fn on(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    n: Triple,
) -> Point<Pixels> {
    spot(cx, handle, view, n.0 + 100, n.2)
}

/// 클립 B 노트 안쪽 한 점.
fn on_b(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    n: Triple,
) -> Point<Pixels> {
    spot(cx, handle, view, B_START + n.0 + 100, n.2)
}
fn mode_event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    event: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| this.apply_mode_event(event, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn wheel(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, dx: f32, dy: f32) {
    VisualTestContext::from_window(handle, cx).simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
        modifiers: none(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
}

// ---------- 시도 1: 누른 채 모드가 바뀌면 ----------

/// 결정 2절 1: 마우스 선택은 Normal에서만. 빈 곳을 누른 채 Step으로 가면 그 뒤 끌기는 선택을 바꾸지 않아야 한다.
#[gpui_kit::test]
fn band_drag_after_entering_step_mode_does_not_select(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-step", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 61);
    down(cx, handle, from, none());
    mode_event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    drag_to(cx, handle, to, none());
    assert_eq!(
        selection_of(cx, &view),
        vec![],
        "Step에서 끌기가 러버밴드 선택을 만든다(결정 2절 1 위반)"
    );
    up(cx, handle, to, none());
}

// ---------- 시도 2: 누른 채 피아노롤을 떠나면 ----------

/// 러버밴드 중 배치 뷰로 가면(뗌은 그려지지 않은 피아노롤에 닿지 않는다) 조작이 남지 않아야 한다.
#[gpui_kit::test]
fn gesture_does_not_survive_leaving_piano_roll(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-leave", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 61);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    mode_event(cx, handle, &view, ModeEvent::ToggleView);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    up(cx, handle, to, none());
    assert!(
        !gesture_active(cx, &view),
        "피아노롤을 떠난 뒤에도 마우스 조작이 살아 있다"
    );
}

/// 위의 결과: 배치 뷰에서 친 첫 `Esc`를 남은 러버밴드가 가로채고, 누르기 전 노트 선택을 되살려 배치 뷰의 FSM에
/// 「선택 있음」을 세운다.
#[gpui_kit::test]
fn escape_in_arrangement_is_not_taken_by_stale_band(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-leave-esc", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 61);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    mode_event(cx, handle, &view, ModeEvent::ToggleView);
    up(cx, handle, to, none());
    let before = has_selection(cx, &view);
    press(cx, handle, "escape");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    assert!(
        !has_selection(cx, &view),
        "배치 뷰에서 Esc 뒤 선택 있음(Esc 전 {before}), 노트 선택 {:?}",
        selection_of(cx, &view)
    );
}

/// 위의 결과: 피아노롤로 돌아와 피아노롤 밖(트랜스포트 쪽)에서 시작한 끌기가 남은 러버밴드를 이어 선택을 바꾼다.
/// (4 px 여백에서 시작하면 분할 손잡이가 끌기를 가져가 재현되지 않는다.)
#[gpui_kit::test]
fn drag_from_outside_after_return_does_not_resume_stale_band(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-leave-drag", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 61);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    mode_event(cx, handle, &view, ModeEvent::ToggleView);
    up(cx, handle, to, none());
    mode_event(cx, handle, &view, ModeEvent::ToggleView);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    // 리뷰 1 당시의 전제(조작이 남음)는 지적 1 수정으로 거짓이 되었다. 고친 동작을 단언하고 뒤 단언은 그대로 둔다.
    assert!(!gesture_active(cx, &view), "뷰 전환이 조작을 버린다");
    // 피아노롤 밖(위쪽 트랜스포트 쪽)에서 눌러 노트 위로 끈다. 피아노롤은 누름을 받지 않았다.
    let margin = main_view_bounds(cx, handle).origin + point(px(200.0), px(-30.0));
    let selected_before = selection_of(cx, &view);
    down(cx, handle, margin, none());
    let far = spot(cx, handle, &view, 6000, 55);
    drag_to(cx, handle, far, none());
    let after = selection_of(cx, &view);
    up(cx, handle, far, none());
    assert_eq!(
        after, selected_before,
        "피아노롤 밖에서 시작한 끌기가 선택을 바꾼다"
    );
}

// ---------- 시도 3: 누른 채 스크롤 ----------

/// 결정 3절 5: 직사각형은 「누른 점」과 지금 점이 마주 본다. 누른 채 가로로 스크롤해도 누른 점(틱 100)은 내용에
/// 붙어 있어야 한다.
#[gpui_kit::test]
fn wheel_scroll_during_band_keeps_press_point_on_content(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-scroll", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 59);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    let first = viewport(cx, &view).first_tick;
    // 내용을 왼쪽으로 50 px(= 뷰포트가 오른쪽으로 1000틱)
    wheel(cx, handle, to, -50.0, 0.0);
    let moved = viewport(cx, &view).first_tick - first;
    assert!((moved - 1000.0).abs() < 1.0, "스크롤 {moved}");
    drag_to(cx, handle, to + point(px(1.0), px(0.0)), none());
    let sel = selection_of(cx, &view);
    up(cx, handle, to + point(px(1.0), px(0.0)), none());
    assert!(
        sel.contains(&N1),
        "누른 점(틱 100)의 N1이 빠졌다: 러버밴드가 화면 좌표에 붙어 있다 {sel:?}"
    );
}

// ---------- 시도 4: 못 깨뜨림 후보 ----------

/// 4 px 여백 누름은 커서를 옮기지 않고 조작을 만들지 않는다(결정 5절 「누를 수 없는 빈 테두리」).
#[gpui_kit::test]
fn press_in_margin_is_inert(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-margin", Preset::Default);
    let cursor = cursor_of(cx, &view);
    let margin = main_view_bounds(cx, handle).origin - point(px(2.0), px(2.0));
    down(cx, handle, margin, none());
    assert!(!gesture_active(cx, &view));
    assert_eq!(cursor_of(cx, &view), cursor);
    up(cx, handle, margin, none());
    // 바로 안쪽 첫 픽셀은 눌린다
    let inside = main_view_bounds(cx, handle).origin + point(px(1.0), px(1.0));
    click(cx, handle, inside, none());
    assert_ne!(cursor_of(cx, &view), cursor);
}

/// vim 프리셋에서도 더블클릭 삭제는 `edit::Delete` 한 단위다.
#[gpui_kit::test]
fn vim_double_click_note_deletes_with_one_undo(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-vim-dbl", Preset::Vim);
    let before = undo_len(cx, &view);
    let p2 = on(cx, handle, &view, N2);
    double_click(cx, handle, p2, none());
    assert!(!clip_of(cx, &view, 0).2.contains(&N2));
    assert_eq!(undo_len(cx, &view), before + 1);
    assert_eq!(selection_of(cx, &view), vec![]);
}

/// 선택된 두 노트 중 하나를 `Cmd`로 더블클릭: 첫 누름이 빼고 둘째 누름이 그 노트만 지운다. 다른 노트는 남는다.
#[gpui_kit::test]
fn cmd_double_click_selected_note_deletes_only_it(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-cmd-dbl", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    let p2 = on(cx, handle, &view, N2);
    click(cx, handle, p1, none());
    click(cx, handle, p2, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N2]);
    down_n(cx, handle, p2, cmd(), 1);
    up_n(cx, handle, p2, cmd(), 1);
    down_n(cx, handle, p2, cmd(), 2);
    up_n(cx, handle, p2, cmd(), 2);
    let notes = clip_of(cx, &view, 0).2;
    assert!(!notes.contains(&N2));
    assert!(notes.contains(&N1));
}

/// `Esc`는 누른 채일 때만 가로챈다. 뗀 뒤 `Esc`는 선택 해제로 간다.
#[gpui_kit::test]
fn escape_after_release_clears_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-esc-after", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    assert!(has_selection(cx, &view));
    press(cx, handle, "escape");
    assert!(!has_selection(cx, &view));
    assert_eq!(selection_of(cx, &view), vec![]);
}

// ---------- 시도 5: 폭 0 러버밴드 ----------

/// 세로로만 끈 러버밴드(문턱 이상)가 정수 틱 위에 있으면 `[t, t)`가 되어 화면에서 가로지르는 노트를 고르지 못한다.
#[test]
fn vertical_band_on_integer_tick_selects_crossed_note() {
    use keyscore_app::pianoroll::mouse::{band, band_notes};
    let vp = Viewport {
        first_tick: 0.0,
        px_per_tick: 0.1,
        width: 800.0,
        first_row: 40.0,
        row_height: 12.0,
        height: 480.0,
    };
    // x 50 → 틱 500. 노트 [0, 960) 음 84(행 43)
    let mut c = Clip::new(Tick::ZERO, BAR).unwrap();
    c.insert_note(note((0, 960, 84))).unwrap();
    let b = band(&vp, (50.0, 1.0), (50.0, 100.0)).unwrap();
    assert!(
        !band_notes(&c, b).is_empty(),
        "세로 러버밴드가 가로지른 노트를 고르지 않는다: {b:?}"
    );
}
