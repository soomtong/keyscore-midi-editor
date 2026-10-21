//! S2-T7a 새로 만들기·열기·저장·닫기·최근 파일(IO-1, 7.5). 대화상자와 시트는 GPUI 시험 플랫폼이 대신 답한다
//! (`simulate_new_path_selection`, `simulate_path_prompt_response`, `simulate_prompt_answer`, `simulate_close`).

use std::path::{Path, PathBuf};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext, VisualTestContext,
};
use keyscore_app::actions::{app, file, track, window as window_actions};
use keyscore_app::document::ReadOnly;
use keyscore_app::files::{self, MAX_RECENT_FILES};
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-files-{}-{name}", std::process::id()));
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
    render(cx, handle);
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
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

/// 문서를 바꾼다(트랙 하나 추가).
fn touch(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    run(cx, handle, track::New);
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
}

fn doc_path(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<PathBuf> {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .path()
            .map(Path::to_path_buf)
    })
}

fn read_only(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<ReadOnly> {
    cx.read(|cx| view.read(cx).document().read(cx).read_only())
}

fn text_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| ksq::to_string(view.read(cx).document().read(cx).project()))
}

fn title(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    VisualTestContext::from_window(handle, cx)
        .window_title()
        .unwrap_or_default()
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

fn windows(cx: &mut TestAppContext) -> Vec<Entity<ProjectWindow>> {
    cx.update(|cx| open_project_windows(cx))
}

fn is_open(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update(|cx| cx.windows().contains(&handle))
}

/// 문서는 정규화한 경로를 가진다(같은 파일을 두 창에서 열지 않게).
fn canon(path: &Path) -> PathBuf {
    path.canonicalize().unwrap()
}

fn window_for(cx: &mut TestAppContext, path: &Path) -> Entity<ProjectWindow> {
    let path = &canon(path);
    let all = windows(cx);
    let found = all
        .into_iter()
        .find(|w| doc_path(cx, w).as_deref() == Some(path));
    found.unwrap_or_else(|| panic!("{} 창이 없다", path.display()))
}

fn handle_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> AnyWindowHandle {
    cx.read(|cx| view.read(cx).window_handle())
}

fn answer_new_path(cx: &mut TestAppContext, path: Option<PathBuf>) {
    assert!(cx.did_prompt_for_new_path(), "새 경로 대화상자가 없다");
    cx.simulate_new_path_selection(move |_| path);
    cx.run_until_parked();
}

fn answer_open(cx: &mut TestAppContext, paths: Vec<PathBuf>) {
    assert!(cx.did_prompt_for_paths(), "열기 대화상자가 없다");
    cx.simulate_path_prompt_response(move |_| Some(paths));
    cx.run_until_parked();
}

fn answer(cx: &mut TestAppContext, button: &str) {
    assert!(cx.has_pending_prompt(), "시트가 없다");
    cx.simulate_prompt_answer(button);
    cx.run_until_parked();
}

fn prompt_text(cx: &mut TestAppContext) -> String {
    let (msg, detail) = cx.pending_prompt().expect("시트가 없다");
    format!("{msg}\n{detail}")
}

/// 끝내기 확인을 시작한다. 결과는 끝나면 채워진다.
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

/// 노트 하나 있는 트랙 하나.
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

// ---------- 저장 ----------

#[gpui_kit::test]
fn save_untitled_prompts_for_path_and_writes_file(cx: &mut TestAppContext) {
    let dir = start(cx, "save-untitled");
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    assert!(title(cx, handle).contains("제목 없음"));
    run(cx, handle, file::Save);
    let path = dir.join("song.ksq");
    answer_new_path(cx, Some(path.clone()));
    let on_disk = std::fs::read_to_string(&path).expect("파일이 생기지 않았다");
    assert_eq!(on_disk, text_of(cx, &view));
    assert!(!dirty(cx, &view));
    assert_eq!(doc_path(cx, &view), Some(canon(&path)));
    assert!(
        title(cx, handle).contains("song.ksq"),
        "{}",
        title(cx, handle)
    );
    assert_eq!(
        VisualTestContext::from_window(handle, cx).document_path(),
        Some(canon(&path))
    );
}

#[gpui_kit::test]
fn save_existing_path_writes_without_prompt(cx: &mut TestAppContext) {
    let dir = start(cx, "save-existing");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, view) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    assert!(!dirty(cx, &view));
    touch(cx, handle);
    assert!(dirty(cx, &view));
    run(cx, handle, file::Save);
    assert!(!cx.did_prompt_for_new_path());
    assert!(!cx.has_pending_prompt());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text_of(cx, &view));
    assert!(!dirty(cx, &view));
}

