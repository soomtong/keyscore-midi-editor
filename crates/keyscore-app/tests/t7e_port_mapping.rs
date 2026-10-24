//! S2-T7e: S2-T7d 리뷰 3 이월(`docs/reports/s2-t7d/review-3.md`). 대체 매핑 포트의 메트로놈·클럭 설정, 보이는
//! 모양이 같은 장치 여럿, 설정에 남은 대체 표시. CoreMIDI 목적지를 만들거나 포트 감시를 돌리는 시험은 목적지
//! 목록·16개 상한을 다른 시험과 나누지 않게 자식 프로세스에서 혼자 돈다.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{files, init};
use keyscore_core::{OutputPort, Project, ksq};

const MAP: &str = "제안대로 매핑";
const WAIT: Duration = Duration::from_secs(2);

fn unique(tag: &str) -> String {
    format!("KS t7e {} {tag}", std::process::id())
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-t7e-ports-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// 트랙마다 포트 하나, 프로그램 5, 박마다 노트가 든 긴 클립.
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

fn install(cx: &mut TestAppContext, tag: &str) {
    let options = EngineOptions {
        client_name: format!("ks-t7e-{tag}"),
        port_name: unique(&format!("main {tag}")),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    cx.update(keyscore_app::ports::start_monitor);
}

fn start(cx: &mut TestAppContext, tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    install(cx, tag);
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

fn own_ports(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(engine::virtual_port_names)
}

fn metronome_and_clock(cx: &mut TestAppContext) -> (OutputPort, Vec<OutputPort>) {
    cx.read(|cx| {
        let s = AppSettings::get(cx);
        (s.metronome.port.clone(), s.midi_output.clock.clone())
    })
}

fn mapped_ports(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(|cx| AppSettings::get(cx).midi_output.mapped_ports.clone())
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

/// 다시 꽂은 USB 장치처럼 보이는 목적지.
struct Destination {
    _rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

impl Destination {
    fn new(name: &str) -> Destination {
        Destination::with_unique_id(name, None)
    }

    /// 고유 ID를 정해 만든다(보이는 모양이 같은 장치 둘의 순서를 시험이 정하려고).
    fn with_unique_id(name: &str, unique_id: Option<i32>) -> Destination {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new(&format!("ks-t7e-dest-{name}")).unwrap();
        let dest = client
            .virtual_destination(name, move |list: &coremidi::PacketList| {
                for p in list {
                    let _ = tx.send(p.data().to_vec());
                }
            })
            .unwrap();
        if let Some(id) = unique_id {
            dest.set_property(&coremidi::Properties::unique_id(), id)
                .unwrap();
        }
        let deadline = Instant::now() + WAIT;
        while !keyscore_midi::destinations()
            .iter()
            .any(|d| d.name() == name)
        {
            assert!(Instant::now() < deadline, "목적지 {name}이 보이지 않는다");
            std::thread::sleep(Duration::from_millis(10));
        }
        Destination {
            _rx: rx,
            _dest: dest,
            _client: client,
        }
    }
}

/// 포트 감시 주기만큼 시계를 돌리며 실제 시간 `limit` 동안 기다린다.
fn pump(cx: &mut TestAppContext, limit: Duration) {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        cx.executor()
            .advance_clock(keyscore_app::ports::POLL_INTERVAL);
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 저장된 이름(두 칸 공백)과 시트가 만들 정규화 이름.
fn wide(tag: &str) -> (String, String) {
    let normal = unique(tag);
    let saved = normal.replacen(' ', "  ", 1);
    (saved, normal)
}

// ---------- C1. 메트로놈·클럭 설정이 장치를 따른다 ----------

/// 메트로놈·클럭 포트로 정규화 이름의 대체 포트를 골라 두었다. 장치가 원래 이름으로 돌아오면 트랙처럼 두 설정도
/// 장치 이름이 된다(리뷰 3 제안 1).
#[gpui_kit::test]
fn metronome_and_clock_on_mapped_port_follow_device(cx: &mut TestAppContext) {
    if run_isolated("metronome_and_clock_on_mapped_port_follow_device") {
        return;
    }
    let dir = start(cx, "met");
    let (saved, normal) = wide("met synth");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&saved)]))).unwrap();
    let (_handle, view) = open_file(cx, &path);
    answer(cx, MAP);
    assert_eq!(ports_of(cx, &view), vec![named(&normal)]);
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.metronome.port = named(&normal);
            s.midi_output.clock = vec![named(&normal)];
        });
    });
    let _back = Destination::new(&saved);
    pump(cx, Duration::from_millis(600));
    assert_eq!(ports_of(cx, &view), vec![named(&saved)]);
    assert!(!own_ports(cx).contains(&normal), "대체 포트가 남았다");
    let (met, clock) = metronome_and_clock(cx);
    assert_eq!(
        met,
        named(&saved),
        "메트로놈 포트가 지운 대체 포트에 남았다"
    );
    assert_eq!(
        clock,
        vec![named(&saved)],
        "클럭 포트가 지운 대체 포트에 남았다"
    );
}

// ---------- C2. 시작 때 장치가 이미 있을 때 ----------

