//! S3-T6a 마우스 선택(REQ 7.4 선택 표, MS-1 러버밴드 미리보기, MS-5, MS-7). 헤드리스 창에 마우스 이벤트를
//! 보낸다. 규칙은 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 2–5절, 시험 목록은
//! `docs/reports/s3-t6a/plan.md`. MIDI는 쓰지 않는다.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

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

// ---------- 클릭 ----------

#[gpui_kit::test]
fn click_note_selects_only_that_note(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "click-note", Preset::Default);
    let p = on(cx, handle, &view, N1);
    click(cx, handle, p, none());
    let p = on(cx, handle, &view, N2);
    down(cx, handle, p, cmd());
    up(cx, handle, p, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N2]);
    let p = on(cx, handle, &view, N3);
    click(cx, handle, p, none());
    assert_eq!(selection_of(cx, &view), vec![N3]);
    assert_eq!(
        cursor_of(cx, &view).pitch.get(),
        64,
        "커서가 누른 칸으로(MS-5)"
    );
    assert!(has_selection(cx, &view));
}

#[gpui_kit::test]
fn click_empty_clears_selection_and_moves_cursor(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "click-empty", Preset::Default);
    let p = on(cx, handle, &view, N1);
    click(cx, handle, p, none());
    assert!(has_selection(cx, &view));
    let p = spot(cx, handle, &view, 2500, 70);
    click(cx, handle, p, none());
    assert_eq!(selection_of(cx, &view), vec![]);
    assert!(!has_selection(cx, &view));
    let c = cursor_of(cx, &view);
    assert_eq!(c.pitch.get(), 70);
    assert!((2480..=2520).contains(&c.tick.get()), "{c:?}");
}

#[gpui_kit::test]
fn click_selected_note_keeps_selection_until_release(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "click-selected", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let p2 = on(cx, handle, &view, N2);
    click(cx, handle, p2, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N2]);
    down(cx, handle, p2, none());
    assert_eq!(
        selection_of(cx, &view),
        vec![N1, N2],
        "누르는 동안은 선택이 그대로(끌면 전체를 옮긴다)"
    );
    up(cx, handle, p2, none());
    assert_eq!(
        selection_of(cx, &view),
        vec![N2],
        "끌지 않고 떼면 그 노트 하나"
    );
}

#[gpui_kit::test]
fn cmd_click_toggles_in_same_clip_and_replaces_in_other_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmd-click", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    let p2 = on(cx, handle, &view, N2);
    click(cx, handle, p1, none());
    click(cx, handle, p2, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N2]);
    click(cx, handle, p1, cmd());
    assert_eq!(selection_of(cx, &view), vec![N2], "같은 클립이면 뺀다");
    let pb = on_b(cx, handle, &view, B2);
    click(cx, handle, pb, cmd());
    assert_eq!(
        selection_of(cx, &view),
        vec![B2],
        "다른 클립의 노트는 그것 하나(선택은 한 클립)"
    );
    // 빈 곳 Cmd+클릭은 선택을 바꾸지 않는다
    let empty = spot(cx, handle, &view, B_START + 2000, 70);
    click(cx, handle, empty, cmd());
    assert_eq!(selection_of(cx, &view), vec![B2]);
}

#[gpui_kit::test]
fn shift_click_extends_by_time_range_from_last_clicked(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "shift-click", Preset::Default);
    let p2 = on(cx, handle, &view, N2);
    click(cx, handle, p2, none());
    let p4 = on(cx, handle, &view, N4);
    click(cx, handle, p4, shift());
    // [960, 4800): N2·N3·N4·N5(음높이 무관). N1은 960에서 끝나 밖
    assert_eq!(selection_of(cx, &view), sorted(vec![N2, N3, N4, N5]));
    // 기준은 여전히 마지막으로 클릭한 N2: [0, 1920)를 더하고 줄이지 않는다
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, shift());
    assert_eq!(selection_of(cx, &view), sorted(vec![N1, N2, N3, N4, N5]));
    // 빈 곳 Shift+클릭: 누른 틱까지
    let empty = spot(cx, handle, &view, 5800, 80);
    click(cx, handle, empty, shift());
    assert_eq!(selection_of(cx, &view), sorted(A_NOTES.to_vec()));
}

#[gpui_kit::test]
fn drag_below_threshold_is_click(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "threshold", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let p = spot(cx, handle, &view, 2500, 70);
    down(cx, handle, p, none());
    drag_to(cx, handle, p + point(px(2.0), px(0.0)), none());
    assert!(!band_rect(cx, &view));
    up(cx, handle, p + point(px(2.0), px(0.0)), none());
    assert_eq!(selection_of(cx, &view), vec![], "문턱 아래는 클릭이다");
}

