//! S2-T6a 적대적 리뷰 2회차: 리뷰 1 대응(c7b767d)을 깨뜨리려는 시도. 깨지지 않은 시도만 남긴다.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, Keystroke, TestAppContext};
use keyscore_app::actions::keymap as km;
use keyscore_app::keymap::user::effective;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

const SETTLE: Duration = Duration::from_millis(1100);

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t6a-r2-{}-{name}", std::process::id()));
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
        effective(&layers)
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

// ---------- 못 깨뜨린 시도 ----------

/// 편집기가 쓴 `zed::Unbind` 항목을 엔진이 오류 없이 걸고, 같은 키의 다른 Action(설정 창의 `ctrl-tab`)은 산다.
/// 지운 Action은 메뉴·팔레트 키(`effective_keys`)에서도 사라진다.
#[gpui_kit::test]
fn unbind_entries_build_cleanly_and_hide_only_target(cx: &mut TestAppContext) {
    let dir = start(cx, "unbind-build", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "settings::Open");
    press(cx, handle, "backspace");
    let text = user_file(&dir);
    assert!(text.contains("zed::Unbind"), "{text}");
    assert!(!text.contains("null"), "{text}");
    assert!(
        cx.read(|cx| keymap::errors(cx)
            .iter()
            .all(|e| e.message.contains("경고"))),
        "{:?}",
        cx.read(|cx| keymap::errors(cx).to_vec())
    );
    assert_eq!(menu_keys(cx, "settings::Open"), None);
    // 같은 키(`cmd-,`)를 다른 Action이 쓰지 않으니 누르면 아무것도 나오지 않는다
    assert_eq!(dispatched(cx, handle, "cmd-,"), None);
}

/// 손으로 쓴 파일에 `null`과 `zed::Unbind`가 섞여 있어도 편집기가 읽고 고쳐 쓰며, `r`(복원)이 그 Action의
/// 지정 해제와 `null`을 함께 뺀다.
#[gpui_kit::test]
fn hand_written_null_and_unbind_mix_restores(cx: &mut TestAppContext) {
    let dir = start(
        cx,
        "mix",
        Some(
            r#"[{"context": "Normal", "bindings": {"m": ["zed::Unbind", "track::ToggleMute"], "s": null}}]"#,
        ),
    );
    let (handle, view) = open_settings(cx);
    assert!(effective_of(cx, "track::ToggleMute").is_empty());
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "r");
    assert!(
        !effective_of(cx, "track::ToggleMute").is_empty(),
        "{}",
        user_file(&dir)
    );
    let text = user_file(&dir);
    assert!(!text.contains("zed::Unbind"), "{text}");
    // 다른 자리의 `null`은 남는다
    assert!(text.contains("null"), "{text}");
}

/// 편집기 목록이 보이는 실효 바인딩(자기 컨텍스트에서)과 엔진이 실제로 고르는 Action이 같다. 편집 몇 번(전역
/// 지우기, 같은 자리 덮기, 다른 키로 옮기기, vim 프리셋) 뒤에도 같다.
#[gpui_kit::test]
fn editor_model_matches_engine_in_own_context(cx: &mut TestAppContext) {
    let dir = start(cx, "model", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "mode::Cycle");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "enter m");
    settle(cx, handle);
    // vim의 `m <표지>`와 접두 충돌: 새 바인딩 유지
    if cx.read(|cx| view.read(cx).sheet_open()) {
        press(cx, handle, "enter");
    }
    select_exact(cx, handle, &view, "cursor::ClipStart");
    press(cx, handle, "enter ctrl-alt-g");
    settle(cx, handle);
    let mismatches = cx.update(|cx| {
        let (layers, _) = keymap::current_layers(cx);
        let swap = keymap::options(cx).swap_alt_cmd;
        assert!(!swap);
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let mut bad = Vec::new();
        for e in effective(&layers) {
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
    });
    assert!(
        mismatches.is_empty(),
        "{mismatches:#?}\n{}",
        user_file(&dir)
    );
}

/// 다른 창에서 부른 내보내기는 설정 창을 열어 넘기고, 설정 창이 대화상자를 연다(무한 전달 없음).
#[gpui_kit::test]
fn export_from_project_window_opens_settings_and_prompts(cx: &mut TestAppContext) {
    let dir = start(cx, "fwd-export", None);
    let (project, _) = open_project(cx);
    cx.update_window(project, |_, window, cx| {
        window.dispatch_action(Box::new(km::Export), cx);
    })
    .unwrap();
    let out = dir.join("out.json");
    cx.simulate_new_path_selection(|_| Some(out.clone()));
    cx.run_until_parked();
    assert!(cx.update(|cx| settings_window::current(cx)).is_some());
    assert!(out.exists(), "내보내기 파일이 없다");
}

/// 수식키 교환은 설정 창이 닫힌 채 프로젝트 창에서 키 없이(Action으로) 켜고 끌 수 있다.
#[gpui_kit::test]
fn swap_modifiers_without_settings_window(cx: &mut TestAppContext) {
    start(cx, "swap-global", None);
    let (project, _) = open_project(cx);
    cx.update_window(project, |_, window, cx| {
        window.dispatch_action(Box::new(km::SwapModifiers), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(cx.read(keymap::options).swap_alt_cmd);
    assert!(cx.update(|cx| settings_window::current(cx)).is_none());
}

/// 프로젝트 창이 둘일 때 `cmd-w`는 지금 창만 닫는다. 남은 창의 키는 그대로 동작한다.
#[gpui_kit::test]
fn cmd_w_closes_only_active_project_window(cx: &mut TestAppContext) {
    start(cx, "cmdw-proj", None);
    let (a, _) = open_project(cx);
    let (b, bview) = open_project(cx);
    assert_eq!(cx.update(|cx| cx.active_window()), Some(b));
    cx.simulate_keystrokes(b, "cmd-w");
    cx.run_until_parked();
    let windows = cx.update(|cx| cx.windows());
    assert_eq!(windows.len(), 1);
    assert!(windows.contains(&a));
    drop(bview);
    activate(cx, a);
    assert_eq!(dispatched(cx, a, "m").as_deref(), Some("track::ToggleMute"));
}

/// 시트가 열린 채 구역을 옮기면 시트가 닫히고 파일은 그대로다.
#[gpui_kit::test]
fn section_switch_with_sheet_writes_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "sheet-switch", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter o");
    settle(cx, handle);
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    press(cx, handle, "ctrl-tab");
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
    press(cx, handle, "ctrl-shift-tab enter");
    assert_eq!(user_file(&dir), "");
}
