//! S2-T7b 적대적 리뷰 1회차 시험(자동 저장본과 크래시 후 복구). 깨뜨리지 못한 시도만 둔다. 깨뜨린 재현은
//! `docs/reports/s2-t7b/review-1.md` 부록에 있다.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{file, track};
use keyscore_app::autosave::{self, AUTOSAVE_DELAY};
use keyscore_app::files;
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

// ---------------------------------------------------------------- 도우미(tests/autosave.rs와 같은 모양)

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t7b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
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
    cx.update_window(handle, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
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

fn advance(cx: &mut TestAppContext, by: Duration) {
    cx.executor().advance_clock(by);
    cx.run_until_parked();
}

fn text_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| ksq::to_string(view.read(cx).document().read(cx).project()))
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
}

fn autosave_dir(cx: &mut TestAppContext) -> PathBuf {
    cx.read(autosave::autosave_dir)
}

fn autosaves(cx: &mut TestAppContext) -> Vec<PathBuf> {
    let dir = autosave_dir(cx);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ksq"))
        .collect();
    out.sort();
    out
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

/// 끝난 세션(잠금 파일 없음)의 저장본.
fn plant_dead(dir: &Path, key: &str, content: &str, path: Option<&Path>, pid: u32) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{key}.ksq")), content).unwrap();
    let meta = serde_json::json!({
        "path": path.map(|p| p.display().to_string()),
        "read_only": null,
        "session": format!("dead-{key}"),
        "pid": pid,
    });
    std::fs::write(dir.join(format!("{key}.json")), meta.to_string()).unwrap();
}

fn answer(cx: &mut TestAppContext, button: &str) {
    assert!(cx.has_pending_prompt(), "시트가 없다");
    cx.simulate_prompt_answer(button);
    cx.run_until_parked();
}

fn windows(cx: &mut TestAppContext) -> Vec<Entity<ProjectWindow>> {
    let open = cx.update(|cx| cx.windows());
    cx.update(|cx| open_project_windows(cx))
        .into_iter()
        .filter(|w| open.contains(&cx.read(|cx| w.read(cx).window_handle())))
        .collect()
}

fn handle_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> AnyWindowHandle {
    cx.read(|cx| view.read(cx).window_handle())
}

fn launch_with_recovery(cx: &mut TestAppContext) -> AnyWindowHandle {
    let (handle, _) = open(cx, Project::new());
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    handle
}

fn edited() -> String {
    r#"{"tracks": [{"name": "Recovered", "channel": 2}]}"#.to_owned()
}

/// 앱의 끝내기 훅이 하는 일 가운데 잠금 파일 지우기를 흉내 낸다(정상 종료·Dock 종료 뒤의 상태).
fn release_session_locks(dir: &Path) {
    for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().starts_with("session-") {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
}

// ---------------------------------------------------------------- 깨뜨리지 못한 시도

/// 명시 저장이 실패하면(쓸 권한 없음 → 취소) 저장본은 남는다.
#[gpui_kit::test]
fn review_failed_save_keeps_autosave(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = start(cx, "failed-save");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosaves(cx).len(), 1);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    run(cx, handle, file::Save);
    answer(cx, "취소");
    assert!(dirty(cx, &view));
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 1, "저장 실패 뒤 저장본이 사라졌다");
    assert_eq!(
        std::fs::read_to_string(&saved[0]).unwrap(),
        text_of(cx, &view)
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

/// 같은 폴더를 쓰는 살아 있는 다른 실행(두 번째 앱)은 이 실행의 저장본을 복구 대상으로 보지 않는다.
/// 이 실행이 끝나 잠금을 놓으면 복구 대상이 된다(Dock 종료 경로).
#[gpui_kit::test]
fn review_second_live_app_does_not_see_first_apps_autosaves(cx: &mut TestAppContext) {
    let dir = start(cx, "two-apps");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosaves(cx).len(), 1);
    let other = cx.new_app();
    other.update(|cx| init(cx, dir.join("settings.json")));
    assert!(
        other.read(autosave::find_recoverable).is_empty(),
        "살아 있는 실행의 저장본을 복구 대상으로 봤다"
    );
    // 첫 실행이 끝내기 훅으로 잠금을 지웠다고 치면 복구 대상이다(두 앱은 pid가 같아 메타의 세션으로 고른다).
    let auto = autosave_dir(cx);
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(autosaves(cx)[0].with_extension("json")).unwrap(),
    )
    .unwrap();
    let first_lock = auto.join(format!(
        "session-{}.lock",
        meta["session"].as_str().unwrap()
    ));
    std::fs::remove_file(first_lock).unwrap();
    assert_eq!(other.read(autosave::find_recoverable).len(), 1);
}

