//! S2-T6a 적대적 리뷰 4회차(좁은 확인): 리뷰 3 지적 1 수정(d313366)을 깨뜨리려는 시도.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, Keystroke, TestAppContext};
use keyscore_app::actions::keymap as km;
use keyscore_app::keymap;
use keyscore_app::keymap::user::effective_with;
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

const SETTLE: Duration = Duration::from_millis(1100);

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t6a-r4-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn one_track() -> Project {
    ksq::from_str(r#"{"tracks": [{"name": "A", "channel": 1}]}"#)
        .unwrap()
        .0
}

fn open_project(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx
        .update(|cx| open_project_window(one_track(), cx))
        .unwrap();
    activate(cx, handle);
    (handle, view)
}

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
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

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    cx.run_until_parked();
    render(cx, handle);
}

fn settle(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.executor().advance_clock(SETTLE);
    cx.run_until_parked();
    render(cx, handle);
}

fn open_settings(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<SettingsWindow>) {
    cx.update(|cx| settings_window::open(cx, None));
    cx.run_until_parked();
    let (handle, view) = cx
        .update(|cx| settings_window::current(cx))
        .expect("설정 창이 없다");
    activate(cx, handle);
    (handle, view)
}

fn user_file(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("keymap.json")).unwrap_or_default()
}

fn selected(cx: &mut TestAppContext, view: &Entity<SettingsWindow>) -> Option<String> {
    cx.read(|cx| view.read(cx).selected_action(cx))
}

fn select_exact(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<SettingsWindow>,
    name: &str,
) {
    press(cx, handle, "/");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, &name.to_lowercase());
    press(cx, handle, "enter");
    for _ in 0..30 {
        if selected(cx, view).as_deref() == Some(name) {
            return;
        }
        press(cx, handle, "down");
    }
    panic!("{name}을 고르지 못했다: {:?}", selected(cx, view));
}

fn effective_of(cx: &mut TestAppContext, name: &str) -> Vec<(Option<String>, String, String)> {
    cx.read(|cx| {
        let (layers, _) = keymap::current_layers(cx);
        // 수식키 교환이 켜져 있으면 실제로 누르는 키로 본다(리뷰 2 대응 뒤 편집기 목록과 같은 기준)
        keyscore_app::keymap::user::effective_with(&layers, keymap::options(cx).swap_alt_cmd)
            .into_iter()
            .filter(|e| e.name == name)
            .map(|e| (e.context, e.keys, e.action))
            .collect()
    })
}

/// 그 창의 지금 포커스 경로에서 `keys`를 누르면 엔진이 고르는 첫 Action 이름(대기 중이면 `None`).
fn dispatched(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) -> Option<String> {
    let strokes: Vec<Keystroke> = keys
        .split_whitespace()
        .map(|k| Keystroke::parse(k).unwrap())
        .collect();
    cx.update_window(handle, |_, window, cx| {
        let stack = window.context_stack();
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let (found, pending) = keymap.bindings_for_input(&strokes, &stack);
        if pending {
            return None;
        }
        found.first().map(|b| b.action().name().to_owned())
    })
    .unwrap()
}

/// 엔진이 메뉴·팔레트에 보일 키(`commands::effective_keys`).
fn menu_keys(cx: &mut TestAppContext, name: &str) -> Option<String> {
    cx.update(|cx| {
        let action = cx.build_action(name, None).unwrap();
        keyscore_app::commands::effective_keys(action.as_ref(), cx).map(|ks| {
            ks.iter()
                .map(Keystroke::unparse)
                .collect::<Vec<_>>()
                .join(" ")
        })
    })
}

const STD: [(&str, &str); 3] = [
    ("edit::Undo", "cmd-z"),
    ("app::Quit", "cmd-q"),
    ("settings::Open", "cmd-,"),
];

