//! S2-T6a 적대적 리뷰 3회차: 리뷰 2 대응(18fad47)을 깨뜨리려는 시도. 깨지지 않은 시도만 남긴다.

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
    let dir = std::env::temp_dir().join(format!("ks-review-t6a-r3-{}-{name}", std::process::id()));
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

fn import(cx: &mut TestAppContext, handle: AnyWindowHandle, file: &std::path::Path) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(km::Import), cx);
    })
    .unwrap();
    let file = file.to_path_buf();
    cx.simulate_path_prompt_response(move |_| Some(vec![file.clone()]));
    cx.run_until_parked();
}

// ---------- 못 깨뜨린 시도 ----------

/// 뮤트의 `m`을 지운 뒤 다시 `m`을 걸면(같은 자리에 지정 해제와 바인딩) 엔진에서 `m`이 뮤트다. 다시 지우면
/// 사라지고, `r`이면 프리셋 `m`이 돌아오며 파일에 뮤트 항목이 남지 않는다.
#[gpui_kit::test]
fn clear_then_rebind_same_key_same_action_works(cx: &mut TestAppContext) {
    let dir = start(cx, "rebind-same", None);
    let (project, _) = open_project(cx);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    press(cx, handle, "enter m");
    settle(cx, handle);
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
    assert_eq!(
        effective_of(cx, "track::ToggleMute"),
        vec![(
            Some("Normal".into()),
            "m".into(),
            "track::ToggleMute".into()
        )],
        "{}",
        user_file(&dir)
    );
    assert!(
        model_engine_mismatches(cx).is_empty(),
        "{}",
        user_file(&dir)
    );
    activate(cx, project);
    assert_eq!(
        dispatched(cx, project, "m").as_deref(),
        Some("track::ToggleMute")
    );
    assert_eq!(menu_keys(cx, "track::ToggleMute").as_deref(), Some("m"));
    activate(cx, handle);
    press(cx, handle, "backspace");
    assert!(
        effective_of(cx, "track::ToggleMute").is_empty(),
        "{}",
        user_file(&dir)
    );
    activate(cx, project);
    assert_ne!(
        dispatched(cx, project, "m").as_deref(),
        Some("track::ToggleMute")
    );
    activate(cx, handle);
    press(cx, handle, "r");
    assert_eq!(effective_of(cx, "track::ToggleMute").len(), 1);
    assert!(
        !user_file(&dir).contains("ToggleMute"),
        "{}",
        user_file(&dir)
    );
}

/// 지정 해제와 다른 Action이 같은 자리에 있는 파일(k번째 절)을 내보내고 다시 가져오면 충돌 시트 없이 같은
/// 결과가 되고, 엔진 오류(경고 밖)가 없다.
#[gpui_kit::test]
fn export_import_round_trip_with_stacked_places(cx: &mut TestAppContext) {
    let dir = start(cx, "stack-roundtrip", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "enter m");
    settle(cx, handle);
    select_exact(cx, handle, &view, "edit::Undo");
    press(cx, handle, "backspace");
    let before = user_file(&dir);
    let out = dir.join("out.json");
    cx.update(|cx| settings_window::export_to(cx, &out))
        .unwrap();
    // 비우고 가져온다
    std::fs::write(dir.join("keymap.json"), "[]").unwrap();
    cx.update(|cx| {
        let path = keymap::user_path(cx);
        let options = keymap::options(cx);
        keymap::install(cx, path, options, |_| {});
    });
    activate(cx, handle);
    import(cx, handle, &out);
    // 표준 `cmd-z`의 지정 해제는 표준 단축키를 바꾸는 것이라 적용 전에 시트로 알린다(KM-12). 적용한다
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    press(cx, handle, "enter");
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
    assert_eq!(user_file(&dir), before);
    assert!(effective_of(cx, "track::ToggleMute").is_empty());
    assert!(effective_of(cx, "edit::Undo").is_empty());
    assert_eq!(
        effective_of(cx, "track::ToggleSolo"),
        vec![(
            Some("Normal".into()),
            "m".into(),
            "track::ToggleSolo".into()
        )]
    );
    assert!(
        cx.read(|cx| keymap::errors(cx)
            .iter()
            .all(|e| e.message.contains("경고"))),
        "{:?}",
        cx.read(|cx| keymap::errors(cx).to_vec())
    );
    assert!(model_engine_mismatches(cx).is_empty(), "{before}");
}

