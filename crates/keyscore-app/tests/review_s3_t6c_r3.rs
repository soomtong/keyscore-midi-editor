//! S3-T6c 적대적 리뷰 3회차의 깨뜨리기 시도. 결정 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절.
//! 2회차 이후 수정(뗌을 놓친 끌기를 단추 없는 움직임·필드 상자 밖 누름이 취소, 필드 상자 기록 위치, 끄는 필드가
//! 포커스를 받으면 끌기를 놓음)이 연 길을 본다. 도우미는 `review_s3_t6c_r2.rs`를 본떴다.
#![allow(dead_code)]

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext,
    point, px,
};
use keyscore_app::actions::args::{Opt, Real};
use keyscore_app::actions::transport;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Project, Tempo, Tick, TimeSignature, Track, TrackColor};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-rev3-t6c-{}-{name}", std::process::id()));
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

// ---------- 0. 공용 도우미 ----------

fn has_text_selection(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, cx| {
        gpui_kit::base::TextSelection::has_selection(window, cx)
    })
    .unwrap()
}

fn set_tempo(bpm: f64) -> transport::SetTempo {
    transport::SetTempo(Opt(Some(Real(bpm))))
}

// ---------- 1. 필드 상자 경계(top_0·left_0 수정) ----------

/// 필드 상자의 네 모서리 바로 안쪽을 눌러도 끌기가 된다. 필드 누름(캡처)이 만든 끌기를 창 전체 누름 처리기가
/// 「상자 밖」으로 보고 바로 취소하면 안 된다(상자와 필드 누름 영역이 어긋나면 그렇게 된다).
#[gpui_kit::test]
fn press_at_field_edges_starts_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "edges");
    let b = bounds(cx, handle, "transport-tempo");
    let inset = px(0.5);
    let corners = [
        b.origin + point(inset, inset),
        b.origin + point(b.size.width - inset, inset),
        b.origin + point(inset, b.size.height - inset),
        b.origin + point(b.size.width - inset, b.size.height - inset),
    ];
    for (i, at) in corners.into_iter().enumerate() {
        down(cx, handle, at);
        // 위로 끌면 위 모서리는 상자를 벗어나지만 끌기는 이어진다
        let to = up_by(at, 8.0);
        move_to(cx, handle, to, Modifiers::default());
        assert!(dragging(cx, &view), "모서리 {i}에서 끌기가 시작되지 않았다");
        up(cx, handle, to);
    }
    assert_eq!(tempo(cx, &view), bpm(1200 + 4 * 20));
    assert_eq!(undo_len(cx, &view), 4);
}

/// 박자표 상자의 왼쪽 끝은 분자, 오른쪽 끝은 분모다(상자 기록이 필드와 같은 자리여야 한다).
#[gpui_kit::test]
fn meter_edges_pick_numerator_and_denominator(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "meter-edges");
    let b = bounds(cx, handle, "transport-meter");
    let left = b.origin + point(px(1.), b.size.height - px(1.));
    down(cx, handle, left);
    move_to(cx, handle, up_by(left, 4.0), Modifiers::default());
    up(cx, handle, up_by(left, 4.0));
    assert_eq!(meter(cx, &view), TimeSignature::new(5, 4).unwrap());
    let right = b.origin + point(b.size.width - px(1.), px(1.));
    down(cx, handle, right);
    move_to(cx, handle, up_by(right, 4.0), Modifiers::default());
    up(cx, handle, up_by(right, 4.0));
    assert_eq!(meter(cx, &view), TimeSignature::new(5, 8).unwrap());
}

// ---------- 2. 창 전체 누름·단추 없는 움직임 취소가 다른 흐름을 망치지 않는가 ----------

