//! S2-T7b 적대적 리뷰 3회차 시험(자동 저장본과 크래시 후 복구). 깨뜨리지 못한 시도만 둔다. 깨뜨린 재현은
//! `docs/reports/s2-t7b/review-3.md` 부록에 있다.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{file, track, window};
use keyscore_app::autosave::{self, AUTOSAVE_DELAY};
use keyscore_app::files;
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

// ---------------------------------------------------------------- 도우미(tests/review_s2_t7b_r2.rs와 같은 모양)

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t7b-r3-{}-{name}", std::process::id()));
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
    dispatch(cx, handle, action);
    cx.run_until_parked();
}

/// 처리기만 돌리고 다른 작업은 돌리지 않는다.
fn dispatch(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
}

fn touch(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    run(cx, handle, track::New);
}

fn advance(cx: &mut TestAppContext, by: Duration) {
    cx.executor().advance_clock(by);
    cx.run_until_parked();
}

/// 시계만 옮긴다. 뒤에 `tick`으로 한 단계씩 돌린다.
fn advance_clock_only(cx: &mut TestAppContext, by: Duration) {
    cx.dispatcher.scheduler().clock().advance(by);
}

fn ticks(cx: &mut TestAppContext, n: usize) {
    for _ in 0..n {
        if !cx.executor().tick() {
            break;
        }
    }
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
    list_autosaves(&dir)
}

fn list_autosaves(dir: &Path) -> Vec<PathBuf> {
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

/// 저장본 폴더의 임시 파일(`<키>.ksq.tmp-<세대>`, `<키>.json.tmp-<세대>`).
fn temps(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".tmp"))
        .collect()
}

fn meta_of(ksq_path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(ksq_path.with_extension("json")).unwrap())
        .unwrap()
}

/// 메타의 원래 경로가 `path`인 저장본의 내용.
fn autosave_text_for(cx: &mut TestAppContext, path: &Path) -> Option<String> {
    autosaves(cx)
        .into_iter()
        .find(|p| meta_of(p)["path"].as_str() == Some(&path.display().to_string()))
        .map(|p| std::fs::read_to_string(p).unwrap())
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
fn plant_dead(dir: &Path, key: &str, content: &str, path: Option<&Path>) {
    plant(dir, key, content, path, &format!("dead-{key}"));
}

fn plant(dir: &Path, key: &str, content: &str, path: Option<&Path>, session: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{key}.ksq")), content).unwrap();
    let meta = serde_json::json!({
        "path": path.map(|p| p.display().to_string()),
        "read_only": null,
        "session": session,
        "pid": 999_999,
    });
    std::fs::write(dir.join(format!("{key}.json")), meta.to_string()).unwrap();
}

fn edited(name: &str) -> String {
    format!(r#"{{"tracks": [{{"name": "{name}", "channel": 2}}]}}"#)
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

fn release_session_locks(dir: &Path) {
    for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().starts_with("session-") {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
}

fn set_mode(dir: &Path, mode: u32) {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
}

// ---------------------------------------------------------------- 깨뜨리지 못한 시도

/// 리뷰 2 지적 1의 변형: 쓰기 도중 k단계 → 또 고침 → 끝내기 훅 → 또 고침 → 끝내기 훅 → 남은 작업 진행.
/// 두 번째 훅의 최신 내용이 남는다(늦게 끝난 옛 쓰기는 버려진다).
#[gpui_kit::test]
fn review_r3_two_flushes_with_inflight_write_keep_latest(cx: &mut TestAppContext) {
    let dir = start(cx, "flush-twice");
    for k in 0..12 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, view) = open_file(cx, &path);
        touch(cx, handle);
        advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        ticks(cx, k);
        dispatch(cx, handle, track::New);
        cx.update(autosave::flush_all);
        dispatch(cx, handle, track::New);
        cx.update(autosave::flush_all);
        let latest = text_of(cx, &view);
        cx.run_until_parked();
        assert_eq!(
            autosave_text_for(cx, &path).as_deref(),
            Some(latest.as_str()),
            "k={k}"
        );
        run(cx, handle, file::Save);
        assert!(autosaves(cx).is_empty(), "k={k}: {:?}", autosaves(cx));
    }
}

/// 쓰기 도중 k단계 → 또 고침 → 끝내기 훅 → `Cmd+S` → 남은 작업 진행. 저장한 문서의 저장본이 늦은 옛 쓰기로
/// 되살아나지 않는다(지우기가 바닥을 올린다).
#[gpui_kit::test]
fn review_r3_save_after_flush_with_inflight_write_leaves_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "flush-save");
    for k in 0..12 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, view) = open_file(cx, &path);
        touch(cx, handle);
        advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        ticks(cx, k);
        dispatch(cx, handle, track::New);
        cx.update(autosave::flush_all);
        run(cx, handle, file::Save);
        assert!(!dirty(cx, &view));
        advance(cx, AUTOSAVE_DELAY * 2);
        assert!(autosaves(cx).is_empty(), "k={k}: {:?}", autosaves(cx));
    }
    let auto = autosave_dir(cx);
    assert!(temps(&auto).is_empty(), "{:?}", temps(&auto));
    release_session_locks(&auto);
    let next = cx.new_app();
    next.update(|cx| init(cx, dir.join("settings.json")));
    assert!(next.read(autosave::find_recoverable).is_empty());
}

