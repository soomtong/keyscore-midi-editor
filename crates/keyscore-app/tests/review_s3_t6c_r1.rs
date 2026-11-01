//! S3-T6c 적대적 리뷰 1회차의 깨뜨리기 시도. 결정 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절.
//! 도우미는 `transport_drag.rs`를 본떴다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext,
    point, px,
};
use keyscore_app::actions::palette;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Project, Tempo, Tick, TimeSignature, Track, TrackColor};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-rev-t6c-{}-{name}", std::process::id()));
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
    render(cx, handle);
    (handle, view)
}

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    open(cx, name, project())
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn bounds(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Bounds<Pixels> {
    cx.update_window(handle, |_, window, _| window.find(id).bounds())
        .unwrap()
}

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

fn down_with(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    button: MouseButton,
    click_count: usize,
) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseDownEvent {
        button,
        position: at,
        modifiers: Modifiers::default(),
        click_count,
        first_mouse: false,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn down(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>) {
    down_with(cx, handle, at, MouseButton::Left, 1);
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

fn up_with(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    button: MouseButton,
    modifiers: Modifiers,
    click_count: usize,
) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseUpEvent {
        button,
        position: at,
        modifiers,
        click_count,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn up(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>) {
    up_with(cx, handle, at, MouseButton::Left, Modifiers::default(), 1);
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

fn tempo_focused(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> bool {
    let view = view.clone();
    cx.update_window(handle, |_, window, cx| {
        view.read(cx).transport_tempo_focused(window, cx)
    })
    .unwrap()
}

fn popup_open(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| {
        window.try_find("popup-menu").is_some()
    })
    .unwrap()
}

fn with_tempo(t: Tempo) -> Project {
    let mut p = project();
    p.tempo_map_mut().set(Tick::ZERO, t);
    p
}

fn with_meter(m: TimeSignature) -> Project {
    let mut p = project();
    p.meter_map_mut().set(Tick::ZERO, m);
    p
}

/// 템포 필드 가운데를 누르고 위로 `dy`만큼 끈 뒤 떼고, 끈 끝점을 돌려준다.
fn drag_tempo_and_release(cx: &mut TestAppContext, handle: AnyWindowHandle, dy: f32) {
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, dy);
    move_to(cx, handle, to, Modifiers::default());
    up(cx, handle, to);
}

// ---------- 1. 다른 필드를 고치는 중의 누름(7절 9) ----------

/// 박자표를 고치는 중(친 글자 있음)에 템포 필드를 누르면 박자표는 취소되고(확정 아님) 템포 끌기가 된다.
#[gpui_kit::test]
fn pressing_tempo_while_editing_meter_cancels_meter_and_drags(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "other-field");
    let m = in_field(cx, handle, "transport-meter", 0.5);
    down(cx, handle, m);
    up(cx, handle, m);
    type_text(cx, handle, "7/8");
    assert_eq!(meter_text(cx, &view), "7/8");
    drag_tempo_and_release(cx, handle, 8.0);
    assert_eq!(meter(cx, &view), TimeSignature::new(4, 4).unwrap());
    assert_eq!(meter_text(cx, &view), "4/4");
    assert_eq!(tempo(cx, &view), bpm(1220));
}

// ---------- 2. 다른 입력란 위에서 떼기 ----------

#[gpui_kit::test]
fn release_over_meter_field_applies_tempo_only(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "release-meter");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    let m = in_field(cx, handle, "transport-meter", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    // 가로로 박자표 위까지 옮겨 거기서 뗀다(세로는 그대로 8 px 위)
    let over = point(m.x, from.y - px(8.));
    move_to(cx, handle, over, Modifiers::default());
    up(cx, handle, over);
    assert_eq!(tempo(cx, &view), bpm(1220));
    assert_eq!(meter(cx, &view), TimeSignature::new(4, 4).unwrap());
    assert_eq!(meter_text(cx, &view), "4/4");
    // 뗀 뒤 포커스는 문서: 바로 친 글자가 박자표 입력란에 들어가지 않는다
    type_text(cx, handle, "9");
    assert_eq!(meter_text(cx, &view), "4/4");
}

// ---------- 3. 팝업·팔레트가 열린 채 누름 ----------

/// 조성 메뉴가 열린 채 템포 필드를 끌면 메뉴가 닫히고 템포가 바뀐다.
#[gpui_kit::test]
fn field_drag_with_key_menu_open_closes_menu(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "popup");
    cx.update_window(handle, |_, window, cx| window.click("transport-key", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(popup_open(cx, handle));
    drag_tempo_and_release(cx, handle, 8.0);
    assert!(!popup_open(cx, handle), "메뉴가 남아 있다");
    assert_eq!(tempo(cx, &view), bpm(1220));
}

/// 대조: 조성 메뉴는 밖(주 뷰)을 누르면 닫힌다.
#[gpui_kit::test]
fn control_key_menu_closes_on_outside_press(cx: &mut TestAppContext) {
    let (handle, _view) = setup(cx, "popup-ctl");
    cx.update_window(handle, |_, window, cx| window.click("transport-key", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(popup_open(cx, handle));
    let main = bounds(cx, handle, "main-view");
    let at = main.origin + point(px(200.), px(40.));
    down(cx, handle, at);
    up(cx, handle, at);
    assert!(!popup_open(cx, handle));
}

/// 팔레트가 열린 채 템포 필드를 끌면 팔레트가 닫히고 템포가 바뀐다.
#[gpui_kit::test]
fn field_drag_with_palette_open_closes_palette(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette");
    run(cx, handle, palette::Open);
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    drag_tempo_and_release(cx, handle, 8.0);
    assert!(
        !cx.read(|cx| view.read(cx).palette_open()),
        "팔레트가 남아 있다"
    );
    assert_eq!(tempo(cx, &view), bpm(1220));
}

/// 대조: 팔레트는 밖을 누르면 닫힌다(주 뷰를 누름).
#[gpui_kit::test]
fn control_palette_closes_on_outside_press(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette-ctl");
    run(cx, handle, palette::Open);
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    let main = bounds(cx, handle, "main-view");
    let at = main.origin + point(px(200.), px(40.));
    down(cx, handle, at);
    up(cx, handle, at);
    assert!(!cx.read(|cx| view.read(cx).palette_open()));
}

/// 팔레트가 열린 채 템포 필드를 클릭(끌지 않음)하면 팔레트가 닫혀야 한다(main에서는 입력란 누름이 닫았다).
#[gpui_kit::test]
fn field_click_with_palette_open_closes_palette(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette-click");
    run(cx, handle, palette::Open);
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    up(cx, handle, at);
    assert!(
        !cx.read(|cx| view.read(cx).palette_open()),
        "팔레트가 남아 있다"
    );
    assert!(tempo_focused(cx, handle, &view));
}

// ---------- 4. 범위 끝·범위 밖 기준 ----------

#[gpui_kit::test]
fn drag_up_at_tempo_maximum_makes_no_edit(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "max", with_tempo(bpm(9999)));
    let before = undo_len(cx, &view);
    drag_tempo_and_release(cx, handle, 40.0);
    assert_eq!(tempo(cx, &view), bpm(9999));
    assert_eq!(undo_len(cx, &view), before);
    assert_eq!(tempo_text(cx, &view), "999.9");
}

/// SMF에서 온 5 BPM: 위로 한 단계에 10.0, 아래로는 그대로(편집 없음).
#[gpui_kit::test]
fn out_of_range_slow_tempo_enters_range_on_first_step(cx: &mut TestAppContext) {
    let slow = Tempo::from_micros_per_quarter(12_000_000).unwrap();
    let (handle, view) = open(cx, "slow", with_tempo(slow));
    let before = undo_len(cx, &view);
    drag_tempo_and_release(cx, handle, -40.0);
    assert_eq!(tempo(cx, &view), slow);
    assert_eq!(undo_len(cx, &view), before);
    drag_tempo_and_release(cx, handle, 4.0);
    assert_eq!(tempo(cx, &view), bpm(100));
}

/// 10분의 1 BPM으로 안 떨어지는 템포(µs 500 001): 끌었다 제자리면 µs를 지키고, 한 단계면 121.0.
#[gpui_kit::test]
fn smf_tempo_off_tenths_round_trips_and_steps(cx: &mut TestAppContext) {
    let odd = Tempo::from_micros_per_quarter(500_001).unwrap();
    let (handle, view) = open(cx, "odd", with_tempo(odd));
    let before = undo_len(cx, &view);
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 12.0), Modifiers::default());
    move_to(cx, handle, from, Modifiers::default());
    up(cx, handle, from);
    assert_eq!(tempo(cx, &view), odd);
    assert_eq!(undo_len(cx, &view), before);
    drag_tempo_and_release(cx, handle, 4.0);
    assert_eq!(tempo(cx, &view), bpm(1210));
}

#[gpui_kit::test]
fn meter_ends_stop_without_edit(cx: &mut TestAppContext) {
    let (handle, view) = open(
        cx,
        "meter-ends",
        with_meter(TimeSignature::new(99, 32).unwrap()),
    );
    let before = undo_len(cx, &view);
    for fx in [0.2, 0.8] {
        let from = in_field(cx, handle, "transport-meter", fx);
        down(cx, handle, from);
        let to = up_by(from, 40.0);
        move_to(cx, handle, to, Modifiers::default());
        assert_eq!(meter_text(cx, &view), "99/32");
        up(cx, handle, to);
    }
    assert_eq!(undo_len(cx, &view), before);
    assert_eq!(meter(cx, &view), TimeSignature::new(99, 32).unwrap());
}

/// 범위 밖 분모 128: 오른쪽 위로는 그대로, 아래로 한 단계에 32.
#[gpui_kit::test]
fn meter_out_of_range_denominator_enters_at_32(cx: &mut TestAppContext) {
    let (handle, view) = open(
        cx,
        "meter-128",
        with_meter(TimeSignature::new(3, 128).unwrap()),
    );
    let from = in_field(cx, handle, "transport-meter", 0.8);
    down(cx, handle, from);
    let to = up_by(from, -4.0);
    move_to(cx, handle, to, Modifiers::default());
    assert_eq!(meter_text(cx, &view), "3/32");
    up(cx, handle, to);
    assert_eq!(meter(cx, &view), TimeSignature::new(3, 32).unwrap());
}

// ---------- 5. `Cmd`와 뗌 ----------

/// `Cmd`를 쥔 채 끌다가 `Cmd`를 놓으며 다른 자리에서 뗀다. 놓는 순간 기준점이 옮겨지므로 값이 튀지 않아야 한다.
#[gpui_kit::test]
fn release_with_cmd_lifted_does_not_jump(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmd-lift");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, 8.0);
    move_to(cx, handle, to, cmd());
    assert_eq!(tempo_text(cx, &view), "120.2");
    up_with(cx, handle, to, MouseButton::Left, Modifiers::default(), 1);
    assert_eq!(tempo(cx, &view), bpm(1202));
}

/// 박자표에서 `Cmd`는 뜻이 없다(7절 4).
#[gpui_kit::test]
fn cmd_has_no_effect_on_meter(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "meter-cmd");
    let from = in_field(cx, handle, "transport-meter", 0.2);
    down(cx, handle, from);
    let mid = up_by(from, 8.0);
    move_to(cx, handle, mid, cmd());
    assert_eq!(meter_text(cx, &view), "6/4");
    let to = up_by(mid, 4.0);
    move_to(cx, handle, to, Modifiers::default());
    assert_eq!(meter_text(cx, &view), "7/4");
    up(cx, handle, to);
    assert_eq!(meter(cx, &view), TimeSignature::new(7, 4).unwrap());
}

// ---------- 6. 취소 뒤·클릭 전의 Esc ----------

/// Esc로 취소한 뒤 계속 움직여도 값이 바뀌지 않고, 떼도 편집이 없다.
#[gpui_kit::test]
fn moves_after_escape_do_nothing(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "esc-move");
    let before = undo_len(cx, &view);
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    press(cx, handle, "escape");
    let to = up_by(from, 40.0);
    move_to(cx, handle, to, Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "120.0");
    up(cx, handle, to);
    assert_eq!(undo_len(cx, &view), before);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

/// 문턱 전에 Esc: 끌기 후보가 취소되어 떼어도 필드에 들어가지 않는다.
#[gpui_kit::test]
fn escape_before_threshold_cancels_click(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "esc-click");
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    press(cx, handle, "escape");
    up(cx, handle, at);
    assert!(!tempo_focused(cx, handle, &view));
}

/// 문서가 실제로 바뀌지 않은 `Cmd+Z`(되돌릴 것 없음)는 끌기를 취소하지 않아야 한다(7절 7 「문서가 바뀌면」).
#[gpui_kit::test]
fn undo_with_empty_history_keeps_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "empty-undo");
    assert_eq!(undo_len(cx, &view), 0);
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, 8.0);
    move_to(cx, handle, to, Modifiers::default());
    press(cx, handle, "cmd-z");
    assert!(
        dragging(cx, &view),
        "바뀌지 않은 문서 알림이 끌기를 취소했다"
    );
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1220));
}

