//! S2-T7b 자동 저장본과 크래시 후 복구(IO-5, 8.4). 시계는 시험 실행기의 `advance_clock`으로 돌린다.
//! 끝난 세션의 저장본은 모듈 문서(`keyscore_app::autosave`)의 형식대로 시험이 직접 만든다.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{file, track};
use keyscore_app::autosave::{self, AUTOSAVE_DELAY};
use keyscore_app::document::ReadOnly;
use keyscore_app::files;
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-autosave-{}-{name}", std::process::id()));
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

/// 저장본 폴더의 `.ksq` 파일들.
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

fn meta_of(ksq_path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(ksq_path.with_extension("json")).unwrap())
        .unwrap()
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

/// 끝난 세션(잠금 파일 없음)의 저장본을 만든다.
fn plant_dead(dir: &Path, key: &str, content: &str, path: Option<&Path>, read_only: Option<u64>) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{key}.ksq")), content).unwrap();
    let meta = serde_json::json!({
        "path": path.map(|p| p.display().to_string()),
        "read_only": read_only,
        "session": format!("dead-{key}"),
        "pid": 999_999,
    });
    std::fs::write(dir.join(format!("{key}.json")), meta.to_string()).unwrap();
}

fn prompt_text(cx: &mut TestAppContext) -> String {
    let (msg, detail) = cx.pending_prompt().expect("시트가 없다");
    format!("{msg}\n{detail}")
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

fn title(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    let handle = cx.read(|cx| view.read(cx).window_handle());
    gpui_kit::VisualTestContext::from_window(handle, cx)
        .window_title()
        .unwrap_or_default()
}

/// 시험 시작 때 열린 빈 창 하나와 복구 제안(앱의 `run`과 같다).
fn launch_with_recovery(cx: &mut TestAppContext) -> AnyWindowHandle {
    let (handle, _) = open(cx, Project::new());
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    handle
}

// ---------- 자동 저장본 ----------

#[gpui_kit::test]
fn autosave_written_30s_after_change_outside_project_folder(cx: &mut TestAppContext) {
    let dir = start(cx, "written");
    let project_dir = dir.join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let path = project_dir.join("song.ksq");
    write_sample(&path);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    advance(
        cx,
        AUTOSAVE_DELAY.checked_sub(Duration::from_secs(1)).unwrap(),
    );
    assert!(autosaves(cx).is_empty(), "30 s 전에 저장본이 생겼다");
    advance(cx, Duration::from_secs(2));
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert!(saved[0].starts_with(autosave_dir(cx)));
    assert_eq!(
        std::fs::read_to_string(&saved[0]).unwrap(),
        text_of(cx, &view)
    );
    assert_eq!(meta_of(&saved[0])["path"], path.display().to_string());
    // 단계 게이트: 프로젝트 폴더에는 원래 파일 하나뿐이다.
    let in_project: Vec<_> = std::fs::read_dir(&project_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert_eq!(in_project, vec![std::ffi::OsString::from("song.ksq")]);
    assert!(dirty(cx, &view), "자동 저장본은 바뀜을 끄지 않는다");
}

#[gpui_kit::test]
fn clean_document_writes_no_autosave(cx: &mut TestAppContext) {
    start(cx, "clean");
    let (_handle, _view) = open(cx, sample());
    advance(cx, AUTOSAVE_DELAY * 2);
    assert!(autosaves(cx).is_empty());
}

#[gpui_kit::test]
fn continuous_editing_still_autosaves_within_30s(cx: &mut TestAppContext) {
    start(cx, "continuous");
    let (handle, view) = open(cx, sample());
    for _ in 0..31 {
        touch(cx, handle);
        advance(cx, Duration::from_secs(1));
    }
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 1, "계속 고치는 동안 저장본이 없다");
    // 그 뒤 고친 내용도 다음 30 s 안에 들어간다.
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(
        std::fs::read_to_string(&autosaves(cx)[0]).unwrap(),
        text_of(cx, &view)
    );
}

#[gpui_kit::test]
fn explicit_save_removes_autosave(cx: &mut TestAppContext) {
    let dir = start(cx, "save-removes");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (handle, _) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosaves(cx).len(), 1);
    run(cx, handle, file::Save);
    assert!(autosaves(cx).is_empty(), "저장한 뒤 저장본이 남았다");
    assert!(
        !std::fs::read_dir(autosave_dir(cx))
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == "json")),
        "메타가 남았다"
    );
}

