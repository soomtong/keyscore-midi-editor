//! S1-T1 적대적 리뷰 1회차(`docs/reports/s1-t1/review-1.md`)의 테스트.
//!
//! 실패하는 테스트는 리뷰가 깨뜨린 사례이고, 통과하는 테스트는 깨뜨리지 못한 경계를 고정한다.
//! CoreMIDI 상태는 시스템 전역이라 포트 이름에 프로세스 id를 넣어 다른 테스트와 겹치지 않게 한다.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use keyscore_midi::{HostTime, MAX_VIRTUAL_OUTPUTS, MidiClient, MidiError, MidiPort, source_names};

fn unique(name: &str) -> String {
    format!("KS review {} {name}", std::process::id())
}

/// 상한까지 포트를 채우거나 정확한 개수를 세는 테스트는 프로세스 전역 상한을 같은 바이너리의 다른
/// 테스트와 나눠 쓰지 않게 자식 프로세스에서 돈다(결정 `2026-10-15-virtual-port-limit-scope.md`).
/// 부모에서는 자식을 띄워 성공 여부만 보고 `true`를, 자식 안에서는 `false`를 돌려준다.
fn run_isolated(test: &str) -> bool {
    const ENV: &str = "KS_MIDI_ISOLATED_TEST";
    if std::env::var(ENV).as_deref() == Ok(test) {
        return false;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(ENV, test)
        .status()
        .unwrap();
    assert!(status.success(), "{test}: 자식 프로세스에서 실패했다");
    true
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

fn source_count(name: &str) -> usize {
    source_names().iter().filter(|n| *n == name).count()
}

// 수신기(`coremidi` crate)는 쓰지 않는다. 그 crate의 `Client`는 drop해도 해제되지 않아
// (`MIDIClientDispose` 호출이 주석 처리돼 있다) MIDI 서버를 붙잡으므로, 서버 유휴 종료를 재현하는
// 아래 테스트를 무력화한다.

/// 테스트가 어디서 패닉하든 자식 프로세스를 죽이고 거둔다(고아 프로세스 방지).
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

const CHILD_ENV: &str = "KS_MIDI_REVIEW_CHILD";

/// 자식 프로세스 역할. 마지막 클라이언트를 drop하고 MIDI 서버가 유휴 종료할 시간을 준 뒤
/// 다시 클라이언트를 만든다. 다른 테스트의 클라이언트와 섞이지 않게 별도 프로세스에서 돈다.
#[test]
#[ignore = "client_new_after_last_client_dropped_and_server_idle_succeeds가 자식 프로세스로 실행한다"]
// 결정 2026-10-15로 `MidiClient`는 해제할 것이 없는 핸들이 되어 `drop`이 아무 일도 하지 않는다.
// 테스트의 뜻(핸들을 모두 놓은 뒤에도 동작)은 그대로라 lint만 끈다.
#[allow(clippy::drop_non_drop)]
fn child_recreates_client_after_server_idle() {
    if std::env::var(CHILD_ENV).is_err() {
        return;
    }
    let first = MidiClient::new("ks-review-idle-1").unwrap();
    let port = first.create_virtual_output(&unique("idle")).unwrap();
    drop(port);
    drop(first);
    // 시스템에 다른 MIDI 클라이언트가 없으면 MIDI 서버는 약 3초 뒤 종료한다(실측).
    // 같은 바이너리의 다른 테스트가 끝나 클라이언트를 모두 해제할 시간까지 넉넉히 기다린다.
    std::thread::sleep(Duration::from_secs(12));
    let again = MidiClient::new("ks-review-idle-2");
    assert!(again.is_ok(), "{:?}", again.err());
}

/// 깨짐. `ClientInner::drop`이 `MIDIClientDispose`를 부른다. Apple 문서는 마지막 클라이언트를
/// 해제하면 MIDI 서버가 종료할 수 있고, 그 뒤 그 프로세스의 `MIDIClientCreate`는 모두 실패한다고
/// 적는다. 실측: -2(또는 -304)가 30초 넘게 이어졌다. 시스템에 다른 MIDI 클라이언트(DAW 등)가
/// 떠 있으면 서버가 살아 있어 재현되지 않고 이 테스트는 통과한다.
#[test]
fn client_new_after_last_client_dropped_and_server_idle_succeeds() {
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_recreates_client_after_server_idle",
            "--ignored",
        ])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut guard = ChildGuard(child);
    let start = Instant::now();
    let status = loop {
        if let Some(status) = guard.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "자식이 30초 안에 끝나지 않았다"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        status.success(),
        "마지막 클라이언트 drop 뒤 MIDI 서버가 내려가면 MidiClient::new가 영구히 실패한다"
    );
}

