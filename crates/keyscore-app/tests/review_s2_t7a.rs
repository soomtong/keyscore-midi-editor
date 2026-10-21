//! S2-T7a 적대적 리뷰 1회차(`docs/reports/s2-t7a/review-1.md`). 깨뜨리지 못한 시도만 남긴다. 깨뜨린 시도는
//! 리뷰 문서 부록에 있다.

use std::path::{Path, PathBuf};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext, VisualTestContext,
};
use keyscore_app::actions::{file, track, window as window_actions};
use keyscore_app::files::{self, FileError};
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    activate(cx, handle);
    (handle, view)
}

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn touch(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    run(cx, handle, track::New);
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
}

fn is_open(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update(|cx| cx.windows().contains(&handle))
}

fn answer(cx: &mut TestAppContext, button: &str) {
    assert!(cx.has_pending_prompt(), "시트가 없다");
    cx.simulate_prompt_answer(button);
    cx.run_until_parked();
}

fn answer_new_path(cx: &mut TestAppContext, path: Option<PathBuf>) {
    assert!(cx.did_prompt_for_new_path(), "새 경로 대화상자가 없다");
    cx.simulate_new_path_selection(move |_| path);
    cx.run_until_parked();
}

fn prompt_text(cx: &mut TestAppContext) -> String {
    let (msg, detail) = cx.pending_prompt().expect("시트가 없다");
    format!("{msg}\n{detail}")
}

fn start_quit(cx: &mut TestAppContext) -> std::rc::Rc<std::cell::Cell<Option<bool>>> {
    let result = std::rc::Rc::new(std::cell::Cell::new(None));
    let slot = result.clone();
    cx.update(|cx| {
        let task = files::confirm_close_all(cx);
        cx.spawn(async move |_| slot.set(Some(task.await))).detach();
    });
    cx.run_until_parked();
    result
}

fn simulate_close(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    let closed = VisualTestContext::from_window(handle, cx).simulate_close();
    cx.run_until_parked();
    closed
}

fn sample() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1,
            "clips": [{"start": 0, "length": 3840, "notes": [[0, 480, 60, 51492]]}]}]}"#,
    )
    .unwrap()
    .0
}

fn write_sample(path: &Path) {
    std::fs::write(path, ksq::to_string(&sample())).unwrap();
}

// ---------- 원본이 바뀌면 안 되는 경로(save_project 직접) ----------

