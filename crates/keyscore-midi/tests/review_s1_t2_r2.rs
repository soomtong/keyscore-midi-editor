//! S1-T2 적대적 리뷰 2회차(`docs/reports/s1-t2/review-2.md`)의 테스트.
//!
//! 리뷰 1 수정을 깨뜨리려 한 시도다. 이번 회차에서는 수정 이상의 결함을 찾지 못해 모두 경계를 고정하는
//! 통과 테스트다. 제안 수준의 관찰(재등록 중 같은 이름 소스 둘, 정규화 뒤 겹치는 `Virtual` 제안 등)은
//! 구현자가 거절할 수 있어 지금 동작을 고정하는 테스트를 두지 않았다.
//!
//! `CoreMIDI` 상태는 시스템 전역이라 포트 이름에 프로세스 id를 넣는다. 프로세스 전체 상태(첫 클라이언트,
//! 탐침 소스 개수)를 보는 테스트는 같은 바이너리를 자식 프로세스로 다시 띄워 그 테스트 하나만 돌린다.
//! 나머지 테스트는 `coremidi` crate 클라이언트보다 먼저 `MidiClient::new`를 부른다.

use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use core_foundation::runloop::CFRunLoop;
use keyscore_midi::{
    HostTime, Link, MidiClient, MidiError, MidiPort, OsOutput, PortEvent, PortInfo, PortWatcher,
    Reconnector, destinations, diff_ports, source_names,
};

const CHILD_ENV: &str = "KS_REVIEW_S1_T2_R2_CHILD";

/// `PortWatcher::start`가 잠깐 만드는 탐침 소스의 이름(`watcher.rs`).
const PROBE_NAME: &str = "KeyScore watcher probe";

/// 탐침 소스는 시스템 전체에 보이므로, 자식 프로세스가 탐침 개수를 세는 동안 이 바이너리의 다른 테스트가
/// `PortWatcher::start`를 부르면 개수가 흔들린다. 감시자를 시작하는 테스트는 이 잠금을 쥔다.
static WATCHER_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    WATCHER_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn init() -> MidiClient {
    MidiClient::new("ks-review-t2-r2").unwrap()
}

fn unique(name: &str) -> String {
    format!("KS rv2r2 {} {name}", std::process::id())
}

fn p(name: &str, id: i32) -> PortInfo {
    PortInfo::new(name, id)
}

fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cond()
}

fn find(name: &str) -> Option<PortInfo> {
    destinations().into_iter().find(|p| p.name() == name)
}

fn event_port(e: &PortEvent) -> &PortInfo {
    match e {
        PortEvent::Appeared(p) | PortEvent::Disappeared(p) => p,
    }
}

/// 이름이 `name`인 이벤트만 `limit` 동안 `count`개까지 모은다.
fn collect(
    rx: &mpsc::Receiver<PortEvent>,
    name: &str,
    count: usize,
    limit: Duration,
) -> Vec<PortEvent> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    while got.len() < count {
        let left = deadline.saturating_duration_since(Instant::now());
        let Ok(e) = rx.recv_timeout(left) else { break };
        if event_port(&e).name() == name {
            got.push(e);
        }
    }
    got
}

fn count_sources(name: &str) -> usize {
    source_names().iter().filter(|n| *n == name).count()
}

/// 고유 ID를 고정한 가상 목적지. 받은 바이트를 `tx`로 넘긴다.
fn fixed_destination(
    client: &coremidi::Client,
    name: &str,
    id: i32,
    tx: mpsc::Sender<Vec<u8>>,
) -> coremidi::VirtualDestination {
    let dest = client
        .virtual_destination(name, move |list: &coremidi::PacketList| {
            for packet in list {
                let _ = tx.send(packet.data().to_vec());
            }
        })
        .unwrap();
    dest.set_property_integer("uniqueID", id).unwrap();
    dest
}