// ---------- 7. 클릭 종류 ----------

/// 클릭 뒤 포커스가 템포 필드에 있고, Esc는 고치기를 취소해 문서로 돌아간다(S2-T5b 규칙).
#[gpui_kit::test]
fn click_then_escape_cancels_field(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "click-esc");
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    up(cx, handle, at);
    assert!(tempo_focused(cx, handle, &view));
    type_text(cx, handle, "77");
    press(cx, handle, "escape");
    assert!(!tempo_focused(cx, handle, &view));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(tempo_text(cx, &view), "120.0");
}

/// 더블클릭은 끌기를 만들지 않고 편집도 없다.
#[gpui_kit::test]
fn double_click_makes_no_edit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dbl");
    let before = undo_len(cx, &view);
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down_with(cx, handle, at, MouseButton::Left, 1);
    up_with(cx, handle, at, MouseButton::Left, Modifiers::default(), 1);
    down_with(cx, handle, at, MouseButton::Left, 2);
    up_with(cx, handle, at, MouseButton::Left, Modifiers::default(), 2);
    assert!(!dragging(cx, &view));
    assert_eq!(undo_len(cx, &view), before);
    assert!(tempo_focused(cx, handle, &view));
}

/// 오른쪽 단추로 누르고 끌어도 값 조정이 없다.
#[gpui_kit::test]
fn right_button_does_not_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "right");
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down_with(cx, handle, at, MouseButton::Right, 1);
    VisualTestContext::from_window(handle, cx).simulate_event(MouseMoveEvent {
        position: up_by(at, 40.0),
        pressed_button: Some(MouseButton::Right),
        modifiers: Modifiers::default(),
    });
    cx.run_until_parked();
    render(cx, handle);
    up_with(
        cx,
        handle,
        up_by(at, 40.0),
        MouseButton::Right,
        Modifiers::default(),
        1,
    );
    assert!(!dragging(cx, &view));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

