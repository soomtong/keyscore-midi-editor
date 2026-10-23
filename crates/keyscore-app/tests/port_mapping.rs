//! S2-T7d 없는 출력 포트의 대체 매핑 제안(OUT-8, 결정 `2026-10-22-out8-port-mapping-sheet.md`).
//! 순수 계산(`port_mapping::plan`)과, 실제 CoreMIDI 엔진을 쓰는 열기·복구 시트를 본다. 가상 포트는 프로세스
//! 단위 16개 상한을 함께 쓰므로 UI 시험은 포트를 하나씩만 만든다.

use std::path::{Path, PathBuf};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, midi};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::port_mapping::{self, Line, MainReason, Mapping};
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{autosave, files, init};
use keyscore_core::{OutputPort, Project, ksq};

const MAP: &str = "제안대로 매핑";
const LEAVE: &str = "그대로 두기";

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// 트랙마다 포트 하나. 이름 포트는 JSON 문자열로 적는다(제어 문자도 넣을 수 있게).
fn project_on(ports: &[OutputPort]) -> Project {
    let tracks: Vec<String> = ports
        .iter()
        .enumerate()
        .map(|(i, port)| {
            let port = match port {
                OutputPort::Virtual => "\"virtual\"".to_owned(),
                OutputPort::Named(n) => {
                    format!("{{\"named\": {}}}", serde_json::to_string(n).unwrap())
                }
            };
            format!(r#"{{"name": "T{i}", "port": {port}, "channel": 1, "clips": []}}"#)
        })
        .collect();
    ksq::from_str(&format!(r#"{{"tracks": [{}]}}"#, tracks.join(",")))
        .unwrap()
        .0
}

fn named(n: &str) -> OutputPort {
    OutputPort::Named(n.to_owned())
}

fn line(saved: &str, mapping: Mapping, tracks: usize) -> Line {
    Line {
        saved: saved.to_owned(),
        mapping,
        tracks,
    }
}

// ---------- 순수 계산 ----------

#[test]
fn plan_missing_name_suggests_virtual_port_with_same_name() {
    let p = project_on(&[named("Synth A"), named("Synth A")]);
    assert_eq!(
        port_mapping::plan(&p, &names(&["KeyScore Out"]), &[]),
        vec![line("Synth A", Mapping::Create("Synth A".into()), 2)]
    );
}

#[test]
fn plan_skips_virtual_own_ports_and_destinations() {
    let p = project_on(&[
        OutputPort::Virtual,
        named("KeyScore Out"),
        named("Drums"),
        named("IAC Bus 1"),
    ]);
    let own = names(&["KeyScore Out", "Drums"]);
    assert_eq!(port_mapping::plan(&p, &own, &names(&["IAC Bus 1"])), vec![]);
}

#[test]
fn plan_lists_each_name_once_in_track_order() {
    let p = project_on(&[
        named("B"),
        named("A"),
        named("B"),
        OutputPort::Virtual,
        named("C"),
    ]);
    let plan = port_mapping::plan(&p, &names(&["KeyScore Out"]), &[]);
    let got: Vec<(&str, usize)> = plan.iter().map(|l| (l.saved.as_str(), l.tracks)).collect();
    assert_eq!(got, vec![("B", 2), ("A", 1), ("C", 1)]);
}

#[test]
fn plan_visually_same_port_retargets_tracks() {
    // 공백 둘·결합자가 든 이름은 앱 포트·목적지와 모양이 같다. 같은 모양의 가상 포트는 만들 수 없다.
    let p = project_on(&[named("Drums\u{200D}"), named("IAC  Bus 1")]);
    let own = names(&["KeyScore Out", "Drums"]);
    assert_eq!(
        port_mapping::plan(&p, &own, &names(&["IAC Bus 1"])),
        vec![
            line("Drums\u{200D}", Mapping::Use("Drums".into()), 1),
            line("IAC  Bus 1", Mapping::Use("IAC Bus 1".into()), 1),
        ]
    );
}

#[test]
fn plan_unnormalized_name_creates_normalized_port() {
    // NFD 「드럼」과 공백이 겹친 이름: 정규화한 이름으로 만든다. 정규화 이름이 같은 다른 이름은 그 포트로 바꾼다.
    // 이미 정규화한 이름 그대로인 트랙은 바꿀 것이 없어 줄이 없다(리뷰 1 제안 4).
    let nfd = "\u{1103}\u{1173}\u{1105}\u{1165}\u{11B7}  1";
    let wide = "\u{B4DC}\u{B7FC}   1";
    let p = project_on(&[named(nfd), named("\u{B4DC}\u{B7FC} 1"), named(wide)]);
    assert_eq!(
        port_mapping::plan(&p, &names(&["KeyScore Out"]), &[]),
        vec![
            line(nfd, Mapping::Create("\u{B4DC}\u{B7FC} 1".into()), 1),
            line(wide, Mapping::Use("\u{B4DC}\u{B7FC} 1".into()), 1),
        ]
    );
}

#[test]
fn plan_invalid_name_falls_back_to_main() {
    let p = project_on(&[named("Bad\u{7}Port"), named("   ")]);
    assert_eq!(
        port_mapping::plan(&p, &names(&["KeyScore Out"]), &[]),
        vec![
            line("Bad\u{7}Port", Mapping::Main(MainReason::InvalidName), 1),
            line("   ", Mapping::Main(MainReason::InvalidName), 1),
        ]
    );
}

#[test]
fn plan_over_limit_falls_back_to_main() {
    let mut own = names(&["KeyScore Out"]);
    own.extend((2..=15).map(|n| format!("KeyScore Out {n}")));
    assert_eq!(own.len(), 15);
    let p = project_on(&[named("X"), named("Y")]);
    assert_eq!(
        port_mapping::plan(&p, &own, &[]),
        vec![
            line("X", Mapping::Create("X".into()), 1),
            line("Y", Mapping::Main(MainReason::TooMany), 1),
        ]
    );
}

#[test]
fn describe_names_action_and_track_count() {
    let main = "KeyScore Out";
    let create =
        port_mapping::describe(&line("Synth A", Mapping::Create("Synth A".into()), 2), main);
    assert!(
        create.contains("「Synth A」") && create.contains("같은 이름의 가상 포트"),
        "{create}"
    );
    assert!(create.contains("트랙 2개"), "{create}");
    let renamed = port_mapping::describe(
        &line("Synth  A", Mapping::Create("Synth A".into()), 1),
        main,
    );
    assert!(renamed.contains("가상 포트 「Synth A」"), "{renamed}");
    assert!(renamed.contains("트랙 1개"), "{renamed}");
    let used = port_mapping::describe(&line("IAC  Bus", Mapping::Use("IAC Bus".into()), 3), main);
    assert!(
        used.contains("「IAC Bus」") && used.contains("트랙 3개"),
        "{used}"
    );
    let invalid = port_mapping::describe(
        &line("Bad\u{7}", Mapping::Main(MainReason::InvalidName), 1),
        main,
    );
    // 제어 문자는 보이게 적는다(시트 글자가 깨지지 않게)
    assert!(
        !invalid.contains('\u{7}') && invalid.contains("Bad\\u{7}"),
        "{invalid}"
    );
    assert!(invalid.contains("기본 포트 「KeyScore Out」"), "{invalid}");
    let many = port_mapping::describe(&line("Y", Mapping::Main(MainReason::TooMany), 1), main);
    assert!(many.contains("16") && many.contains("기본 포트"), "{many}");
}

// ---------- 창(CoreMIDI) ----------

fn unique(tag: &str) -> String {
    format!("KS t7d {} {tag}", std::process::id())
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-t7d-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// 설정과 엔진을 준비하고 (폴더, 기본 포트 이름)을 돌려준다.
fn start(cx: &mut TestAppContext, tag: &str) -> (PathBuf, String) {
    let dir = temp_dir(tag);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let main = unique(tag);
    let options = EngineOptions {
        client_name: format!("ks-t7d-{tag}"),
        port_name: main.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    (dir, main)
}

fn write_project(path: &Path, p: &Project) {
    std::fs::write(path, ksq::to_string(p)).unwrap();
}

fn open_file(cx: &mut TestAppContext, path: &Path) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| files::open_path(path, cx)).unwrap();
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

fn prompt_text(cx: &mut TestAppContext) -> String {
    let (msg, detail) = cx.pending_prompt().expect("시트가 없다");
    format!("{msg}\n{detail}")
}

fn answer(cx: &mut TestAppContext, button: &str) {
    assert!(cx.has_pending_prompt(), "시트가 없다");
    cx.simulate_prompt_answer(button);
    cx.run_until_parked();
}

fn ports_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<OutputPort> {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .project()
            .tracks()
            .iter()
            .map(|t| t.port().clone())
            .collect()
    })
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
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

fn own_ports(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(engine::virtual_port_names)
}

fn saved_virtual_ports(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(|cx| AppSettings::get(cx).midi_output.virtual_ports.clone())
}

#[gpui_kit::test]
fn open_file_with_missing_port_offers_sheet_and_creates_port(cx: &mut TestAppContext) {
    let (dir, _) = start(cx, "open");
    let missing = unique("open missing");
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[named(&missing), OutputPort::Virtual]));
    let (_, view) = open_file(cx, &path);
    let text = prompt_text(cx);
    assert!(text.contains("없는 출력 포트"), "{text}");
    assert!(text.contains(&format!("「{missing}」")), "{text}");
    answer(cx, MAP);
    assert!(own_ports(cx).contains(&missing), "{:?}", own_ports(cx));
    assert_eq!(saved_virtual_ports(cx), vec![missing.clone()]);
    // 트랙은 고치지 않는다(원래 Mac에서 열어도 맞게). 문서도 바뀌지 않는다
    assert_eq!(
        ports_of(cx, &view),
        vec![named(&missing), OutputPort::Virtual]
    );
    assert!(!dirty(cx, &view));
    let said = messages(cx, &view).join("\n");
    assert!(said.contains("가상 포트 1개"), "{said}");
    // 다시 물어도 없는 포트가 없다
    cx.update(|cx| {
        let project = view.read(cx).document().read(cx).project().clone();
        assert_eq!(port_mapping::plan_for(cx, &project), Some(vec![]));
    });
}

#[gpui_kit::test]
fn leave_as_is_changes_nothing(cx: &mut TestAppContext) {
    let (dir, _) = start(cx, "leave");
    let missing = unique("leave missing");
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[named(&missing)]));
    let (_, view) = open_file(cx, &path);
    let before = own_ports(cx);
    answer(cx, LEAVE);
    assert_eq!(own_ports(cx), before);
    assert!(saved_virtual_ports(cx).is_empty());
    assert_eq!(ports_of(cx, &view), vec![named(&missing)]);
    assert!(!dirty(cx, &view));
    assert!(!cx.has_pending_prompt());
}

