//! S1-T1 적대적 리뷰 5회차(`docs/reports/s1-t1/review-5.md`)의 테스트.
//!
//! 실패하는 테스트는 리뷰가 깨뜨린 사례이고, 통과하는 테스트는 깨뜨리지 못한 경계를 고정한다.
//! CoreMIDI 상태는 시스템 전역이라 포트 이름에 프로세스 id를 넣어 다른 테스트와 겹치지 않게 한다.
//! 같은 바이너리의 테스트가 동시에 돌아도 프로세스 상한(16)에 닿지 않도록, 상한을 채우지 않는
//! 테스트는 한 번에 쥐는 포트를 둘 이하로 둔다.

use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use keyscore_midi::{MAX_VIRTUAL_OUTPUTS, MidiClient, MidiError, MidiPort, source_names};

fn unique(name: &str) -> String {
    format!("KS review5 {} {name}", std::process::id())
}

fn source_count(name: &str) -> usize {
    source_names().iter().filter(|n| *n == name).count()
}

/// 상한까지 포트를 채우는 테스트는 같은 바이너리의 다른 테스트와 상한을 나눠 쓰지 않게 자식
/// 프로세스에서 돈다. 부모에서는 자식을 띄워 성공 여부만 보고 `true`를, 자식 안에서는 `false`를
/// 돌려준다.
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
    assert!(
        out.status.success() && stdout.contains("test result: ok. 1 passed"),
        "{test}: 자식 프로세스에서 실패했거나 테스트를 찾지 못했다\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    true
}

/// `base`로 포트를 만든 채 `variant`로 하나 더 만들어 본다. 둘째가 만들어지면 그 이름을 돌려준다.
fn accepted_as_distinct(client: &MidiClient, base: &str, variant: &str) -> Option<String> {
    let _base = client.create_virtual_output(base).unwrap();
    match client.create_virtual_output(variant) {
        Ok(port) => Some(port.name().escape_debug().to_string()),
        Err(MidiError::DuplicateName(_)) => None,
        Err(e) => panic!("{}: 예상하지 못한 오류 {e:?}", variant.escape_unicode()),
    }
}

/// 깨짐. 공백 정리(앞뒤 떼기, 안쪽 한 칸)는 허용 예외 문자를 빼기 전에 하므로, 허용 예외 문자가
/// 공백을 감싸면 공백이 살아남는다. `"Lead Synth \u{FE0F}"`의 키는 끝 공백이 남은 `"Lead Synth "`,
/// `"Lead \u{200C} Synth"`의 키는 안쪽 두 칸인 `"Lead  Synth"`다. `AppKit`으로 그려 보면 앞의 것은
/// `"Lead Synth"`와, 뒤의 것은 `"Lead  Synth"`와 픽셀까지 같다. 결정(보이는 모양 기준)이 같은 이름으로
/// 본 경우(앞뒤 공백, 공백 개수)와 같은 모양이다.
#[test]
fn create_virtual_output_whitespace_shielded_by_allowed_ignorable_fails_with_duplicate() {
    let client = MidiClient::new("ks-review5-shield").unwrap();
    let base = unique("Lead Synth");
    let accepted: Vec<String> = [
        format!("{base} \u{FE0F}"),
        format!("{base} \u{200D}"),
        format!("\u{200D} {base}"),
        unique("Lead \u{200C} Synth"),
        unique("Lead\u{00A0}\u{E0100}\u{00A0}Synth"),
    ]
    .iter()
    .filter_map(|variant| accepted_as_distinct(&client, &base, variant))
    .collect();
    assert!(
        accepted.is_empty(),
        "\"Lead Synth\"와 같아 보이는 이름이 따로 만들어졌다: {accepted:?}"
    );
}

/// 깨짐. 중복 키는 NFC 이름에서 허용 예외 문자를 뺄 뿐 다시 NFC로 맞추지 않는다. 이체자 선택자는
/// 결합 등급이 0이라 NFC 합성을 막으므로, `"Cafe\u{FE00}\u{0301}"`은 NFC여도 키가 분해형
/// `"Cafe\u{0301}"`로 남아 `"Caf\u{E9}"`의 키와 다르다. `AppKit` 시스템 글꼴(13pt)로 그려 보면 둘 다
/// 「Café」이고 안티에일리어싱 몇 픽셀만 다르다. ZWJ를 끼우면 악센트가 떨어져 그려져 눈으로 구분되므로
/// 여기 넣지 않았다.
#[test]
fn create_virtual_output_ignorable_between_base_and_mark_fails_with_duplicate() {
    let client = MidiClient::new("ks-review5-nfc-key").unwrap();
    let base = unique("Caf\u{00E9}");
    let accepted: Vec<String> = [
        unique("Cafe\u{FE00}\u{0301}"),
        unique("Cafe\u{E0100}\u{0301}"),
    ]
    .iter()
    .filter_map(|variant| accepted_as_distinct(&client, &base, variant))
    .collect();
    assert!(
        accepted.is_empty(),
        "\"Café\"와 같아 보이는 이름이 따로 만들어졌다: {accepted:?}"
    );
}