/// 복구한 새 프로젝트를 다른 이름으로 저장하면 옛 저장본이 남지 않는다.
#[gpui_kit::test]
fn review_recovered_untitled_save_as_leaves_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "recover-save-as");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "untitled-1-2-3", &edited(), None, 999_999);
    launch_with_recovery(cx);
    answer(cx, "복구");
    let restored = windows(cx).pop().unwrap();
    let handle = handle_of(cx, &restored);
    run(cx, handle, file::SaveAs);
    let target = dir.join("named.ksq");
    cx.simulate_new_path_selection(move |_| Some(target.clone()));
    cx.run_until_parked();
    assert!(!dirty(cx, &restored));
    assert!(autosaves(cx).is_empty(), "{:?}", autosaves(cx));
    assert!(cx.read(autosave::find_recoverable).is_empty());
}

/// 복구한 창을 [저장 안 함]으로 닫으면 저장본이 지워져 다음 실행에서 다시 묻지 않는다.
#[gpui_kit::test]
fn review_recovered_window_discarded_not_offered_again(cx: &mut TestAppContext) {
    start(cx, "recover-close");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "untitled-9-9-9", &edited(), None, 999_999);
    launch_with_recovery(cx);
    answer(cx, "복구");
    let restored = windows(cx).pop().unwrap();
    let handle = handle_of(cx, &restored);
    run(cx, handle, keyscore_app::actions::window::Close);
    answer(cx, "저장 안 함");
    advance(cx, AUTOSAVE_DELAY * 2);
    assert!(autosaves(cx).is_empty(), "{:?}", autosaves(cx));
    release_session_locks(&auto);
    assert!(cx.read(autosave::find_recoverable).is_empty());
}

/// 복구한 창이 30 s 뒤 다시 써도 프로젝트 폴더에는 원래 파일 하나뿐이고 원래 파일 바이트·수정 시각은 그대로다.
#[gpui_kit::test]
fn review_recovered_window_never_writes_project_folder(cx: &mut TestAppContext) {
    let dir = start(cx, "recover-folder");
    let project = dir.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let path = project.join("song.ksq");
    write_sample(&path);
    let before = std::fs::read(&path).unwrap();
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited(), Some(&path), 999_999);
    launch_with_recovery(cx);
    answer(cx, "복구");
    let restored = windows(cx).pop().unwrap();
    let handle = handle_of(cx, &restored);
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY * 3);
    cx.update(autosave::flush_all);
    let names: Vec<_> = std::fs::read_dir(&project)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("song.ksq")]);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime);
    // 다시 쓴 저장본은 새 키(경로 해시)로 옮겨 가고 옛 키는 지워진다.
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(
        std::fs::read_to_string(&saved[0]).unwrap(),
        text_of(cx, &restored)
    );
}

/// 같은 파일을 가리키는 두 끝난 저장본(링크 경로와 실제 경로)은 경로를 가진 창 하나와 새 프로젝트 창 하나로 연다.
#[gpui_kit::test]
fn review_two_autosaves_same_file_one_window_with_path(cx: &mut TestAppContext) {
    let dir = start(cx, "same-file-two");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let link = dir.join("link.ksq");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let auto = autosave_dir(cx);
    plant_dead(&auto, "a", &edited(), Some(&path), 999_999);
    plant_dead(&auto, "b", &edited(), Some(&link), 999_999);
    launch_with_recovery(cx);
    answer(cx, "복구");
    let all = windows(cx);
    assert_eq!(all.len(), 2);
    let with_path = all
        .iter()
        .filter(|w| cx.read(|cx| w.read(cx).document().read(cx).path().is_some()))
        .count();
    assert_eq!(with_path, 1, "같은 파일이 두 창에 열렸다");
}

