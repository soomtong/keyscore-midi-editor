//! S2-T6d 적대적 리뷰 3회차. 가상 포트는 프로세스 전역 16개 상한이 있어 GUI 테스트는 모두 자식 프로세스에서
//! 돈다(`run_isolated`). 도우미는 `tests/review_s2_t6d_r2.rs`에서 복사했다.

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
    format!("KS rv6d3 {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv6d3-{}-{name}", std::process::id()));
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
        client_name: format!("ks-rv6d3-{tag}"),
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
        let client = coremidi::Client::new("ks-rv6d3-rx").unwrap();
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
        let client = coremidi::Client::new("ks-rv6d3-dest").unwrap();
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

fn wait_source(name: &str, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if keyscore_midi::source_names().iter().any(|n| n == name) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 채널 `ch`(1–16)의 Note On을 `limit` 안에 받는가.
fn wait_note_on_channel(inbox: &Inbox, ch: u8, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match inbox.rx.recv_timeout(left) {
            Ok(m) if is_note_on(&m) && m[0] & 0x0F == ch - 1 => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

// ---------- 지적: 지난 재생이 쓴 추가 포트는 정지해도 다시 등록되지 않는다 ----------

/// 리뷰 2 지적 2 대응의 `reregister_extras`는 스냅샷이 쥔 포트를 「놓일 때까지(재생을 멈추거나 …)」 미룬다. 그런데
/// 정지(`stop`)는 스케줄러를 멈출 뿐 스냅샷을 바꾸지 않아, 지난 재생이 쓴 포트는 정지 뒤에도 스냅샷에 남는다. 그
/// 포트는 서버 확인을 몇 번 거쳐도 다시 등록되지 않고, 다시 재생하면(그 스냅샷이 또 쥔다) 그 트랙은 알림 없이
/// 소리가 나지 않는다.
#[gpui_kit::test]
fn lost_extra_port_used_by_last_playback_is_reregistered_after_stop(cx: &mut TestAppContext) {
    if run_isolated("lost_extra_port_used_by_last_playback_is_reregistered_after_stop") {
        return;
    }
    let _main = start(cx, "held-lost", None);
    cx.update(ports::start_monitor);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let (h, view) = open(cx, project_on(&[OutputPort::Named(extra.clone())]));
    let rx = Receiver::source(&extra);
    run(cx, h, transport::Play);
    assert!(rx.inbox.wait_note(WAIT));
    dispose_source(&extra);
    pump_monitor(cx, 3);
    run(cx, h, transport::Stop);
    // 정지 뒤 서버 확인(감시 10주기마다)을 여러 번 거친다
    pump_monitor(cx, 35);
    let back = wait_source(&extra, Duration::from_millis(500));
    assert!(
        back,
        "정지한 뒤 서버 확인을 세 번 거쳤는데 추가 포트 「{extra}」이 다시 등록되지 않았다. 메시지: {:?}",
        messages(cx, &view)
    );
    // 다시 재생하면 그 포트로 소리가 나간다
    let rx = Receiver::source(&extra);
    run(cx, h, transport::Play);
    let got = rx.inbox.wait_note(WAIT);
    run(cx, h, transport::Stop);
    assert!(got, "다시 등록한 추가 포트로 소리가 나가지 않는다");
}

// ---------- 제안: 기본 포트를 기본 이름으로 대신 만들 때 추가 포트 이름과 겹친다 ----------

/// 리뷰 2 지적 1 대응은 설정 `main_name`이 목적지 이름이면 기본 포트를 OUT-1 기본 이름으로 만든다. 그 이름이
/// 설정의 추가 포트 이름이면(설정 읽기는 추가 포트를 `main_name`과만 견준다) 그 추가 포트는 이름 중복으로 만들지
/// 못하고, `Named(기본 이름)` 트랙은 기본 포트로 간다.
#[gpui_kit::test]
fn main_fallback_name_does_not_drop_extra_port(cx: &mut TestAppContext) {
    if run_isolated("main_fallback_name_does_not_drop_extra_port") {
        return;
    }
    let dest_name = unique("fallback-dest");
    let _dest = Destination::new(&dest_name);
    let json = format!(
        r#"{{"midi_output": {{"main_name": "{dest_name}", "virtual_ports": ["{}"]}}}}"#,
        keyscore_midi::DEFAULT_PORT_NAME
    );
    init_with(cx, "fallback-extra", Some(&json));
    let options = EngineOptions {
        client_name: "ks-rv6d3-fallback-extra".to_owned(),
        port_name: cx.read(|cx| AppSettings::get(cx).midi_output.main_or_default()),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let got = names(cx);
    assert_eq!(
        got.len(),
        2,
        "기본 포트를 기본 이름으로 대신 만들면서 설정의 추가 포트 「{}」을 만들지 못했다: {got:?}",
        keyscore_midi::DEFAULT_PORT_NAME
    );
}

// ---------- 깨뜨리지 못한 시도 ----------

/// 기본 이름으로 대신 만든 뒤에도 설정·이름 목록·이름 변경·`Named(설정 이름)` 트랙이 어긋나지 않는다.
#[gpui_kit::test]
fn main_fallback_stays_consistent_through_rename(cx: &mut TestAppContext) {
    if run_isolated("main_fallback_stays_consistent_through_rename") {
        return;
    }
    let dest_name = unique("fallback-rename-dest");
    let dest = Destination::new(&dest_name);
    let json = format!(r#"{{"midi_output": {{"main_name": "{dest_name}"}}}}"#);
    init_with(cx, "fallback-rename", Some(&json));
    let options = EngineOptions {
        client_name: "ks-rv6d3-fallback-rename".to_owned(),
        port_name: cx.read(|cx| AppSettings::get(cx).midi_output.main_or_default()),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    assert_eq!(names(cx), vec![keyscore_midi::DEFAULT_PORT_NAME.to_owned()]);
    // 설정은 그대로다
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).midi_output.main_name.clone()),
        Some(dest_name.clone())
    );
    assert!(!cx.read(|cx| engine::is_own_port(cx, &dest_name)));
    let (h, _) = open(
        cx,
        project_on(&[OutputPort::Named(dest_name.clone()), OutputPort::Virtual]),
    );
    let new = unique("fallback-rename-new");
    cx.update(|cx| engine::rename_virtual_port(cx, keyscore_midi::DEFAULT_PORT_NAME, &new))
        .unwrap();
    assert_eq!(names(cx), vec![new.clone()]);
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).midi_output.main_name.clone()),
        Some(new.clone())
    );
    let rx = Receiver::source(&new);
    run(cx, h, transport::Play);
    let to_dest = dest.inbox.wait_note(WAIT);
    let to_main = wait_note_on_channel(&rx.inbox, 2, WAIT);
    run(cx, h, transport::Stop);
    assert!(to_dest, "Named(설정 이름) 트랙이 목적지로 가지 않는다");
    assert!(
        to_main,
        "Virtual 트랙이 이름을 바꾼 기본 포트로 가지 않는다"
    );
}

