//! S3-T6c 트랜스포트 템포·박자표 끌어서 조정(REQ 7.6 「클릭해 입력, 끌어서 조정」). 결정
//! `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절. 헤드리스 창에 누름·이동·뗌 사건을 보낸다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};
use keyscore_app::mode::Mode;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Project, Tempo, Tick, TimeSignature, Track, TrackColor};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-tdrag-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn project() -> Project {
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track
        .insert_clip(Clip::new(Tick::ZERO, 7680).unwrap())
        .unwrap();
    project.push_track(track).unwrap();
    project
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
}

fn bounds(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Bounds<Pixels> {
    cx.update_window(handle, |_, window, _| window.find(id).bounds())
        .unwrap()
}

/// 필드 상자 안의 점(왼쪽에서 `fx`, 위에서 가운데).
fn in_field(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    id: &'static str,
    fx: f32,
) -> Point<Pixels> {
    let b = bounds(cx, handle, id);
    b.origin + point(b.size.width * fx, b.size.height / 2.0)
}

fn cmd() -> Modifiers {
    Modifiers {
        platform: true,
        ..Modifiers::default()
    }
}

fn down(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn move_to(
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

fn up(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    render(cx, handle);
}

fn up_by(at: Point<Pixels>, dy: f32) -> Point<Pixels> {
    at - point(px(0.), px(dy))
}

fn tempo(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Tempo {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .project()
            .tempo_map()
            .tempo_at(Tick::ZERO)
    })
}

fn meter(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> TimeSignature {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .project()
            .meter_map()
            .signature_at(Tick::ZERO)
    })
}

fn bpm(tenths: u32) -> Tempo {
    keyscore_app::transport_drag::tempo_from_tenths(tenths).unwrap()
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

fn tempo_text(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| view.read(cx).tempo_field_text(cx))
}

fn meter_text(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| view.read(cx).meter_field_text(cx))
}

fn dragging(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).transport_drag_active())
}

/// 템포 필드 가운데에서 누르고 위로 `dy` px 끈다(떼지 않음).
fn drag_tempo(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    dy: f32,
    modifiers: Modifiers,
) -> Point<Pixels> {
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, dy);
    move_to(cx, handle, to, modifiers);
    to
}

// ---------- 끌기 ----------

#[gpui_kit::test]
fn dragging_tempo_up_sets_tempo_with_one_undo(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "up");
    let undo_before = undo_len(cx, &view);
    let to = drag_tempo(cx, handle, 40.0, Modifiers::default());
    // 끄는 중: 필드는 새 값, 문서는 그대로
    assert!(dragging(cx, &view));
    assert_eq!(tempo_text(cx, &view), "130.0");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    up(cx, handle, to);
    assert!(!dragging(cx, &view));
    assert_eq!(tempo(cx, &view), bpm(1300));
    assert_eq!(tempo_text(cx, &view), "130.0");
    assert_eq!(undo_len(cx, &view), undo_before + 1);
    press(cx, handle, "cmd-z");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(tempo_text(cx, &view), "120.0");
}

#[gpui_kit::test]
fn cmd_drag_fine_tunes_tempo_without_jump(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "fine");
    let to = drag_tempo(cx, handle, 40.0, cmd());
    assert_eq!(tempo_text(cx, &view), "121.0");
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1210));

    // 끄는 중 `Cmd`를 쥐어도 그 자리에서 값이 튀지 않고, 거기서부터 0.1 BPM 단계
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let mid = up_by(from, 20.0);
    move_to(cx, handle, mid, Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "126.0");
    move_to(cx, handle, mid, cmd());
    assert_eq!(tempo_text(cx, &view), "126.0");
    let to = up_by(mid, 8.0);
    move_to(cx, handle, to, cmd());
    assert_eq!(tempo_text(cx, &view), "126.2");
    // `Cmd`를 놓아도 튀지 않는다
    move_to(cx, handle, to, Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "126.2");
    let to2 = up_by(to, 4.0);
    move_to(cx, handle, to2, Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "127.2");
    up(cx, handle, to2);
    assert_eq!(tempo(cx, &view), bpm(1272));
}

#[gpui_kit::test]
fn drag_down_stops_at_tempo_minimum(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "min");
    let to = drag_tempo(cx, handle, -2000.0, Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "10.0");
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(100));
}

#[gpui_kit::test]
fn meter_drag_left_half_numerator_right_half_denominator(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "meter");
    let from = in_field(cx, handle, "transport-meter", 0.2);
    down(cx, handle, from);
    let to = up_by(from, 4.0);
    move_to(cx, handle, to, Modifiers::default());
    assert_eq!(meter_text(cx, &view), "5/4");
    up(cx, handle, to);
    assert_eq!(meter(cx, &view), TimeSignature::new(5, 4).unwrap());

    let from = in_field(cx, handle, "transport-meter", 0.8);
    down(cx, handle, from);
    let to = up_by(from, 4.0);
    move_to(cx, handle, to, Modifiers::default());
    assert_eq!(meter_text(cx, &view), "5/8");
    up(cx, handle, to);
    assert_eq!(meter(cx, &view), TimeSignature::new(5, 8).unwrap());
}