/// [버리기]는 끝난 세션의 저장본만 지운다. 살아 있는 다른 세션의 저장본, 깨진 저장본, 잠금 파일은 남는다.
#[gpui_kit::test]
fn review_discard_touches_only_dead_good_autosaves(cx: &mut TestAppContext) {
    start(cx, "discard-only");
    let auto = autosave_dir(cx);
    std::fs::create_dir_all(&auto).unwrap();
    let lock = std::fs::File::create(auto.join("session-alive.lock")).unwrap();
    lock.try_lock().unwrap();
    std::fs::write(auto.join("live.ksq"), ksq::to_string(&sample())).unwrap();
    std::fs::write(
        auto.join("live.json"),
        serde_json::json!({"path": null, "read_only": null, "session": "alive", "pid": 1})
            .to_string(),
    )
    .unwrap();
    plant_dead(&auto, "dead", &edited(), None, 999_999);
    plant_dead(&auto, "broken", "{ nope", None, 999_999);
    launch_with_recovery(cx);
    answer(cx, "버리기");
    assert!(auto.join("live.ksq").exists());
    assert!(auto.join("live.json").exists());
    assert!(auto.join("broken.ksq").exists());
    assert!(!auto.join("dead.ksq").exists());
    assert!(auto.join("session-alive.lock").exists());
    drop(lock);
}

/// 저장한 뒤 바로 고쳐도 새 30 s 타이머가 걸린다(저장이 타이머를 놓아도 다음 변경이 다시 건다).
#[gpui_kit::test]
fn review_edit_after_save_autosaves_again(cx: &mut TestAppContext) {
    let dir = start(cx, "edit-after-save");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, Duration::from_secs(10));
    run(cx, handle, file::Save);
    advance(cx, Duration::from_secs(5));
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&saved[0]).unwrap(),
        text_of(cx, &view)
    );
}

/// 최근 목록 캐시가 열기·다른 이름 저장으로 바뀐 목록을 바로 반영한다.
#[gpui_kit::test]
fn review_recent_cache_follows_list_changes(cx: &mut TestAppContext) {
    let dir = start(cx, "recent-follow");
    let a = dir.join("a.ksq");
    let b = dir.join("b.ksq");
    write_sample(&a);
    write_sample(&b);
    open_file(cx, &a);
    assert_eq!(cx.read(files::recent_files), vec![a.clone()]);
    let (h, _) = open_file(cx, &b);
    assert_eq!(cx.read(files::recent_files), vec![b.clone(), a.clone()]);
    run(cx, h, file::SaveAs);
    let c = dir.join("c.ksq");
    let target = c.clone();
    cx.simulate_new_path_selection(move |_| Some(target.clone()));
    cx.run_until_parked();
    assert_eq!(cx.read(files::recent_files)[0], c);
    let before = cx.read(files::recent_label_computations);
    let _ = cx.update(|cx| keyscore_app::commands::menus(cx));
    assert_eq!(cx.read(files::recent_label_computations), before);
}

/// 닫기 경로를 같은 방법(시계만 옮기고 한 단계씩)으로 쓰기 도중에 끼워 넣는다.
#[gpui_kit::test]
fn review_close_stepped_into_autosave_write_leaves_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "race-close");
    for k in 0..12 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, _) = open_file(cx, &path);
        touch(cx, handle);
        cx.dispatcher
            .scheduler()
            .clock()
            .advance(AUTOSAVE_DELAY + Duration::from_secs(1));
        for _ in 0..k {
            if !cx.executor().tick() {
                break;
            }
        }
        cx.update_window(handle, |_, window, cx| {
            window.dispatch_action(Box::new(keyscore_app::actions::window::Close), cx);
        })
        .unwrap();
        cx.run_until_parked();
        answer(cx, "저장 안 함");
        advance(cx, AUTOSAVE_DELAY * 2);
        assert!(autosaves(cx).is_empty(), "k={k}: 버린 창의 저장본이 남았다");
    }
}

