//! S1-T1 가상 출력 포트 통합 테스트.
//!
//! 수신 쪽은 `coremidi` crate(구현과 다른 경로, 레거시 입력 포트)로 받아 대조한다.
//! CoreMIDI 상태는 시스템 전역이라, 포트 이름에 프로세스 id와 테스트 이름을 넣어 겹치지 않게 한다.

use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_midi::{HostTime, MAX_VIRTUAL_OUTPUTS, MidiClient, MidiError, MidiPort, source_names};

fn unique(name: &str) -> String {
    format!("KS test {} {name}", std::process::id())
}

/// 상한까지 포트를 채우거나 정확한 개수를 세는 테스트는 프로세스 전역 상한을 같은 바이너리의 다른
/// 테스트와 나눠 쓰지 않게 자식 프로세스에서 돈다(결정 `2026-10-15-virtual-port-limit-scope.md`).
/// 부모에서는 자식을 띄워 성공 여부만 보고 `true`를, 자식 안에서는 `false`를 돌려준다.
fn run_isolated(test: &str) -> bool {
    const ENV: &str = "KS_MIDI_ISOLATED_TEST";
    if std::env::var(ENV).as_deref() == Ok(test) {
        return false;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(ENV, test)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    // 이름이 틀리면 자식은 아무것도 돌리지 않고 성공하므로, 정확히 하나가 돌았는지도 본다(리뷰 3 제안 4).
    assert!(
        out.status.success() && stdout.contains("test result: ok. 1 passed"),
        "{test}: 자식 프로세스에서 실패했거나 테스트를 찾지 못했다\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    true
}

/// 조건이 참이 될 때까지 기다린다. CoreMIDI 서버와의 동기화에 약간의 시간이 걸릴 수 있다.
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

/// 수신기 쪽 소스를 찾는다. `coremidi` crate의 열거도 인덱스 기반이라 다른 테스트가 소스를
/// 만들고 지우는 동안 잠깐 놓칠 수 있어(리뷰 1 지적 3) 다시 시도한다.
fn find_source(name: &str) -> coremidi::Source {
    let mut found = None;
    assert!(wait_until(Duration::from_secs(2), || {
        found = coremidi::Source::from_name(name);
        found.is_some()
    }));
    found.unwrap()
}

fn has_source(name: &str) -> bool {
    source_names().iter().any(|n| n == name)
}

/// MIDI 1.0 채널 보이스 UMP(메시지 타입 2) Note On.
fn note_on_ump(channel: u8, note: u8, velocity: u8) -> u32 {
    0x2000_0000 | (u32::from(0x90 | channel) << 16) | (u32::from(note) << 8) | u32::from(velocity)
}

/// ms를 mach 틱으로 바꾼다. 틱 단위는 기기마다 다르므로(Apple Silicon 125/3 ns, Intel 1 ns)
/// 실제 경과 시간으로 비율을 잰다. 테스트에서 예약 시각을 만드는 데만 쓴다.
fn ms_to_ticks(ms: u64) -> u64 {
    let (t0, i0) = (HostTime::now(), Instant::now());
    std::thread::sleep(Duration::from_millis(50));
    let (t1, i1) = (HostTime::now(), Instant::now());
    let ticks = u128::from(t1.ticks() - t0.ticks());
    let nanos = i1.duration_since(i0).as_nanos();
    u64::try_from(ticks * u128::from(ms) * 1_000_000 / nanos).unwrap()
}

#[test]
fn virtual_output_create_is_visible_to_source_enumeration() {
    let client = MidiClient::new("ks-test-enum").unwrap();
    let name = unique("enum");
    let port = client.create_virtual_output(&name).unwrap();
    assert_eq!(port.name(), name);
    assert!(wait_until(Duration::from_secs(2), || has_source(&name)));
    drop(port);
    assert!(wait_until(Duration::from_secs(2), || !has_source(&name)));
}

#[test]
fn virtual_output_send_is_received_by_in_process_client() {
    let client = MidiClient::new("ks-test-send").unwrap();
    let name = unique("send");
    let port = client.create_virtual_output(&name).unwrap();
    assert!(wait_until(Duration::from_secs(2), || has_source(&name)));

    let (tx, rx) = mpsc::channel::<(u64, Vec<u8>)>();
    let receiver = coremidi::Client::new("ks-test-receiver").unwrap();
    let input = receiver
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send((p.timestamp(), p.data().to_vec()));
            }
        })
        .unwrap();
    input.connect_source(&find_source(&name)).unwrap();

    let at = HostTime::now();
    port.send(at, &[note_on_ump(2, 60, 100)]).unwrap();
    let (ts, data) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(data, vec![0x92, 60, 100]);
    assert_eq!(ts, at.ticks());
}

