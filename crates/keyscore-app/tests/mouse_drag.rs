//! S3-T6b 드래그 이동·조절(REQ 7.4 이동·조절 표, MS-1·MS-2·MS-3·MS-6). 헤드리스 창에 마우스 이벤트를 보낸다.
//! 규칙은 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 6절, 시험 목록은 `docs/reports/s3-t6b/plan.md`.
//! MIDI는 쓰지 않는다(미리듣기는 `mouse_drag_preview.rs`).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Capslock, Entity, Modifiers, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
};
use keyscore_app::actions::args::{LengthCode, MouseSnap, Opt, SettingsSection};
use keyscore_app::actions::settings;
use keyscore_app::app_state::AppSettings;
use keyscore_app::clipboard;
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::{Cursor, Viewport};
use keyscore_app::settings_window;
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
/// 좁은 노트: 0.05 px/틱에서 6 px라 끝 영역이 2 px다.
const N7: Triple = (6720, 120, 70);
const A_NOTES: [Triple; 7] = [N1, N2, N3, N4, N5, N6, N7];
const B_START: u64 = 3 * BAR;
const B1: Triple = (0, 960, 60);
const B2: Triple = (960, 960, 64);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-drag-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(cx, &format!("ks-test-{}-drag-{name}", std::process::id()));
        // 스냅 격자(길이 코드): 8분. 미리듣기는 MIDI 없이 돈다.
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

/// 피아노롤에 들어가 가로 배율을 0.05 px/틱(4분 = 48 px, 8분 = 24 px)으로 줄인다.
fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
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

/// 노트 가운데(본체).
fn mid(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    n: Triple,
) -> Point<Pixels> {
    spot(cx, handle, view, n.0 + u64::from(n.1) / 2, n.2)
}

/// 같은 행에서 `dt`틱, `dp`반음 떨어진 자리.
fn moved_by(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    from: Point<Pixels>,
    dt: i64,
    dp: i32,
) -> Point<Pixels> {
    let vp = viewport(cx, view);
    from + point(
        px((dt as f64 * vp.px_per_tick) as f32),
        px(-(dp as f32) * vp.row_height),
    )
}

fn mods(platform: bool, shift: bool, alt: bool) -> Modifiers {
    Modifiers {
        platform,
        shift,
        alt,
        ..Modifiers::default()
    }
}

fn none() -> Modifiers {
    Modifiers::default()
}

fn cmd() -> Modifiers {
    mods(true, false, false)
}

fn down(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, modifiers: Modifiers) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers,
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    render(cx, handle);
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

fn up(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, modifiers: Modifiers) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers,
        click_count: 1,
    });
    cx.run_until_parked();
    render(cx, handle);
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

/// 누르고, 문턱을 넘겨 `to`까지 끌고, 뗀다.
fn drag(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    from: Point<Pixels>,
    to: Point<Pixels>,
    modifiers: Modifiers,
) {
    down(cx, handle, from, none());
    drag_to(cx, handle, to, modifiers);
    up(cx, handle, to, modifiers);
}

fn modifiers_changed(cx: &mut TestAppContext, handle: AnyWindowHandle, modifiers: Modifiers) {
    VisualTestContext::from_window(handle, cx).simulate_event(ModifiersChangedEvent {
        modifiers,
        capslock: Capslock::default(),
    });
    cx.run_until_parked();
    render(cx, handle);
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

fn a_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    sorted(clip_of(cx, view, 0).2)
}

/// `from`의 노트를 `to`로 바꾼 클립 A 노트(정렬).
fn a_with(changes: &[(Triple, Triple)]) -> Vec<Triple> {
    let mut v: Vec<Triple> = A_NOTES.to_vec();
    for (from, to) in changes {
        let i = v.iter().position(|n| n == from).unwrap();
        v[i] = *to;
    }
    sorted(v)
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

fn gesture_active(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).piano_roll().gesture_active())
}