/// 이 테스트 하나만 자식 프로세스에서 돌린다. 자식이면 `Some(())`, 부모면 자식 결과를 확인하고 `None`.
fn in_child(test: &str, mode: &str) -> Option<()> {
    if std::env::var(CHILD_ENV).as_deref() == Ok(mode) {
        return Some(());
    }
    let _serial = serial();
    let out = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, mode)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "자식 프로세스 실패\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    None
}

/// 경계(못 깨뜨림). 객체 참조 비교가 기대는 전제다. `CoreMIDI`는 이미 쓰이는 고유 ID를 다른 목적지에
/// 주지 않는다(`kMIDIIDNotUnique`). 그래서 이름·ID가 같은 목적지 둘이 동시에 있어 `diff_ports`나
/// `Reconnector`가 하나의 사라짐을 둘의 사라짐으로 볼 일은 없다.
#[test]
fn second_destination_with_taken_unique_id_is_refused() {
    let _midi = init();
    let other = coremidi::Client::new("ks-review-r2-dup-id").unwrap();
    let name = unique("dup id");
    let id = 0x4b55_0000_i32 | (std::process::id().cast_signed() & 0xffff);
    let (tx, _rx) = mpsc::channel();
    let _first = fixed_destination(&other, &name, id, tx);
    let second = other
        .virtual_destination(&name, |_: &coremidi::PacketList| {})
        .unwrap();
    assert_eq!(second.set_property_integer("uniqueID", id), Err(-10843));
    assert!(wait_until(Duration::from_secs(2), || {
        destinations()
            .iter()
            .filter(|p| p.name() == name && p.unique_id() == id)
            .count()
            == 1
    }));
}

/// 경계(못 깨뜨림). 객체 참조가 다시 쓰이면 같은 이름·ID의 빠른 재생성을 다시 놓친다. 10회 연속으로
/// 지우고 같은 이름·ID로 다시 만들어도 매번 사라짐·나타남이 오고, 재연결한 출력으로 송신이 닿는다.
#[test]
fn watcher_repeated_quick_recreate_with_same_unique_id_reports_every_round() {
    let midi = init();
    let _serial = serial();
    let (_watcher, events) = PortWatcher::start().unwrap();
    let other = coremidi::Client::new("ks-review-r2-recreate").unwrap();
    let name = unique("recreate loop");
    let id = 0x4b56_0000_i32 | (std::process::id().cast_signed() & 0xffff);
    let (tx, _first_rx) = mpsc::channel();
    let mut dest = fixed_destination(&other, &name, id, tx);
    assert!(wait_until(Duration::from_secs(2), || {
        find(&name).is_some_and(|p| p.unique_id() == id)
    }));
    let mut reconnector = Reconnector::new();
    assert_eq!(
        reconnector.want(&name, &destinations()),
        &Link::Connected(p(&name, id))
    );
    // 생성·ID 변경 때의 이벤트를 비운다.
    let _ = collect(&events, &name, 10, Duration::from_millis(500));

    for round in 0..10 {
        drop(dest);
        let (tx, rx) = mpsc::channel();
        dest = fixed_destination(&other, &name, id, tx);
        // 무작위 ID로 잠깐 나타났다가 고정 ID로 바뀌는 사이의 이벤트도 함께 받는다. 마지막 이벤트는
        // 고정 ID의 나타남이어야 한다.
        let mut actions = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_gone = false;
        while Instant::now() < deadline {
            let left = deadline.saturating_duration_since(Instant::now());
            let Ok(e) = events.recv_timeout(left) else {
                break;
            };
            if event_port(&e).name() != name {
                continue;
            }
            if matches!(&e, PortEvent::Disappeared(q) if q.unique_id() == id) {
                saw_gone = true;
            }
            let done = matches!(&e, PortEvent::Appeared(q) if q.unique_id() == id);
            actions.extend(reconnector.apply(&e));
            if done {
                break;
            }
        }
        assert!(saw_gone, "{round}회차: 옛 목적지의 사라짐이 없다");
        assert_eq!(
            reconnector.link(&name),
            Some(&Link::Connected(p(&name, id))),
            "{round}회차: {actions:?}"
        );
        let out = OsOutput::open(&midi, &p(&name, id)).unwrap();
        assert!(out.is_present(), "{round}회차");
        out.send(HostTime::now(), &[0x2091_3c64]).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).ok(),
            Some(vec![0x91, 0x3c, 0x64]),
            "{round}회차: 다시 연 출력으로 송신이 닿지 않았다"
        );
    }
    drop(dest);
}

