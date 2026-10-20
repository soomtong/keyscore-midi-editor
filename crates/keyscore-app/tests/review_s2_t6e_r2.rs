//! S2-T6e 적대적 리뷰 2회차. 리뷰 1 지적 1 대응(포트 감시가 클럭 포트를 본다)이 새 문제를 만들지 않았는지
//! 깨뜨려 본다: 창마다 메시지 중복, 클럭이면서 트랙 포트인 경우, 재생 중 켜고 끈 직후의 감시 대상, 앱 가상 포트
//! 이름이 클럭 목록에 있을 때. 도우미는 `review_s2_t6e.rs`에서 복사했다. 모두 포트 감시를 켜므로 자식
//! 프로세스에서 혼자 돈다.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{Opt, SettingsSection};
use keyscore_app::actions::{settings, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, ports};
use keyscore_core::{OutputPort, Project, Velocity, ksq};

const WAIT: Duration = Duration::from_secs(2);

fn unique(name: &str) -> String {
    format!("KS rv6e2 {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv6e2-{}-{name}", std::process::id()));
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
        client_name: format!("ks-rv6e2-{tag}"),
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

fn click_id(cx: &mut TestAppContext, handle: AnyWindowHandle, id: String) {
    cx.update_window(handle, |_, window, cx| {
        window.click(gpui_kit::ElementId::Name(id.into()), cx);
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

fn open_settings(
    cx: &mut TestAppContext,
    from: AnyWindowHandle,
) -> (AnyWindowHandle, Entity<SettingsWindow>) {
    run(
        cx,
        from,
        settings::Open(Opt(Some(SettingsSection::MidiOutput))),
    );
    let (handle, view) = cx
        .update(|cx| settings_window::current(cx))
        .expect("설정 창이 없다");
    activate(cx, handle);
    (handle, view)
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
        let client = coremidi::Client::new("ks-rv6e2-rx").unwrap();
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
        let client = coremidi::Client::new(&format!("ks-rv6e2-dest-{name}")).unwrap();
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

fn note_ons(got: &[Vec<u8>]) -> usize {
    got.iter()
        .filter(|m| m.first().is_some_and(|s| s & 0xF0 == 0x90) && m.get(2) != Some(&0))
        .count()
}

fn clock_json(ports: &[&str]) -> String {
    let items: Vec<String> = ports
        .iter()
        .map(|p| format!("{{\"named\": \"{p}\"}}"))
        .collect();
    format!(r#"{{"midi_output": {{"clock": [{}]}}}}"#, items.join(", "))
}

fn clock_setting(cx: &mut TestAppContext) -> Vec<OutputPort> {
    cx.read(|cx| AppSettings::get(cx).midi_output.clock.clone())
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

// ---------- 리뷰 1 지적 1 대응 깨뜨리기 ----------

/// 창 둘(하나는 재생, 하나는 정지)에서 클럭 전용 포트가 사라졌다 돌아오면, 창마다 「사라졌다」·「다시
/// 연결했다」가 한 줄씩이고 「쓰지 않는 포트」 핫플러그 문구(「없어졌다」·「새로 보인다」)는 없다. 돌아온
/// 목적지로 Clock이 다시 간다.
#[gpui_kit::test]
fn review2_clock_port_loss_and_return_message_once_per_window(cx: &mut TestAppContext) {
    if run_isolated("review2_clock_port_loss_and_return_message_once_per_window") {
        return;
    }
    let x = unique("two-win");
    start(cx, "two-win", Some(&clock_json(&[&x])));
    let dest = Destination::new(&x);
    settle(cx);
    let (ha, a) = open(cx, project_on(&OutputPort::Virtual));
    let (_hb, b) = open(cx, project_on(&OutputPort::Virtual));
    activate(cx, ha);
    run(cx, ha, transport::Play);
    assert!(count(&dest.collect(Duration::from_millis(500)), 0xF8) > 0);
    drop(dest);
    wait_gone(&x);
    pump_for(cx, ha, Duration::from_secs(1));
    for (who, view) in [("재생 창", &a), ("정지 창", &b)] {
        assert_eq!(
            said(cx, view, &x, "사라졌다"),
            1,
            "{who}: {:?}",
            messages(cx, view)
        );
        assert_eq!(
            said(cx, view, &x, "없어졌다"),
            0,
            "{who}: {:?}",
            messages(cx, view)
        );
    }
    let back = Destination::new(&x);
    pump_for(cx, ha, Duration::from_secs(1));
    let got = back.collect(Duration::from_millis(500));
    run(cx, ha, transport::Stop);
    for (who, view) in [("재생 창", &a), ("정지 창", &b)] {
        assert_eq!(
            said(cx, view, &x, "다시 연결했다"),
            1,
            "{who}: {:?}",
            messages(cx, view)
        );
        assert_eq!(
            said(cx, view, &x, "새로 보인다"),
            0,
            "{who}: {:?}",
            messages(cx, view)
        );
    }
    assert!(count(&got, 0xF8) > 0, "돌아온 클럭 포트로 Clock이 없다");
}

/// 클럭 포트이면서 재생 창의 트랙 포트: 사라졌다 돌아오면 알림은 한 번씩이고, 돌아온 뒤 Clock은 한 벌(1초에
/// 48개 가까이, 두 번 켜져 겹치지 않는다)이며 노트도 다시 간다.
#[gpui_kit::test]
fn review2_clock_and_track_same_port_return_once(cx: &mut TestAppContext) {
    if run_isolated("review2_clock_and_track_same_port_return_once") {
        return;
    }
    let x = unique("shared");
    start(cx, "shared", Some(&clock_json(&[&x])));
    let dest = Destination::new(&x);
    settle(cx);
    let (h, view) = open(cx, project_on(&OutputPort::Named(x.clone())));
    run(cx, h, transport::Play);
    let before = dest.collect(Duration::from_secs(1));
    assert!(
        (36..=60).contains(&count(&before, 0xF8)),
        "재생 시작 Clock 수"
    );
    drop(dest);
    wait_gone(&x);
    pump_for(cx, h, Duration::from_millis(800));
    let back = Destination::new(&x);
    pump_for(cx, h, Duration::from_millis(800));
    let _ = back.collect(Duration::from_millis(100));
    let got = back.collect(Duration::from_secs(1));
    run(cx, h, transport::Stop);
    let said_lost = said(cx, &view, &x, "사라졌다");
    let said_back = said(cx, &view, &x, "다시 연결했다");
    assert_eq!(said_lost, 1, "{:?}", messages(cx, &view));
    assert_eq!(said_back, 1, "{:?}", messages(cx, &view));
    let clocks = count(&got, 0xF8);
    assert!(
        (36..=60).contains(&clocks),
        "돌아온 뒤 1초에 Clock {clocks}개"
    );
    assert!(note_ons(&got) > 0, "돌아온 뒤 노트가 없다");
}

/// 재생 중 설정 창에서 클럭을 켠 직후의 포트도 감시 대상이다: 사라졌다 돌아오면 Clock이 이어진다.
#[gpui_kit::test]
fn review2_clock_turned_on_during_play_follows_return(cx: &mut TestAppContext) {
    if run_isolated("review2_clock_turned_on_during_play_follows_return") {
        return;
    }
    let x = unique("live-on");
    start(cx, "live-on", None);
    let dest = Destination::new(&x);
    settle(cx);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    let (sh, _) = open_settings(cx, h);
    click_id(cx, sh, format!("settings-clock-{x}"));
    assert_eq!(clock_setting(cx), [OutputPort::Named(x.clone())]);
    assert!(count(&dest.collect(Duration::from_millis(500)), 0xF8) > 0);
    // 감시가 새 이름을 등록할 시간
    pump_for(cx, h, Duration::from_millis(300));
    drop(dest);
    wait_gone(&x);
    pump_for(cx, h, Duration::from_millis(800));
    let back = Destination::new(&x);
    pump_for(cx, h, Duration::from_millis(800));
    let got = back.collect(Duration::from_millis(500));
    run(cx, h, transport::Stop);
    assert!(
        count(&got, 0xF8) > 0,
        "재생 중 켠 클럭 포트가 돌아와도 이어지지 않는다: {:?}",
        messages(cx, &view)
    );
}

/// 재생 중 클럭을 끈 직후에는 그 포트가 사라졌다 돌아와도 Clock이 가지 않고, 「다시 연결했다」 대신 쓰지 않는
/// 포트의 핫플러그 문구가 나온다.
#[gpui_kit::test]
fn review2_clock_turned_off_during_play_ignores_return(cx: &mut TestAppContext) {
    if run_isolated("review2_clock_turned_off_during_play_ignores_return") {
        return;
    }
    let x = unique("live-off");
    start(cx, "live-off", Some(&clock_json(&[&x])));
    let dest = Destination::new(&x);
    settle(cx);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    assert!(count(&dest.collect(Duration::from_millis(500)), 0xF8) > 0);
    let (sh, _) = open_settings(cx, h);
    click_id(cx, sh, format!("settings-clock-{x}"));
    assert!(clock_setting(cx).is_empty());
    let _ = dest.collect(Duration::from_millis(100));
    assert_eq!(count(&dest.collect(Duration::from_millis(300)), 0xF8), 0);
    drop(dest);
    wait_gone(&x);
    pump_for(cx, h, Duration::from_millis(800));
    let back = Destination::new(&x);
    pump_for(cx, h, Duration::from_millis(800));
    let got = back.collect(Duration::from_millis(500));
    run(cx, h, transport::Stop);
    assert_eq!(count(&got, 0xF8), 0, "끈 클럭 포트로 Clock이 갔다");
    assert_eq!(
        said(cx, &view, &x, "다시 연결했다"),
        0,
        "{:?}",
        messages(cx, &view)
    );
    assert_eq!(
        said(cx, &view, &x, "새로 보인다"),
        1,
        "{:?}",
        messages(cx, &view)
    );
}

/// 클럭 목록에 앱 추가 가상 포트 이름이 있으면 감시하지 않는다: 같은 이름의 목적지가 생겼다 없어져도 Clock은
/// 앱 포트(소스)로 계속 가고, 목적지로는 가지 않으며, 재연결 문구가 나오지 않는다.
#[gpui_kit::test]
fn review2_own_port_in_clock_list_ignores_same_name_destination(cx: &mut TestAppContext) {
    if run_isolated("review2_own_port_in_clock_list_ignores_same_name_destination") {
        return;
    }
    let y = unique("own");
    start(
        cx,
        "own",
        Some(&format!(
            r#"{{"midi_output": {{"virtual_ports": ["{y}"], "clock": [{{"named": "{y}"}}]}}}}"#
        )),
    );
    let rx = Receiver::source(&y);
    settle(cx);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    assert!(count(&rx.collect(Duration::from_millis(500)), 0xF8) > 0);
    let twin = Destination::new(&y);
    pump_for(cx, h, Duration::from_millis(800));
    let _ = rx.collect(Duration::from_millis(50));
    let to_twin = twin.collect(Duration::from_millis(500));
    let to_own = rx.collect(Duration::from_millis(500));
    drop(twin);
    wait_gone(&y);
    pump_for(cx, h, Duration::from_millis(800));
    let _ = rx.collect(Duration::from_millis(50));
    let after = rx.collect(Duration::from_millis(500));
    run(cx, h, transport::Stop);
    let all = messages(cx, &view);
    eprintln!("메시지: {all:?}");
    assert_eq!(count(&to_twin, 0xF8), 0, "같은 이름 목적지로 Clock이 갔다");
    assert!(count(&to_own, 0xF8) > 0, "앱 포트로 Clock이 끊겼다");
    assert!(count(&after, 0xF8) > 0, "목적지가 없어진 뒤 Clock이 끊겼다");
    assert_eq!(said(cx, &view, &y, "다시 연결했다"), 0, "{all:?}");
    assert_eq!(said(cx, &view, &y, "사라졌다"), 0, "{all:?}");
}

/// 지적 1(리뷰 2): 클럭 목록에 앱 추가 가상 포트 이름이 있을 때, 같은 이름의 목적지(앱이 쓰지 않는 포트)가
/// 생기고 없어지는 것도 다른 쓰지 않는 포트처럼 상태줄에 알려야 한다(OUT-3 「핫플러그와 포트 소멸을 감지해
/// 상태줄에 표시」). `windows_using`이 클럭 목록의 이름을 앱 포트 여부와 상관없이 「모든 창이 쓰는 포트」로
/// 보아 `announce_hotplug`가 건너뛰고, `wanted_names`는 앱 포트 이름을 빼서 재연결 동작도 없다. 어느 문구도
/// 나오지 않는다(리뷰 1 수정 전에는 「새로 보인다」·「없어졌다」가 나왔다).
#[gpui_kit::test]
fn review2_destination_named_like_own_clock_port_is_announced(cx: &mut TestAppContext) {
    if run_isolated("review2_destination_named_like_own_clock_port_is_announced") {
        return;
    }
    let y = unique("own-hotplug");
    start(
        cx,
        "own-hotplug",
        Some(&format!(
            r#"{{"midi_output": {{"virtual_ports": ["{y}"], "clock": [{{"named": "{y}"}}]}}}}"#
        )),
    );
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    let twin = Destination::new(&y);
    pump_for(cx, h, Duration::from_millis(800));
    drop(twin);
    wait_gone(&y);
    pump_for(cx, h, Duration::from_millis(800));
    let all = messages(cx, &view);
    assert_eq!(
        said(cx, &view, &y, "새로 보인다"),
        1,
        "앱 포트와 이름이 같은 목적지의 나타남을 알리지 않았다: {all:?}"
    );
    assert_eq!(
        said(cx, &view, &y, "없어졌다"),
        1,
        "앱 포트와 이름이 같은 목적지의 사라짐을 알리지 않았다: {all:?}"
    );
}
