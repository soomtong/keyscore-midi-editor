//! S2-T7d 적대적 리뷰 2회차 수용 시험(`docs/reports/s2-t7d/review-2.md`). CoreMIDI 목적지를 만들거나 포트
//! 감시를 돌리는 시험은 목적지 목록·16개 상한을 다른 시험과 나누지 않게 자식 프로세스에서 혼자 돈다.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{files, init};
use keyscore_core::{OutputPort, Project, ksq};

use gpui_kit::test::TestWindowExt as _;

const MAP: &str = "제안대로 매핑";
const LEAVE: &str = "그대로 두기";
const WAIT: Duration = Duration::from_secs(2);

fn unique(tag: &str) -> String {
    format!("KS rv7d2 {} {tag}", std::process::id())
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv7d2-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// 트랙마다 포트 하나, 프로그램 5. 각 트랙에 박마다 노트가 든 긴 클립(재생 시험용).
fn project_on(ports: &[OutputPort]) -> Project {
    let notes: Vec<String> = (0..64)
        .map(|i| format!("[{}, 240, 60, 100]", if i == 0 { 0 } else { 960 }))
        .collect();
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
            format!(
                r#"{{"name": "T{i}", "port": {port}, "channel": 1, "program": 5,
                    "clips": [{{"start": 0, "length": 61440, "notes": [{}]}}]}}"#,
                notes.join(", ")
            )
        })
        .collect();
    ksq::from_str(&format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(",")
    ))
    .unwrap()
    .0
}

fn named(n: &str) -> OutputPort {
    OutputPort::Named(n.to_owned())
}

fn start(cx: &mut TestAppContext, tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let options = EngineOptions {
        client_name: format!("ks-rv7d2-{tag}"),
        port_name: unique(&format!("main {tag}")),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    cx.update(keyscore_app::ports::start_monitor);
    dir
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

fn split(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let len = match data[i] & 0xF0 {
            0xC0 | 0xD0 => 2,
            0xF0 => 1,
            _ => 3,
        };
        out.push(data[i..(i + len).min(data.len())].to_vec());
        i += len;
    }
    out
}

/// 다시 꽂은 USB 장치처럼 보이는 목적지. 받은 메시지를 모은다.
struct Destination {
    rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

impl Destination {
    fn new(name: &str) -> Destination {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new(&format!("ks-rv7d2-dest-{name}")).unwrap();
        let dest = client
            .virtual_destination(name, move |list: &coremidi::PacketList| {
                for p in list {
                    for msg in split(p.data()) {
                        let _ = tx.send(msg);
                    }
                }
            })
            .unwrap();
        let deadline = Instant::now() + WAIT;
        while !keyscore_midi::destinations()
            .iter()
            .any(|d| d.name() == name)
        {
            assert!(Instant::now() < deadline, "목적지 {name}이 보이지 않는다");
            std::thread::sleep(Duration::from_millis(10));
        }
        Destination {
            rx,
            _dest: dest,
            _client: client,
        }
    }
}

/// 포트 감시 주기만큼 시계를 돌리며 실제 시간 `limit` 동안 받은 메시지를 모은다.
fn pump_collect(
    cx: &mut TestAppContext,
    rx: &mpsc::Receiver<Vec<u8>>,
    limit: Duration,
) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        cx.executor()
            .advance_clock(keyscore_app::ports::POLL_INTERVAL);
        cx.run_until_parked();
        while let Ok(m) = rx.try_recv() {
            got.push(m);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    got
}

fn count(got: &[Vec<u8>], status: u8) -> usize {
    got.iter().filter(|m| m.first() == Some(&status)).count()
}

// ---------- 재생 중 돌아온 장치의 음색 ----------

/// 재생 중에 장치가 돌아오면 `release_mapped`가 대체 포트를 지우고 `remove_virtual_port`의 다시 준비로 트랙을
/// 장치에 잇는다. 그런데 그 다시 준비는 창의 음색 기록(`transport.setup`, 목적지 이름이 열쇠)을 그대로 둔다.
/// 대체 포트에 이미 보낸 프로그램이 「같은 목적지에 보낸 것」으로 남아, 돌아온 장치에는 프로그램 체인지가 가지
/// 않는다. 일반 재연결(`transport_port_returned`)은 그 기록을 지워 다시 보낸다(대조군).
#[gpui_kit::test]
fn review2_returning_device_during_playback_gets_program(cx: &mut TestAppContext) {
    if run_isolated("review2_returning_device_during_playback_gets_program") {
        return;
    }
    let dir = start(cx, "program");
    let device = unique("program synth");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&device)]))).unwrap();
    let (handle, view) = open_file(cx, &path);
    answer(cx, MAP);
    assert!(own_ports(cx).contains(&device));
    run(cx, handle, transport::Play);
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(300));
    let back = Destination::new(&device);
    let got = pump_collect(cx, &back.rx, Duration::from_millis(1500));
    run(cx, handle, transport::Stop);
    let notes = count(&got, 0x90);
    let programs = count(&got, 0xC0);
    eprintln!(
        "own: {:?}\nnotes: {notes}, programs: {programs}\nsaid: {:?}",
        own_ports(cx),
        messages(cx, &view)
    );
    assert!(!own_ports(cx).contains(&device), "대체 포트가 남았다");
    assert!(notes > 0, "돌아온 장치로 노트가 가지 않는다");
    assert!(
        programs > 0,
        "돌아온 장치에 트랙의 프로그램(5)이 가지 않아 다른 음색으로 소리 난다"
    );
}

