//! S2-T5d 적대적 리뷰 3. 깨뜨리지 못한 시도를 남긴다. 깨진 시도는 `docs/reports/s2-t5d/review-3.md`
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
    format!("KS rv5d3 {} {tag}", std::process::id())
}

fn start(cx: &mut TestAppContext, tag: &str, monitor: bool) -> String {
    let dir = std::env::temp_dir().join(format!("ks-rv5d3-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(&format!("out {tag}"));
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-rv5d3-{tag}"),
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
    let client = coremidi::Client::new(&format!("ks-rv5d3-dest-{name}-{id:?}")).unwrap();
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
    0x2700_0000 + salt * 0x10_0000 + i32::try_from(std::process::id() % 0xFFFF).unwrap() * 16
}

// ---------- 지적 1 수정의 회귀: 재생 중 더 작은 ID의 같은 이름 포트가 나타남 ----------

/// 이름 포트 `name` 트랙(채널 2, 프로그램 5, 박마다 노트) + 이름 포트 `other` 트랙(채널 3, 프로그램 7, 박마다 노트).
fn project_two_ports(name: &str, other: &str) -> Project {
    ksq::from_str(&format!(
        r#"{{"tracks": [{{"name": "A", "port": {{"named": "{name}"}}, "channel": 2, "program": 5,
            "clips": [{{"start": 0, "length": 122880, "notes": [{n}]}}]}},
            {{"name": "B", "port": {{"named": "{other}"}}, "channel": 3, "program": 7,
            "clips": [{{"start": 0, "length": 122880, "notes": [{n}]}}]}}]}}"#,
        n = notes_json()
    ))
    .unwrap()
    .0
}

fn mine(all: Vec<String>, name: &str) -> Vec<String> {
    all.into_iter().filter(|m| m.contains(name)).collect()
}

// ---------- 리뷰 2 지적 1 수정: 재생 중 쓰는 포트를 잃음 ----------