/// 고스트 사각형마다 (절대 틱, 음높이). 사각형 왼쪽 위를 내용 위치로 되돌린다.
fn ghosts(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u8)> {
    let vp = viewport(cx, view);
    let rects = cx.read(|cx| view.read(cx).piano_roll().ghost_rects());
    let mut v: Vec<(u64, u8)> = rects
        .iter()
        .map(|r| {
            let tick = (vp.first_tick + f64::from(r.x) / vp.px_per_tick).round() as u64;
            let row = (vp.first_row + r.y / vp.row_height).round() as u8;
            (tick, 127 - row)
        })
        .collect();
    v.sort_unstable();
    v
}

fn ghost_blocked(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).piano_roll().ghost_blocked())
}

fn last_message(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .last()
            .map(ToString::to_string)
            .unwrap_or_default()
    })
}

/// N1·N2·N3을 클릭과 `Cmd+클릭`으로 고른다.
fn select_three(cx: &mut TestAppContext, handle: AnyWindowHandle, view: &Entity<ProjectWindow>) {
    let p1 = mid(cx, handle, view, N1);
    click(cx, handle, p1, none());
    let p2 = mid(cx, handle, view, N2);
    click(cx, handle, p2, cmd());
    let p3 = mid(cx, handle, view, N3);
    click(cx, handle, p3, cmd());
    assert_eq!(selection_of(cx, view), vec![N1, N2, N3]);
}

// ---------- 본체 이동 ----------

#[gpui_kit::test]
fn drag_selection_keeps_relative_positions_with_one_undo(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "relative");
    select_three(cx, handle, &view);
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 2);
    drag(cx, handle, from, to, none());
    let moved = [
        (N1, (960, 960, 62)),
        (N2, (1920, 960, 64)),
        (N3, (2880, 480, 66)),
    ];
    assert_eq!(a_notes(cx, &view), a_with(&moved), "모두 같은 Δ(MS-2)");
    assert_eq!(
        selection_of(cx, &view),
        sorted(moved.iter().map(|m| m.1).collect())
    );
    assert_eq!(
        undo_len(cx, &view),
        undo_before + 1,
        "드래그 한 번 = undo 하나"
    );
    press(cx, handle, "cmd-z");
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
}

#[gpui_kit::test]
fn ghost_shows_result_while_document_stays(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "ghost");
    select_three(cx, handle, &view);
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 2);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(
        a_notes(cx, &view),
        sorted(A_NOTES.to_vec()),
        "끄는 중 문서 그대로"
    );
    assert_eq!(ghosts(cx, &view), vec![(960, 62), (1920, 64), (2880, 66)]);
    assert!(!ghost_blocked(cx, &view));
    up(cx, handle, to, none());
    assert!(ghosts(cx, &view).is_empty());
    assert!(!gesture_active(cx, &view));
}

#[gpui_kit::test]
fn escape_during_drag_cancels_and_release_does_nothing(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "esc");
    select_three(cx, handle, &view);
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 2);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    press(cx, handle, "escape");
    assert!(ghosts(cx, &view).is_empty());
    assert!(!gesture_active(cx, &view));
    assert_eq!(
        selection_of(cx, &view),
        vec![N1, N2, N3],
        "Esc는 조작만 취소"
    );
    let further = moved_by(cx, &view, from, 1920, 3);
    drag_to(cx, handle, further, none());
    up(cx, handle, further, none());
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    assert_eq!(undo_len(cx, &view), undo_before);
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
}

#[gpui_kit::test]
fn cmd_during_drag_disables_snap(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmd");
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 700, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    // 960 + 700 = 1660 → 가까운 8분 격자 1440
    assert_eq!(ghosts(cx, &view), vec![(1440, 62)]);
    modifiers_changed(cx, handle, cmd());
    assert_eq!(
        ghosts(cx, &view),
        vec![(1660, 62)],
        "수식키만 바꿔도 다시 계산"
    );
    modifiers_changed(cx, handle, none());
    assert_eq!(ghosts(cx, &view), vec![(1440, 62)]);
    drag_to(cx, handle, to, cmd());
    up(cx, handle, to, cmd());
    assert_eq!(a_notes(cx, &view), a_with(&[(N2, (1660, 960, 62))]));
}