#[gpui_kit::test]
fn open_file_without_missing_port_has_no_sheet(cx: &mut TestAppContext) {
    let (dir, main) = start(cx, "none");
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[OutputPort::Virtual, named(&main)]));
    let _ = open_file(cx, &path);
    assert!(!cx.has_pending_prompt());
}

#[gpui_kit::test]
fn retargeted_tracks_are_one_undo_unit(cx: &mut TestAppContext) {
    let (dir, main) = start(cx, "undo");
    // 기본 포트와 모양이 같은 이름(공백 둘)과 가상 포트로 만들 수 없는 이름
    let spaced = main.replacen(' ', "  ", 1);
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[named(&spaced), named("Bad\u{7}Port")]));
    let (handle, view) = open_file(cx, &path);
    let text = prompt_text(cx);
    assert!(text.contains("Bad\\u{7}Port"), "{text}");
    let before = own_ports(cx);
    answer(cx, MAP);
    assert_eq!(own_ports(cx), before, "포트를 만들지 않는다");
    assert_eq!(ports_of(cx, &view), vec![named(&main), OutputPort::Virtual]);
    assert!(dirty(cx, &view));
    let said = messages(cx, &view).join("\n");
    assert!(said.contains("트랙 2개"), "{said}");
    run(cx, handle, edit::Undo);
    assert_eq!(
        ports_of(cx, &view),
        vec![named(&spaced), named("Bad\u{7}Port")]
    );
}

