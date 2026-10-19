//! S1-T1 적대적 리뷰 3회차(`docs/reports/s1-t1/review-3.md`)의 테스트.
//!
//! 실패하는 테스트는 리뷰가 깨뜨린 사례이고, 통과하는 테스트는 깨뜨리지 못한 경계를 고정한다.
//! CoreMIDI 상태는 시스템 전역이라 포트 이름에 프로세스 id를 넣어 다른 테스트와 겹치지 않게 한다.
//! 16개 상한을 채우거나 정확히 세는 테스트는 자식 프로세스에서 돈다(결정
//! `2026-10-15-virtual-port-limit-scope.md`).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use keyscore_midi::{MAX_VIRTUAL_OUTPUTS, MidiClient, MidiError, MidiPort, source_names};

fn unique(name: &str) -> String {
    format!("KS review3 {} {name}", std::process::id())
}

/// 다른 리뷰 파일의 `run_isolated`와 같되, 자식이 테스트를 정확히 하나 돌렸는지도 본다. 이름이
/// 틀리거나 테스트가 모듈로 옮겨져 필터에 걸리는 것이 없으면 자식은 「0 passed」로 성공하고, 부모는
/// 아무것도 시험하지 않은 채 통과하기 때문이다(리뷰 3 제안 4).
fn run_isolated(test: &str) -> bool {
    const ENV: &str = "KS_MIDI_ISOLATED_TEST";
    if std::env::var(ENV).as_deref() == Ok(test) {
        return false;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(ENV, test)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{test}: 자식 프로세스에서 실패했다\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("test result: ok. 1 passed"),
        "{test}: 자식이 테스트를 하나도 돌리지 않았다\n{stdout}"
    );
    true
}

fn source_count(name: &str) -> usize {
    source_names().iter().filter(|n| *n == name).count()
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

/// 재현 가능한 의사 난수(xorshift64). 테스트 의존성을 늘리지 않으려고 직접 둔다.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % n as u64).unwrap()
    }
}

/// 경계. 핸들 8개를 가진 스레드 8개가 이름 24개를 두고 생성·이름 변경·drop을 무작위로 섞어 1.5초
/// 동안 다툰다. (가) 시스템에 보이는 이 테스트의 소스는 16개를 넘지 않고, (나) 오류는 `DuplicateName`
/// ·`TooManyPorts`뿐이며, (다) 끝난 뒤 목록에 찌꺼기가 없어 새 핸들로 다시 16개가 만들어진다.
#[test]
fn create_rename_drop_concurrent_across_handles_never_exceeds_max_and_frees_all_slots() {
    const THREADS: usize = 8;
    const POOL: usize = 24;
    const HELD_PER_THREAD: usize = 4;
    if run_isolated(
        "create_rename_drop_concurrent_across_handles_never_exceeds_max_and_frees_all_slots",
    ) {
        return;
    }
    let prefix = unique("mix");
    let pool: Arc<Vec<String>> = Arc::new((0..POOL).map(|i| format!("{prefix} {i}")).collect());
    let stop = Arc::new(AtomicBool::new(false));
    let too_many = Arc::new(AtomicUsize::new(0));
    let renamed = Arc::new(AtomicUsize::new(0));
    let start = Arc::new(Barrier::new(THREADS + 1));

    let workers: Vec<_> = (0..THREADS)
        .map(|t| {
            let (pool, stop, too_many, renamed, start) = (
                Arc::clone(&pool),
                Arc::clone(&stop),
                Arc::clone(&too_many),
                Arc::clone(&renamed),
                Arc::clone(&start),
            );
            std::thread::spawn(move || {
                let client = MidiClient::new(&format!("ks-review3-mix-{t}")).unwrap();
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (t as u64 + 1));
                let mut held = Vec::new();
                start.wait();
                while !stop.load(Ordering::SeqCst) {
                    match rng.below(3) {
                        0 if held.len() < HELD_PER_THREAD => {
                            match client.create_virtual_output(&pool[rng.below(POOL)]) {
                                Ok(port) => held.push(port),
                                Err(MidiError::TooManyPorts) => {
                                    too_many.fetch_add(1, Ordering::SeqCst);
                                }
                                Err(MidiError::DuplicateName(_)) => {}
                                Err(e) => panic!("생성: {e:?}"),
                            }
                        }
                        1 if !held.is_empty() => {
                            let i = rng.below(held.len());
                            let port: &mut keyscore_midi::VirtualOutput = &mut held[i];
                            match port.rename(&pool[rng.below(POOL)]) {
                                Ok(()) => {
                                    renamed.fetch_add(1, Ordering::SeqCst);
                                }
                                Err(MidiError::DuplicateName(_)) => {}
                                Err(e) => panic!("이름 변경: {e:?}"),
                            }
                        }
                        _ if !held.is_empty() => {
                            let i = rng.below(held.len());
                            drop(held.swap_remove(i));
                        }
                        _ => {}
                    }
                }
                held.len()
            })
        })
        .collect();

    start.wait();
    let begin = Instant::now();
    let mut worst = 0;
    while begin.elapsed() < Duration::from_millis(1500) {
        let visible = source_names()
            .iter()
            .filter(|n| n.starts_with(&prefix))
            .count();
        worst = worst.max(visible);
    }
    stop.store(true, Ordering::SeqCst);
    for w in workers {
        w.join().unwrap();
    }

    assert!(
        worst <= MAX_VIRTUAL_OUTPUTS,
        "이 프로세스의 소스가 동시에 {worst}개 보였다"
    );
    assert!(
        too_many.load(Ordering::SeqCst) > 0,
        "상한에 닿지 않아 경합을 시험하지 못했다"
    );
    assert!(renamed.load(Ordering::SeqCst) > 0);
    assert!(wait_until(Duration::from_secs(2), || {
        !source_names().iter().any(|n| n.starts_with(&prefix))
    }));

    let fresh = MidiClient::new("ks-review3-mix-after").unwrap();
    let ports: Vec<_> = pool
        .iter()
        .take(MAX_VIRTUAL_OUTPUTS)
        .map(|name| fresh.create_virtual_output(name).unwrap())
        .collect();
    assert_eq!(
        fresh.create_virtual_output(&pool[POOL - 1]).err(),
        Some(MidiError::TooManyPorts)
    );
    assert_eq!(ports.len(), MAX_VIRTUAL_OUTPUTS);
}