// ---------- 리뷰 1 부록 A(수정 확인) ----------

// ---------------------------------------------------------------- 부록 A: 깨뜨린 시도

/// 앱의 저장본 키(원래 경로의 FNV-1a 64비트 16진, `autosave.rs`의 `path_key`와 같은 계산).
fn path_key(path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn autosave_texts(cx: &mut TestAppContext) -> Vec<String> {
    autosaves(cx)
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
}

/// A1. 30 s 타이머의 백그라운드 쓰기가 끝난 직후, 그 결과를 받기 전에 `Cmd+S`가 먼저 돌면 저장본이 남는다.
/// 문서는 깨끗한데 저장본이 남고, 정상 종료(잠금 해제) 뒤 다음 실행이 저장한 문서의 복구를 제안한다.
/// 시험 실행기는 한 단계씩(`tick`) 돌려 「쓰기 끝 → 저장 → 결과 받기」 순서를 찾는다.
#[gpui_kit::test]
fn review_a1_save_racing_autosave_write_leaves_saved_doc_recoverable(cx: &mut TestAppContext) {
    let dir = start(cx, "race-save");
    let mut leaked = None;
    for k in 0..40 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, view) = open_file(cx, &path);
        touch(cx, handle);
        // `advance_clock`은 진행하며 모든 작업을 돌린다. 시계만 옮기고 한 단계씩 돌린다.
        cx.dispatcher
            .scheduler()
            .clock()
            .advance(AUTOSAVE_DELAY + Duration::from_secs(1));
        for _ in 0..k {
            if !cx.executor().tick() {
                break;
            }
        }
        cx.update_window(handle, |_, window, cx| {
            window.dispatch_action(Box::new(file::Save), cx);
        })
        .unwrap();
        cx.run_until_parked();
        advance(cx, AUTOSAVE_DELAY * 2);
        assert!(!dirty(cx, &view));
        let key = path_key(&path);
        if autosave_dir(cx).join(format!("{key}.ksq")).exists() {
            leaked = Some(path);
            break;
        }
    }
    let Some(path) = leaked else {
        return;
    };
    // 정상 종료 뒤 다음 실행.
    let auto = autosave_dir(cx);
    release_session_locks(&auto);
    let next = cx.new_app();
    next.update(|cx| init(cx, dir.join("settings.json")));
    let found = next.read(autosave::find_recoverable);
    panic!(
        "저장한 {}의 저장본이 남아 다음 실행이 복구를 제안한다: {:?}",
        path.display(),
        found
            .iter()
            .map(|r| r.path().map(Path::to_path_buf))
            .collect::<Vec<_>>()
    );
}

