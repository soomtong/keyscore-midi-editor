//! S2-T5c 적대적 리뷰 3. 깨뜨리지 못한 시도를 남긴다. 깨진 시도는 `docs/reports/s2-t5c/review-3.md`
//! 부록에 있다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review3-t5c-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn right_click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.right_click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    cx.run_until_parked();
    render(cx, handle);
}

fn shown(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.value().map(str::to_owned))
    })
    .unwrap()
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn project_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Project {
    cx.read(|cx| view.read(cx).document().read(cx).project().clone())
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

fn two_tracks() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1, "program": 0},
            {"name": "Bass", "channel": 2, "program": 33, "bank": 5}]}"#,
    )
    .unwrap()
    .0
}

fn dirty_tempo(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, text);
}

fn dirty_name(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    click(cx, handle, "inspector-name");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, text);
}

fn mute_after(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> bool {
    press(cx, handle, "m");
    project_of(cx, view).tracks()[0].mute()
}

// ---------- 오른쪽 클릭: 포커스를 받는 요소 ----------

/// 인스펙터 이름 입력란 오른쪽 클릭: 입력란이 포커스를 받고(바깥 처리기가 빼앗지 않는다) 글자가 들어간다.
#[gpui_kit::test]
fn right_click_inspector_input_takes_focus(cx: &mut TestAppContext) {
    start(cx, "r-input");
    let (handle, view) = open(cx, two_tracks());
    dirty_tempo(cx, handle, "95");
    right_click(cx, handle, "inspector-name");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, "Qm");
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Qm"));
    assert!(!project_of(cx, &view).tracks()[0].mute());
}

// ---------- 상태줄 ----------

