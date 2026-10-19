//! S2-T6d 설정 > MIDI 출력의 가상 포트(OUT-1·2·8, 7.6). 앱의 가상 포트(소스)는 같은 프로세스의 `coremidi`
//! 수신기로 받는다.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{Opt, SettingsSection};
use keyscore_app::actions::{edit, midi, settings, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::settings::{MidiOutputSettings, Settings};
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, ports};
use keyscore_core::{OutputPort, Project, Velocity, ksq};

const WAIT: Duration = Duration::from_secs(2);

fn unique(name: &str) -> String {
    format!("KS vp {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-vports-{}-{name}", std::process::id()));
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
        client_name: format!("ks-vp-{tag}"),
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

/// 이름 입력란을 비우고 새 이름을 친 뒤 Enter.
fn rename_in_field(cx: &mut TestAppContext, sh: AnyWindowHandle, name: &str) {
    click(cx, sh, "settings-port-name");
    press(cx, sh, "cmd-a backspace");
    type_text(cx, sh, name);
    press(cx, sh, "enter");
}

fn names(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(engine::virtual_port_names)
}

fn midi_output(cx: &mut TestAppContext) -> MidiOutputSettings {
    cx.read(|cx| AppSettings::get(cx).midi_output.clone())
}

fn track_port(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> OutputPort {
    cx.read(|cx| {
        view.read(cx).document().read(cx).project().tracks()[0]
            .port()
            .clone()
    })
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
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

fn pump(cx: &mut TestAppContext, done: impl Fn(&mut TestAppContext) -> bool, why: &str) {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "{why}");
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
        let client = coremidi::Client::new("ks-vp-rx").unwrap();
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

    fn wait_note(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(m) if is_note_on(&m) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
    }
}

fn is_note_on(m: &[u8]) -> bool {
    m.len() == 3 && m[0] & 0xF0 == 0x90 && m[2] > 0
}

// ---------- 1. 설정 파일 ----------

#[test]
fn midi_output_settings_round_trip() {
    let path = temp_dir("round-trip").join("settings.json");
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty());
    assert_eq!(s.midi_output, MidiOutputSettings::default());
    assert_eq!(s.midi_output.main_name, None);
    assert!(s.midi_output.virtual_ports.is_empty());
    std::fs::write(
        &path,
        r#"{"midi_output": {"main_name": "Studio", "virtual_ports": ["Drums", "Bass"], "later": 1}}"#,
    )
    .unwrap();
    let (mut s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(s.midi_output.main_name.as_deref(), Some("Studio"));
    assert_eq!(s.midi_output.virtual_ports, ["Drums", "Bass"]);
    s.midi_output.virtual_ports.push("Pad".to_owned());
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"later\""), "{text}");
    assert_eq!(Settings::load(&path).0, s);

    // 잘못된 이름·중복(기본 포트와도)·17개째는 빼고 항목마다 경고
    let mut list: Vec<String> = (1..=15).map(|i| format!("\"P{i}\"")).collect();
    list.extend(["\"\"", "\"P1\"", "\"Studio\"", "3", "\"P16\""].map(str::to_owned));
    std::fs::write(
        &path,
        format!(
            r#"{{"midi_output": {{"main_name": "Studio", "virtual_ports": [{}]}}}}"#,
            list.join(", ")
        ),
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    let expected: Vec<String> = (1..=15).map(|i| format!("P{i}")).collect();
    assert_eq!(s.midi_output.virtual_ports, expected);
    assert_eq!(warnings.len(), 5, "{warnings:?}");
    // 형식이 틀리면 필드만 기본값
    std::fs::write(
        &path,
        r#"{"midi_output": {"main_name": 7, "virtual_ports": "x"}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s.midi_output, MidiOutputSettings::default());
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}

// ---------- 3. 추가와 라우팅 ----------

#[gpui_kit::test]
fn add_virtual_port_routes_named_tracks(cx: &mut TestAppContext) {
    let main = start(cx, "add", None);
    let main_rx = Receiver::source(&main);
    let added = cx.update(engine::add_virtual_port).unwrap();
    assert_eq!(added, format!("{main} 2"));
    assert_eq!(names(cx), [main.clone(), added.clone()]);
    assert_eq!(midi_output(cx).virtual_ports, std::slice::from_ref(&added));
    wait_source(&added, true);
    let extra_rx = Receiver::source(&added);
    // 포트 메뉴 선택지: 기본, 추가 가상, 목적지
    let choices = cx.read(engine::port_choices);
    assert_eq!(choices[0], OutputPort::Virtual);
    assert_eq!(choices[1], OutputPort::Named(added.clone()));
    let (handle, _) = open(cx, project_on(&OutputPort::Named(added.clone())));
    run(cx, handle, transport::Play);
    assert!(extra_rx.wait_note(WAIT), "추가 포트로 노트가 오지 않았다");
    let leaked = main_rx.drain_for(Duration::from_millis(300));
    assert!(
        !leaked.iter().any(|m| is_note_on(m)),
        "기본 포트로도 갔다: {leaked:?}"
    );
    run(cx, handle, transport::Stop);
    // 두 번째 추가는 다음 번호
    let third = cx.update(engine::add_virtual_port).unwrap();
    assert_eq!(third, format!("{main} 3"));
}

// ---------- 4. 이름 변경 ----------

#[gpui_kit::test]
fn rename_virtual_port_updates_open_tracks(cx: &mut TestAppContext) {
    let _main = start(cx, "rename", None);
    let old = cx.update(engine::add_virtual_port).unwrap();
    wait_source(&old, true);
    let rx = Receiver::source(&old);
    let port = OutputPort::Named(old.clone());
    let (h1, v1) = open(cx, project_on(&port));
    let (_h2, v2) = open(cx, project_on(&port));
    cx.update(|cx| AppSettings::update(cx, |s| s.metronome.port = port.clone()));
    let (u1, u2) = (undo_len(cx, &v1), undo_len(cx, &v2));
    run(cx, h1, transport::Play);
    assert!(rx.wait_note(WAIT));

    let new = unique("rename-new");
    let (sh, view) = open_settings(cx, h1);
    choose_port(cx, sh, &old);
    assert_eq!(
        cx.read(|cx| view.read(cx).selected_port()),
        Some(old.clone())
    );
    rename_in_field(cx, sh, &new);
    wait_source(&new, true);
    wait_source(&old, false);
    assert!(names(cx).contains(&new));
    assert_eq!(midi_output(cx).virtual_ports, std::slice::from_ref(&new));
    // 열린 두 창의 트랙과 메트로놈 포트가 따라온다
    assert_eq!(track_port(cx, &v1), OutputPort::Named(new.clone()));
    assert_eq!(track_port(cx, &v2), OutputPort::Named(new.clone()));
    assert_eq!(undo_len(cx, &v1), u1 + 1);
    assert_eq!(undo_len(cx, &v2), u2 + 1);
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).metronome.port.clone()),
        OutputPort::Named(new.clone())
    );
    // 재생은 끊기지 않고 같은 엔드포인트로 계속 간다
    rx.drain_for(Duration::from_millis(50));
    assert!(rx.wait_note(WAIT), "이름을 바꾼 뒤 소리가 끊겼다");
    run(cx, h1, transport::Stop);
    // undo는 그 창의 트랙만 옛 이름으로
    run(cx, h1, edit::Undo);
    assert_eq!(track_port(cx, &v1), OutputPort::Named(old.clone()));
    assert_eq!(track_port(cx, &v2), OutputPort::Named(new.clone()));

    // Action 경로: 고른 포트의 이름을 바꾼다
    let newer = unique("rename-newer");
    run(cx, sh, midi::RenameVirtualPort(newer.clone()));
    wait_source(&newer, true);
    assert_eq!(track_port(cx, &v2), OutputPort::Named(newer.clone()));
}