#[test]
fn virtual_output_future_timestamp_arrives_no_earlier_than_scheduled() {
    let client = MidiClient::new("ks-test-future").unwrap();
    let name = unique("future");
    let port = client.create_virtual_output(&name).unwrap();
    assert!(wait_until(Duration::from_secs(2), || has_source(&name)));

    let (tx, rx) = mpsc::channel::<Instant>();
    let receiver = coremidi::Client::new("ks-test-future-rx").unwrap();
    let input = receiver
        .input_port("in", move |_list: &coremidi::PacketList| {
            let _ = tx.send(Instant::now());
        })
        .unwrap();
    input.connect_source(&find_source(&name)).unwrap();

    let sent = Instant::now();
    let at = HostTime::from_ticks(HostTime::now().ticks() + ms_to_ticks(20));
    port.send(at, &[note_on_ump(0, 64, 90)]).unwrap();
    let arrived = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    // 예약 시각 전에 도착하면 CoreMIDI가 타임스탬프를 지키지 않은 것이다(SP-3).
    assert!(
        arrived.duration_since(sent) >= Duration::from_millis(18),
        "{:?}",
        arrived.duration_since(sent)
    );
}

#[test]
fn virtual_output_rename_updates_enumerated_name() {
    let client = MidiClient::new("ks-test-rename").unwrap();
    let before = unique("rename-a");
    let after = unique("rename-b");
    let mut port = client.create_virtual_output(&before).unwrap();
    assert!(wait_until(Duration::from_secs(2), || has_source(&before)));
    port.rename(&after).unwrap();
    assert_eq!(port.name(), after);
    assert!(wait_until(Duration::from_secs(2), || has_source(&after)
        && !has_source(&before)));
    // 옛 이름은 다시 쓸 수 있다.
    let again = client.create_virtual_output(&before).unwrap();
    assert_eq!(again.name(), before);
}

#[test]
fn create_virtual_output_seventeenth_fails_with_too_many_ports() {
    if run_isolated("create_virtual_output_seventeenth_fails_with_too_many_ports") {
        return;
    }
    let client = MidiClient::new("ks-test-max").unwrap();
    let mut ports: Vec<_> = (0..MAX_VIRTUAL_OUTPUTS)
        .map(|i| {
            client
                .create_virtual_output(&unique(&format!("max-{i}")))
                .unwrap()
        })
        .collect();
    assert_eq!(
        client.create_virtual_output(&unique("max-extra")).err(),
        Some(MidiError::TooManyPorts)
    );
    ports.pop();
    let extra = client.create_virtual_output(&unique("max-extra")).unwrap();
    assert!(wait_until(Duration::from_secs(2), || has_source(
        extra.name()
    )));
}

#[test]
fn create_virtual_output_duplicate_or_blank_name_fails() {
    let client = MidiClient::new("ks-test-names").unwrap();
    let name = unique("dup");
    let _first = client.create_virtual_output(&name).unwrap();
    assert_eq!(
        client.create_virtual_output(&name).err(),
        Some(MidiError::DuplicateName(name.clone()))
    );
    assert_eq!(
        client.create_virtual_output("").err(),
        Some(MidiError::InvalidName)
    );
    assert_eq!(
        client.create_virtual_output(" \t").err(),
        Some(MidiError::InvalidName)
    );

    let mut other = client.create_virtual_output(&unique("dup-other")).unwrap();
    assert_eq!(
        other.rename(&name),
        Err(MidiError::DuplicateName(name.clone()))
    );
    assert_eq!(other.rename("  "), Err(MidiError::InvalidName));
    // 실패한 이름 변경은 이름을 바꾸지 않는다.
    assert_eq!(other.name(), unique("dup-other"));
    // 자기 이름으로 바꾸는 것은 허용한다.
    other.rename(&unique("dup-other")).unwrap();
    // 다른 핸들도 같은 프로세스 목록을 쓰므로 같은 이름은 거부된다(결정 2026-10-15).
    let other_client = MidiClient::new("ks-test-names-2").unwrap();
    assert_eq!(
        other_client.create_virtual_output(&name).err(),
        Some(MidiError::DuplicateName(name.clone()))
    );
    assert_eq!(MidiClient::new(" ").err(), Some(MidiError::InvalidName));
}