#[gpui_kit::test]
fn map_missing_ports_action_reopens_sheet_or_says_none(cx: &mut TestAppContext) {
    let (dir, _) = start(cx, "action");
    let missing = unique("action missing");
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[named(&missing)]));
    let (handle, view) = open_file(cx, &path);
    answer(cx, LEAVE);
    run(cx, handle, midi::MapMissingPorts);
    assert!(prompt_text(cx).contains(&missing));
    answer(cx, MAP);
    assert!(own_ports(cx).contains(&missing));
    run(cx, handle, midi::MapMissingPorts);
    assert!(!cx.has_pending_prompt());
    let said = messages(cx, &view).join("\n");
    assert!(said.contains("없는 출력 포트가 없다"), "{said}");
}

#[gpui_kit::test]
fn apply_recomputes_when_port_appeared_meanwhile(cx: &mut TestAppContext) {
    let (dir, _) = start(cx, "meanwhile");
    let missing = unique("meanwhile missing");
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[named(&missing)]));
    let (_, view) = open_file(cx, &path);
    assert!(cx.has_pending_prompt());
    // 시트가 떠 있는 동안 다른 경로(다른 창의 매핑, 설정)로 같은 포트가 생겼다
    cx.update(|cx| engine::add_named_virtual_port(cx, &missing))
        .unwrap();
    answer(cx, MAP);
    let own = own_ports(cx);
    assert_eq!(own.iter().filter(|n| **n == missing).count(), 1, "{own:?}");
    assert_eq!(ports_of(cx, &view), vec![named(&missing)]);
    let said = messages(cx, &view).join("\n");
    // 다시 계산하면 없는 포트가 없다: 만들려다 실패하지 않고 그렇다고 알린다(리뷰 1 제안 5)
    assert!(!said.contains("만들지 못한"), "{said}");
    assert!(said.contains("없는 출력 포트가 없다"), "{said}");
}

