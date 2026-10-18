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
    // 리뷰 1 지적 7 대응 뒤: 전역 `cmd-w`(window::Close)와 한 경로에서 부딪혀 충돌 시트가 먼저 뜬다
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    press(cx, handle, "enter");
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

// ---------- 리뷰 1 부록 A ----------

/// 인자 있는 Action(`step::Note`)의 줄에서 캡처해도 다른 인자의 바인딩(`s`→`step::Note(s)` 등)은 남아야 한다.
#[gpui_kit::test]
fn brk_capture_on_args_action_keeps_other_variants(cx: &mut TestAppContext) {
    let dir = start(cx, "args", None);
    let (handle, view) = open_settings(cx);
    let before = effective_of(cx, "step::Note").len();
    assert!(before > 5, "{before}");
    select_exact(cx, handle, &view, "step::Note");
    press(cx, handle, "enter ctrl-alt-n");
    settle(cx, handle);
    let after = effective_of(cx, "step::Note");
    assert!(
        after.len() >= before - 1,
        "캡처 한 번에 step::Note 바인딩이 {before}개에서 {}개로: {after:?}\n{}",
        after.len(),
        user_file(&dir)
    );
}

/// 수식키 교환을 켠 채 캡처한 키를 누르면 그 Action이 나와야 한다(KM-11, press-to-bind).
#[gpui_kit::test]
fn brk_capture_with_swap_modifiers_fires_captured_key(cx: &mut TestAppContext) {
    let dir = start(cx, "swap", None);
    let (project, pview) = open_project(cx);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter cmd-ctrl-u");
    settle(cx, handle);
    activate(cx, project);
    press(cx, project, "cmd-ctrl-u");
    assert!(
        muted(cx, &pview),
        "캡처한 cmd-ctrl-u가 뮤트하지 않는다: {}",
        user_file(&dir)
    );
}

/// 전역 바인딩(`mode::Cycle`의 `ctrl-tab`)을 지워도 설정 창의 `ctrl-tab`(`settings::NextSection`, 목록에
/// 그대로 보인다)은 살아 있어야 한다.
#[gpui_kit::test]
fn brk_clearing_global_binding_keeps_context_bindings(cx: &mut TestAppContext) {
    let dir = start(cx, "global-null", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "mode::Cycle");
    press(cx, handle, "backspace");
    let shown = effective_of(cx, "settings::NextSection");
    assert!(
        !shown.is_empty(),
        "편집기는 NextSection이 걸려 있다고 보인다"
    );
    press(cx, handle, "ctrl-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::Ai,
        "ctrl-tab이 죽었다: {shown:?}\n{}",
        user_file(&dir)
    );
}

/// 전역 Action(`settings::Open`)에 목록의 키 `r`을 걸면 충돌로 묻거나, 적어도 목록의 `r`(복원)은 살아야 한다.
#[gpui_kit::test]
fn brk_global_binding_shadowing_context_key_is_a_conflict(cx: &mut TestAppContext) {
    let dir = start(cx, "global-shadow", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    assert!(user_file(&dir).contains("track::ToggleMute") || user_file(&dir).contains("null"));
    select_exact(cx, handle, &view, "settings::Open");
    press(cx, handle, "enter r");
    settle(cx, handle);
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    if sheet {
        return;
    }
    // 시트 없이 걸렸다. 검색란에 `r`이 들어가는지, 목록의 `r`이 아직 복원인지 본다
    press(cx, handle, "/");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, "track::togglemute");
    let query_ok = selected(cx, &view).as_deref() == Some("track::ToggleMute");
    press(cx, handle, "escape");
    press(cx, handle, "/");
    type_text(cx, handle, "togglemute");
    press(cx, handle, "enter");
    assert_eq!(selected(cx, &view).as_deref(), Some("track::ToggleMute"));
    press(cx, handle, "r");

    assert!(
        !effective_of(cx, "track::ToggleMute").is_empty(),
        "시트 없이 r이 settings::Open에 먹혀 복원이 안 된다\n{}",
        user_file(&dir)
    );
    assert!(
        query_ok,
        "검색란에 r을 칠 수 없다(전역 r이 먹는다)\n{}",
        user_file(&dir)
    );
}