/// 클릭으로 필드에 들어간 뒤 단추 없는 움직임(마우스를 옮김)은 고치는 중인 필드를 건드리지 않는다.
#[gpui_kit::test]
fn hover_after_click_keeps_field_editing(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "hover-after-click");
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    up(cx, handle, at);
    assert!(tempo_focused(cx, handle, &view));
    type_text(cx, handle, "90");
    let main = bounds(cx, handle, "main-view");
    cx.update_window(handle, |_, window, cx| {
        window.simulate_mouse_move(main.origin + point(px(100.), px(40.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(tempo_focused(cx, handle, &view));
    assert_eq!(tempo_text(cx, &view), "90");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), bpm(900));
}

/// 고치는 중인 템포 필드 안을 눌러 글자를 끌어 고르는 동안(입력란의 보통 동작) 창 전체 처리기가 아무것도
/// 바꾸지 않는다.
#[gpui_kit::test]
fn text_selection_drag_in_focused_field_is_untouched(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "select-in-field");
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    up(cx, handle, at);
    type_text(cx, handle, "95");
    // 가운데에서 눌러 오른쪽으로 끈다(가운데 밖 누름이 포커스를 잃는 것은 main에도 있는 별개 동작이다)
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(
        cx,
        handle,
        from + point(px(20.), px(-12.)),
        Modifiers::default(),
    );
    assert!(!dragging(cx, &view));
    up(cx, handle, from + point(px(20.), px(-12.)));
    assert!(tempo_focused(cx, handle, &view));
    assert_eq!(tempo_text(cx, &view), "95");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), bpm(950));
}

/// 남은(뗌을 놓친) 템포 끌기 뒤 피아노롤을 눌러 끌어도 템포는 그대로이고 끌기는 끝난다.
#[gpui_kit::test]
fn stale_drag_then_press_in_main_view_with_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "stale-main-drag");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    let main = bounds(cx, handle, "main-view");
    let at = main.origin + point(px(200.), px(60.));
    down(cx, handle, at);
    assert!(!dragging(cx, &view));
    move_to(cx, handle, up_by(at, 30.0), Modifiers::default());
    up(cx, handle, up_by(at, 30.0));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(tempo_text(cx, &view), "120.0");
}

/// 남은 템포 끌기 뒤 박자표 필드를 누르고 끌면, 템포는 그대로이고 박자표만 바뀐다.
#[gpui_kit::test]
fn stale_tempo_drag_then_meter_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "stale-meter");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    let m = in_field(cx, handle, "transport-meter", 0.2);
    down(cx, handle, m);
    assert_eq!(tempo_text(cx, &view), "120.0");
    move_to(cx, handle, up_by(m, 4.0), Modifiers::default());
    up(cx, handle, up_by(m, 4.0));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(meter(cx, &view), TimeSignature::new(5, 4).unwrap());
    assert_eq!(undo_len(cx, &view), 1);
}

/// 뗌을 놓친 끌기가 남은 채(포인터는 움직이지 않음) 키로 퀀타이즈 시트를 열고, 시트 바탕이 덮은 필드 상자
/// 자리를 그대로 눌렀다 떼면 남은 끌기가 그 뗌을 받아 템포를 바꾸면 안 된다(결정 7절 10). 창 전체 누름
/// 처리기는 「상자 안」이라 건너뛰고, 필드 누름은 가려져 불리지 않는다.
#[gpui_kit::test]
fn stale_drag_then_press_on_overlay_over_field(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "stale-overlay");
    let b = bounds(cx, handle, "transport-tempo");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    let p = up_by(from, 6.0);
    assert!(b.contains(&p), "시험 전제: 끈 자리가 필드 상자 안");
    down(cx, handle, from);
    move_to(cx, handle, p, Modifiers::default());
    assert!(dragging(cx, &view));
    // 뗌이 오지 않았다. 키로 시트를 연다
    run(cx, handle, keyscore_app::actions::edit::QuantizeSettings);
    assert!(cx.read(|cx| view.read(cx).quantize_sheet().is_some()));
    down(cx, handle, p);
    up(cx, handle, p);
    assert_eq!(
        tempo(cx, &view),
        Tempo::DEFAULT,
        "시트 바탕 클릭이 남은 끌기의 템포를 넣었다"
    );
}

// ---------- 3. 끄는 중 포커스(7절 11) ----------

