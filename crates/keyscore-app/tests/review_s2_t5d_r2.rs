//! S2-T5d 적대적 리뷰 2. 깨뜨리지 못한 시도를 남긴다. 깨진 시도는 `docs/reports/s2-t5d/review-2.md`
//! 부록에 있다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, ports};
use keyscore_core::{Project, Velocity, ksq};

fn unique(tag: &str) -> String {
    format!("KS rv5d2 {} {tag}", std::process::id())
}

fn start(cx: &mut TestAppContext, tag: &str, monitor: bool) -> String {
    let dir = std::env::temp_dir().join(format!("ks-rv5d2-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(&format!("out {tag}"));
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-rv5d2-{tag}"),
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
    let client = coremidi::Client::new(&format!("ks-rv5d2-dest-{name}-{id:?}")).unwrap();
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

/// `rx`에서 `within` 동안 받은 메시지를 모두 모은다.
fn collect(rx: &mpsc::Receiver<Vec<u8>>, within: Duration) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + within;
    let mut got = Vec::new();
    while Instant::now() < deadline {
        if let Ok(m) = rx.recv_timeout(Duration::from_millis(20)) {
            got.push(m);
        }
    }
    got
}

/// 감시를 돌리며 `rx`에 `status`가 올 때까지 기다린다. 그동안 받은 메시지를 모두 돌려준다. 다른 테스트의
/// 핫플러그 알림이 창 메시지 목록(상한 32)을 밀어낼 수 있어 메시지 대신 소리로 판정한다.
fn pump_until_heard(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    rx: &mpsc::Receiver<Vec<u8>>,
    status: u8,
    why: &str,
) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut got = Vec::new();
    loop {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
        render(cx, handle);
        while let Ok(m) = rx.try_recv() {
            got.push(m);
        }
        if got.iter().any(|m| m[0] == status) {
            return got;
        }
        assert!(Instant::now() < deadline, "{why}: {got:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn base_id(salt: i32) -> i32 {
    0x2300_0000 + salt * 0x10_0000 + i32::try_from(std::process::id() % 0xFFFF).unwrap() * 16
}

// ---------- 지적 1 수정의 회귀: 재생 중 더 작은 ID의 같은 이름 포트가 나타남 ----------

/// 재생 중 같은 이름의 더 작은 고유 ID 포트가 나타나면 엔진과 감시가 함께 그쪽으로 옮긴다. 옛 포트에는
/// 노트가 끊기지 않고 남지 않으며(Note Off), 새 포트에는 프로그램이 한 번 간다.
#[gpui_kit::test]
fn lower_id_duplicate_appearing_mid_playback_moves_playback_once(cx: &mut TestAppContext) {
    start(cx, "lowmid", true);
    let name = unique("dup lowmid");
    let base = base_id(1);
    let high = destination_with(&name, Some(base + 2));
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&high.rx, 0x91, Duration::from_secs(3)));
    let low = destination_with(&name, Some(base + 1));
    let mut got_low = pump_until_heard(
        cx,
        h,
        &low.rx,
        0x91,
        "더 작은 ID 포트가 나타났는데 옮기지 않았다",
    );
    got_low.extend(collect(&low.rx, Duration::from_secs(1)));
    drain(&high.rx);
    let got_high = collect(&high.rx, Duration::from_millis(1200));
    run(cx, h, transport::Stop);
    assert_eq!(
        got_low.iter().filter(|m| *m == &vec![0xC1, 5]).count(),
        1,
        "{got_low:?}"
    );
    assert!(got_low.iter().any(|m| m[0] == 0x91), "{got_low:?}");
    assert!(
        got_high.iter().all(|m| m[0] != 0x91),
        "옛 포트로 노트가 계속 간다: {got_high:?}"
    );
    // 쓰는 창에는 핫플러그 문구가 가지 않는다
    assert!(
        messages(cx, &v)
            .iter()
            .all(|m| !(m.contains(&name) && m.contains("새로 보인다"))),
        "{:?}",
        messages(cx, &v)
    );
}

// ---------- 지적 2 수정의 회귀 ----------

/// 감시 없이, 정지 중 이름이 바뀌었다가 원래 이름으로 돌아오면 다시 그 포트로 소리가 간다(놓은 핸들을 다시 연다).
#[gpui_kit::test]
fn rename_and_back_without_monitor_reopens(cx: &mut TestAppContext) {
    start(cx, "rnback", false);
    let name = unique("synth rnback");
    let dest = destination(&name);
    let (h, _v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    run(cx, h, transport::Stop);
    dest.dest
        .set_property_string("name", &format!("{name} x"))
        .unwrap();
    wait_count(&name, 0);
    run(cx, h, transport::Play);
    run(cx, h, transport::Stop);
    dest.dest.set_property_string("name", &name).unwrap();
    wait_count(&name, 1);
    drain(&dest.rx);
    run(cx, h, transport::Play);
    let got = collect(&dest.rx, Duration::from_millis(1500));
    run(cx, h, transport::Stop);
    assert!(got.contains(&vec![0xC1, 5]), "{got:?}");
    assert!(got.iter().any(|m| m[0] == 0x91), "{got:?}");
}

// ---------- 제안 5 수정의 회귀: 돌아온 포트의 뱅크·프로그램 ----------

fn project_two_channels(name: &str) -> Project {
    ksq::from_str(&format!(
        r#"{{"tracks": [
            {{"name": "A", "port": {{"named": "{name}"}}, "channel": 2, "program": 5, "bank": 3,
              "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}},
            {{"name": "B", "port": {{"named": "{name}"}}, "channel": 3, "program": 7}},
            {{"name": "C", "port": {{"named": "{name}"}}, "channel": 3, "program": 9, "mute": true}},
            {{"name": "Main", "channel": 1, "program": 11}}]}}"#,
        notes_json()
    ))
    .unwrap()
    .0
}

/// 돌아온 포트에 그 목적지의 채널마다 실효 뱅크·프로그램을 정확히 한 번 보낸다. 뮤트 트랙의 음색은 들리는
/// 트랙에 진다. 다른 목적지(가상 포트)의 프로그램은 다시 보내지 않는다.
#[gpui_kit::test]
fn returned_port_gets_each_channel_setup_once(cx: &mut TestAppContext) {
    start(cx, "setup", true);
    let name = unique("synth setup");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_two_channels(&name));
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
    let back = destination(&name);
    pump(
        cx,
        Some(h),
        |cx| messages(cx, &v).iter().any(|m| m.contains("다시 연결했다")),
        "다시 연결 메시지가 없다",
    );
    let got = collect(&back.rx, Duration::from_millis(1200));
    run(cx, h, transport::Stop);
    let count = |m: &[u8]| got.iter().filter(|g| g.as_slice() == m).count();
    assert_eq!(count(&[0xC1, 5]), 1, "{got:?}");
    assert_eq!(count(&[0xC2, 7]), 1, "{got:?}");
    assert_eq!(count(&[0xC2, 9]), 0, "{got:?}");
    assert!(
        got.iter()
            .any(|m| m.len() == 3 && m[0] == 0xB1 && m[1] == 0),
        "뱅크 MSB가 없다: {got:?}"
    );
    let first_note = got.iter().position(|m| m[0] == 0x91);
    let program = got.iter().position(|m| *m == vec![0xC1, 5]);
    assert!(program < first_note || first_note.is_none(), "{got:?}");
}