/// 깨짐. 이름 중복 검사의 근거는 「신디 입력 목록에서 구분되지 않는다」(plan.md)인데,
/// 앞뒤 공백만 다른 이름은 목록에서 구분되지 않으면서 검사를 통과한다.
#[test]
fn create_virtual_output_name_differing_only_in_surrounding_whitespace_fails() {
    let client = MidiClient::new("ks-review-ws").unwrap();
    let name = unique("ws");
    let _first = client.create_virtual_output(&name).unwrap();
    assert!(
        client.create_virtual_output(&format!("{name} ")).is_err(),
        "뒤 공백만 다른 이름이 허용됐다"
    );
    assert!(
        client.create_virtual_output(&format!(" {name}")).is_err(),
        "앞 공백만 다른 이름이 허용됐다"
    );
}

/// 깨짐(unsafe 범위). `ClientInner`의 필드는 `u32`와 `Mutex<Vec<String>>`라 자동으로
/// `Send + Sync`다. `unsafe impl`은 필요 없고, 나중에 `!Send` 필드가 더해져도 컴파일러가 잡지
/// 못하게 만든다. 리뷰어가 사본에서 두 줄을 지우고 `virtual_output_and_client_are_send_and_sync`가
/// 그대로 통과함을 확인했다.
#[test]
fn client_source_send_sync_auto_traits_suffice_has_no_unsafe_impl() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/client.rs");
    let src = std::fs::read_to_string(path).unwrap();
    assert!(
        !src.contains("unsafe impl Send") && !src.contains("unsafe impl Sync"),
        "불필요한 unsafe impl Send/Sync가 있다"
    );
}

/// 경계. 여러 스레드가 한 클라이언트에 동시에 만들어도 정확히 16개만 성공하고, 이름이 겹치지 않는다.
#[test]
fn create_virtual_output_concurrent_threads_never_exceeds_max() {
    if run_isolated("create_virtual_output_concurrent_threads_never_exceeds_max") {
        return;
    }
    let client = MidiClient::new("ks-review-concurrent").unwrap();
    let handles: Vec<_> = (0..MAX_VIRTUAL_OUTPUTS * 2)
        .map(|i| {
            let client = client.clone();
            std::thread::spawn(move || client.create_virtual_output(&unique(&format!("conc-{i}"))))
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let ok = results.iter().filter(|r| r.is_ok()).count();
    let too_many = results
        .iter()
        .filter(|r| matches!(r, Err(MidiError::TooManyPorts)))
        .count();
    assert_eq!(ok, MAX_VIRTUAL_OUTPUTS);
    assert_eq!(too_many, MAX_VIRTUAL_OUTPUTS);
}

/// 경계(「리뷰어에게」 2). drop은 이름 목록 잠금을 `MIDIEndpointDispose`가 끝날 때까지 쥔다.
/// 두 스레드가 같은 이름을 만들고 지우기를 반복해도 시스템에 그 이름의 소스는 둘이 되지 않는다.
#[test]
fn create_and_drop_same_name_concurrently_never_shows_two_sources() {
    let client = MidiClient::new("ks-review-race").unwrap();
    let name = unique("race");
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let client = client.clone();
            let name = name.clone();
            std::thread::spawn(move || {
                let (mut ok, mut worst) = (0, 0);
                let start = Instant::now();
                while ok < 20 && start.elapsed() < Duration::from_secs(10) {
                    match client.create_virtual_output(&name) {
                        Ok(port) => {
                            ok += 1;
                            worst = worst.max(source_count(&name));
                            drop(port);
                        }
                        Err(MidiError::DuplicateName(_)) => std::thread::yield_now(),
                        Err(e) => panic!("{e:?}"),
                    }
                }
                (ok, worst)
            })
        })
        .collect();
    for h in handles {
        let (ok, worst) = h.join().unwrap();
        assert!(ok > 0);
        assert!(worst <= 1, "같은 이름의 소스가 {worst}개 보였다");
    }
    assert!(wait_until(Duration::from_secs(2), || source_count(&name) == 0));
}

