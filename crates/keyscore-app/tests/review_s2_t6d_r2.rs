//! S2-T6d 적대적 리뷰 2회차. 가상 포트는 프로세스 전역 16개 상한이 있어 GUI 테스트는 모두 자식 프로세스에서
//! 돈다(`run_isolated`). 도우미는 `tests/review_s2_t6d.rs`에서 복사했다.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, ports};
use keyscore_core::{OutputPort, Project, Velocity, ksq};

const WAIT: Duration = Duration::from_secs(2);

fn unique(name: &str) -> String {
    format!("KS rv6d2 {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv6d2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn init_with(cx: &mut TestAppContext, tag: &str, settings_json: Option<&str>) {
    let dir = temp_dir(tag);
    if let Some(text) = settings_json {
        std::fs::write(dir.join("settings.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn start(cx: &mut TestAppContext, tag: &str, settings_json: Option<&str>) -> String {
    init_with(cx, tag, settings_json);
    let main = unique(tag);
    let options = EngineOptions {
        client_name: format!("ks-rv6d2-{tag}"),
        port_name: main.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    main
}

fn port_json(port: &OutputPort) -> String {
    match port {
        OutputPort::Virtual => "\"virtual\"".to_owned(),
        OutputPort::Named(n) => format!("{{\"named\": \"{n}\"}}"),
    }
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
            format!(
                r#"{{"name": "T{i}", "port": {}, "channel": {}, "program": 5,
                    "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}}"#,
                port_json(port),
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

/// 트랙 하나, 틱 0에 긴 노트 하나(8박 = 4 s).
fn long_note_on(port: &OutputPort) -> Project {
    let v = Velocity::from_midi1(100).unwrap().get();
    ksq::from_str(&format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{{"name": "L", "port": {}, "channel": 1,
            "clips": [{{"start": 0, "length": 15360, "notes": [[0, 7680, 60, {v}]]}}]}}]}}"#,
        port_json(port)
    ))
    .unwrap()
    .0
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
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

fn names(cx: &mut TestAppContext) -> Vec<String> {
    cx.read(engine::virtual_port_names)
}

fn state(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> TransportState {
    cx.read(|cx| view.read(cx).transport_state())
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

fn is_note_on(m: &[u8]) -> bool {
    m.len() == 3 && m[0] & 0xF0 == 0x90 && m[2] > 0
}

fn is_note_off(m: &[u8]) -> bool {
    m.len() == 3 && (m[0] & 0xF0 == 0x80 || (m[0] & 0xF0 == 0x90 && m[2] == 0))
}

/// 받은 메시지 흐름(앱 소스 또는 같은 프로세스의 가상 목적지).
struct Inbox {
    rx: mpsc::Receiver<Vec<u8>>,
}

impl Inbox {
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

    /// `limit` 동안 받은 것을 모두 모은다.
    fn collect(&self, limit: Duration) -> Vec<Vec<u8>> {
        let deadline = Instant::now() + limit;
        let mut out = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(m) => out.push(m),
                Err(_) => return out,
            }
        }
    }
}

struct Receiver {
    inbox: Inbox,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

impl Receiver {
    fn source(name: &str) -> Receiver {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-rv6d2-rx").unwrap();
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
            inbox: Inbox { rx },
            _input: input,
            _client: client,
        }
    }
}

/// 같은 프로세스의 가상 목적지(외부 신디·IAC 버스 대신).
struct Destination {
    inbox: Inbox,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

impl Destination {
    fn new(name: &str) -> Destination {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-rv6d2-dest").unwrap();
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
            inbox: Inbox { rx },
            _dest: dest,
            _client: client,
        }
    }
}

// ---------- CoreMIDI 직접 호출(앱 소스 하나만 등록을 푼다) ----------

#[link(name = "CoreMIDI", kind = "framework")]
unsafe extern "C" {
    fn MIDIObjectFindByUniqueID(id: i32, object: *mut u32, kind: *mut i32) -> i32;
    fn MIDIEndpointDispose(endpoint: u32) -> i32;
}

/// 이름이 `name`인 소스 엔드포인트를 해제한다. 서버 재시작 없이 그 앱 가상 포트 하나만 「등록이 풀린」 상태를
/// 만든다(앱의 `VirtualOutput`은 그대로 남는다).
#[allow(
    unsafe_code,
    reason = "리뷰 테스트가 한 엔드포인트만 해제하려고 CoreMIDI를 직접 부른다"
)]
fn dispose_source(name: &str) {
    let source = coremidi::Source::from_name(name).expect("소스가 없다");
    let id = source.unique_id().expect("고유 ID가 없다");
    let mut object = 0u32;
    let mut kind = 0i32;
    // SAFETY: 출력 포인터는 이 스택 변수를 가리킨다. 고유 ID는 방금 읽은 값이다.
    let status =
        unsafe { MIDIObjectFindByUniqueID(id.cast_signed(), &raw mut object, &raw mut kind) };
    assert_eq!(status, 0, "엔드포인트를 찾지 못했다");
    // SAFETY: 이 프로세스의 클라이언트가 만든 가상 소스 엔드포인트다.
    let status = unsafe { MIDIEndpointDispose(object) };
    assert_eq!(status, 0, "엔드포인트를 해제하지 못했다");
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

// ---------- 지적: 설정 파일의 기본 포트 이름이 목적지 이름이면 그 목적지를 가로챈다 ----------

/// 리뷰 1 지적 2의 `main_name` 쪽. 설치 검사는 `virtual_ports`만 보고, 바이너리(`run`)는 `main_name`을 그대로
/// 기본 포트 이름으로 준다. 지금 보이는 목적지 이름이면 그 목적지를 쓰던 트랙이 기본 포트(포트 0)로 간다.
#[gpui_kit::test]
fn settings_main_name_named_like_destination_does_not_hijack(cx: &mut TestAppContext) {
    if run_isolated("settings_main_name_named_like_destination_does_not_hijack") {
        return;
    }
    let dest_name = unique("main-hijack-dest");
    let dest = Destination::new(&dest_name);
    let json = format!(r#"{{"midi_output": {{"main_name": "{dest_name}"}}}}"#);
    init_with(cx, "main-hijack", Some(&json));
    // `run`과 같은 길: 설정의 기본 포트 이름으로 설치한다
    let options = EngineOptions {
        client_name: "ks-rv6d2-main-hijack".to_owned(),
        port_name: cx.read(|cx| AppSettings::get(cx).midi_output.main_or_default()),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let (h, _) = open(cx, project_on(&[OutputPort::Named(dest_name.clone())]));
    run(cx, h, transport::Play);
    let got = dest.inbox.wait_note(WAIT);
    run(cx, h, transport::Stop);
    assert!(
        got,
        "목적지 「{dest_name}」을 쓰던 트랙의 소리가 목적지로 가지 않는다(가상 포트: {:?})",
        names(cx)
    );
}

// ---------- 지적: 추가 포트 하나만 등록이 풀려도 복구가 재생을 멈춘다 ----------

/// 제안 3 대응으로 서버 확인이 추가 포트까지 본다(`all_registered`). 그런데 복구(`recover`)는 기본 포트가 멀쩡해도
/// 늘 재생을 멈추고 「MIDI 서버가 다시 시작되어 재생을 멈췄다」를 남긴다. 기본 포트만 쓰는 재생이 아무 관련 없는
/// 추가 포트 때문에 멈춘다. 추가 포트 재등록이 계속 실패하면 서버 확인 주기마다 되풀이된다(감시자도 다시 시작하지
/// 않는다).
#[gpui_kit::test]
fn extra_port_unregistered_does_not_stop_playback_on_main(cx: &mut TestAppContext) {
    if run_isolated("extra_port_unregistered_does_not_stop_playback_on_main") {
        return;
    }
    let main = start(cx, "extra-lost", None);
    cx.update(ports::start_monitor);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let (h, view) = open(cx, project_on(&[OutputPort::Virtual]));
    let rx = Receiver::source(&main);
    run(cx, h, transport::Play);
    assert!(rx.inbox.wait_note(WAIT));
    dispose_source(&extra);
    // 서버 확인은 감시 10주기마다다
    pump_monitor(cx, 25);
    assert_eq!(
        state(cx, &view),
        TransportState::Playing,
        "기본 포트로 재생 중이었는데 추가 포트 「{extra}」의 등록이 풀리자 재생이 멈췄다. 메시지: {:?}",
        messages(cx, &view)
    );
    run(cx, h, transport::Stop);
}

// ---------- 제안: 재생 중 이름 변경이 같은 장치로 뱅크·프로그램을 다시 보낸다 ----------

/// 이름 변경은 엔드포인트를 그대로 두어 「재생 중에도 끊기지 않는다」(계획 4). 그런데 트랙 편집으로 `Setup`의
/// 목적지 문자열이 바뀌어, 같은 장치·같은 채널·같은 프로그램인데 프로그램 체인지를 다시 보낸다.
#[gpui_kit::test]
fn rename_during_playback_does_not_resend_program(cx: &mut TestAppContext) {
    if run_isolated("rename_during_playback_does_not_resend_program") {
        return;
    }
    let _main = start(cx, "rename-program", None);
    cx.update(ports::start_monitor);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let (h, _) = open(cx, project_on(&[OutputPort::Named(extra.clone())]));
    let rx = Receiver::source(&extra);
    run(cx, h, transport::Play);
    assert!(rx.inbox.wait_note(WAIT));
    // 재생 시작의 chase(프로그램)를 비운다
    let _ = rx.inbox.collect(Duration::from_millis(300));
    let new = unique("rename-program-new");
    cx.update(|cx| engine::rename_virtual_port(cx, &extra, &new))
        .unwrap();
    pump_monitor(cx, 3);
    let after = rx.inbox.collect(Duration::from_millis(500));
    run(cx, h, transport::Stop);
    let programs: Vec<&Vec<u8>> = after.iter().filter(|m| m[0] & 0xF0 == 0xC0).collect();
    assert!(
        after.iter().any(|m| is_note_on(m)),
        "이름을 바꾼 뒤 소리가 끊겼다"
    );
    assert!(
        programs.is_empty(),
        "같은 장치·같은 프로그램인데 이름 변경 뒤 프로그램 체인지를 다시 보냈다: {programs:?}"
    );
}

// ---------- 깨뜨리지 못한 시도 ----------

/// 재생 중 아무 트랙도 쓰지 않는 포트를 추가한다(`ports_edited`의 교체). 목적지에서 울리는 긴 노트가 끊기지 않는다.
#[gpui_kit::test]
fn add_unused_port_during_playback_keeps_sounding_note(cx: &mut TestAppContext) {
    if run_isolated("add_unused_port_during_playback_keeps_sounding_note") {
        return;
    }
    let dest_name = unique("keep-dest");
    let dest = Destination::new(&dest_name);
    let _main = start(cx, "keep", None);
    cx.update(ports::start_monitor);
    let (h, _) = open(cx, long_note_on(&OutputPort::Named(dest_name.clone())));
    run(cx, h, transport::Play);
    assert!(dest.inbox.wait_note(WAIT));
    for _ in 0..3 {
        cx.update(engine::add_virtual_port).unwrap();
    }
    let last = names(cx).last().cloned().unwrap();
    cx.update(|cx| engine::rename_virtual_port(cx, &last, &unique("keep-renamed")))
        .unwrap();
    let after = dest.inbox.collect(Duration::from_millis(600));
    run(cx, h, transport::Stop);
    assert!(
        !after.iter().any(|m| is_note_off(m)),
        "쓰지 않는 포트를 추가·이름 변경했는데 목적지의 울리는 노트가 끊겼다: {after:?}"
    );
}

/// 재생 중 기본 포트 이름을, 재생 중인 트랙이 기다리던 이름으로 바꾼다. 그 트랙이 곧바로 기본 포트로 나간다.
#[gpui_kit::test]
fn rename_main_during_playback_reconnects_waiting_track(cx: &mut TestAppContext) {
    if run_isolated("rename_main_during_playback_reconnects_waiting_track") {
        return;
    }
    let main = start(cx, "rename-main-live", None);
    cx.update(ports::start_monitor);
    let wanted = unique("rename-main-live-wanted");
    let (h, _) = open(cx, project_on(&[OutputPort::Named(wanted.clone())]));
    let rx = Receiver::source(&main);
    run(cx, h, transport::Play);
    let _ = rx.inbox.collect(Duration::from_millis(300));
    cx.update(|cx| engine::rename_virtual_port(cx, &main, &wanted))
        .unwrap();
    pump_monitor(cx, 3);
    let got = rx.inbox.wait_note(WAIT);
    run(cx, h, transport::Stop);
    assert!(
        got,
        "기본 포트 이름을 기다리던 이름으로 바꿨는데 소리가 나가지 않는다"
    );
}

/// 정지 중 추가 포트를 지울 때 상태줄 메시지가 겹치지 않는다(설정 창 없이 Action 경로가 아니라 엔진 API, 창 하나).
#[gpui_kit::test]
fn remove_while_stopped_says_nothing_twice(cx: &mut TestAppContext) {
    if run_isolated("remove_while_stopped_says_nothing_twice") {
        return;
    }
    let _main = start(cx, "remove-stopped", None);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let (h, view) = open(cx, project_on(&[OutputPort::Named(extra.clone())]));
    run(cx, h, transport::Play);
    run(cx, h, transport::Stop);
    let before = messages(cx, &view).len();
    cx.update(|cx| engine::remove_virtual_port(cx, &extra))
        .unwrap();
    cx.run_until_parked();
    let after = messages(cx, &view);
    assert_eq!(
        after.len(),
        before,
        "정지 중 삭제가 메시지를 남겼다: {after:?}"
    );
    // 지운 소스는 사라진다
    let deadline = Instant::now() + WAIT;
    while keyscore_midi::source_names().contains(&extra) {
        assert!(Instant::now() < deadline, "지운 소스가 남았다");
        std::thread::sleep(Duration::from_millis(10));
    }
}
