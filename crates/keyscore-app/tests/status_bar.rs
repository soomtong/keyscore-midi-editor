//! S2-T5c 상태줄(UI-5, 7.1 아래, KM-6).

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::Int;
use keyscore_app::actions::{track, transport};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-statusbar-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

#[gpui_kit::test]
fn status_bar_shows_track_and_messages(cx: &mut TestAppContext) {
    start(cx, "track");
    let p = ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1},
            {"name": "Drums", "port": {"named": "Some Synth"}, "channel": 10}]}"#,
    )
    .unwrap()
    .0;
    let (handle, view) = open(cx, p);
    let virtual_name = cx.read(keyscore_app::engine::virtual_port_name);
    assert_eq!(
        label(cx, handle, "status-track"),
        Some(format!("Lead · {virtual_name} ch1"))
    );
    run(cx, handle, track::Next);
    assert_eq!(
        label(cx, handle, "status-track").as_deref(),
        Some("Drums · Some Synth ch10")
    );
    run(cx, handle, track::SetChannel(Int(11)));
    run(cx, handle, track::Rename("Perc".into()));
    assert_eq!(
        label(cx, handle, "status-track").as_deref(),
        Some("Perc · Some Synth ch11")
    );
    // 오른쪽: 마지막 메시지(엔진이 없어 재생 메시지가 남는다)
    assert_eq!(label(cx, handle, "status-message").as_deref(), Some(""));
    run(cx, handle, transport::Play);
    let last = cx.read(|cx| view.read(cx).messages().last().unwrap().to_string());
    assert_eq!(label(cx, handle, "status-message"), Some(last));
    // 모드 표시는 그대로 있다
    assert!(label(cx, handle, "status-mode").is_some());
    // 트랙이 없으면 가운데는 비어 있다
    start(cx, "empty");
    let (handle, _) = open(cx, Project::new());
    assert_eq!(label(cx, handle, "status-track").as_deref(), Some(""));
}

#[gpui_kit::test]
fn status_bar_shows_keymap_errors(cx: &mut TestAppContext) {
    let dir = start(cx, "keymap");
    let (handle, _view) = open(cx, Project::new());
    let path = dir.join("keymap.json");
    std::fs::write(
        &path,
        "[{\"bindings\": {\n  \"f6\": \"no::SuchAction\"\n}}]",
    )
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
    render(cx, handle);
    let shown = label(cx, handle, "status-keymap-error").expect("키맵 오류가 상태줄에 없다");
    assert!(shown.contains("2:"), "{shown}");
    assert!(shown.contains("no::SuchAction"), "{shown}");
    std::fs::write(&path, r#"[{"bindings": {"f6": "view::ToggleInspector"}}]"#).unwrap();
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(label(cx, handle, "status-keymap-error"), None);
}
