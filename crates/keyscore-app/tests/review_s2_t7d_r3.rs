//! S2-T7d 적대적 리뷰 3회차 수용 시험(`docs/reports/s2-t7d/review-3.md`). CoreMIDI 목적지를 만들거나 포트
//! 감시를 돌리는 시험은 목적지 목록·16개 상한을 다른 시험과 나누지 않게 자식 프로세스에서 혼자 돈다.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{files, init};
use keyscore_core::{OutputPort, Project, ksq};

use gpui_kit::test::TestWindowExt as _;

const MAP: &str = "제안대로 매핑";
const WAIT: Duration = Duration::from_secs(2);

fn unique(tag: &str) -> String {
    format!("KS rv7d3 {} {tag}", std::process::id())
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv7d3-{}-{tag}", std::process::id()));
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
        client_name: format!("ks-rv7d3-{tag}"),
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
        let client = coremidi::Client::new(&format!("ks-rv7d3-dest-{name}")).unwrap();
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
/// 저장된 이름(두 칸 공백)과 시트가 만들 정규화 이름.
fn wide(tag: &str) -> (String, String) {
    let normal = unique(tag);
    let saved = normal.replacen(' ', "  ", 1);
    (saved, normal)
}

// ---------- 여러 창, 장치 둘, 재생 중 ----------

/// 창 A는 정규화된 이름 둘(장치 둘)을 매핑하고, 창 B는 같은 장치 하나를 쓰는 파일이라 시트가 A가 만든 대체 포트를
/// 쓰자고 한다. A를 재생하는 중에 장치 하나만 원래 이름으로 돌아온다. 그 장치의 대체 포트만 지워지고, 두 창의 그
/// 트랙만 장치 이름으로 바뀌고, 다른 대체 포트와 그 트랙은 그대로이며, 돌아온 장치에 노트와 프로그램이 간다.
#[gpui_kit::test]
fn review3_two_windows_two_devices_during_playback(cx: &mut TestAppContext) {
    if run_isolated("review3_two_windows_two_devices_during_playback") {
        return;
    }
    let dir = start(cx, "two");
    let (s1, n1) = wide("two synth a");
    let (s2, n2) = wide("two synth b");
    let a_path = dir.join("a.ksq");
    let b_path = dir.join("b.ksq");
    std::fs::write(
        &a_path,
        ksq::to_string(&project_on(&[named(&s1), named(&s2)])),
    )
    .unwrap();
    std::fs::write(&b_path, ksq::to_string(&project_on(&[named(&s1)]))).unwrap();
    let (a, a_view) = open_file(cx, &a_path);
    answer(cx, MAP);
    assert_eq!(ports_of(cx, &a_view), vec![named(&n1), named(&n2)]);
    let (b, b_view) = open_file(cx, &b_path);
    answer(cx, MAP);
    assert_eq!(ports_of(cx, &b_view), vec![named(&n1)]);
    run(cx, a, transport::Play);
    std::thread::sleep(Duration::from_millis(300));
    let back = Destination::new(&s1);
    let got = pump_collect(cx, &back.rx, Duration::from_millis(1500));
    let mapped: Vec<String> = cx.read(|cx| AppSettings::get(cx).midi_output.mapped_ports.clone());
    let notes = count(&got, 0x90);
    let programs = count(&got, 0xC0);
    eprintln!(
        "own: {:?}\nmapped: {mapped:?}\nA: {:?}\nB: {:?}\nnotes {notes} programs {programs}\nsaid A: {:?}",
        own_ports(cx),
        ports_of(cx, &a_view),
        ports_of(cx, &b_view),
        messages(cx, &a_view)
    );
    assert!(
        !own_ports(cx).contains(&n1),
        "돌아온 장치의 대체 포트가 남았다"
    );
    assert!(
        own_ports(cx).contains(&n2),
        "다른 장치의 대체 포트를 지웠다"
    );
    assert_eq!(mapped, vec![n2.clone()]);
    assert_eq!(ports_of(cx, &a_view), vec![named(&s1), named(&n2)]);
    assert_eq!(ports_of(cx, &b_view), vec![named(&s1)]);
    assert!(
        notes > 0 && programs > 0,
        "돌아온 장치에 노트·프로그램이 가지 않는다"
    );
    // 관찰: 되돌린 편집을 undo하면 지운 대체 포트 이름으로 돌아간다(제안)
    run(cx, b, edit::Undo);
    let choices = cx.read(engine::port_choices);
    eprintln!(
        "B after undo: {:?}\nchoices has n1: {}\nchoices has s1: {}",
        ports_of(cx, &b_view),
        choices.contains(&named(&n1)),
        choices.contains(&named(&s1))
    );
    // 다른 장치도 돌아온다
    let back2 = Destination::new(&s2);
    let got2 = pump_collect(cx, &back2.rx, Duration::from_millis(1500));
    run(cx, a, transport::Stop);
    eprintln!("A: {:?}, own: {:?}", ports_of(cx, &a_view), own_ports(cx));
    assert_eq!(ports_of(cx, &a_view), vec![named(&s1), named(&s2)]);
    assert!(!own_ports(cx).contains(&n2));
    assert!(count(&got2, 0x90) > 0 && count(&got2, 0xC0) > 0);
}

// ---------- 트랙 밖의 포트(메트로놈, 클럭) ----------

/// 메트로놈·클럭 포트로 정규화 이름의 대체 포트를 골라 두었다. 장치가 원래 이름으로 돌아오면 트랙은 장치 이름으로
/// 바뀌지만 메트로놈·클럭 설정은 지운 대체 포트 이름에 남는다(이름 변경 `rename_virtual_port`는 둘 다 고친다).
/// 트랙만 단언하고 메트로놈·클럭은 관찰로 남긴다(리뷰 3 제안 1).
#[gpui_kit::test]
fn review3_metronome_and_clock_on_mapped_port_follow_device(cx: &mut TestAppContext) {
    if run_isolated("review3_metronome_and_clock_on_mapped_port_follow_device") {
        return;
    }
    let dir = start(cx, "met");
    let (saved, normal) = wide("met synth");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&saved)]))).unwrap();
    let (_handle, view) = open_file(cx, &path);
    answer(cx, MAP);
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.metronome.port = named(&normal);
            s.midi_output.clock = vec![named(&normal)];
        });
    });
    let back = Destination::new(&saved);
    let _ = pump_collect(cx, &back.rx, Duration::from_millis(600));
    let (met, clock) = cx.read(|cx| {
        let s = AppSettings::get(cx);
        (s.metronome.port.clone(), s.midi_output.clock.clone())
    });
    eprintln!(
        "own: {:?}\ntrack: {:?}\nmetronome: {met:?}\nclock: {clock:?}",
        own_ports(cx),
        ports_of(cx, &view)
    );
    assert_eq!(ports_of(cx, &view), vec![named(&saved)]);
    // S2-T7e가 고쳤다(리뷰 3 제안 1): 두 설정도 장치 이름을 따른다
    assert!(
        met != named(&normal) && !clock.contains(&named(&normal)),
        "메트로놈·클럭 설정이 지운 대체 포트 이름에 남았다"
    );
}

// ---------- 시작 경로 뒤 닫힌 파일 ----------

/// 시작 때 장치(원래 이름)가 이미 있어 대체 포트(정규화 이름)를 설정에서 뺐다. 그 이름을 쓰는 파일을 열면 시트가
/// 장치로 바꾸자고 하고, 받아들이면 트랙이 장치로 간다.
#[gpui_kit::test]
fn review3_startup_release_then_open_offers_device(cx: &mut TestAppContext) {
    if run_isolated("review3_startup_release_then_open_offers_device") {
        return;
    }
    let dir = temp_dir("startup-open");
    let (saved, normal) = wide("startup open");
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
        client_name: "ks-rv7d3-startup".to_owned(),
        port_name: unique("startup main"),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    cx.update(keyscore_app::ports::start_monitor);
    assert!(!own_ports(cx).contains(&normal));
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&normal)]))).unwrap();
    let (_handle, view) = open_file(cx, &path);
    answer(cx, MAP);
    assert_eq!(ports_of(cx, &view), vec![named(&saved)]);
}
