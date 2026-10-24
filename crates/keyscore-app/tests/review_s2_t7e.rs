//! S2-T7e 적대적 리뷰 1회차 시험(자동 저장본, S2-T7b 리뷰 4 이월). 깨뜨리지 못한 시도만 남긴다. 깨진 재현은
//! `docs/reports/s2-t7e/review-1.md` 부록에 있다.
//!
//! 겨냥: 시작 때마다 도는 `remove_dead_locks`가 살아 있는 세션·이 세션·막 생긴 것을 지우는 경로, `recover`의
//! `claim`이 새는 경로(같은 파일을 연 창이 있어 경로 없이 열 때), 정리가 복구 대상이 아닌 파일을 건드리는 경로.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::autosave;
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{files, init, open_project_window};
use keyscore_core::Project;

// ---------------------------------------------------------------- 도우미(tests/t7e_autosave.rs와 같은 모양)

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-r-t7e-auto-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    (handle, view)
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
}

fn path_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<PathBuf> {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .path()
            .map(Path::to_path_buf)
    })
}

fn autosave_dir(cx: &mut TestAppContext) -> PathBuf {
    cx.read(autosave::autosave_dir)
}

fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

fn locks(dir: &Path) -> Vec<String> {
    names(dir)
        .into_iter()
        .filter(|n| {
            n.starts_with("session-") && Path::new(n).extension().is_some_and(|x| x == "lock")
        })
        .collect()
}

fn plant_dead(dir: &Path, key: &str, content: &str) {
    plant(dir, key, content, &format!("dead-{key}"), None);
}

fn plant(dir: &Path, key: &str, content: &str, session: &str, path: Option<&Path>) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{key}.ksq")), content).unwrap();
    plant_meta(dir, key, session, path);
}

fn plant_meta(dir: &Path, key: &str, session: &str, path: Option<&Path>) {
    std::fs::create_dir_all(dir).unwrap();
    let meta = serde_json::json!({
        "path": path.map(|p| p.display().to_string()),
        "read_only": null,
        "session": session,
        "pid": 999_999,
    });
    std::fs::write(dir.join(format!("{key}.json")), meta.to_string()).unwrap();
}

fn meta_of(dir: &Path, key: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{key}.json"))).unwrap())
        .unwrap()
}

fn edited(name: &str) -> String {
    format!(r#"{{"tracks": [{{"name": "{name}", "channel": 2}}]}}"#)
}

/// 파일의 수정 시각을 `ago` 전으로 맞춘다(오래된 잠금 파일 흉내).
fn age(path: &Path, ago: Duration) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() - ago)
        .unwrap();
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

fn launch_with_recovery(cx: &mut TestAppContext) -> AnyWindowHandle {
    let (handle, _) = open(cx, Project::new());
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    handle
}

fn own_session(cx: &mut TestAppContext, auto: &Path) -> String {
    let lock = locks(auto);
    assert_eq!(lock.len(), 1, "{lock:?}");
    let _ = cx;
    lock[0]
        .strip_prefix("session-")
        .and_then(|s| s.strip_suffix(".lock"))
        .unwrap()
        .to_owned()
}

/// 새 실행(같은 설정 폴더)을 띄워 복구 대상을 본다.
fn probe_recoverable(cx: &mut TestAppContext, dir: &Path) -> usize {
    let probe = cx.new_app();
    probe.update(|cx| init(cx, dir.join("settings.json")));
    probe.read(autosave::find_recoverable).len()
}

// ---------------------------------------------------------------- 시작 때 정리의 범위

/// 끝난 세션의 내용 없는 메타는 바로 지우지만, 그 세션의 막 생긴 잠금 파일은 둔다(잠그기 전일 수 있다). 1분이
/// 지난 다음 정리가 지운다.
#[gpui_kit::test]
fn orphan_meta_goes_now_but_fresh_lock_waits_until_old(cx: &mut TestAppContext) {
    start(cx, "orphan-fresh-lock");
    let auto = autosave_dir(cx);
    plant_meta(&auto, "o", "dead-o", None);
    let lock = auto.join("session-dead-o.lock");
    std::fs::write(&lock, "").unwrap();
    launch_with_recovery(cx);
    assert!(!cx.has_pending_prompt());
    let left = names(&auto);
    assert!(!left.contains(&"o.json".to_owned()), "{left:?}");
    assert!(lock.exists(), "막 생긴 잠금 파일을 지웠다: {left:?}");
    age(&lock, Duration::from_mins(10));
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    assert!(
        !lock.exists(),
        "오래된 끝난 세션 잠금 파일이 남았다: {:?}",
        names(&auto)
    );
}