// ---------- 8. 끄는 중 모드 전환·뗀 뒤 포커스 ----------

/// 끄는 중 모드가 바뀌어도(Enter로 Step) 끌기는 이어진다(7절 8).
#[gpui_kit::test]
fn mode_change_during_drag_keeps_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "mode");
    // 첫 Enter는 피아노롤로 들어가고(모드는 Normal), 끄는 중의 두 번째 Enter가 Step으로 바꾼다
    press(cx, handle, "enter");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    let mode_before = cx.read(|cx| view.read(cx).mode_state().mode());
    press(cx, handle, "enter");
    assert_ne!(cx.read(|cx| view.read(cx).mode_state().mode()), mode_before);
    assert!(dragging(cx, &view));
    let to = up_by(from, 12.0);
    move_to(cx, handle, to, Modifiers::default());
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1230));
}

/// 끌기 뒤 템포 편집은 다른 편집과 묶이지 않는다: undo 한 번에 끌기만 되돌린다.
#[gpui_kit::test]
fn two_drags_are_two_undo_units(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "two-undo");
    drag_tempo_and_release(cx, handle, 8.0);
    drag_tempo_and_release(cx, handle, 8.0);
    assert_eq!(tempo(cx, &view), bpm(1240));
    press(cx, handle, "cmd-z");
    assert_eq!(tempo(cx, &view), bpm(1220));
    assert!(!tempo_focused(cx, handle, &view));
}
