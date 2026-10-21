//! S2-T7b 적대적 리뷰 2회차 시험(자동 저장본과 크래시 후 복구). 깨뜨리지 못한 시도만 둔다. 깨뜨린 재현은
//! `docs/reports/s2-t7b/review-2.md` 부록에 있다.

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

// ---------------------------------------------------------------- 도우미(tests/review_s2_t7b.rs와 같은 모양)

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t7b-r2-{}-{name}", std::process::id()));
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

/// 시계만 옮긴다(`advance_clock`은 진행하며 작업을 모두 돌린다). 뒤에 `tick`으로 한 단계씩 돌린다.
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

fn notices(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .iter()
            .filter(|m| m.contains("자동 저장"))
            .count()
    })
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

fn autosave_texts(cx: &mut TestAppContext) -> Vec<String> {
    autosaves(cx)
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
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
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{key}.ksq")), content).unwrap();
    let meta = serde_json::json!({
        "path": path.map(|p| p.display().to_string()),
        "read_only": null,
        "session": format!("dead-{key}"),
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

/// 앱의 끝내기 훅이 하는 일 가운데 잠금 파일 지우기를 흉내 낸다(정상 종료·Dock 종료 뒤의 상태).
fn release_session_locks(dir: &Path) {
    for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().starts_with("session-") {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
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

// ---------------------------------------------------------------- 깨뜨리지 못한 시도

/// 리뷰 1 부록 A1은 저장본 이름을 경로 해시로 찾아, 키를 창마다 바꾼 지금은 아무것도 검사하지 않는다(늘 통과).
/// 같은 단계 진행을 저장본 폴더 전체로 다시 본다: 쓰기 도중 어느 단계에 `Cmd+S`를 끼워도 저장본이 남지 않는다.
#[gpui_kit::test]
fn review_r2_save_stepped_into_autosave_write_leaves_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "race-save");
    for k in 0..40 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, view) = open_file(cx, &path);
        touch(cx, handle);
        advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        ticks(cx, k);
        run(cx, handle, file::Save);
        assert!(!dirty(cx, &view));
        assert!(
            autosaves(cx).is_empty(),
            "k={k}: 저장한 문서의 저장본이 남았다"
        );
        advance(cx, AUTOSAVE_DELAY * 2);
        assert!(
            autosaves(cx).is_empty(),
            "k={k}: 저장한 문서의 저장본이 남았다"
        );
    }
    let auto = autosave_dir(cx);
    release_session_locks(&auto);
    let next = cx.new_app();
    next.update(|cx| init(cx, dir.join("settings.json")));
    assert!(next.read(autosave::find_recoverable).is_empty());
}

/// 쓰기 도중 어느 단계에 저장하고 곧바로 다시 고쳐도, 30 s 안에 마지막 내용의 저장본이 생긴다
/// (진행 중 작업을 놓지 않게 바꾼 뒤 「저장 → 다시 편집」 타이밍).
#[gpui_kit::test]
fn review_r2_save_then_edit_during_write_autosaves_latest(cx: &mut TestAppContext) {
    let dir = start(cx, "save-edit");
    for k in 0..30 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, view) = open_file(cx, &path);
        touch(cx, handle);
        advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        ticks(cx, k);
        run(cx, handle, file::Save);
        touch(cx, handle);
        advance(cx, AUTOSAVE_DELAY);
        assert!(dirty(cx, &view));
        assert_eq!(
            autosave_text_for(cx, &path),
            Some(text_of(cx, &view)),
            "k={k}: 다시 고친 뒤 30 s 안에 마지막 내용의 저장본이 없다"
        );
        // 다음 회차를 위해 저장한다.
        run(cx, handle, file::Save);
        assert!(autosave_text_for(cx, &path).is_none());
    }
}

/// 저장 → 10 s 뒤 다시 고침 → 놓지 않은 첫 타이머(첫 변경 + 30 s)가 울릴 때 마지막 내용을 쓴다. 그 뒤 또 고치면
/// 새 타이머가 걸린다.
#[gpui_kit::test]
fn review_r2_kept_timer_after_save_writes_latest_then_rearms(cx: &mut TestAppContext) {
    let dir = start(cx, "kept-timer");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, Duration::from_secs(5));
    run(cx, handle, file::Save);
    advance(cx, Duration::from_secs(20));
    touch(cx, handle);
    advance(cx, Duration::from_secs(6));
    assert_eq!(autosave_text_for(cx, &path), Some(text_of(cx, &view)));
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosave_text_for(cx, &path), Some(text_of(cx, &view)));
}