/// 고치던 템포에서 상태줄 모드 표시를 누르면 메뉴가 열리고(바깥 처리기의 포커스 이동이 메뉴를 닫지 않는다)
/// 고르면 모드가 바뀐다. 템포는 확정되지 않는다.
#[gpui_kit::test]
fn status_mode_click_with_dirty_tempo_opens_menu(cx: &mut TestAppContext) {
    start(cx, "mode-tempo");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_tempo(cx, handle, "95");
    click(cx, handle, "status-mode");
    assert!(cx.read(|cx| view.read(cx).mode_menu_open()));
    cx.update_window(handle, |_, window, cx| {
        window
            .within("popup-menu")
            .click(gpui_kit::ElementId::Integer(1), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(label(cx, handle, "status-mode").as_deref(), Some("STEP"));
    assert_eq!(undo_len(cx, &view), len);
    assert_ne!(shown(cx, handle, "transport-tempo").as_deref(), Some("95"));
}

/// 상태줄 모드 표시를 오른쪽 클릭: 메뉴는 열리지 않고 문서로 돌아온다.
#[gpui_kit::test]
fn right_click_status_mode_returns_focus_without_menu(cx: &mut TestAppContext) {
    start(cx, "r-mode");
    let (handle, view) = open(cx, two_tracks());
    dirty_name(cx, handle, "Zz");
    right_click(cx, handle, "status-mode");
    assert!(!cx.read(|cx| view.read(cx).mode_menu_open()));
    assert!(mute_after(cx, handle, &view));
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "Lead");
}

/// 상태줄 메시지 자리를 가운데 버튼으로 눌러도 문서로 돌아온다.
#[gpui_kit::test]
fn middle_click_status_bar_returns_focus(cx: &mut TestAppContext) {
    start(cx, "m-status");
    let (handle, view) = open(cx, two_tracks());
    dirty_tempo(cx, handle, "95");
    let at = cx
        .update_window(handle, |_, window, _| {
            window.find("status-track").bounds().center()
        })
        .unwrap();
    let mut vcx = gpui_kit::VisualTestContext::from_window(handle, cx);
    vcx.simulate_mouse_down(
        at,
        gpui_kit::MouseButton::Middle,
        gpui_kit::Modifiers::none(),
    );
    vcx.simulate_mouse_up(
        at,
        gpui_kit::MouseButton::Middle,
        gpui_kit::Modifiers::none(),
    );
    cx.run_until_parked();
    render(cx, handle);
    assert!(
        mute_after(cx, handle, &view),
        "tempo={:?}",
        shown(cx, handle, "transport-tempo")
    );
}

/// 상태줄을 눌러 돌아온 뒤 Vim 카운트가 남지 않는다: 상태줄 클릭 전 템포 입력란 숫자가 카운트로 새지 않는다.
#[gpui_kit::test]
fn status_click_then_mute_once(cx: &mut TestAppContext) {
    start(cx, "status-once");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_tempo(cx, handle, "3");
    click(cx, handle, "status-track");
    assert!(mute_after(cx, handle, &view));
    assert_eq!(undo_len(cx, &view), len + 1);
}

/// 트랙 목록 줄(포커스 요소가 아닌 자리)을 오른쪽 클릭하면 문서로 돌아온다(트랙 목록 자체 처리기는 왼쪽만
/// 보지만 `content` 처리기가 받는다).
#[gpui_kit::test]
fn right_click_track_row_returns_focus(cx: &mut TestAppContext) {
    start(cx, "r-row");
    let (handle, view) = open(cx, two_tracks());
    dirty_name(cx, handle, "Zz");
    right_click(cx, handle, "track-row-0");
    assert!(mute_after(cx, handle, &view));
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "Lead");
}

// ---------- 오른쪽 클릭으로 포커스를 받은 버튼 ----------

/// 오른쪽 클릭으로 포커스를 받은 뮤트 토글에서 `space`를 쳐도 토글이 눌리지 않는다(부록 A의 모드 키 죽음이
/// 편집으로 번지지는 않는다).
#[gpui_kit::test]
fn right_click_mute_toggle_then_space(cx: &mut TestAppContext) {
    start(cx, "r-space");
    let (handle, view) = open(cx, two_tracks());
    right_click(cx, handle, "track-mute-0");
    press(cx, handle, "space");
    let muted = project_of(cx, &view).tracks()[0].mute();
    let undo = undo_len(cx, &view);
    assert!(!muted, "space가 재생 대신 뮤트를 토글했다: undo={undo}");
}

// ---------- 리뷰 3 부록 A(지적 1, 제안 2) ----------

fn ctx(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    cx.update_window(handle, |_, window, _| {
        format!("{:?}", window.context_stack())
    })
    .unwrap()
}

/// 트랙 목록 뮤트 토글을 오른쪽 클릭한 뒤에도 모드 키가 먹어야 한다. `main`에서는 통과한다.
#[gpui_kit::test]
fn right_click_track_mute_toggle_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "r-mute");
    let (handle, view) = open(cx, two_tracks());
    right_click(cx, handle, "track-mute-0");
    assert!(
        !project_of(cx, &view).tracks()[0].mute(),
        "오른쪽 클릭이 토글했다"
    );
    let c = ctx(cx, handle);
    assert!(
        mute_after(cx, handle, &view),
        "오른쪽 클릭 뒤 모드 키가 죽었다: context={c}"
    );
}

/// 트랙 목록 `+` 버튼을 오른쪽 클릭한 뒤에도 모드 키가 먹어야 한다. `main`에서는 통과한다.
#[gpui_kit::test]
fn right_click_track_add_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "r-add");
    let (handle, view) = open(cx, two_tracks());
    right_click(cx, handle, "track-add");
    let c = ctx(cx, handle);
    assert!(
        mute_after(cx, handle, &view),
        "오른쪽 클릭 뒤 모드 키가 죽었다: context={c}"
    );
}

/// 인스펙터 테스트 노트 버튼 오른쪽 클릭 뒤 모드 키.
#[gpui_kit::test]
fn right_click_inspector_button_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "r-test");
    let (handle, view) = open(cx, two_tracks());
    right_click(cx, handle, "inspector-test");
    let c = ctx(cx, handle);
    assert!(
        mute_after(cx, handle, &view),
        "오른쪽 클릭 뒤 모드 키가 죽었다: context={c}"
    );
}

/// 트랜스포트 바 버튼 오른쪽 클릭 뒤 모드 키(리뷰 3 제안 2).
#[gpui_kit::test]
fn right_click_transport_button_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "r-transport");
    let (handle, view) = open(cx, two_tracks());
    right_click(cx, handle, "transport-tap");
    let c = ctx(cx, handle);
    assert!(
        mute_after(cx, handle, &view),
        "오른쪽 클릭 뒤 모드 키가 죽었다: context={c}"
    );
}