/// A2. 파일 F를 연 창이 있을 때 F의 끝난 저장본을 복구하면, 경로 없이 연 복구 창이 F의 키(경로 해시)를 물려받는다.
/// F 창이 30 s 뒤 저장본을 쓰면 복구 창의 저장본을 덮고, F 창을 저장하면 그 키를 지운다. 복구 창은 바뀐 채
/// 저장본이 없다(두 번째 크래시에서 복구한 작업을 잃는다).
#[gpui_kit::test]
fn review_a2_recovered_untitled_shares_key_with_open_file_window(cx: &mut TestAppContext) {
    let dir = start(cx, "shared-key");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (file_handle, _) = open_file(cx, &path);
    let auto = autosave_dir(cx);
    plant_dead(&auto, &path_key(&path), &edited(), Some(&path), 999_999);
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    answer(cx, "복구");
    let recovered = windows(cx)
        .into_iter()
        .find(|w| cx.read(|cx| w.read(cx).document().read(cx).path().is_none()))
        .expect("경로 없이 연 복구 창");
    let recovered_text = text_of(cx, &recovered);
    touch(cx, file_handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert!(
        autosave_texts(cx).contains(&recovered_text),
        "복구 창의 저장본이 F 창의 저장본에 덮였다"
    );
    run(cx, file_handle, file::Save);
    assert!(dirty(cx, &recovered));
    assert!(
        autosave_texts(cx).contains(&recovered_text),
        "F 창을 저장하자 바뀐 복구 창의 저장본이 지워졌다"
    );
}

/// A3. 복구 시트가 떠 있는 동안 F를 열어 고치면(30 s 뒤 저장본이 생김) [버리기]가 그 살아 있는 저장본을 지운다.
#[gpui_kit::test]
fn review_a3_discard_deletes_live_autosave_of_file_opened_during_sheet(cx: &mut TestAppContext) {
    let dir = start(cx, "discard-live");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let auto = autosave_dir(cx);
    plant_dead(&auto, &path_key(&path), &edited(), Some(&path), 999_999);
    launch_with_recovery(cx);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let live = text_of(cx, &view);
    assert!(autosave_texts(cx).contains(&live));
    answer(cx, "버리기");
    assert!(dirty(cx, &view));
    assert!(
        autosave_texts(cx).contains(&live),
        "[버리기]가 이 실행에서 고친 창의 저장본을 지웠다"
    );
}

/// A4. 같은 파일을 연 살아 있는 다른 실행의 저장본(같은 경로 해시 키)을 이 실행이 덮어쓰고, 창을 닫을 때 지운다.
#[gpui_kit::test]
fn review_a4_live_other_session_autosave_of_same_file_overwritten(cx: &mut TestAppContext) {
    let dir = start(cx, "other-live");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let auto = autosave_dir(cx);
    std::fs::create_dir_all(&auto).unwrap();
    let lock = std::fs::File::create(auto.join("session-other.lock")).unwrap();
    lock.try_lock().unwrap();
    let key = path_key(&path);
    std::fs::write(auto.join(format!("{key}.ksq")), edited()).unwrap();
    std::fs::write(
        auto.join(format!("{key}.json")),
        serde_json::json!({"path": path.display().to_string(), "read_only": null,
            "session": "other", "pid": 1})
        .to_string(),
    )
    .unwrap();
    let (handle, _) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(auto.join(format!("{key}.json"))).unwrap())
            .unwrap();
    drop(lock);
    assert_eq!(
        meta["session"], "other",
        "살아 있는 다른 실행의 저장본을 덮어썼다"
    );
}

/// A5. 저장본 쓰기가 한 번 실패한 뒤 막힘이 풀려도, 더 고치지 않으면 다시 쓰지 않는다. 바뀐 문서가 저장본 없이
/// 남는다(그 뒤 크래시면 실패 전 30 s 넘는 작업을 잃는다).
#[gpui_kit::test]
fn review_a5_failed_autosave_retried_without_new_edit(cx: &mut TestAppContext) {
    start(cx, "retry");
    let blocker = autosave_dir(cx);
    let _ = std::fs::remove_dir_all(&blocker);
    std::fs::write(&blocker, "not a directory").unwrap();
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    std::fs::remove_file(&blocker).unwrap();
    advance(cx, AUTOSAVE_DELAY * 10);
    assert!(dirty(cx, &view));
    assert_eq!(
        autosaves(cx).len(),
        1,
        "막힘이 풀린 뒤 5분이 지나도 바뀐 문서의 저장본이 없다"
    );
}

/// A6. 끝난 세션의 pid가 지금 살아 있는 다른 프로세스에 다시 쓰였으면, 복구·버리기가 그 프로세스의 임시 파일을 지운다.
#[gpui_kit::test]
fn review_a6_stale_temp_cleanup_hits_reused_live_pid(cx: &mut TestAppContext) {
    let dir = start(cx, "pid-reuse");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let pid = child.id();
    let temp = dir.join(format!(".song.ksq.tmp-{pid}-0"));
    std::fs::write(&temp, "in flight").unwrap();
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited(), Some(&path), pid);
    launch_with_recovery(cx);
    answer(cx, "버리기");
    let kept = temp.exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(kept, "살아 있는 프로세스(pid {pid})의 임시 파일을 지웠다");
}
