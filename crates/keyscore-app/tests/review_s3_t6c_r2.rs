//! S3-T6c 적대적 리뷰 2회차의 깨뜨리기 시도. 결정 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절.
//! 1회차 이후 수정(`prevent_default`로 바꾼 필드 누름, 끄는 값이 바뀔 때만 취소)이 연 길을 본다. 도우미는
//! `review_s3_t6c_r1.rs`를 본떴다.
#![allow(dead_code)]

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext,
    point, px,
};
use keyscore_app::actions::args::{Opt, Real};
use keyscore_app::actions::{edit, transport};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Project, Tempo, Tick, TimeSignature, Track, TrackColor};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-rev2-t6c-{}-{name}", std::process::id()));
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

fn has_text_selection(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, cx| {
        gpui_kit::base::TextSelection::has_selection(window, cx)
    })
    .unwrap()
}

fn set_tempo(bpm: f64) -> transport::SetTempo {
    transport::SetTempo(Opt(Some(Real(bpm))))
}

// ---------- 1. 끄는 값이 아닌 변경은 끌기를 끊지 않는다(33612e2) ----------

/// 템포를 끄는 중 박자표 편집이 되돌려져도 끌기는 이어지고, 템포 필드는 끌어서 고른 값을 계속 보인다.
/// 문서 변경 관찰자의 `sync_fields`가 끄는 필드 글자를 덮지 않아야 한다.
#[gpui_kit::test]
fn tempo_drag_survives_meter_undo_and_keeps_text(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "meter-undo");
    // 박자표를 5/4로(분자 쪽 위로 한 단계)
    let m = in_field(cx, handle, "transport-meter", 0.2);
    down(cx, handle, m);
    move_to(cx, handle, up_by(m, 4.0), Modifiers::default());
    up(cx, handle, up_by(m, 4.0));
    assert_eq!(meter(cx, &view), TimeSignature::new(5, 4).unwrap());
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "122.0");
    press(cx, handle, "cmd-z");
    assert_eq!(meter(cx, &view), TimeSignature::new(4, 4).unwrap());
    assert_eq!(meter_text(cx, &view), "4/4");
    assert!(dragging(cx, &view), "박자표 되돌리기가 템포 끌기를 끊었다");
    assert_eq!(
        tempo_text(cx, &view),
        "122.0",
        "문서 변경이 끄는 글자를 덮었다"
    );
    let to = up_by(from, 12.0);
    move_to(cx, handle, to, Modifiers::default());
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1230));
    assert_eq!(tempo_text(cx, &view), "123.0");
}

/// 박자표를 끄는 중 템포 편집이 되돌려져도 박자표 끌기는 이어진다.
#[gpui_kit::test]
fn meter_drag_survives_tempo_undo(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "tempo-undo");
    drag_tempo_and_release(cx, handle, 8.0);
    assert_eq!(tempo(cx, &view), bpm(1220));
    let m = in_field(cx, handle, "transport-meter", 0.2);
    down(cx, handle, m);
    move_to(cx, handle, up_by(m, 8.0), Modifiers::default());
    assert_eq!(meter_text(cx, &view), "6/4");
    press(cx, handle, "cmd-z");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(tempo_text(cx, &view), "120.0");
    assert!(dragging(cx, &view));
    assert_eq!(meter_text(cx, &view), "6/4");
    up(cx, handle, up_by(m, 8.0));
    assert_eq!(meter(cx, &view), TimeSignature::new(6, 4).unwrap());
}

// ---------- 2. 끄는 값이 바뀌는 다른 길은 취소 ----------

/// 끄는 중 redo로 템포가 바뀌면 취소, 뗌은 아무 일도 없다.
#[gpui_kit::test]
fn redo_of_tempo_during_drag_cancels(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "redo");
    drag_tempo_and_release(cx, handle, 8.0);
    press(cx, handle, "cmd-z");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    let undo_before = undo_len(cx, &view);
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, 40.0);
    move_to(cx, handle, to, Modifiers::default());
    run(cx, handle, edit::Redo);
    assert_eq!(tempo(cx, &view), bpm(1220));
    assert!(!dragging(cx, &view));
    assert_eq!(tempo_text(cx, &view), "122.0");
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(1220));
    assert_eq!(undo_len(cx, &view), undo_before + 1);
}

