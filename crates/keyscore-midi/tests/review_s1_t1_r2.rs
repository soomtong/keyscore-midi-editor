//! S1-T1 적대적 리뷰 2회차(`docs/reports/s1-t1/review-2.md`)의 테스트.
//!
//! 실패하는 테스트는 리뷰가 깨뜨린 사례이고, 통과하는 테스트는 깨뜨리지 못한 경계를 고정한다.
//! CoreMIDI 상태는 시스템 전역이라 포트 이름에 프로세스 id를 넣어 다른 테스트와 겹치지 않게 한다.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Duration;

use keyscore_midi::{MAX_VIRTUAL_OUTPUTS, MidiClient, MidiError, MidiPort, source_names};

fn unique(name: &str) -> String {
    format!("KS review2 {} {name}", std::process::id())
}

fn source_count(name: &str) -> usize {
    source_names().iter().filter(|n| *n == name).count()
}

/// 깨짐. 리뷰 1 뒤 CoreMIDI 클라이언트는 프로세스에 하나다. 그런데 이름 중복 검사는 `MidiClient`
/// 핸들 단위라, 핸들을 하나 더 만들면 같은 앱·같은 CoreMIDI 클라이언트에서 같은 이름의 소스가 둘
/// 생긴다. 신디의 입력 목록에서 둘은 구분되지 않는다(plan.md가 든 중복 금지의 근거).
#[test]
fn create_virtual_output_same_name_on_second_handle_fails() {
    let first = MidiClient::new("ks-review2-dup-a").unwrap();
    let second = MidiClient::new("ks-review2-dup-b").unwrap();
    let name = unique("dup-across-handles");
    let _a = first.create_virtual_output(&name).unwrap();
    let b = second.create_virtual_output(&name);
    let visible = source_count(&name);
    assert!(
        matches!(b, Err(MidiError::DuplicateName(_))),
        "두 번째 핸들에서 같은 이름이 허용됐다(시스템에 보이는 같은 이름 소스 {visible}개)"
    );
}

/// 깨짐. OUT-2의 「최대 16」은 앱 전체 수다. 프로세스 클라이언트 하나에 핸들이 둘이면 17번째가
/// 만들어진다. 다른 테스트가 동시에 포트를 쥐고 있어도 판정이 같도록, 첫 핸들은 상한에 닿을 때까지만
/// 만든다.
#[test]
fn create_virtual_output_seventeenth_across_two_handles_fails_with_too_many_ports() {
    let first = MidiClient::new("ks-review2-max-a").unwrap();
    let second = MidiClient::new("ks-review2-max-b").unwrap();
    let mut ports = Vec::new();
    for i in 0..MAX_VIRTUAL_OUTPUTS {
        match first.create_virtual_output(&unique(&format!("max {i}"))) {
            Ok(port) => ports.push(port),
            Err(MidiError::TooManyPorts) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
    let extra = second.create_virtual_output(&unique("max extra"));
    assert_eq!(
        extra.err(),
        Some(MidiError::TooManyPorts),
        "핸들 둘로 앱 전체 {}개를 만들었다",
        ports.len() + 1
    );
}

/// 깨짐. `CoreMIDI`는 이름을 NUL 앞에서 자른다(실측: `"X\0tail"`이 `"X"`로 열거된다). 정규화는
/// 앞뒤 공백만 떼므로, 같은 핸들에서 `"X"`와 `"X\0tail"`이 둘 다 통과해 시스템에 같은 이름 소스가
/// 둘 생긴다.
#[test]
fn create_virtual_output_name_with_nul_never_yields_two_identical_sources() {
    let client = MidiClient::new("ks-review2-nul-dup").unwrap();
    let name = unique("nul-dup");
    let _first = client.create_virtual_output(&name).unwrap();
    let _second = client.create_virtual_output(&format!("{name}\0tail"));
    assert_eq!(
        source_count(&name),
        1,
        "NUL 뒤만 다른 이름으로 같은 이름 소스가 둘 생겼다"
    );
}

/// 깨짐. 포트가 스스로 아는 이름(`MidiPort::name`)과 시스템이 보여 주는 이름이 달라진다. S1-T2의
/// 이름 기반 재연결과 프로젝트 저장은 이 이름을 쓴다.
#[test]
fn virtual_output_name_with_nul_matches_enumerated_name() {
    let client = MidiClient::new("ks-review2-nul-name").unwrap();
    let name = format!("{}\0tail", unique("nul-name"));
    if let Ok(port) = client.create_virtual_output(&name) {
        assert!(
            source_names().iter().any(|n| n == port.name()),
            "포트 이름 {:?}이 열거에 없다",
            port.name()
        );
    }
}

/// 경계. 여러 스레드가 동시에 처음 `MidiClient::new`를 불러도 모두 성공하고, 각 핸들로 만든
/// 포트가 모두 보인다(프로세스 클라이언트 초기화 경합).
#[test]
fn client_new_concurrent_first_calls_all_succeed_and_create_ports() {
    const THREADS: usize = 16;
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let client = MidiClient::new(&format!("ks-review2-race-{i}")).unwrap();
                let name = unique(&format!("race {i}"));
                let port = client.create_virtual_output(&name).unwrap();
                (name, port)
            })
        })
        .collect();
    let ports: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let names = source_names();
    for (name, _) in &ports {
        assert!(names.contains(name), "{name}이 보이지 않는다");
    }
}

