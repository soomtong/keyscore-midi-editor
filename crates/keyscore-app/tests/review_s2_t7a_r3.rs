//! S2-T7a 적대적 리뷰 3회차(`docs/reports/s2-t7a/review-3.md`). 깨뜨리지 못한 시도만 남긴다. 깨뜨린 시도는
//! 리뷰 문서 부록에 있다.

use std::path::{Path, PathBuf};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{file, track, window as window_actions};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{files, init, open_project_window};
use keyscore_core::{Project, ksq};

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-r3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    activate(cx, handle);
    (handle, view)
}

fn open_file(cx: &mut TestAppContext, path: &Path) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| files::open_path(path, cx)).unwrap();
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

fn doc_path(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<PathBuf> {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .path()
            .map(Path::to_path_buf)
    })
}

fn is_open(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update(|cx| cx.windows().contains(&handle))
}

fn window_count(cx: &mut TestAppContext) -> usize {
    cx.update(|cx| cx.windows().len())
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

/// 바깥 프로그램의 원자적 다시 쓰기: 임시 파일에 쓰고 rename한다(새 inode).
fn rewrite_atomically(path: &Path, bytes: &[u8]) {
    let tmp = path.with_extension("tmp-rewrite");
    std::fs::write(&tmp, bytes).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

const NEWER: &str = r#"{"format_version": 7, "future": {"x": 1}, "tracks": []}"#;

fn open_newer(
    cx: &mut TestAppContext,
    path: &Path,
) -> (AnyWindowHandle, Entity<ProjectWindow>, Vec<u8>) {
    std::fs::write(path, NEWER).unwrap();
    let before = std::fs::read(path).unwrap();
    let (handle, view) = open_file(cx, path);
    touch(cx, handle);
    (handle, view, before)
}

/// 흐름이 풀렸는가: 다른 이름 저장을 다시 내면 새 대화상자가 뜬다(취소한다).
fn assert_flow_cleared(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    assert!(!cx.did_prompt_for_new_path(), "이미 대화상자가 떠 있다");
    run(cx, handle, file::SaveAs);
    assert!(cx.did_prompt_for_new_path(), "흐름이 풀리지 않았다");
    answer_new_path(cx, None);
}

fn broken(dir: &Path) -> PathBuf {
    let bad = dir.join("broken.ksq");
    std::fs::write(&bad, "{ not json").unwrap();
    bad
}

fn choose_open(cx: &mut TestAppContext, handle: AnyWindowHandle, paths: Vec<PathBuf>) {
    run(cx, handle, file::Open);
    cx.simulate_path_prompt_response(move |_| Some(paths));
    cx.run_until_parked();
}

// ---------- 파일 정체(FileId)의 갱신 ----------

/// 저장(rename으로 새 inode) 뒤 바깥에서 이름을 바꿔도, 새 이름으로 열면 그 창을 앞으로 가져온다.
#[gpui_kit::test]
fn review_r3_rename_after_save_activates_existing_window(cx: &mut TestAppContext) {
    let dir = start(cx, "rename-after-save");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    run(cx, handle, file::Save);
    assert!(!dirty(cx, &view));
    let renamed = dir.join("b.ksq");
    std::fs::rename(&path, &renamed).unwrap();
    let (again, _) = cx.update(|cx| files::open_path(&renamed, cx)).unwrap();
    assert_eq!(again, handle);
}

/// 다른 이름으로 저장한 뒤 옛 파일은 이 창의 파일이 아니다. 열면 새 창이다.
#[gpui_kit::test]
fn review_r3_old_file_after_save_as_opens_in_new_window(cx: &mut TestAppContext) {
    let dir = start(cx, "old-after-save-as");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    let copy = dir.join("copy.ksq");
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(copy.clone()));
    assert_eq!(doc_path(cx, &view), Some(copy.canonicalize().unwrap()));
    let (other, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    assert_ne!(other, handle, "옛 파일이 새 파일의 창으로 묶였다");
}

/// 다른 이름으로 저장한 뒤 다른 창은 옛 파일 위에 다른 이름으로 저장할 수 있다(옛 정체가 남아 막지 않는다).
#[gpui_kit::test]
fn review_r3_other_window_may_save_onto_old_file_after_save_as(cx: &mut TestAppContext) {
    let dir = start(cx, "other-onto-old");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (first, _) = open_file(cx, &path);
    touch(cx, first);
    run(cx, first, file::SaveAs);
    answer_new_path(cx, Some(dir.join("moved-on.ksq")));
    let (second, second_view) = open(cx, sample());
    touch(cx, second);
    run(cx, second, file::SaveAs);
    answer_new_path(cx, Some(path.clone()));
    assert!(!cx.has_pending_prompt(), "옛 정체가 남아 저장을 막았다");
    assert_eq!(
        doc_path(cx, &second_view),
        Some(path.canonicalize().unwrap())
    );
}

/// 내용 복사본(새 inode)은 원본 창으로 묶이지 않는다.
#[gpui_kit::test]
fn review_r3_copy_made_after_open_opens_separately(cx: &mut TestAppContext) {
    let dir = start(cx, "copy-after-open");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, _) = open_file(cx, &path);
    let copy = dir.join("copy.ksq");
    std::fs::copy(&path, &copy).unwrap();
    let (other, _) = cx.update(|cx| files::open_path(&copy, cx)).unwrap();
    assert_ne!(other, handle);
}

/// 바깥 프로그램이 연 파일을 원자적으로 다시 써도(새 inode) 같은 경로로 열면 그 창이다.
#[gpui_kit::test]
fn review_r3_atomic_rewrite_same_path_still_activates(cx: &mut TestAppContext) {
    let dir = start(cx, "atomic-same-path");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, _) = open_file(cx, &path);
    rewrite_atomically(&path, ksq::to_string(&sample()).as_bytes());
    let (again, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    assert_eq!(again, handle);
}

// ---------- 상위 버전 원본(결정 2026-10-12 A) ----------

/// 원본을 다른 폴더로 옮긴 뒤 닫기 [저장] → [다른 이름으로 저장] → 옮긴 원본: 쓰지 않고 창·바뀜이 남는다.
#[gpui_kit::test]
fn review_r3_close_save_onto_newer_original_moved_to_other_folder_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-moved-folder");
    let path = dir.join("future.ksq");
    let (handle, view, before) = open_newer(cx, &path);
    std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
    let moved = dir.join("elsewhere/renamed.ksq");
    std::fs::rename(&path, &moved).unwrap();
    run(cx, handle, window_actions::Close);
    answer(cx, "저장");
    answer(cx, "다른 이름으로 저장");
    answer_new_path(cx, Some(moved.clone()));
    assert!(prompt_text(cx).contains("덮어쓸 수 없다"));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&moved).unwrap(), before);
    assert!(is_open(cx, handle));
    assert!(dirty(cx, &view));
    assert_flow_cleared(cx, handle);
}