// ---------- 5. 거부 ----------

#[gpui_kit::test]
fn rename_rejects_bad_names(cx: &mut TestAppContext) {
    let main = start(cx, "reject", None);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let dest_name = unique("reject-dest");
    let client = coremidi::Client::new("ks-vp-dest").unwrap();
    let _dest = client
        .virtual_destination(&dest_name, |_: &coremidi::PacketList| {})
        .unwrap();
    let deadline = Instant::now() + WAIT;
    while !keyscore_midi::destinations()
        .iter()
        .any(|d| d.name() == dest_name)
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    let (sh, view) = open_settings(cx, h);
    choose_port(cx, sh, &extra);
    for bad in ["", " ", main.as_str(), dest_name.as_str()] {
        rename_in_field(cx, sh, bad);
        assert_eq!(names(cx), [main.clone(), extra.clone()], "{bad:?}를 받았다");
        assert!(
            cx.read(|cx| view.read(cx).message()).is_some(),
            "{bad:?} 거부를 알리지 않았다"
        );
    }
    // Esc는 입력을 버리고 지금 이름으로 되돌린다
    click(cx, sh, "settings-port-name");
    type_text(cx, sh, "zzz");
    press(cx, sh, "escape");
    assert_eq!(names(cx), [main.clone(), extra.clone()]);
    assert_eq!(cx.read(|cx| view.read(cx).port_name_text(cx)), extra);
}

// ---------- 6. 삭제 ----------