/// 끄는 중 `transport::SetTempo`(값 있음)가 템포를 바꾸면 취소.
#[gpui_kit::test]
fn set_tempo_action_during_drag_cancels(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "set-tempo");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    let to = up_by(from, 40.0);
    move_to(cx, handle, to, Modifiers::default());
    run(cx, handle, set_tempo(90.0));
    assert_eq!(tempo(cx, &view), bpm(900));
    assert!(!dragging(cx, &view));
    up(cx, handle, to);
    assert_eq!(tempo(cx, &view), bpm(900));
    assert_eq!(tempo_text(cx, &view), "90.0");
}

/// 10분의 1 BPM으로 같은 값이 되는 변경(µs 500 001 → 500 000)은 취소하지 않는다. 제자리로 끌어 떼면 그 변경이
/// 남고, 끌어 고른 값은 그 위에 들어간다(덮는 것이 아니라 뒤에 쌓인다).
#[gpui_kit::test]
fn sub_tenth_change_does_not_cancel_and_is_not_lost(cx: &mut TestAppContext) {
    let off = Tempo::from_micros_per_quarter(500_001).unwrap();
    let (handle, view) = open(cx, "sub-tenth", with_tempo(off));
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    run(cx, handle, set_tempo(120.0));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert!(dragging(cx, &view));
    move_to(cx, handle, from, Modifiers::default());
    up(cx, handle, from);
    // 제자리: 편집 없음, SetTempo의 변경이 남는다
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

// ---------- 3. 끄는 중 필드에 포커스가 들어옴 ----------

/// 끄는 중 `transport::SetTempo`(값 없음, 키에 묶을 수 있다)로 템포 필드에 들어가 「90」을 치면, 그 글자는
/// 이어지는 움직임에 덮이거나 뗌에서 말없이 버려지면 안 된다. 포커스가 있는 필드는 「고치는 중」이다(7절 2).
#[gpui_kit::test]
fn field_entered_during_drag_keeps_typed_text(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "enter-mid");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    run(cx, handle, transport::SetTempo(Opt(None)));
    assert!(tempo_focused(cx, handle, &view));
    type_text(cx, handle, "90");
    assert_eq!(tempo_text(cx, &view), "90");
    let to = up_by(from, 12.0);
    move_to(cx, handle, to, Modifiers::default());
    assert_eq!(tempo_text(cx, &view), "90", "움직임이 친 글자를 덮었다");
    up(cx, handle, to);
    let kept = tempo_text(cx, &view) == "90" || tempo(cx, &view) == bpm(900);
    assert!(
        kept,
        "친 「90」이 말없이 사라졌다: 템포 {:?}",
        tempo(cx, &view)
    );
}

// ---------- 4. 뗌을 잃은 끌기 ----------

/// 뗌 사건 없이(창 밖에서 놓쳐 등) 다음 누름이 다른 곳에서 오면, 남은 템포 끌기가 그 누름의 뗌을 자기 뗌으로
/// 받아 템포를 바꾸면 안 된다. 주 뷰 클릭은 템포 편집이 아니다.
#[gpui_kit::test]
fn stale_drag_does_not_apply_on_later_click_elsewhere(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "stale");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    // 뗌이 오지 않은 채 주 뷰를 누르고 뗀다
    let main = bounds(cx, handle, "main-view");
    let at = main.origin + point(px(200.), px(40.));
    down(cx, handle, at);
    up(cx, handle, at);
    assert_eq!(
        tempo(cx, &view),
        Tempo::DEFAULT,
        "주 뷰 클릭이 템포를 바꿨다"
    );
}

// ---------- 5. prevent_default가 연 길: 창 수준 글자 선택 ----------

/// 필드 누름은 기본 동작을 막아 입력란이 창 수준 글자 선택을 누르지 않는다(`suppress_text_selection`을 부르지
/// 않는다). 그 누름으로 창 수준 선택이 시작돼 끄는 동안 지나간 글자가 선택되면 안 된다.
#[gpui_kit::test]
fn field_drag_does_not_start_window_text_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "text-sel");
    assert!(!has_text_selection(cx, handle));
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    let main = bounds(cx, handle, "main-view");
    down(cx, handle, from);
    // 트랜스포트 바를 가로질러 주 뷰·상태줄 쪽까지
    let mid = point(from.x + px(300.), from.y);
    move_to(cx, handle, mid, Modifiers::default());
    let far = point(
        main.origin.x + px(20.),
        main.origin.y + main.size.height + px(10.),
    );
    move_to(cx, handle, far, Modifiers::default());
    assert!(dragging(cx, &view));
    assert!(
        !has_text_selection(cx, handle),
        "끄는 동안 창 글자 선택이 생겼다"
    );
    up(cx, handle, far);
    assert!(
        !has_text_selection(cx, handle),
        "끈 뒤 창 글자 선택이 남았다"
    );
}