/// 경계. 안쪽 공백 연속(NBSP, 한자 공백, 얇은 공백, 좁은 NBSP, 오검 공백)은 U+0020 한 칸으로
/// 저장되고, 포트 이름과 열거 이름이 같으며, 원래 모양 그대로의 소스는 보이지 않는다.
#[test]
fn create_virtual_output_inner_whitespace_run_is_stored_and_enumerated_as_one_space() {
    let client = MidiClient::new("ks-review5-collapse").unwrap();
    let expected = unique("Mono Lead");
    for raw in [
        "Mono\u{00A0}\u{3000}\u{2009}Lead",
        "Mono \u{202F}\u{1680} Lead",
        "\u{00A0}Mono\u{2007}Lead\u{3000}",
    ] {
        let raw = unique(raw);
        let port = client.create_virtual_output(&raw).unwrap();
        assert_eq!(port.name(), expected, "{}", raw.escape_unicode());
        assert_eq!(source_count(&expected), 1, "{}", raw.escape_unicode());
        assert_eq!(source_count(&raw), 0, "{}", raw.escape_unicode());
    }
}

/// 경계. 앞뒤 공백은 줄 구분자·탭·NEL까지 떼고 받지만, 안쪽의 탭·줄바꿈·NEL·줄/문단 구분자는
/// 공백으로 줄어들지 않고 `InvalidName`이다(거부 검사가 줄이기보다 먼저다).
#[test]
fn create_virtual_output_inner_control_or_separator_fails_while_outer_is_trimmed() {
    let client = MidiClient::new("ks-review5-order").unwrap();
    for inner in [
        '\t', '\n', '\r', '\u{000B}', '\u{000C}', '\u{0085}', '\u{2028}', '\u{2029}',
    ] {
        let name = unique(&format!("Poly{inner}Pad"));
        assert_eq!(
            client.create_virtual_output(&name).err(),
            Some(MidiError::InvalidName),
            "{}",
            name.escape_unicode()
        );
        let mut port = client.create_virtual_output(&unique("order")).unwrap();
        assert_eq!(port.rename(&name), Err(MidiError::InvalidName));
        assert_eq!(port.name(), unique("order"));
    }
    let expected = unique("Poly Pad");
    let port = client
        .create_virtual_output(&format!("\t\u{2028}{expected}\u{0085}\u{2029}\n"))
        .unwrap();
    assert_eq!(port.name(), expected);
    assert_eq!(source_count(&expected), 1);
}

/// 경계. 공백 종류만 다른 문자(NBSP, 한자 공백)와 허용 예외 문자만으로는 보이는 글자가 없어
/// `InvalidName`이다.
#[test]
fn create_virtual_output_only_space_variants_and_joiners_fails_with_invalid_name() {
    let client = MidiClient::new("ks-review5-blank").unwrap();
    for name in [
        "\u{00A0}",
        "\u{3000}\u{2009}",
        "\u{00A0}\u{200D}\u{00A0}",
        "\u{202F}\u{FE0F}\u{1680}",
        "\u{1D159}\u{00A0}\u{FFFB}",
    ] {
        assert_eq!(
            client.create_virtual_output(name).err(),
            Some(MidiError::InvalidName),
            "{}",
            name.escape_unicode()
        );
    }
}