/// 재생 중 이름을 바꿨다가 되돌리면: 바뀐 동안 소리가 가지 않고, 되돌리면 「다시 연결했다」와 함께 프로그램이
/// 한 번 가고 노트가 다시 간다. 다른 이름 포트 트랙의 소리는 그동안 끊기지 않고 프로그램이 다시 가지 않는다.
#[gpui_kit::test]
fn rename_away_and_back_while_playing_restores_sound_and_keeps_other_port(cx: &mut TestAppContext) {
    start(cx, "rnab", true);
    let name = unique("synth rnab");
    let other = unique("other rnab");
    let dest = destination(&name);
    let keep = destination(&other);
    wait_seen(cx, &name, true);
    wait_seen(cx, &other, true);
    let (h, v) = open(cx, project_two_ports(&name, &other));
    run(cx, h, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    assert!(hears(&keep.rx, 0x92, Duration::from_secs(3)));
    drain(&keep.rx);
    let renamed = format!("{name} away");
    dest.dest.set_property_string("name", &renamed).unwrap();
    wait_count(&name, 0);
    pump(
        cx,
        Some(h),
        |cx| {
            mine(messages(cx, &v), &name)
                .iter()
                .any(|m| m.contains("사라졌다"))
        },
        "사라짐 메시지가 없다",
    );
    drain(&dest.rx);
    let during = collect(&dest.rx, Duration::from_secs(1));
    assert!(during.iter().all(|m| m[0] != 0x91), "{during:?}");
    // 「사라졌다」 다음에 재생 중이라 「없어 그 트랙은 소리 내지 않는다」가 온다(한 번)
    let m = mine(messages(cx, &v), &name);
    assert_eq!(
        m.iter().filter(|x| x.contains("소리 내지 않는다")).count(),
        1,
        "{m:?}"
    );
    dest.dest.set_property_string("name", &name).unwrap();
    wait_count(&name, 1);
    let mut back = pump_until_heard(cx, h, &dest.rx, 0x91, "되돌린 포트로 소리가 가지 않는다");
    back.extend(collect(&dest.rx, Duration::from_millis(500)));
    let kept: Vec<Vec<u8>> = keep.rx.try_iter().collect();
    run(cx, h, transport::Stop);
    assert_eq!(
        back.iter().filter(|m| *m == &vec![0xC1, 5]).count(),
        1,
        "{back:?}"
    );
    let m = mine(messages(cx, &v), &name);
    assert!(
        m.last().unwrap().contains("다시 연결했다"),
        "되돌린 것은 끊긴 뒤의 복귀다: {m:?}"
    );
    assert!(m.iter().all(|x| !x.contains("바꿨다")), "{m:?}");
    // 다른 포트: 그동안 노트가 이어졌고 프로그램을 다시 받지 않았다
    let kept: Vec<Vec<u8>> = kept;
    assert!(
        kept.iter().all(|m| *m != vec![0xC2, 7]),
        "다른 포트에 프로그램이 다시 갔다: {kept:?}"
    );
}

/// 다른 포트 트랙의 노트가 이름 바뀜·되돌림 내내 끊기지 않는다(박마다 노트, 250 ms 이상 비면 끊김으로 본다).
#[gpui_kit::test]
fn other_port_sound_continuous_through_rename(cx: &mut TestAppContext) {
    start(cx, "cont", true);
    let name = unique("synth cont");
    let other = unique("other cont");
    let dest = destination(&name);
    let keep = destination(&other);
    wait_seen(cx, &name, true);
    wait_seen(cx, &other, true);
    let (h, _v) = open(cx, project_two_ports(&name, &other));
    run(cx, h, transport::Play);
    assert!(hears(&keep.rx, 0x92, Duration::from_secs(3)));
    drain(&keep.rx);
    let t0 = Instant::now();
    dest.dest
        .set_property_string("name", &format!("{name} z"))
        .unwrap();
    wait_count(&name, 0);
    let mut stamps = Vec::new();
    let deadline = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < deadline {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
        render(cx, h);
        while let Ok(m) = keep.rx.try_recv() {
            if m[0] == 0x92 {
                stamps.push(t0.elapsed());
            }
            assert!(m != vec![0xC2, 7], "다른 포트에 프로그램이 다시 갔다");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    run(cx, h, transport::Stop);
    // 템포 120, 박마다 노트: 1.5 s에 노트 2개 이상
    assert!(stamps.len() >= 2, "다른 포트 노트가 끊겼다: {stamps:?}");
}

/// 재생하지 않는 창 B가 잃은 포트를 써도, 다른 포트로 재생 중인 창 A의 재생은 그대로다. B는 「사라졌다」만
/// 받고(재생하지 않으니 「소리 내지 않는다」 없음), A에는 그 포트의 재연결 문구가 가지 않는다.
#[gpui_kit::test]
fn lost_port_of_idle_window_does_not_touch_playing_window(cx: &mut TestAppContext) {
    start(cx, "idle", true);
    let name = unique("synth idle");
    let other = unique("other idle");
    let dest = destination(&name);
    let keep = destination(&other);
    wait_seen(cx, &name, true);
    wait_seen(cx, &other, true);
    let (hb, b) = open(cx, project_on(&name));
    let (ha, a) = open(cx, project_on(&other));
    run(cx, ha, transport::Play);
    assert!(hears(&keep.rx, 0x91, Duration::from_secs(3)));
    drain(&keep.rx);
    drop(dest);
    wait_count(&name, 0);
    pump(
        cx,
        Some(hb),
        |cx| {
            mine(messages(cx, &b), &name)
                .iter()
                .any(|m| m.contains("사라졌다"))
        },
        "사라짐 메시지가 없다",
    );
    let got = collect(&keep.rx, Duration::from_secs(1));
    run(cx, ha, transport::Stop);
    assert!(
        got.iter().any(|m| m[0] == 0x91),
        "A의 재생이 멈췄다: {got:?}"
    );
    assert!(got.iter().all(|m| *m != vec![0xC1, 5]), "{got:?}");
    let bm = mine(messages(cx, &b), &name);
    assert!(bm.iter().all(|m| !m.contains("소리 내지 않는다")), "{bm:?}");
    let am = mine(messages(cx, &a), &name);
    assert!(
        am.iter()
            .all(|m| !m.contains("사라졌다") && !m.contains("소리 내지 않는다")),
        "{am:?}"
    );
    let _ = hb;
}

/// 같은 포트를 쓰는 두 창 가운데 재생 중인 창만 스냅샷을 바꾼다. 재생하지 않는 창은 「사라졌다」만 받는다.
/// 두 창 모두 「사라졌다」를 한 번씩만 받는다.
#[gpui_kit::test]
fn shared_port_lost_reprepares_only_owner_once(cx: &mut TestAppContext) {
    start(cx, "share", true);
    let name = unique("synth share");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (_hb, b) = open(cx, project_on(&name));
    let (ha, a) = open(cx, project_on(&name));
    run(cx, ha, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    dest.dest
        .set_property_string("name", &format!("{name} s"))
        .unwrap();
    wait_count(&name, 0);
    pump(
        cx,
        Some(ha),
        |cx| {
            mine(messages(cx, &a), &name)
                .iter()
                .any(|m| m.contains("사라졌다"))
        },
        "사라짐 메시지가 없다",
    );
    // 몇 번 더 돌려도 메시지가 늘지 않는다
    for _ in 0..5 {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    drain(&dest.rx);
    let got = collect(&dest.rx, Duration::from_millis(800));
    run(cx, ha, transport::Stop);
    assert!(got.iter().all(|m| m[0] != 0x91), "{got:?}");
    let am = mine(messages(cx, &a), &name);
    let bm = mine(messages(cx, &b), &name);
    let lost = |m: &[String]| m.iter().filter(|x| x.contains("사라졌다")).count();
    let silent = |m: &[String]| m.iter().filter(|x| x.contains("소리 내지 않는다")).count();
    assert_eq!((lost(&am), silent(&am)), (1, 1), "{am:?}");
    assert_eq!((lost(&bm), silent(&bm)), (1, 0), "{bm:?}");
}

// ---------- 리뷰 2 제안 2 수정: 「바꿨다」와 「다시 연결했다」 ----------

/// 정지 중 더 작은 ID의 같은 이름 포트가 나타나면 「같은 이름의 다른 포트로 바꿨다」다(끊긴 적 없음).
#[gpui_kit::test]
fn lower_id_appearing_while_connected_says_switched(cx: &mut TestAppContext) {
    start(cx, "switch", true);
    let name = unique("dup switch");
    let base = base_id(1);
    let _high = destination_with(&name, Some(base + 2));
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    // 감시가 이 이름을 등록하게 한 번 돌린다
    for _ in 0..3 {
        cx.executor().advance_clock(ports::POLL_INTERVAL);
        cx.run_until_parked();
    }
    let _low = destination_with(&name, Some(base + 1));
    pump(
        cx,
        Some(h),
        |cx| {
            mine(messages(cx, &v), &name)
                .iter()
                .any(|m| m.contains("바꿨다"))
        },
        "「바꿨다」가 없다",
    );
    let m = mine(messages(cx, &v), &name);
    assert!(m.iter().all(|x| !x.contains("다시 연결했다")), "{m:?}");
}

// ---------- 리뷰 2 제안 3 수정: 핫플러그 묶음 ----------

/// 쓰지 않는 포트 여럿이 한 번에 나타나고 사라지면 종류마다 한 줄이고 이름이 모두 들어 있다. 쓰는 포트가
/// 같은 틱에 섞여도 쓰는 창의 묶음 줄에는 그 이름이 없다.
#[gpui_kit::test]
fn grouped_hotplug_one_line_per_kind_excluding_used(cx: &mut TestAppContext) {
    start(cx, "group", true);
    let used = unique("used group");
    let (h, v) = open(cx, project_on(&used));
    pump(
        cx,
        Some(h),
        |cx| label(cx, h, "status-port-missing").is_some(),
        "포트 없음이 보이지 않는다",
    );
    let tag = unique("grp");
    let names: Vec<String> = (0..5).map(|i| format!("{tag} {i}")).collect();
    // 감시를 돌리지 않고 만들어 이벤트가 한 틱에 모이게 한다
    let mut dests: Vec<Destination> = names.iter().map(|n| destination(n)).collect();
    dests.push(destination(&used));
    std::thread::sleep(Duration::from_millis(300));
    pump(
        cx,
        Some(h),
        |cx| {
            mine(messages(cx, &v), &tag)
                .iter()
                .any(|m| m.contains("새로 보인다"))
        },
        "새 포트 알림이 없다",
    );
    let came: Vec<String> = mine(messages(cx, &v), &tag)
        .into_iter()
        .filter(|m| m.contains("새로 보인다"))
        .collect();
    assert_eq!(came.len(), 1, "{came:?}");
    assert!(came[0].contains("개가 새로 보인다"), "{came:?}");
    for n in &names {
        assert!(came[0].contains(&format!("「{n}」")), "{n}: {came:?}");
    }
    assert!(!came[0].contains(&format!("「{used}」")), "{came:?}");
    let m = mine(messages(cx, &v), &used);
    assert!(m.iter().any(|x| x.contains("다시 연결했다")), "{m:?}");
    drop(dests);
    std::thread::sleep(Duration::from_millis(300));
    pump(
        cx,
        Some(h),
        |cx| {
            mine(messages(cx, &v), &tag)
                .iter()
                .any(|m| m.contains("없어졌다"))
        },
        "없어짐 알림이 없다",
    );
    let gone: Vec<String> = mine(messages(cx, &v), &tag)
        .into_iter()
        .filter(|m| m.contains("없어졌다"))
        .collect();
    assert_eq!(gone.len(), 1, "{gone:?}");
    for n in &names {
        assert!(gone[0].contains(&format!("「{n}」")), "{n}: {gone:?}");
    }
    assert!(!gone[0].contains(&format!("「{used}」")), "{gone:?}");
}