/// 대조군: 매핑하지 않은 트랙의 장치가 재생 중에 돌아오면(OUT-3 일반 재연결) 프로그램을 다시 보낸다.
#[gpui_kit::test]
fn review2_control_plain_reconnect_during_playback_gets_program(cx: &mut TestAppContext) {
    if run_isolated("review2_control_plain_reconnect_during_playback_gets_program") {
        return;
    }
    let dir = start(cx, "program-control");
    let device = unique("program control");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&device)]))).unwrap();
    let (handle, _view) = open_file(cx, &path);
    answer(cx, LEAVE);
    run(cx, handle, transport::Play);
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(300));
    let back = Destination::new(&device);
    let got = pump_collect(cx, &back.rx, Duration::from_millis(1500));
    run(cx, handle, transport::Stop);
    let notes = count(&got, 0x90);
    let programs = count(&got, 0xC0);
    eprintln!("notes: {notes}, programs: {programs}");
    assert!(notes > 0 && programs > 0, "대조군이 성립하지 않는다");
}

// ---------- 정규화로 이름이 바뀐 매핑 ----------

/// 저장된 이름이 정규화되지 않은 장치 이름(두 칸 공백, 뒤 공백 등)이면 시트는 정규화한 이름의 가상 포트를 만들고
/// 트랙을 그 이름으로 바꾼다. 그 장치가 원래 이름 그대로 돌아오면 `release_mapped`(정확히 같은 이름만 본다)가
/// 대체 포트를 지우지 않는다. 대체 포트는 보이는 모양이 같아 `port_choices`가 장치를 메뉴에서 빼고, 트랙은 대체
/// 포트로 간다. 결정 3 A 「같은 이름의 목적지가 나타나면 지우고 트랙은 장치로 다시 이어진다」가 성립하지 않는다.
#[gpui_kit::test]
fn review2_unnormalized_device_name_returns(cx: &mut TestAppContext) {
    if run_isolated("review2_unnormalized_device_name_returns") {
        return;
    }
    let dir = start(cx, "wide");
    let normal = unique("wide synth");
    let saved = normal.replacen(' ', "  ", 1);
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&saved)]))).unwrap();
    let (handle, view) = open_file(cx, &path);
    answer(cx, MAP);
    assert!(own_ports(cx).contains(&normal));
    assert_eq!(ports_of(cx, &view), vec![named(&normal)]);
    let mapped: Vec<String> = cx.read(|cx| AppSettings::get(cx).midi_output.mapped_ports.clone());
    assert_eq!(mapped, vec![normal.clone()]);
    // 같은 장치가 원래 이름 그대로 돌아왔다
    let back = Destination::new(&saved);
    let _ = pump_collect(cx, &back.rx, Duration::from_millis(600));
    let choices = cx.read(engine::port_choices);
    run(cx, handle, transport::Play);
    let got = pump_collect(cx, &back.rx, Duration::from_millis(1200));
    run(cx, handle, transport::Stop);
    let notes = count(&got, 0x90);
    eprintln!(
        "own: {:?}\nchoices: {choices:?}\nports: {:?}\nnotes to device: {notes}\nsaid: {:?}",
        own_ports(cx),
        ports_of(cx, &view),
        messages(cx, &view)
    );
    assert!(
        choices.contains(&named(&saved)) || !own_ports(cx).contains(&normal),
        "돌아온 장치가 포트 메뉴에 없고 대체 포트도 남았다"
    );
    assert!(notes > 0, "돌아온 장치로 노트가 가지 않는다");
}

/// 같은 경우의 시작 경로: 설정에 대체 포트(정규화 이름)가 있고 장치가 원래 이름으로 이미 있으면, `install`은
/// 정확히 같은 이름만 「돌아왔다」로 보고 빼므로 이 포트는 「목적지 이름이라 만들지 않았다」로 건너뛰고 설정·표시는
/// 남긴다. 트랙(정규화 이름)은 「포트 없음」이고 장치는 메뉴에 보인다.
#[gpui_kit::test]
fn review2_startup_unnormalized_device_keeps_stale_mapping(cx: &mut TestAppContext) {
    if run_isolated("review2_startup_unnormalized_device_keeps_stale_mapping") {
        return;
    }
    let dir = temp_dir("startup-wide");
    let normal = unique("startup wide");
    let saved = normal.replacen(' ', "  ", 1);
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({"midi_output": {
            "virtual_ports": [normal], "mapped_ports": [normal]}})
        .to_string(),
    )
    .unwrap();
    let _back = Destination::new(&saved);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let options = EngineOptions {
        client_name: "ks-rv7d2-startup".to_owned(),
        port_name: unique("startup main"),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let own = own_ports(cx);
    let (virtual_ports, mapped) = cx.read(|cx| {
        let m = &AppSettings::get(cx).midi_output;
        (m.virtual_ports.clone(), m.mapped_ports.clone())
    });
    eprintln!("own: {own:?}\nvirtual_ports: {virtual_ports:?}\nmapped: {mapped:?}");
    assert!(
        !mapped.contains(&normal),
        "장치가 있는데 대체 포트 표시가 설정에 남았다(포트는 만들지 않았다: {own:?})"
    );
}
