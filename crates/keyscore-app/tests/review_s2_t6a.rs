//! S2-T6a 적대적 리뷰 1회차: 설정 창과 키맵 편집기를 깨뜨리려는 시도. 깨지지 않은 시도만 남긴다.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::SettingsSection;
use keyscore_app::actions::keymap as km;
use keyscore_app::keymap::user::effective;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

const SETTLE: Duration = Duration::from_millis(1100);

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t6a-{}-{name}", std::process::id()));
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

/// 검색으로 좁힌 뒤 정확히 그 Action에 선택을 둔다(키보드만).
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

fn muted(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).project().tracks()[0].mute())
}

/// 지금 실효 바인딩 중 그 Action 이름의 것(컨텍스트, 키, 라벨).
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

// ---------- 못 깨뜨린 시도 ----------

/// 캡처 중 표준 단축키(`cmd-w` 창 닫기, `cmd-q` 종료, `cmd-,` 설정)는 디스패치되지 않고 캡처된다. 확정하면
/// 표준 키 경고가 나온다(KM-12).
#[gpui_kit::test]
fn capture_swallows_standard_shortcuts_and_warns(cx: &mut TestAppContext) {
    let dir = start(cx, "cmdw", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter cmd-w");
    // 창이 닫히지 않았다
    assert!(cx.update(|cx| settings_window::current(cx)).is_some());
    assert!(cx.read(|cx| view.read(cx).capturing()));
    settle(cx, handle);
    assert!(user_file(&dir).contains("cmd-w"), "{}", user_file(&dir));
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    assert!(msg.contains("표준"), "{msg}");
}

/// 수식키 붙은 `escape`는 취소가 아니라 캡처된 키다(KM-8은 맨 `Esc`만 취소).
#[gpui_kit::test]
fn modified_escape_is_captured_not_cancel(cx: &mut TestAppContext) {
    let dir = start(cx, "shift-esc", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter shift-escape");
    assert!(cx.read(|cx| view.read(cx).capturing()));
    settle(cx, handle);
    assert!(
        user_file(&dir).contains("shift-escape"),
        "{}",
        user_file(&dir)
    );
}

/// 시퀀스의 두 번째 키가 1 s 안에 오면 하나의 시퀀스가 되고, 첫 키 뒤 대기는 버려진다.
#[gpui_kit::test]
fn capture_sequence_restarts_settle_timer(cx: &mut TestAppContext) {
    let dir = start(cx, "seq", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter ctrl-alt-u");
    cx.executor().advance_clock(Duration::from_millis(900));
    cx.run_until_parked();
    press(cx, handle, "ctrl-alt-y");
    cx.executor().advance_clock(Duration::from_millis(900));
    cx.run_until_parked();
    // 첫 키 뒤 1.8 s지만 마지막 키 뒤 0.9 s: 아직 캡처 중
    assert!(cx.read(|cx| view.read(cx).capturing()));
    assert_eq!(user_file(&dir), "");
    settle(cx, handle);
    assert!(
        user_file(&dir).contains("ctrl-alt-u ctrl-alt-y"),
        "{}",
        user_file(&dir)
    );
}

/// 캡처 중 구역을 바꾸거나 창을 닫으면 1 s 뒤에도 아무것도 쓰지 않는다. 창을 닫은 뒤 프로젝트 창 키는
/// 가로채지지 않는다.
#[gpui_kit::test]
fn capture_abandoned_by_section_switch_or_close_writes_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "abandon", None);
    let (project, pview) = open_project(cx);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter ctrl-alt-u");
    cx.update(|cx| settings_window::open(cx, Some(SettingsSection::Ai)));
    cx.run_until_parked();
    assert!(!cx.read(|cx| view.read(cx).capturing()));
    settle(cx, handle);
    assert_eq!(user_file(&dir), "");
    // 다시 키맵으로, 캡처 중 창을 닫는다
    cx.update(|cx| settings_window::open(cx, Some(SettingsSection::Keymap)));
    cx.run_until_parked();
    activate(cx, handle);
    press(cx, handle, "enter ctrl-alt-u");
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(SETTLE);
    cx.run_until_parked();
    assert_eq!(user_file(&dir), "");
    activate(cx, project);
    press(cx, project, "m");
    assert!(muted(cx, &pview), "설정 창을 닫은 뒤 m이 뮤트해야 한다");
}

/// 검색이 아무것도 고르지 않으면 `enter`는 캡처를 시작하지 않는다.
#[gpui_kit::test]
fn capture_with_empty_filter_does_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "empty-filter", None);
    let (handle, view) = open_settings(cx);
    press(cx, handle, "/");
    type_text(cx, handle, "zzzz-no-such-action");
    press(cx, handle, "enter");
    assert!(selected(cx, &view).is_none());
    press(cx, handle, "enter x");
    assert!(!cx.read(|cx| view.read(cx).capturing()));
    settle(cx, handle);
    assert_eq!(user_file(&dir), "");
}