/// 쓰기 도중 k단계 → `Cmd+S` → 또 고침 → 끝내기 훅 → 남은 작업 진행. 저장 뒤의 변경이 남고, 늦게 끝난 옛
/// 쓰기(저장 전 내용)가 덮지 않는다. 그 뒤 더 고치면 30 s 안에 다시 쓴다(버려진 쓰기가 다음 타이머를 막지
/// 않는다).
#[gpui_kit::test]
fn review_r3_flush_after_save_with_inflight_write_keeps_latest_and_rearms(cx: &mut TestAppContext) {
    let dir = start(cx, "save-flush");
    for k in 0..12 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, view) = open_file(cx, &path);
        touch(cx, handle);
        advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        ticks(cx, k);
        run(cx, handle, file::Save);
        dispatch(cx, handle, track::New);
        cx.update(autosave::flush_all);
        let latest = text_of(cx, &view);
        cx.run_until_parked();
        assert_eq!(
            autosave_text_for(cx, &path).as_deref(),
            Some(latest.as_str()),
            "k={k}"
        );
        touch(cx, handle);
        let newer = text_of(cx, &view);
        advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        assert_eq!(
            autosave_text_for(cx, &path).as_deref(),
            Some(newer.as_str()),
            "k={k}: 버려진 쓰기 뒤 다음 변경을 30 s 안에 쓰지 않았다"
        );
        run(cx, handle, file::Save);
    }
    assert!(temps(&autosave_dir(cx)).is_empty());
}

/// 쓰기 도중 k단계 → 또 고침 → 끝내기 훅 → [저장 안 함]으로 닫기 → 남은 작업 진행. 버린 창의 저장본이 남지
/// 않고 세대별 임시 파일도 남지 않는다.
#[gpui_kit::test]
fn review_r3_close_after_flush_with_inflight_write_leaves_nothing(cx: &mut TestAppContext) {
    let _dir = start(cx, "flush-close");
    for k in 0..12 {
        let (handle, _view) = open(cx, sample());
        touch(cx, handle);
        advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        ticks(cx, k);
        dispatch(cx, handle, track::New);
        cx.update(autosave::flush_all);
        run(cx, handle, window::Close);
        answer(cx, "저장 안 함");
        advance(cx, AUTOSAVE_DELAY * 2);
        assert!(autosaves(cx).is_empty(), "k={k}: {:?}", autosaves(cx));
    }
    assert!(temps(&autosave_dir(cx)).is_empty());
}