/// 경계. 키가 같고 저장 이름이 다른 이름으로 바꾸기, 다른 포트의 키로 바꾸기, 바꾼 뒤 해제를
/// 섞어도 목록의 키가 실제 포트와 어긋나지 않는다. 해제는 자기 키만 지운다.
#[test]
fn rename_and_drop_with_joiner_variants_keep_keys_in_step_with_ports() {
    let client = MidiClient::new("ks-review5-rename").unwrap();
    let (pad, bass, keys) = (unique("PadSynth"), unique("Bass"), unique("Keys"));
    let pad_joined = unique("Pad\u{200D}Synth");
    let pad_tagged = unique("Pad\u{200C}Synth\u{FE0F}");
    let mut first = client.create_virtual_output(&pad).unwrap();
    let mut second = client.create_virtual_output(&bass).unwrap();

    // 자기 키의 다른 모양으로는 바꿀 수 있고, 열거에도 새 모양만 보인다.
    assert_eq!(first.rename(&pad_joined), Ok(()));
    assert_eq!(first.name(), pad_joined);
    assert_eq!((source_count(&pad_joined), source_count(&pad)), (1, 0));

    // 다른 포트는 그 키의 어떤 모양으로도 바꿀 수 없고, 실패 뒤 이름은 그대로다.
    for taken in [&pad, &pad_joined, &pad_tagged] {
        assert!(matches!(
            second.rename(taken),
            Err(MidiError::DuplicateName(_))
        ));
        assert_eq!(second.name(), bass);
    }
    assert!(matches!(
        client.create_virtual_output(&pad_tagged),
        Err(MidiError::DuplicateName(_))
    ));

    // first가 떠나면 그 키가 비고 first의 새 키가 잡힌다.
    assert_eq!(first.rename(&keys), Ok(()));
    assert_eq!(second.rename(&pad_tagged), Ok(()));
    assert!(matches!(
        client.create_virtual_output(&unique("Keys\u{200D}")),
        Err(MidiError::DuplicateName(_))
    ));

    // first를 해제하면 그 키(Keys)만 빈다. second의 키(PadSynth)는 남는다.
    drop(first);
    assert!(matches!(
        client.create_virtual_output(&pad),
        Err(MidiError::DuplicateName(_))
    ));
    let keys_again = client
        .create_virtual_output(&unique("Keys\u{200D}"))
        .unwrap();
    drop(second);
    let pad_again = client.create_virtual_output(&pad).unwrap();
    assert_eq!(source_count(&pad), 1);
    assert_eq!(source_count(&pad_tagged), 0);
    drop((keys_again, pad_again));
    for name in [&pad, &pad_joined, &pad_tagged, &bass, &keys] {
        assert_eq!(source_count(name), 0, "{}", name.escape_unicode());
    }
}

/// 경계. 상한까지 채운 상태에서도 자기 키의 다른 모양으로 이름을 바꿀 수 있고, 다른 포트 키의 다른
/// 모양은 상한보다 중복이 먼저 걸린다. 키를 바꾼 포트를 해제하면 정확히 한 자리가 빈다.
#[test]
fn rename_to_own_key_variant_at_limit_succeeds_and_drop_frees_exactly_one_slot() {
    if run_isolated("rename_to_own_key_variant_at_limit_succeeds_and_drop_frees_exactly_one_slot") {
        return;
    }
    let client = MidiClient::new("ks-review5-limit").unwrap();
    let mut ports: Vec<_> = (0..MAX_VIRTUAL_OUTPUTS)
        .map(|i| {
            client
                .create_virtual_output(&unique(&format!("slot {i}")))
                .unwrap()
        })
        .collect();
    assert_eq!(
        ports[0].rename(&unique("slot\u{00A0}\u{00A0}0\u{200D}")),
        Ok(())
    );
    assert_eq!(ports[0].name(), unique("slot 0\u{200D}"));
    assert!(matches!(
        client.create_virtual_output(&unique("slot\u{3000}1\u{FE0F}")),
        Err(MidiError::DuplicateName(_))
    ));
    assert_eq!(
        client.create_virtual_output(&unique("slot 99")).err(),
        Some(MidiError::TooManyPorts)
    );
    drop(ports.swap_remove(0));
    let refill = client.create_virtual_output(&unique("slot 0")).unwrap();
    assert_eq!(
        client.create_virtual_output(&unique("slot 99")).err(),
        Some(MidiError::TooManyPorts)
    );
    drop(refill);
}

/// 경계. 프로세스 클라이언트가 이미 있으면 `MidiClient::new`는 재시도 대기(10초)에 들어가지 않는다.
/// 여러 스레드가 한꺼번에 불러도 모두 곧바로 돌아온다.
#[test]
fn client_new_concurrent_calls_after_first_return_without_retry_wait() {
    const THREADS: usize = 32;
    MidiClient::new("ks-review5-warm").unwrap();
    let barrier = Arc::new(Barrier::new(THREADS));
    let start = Instant::now();
    let handles: Vec<_> = (0..THREADS)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                MidiClient::new(&format!("ks-review5-warm {i}")).map(|_| ())
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), Ok(()));
    }
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}