/// 경계. `str::trim`은 유니코드 공백(줄바꿈, 탭, NBSP, 전각 공백)도 떼므로 그런 변형도 중복이다.
#[test]
fn create_virtual_output_name_padded_with_unicode_whitespace_fails_with_duplicate() {
    let client = MidiClient::new("ks-review2-uws").unwrap();
    let name = unique("uws");
    let _first = client.create_virtual_output(&name).unwrap();
    for padded in [
        format!("\t{name}\n"),
        format!("\u{00A0}{name}"),
        format!("{name}\u{3000}"),
        format!("\u{2003}{name}\u{2028}"),
    ] {
        assert_eq!(
            client.create_virtual_output(&padded).err(),
            Some(MidiError::DuplicateName(name.clone())),
            "{padded:?}"
        );
    }
    assert_eq!(
        client.create_virtual_output("\u{00A0}\u{3000}\t").err(),
        Some(MidiError::InvalidName)
    );
}

/// 경계. 앞뒤 공백이 붙은 이름으로 만들면 시스템에는 뗀 이름으로 보이고, 포트 이름도 같다.
#[test]
fn create_virtual_output_padded_name_is_enumerated_trimmed() {
    let client = MidiClient::new("ks-review2-trim-enum").unwrap();
    let name = unique("trim-enum");
    let port = client.create_virtual_output(&format!("  {name} ")).unwrap();
    assert_eq!(port.name(), name);
    assert_eq!(source_count(&name), 1);
    assert_eq!(source_count(&format!("  {name} ")), 0);
}

/// 경계. rename도 같은 정규화를 거친다. 자기 이름에 공백만 붙이면 그대로, 다른 포트 이름에 공백을
/// 붙이면 중복, 공백뿐이면 무효. 공백 붙은 새 이름으로 바꾼 뒤 drop하면 뗀 이름이 다시 쓰인다.
#[test]
fn rename_with_padded_names_follows_trimmed_rules() {
    let client = MidiClient::new("ks-review2-rename").unwrap();
    let (a_name, b_name, c_name) = (unique("ren a"), unique("ren b"), unique("ren c"));
    let mut a = client.create_virtual_output(&a_name).unwrap();
    let _b = client.create_virtual_output(&b_name).unwrap();

    a.rename(&format!(" {a_name}\t")).unwrap();
    assert_eq!(a.name(), a_name);
    assert_eq!(
        a.rename(&format!("{b_name} ")).err(),
        Some(MidiError::DuplicateName(b_name.clone()))
    );
    assert_eq!(a.rename(" \n").err(), Some(MidiError::InvalidName));
    assert_eq!(a.name(), a_name);

    a.rename(&format!(" {c_name} ")).unwrap();
    assert_eq!(a.name(), c_name);
    assert!(
        client.create_virtual_output(&a_name).is_ok(),
        "옛 이름이 풀리지 않았다"
    );
    drop(a);
    assert!(
        client.create_virtual_output(&c_name).is_ok(),
        "drop 뒤 새 이름이 풀리지 않았다"
    );
}

/// 경계. 리뷰 1 시도 4보다 거칠게 흔든다. 고정 소스보다 앞에 소스 240개를 두고, 네 스레드가 그것을
/// 0.2 ms 간격으로 지워 고정 소스의 인덱스를 계속 당기는 동안에도 고정 소스가 열거에서 빠지지 않는다.
/// 리뷰어 계측(사본)에서는 이 조건에서 8회 재시도가 바닥나는 호출이 실행마다 나왔지만, 그 결과에서도
/// 고정 소스는 빠지지 않았다. 수정 전 코드에서는 실행마다 1–4회 빠졌다. 핸들 단위 상한(16)을 피하려고
/// 핸들을 여럿 쓴다.
#[test]
fn source_names_under_heavy_churn_never_misses_steady_source() {
    const CHURNERS: usize = 4;
    const PER_HANDLE: usize = 15;
    const HANDLES_PER_CHURNER: usize = 4;
    let mut pools: Vec<Vec<keyscore_midi::VirtualOutput>> = Vec::new();
    for c in 0..CHURNERS {
        let mut pool = Vec::new();
        for h in 0..HANDLES_PER_CHURNER {
            let client = MidiClient::new("ks-review2-churn").unwrap();
            for i in 0..PER_HANDLE {
                pool.push(
                    client
                        .create_virtual_output(&unique(&format!("churn {c} {h} {i}")))
                        .unwrap(),
                );
            }
        }
        pools.push(pool);
    }
    let steady_name = unique("steady");
    let steady_client = MidiClient::new("ks-review2-steady").unwrap();
    let _steady = steady_client.create_virtual_output(&steady_name).unwrap();
    assert_eq!(source_count(&steady_name), 1);

    let go = Arc::new(Barrier::new(CHURNERS + 1));
    let remaining = Arc::new(AtomicU64::new(0));
    let churners: Vec<_> = pools
        .into_iter()
        .map(|mut pool| {
            let (go, remaining) = (Arc::clone(&go), Arc::clone(&remaining));
            remaining.fetch_add(pool.len() as u64, Ordering::SeqCst);
            std::thread::spawn(move || {
                go.wait();
                // 앞에서부터 지워 고정 소스의 인덱스를 당긴다.
                while !pool.is_empty() {
                    drop(pool.remove(0));
                    remaining.fetch_sub(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_micros(200));
                }
            })
        })
        .collect();
    go.wait();
    let (mut checked, mut misses) = (0u32, 0u32);
    while remaining.load(Ordering::SeqCst) > 0 {
        checked += 1;
        if !source_names().contains(&steady_name) {
            misses += 1;
        }
    }
    for c in churners {
        c.join().unwrap();
    }
    assert!(checked > 0);
    assert_eq!(
        misses, 0,
        "열거 {checked}회 가운데 {misses}회 살아 있는 소스가 빠졌다"
    );
}
