//! S2-T6a 설정 창과 키맵 편집기(KM-2·3·4·8·9·11, 7.3). 키는 `simulate_keystrokes`(키 바인딩을 거침).

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{keymap as km, settings};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

/// 시퀀스 확정 대기(KM-5)보다 조금 길게.
const SETTLE: Duration = Duration::from_millis(1100);

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-settings-{}-{name}", std::process::id()));
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn settle(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.executor().advance_clock(SETTLE);
    cx.run_until_parked();
    render(cx, handle);
}

/// 열린 설정 창과 그 뷰.
fn settings_view(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<SettingsWindow>) {
    let found = cx.update(|cx| settings_window::current(cx));
    let (handle, view) = found.expect("설정 창이 없다");
    render(cx, handle);
    (handle, view)
}

fn muted(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).project().tracks()[0].mute())
}

fn user_file(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("keymap.json")).unwrap_or_default()
}

fn selected(cx: &mut TestAppContext, view: &Entity<SettingsWindow>) -> Option<String> {
    cx.read(|cx| view.read(cx).selected_action(cx))
}

/// 목록을 검색으로 좁혀 그 Action을 고른다(키보드만).
fn select_action(cx: &mut TestAppContext, handle: AnyWindowHandle, query: &str) {
    press(cx, handle, "/");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, query);
    press(cx, handle, "enter");
}

// ---------- 3. 설정 창 열기와 구역 ----------

#[gpui_kit::test]
fn settings_open_and_sections(cx: &mut TestAppContext) {
    start(cx, "open", None);
    let (project, _) = open_project(cx);
    press(cx, project, "cmd-,");
    let (handle, view) = settings_view(cx);
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        keyscore_app::actions::args::SettingsSection::Keymap
    );
    // 두 번 열어도 창은 하나
    press(cx, project, "cmd-,");
    let windows = cx.update(|cx| cx.windows().len());
    assert_eq!(windows, 2);
    // 구역 이동
    activate(cx, handle);
    press(cx, handle, "ctrl-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        keyscore_app::actions::args::SettingsSection::Ai
    );
    press(cx, handle, "ctrl-shift-tab ctrl-shift-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        keyscore_app::actions::args::SettingsSection::Metronome
    );
    // 구역을 지정해 연다
    run(
        cx,
        project,
        settings::Open(keyscore_app::actions::args::Opt(Some(
            keyscore_app::actions::args::SettingsSection::MidiOutput,
        ))),
    );
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        keyscore_app::actions::args::SettingsSection::MidiOutput
    );
    run(cx, project, km::Edit);
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        keyscore_app::actions::args::SettingsSection::Keymap
    );
}

// ---------- 4. 단계 게이트: 키보드만으로 편집기 끝까지 ----------

#[gpui_kit::test]
fn keymap_editor_keyboard_only_flow(cx: &mut TestAppContext) {
    let dir = start(cx, "flow", None);
    let (project, pview) = open_project(cx);
    press(cx, project, "cmd-,");
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    select_action(cx, handle, "track::togglemute");
    assert_eq!(selected(cx, &view).as_deref(), Some("track::ToggleMute"));
    // enter → 캡처 → 키 → 1 s 뒤 확정
    press(cx, handle, "enter");
    assert!(cx.read(|cx| view.read(cx).capturing()));
    press(cx, handle, "ctrl-alt-u");
    settle(cx, handle);
    assert!(!cx.read(|cx| view.read(cx).capturing()));
    let text = user_file(&dir);
    assert!(text.contains("ctrl-alt-u"), "{text}");
    assert!(text.contains("track::ToggleMute"), "{text}");
    // 엔진에 걸렸다: 새 키가 뮤트하고 옛 키 `m`은 하지 않는다
    activate(cx, project);
    press(cx, project, "ctrl-alt-u");
    assert!(muted(cx, &pview));
    press(cx, project, "m");
    assert!(muted(cx, &pview));
    // 편집기로 돌아가 지우기(backspace)
    activate(cx, handle);
    press(cx, handle, "backspace");
    activate(cx, project);
    press(cx, project, "ctrl-alt-u");
    assert!(muted(cx, &pview));
    // 복원(r): 프리셋의 `m`이 돌아온다. 사용자 파일에 ToggleMute 항목이 없다
    activate(cx, handle);
    press(cx, handle, "r");
    assert!(!user_file(&dir).contains("track::ToggleMute"));
    activate(cx, project);
    press(cx, project, "m");
    assert!(!muted(cx, &pview));
    // 편집이 엔진 오류를 남기지 않았다
    assert!(cx.read(|cx| keymap::errors(cx).is_empty()));
}

