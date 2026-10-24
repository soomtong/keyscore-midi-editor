//! S2-T7e 적대적 리뷰 2회차 시험(리뷰 1 제안 1·4·6 대응 확인). 깨뜨리지 못한 시도만 남긴다. 깨진 재현은
//! `docs/reports/s2-t7e/review-2.md` 부록에 있다.
//!
//! 겨냥: 임시 파일 정리가 살아 있는 세션(메타만 있는 키, 쓰는 중인 창)의 것을 지우는 경로, 건너뛴 복구의 알림,
//! 보이는 모양 비교의 표시 지우기가 살아 있는 다른 포트의 표시를 지우는 경로. `CoreMIDI`를 쓰는 시험은 자식
//! 프로세스에서 혼자 돈다.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::track;
use keyscore_app::app_state::AppSettings;
use keyscore_app::autosave::{self, AUTOSAVE_DELAY};
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{files, init, open_project_window};
use keyscore_core::{Project, ksq};

// ---------------------------------------------------------------- 도우미(자동 저장)

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = dir_for(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-r2-t7e-{}-{name}", std::process::id()));
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
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{key}.ksq")), content).unwrap();
    plant_meta(dir, key, &format!("dead-{key}"));
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

fn plant_old_temps(dir: &Path, key: &str) {
    for n in [format!("{key}.ksq.tmp-1"), format!("{key}.json.tmp-1")] {
        std::fs::write(dir.join(&n), "x").unwrap();
        age(&dir.join(&n), Duration::from_mins(10));
    }
}

fn edited(name: &str) -> String {
    format!(r#"{{"tracks": [{{"name": "{name}", "channel": 2}}]}}"#)
}

/// 파일의 수정 시각을 `ago` 전으로 맞춘다(오래된 파일 흉내).
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

fn launch_with_recovery(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = open(cx, Project::new());
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    (handle, view)
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

/// 새 실행(같은 설정 폴더)을 띄워 그 세션 id를 돌려준다.
fn another_run(cx: &mut TestAppContext, dir: &Path, auto: &Path) -> (TestAppContext, String) {
    let before = locks(auto);
    let app = cx.new_app();
    app.update(|cx| init(cx, dir.join("settings.json")));
    let lock = locks(auto)
        .into_iter()
        .find(|l| !before.contains(l))
        .expect("새 실행의 잠금 파일");
    let session = lock
        .strip_prefix("session-")
        .and_then(|s| s.strip_suffix(".lock"))
        .unwrap()
        .to_owned();
    (app, session)
}

// ---------------------------------------------------------------- 제안 1: 임시 파일 정리의 범위

/// 메타만 있는 키(두 rename 사이)의 오래된 임시 파일: 끝난 세션 것은 메타와 함께 지우고, 살아 있는 세션 것은
/// 메타와 함께 둔다. 정리 한 번에 메타 판정과 임시 파일 판정이 같은 쪽을 가리킨다.
#[gpui_kit::test]
fn old_temps_of_meta_only_key_follow_the_meta(cx: &mut TestAppContext) {
    let dir = start(cx, "temps-meta-only");
    let auto = autosave_dir(cx);
    let (live, live_session) = another_run(cx, &dir, &auto);
    plant_meta(&auto, "alive", &live_session);
    plant_old_temps(&auto, "alive");
    plant_meta(&auto, "gone", "dead-gone");
    plant_old_temps(&auto, "gone");
    launch_with_recovery(cx);
    assert!(!cx.has_pending_prompt());
    let left = names(&auto);
    assert!(
        left.contains(&"alive.json".to_owned())
            && left.contains(&"alive.ksq.tmp-1".to_owned())
            && left.contains(&"alive.json.tmp-1".to_owned()),
        "살아 있는 세션의 메타·임시 파일을 지웠다: {left:?}"
    );
    assert!(
        !left.iter().any(|n| n.starts_with("gone.")),
        "끝난 세션의 메타·임시 파일이 남았다: {left:?}"
    );
    drop(live);
}

/// 메타·내용이 있는 키의 오래된 임시 파일은 그 세션이 끝났어도 [버리기] 전에는 둔다([버리기]가 키째 지운다).
#[gpui_kit::test]
fn old_temps_of_recoverable_key_wait_for_the_sheet(cx: &mut TestAppContext) {
    start(cx, "temps-recoverable");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    plant_old_temps(&auto, "k");
    launch_with_recovery(cx);
    assert!(cx.has_pending_prompt());
    let left = names(&auto);
    assert!(left.contains(&"k.ksq.tmp-1".to_owned()), "{left:?}");
    answer(cx, "버리기");
    let left = names(&auto);
    assert!(!left.iter().any(|n| n.starts_with("k.")), "{left:?}");
}

/// 살아 있는 창이 실제로 쓴 저장본은 다른 실행의 시작 정리를 지나도 그대로다. 쓰기가 끝나면 임시 파일도 없다.
#[gpui_kit::test]
fn live_window_autosave_survives_other_runs_startup_cleanup(cx: &mut TestAppContext) {
    let dir = start(cx, "temps-live-window");
    let auto = autosave_dir(cx);
    let (handle, view) = open(cx, Project::new());
    run(cx, handle, track::New);
    assert!(dirty(cx, &view));
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let before = names(&auto);
    assert!(
        before
            .iter()
            .any(|n| Path::new(n).extension().is_some_and(|x| x == "ksq")),
        "{before:?}"
    );
    assert!(!before.iter().any(|n| n.contains(".tmp-")), "{before:?}");
    let (mut other, _) = another_run(cx, &dir, &auto);
    launch_with_recovery(&mut other);
    assert!(
        !other.has_pending_prompt(),
        "살아 있는 창의 저장본을 복구 대상으로 봤다"
    );
    let after: Vec<String> = names(&auto)
        .into_iter()
        .filter(|n| !n.starts_with("session-"))
        .collect();
    let before: Vec<String> = before
        .into_iter()
        .filter(|n| !n.starts_with("session-"))
        .collect();
    assert_eq!(after, before);
}

/// 막 생긴 임시 파일(메타 없음)은 키가 어떤 것이든 둔다. 늙으면 지운다.
#[gpui_kit::test]
fn fresh_temps_without_meta_are_kept_until_old(cx: &mut TestAppContext) {
    start(cx, "temps-fresh");
    let auto = autosave_dir(cx);
    std::fs::create_dir_all(&auto).unwrap();
    std::fs::write(auto.join("w.ksq.tmp-2"), "x").unwrap();
    std::fs::write(auto.join("w.json.tmp-2"), "x").unwrap();
    launch_with_recovery(cx);
    let left = names(&auto);
    assert!(left.contains(&"w.ksq.tmp-2".to_owned()), "{left:?}");
    assert!(left.contains(&"w.json.tmp-2".to_owned()), "{left:?}");
    age(&auto.join("w.ksq.tmp-2"), Duration::from_mins(10));
    age(&auto.join("w.json.tmp-2"), Duration::from_mins(10));
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    let left = names(&auto);
    assert!(!left.iter().any(|n| n.starts_with("w.")), "{left:?}");
}

// ---------------------------------------------------------------- 제안 6: 건너뛴 복구의 알림

/// 다른 실행이 잠근(아직 다시 쓰지 않은) 저장본을 [복구]가 건너뛰면 그 수를 알린다. 건너뛰지 않은 것은 연다.
#[gpui_kit::test]
fn recover_reports_autosaves_claimed_by_another_run(cx: &mut TestAppContext) {
    start(cx, "notify-claimed");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    plant_dead(&auto, "m", &edited("M"));
    let (_, host) = launch_with_recovery(cx);
    let held = std::fs::File::open(auto.join("k.ksq")).unwrap();
    held.try_lock().unwrap();
    answer(cx, "복구");
    let said = messages(cx, &host);
    assert!(
        said.iter()
            .any(|m| m.contains("1개") && m.contains("열지 않았다")),
        "건너뛴 저장본을 알리지 않았다: {said:?}"
    );
    assert_eq!(windows(cx).iter().filter(|w| dirty(cx, w)).count(), 1);
    drop(held);
}

/// 건너뛴 것이 없으면 알리지 않는다.
#[gpui_kit::test]
fn recover_says_nothing_when_nothing_is_skipped(cx: &mut TestAppContext) {
    start(cx, "notify-none");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    let (_, host) = launch_with_recovery(cx);
    answer(cx, "복구");
    let said = messages(cx, &host);
    assert!(
        !said.iter().any(|m| m.contains("열지 않았다")),
        "건너뛴 것이 없는데 알렸다: {said:?}"
    );
}

// ---------------------------------------------------------------- 제안 4: 보이는 모양 비교(CoreMIDI, 자식 프로세스)

const MAP: &str = "제안대로 매핑";

fn unique(tag: &str) -> String {
    format!("KS r2t7e {} {tag}", std::process::id())
}

fn install(cx: &mut TestAppContext, tag: &str) {
    let options = EngineOptions {
        client_name: format!("ks-r2t7e-{tag}"),
        port_name: unique(&format!("main {tag}")),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
}

fn mapped_ports(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(|cx| AppSettings::get(cx).midi_output.mapped_ports.clone())
}

fn own_ports(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(engine::virtual_port_names)
}

/// 결합자(ZWJ)를 끼운 이름: 보이는 모양은 같고 글자는 다르다.
fn joined(name: &str) -> String {
    let out = name.replacen("stale", "sta\u{200D}le", 1);
    assert_ne!(out, name);
    assert!(keyscore_midi::same_port_name(&out, name));
    out
}

fn project_on(port: &str) -> Project {
    ksq::from_str(&format!(
        r#"{{"tracks": [{{"name": "T", "port": {{"named": {}}}, "channel": 1,
            "clips": [{{"start": 0, "length": 3840, "notes": [[0, 480, 60, 51492]]}}]}}]}}"#,
        serde_json::to_string(port).unwrap()
    ))
    .unwrap()
    .0
}