#[gpui_kit::test]
fn save_as_moves_window_to_new_path(cx: &mut TestAppContext) {
    let dir = start(cx, "save-as");
    let first = dir.join("first.ksq");
    write_sample(&first);
    let before = std::fs::read(&first).unwrap();
    let (handle, view) = cx.update(|cx| files::open_path(&first, cx)).unwrap();
    activate(cx, handle);
    touch(cx, handle);
    run(cx, handle, file::SaveAs);
    let second = dir.join("second.ksq");
    answer_new_path(cx, Some(second.clone()));
    assert_eq!(
        std::fs::read(&first).unwrap(),
        before,
        "원래 파일이 바뀌었다"
    );
    assert_eq!(
        std::fs::read_to_string(&second).unwrap(),
        text_of(cx, &view)
    );
    assert_eq!(doc_path(cx, &view), Some(canon(&second)));
    assert!(title(cx, handle).contains("second.ksq"));
    assert!(!dirty(cx, &view));
}

#[gpui_kit::test]
fn save_as_cancelled_keeps_state(cx: &mut TestAppContext) {
    let dir = start(cx, "save-as-cancel");
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, None);
    assert!(dirty(cx, &view));
    assert_eq!(doc_path(cx, &view), None);
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "ksq"))
        .collect();
    assert!(entries.is_empty());
}

#[gpui_kit::test]
fn save_ksqz_extension_writes_gzip(cx: &mut TestAppContext) {
    let dir = start(cx, "save-ksqz");
    let (handle, view) = open(cx, sample());
    run(cx, handle, file::SaveAs);
    let path = dir.join("packed.ksqz");
    answer_new_path(cx, Some(path.clone()));
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(&[0x1f, 0x8b]), "gzip이 아니다");
    let (back, _) = ksq::from_bytes(&bytes).unwrap();
    assert_eq!(ksq::to_string(&back), text_of(cx, &view));
}

#[gpui_kit::test]
fn save_write_failure_shows_error_and_stays_dirty(cx: &mut TestAppContext) {
    let dir = start(cx, "save-fail");
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let path = sub.join("a.ksq");
    write_sample(&path);
    let (handle, view) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    touch(cx, handle);
    std::fs::remove_dir_all(&sub).unwrap();
    run(cx, handle, file::Save);
    assert!(
        prompt_text(cx).contains("저장하지 못했다"),
        "{}",
        prompt_text(cx)
    );
    answer(cx, "확인");
    assert!(dirty(cx, &view));
}

#[gpui_kit::test]
fn save_as_to_path_open_in_other_window_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "save-as-open");
    let taken = dir.join("taken.ksq");
    write_sample(&taken);
    let before = std::fs::read(&taken).unwrap();
    let (_, _other) = cx.update(|cx| files::open_path(&taken, cx)).unwrap();
    let (handle, view) = open(cx, Project::new());
    touch(cx, handle);
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(taken.clone()));
    assert!(prompt_text(cx).contains("다른 창"), "{}", prompt_text(cx));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&taken).unwrap(), before);
    assert_eq!(doc_path(cx, &view), None);
    assert!(dirty(cx, &view));
}

// ---------- 읽기 전용 ----------