// ---------- 5. 캡처 중 키는 디스패치되지 않고 Esc가 취소한다 ----------

#[gpui_kit::test]
fn capture_escape_cancels_and_keys_do_not_dispatch(cx: &mut TestAppContext) {
    let dir = start(cx, "capture-esc", None);
    run_settings(cx);
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    select_action(cx, handle, "track::togglemute");
    press(cx, handle, "enter");
    // 캡처 중 `r`(RestoreDefault)·`backspace`(ClearBinding)는 Action이 아니라 캡처된 키다
    press(cx, handle, "r");
    assert!(cx.read(|cx| view.read(cx).capturing()));
    press(cx, handle, "escape");
    assert!(!cx.read(|cx| view.read(cx).capturing()));
    settle(cx, handle);
    assert_eq!(user_file(&dir), "");
    // 취소 뒤 목록 키가 다시 산다: `/`로 검색란
    press(cx, handle, "/");
    assert!(cx.read(|cx| view.read(cx).search_focused_for_test()));
}

fn run_settings(cx: &mut TestAppContext) {
    cx.update(|cx| settings_window::open(cx, None));
    cx.run_until_parked();
}

// ---------- 6. 충돌 시트 ----------

#[gpui_kit::test]
fn binding_conflict_sheet_asks_which_to_keep(cx: &mut TestAppContext) {
    let dir = start(cx, "conflict", None);
    let (project, pview) = open_project(cx);
    run_settings(cx);
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    // ToggleMute에 `o`(ToggleSolo의 키)
    select_action(cx, handle, "track::togglemute");
    press(cx, handle, "enter o");
    settle(cx, handle);
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    let text = cx.update_window(handle, |_, window, _| {
        window.find("keymap-sheet").label().map(str::to_owned)
    });
    let text = text.unwrap().unwrap();
    assert!(text.contains("track::ToggleSolo"), "{text}");
    // 취소: 바뀌지 않는다
    press(cx, handle, "escape");
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
    assert_eq!(user_file(&dir), "");
    // 다시, 이번엔 새 바인딩 유지
    press(cx, handle, "enter o");
    settle(cx, handle);
    press(cx, handle, "enter");
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
    activate(cx, project);
    press(cx, project, "o");
    let t = cx.read(|cx| pview.read(cx).document().read(cx).project().tracks()[0].clone());
    assert!(t.mute(), "o가 뮤트를 내야 한다");
    assert!(!t.solo(), "o는 더 솔로가 아니다");
}