/// 경계. 서로 다른 핸들의 포트 8개가 동시에 같은 이름으로 바꾸면 정확히 하나만 성공하고, 시스템에도
/// 그 이름은 하나뿐이다. 20번 되풀이한다.
#[test]
fn rename_concurrent_to_same_name_across_handles_only_one_succeeds() {
    const THREADS: usize = 8;
    const ROUNDS: usize = 20;
    if run_isolated("rename_concurrent_to_same_name_across_handles_only_one_succeeds") {
        return;
    }
    for round in 0..ROUNDS {
        let target = unique(&format!("rename-target {round}"));
        let barrier = Arc::new(Barrier::new(THREADS));
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let (barrier, target) = (Arc::clone(&barrier), target.clone());
                std::thread::spawn(move || {
                    let client = MidiClient::new(&format!("ks-review3-ren-{t}")).unwrap();
                    let mut port = client
                        .create_virtual_output(&unique(&format!("ren {round} {t}")))
                        .unwrap();
                    barrier.wait();
                    let result = port.rename(&target);
                    // 모든 스레드가 결과를 낸 뒤에 포트를 놓아, 이긴 쪽이 먼저 drop해 이름이 풀리는
                    // 일을 막는다.
                    barrier.wait();
                    (result, port)
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let winners = results.iter().filter(|(r, _)| r.is_ok()).count();
        assert_eq!(winners, 1, "{round}회차: {winners}개가 같은 이름을 얻었다");
        for (r, port) in &results {
            match r {
                Ok(()) => assert_eq!(port.name(), target),
                Err(e) => assert_eq!(e, &MidiError::DuplicateName(target.clone())),
            }
        }
        assert_eq!(source_count(&target), 1, "{round}회차");
    }
}

/// 경계. 상한이 찬 상태에서 한 스레드가 포트를 drop하는 동안 다른 핸들의 스레드가 생성을 거듭하면,
/// 정확히 빈자리 하나만 다시 채워지고 그다음은 다시 `TooManyPorts`다.
#[test]
fn drop_on_one_thread_while_other_handle_creates_refills_exactly_one_slot() {
    if run_isolated("drop_on_one_thread_while_other_handle_creates_refills_exactly_one_slot") {
        return;
    }
    let first = MidiClient::new("ks-review3-refill-a").unwrap();
    let mut ports: Vec<_> = (0..MAX_VIRTUAL_OUTPUTS)
        .map(|i| {
            first
                .create_virtual_output(&unique(&format!("refill {i}")))
                .unwrap()
        })
        .collect();
    let victim = ports.pop().unwrap();
    let creator = std::thread::spawn(|| {
        let second = MidiClient::new("ks-review3-refill-b").unwrap();
        let mut made = Vec::new();
        let start = Instant::now();
        while made.is_empty() && start.elapsed() < Duration::from_secs(5) {
            match second.create_virtual_output(&unique("refill extra")) {
                Ok(port) => made.push(port),
                Err(MidiError::TooManyPorts) => std::thread::yield_now(),
                Err(e) => panic!("{e:?}"),
            }
        }
        let next = second.create_virtual_output(&unique("refill extra 2"));
        (made, next.err())
    });
    std::thread::sleep(Duration::from_millis(20));
    let dropper = std::thread::spawn(move || drop(victim));
    dropper.join().unwrap();
    let (made, next) = creator.join().unwrap();
    assert_eq!(made.len(), 1);
    assert_eq!(next, Some(MidiError::TooManyPorts));
}

/// 깨짐. 한글 이름은 NFC(`"드럼"`, 입력기)와 NFD(조합형 자모, Finder에서 복사한 파일 이름 등)로
/// 들어올 수 있다. 둘은 유니코드 정준 동치라 신디의 입력 목록에서 똑같이 보이는데, 정규화 없이
/// 바이트로 비교하므로 같은 프로세스에 둘 다 만들어진다. plan.md의 중복 금지 근거(「신디의 입력 목록에서
/// 구분되지 않는다」)에 어긋난다.
#[test]
fn create_virtual_output_nfd_variant_of_existing_name_fails_with_duplicate() {
    let client = MidiClient::new("ks-review3-nfd").unwrap();
    let nfc = unique("\u{B4DC}\u{B7FC}");
    let nfd = unique("\u{1103}\u{1173}\u{1105}\u{1165}\u{11B7}");
    let _first = client.create_virtual_output(&nfc).unwrap();
    let second = client.create_virtual_output(&nfd);
    let visible = source_names()
        .iter()
        .filter(|n| **n == nfc || **n == nfd)
        .count();
    assert!(
        matches!(second, Err(MidiError::DuplicateName(_))),
        "NFD 변형이 허용됐다(눈으로 같은 이름의 소스 {visible}개)"
    );
}

/// 깨짐. 정규화는 「보이지 않는 서식 문자」를 거부한다고 하지만 목록이 같은 부류(유니코드 Cf,
/// `Default_Ignorable_Code_Point`)의 일부만 담는다. LRM·RLM(U+200E/F)은 거부하면서 같은 양방향
/// 제어인 LRE~RLO(U+202A–202E), LRI~PDI(U+2066–2069), ALM(U+061C)은 받고, 결합 문자 결합자
/// (U+034F), 몽골 모음 분리자(U+180E)도 받는다. 그 결과 보이지 않는 문자만으로 된 이름이 만들어지고,
/// RLO로 글자 순서를 뒤집어 다른 이름처럼 보이게 할 수 있다.
#[test]
fn create_virtual_output_name_of_only_invisible_format_chars_fails_with_invalid_name() {
    let client = MidiClient::new("ks-review3-invisible").unwrap();
    let mut accepted = Vec::new();
    for name in [
        "\u{2066}\u{2069}",
        "\u{202A}\u{202C}",
        "\u{202E}",
        "\u{061C}",
        "\u{034F}",
        "\u{180E}",
    ] {
        if client.create_virtual_output(name).is_ok() {
            accepted.push(name.escape_unicode().to_string());
        }
    }
    assert!(
        accepted.is_empty(),
        "보이지 않는 이름이 만들어졌다: {accepted:?}"
    );
}

/// 경계. `CoreMIDI`는 NUL과 달리 긴 이름(1000자)을 자르지 않는다. 앞부분이 같은 긴 이름 둘이 시스템에서
/// 하나로 합쳐지지 않고, 포트 이름과 열거 이름이 같다.
#[test]
fn create_virtual_output_long_names_are_enumerated_verbatim() {
    let client = MidiClient::new("ks-review3-long").unwrap();
    let stem = unique(&"x".repeat(1000));
    let a = client.create_virtual_output(&format!("{stem}a")).unwrap();
    let b = client.create_virtual_output(&format!("{stem}b")).unwrap();
    assert_eq!(source_count(&a.name()), 1);
    assert_eq!(source_count(&b.name()), 1);
}

/// 경계. 리뷰 2 뒤 거부 목록의 양 끝(U+200B·U+200F·U+2060·U+2064·U+FEFF·U+00AD)과 제어 문자(C0·DEL·C1)는
/// 이름 가운데 있어도 생성과 이름 변경 모두 `InvalidName`이고, 실패한 이름 변경은 이름을 바꾸지 않는다.
#[test]
fn create_and_rename_with_listed_invisible_or_control_char_fail_with_invalid_name() {
    let client = MidiClient::new("ks-review3-listed").unwrap();
    let base = unique("listed");
    let mut port = client.create_virtual_output(&base).unwrap();
    for c in [
        '\u{200B}', '\u{200F}', '\u{2060}', '\u{2064}', '\u{FEFF}', '\u{00AD}', '\u{0000}',
        '\u{001F}', '\u{007F}', '\u{0085}', '\u{009F}',
    ] {
        let name = format!("{base}{c}x");
        assert_eq!(
            client.create_virtual_output(&name).err(),
            Some(MidiError::InvalidName),
            "{:?}",
            c.escape_unicode().to_string()
        );
        assert_eq!(port.rename(&name).err(), Some(MidiError::InvalidName));
        assert_eq!(port.name(), base);
    }
    assert_eq!(
        MidiClient::new("\u{200B}").err(),
        Some(MidiError::InvalidName)
    );
}