fn run_isolated(test: &str) -> bool {
    const ENV: &str = "KS_APP_ISOLATED_TEST";
    if std::env::var(ENV).as_deref() == Ok(test) {
        return false;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(ENV, test)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("test result: ok. 1 passed"),
        "{test}: 자식 프로세스에서 실패했다\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    true
}

/// 결합자만 다른(보이는 모양이 같은) 남은 표시는 그 이름으로 포트를 만들거나 바꿀 때 지워진다. 보이는 모양이 다른
/// 남은 표시는 남는다.
#[gpui_kit::test]
fn visually_same_stale_mark_is_cleared_on_add_and_rename(cx: &mut TestAppContext) {
    if run_isolated("visually_same_stale_mark_is_cleared_on_add_and_rename") {
        return;
    }
    let dir = dir_for("visual-stale");
    let x = unique("stale x");
    let z = unique("stale z");
    let other = unique("other");
    cx.update(|cx| init(cx, dir.join("settings.json")));
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.midi_output.mapped_ports = vec![joined(&x), other.clone(), joined(&z)];
        });
    });
    install(cx, "visual-stale");
    let made = cx
        .update(|cx| engine::add_named_virtual_port(cx, &x))
        .unwrap();
    assert_eq!(made, x);
    assert_eq!(
        mapped_ports(cx),
        vec![other.clone(), joined(&z)],
        "보이는 모양이 같은 표시가 남았거나 다른 표시를 지웠다"
    );
    let renamed = cx
        .update(|cx| engine::rename_virtual_port(cx, &x, &z))
        .unwrap();
    assert_eq!(renamed, z);
    assert_eq!(mapped_ports(cx), vec![other.clone()]);
}