#[gpui_kit::test]
fn prefix_conflict_opens_sheet(cx: &mut TestAppContext) {
    start(cx, "prefix", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    run_settings(cx);
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    // vim Normal의 `g g`(cursor::ClipStart)와 `g t`가 있다. ToggleMute에 `g`
    select_action(cx, handle, "track::togglemute");
    press(cx, handle, "enter g");
    settle(cx, handle);
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    press(cx, handle, "escape");
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
}

// ---------- 7. 프리셋 전환과 수식키 교환 ----------

#[gpui_kit::test]
fn preset_switch_keeps_user_overrides(cx: &mut TestAppContext) {
    let dir = start(
        cx,
        "preset",
        Some(r#"[{"context": "Normal", "bindings": {"ctrl-alt-u": "track::ToggleMute"}}]"#),
    );
    let (project, pview) = open_project(cx);
    run_settings(cx);
    let (handle, _view) = settings_view(cx);
    cx.update_window(handle, |_, window, cx| {
        window.click("keymap-preset-vim", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.read(keymap::options).preset, Preset::Vim);
    assert_eq!(
        cx.read(|cx| keyscore_app::app_state::AppSettings::get(cx).keymap.preset),
        Preset::Vim
    );
    assert!(user_file(&dir).contains("ctrl-alt-u"));
    activate(cx, project);
    press(cx, project, "ctrl-alt-u");
    assert!(muted(cx, &pview));
    // 수식키 교환 토글
    cx.update_window(handle, |_, window, cx| window.click("keymap-swap", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(cx.read(keymap::options).swap_alt_cmd);
    cx.update_window(handle, |_, window, cx| {
        window.click("keymap-preset-default", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.read(keymap::options).preset, Preset::Default);
}

// ---------- 8. 단계 게이트: 가져온 키맵 충돌은 적용 전에 ----------

#[gpui_kit::test]
fn import_conflicts_are_caught_before_apply(cx: &mut TestAppContext) {
    let dir = start(cx, "import", None);
    run_settings(cx);
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    let file = dir.join("incoming.json");
    std::fs::write(
        &file,
        r#"[{"context": "Normal", "bindings": {
            "ctrl-alt-u": "track::ToggleMute",
            "ctrl-alt-v": "track::ToggleSolo"
        }},
        {"context": "Normal", "bindings": {"ctrl-alt-u": "track::New"}}]"#,
    )
    .unwrap();
    run(cx, handle, km::Import);
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    render(cx, handle);
    // 충돌 시트가 먼저 뜨고, 아직 적용되지 않았다
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    assert_eq!(user_file(&dir), "");
    press(cx, handle, "escape");
    assert_eq!(user_file(&dir), "");
    // 다시 가져와 적용: 충돌에서 뒤 항목(track::New)만 남는다
    run(cx, handle, km::Import);
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    let text = user_file(&dir);
    assert!(text.contains("track::New"), "{text}");
    assert!(text.contains("track::ToggleSolo"), "{text}");
    assert!(!text.contains("track::ToggleMute"), "{text}");
    assert!(cx.read(|cx| keymap::errors(cx).is_empty()));
    // 문법 오류 파일은 거부하고 바꾸지 않는다
    std::fs::write(&file, "[{").unwrap();
    run(cx, handle, km::Import);
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    render(cx, handle);
    assert!(!cx.read(|cx| view.read(cx).sheet_open()));
    assert_eq!(user_file(&dir), text);
    assert!(
        cx.read(|cx| view.read(cx).message())
            .is_some_and(|m| m.contains("가져오지 못했다"))
    );
}

// ---------- 9. 내보내기 ----------

#[gpui_kit::test]
fn export_writes_user_layer(cx: &mut TestAppContext) {
    let user =
        r#"[{"context": "Normal", "bindings": {"ctrl-alt-u": "track::ToggleMute", "o": null}}]"#;
    let dir = start(cx, "export", Some(user));
    run_settings(cx);
    let (handle, _view) = settings_view(cx);
    let out = dir.join("out.json");
    run(cx, handle, km::Export);
    cx.simulate_new_path_selection(|_| Some(out.clone()));
    cx.run_until_parked();
    let written = std::fs::read_to_string(&out).unwrap();
    let a = keyscore_app::keymap::user::UserKeymap::parse(user).unwrap();
    let b = keyscore_app::keymap::user::UserKeymap::parse(&written).unwrap();
    assert_eq!(a, b);
}

// ---------- 10. 깨진 사용자 파일은 덮지 않는다 ----------

#[gpui_kit::test]
fn editor_refuses_to_overwrite_broken_user_file(cx: &mut TestAppContext) {
    let dir = start(cx, "broken", Some("[{ // 고치는 중"));
    run_settings(cx);
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    select_action(cx, handle, "track::togglemute");
    press(cx, handle, "enter ctrl-alt-u");
    settle(cx, handle);
    assert_eq!(user_file(&dir), "[{ // 고치는 중");
    assert!(
        cx.read(|cx| view.read(cx).message())
            .is_some_and(|m| m.contains("keymap.json"))
    );
}