// ---------- 클릭 ----------

#[gpui_kit::test]
fn click_without_drag_enters_field_with_all_selected(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "click");
    let undo_before = undo_len(cx, &view);
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    up(cx, handle, at);
    assert!(!dragging(cx, &view));
    assert_eq!(undo_len(cx, &view), undo_before);
    // 전체 선택이라 바로 친 글자가 값을 바꾼다
    type_text(cx, handle, "90");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
}

#[gpui_kit::test]
fn small_move_under_threshold_is_click(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "small");
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    let to = up_by(at, 2.0);
    move_to(cx, handle, to, Modifiers::default());
    assert!(!dragging(cx, &view));
    assert_eq!(tempo_text(cx, &view), "120.0");
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    type_text(cx, handle, "95");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(95).unwrap());
}

// ---------- 취소 ----------

#[gpui_kit::test]
fn escape_cancels_tempo_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "esc");
    let undo_before = undo_len(cx, &view);
    let to = drag_tempo(cx, handle, 40.0, Modifiers::default());
    press(cx, handle, "escape");
    assert!(!dragging(cx, &view));
    assert_eq!(tempo_text(cx, &view), "120.0");
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(tempo_text(cx, &view), "120.0");
    assert_eq!(undo_len(cx, &view), undo_before);
}

#[gpui_kit::test]
fn rebound_cancel_drag_key_cancels_tempo_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rebound");
    cx.update(|cx| {
        cx.bind_keys([gpui_kit::KeyBinding::new(
            "f9",
            keyscore_app::actions::edit::CancelDrag,
            Some("Dragging"),
        )]);
    });
    let to = drag_tempo(cx, handle, 40.0, Modifiers::default());
    press(cx, handle, "f9");
    assert!(!dragging(cx, &view));
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

#[gpui_kit::test]
fn document_change_during_drag_cancels(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "docchange");
    let to = drag_tempo(cx, handle, 40.0, Modifiers::default());
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1300));
    let undo_before = undo_len(cx, &view);
    let to = drag_tempo(cx, handle, 20.0, Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "135.0");
    press(cx, handle, "cmd-z");
    assert!(!dragging(cx, &view));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(tempo_text(cx, &view), "120.0");
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(undo_len(cx, &view), undo_before - 1);
}

#[gpui_kit::test]
fn drag_back_to_start_makes_no_edit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "back");
    let undo_before = undo_len(cx, &view);
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 40.0), Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "130.0");
    move_to(cx, handle, from, Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "120.0");
    up(cx, handle, from);
    assert_eq!(undo_len(cx, &view), undo_before);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

// ---------- 그 밖 ----------

#[gpui_kit::test]
fn pressing_focused_field_does_not_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "focused");
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    // 클릭으로 들어가 고치는 중
    down(cx, handle, at);
    up(cx, handle, at);
    // 고치는 중인 필드를 끌면 입력란 동작(값 조정 없음)
    down(cx, handle, at);
    let to = up_by(at, 40.0);
    move_to(cx, handle, to, Modifiers::default());
    assert!(!dragging(cx, &view));
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(tempo_text(cx, &view), "120.0");
}

#[gpui_kit::test]
fn drag_continues_outside_field(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "outside");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    let main = bounds(cx, handle, "main-view");
    down(cx, handle, from);
    // 아래로 끌어 주 뷰 위까지
    let to = point(from.x, main.origin.y + px(40.));
    move_to(cx, handle, to, Modifiers::default());
    assert!(dragging(cx, &view));
    let steps = keyscore_app::transport_drag::drag_steps(f32::from(from.y - to.y));
    let expected = keyscore_app::transport_drag::dragged_tempo(1200, steps, false);
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(expected));
    assert_ne!(expected, 1200);
}

#[gpui_kit::test]
fn drag_works_in_step_mode(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "step");
    press(cx, handle, "enter");
    press(cx, handle, "enter");
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    let to = drag_tempo(cx, handle, 8.0, Modifiers::default());
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1220));
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
}

/// 움직임 사건 없이 먼 자리에서 떼도 그 자리로 계산한다(뗀 자리가 마지막 움직임과 다를 수 있다).
#[gpui_kit::test]
fn release_away_without_move_uses_release_position(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "release-only");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "122.0");
    up(cx, handle, up_by(from, 20.0));
    assert_eq!(tempo(cx, &view), bpm(1250));
}