/// 옮긴 원본에 하드 링크를 만들고 그 이름을 골라도 막힌다.
#[gpui_kit::test]
fn review_r3_save_as_onto_hard_link_of_moved_newer_original_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-moved-link");
    let path = dir.join("future.ksq");
    let (handle, _, before) = open_newer(cx, &path);
    let moved = dir.join("moved.ksq");
    std::fs::rename(&path, &moved).unwrap();
    let link = dir.join("link.ksq");
    std::fs::hard_link(&moved, &link).unwrap();
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(link.clone()));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&moved).unwrap(), before);
}

/// 원본을 옮긴 뒤 옮긴 원본을 가리키는 심볼릭 링크를 골라도 막히고 링크도 그대로다.
#[gpui_kit::test]
fn review_r3_save_as_via_symlink_to_moved_newer_original_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-moved-symlink");
    let path = dir.join("future.ksq");
    let (handle, _, before) = open_newer(cx, &path);
    let moved = dir.join("moved.ksq");
    std::fs::rename(&path, &moved).unwrap();
    let link = dir.join("alias.ksq");
    std::os::unix::fs::symlink(&moved, &link).unwrap();
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(link.clone()));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&moved).unwrap(), before);
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

/// 바깥에서 원본을 원자적으로 다시 쓴 뒤(새 inode) 원래 경로를 골라도 막힌다(경로로 막힌다).
#[gpui_kit::test]
fn review_r3_save_as_onto_newer_path_after_external_rewrite_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-rewritten");
    let path = dir.join("future.ksq");
    let (handle, _, _) = open_newer(cx, &path);
    let newer_again = br#"{"format_version": 8, "tracks": []}"#;
    rewrite_atomically(&path, newer_again);
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(path.clone()));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&path).unwrap(), newer_again);
}