/// 재생 창의 트랙을 고친 이름 변경(`refresh_owner` 거짓): 새 이름을 기다리던 같은 창의 다른 트랙도 곧바로 잇는다.
#[gpui_kit::test]
fn rename_with_owner_edit_reconnects_waiting_track(cx: &mut TestAppContext) {
    if run_isolated("rename_with_owner_edit_reconnects_waiting_track") {
        return;
    }
    let _main = start(cx, "owner-edit", None);
    cx.update(ports::start_monitor);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let new = unique("owner-edit-new");
    let (h, _) = open(
        cx,
        project_on(&[
            OutputPort::Named(extra.clone()),
            OutputPort::Named(new.clone()),
        ]),
    );
    let rx = Receiver::source(&extra);
    run(cx, h, transport::Play);
    assert!(wait_note_on_channel(&rx.inbox, 1, WAIT));
    cx.update(|cx| engine::rename_virtual_port(cx, &extra, &new))
        .unwrap();
    pump_monitor(cx, 3);
    let waiting = wait_note_on_channel(&rx.inbox, 2, WAIT);
    run(cx, h, transport::Stop);
    assert!(waiting, "새 이름을 기다리던 트랙(채널 2)이 이어지지 않는다");
}

/// 재생 창이 아닌 창의 트랙만 고친 이름 변경: 재생 창은 `ports_edited`가 교체해 기다리던 트랙을 잇는다.
#[gpui_kit::test]
fn rename_editing_only_other_window_refreshes_owner(cx: &mut TestAppContext) {
    if run_isolated("rename_editing_only_other_window_refreshes_owner") {
        return;
    }
    let _main = start(cx, "other-edit", None);
    cx.update(ports::start_monitor);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let new = unique("other-edit-new");
    let (h, _) = open(cx, project_on(&[OutputPort::Named(new.clone())]));
    let (_h2, other) = open(cx, project_on(&[OutputPort::Named(extra.clone())]));
    let rx = Receiver::source(&extra);
    cx.update_window(h, |_, window, _| window.activate_window())
        .unwrap();
    run(cx, h, transport::Play);
    let _ = rx.inbox.collect(Duration::from_millis(300));
    cx.update(|cx| engine::rename_virtual_port(cx, &extra, &new))
        .unwrap();
    pump_monitor(cx, 3);
    let got = rx.inbox.wait_note(WAIT);
    run(cx, h, transport::Stop);
    let edited = cx.read(|cx| {
        other.read(cx).document().read(cx).project().tracks()[0].port()
            == &OutputPort::Named(new.clone())
    });
    assert!(edited, "다른 창의 트랙 포트가 새 이름으로 바뀌지 않았다");
    assert!(
        got,
        "다른 창만 고친 이름 변경 뒤 재생 창의 기다리던 트랙이 이어지지 않는다"
    );
}

/// 기본 포트로 재생 중 쓰지 않는 추가 포트의 등록이 풀리면, 재생은 그대로이고 그 포트는 다시 등록된다.
#[gpui_kit::test]
fn unused_extra_port_lost_is_reregistered_during_playback(cx: &mut TestAppContext) {
    if run_isolated("unused_extra_port_lost_is_reregistered_during_playback") {
        return;
    }
    let main = start(cx, "unused-lost", None);
    cx.update(ports::start_monitor);
    let extra = cx.update(engine::add_virtual_port).unwrap();
    let (h, view) = open(cx, project_on(&[OutputPort::Virtual]));
    let rx = Receiver::source(&main);
    run(cx, h, transport::Play);
    assert!(rx.inbox.wait_note(WAIT));
    dispose_source(&extra);
    pump_monitor(cx, 25);
    let back = wait_source(&extra, Duration::from_millis(500));
    let playing = state(cx, &view);
    let said = messages(cx, &view);
    let _ = rx.inbox.collect(Duration::from_millis(100));
    let sounding = rx.inbox.wait_note(WAIT);
    run(cx, h, transport::Stop);
    assert_eq!(playing, TransportState::Playing);
    assert!(sounding, "기본 포트 소리가 끊겼다");
    assert!(back, "쓰지 않는 추가 포트가 다시 등록되지 않았다: {said:?}");
    assert!(
        said.iter()
            .any(|m| m.contains(&extra) && m.contains("다시 등록")),
        "다시 등록을 알리지 않았다: {said:?}"
    );
}
