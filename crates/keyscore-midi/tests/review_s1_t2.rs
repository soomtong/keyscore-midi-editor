//! S1-T2 적대적 리뷰 1회차(`docs/reports/s1-t2/review-1.md`)의 테스트.
//!
//! 실패하는 테스트가 깨뜨린 사례이고, 통과하는 테스트는 경계를 고정한다. CoreMIDI 상태는 시스템
//! 전역이라 포트 이름에 프로세스 id를 넣는다.
//!
//! 「프로세스의 첫 클라이언트」 가정과 CPU 사용량은 프로세스 전체 상태라, 같은 바이너리를 자식
//! 프로세스로 다시 띄워 그 테스트 하나만 돌린다(`in_child`). 나머지 테스트는 `os_ports.rs`처럼
//! `coremidi` crate 클라이언트보다 먼저 `MidiClient::new`를 부른다.

use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_midi::{
    Link, MidiClient, MidiError, MidiPort, OsOutput, PortEvent, PortInfo, PortWatcher, Reconnector,
    destinations, diff_ports,
};

const CHILD_ENV: &str = "KS_REVIEW_S1_T2_CHILD";

fn init() -> MidiClient {
    MidiClient::new("ks-review-t2").unwrap()
}

fn unique(name: &str) -> String {
    format!("KS rv2 {} {name}", std::process::id())
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

fn event_name(e: &PortEvent) -> &str {
    match e {
        PortEvent::Appeared(p) | PortEvent::Disappeared(p) => p.name(),
    }
}

/// 이름이 `names`에 있는 이벤트만 `limit` 동안 모은다(시스템의 다른 포트 변화는 거른다).
fn collect(
    rx: &mpsc::Receiver<PortEvent>,
    names: &[&str],
    count: usize,
    limit: Duration,
) -> Vec<PortEvent> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    while got.len() < count {
        let left = deadline.saturating_duration_since(Instant::now());
        let Ok(e) = rx.recv_timeout(left) else { break };
        if names.contains(&event_name(&e)) {
            got.push(e);
        }
    }
    got
}

fn virtual_destination(client: &coremidi::Client, name: &str) -> coremidi::VirtualDestination {
    client
        .virtual_destination(name, |_: &coremidi::PacketList| {})
        .unwrap()
}

/// 이 테스트 하나만 자식 프로세스에서 돌린다. 자식이면 `Some(mode)`를 돌려주고, 부모면 자식을 띄워
/// 결과를 확인한 뒤 `None`을 돌려준다.
fn in_child(test: &str, mode: &str) -> Option<()> {
    if std::env::var(CHILD_ENV).as_deref() == Ok(mode) {
        return Some(());
    }
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

/// 이 프로세스가 지금까지 쓴 CPU 시간(사용자+시스템). `ps`의 `m:ss.cc` 꼴을 읽는다.
fn cpu_time() -> Duration {
    let out = Command::new("ps")
        .args(["-o", "time=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let secs = text.trim().split(':').fold(0.0_f64, |acc, part| {
        acc * 60.0 + part.parse::<f64>().unwrap()
    });
    Duration::from_secs_f64(secs)
}

/// 경계(못 깨뜨림). 쉬고 있을 때 CoreMIDI 스레드가 CPU를 태우지 않는다. 알림 함수가 없는 프로세스
/// 클라이언트만 있으면 그 스레드의 run loop에 소스가 없어 `CFRunLoopRunInMode`가 곧바로 끝나 `run`이
/// 바쁜 대기가 될 수 있다고 보고 쟀다. 실측은 2초에 20 ms 이하였다. 감시자를 만든 뒤도 함께 잰다.
#[test]
fn midi_thread_idle_after_client_create_uses_little_cpu() {
    if in_child(
        "midi_thread_idle_after_client_create_uses_little_cpu",
        "cpu",
    )
    .is_none()
    {
        return;
    }
    let _midi = init();
    std::thread::sleep(Duration::from_millis(300));
    let before = cpu_time();
    std::thread::sleep(Duration::from_secs(2));
    let client_only = cpu_time().saturating_sub(before);

    let (_watcher, _events) = PortWatcher::start().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let before = cpu_time();
    std::thread::sleep(Duration::from_secs(2));
    let with_watcher = cpu_time().saturating_sub(before);

    println!("CPU 2초 동안: 클라이언트만 {client_only:?}, 감시자 있음 {with_watcher:?}");
    assert!(
        client_only < Duration::from_millis(200) && with_watcher < Duration::from_millis(200),
        "쉬는 동안 CPU: 클라이언트만 {client_only:?}, 감시자 있음 {with_watcher:?}"
    );
}

/// 깨짐(수정, 보고서 「알려진 한계」의 실측). 다른 코드가 프로세스의 첫 CoreMIDI 클라이언트를 만들면
/// `PortWatcher::start`는 성공하지만 이벤트가 하나도 오지 않는다. 오류도 없어 앱은 핫플러그 감시가
/// 죽었음을 알 수 없다(OUT-3).
#[test]
fn watcher_after_foreign_first_client_reports_appeared_or_fails_start() {
    if in_child(
        "watcher_after_foreign_first_client_reports_appeared_or_fails_start",
        "foreign-first",
    )
    .is_none()
    {
        return;
    }
    let other = coremidi::Client::new("ks-review-foreign-first").unwrap();
    let _midi = init();
    let Ok((_watcher, events)) = PortWatcher::start() else {
        // 감시가 불가능함을 오류로 알렸다면 받아들인다.
        return;
    };
    let name = unique("foreign first");
    let _dest = virtual_destination(&other, &name);
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_some()));
    let got = collect(&events, &[&name], 1, Duration::from_secs(3));
    assert!(
        matches!(&got[..], [PortEvent::Appeared(p)] if p.name() == name),
        "감시자가 시작에 성공했는데 이벤트가 오지 않았다: {got:?}"
    );
}