#[gpui_kit::test]
fn no_engine_no_sheet(cx: &mut TestAppContext) {
    let dir = temp_dir("no-engine");
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[named("Somewhere")]));
    let (handle, view) = open_file(cx, &path);
    assert!(!cx.has_pending_prompt());
    run(cx, handle, midi::MapMissingPorts);
    assert!(!cx.has_pending_prompt());
    let said = messages(cx, &view).join("\n");
    assert!(said.contains("MIDI 엔진이 없다"), "{said}");
}

#[gpui_kit::test]
fn recovered_window_offers_sheet(cx: &mut TestAppContext) {
    let (_, _) = start(cx, "recover");
    let missing = unique("recover missing");
    let dir = cx.read(autosave::autosave_dir);
    std::fs::create_dir_all(&dir).unwrap();
    let key = "dead-key";
    std::fs::write(
        dir.join(format!("{key}.ksq")),
        ksq::to_string(&project_on(&[named(&missing)])),
    )
    .unwrap();
    let meta = serde_json::json!({
        "path": null, "read_only": null, "session": format!("dead-{key}"), "pid": 999_999,
    });
    std::fs::write(dir.join(format!("{key}.json")), meta.to_string()).unwrap();
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    answer(cx, "복구");
    let text = prompt_text(cx);
    assert!(text.contains(&format!("「{missing}」")), "{text}");
    answer(cx, MAP);
    assert!(own_ports(cx).contains(&missing));
    let windows = cx.read(open_project_windows);
    assert!(
        windows
            .iter()
            .any(|v| ports_of(cx, v) == vec![named(&missing)]),
        "복구한 창이 없다"
    );
}

// ---------- 같은 이름의 장치가 돌아올 때(리뷰 1 지적 1, 결정 3 A) ----------