fn toggle_via_action(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(km::SwapModifiers), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// 세 표준 단축키 모두 목록·메뉴가 비었고 엔진이 실제 키(cmd/alt 둘 다)에서 그 Action을 내지 않는다.
fn assert_all_cleared(
    cx: &mut TestAppContext,
    project: AnyWindowHandle,
    tag: &str,
    dir: &std::path::Path,
) {
    for (name, key) in STD {
        assert!(
            effective_of(cx, name).is_empty(),
            "{tag}: {name} 목록\n{}",
            user_file(dir)
        );
        assert_eq!(
            menu_keys(cx, name),
            None,
            "{tag}: {name} 메뉴\n{}",
            user_file(dir)
        );
        let alt = key.replace("cmd-", "alt-");
        activate(cx, project);
        for k in [key.to_owned(), alt] {
            assert_ne!(
                dispatched(cx, project, &k).as_deref(),
                Some(name),
                "{tag}: {name} {k}\n{}",
                user_file(dir)
            );
        }
    }
    assert!(
        cx.read(|cx| keymap::errors(cx)
            .iter()
            .all(|e| e.message.contains("경고"))),
        "{tag}: {:?}",
        cx.read(|cx| keymap::errors(cx).to_vec())
    );
}

fn clear_std(cx: &mut TestAppContext, handle: AnyWindowHandle, view: &Entity<SettingsWindow>) {
    for (name, _) in STD {
        select_exact(cx, handle, view, name);
        press(cx, handle, "backspace");
    }
}

fn restore_std(cx: &mut TestAppContext, handle: AnyWindowHandle, view: &Entity<SettingsWindow>) {
    for (name, key) in STD {
        select_exact(cx, handle, view, name);
        press(cx, handle, "r");
        assert_eq!(menu_keys(cx, name).as_deref(), Some(key), "복원 {name}");
    }
}

#[gpui_kit::test]
fn swap_toggles_keep_standard_cleared_from_off(cx: &mut TestAppContext) {
    let dir = start(cx, "std-from-off", None);
    let (project, _) = open_project(cx);
    let (handle, view) = open_settings(cx);
    clear_std(cx, handle, &view);
    assert_all_cleared(cx, project, "off", &dir);
    for (i, on) in [true, false, true, false, true].into_iter().enumerate() {
        cx.update(|cx| keymap::set_swap_alt_cmd(cx, on));
        assert_all_cleared(cx, project, &format!("set {i} {on}"), &dir);
    }
    activate(cx, handle);
    restore_std(cx, handle, &view);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, false));
    for (name, key) in STD {
        assert_eq!(menu_keys(cx, name).as_deref(), Some(key));
    }
    assert!(!user_file(&dir).contains("Unbind"), "{}", user_file(&dir));
}

#[gpui_kit::test]
fn swap_toggles_keep_standard_cleared_from_on_via_action(cx: &mut TestAppContext) {
    let dir = start(cx, "std-from-on", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (project, _) = open_project(cx);
    let (handle, view) = open_settings(cx);
    clear_std(cx, handle, &view);
    assert_all_cleared(cx, project, "on", &dir);
    for i in 0..4 {
        activate(cx, handle);
        toggle_via_action(cx, handle);
        assert_all_cleared(cx, project, &format!("action {i}"), &dir);
    }
    // 키 alt-m(설정 창 단축키)로도
    activate(cx, handle);
    let key = if cx.read(|cx| keymap::options(cx).swap_alt_cmd) {
        "cmd-m"
    } else {
        "alt-m"
    };
    let before = cx.read(|cx| keymap::options(cx).swap_alt_cmd);
    press(cx, handle, key);
    assert_ne!(
        cx.read(|cx| keymap::options(cx).swap_alt_cmd),
        before,
        "{key}가 교환을 바꾸지 않았다"
    );
    assert_all_cleared(cx, project, "key", &dir);
    activate(cx, handle);
    restore_std(cx, handle, &view);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, !keymap::options(cx).swap_alt_cmd));
    for (name, key) in STD {
        assert_eq!(menu_keys(cx, name).as_deref(), Some(key));
    }
}

/// 프리셋 지정 해제(`mode::Play`)와 사용자 바인딩은 토글 뒤에도 같은 실제 키를 유지한다.
#[gpui_kit::test]
fn swap_toggle_keeps_preset_unbind_and_user_bindings(cx: &mut TestAppContext) {
    let dir = start(cx, "preset-user", None);
    let (project, _) = open_project(cx);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "mode::Play");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "edit::Undo");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "backspace");
    press(cx, handle, "enter alt-o");
    settle(cx, handle);
    assert_eq!(menu_keys(cx, "track::ToggleSolo").as_deref(), Some("alt-o"));
    for on in [true, false, true, false] {
        cx.update(|cx| keymap::set_swap_alt_cmd(cx, on));
        assert!(
            effective_of(cx, "mode::Play").is_empty(),
            "{on}\n{}",
            user_file(&dir)
        );
        assert!(
            effective_of(cx, "edit::Undo").is_empty(),
            "{on}\n{}",
            user_file(&dir)
        );
        assert!(
            model_engine_mismatches(cx).is_empty(),
            "{on}\n{}",
            user_file(&dir)
        );
        assert_eq!(effective_of(cx, "track::ToggleSolo").len(), 1);
        activate(cx, project);
        assert_ne!(
            dispatched(cx, project, "cmd-z").as_deref(),
            Some("edit::Undo")
        );
        assert_ne!(
            dispatched(cx, project, "alt-z").as_deref(),
            Some("edit::Undo")
        );
    }
}

