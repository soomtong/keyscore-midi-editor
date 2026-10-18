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

// ---------- 리뷰 2 부록 A ----------

/// 수식키 교환을 켠 채 표준 단축키(`edit::Undo`의 `cmd-z`)를 지우면 엔진에서도 꺼져야 한다(목록은 꺼졌다고 보인다).
#[gpui_kit::test]
fn brk_swap_clear_standard_really_unbinds(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-clear-std", None);
    let (project, _) = open_project(cx);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "edit::Undo");
    press(cx, handle, "backspace");
    assert!(effective_of(cx, "edit::Undo").is_empty());
    activate(cx, project);
    let fired = dispatched(cx, project, "cmd-z");
    let menu = menu_keys(cx, "edit::Undo");
    assert!(
        fired.as_deref() != Some("edit::Undo") && menu.is_none(),
        "편집기는 Undo 키가 없다고 보이지만 cmd-z가 {fired:?}, 메뉴 키 {menu:?}\n{}",
        user_file(&dir)
    );
}

/// 수식키 교환을 켠 채 `cmd-z`를 캡처하면 표준 단축키(`edit::Undo`)와의 충돌 시트나 표준 경고가 나와야 한다
/// (KM-4, KM-12). 교환 없이는 시트가 뜬다.
#[gpui_kit::test]
fn brk_swap_capture_standard_key_warns(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-cap-std", None);
    let (project, _) = open_project(cx);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter cmd-z");
    settle(cx, handle);
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    activate(cx, project);
    let fired = dispatched(cx, project, "cmd-z");
    assert!(
        sheet || msg.contains("표준"),
        "시트·경고 없이 걸렸다: {msg:?}, cmd-z는 이제 {fired:?}\n{}",
        user_file(&dir)
    );
}

/// 수식키 교환을 켠 채 `alt-z`를 캡처하면 실제로 걸리는 키는 `alt-z`라 표준 `cmd-z`와 부딪히지 않는다. 충돌
/// 시트가 뜨면 안 된다.
#[gpui_kit::test]
fn brk_swap_capture_alt_key_no_false_conflict(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-cap-alt", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter alt-z");
    settle(cx, handle);
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    if sheet {
        // 「새 바인딩 유지」를 고르면 Undo가 목록에서 사라지지만 엔진은 여전히 cmd-z로 Undo한다
        press(cx, handle, "enter");
    }
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    assert!(
        !sheet,
        "alt-z(실제 키)가 표준 cmd-z와 충돌한다고 묻는다. 유지 뒤 알림: {msg:?}, Undo 목록 {:?}, 메뉴 {:?}\n{}",
        effective_of(cx, "edit::Undo"),
        menu_keys(cx, "edit::Undo"),
        user_file(&dir)
    );
}

/// 앱을 다시 띄운 뒤(엔진 재설치) 첫 편집이 `.bak`의 손으로 쓴 원본을 편집기 출력으로 덮으면 안 된다.
#[gpui_kit::test]
fn brk_bak_survives_restart(cx: &mut TestAppContext) {
    let original = "// 내 메모\n[{\"context\": \"Normal\", \"bindings\": {\"ctrl-alt-u\": \"track::ToggleMute\"}}]\n";
    let dir = start(cx, "bak-restart", Some(original));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "backspace");
    // 다시 띄우기: 엔진만 새로 설치한다(같은 사용자 파일)
    cx.update(|cx| {
        let path = keymap::user_path(cx);
        let options = keymap::options(cx);
        keymap::install(cx, path, options, |_| {});
    });
    activate(cx, handle);
    select_exact(cx, handle, &view, "track::New");
    press(cx, handle, "backspace");
    let bak = std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap_or_default();
    assert!(bak.contains("내 메모"), "재시작 뒤 원본이 사라졌다: {bak}");
}

/// `m`을 지운 Action(뮤트)은 그 자리를 다른 Action(솔로)이 가졌다가 놓아도 `m`을 되찾지 않아야 한다(사용자가 지운
/// 것은 지운 채로).
#[gpui_kit::test]
fn brk_cleared_key_not_revived_after_other_action_releases_it(cx: &mut TestAppContext) {
    let dir = start(cx, "revive", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "enter m");
    settle(cx, handle);
    press(cx, handle, "backspace");
    let mute = effective_of(cx, "track::ToggleMute");
    assert!(
        mute.is_empty(),
        "지운 뮤트의 m이 되살아났다: {mute:?}\n{}",
        user_file(&dir)
    );
}

/// 수식키 교환을 켠 채 `alt-s`(엔진은 `cmd-s`로 건다)를 가져오면 표준 단축키(저장) 덮기를 적용 전에 알려야 한다.
#[gpui_kit::test]
fn brk_swap_import_warns_standard_override(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-import", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    let file = dir.join("in.json");
    std::fs::write(&file, r#"[{"bindings": {"alt-s": "track::ToggleMute"}}]"#).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(km::Import), cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    assert!(
        sheet || msg.contains("표준"),
        "표준 저장 키를 덮는데 경고 없이 적용: {msg:?}, cmd-s 메뉴 {:?}",
        menu_keys(cx, "file::Save")
    );
}