/// 복구 때 이 세션 것으로 다시 쓰기에 실패해 내용 파일을 잠근 채인 창: 막힘이 풀린 뒤 재시도가 성공하면
/// 잠금을 놓아도 다른 실행이 가져가지 않고(메타가 살아 있는 이 세션), [저장 안 함]으로 닫으면 아무것도
/// 남지 않는다.
#[gpui_kit::test]
fn review_r3_claim_released_after_retry_and_close_leaves_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "claim-retry");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Claimed"), None);
    launch_with_recovery(cx);
    set_mode(&auto, 0o555);
    answer(cx, "복구");
    let restored = windows(cx)
        .into_iter()
        .find(|w| dirty(cx, w))
        .expect("복구 창");
    let other = cx.new_app();
    other.update(|cx| init(cx, dir.join("settings.json")));
    let while_failing = other.read(autosave::find_recoverable).len();
    set_mode(&auto, 0o755);
    assert_eq!(while_failing, 0);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let saved = autosaves(cx);
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_ne!(meta_of(&saved[0])["session"], "dead-k");
    let third = cx.new_app();
    third.update(|cx| init(cx, dir.join("settings.json")));
    assert_eq!(third.read(autosave::find_recoverable).len(), 0);
    let h = handle_of(cx, &restored);
    run(cx, h, window::Close);
    answer(cx, "저장 안 함");
    assert!(autosaves(cx).is_empty(), "{:?}", autosaves(cx));
    let fourth = cx.new_app();
    fourth.update(|cx| init(cx, dir.join("settings.json")));
    assert_eq!(fourth.read(autosave::find_recoverable).len(), 0);
}

/// 같은 실행 안의 판정: 복구 창이 잠근 내용 파일은 이 실행의 다른 앱(같은 프로세스, 다른 세션)에서도 잠긴
/// 것으로 보인다(flock은 열린 파일마다라 같은 프로세스에서도 겹친다).
#[gpui_kit::test]
fn review_r3_claim_is_seen_from_same_process(cx: &mut TestAppContext) {
    let dir = start(cx, "claim-same-proc");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Mine"), None);
    launch_with_recovery(cx);
    set_mode(&auto, 0o555);
    answer(cx, "복구");
    let content = auto.join("k.ksq");
    let probe = std::fs::File::open(&content).unwrap();
    let locked = probe.try_lock().is_err();
    drop(probe);
    let other = cx.new_app();
    other.update(|cx| init(cx, dir.join("settings.json")));
    let found = other.read(autosave::find_recoverable).len();
    set_mode(&auto, 0o755);
    assert!(locked, "복구 창이 내용 파일을 잠그지 않았다");
    assert_eq!(found, 0);
}

/// 끝난 세션 잠금 파일 정리는 살아 있는 다른 실행(잠금을 쥔 세션)과 이 실행의 잠금 파일, 저장본이 남은 끝난
/// 세션의 잠금 파일을 지우지 않는다.
#[gpui_kit::test]
fn review_r3_dead_lock_cleanup_keeps_live_and_in_use_locks(cx: &mut TestAppContext) {
    let dir = start(cx, "lock-cleanup");
    let auto = autosave_dir(cx);
    let mut live = cx.new_app();
    live.update(|cx| init(cx, dir.join("settings.json")));
    launch_with_recovery(&mut live);
    // 끝난 세션 둘: 하나는 버린다(잠금 파일 지움), 하나는 읽을 수 없는 저장본이 남는다(잠금 파일 둠).
    plant_dead(&auto, "gone", &edited("Gone"), None);
    std::fs::write(auto.join("session-dead-gone.lock"), "").unwrap();
    plant_dead(&auto, "bad", "not ksq", None);
    std::fs::write(auto.join("session-dead-bad.lock"), "").unwrap();
    let locks_before: Vec<String> = std::fs::read_dir(&auto)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("session-") && !n.contains("dead-"))
        .collect();
    assert_eq!(locks_before.len(), 2, "{locks_before:?}");
    launch_with_recovery(cx);
    answer(cx, "버리기");
    let locks_after: Vec<String> = std::fs::read_dir(&auto)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("session-"))
        .collect();
    for l in &locks_before {
        assert!(
            locks_after.contains(l),
            "살아 있는 세션의 잠금 파일을 지웠다: {l}"
        );
    }
    assert!(!locks_after.iter().any(|n| n == "session-dead-gone.lock"));
    assert!(locks_after.iter().any(|n| n == "session-dead-bad.lock"));
    // 살아 있는 다른 실행의 바뀐 창 저장본은 여전히 복구 대상이 아니다.
    let (h, _) = open(&mut live, sample());
    touch(&mut live, h);
    advance(&mut live, AUTOSAVE_DELAY + Duration::from_secs(1));
    let probe = cx.new_app();
    probe.update(|cx| init(cx, dir.join("settings.json")));
    assert!(probe.read(autosave::find_recoverable).is_empty());
}