/// 7절 11: 끄는 중 그 필드에 들어가 친 글자는 `Esc`로 취소된다. 입력란의 `Esc`처럼 포커스도 문서로 돌아가야
/// 한다(움직임 없이 바로 `Esc`를 친 경우: 끌기가 아직 남아 있다).
#[gpui_kit::test]
fn escape_after_entering_field_mid_drag_leaves_field(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "enter-esc");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    run(cx, handle, transport::SetTempo(Opt(None)));
    type_text(cx, handle, "90");
    press(cx, handle, "escape");
    assert_eq!(tempo_text(cx, &view), "120.0");
    assert!(
        !tempo_focused(cx, handle, &view),
        "Esc 뒤에도 템포 필드에 포커스가 남았다(Esc를 두 번 쳐야 나온다)"
    );
    up(cx, handle, up_by(from, 8.0));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

/// 7절 11의 변형: 박자표를 끄는 중 키로 템포 필드에 들어가 「90」을 치고 떼면, 박자표 끌기는 적용되더라도
/// 템포에 친 글자가 뗌의 문서 포커스 되돌림으로 말없이 사라지면 안 된다.
#[gpui_kit::test]
fn meter_drag_then_entering_tempo_field_keeps_typed_text(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "enter-other");
    let from = in_field(cx, handle, "transport-meter", 0.2);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 4.0), Modifiers::default());
    run(cx, handle, transport::SetTempo(Opt(None)));
    assert!(tempo_focused(cx, handle, &view));
    type_text(cx, handle, "90");
    let to = up_by(from, 8.0);
    move_to(cx, handle, to, Modifiers::default());
    up(cx, handle, to);
    let kept = (tempo_focused(cx, handle, &view) && tempo_text(cx, &view) == "90")
        || tempo(cx, &view) == bpm(900);
    assert!(
        kept,
        "템포에 친 「90」이 말없이 사라졌다: 글자 {:?}, 포커스 {}",
        tempo_text(cx, &view),
        tempo_focused(cx, handle, &view)
    );
}

/// 끄는 중 키로 연 팔레트는 뗌으로 닫히지 않는다(사람이 끄는 중에 일부러 연 것이다).
#[gpui_kit::test]
fn palette_opened_mid_drag_survives_release(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette-survive");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, 8.0);
    move_to(cx, handle, to, Modifiers::default());
    run(cx, handle, keyscore_app::actions::palette::Open);
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1220));
    assert!(
        cx.read(|cx| view.read(cx).palette_open()),
        "뗌이 끄는 중 연 팔레트를 닫았다"
    );
}

/// 끄는 중 키로 연 퀀타이즈 시트(포커스를 잃으면 닫힌다)도 뗌으로 닫히지 않는다. 뗌의 문서 포커스 되돌림이
/// 끄는 중 사람이 연 것의 포커스를 뺏으면 안 된다.
#[gpui_kit::test]
fn quantize_sheet_opened_mid_drag_survives_release(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "sheet-survive");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, 8.0);
    move_to(cx, handle, to, Modifiers::default());
    run(cx, handle, keyscore_app::actions::edit::QuantizeSettings);
    assert!(cx.read(|cx| view.read(cx).quantize_sheet().is_some()));
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1220));
    assert!(
        cx.read(|cx| view.read(cx).quantize_sheet().is_some()),
        "뗌이 끄는 중 연 퀀타이즈 시트를 닫았다"
    );
}

// ---------- 4. 다른 단추 누름 ----------

/// 왼쪽으로 끄는 중 필드 상자 밖에서 오른쪽 단추를 누르면 끌기는 취소된다(창 전체 누름 처리기는 단추를 가리지
/// 않는다). 값은 들어가지 않고, 뒤이은 왼쪽 뗌도 아무 일이 없다. 현재 동작의 기록이다.
#[gpui_kit::test]
fn right_press_during_left_drag_cancels_without_edit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "right-mid");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, 40.0) + point(px(200.), px(0.));
    move_to(cx, handle, to, Modifiers::default());
    assert!(dragging(cx, &view));
    down_with(cx, handle, to, MouseButton::Right, 1);
    assert!(!dragging(cx, &view));
    assert_eq!(tempo_text(cx, &view), "120.0");
    up_with(cx, handle, to, MouseButton::Right, Modifiers::default(), 1);
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(undo_len(cx, &view), 0);
}