/// 상위 버전 원본을 연 창을 다른 이름으로 저장한 뒤에는 원본이 더 이상 이 문서의 파일이 아니지만, 원본은 여전히
/// 다른 창에서만 열 수 있고 이 창에서 저장은 새 파일에만 간다.
#[gpui_kit::test]
fn review_r3_newer_after_save_as_original_untouched_by_save(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-after-save-as");
    let path = dir.join("future.ksq");
    let (handle, view, before) = open_newer(cx, &path);
    let target = dir.join("mine.ksq");
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(target.clone()));
    touch(cx, handle);
    run(cx, handle, file::Save);
    assert!(!dirty(cx, &view));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(doc_path(cx, &view), Some(target.canonicalize().unwrap()));
}

// ---------- 열기 실패 시트(리뷰 2 지적 3 수정) ----------

/// 바뀐 창에서 열기 실패 시트: 그 창의 흐름이 서고, 그동안 저장·닫기는 새 시트를 띄우지 않으며, 답하면 풀린다.
#[gpui_kit::test]
fn review_r3_open_error_on_origin_blocks_then_releases(cx: &mut TestAppContext) {
    let dir = start(cx, "error-origin");
    let bad = broken(&dir);
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    choose_open(cx, handle, vec![bad]);
    assert!(prompt_text(cx).contains("열지 못했다"));
    run(cx, handle, file::SaveAs);
    assert!(
        !cx.did_prompt_for_new_path(),
        "실패 시트 위에 저장 대화상자"
    );
    run(cx, handle, window_actions::Close);
    answer(cx, "확인");
    assert!(!cx.has_pending_prompt(), "실패 시트 뒤에 시트가 남았다");
    assert!(is_open(cx, handle));
    assert!(dirty(cx, &view));
    assert_flow_cleared(cx, handle);
    run(cx, handle, window_actions::Close);
    assert!(prompt_text(cx).contains("저장하겠는가"));
    answer(cx, "취소");
}

/// 명령을 낸 창에 닫기 시트가 떠 있으면 실패 시트는 새 창에 뜨고, 두 시트 모두 답하면 두 창의 흐름이 풀린다.
#[gpui_kit::test]
fn review_r3_open_error_while_origin_has_close_sheet_uses_new_window(cx: &mut TestAppContext) {
    let dir = start(cx, "error-new-window");
    let bad = broken(&dir);
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, window_actions::Close);
    assert!(prompt_text(cx).contains("저장하겠는가"));
    let before = window_count(cx);
    choose_open(cx, handle, vec![bad]);
    assert_eq!(
        window_count(cx),
        before + 1,
        "실패 시트를 띄울 새 창이 없다"
    );
    let host = cx
        .update(|cx| cx.windows())
        .into_iter()
        .find(|h| *h != handle)
        .unwrap();
    answer(cx, "취소");
    answer(cx, "확인");
    assert!(!cx.has_pending_prompt());
    assert!(is_open(cx, handle));
    assert!(dirty(cx, &view));
    assert_flow_cleared(cx, handle);
    assert_flow_cleared(cx, host);
    // 새 창은 손대지 않은 창이라 묻지 않고 닫힌다.
    run(cx, host, window_actions::Close);
    assert!(!cx.has_pending_prompt());
    assert!(!is_open(cx, host));
}

/// 명령을 낸 창이 바쁘고 다른 창이 한가하면 그 창에 띄운다(새 창을 만들지 않는다).
#[gpui_kit::test]
fn review_r3_open_error_prefers_other_free_window(cx: &mut TestAppContext) {
    let dir = start(cx, "error-other-window");
    let bad = broken(&dir);
    let (other, other_view) = open(cx, sample());
    touch(cx, other);
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, window_actions::Close);
    let before = window_count(cx);
    choose_open(cx, handle, vec![bad]);
    assert_eq!(window_count(cx), before);
    // 실패 시트가 뜬 창(other)에 닫기를 내도 겹치지 않는다.
    run(cx, other, window_actions::Close);
    answer(cx, "취소");
    answer(cx, "확인");
    assert!(!cx.has_pending_prompt(), "다른 창에 시트가 겹쳤다");
    assert!(dirty(cx, &other_view));
    assert_flow_cleared(cx, other);
    assert_flow_cleared(cx, handle);
}