/// 같은 창이 같은 이름을 두 번 잃고 되찾아도 매번 프로그램이 간다(잊은 음색이 다음 복귀를 막지 않는다).
#[gpui_kit::test]
fn returned_twice_sends_program_each_time(cx: &mut TestAppContext) {
    start(cx, "twice", true);
    let name = unique("synth twice");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, _v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    let mut current = dest;
    for round in 1..=2 {
        drop(current);
        wait_count(&name, 0);
        wait_seen(cx, &name, false);
        current = destination(&name);
        let mut got =
            pump_until_heard(cx, h, &current.rx, 0x91, "돌아온 포트로 소리가 가지 않는다");
        got.extend(collect(&current.rx, Duration::from_millis(600)));
        assert_eq!(
            got.iter().filter(|m| *m == &vec![0xC1, 5]).count(),
            1,
            "{round}: {got:?}"
        );
    }
    run(cx, h, transport::Stop);
}

// ---------- 제안 6 대응: 쓰지 않는 포트의 핫플러그 ----------

/// 포트를 많이 내는 장치(여러 목적지가 한꺼번에)가 꽂혔다 빠지면 쓰지 않는 창마다 메시지가 남는다. 메시지
/// 목록은 상한(32)을 넘지 않고, 상태줄은 마지막 메시지를 보인다. 쓰는 창의 「포트 없음」은 유지된다.
#[gpui_kit::test]
fn many_port_hotplug_is_bounded_and_status_shows_last(cx: &mut TestAppContext) {
    start(cx, "many", true);
    let used = unique("synth gone");
    let (h, v) = open(cx, project_on(&used));
    pump(
        cx,
        Some(h),
        |cx| label(cx, h, "status-port-missing").is_some(),
        "포트 없음이 보이지 않는다",
    );
    let names: Vec<String> = (0..40).map(|i| unique(&format!("multi {i:02}"))).collect();
    let dests: Vec<Destination> = names.iter().map(|n| destination(n)).collect();
    pump(
        cx,
        Some(h),
        |cx| {
            messages(cx, &v)
                .iter()
                .any(|m| m.contains(&names[39]) && m.contains("새로 보인다"))
        },
        "새 포트 알림이 없다",
    );
    assert!(messages(cx, &v).len() <= 32);
    drop(dests);
    pump(
        cx,
        Some(h),
        |cx| {
            messages(cx, &v)
                .iter()
                .any(|m| m.contains(&names[39]) && m.contains("없어졌다"))
        },
        "없어짐 알림이 없다",
    );
    let last = messages(cx, &v).last().cloned().unwrap();
    assert_eq!(label(cx, h, "status-message"), Some(last));
    assert!(label(cx, h, "status-port-missing").is_some());
}