/// 이 세션의 내용 없는 메타(이 실행의 창이 두 rename 사이에 있을 때의 모양)는 정리가 지우지 않는다.
#[gpui_kit::test]
fn own_orphan_meta_survives_startup_cleanup(cx: &mut TestAppContext) {
    start(cx, "own-orphan");
    let auto = autosave_dir(cx);
    let own = own_session(cx, &auto);
    plant_meta(&auto, "mine", &own, None);
    plant_meta(&auto, "o", "dead-o", None);
    launch_with_recovery(cx);
    let left = names(&auto);
    assert!(left.contains(&"mine.json".to_owned()), "{left:?}");
    assert!(!left.contains(&"o.json".to_owned()), "{left:?}");
}

/// 메타 없는 내용 파일(읽을 수 없는 저장본)은 복구 대상이 없어도 그대로 둔다. 정리는 메타만 본다.
#[gpui_kit::test]
fn content_without_meta_is_left_alone_by_startup_cleanup(cx: &mut TestAppContext) {
    start(cx, "content-only");
    let auto = autosave_dir(cx);
    std::fs::create_dir_all(&auto).unwrap();
    std::fs::write(auto.join("c.ksq"), edited("C")).unwrap();
    age(&auto.join("c.ksq"), Duration::from_mins(10));
    launch_with_recovery(cx);
    assert!(!cx.has_pending_prompt(), "메타 없는 내용으로 시트가 떴다");
    assert_eq!(
        std::fs::read_to_string(auto.join("c.ksq")).unwrap(),
        edited("C")
    );
}