/// 쓰기 실패가 계속되면 30 s마다 다시 시도하되 알림은 한 번뿐이다. 저장해 깨끗해지면 재시도를 멈추고, 막힘이
/// 풀린 뒤 고치지 않으면 쓰지 않는다. 다시 고치면 30 s 안에 쓴다.
#[gpui_kit::test]
fn review_r2_failing_retry_is_quiet_and_stops_when_clean(cx: &mut TestAppContext) {
    let dir = start(cx, "retry-clean");
    let blocker = autosave_dir(cx);
    let _ = std::fs::remove_dir_all(&blocker);
    std::fs::write(&blocker, "not a directory").unwrap();
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY * 20);
    assert_eq!(notices(cx, &view), 1, "실패 알림이 되풀이된다");
    run(cx, handle, file::Save);
    assert!(!dirty(cx, &view));
    std::fs::remove_file(&blocker).unwrap();
    advance(cx, AUTOSAVE_DELAY * 4);
    assert!(autosaves(cx).is_empty(), "깨끗한 문서의 저장본을 썼다");
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(autosave_text_for(cx, &path), Some(text_of(cx, &view)));
}

/// 쓰기 실패 중에 창을 [저장 안 함]으로 닫으면 재시도도 멈춘다: 막힘이 풀려도 버린 문서의 저장본이 생기지 않는다.
#[gpui_kit::test]
fn review_r2_failing_retry_stops_after_close(cx: &mut TestAppContext) {
    start(cx, "retry-close");
    let blocker = autosave_dir(cx);
    let _ = std::fs::remove_dir_all(&blocker);
    std::fs::write(&blocker, "not a directory").unwrap();
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    run(cx, handle, window::Close);
    answer(cx, "저장 안 함");
    std::fs::remove_file(&blocker).unwrap();
    advance(cx, AUTOSAVE_DELAY * 4);
    assert!(autosaves(cx).is_empty(), "버린 창의 저장본이 생겼다");
    drop(view);
}

/// 경로 없는 두 저장본을 복구하면 창마다 다른 키를 이어받는다. 하나를 다른 이름으로 저장해도 다른 창의
/// 저장본은 남고, 다른 창을 고치면 그 키에 이어 쓴다.
#[gpui_kit::test]
fn review_r2_two_recovered_windows_keep_separate_keys(cx: &mut TestAppContext) {
    let dir = start(cx, "two-recovered");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "u1", &edited("One"), None);
    plant_dead(&auto, "u2", &edited("Two"), None);
    launch_with_recovery(cx);
    answer(cx, "복구");
    let all = windows(cx);
    assert_eq!(all.len(), 2);
    let (a, b) = (all[0].clone(), all[1].clone());
    let ha = handle_of(cx, &a);
    let hb = handle_of(cx, &b);
    run(cx, ha, file::SaveAs);
    let target = dir.join("one.ksq");
    cx.simulate_new_path_selection(move |_| Some(target.clone()));
    cx.run_until_parked();
    assert!(!dirty(cx, &a));
    let b_text = text_of(cx, &b);
    assert_eq!(
        autosave_texts(cx),
        vec![b_text],
        "다른 창의 저장본이 지워졌다"
    );
    touch(cx, hb);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let left = autosaves(cx);
    assert_eq!(left.len(), 1, "{left:?}");
    assert_eq!(std::fs::read_to_string(&left[0]).unwrap(), text_of(cx, &b));
    // 다른 이름으로 저장한 창을 다시 고치면 새 경로의 메타로 쓴다.
    touch(cx, ha);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert_eq!(
        autosave_text_for(cx, &dir.join("one.ksq")),
        Some(text_of(cx, &a))
    );
    run(cx, hb, window::Close);
    answer(cx, "저장 안 함");
    run(cx, ha, window::Close);
    answer(cx, "저장 안 함");
    assert!(autosaves(cx).is_empty(), "{:?}", autosaves(cx));
}

/// 경로를 가진 복구 창을 다른 이름으로 저장하면 옛 키가 남지 않고, 원래 파일은 그대로다.
#[gpui_kit::test]
fn review_r2_recovered_with_path_save_as_leaves_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "recovered-path-save-as");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let before = std::fs::read(&path).unwrap();
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("R"), Some(&path));
    launch_with_recovery(cx);
    answer(cx, "복구");
    let restored = windows(cx).pop().unwrap();
    let handle = handle_of(cx, &restored);
    run(cx, handle, file::SaveAs);
    let target = dir.join("other.ksq");
    cx.simulate_new_path_selection(move |_| Some(target.clone()));
    cx.run_until_parked();
    assert!(!dirty(cx, &restored));
    assert!(autosaves(cx).is_empty(), "{:?}", autosaves(cx));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    release_session_locks(&auto);
    assert!(cx.read(autosave::find_recoverable).is_empty());
}