/// 0444 파일을 가리키는 링크로 저장해도 대상은 그대로다.
#[test]
fn review_save_project_through_symlink_to_read_only_target_refuses() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-{}-symlink", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("real")).unwrap();
    let target = dir.join("real/locked.ksq");
    write_sample(&target);
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o444)).unwrap();
    let link = dir.join("link.ksq");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let before = std::fs::read(&target).unwrap();
    let result = files::save_project(&link, &Project::new());
    assert!(
        matches!(result, Err(FileError::PermissionDenied)),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&target).unwrap(), before);
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "링크가 일반 파일로 바뀌었다"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 파일은 쓸 수 있지만 폴더가 0555면 쓰기가 실패하고 원본은 그대로다.
#[test]
fn review_save_project_in_read_only_folder_keeps_original() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-{}-rofolder", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.ksq");
    write_sample(&path);
    let before = std::fs::read(&path).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let result = files::save_project(&path, &Project::new());
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err(), "폴더에 쓸 수 없는데 성공했다");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(leftovers.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// 잠긴 파일(`chflags uchg`)도 쓰기로 열 수 없으니 거부한다. rename은 잠긴 파일도 바꾸지 못하지만
/// 오류가 권한 시트로 이어지는지 본다.
#[test]
fn review_save_project_immutable_flag_refuses_and_keeps_original() {
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-{}-uchg", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.ksq");
    write_sample(&path);
    let before = std::fs::read(&path).unwrap();
    let locked = std::process::Command::new("chflags")
        .arg("uchg")
        .arg(&path)
        .status()
        .unwrap();
    assert!(locked.success());
    let result = files::save_project(&path, &Project::new());
    let _ = std::process::Command::new("chflags")
        .arg("nouchg")
        .arg(&path)
        .status();
    assert!(
        matches!(result, Err(FileError::PermissionDenied)),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 폴더 경로에 저장하면 실패하고 폴더는 그대로다.
#[test]
fn review_save_project_onto_directory_fails() {
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-{}-ontodir", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("folder.ksq")).unwrap();
    std::fs::write(dir.join("folder.ksq/keep"), "x").unwrap();
    let result = files::save_project(&dir.join("folder.ksq"), &Project::new());
    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(dir.join("folder.ksq/keep")).unwrap(),
        "x"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- 같은 파일 두 창 금지 ----------

/// 링크로 가리켜 열어도 같은 창이다.
#[gpui_kit::test]
fn review_open_through_symlink_activates_existing_window(cx: &mut TestAppContext) {
    let dir = start(cx, "open-link");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let link = dir.join("alias.ksq");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let (first, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let (again, _) = cx.update(|cx| files::open_path(&link, cx)).unwrap();
    assert_eq!(again, first);
    assert_eq!(cx.update(|cx| open_project_windows(cx)).len(), 1);
}

/// 다른 창이 연 파일을 가리키는 링크로 「다른 이름으로 저장」해도 거부한다.
#[gpui_kit::test]
fn review_save_as_onto_symlink_to_other_windows_file_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "saveas-link");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let link = dir.join("alias.ksq");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, _) = open(cx, Project::new());
    touch(cx, handle);
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(link.clone()));
    assert!(prompt_text(cx).contains("이미"), "{}", prompt_text(cx));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

// ---------- 시트와 file_flow ----------

/// 「다른 이름으로 저장」 대화상자가 떠 있는 동안 창 닫기 단추는 무시되고, 대화상자를 취소하면 다시 닫을 수 있다.
#[gpui_kit::test]
fn review_close_during_save_as_dialog_is_ignored_then_flow_clears(cx: &mut TestAppContext) {
    start(cx, "close-during-dialog");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, file::SaveAs);
    assert!(!simulate_close(cx, handle));
    assert!(!cx.has_pending_prompt(), "대화상자 위에 시트를 겹쳤다");
    answer_new_path(cx, None);
    assert!(is_open(cx, handle));
    assert!(!simulate_close(cx, handle));
    assert!(prompt_text(cx).contains("변경 사항"));
    answer(cx, "저장 안 함");
    assert!(!is_open(cx, handle));
}

/// 닫기 시트에서 [저장] → 권한 시트 [취소]면 창은 남고, 흐름 표시가 풀려 다시 닫을 수 있다.
#[gpui_kit::test]
fn review_close_save_on_read_only_file_cancel_keeps_window(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = start(cx, "close-perm");
    let path = dir.join("locked.ksq");
    write_sample(&path);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, view) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    touch(cx, handle);
    run(cx, handle, window_actions::Close);
    answer(cx, "저장");
    assert!(prompt_text(cx).contains("권한"), "{}", prompt_text(cx));
    answer(cx, "취소");
    assert!(is_open(cx, handle));
    assert!(dirty(cx, &view));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!simulate_close(cx, handle));
    answer(cx, "저장 안 함");
    assert!(!is_open(cx, handle));
}

/// 닫기 시트가 떠 있는 동안 끝내기는 묻지 않고 멈추며, 그 뒤 다시 끝내면 정상으로 묻는다(흐름 표시가 남지 않는다).
#[gpui_kit::test]
fn review_quit_during_close_sheet_aborts_then_works(cx: &mut TestAppContext) {
    start(cx, "quit-during-close");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    assert!(!simulate_close(cx, handle));
    let quit = start_quit(cx);
    assert_eq!(quit.get(), Some(false));
    answer(cx, "취소");
    assert!(is_open(cx, handle));
    let quit = start_quit(cx);
    answer(cx, "저장 안 함");
    assert_eq!(quit.get(), Some(true));
}

/// 끝내기 시트에서 [저장]을 누른 새 프로젝트가 경로 대화상자를 취소하면 끝내지 않고 창도 남는다.
#[gpui_kit::test]
fn review_quit_save_untitled_cancel_dialog_aborts(cx: &mut TestAppContext) {
    start(cx, "quit-save-untitled");
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    let quit = start_quit(cx);
    answer(cx, "저장");
    answer_new_path(cx, None);
    assert_eq!(quit.get(), Some(false));
    assert!(is_open(cx, handle));
    assert!(dirty(cx, &view));
    assert!(!simulate_close(cx, handle), "닫기는 다시 시트를 띄운다");
    assert!(cx.has_pending_prompt());
}

