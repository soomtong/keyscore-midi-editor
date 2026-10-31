//! S3-T6a 적대적 리뷰 2회차의 깨뜨리기 시험(`docs/reports/s3-t6a/review-2.md`). 픽스처는 `mouse_select.rs`에서
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

fn wheel_mods(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    dx: f32,
    dy: f32,
    modifiers: Modifiers,
) {
    VisualTestContext::from_window(handle, cx).simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
        modifiers,
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
}

/// 그린 직사각형(뷰 기준 px)의 오른쪽 끝 x에 해당하는 절대 틱.
fn band_right_tick(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> f64 {
    let vp = viewport(cx, view);
    let rect = cx
        .read(|cx| view.read(cx).piano_roll().band_rect())
        .expect("러버밴드 직사각형");
    vp.first_tick + f64::from(rect.x + rect.w) / vp.px_per_tick
}

// ---------- 시도 1: 누른 채 스크롤하고 움직이지 않고 뗀다 ----------

/// 결정 3절 5·MS-1: 끄는 동안 선택은 그린 직사각형의 「안의 노트」다. 누른 채 휠로 내용을 2000틱 옮기면 직사각형은
/// (누른 점 틱 100 ↔ 포인터 아래 틱 약 4000)으로 다시 그려져 N4·N5를 덮는다. 포인터를 움직이지 않고 떼면 선택도
/// 그 직사각형과 같아야 한다.
#[gpui_kit::test]
fn wheel_scroll_then_release_without_move_selects_drawn_band(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv2-scroll-release", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 59);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    wheel(cx, handle, to, -100.0, 0.0);
    let right = band_right_tick(cx, &view);
    assert!(
        right > N4.0 as f64 + 100.0,
        "직사각형이 N4 위까지 그려진다: {right}"
    );
    let shown = selection_of(cx, &view);
    up(cx, handle, to, none());
    let after = selection_of(cx, &view);
    assert_eq!(
        (shown, after),
        (
            sorted(vec![N1, N2, N3, N4, N5]),
            sorted(vec![N1, N2, N3, N4, N5])
        ),
        "그린 직사각형(틱 100–{right:.0})과 선택이 다르다"
    );
}

// ---------- 시도 2: 누른 채 Cmd+휠 줌 ----------

/// 결정 3절 5: 누른 점은 줌 뒤에도 내용(틱 100)에 붙어 있다.
#[gpui_kit::test]
fn cmd_wheel_zoom_during_band_keeps_press_point_on_content(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv2-zoom", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 59);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    let before = viewport(cx, &view).px_per_tick;
    // 확대 두 단계: 틱 100은 왼쪽 밖으로 나간다
    wheel_mods(cx, handle, to, 0.0, 40.0, cmd());
    let vp = viewport(cx, &view);
    assert!(vp.px_per_tick > before, "{vp:?}");
    assert!(
        vp.first_tick > 100.0,
        "누른 점이 화면 밖으로 나가야 한다: {vp:?}"
    );
    drag_to(cx, handle, to + point(px(1.0), px(0.0)), none());
    let sel = selection_of(cx, &view);
    up(cx, handle, to + point(px(1.0), px(0.0)), none());
    assert!(sel.contains(&N1), "줌 뒤 누른 점의 N1이 빠졌다: {sel:?}");
}

// ---------- 시도 3: 누른 채 세로 스크롤 ----------

/// 누른 점의 행(음 64)은 세로 스크롤 뒤에도 내용에 붙어 있다. 행이 화면에 붙어 미끄러지면 N3(음 64)이 빠진다.
#[gpui_kit::test]
fn vertical_scroll_during_band_keeps_press_row_on_content(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv2-vscroll", Preset::Default);
    let from = spot(cx, handle, &view, 100, 64);
    let to = spot(cx, handle, &view, 2000, 62);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), vec![N2, N3]);
    let first = viewport(cx, &view).first_row;
    let row_height = viewport(cx, &view).row_height;
    wheel(cx, handle, to, 0.0, -4.0 * row_height);
    let moved = viewport(cx, &view).first_row - first;
    assert!(moved.abs() >= 2.0, "세로 스크롤 {moved}");
    drag_to(cx, handle, to + point(px(1.0), px(0.0)), none());
    let sel = selection_of(cx, &view);
    up(cx, handle, to + point(px(1.0), px(0.0)), none());
    assert!(
        sel.contains(&N3),
        "누른 행(음 64)의 N3이 빠졌다(스크롤 {moved}행): {sel:?}"
    );
}

// ---------- 시도 4: 누른 채 모드를 다녀온다 ----------