/// 깨짐(제안). `on_midi_thread`는 스레드 이름으로 CoreMIDI 스레드를 알아본다. 같은 이름의 사용자
/// 스레드에서 처음 `MidiClient::new`를 부르면 클라이언트가 그 스레드에서 만들어지고, 알림은 run
/// loop이 돌지 않는 그 스레드로 간다.
#[test]
fn watcher_after_client_from_thread_named_like_midi_thread_reports_appeared() {
    if in_child(
        "watcher_after_client_from_thread_named_like_midi_thread_reports_appeared",
        "impostor",
    )
    .is_none()
    {
        return;
    }
    std::thread::Builder::new()
        .name("keyscore-coremidi".into())
        .spawn(init)
        .unwrap()
        .join()
        .unwrap();
    let (_watcher, events) = PortWatcher::start().unwrap();
    let other = coremidi::Client::new("ks-review-impostor").unwrap();
    let name = unique("impostor");
    let _dest = virtual_destination(&other, &name);
    let got = collect(&events, &[&name], 1, Duration::from_secs(3));
    assert!(
        matches!(&got[..], [PortEvent::Appeared(p)] if p.name() == name),
        "이벤트가 오지 않았다: {got:?}"
    );
}

/// 경계(못 깨뜨림). 계획은 「같은 ID인데 이름이 바뀌면 옛 이름 `Disappeared` 뒤 새 이름 `Appeared`」를
/// 약속한다. 감시자는 `kMIDIMsgSetupChanged`만 보므로 이름 변경(`kMIDIMsgPropertyChanged`)을 놓칠 수
/// 있다고 보고 시험했다. 실측으로는 이름 변경 뒤에도 `SetupChanged`가 와서 이벤트가 나온다.
#[test]
fn watcher_reports_rename_of_destination_as_disappear_then_appear() {
    let _midi = init();
    let (_watcher, events) = PortWatcher::start().unwrap();
    let other = coremidi::Client::new("ks-review-rename").unwrap();
    let old = unique("rename old");
    let new = unique("rename new");
    let dest = virtual_destination(&other, &old);
    let first = collect(&events, &[&old], 1, Duration::from_secs(3));
    let [PortEvent::Appeared(info)] = &first[..] else {
        panic!("생성 이벤트가 없다: {first:?}");
    };
    let id = info.unique_id();
    dest.set_property_string("name", &new).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || find(&new).is_some()),
        "이름 변경 자체가 반영되지 않았다"
    );
    let got = collect(&events, &[&old, &new], 2, Duration::from_secs(3));
    assert_eq!(
        got,
        vec![
            PortEvent::Disappeared(p(&old, id)),
            PortEvent::Appeared(p(&new, id))
        ]
    );
}

