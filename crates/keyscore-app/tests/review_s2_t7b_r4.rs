//! S2-T7b 적대적 리뷰 4회차(좁은 확인) 시험. 깨뜨리지 못한 시도만 둔다. 깨뜨린 재현은
//! `docs/reports/s2-t7b/review-4.md` 부록에 있다.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::track;
use keyscore_app::autosave::{self, AUTOSAVE_DELAY};
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

// ---------------------------------------------------------------- 도우미(tests/review_s2_t7b_r3.rs와 같은 모양)

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t7b-r4-{}-{name}", std::process::id()));
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
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

fn autosaves(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
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

fn plant_dead(dir: &Path, key: &str, content: &str) {
    plant(dir, key, content, &format!("dead-{key}"));
}

fn plant(dir: &Path, key: &str, content: &str, session: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{key}.ksq")), content).unwrap();
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

// ---------------------------------------------------------------- 깨뜨리지 못한 시도

/// 항목 3: 리뷰 3 잠금 정리 시험의 살아 있는 잠금 파일은 방금 만든 것이라, 수정 뒤에는 「오래된 것만 지움」
/// 조건만으로 남는다(`session_alive` 판정을 거치지 않는다). 살아 있는 다른 실행과 이 실행의 잠금 파일을 10분
/// 전으로 맞춰도 정리가 지우지 않는다(잠금을 쥔 세션 판정이 여전히 지킨다).
#[gpui_kit::test]
fn review_r4_aged_live_and_own_locks_survive_cleanup(cx: &mut TestAppContext) {
    let dir = start(cx, "aged-live");
    let auto = autosave_dir(cx);
    let mut live = cx.new_app();
    live.update(|cx| init(cx, dir.join("settings.json")));
    let live_locks = locks(&auto);
    assert_eq!(live_locks.len(), 2, "{live_locks:?}");
    for l in &live_locks {
        age(&auto.join(l), Duration::from_mins(10));
    }
    plant_dead(&auto, "gone", &edited("Gone"));
    let dead_lock = auto.join("session-dead-gone.lock");
    std::fs::write(&dead_lock, "").unwrap();
    age(&dead_lock, Duration::from_mins(10));
    launch_with_recovery(cx);
    answer(cx, "버리기");
    let after = locks(&auto);
    for l in &live_locks {
        assert!(
            after.contains(l),
            "살아 있는 세션의 오래된 잠금 파일을 지웠다: {l}"
        );
    }
    assert!(!dead_lock.exists(), "끝난 세션의 오래된 잠금 파일이 남았다");
    // 살아 있는 다른 실행의 바뀐 창 저장본은 여전히 복구 대상이 아니다.
    let (h, _) = open(&mut live, sample());
    run(&mut live, h, track::New);
    advance(&mut live, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosaves(&auto).len(), 1);
    let probe = cx.new_app();
    probe.update(|cx| init(cx, dir.join("settings.json")));
    assert!(probe.read(autosave::find_recoverable).is_empty());
}

/// 항목 2·3: 막 끝난 세션(크래시 뒤 1분 안에 다시 띄움)의 잠금 파일은 이번 정리에서 남지만, 오래된 뒤의 다음
/// 정리에서 지워진다. 쌓이지 않는다는 리뷰 2 B4의 의도는 「다음 복구·버리기 때」로 지켜진다.
#[gpui_kit::test]
fn review_r4_fresh_dead_lock_kept_then_removed_when_old(cx: &mut TestAppContext) {
    let dir = start(cx, "fresh-dead");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    let fresh = auto.join("session-dead-k.lock");
    std::fs::write(&fresh, "").unwrap();
    launch_with_recovery(cx);
    answer(cx, "버리기");
    assert!(autosaves(&auto).is_empty());
    assert!(fresh.exists(), "막 생긴 잠금 파일을 지웠다");
    // 다음 크래시 뒤의 실행: 앞의 잠금 파일은 이제 오래됐다.
    age(&fresh, Duration::from_mins(10));
    plant_dead(&auto, "m", &edited("M"));
    let mut next = cx.new_app();
    next.update(|cx| init(cx, dir.join("settings.json")));
    launch_with_recovery(&mut next);
    answer(&mut next, "버리기");
    assert!(!fresh.exists(), "오래된 끝난 세션의 잠금 파일이 남았다");
    assert!(autosaves(&auto).is_empty());
}

/// 항목 2: [복구]도 그 키의 세대별 임시 파일을 지운다. 이웃 키(접두가 겹치는 이름)의 임시 파일은 건드리지 않는다.
#[gpui_kit::test]
fn review_r4_recover_removes_only_own_generation_temps(cx: &mut TestAppContext) {
    start(cx, "recover-temps");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "123-1", &edited("T"));
    for n in ["123-1.ksq.tmp-0", "123-1.json.tmp-0", "123-1.ksq.tmp-7"] {
        std::fs::write(auto.join(n), "partial").unwrap();
    }
    // 다른 실행이 쓰는 중인 이웃 키 `123-12`의 임시 파일.
    for n in ["123-12.ksq.tmp-0", "123-12.json.tmp-0"] {
        std::fs::write(auto.join(n), "partial").unwrap();
    }
    launch_with_recovery(cx);
    answer(cx, "복구");
    assert!(windows(cx).iter().any(|w| dirty(cx, w)), "복구 창이 없다");
    let left: Vec<String> = names(&auto)
        .into_iter()
        .filter(|n| n.contains(".tmp"))
        .collect();
    assert_eq!(left, vec!["123-12.json.tmp-0", "123-12.ksq.tmp-0"]);
    let saved = autosaves(&auto);
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_ne!(meta_of(&saved[0])["session"], "dead-123-1");
}

/// 항목 1(제안 2의 남은 절반): A가 [복구]했지만 다시 쓰지 못해 잠금만 쥔 저장본을, 함께 시트를 받은 B가
/// [버리기]하면 지운다(`discard`는 `claimed`를 보지 않는다). 그래도 A의 창은 재시도(30 s)로 이 세션 것으로
/// 다시 써, 그 뒤 다른 실행은 복구 대상으로 보지 않는다.
#[gpui_kit::test]
fn review_r4_discard_of_claimed_autosave_is_rewritten_by_holder(cx: &mut TestAppContext) {
    let dir = start(cx, "claim-discard");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Claimed"));
    launch_with_recovery(cx);
    let mut b = cx.new_app();
    b.update(|cx| init(cx, dir.join("settings.json")));
    launch_with_recovery(&mut b);
    set_mode(&auto, 0o555);
    answer(cx, "복구");
    set_mode(&auto, 0o755);
    assert!(
        windows(cx).iter().any(|w| dirty(cx, w)),
        "A가 복구 창을 열지 않았다"
    );
    answer(&mut b, "버리기");
    let after_discard = autosaves(&auto).len();
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let saved = autosaves(&auto);
    assert_eq!(
        after_discard, 0,
        "B의 [버리기]가 잠긴 저장본을 지우지 않았다(동작이 바뀜)"
    );
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_ne!(meta_of(&saved[0])["session"], "dead-k");
    let probe = cx.new_app();
    probe.update(|cx| init(cx, dir.join("settings.json")));
    assert!(probe.read(autosave::find_recoverable).is_empty());
}

/// 항목 1·2: 리뷰 3 C1·C2 상황을 [복구]로 바꿔 본다. 막 뜬 실행 C의 잠금 파일(잠그기 전)은 [복구] 뒤 정리에도
/// 남고, 잠근 뒤 C의 저장본은 다른 실행의 복구 대상이 아니다.
#[gpui_kit::test]
fn review_r4_fresh_lock_survives_recover_cleanup(cx: &mut TestAppContext) {
    let dir = start(cx, "fresh-recover");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Old"));
    launch_with_recovery(cx);
    let fresh = auto.join("session-fresh.lock");
    let c_lock = std::fs::File::create(&fresh).unwrap();
    answer(cx, "복구");
    c_lock.try_lock().unwrap();
    plant(&auto, "c-window", &edited("Live"), "fresh");
    let survived = fresh.exists();
    let later = cx.new_app();
    later.update(|cx| init(cx, dir.join("settings.json")));
    let found = later.read(autosave::find_recoverable).len();
    drop(c_lock);
    assert!(survived, "막 뜬 실행의 잠금 파일을 지웠다");
    assert_eq!(found, 0);
}