/// 쓰는 창과 쓰지 않는 창: 쓰는 포트가 사라지면 쓰는 창은 「사라졌다」만, 쓰지 않는 창은 「없어졌다」만 받는다.
/// 이름이 바뀌면 쓰는 창은 새 이름의 「새로 보인다」를 받아도 마지막 메시지는 「사라졌다」다.
#[gpui_kit::test]
fn hotplug_wording_per_window_and_rename_order(cx: &mut TestAppContext) {
    start(cx, "words", true);
    let name = unique("synth words");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (hu, u) = open(cx, project_on(&name));
    let (ho, o) = open(cx, project_virtual());
    let renamed = format!("{name} r");
    dest.dest.set_property_string("name", &renamed).unwrap();
    wait_count(&name, 0);
    pump(
        cx,
        Some(hu),
        |cx| messages(cx, &u).iter().any(|m| m.contains("사라졌다")),
        "사라짐 메시지가 없다",
    );
    // 다른 테스트의 목적지도 같은 시스템에 보이므로 이 테스트의 이름만 본다. 다른 테스트의 많은 포트 알림이
    // 메시지 목록(상한 32)을 밀어낼 수 있어 곧바로 읽는다.
    let mine = |all: Vec<String>| -> Vec<String> {
        all.into_iter().filter(|m| m.contains(&name)).collect()
    };
    let um = mine(messages(cx, &u));
    let om = mine(messages(cx, &o));
    let o_full = messages(cx, &o).len() == 32;
    assert!(um.last().unwrap().contains("사라졌다"), "{um:?}");
    assert!(um.iter().all(|m| !m.contains("없어졌다")), "{um:?}");
    assert!(
        o_full
            || om
                .iter()
                .any(|m| m.contains(&name) && !m.contains(&renamed) && m.contains("없어졌다")),
        "{om:?}"
    );
    assert!(
        o_full
            || om
                .iter()
                .any(|m| m.contains(&renamed) && m.contains("새로 보인다")),
        "{om:?}"
    );
    let _ = ho;
    assert!(om.iter().all(|m| !m.contains("사라졌다")), "{om:?}");
}

/// 실험: 만든 직후 고유 ID를 바꾸는 목적지(앱들이 흔히 저장된 ID로 바꾼다)를 감시자가 새 ID로 보는가.
#[test]
fn probe_watcher_sees_unique_id_change() {
    let (_w, rx) = keyscore_midi::PortWatcher::start().unwrap();
    let name = unique("probe uid");
    let id = base_id(5) + 7;
    let d = destination_with(&name, Some(id));
    std::thread::sleep(Duration::from_millis(500));
    let events: Vec<_> = rx.try_iter().collect();
    drop(d);
    let ids: Vec<(String, i32)> = events
        .iter()
        .filter_map(|e| match e {
            keyscore_midi::PortEvent::Appeared(p) if p.name() == name => {
                Some(("appeared".to_owned(), p.unique_id()))
            }
            keyscore_midi::PortEvent::Disappeared(p) if p.name() == name => {
                Some(("disappeared".to_owned(), p.unique_id()))
            }
            _ => None,
        })
        .collect();
    assert!(
        ids.iter().any(|(k, i)| k == "appeared" && *i == id),
        "감시자가 바뀐 고유 ID {id}를 보지 못했다: {ids:?}"
    );
}

/// 실험: 만들고 잠시 뒤 고유 ID를 바꾸면 감시자가 새 ID를 보는가.
#[test]
fn probe_watcher_sees_late_unique_id_change() {
    let (_w, rx) = keyscore_midi::PortWatcher::start().unwrap();
    let name = unique("probe late uid");
    let d = destination(&name);
    std::thread::sleep(Duration::from_millis(300));
    let id = base_id(6) + 3;
    d.dest.set_property_integer("uniqueID", id).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    let events: Vec<_> = rx.try_iter().collect();
    let fresh: Vec<i32> = keyscore_midi::destinations()
        .iter()
        .filter(|p| p.name() == name)
        .map(keyscore_midi::PortInfo::unique_id)
        .collect();
    let seen: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            keyscore_midi::PortEvent::Appeared(p) if p.name() == name => {
                Some(format!("+{}", p.unique_id()))
            }
            keyscore_midi::PortEvent::Disappeared(p) if p.name() == name => {
                Some(format!("-{}", p.unique_id()))
            }
            _ => None,
        })
        .collect();
    eprintln!("PROBE fresh {fresh:?} seen {seen:?}");
    drop(d);
    assert_eq!(fresh, vec![id]);
    assert!(
        seen.contains(&format!("+{id}")),
        "감시자가 바뀐 고유 ID를 보지 못했다: {seen:?}"
    );
}