#[gpui_kit::test]
fn closing_window_removes_autosave(cx: &mut TestAppContext) {
    start(cx, "close-removes");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosaves(cx).len(), 1);
    run(cx, handle, keyscore_app::actions::window::Close);
    answer(cx, "저장 안 함");
    assert!(autosaves(cx).is_empty(), "닫은 창의 저장본이 남았다");
}

#[gpui_kit::test]
fn untitled_windows_get_separate_autosaves(cx: &mut TestAppContext) {
    start(cx, "untitled");
    let (a, _) = open(cx, sample());
    let (b, _) = open(cx, Project::new());
    touch(cx, a);
    touch(cx, b);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 2, "{saved:?}");
    for s in &saved {
        assert!(meta_of(s)["path"].is_null());
    }
}

#[gpui_kit::test]
fn save_as_moves_autosave_key(cx: &mut TestAppContext) {
    let dir = start(cx, "save-as-key");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosaves(cx).len(), 1);
    run(cx, handle, file::SaveAs);
    cx.simulate_new_path_selection(move |_| Some(dir.join("named.ksq")));
    cx.run_until_parked();
    assert!(autosaves(cx).is_empty(), "새 프로젝트의 저장본이 남았다");
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 1);
    assert!(
        meta_of(&saved[0])["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("named.ksq"))
    );
}

#[gpui_kit::test]
fn autosave_failure_is_reported_once_and_retried(cx: &mut TestAppContext) {
    start(cx, "failure");
    let blocker = autosave_dir(cx);
    let _ = std::fs::remove_dir_all(&blocker);
    std::fs::write(&blocker, "not a directory").unwrap();
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let notices = cx.read(|cx| {
        view.read(cx)
            .messages()
            .iter()
            .filter(|m| m.contains("자동 저장"))
            .count()
    });
    assert_eq!(notices, 1, "실패 알림은 한 번");
    std::fs::remove_file(&blocker).unwrap();
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosaves(cx).len(), 1, "막힘이 풀린 뒤 다시 쓰지 않았다");
}

#[gpui_kit::test]
fn app_quit_hook_flushes_dirty_autosaves(cx: &mut TestAppContext) {
    start(cx, "flush");
    let (handle, view) = open(cx, sample());
    let (_clean, _) = open(cx, sample());
    touch(cx, handle);
    cx.update(autosave::flush_all);
    cx.run_until_parked();
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 1, "타이머를 기다리지 않고 바뀐 창만 써야 한다");
    assert_eq!(
        std::fs::read_to_string(&saved[0]).unwrap(),
        text_of(cx, &view)
    );
}

// ---------- 세션 ----------

#[gpui_kit::test]
fn live_session_autosaves_are_not_recoverable(cx: &mut TestAppContext) {
    start(cx, "live");
    let dir = autosave_dir(cx);
    std::fs::create_dir_all(&dir).unwrap();
    // 다른 실행 중인 KeyScore: 세션 잠금을 쥐고 있다.
    let lock = std::fs::File::create(dir.join("session-alive.lock")).unwrap();
    lock.try_lock().unwrap();
    std::fs::write(dir.join("k1.ksq"), ksq::to_string(&sample())).unwrap();
    std::fs::write(
        dir.join("k1.json"),
        serde_json::json!({"path": null, "read_only": null, "session": "alive", "pid": 1})
            .to_string(),
    )
    .unwrap();
    plant_dead(&dir, "k2", &ksq::to_string(&sample()), None, None);
    let found = cx.read(autosave::find_recoverable);
    assert_eq!(
        found.len(),
        1,
        "살아 있는 세션의 저장본을 복구 대상으로 봤다"
    );
    drop(lock);
    assert_eq!(cx.read(autosave::find_recoverable).len(), 2);
}

#[gpui_kit::test]
fn own_session_autosaves_are_not_recoverable(cx: &mut TestAppContext) {
    start(cx, "own");
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosaves(cx).len(), 1);
    assert!(cx.read(autosave::find_recoverable).is_empty());
}

const CHILD_ENV: &str = "KS_AUTOSAVE_CHILD";