// ---------- 최근 파일 ----------

/// 손상된 설정(문자열 아닌 항목, 중복, 초과)을 읽어도 목록은 10개 이하, 문자열 중복 없음.
#[gpui_kit::test]
fn review_recent_files_from_corrupt_settings_are_sanitised(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-{}-corrupt", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut items: Vec<serde_json::Value> = (0..15)
        .map(|i| serde_json::Value::String(format!("/nope/{i}.ksq")))
        .collect();
    items.insert(1, serde_json::json!(42));
    items.insert(2, serde_json::json!("/nope/0.ksq"));
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({ "recent_files": items }).to_string(),
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let recent = cx.update(|cx| files::recent_files(cx));
    assert_eq!(recent.len(), files::MAX_RECENT_FILES);
    assert_eq!(recent[1], PathBuf::from("/nope/1.ksq"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 「최근 파일」에서 연 파일이 깨졌으면 목록에서 빠지고 창은 늘지 않는다.
#[gpui_kit::test]
fn review_open_recent_corrupt_file_drops_entry(cx: &mut TestAppContext) {
    let dir = start(cx, "recent-corrupt");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (handle, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    // `simulate_close`는 닫아도 되는지만 묻는다. 실제로 닫으려면 Action을 쓴다.
    run(cx, handle, window_actions::Close);
    assert!(!is_open(cx, handle));
    std::fs::write(&path, "{ not json").unwrap();
    let (host, _) = open(cx, sample());
    touch(cx, host);
    run(
        cx,
        host,
        file::OpenRecent(path.canonicalize().unwrap().display().to_string()),
    );
    assert!(prompt_text(cx).contains("열지 못했다"));
    answer(cx, "확인");
    assert!(cx.update(|cx| files::recent_files(cx)).is_empty());
    assert_eq!(cx.update(|cx| open_project_windows(cx)).len(), 1);
}

// ---------- 같은 파일: 대소문자·정규화(이 저장소의 임시 폴더는 APFS, 둘 다 가리지 않는다) ----------

/// 대소문자만 다른 경로로 같은 파일을 열면 같은 창이어야 한다(macOS 기본 볼륨은 대소문자를 가리지 않는다).
#[gpui_kit::test]
fn review_open_case_variant_activates_existing_window(cx: &mut TestAppContext) {
    let dir = start(cx, "case");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let upper = dir.join("TUNE.ksq");
    if !upper.exists() {
        return; // 대소문자를 가리는 볼륨
    }
    let (first, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let (again, _) = cx.update(|cx| files::open_path(&upper, cx)).unwrap();
    assert_eq!(again, first, "같은 파일이 두 창에 열렸다");
}

/// 다른 창이 연 파일을 대소문자만 바꾼 이름으로 「다른 이름으로 저장」하면 거부해야 한다.
#[gpui_kit::test]
fn review_save_as_case_variant_of_other_window_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "case-saveas");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let upper = dir.join("TUNE.ksq");
    if !upper.exists() {
        return;
    }
    cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, _) = open(cx, Project::new());
    touch(cx, handle);
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(upper));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "다른 창이 연 파일을 덮었다"
    );
}

/// 한글 이름을 NFD로 가리켜 열어도 같은 창이어야 한다(APFS는 정규화를 가리지 않는다).
#[gpui_kit::test]
fn review_open_nfd_name_activates_existing_window(cx: &mut TestAppContext) {
    let dir = start(cx, "nfd");
    let nfc = dir.join("\u{AC00}.ksq"); // 가
    let nfd = dir.join("\u{1100}\u{1161}.ksq"); // ᄀ + ᅡ
    write_sample(&nfc);
    if !nfd.exists() {
        return;
    }
    let (first, _) = cx.update(|cx| files::open_path(&nfc, cx)).unwrap();
    let (again, _) = cx.update(|cx| files::open_path(&nfd, cx)).unwrap();
    assert_eq!(again, first, "같은 파일이 두 창에 열렸다");
}