// ---------- 리뷰 3 부록 A(수정 확인) ----------

// ---------------------------------------------------------------- 부록 A: 깨뜨린 시도


/// C1. 막 뜬 다른 실행이 잠금 파일을 만들고 아직 잠그지 않은 틈에 이 실행이 [버리기]·[복구]를 마치면, 끝난 세션
/// 잠금 정리가 그 잠금 파일을 지운다. 그 실행은 잠금 파일 없이 살아, 그 뒤 뜬 실행이 그 실행의 저장본을
/// 끝난 세션 것으로 보고 복구 대상에 넣는다.
#[gpui_kit::test]
fn review_r3_c1_fresh_session_lock_removed_by_dead_lock_cleanup(cx: &mut TestAppContext) {
    let dir = start(cx, "fresh-lock");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Old"), None);
    launch_with_recovery(cx);
    // 다른 실행 C의 `init`: `File::create`까지 마치고 `try_lock` 전이다.
    let fresh = auto.join("session-fresh.lock");
    let c_lock = std::fs::File::create(&fresh).unwrap();
    answer(cx, "버리기");
    // C가 잠그고, C의 바뀐 창이 저장본을 쓴다.
    c_lock.try_lock().unwrap();
    plant(&auto, "c-window", &edited("Live"), None, "fresh");
    let lock_survived = fresh.exists();
    let later = cx.new_app();
    later.update(|cx| init(cx, dir.join("settings.json")));
    let found = later.read(autosave::find_recoverable).len();
    drop(c_lock);
    assert_eq!(found, 0, "살아 있는 실행 C의 저장본을 복구 대상으로 본다");
    assert!(lock_survived, "막 뜬 실행의 잠금 파일을 지웠다");
}

/// C2. 두 실행이 같은 끝난 저장본을 함께 제안받는다. A가 [복구]했지만 이 세션 것으로 다시 쓰지 못해(저장본
/// 폴더에 쓸 수 없음) 내용 파일 잠금만 쥔 채다. B의 [복구]는 `still_dead`만 보고 잠금을 보지 않아 같은 키로
/// 두 번째 복구 창을 연다.
#[gpui_kit::test]
fn review_r3_c2_claimed_autosave_recovered_again_by_open_sheet(cx: &mut TestAppContext) {
    let dir = start(cx, "claim-race");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Claimed"), None);
    launch_with_recovery(cx);
    let mut b = cx.new_app();
    b.update(|cx| init(cx, dir.join("settings.json")));
    launch_with_recovery(&mut b);
    set_mode(&auto, 0o555);
    answer(cx, "복구");
    let a_held = windows(cx).iter().any(|w| dirty(cx, w));
    answer(&mut b, "복구");
    let b_dirty = windows(&mut b).iter().filter(|w| dirty(&mut b, w)).count();
    set_mode(&auto, 0o755);
    assert!(a_held, "A가 복구 창을 열지 않았다");
    assert_eq!(b_dirty, 0, "A가 잠근 저장본으로 B가 또 복구 창을 열었다");
}

/// C3. 쓰기 도중 끝난 세션이 남긴 세대별 임시 파일은 [버리기] 뒤에도 저장본 폴더에 남는다.
#[gpui_kit::test]
fn review_r3_c3_crash_left_generation_temps_survive_discard(cx: &mut TestAppContext) {
    start(cx, "crash-temps");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("T"), None);
    std::fs::write(auto.join("k.ksq.tmp-3"), "partial").unwrap();
    std::fs::write(auto.join("k.json.tmp-3"), "{").unwrap();
    launch_with_recovery(cx);
    answer(cx, "버리기");
    assert!(autosaves(cx).is_empty());
    assert!(temps(&auto).is_empty(), "남은 임시 파일: {:?}", temps(&auto));
}