/// 경계(못 깨뜨림). 객체 참조를 비교에 넣어도 같은 목적지를 두 번 열거하면 참조가 같아 이벤트가 없다
/// (가짜 사라짐·나타남이 없다).
#[test]
fn diff_ports_of_two_snapshots_without_change_reports_nothing_for_live_destination() {
    let _midi = init();
    let other = coremidi::Client::new("ks-review-r2-stable").unwrap();
    let name = unique("stable");
    let _dest = other
        .virtual_destination(&name, |_: &coremidi::PacketList| {})
        .unwrap();
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_some()));
    for _ in 0..20 {
        let a = destinations();
        let b = destinations();
        let ours: Vec<_> = diff_ports(&a, &b)
            .into_iter()
            .filter(|e| event_port(e).name() == name)
            .collect();
        assert_eq!(ours, vec![]);
    }
}

/// 경계(못 깨뜨림). 「최소 ID」 규칙을 묶음 방식만이 아니라 모든 이벤트 순서로 시험한다. 포트 넷의 모든
/// 시작·끝 부분집합 쌍과, 그 사이 사라짐·나타남 이벤트의 모든 순서에서, 매 단계의 연결 대상이 그때
/// 있는 같은 이름 포트 가운데 최소 ID다.
#[test]
fn reconnector_any_event_order_links_lowest_present_id_at_every_step() {
    let universe = [p("Dup", 5), p("Dup", 20), p("Dup", 30), p("Other", 1)];
    let subset = |mask: u32| -> Vec<PortInfo> {
        (0..universe.len())
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| universe[i].clone())
            .collect()
    };
    let best = |set: &[PortInfo]| -> Link {
        set.iter()
            .filter(|q| q.name() == "Dup")
            .min_by_key(|q| q.unique_id())
            .map_or(Link::Missing, |q| Link::Connected(q.clone()))
    };
    let mut checked = 0usize;
    for start_mask in 0..16 {
        for end_mask in 0..16 {
            let (start, end) = (subset(start_mask), subset(end_mask));
            let mut events: Vec<PortEvent> = start
                .iter()
                .filter(|q| !end.contains(q))
                .map(|q| PortEvent::Disappeared(q.clone()))
                .chain(
                    end.iter()
                        .filter(|q| !start.contains(q))
                        .map(|q| PortEvent::Appeared(q.clone())),
                )
                .collect();
            for order in permutations(&mut events) {
                let mut r = Reconnector::new();
                r.want("Dup", &start);
                let mut now = start.clone();
                for e in &order {
                    let actions = r.apply(e);
                    match e {
                        PortEvent::Appeared(q) => now.push(q.clone()),
                        PortEvent::Disappeared(q) => now.retain(|x| x != q),
                    }
                    assert_eq!(
                        r.link("Dup"),
                        Some(&best(&now)),
                        "{start:?} -> {end:?}, 순서 {order:?}, 동작 {actions:?}"
                    );
                }
                assert_eq!(r.link("Dup"), Some(&best(&end)));
                checked += 1;
            }
        }
    }
    assert!(checked > 256);
}

/// `items`의 모든 순열(힙 알고리즘). 최대 4개라 24가지다.
fn permutations(items: &mut [PortEvent]) -> Vec<Vec<PortEvent>> {
    fn heap(k: usize, items: &mut [PortEvent], out: &mut Vec<Vec<PortEvent>>) {
        if k <= 1 {
            out.push(items.to_vec());
            return;
        }
        for i in 0..k {
            heap(k - 1, items, out);
            if k.is_multiple_of(2) {
                items.swap(i, k - 1);
            } else {
                items.swap(0, k - 1);
            }
        }
    }
    let mut out = Vec::new();
    heap(items.len(), items, &mut out);
    out
}

