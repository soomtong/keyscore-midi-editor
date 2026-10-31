//! S3-T6a 적대적 리뷰 3회차의 깨뜨리기 시험(`docs/reports/s3-t6a/review-3.md`). 픽스처는 `review_s3_t6a_r2.rs`에서
//! 복사했다(클립 A [0, 2마디), 틈, 클립 B [3, 4마디), 0.05 px/틱).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    dead_code
)]

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Entity, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PinchEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent,
    TestAppContext, TouchPhase, VisualTestContext, point, px,
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

fn pinch(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, delta: f32) {
    VisualTestContext::from_window(handle, cx).simulate_event(PinchEvent {
        position: at,
        delta,
        modifiers: none(),
        phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
}

/// 러버밴드(틱 100·음 72 → 틱 2000·음 59)를 끌어 둔다. 선택은 [N1, N2, N3].
fn band_100_2000(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    modifiers: Modifiers,
) -> Point<Pixels> {
    let from = spot(cx, handle, view, 100, 72);
    let to = spot(cx, handle, view, 2000, 59);
    down(cx, handle, from, modifiers);
    drag_to(cx, handle, to, modifiers);
    to
}

// ---------- 시도 1: 핀치 축소 뒤 움직이지 않고 뗀다 ----------

/// 핀치도 `on_view_change`를 부르는가. 첫 틱이 0이라 축소하면 왼쪽 끝이 0에 걸려 포인터 아래 틱이 2000 → 4000으로
/// 바뀐다. 포인터를 움직이지 않아도 선택이 N4·N5까지 넓어지고 뗀 뒤에도 같아야 한다.
#[gpui_kit::test]
fn pinch_zoom_out_during_band_recomputes_without_move(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv3-pinch", Preset::Default);
    assert!(viewport(cx, &view).first_tick.abs() < 1e-9);
    let to = band_100_2000(cx, handle, &view, none());
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    pinch(cx, handle, to, -0.5);
    let right = band_right_tick(cx, &view);
    assert!(right > 3900.0, "직사각형 오른쪽 {right}");
    let shown = selection_of(cx, &view);
    up(cx, handle, to, none());
    let want = sorted(vec![N1, N2, N3, N4, N5]);
    assert_eq!((shown, selection_of(cx, &view)), (want.clone(), want));
}

// ---------- 시도 2: Cmd+휠 축소 뒤 움직이지 않고 뗀다 ----------

/// `Cmd+휠` 줌 길(스크롤과 같은 처리기 안의 다른 가지)도 다시 계산한다.
#[gpui_kit::test]
fn cmd_wheel_zoom_out_during_band_recomputes_without_move(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv3-cmdwheel", Preset::Default);
    let to = band_100_2000(cx, handle, &view, none());
    // 아래로 세 줄 = 축소 세 단계(0.05 → 0.0256 px/틱): 포인터 아래 틱 약 3906
    wheel_mods(cx, handle, to, 0.0, -60.0, cmd());
    let right = band_right_tick(cx, &view);
    assert!(right > 3850.0, "직사각형 오른쪽 {right}");
    let shown = selection_of(cx, &view);
    up(cx, handle, to, none());
    let want = sorted(vec![N1, N2, N3, N4, N5]);
    assert_eq!((shown, selection_of(cx, &view)), (want.clone(), want));
}

// ---------- 시도 3: 키보드 줌(cmd--)은 on_view_change를 거치지 않는다 ----------

/// 결정 3절 5 「끄는 동안 선택 강조가 실시간으로 바뀐다」. 누른 채 `cmd--`(`view::ZoomOut`) 세 번이면 직사각형은
/// 틱 100–약 3900으로 다시 그려진다. 끄는 동안의 선택이 그 직사각형과 같은가, 뗀 뒤는 같은가.
#[gpui_kit::test]
fn keyboard_zoom_during_band_updates_preview(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv3-keyzoom", Preset::Default);
    let to = band_100_2000(cx, handle, &view, none());
    press(cx, handle, "cmd--");
    press(cx, handle, "cmd--");
    press(cx, handle, "cmd--");
    assert!(gesture_active(cx, &view), "키 줌이 조작을 버렸다");
    let right = band_right_tick(cx, &view);
    assert!(right > 3850.0, "직사각형 오른쪽 {right}");
    let shown = selection_of(cx, &view);
    up(cx, handle, to, none());
    let after = selection_of(cx, &view);
    let want = sorted(vec![N1, N2, N3, N4, N5]);
    assert_eq!(after, want, "뗀 뒤 선택이 그린 직사각형과 다르다");
    assert_eq!(
        shown, want,
        "키 줌 뒤 끄는 동안의 선택이 그린 직사각형(틱 100–{right:.0})과 다르다"
    );
}

// ---------- 시도 4: Esc로 취소한 뒤 휠 ----------

/// `Esc`는 누르기 전 선택으로 되돌리고 조작을 취소 상태로 둔다. 그 뒤 휠의 `on_view_change`나 뗌이 러버밴드를
/// 되살리면 안 된다.
#[gpui_kit::test]
fn wheel_after_escape_does_not_revive_band(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv3-esc-wheel", Preset::Default);
    let p6 = on(cx, handle, &view, N6);
    click(cx, handle, p6, none());
    assert_eq!(selection_of(cx, &view), vec![N6]);
    let to = band_100_2000(cx, handle, &view, none());
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    press(cx, handle, "escape");
    assert_eq!(selection_of(cx, &view), vec![N6]);
    wheel(cx, handle, to, -100.0, 0.0);
    pinch(cx, handle, to, -0.3);
    assert_eq!(
        selection_of(cx, &view),
        vec![N6],
        "휠이 취소된 밴드를 되살렸다"
    );
    up(cx, handle, to, none());
    assert_eq!(
        selection_of(cx, &view),
        vec![N6],
        "뗌이 취소된 밴드를 되살렸다"
    );
    assert!(!band_rect(cx, &view));
}

// ---------- 시도 5: 문턱 전(Empty)에 휠 뒤 뗌 ----------

/// 빈 곳을 누르고 끌지 않은 채(문턱 전) 휠로 내용을 옮긴 뒤 떼면 빈 곳 클릭이다: 선택이 비고 러버밴드는 없다.
/// `on_view_change`가 `Empty`를 밴드로 바꾸지 않아야 한다.
#[gpui_kit::test]
fn wheel_before_threshold_then_release_is_plain_click(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv3-empty-wheel", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let empty = spot(cx, handle, &view, 2500, 70);
    down(cx, handle, empty, none());
    wheel(cx, handle, empty, -200.0, 0.0);
    assert!(!band_rect(cx, &view), "문턱 전 휠이 러버밴드를 만들었다");
    assert_eq!(selection_of(cx, &view), vec![N1]);
    up(cx, handle, empty, none());
    assert!(selection_of(cx, &view).is_empty());
    assert!(!has_selection(cx, &view));
}

// ---------- 시도 6: Cmd+드래그 중 스크롤 왕복 ----------

/// `Cmd+드래그`는 누를 때 선택에 더한다. 다시 계산할 때마다 `before`에서 새로 합치므로, 스크롤로 넓혔다가 되돌리면
/// 넓힌 동안 들어온 노트(N4·N5)가 빠지고 처음 선택(N6)은 남아야 한다.
#[gpui_kit::test]
fn cmd_band_scroll_round_trip_does_not_accumulate(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv3-cmd-roundtrip", Preset::Default);
    let p6 = on(cx, handle, &view, N6);
    click(cx, handle, p6, none());
    let to = band_100_2000(cx, handle, &view, cmd());
    assert_eq!(selection_of(cx, &view), sorted(vec![N1, N2, N3, N6]));
    wheel(cx, handle, to, -100.0, 0.0);
    assert_eq!(
        selection_of(cx, &view),
        sorted(vec![N1, N2, N3, N4, N5, N6])
    );
    wheel(cx, handle, to, 100.0, 0.0);
    let back = selection_of(cx, &view);
    up(cx, handle, to, cmd());
    assert_eq!(
        (back, selection_of(cx, &view)),
        (sorted(vec![N1, N2, N3, N6]), sorted(vec![N1, N2, N3, N6]))
    );
}

// ---------- 시도 7: 조작이 없을 때 휠 ----------

/// 러버밴드를 뗀 뒤(조작 없음)와 노트 누름(줄이기 대기) 중의 휠은 선택을 바꾸지 않는다. 노트 누름 중 휠 뒤 뗌은
/// 여전히 그 노트 하나로 줄인다.
#[gpui_kit::test]
fn wheel_without_band_leaves_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv3-no-band", Preset::Default);
    let to = band_100_2000(cx, handle, &view, none());
    up(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    wheel(cx, handle, to, -100.0, 0.0);
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    wheel(cx, handle, to, 100.0, 0.0);
    let p2 = on(cx, handle, &view, N2);
    down(cx, handle, p2, none());
    wheel(cx, handle, p2, 0.0, -30.0);
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    up(cx, handle, p2, none());
    assert_eq!(selection_of(cx, &view), vec![N2]);
}

// ---------- 시도 8: 누른 채 Step으로 간 뒤 휠 ----------

/// 조작이 버려진 뒤(Step 피아노롤)의 휠은 `live_gesture`에서 걸러져 선택을 바꾸지 않는다.
#[gpui_kit::test]
fn wheel_in_step_after_band_does_not_select(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv3-step-wheel", Preset::Default);
    let to = band_100_2000(cx, handle, &view, none());
    mode_event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    let in_step = selection_of(cx, &view);
    wheel(cx, handle, to, -100.0, 0.0);
    assert_eq!(selection_of(cx, &view), in_step);
    mode_event(cx, handle, &view, ModeEvent::ToNormal);
    wheel(cx, handle, to, 100.0, 0.0);
    up(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), in_step);
    assert!(!gesture_active(cx, &view));
}

// ---------- 시도 9: band_between의 포화 경계 ----------

/// 폭 0 넓히기는 「한 틱」이다(결정 3절 5). 틱 상한에서 `saturating_add`면 `[MAX, MAX)`로 빈 범위가 된다.
#[test]
fn band_between_at_tick_ceiling_is_not_empty() {
    use keyscore_app::pianoroll::mouse::{Anchor, band_between};
    let a = Anchor {
        tick: 1.0e30,
        row: 60.0,
    };
    let band = band_between(a, a).unwrap();
    assert!(
        band.lo < band.hi,
        "빈 범위: [{}, {})",
        band.lo.get(),
        band.hi.get()
    );
}