/// 메타가 JSON이 아니거나 세션이 없는 `.json`은 정리가 건드리지 않는다(판정할 수 없으면 둔다).
#[gpui_kit::test]
fn unreadable_meta_is_left_alone_by_startup_cleanup(cx: &mut TestAppContext) {
    start(cx, "bad-meta");
    let auto = autosave_dir(cx);
    std::fs::create_dir_all(&auto).unwrap();
    std::fs::write(auto.join("garbage.json"), "not json").unwrap();
    std::fs::write(auto.join("nosession.json"), r#"{"path": null}"#).unwrap();
    launch_with_recovery(cx);
    let left = names(&auto);
    assert!(left.contains(&"garbage.json".to_owned()), "{left:?}");
    assert!(left.contains(&"nosession.json".to_owned()), "{left:?}");
}

/// 살아 있는 다른 실행의 내용 없는 메타는 복구 대상이 하나도 없는 시작 때 정리에서도 남는다(A6은 [버리기]
/// 경로만 본다).
#[gpui_kit::test]
fn live_orphan_meta_survives_startup_cleanup_without_recoverables(cx: &mut TestAppContext) {
    let dir = start(cx, "live-orphan-startup");
    let auto = autosave_dir(cx);
    let before = locks(&auto);
    let live = cx.new_app();
    live.update(|cx| init(cx, dir.join("settings.json")));
    let live_lock = locks(&auto)
        .into_iter()
        .find(|l| !before.contains(l))
        .unwrap();
    let live_session = live_lock
        .strip_prefix("session-")
        .and_then(|s| s.strip_suffix(".lock"))
        .unwrap()
        .to_owned();
    plant_meta(&auto, "between", &live_session, None);
    age(&auto.join(&live_lock), Duration::from_mins(10));
    launch_with_recovery(cx);
    assert!(!cx.has_pending_prompt());
    let left = names(&auto);
    assert!(left.contains(&"between.json".to_owned()), "{left:?}");
    assert!(left.contains(&live_lock), "{left:?}");
    drop(live);
}

// ---------------------------------------------------------------- `recover`의 잠금

/// 저장본의 원래 파일을 이미 연 창이 있으면 경로 없이 연다. 그 창도 잠금을 이어받아 저장본을 이 세션 것으로 다시
/// 쓰고, 다른 실행은 복구 대상으로 보지 않는다.
#[gpui_kit::test]
fn recover_without_path_still_takes_over_autosave(cx: &mut TestAppContext) {
    let dir = start(cx, "recover-no-path");
    let auto = autosave_dir(cx);
    let file = dir.join("song.ksq");
    std::fs::write(&file, edited("Saved")).unwrap();
    let (_handle, opened) = cx.update(|cx| files::open_path(&file, cx)).unwrap();
    cx.run_until_parked();
    let file = file.canonicalize().unwrap();
    plant(&auto, "k", &edited("Recovered"), "dead-k", Some(&file));
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    answer(cx, "복구");
    let recovered: Vec<Entity<ProjectWindow>> = windows(cx)
        .into_iter()
        .filter(|w| *w != opened && dirty(cx, w))
        .collect();
    assert_eq!(recovered.len(), 1, "복구 창 하나");
    assert_eq!(path_of(cx, &recovered[0]), None, "같은 파일 두 창");
    assert_eq!(
        path_of(cx, &opened).as_deref(),
        Some(file.as_path()),
        "먼저 연 창의 경로가 바뀌었다"
    );
    let own = own_session(cx, &auto);
    assert_eq!(
        meta_of(&auto, "k")["session"],
        own,
        "저장본을 이 세션 것으로 다시 쓰지 않았다"
    );
    assert_eq!(meta_of(&auto, "k")["path"], serde_json::Value::Null);
    assert_eq!(
        probe_recoverable(cx, &dir),
        0,
        "다른 실행이 복구 대상으로 본다"
    );
}

/// 복구 대상 둘 가운데 하나를 다른 실행이 잠갔으면, 잠긴 것은 건너뛰고 다른 하나만 연다. 건너뛴 저장본은
/// 잠금이 풀리면 다음 실행의 복구 대상으로 돌아온다(잠금이 새지 않는다).
#[gpui_kit::test]
fn skipped_claimed_autosave_returns_when_claim_is_released(cx: &mut TestAppContext) {
    let dir = start(cx, "claim-returns");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    plant_dead(&auto, "m", &edited("M"));
    launch_with_recovery(cx);
    let held = std::fs::File::open(auto.join("k.ksq")).unwrap();
    held.try_lock().unwrap();
    answer(cx, "복구");
    assert_eq!(
        probe_recoverable(cx, &dir),
        0,
        "잠긴 동안에는 복구 대상이 아니다"
    );
    drop(held);
    assert_eq!(
        probe_recoverable(cx, &dir),
        1,
        "잠금이 풀린 저장본이 돌아오지 않았다"
    );
    assert_eq!(meta_of(&auto, "k")["session"], "dead-k");
    assert_eq!(
        std::fs::read_to_string(auto.join("k.ksq")).unwrap(),
        edited("K")
    );
}

/// [복구] 뒤 정리: 복구한 저장본의 끝난 세션 잠금 파일(오래됨)은 지워지고, 아직 끝난 세션 것으로 남은(다른
/// 실행이 잠근) 저장본의 세션 잠금 파일은 남는다.
#[gpui_kit::test]
fn recover_cleanup_keeps_lock_of_session_whose_autosave_is_still_claimed(cx: &mut TestAppContext) {
    start(cx, "recover-cleanup-claimed");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    plant_dead(&auto, "m", &edited("M"));
    for s in ["dead-k", "dead-m"] {
        let lock = auto.join(format!("session-{s}.lock"));
        std::fs::write(&lock, "").unwrap();
        age(&lock, Duration::from_mins(10));
    }
    launch_with_recovery(cx);
    let held = std::fs::File::open(auto.join("k.ksq")).unwrap();
    held.try_lock().unwrap();
    answer(cx, "복구");
    let left = locks(&auto);
    assert!(left.contains(&"session-dead-k.lock".to_owned()), "{left:?}");
    assert!(
        !left.contains(&"session-dead-m.lock".to_owned()),
        "{left:?}"
    );
    drop(held);
}