/// 경계(못 깨뜨림). 탐침의 가짜 실패를 노렸다. 감시자 40개가 알림마다 목록을 다시 읽고, 다른 스레드가
/// 목적지를 쉬지 않고 만들고 지우는 동안 16개 스레드가 함께 `start`를 불러도 모두 성공하고, 탐침 대기
/// 상한(2초)보다 훨씬 빨리 끝난다. 실측은 각각 약 0.2–0.25초였다.
#[test]
fn watcher_concurrent_starts_under_churn_all_succeed_well_before_probe_timeout() {
    let _midi = init();
    let _serial = serial();
    let keep: Vec<_> = (0..40).map(|_| PortWatcher::start().unwrap()).collect();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let churn = {
        let stop = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            let other = coremidi::Client::new("ks-review-r2-churn").unwrap();
            let name = unique("churn");
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let dest = other
                    .virtual_destination(&name, |_: &coremidi::PacketList| {})
                    .unwrap();
                std::thread::sleep(Duration::from_millis(1));
                drop(dest);
            }
        })
    };
    let starts: Vec<_> = (0..16)
        .map(|_| {
            std::thread::spawn(|| {
                let t = Instant::now();
                let r = PortWatcher::start().map(|_| ());
                (t.elapsed(), r)
            })
        })
        .collect();
    let results: Vec<_> = starts.into_iter().map(|h| h.join().unwrap()).collect();
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    churn.join().unwrap();
    drop(keep);
    for (took, r) in &results {
        assert_eq!(r, &Ok(()), "{results:?}");
        assert!(*took < Duration::from_secs(1), "{results:?}");
    }
}