#[gpui_kit::test]
fn shift_drag_moves_on_one_axis(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "shift");
    let shift = mods(false, true, false);
    // 가로 50 px, 세로 한 행 → 시간만
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 1000, 1);
    drag(cx, handle, from, to, shift);
    assert_eq!(a_notes(cx, &view), a_with(&[(N2, (1920, 960, 62))]));
    // 가로 5 px, 세로 세 행 → 음높이만
    let from = mid(cx, handle, &view, N6);
    let to = moved_by(cx, &view, from, 100, 3);
    drag(cx, handle, from, to, shift);
    assert_eq!(
        a_notes(cx, &view),
        a_with(&[(N2, (1920, 960, 62)), (N6, (5760, 960, 68))])
    );
}

#[gpui_kit::test]
fn alt_drag_duplicates_with_one_undo(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "alt");
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    let alt = mods(false, false, true);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, alt);
    assert_eq!(ghosts(cx, &view), vec![(1920, 60)]);
    up(cx, handle, to, alt);
    let mut expected = A_NOTES.to_vec();
    expected.push((1920, 960, 60));
    assert_eq!(a_notes(cx, &view), sorted(expected), "원본이 남는다");
    assert_eq!(
        selection_of(cx, &view),
        vec![(1920, 960, 60)],
        "사본이 선택"
    );
    assert_eq!(undo_len(cx, &view), undo_before + 1);
    press(cx, handle, "cmd-z");
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
}

// ---------- 끝 조절 ----------

#[gpui_kit::test]
fn right_edge_drag_moves_all_ends_by_same_amount(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "right");
    select_three(cx, handle, &view);
    // N2 오른쪽 끝(1920) 2 px 안쪽을 잡고 600틱 왼쪽으로: 끝 1320 → 1440, Δ −480
    let from = spot(cx, handle, &view, 1880, 62);
    let to = moved_by(cx, &view, from, -600, 0);
    drag(cx, handle, from, to, none());
    // N3(길이 480)은 0이 되므로 그대로
    let changed = [(N1, (0, 480, 60)), (N2, (960, 480, 62))];
    assert_eq!(a_notes(cx, &view), a_with(&changed));
    assert_eq!(
        selection_of(cx, &view),
        vec![(0, 480, 60), (960, 480, 62), N3]
    );
}

#[gpui_kit::test]
fn left_edge_drag_moves_start_and_keeps_end(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "left");
    let from = spot(cx, handle, &view, 1000, 62);
    let to = moved_by(cx, &view, from, 500, 0);
    drag(cx, handle, from, to, none());
    assert_eq!(a_notes(cx, &view), a_with(&[(N2, (1440, 480, 62))]));
}

#[gpui_kit::test]
fn narrow_note_edge_zone_is_a_third(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "narrow");
    // 6 px 노트: 왼쪽 2 px는 시작, 가운데는 본체, 오른쪽 2 px는 끝
    let start_zone = spot(cx, handle, &view, N7.0 + 20, N7.2);
    let to = moved_by(cx, &view, start_zone, 60, 0);
    drag(cx, handle, start_zone, to, cmd());
    assert_eq!(a_notes(cx, &view), a_with(&[(N7, (6780, 60, 70))]));
    press(cx, handle, "cmd-z");
    let end_zone = spot(cx, handle, &view, N7.0 + 110, N7.2);
    let to = moved_by(cx, &view, end_zone, 100, 0);
    drag(cx, handle, end_zone, to, cmd());
    assert_eq!(a_notes(cx, &view), a_with(&[(N7, (6720, 220, 70))]));
    press(cx, handle, "cmd-z");
    let body = spot(cx, handle, &view, N7.0 + 60, N7.2);
    let to = moved_by(cx, &view, body, -480, 0);
    drag(cx, handle, body, to, cmd());
    assert_eq!(a_notes(cx, &view), a_with(&[(N7, (6240, 120, 70))]));
}

