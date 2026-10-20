//! S2-T6e 포트별 MIDI Clock(OUT-5), 테스트 노트(PRE-3), 포트 상태 표시(PRE-2). 앱 포트(소스)는 같은 프로세스의
//! `coremidi` 수신기로, 이름 포트는 테스트가 만든 목적지로 받는다.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{Opt, SettingsSection};
use keyscore_app::actions::{midi, settings, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::settings::Settings;
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{OutputPort, Project, Velocity, ksq};

const WAIT: Duration = Duration::from_secs(2);

fn unique(name: &str) -> String {
    format!("KS ck {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-clock-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 앱을 띄우고(설정 파일 내용을 줄 수 있다) 엔진을 설치한다. 기본 포트 이름을 돌려준다.
fn start(cx: &mut TestAppContext, tag: &str, settings_json: Option<&str>) -> String {
    let dir = temp_dir(tag);
    if let Some(text) = settings_json {
        std::fs::write(dir.join("settings.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let main = unique(tag);
    let options = EngineOptions {
        client_name: format!("ks-ck-{tag}"),
        port_name: main.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    main
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

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
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

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
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

/// 편집할 포트 선택 상자에서 검색어로 고른다.
fn choose_port(cx: &mut TestAppContext, sh: AnyWindowHandle, search: &str) {
    click(cx, sh, "settings-port-select");
    type_text(cx, sh, search);
    press(cx, sh, "enter");
}

fn has_source(name: &str) -> bool {
    keyscore_midi::source_names().iter().any(|n| n == name)
}

fn wait_source(name: &str, present: bool) {
    let deadline = Instant::now() + WAIT;
    while has_source(name) != present {
        assert!(
            Instant::now() < deadline,
            "소스 {name}이 {}",
            if present {
                "보이지 않는다"
            } else {
                "사라지지 않는다"
            }
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

// ---------- 수신기 ----------

struct Receiver {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
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

impl Receiver {
    fn source(name: &str) -> Receiver {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-ck-rx").unwrap();
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

    fn drain_for(&self, limit: Duration) -> Vec<Vec<u8>> {
        let mut got = Vec::new();
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(m) => got.push(m),
                Err(_) => return got,
            }
        }
    }
}

// ---------- 목적지 ----------

struct Destination {
    rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

impl Destination {
    fn new(name: &str) -> Destination {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-ck-dest").unwrap();
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
        let mut got = Vec::new();
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(m) => got.push(m),
                Err(_) => return got,
            }
        }
    }
}

fn count(got: &[Vec<u8>], status: u8) -> usize {
    got.iter().filter(|m| m.first() == Some(&status)).count()
}

fn click_id(cx: &mut TestAppContext, handle: AnyWindowHandle, id: String) {
    cx.update_window(handle, |_, window, cx| {
        window.click(gpui_kit::ElementId::Name(id.into()), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn clock_id(name: &str) -> String {
    format!("settings-clock-{name}")
}

fn clock_setting(cx: &mut TestAppContext) -> Vec<OutputPort> {
    cx.read(|cx| AppSettings::get(cx).midi_output.clock.clone())
}

// ---------- 1. 설정 파일 ----------

#[test]
fn clock_settings_round_trip() {
    let path = temp_dir("round-trip").join("settings.json");
    assert!(Settings::load(&path).0.midi_output.clock.is_empty());
    std::fs::write(
        &path,
        r#"{"midi_output": {"clock": ["virtual", {"named": "IAC Bus 1"}, {"named": "IAC Bus 1"}, 7, {"named": ""}]}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(
        s.midi_output.clock,
        [
            OutputPort::Virtual,
            OutputPort::Named("IAC Bus 1".to_owned())
        ]
    );
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    s.save(&path).unwrap();
    assert_eq!(Settings::load(&path).0, s);
    std::fs::write(&path, r#"{"midi_output": {"clock": "on"}}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert!(s.midi_output.clock.is_empty());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

// ---------- 2. 클럭 송신 ----------

#[gpui_kit::test]
fn clock_port_receives_start_clock_stop(cx: &mut TestAppContext) {
    let main = start(cx, "clock", None);
    let main_rx = Receiver::source(&main);
    let dest_name = unique("clock-dest");
    let dest = Destination::new(&dest_name);
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    let (sh, _) = open_settings(cx, h);
    click_id(cx, sh, clock_id(&dest_name));
    assert_eq!(clock_setting(cx), [OutputPort::Named(dest_name.clone())]);
    run(cx, h, transport::Play);
    // 120 BPM: 0.5 s에 한 박 = Clock 24개
    let got = dest.collect(Duration::from_secs(1));
    assert_eq!(
        got.first().map(Vec::as_slice),
        Some(&[0xFA][..]),
        "Start가 먼저 오지 않았다: {:?}",
        &got[..got.len().min(4)]
    );
    let clocks = count(&got, 0xF8);
    assert!(
        (36..=60).contains(&clocks),
        "1초에 Clock {clocks}개(48개 가까이여야 한다)"
    );
    run(cx, h, transport::Stop);
    let after = dest.collect(Duration::from_millis(300));
    assert!(count(&after, 0xFC) >= 1, "Stop이 오지 않았다: {after:?}");
    let leaked = main_rx.drain_for(Duration::from_millis(100));
    assert_eq!(
        count(&leaked, 0xF8),
        0,
        "클럭을 켜지 않은 기본 포트로 Clock이 갔다"
    );
}

// ---------- 3. 재생 중 켜고 끄기, 번호 바뀜 ----------

#[gpui_kit::test]
fn clock_toggle_while_playing_and_survives_edits(cx: &mut TestAppContext) {
    let main = start(cx, "clock-live", None);
    let main_rx = Receiver::source(&main);
    let dest_name = unique("clock-live-dest");
    let dest = Destination::new(&dest_name);
    let other_name = unique("clock-live-other");
    let other = Destination::new(&other_name);
    let (h, view) = open(cx, project_on(&OutputPort::Virtual));
    run(cx, h, transport::Play);
    let (sh, _) = open_settings(cx, h);
    click_id(cx, sh, clock_id(&dest_name));
    assert!(
        count(&dest.collect(Duration::from_millis(500)), 0xF8) > 0,
        "재생 중 켠 클럭이 오지 않는다"
    );
    // 다른 목적지를 쓰는 트랙을 맨 앞에 더해 포트 번호를 바꾼다
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
    let other_got = other.collect(Duration::from_millis(500));
    assert_eq!(
        count(&other_got, 0xF8),
        0,
        "번호가 바뀐 뒤 클럭이 다른 포트로 갔다"
    );
    assert!(
        count(&dest.collect(Duration::from_millis(500)), 0xF8) > 0,
        "번호가 바뀐 뒤 클럭이 끊겼다"
    );
    // 끄면 멈춘다
    click_id(cx, sh, clock_id(&dest_name));
    assert!(clock_setting(cx).is_empty());
    let _ = dest.collect(Duration::from_millis(100));
    assert_eq!(
        count(&dest.collect(Duration::from_millis(400)), 0xF8),
        0,
        "끈 클럭이 계속 온다"
    );
    run(cx, h, transport::Stop);
    assert_eq!(
        count(&main_rx.drain_for(Duration::from_millis(50)), 0xF8),
        0
    );
}

// ---------- 4. 설정 창의 테스트 노트 ----------

#[gpui_kit::test]
fn settings_test_note_sends_to_selected_port(cx: &mut TestAppContext) {
    let _main = start(cx, "test-note", None);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    wait_source(&extra, true);
    let rx = Receiver::source(&extra);
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    let (sh, view) = open_settings(cx, h);
    choose_port(cx, sh, &extra);
    click(cx, sh, "settings-port-test");
    let got = rx.drain_for(Duration::from_millis(500));
    assert!(
        got.iter().any(|m| m.as_slice() == [0x90, 60, 100]),
        "Note On이 오지 않았다: {got:?}"
    );
    assert!(
        got.iter()
            .any(|m| m.len() == 3 && m[1] == 60 && (m[0] == 0x80 || (m[0] == 0x90 && m[2] == 0))),
        "Note Off가 오지 않았다: {got:?}"
    );
    let last = cx.read(engine::last_test_note).map(|(_, p)| p);
    assert_eq!(last.as_deref(), Some(extra.as_str()));
    assert!(label(cx, sh, "settings-port-status").is_some_and(|l| l.contains(&extra)));
    // 설정 창에서 낸 Action도 같다
    run(cx, sh, midi::SendTestNote);
    assert!(
        rx.drain_for(Duration::from_millis(400))
            .iter()
            .any(|m| m.as_slice() == [0x90, 60, 100])
    );
    // 다른 창이 재생 중이고 그 재생이 이 포트를 쓰지 않으면 보내지 않고 알린다
    run(cx, h, transport::Play);
    let _ = rx.drain_for(Duration::from_millis(100));
    click(cx, sh, "settings-port-test");
    assert!(
        !rx.drain_for(Duration::from_millis(400))
            .iter()
            .any(|m| m.as_slice() == [0x90, 60, 100])
    );
    assert!(
        cx.read(|cx| view.read(cx).message())
            .is_some_and(|m| m.contains("재생"))
    );
    run(cx, h, transport::Stop);
}

// ---------- 5. 포트 상태 ----------

#[gpui_kit::test]
fn port_status_shows_known_facts(cx: &mut TestAppContext) {
    let main = start(cx, "status", None);
    let _rx = Receiver::source(&main);
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    let (sh, _) = open_settings(cx, h);
    let status = label(cx, sh, "settings-port-status").unwrap();
    assert!(status.contains("가상 포트 1개"), "{status}");
    assert!(
        status.contains("테스트 노트") && status.contains("없음"),
        "{status}"
    );
    assert!(status.contains("MIDI 입력"), "리스너 안내가 없다: {status}");
    // 인스펙터에서 보낸 테스트 노트도 보인다
    run(cx, h, midi::SendTestNote);
    render(cx, sh);
    let status = label(cx, sh, "settings-port-status").unwrap();
    assert!(status.contains(&main), "{status}");
    cx.update(engine::add_virtual_port).unwrap();
    render(cx, sh);
    assert!(
        label(cx, sh, "settings-port-status")
            .unwrap()
            .contains("가상 포트 2개")
    );
}

// ---------- 6. 클럭 명령 계산 ----------

#[test]
fn clock_commands_resend_everything_after_failure() {
    let a = |n: &str, i: u16| (n.to_owned(), i);
    // 같으면 명령 없음
    let same = [a("x", 1), a("y", 2)];
    assert_eq!(engine::clock_commands(Some(&same), &same), (vec![], vec![]));
    // 번호가 바뀌면 옛 번호를 끄고 새 번호를 켠다
    let moved = [a("x", 2), a("y", 3)];
    assert_eq!(
        engine::clock_commands(Some(&same), &moved),
        (vec![1, 2], vec![2, 3])
    );
    // 처음(지난 상태 없음)이나 지난 묶음 실패: 모든 번호를 끄고 새 번호를 켠다
    let all: Vec<u16> = (0..u16::try_from(keyscore_midi::MAX_SCHEDULER_PORTS).unwrap()).collect();
    assert_eq!(engine::clock_commands(None, &moved), (all, vec![2, 3]));
    // 끄기만
    assert_eq!(
        engine::clock_commands(Some(&same), &[]),
        (vec![1, 2], vec![])
    );
}

// ---------- 7. 이름 변경 ----------

#[gpui_kit::test]
fn clock_follows_virtual_port_rename(cx: &mut TestAppContext) {
    let _main = start(cx, "clock-rename", None);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    let (sh, _) = open_settings(cx, h);
    click_id(cx, sh, clock_id(&extra));
    assert_eq!(clock_setting(cx), [OutputPort::Named(extra.clone())]);
    let new = unique("clock-rename-new");
    cx.update(|cx| engine::rename_virtual_port(cx, &extra, &new))
        .unwrap();
    assert_eq!(clock_setting(cx), [OutputPort::Named(new.clone())]);
    render(cx, sh);
    assert!(
        label(cx, sh, Box::leak(clock_id(&new).into_boxed_str())).is_some_and(|l| l.contains("켬"))
    );
}
