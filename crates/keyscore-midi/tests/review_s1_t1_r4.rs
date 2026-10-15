//! S1-T1 적대적 리뷰 4회차(`docs/reports/s1-t1/review-4.md`)의 테스트.
//!
//! 실패하는 테스트는 리뷰가 깨뜨린 사례이고, 통과하는 테스트는 깨뜨리지 못한 경계를 고정한다.
//! CoreMIDI 상태는 시스템 전역이라 포트 이름에 프로세스 id를 넣어 다른 테스트와 겹치지 않게 한다.
//! 같은 바이너리의 테스트가 동시에 돌아도 프로세스 상한(16)에 닿지 않도록, 테스트마다 한 번에 쥐는
//! 포트를 둘 이하로 둔다.

use keyscore_midi::{MidiClient, MidiError, MidiPort, source_names};

fn unique(name: &str) -> String {
    format!("KS review4 {} {name}", std::process::id())
}

fn source_count(name: &str) -> usize {
    source_names().iter().filter(|n| *n == name).count()
}

/// `base`로 포트를 만든 채 `variant`로 하나 더 만들어 본다. 둘째가 만들어지면 그 이름을 돌려준다.
fn accepted_as_distinct(client: &MidiClient, base: &str, variant: &str) -> Option<String> {
    let _base = client.create_virtual_output(&unique(base)).unwrap();
    match client.create_virtual_output(&unique(variant)) {
        Ok(port) => Some(port.name().escape_unicode().to_string()),
        Err(MidiError::DuplicateName(_)) => None,
        Err(e) => panic!("{}: 예상하지 못한 오류 {e:?}", variant.escape_unicode()),
    }
}

/// 깨짐. 공백의 종류나 개수만 다른 이름이 서로 다른 포트로 만들어진다. NBSP(U+00A0)는 macOS에서
/// Option+Space로 잘못 눌러 흔히 들어오고, 신디의 입력 목록에서 보통 공백과 똑같이 그려진다.
/// 공백 두 칸도 비례 글꼴의 목록에서 한 칸과 거의 구분되지 않는다. 앞뒤 공백을 떼는 규칙(리뷰 1
/// 지적 4)과 같은 이유다.
#[test]
fn create_virtual_output_whitespace_variant_of_existing_name_fails_with_duplicate() {
    let client = MidiClient::new("ks-review4-space").unwrap();
    let accepted: Vec<String> = ["Lead\u{00A0}Synth", "Lead  Synth"]
        .into_iter()
        .filter_map(|variant| accepted_as_distinct(&client, "Lead Synth", variant))
        .collect();
    assert!(
        accepted.is_empty(),
        "\"Lead Synth\"와 구분되지 않는 이름이 따로 만들어졌다: {accepted:?}"
    );
}

/// 깨짐. 리뷰 3 대응으로 받기로 한 표시되지 않는 문자(ZWJ, ZWNJ, 이체자 선택자, 태그 문자)는 정상
/// 텍스트에서만 뜻이 있다. 라틴 글자 사이나 뒤에 붙으면 아무것도 그리지 않으므로, 그것만 다른 이름이
/// 눈으로 같은 소스 둘이 된다. 중복 판정에서는 이 문자들을 빼고 비교해야 한다.
#[test]
fn create_virtual_output_name_differing_only_by_allowed_ignorable_fails_with_duplicate() {
    let client = MidiClient::new("ks-review4-ignorable").unwrap();
    let accepted: Vec<String> = [
        ("PadSynth", "Pad\u{200D}Synth"),
        ("PadSynth", "Pad\u{200C}Synth"),
        ("Pad Synth", "\u{200C}Pad Synth"),
        ("Pad Synth", "Pad Synth\u{FE0F}"),
        ("Pad Synth", "Pad Synth\u{E0100}"),
        ("Pad Synth", "Pad Synth\u{E0041}\u{E007F}"),
    ]
    .into_iter()
    .filter_map(|(base, variant)| accepted_as_distinct(&client, base, variant))
    .collect();
    assert!(
        accepted.is_empty(),
        "허용 예외 문자만 다른 이름이 따로 만들어졌다: {accepted:?}"
    );
}

/// 경계. 한글 외의 정준 동치(옴 기호와 그리스 오메가, 옹스트롬 기호와 Å, 결합 문자 순서)도 같은
/// 이름으로 판정되고, 포트 이름과 열거 이름은 NFC로 같다.
#[test]
fn create_virtual_output_canonical_equivalents_fail_with_duplicate_and_enumerate_nfc() {
    let client = MidiClient::new("ks-review4-canonical").unwrap();
    for (first, second, nfc) in [
        ("\u{2126}", "\u{03A9}", "\u{03A9}"),
        ("\u{212B}", "A\u{030A}", "\u{00C5}"),
        ("e\u{0301}\u{0327}", "e\u{0327}\u{0301}", "\u{0229}\u{0301}"),
    ] {
        let port = client.create_virtual_output(&unique(first)).unwrap();
        assert_eq!(port.name(), unique(nfc));
        assert_eq!(source_count(&unique(nfc)), 1);
        assert_eq!(source_count(&unique(first)), usize::from(first == nfc));
        assert!(
            matches!(
                client.create_virtual_output(&unique(second)),
                Err(MidiError::DuplicateName(_))
            ),
            "{}",
            second.escape_unicode()
        );
    }
}