#[test]
fn send_rejects_zero_timestamp_and_wrong_word_count() {
    let client = MidiClient::new("ks-test-send-check").unwrap();
    let port = client.create_virtual_output(&unique("send-check")).unwrap();
    let now = HostTime::now();
    assert_eq!(
        port.send(HostTime::from_ticks(0), &[note_on_ump(0, 60, 1)]),
        Err(MidiError::ZeroTimestamp)
    );
    assert_eq!(port.send(now, &[]), Err(MidiError::InvalidMessage));
    // 타입 2는 1워드인데 2워드를 줌.
    assert_eq!(
        port.send(now, &[note_on_ump(0, 60, 1), 0]),
        Err(MidiError::InvalidMessage)
    );
    // 타입 4(MIDI 2.0 채널 보이스)는 2워드인데 1워드를 줌.
    assert_eq!(
        port.send(now, &[0x4090_3c00]),
        Err(MidiError::InvalidMessage)
    );
    // 5워드는 어떤 타입에도 없다.
    assert_eq!(
        port.send(now, &[0x5000_0000, 0, 0, 0, 0]),
        Err(MidiError::InvalidMessage)
    );
    port.send(now, &[note_on_ump(0, 60, 1)]).unwrap();
}

const CHILD_ENV: &str = "KS_MIDI_TEST_CHILD_PORT";

/// 자식 프로세스 역할. 부모 테스트가 이 테스트만 골라 다시 실행한다.
#[test]
#[ignore = "killed_process_leaves_no_virtual_source가 자식 프로세스로 실행한다"]
fn child_holds_virtual_output_until_killed() {
    let Ok(name) = std::env::var(CHILD_ENV) else {
        return;
    };
    let client = MidiClient::new("ks-test-child").unwrap();
    let _port = client.create_virtual_output(&name).unwrap();
    // 부모 테스트가 외부에서 죽어 kill을 못 보내도 고아로 오래 남지 않게, 부모가 바뀌면(재부모화)
    // 곧 끝낸다(리뷰 1 제안 9). 최대 1분.
    let parent = std::os::unix::process::parent_id();
    let start = Instant::now();
    while std::os::unix::process::parent_id() == parent && start.elapsed() < Duration::from_mins(1)
    {
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn killed_process_leaves_no_virtual_source() {
    let name = unique("child");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_holds_virtual_output_until_killed",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_ENV, &name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let appeared = wait_until(Duration::from_secs(10), || has_source(&name));
    // SIGKILL: 프로세스가 정리 코드를 돌릴 기회가 없다(REQ 8.4 「비정상 종료해도 남지 않음」).
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(appeared, "자식의 가상 소스가 보이지 않았다");
    assert!(
        wait_until(Duration::from_secs(2), || !has_source(&name)),
        "kill 뒤에도 가상 소스가 남았다"
    );
}

#[test]
fn virtual_output_and_client_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    // 트레이트 객체로 스케줄러 스레드에 넘길 수 있어야 한다.
    fn assert_object(_: &dyn MidiPort) {}
    assert_send_sync::<MidiClient>();
    assert_send_sync::<keyscore_midi::VirtualOutput>();
    let client = MidiClient::new("ks-test-dyn").unwrap();
    let port = client.create_virtual_output(&unique("dyn")).unwrap();
    assert_object(&port);
}

/// 서버 재시작 뒤를 대비한 다시 등록(S1-T2 리뷰 1 지적 7). 실제 서버 재시작은 시스템 프로세스를
/// 종료해야 해서 시험하지 않고, 다시 등록한 뒤에도 이름·열거·송신이 그대로인지 본다.
#[test]
fn reregister_keeps_name_listing_and_delivery() {
    let client = MidiClient::new("ks-test-rereg").unwrap();
    let name = unique("rereg");
    let mut port = client.create_virtual_output(&name).unwrap();
    assert!(port.is_registered());
    port.reregister(&client).unwrap();
    assert!(port.is_registered());
    assert_eq!(port.name(), name);
    assert!(wait_until(Duration::from_secs(2), || {
        source_names().iter().filter(|n| **n == name).count() == 1
    }));
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let receiver = coremidi::Client::new("ks-test-rereg-rx").unwrap();
    let input = receiver
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
            }
        })
        .unwrap();
    input.connect_source(&find_source(&name)).unwrap();
    port.send(HostTime::now(), &[note_on_ump(1, 61, 90)])
        .unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        vec![0x91, 61, 90]
    );
    // 같은 이름 자리를 그대로 쓰므로 그 이름은 여전히 중복이다.
    assert_eq!(
        client.create_virtual_output(&name).err(),
        Some(MidiError::DuplicateName(name.clone()))
    );
}