/// 자식 프로세스가 세션 잠금을 쥐고 저장본을 쓴 채 `SIGKILL`로 죽으면, 잠금이 풀려 복구 대상이 된다.
#[test]
fn dead_session_autosaves_are_recoverable() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let dir = PathBuf::from(dir);
        let lock = std::fs::File::create(dir.join("session-child.lock")).unwrap();
        lock.try_lock().unwrap();
        std::fs::write(dir.join("c.ksq"), ksq::to_string(&sample())).unwrap();
        std::fs::write(
            dir.join("c.json"),
            serde_json::json!({"path": null, "read_only": null, "session": "child",
                "pid": std::process::id()})
            .to_string(),
        )
        .unwrap();
        std::fs::write(dir.join("ready"), "").unwrap();
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let base = dir_for("dead-child");
    let dir = base.join("autosave");
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "dead_session_autosaves_are_recoverable",
            "--nocapture",
        ])
        .env(CHILD_ENV, &dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !dir.join("ready").exists() {
        assert!(Instant::now() < deadline, "자식이 준비되지 않았다");
        std::thread::sleep(Duration::from_millis(10));
    }
    let probe = std::fs::File::open(dir.join("session-child.lock")).unwrap();
    assert!(probe.try_lock().is_err(), "살아 있는 자식의 잠금을 잡았다");
    drop(probe);
    child.kill().unwrap();
    child.wait().unwrap();
    let probe = std::fs::File::open(dir.join("session-child.lock")).unwrap();
    assert!(probe.try_lock().is_ok(), "죽은 자식의 잠금이 풀리지 않았다");
    // 시험이 쥔 잠금을 놓는다. 쥔 채면 앱 판정도 「살아 있음」이 된다(같은 프로세스라도 열린 파일마다 잠금).
    drop(probe);
    // 앱 판정도 같다.
    let cx = TestAppContext::single();
    cx.update(|cx| init(cx, base.join("settings.json")));
    let found = cx.read(autosave::find_recoverable);
    assert_eq!(found.len(), 1);
    assert!(found[0].path().is_none());
}

// ---------- 복구 ----------

#[gpui_kit::test]
fn recover_restores_dirty_windows_without_touching_original(cx: &mut TestAppContext) {
    let dir = start(cx, "recover");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let before = std::fs::read(&path).unwrap();
    let mut edited = sample();
    edited
        .push_track(keyscore_core::Track::new(
            "Recovered",
            keyscore_core::Channel::new(2).unwrap(),
            keyscore_core::TrackColor::new(1).unwrap(),
        ))
        .unwrap();
    let edited_text = ksq::to_string(&edited);
    let auto = autosave_dir(cx);
    plant_dead(&auto, "song", &edited_text, Some(&path), None);
    plant_dead(&auto, "untitled", &ksq::to_string(&sample()), None, None);
    let first = launch_with_recovery(cx);
    assert!(prompt_text(cx).contains('2'), "{}", prompt_text(cx));
    answer(cx, "복구");
    assert!(
        !cx.update(|cx| cx.windows().contains(&first)),
        "손대지 않은 첫 창이 남았다"
    );
    let all = windows(cx);
    assert_eq!(all.len(), 2);
    let restored = all
        .iter()
        .find(|w| {
            cx.read(|cx| w.read(cx).document().read(cx).path().map(Path::to_path_buf))
                == Some(path.clone())
        })
        .expect("원래 경로의 창")
        .clone();
    assert_eq!(text_of(cx, &restored), edited_text);
    assert!(dirty(cx, &restored));
    assert!(cx.read(|cx| restored.read(cx).document().read(cx).is_recovered()));
    assert!(
        title(cx, &restored).contains("복구됨"),
        "{}",
        title(cx, &restored)
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "복구가 원래 파일을 바꿨다"
    );
    // 저장본은 이제 이 세션 것이다(다시 복구 대상이 아니다).
    assert!(cx.read(autosave::find_recoverable).is_empty());
    assert_eq!(autosaves(cx).len(), 2);
}

#[gpui_kit::test]
fn recovered_window_save_writes_original_and_clears_autosave(cx: &mut TestAppContext) {
    let dir = start(cx, "recover-save");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let edited_text = r#"{"tracks": [{"name": "Edited", "channel": 3}]}"#;
    let auto = autosave_dir(cx);
    plant_dead(&auto, "song", edited_text, Some(&path), None);
    launch_with_recovery(cx);
    answer(cx, "복구");
    let restored = windows(cx).pop().unwrap();
    let handle = cx.read(|cx| restored.read(cx).window_handle());
    run(cx, handle, file::Save);
    let (back, _) = ksq::read_file(&path).unwrap();
    assert_eq!(back.tracks()[0].name(), "Edited");
    assert!(autosaves(cx).is_empty());
    assert!(!dirty(cx, &restored));
    assert!(!title(cx, &restored).contains("복구됨"));
}