#[gpui_kit::test]
fn step_mode_click_only_moves_cursor(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "step-click", Preset::Default);
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| {
            this.apply_mode_event(ModeEvent::Enter(Mode::Step), window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    let p = on(cx, handle, &view, N2);
    click(cx, handle, p, none());
    assert_eq!(selection_of(cx, &view), vec![]);
    assert_eq!(cursor_of(cx, &view).pitch.get(), 62);
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
}

#[gpui_kit::test]
fn visual_mode_ends_on_mouse_press(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "visual", Preset::Vim);
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(100),
            pitch: Pitch::new(60).unwrap(),
        });
    });
    press(cx, handle, "v");
    assert!(cx.read(|cx| view.read(cx).piano_roll().visual().is_some()));
    let p = on(cx, handle, &view, N3);
    click(cx, handle, p, none());
    assert!(cx.read(|cx| view.read(cx).piano_roll().visual().is_none()));
    assert_eq!(selection_of(cx, &view), vec![N3]);
}

#[gpui_kit::test]
fn edge_pixel_of_piano_roll_is_clickable(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "edge", Preset::Default);
    let bounds = main_view_bounds(cx, handle);
    let vp = viewport(cx, &view);
    let expected = vp.cell_at(1.0, 1.0).unwrap();
    click(cx, handle, bounds.origin + point(px(1.0), px(1.0)), none());
    assert_eq!(cursor_of(cx, &view), expected, "맨 위 행·왼쪽 끝도 눌린다");
}

// ---------- 러버밴드 ----------

#[gpui_kit::test]
fn rubber_band_selects_notes_inside_and_previews_live(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "band", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 61);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(
        selection_of(cx, &view),
        vec![N2, N3],
        "끄는 동안 선택이 바뀐다(미리보기)"
    );
    assert!(band_rect(cx, &view));
    assert!(gesture_active(cx, &view));
    up(cx, handle, to, none());
    assert!(!band_rect(cx, &view));
    assert!(!gesture_active(cx, &view));
    assert_eq!(selection_of(cx, &view), vec![N2, N3]);
    assert!(has_selection(cx, &view));
}

#[gpui_kit::test]
fn cmd_rubber_band_adds_to_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmd-band", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let from = spot(cx, handle, &view, 3000, 70);
    let to = spot(cx, handle, &view, 6000, 66);
    down(cx, handle, from, cmd());
    drag_to(cx, handle, to, cmd());
    up(cx, handle, to, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N5]);
}

#[gpui_kit::test]
fn escape_during_rubber_band_restores_selection_and_keeps_mode(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "band-esc", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 61);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), vec![N2, N3]);
    press(cx, handle, "escape");
    assert_eq!(selection_of(cx, &view), vec![N1], "누르기 전 선택으로");
    assert!(
        has_selection(cx, &view),
        "Esc는 조작만 취소하고 선택을 지우지 않는다"
    );
    assert!(!band_rect(cx, &view));
    let further = spot(cx, handle, &view, 4000, 59);
    drag_to(cx, handle, further, none());
    up(cx, handle, further, none());
    assert_eq!(selection_of(cx, &view), vec![N1], "취소 뒤 끌기·뗌은 무시");
}

#[gpui_kit::test]
fn rubber_band_across_clips_picks_press_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "band-clips", Preset::Default);
    let from = spot(cx, handle, &view, 5000, 70);
    let to = spot(cx, handle, &view, 13000, 59);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    up(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), vec![N6], "누른 클립 A의 노트만");
}

#[gpui_kit::test]
fn rubber_band_from_gap_moves_cursor_into_selected_clip_and_alt_k_works(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "band-gap", Preset::Default);
    let from = spot(cx, handle, &view, 9000, 70);
    let to = spot(cx, handle, &view, 13000, 59);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    up(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), vec![B1, B2]);
    assert_eq!(
        cursor_of(cx, &view),
        Cursor {
            tick: Tick::new(B_START),
            pitch: Pitch::new(60).unwrap(),
        },
        "커서가 선택의 첫 노트 칸으로"
    );
    press(cx, handle, "alt-k");
    assert_eq!(clip_of(cx, &view, 1).2, vec![(0, 960, 61), (960, 960, 65)]);
}

#[gpui_kit::test]
fn mouse_selection_then_alt_k_transposes(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "alt-k", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let before = undo_len(cx, &view);
    press(cx, handle, "alt-k");
    assert!(clip_of(cx, &view, 0).2.contains(&(0, 960, 61)));
    assert_eq!(selection_of(cx, &view), vec![(0, 960, 61)]);
    assert_eq!(undo_len(cx, &view), before + 1);
    press(cx, handle, "cmd-z");
    assert!(clip_of(cx, &view, 0).2.contains(&N1));
}

