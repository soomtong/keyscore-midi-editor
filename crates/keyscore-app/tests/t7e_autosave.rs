//! S2-T7e: S2-T7b 리뷰 4 이월(`docs/reports/s2-t7b/review-4.md`). 내용 없는 메타, [버리기]와 복구 잠금,
//! [복구]의 잠금 순서, 시작 때 잠금 파일 정리.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::autosave::{self, AUTOSAVE_DELAY};
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

// ---------------------------------------------------------------- 도우미(tests/review_s2_t7b_r4.rs와 같은 모양)

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-t7e-auto-{}-{name}", std::process::id()));
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

fn advance(cx: &mut TestAppContext, by: Duration) {
    cx.executor().advance_clock(by);
    cx.run_until_parked();
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
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

fn autosaves(dir: &Path) -> Vec<String> {
    names(dir)
        .into_iter()
        .filter(|n| Path::new(n).extension().is_some_and(|x| x == "ksq"))
        .collect()
}

fn plant_dead(dir: &Path, key: &str, content: &str) {
    plant(dir, key, content, &format!("dead-{key}"));
}

fn plant(dir: &Path, key: &str, content: &str, session: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{key}.ksq")), content).unwrap();
    plant_meta(dir, key, session);
}

fn plant_meta(dir: &Path, key: &str, session: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let meta = serde_json::json!({
        "path": null,
        "read_only": null,
        "session": session,
        "pid": 999_999,
    });
    std::fs::write(dir.join(format!("{key}.json")), meta.to_string()).unwrap();
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

fn old_dead_lock(dir: &Path, session: &str) -> PathBuf {
    let path = dir.join(format!("session-{session}.lock"));
    std::fs::write(&path, "").unwrap();
    age(&path, Duration::from_mins(10));
    path
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

fn set_mode(dir: &Path, mode: u32) {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// 다른 실행이 복구하며 내용 파일을 잠근 상태를 흉내 낸다. 돌려준 파일을 쥐고 있는 동안 잠겨 있다.
fn claim_like_another_run(content: &Path) -> std::fs::File {
    let file = std::fs::File::open(content).unwrap();
    file.try_lock().unwrap();
    file
}

/// 새 실행(같은 설정 폴더)을 띄워 그 세션의 잠금 파일 이름을 돌려준다.
fn another_run(cx: &mut TestAppContext, dir: &Path, auto: &Path) -> (TestAppContext, String) {
    let before = locks(auto);
    let app = cx.new_app();
    app.update(|cx| init(cx, dir.join("settings.json")));
    let lock = locks(auto)
        .into_iter()
        .find(|l| !before.contains(l))
        .expect("새 실행의 잠금 파일");
    (app, lock)
}

// ---------------------------------------------------------------- A1. 내용 없는 메타

/// 리뷰 4 부록 D1. 첫 쓰기에서 메타 rename 뒤 내용 rename 전에 끝난 세션은 메타만 남긴다. [버리기] 뒤에 그
/// 메타와 그 세션의 잠금 파일이 함께 지워진다.
#[gpui_kit::test]
fn orphan_meta_of_dead_session_is_removed_with_its_lock(cx: &mut TestAppContext) {
    start(cx, "orphan-meta");
    let auto = autosave_dir(cx);
    plant_meta(&auto, "o", "dead-o");
    old_dead_lock(&auto, "dead-o");
    plant_dead(&auto, "k", &edited("K"));
    launch_with_recovery(cx);
    answer(cx, "버리기");
    let left = names(&auto);
    assert!(
        !left
            .iter()
            .any(|n| n == "o.json" || n == "session-dead-o.lock"),
        "끝난 세션의 메타만 있는 저장본과 그 잠금 파일이 남았다: {left:?}"
    );
}

/// 살아 있는 세션의 내용 없는 메타(두 rename 사이)는 정리가 지우지 않는다. 그 세션의 잠금 파일도 남는다.
#[gpui_kit::test]
fn live_session_meta_without_content_is_kept(cx: &mut TestAppContext) {
    let dir = start(cx, "live-orphan");
    let auto = autosave_dir(cx);
    let (_live, live_lock) = another_run(cx, &dir, &auto);
    let live_session = live_lock
        .strip_prefix("session-")
        .and_then(|s| s.strip_suffix(".lock"))
        .unwrap()
        .to_owned();
    plant_meta(&auto, "between", &live_session);
    age(&auto.join(&live_lock), Duration::from_mins(10));
    plant_dead(&auto, "k", &edited("K"));
    launch_with_recovery(cx);
    answer(cx, "버리기");
    let left = names(&auto);
    assert!(left.contains(&"between.json".to_owned()), "{left:?}");
    assert!(left.contains(&live_lock), "{left:?}");
}

// ---------------------------------------------------------------- A2·A3. 복구 잠금과 [버리기]·[복구]

/// 시트가 떠 있는 동안 다른 실행이 복구하며 잠근 저장본은 [버리기]가 지우지 않는다(리뷰 4 지적 2).
#[gpui_kit::test]
fn discard_skips_autosave_claimed_by_another_run(cx: &mut TestAppContext) {
    start(cx, "discard-claimed");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    plant_dead(&auto, "m", &edited("M"));
    launch_with_recovery(cx);
    let held = claim_like_another_run(&auto.join("k.ksq"));
    answer(cx, "버리기");
    assert_eq!(
        autosaves(&auto),
        vec!["k.ksq".to_owned()],
        "{:?}",
        names(&auto)
    );
    assert!(auto.join("k.json").exists(), "잠긴 저장본의 메타를 지웠다");
    drop(held);
}

/// 시트가 떠 있는 동안 다른 실행이 잠근 저장본은 [복구]가 창을 열지 않는다. 잠그지 못한 쪽은 건너뛴다(리뷰 4
/// 지적 3).
#[gpui_kit::test]
fn recover_skips_autosave_claimed_meanwhile(cx: &mut TestAppContext) {
    start(cx, "recover-claimed");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    plant_dead(&auto, "m", &edited("M"));
    launch_with_recovery(cx);
    let held = claim_like_another_run(&auto.join("k.ksq"));
    answer(cx, "복구");
    let open = windows(cx);
    let mut restored = Vec::new();
    for w in &open {
        if dirty(cx, w) {
            restored.push(cx.read(|cx| {
                w.read(cx).document().read(cx).project().tracks()[0]
                    .name()
                    .to_owned()
            }));
        }
    }
    assert_eq!(
        restored,
        vec!["M".to_owned()],
        "잠긴 저장본으로 창을 열었다"
    );
    let content = std::fs::read_to_string(auto.join("k.ksq")).unwrap();
    assert_eq!(content, edited("K"), "잠긴 저장본을 덮어썼다");
    drop(held);
}

/// 복구 창은 첫 쓰기가 실패해도 잠금을 쥔다. 잠금은 창을 열기 전에 잡으므로 다른 실행은 그 사이에도 복구
/// 대상으로 보지 않는다.
#[gpui_kit::test]
fn recovered_window_holds_claim_even_if_first_write_fails(cx: &mut TestAppContext) {
    let dir = start(cx, "claim-first");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    launch_with_recovery(cx);
    set_mode(&auto, 0o555);
    answer(cx, "복구");
    assert!(
        windows(cx).iter().any(|w| dirty(cx, w)),
        "복구 창을 열지 않았다"
    );
    let probe = cx.new_app();
    probe.update(|cx| init(cx, dir.join("settings.json")));
    assert!(
        probe.read(autosave::find_recoverable).is_empty(),
        "첫 쓰기가 실패한 복구 창의 저장본을 다른 실행이 가져갔다"
    );
    set_mode(&auto, 0o755);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_ne!(
        std::fs::read_to_string(auto.join("k.ksq")).unwrap(),
        edited("K"),
        "복구 창이 이 세션 것으로 다시 쓰지 않았다"
    );
}

// ---------------------------------------------------------------- A5. 시작 때 정리

/// 복구 대상이 없어도 시작 때 끝난 세션의 오래된 잠금 파일과 내용 없는 메타를 지운다. 막 생긴 잠금 파일(잠그기
/// 전일 수 있다)은 둔다.
#[gpui_kit::test]
fn startup_without_recoverables_removes_old_dead_locks(cx: &mut TestAppContext) {
    start(cx, "startup-clean");
    let auto = autosave_dir(cx);
    let old = old_dead_lock(&auto, "dead-old");
    let fresh = auto.join("session-dead-fresh.lock");
    std::fs::write(&fresh, "").unwrap();
    plant_meta(&auto, "o", "dead-o");
    old_dead_lock(&auto, "dead-o");
    launch_with_recovery(cx);
    assert!(!cx.has_pending_prompt(), "복구 대상이 없는데 시트가 떴다");
    let left = names(&auto);
    assert!(
        !old.exists(),
        "오래된 끝난 세션 잠금 파일이 남았다: {left:?}"
    );
    assert!(fresh.exists(), "막 생긴 잠금 파일을 지웠다: {left:?}");
    assert!(
        !left
            .iter()
            .any(|n| n == "o.json" || n == "session-dead-o.lock"),
        "{left:?}"
    );
    // 이 실행의 잠금 파일은 그대로다
    assert_eq!(locks(&auto).len(), 2, "{left:?}");
}