#[gpui_kit::test]
fn newer_version_opens_read_only_and_save_offers_save_as(cx: &mut TestAppContext) {
    let dir = start(cx, "newer");
    let path = dir.join("future.ksq");
    std::fs::write(
        &path,
        r#"{"format_version": 7, "future": {"x": 1}, "tracks": []}"#,
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, view) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    assert_eq!(read_only(cx, &view), Some(ReadOnly::NewerFormat(7)));
    assert!(
        title(cx, handle).contains("읽기 전용"),
        "{}",
        title(cx, handle)
    );
    assert!(
        messages(cx, &view)
            .iter()
            .any(|m| m.contains("형식 버전 7")),
        "{:?}",
        messages(cx, &view)
    );
    touch(cx, handle);
    run(cx, handle, file::Save);
    assert!(!cx.did_prompt_for_new_path(), "시트 없이 경로를 물었다");
    assert!(prompt_text(cx).contains("새 버전"), "{}", prompt_text(cx));
    answer(cx, "다른 이름으로 저장");
    let copy = dir.join("copy.ksq");
    answer_new_path(cx, Some(copy.clone()));
    assert_eq!(std::fs::read(&path).unwrap(), before, "원본이 바뀌었다");
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&copy).unwrap()).unwrap();
    assert_eq!(written["format_version"], 1);
    assert_eq!(written["future"]["x"], 1, "모르는 키가 사라졌다");
    assert_eq!(read_only(cx, &view), None);
    assert_eq!(doc_path(cx, &view), Some(canon(&copy)));
    assert!(!title(cx, handle).contains("읽기 전용"));
}

#[gpui_kit::test]
fn newer_version_save_cancel_keeps_original(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-cancel");
    let path = dir.join("future.ksq");
    std::fs::write(&path, r#"{"format_version": 2}"#).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, view) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    touch(cx, handle);
    run(cx, handle, file::Save);
    answer(cx, "취소");
    assert!(!cx.did_prompt_for_new_path());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(dirty(cx, &view));
}

#[cfg(unix)]
#[gpui_kit::test]
fn save_read_only_permission_file_refuses_and_keeps_original(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = start(cx, "perm");
    let path = dir.join("locked.ksq");
    write_sample(&path);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, view) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    touch(cx, handle);
    run(cx, handle, file::Save);
    assert!(prompt_text(cx).contains("권한"), "{}", prompt_text(cx));
    answer(cx, "취소");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "읽기 전용 파일이 바뀌었다"
    );
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o444);
    assert!(dirty(cx, &view));
    // [다른 이름으로 저장]은 새 경로로 이어진다.
    run(cx, handle, file::Save);
    answer(cx, "다른 이름으로 저장");
    let copy = dir.join("unlocked.ksq");
    answer_new_path(cx, Some(copy.clone()));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), text_of(cx, &view));
    assert!(!dirty(cx, &view));
}