/// 경계. 감시자 둘이 같은 변화를 각자 받는다.
#[test]
fn two_watchers_both_receive_appeared_and_disappeared() {
    let _midi = init();
    let (w1, e1) = PortWatcher::start().unwrap();
    let (w2, e2) = PortWatcher::start().unwrap();
    let other = coremidi::Client::new("ks-review-two").unwrap();
    let name = unique("two watchers");
    let dest = virtual_destination(&other, &name);
    for events in [&e1, &e2] {
        let got = collect(events, &[&name], 1, Duration::from_secs(3));
        assert!(matches!(&got[..], [PortEvent::Appeared(_)]), "{got:?}");
    }
    // 하나를 놓아도 다른 하나는 계속 받는다.
    drop(w1);
    drop(dest);
    let got = collect(&e2, &[&name], 1, Duration::from_secs(3));
    assert!(matches!(&got[..], [PortEvent::Disappeared(_)]), "{got:?}");
    drop(w2);
}

/// 경계. 목적지가 빠르게 생기고 사라지는 동안 감시자를 거듭 만들고 놓아도 죽지 않고(해제 뒤 콜백이
/// refcon을 쓰면 여기서 터질 수 있다), 마지막 감시자는 여전히 이벤트를 받는다.
#[test]
fn watcher_start_drop_churn_while_destinations_change_keeps_working() {
    let _midi = init();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let churn = {
        let stop = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            let other = coremidi::Client::new("ks-review-churn").unwrap();
            let name = unique("churn");
            let mut n = 0u32;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let dest = virtual_destination(&other, &name);
                std::thread::sleep(Duration::from_millis(3));
                drop(dest);
                n += 1;
            }
            n
        })
    };
    let start = Instant::now();
    let mut cycles = 0;
    while start.elapsed() < Duration::from_secs(3) {
        let (w, events) = PortWatcher::start().unwrap();
        std::thread::sleep(Duration::from_millis(5));
        drop(w);
        // 놓은 뒤 채널은 닫혀야 한다(쌓인 이벤트는 비운다).
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => panic!("drop 뒤에도 채널이 열려 있다"),
            }
        }
        cycles += 1;
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let made = churn.join().unwrap();
    println!("감시자 {cycles}회, 목적지 {made}회");
    let (_w, events) = PortWatcher::start().unwrap();
    let other = coremidi::Client::new("ks-review-churn-last").unwrap();
    let name = unique("churn last");
    let _dest = virtual_destination(&other, &name);
    let got = collect(&events, &[&name], 1, Duration::from_secs(3));
    assert!(matches!(&got[..], [PortEvent::Appeared(_)]), "{got:?}");
}

/// 경계. 소스(앱 자신의 가상 포트)의 고유 ID로는 `OsOutput`을 열 수 없다(`kind == 3` 검사).
#[test]
fn os_output_open_with_unique_id_of_source_fails_with_port_gone() {
    let midi = init();
    let name = unique("source id");
    let out = midi.create_virtual_output(&name).unwrap();
    assert_eq!(out.name(), name);
    let mut source_id = None;
    assert!(wait_until(Duration::from_secs(2), || {
        source_id = coremidi::Sources
            .into_iter()
            .find(|s| s.name().as_deref() == Some(name.as_str()))
            .and_then(|s| s.unique_id());
        source_id.is_some()
    }));
    let info = p(&name, source_id.unwrap().cast_signed());
    assert_eq!(
        OsOutput::open(&midi, &info).err(),
        Some(MidiError::PortGone(name))
    );
}

/// 깨짐(수정). 계획은 「같은 이름의 포트가 여럿이면 고유 ID가 가장 작은 것을 고른다(결정적)」이다. 같은
/// 최종 목록이라도, 변화가 한 번의 비교(사라짐 먼저)로 왔는지 두 번의 알림으로 나눠 왔는지에 따라 연결
/// 대상이 달라진다.
#[test]
fn reconnector_same_final_ports_reach_same_link_regardless_of_batching() {
    let start = [p("Dup", 20), p("Dup", 30)];
    let end = [p("Dup", 5), p("Dup", 30)];

    // 한 번의 비교: Disappeared(20) 뒤 Appeared(5).
    let mut batched = Reconnector::new();
    batched.want("Dup", &start);
    for e in diff_ports(&start, &end) {
        batched.apply(&e);
    }

    // 두 번의 알림: 5가 먼저 나타나고, 그 뒤 20이 사라짐.
    let mut split = Reconnector::new();
    split.want("Dup", &start);
    split.apply(&PortEvent::Appeared(p("Dup", 5)));
    split.apply(&PortEvent::Disappeared(p("Dup", 20)));

    assert_eq!(split.link("Dup"), Some(&Link::Connected(p("Dup", 5))));
    assert_eq!(
        batched.link("Dup"),
        split.link("Dup"),
        "같은 최종 목록에서 묶음 방식에 따라 연결 대상이 다르다"
    );
}