/// 같은 폴더를 보는 두 실행이 같은 끝난 저장본을 동시에 제안받는다. B가 먼저 [복구]하면 A의 [버리기]는 B가
/// 가져간 저장본을 지우지 않고, A의 [복구]는 창을 열지 않는다(`still_dead`).
#[gpui_kit::test]
fn review_r2_other_run_recovered_first_is_left_alone(cx: &mut TestAppContext) {
    for a_answer in ["버리기", "복구"] {
        let dir = start(cx, &format!("two-runs-{a_answer}"));
        let auto = autosave_dir(cx);
        plant_dead(&auto, "k", &edited("Shared"), None);
        launch_with_recovery(cx);
        assert!(cx.has_pending_prompt());
        let mut b = cx.new_app();
        b.update(|cx| init(cx, dir.join("settings.json")));
        launch_with_recovery(&mut b);
        answer(&mut b, "복구");
        let b_windows = windows(&mut b);
        let b_view = b_windows
            .iter()
            .find(|w| b.read(|cx| w.read(cx).document().read(cx).is_dirty()))
            .expect("B의 복구 창")
            .clone();
        let b_text = text_of(&mut b, &b_view);
        let a_before = windows(cx).len();
        answer(cx, a_answer);
        assert_eq!(
            windows(cx).len(),
            a_before,
            "{a_answer}: A가 B의 저장본으로 창을 열었다"
        );
        assert_eq!(
            list_autosaves(&auto)
                .iter()
                .map(|p| std::fs::read_to_string(p).unwrap())
                .collect::<Vec<_>>(),
            vec![b_text],
            "{a_answer}: B가 가져간 저장본이 지워지거나 바뀌었다"
        );
        // 다음 회차는 같은 앱을 다시 쓴다. B의 창은 버린다.
        let hb = handle_of(&mut b, &b_view);
        run(&mut b, hb, window::Close);
        answer(&mut b, "저장 안 함");
        for w in windows(cx) {
            let h = handle_of(cx, &w);
            run(cx, h, window::Close);
        }
    }
}

/// 끝내기 [저장]을 쓰기 도중 여러 단계에 끼워도 저장본이 남지 않고 다음 실행이 복구를 제안하지 않는다.
#[gpui_kit::test]
fn review_r2_quit_save_stepped_into_write_leaves_nothing(cx: &mut TestAppContext) {
    let dir = start(cx, "quit-save");
    for k in 0..16 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, _) = open_file(cx, &path);
        touch(cx, handle);
        advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        ticks(cx, k);
        let quit = start_quit(cx);
        answer(cx, "저장");
        assert_eq!(quit.get(), Some(true), "k={k}");
        cx.update(autosave::flush_all);
        assert!(autosaves(cx).is_empty(), "k={k}: {:?}", autosaves(cx));
        advance(cx, AUTOSAVE_DELAY * 2);
        assert!(autosaves(cx).is_empty(), "k={k}: {:?}", autosaves(cx));
    }
}

/// 시계만 옮긴 뒤 쓰기 단계(k)와 [저장 안 함] 답 뒤 단계(j)를 섞어도 버린 창의 저장본이 남지 않는다.
#[gpui_kit::test]
fn review_r2_close_answer_interleaved_with_write_leaves_nothing(cx: &mut TestAppContext) {
    let _dir = start(cx, "close-interleave");
    let mut leaked = Vec::new();
    for k in 0..8 {
        for j in 0..4 {
            let (handle, view) = open(cx, sample());
            touch(cx, handle);
            run(cx, handle, window::Close);
            assert!(cx.has_pending_prompt());
            advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
            ticks(cx, k);
            cx.simulate_prompt_answer("저장 안 함");
            ticks(cx, j);
            cx.run_until_parked();
            advance(cx, AUTOSAVE_DELAY * 2);
            if !autosaves(cx).is_empty() {
                leaked.push((k, j));
                for p in autosaves(cx) {
                    let _ = std::fs::remove_file(p.with_extension("json"));
                    let _ = std::fs::remove_file(p);
                }
            }
            drop(view);
        }
    }
    assert!(leaked.is_empty(), "버린 창의 저장본이 남았다: {leaked:?}");
}