#[gpui_kit::test]
fn discard_removes_recoverable_autosaves(cx: &mut TestAppContext) {
    start(cx, "discard");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "a", &ksq::to_string(&sample()), None, None);
    let first = launch_with_recovery(cx);
    answer(cx, "버리기");
    assert!(autosaves(cx).is_empty());
    assert!(cx.read(autosave::find_recoverable).is_empty());
    assert_eq!(windows(cx).len(), 1);
    assert!(cx.update(|cx| cx.windows().contains(&first)));
}

#[gpui_kit::test]
fn no_recoverable_shows_no_sheet(cx: &mut TestAppContext) {
    start(cx, "none");
    launch_with_recovery(cx);
    assert!(!cx.has_pending_prompt());
}

#[gpui_kit::test]
fn recover_newer_version_keeps_read_only(cx: &mut TestAppContext) {
    let dir = start(cx, "recover-newer");
    let path = dir.join("future.ksq");
    std::fs::write(&path, r#"{"format_version": 5}"#).unwrap();
    let before = std::fs::read(&path).unwrap();
    let auto = autosave_dir(cx);
    plant_dead(&auto, "future", r#"{"tracks": []}"#, Some(&path), Some(5));
    launch_with_recovery(cx);
    answer(cx, "복구");
    let restored = windows(cx).pop().unwrap();
    assert_eq!(
        cx.read(|cx| restored.read(cx).document().read(cx).read_only()),
        Some(ReadOnly::NewerFormat(5))
    );
    let handle = cx.read(|cx| restored.read(cx).window_handle());
    run(cx, handle, file::Save);
    assert!(prompt_text(cx).contains("새 버전"));
    answer(cx, "취소");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[gpui_kit::test]
fn recover_path_already_open_opens_untitled(cx: &mut TestAppContext) {
    let dir = start(cx, "recover-open");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (_h, _) = open_file(cx, &path);
    let auto = autosave_dir(cx);
    plant_dead(&auto, "song", &ksq::to_string(&sample()), Some(&path), None);
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    answer(cx, "복구");
    let all = windows(cx);
    assert_eq!(all.len(), 2);
    let with_path = all
        .iter()
        .filter(|w| cx.read(|cx| w.read(cx).document().read(cx).path().is_some()))
        .count();
    assert_eq!(with_path, 1, "같은 파일이 두 창에 열렸다");
}

#[gpui_kit::test]
fn corrupt_autosave_is_left_and_counted(cx: &mut TestAppContext) {
    start(cx, "corrupt");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "good", &ksq::to_string(&sample()), None, None);
    plant_dead(&auto, "bad", "{ not json", None, None);
    std::fs::write(auto.join("orphan.ksq"), ksq::to_string(&sample())).unwrap();
    launch_with_recovery(cx);
    let text = prompt_text(cx);
    assert!(text.contains("읽을 수 없는"), "{text}");
    answer(cx, "복구");
    assert_eq!(windows(cx).len(), 1);
    assert!(auto.join("bad.ksq").exists(), "깨진 저장본을 지웠다");
    assert!(
        auto.join("orphan.ksq").exists(),
        "메타 없는 저장본을 지웠다"
    );
}

#[gpui_kit::test]
fn recovery_removes_stale_temp_files_of_dead_session(cx: &mut TestAppContext) {
    let dir = start(cx, "temps");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let stale = dir.join(".song.ksq.tmp-999999-0");
    let other = dir.join(".song.ksq.tmp-12345-0");
    std::fs::write(&stale, "half").unwrap();
    std::fs::write(&other, "someone else").unwrap();
    let auto = autosave_dir(cx);
    plant_dead(&auto, "song", &ksq::to_string(&sample()), Some(&path), None);
    launch_with_recovery(cx);
    answer(cx, "버리기");
    assert!(!stale.exists(), "끝난 세션의 임시 파일이 남았다");
    assert!(other.exists(), "다른 프로세스의 임시 파일을 지웠다");
}

// ---------- S2-T7a 이월(리뷰 3 제안) ----------

#[gpui_kit::test]
fn stale_identity_ignored_when_path_holds_other_file(cx: &mut TestAppContext) {
    let dir = start(cx, "stale-id");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, _) = open_file(cx, &path);
    // 바깥에서 원본을 옮기고 같은 이름으로 새 파일을 썼다. 옮긴 옛 파일은 이 창의 파일이 아니다.
    let moved = dir.join("old.ksq");
    std::fs::rename(&path, &moved).unwrap();
    write_sample(&path);
    let (other, _) = cx.update(|cx| files::open_path(&moved, cx)).unwrap();
    assert_ne!(other, handle);
    // 경로로는 여전히 같은 창이다.
    let (again, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    assert_eq!(again, handle);
}

#[gpui_kit::test]
fn moved_file_still_matches_remembered_identity(cx: &mut TestAppContext) {
    let dir = start(cx, "moved-id");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, _) = open_file(cx, &path);
    let moved = dir.join("moved.ksq");
    std::fs::rename(&path, &moved).unwrap();
    let (again, _) = cx.update(|cx| files::open_path(&moved, cx)).unwrap();
    assert_eq!(again, handle, "옮긴 파일이 두 창에 열렸다");
}

#[gpui_kit::test]
fn recent_broken_hard_link_pair_removed_together(cx: &mut TestAppContext) {
    let dir = dir_for("recent-link");
    let a = dir.join("a.ksq");
    std::fs::write(&a, "{ not json").unwrap();
    let b = dir.join("b.ksq");
    std::fs::hard_link(&a, &b).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({ "recent_files": [a.display().to_string(), b.display().to_string()] })
            .to_string(),
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    assert_eq!(cx.read(files::recent_files).len(), 1);
    let (h, _) = open(cx, Project::new());
    run(cx, h, file::OpenRecent(a.display().to_string()));
    answer(cx, "확인");
    assert!(
        cx.read(files::recent_files).is_empty(),
        "{:?}",
        cx.read(files::recent_files)
    );
}

#[gpui_kit::test]
fn menu_build_uses_cached_recent_labels(cx: &mut TestAppContext) {
    let dir = start(cx, "recent-cache");
    let path = dir.join("a.ksq");
    write_sample(&path);
    cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let before = cx.read(files::recent_label_computations);
    for _ in 0..5 {
        let _ = cx.update(|cx| keyscore_app::commands::menus(cx));
        let _ = cx.update(|cx| keyscore_app::commands::palette_entries(cx));
    }
    assert!(
        cx.read(files::recent_label_computations) <= before + 1,
        "목록이 그대로인데 라벨을 다시 계산했다"
    );
}

#[gpui_kit::test]
fn close_during_autosave_write_leaves_no_autosave(cx: &mut TestAppContext) {
    start(cx, "close-in-flight");
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    // 타이머만 울리고 백그라운드 쓰기는 아직 끝내지 않은 채 창을 닫는다.
    cx.executor()
        .advance_clock(AUTOSAVE_DELAY + Duration::from_secs(1));
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(keyscore_app::actions::window::Close), cx);
    })
    .unwrap();
    cx.run_until_parked();
    answer(cx, "저장 안 함");
    advance(cx, AUTOSAVE_DELAY * 2);
    assert!(autosaves(cx).is_empty(), "버린 창의 저장본이 남았다");
    // 시험이 뷰를 쥐고 있어도(뷰가 살아 있어도) 다시 쓰지 않는다.
    drop(view);
}

#[gpui_kit::test]
fn closed_view_kept_alive_does_not_autosave_again(cx: &mut TestAppContext) {
    start(cx, "closed-alive");
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, keyscore_app::actions::window::Close);
    answer(cx, "저장 안 함");
    // 닫힌 창의 문서가 바뀌어도(뷰는 시험이 쥐고 있다) 저장본을 만들지 않는다.
    view.update(cx, |w, cx| {
        w.document().update(cx, |d, cx| {
            let _ = d.apply(vec![]);
            cx.notify();
        });
    });
    advance(cx, AUTOSAVE_DELAY * 2);
    let left = autosaves(cx);
    assert!(
        left.is_empty(),
        "{left:?} {:?}",
        left.first().map(|p| meta_of(p))
    );
}