#[cfg(unix)]
#[test]
fn save_project_refuses_read_only_file() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = std::env::temp_dir().join(format!("ks-files-{}-perm-fn", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("locked.ksq");
    write_sample(&path);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let before = std::fs::read(&path).unwrap();
    let result = files::save_project(&path, &Project::new());
    assert!(
        matches!(result, Err(files::FileError::PermissionDenied)),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

// ---------- 열기 ----------

#[gpui_kit::test]
fn open_reads_file_into_window_with_title_and_warnings(cx: &mut TestAppContext) {
    let dir = start(cx, "open");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let (handle, base) = open(cx, Project::new());
    touch(cx, handle); // 손댄 창은 바뀌지 않는다
    run(cx, handle, file::Open);
    answer_open(cx, vec![path.clone()]);
    let view = window_for(cx, &path);
    assert_eq!(text_of(cx, &view), ksq::to_string(&sample()));
    let opened = handle_of(cx, &view);
    assert_ne!(opened, handle);
    assert!(title(cx, opened).contains("tune.ksq"));
    assert!(!dirty(cx, &view));
    assert!(is_open(cx, handle));
    assert!(dirty(cx, &base));
}

#[gpui_kit::test]
fn open_several_files_opens_window_each(cx: &mut TestAppContext) {
    let dir = start(cx, "open-many");
    let a = dir.join("a.ksq");
    let b = dir.join("b.ksqz");
    write_sample(&a);
    std::fs::write(
        &b,
        ksq::to_bytes(&sample(), ksq::Compression::Gzip).unwrap(),
    )
    .unwrap();
    let (handle, _) = open(cx, Project::new());
    touch(cx, handle);
    run(cx, handle, file::Open);
    answer_open(cx, vec![a.clone(), b.clone()]);
    let _ = window_for(cx, &a);
    let _ = window_for(cx, &b);
    assert_eq!(windows(cx).len(), 3);
}

#[gpui_kit::test]
fn open_replaces_pristine_untitled_window(cx: &mut TestAppContext) {
    let dir = start(cx, "open-pristine");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::Open);
    answer_open(cx, vec![path.clone()]);
    let _ = window_for(cx, &path);
    assert!(!is_open(cx, handle), "손대지 않은 새 창이 남았다");
    assert_eq!(windows(cx).len(), 1);
}

#[gpui_kit::test]
fn open_same_path_twice_activates_existing_window(cx: &mut TestAppContext) {
    let dir = start(cx, "open-twice");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let (first, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let (other, _) = open(cx, Project::new());
    touch(cx, other);
    let (again, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    cx.run_until_parked();
    assert_eq!(again, first);
    assert_eq!(windows(cx).len(), 2);
    // 다른 이름(상대 경로, `..`)으로 가리켜도 같은 파일이다.
    let roundabout = dir.join("sub/../tune.ksq");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let (third, _) = cx.update(|cx| files::open_path(&roundabout, cx)).unwrap();
    assert_eq!(third, first);
    assert_eq!(windows(cx).len(), 2);
}

#[gpui_kit::test]
fn open_unreadable_file_shows_error_sheet(cx: &mut TestAppContext) {
    let dir = start(cx, "open-bad");
    let path = dir.join("broken.ksq");
    std::fs::write(&path, "{ not json").unwrap();
    let (handle, _) = open(cx, Project::new());
    touch(cx, handle);
    run(cx, handle, file::Open);
    answer_open(cx, vec![path.clone()]);
    assert!(
        prompt_text(cx).contains("열지 못했다"),
        "{}",
        prompt_text(cx)
    );
    answer(cx, "확인");
    assert_eq!(windows(cx).len(), 1);
    assert!(is_open(cx, handle));
}

#[gpui_kit::test]
fn open_recent_missing_file_without_window_shows_error_in_new_window(cx: &mut TestAppContext) {
    let dir = start(cx, "recent-missing");
    let path = dir.join("gone.ksq");
    write_sample(&path);
    let (handle, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    std::fs::remove_file(&path).unwrap();
    assert!(windows(cx).is_empty());
    cx.update(|cx| cx.dispatch_action(&file::OpenRecent(path.display().to_string())));
    cx.run_until_parked();
    assert_eq!(windows(cx).len(), 1, "시트를 띄울 창이 없다");
    assert!(
        prompt_text(cx).contains("열지 못했다"),
        "{}",
        prompt_text(cx)
    );
    answer(cx, "확인");
    assert!(
        cx.update(|cx| files::recent_files(cx)).is_empty(),
        "지워진 파일이 목록에 남았다"
    );
}

// ---------- 닫기 ----------

#[gpui_kit::test]
fn close_clean_window_closes_without_sheet(cx: &mut TestAppContext) {
    start(cx, "close-clean");
    let (handle, _) = open(cx, sample());
    assert!(simulate_close(cx, handle));
    assert!(!cx.has_pending_prompt());
}

#[gpui_kit::test]
fn close_dirty_window_cancel_keeps_window(cx: &mut TestAppContext) {
    start(cx, "close-cancel");
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    assert!(!simulate_close(cx, handle));
    assert!(prompt_text(cx).contains("제목 없음"), "{}", prompt_text(cx));
    answer(cx, "취소");
    assert!(is_open(cx, handle));
    assert!(dirty(cx, &view));
}

#[gpui_kit::test]
fn close_dirty_window_dont_save_closes(cx: &mut TestAppContext) {
    let dir = start(cx, "close-dont-save");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let before = std::fs::read(&path).unwrap();
    let (handle, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    touch(cx, handle);
    assert!(!simulate_close(cx, handle));
    assert!(prompt_text(cx).contains("a.ksq"), "{}", prompt_text(cx));
    answer(cx, "저장 안 함");
    assert!(!is_open(cx, handle));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[gpui_kit::test]
fn close_dirty_window_save_writes_then_closes(cx: &mut TestAppContext) {
    let dir = start(cx, "close-save");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, view) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    touch(cx, handle);
    let expected = text_of(cx, &view);
    assert!(!simulate_close(cx, handle));
    answer(cx, "저장");
    assert!(!is_open(cx, handle));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
}

#[gpui_kit::test]
fn close_dirty_untitled_save_cancelled_keeps_window(cx: &mut TestAppContext) {
    let dir = start(cx, "close-untitled");
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    assert!(!simulate_close(cx, handle));
    answer(cx, "저장");
    answer_new_path(cx, None);
    assert!(is_open(cx, handle), "대화상자를 취소했는데 창이 닫혔다");
    assert!(dirty(cx, &view));
    // 다시 닫기 → 저장 → 경로를 고르면 쓰고 닫는다.
    let expected = text_of(cx, &view);
    assert!(!simulate_close(cx, handle));
    answer(cx, "저장");
    let path = dir.join("new.ksq");
    answer_new_path(cx, Some(path.clone()));
    assert!(!is_open(cx, handle));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
}

#[gpui_kit::test]
fn close_action_uses_same_sheet(cx: &mut TestAppContext) {
    start(cx, "close-action");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, window_actions::Close);
    assert!(is_open(cx, handle));
    answer(cx, "저장 안 함");
    assert!(!is_open(cx, handle));
    let (clean, _) = open(cx, sample());
    run(cx, clean, window_actions::Close);
    assert!(!cx.has_pending_prompt());
    assert!(!is_open(cx, clean));
}

#[gpui_kit::test]
fn second_close_while_sheet_open_does_not_stack_sheets(cx: &mut TestAppContext) {
    start(cx, "close-twice");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    assert!(!simulate_close(cx, handle));
    run(cx, handle, window_actions::Close);
    answer(cx, "취소");
    assert!(!cx.has_pending_prompt(), "시트가 두 번 떴다");
    assert!(is_open(cx, handle));
}

// ---------- 끝내기 ----------

#[gpui_kit::test]
fn quit_prompts_each_dirty_window_and_cancel_aborts(cx: &mut TestAppContext) {
    start(cx, "quit-cancel");
    let (first, _) = open(cx, sample());
    touch(cx, first);
    let (second, _) = open(cx, sample());
    touch(cx, second);
    let (clean, _) = open(cx, sample());
    let quit = start_quit(cx);
    answer(cx, "저장 안 함");
    answer(cx, "취소");
    assert_eq!(quit.get(), Some(false));
    let open_count = [first, second, clean]
        .into_iter()
        .filter(|h| is_open(cx, *h))
        .count();
    assert_eq!(open_count, 2, "취소한 창과 손대지 않은 창은 남는다");
}

#[gpui_kit::test]
fn quit_after_all_answered_resolves_true(cx: &mut TestAppContext) {
    start(cx, "quit-ok");
    let (first, _) = open(cx, sample());
    touch(cx, first);
    let (_clean, _) = open(cx, sample());
    let quit = start_quit(cx);
    answer(cx, "저장 안 함");
    assert_eq!(quit.get(), Some(true));
    assert!(!cx.has_pending_prompt());
}

#[gpui_kit::test]
fn quit_action_with_dirty_window_asks_first(cx: &mut TestAppContext) {
    start(cx, "quit-action");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, app::Quit);
    assert!(cx.has_pending_prompt(), "바뀐 창이 있는데 묻지 않았다");
    answer(cx, "취소");
    assert!(is_open(cx, handle));
}

// ---------- 최근 파일 ----------

#[gpui_kit::test]
fn recent_files_are_recorded_deduped_and_capped(cx: &mut TestAppContext) {
    let dir = start(cx, "recent");
    let mut paths = Vec::new();
    for i in 0..(MAX_RECENT_FILES + 2) {
        let p = dir.join(format!("f{i}.ksq"));
        write_sample(&p);
        paths.push(p);
    }
    for p in &paths {
        cx.update(|cx| files::open_path(p, cx)).unwrap();
    }
    cx.update(|cx| files::open_path(&paths[3], cx)).unwrap();
    let recent = cx.update(|cx| files::recent_files(cx));
    assert_eq!(recent.len(), MAX_RECENT_FILES);
    assert_eq!(recent[0], paths[3].canonicalize().unwrap());
    assert_eq!(
        recent[1],
        paths[MAX_RECENT_FILES + 1].canonicalize().unwrap()
    );
    assert_eq!(
        recent.iter().filter(|p| **p == recent[0]).count(),
        1,
        "중복"
    );
    // 다른 이름으로 저장도 앞에 넣고, 설정 파일에 남는다.
    let (handle, _) = open(cx, sample());
    run(cx, handle, file::SaveAs);
    let saved = dir.join("saved.ksq");
    answer_new_path(cx, Some(saved.clone()));
    let recent = cx.update(|cx| files::recent_files(cx));
    assert_eq!(recent[0], saved.canonicalize().unwrap());
    cx.update(keyscore_app::app_state::AppSettings::flush);
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        settings["recent_files"][0],
        saved.canonicalize().unwrap().display().to_string()
    );
}

#[gpui_kit::test]
fn recent_files_menu_and_palette_list_entries(cx: &mut TestAppContext) {
    let dir = start(cx, "recent-menu");
    std::fs::create_dir_all(dir.join("x")).unwrap();
    std::fs::create_dir_all(dir.join("y")).unwrap();
    let a = dir.join("x/song.ksq");
    let b = dir.join("y/song.ksq");
    let c = dir.join("solo.ksq");
    for p in [&a, &b, &c] {
        write_sample(p);
        cx.update(|cx| files::open_path(p, cx)).unwrap();
    }
    let menus = cx.update(|cx| keyscore_app::commands::menus(cx));
    let file_menu = menus.iter().find(|m| m.name == "파일").expect("파일 메뉴");
    let recent = file_menu
        .items
        .iter()
        .find_map(|item| match item {
            gpui_kit::MenuItem::Submenu(menu) if menu.name == "최근 파일" => Some(menu),
            _ => None,
        })
        .expect("최근 파일 하위 메뉴");
    assert!(!recent.disabled);
    let names: Vec<String> = recent
        .items
        .iter()
        .filter_map(|item| match item {
            gpui_kit::MenuItem::Action { name, .. } => Some(name.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(names.len(), 3, "{names:?}");
    assert_eq!(names[0], "solo.ksq");
    assert!(
        names[1].contains("song.ksq") && names[1].contains('y'),
        "{names:?}"
    );
    assert!(
        names[2].contains("song.ksq") && names[2].contains('x'),
        "{names:?}"
    );
    let palette = cx.update(|cx| keyscore_app::commands::palette_entries(cx));
    let labels: Vec<String> = palette
        .iter()
        .filter(|e| e.name() == "file::OpenRecent")
        .map(|e| e.label().to_string())
        .collect();
    assert_eq!(labels.len(), 3, "{labels:?}");
    assert!(labels.iter().any(|l| l.contains("solo.ksq")));
    assert!(
        palette
            .iter()
            .filter(|e| e.name() == "file::OpenRecent")
            .all(|e| e.action().is_some()),
        "실행할 수 없는 최근 파일 줄"
    );
}

#[gpui_kit::test]
fn open_recent_action_opens_file(cx: &mut TestAppContext) {
    let dir = start(cx, "recent-open");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let (handle, _) = open(cx, Project::new());
    touch(cx, handle);
    run(cx, handle, file::OpenRecent(path.display().to_string()));
    let view = window_for(cx, &path);
    assert_eq!(text_of(cx, &view), ksq::to_string(&sample()));
}