/// vim 오버레이의 키를 지우고 `r`로 되돌린다. 프리셋을 바꿔도 사용자 항목은 남는다(KM-2).
#[gpui_kit::test]
fn vim_overlay_clear_restore_and_preset_switch(cx: &mut TestAppContext) {
    let dir = start(cx, "vim", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "cursor::ClipStart");
    let before = effective_of(cx, "cursor::ClipStart");
    assert!(before.iter().any(|(_, k, _)| k == "g g"), "{before:?}");
    press(cx, handle, "backspace");
    assert!(effective_of(cx, "cursor::ClipStart").is_empty());
    // 프리셋 왕복 뒤에도 지운 상태(사용자 항목)가 남는다
    cx.update(|cx| keymap::set_preset(cx, Preset::Default));
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    assert!(effective_of(cx, "cursor::ClipStart").is_empty());
    activate(cx, handle);
    press(cx, handle, "r");
    assert_eq!(effective_of(cx, "cursor::ClipStart"), before);
    assert!(cx.read(|cx| keymap::errors(cx).is_empty()));
    assert!(!user_file(&dir).contains("g g"), "{}", user_file(&dir));
}

/// 공백이 다른 컨텍스트(`Normal  >  PianoRoll`)의 손 편집 항목도 같은 자리로 보고, 편집 결과를 엔진이
/// 오류 없이 읽는다.
#[gpui_kit::test]
fn spaced_context_in_user_file_round_trips(cx: &mut TestAppContext) {
    let dir = start(
        cx,
        "spaced",
        Some(
            r#"[{"context": "Normal  >  PianoRoll", "bindings": {"ctrl-alt-u": "track::ToggleSolo"}}]"#,
        ),
    );
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    assert!(
        cx.read(|cx| keymap::errors(cx).is_empty()),
        "{}",
        user_file(&dir)
    );
    let text = user_file(&dir);
    assert!(text.contains("ctrl-alt-u"), "{text}");
    // 백업이 원문을 담는다
    let bak = std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap();
    assert!(bak.contains("Normal  >  PianoRoll"));
}

/// 내보내기: 사용자 파일이 없으면 빈 레이어를 쓰고, 깨져 있으면 쓰지 않고 알린다.
#[gpui_kit::test]
fn export_missing_and_broken_user_file(cx: &mut TestAppContext) {
    let dir = start(cx, "export-edge", None);
    let out = dir.join("out.json");
    cx.update(|cx| settings_window::export_to(cx, &out))
        .unwrap();
    let written = std::fs::read_to_string(&out).unwrap();
    assert!(keyscore_app::keymap::user::UserKeymap::parse(&written).is_ok());
    std::fs::write(dir.join("keymap.json"), "[{").unwrap();
    let out2 = dir.join("out2.json");
    assert!(
        cx.update(|cx| settings_window::export_to(cx, &out2))
            .is_err()
    );
    assert!(!out2.exists());
}

/// 지운 프리셋 키 자리에 다른 Action을 걸면 `null`이 사라지고 새 바인딩이 산다.
#[gpui_kit::test]
fn clear_then_bind_same_key_elsewhere(cx: &mut TestAppContext) {
    let dir = start(cx, "clear-bind", None);
    let (project, pview) = open_project(cx);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "enter m");
    settle(cx, handle);
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
    activate(cx, project);
    press(cx, project, "m");
    let t = cx.read(|cx| pview.read(cx).document().read(cx).project().tracks()[0].clone());
    assert!(t.solo() && !t.mute(), "{}", user_file(&dir));
    assert!(cx.read(|cx| keymap::errors(cx).is_empty()));
}

/// 가져오기가 고른 파일의 `null` 항목은 충돌로 세지 않고 그대로 적용한다.
#[gpui_kit::test]
fn import_with_unbind_entries_applies(cx: &mut TestAppContext) {
    let dir = start(cx, "import-null", None);
    let (handle, view) = open_settings(cx);
    let file = dir.join("in.json");
    std::fs::write(
        &file,
        r#"[{"context": "Normal", "bindings": {"m": null, "ctrl-alt-u": "track::ToggleMute"}}]"#,
    )
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(km::Import), cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
    let keys: Vec<String> = effective_of(cx, "track::ToggleMute")
        .into_iter()
        .map(|(_, k, _)| k)
        .collect();
    assert_eq!(keys, vec!["ctrl-alt-u".to_owned()]);
}
