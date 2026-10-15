//! S1-T2 OS 출력 포트 열거·송신·감시 통합 테스트.
//!
//! 목적지는 `coremidi` crate의 가상 목적지로 만든다(다른 앱의 가상 포트를 흉내 낸다). 그 crate의
//! 클라이언트는 해제되지 않으므로 MIDI 서버를 붙잡아 두는 부작용도 있다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_midi::{
    Action, HostTime, Link, MidiClient, MidiError, MidiPort, OsOutput, PortEvent, PortInfo,
    PortWatcher, Reconnector, destinations,
};

fn unique(name: &str) -> String {
    format!("KS os {} {name}", std::process::id())
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

/// 받은 바이트를 채널로 넘기는 가상 목적지.
fn destination(
    client: &coremidi::Client,
    name: &str,
) -> (coremidi::VirtualDestination, mpsc::Receiver<Vec<u8>>) {
    let (tx, rx) = mpsc::channel();
    let dest = client
        .virtual_destination(name, move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
            }
        })
        .unwrap();
    (dest, rx)
}

const NOTE_ON: u32 = 0x2091_3c64;

/// 이름이 `names`에 있는 이벤트만 기다려 모은다(시스템의 다른 포트 변화는 거른다).
fn collect(rx: &mpsc::Receiver<PortEvent>, names: &[&str], count: usize) -> Vec<PortEvent> {
    let mut got = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while got.len() < count {
        let left = deadline.saturating_duration_since(Instant::now());
        let Ok(e) = rx.recv_timeout(left) else { break };
        let name = match &e {
            PortEvent::Appeared(p) | PortEvent::Disappeared(p) => p.name().to_owned(),
        };
        if names.contains(&name.as_str()) {
            got.push(e);
        }
    }
    got
}

#[test]
fn destinations_lists_virtual_destination_created_by_other_client() {
    let other = coremidi::Client::new("ks-os-enum").unwrap();
    let name = unique("enum");
    let (dest, _rx) = destination(&other, &name);
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_some()));
    let info = find(&name).unwrap();
    assert_ne!(info.unique_id(), 0);
    drop(dest);
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_none()));
}

#[test]
fn os_output_send_reaches_destination() {
    let other = coremidi::Client::new("ks-os-send").unwrap();
    let name = unique("send");
    let (_dest, rx) = destination(&other, &name);
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_some()));
    let client = MidiClient::new("ks-os-send").unwrap();
    let out = OsOutput::open(&client, &find(&name).unwrap()).unwrap();
    assert_eq!(out.name(), name);
    out.send(HostTime::now(), &[NOTE_ON]).unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        vec![0x91, 0x3c, 0x64]
    );
    assert_eq!(
        out.send(HostTime::from_ticks(0), &[NOTE_ON]),
        Err(MidiError::ZeroTimestamp)
    );
    assert_eq!(
        out.send(HostTime::now(), &[NOTE_ON, 0]),
        Err(MidiError::InvalidMessage)
    );
}

#[test]
fn os_output_send_after_destination_removed_fails_with_port_gone() {
    let other = coremidi::Client::new("ks-os-gone").unwrap();
    let name = unique("gone");
    let (dest, _rx) = destination(&other, &name);
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_some()));
    let info = find(&name).unwrap();
    let client = MidiClient::new("ks-os-gone").unwrap();
    let out = OsOutput::open(&client, &info).unwrap();
    drop(dest);
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_none()));
    assert_eq!(
        out.send(HostTime::now(), &[NOTE_ON]),
        Err(MidiError::PortGone(name.clone()))
    );
    // 없는 포트는 열 수 없다.
    assert_eq!(
        OsOutput::open(&client, &info).err(),
        Some(MidiError::PortGone(name))
    );
}

#[test]
fn watcher_reports_disappear_then_appear_in_order() {
    let (watcher, events) = PortWatcher::start().unwrap();
    let other = coremidi::Client::new("ks-os-watch").unwrap();
    let a = unique("watch a");
    let b = unique("watch b");
    let (dest_a, _ra) = destination(&other, &a);
    let first = collect(&events, &[&a, &b], 1);
    assert!(
        matches!(&first[..], [PortEvent::Appeared(p)] if p.name() == a),
        "{first:?}"
    );
    let (dest_b, _rb) = destination(&other, &b);
    let second = collect(&events, &[&a, &b], 1);
    assert!(
        matches!(&second[..], [PortEvent::Appeared(p)] if p.name() == b),
        "{second:?}"
    );
    drop(dest_a);
    drop(dest_b);
    let gone = collect(&events, &[&a, &b], 2);
    assert_eq!(gone.len(), 2, "{gone:?}");
    assert!(
        gone.iter().all(|e| matches!(e, PortEvent::Disappeared(_))),
        "{gone:?}"
    );
    let (_again, _r) = destination(&other, &a);
    let back = collect(&events, &[&a, &b], 1);
    assert!(
        matches!(&back[..], [PortEvent::Appeared(p)] if p.name() == a),
        "{back:?}"
    );
    drop(watcher);
}

#[test]
fn watcher_drop_stops_thread_promptly() {
    let (watcher, events) = PortWatcher::start().unwrap();
    let start = Instant::now();
    drop(watcher);
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "{:?}",
        start.elapsed()
    );
    // 스레드가 끝나면 보내는 쪽이 사라져 채널이 닫힌다.
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

/// 단계 게이트: 포트가 사라졌다 같은 이름으로 돌아오면 다시 연결해 송신이 새 포트에 도착한다.
#[test]
fn simulated_port_vanish_and_return_reconnects_end_to_end() {
    let (_watcher, events) = PortWatcher::start().unwrap();
    let other = coremidi::Client::new("ks-os-e2e").unwrap();
    let client = MidiClient::new("ks-os-e2e").unwrap();
    let name = unique("e2e");
    let (dest, first_rx) = destination(&other, &name);
    assert!(wait_until(Duration::from_secs(2), || find(&name).is_some()));

    let mut reconnector = Reconnector::new();
    let Link::Connected(port) = reconnector.want(&name, &destinations()).clone() else {
        panic!("처음에 연결되지 않았다");
    };
    let mut out = Some(OsOutput::open(&client, &port).unwrap());
    out.as_ref()
        .unwrap()
        .send(HostTime::now(), &[NOTE_ON])
        .unwrap();
    assert!(first_rx.recv_timeout(Duration::from_secs(2)).is_ok());
    let _ = collect(&events, &[&name], 1); // 생성 때의 Appeared는 버린다.

    drop(dest);
    let mut actions = Vec::new();
    for e in collect(&events, &[&name], 1) {
        actions.extend(reconnector.apply(&e));
    }
    assert_eq!(actions, vec![Action::Lost { name: name.clone() }]);
    out = None;

    let (_dest2, rx2) = destination(&other, &name);
    for e in collect(&events, &[&name], 1) {
        for action in reconnector.apply(&e) {
            if let Action::Reconnect { port, .. } = action {
                out = Some(OsOutput::open(&client, &port).unwrap());
            }
        }
    }
    let out = out.expect("다시 연결되지 않았다");
    out.send(HostTime::now(), &[NOTE_ON]).unwrap();
    assert_eq!(
        rx2.recv_timeout(Duration::from_secs(2)).unwrap(),
        vec![0x91, 0x3c, 0x64]
    );
}