/// 여러 파일 중 일부만 실패: 손대지 않은 명령 창은 닫히고 실패 시트는 연 창에 뜨며, 답하면 그 창의 흐름이 풀린다.
#[gpui_kit::test]
fn review_r3_partial_open_failure_hosts_on_opened_window(cx: &mut TestAppContext) {
    let dir = start(cx, "error-partial");
    let bad = broken(&dir);
    let good = dir.join("good.ksq");
    write_sample(&good);
    let (origin, _) = open(cx, Project::new());
    choose_open(cx, origin, vec![good.clone(), bad]);
    assert!(!is_open(cx, origin));
    assert!(prompt_text(cx).contains("열지 못했다"));
    let windows = cx.update(|cx| cx.windows());
    assert_eq!(windows.len(), 1);
    answer(cx, "확인");
    assert_flow_cleared(cx, windows[0]);
}

/// 실패 시트가 떠 있는 깨끗한 창이 있어도 끝내기는 다른 바뀐 창에 묻고 끝낼 수 있다(묻지 않고 잃는 창 없음).
#[gpui_kit::test]
fn review_r3_quit_with_error_sheet_on_clean_window_asks_dirty_one(cx: &mut TestAppContext) {
    let dir = start(cx, "error-quit-clean");
    let bad = broken(&dir);
    let (dirty_handle, dirty_view) = open(cx, sample());
    touch(cx, dirty_handle);
    let (clean, _) = open(cx, Project::new());
    choose_open(cx, clean, vec![bad]);
    let quit = start_quit(cx);
    // FIFO: 실패 시트가 먼저다.
    answer(cx, "확인");
    assert!(prompt_text(cx).contains("저장하겠는가"));
    answer(cx, "취소");
    assert_eq!(quit.get(), Some(false));
    assert!(dirty(cx, &dirty_view));
    assert!(is_open(cx, dirty_handle));
}

/// 실패 시트가 떠 있는 바뀐 창이 있으면 끝내기는 묻지 않고 멈춘다(끝내지 않는다). 시트에 답한 뒤 다시 끝내면 묻는다.
#[gpui_kit::test]
fn review_r3_quit_with_error_sheet_on_dirty_window_aborts_then_asks(cx: &mut TestAppContext) {
    let dir = start(cx, "error-quit-dirty");
    let bad = broken(&dir);
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    choose_open(cx, handle, vec![bad]);
    let quit = start_quit(cx);
    assert_eq!(quit.get(), Some(false));
    answer(cx, "확인");
    assert!(!cx.has_pending_prompt());
    let quit = start_quit(cx);
    assert!(prompt_text(cx).contains("저장하겠는가"));
    answer(cx, "취소");
    assert_eq!(quit.get(), Some(false));
    assert!(dirty(cx, &view));
}

// ---------- 최근 파일 ----------

/// 없는 파일과 있는 파일은 합치지 않는다.
#[gpui_kit::test]
fn review_r3_recent_missing_and_existing_not_merged(cx: &mut TestAppContext) {
    let dir = dir_for("recent-missing");
    let exists = dir.join("b.ksq");
    write_sample(&exists);
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({ "recent_files": [
            dir.join("gone.ksq").display().to_string(),
            exists.display().to_string()
        ] })
        .to_string(),
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let recent = cx.read(files::recent_files);
    assert_eq!(recent.len(), 2);
}

// ---------- 이미 바쁜 창에 열린 파일 ----------

/// 열려는 파일이 이미 바쁜 창(닫기 시트)에 열려 있고 명령 창이 손대지 않은 창이면, 명령 창은 닫히고 실패 시트는
/// 다른 곳(새 창)에 뜨고 답하면 흐름이 풀린다.
#[gpui_kit::test]
fn review_r3_open_error_when_opened_window_busy_still_shown(cx: &mut TestAppContext) {
    let dir = start(cx, "opened-busy");
    let bad = broken(&dir);
    let good = dir.join("good.ksq");
    write_sample(&good);
    let (busy, busy_view) = open_file(cx, &good);
    touch(cx, busy);
    run(cx, busy, window_actions::Close);
    let (origin, _) = open(cx, Project::new());
    choose_open(cx, origin, vec![good.clone(), bad]);
    answer(cx, "취소");
    assert!(dirty(cx, &busy_view));
    assert!(
        cx.has_pending_prompt() && prompt_text(cx).contains("열지 못했다"),
        "열기 실패를 알리는 시트가 없다"
    );
    answer(cx, "확인");
    assert_flow_cleared(cx, busy);
}