// ---------- 클립 경계 ----------

#[gpui_kit::test]
fn drag_past_clip_end_extends_clip_by_bars(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "extend");
    let from = mid(cx, handle, &view, N6);
    let to = moved_by(cx, &view, from, 1920, 0);
    drag(cx, handle, from, to, none());
    let (_, length, _) = clip_of(cx, &view, 0);
    assert_eq!(length, 3 * BAR);
    assert_eq!(a_notes(cx, &view), a_with(&[(N6, (7680, 960, 65))]));
    press(cx, handle, "cmd-z");
    let (_, length, _) = clip_of(cx, &view, 0);
    assert_eq!(length, 2 * BAR, "undo 한 번에 클립 길이도");
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
}

#[gpui_kit::test]
fn blocked_drag_shows_blocked_ghost_and_does_not_edit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "blocked");
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N6);
    let to = moved_by(cx, &view, from, 5760, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert!(ghost_blocked(cx, &view), "클립 B와 겹치는 연장");
    assert_eq!(ghosts(cx, &view), vec![(11520, 65)]);
    up(cx, handle, to, none());
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    assert_eq!(undo_len(cx, &view), undo_before);
    assert!(
        last_message(cx, &view).contains("옮기기"),
        "{}",
        last_message(cx, &view)
    );
    assert!(!ghost_blocked(cx, &view));
}

// ---------- 커서·선택 ----------

#[gpui_kit::test]
fn cursor_follows_body_move(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cursor");
    let from = mid(cx, handle, &view, N2);
    down(cx, handle, from, none());
    let pressed = cursor_of(cx, &view);
    let to = moved_by(cx, &view, from, 960, 2);
    drag_to(cx, handle, to, none());
    up(cx, handle, to, none());
    let after = cursor_of(cx, &view);
    assert_eq!(after.tick.get(), pressed.tick.get() + 960);
    assert_eq!(after.pitch.get(), pressed.pitch.get() + 2);
    // 끝 조절이면 커서는 누른 칸에 남는다
    let edge = spot(cx, handle, &view, 1920 + 1880 - 960, 64);
    down(cx, handle, edge, none());
    let pressed = cursor_of(cx, &view);
    let to = moved_by(cx, &view, edge, 480, 0);
    drag_to(cx, handle, to, none());
    up(cx, handle, to, none());
    assert_eq!(
        a_notes(cx, &view),
        a_with(&[(N2, (1920, 1440, 64))]),
        "N2 끝 조절"
    );
    assert_eq!(cursor_of(cx, &view), pressed);
}

#[gpui_kit::test]
fn drag_of_cmd_toggled_out_note_does_nothing(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "toggled");
    let p1 = mid(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let p2 = mid(cx, handle, &view, N2);
    click(cx, handle, p2, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N2]);
    let undo_before = undo_len(cx, &view);
    // `Cmd`로 눌러 N2를 빼고 끈다
    down(cx, handle, p2, cmd());
    let to = moved_by(cx, &view, p2, 960, 2);
    drag_to(cx, handle, to, cmd());
    assert!(ghosts(cx, &view).is_empty());
    up(cx, handle, to, cmd());
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    assert_eq!(undo_len(cx, &view), undo_before);
    assert_eq!(selection_of(cx, &view), vec![N1]);
}

#[gpui_kit::test]
fn document_change_during_drag_cancels(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "undo");
    let p6 = mid(cx, handle, &view, N6);
    click(cx, handle, p6, none());
    press(cx, handle, "alt-k");
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(ghosts(cx, &view), vec![(1920, 62)]);
    press(cx, handle, "cmd-z");
    assert!(ghosts(cx, &view).is_empty());
    assert!(!gesture_active(cx, &view));
    let further = moved_by(cx, &view, from, 1920, 0);
    drag_to(cx, handle, further, none());
    up(cx, handle, further, none());
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()), "undo만 됐다");
}