/// 경계. 이름이 바뀐 포트(같은 ID)는 옛 이름의 연결을 끊고, 같은 이름으로 돌아오면 다시 잇는다.
#[test]
fn reconnector_rename_away_and_back_loses_then_reconnects() {
    let mut r = Reconnector::new();
    assert_eq!(r.want("A", &[p("A", 1)]), &Link::Connected(p("A", 1)));
    for e in diff_ports(&[p("A", 1)], &[p("B", 1)]) {
        r.apply(&e);
    }
    assert_eq!(r.link("A"), Some(&Link::Missing));
    for e in diff_ports(&[p("B", 1)], &[p("A", 1)]) {
        r.apply(&e);
    }
    assert_eq!(r.link("A"), Some(&Link::Connected(p("A", 1))));
}

/// 깨짐(차단). 다른 앱이 고유 ID를 고정한 가상 목적지를 빠르게 다시 만들면(앱 재시작. 고유 ID를
/// 저장해 다시 쓰는 것은 `CoreMIDI`가 권하는 관행이다) 알림이 묶여 와서 `diff_ports`의 앞뒤 목록이
/// 같아진다. 이벤트가 없으니 `Reconnector`는 연결된 줄 알고, 기존 `OsOutput`은 사라진 엔드포인트로
/// 보내 `CoreMIDI`가 소리 없이 버린다. 같은 이름의 포트가 돌아왔는데 소리가 나지 않는다(OUT-3).
#[test]
fn watcher_quick_recreate_with_same_unique_id_keeps_output_delivering_or_reports_change() {
    let _midi = init();
    let (_watcher, events) = PortWatcher::start().unwrap();
    let other = coremidi::Client::new("ks-review-recreate").unwrap();
    let name = unique("recreate");
    let fixed = 0x4b52_0000_i32 | (std::process::id().cast_signed() & 0xffff);
    let dest = virtual_destination(&other, &name);
    dest.set_property_integer("uniqueID", fixed).unwrap();
    assert!(wait_until(Duration::from_secs(2), || {
        find(&name).is_some_and(|p| p.unique_id() == fixed)
    }));
    let midi = init();
    let mut reconnector = Reconnector::new();
    assert_eq!(
        reconnector.want(&name, &destinations()),
        &Link::Connected(p(&name, fixed))
    );
    let out = OsOutput::open(&midi, &p(&name, fixed)).unwrap();
    // 생성·ID 변경 때의 이벤트를 비운다.
    let _ = collect(&events, &[&name], 10, Duration::from_millis(500));

    drop(dest);
    let (tx, rx) = mpsc::channel();
    let again = other
        .virtual_destination(&name, move |list: &coremidi::PacketList| {
            for packet in list {
                let _ = tx.send(packet.data().to_vec());
            }
        })
        .unwrap();
    again.set_property_integer("uniqueID", fixed).unwrap();
    assert!(wait_until(Duration::from_secs(2), || {
        find(&name).is_some_and(|p| p.unique_id() == fixed)
    }));
    let got = collect(&events, &[&name], 2, Duration::from_secs(2));
    let actions: Vec<_> = got.iter().flat_map(|e| reconnector.apply(e)).collect();
    out.send(keyscore_midi::HostTime::now(), &[0x2091_3c64])
        .unwrap();
    let delivered = rx.recv_timeout(Duration::from_secs(1)).is_ok();
    assert!(
        delivered || !actions.is_empty(),
        "같은 이름·ID의 목적지가 돌아왔는데 송신이 닿지 않고 재연결 동작도 없다. \
         is_present={}, events={got:?}",
        out.is_present()
    );
}