/// 경계(못 깨뜨림, 자식 프로세스). 다른 코드가 run loop이 도는 스레드에서 첫 클라이언트를 만든 경우다.
/// 알림이 그 스레드로 가므로 `start`는 탐침 대기 상한을 기다리지 않고 곧바로 `NotificationsUnavailable`을
/// 돌려준다. 거듭 불러도 같고, 그 뒤 다른 스레드에서 알림이 계속 와도 프로세스가 죽지 않으며(오류 경로가
/// 공유 상태를 놓지 않는다), 탐침 소스가 남지 않고, 가상 포트는 그대로 쓸 수 있다.
#[test]
fn watcher_start_after_foreign_first_client_on_run_loop_thread_fails_fast_and_stays_usable() {
    if in_child(
        "watcher_start_after_foreign_first_client_on_run_loop_thread_fails_fast_and_stays_usable",
        "foreign-run-loop",
    )
    .is_none()
    {
        return;
    }
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _foreign = coremidi::Client::new_with_notifications(
            "ks-review-r2-foreign",
            |_: &coremidi::Notification| {},
        )
        .unwrap();
        ready_tx.send(()).unwrap();
        loop {
            CFRunLoop::run_current();
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    ready_rx.recv().unwrap();
    let midi = init();
    for attempt in 0..3 {
        let t = Instant::now();
        let r = PortWatcher::start().map(|_| ());
        assert_eq!(r, Err(MidiError::NotificationsUnavailable), "{attempt}");
        assert!(t.elapsed() < Duration::from_millis(1500), "{attempt}");
    }
    let other = coremidi::Client::new("ks-review-r2-foreign-churn").unwrap();
    for i in 0..30 {
        let dest = other
            .virtual_destination(
                &unique(&format!("foreign churn {i}")),
                |_: &coremidi::PacketList| {},
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(5));
        drop(dest);
    }
    assert!(wait_until(Duration::from_secs(2), || count_sources(
        PROBE_NAME
    ) == 0));
    let name = unique("after foreign");
    let port = midi.create_virtual_output(&name).unwrap();
    assert!(wait_until(Duration::from_secs(2), || count_sources(&name) == 1));
    assert!(port.is_registered());
}

/// 경계(못 깨뜨림, 자식 프로세스). 탐침 소스와 이름이 같은 가상 포트가 있어도 `start`는 성공하고, 끝나면
/// 탐침만 사라지고 앱의 포트는 남는다. 성공 경로에서도 탐침 소스가 남지 않는다.
#[test]
fn watcher_start_with_own_port_named_like_probe_removes_only_probe() {
    if in_child(
        "watcher_start_with_own_port_named_like_probe_removes_only_probe",
        "probe-name",
    )
    .is_none()
    {
        return;
    }
    let midi = init();
    let port = midi.create_virtual_output(PROBE_NAME).unwrap();
    assert!(wait_until(Duration::from_secs(2), || count_sources(
        PROBE_NAME
    ) == 1));
    let mut watchers = Vec::new();
    for _ in 0..3 {
        watchers.push(PortWatcher::start().unwrap());
        assert_eq!(count_sources(PROBE_NAME), 1);
    }
    assert!(port.is_registered());
    drop(port);
    assert!(wait_until(Duration::from_secs(2), || count_sources(
        PROBE_NAME
    ) == 0));
    watchers.push(PortWatcher::start().unwrap());
    assert_eq!(count_sources(PROBE_NAME), 0);
}

/// 경계(못 깨뜨림). 다시 등록한 뒤 이름 바꾸기와 해제가 새 엔드포인트에 작동한다. 해제하면 소스가
/// 사라지고 이름 자리가 풀려 같은 이름을 다시 만들 수 있다(옛 엔드포인트를 두 번 해제하거나 새 것을
/// 남기지 않는다).
#[test]
fn reregister_then_rename_and_drop_act_on_new_endpoint_and_free_name() {
    let midi = init();
    let first = unique("rereg first");
    let second = unique("rereg second");
    let mut port = midi.create_virtual_output(&first).unwrap();
    for _ in 0..3 {
        port.reregister(&midi).unwrap();
    }
    assert!(wait_until(Duration::from_secs(2), || count_sources(&first) == 1));
    port.rename(&second).unwrap();
    assert!(wait_until(Duration::from_secs(2), || {
        count_sources(&first) == 0 && count_sources(&second) == 1
    }));
    // 옛 이름 자리는 풀렸고, 새 이름은 이 포트가 쥐고 있다.
    let again = midi.create_virtual_output(&first).unwrap();
    assert_eq!(
        midi.create_virtual_output(&second).err(),
        Some(MidiError::DuplicateName(second.clone()))
    );
    port.reregister(&midi).unwrap();
    assert!(port.is_registered());
    drop(port);
    assert!(wait_until(Duration::from_secs(2), || count_sources(
        &second
    ) == 0));
    let reborn = midi.create_virtual_output(&second).unwrap();
    drop((again, reborn));
}

/// 경계(못 깨뜨림). 사라진 목적지로 보내도 송신 오류는 할당 없는 `DestinationGone`이거나 성공(서버가
/// 조용히 버림)뿐이고, 이름을 담는 `PortGone`은 나오지 않는다.
#[test]
fn os_output_send_after_destination_removed_returns_ok_or_destination_gone() {
    let midi = init();
    let other = coremidi::Client::new("ks-review-r2-gone").unwrap();
    let name = unique("gone");
    let dest = other
        .virtual_destination(&name, |_: &coremidi::PacketList| {})
        .unwrap();
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_some()));
    let out = OsOutput::open(&midi, &find(&name).unwrap()).unwrap();
    drop(dest);
    assert!(wait_until(Duration::from_secs(2), || !out.is_present()));
    for _ in 0..50 {
        let r = out.send(HostTime::now(), &[0x2091_3c64]);
        assert!(
            matches!(r, Ok(()) | Err(MidiError::DestinationGone)),
            "{r:?}"
        );
    }
}