#[gpui_kit::test]
fn document_change_during_band_cancels_gesture(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "band-undo", Preset::Default);
    let p6 = on(cx, handle, &view, N6);
    click(cx, handle, p6, none());
    press(cx, handle, "alt-k");
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 61);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(selection_of(cx, &view), vec![N2, N3]);
    press(cx, handle, "cmd-z");
    assert!(!gesture_active(cx, &view));
    assert!(!band_rect(cx, &view));
    // undo는 되살린 노트를 선택으로 둔다(S3-T3a). 그 뒤 움직임이 러버밴드로 덮어쓰지 않는다.
    let after_undo = selection_of(cx, &view);
    assert_eq!(after_undo, vec![N6]);
    let further = spot(cx, handle, &view, 4000, 59);
    drag_to(cx, handle, further, none());
    assert_eq!(
        selection_of(cx, &view),
        after_undo,
        "취소 뒤 움직임은 선택을 바꾸지 않는다"
    );
    up(cx, handle, further, none());
}

#[gpui_kit::test]
fn release_outside_piano_roll_finishes_band(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "band-outside", Preset::Default);
    let from = spot(cx, handle, &view, 100, 72);
    let to = spot(cx, handle, &view, 2000, 61);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    let outside = main_view_bounds(cx, handle).origin - point(px(30.0), px(30.0));
    drag_to(cx, handle, outside, none());
    assert!(
        band_rect(cx, &view),
        "밖에서도 러버밴드는 이어진다(가장자리에서 멈춤)"
    );
    up(cx, handle, outside, none());
    assert!(!band_rect(cx, &view));
    assert!(!gesture_active(cx, &view));
}

// ---------- 더블클릭 ----------

#[gpui_kit::test]
fn double_click_note_deletes_it_with_one_undo(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dbl-note", Preset::Default);
    let before = undo_len(cx, &view);
    let p2 = on(cx, handle, &view, N2);
    double_click(cx, handle, p2, none());
    assert!(!clip_of(cx, &view, 0).2.contains(&N2));
    assert_eq!(undo_len(cx, &view), before + 1);
    press(cx, handle, "cmd-z");
    assert!(clip_of(cx, &view, 0).2.contains(&N2));
}

#[gpui_kit::test]
fn double_click_empty_creates_note_of_length_code_snapped(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dbl-empty", Preset::Default);
    let before = undo_len(cx, &view);
    let p = spot(cx, handle, &view, 700, 70);
    double_click(cx, handle, p, none());
    assert!(
        clip_of(cx, &view, 0).2.contains(&(480, 480, 70)),
        "8분 격자로 내림, 길이 8분"
    );
    assert_eq!(selection_of(cx, &view), vec![(480, 480, 70)]);
    assert_eq!(
        cursor_of(cx, &view),
        Cursor {
            tick: Tick::new(480),
            pitch: Pitch::new(70).unwrap(),
        }
    );
    assert_eq!(undo_len(cx, &view), before + 1);
    press(cx, handle, "cmd-z");
    assert_eq!(clip_of(cx, &view, 0).2, A_NOTES.to_vec());
}

#[gpui_kit::test]
fn cmd_double_click_empty_does_not_snap(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dbl-free", Preset::Default);
    let p = spot(cx, handle, &view, 700, 70);
    let local = p - main_view_bounds(cx, handle).origin;
    let tick = viewport(cx, &view)
        .cell_at(f32::from(local.x), f32::from(local.y))
        .unwrap()
        .tick
        .get();
    double_click(cx, handle, p, cmd());
    assert!(clip_of(cx, &view, 0).2.contains(&(tick, 480, 70)), "{tick}");
}

#[gpui_kit::test]
fn double_click_in_gap_creates_bar_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dbl-gap", Preset::Default);
    let p = spot(cx, handle, &view, 9000, 70);
    double_click(cx, handle, p, none());
    assert_eq!(clip_count(cx, &view), 3);
    let (start, length, notes) = clip_of(cx, &view, 1);
    assert_eq!((start, length), (2 * BAR, BAR));
    assert_eq!(notes, vec![(8640 - 2 * BAR, 480, 70)]);
    assert_eq!(
        clip_of(cx, &view, 2).2,
        vec![B1, B2],
        "뒤 노트를 밀지 않는다"
    );
}

// ---------- 줌 ----------

