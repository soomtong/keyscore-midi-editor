//! S2-T6d 적대적 리뷰 1회차. 가상 포트는 프로세스 전역 16개 상한이 있어 GUI 테스트는 모두 자식 프로세스에서
//! 돈다(`run_isolated`). 도우미는 `tests/virtual_ports.rs`에서 복사했다.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, midi, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::settings_window;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, ports};
use keyscore_core::{OutputPort, Project, Velocity, ksq};

const WAIT: Duration = Duration::from_secs(2);

fn unique(name: &str) -> String {
    format!("KS rv6d {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv6d-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(cx: &mut TestAppContext, tag: &str, settings_json: Option<&str>) -> String {
    let dir = temp_dir(tag);
    if let Some(text) = settings_json {
        std::fs::write(dir.join("settings.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let main = unique(tag);
    let options = EngineOptions {
        client_name: format!("ks-rv6d-{tag}"),
        port_name: main.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    main
}

/// 박마다 노트 하나(32마디). 트랙마다 포트 하나.
fn project_on(ports: &[OutputPort]) -> Project {
    let v = Velocity::from_midi1(100).unwrap().get();
    let notes: Vec<String> = (0..128)
        .map(|i| format!("[{}, 240, 60, {v}]", if i == 0 { 0 } else { 960 }))
        .collect();
    let tracks: Vec<String> = ports
        .iter()
        .enumerate()
        .map(|(i, port)| {
            let port = match port {
                OutputPort::Virtual => "\"virtual\"".to_owned(),
                OutputPort::Named(n) => format!("{{\"named\": \"{n}\"}}"),
            };
            format!(
                r#"{{"name": "T{i}", "port": {port}, "channel": {},
                    "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}}"#,
                i + 1,
                notes.join(", ")
            )
        })
        .collect();
    ksq::from_str(&format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(", ")
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

fn choose_port(cx: &mut TestAppContext, sh: AnyWindowHandle, search: &str) {
    click(cx, sh, "settings-port-select");
    type_text(cx, sh, search);
    press(cx, sh, "enter");
}

fn names(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(engine::virtual_port_names)
}

fn track_ports(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<OutputPort> {
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

fn wait_source(name: &str, present: bool) {
    let deadline = Instant::now() + WAIT;
    while keyscore_midi::source_names().iter().any(|n| n == name) != present {
        assert!(
            Instant::now() < deadline,
            "소스 {name} 상태가 {present}가 아니다"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_destination(name: &str) {
    let deadline = Instant::now() + WAIT;
    while !keyscore_midi::destinations()
        .iter()
        .any(|d| d.name() == name)
    {
        assert!(Instant::now() < deadline, "목적지 {name}이 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 포트 감시 주기를 몇 번 돌린다.
fn pump_monitor(cx: &mut TestAppContext, times: usize) {
    for _ in 0..times {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
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
        let client = coremidi::Client::new("ks-rv6d-rx").unwrap();
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

/// 같은 프로세스의 가상 목적지(외부 신디·IAC 버스 대신).
struct Destination {
    rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

impl Destination {
    fn new(name: &str) -> Destination {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-rv6d-dest").unwrap();
        let dest = client
            .virtual_destination(name, move |list: &coremidi::PacketList| {
                for p in list {
                    for msg in split(p.data()) {
                        let _ = tx.send(msg);
                    }
                }
            })
            .unwrap();
        wait_destination(name);
        Destination {
            rx,
            _dest: dest,
            _client: client,
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

// ---------- 지적: 재생 중 추가·이름 변경이 기다리던 트랙을 잇지 않는다 ----------

/// 재생 중인 트랙이 아직 없는 이름 포트(「기본 이름 2」)를 가리킨다. 그 이름의 가상 포트를 추가하면 상태줄은
/// 「있음」으로 바뀌는데, 소리는 다음 편집·재생 시작까지 나가지 않는다(OUT-3의 「돌아오면 자동 재연결」과 다르다).
#[gpui_kit::test]
fn add_virtual_port_during_playback_reconnects_waiting_track(cx: &mut TestAppContext) {
    if run_isolated("add_virtual_port_during_playback_reconnects_waiting_track") {
        return;
    }
    let main = start(cx, "add-live", None);
    cx.update(ports::start_monitor);
    let wanted = format!("{main} 2");
    let (h, _) = open(cx, project_on(&[OutputPort::Named(wanted.clone())]));
    run(cx, h, transport::Play);
    let added = cx.update(engine::add_virtual_port).unwrap();
    assert_eq!(added, wanted);
    let rx = Receiver::source(&added);
    pump_monitor(cx, 5);
    render(cx, h);
    // 상태줄은 앱 포트를 「있음」으로 본다
    assert_eq!(label(cx, h, "status-port-missing"), None);
    assert!(
        rx.wait_note(WAIT),
        "상태줄은 포트가 있다는데 재생 중인 트랙의 소리가 새 가상 포트로 나가지 않는다"
    );
}

/// 같은 원인: 재생 중 추가 포트의 이름을, 재생 중인 트랙이 기다리던 이름으로 바꾼다.
#[gpui_kit::test]
fn rename_during_playback_reconnects_waiting_track(cx: &mut TestAppContext) {
    if run_isolated("rename_during_playback_reconnects_waiting_track") {
        return;
    }
    let _main = start(cx, "rename-live", None);
    cx.update(ports::start_monitor);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let wanted = unique("rename-live-wanted");
    let (h, _) = open(cx, project_on(&[OutputPort::Named(wanted.clone())]));
    run(cx, h, transport::Play);
    let renamed = cx
        .update(|cx| engine::rename_virtual_port(cx, &extra, &wanted))
        .unwrap();
    assert_eq!(renamed, wanted);
    let rx = Receiver::source(&wanted);
    pump_monitor(cx, 5);
    assert!(
        rx.wait_note(WAIT),
        "재생 중인 트랙이 기다리던 이름으로 바꿨는데 소리가 나가지 않는다"
    );
}

// ---------- 지적: 설정 파일의 추가 포트 이름이 목적지 이름과 같으면 그 목적지를 가로챈다 ----------

/// 설정 파일(손 편집)의 `virtual_ports`에 지금 보이는 목적지 이름이 있다. UI의 추가·이름 변경은 같은 이름을
/// 거부하는데(계획 3의 이름 규칙) 설정 파일 검사는 받아들여, 그 목적지를 쓰던 프로젝트의 소리가 앱 가상 포트로 간다.
#[gpui_kit::test]
fn settings_virtual_port_named_like_destination_is_rejected(cx: &mut TestAppContext) {
    if run_isolated("settings_virtual_port_named_like_destination_is_rejected") {
        return;
    }
    let dest_name = unique("hijack-dest");
    let dest = Destination::new(&dest_name);
    let json = format!(r#"{{"midi_output": {{"virtual_ports": ["{dest_name}"]}}}}"#);
    let main = start(cx, "hijack", Some(&json));
    let (h, _) = open(cx, project_on(&[OutputPort::Named(dest_name.clone())]));
    run(cx, h, transport::Play);
    let got = dest.wait_note(WAIT);
    run(cx, h, transport::Stop);
    assert!(
        got,
        "목적지 「{dest_name}」을 쓰던 트랙의 소리가 목적지로 가지 않는다(가상 포트: {:?})",
        names(cx)
    );
    assert_eq!(
        names(cx),
        [main],
        "목적지와 같은 이름의 가상 포트를 설정 파일에서 만들었다"
    );
}

// ---------- 깨뜨리지 못한 시도 ----------

/// 기본 포트 이름을 프로젝트 창의 Action(설정 창 없음)으로 바꾼다. 기본 포트를 이름으로 가리키던 트랙만 고치고,
/// `Virtual` 트랙은 그대로. undo·redo가 창마다 한 단위.
#[gpui_kit::test]
fn rename_main_from_project_window_updates_named_main_tracks(cx: &mut TestAppContext) {
    if run_isolated("rename_main_from_project_window_updates_named_main_tracks") {
        return;
    }
    let main = start(cx, "rename-main", None);
    let (h, view) = open(
        cx,
        project_on(&[OutputPort::Virtual, OutputPort::Named(main.clone())]),
    );
    let new = unique("rename-main-new");
    run(cx, h, midi::RenameVirtualPort(new.clone()));
    wait_source(&new, true);
    wait_source(&main, false);
    assert_eq!(names(cx), std::slice::from_ref(&new));
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).midi_output.main_name.clone()),
        Some(new.clone())
    );
    assert_eq!(
        track_ports(cx, &view),
        [OutputPort::Virtual, OutputPort::Named(new.clone())]
    );
    run(cx, h, edit::Undo);
    assert_eq!(
        track_ports(cx, &view),
        [OutputPort::Virtual, OutputPort::Named(main.clone())]
    );
    run(cx, h, edit::Redo);
    assert_eq!(
        track_ports(cx, &view),
        [OutputPort::Virtual, OutputPort::Named(new.clone())]
    );
    // 두 트랙 모두 같은 소스로 나간다
    let rx = Receiver::source(&new);
    run(cx, h, transport::Play);
    assert!(rx.wait_note(WAIT));
    run(cx, h, transport::Stop);
}

/// 지운 뒤 곧바로 추가하면 같은 번호를 다시 쓴다(지운 포트가 이름 목록에 남아 있지 않다). 재생 중에도.
#[gpui_kit::test]
fn remove_then_add_reuses_name(cx: &mut TestAppContext) {
    if run_isolated("remove_then_add_reuses_name") {
        return;
    }
    let main = start(cx, "remove-add", None);
    let first = cx.update(engine::add_virtual_port).unwrap();
    let (h, _) = open(cx, project_on(&[OutputPort::Named(first.clone())]));
    run(cx, h, transport::Play);
    cx.update(|cx| engine::remove_virtual_port(cx, &first))
        .unwrap();
    let again = cx.update(engine::add_virtual_port).unwrap();
    assert_eq!(again, format!("{main} 2"));
    run(cx, h, transport::Stop);
    cx.update(|cx| engine::remove_virtual_port(cx, &again))
        .unwrap();
    let third = cx.update(engine::add_virtual_port).unwrap();
    assert_eq!(third, format!("{main} 2"));
    assert_eq!(names(cx), [main, third]);
}

/// 서버 재시작 복구 경로(`recover_virtual_port`)를 서버를 끄지 않고 부른다. 추가 포트도 새 엔드포인트로 다시
/// 등록되어 소리가 나간다.
#[gpui_kit::test]
fn recover_reregisters_extra_ports(cx: &mut TestAppContext) {
    if run_isolated("recover_reregisters_extra_ports") {
        return;
    }
    let _main = start(cx, "recover", None);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let (h, _) = open(cx, project_on(&[OutputPort::Named(extra.clone())]));
    run(cx, h, transport::Play);
    cx.update(engine::recover_virtual_port).unwrap();
    wait_source(&extra, true);
    let rx = Receiver::source(&extra);
    run(cx, h, transport::Play);
    assert!(
        rx.wait_note(WAIT),
        "다시 등록한 추가 포트로 소리가 나가지 않는다"
    );
    run(cx, h, transport::Stop);
}

/// 설정 창이 없을 때 삭제 Action은 설정 창(MIDI 출력)을 열고 아무것도 지우지 않는다. 열린 뒤 기본 포트가
/// 골라져 있으면 키보드(space)로도 삭제 버튼이 꺼져 있다.
#[gpui_kit::test]
fn remove_action_without_settings_window_and_main_by_keyboard(cx: &mut TestAppContext) {
    if run_isolated("remove_action_without_settings_window_and_main_by_keyboard") {
        return;
    }
    let main = start(cx, "remove-action", None);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let (h, _) = open(cx, project_on(&[OutputPort::Virtual]));
    run(cx, h, midi::RemoveVirtualPort);
    assert_eq!(names(cx), [main.clone(), extra.clone()]);
    let (sh, view) = cx
        .update(|cx| settings_window::current(cx))
        .expect("설정 창을 열지 않았다");
    activate(cx, sh);
    choose_port(cx, sh, &main);
    assert_eq!(
        cx.read(|cx| view.read(cx).selected_port()),
        Some(main.clone())
    );
    assert_eq!(
        label(cx, sh, "settings-port-remove").as_deref(),
        Some("삭제(꺼짐)")
    );
    press(cx, sh, "tab tab tab");
    let focused = cx
        .update_window(sh, |_, window, cx| {
            view.read(cx).focused_control(window, cx)
        })
        .unwrap();
    assert_eq!(focused.as_deref(), Some("settings-port-remove"));
    press(cx, sh, "space");
    press(cx, sh, "enter");
    assert_eq!(names(cx), [main, extra]);
}

/// 다른 가상 포트와 보이는 모양만 같은 이름(NBSP, ZWJ 끼움)은 거부한다.
#[gpui_kit::test]
fn rename_rejects_visual_duplicate_of_other_port(cx: &mut TestAppContext) {
    if run_isolated("rename_rejects_visual_duplicate_of_other_port") {
        return;
    }
    let main = start(cx, "visual", None);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let nbsp = main.replacen(' ', "\u{00A0}", 1);
    let zwj = format!("{}\u{200D}{}", &main[..2], &main[2..]);
    for bad in [nbsp, zwj] {
        let r = cx.update(|cx| engine::rename_virtual_port(cx, &extra, &bad));
        assert!(r.is_err(), "{bad:?}를 받았다");
    }
    assert_eq!(names(cx), [main, extra]);
}