/// 다른 앱(또는 다시 꽂은 USB 장치)의 입력처럼 보이는 목적지.
struct Device {
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

impl Device {
    fn new(name: &str) -> Device {
        let client = coremidi::Client::new(&format!("ks-t7d-dev-{name}")).unwrap();
        let dest = client
            .virtual_destination(name, |_: &coremidi::PacketList| {})
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !keyscore_midi::destinations()
            .iter()
            .any(|d| d.name() == name)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "목적지 {name}이 보이지 않는다"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Device {
            _dest: dest,
            _client: client,
        }
    }
}

fn mapped_ports(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(|cx| AppSettings::get(cx).midi_output.mapped_ports.clone())
}

/// 감시 주기만큼 시계를 돌리며 조건을 기다린다(CoreMIDI 알림은 실제 시간으로 온다).
fn pump(cx: &mut TestAppContext, done: impl Fn(&mut TestAppContext) -> bool, why: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    loop {
        cx.executor()
            .advance_clock(keyscore_app::ports::POLL_INTERVAL);
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "{why}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[gpui_kit::test]
fn returning_device_removes_mapped_port(cx: &mut TestAppContext) {
    let (dir, _) = start(cx, "return");
    cx.update(keyscore_app::ports::start_monitor);
    let device = unique("return synth");
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[named(&device)]));
    let (_, view) = open_file(cx, &path);
    answer(cx, MAP);
    assert_eq!(mapped_ports(cx), vec![device.clone()]);
    let _back = Device::new(&device);
    pump(
        cx,
        |cx| !own_ports(cx).contains(&device),
        "장치가 돌아와도 대체 가상 포트가 남았다",
    );
    assert!(saved_virtual_ports(cx).is_empty());
    assert!(mapped_ports(cx).is_empty());
    // 트랙은 이름 그대로 장치를 가리킨다
    assert_eq!(ports_of(cx, &view), vec![named(&device)]);
    let said = messages(cx, &view).join("\n");
    assert!(
        said.contains("돌아와 대체로 만든 가상 포트"),
        "{said}"
    );
}

#[gpui_kit::test]
fn returning_device_keeps_user_and_renamed_ports(cx: &mut TestAppContext) {
    let (dir, _) = start(cx, "keep");
    cx.update(keyscore_app::ports::start_monitor);
    // 사용자가 직접 만든 포트(S2-T6d 규칙: 앱 포트가 이긴다)
    let user = unique("keep user");
    cx.update(|cx| engine::add_named_virtual_port(cx, &user))
        .unwrap();
    // 매핑으로 만들었지만 사용자가 이름을 바꾼 포트
    let mapped = unique("keep mapped");
    let path = dir.join("song.ksq");
    write_project(&path, &project_on(&[named(&mapped)]));
    let _ = open_file(cx, &path);
    answer(cx, MAP);
    let renamed = unique("keep renamed");
    cx.update(|cx| engine::rename_virtual_port(cx, &mapped, &renamed))
        .unwrap();
    assert!(mapped_ports(cx).is_empty(), "이름을 바꾸면 표시를 지운다");
    let _a = Device::new(&user);
    let _b = Device::new(&renamed);
    for _ in 0..10 {
        cx.executor()
            .advance_clock(keyscore_app::ports::POLL_INTERVAL);
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let own = own_ports(cx);
    assert!(own.contains(&user) && own.contains(&renamed), "{own:?}");
}

#[gpui_kit::test]
fn startup_drops_mapped_port_whose_device_is_present(cx: &mut TestAppContext) {
    let dir = temp_dir("startup");
    let device = unique("startup synth");
    let other = unique("startup other");
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({"midi_output": {
            "virtual_ports": [device, other], "mapped_ports": [device, other]}})
        .to_string(),
    )
    .unwrap();
    let _back = Device::new(&device);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let options = EngineOptions {
        client_name: "ks-t7d-startup".to_owned(),
        port_name: unique("startup main"),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let own = own_ports(cx);
    assert!(!own.contains(&device) && own.contains(&other), "{own:?}");
    assert_eq!(saved_virtual_ports(cx), vec![other.clone()]);
    assert_eq!(mapped_ports(cx), vec![other]);
}

#[test]
fn settings_keep_only_mapped_names_that_are_virtual_ports() {
    let dir = temp_dir("settings");
    let path = dir.join("settings.json");
    std::fs::write(
        &path,
        r#"{"midi_output": {"virtual_ports": ["A", "B"], "mapped_ports": ["B", "C", 3, "B"]}}"#,
    )
    .unwrap();
    let (settings, warnings) = keyscore_app::settings::Settings::load(&path);
    assert_eq!(settings.midi_output.mapped_ports, vec!["B".to_owned()]);
    assert_eq!(warnings.len(), 3, "{warnings:?}");
}