/// 충돌 시트가 열린 채 구역을 옮겼다 돌아와도 키보드만으로 시트를 닫을 수 있어야 한다(KM-8, P1).
#[gpui_kit::test]
fn brk_sheet_reachable_after_section_round_trip(cx: &mut TestAppContext) {
    let dir = start(cx, "sheet-stuck", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter o");
    settle(cx, handle);
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    press(cx, handle, "ctrl-tab ctrl-shift-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::Keymap
    );
    press(cx, handle, "escape");
    assert!(
        !cx.read(|cx| view.read(cx).sheet_open()),
        "시트가 남았고 escape가 닿지 않는다"
    );
    let _ = dir;
}

/// 프리셋 선택·수식키 교환을 키보드만으로 할 수 있어야 한다(KM-8, P1). 리뷰 1 대응으로 키맵 구역에 키를
/// 두었다(`alt-1`·`alt-2` 프리셋, `alt-m` 수식키 교환). 탭 이동 + space 전제를 이 키로 바꿨다.
#[gpui_kit::test]
fn brk_preset_and_swap_by_keyboard_only(cx: &mut TestAppContext) {
    start(cx, "kbd-preset", None);
    let (handle, _view) = open_settings(cx);
    press(cx, handle, "alt-2");
    assert_eq!(cx.read(keymap::options).preset, Preset::Vim);
    press(cx, handle, "alt-m");
    assert!(cx.read(keymap::options).swap_alt_cmd);
    // 수식키 교환이 켜지면 프리셋 키의 alt가 cmd가 된다(KM-11): alt-m은 cmd-m, alt-1은 cmd-1
    press(cx, handle, "cmd-m alt-1");
    assert!(!cx.read(keymap::options).swap_alt_cmd);
    assert_eq!(cx.read(keymap::options).preset, Preset::Default);
}

/// 표준 단축키를 덮는 키맵을 가져오면 경고해야 한다(계획 7항, KM-12).
#[gpui_kit::test]
fn brk_import_warns_standard_override(cx: &mut TestAppContext) {
    let dir = start(cx, "import-std", None);
    let (handle, view) = open_settings(cx);
    let file = dir.join("in.json");
    std::fs::write(&file, r#"[{"bindings": {"cmd-s": "track::ToggleMute"}}]"#).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(km::Import), cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    assert!(
        sheet || msg.contains("표준"),
        "표준 단축키 덮기 경고 없음: {msg:?}"
    );
}

/// 손으로 쓴 주석은 편집기가 두 번 써도 어딘가(.bak)에 남아야 한다.
#[gpui_kit::test]
fn brk_bak_keeps_hand_written_original_after_two_edits(cx: &mut TestAppContext) {
    let original = "// 내 메모\n[{\"context\": \"Normal\", \"bindings\": {\"ctrl-alt-u\": \"track::ToggleMute\"}}]\n";
    let dir = start(cx, "bak", Some(original));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::New");
    press(cx, handle, "backspace");
    let bak = std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap_or_default();
    assert!(bak.contains("내 메모"), "원본 주석이 사라졌다: {bak}");
}

#[gpui_kit::test]
fn brk_reopen_settings_after_close(cx: &mut TestAppContext) {
    start(cx, "reopen", None);
    let (project, _) = open_project(cx);
    let (handle, _view) = open_settings(cx);
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    for _ in 0..3 {
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
    }
    assert_eq!(cx.update(|cx| cx.windows().len()), 1, "창이 닫히지 않았다");
    activate(cx, project);
    press(cx, project, "cmd-,");
    assert_eq!(
        cx.update(|cx| cx.windows().len()),
        2,
        "설정 창을 닫은 뒤 cmd-,가 다시 열지 못한다(current: {})",
        cx.update(|cx| settings_window::current(cx)).is_some()
    );
}

/// 창 닫기(`window::Close`)를 키로 부르면 지금 창이 닫혀야 한다(계획 1항 「`cmd-w`로 닫는다」).
#[gpui_kit::test]
fn brk_cmd_w_closes_settings_window(cx: &mut TestAppContext) {
    start(cx, "cmdw-close", None);
    let (_project, _) = open_project(cx);
    let (handle, _view) = open_settings(cx);
    assert_eq!(cx.update(|cx| cx.active_window()), Some(handle));
    // 창이 닫히므로 그리지 않는 키 보내기를 쓴다
    cx.simulate_keystrokes(handle, "cmd-w");
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.windows().len()),
        1,
        "cmd-w가 설정 창을 닫지 않았다"
    );
}

/// 팔레트에 보이는 「vim 프리셋」(`keymap::SelectPreset`)을 프로젝트 창의 팔레트에서 고르면 프리셋이 바뀌어야
/// 한다(P1: 키보드만으로 모든 기능, 설정 창 위 줄의 프리셋·수식키 교환에는 키가 없다).
#[gpui_kit::test]
fn brk_palette_select_preset_reaches_handler(cx: &mut TestAppContext) {
    start(cx, "pal-preset", None);
    let (project, _) = open_project(cx);
    let (_h, _v) = open_settings(cx);
    activate(cx, project);
    press(cx, project, "cmd-shift-p");
    type_text(cx, project, "vim 프리셋");
    press(cx, project, "enter");
    assert_eq!(
        cx.read(keymap::options).preset,
        Preset::Vim,
        "팔레트의 vim 프리셋이 아무 일도 하지 않는다"
    );
}