// ---------- 6. 전파를 멈추지 않는 누름과 팝업·팔레트 ----------

/// 조성 메뉴가 열린 채 템포 필드를 클릭하면 메뉴가 닫히고, 메뉴 닫힘의 포커스 되돌림이 클릭 진입을 뺏지 않는다.
#[gpui_kit::test]
fn field_click_with_key_menu_open_enters_field(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "menu-click");
    cx.update_window(handle, |_, window, cx| window.click("transport-key", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(popup_open(cx, handle));
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    up(cx, handle, at);
    assert!(!popup_open(cx, handle));
    assert!(tempo_focused(cx, handle, &view));
    type_text(cx, handle, "90");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), bpm(900));
}

/// 팔레트가 열린 채 템포 필드를 클릭하고 바로 친 값이 확정된다(팔레트 닫힘이 포커스를 늦게 뺏지 않는다).
#[gpui_kit::test]
fn field_click_with_palette_open_then_type_commits(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette-type");
    run(cx, handle, keyscore_app::actions::palette::Open);
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    up(cx, handle, at);
    assert!(!cx.read(|cx| view.read(cx).palette_open()));
    type_text(cx, handle, "90");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), bpm(900));
}

/// 박자표를 클릭해 고치는 중, 그 입력란의 오른쪽 단추 메뉴(입력란 문맥 메뉴)를 열지 않고 템포를 끌면 박자표
/// 입력은 취소되고 템포만 바뀐다. 전파가 이어져도 바깥 `focus_document_on_press`가 고치던 입력을 확정하지 않는다.
#[gpui_kit::test]
fn press_reaches_outer_handler_without_committing_other_field(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "outer");
    let m = in_field(cx, handle, "transport-meter", 0.5);
    down(cx, handle, m);
    up(cx, handle, m);
    type_text(cx, handle, "3/4");
    let undo_before = undo_len(cx, &view);
    let at = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, at);
    up(cx, handle, at);
    assert_eq!(meter(cx, &view), TimeSignature::new(4, 4).unwrap());
    assert_eq!(meter_text(cx, &view), "4/4");
    assert_eq!(undo_len(cx, &view), undo_before);
    assert!(tempo_focused(cx, handle, &view));
}

/// 끄는 중 키로 팔레트를 열고 그 위에서 떼어도 끌기는 끝난다(남은 끌기가 다음 클릭에 템포를 바꾸지 않는다).
#[gpui_kit::test]
fn release_over_palette_opened_mid_drag_ends_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette-mid");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    run(cx, handle, keyscore_app::actions::palette::Open);
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    let p = bounds(cx, handle, "command-palette");
    let over = p.origin + point(px(40.), px(20.));
    move_to(cx, handle, over, Modifiers::default());
    up(cx, handle, over);
    assert!(
        !dragging(cx, &view),
        "팔레트 위에서 뗀 뒤에도 끌기가 남았다"
    );
}

/// 끄는 중 상태줄 모드 메뉴(`mode::ShowMenu`)를 열고 메뉴 위에서 떼어도 끌기는 끝난다.
#[gpui_kit::test]
fn release_over_mode_menu_opened_mid_drag_ends_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "menu-mid");
    let from = in_field(cx, handle, "transport-tempo", 0.5);
    down(cx, handle, from);
    move_to(cx, handle, up_by(from, 8.0), Modifiers::default());
    run(cx, handle, keyscore_app::actions::mode::ShowMenu);
    assert!(popup_open(cx, handle));
    let m = bounds(cx, handle, "popup-menu");
    let over = m.origin + point(px(20.), px(12.));
    move_to(cx, handle, over, Modifiers::default());
    up(cx, handle, over);
    assert!(!dragging(cx, &view), "메뉴 위에서 뗀 뒤에도 끌기가 남았다");
}