#[gpui_kit::test]
fn remove_virtual_port_leaves_tracks_missing(cx: &mut TestAppContext) {
    let main = start(cx, "remove", None);
    cx.update(ports::start_monitor);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    wait_source(&extra, true);
    let (h, view) = open(cx, project_on(&OutputPort::Named(extra.clone())));
    pump(
        cx,
        |cx| cx.read(|cx| ports::port_present(cx, &extra)) == Some(true),
        "앱 포트를 있다고 보지 않는다",
    );
    render(cx, h);
    assert_eq!(label(cx, h, "status-port-missing"), None);
    let (sh, _) = open_settings(cx, h);
    // 기본 포트는 지울 수 없다
    choose_port(cx, sh, &main);
    click(cx, sh, "settings-port-remove");
    assert_eq!(names(cx), [main.clone(), extra.clone()]);
    choose_port(cx, sh, &extra);
    click(cx, sh, "settings-port-remove");
    assert_eq!(names(cx), std::slice::from_ref(&main));
    assert!(midi_output(cx).virtual_ports.is_empty());
    wait_source(&extra, false);
    assert_eq!(track_port(cx, &view), OutputPort::Named(extra.clone()));
    pump(
        cx,
        |cx| {
            render(cx, h);
            label(cx, h, "status-port-missing").is_some()
        },
        "지운 포트의 트랙이 「포트 없음」이 아니다",
    );
}

// ---------- 7. 키보드 ----------

#[gpui_kit::test]
fn settings_midi_output_keyboard_flow(cx: &mut TestAppContext) {
    let main = start(cx, "keys", None);
    let (h, _) = open(cx, project_on(&OutputPort::Virtual));
    let (sh, view) = open_settings(cx, h);
    let focused = |cx: &mut TestAppContext| {
        cx.update_window(sh, |_, window, cx| {
            view.read(cx).focused_control(window, cx)
        })
        .unwrap()
    };
    // 구역을 열면 포트 선택 상자, Tab으로 이름 입력란 → 추가 → 삭제
    assert_eq!(focused(cx).as_deref(), Some("settings-port-select"));
    press(cx, sh, "tab tab");
    assert_eq!(focused(cx).as_deref(), Some("settings-port-add"));
    press(cx, sh, "enter");
    let added = format!("{main} 2");
    assert_eq!(names(cx), [main.clone(), added.clone()]);
    // 추가하면 그 포트를 고르고 이름 입력란으로 간다
    assert_eq!(
        cx.read(|cx| view.read(cx).selected_port()),
        Some(added.clone())
    );
    assert_eq!(focused(cx).as_deref(), Some("settings-port-name"));
    let named = unique("keys-named");
    press(cx, sh, "cmd-a backspace");
    type_text(cx, sh, &named);
    press(cx, sh, "enter");
    assert_eq!(names(cx), [main.clone(), named.clone()]);
    // 선택 상자로 기본 포트를 고르고 다시 추가 포트로
    press(cx, sh, "shift-tab");
    assert_eq!(focused(cx).as_deref(), Some("settings-port-select"));
    press(cx, sh, "enter up enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).selected_port()),
        Some(main.clone())
    );
    press(cx, sh, "space down enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).selected_port()),
        Some(named.clone())
    );
    press(cx, sh, "tab tab tab");
    assert_eq!(focused(cx).as_deref(), Some("settings-port-remove"));
    press(cx, sh, "space");
    assert_eq!(names(cx), std::slice::from_ref(&main));
}

// ---------- 8. 설정에서 만들기 ----------

#[gpui_kit::test]
fn virtual_ports_created_from_settings_at_install(cx: &mut TestAppContext) {
    let extra = unique("from-settings");
    let json = format!(r#"{{"midi_output": {{"virtual_ports": ["{extra}"]}}}}"#);
    let main = start(cx, "install", Some(&json));
    assert_eq!(names(cx), [main, extra.clone()]);
    wait_source(&extra, true);
    let rx = Receiver::source(&extra);
    cx.update(ports::start_monitor);
    let (h, _) = open(cx, project_on(&OutputPort::Named(extra.clone())));
    pump(
        cx,
        |cx| cx.read(|cx| ports::port_present(cx, &extra)) == Some(true),
        "앱 포트를 있다고 보지 않는다",
    );
    render(cx, h);
    assert_eq!(label(cx, h, "status-port-missing"), None);
    run(cx, h, transport::Play);
    assert!(rx.wait_note(WAIT));
    run(cx, h, transport::Stop);
}

// ---------- 3b. 16개 상한(프로세스 전역 상한이라 자식 프로세스에서) ----------

/// 자식 프로세스에서 그 테스트만 돌린다. 부모는 `true`(자식 성공 확인), 자식은 `false`.
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

#[gpui_kit::test]
fn add_virtual_port_stops_at_sixteen(cx: &mut TestAppContext) {
    if run_isolated("add_virtual_port_stops_at_sixteen") {
        return;
    }
    start(cx, "limit", None);
    for _ in 0..15 {
        cx.update(engine::add_virtual_port).unwrap();
    }
    assert_eq!(names(cx).len(), 16);
    assert!(cx.update(engine::add_virtual_port).is_err());
    assert_eq!(names(cx).len(), 16);
    assert_eq!(midi_output(cx).virtual_ports.len(), 15);
}