/// 시작 때 장치(원래 이름)가 이미 있어 대체 포트(정규화 이름)를 설정에서 뺀다. 그 이름을 가리키던 메트로놈·클럭
/// 설정도 장치 이름으로 고친다.
#[gpui_kit::test]
fn startup_release_fixes_metronome_and_clock(cx: &mut TestAppContext) {
    if run_isolated("startup_release_fixes_metronome_and_clock") {
        return;
    }
    let dir = temp_dir("startup-met");
    let (saved, normal) = wide("startup met");
    let _back = Destination::new(&saved);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.midi_output.virtual_ports = vec![normal.clone()];
            s.midi_output.mapped_ports = vec![normal.clone()];
            s.midi_output.clock = vec![named(&normal)];
            s.metronome.port = named(&normal);
        });
    });
    install(cx, "startup-met");
    assert!(!own_ports(cx).contains(&normal));
    assert!(mapped_ports(cx).is_empty());
    let (met, clock) = metronome_and_clock(cx);
    assert_eq!(met, named(&saved));
    assert_eq!(clock, vec![named(&saved)]);
}

// ---------- C3. 보이는 모양이 같은 장치 둘 ----------

/// 보이는 모양이 같은(공백 위치만 다른) 장치 둘이 함께 나타나면 고유 ID가 가장 작은 장치로 트랙을 되돌린다. 일반
/// 재연결(`best_match`)과 같은 기준이다(리뷰 3 제안 2).
#[gpui_kit::test]
fn two_visually_same_devices_pick_smallest_unique_id(cx: &mut TestAppContext) {
    if run_isolated("two_visually_same_devices_pick_smallest_unique_id") {
        return;
    }
    let dir = start(cx, "twin");
    let (saved, normal) = wide("twin synth");
    let other = normal.replacen(' ', "  ", 2).replacen("  ", " ", 1);
    assert_ne!(other, saved);
    assert!(keyscore_midi::same_port_name(&other, &normal));
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&saved)]))).unwrap();
    let (_handle, view) = open_file(cx, &path);
    answer(cx, MAP);
    assert_eq!(ports_of(cx, &view), vec![named(&normal)]);
    // 나타남 이벤트는 이름순이라 `saved`(공백 둘이 앞에 있다)가 먼저 온다. 그쪽에 큰 고유 ID를 준다
    let pid = i32::try_from(std::process::id() & 0xFFFF).unwrap();
    let _a = Destination::with_unique_id(&saved, Some(0x6000_0000 | pid));
    let _b = Destination::with_unique_id(&other, Some(0x2000_0000 | pid));
    let ids: Vec<(String, i32)> = keyscore_midi::destinations()
        .into_iter()
        .filter(|d| d.name() == saved || d.name() == other)
        .map(|d| (d.name().to_owned(), d.unique_id()))
        .collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(
        ids.iter()
            .any(|(n, id)| *n == other && *id == 0x2000_0000 | pid),
        "고유 ID를 정하지 못했다: {ids:?}"
    );
    pump(cx, Duration::from_millis(600));
    assert!(!own_ports(cx).contains(&normal), "대체 포트가 남았다");
    assert_eq!(
        ports_of(cx, &view),
        vec![named(&other)],
        "먼저 나타난 장치가 아니라 고유 ID가 작은 장치여야 한다"
    );
}

// ---------- C4. 설정에 남은 대체 표시 ----------

/// 시작 때 만들지 못한 대체 포트 이름이 설정 `mapped_ports`에 남아 있어도, 사용자가 그 이름으로 포트를 만들거나
/// 바꾸면 표시가 지워진다(리뷰 3 제안 4).
#[gpui_kit::test]
fn rename_and_add_clear_stale_mapped_mark(cx: &mut TestAppContext) {
    if run_isolated("rename_and_add_clear_stale_mapped_mark") {
        return;
    }
    let dir = temp_dir("stale-mark");
    let x = unique("stale x");
    let y = unique("stale y");
    cx.update(|cx| init(cx, dir.join("settings.json")));
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.midi_output.mapped_ports = vec![x.clone(), y.clone()];
        });
    });
    install(cx, "stale-mark");
    assert_eq!(mapped_ports(cx), vec![x.clone(), y.clone()]);
    let made = cx
        .update(|cx| engine::add_named_virtual_port(cx, &x))
        .unwrap();
    assert_eq!(made, x);
    assert_eq!(
        mapped_ports(cx),
        vec![y.clone()],
        "만든 포트의 표시가 남았다"
    );
    let renamed = cx
        .update(|cx| engine::rename_virtual_port(cx, &x, &y))
        .unwrap();
    assert_eq!(renamed, y);
    assert!(mapped_ports(cx).is_empty(), "바꾼 이름의 표시가 남았다");
    let (virtuals, main) = cx.read(|cx| {
        let s = AppSettings::get(cx);
        (
            s.midi_output.virtual_ports.clone(),
            s.midi_output.main_name.clone(),
        )
    });
    assert_eq!(virtuals, vec![y.clone()]);
    assert!(main.is_none());
    // 기본 포트 이름을 그 이름으로 바꿔도 같다
    let main_now = cx.read(engine::virtual_port_name);
    cx.update(|cx| {
        AppSettings::update(cx, |s| s.midi_output.mapped_ports = vec![x.clone()]);
    });
    cx.update(|cx| engine::rename_virtual_port(cx, &main_now, &x))
        .unwrap();
    assert!(mapped_ports(cx).is_empty());
}
