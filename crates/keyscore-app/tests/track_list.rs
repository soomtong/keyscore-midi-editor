//! S2-T5b 트랙 목록과 트랙 Action(SEQ-7, SEQ-8, 5.11 `Tab`).

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, track};
use keyscore_app::keymap;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, TrackId, ksq};

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-tracklist-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: impl Into<gpui_kit::ElementId>) {
    let id = id.into();
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: String) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn exists(cx: &mut TestAppContext, handle: AnyWindowHandle, id: String) -> bool {
    cx.update_window(handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn project_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Project {
    cx.read(|cx| view.read(cx).document().read(cx).project().clone())
}

fn channels(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<u8> {
    project_of(cx, view)
        .tracks()
        .iter()
        .map(|t| t.channel().get())
        .collect()
}

fn current(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<TrackId> {
    cx.read(|cx| view.read(cx).current_track())
}

fn ids(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<TrackId> {
    project_of(cx, view)
        .tracks()
        .iter()
        .map(keyscore_core::Track::id)
        .collect()
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

// ---------- 1. 새 트랙의 채널(SEQ-7, SEQ-8) ----------

#[gpui_kit::test]
fn new_track_takes_lowest_free_channel(cx: &mut TestAppContext) {
    start(cx, "new-channel", None);
    let (handle, view) = open(cx, Project::new());
    assert_eq!(current(cx, &view), None);
    for _ in 0..3 {
        run(cx, handle, track::New);
    }
    assert_eq!(channels(cx, &view), [1, 2, 3]);
    // 새 트랙이 현재 트랙이 된다
    assert_eq!(current(cx, &view), ids(cx, &view).last().copied());
    // 둘째 트랙을 지우면 채널 2가 비고, 다음 새 트랙이 그 자리를 받는다
    click(cx, handle, "track-row-1");
    run(cx, handle, track::Delete);
    assert_eq!(channels(cx, &view), [1, 3]);
    run(cx, handle, track::New);
    assert_eq!(channels(cx, &view), [1, 3, 2]);
    for _ in 0..13 {
        run(cx, handle, track::New);
    }
    assert_eq!(channels(cx, &view).len(), 16);
    // 채널 10은 드럼으로 표시한다
    let drum_row = channels(cx, &view).iter().position(|c| *c == 10).unwrap();
    let row = label(cx, handle, format!("track-row-{drum_row}")).unwrap();
    assert!(row.contains("드럼"), "{row}");
    // 16채널이 다 차면 다른 포트를 권한다
    let before = cx.read(|cx| view.read(cx).messages().len());
    run(cx, handle, track::New);
    assert_eq!(channels(cx, &view).len(), 17);
    let messages = cx.read(|cx| view.read(cx).messages().to_vec());
    assert!(messages.len() > before, "{messages:?}");
    assert!(messages.last().unwrap().contains("포트"), "{messages:?}");
}

// ---------- 2. 줄과 현재 트랙 ----------

#[gpui_kit::test]
fn track_list_rows_and_current_track(cx: &mut TestAppContext) {
    start(cx, "rows", None);
    let p = ksq::from_str(
        r#"{"tracks": [
            {"name": "Lead", "port": "virtual", "channel": 1},
            {"name": "Bass", "port": {"named": "IAC Bus 1"}, "channel": 2},
            {"name": "Pad", "port": "virtual", "channel": 3}]}"#,
    )
    .unwrap()
    .0;
    let (handle, view) = open(cx, p);
    let tracks = ids(cx, &view);
    // 트랙이 있으면 첫 트랙이 현재 트랙이다
    assert_eq!(current(cx, &view), Some(tracks[0]));
    let row = label(cx, handle, "track-row-1".into()).unwrap();
    assert!(
        row.contains("Bass") && row.contains("IAC Bus 1") && row.contains("ch 2"),
        "{row}"
    );
    click(cx, handle, "track-row-2");
    assert_eq!(current(cx, &view), Some(tracks[2]));
    // Tab은 다음 트랙, Shift+Tab은 이전 트랙(5.11). 끝에서는 멈춘다
    press(cx, handle, "shift-tab");
    assert_eq!(current(cx, &view), Some(tracks[1]));
    press(cx, handle, "tab");
    press(cx, handle, "tab");
    assert_eq!(current(cx, &view), Some(tracks[2]));
    // 창 포커스는 그대로(트랙 목록에 탭 정지점이 없다)
    let focused = cx
        .update_window(handle, |_, window, cx| {
            gpui_kit::Focusable::focus_handle(view.read(cx), cx).is_focused(window)
        })
        .unwrap();
    assert!(focused);
    // 현재 트랙을 지우면 이웃 트랙이 현재 트랙이 된다
    run(cx, handle, track::Delete);
    assert_eq!(current(cx, &view), Some(tracks[1]));
}

// ---------- 3. undo 단위 ----------

#[gpui_kit::test]
fn track_actions_are_undo_units(cx: &mut TestAppContext) {
    start(cx, "undo", None);
    let (handle, view) = open(cx, Project::new());
    run(cx, handle, track::New);
    run(cx, handle, track::New);
    assert_eq!(undo_len(cx, &view), 2);
    let first = ids(cx, &view)[0];
    let second = ids(cx, &view)[1];
    // 현재 트랙은 둘째. 위로 옮기면 순서가 바뀐다
    run(cx, handle, track::MoveUp);
    assert_eq!(ids(cx, &view), [second, first]);
    assert_eq!(undo_len(cx, &view), 3);
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view), [first, second]);
    run(cx, handle, edit::Redo);
    assert_eq!(ids(cx, &view), [second, first]);
    run(cx, handle, track::ToggleMute);
    assert!(project_of(cx, &view).tracks()[0].mute());
    run(cx, handle, track::ToggleSolo);
    assert!(project_of(cx, &view).tracks()[0].solo());
    run(cx, handle, edit::Undo);
    run(cx, handle, edit::Undo);
    assert!(!project_of(cx, &view).tracks()[0].mute());
    assert!(!project_of(cx, &view).tracks()[0].solo());
    run(cx, handle, track::NewAbove);
    assert_eq!(ids(cx, &view).len(), 3);
    assert_eq!(current(cx, &view), Some(ids(cx, &view)[0]));
    run(cx, handle, track::NewBelow);
    assert_eq!(current(cx, &view), Some(ids(cx, &view)[1]));
    run(
        cx,
        handle,
        keyscore_app::actions::track::SetChannel(keyscore_app::actions::args::Int(9)),
    );
    assert_eq!(project_of(cx, &view).tracks()[1].channel().get(), 9);
}

// ---------- 4. M/S 버튼은 실행 함수를 거친다 ----------

#[gpui_kit::test]
fn mute_solo_buttons_run_through_run_action(cx: &mut TestAppContext) {
    start(
        cx,
        "ms-buttons",
        Some(r#"[{"bindings": {"3": "operator::Count(3)"}}]"#),
    );
    let p = ksq::from_str(r#"{"tracks": [{"channel": 1}, {"channel": 2}]}"#)
        .unwrap()
        .0;
    let (handle, view) = open(cx, p);
    press(cx, handle, "3");
    assert_eq!(cx.read(keymap::pending_count), Some(3));
    click(cx, handle, "track-mute-1");
    assert!(project_of(cx, &view).tracks()[1].mute());
    assert_eq!(
        cx.read(keymap::pending_count),
        None,
        "버튼이 카운트를 비우지 않았다"
    );
    // 버튼은 그 줄의 트랙을 대상으로 한다(현재 트랙이 아니어도)
    assert_eq!(current(cx, &view), Some(ids(cx, &view)[1]));
    click(cx, handle, "track-solo-0");
    assert!(project_of(cx, &view).tracks()[0].solo());
    click(cx, handle, "track-add");
    assert_eq!(ids(cx, &view).len(), 3);
}

// ---------- 10. SEQ-8 경고 ----------

#[gpui_kit::test]
fn seq8_warns_on_shared_channel_with_different_programs(cx: &mut TestAppContext) {
    start(cx, "seq8", None);
    let p = ksq::from_str(
        r#"{"tracks": [
            {"port": "virtual", "channel": 1, "program": 5},
            {"port": "virtual", "channel": 1, "program": 9},
            {"port": "virtual", "channel": 2, "program": 5},
            {"port": "virtual", "channel": 2, "program": 5}]}"#,
    )
    .unwrap()
    .0;
    let (handle, _) = open(cx, p);
    assert!(exists(cx, handle, "track-warning-0".into()));
    assert!(exists(cx, handle, "track-warning-1".into()));
    assert!(!exists(cx, handle, "track-warning-2".into()));
    assert!(!exists(cx, handle, "track-warning-3".into()));
    let why = label(cx, handle, "track-warning-0".into()).unwrap();
    assert!(why.contains("프로그램"), "{why}");
}
