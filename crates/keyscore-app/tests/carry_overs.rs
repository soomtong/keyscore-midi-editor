//! S2-T6f 이월 줄: undo 상한 알림(결정 2026-10-19 A), 바탕 레이어 디버그 키, 창 최소 크기.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, AsKeystroke as _, Entity, TestAppContext};
use keyscore_app::actions::{help, track};
use keyscore_app::keymap;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{
    PROJECT_WINDOW_MIN_SIZE, SETTINGS_WINDOW_MIN_SIZE, init, open_project_window,
    open_project_window_with_history_limit,
};
use keyscore_core::ksq;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-carry-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn one_track() -> keyscore_core::Project {
    ksq::from_str(r#"{"tracks": [{"name": "A", "channel": 1}]}"#)
        .unwrap()
        .0
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl gpui_kit::Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn messages(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<String> {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .iter()
            .map(ToString::to_string)
            .collect()
    })
}

const NOTICE: &str = "되돌리기 기록이 메모리 상한";

// ---------- 1. undo 상한 알림 ----------

#[gpui_kit::test]
fn undo_limit_notice_shows_once(cx: &mut TestAppContext) {
    start(cx, "undo-limit");
    // 상한 안: 알림 없음
    let (h, view) = cx
        .update(|cx| open_project_window_with_history_limit(one_track(), 1 << 20, cx))
        .unwrap();
    for i in 0..10 {
        run(cx, h, track::Rename(format!("이름 {i:0>100}")));
    }
    assert!(!messages(cx, &view).iter().any(|m| m.contains(NOTICE)));

    // 작은 상한: 처음 버릴 때 한 번
    let (h, view) = cx
        .update(|cx| open_project_window_with_history_limit(one_track(), 1500, cx))
        .unwrap();
    for i in 0..30 {
        run(cx, h, track::Rename(format!("이름 {i:0>100}")));
    }
    let notices = messages(cx, &view)
        .iter()
        .filter(|m| m.contains(NOTICE))
        .count();
    assert_eq!(notices, 1, "{:?}", messages(cx, &view));
    let dropped = cx.read(|cx| view.read(cx).document().read(cx).dropped_undo_units());
    assert!(dropped > 1, "여러 단위를 버렸어야 한다: {dropped}");
    // 되돌리기·다시 실행으로 더 버려도 다시 알리지 않는다
    for _ in 0..5 {
        run(cx, h, keyscore_app::actions::edit::Undo);
    }
    for _ in 0..5 {
        run(cx, h, keyscore_app::actions::edit::Redo);
    }
    let notices = messages(cx, &view)
        .iter()
        .filter(|m| m.contains(NOTICE))
        .count();
    assert_eq!(notices, 1);
}

// ---------- 2. 바탕 레이어 디버그 키 ----------

#[gpui_kit::test]
fn base_debug_key_is_hidden_from_editor_overlay_and_menus(cx: &mut TestAppContext) {
    start(cx, "debug-key");
    let (h, view) = cx
        .update(|cx| open_project_window(one_track(), cx))
        .unwrap();
    let debug = "alt-cmd-i";
    // 바탕 레이어에는 있다(디버그 빌드). 없으면 이 시험은 의미가 없다
    let in_base = cx.read(|cx| {
        keymap::base_bindings(cx).iter().any(|b| {
            b.keystrokes()
                .iter()
                .map(|k| k.as_keystroke().unparse())
                .collect::<Vec<_>>()
                .join(" ")
                == debug
        })
    });
    if cfg!(debug_assertions) {
        assert!(in_base, "바탕 레이어에 {debug}가 없다");
    }
    // 팔레트·메뉴의 키 표시
    let shown: Vec<String> = cx.read(|cx| {
        keyscore_app::commands::palette_entries(cx)
            .iter()
            .filter_map(|e| e.action())
            .filter_map(|a| keyscore_app::commands::effective_keys(a, cx))
            .flatten()
            .map(|k| k.unparse())
            .collect()
    });
    assert!(!shown.iter().any(|k| k == debug), "팔레트·메뉴에 {debug}");
    // 키맵 편집기의 실효 바인딩과 충돌 검사
    let (eff, conflicts) = cx.read(|cx| {
        let (layers, _) = keymap::current_layers(cx);
        let eff = keymap::user::effective_with(&layers, keymap::options(cx).swap_alt_cmd);
        let conflicts = keymap::user::binding_conflicts(&eff, None, debug, "track::ToggleMute");
        (eff, conflicts)
    });
    assert!(!eff.iter().any(|e| e.keys == debug));
    assert!(conflicts.is_empty(), "{conflicts:?}");
    // 오버레이
    run(cx, h, help::KeymapOverlay);
    let lines = cx.read(|cx| view.read(cx).overlay_lines(cx)).unwrap();
    assert!(!lines.iter().any(|l| l.contains(debug)), "{lines:?}");
}

// ---------- 3. 창 최소 크기 ----------

#[gpui_kit::test]
fn project_window_has_min_size(cx: &mut TestAppContext) {
    start(cx, "min-size");
    let options = cx.read(keyscore_app::project_window_options);
    assert_eq!(options.window_min_size, Some(PROJECT_WINDOW_MIN_SIZE));
    let settings = keyscore_app::settings_window::window_options();
    assert_eq!(settings.window_min_size, Some(SETTINGS_WINDOW_MIN_SIZE));
    // 세 패널 기본 크기와 메인 영역이 들어가고, 모드 메뉴가 모드 표시를 덮지 않는 높이
    assert!(PROJECT_WINDOW_MIN_SIZE.width >= gpui_kit::px(720.0));
    assert!(PROJECT_WINDOW_MIN_SIZE.height >= gpui_kit::px(480.0));
}
