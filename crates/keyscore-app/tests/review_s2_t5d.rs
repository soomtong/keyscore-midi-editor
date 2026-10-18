//! S2-T5d 적대적 리뷰 1. 깨뜨리지 못한 시도를 남긴다. 깨진 시도는 `docs/reports/s2-t5d/review-1.md`
//! 부록에 있다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, midi, track, transport};
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, ports};
use keyscore_core::{Project, Velocity, ksq};

fn unique(tag: &str) -> String {
    format!("KS rv5d {} {tag}", std::process::id())
}

fn start(cx: &mut TestAppContext, tag: &str, monitor: bool) -> String {
    let dir = std::env::temp_dir().join(format!("ks-rv5d-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(&format!("out {tag}"));
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-rv5d-{tag}"),
                port_name: port.clone(),
            },
        )
    })
    .unwrap();
    if monitor {
        start_monitor(cx);
    }
    port
}

fn start_monitor(cx: &mut TestAppContext) {
    cx.update(ports::start_monitor);
    pump(
        cx,
        None,
        |cx| cx.read(ports::is_monitoring),
        "감시가 시작되지 않았다",
    );
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

fn state(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> TransportState {
    cx.read(|cx| view.read(cx).transport_state())
}

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

/// 조건 없이 감시 몇 바퀴를 돌린다.
fn idle(cx: &mut TestAppContext, handle: Option<AnyWindowHandle>, rounds: usize) {
    for _ in 0..rounds {
        pump(cx, handle, |_| true, "");
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct Destination {
    rx: mpsc::Receiver<Vec<u8>>,
    dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

fn count_named(name: &str) -> usize {
    keyscore_midi::destinations()
        .iter()
        .filter(|d| d.name() == name)
        .count()
}

fn destination(name: &str) -> Destination {
    destination_with(name, None)
}

/// 목적지를 만든다. `id`가 있으면 고유 ID를 그 값으로 정한다.
fn destination_with(name: &str, id: Option<i32>) -> Destination {
    let before = count_named(name);
    let client = coremidi::Client::new(&format!("ks-rv5d-dest-{name}-{id:?}")).unwrap();
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
    if let Some(id) = id {
        dest.set_property_integer("uniqueID", id).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while count_named(name) <= before
        || id.is_some_and(|id| {
            !keyscore_midi::destinations()
                .iter()
                .any(|d| d.name() == name && d.unique_id() == id)
        })
    {
        assert!(Instant::now() < deadline, "목적지 {name}가 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    }
    Destination {
        rx,
        dest,
        _client: client,
    }
}

fn wait_count(name: &str, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while count_named(name) != n {
        assert!(Instant::now() < deadline, "목적지 {name} 수가 {n}이 아니다");
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

fn notes_json() -> String {
    let v = Velocity::from_midi1(100).unwrap().get();
    (0..128)
        .map(|i| format!("[{}, 240, 60, {v}]", if i == 0 { 0 } else { 960 }))
        .collect::<Vec<_>>()
        .join(", ")
}

/// 이름 포트 트랙(채널 2, 프로그램 5, 박마다 노트) + 가상 포트 트랙.
fn project_on(name: &str) -> Project {
    ksq::from_str(&format!(
        r#"{{"tracks": [{{"name": "Ext", "port": {{"named": "{name}"}}, "channel": 2, "program": 5,
            "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}},
            {{"name": "Main", "channel": 1}}]}}"#,
        notes_json()
    ))
    .unwrap()
    .0
}

/// 가상 포트 트랙 하나(박마다 노트).
fn project_virtual() -> Project {
    ksq::from_str(&format!(
        r#"{{"tracks": [{{"name": "V", "channel": 1,
            "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}}]}}"#,
        notes_json()
    ))
    .unwrap()
    .0
}

fn wait_seen(cx: &mut TestAppContext, name: &str, present: bool) {
    pump(
        cx,
        None,
        |cx| cx.read(|cx| ports::port_present(cx, name)) == Some(present),
        "감시가 목적지 변화를 보지 못했다",
    );
}

/// `rx`에서 `status`로 시작하는 메시지를 기다린다.
fn hears(rx: &mpsc::Receiver<Vec<u8>>, status: u8, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Ok(m) = rx.recv_timeout(Duration::from_millis(20))
            && m[0] == status
        {
            return true;
        }
    }
    false
}

fn drain(rx: &mpsc::Receiver<Vec<u8>>) {
    while rx.try_recv().is_ok() {}
}

// ---------- 다른 창 ----------

/// 재생하지 않는 창만 그 포트를 쓴다. 메시지는 그 창에만 가고, 재생 중인 창은 그대로 재생한다.
#[gpui_kit::test]
fn vanish_and_return_messages_only_user_window_and_other_keeps_playing(cx: &mut TestAppContext) {
    start(cx, "two", true);
    let name = unique("synth two");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (ha, a) = open(cx, project_virtual());
    let (hb, b) = open(cx, project_on(&name));
    run(cx, ha, transport::Play);
    assert_eq!(state(cx, &a), TransportState::Playing);
    let a_before = messages(cx, &a).len();
    drop(dest);
    wait_count(&name, 0);
    pump(
        cx,
        Some(hb),
        |cx| messages(cx, &b).iter().any(|m| m.contains("사라졌다")),
        "사용하는 창에 사라짐 메시지가 없다",
    );
    let _back = destination(&name);
    pump(
        cx,
        Some(hb),
        |cx| messages(cx, &b).iter().any(|m| m.contains("다시 연결했다")),
        "사용하는 창에 다시 연결 메시지가 없다",
    );
    assert_eq!(messages(cx, &a).len(), a_before, "{:?}", messages(cx, &a));
    assert_eq!(state(cx, &a), TransportState::Playing);
    assert_eq!(state(cx, &b), TransportState::Stopped);
    run(cx, ha, transport::Stop);
}

// ---------- 빠른 소멸·복귀 ----------

/// 감시 한 바퀴 안에 사라졌다 돌아온다. 재생은 새 목적지로 이어진다.
#[gpui_kit::test]
fn rapid_vanish_and_return_within_one_tick_reconnects_playback(cx: &mut TestAppContext) {
    start(cx, "rapid", true);
    let name = unique("synth rapid");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    drop(dest);
    wait_count(&name, 0);
    let back = destination(&name);
    pump(
        cx,
        Some(h),
        |cx| messages(cx, &v).iter().any(|m| m.contains("다시 연결했다")),
        "다시 연결 메시지가 없다",
    );
    let heard = hears(&back.rx, 0x91, Duration::from_secs(3));
    run(cx, h, transport::Stop);
    assert!(heard, "돌아온 목적지로 노트가 닿지 않았다");
    assert_eq!(label(cx, h, "status-port-missing"), None);
}

// ---------- 없는 이름으로 바꾸기 ----------

/// 재생 중 트랙 포트를 없는 이름으로 바꾸면 「포트 없음」, 그 이름이 나타나면 그쪽으로 소리가 간다.
#[gpui_kit::test]
fn port_changed_to_missing_name_while_playing_then_appears(cx: &mut TestAppContext) {
    start(cx, "setport", true);
    let first = unique("synth first");
    let later = unique("synth later");
    let d1 = destination(&first);
    wait_seen(cx, &first, true);
    let (h, v) = open(cx, project_on(&first));
    run(cx, h, transport::Play);
    assert!(hears(&d1.rx, 0x91, Duration::from_secs(3)));
    run(cx, h, track::SetPort(later.clone()));
    pump(
        cx,
        Some(h),
        |cx| label(cx, h, "status-port-missing").is_some(),
        "없는 이름인데 「포트 없음」이 없다",
    );
    assert!(
        messages(cx, &v)
            .iter()
            .any(|m| m.contains(&later) && m.contains("없어")),
        "{:?}",
        messages(cx, &v)
    );
    let d2 = destination(&later);
    pump(
        cx,
        Some(h),
        |cx| label(cx, h, "status-port-missing").is_none(),
        "나타났는데 「포트 없음」이 남았다",
    );
    let heard = hears(&d2.rx, 0x91, Duration::from_secs(3));
    run(cx, h, transport::Stop);
    assert!(heard, "새로 나타난 목적지로 노트가 닿지 않았다");
}

// ---------- undo ----------

/// undo로 사라진 포트 이름이 돌아오면 「포트 없음」이 보이고, 그 포트가 돌아오면 사라진다.
#[gpui_kit::test]
fn undo_restoring_missing_port_shows_and_clears_status(cx: &mut TestAppContext) {
    start(cx, "undo", true);
    let name = unique("synth undo");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, track::SetPort("virtual".into()));
    drop(dest);
    wait_count(&name, 0);
    wait_seen(cx, &name, false);
    assert_eq!(label(cx, h, "status-port-missing"), None);
    // 그 포트를 쓰는 창이 없었으므로 사라짐 메시지는 없다
    assert!(!messages(cx, &v).iter().any(|m| m.contains("사라졌다")));
    run(cx, h, edit::Undo);
    pump(
        cx,
        Some(h),
        |cx| label(cx, h, "status-port-missing").is_some(),
        "undo 뒤 「포트 없음」이 없다",
    );
    let _back = destination(&name);
    pump(
        cx,
        Some(h),
        |cx| label(cx, h, "status-port-missing").is_none(),
        "돌아왔는데 「포트 없음」이 남았다",
    );
}

// ---------- 닫힌 창 ----------

/// 창을 닫은 뒤 그 창이 쓰던 포트가 사라졌다 돌아와도 감시는 계속 돌고 다른 창에 영향이 없다.
#[gpui_kit::test]
fn closed_window_port_changes_are_harmless(cx: &mut TestAppContext) {
    start(cx, "closed", true);
    let name = unique("synth closed");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, _v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    cx.update_window(h, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    drop(dest);
    wait_count(&name, 0);
    idle(cx, None, 5);
    let _back = destination(&name);
    idle(cx, None, 5);
    assert!(cx.read(ports::is_monitoring));
    let (h2, v2) = open(cx, project_virtual());
    idle(cx, Some(h2), 2);
    assert!(messages(cx, &v2).is_empty(), "{:?}", messages(cx, &v2));
}

// ---------- 가상 포트 복구 ----------

/// 엔진이 없으면 복구는 아무 일도 하지 않는다. 감시 없이 부르면 감시는 시작되지 않는다.
#[gpui_kit::test]
fn recover_without_engine_or_monitor(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("ks-rv5d-{}-noengine", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    assert!(cx.update(engine::recover_virtual_port).is_ok());
    let port = unique("out nomon");
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: "ks-rv5d-nomon".into(),
                port_name: port.clone(),
            },
        )
    })
    .unwrap();
    let (h, v) = open(cx, project_virtual());
    assert!(cx.update(engine::recover_virtual_port).is_ok());
    idle(cx, Some(h), 3);
    assert!(!cx.read(ports::is_monitoring));
    assert!(messages(cx, &v).iter().any(|m| m.contains("다시 등록했다")));
}

/// 두 번 연달아 복구한다. 정지 중 이름 포트 테스트 노트가 넣은 스냅샷이 있어도 복구는 빨리 끝나고, 뒤의
/// 테스트 노트(이름 포트·가상 포트)가 닿으며 일시정지 자리는 지켜진다.
#[gpui_kit::test]
fn recover_twice_then_test_notes_still_reach(cx: &mut TestAppContext) {
    let port = start(cx, "recover2", true);
    let name = unique("synth rec");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    let (hp, paused) = open(cx, project_virtual());
    run(cx, hp, transport::Play);
    std::thread::sleep(Duration::from_millis(100));
    run(cx, hp, transport::Pause);
    let paused_state = state(cx, &paused);
    assert!(matches!(paused_state, TransportState::Paused(_)));
    // 이름 포트로 테스트 노트(정지 중: 이 창의 스냅샷이 스케줄러에 들어간다)
    run(cx, h, midi::SendTestNote);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(2)));
    let t = Instant::now();
    cx.update(engine::recover_virtual_port).unwrap();
    cx.update(engine::recover_virtual_port).unwrap();
    assert!(
        t.elapsed() < Duration::from_millis(500),
        "복구가 메인 스레드를 오래 막았다: {:?}",
        t.elapsed()
    );
    cx.run_until_parked();
    assert_eq!(state(cx, &paused), paused_state);
    pump(
        cx,
        Some(h),
        |cx| cx.read(ports::is_monitoring),
        "감시가 다시 시작되지 않았다",
    );
    drain(&dest.rx);
    run(cx, h, midi::SendTestNote);
    assert!(
        hears(&dest.rx, 0x91, Duration::from_secs(2)),
        "복구 뒤 이름 포트 테스트 노트가 닿지 않았다"
    );
    // 가상 포트(같은 이름 소스 하나)
    let sources: Vec<_> = coremidi::Sources
        .into_iter()
        .filter(|s| s.name().as_deref() == Some(port.as_str()))
        .collect();
    assert_eq!(sources.len(), 1);
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-rv5d-recover-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                for m in split(p.data()) {
                    let _ = tx.send(m);
                }
            }
        })
        .unwrap();
    input.connect_source(&sources[0]).unwrap();
    run(cx, h, track::Next);
    run(cx, h, midi::SendTestNote);
    assert!(
        hears(&rx, 0x90, Duration::from_secs(2)),
        "복구 뒤 가상 포트 테스트 노트가 닿지 않았다: {:?}",
        messages(cx, &v)
    );
}

