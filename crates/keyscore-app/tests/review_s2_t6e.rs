//! S2-T6e 적대적 리뷰 1회차. 포트별 MIDI Clock(OUT-5)이 엉뚱한 포트로 나가는 틈, 클럭 포트의 소멸·복귀(OUT-3),
//! 테스트 노트(PRE-3), 설정 화면의 클럭 목록을 깨뜨려 본다. 도우미는 `clock_test_note.rs`·`port_watch.rs`에서
//! 복사했다.

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
    format!("KS rv6e {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv6e-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 앱을 띄우고(설정 파일 내용을 줄 수 있다) 엔진을 설치한다. `monitor`면 포트 감시도 켠다. 기본 포트 이름.
fn start(cx: &mut TestAppContext, tag: &str, settings_json: Option<&str>, monitor: bool) -> String {
    let dir = temp_dir(tag);
    if let Some(text) = settings_json {
        std::fs::write(dir.join("settings.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let main = main_name(tag);
    let options = EngineOptions {
        client_name: format!("ks-rv6e-{tag}"),
        port_name: main.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    if monitor {
        cx.update(ports::start_monitor);
        pump(
            cx,
            None,
            |cx| cx.read(ports::is_monitoring),
            "감시가 시작되지 않았다",
        );
    }
    main
}

fn main_name(tag: &str) -> String {
    unique(&format!("main {tag}"))
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

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn label_of(cx: &mut TestAppContext, handle: AnyWindowHandle, id: String) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(gpui_kit::ElementId::Name(id.into()))
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
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

/// 감시 주기만큼 시계를 돌리며 조건을 기다린다(CoreMIDI 알림은 실제 시간으로 온다).
fn pump(
    cx: &mut TestAppContext,
    handle: Option<AnyWindowHandle>,
    done: impl Fn(&mut TestAppContext) -> bool,
    why: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
        if let Some(h) = handle {
            render(cx, h);
        }
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "{why}");
        std::thread::sleep(Duration::from_millis(10));
    }
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

fn has_source(name: &str) -> bool {
    keyscore_midi::source_names().iter().any(|n| n == name)
}

fn wait_source(name: &str) {
    let deadline = Instant::now() + WAIT;
    while !has_source(name) {
        assert!(Instant::now() < deadline, "소스 {name}이 보이지 않는다");
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
        let client = coremidi::Client::new("ks-rv6e-rx").unwrap();
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
        let client = coremidi::Client::new(&format!("ks-rv6e-dest-{name}")).unwrap();
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

// ---------- A. 클럭 포트의 소멸·복귀(OUT-3) ----------

/// 재생 시작 때 없던 클럭 전용 목적지가 재생 중에 나타나면, 트랙·메트로놈 포트처럼 자동으로 이어져 Clock이
/// 가야 한다(OUT-3 「같은 이름의 포트가 돌아오면 자동 재연결」).
#[gpui_kit::test]
fn review_clock_port_appearing_during_play_gets_clock(cx: &mut TestAppContext) {
    if run_isolated("review_clock_port_appearing_during_play_gets_clock") {
        return;
    }
    let dest_name = unique("late-dest");
    let _main = start(cx, "late", Some(&clock_json(&[&dest_name])), true);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    assert!(
        messages(cx, &view)
            .iter()
            .any(|m| m.contains("MIDI Clock") && m.contains(&dest_name)),
        "없는 클럭 포트를 알리지 않았다: {:?}",
        messages(cx, &view)
    );
    // 감시가 이 이름을 「없음」으로 등록하게 한 주기 돌린 뒤 만든다
    pump_for(cx, h, Duration::from_millis(300));
    let dest = Destination::new(&dest_name);
    pump_for(cx, h, Duration::from_millis(1500));
    let got = dest.collect(Duration::from_millis(500));
    run(cx, h, transport::Stop);
    assert!(
        count(&got, 0xF8) > 0,
        "재생 중에 나타난 클럭 포트로 Clock이 오지 않는다(포트 감시가 클럭 포트 이름을 보지 않는다): {:?}",
        &got[..got.len().min(8)]
    );
}

/// 재생 중 클럭 전용 목적지가 사라지면 상태줄에 알린다(OUT-3 「포트 소멸을 감지해 상태줄에 표시」).
#[gpui_kit::test]
fn review_clock_port_vanish_is_reported(cx: &mut TestAppContext) {
    if run_isolated("review_clock_port_vanish_is_reported") {
        return;
    }
    let dest_name = unique("vanish-dest");
    let _main = start(cx, "vanish", Some(&clock_json(&[&dest_name])), true);
    let dest = Destination::new(&dest_name);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    assert!(count(&dest.collect(Duration::from_millis(500)), 0xF8) > 0);
    drop(dest);
    wait_gone(&dest_name);
    pump_for(cx, h, Duration::from_millis(1500));
    let said = messages(cx, &view);
    run(cx, h, transport::Stop);
    assert!(
        said.iter().any(|m| m.contains(&dest_name)),
        "사라진 클럭 포트를 알리지 않았다: {said:?}"
    );
}

/// 재생 중 클럭 전용 목적지가 사라졌다 같은 이름으로 돌아오면 편집 없이 Clock이 다시 가야 한다(OUT-3).
#[gpui_kit::test]
fn review_clock_port_return_reconnects(cx: &mut TestAppContext) {
    if run_isolated("review_clock_port_return_reconnects") {
        return;
    }
    let dest_name = unique("return-dest");
    let _main = start(cx, "return", Some(&clock_json(&[&dest_name])), true);
    let dest = Destination::new(&dest_name);
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    assert!(count(&dest.collect(Duration::from_millis(500)), 0xF8) > 0);
    drop(dest);
    wait_gone(&dest_name);
    pump_for(cx, h, Duration::from_millis(500));
    let back = Destination::new(&dest_name);
    pump_for(cx, h, Duration::from_millis(1500));
    let got = back.collect(Duration::from_millis(500));
    run(cx, h, transport::Stop);
    assert!(
        count(&got, 0xF8) > 0,
        "돌아온 클럭 포트로 Clock이 다시 오지 않는다: {:?}",
        &got[..got.len().min(8)]
    );
}

// ---------- B. 엉뚱한 포트로 가는 틈 ----------

/// 정지 중 설정 창의 테스트 노트는 [기본, X] 스냅샷을 넣는다. 지난 재생의 클럭 번호(1)가 스케줄러에 남아 있어도
/// 다음 재생의 Start·Clock은 X가 아니라 클럭 포트로 가야 한다.
#[gpui_kit::test]
fn review_start_goes_to_clock_port_after_stopped_test_note(cx: &mut TestAppContext) {
    let dest_name = unique("stn-dest");
    let dest = Destination::new(&dest_name);
    let _main = start(cx, "stn", Some(&clock_json(&[&dest_name])), false);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    wait_source(&extra);
    let x = Receiver::source(&extra);
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    assert!(count(&dest.collect(Duration::from_millis(400)), 0xF8) > 0);
    run(cx, h, transport::Stop);
    let _ = dest.collect(Duration::from_millis(200));
    cx.update(|cx| engine::send_port_test_note(cx, &extra))
        .unwrap();
    let note = x.collect(Duration::from_millis(400));
    assert!(note.iter().any(|m| m.as_slice() == [0x90, 60, 100]));
    run(cx, h, transport::Play);
    let at_x = x.collect(Duration::from_millis(600));
    let at_dest = dest.collect(Duration::from_millis(100));
    run(cx, h, transport::Stop);
    assert_eq!(count(&at_x, 0xFA) + count(&at_x, 0xF8), 0, "{at_x:?}");
    assert_eq!(
        count(&at_dest, 0xFA),
        1,
        "{:?}",
        &at_dest[..at_dest.len().min(4)]
    );
    assert!(count(&at_dest, 0xF8) > 0);
}

/// 재생하는 창이 바뀌면(번호가 다른 스냅샷) 앞 재생의 Stop과 새 재생의 Start·Clock이 클럭 포트로만 간다.
/// 새 창의 트랙 포트(번호 1)는 아무 실시간 메시지도 받지 않는다.
#[gpui_kit::test]
fn review_window_switch_keeps_clock_on_its_port(cx: &mut TestAppContext) {
    let dest_name = unique("sw-dest");
    let dest = Destination::new(&dest_name);
    let other_name = unique("sw-other");
    let other = Destination::new(&other_name);
    let _main = start(cx, "switch", Some(&clock_json(&[&dest_name])), false);
    let (a, _) = open(cx, project_on(&OutputPort::Virtual));
    let (b, _) = open(cx, project_on(&OutputPort::Named(other_name.clone())));
    activate(cx, a);
    run(cx, a, transport::Play);
    assert!(count(&dest.collect(Duration::from_millis(400)), 0xF8) > 0);
    activate(cx, b);
    run(cx, b, transport::Play);
    let at_other = other.collect(Duration::from_millis(600));
    let at_dest = dest.collect(Duration::from_millis(100));
    run(cx, b, transport::Stop);
    let rt: Vec<_> = at_other
        .iter()
        .filter(|m| m.first().is_some_and(|s| *s >= 0xF8 || *s == 0xF2))
        .collect();
    assert!(rt.is_empty(), "트랙 포트로 실시간 메시지: {rt:?}");
    assert!(count(&at_dest, 0xFC) >= 1, "앞 재생의 Stop이 없다");
    assert_eq!(count(&at_dest, 0xFA), 1, "새 재생의 Start가 없다");
    assert!(count(&at_dest, 0xF8) > 0);
}

/// 메트로놈 포트와 클럭 포트가 같은 이름 목적지이고, 재생 중 트랙을 앞에 더해 번호가 바뀌어도 클릭과 Clock이
/// 모두 그 목적지로 가고 새 트랙의 포트로 가지 않는다.
#[gpui_kit::test]
fn review_metronome_and_clock_share_port_across_renumber(cx: &mut TestAppContext) {
    let dest_name = unique("share-dest");
    let dest = Destination::new(&dest_name);
    let other_name = unique("share-other");
    let other = Destination::new(&other_name);
    let json = format!(
        r#"{{"midi_output": {{"clock": [{{"named": "{dest_name}"}}]}}, "metronome": {{"port": {{"named": "{dest_name}"}}}}}}"#
    );
    let _main = start(cx, "share", Some(&json), false);
    assert_eq!(clock_setting(cx), [OutputPort::Named(dest_name.clone())]);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::ToggleMetronome);
    run(cx, h, transport::Play);
    let first = dest.collect(Duration::from_millis(800));
    assert!(count(&first, 0xF8) > 0);
    assert!(
        first.iter().any(|m| m[0] & 0xF0 == 0x90 && m[0] != 0xF8),
        "클릭이 없다: {first:?}"
    );
    let mut track = keyscore_core::Track::new(
        "X",
        keyscore_core::Channel::new(2).unwrap(),
        keyscore_core::TrackColor::new(0).unwrap(),
    );
    track.set_port(OutputPort::Named(other_name.clone()));
    let doc = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        doc.update(cx, |d, cx| {
            d.apply(vec![keyscore_core::Edit::InsertTrack {
                index: 0,
                track: Box::new(track),
            }])
            .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    let _ = dest.collect(Duration::from_millis(100));
    let at_other = other.collect(Duration::from_millis(800));
    let at_dest = dest.collect(Duration::from_millis(100));
    run(cx, h, transport::Stop);
    assert!(
        at_other.is_empty(),
        "새 트랙 포트로 클릭·Clock이 갔다: {at_other:?}"
    );
    assert!(count(&at_dest, 0xF8) > 0);
}

// ---------- C. 테스트 노트 ----------

/// 정지 중 X로 테스트 노트를 보내고 곧바로 Y로 보내면(같은 번호 1에 다른 포트), X의 Note Off가 X로 가야 한다.
#[gpui_kit::test]
fn review_test_note_off_reaches_first_port_after_quick_second(cx: &mut TestAppContext) {
    let _main = start(cx, "twice", None, false);
    let x_name = cx.update(engine::add_virtual_port).unwrap();
    let y_name = cx.update(engine::add_virtual_port).unwrap();
    wait_source(&x_name);
    wait_source(&y_name);
    let x = Receiver::source(&x_name);
    let y = Receiver::source(&y_name);
    cx.update(|cx| engine::send_port_test_note(cx, &x_name))
        .unwrap();
    cx.update(|cx| engine::send_port_test_note(cx, &y_name))
        .unwrap();
    let at_x = x.collect(Duration::from_millis(600));
    let at_y = y.collect(Duration::from_millis(100));
    let is_off =
        |m: &Vec<u8>| m.len() == 3 && m[1] == 60 && (m[0] == 0x80 || (m[0] == 0x90 && m[2] == 0));
    assert!(
        at_x.iter().any(|m| m.as_slice() == [0x90, 60, 100]),
        "{at_x:?}"
    );
    assert!(at_x.iter().any(is_off), "X의 Note Off가 없다: {at_x:?}");
    assert_eq!(
        at_y.iter().filter(|m| is_off(m)).count(),
        1,
        "Y가 Note Off를 두 번 받았다: {at_y:?}"
    );
    assert_eq!(
        cx.read(engine::last_test_note).map(|(_, p)| p).as_deref(),
        Some(y_name.as_str())
    );
}

// ---------- D. 설정 화면의 클럭 목록 ----------

/// 클럭 체크 상자는 Tab으로 닿고 space로 켠다(UI-11 키보드만으로).
#[gpui_kit::test]
fn review_clock_checkbox_reachable_by_keyboard(cx: &mut TestAppContext) {
    let _main = start(cx, "keys", None, false);
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    let (sh, view) = open_settings(cx, h);
    let focused = |cx: &mut TestAppContext| {
        cx.update_window(sh, |_, window, cx| {
            view.read(cx).focused_control(window, cx)
        })
        .unwrap()
    };
    let mut reached = false;
    for _ in 0..10 {
        if focused(cx).as_deref() == Some("settings-port-test") {
            reached = true;
            break;
        }
        press(cx, sh, "tab");
    }
    assert!(reached, "테스트 노트 버튼에 Tab으로 닿지 않는다");
    press(cx, sh, "tab");
    // 체크 상자는 키를 뗄 때 켠다. `simulate_keystrokes`는 키 뗌을 보내지 않아 실제 키 눌림·뗌을 보낸다.
    let space = gpui_kit::Keystroke::parse("space").unwrap();
    cx.update_window(sh, |_, window, cx| {
        window.dispatch_event(
            gpui_kit::PlatformInput::KeyDown(gpui_kit::KeyDownEvent {
                keystroke: space.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        window.dispatch_event(
            gpui_kit::PlatformInput::KeyUp(gpui_kit::KeyUpEvent { keystroke: space }),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, sh);
    assert_eq!(
        clock_setting(cx),
        [OutputPort::Virtual],
        "테스트 노트 버튼 다음 Tab·space로 첫 클럭 체크 상자(기본 포트)가 켜지지 않았다"
    );
}

/// 설정 파일이 기본 포트를 이름으로 가리키면(`{"named": 기본 포트 이름}`) 클럭은 포트 0으로 나간다. 설정 화면도
/// 그 포트를 「켬」으로 보여야 하고, 같은 포트의 줄이 둘이면 안 된다.
#[gpui_kit::test]
fn review_clock_named_main_port_shows_on(cx: &mut TestAppContext) {
    let tag = "named-main";
    let main = main_name(tag);
    let main_rx_name = main.clone();
    let _ = start(cx, tag, Some(&clock_json(&[&main])), false);
    let rx = Receiver::source(&main_rx_name);
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    let got = rx.collect(Duration::from_millis(500));
    run(cx, h, transport::Stop);
    assert!(count(&got, 0xF8) > 0, "기본 포트로 Clock이 가지 않는다");
    let (sh, _) = open_settings(cx, h);
    let shown = label_of(cx, sh, format!("settings-clock-{main}"));
    assert!(
        shown.as_deref().is_some_and(|l| l.ends_with("켬")),
        "Clock이 나가는 기본 포트가 「끔」으로 보인다: {shown:?}"
    );
}

/// 대조군: 같은 하네스에서 나타난 클럭 포트는 정지 뒤 다시 재생하면 Clock을 받는다. 편집만으로는 받지 못한다(재생 시작이 「없음」으로 기억했고 포트 감시가 그 이름을 보지 않는다).
#[gpui_kit::test]
fn review_control_clock_port_appearing_needs_replay(cx: &mut TestAppContext) {
    if run_isolated("review_control_clock_port_appearing_needs_replay") {
        return;
    }
    let dest_name = unique("late-ctl");
    let _main = start(cx, "late-ctl", Some(&clock_json(&[&dest_name])), true);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    pump_for(cx, h, Duration::from_millis(300));
    let dest = Destination::new(&dest_name);
    pump_for(cx, h, Duration::from_secs(1));
    // 편집(트랙 하나 더하기)으로 스냅샷을 다시 만들어도 이어지지 않는다
    let track = keyscore_core::Track::new(
        "Y",
        keyscore_core::Channel::new(3).unwrap(),
        keyscore_core::TrackColor::new(0).unwrap(),
    );
    let doc = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        doc.update(cx, |d, cx| {
            d.apply(vec![keyscore_core::Edit::InsertTrack {
                index: 0,
                track: Box::new(track),
            }])
            .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    let after_edit = dest.collect(Duration::from_millis(500));
    assert_eq!(
        count(&after_edit, 0xF8),
        0,
        "편집으로 이어졌다(지적 재검토 필요)"
    );
    // 다시 재생하면 이어진다: 스케줄러·하네스는 늦게 나타난 목적지로 Clock을 보낼 수 있다
    run(cx, h, transport::Stop);
    run(cx, h, transport::Play);
    let got = dest.collect(Duration::from_millis(600));
    run(cx, h, transport::Stop);
    assert!(
        count(&got, 0xF8) > 0,
        "대조군: 다시 재생해도 Clock이 오지 않는다: {got:?}"
    );
}