/// 경계(RAII). 모든 `MidiClient` 복제본을 drop해도 포트가 클라이언트를 붙잡아 소스가 남고
/// 송신이 된다.
#[test]
// 결정 2026-10-15로 `MidiClient`는 해제할 것이 없는 핸들이 되어 `drop`이 아무 일도 하지 않는다.
// 테스트의 뜻(핸들을 모두 놓은 뒤에도 동작)은 그대로라 lint만 끈다.
#[allow(clippy::drop_non_drop)]
fn send_after_all_client_handles_dropped_succeeds() {
    let client = MidiClient::new("ks-review-outlive").unwrap();
    let copy = client.clone();
    let name = unique("outlive");
    let port = copy.create_virtual_output(&name).unwrap();
    drop(client);
    drop(copy);
    assert!(wait_until(Duration::from_secs(2), || source_count(&name) == 1));
    assert_eq!(port.send(HostTime::now(), &[0x2093_3c40]), Ok(()));
    drop(port);
    assert!(wait_until(Duration::from_secs(2), || source_count(&name) == 0));
}

/// 경계. 이름을 바꾼 뒤 drop하면 옛 이름과 새 이름이 모두 풀린다(목록에 찌꺼기가 없다).
#[test]
fn drop_after_rename_frees_both_names() {
    let client = MidiClient::new("ks-review-rename-drop").unwrap();
    let (a, b) = (unique("rd-a"), unique("rd-b"));
    let mut port = client.create_virtual_output(&a).unwrap();
    port.rename(&b).unwrap();
    drop(port);
    let again_a = client.create_virtual_output(&a).unwrap();
    let again_b = client.create_virtual_output(&b).unwrap();
    assert_eq!((again_a.name(), again_b.name()), (a.as_str(), b.as_str()));
}

/// 경계(「리뷰어에게」 1). 4워드 메시지도 스택 `MIDIEventList`에 들어간다. 1.0 소스에서 타입 0xD는
/// 전달되지 않지만 `MIDIEventListAdd`가 NULL을 돌려주지 않는다(`InvalidMessage`가 아님).
#[test]
fn send_four_word_message_fits_stack_event_list() {
    let client = MidiClient::new("ks-review-4w").unwrap();
    let port = client.create_virtual_output(&unique("4w")).unwrap();
    assert_eq!(port.send(HostTime::now(), &[0xD000_0000, 0, 0, 0]), Ok(()));
}

/// 깨짐. `source_names`는 개수를 읽은 뒤 인덱스로 하나씩 꺼낸다. 열거 중에 앞쪽 소스가 사라지면
/// 인덱스가 당겨져, 열거 내내 살아 있던 소스를 건너뛴다. S1-T2가 이 목록으로 소멸을 판정하면
/// 살아 있는 포트를 「사라짐」으로 오판한다. 기존 테스트의 간헐 실패(`Source::from_name`이 None)도
/// 같은 원인이다.
#[test]
fn source_names_while_earlier_source_disposed_lists_live_source() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    let client = MidiClient::new("ks-review-churn").unwrap();
    let live = unique("churn-live");
    // 홀수: `live`가 살아 있는 구간. 짝수: 만들기 전이거나 지우는 중.
    let generation = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let churn = {
        let (client, live) = (client.clone(), live.clone());
        let (generation, stop) = (Arc::clone(&generation), Arc::clone(&stop));
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let earlier = client
                    .create_virtual_output(&unique("churn-earlier"))
                    .unwrap();
                let port = client.create_virtual_output(&live).unwrap();
                generation.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(1));
                // 앞쪽 소스를 지워 `live`의 인덱스를 당긴다.
                drop(earlier);
                std::thread::sleep(Duration::from_millis(1));
                generation.fetch_add(1, Ordering::SeqCst);
                drop(port);
            }
        })
    };
    let start = Instant::now();
    let (mut checked, mut misses) = (0, 0);
    while start.elapsed() < Duration::from_secs(3) {
        let before = generation.load(Ordering::SeqCst);
        if before.is_multiple_of(2) {
            continue;
        }
        let names = source_names();
        if generation.load(Ordering::SeqCst) == before {
            checked += 1;
            if !names.contains(&live) {
                misses += 1;
            }
        }
    }
    stop.store(true, Ordering::SeqCst);
    churn.join().unwrap();
    assert!(checked > 0);
    assert_eq!(
        misses, 0,
        "열거 {checked}회 가운데 {misses}회 살아 있는 소스가 빠졌다"
    );
}