#[gpui_kit::test]
fn wheel_during_drag_recomputes_ghost(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "wheel");
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert_eq!(ghosts(cx, &view), vec![(1920, 62)]);
    let before = viewport(cx, &view);
    // 내용을 왼쪽으로 48 px(뷰포트는 960틱 오른쪽으로)
    VisualTestContext::from_window(handle, cx).simulate_event(ScrollWheelEvent {
        position: to,
        delta: ScrollDelta::Pixels(point(px(-48.0), px(0.0))),
        modifiers: none(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
    let after = viewport(cx, &view);
    assert!(
        (after.first_tick - before.first_tick - 960.0).abs() < 1e-6,
        "{before:?} {after:?}"
    );
    assert_eq!(
        ghosts(cx, &view),
        vec![(2880, 62)],
        "포인터 아래 내용이 바뀌었다"
    );
    up(cx, handle, to, none());
    assert_eq!(a_notes(cx, &view), a_with(&[(N2, (2880, 960, 62))]));
}

#[gpui_kit::test]
fn mouse_snap_setting_changes_drag_and_double_click_grid(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "snap-setting");
    run(cx, handle, settings::SetMouseSnap(MouseSnap::Sixteenth));
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).input.mouse_snap),
        MouseSnap::Sixteenth
    );
    // 960 + 700 = 1660 → 16분 격자 1680
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 700, 0);
    drag(cx, handle, from, to, none());
    assert_eq!(a_notes(cx, &view), a_with(&[(N2, (1680, 960, 62))]));
    // 더블클릭 생성: 시작은 16분 격자로 내림(2200 → 2160), 길이는 길이 코드(8분)
    let at = spot(cx, handle, &view, 2200, 72);
    click(cx, handle, at, none());
    VisualTestContext::from_window(handle, cx).simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.run_until_parked();
    VisualTestContext::from_window(handle, cx).simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: none(),
        click_count: 2,
    });
    cx.run_until_parked();
    render(cx, handle);
    assert!(
        a_notes(cx, &view).contains(&(2160, 480, 72)),
        "{:?}",
        a_notes(cx, &view)
    );
}

#[gpui_kit::test]
fn drag_in_step_mode_does_nothing(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "step");
    press(cx, handle, "enter");
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 2);
    drag(cx, handle, from, to, none());
    assert!(ghosts(cx, &view).is_empty());
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    assert_eq!(undo_len(cx, &view), undo_before);
}

#[gpui_kit::test]
fn settings_input_has_mouse_snap_choice(cx: &mut TestAppContext) {
    let (handle, _view) = setup(cx, "settings-ui");
    run(
        cx,
        handle,
        settings::Open(Opt(Some(SettingsSection::Input))),
    );
    let (sh, sv) = cx
        .update(|cx| settings_window::current(cx))
        .expect("설정 창");
    cx.update_window(sh, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, sh);
    let label = |cx: &mut TestAppContext| {
        cx.update_window(sh, |_, window, _| {
            window
                .try_find("settings-mouse-snap")
                .and_then(|e| e.label().map(str::to_owned))
        })
        .unwrap()
    };
    assert!(
        label(cx).is_some_and(|l| l.contains("길이 코드")),
        "{:?}",
        label(cx)
    );
    // 레이턴시 보정·Step 입력의 두 상자 뒤
    for key in "tab tab tab tab tab tab".split_whitespace() {
        press(cx, sh, key);
    }
    let focused = cx
        .update_window(sh, |_, window, cx| sv.read(cx).focused_control(window, cx))
        .unwrap();
    assert_eq!(focused.as_deref(), Some("settings-mouse-snap"));
    for key in "enter down down down enter".split_whitespace() {
        press(cx, sh, key);
    }
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).input.mouse_snap),
        MouseSnap::Sixteenth
    );
    assert!(
        label(cx).is_some_and(|l| l.contains("1/16")),
        "{:?}",
        label(cx)
    );
}