// ---------- 소멸 중 편집 ----------

/// 재생 중 포트가 사라진 동안 편집하고(그 트랙은 소리 내지 않는다), 돌아오면 새 목적지로 노트가 닿는다.
#[gpui_kit::test]
fn edit_during_missing_then_return_reaches_new_destination(cx: &mut TestAppContext) {
    start(cx, "editmiss", true);
    let name = unique("synth editmiss");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    drop(dest);
    wait_count(&name, 0);
    pump(
        cx,
        Some(h),
        |cx| messages(cx, &v).iter().any(|m| m.contains("사라졌다")),
        "사라짐 메시지가 없다",
    );
    // 가상 포트 트랙을 뮤트(재생 중 편집 → 스냅샷 교체)
    run(cx, h, track::Next);
    run(cx, h, track::ToggleMute);
    assert!(
        messages(cx, &v)
            .iter()
            .any(|m| m.contains(&name) && m.contains("없어")),
        "{:?}",
        messages(cx, &v)
    );
    let back = destination(&name);
    pump(
        cx,
        Some(h),
        |cx| messages(cx, &v).iter().any(|m| m.contains("다시 연결했다")),
        "다시 연결 메시지가 없다",
    );
    let heard = hears(&back.rx, 0x91, Duration::from_secs(3));
    run(cx, h, transport::Stop);
    assert!(heard);
    assert_eq!(state(cx, &v), TransportState::Stopped);
}