/// 깨진 파일은 토글이 건드리지 않는다(내용·.bak 동일). 오류는 문법 오류 하나뿐.
#[gpui_kit::test]
fn swap_toggle_does_not_touch_broken_file(cx: &mut TestAppContext) {
    let broken = "{ \"x\": [1, 2,, // 깨짐\n";
    let dir = start(cx, "broken", Some(broken));
    for on in [true, false, true] {
        cx.update(|cx| keymap::set_swap_alt_cmd(cx, on));
        assert_eq!(user_file(&dir), broken);
        assert!(!dir.join("keymap.json.bak").exists());
    }
}

/// 손으로 쓴(주석 있는) 유효한 파일에 표준 지정 해제가 있으면 토글이 다시 쓰되 원본을 .bak으로 남긴다.
/// 표준 지정 해제가 없으면 파일을 건드리지 않는다. 편집기가 쓴 파일이면 .bak을 만들지 않는다.
#[gpui_kit::test]
fn swap_toggle_bak_policy_is_content_based(cx: &mut TestAppContext) {
    let hand = "// 내 설정\n[\n  { \"context\": \"Normal\", \"bindings\": { \"alt-o\": \"track::ToggleSolo\" } }\n]\n";
    let dir = start(cx, "bak-nostd", Some(hand));
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    assert_eq!(user_file(&dir), hand, "표준 지정 해제 없는데 다시 썼다");
    assert!(!dir.join("keymap.json.bak").exists());

    let hand2 =
        "// 손으로\n[\n  { \"bindings\": { \"cmd-z\": [\"zed::Unbind\", \"edit::Undo\"] } }\n]\n";
    let dir = start(cx, "bak-std", Some(hand2));
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    assert_ne!(user_file(&dir), hand2);
    assert_eq!(
        std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap(),
        hand2
    );
    let after = user_file(&dir);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, false));
    assert_eq!(
        std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap(),
        hand2,
        "편집기 출력이 .bak을 덮었다"
    );
    assert_ne!(user_file(&dir), after);
    let (project, _) = open_project(cx);
    assert_ne!(
        dispatched(cx, project, "cmd-z").as_deref(),
        Some("edit::Undo")
    );
    assert_ne!(
        dispatched(cx, project, "alt-z").as_deref(),
        Some("edit::Undo")
    );
}
/// 지금 실효 바인딩(편집기 기준, 교환 반영) 하나하나를 그 바인딩 자신의 컨텍스트 경로에서 엔진에 물어 다른
/// Action이 나오는 것을 모은다.
fn model_engine_mismatches(cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| {
        let (layers, _) = keymap::current_layers(cx);
        let swap = keymap::options(cx).swap_alt_cmd;
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let mut bad = Vec::new();
        for e in effective_with(&layers, swap) {
            let stack: Vec<gpui_kit::KeyContext> = e
                .context
                .as_deref()
                .map(|c| {
                    c.split('>')
                        .map(|p| gpui_kit::KeyContext::parse(p.trim()).unwrap())
                        .collect()
                })
                .unwrap_or_default();
            let strokes: Vec<Keystroke> = e
                .keys
                .split_whitespace()
                .map(|k| Keystroke::parse(k).unwrap())
                .collect();
            let (found, _) = keymap.bindings_for_input(&strokes, &stack);
            let first = found.first().map(|b| b.action().name().to_owned());
            if first.as_deref() != Some(e.name.as_str()) {
                bad.push(format!(
                    "{:?} {} {} -> {first:?}",
                    e.context, e.keys, e.name
                ));
            }
        }
        bad
    })
}