/// 살아 있는 대체 매핑 포트의 표시는 지워지지 않는다: 보이는 모양이 같은 이름으로 포트를 만들거나 바꾸는 것은
/// 거부되고, 다른 이름으로 만들면 표시가 그대로다.
#[gpui_kit::test]
fn live_mapped_mark_survives_adding_and_renaming_other_ports(cx: &mut TestAppContext) {
    if run_isolated("live_mapped_mark_survives_adding_and_renaming_other_ports") {
        return;
    }
    let dir = dir_for("visual-live");
    cx.update(|cx| init(cx, dir.join("settings.json")));
    install(cx, "visual-live");
    let device = unique("stale dev");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&device))).unwrap();
    let (_handle, _view) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    cx.run_until_parked();
    answer(cx, MAP);
    assert_eq!(mapped_ports(cx), vec![device.clone()]);
    assert!(own_ports(cx).contains(&device));
    // 보이는 모양이 같은 이름은 만들 수 없다
    assert!(
        cx.update(|cx| engine::add_named_virtual_port(cx, &joined(&device)))
            .is_err()
    );
    assert_eq!(mapped_ports(cx), vec![device.clone()]);
    // 다른 이름의 포트를 만들고 그 포트를 보이는 모양이 같은 이름으로 바꾸는 것도 거부된다
    let extra = cx
        .update(|cx| engine::add_named_virtual_port(cx, &unique("extra")))
        .unwrap();
    assert_eq!(mapped_ports(cx), vec![device.clone()]);
    assert!(
        cx.update(|cx| engine::rename_virtual_port(cx, &extra, &joined(&device)))
            .is_err()
    );
    assert_eq!(mapped_ports(cx), vec![device.clone()]);
    assert_eq!(
        cx.read(|cx| {
            AppSettings::get(cx)
                .midi_output
                .virtual_ports
                .iter()
                .filter(|n| **n == device)
                .count()
        }),
        1
    );
}