#[gpui_kit::test]
fn cmd_wheel_zooms_around_pointer(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmd-wheel", Preset::Default);
    let before = viewport(cx, &view);
    let bounds = main_view_bounds(cx, handle);
    let at = bounds.origin + point(px(200.0), px(100.0));
    let tick_under = before.first_tick + 200.0 / before.px_per_tick;
    VisualTestContext::from_window(handle, cx).simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.0), px(20.0))),
        modifiers: cmd(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
    let after = viewport(cx, &view);
    assert!(
        (after.px_per_tick - before.px_per_tick * 1.25).abs() < 1e-9,
        "{after:?}"
    );
    let still = after.first_tick + 200.0 / after.px_per_tick;
    assert!((still - tick_under).abs() < 0.5, "{tick_under} → {still}");
    assert!(
        (after.first_row - before.first_row).abs() < f32::EPSILON,
        "세로 스크롤 없음"
    );
}

#[gpui_kit::test]
fn pinch_zooms_around_pointer(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "pinch", Preset::Default);
    let before = viewport(cx, &view);
    let bounds = main_view_bounds(cx, handle);
    let at = bounds.origin + point(px(300.0), px(100.0));
    let tick_under = before.first_tick + 300.0 / before.px_per_tick;
    VisualTestContext::from_window(handle, cx).simulate_event(PinchEvent {
        position: at,
        delta: 0.5,
        modifiers: none(),
        phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
    let after = viewport(cx, &view);
    assert!(
        (after.px_per_tick - before.px_per_tick * 1.5).abs() < 1e-9,
        "{after:?}"
    );
    let still = after.first_tick + 300.0 / after.px_per_tick;
    assert!((still - tick_under).abs() < 0.5, "{tick_under} → {still}");
}

// ---------- 리뷰 1 지적 4: 빠진 시험 ----------

/// 끌지 않은 누름 중 `Esc`: 선택된 노트를 누른 채 취소하면 떼어도 그 노트 하나로 줄이지 않고, 빈 곳을 누른 채
/// 취소하면 떼어도 선택을 비우지 않는다.
#[gpui_kit::test]
fn escape_during_plain_press_cancels_release_action(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "press-esc", Preset::Default);
    let p1 = on(cx, handle, &view, N1);
    let p2 = on(cx, handle, &view, N2);
    click(cx, handle, p1, none());
    click(cx, handle, p2, cmd());
    down(cx, handle, p2, none());
    press(cx, handle, "escape");
    up(cx, handle, p2, none());
    assert_eq!(
        selection_of(cx, &view),
        vec![N1, N2],
        "취소 뒤 뗌은 줄이지 않는다"
    );
    let empty = spot(cx, handle, &view, 2500, 70);
    down(cx, handle, empty, none());
    press(cx, handle, "escape");
    up(cx, handle, empty, none());
    assert_eq!(
        selection_of(cx, &view),
        vec![N1, N2],
        "취소 뒤 뗌은 비우지 않는다"
    );
    assert!(has_selection(cx, &view));
}

/// 선택이 없을 때 빈 곳 `Shift+클릭`은 보통 클릭과 같다(선택 없음, 커서만). 틈에서도 같다.
#[gpui_kit::test]
fn shift_click_empty_without_base_is_plain_click(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "shift-empty", Preset::Default);
    let empty = spot(cx, handle, &view, 2500, 70);
    click(cx, handle, empty, shift());
    assert_eq!(selection_of(cx, &view), vec![]);
    assert_eq!(cursor_of(cx, &view).pitch.get(), 70);
    let p1 = on(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let gap = spot(cx, handle, &view, 9000, 70);
    click(cx, handle, gap, shift());
    assert_eq!(
        selection_of(cx, &view),
        vec![],
        "틈은 클립이 없어 보통 클릭"
    );
}

/// `Cmd+휠` 줌은 키 줌과 같은 상·하한에서 멈춘다.
#[gpui_kit::test]
fn cmd_wheel_zoom_stops_at_bounds(cx: &mut TestAppContext) {
    use keyscore_app::pianoroll::{MAX_PX_PER_TICK, MIN_PX_PER_TICK};
    let (handle, view) = setup(cx, "zoom-bounds", Preset::Default);
    let at = main_view_bounds(cx, handle).origin + point(px(200.0), px(100.0));
    let wheel = |cx: &mut TestAppContext, dy: f32| {
        VisualTestContext::from_window(handle, cx).simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.0), px(dy))),
            modifiers: cmd(),
            touch_phase: TouchPhase::Moved,
        });
        cx.run_until_parked();
        render(cx, handle);
    };
    wheel(cx, 20_000.0);
    assert!((viewport(cx, &view).px_per_tick - MAX_PX_PER_TICK).abs() < 1e-12);
    wheel(cx, -100_000.0);
    let vp = viewport(cx, &view);
    assert!((vp.px_per_tick - MIN_PX_PER_TICK).abs() < 1e-12, "{vp:?}");
    assert!(vp.first_tick >= 0.0);
}
