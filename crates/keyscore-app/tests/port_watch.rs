//! S2-T5d 포트 감시와 재연결(OUT-1, OUT-3, UI-5). 같은 프로세스의 `coremidi` 가상 목적지를 다른 앱의 입력
//! 포트처럼 만들고 지웠다 다시 만든다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{track, transport};
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, ports};
use keyscore_core::{Project, Velocity, ksq};

fn unique(tag: &str) -> String {
    format!("KS watch {} {tag}", std::process::id())
}

/// 앱·엔진을 띄우고 감시를 시작한다. 엔진 포트 이름을 돌려준다.
fn start(cx: &mut TestAppContext, tag: &str, monitor: bool) -> String {
    let dir = std::env::temp_dir().join(format!("ks-watch-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(&format!("out {tag}"));
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-watch-{tag}"),
                port_name: port.clone(),
            },
        )
    })
    .unwrap();
    if monitor {
        cx.update(ports::start_monitor);
        pump(
            cx,
            None,
            |cx| cx.read(ports::is_monitoring),
            "감시가 시작되지 않았다",
        );
    }
    port
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

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
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

/// 감시 주기만큼 시계를 돌리며 조건을 기다린다(CoreMIDI 알림은 실제 시간으로 온다).
fn pump(
    cx: &mut TestAppContext,
    handle: Option<AnyWindowHandle>,
    done: impl Fn(&mut TestAppContext) -> bool,
    why: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(4);
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

/// 다른 앱의 입력 포트처럼 보이는 목적지. 받은 메시지를 모은다.
struct Destination {
    rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

fn destination(name: &str) -> Destination {
    let client = coremidi::Client::new(&format!("ks-watch-dest-{name}")).unwrap();
    let (tx, rx) = mpsc::channel();
    let dest = client
        .virtual_destination(name, move |list: &coremidi::PacketList| {
            for p in list {
                for m in split(p.data()) {
                    let _ = tx.send(m);
                }
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !keyscore_midi::destinations()
        .iter()
        .any(|d| d.name() == name)
    {
        assert!(Instant::now() < deadline, "목적지 {name}가 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    }
    Destination {
        rx,
        _dest: dest,
        _client: client,
    }
}

fn wait_gone(name: &str) {
    let deadline = Instant::now() + Duration::from_secs(2);
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

/// 이름 포트 트랙 하나 + 가상 포트 트랙 하나. 이름 포트 트랙은 박마다 노트(32마디). `.ksq` 노트 시작은
/// 앞 노트에서의 차이다.
fn project_on(name: &str) -> Project {
    let v = Velocity::from_midi1(100).unwrap().get();
    let notes: Vec<String> = (0..128)
        .map(|i| format!("[{}, 240, 60, {v}]", if i == 0 { 0 } else { 960 }))
        .collect();
    ksq::from_str(&format!(
        r#"{{"tracks": [{{"name": "Ext", "port": {{"named": "{name}"}}, "channel": 2, "program": 5,
            "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}},
            {{"name": "Main", "channel": 1}}]}}"#,
        notes.join(", ")
    ))
    .unwrap()
    .0
}

/// 감시가 그 목적지를 보게 될 때까지(알림은 목록보다 늦게 온다).
fn wait_seen(cx: &mut TestAppContext, name: &str, present: bool) {
    pump(
        cx,
        None,
        |cx| cx.read(|cx| ports::port_present(cx, name)) == Some(present),
        "감시가 목적지 변화를 보지 못했다",
    );
}

// ---------- 1. 상태줄의 「포트 없음」 ----------

#[gpui_kit::test]
fn missing_port_shows_in_status_bar(cx: &mut TestAppContext) {
    start(cx, "missing", true);
    let (handle, _view) = open(cx, project_on(&unique("never there")));
    pump(
        cx,
        Some(handle),
        |cx| label(cx, handle, "status-port-missing").is_some(),
        "없는 이름 포트인데 상태줄에 「포트 없음」이 없다",
    );
    assert_eq!(
        label(cx, handle, "status-port-missing").as_deref(),
        Some("포트 없음")
    );
    run(cx, handle, track::Next);
    assert_eq!(label(cx, handle, "status-port-missing"), None);
}

/// 엔진이 없으면 감시도 없다.
#[gpui_kit::test]
fn monitor_without_engine_is_inert(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("ks-watch-{}-noengine", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    cx.update(ports::start_monitor);
    cx.executor().advance_clock(ports::POLL_INTERVAL);
    cx.run_until_parked();
    assert!(!cx.read(ports::is_monitoring));
    let (handle, _) = open(cx, project_on(&unique("noengine")));
    assert_eq!(label(cx, handle, "status-port-missing"), None);
}

// ---------- 2. 사라짐과 돌아옴 ----------

#[gpui_kit::test]
fn port_vanish_and_return_updates_status_and_messages(cx: &mut TestAppContext) {
    start(cx, "vanish", true);
    let name = unique("synth vanish");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (handle, view) = open(cx, project_on(&name));
    pump(cx, Some(handle), |_| true, "");
    assert_eq!(label(cx, handle, "status-port-missing"), None);
    // 쓰지 않는 포트의 변화는 알리지 않는다
    let before = messages(cx, &view).len();
    let other_name = unique("unrelated");
    let other = destination(&other_name);
    drop(other);
    wait_gone(&other_name);
    for _ in 0..5 {
        pump(cx, Some(handle), |_| true, "");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(messages(cx, &view).len(), before);
    // 쓰는 포트가 사라진다
    drop(dest);
    wait_gone(&name);
    pump(
        cx,
        Some(handle),
        |cx| {
            messages(cx, &view)
                .iter()
                .any(|m| m.contains(&name) && m.contains("사라졌다"))
        },
        "사라짐 메시지가 없다",
    );
    assert_eq!(
        label(cx, handle, "status-port-missing").as_deref(),
        Some("포트 없음")
    );
    // 같은 이름으로 돌아온다
    let _back = destination(&name);
    pump(
        cx,
        Some(handle),
        |cx| {
            messages(cx, &view)
                .iter()
                .any(|m| m.contains(&name) && m.contains("다시 연결했다"))
        },
        "다시 연결 메시지가 없다",
    );
    assert_eq!(label(cx, handle, "status-port-missing"), None);
}

// ---------- 3. 재생 중 재연결 ----------

#[gpui_kit::test]
fn playback_reconnects_to_returned_port(cx: &mut TestAppContext) {
    start(cx, "replay", true);
    let name = unique("synth replay");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (handle, view) = open(cx, project_on(&name));
    run(cx, handle, transport::Play);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(m) = dest.rx.recv_timeout(Duration::from_millis(50))
            && m[0] == 0x91
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "재생 노트가 첫 목적지에 닿지 않았다"
        );
    }
    drop(dest);
    wait_gone(&name);
    pump(
        cx,
        Some(handle),
        |cx| messages(cx, &view).iter().any(|m| m.contains("사라졌다")),
        "사라짐 메시지가 없다",
    );
    let back = destination(&name);
    pump(
        cx,
        Some(handle),
        |cx| {
            messages(cx, &view)
                .iter()
                .any(|m| m.contains("다시 연결했다"))
        },
        "다시 연결 메시지가 없다",
    );
    // 새 목적지로 프로그램(5)이 먼저, 그 뒤 노트가 닿는다
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut got = Vec::new();
    while Instant::now() < deadline && !got.iter().any(|m: &Vec<u8>| m[0] == 0x91) {
        if let Ok(m) = back.rx.recv_timeout(Duration::from_millis(50)) {
            got.push(m);
        }
    }
    run(cx, handle, transport::Stop);
    let program = got.iter().position(|m| m == &[0xC1, 5]);
    let note = got.iter().position(|m| m[0] == 0x91);
    assert!(program.is_some() && note.is_some(), "{got:?}");
    assert!(program < note, "{got:?}");
}

// ---------- 4. 연 포트 정리 ----------

#[gpui_kit::test]
fn unused_opened_ports_are_released(cx: &mut TestAppContext) {
    start(cx, "release", false);
    let a = unique("synth a");
    let b = unique("synth b");
    let dest_a = destination(&a);
    let _dest_b = destination(&b);
    let both = ksq::from_str(&format!(
        r#"{{"tracks": [{{"port": {{"named": "{a}"}}, "channel": 1}},
            {{"port": {{"named": "{b}"}}, "channel": 2}}]}}"#
    ))
    .unwrap()
    .0;
    let (handle, view) = open(cx, both);
    run(cx, handle, transport::Play);
    run(cx, handle, transport::Stop);
    let mut names = cx.read(engine::opened_ports);
    names.sort();
    let mut want = vec![a.clone(), b.clone()];
    want.sort();
    assert_eq!(names, want);
    // b를 쓰는 트랙을 지우고 다시 재생: b 핸들을 놓는다
    run(cx, handle, track::Next);
    run(cx, handle, track::Delete);
    run(cx, handle, transport::Play);
    run(cx, handle, transport::Stop);
    assert_eq!(cx.read(engine::opened_ports), vec![a.clone()]);
    // a가 사라지면(감시 없이) 다음 재생 준비가 핸들을 확인해 놓고 「없어」를 알린다
    drop(dest_a);
    wait_gone(&a);
    run(cx, handle, transport::Play);
    run(cx, handle, transport::Stop);
    assert!(cx.read(engine::opened_ports).is_empty());
    assert!(
        messages(cx, &view)
            .iter()
            .any(|m| m.contains(&a) && m.contains("없어"))
    );
    assert_eq!(
        cx.read(|cx| view.read(cx).transport_state()),
        TransportState::Stopped
    );
}

// ---------- 5. MIDI 서버 재시작 복구 ----------

#[gpui_kit::test]
fn recover_virtual_port_stops_playback_and_reregisters(cx: &mut TestAppContext) {
    let port = start(cx, "recover", true);
    let p = ksq::from_str(&format!(
        r#"{{"tracks": [{{"channel": 1, "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}}]}}"#,
        (0..128)
            .map(|i| format!("[{}, 240, 60, 51492]", if i == 0 { 0 } else { 960 }))
            .collect::<Vec<_>>()
            .join(", ")
    ))
    .unwrap()
    .0;
    let (handle, view) = open(cx, p);
    run(cx, handle, transport::Play);
    cx.update(engine::recover_virtual_port).unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(
        cx.read(|cx| view.read(cx).transport_state()),
        TransportState::Stopped
    );
    let m = messages(cx, &view);
    assert!(m.iter().any(|m| m.contains("재생을 멈췄다")), "{m:?}");
    assert!(
        m.iter()
            .any(|m| m.contains(&port) && m.contains("다시 등록했다")),
        "{m:?}"
    );
    // 같은 이름의 소스가 하나 있고, 다시 재생하면 노트가 그 소스로 나온다
    let sources: Vec<_> = coremidi::Sources
        .into_iter()
        .filter(|s| s.name().as_deref() == Some(port.as_str()))
        .collect();
    assert_eq!(sources.len(), 1);
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-watch-recover-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
            }
        })
        .unwrap();
    input.connect_source(&sources[0]).unwrap();
    run(cx, handle, transport::Play);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut heard = false;
    while Instant::now() < deadline && !heard {
        if let Ok(d) = rx.recv_timeout(Duration::from_millis(50)) {
            heard = split(&d).iter().any(|m| m[0] == 0x90);
        }
    }
    run(cx, handle, transport::Stop);
    assert!(heard, "다시 등록한 가상 포트로 노트가 나오지 않았다");
    // 감시는 다시 시작되어 계속 돈다
    pump(
        cx,
        Some(handle),
        |cx| cx.read(ports::is_monitoring),
        "감시가 다시 시작되지 않았다",
    );
}