/// 경계. 자기 이름의 정준 동치로 바꾸면 아무것도 하지 않고, 다른 포트 이름의 정준 동치로 바꾸면
/// `DuplicateName`이며, 실패한 rename은 이름을 바꾸지 않는다.
#[test]
fn rename_to_canonical_variant_is_noop_for_self_and_duplicate_for_other() {
    let client = MidiClient::new("ks-review4-rename").unwrap();
    let nfc = unique("\u{B4DC}\u{B7FC}");
    let nfd = unique("\u{1103}\u{1173}\u{1105}\u{1165}\u{11B7}");
    let mut drum = client.create_virtual_output(&nfc).unwrap();
    assert_eq!(drum.rename(&nfd), Ok(()));
    assert_eq!(drum.name(), nfc);
    assert_eq!(source_count(&nfc), 1);

    let mut other = client.create_virtual_output(&unique("other")).unwrap();
    assert!(matches!(
        other.rename(&nfd),
        Err(MidiError::DuplicateName(_))
    ));
    assert_eq!(other.name(), unique("other"));
    assert_eq!(other.rename("\u{202E}"), Err(MidiError::InvalidName));
    assert_eq!(other.name(), unique("other"));
    assert_eq!(source_count(&unique("other")), 1);
}

/// 경계. 허용 예외 문자와 공백·점자 빈칸만으로 된 이름은 보이는 글자가 없어 거부된다. 태그 문자
/// 범위 밖(U+E0000, U+E0001, U+E0080)은 허용 예외가 아니다.
#[test]
fn create_virtual_output_name_without_visible_char_fails_with_invalid_name() {
    let client = MidiClient::new("ks-review4-blank").unwrap();
    for name in [
        "\u{200D} \u{FE0F}",
        "\u{200C}\u{200D}",
        "\u{E0041}\u{E0042}\u{E007F}",
        "\u{E0100}",
        "\u{2800}\u{200D}",
        "\u{FE0F}\u{2800}",
        "\u{3164}\u{3164}",
        "a\u{E0000}",
        "a\u{E0001}",
        "a\u{E0080}",
    ] {
        assert_eq!(
            client.create_virtual_output(name).err(),
            Some(MidiError::InvalidName),
            "{}",
            name.escape_unicode()
        );
    }
}

/// 경계. 유니코드 16.0 `DerivedCoreProperties.txt`의 `Default_Ignorable_Code_Point` 전체에서 허용
/// 예외(U+200C, U+200D, U+FE00–FE0F, U+E0020–E007F, U+E0100–E01EF)를 뺀 모든 코드 포인트는 이름
/// 가운데 있어도 `InvalidName`이다. 목록은 구현과 따로 옮겨 적었다.
#[test]
fn create_virtual_output_every_rejected_default_ignorable_fails_with_invalid_name() {
    const DEFAULT_IGNORABLE: &[(u32, u32)] = &[
        (0x00AD, 0x00AD),
        (0x034F, 0x034F),
        (0x061C, 0x061C),
        (0x115F, 0x1160),
        (0x17B4, 0x17B5),
        (0x180B, 0x180F),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x206F),
        (0x3164, 0x3164),
        (0xFE00, 0xFE0F),
        (0xFEFF, 0xFEFF),
        (0xFFA0, 0xFFA0),
        (0xFFF0, 0xFFF8),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0000, 0xE0FFF),
    ];
    let allowed = |c: u32| matches!(c, 0x200C | 0x200D | 0xFE00..=0xFE0F | 0xE0020..=0xE007F | 0xE0100..=0xE01EF);
    let client = MidiClient::new("ks-review4-ignorable-list").unwrap();
    let mut accepted = Vec::new();
    for &(lo, hi) in DEFAULT_IGNORABLE {
        for c in (lo..=hi)
            .filter(|&c| !allowed(c))
            .filter_map(char::from_u32)
        {
            if client.create_virtual_output(&format!("a{c}b")).is_ok() {
                accepted.push(format!("U+{:04X}", u32::from(c)));
            }
        }
    }
    assert!(accepted.is_empty(), "거부되지 않았다: {accepted:?}");
}

/// 경계. 허용 예외가 뜻을 갖는 정상 텍스트(잉글랜드 깃발 태그 연쇄, ZWJ 이모지 가족, 페르시아어
/// ZWNJ)는 만들어지고, 포트 이름 그대로 열거된다.
#[test]
fn create_virtual_output_real_text_with_joiners_and_tags_is_enumerated_verbatim() {
    let client = MidiClient::new("ks-review4-real-text").unwrap();
    for text in [
        "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\u{0627}\u{0647}\u{0645}",
    ] {
        let name = unique(text);
        let port = client.create_virtual_output(&name).unwrap();
        assert_eq!(port.name(), name);
        assert_eq!(source_count(&name), 1, "{}", text.escape_unicode());
    }
}