// ---------- 같은 이름 포트 여럿 (엔진과 Reconnector가 같은 쪽을 고를 때) ----------

/// 같은 이름 포트가 둘이고 먼저 열거되는 쪽의 고유 ID가 더 작다(엔진·`Reconnector`가 같은 것을 고른다).
/// 그것이 사라지면 남은 것으로 재생이 이어진다.
#[gpui_kit::test]
fn duplicate_names_consistent_choice_reconnects(cx: &mut TestAppContext) {
    start(cx, "dupok", false);
    let name = unique("dup ok");
    let base = 0x2100_0000 + i32::try_from(std::process::id() % 0xFFFF).unwrap() * 16;
    let low = destination_with(&name, Some(base + 1));
    let high = destination_with(&name, Some(base + 2));
    let order: Vec<i32> = keyscore_midi::destinations()
        .iter()
        .filter(|d| d.name() == name)
        .map(keyscore_midi::PortInfo::unique_id)
        .collect();
    assert_eq!(order, vec![base + 1, base + 2], "열거 순서 전제");
    start_monitor(cx);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&low.rx, 0x91, Duration::from_secs(3)));
    drop(low);
    wait_count(&name, 1);
    pump(
        cx,
        Some(h),
        |cx| messages(cx, &v).iter().any(|m| m.contains("다시 연결했다")),
        "다시 연결 메시지가 없다",
    );
    let heard = hears(&high.rx, 0x91, Duration::from_secs(3));
    run(cx, h, transport::Stop);
    assert!(heard);
    let _ = &high.dest;
}

// ---------- 이름 바뀜 ----------

/// 쓰는 포트의 이름이 바뀌면(장치·앱이 이름을 바꿈) 그 이름의 포트가 사라진 것이다.
#[gpui_kit::test]
fn renamed_in_use_port_is_reported_lost(cx: &mut TestAppContext) {
    start(cx, "rename", true);
    let name = unique("synth rename");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    let renamed = format!("{name} 2");
    dest.dest.set_property_string("name", &renamed).unwrap();
    wait_count(&name, 0);
    pump(
        cx,
        Some(h),
        |cx| label(cx, h, "status-port-missing").is_some(),
        "이름이 바뀌어 그 이름의 포트가 없는데 「포트 없음」이 보이지 않는다",
    );
    assert!(messages(cx, &v).iter().any(|m| m.contains("사라졌다")));
}
