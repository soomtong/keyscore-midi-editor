//! S2-T6e 적대적 리뷰 3회차. 리뷰 2 지적 1 대응(`windows_using`이 앱 가상 포트 이름에는 메트로놈·클럭 규칙을
//! 적용하지 않는다)이 맞는지, 앱 포트가 지워지거나 이름이 바뀌어 같은 이름의 목적지와 자리를 바꿀 때 재연결·알림이
//! 회귀하지 않았는지 본다. 도우미는 `review_s2_t6e_r2.rs`에서 복사했다. 모두 포트 감시를 켜므로 자식 프로세스에서
//! 혼자 돈다.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, ports};
use keyscore_core::{OutputPort, Project, Velocity, ksq};

const WAIT: Duration = Duration::from_secs(2);

fn unique(name: &str) -> String {
    format!("KS rv6e3 {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv6e3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 앱을 띄우고(설정 파일 내용을 줄 수 있다) 엔진과 포트 감시를 켠다.
fn start(cx: &mut TestAppContext, tag: &str, settings_json: Option<&str>) {
    let dir = temp_dir(tag);
    if let Some(text) = settings_json {
        std::fs::write(dir.join("settings.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let options = EngineOptions {
        client_name: format!("ks-rv6e3-{tag}"),
        port_name: unique(&format!("main {tag}")),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    cx.update(ports::start_monitor);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
        if cx.read(ports::is_monitoring) {
            return;
        }
        assert!(Instant::now() < deadline, "감시가 시작되지 않았다");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 박마다 노트 하나(32마디), 트랙 포트는 `port`. 120 BPM.
fn project_on(port: &OutputPort) -> Project {
    let v = Velocity::from_midi1(100).unwrap().get();
    let notes: Vec<String> = (0..128)
        .map(|i| format!("[{}, 240, 60, {v}]", if i == 0 { 0 } else { 960 }))
        .collect();
    let port = match port {
        OutputPort::Virtual => "\"virtual\"".to_owned(),
        OutputPort::Named(n) => format!("{{\"named\": \"{n}\"}}"),
    };
    ksq::from_str(&format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{{"name": "T", "port": {port}, "channel": 1,
            "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}}]}}"#,
        notes.join(", ")
    ))
    .unwrap()
    .0
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    activate(cx, handle);
    (handle, view)
}

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
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

/// 그 창의 메시지 가운데 `name`과 `word`를 함께 담은 줄 수.
fn said(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, name: &str, word: &str) -> usize {
    messages(cx, view)
        .iter()
        .filter(|m| m.contains(name) && m.contains(word))
        .count()
}

/// 조건 없이 `limit` 동안 감시를 돌린다.
fn pump_for(cx: &mut TestAppContext, handle: AnyWindowHandle, limit: Duration) {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
        render(cx, handle);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 창 없이 감시를 돌린다. 테스트 목적지를 만든 뒤 그 나타남을 재생 전에 처리해, 뒤의 사라짐과 한 주기에
/// 섞이지 않게 한다.
fn settle(cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_gone(name: &str) {
    let deadline = Instant::now() + WAIT;
    while keyscore_midi::destinations()
        .iter()
        .any(|d| d.name() == name)
    {
        assert!(Instant::now() < deadline, "목적지 {name}가 사라지지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    }
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

fn drain(rx: &mpsc::Receiver<Vec<u8>>, limit: Duration) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(m) => got.push(m),
            Err(_) => return got,
        }
    }
}

/// 앱 가상 포트(소스)의 수신기.
struct Receiver {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

impl Receiver {
    fn source(name: &str) -> Receiver {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-rv6e3-rx").unwrap();
        let input = client
            .input_port("in", move |list: &coremidi::PacketList| {
                for p in list {
                    for msg in split(p.data()) {
                        let _ = tx.send(msg);
                    }
                }
            })
            .unwrap();
        let deadline = Instant::now() + WAIT;
        let source = loop {
            if let Some(s) = coremidi::Source::from_name(name) {
                break s;
            }
            assert!(Instant::now() < deadline, "소스 {name}이 보이지 않는다");
            std::thread::sleep(Duration::from_millis(10));
        };
        input.connect_source(&source).unwrap();
        Receiver {
            rx,
            _input: input,
            _client: client,
        }
    }

    fn collect(&self, limit: Duration) -> Vec<Vec<u8>> {
        drain(&self.rx, limit)
    }
}

/// 다른 앱의 입력 포트처럼 보이는 목적지.
struct Destination {
    rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

impl Destination {
    fn new(name: &str) -> Destination {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new(&format!("ks-rv6e3-dest-{name}")).unwrap();
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

    fn collect(&self, limit: Duration) -> Vec<Vec<u8>> {
        drain(&self.rx, limit)
    }
}

fn count(got: &[Vec<u8>], status: u8) -> usize {
    got.iter().filter(|m| m.first() == Some(&status)).count()
}

/// 포트 감시는 프로세스에 하나라, 감시를 켜는 테스트는 자식 프로세스에서 혼자 돌린다(`virtual_ports.rs`와 같다).
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

// ---------- 리뷰 2 지적 1 대응 확인 ----------

/// 리뷰 2 제안 2: 메트로놈 포트가 앱 추가 가상 포트 이름일 때도 같은 이름 목적지의 나타남·사라짐을 상태줄에
/// 알린다. 재연결 문구는 없다.
#[gpui_kit::test]
fn review3_destination_named_like_own_metronome_port_is_announced(cx: &mut TestAppContext) {
    if run_isolated("review3_destination_named_like_own_metronome_port_is_announced") {
        return;
    }
    let y = unique("own-met");
    start(
        cx,
        "own-met",
        Some(&format!(
            r#"{{"midi_output": {{"virtual_ports": ["{y}"]}}, "metronome": {{"port": {{"named": "{y}"}}}}}}"#
        )),
    );
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    let twin = Destination::new(&y);
    pump_for(cx, h, Duration::from_millis(800));
    drop(twin);
    wait_gone(&y);
    pump_for(cx, h, Duration::from_millis(800));
    let all = messages(cx, &view);
    assert_eq!(said(cx, &view, &y, "새로 보인다"), 1, "{all:?}");
    assert_eq!(said(cx, &view, &y, "없어졌다"), 1, "{all:?}");
    assert_eq!(said(cx, &view, &y, "사라졌다"), 0, "{all:?}");
    assert_eq!(said(cx, &view, &y, "다시 연결했다"), 0, "{all:?}");
}

/// 클럭 목록의 앱 포트 Y와 같은 이름의 목적지가 있는 채로 재생 중 앱 포트 Y를 지우면, Clock은 목적지 Y로 옮겨
/// 가고(이제 앱 포트가 아니다) 그 목적지는 클럭 포트로 감시된다: 사라지면 창마다 「사라졌다」, 돌아오면 「다시
/// 연결했다」이고 Clock이 이어진다.
#[gpui_kit::test]
fn review3_removed_own_clock_port_hands_over_to_same_name_destination(cx: &mut TestAppContext) {
    if run_isolated("review3_removed_own_clock_port_hands_over_to_same_name_destination") {
        return;
    }
    let y = unique("handover");
    start(
        cx,
        "handover",
        Some(&format!(
            r#"{{"midi_output": {{"virtual_ports": ["{y}"], "clock": [{{"named": "{y}"}}]}}}}"#
        )),
    );
    let rx = Receiver::source(&y);
    settle(cx);
    let (ha, a) = open(cx, project_on(&OutputPort::Virtual));
    let (_hb, b) = open(cx, project_on(&OutputPort::Virtual));
    activate(cx, ha);
    run(cx, ha, transport::Play);
    assert!(count(&rx.collect(Duration::from_millis(500)), 0xF8) > 0);
    let twin = Destination::new(&y);
    pump_for(cx, ha, Duration::from_millis(800));
    assert_eq!(count(&twin.collect(Duration::from_millis(300)), 0xF8), 0);
    cx.update(|cx| engine::remove_virtual_port(cx, &y)).unwrap();
    pump_for(cx, ha, Duration::from_millis(500));
    let _ = twin.collect(Duration::from_millis(50));
    assert!(
        count(&twin.collect(Duration::from_millis(500)), 0xF8) > 0,
        "앱 포트를 지운 뒤 같은 이름 목적지로 Clock이 가지 않는다: {:?}",
        messages(cx, &a)
    );
    drop(twin);
    wait_gone(&y);
    pump_for(cx, ha, Duration::from_secs(1));
    let back = Destination::new(&y);
    pump_for(cx, ha, Duration::from_secs(1));
    let _ = back.collect(Duration::from_millis(50));
    let got = back.collect(Duration::from_millis(500));
    run(cx, ha, transport::Stop);
    for (who, view) in [("재생 창", &a), ("정지 창", &b)] {
        let all = messages(cx, view);
        assert_eq!(said(cx, view, &y, "사라졌다"), 1, "{who}: {all:?}");
        assert_eq!(said(cx, view, &y, "다시 연결했다"), 1, "{who}: {all:?}");
    }
    assert!(count(&got, 0xF8) > 0, "돌아온 목적지로 Clock이 없다");
}

/// 클럭 포트인 목적지 Y가 사라진 사이 앱 추가 포트의 이름을 Y로 바꾸면 Clock은 앱 포트 Y로 간다. 그 뒤 목적지
/// Y가 돌아와도(감시는 Y를 기억한다) 앱 포트가 이기므로 Clock은 목적지로 가지 않고, 「다시 연결했다」 대신
/// 쓰지 않는 포트의 「새로 보인다」가 나온다.
#[gpui_kit::test]
fn review3_own_port_renamed_to_lost_clock_port_wins_on_return(cx: &mut TestAppContext) {
    if run_isolated("review3_own_port_renamed_to_lost_clock_port_wins_on_return") {
        return;
    }
    let y = unique("takeover");
    let z = unique("takeover-own");
    start(
        cx,
        "takeover",
        Some(&format!(
            r#"{{"midi_output": {{"virtual_ports": ["{z}"], "clock": [{{"named": "{y}"}}]}}}}"#
        )),
    );
    let dest = Destination::new(&y);
    settle(cx);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    assert!(count(&dest.collect(Duration::from_millis(500)), 0xF8) > 0);
    drop(dest);
    wait_gone(&y);
    pump_for(cx, h, Duration::from_secs(1));
    let renamed = cx
        .update(|cx| engine::rename_virtual_port(cx, &z, &y))
        .unwrap();
    assert_eq!(renamed, y);
    let rx = Receiver::source(&y);
    pump_for(cx, h, Duration::from_millis(300));
    assert!(
        count(&rx.collect(Duration::from_millis(500)), 0xF8) > 0,
        "이름을 바꾼 앱 포트로 Clock이 가지 않는다: {:?}",
        messages(cx, &view)
    );
    let back = Destination::new(&y);
    pump_for(cx, h, Duration::from_secs(1));
    let _ = rx.collect(Duration::from_millis(50));
    let to_back = back.collect(Duration::from_millis(500));
    let to_own = rx.collect(Duration::from_millis(500));
    run(cx, h, transport::Stop);
    let all = messages(cx, &view);
    assert_eq!(
        count(&to_back, 0xF8),
        0,
        "앱 포트와 같은 이름 목적지로 Clock이 갔다"
    );
    assert!(
        count(&to_own, 0xF8) > 0,
        "목적지가 돌아온 뒤 앱 포트 Clock이 끊겼다"
    );
    assert_eq!(said(cx, &view, &y, "사라졌다"), 1, "{all:?}");
    assert_eq!(said(cx, &view, &y, "다시 연결했다"), 0, "{all:?}");
    assert_eq!(said(cx, &view, &y, "새로 보인다"), 1, "{all:?}");
}

/// 대조군(이 Task 전, S2-T6d부터의 동작): 트랙이 앱 추가 포트 Y를 쓰는 창에서는 같은 이름 목적지 Y의 나타남·
/// 사라짐이 어떤 문구로도 보이지 않는다(`windows_using`의 트랙 경로는 앱 포트 이름을 빼지 않는다). 다른 창은
/// 알린다. 리뷰 3 제안 1을 받아들이면 창 A의 기대값을 1로 뒤집는다.
#[gpui_kit::test]
fn review3_control_track_on_own_port_hides_same_name_destination(cx: &mut TestAppContext) {
    if run_isolated("review3_control_track_on_own_port_hides_same_name_destination") {
        return;
    }
    let y = unique("own-track");
    start(
        cx,
        "own-track",
        Some(&format!(
            r#"{{"midi_output": {{"virtual_ports": ["{y}"]}}}}"#
        )),
    );
    let (ha, a) = open(cx, project_on(&OutputPort::Named(y.clone())));
    let (_hb, b) = open(cx, project_on(&OutputPort::Virtual));
    activate(cx, ha);
    let twin = Destination::new(&y);
    pump_for(cx, ha, Duration::from_millis(800));
    drop(twin);
    wait_gone(&y);
    pump_for(cx, ha, Duration::from_millis(800));
    let in_a = messages(cx, &a);
    let in_b = messages(cx, &b);
    eprintln!("A: {in_a:?}\nB: {in_b:?}");
    assert_eq!(said(cx, &b, &y, "새로 보인다"), 1, "{in_b:?}");
    assert_eq!(said(cx, &b, &y, "없어졌다"), 1, "{in_b:?}");
    let in_a_about_y = in_a.iter().filter(|m| m.contains(&y)).count();
    assert_eq!(in_a_about_y, 0, "{in_a:?}");
}