/// 교환을 켠 채 여러 편집(표준 지우기, 표준 키 캡처 후 유지, 프리셋 cmd 키 옮기기, 지운 키 다시 쓰기) 뒤 목록의 모든
/// 실효 바인딩이 엔진과 같다.
#[gpui_kit::test]
fn swap_on_model_matches_engine_after_edits(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-model", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "settings::Open");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter cmd-z");
    settle(cx, handle);
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    press(cx, handle, "enter");
    select_exact(cx, handle, &view, "cursor::ClipStart");
    press(cx, handle, "enter cmd-g");
    settle(cx, handle);
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "backspace");
    press(cx, handle, "enter o");
    settle(cx, handle);
    assert!(
        model_engine_mismatches(cx).is_empty(),
        "{}",
        user_file(&dir)
    );
    assert_eq!(menu_keys(cx, "edit::Undo"), None);
    assert_eq!(menu_keys(cx, "settings::Open"), None);
    assert_eq!(menu_keys(cx, "track::ToggleMute").as_deref(), Some("cmd-z"));
    assert_eq!(menu_keys(cx, "cursor::ClipStart").as_deref(), Some("cmd-g"));
    // r은 표준 지정 해제도 되돌린다
    select_exact(cx, handle, &view, "settings::Open");
    press(cx, handle, "r");
    assert_eq!(menu_keys(cx, "settings::Open").as_deref(), Some("cmd-,"));
}

/// 교환을 켠 채 표준 `cmd-k`가 없는 키에 프리셋 `alt-k`(교환 뒤 `cmd-k`)를 지우고 교환을 껐다 켜도, 프리셋
/// 지정 해제는 프리셋 바인딩과 같이 교환되어 계속 꺼져 있다.
#[gpui_kit::test]
fn swap_toggle_keeps_preset_unbind(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-toggle-preset", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "mode::Play");
    press(cx, handle, "backspace");
    assert!(effective_of(cx, "mode::Play").is_empty());
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, false));
    assert!(
        effective_of(cx, "mode::Play").is_empty(),
        "{}",
        user_file(&dir)
    );
    assert_eq!(menu_keys(cx, "mode::Play"), None);
}

/// 편집기가 쓴(k번째 절이 있는) 파일은 다시 띄운 뒤 편집해도 `.bak`의 손으로 쓴 원본을 덮지 않는다. 사람이 그
/// 뒤에 고치면 그 내용이 `.bak`이 된다.
#[gpui_kit::test]
fn bak_policy_with_stacked_places_and_hand_edit(cx: &mut TestAppContext) {
    let original = "// 내 메모\n[{\"context\": \"Normal\", \"bindings\": {\"ctrl-alt-u\": \"track::ToggleMute\"}}]\n";
    let dir = start(cx, "bak-stack", Some(original));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "enter m");
    settle(cx, handle);
    // 같은 자리(Normal, m)에 지정 해제(뮤트)와 바인딩(솔로): 파일에 같은 컨텍스트 절이 둘(원본 절과 합쳐 셋)
    let text = user_file(&dir);
    assert!(text.matches("\"Normal\"").count() >= 2, "{text}");
    cx.update(|cx| {
        let path = keymap::user_path(cx);
        let options = keymap::options(cx);
        keymap::install(cx, path, options, |_| {});
    });
    activate(cx, handle);
    select_exact(cx, handle, &view, "mode::Play");
    press(cx, handle, "backspace");
    let bak = std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap_or_default();
    assert!(bak.contains("내 메모"), "{bak}");
    // 사람이 고친 내용은 다음 편집에서 .bak이 된다
    let hand = format!("// 두 번째\n{}", user_file(&dir));
    std::fs::write(dir.join("keymap.json"), &hand).unwrap();
    cx.update(|cx| {
        let path = keymap::user_path(cx);
        let options = keymap::options(cx);
        keymap::install(cx, path, options, |_| {});
    });
    activate(cx, handle);
    select_exact(cx, handle, &view, "track::New");
    press(cx, handle, "backspace");
    let bak = std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap_or_default();
    assert!(bak.contains("두 번째"), "{bak}");
    // 끝까지 뮤트의 m은 꺼진 채다
    assert!(
        !effective_of(cx, "track::ToggleMute")
            .iter()
            .any(|(_, k, _)| k == "m"),
        "{}",
        user_file(&dir)
    );
}

/// 교환을 켠 채 접두 충돌: 프리셋 `cmd-k`(교환 뒤 `alt-k`)가 아니라 실제 키 기준으로 묻는다. `alt-k` 시퀀스
/// 앞부분을 캡처하면 시트가 뜬다.
#[gpui_kit::test]
fn swap_on_prefix_conflict_uses_actual_keys(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-prefix", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter alt-k x");
    settle(cx, handle);
    assert!(
        cx.read(|cx| view.read(cx).sheet_open()),
        "실제 키 alt-k(mode::Play)와의 접두 충돌을 묻지 않았다\n{}",
        user_file(&dir)
    );
}