/// 결정 2절 1(고친 글): 누른 채 모드가 바뀌면 조작을 버리고 그 뒤 뗌은 아무 일도 하지 않는다. 선택된 노트를 누른
/// 채(뗄 때 줄이기 대기) Step에 갔다 돌아와 떼도 선택은 줄지 않는다. 빈 곳 누름도 떼어 선택을 비우지 않는다.
#[gpui_kit::test]
fn mode_round_trip_during_press_makes_release_inert(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv2-roundtrip", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    let p2 = on(cx, handle, &view, N2);
    click(cx, handle, p1, none());
    click(cx, handle, p2, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N2]);
    down(cx, handle, p2, none());
    mode_event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    mode_event(cx, handle, &view, ModeEvent::ToNormal);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        Mode::Normal
    );
    assert!(!gesture_active(cx, &view));
    up(cx, handle, p2, none());
    let after_note = selection_of(cx, &view);
    let empty = spot(cx, handle, &view, 2500, 70);
    click(cx, handle, p1, none());
    click(cx, handle, p2, cmd());
    down(cx, handle, empty, none());
    mode_event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    mode_event(cx, handle, &view, ModeEvent::ToNormal);
    up(cx, handle, empty, none());
    let after_empty = selection_of(cx, &view);
    // 모드 전환은 선택을 그대로 둔다(`ModeState::with_mode`). 그러니 뗌이 아무 일도 하지 않으면 [N1, N2]가 남는다.
    assert!(has_selection(cx, &view));
    assert_eq!(
        (after_note, after_empty),
        (vec![N1, N2], vec![N1, N2]),
        "모드를 다녀온 뒤의 뗌이 선택을 바꾼다"
    );
}

// ---------- 시도 5: 선택만 바꾸는 FSM 사건은 조작을 버리지 않는다 ----------

/// `apply_mode_event(ClearSelection)`은 모드·뷰가 그대로라 러버밴드를 버리지 않는다. 그 뒤 끌기가 러버밴드를 잇고
/// `Esc`는 여전히 조작만 취소한다.
#[gpui_kit::test]
fn selection_only_event_keeps_band_alive(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv2-select-event", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 59);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    mode_event(cx, handle, &view, ModeEvent::ClearSelection);
    assert!(gesture_active(cx, &view), "선택 사건이 조작을 버렸다");
    drag_to(cx, handle, to + point(px(1.0), px(0.0)), none());
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    assert!(has_selection(cx, &view));
    up(cx, handle, to + point(px(1.0), px(0.0)), none());
}

// ---------- 시도 6: Step에서 문서가 바뀌는 경로(cancel → live_gesture) ----------

/// 문서 변경 취소(`selection_document_changed`)는 이제 `live_gesture`를 거친다. Normal에서 러버밴드 중 undo로
/// 문서가 바뀌면 조작은 취소 상태로 남고(뗌까지 끌기 무시), 그 뒤 끌기는 선택을 바꾸지 않는다.
#[gpui_kit::test]
fn undo_during_band_then_drag_is_ignored(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv2-undo", Preset::Default);
    // undo할 편집 하나: N6 더블클릭 삭제
    let p6 = on(cx, handle, &view, N6);
    double_click(cx, handle, p6, none());
    assert!(!clip_of(cx, &view, 0).2.contains(&N6));
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 59);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    press(cx, handle, "cmd-z");
    assert!(clip_of(cx, &view, 0).2.contains(&N6), "undo가 되지 않았다");
    let after_undo = selection_of(cx, &view);
    let far = spot(cx, handle, &view, 6000, 55);
    drag_to(cx, handle, far, none());
    assert_eq!(
        selection_of(cx, &view),
        after_undo,
        "취소된 러버밴드가 끌기로 되살아난다"
    );
    up(cx, handle, far, none());
    assert!(!gesture_active(cx, &view));
}

// ---------- 시도 7: band_between의 경계 ----------

/// 폭 0 넓히기 `lo + 1`이 틱 상한에서 넘친다(디버그 빌드는 패닉). 공개 함수라 직접 부른다.
#[test]
fn band_between_at_tick_ceiling_does_not_overflow() {
    use keyscore_app::pianoroll::mouse::{Anchor, band_between};
    let a = Anchor {
        tick: 1.0e30,
        row: 60.0,
    };
    let r = std::panic::catch_unwind(|| band_between(a, a));
    assert!(r.is_ok(), "band_between이 틱 상한에서 패닉한다");
}

/// 폭 0 넓히기는 정수 틱에서 한 틱이다(리뷰 1 제안 5 수락분).
#[test]
fn band_between_zero_width_is_one_tick() {
    use keyscore_app::pianoroll::mouse::{Anchor, band_between};
    let a = Anchor {
        tick: 500.0,
        row: 60.5,
    };
    let b = Anchor {
        tick: 500.0,
        row: 70.2,
    };
    let band = band_between(a, b).unwrap();
    assert_eq!((band.lo.get(), band.hi.get()), (500, 501));
    assert_eq!((band.low.get(), band.high.get()), (127 - 70, 127 - 60));
}
