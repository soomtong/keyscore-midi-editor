//! S2-T6b `?` 키맵 오버레이(UI-6, KM-10).

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{help, mode};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::ksq;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-overlay-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let p = ksq::from_str(r#"{"tracks": [{"name": "A", "channel": 1}]}"#)
        .unwrap()
        .0;
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

fn lines(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<Vec<String>> {
    cx.read(|cx| view.read(cx).overlay_lines(cx))
}

fn page(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<(usize, usize)> {
    cx.read(|cx| view.read(cx).overlay_page())
}

fn muted(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).project().tracks()[0].mute())
}

fn has_line(lines: &[String], keys: &str, action: &str) -> bool {
    lines.iter().any(|l| {
        l.split(" · ")
            .next()
            .is_some_and(|k| k.split(", ").any(|k| k == keys))
            && l.contains(action)
    })
}

#[gpui_kit::test]
fn question_mark_opens_overlay_with_mode_keys(cx: &mut TestAppContext) {
    start(cx, "open", None);
    let (handle, view) = open(cx);
    assert!(lines(cx, &view).is_none());
    press(cx, handle, "?");
    let l = lines(cx, &view).expect("오버레이가 열리지 않았다");
    assert!(has_line(&l, "m", "track::ToggleMute"), "{l:?}");
    // Step 전용 키는 Normal 오버레이에 없다
    assert!(!l.iter().any(|x| x.contains("step::Rest")), "{l:?}");
    // 오버레이 요소가 그려진다
    let header = cx
        .update_window(handle, |_, window, _| {
            window
                .try_find("keymap-overlay")
                .and_then(|e| e.label().map(str::to_owned))
        })
        .unwrap();
    assert!(header.is_some_and(|h| h.contains("Normal")));
    // 다시 KeymapOverlay(메뉴)면 닫힌다
    run(cx, handle, help::KeymapOverlay);
    assert!(lines(cx, &view).is_none());
}

#[gpui_kit::test]
fn overlay_pages_with_tab_and_closes_with_escape(cx: &mut TestAppContext) {
    start(cx, "pages", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    let (first, total) = page(cx, &view).unwrap();
    assert_eq!(first, 0);
    assert!(total > 1, "Normal 키가 한 쪽에 다 들어간다: {total}");
    press(cx, handle, "tab");
    assert_eq!(page(cx, &view), Some((1, total)));
    press(cx, handle, "shift-tab shift-tab");
    assert_eq!(page(cx, &view), Some((0, total)));
    for _ in 0..total + 2 {
        press(cx, handle, "tab");
    }
    assert_eq!(page(cx, &view), Some((total - 1, total)));
    // 열린 동안 문서 키는 먹지 않는다
    press(cx, handle, "m");
    assert!(!muted(cx, &view));
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    press(cx, handle, "m");
    assert!(muted(cx, &view));
}

#[gpui_kit::test]
fn overlay_reflects_user_keymap(cx: &mut TestAppContext) {
    start(
        cx,
        "user",
        Some(
            r#"[{"context": "Normal", "bindings": {
                "m": ["zed::Unbind", "track::ToggleMute"],
                "ctrl-alt-u": "track::ToggleMute"
            }}]"#,
        ),
    );
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    let l = lines(cx, &view).unwrap();
    assert!(has_line(&l, "ctrl-alt-u", "track::ToggleMute"), "{l:?}");
    assert!(!has_line(&l, "m", "track::ToggleMute"), "{l:?}");
}

#[gpui_kit::test]
fn overlay_shows_current_mode(cx: &mut TestAppContext) {
    start(cx, "step", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Step);
    run(cx, handle, help::KeymapOverlay);
    let l = lines(cx, &view).unwrap();
    assert!(has_line(&l, "space", "step::Rest"), "{l:?}");
    assert!(!l.iter().any(|x| x.contains("track::ToggleMute")), "{l:?}");
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        keyscore_app::mode::Mode::Step,
        "오버레이를 닫는 Esc가 Step을 나가게 하면 안 된다"
    );
}
