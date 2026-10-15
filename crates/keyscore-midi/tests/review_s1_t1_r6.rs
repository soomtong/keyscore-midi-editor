//! S1-T1 적대적 리뷰 6회차(`docs/reports/s1-t1/review-6.md`)의 테스트.
//!
//! 대상은 중복 키 생성 순서 수정(`midi: Build the duplicate key after removing invisible
//! characters`) 하나다. 실패하는 테스트는 리뷰가 깨뜨린 사례이고, 통과하는 테스트는 깨뜨리지 못한
//! 경계를 고정한다. CoreMIDI 상태는 시스템 전역이라 포트 이름에 프로세스 id를 넣는다. 같은 바이너리의
//! 테스트가 동시에 돌아도 프로세스 상한(16)에 닿지 않도록 한 번에 쥐는 포트를 둘 이하로 둔다.

use keyscore_midi::{MidiClient, MidiError, MidiPort};

fn unique(name: &str) -> String {
    format!("KS review6 {} {name}", std::process::id())
}

/// `base`로 포트를 만든 채 `variant`로 하나 더 만들어 본다. 둘째가 만들어지면 `true`.
fn accepted_as_distinct(client: &MidiClient, base: &str, variant: &str) -> bool {
    let _base = client.create_virtual_output(base).unwrap();
    match client.create_virtual_output(variant) {
        Ok(_) => true,
        Err(MidiError::DuplicateName(_)) => false,
        Err(e) => panic!("{}: 예상하지 못한 오류 {e:?}", variant.escape_unicode()),
    }
}

/// 깨짐(과잉 병합). 키가 표시되지 않는 문자를 뺀 뒤 NFC로 다시 합치므로, ZWJ·ZWNJ·점자 빈칸이
/// 갈라 놓아 따로 그려지는 글자가 키에서는 한 글자로 합쳐진다. `AppKit` 시스템 글꼴로 그리면
/// `"Cafe\u{200D}\u{301}"`은 악센트가 떨어진 「Cafe´」, `"\u{1100}\u{200D}\u{1161}"`은 낱자 「ㄱㅏ」,
/// `"\u{AC00}\u{200D}\u{11A8}"`은 「가ㄱ」이다(너비도 두 배). 보이는 모양이 다른데 `DuplicateName`이다.
/// 이 커밋 전에는 따로 만들어졌다.
#[test]
fn create_virtual_output_joiner_or_blank_splitting_composition_succeeds_as_distinct_name() {
    let client = MidiClient::new("ks-review6-split").unwrap();
    let cases = [
        ("Caf\u{00E9}", "Cafe\u{200D}\u{0301}"),
        ("Caf\u{00E9}", "Cafe\u{200C}\u{0301}"),
        ("Caf\u{00E9}", "Cafe\u{2800}\u{0301}"),
        ("\u{AC00}", "\u{1100}\u{200D}\u{1161}"),
        ("\u{AC00}", "\u{1100}\u{200C}\u{1161}"),
        ("\u{AC01}", "\u{AC00}\u{200D}\u{11A8}"),
    ];
    let merged: Vec<String> = cases
        .iter()
        .filter(|(base, variant)| !accepted_as_distinct(&client, &unique(base), &unique(variant)))
        .map(|(_, variant)| variant.escape_unicode().to_string())
        .collect();
    assert!(
        merged.is_empty(),
        "모양이 다른 이름이 중복으로 거부됐다: {merged:?}"
    );
}

/// 깨짐(과잉 병합). 글자 사이의 빈칸 모양 문자는 키에서 아예 빠져 `"LeadSynth"`와 같은 키가 된다.
/// 그려 보면 `"Lead\u{2800}Synth"`는 너비 73.2로 `"LeadSynth"`(64.3)와도 `"Lead Synth"`(67.8)와도
/// 다르고, 눈으로 넓은 틈이 보인다. U+1D159, U+16FE4는 더 넓다(78.6).
#[test]
fn create_virtual_output_blank_looking_between_letters_succeeds_as_distinct_name() {
    let client = MidiClient::new("ks-review6-inner-blank").unwrap();
    let mut merged = Vec::new();
    for blank in ['\u{2800}', '\u{1D159}', '\u{16FE4}'] {
        let variant = unique(&format!("Lead{blank}Synth"));
        for base in ["LeadSynth", "Lead Synth"] {
            if !accepted_as_distinct(&client, &unique(base), &variant) {
                merged.push(format!("{} ~ {base}", blank.escape_unicode()));
            }
        }
    }
    assert!(
        merged.is_empty(),
        "모양이 다른 이름이 중복으로 거부됐다: {merged:?}"
    );
}

/// 경계. 결정이 같은 이름으로 정한 변형은 위치와 상관없이 모두 `DuplicateName`이다. 허용 예외
/// 문자(ZWJ, ZWNJ, VS16, VS17, 태그 문자)와 폭 없는 빈칸 모양 문자(U+FFF9–FFFB)를 아무 글자 경계에
/// 끼운 이름, 공백 종류·개수·앞뒤 공백만 다른 이름, 그 둘을 섞은 이름, NFD 이름, 끝에 붙인 점자 빈칸
/// (그려 보면 픽셀까지 같다)을 모두 막는다.
#[test]
fn create_virtual_output_invisible_or_space_variant_at_any_position_fails_with_duplicate() {
    let client = MidiClient::new("ks-review6-any-position").unwrap();
    let base = unique("Pad Caf\u{00E9} \u{AC00}");
    let _port = client.create_virtual_output(&base).unwrap();

    let chars: Vec<char> = base.chars().collect();
    let mut variants = Vec::new();
    for invisible in [
        '\u{200D}',
        '\u{200C}',
        '\u{FE0F}',
        '\u{E0100}',
        '\u{E0041}',
        '\u{FFF9}',
        '\u{FFFA}',
        '\u{FFFB}',
    ] {
        for at in 0..=chars.len() {
            let mut v = chars.clone();
            v.insert(at, invisible);
            variants.push(v.into_iter().collect::<String>());
        }
    }
    for space in [
        "\u{00A0}",
        "\u{3000}",
        "  ",
        " \u{200D} ",
        "\u{00A0}\u{FE0F}\u{2009}",
    ] {
        variants.push(base.replace(' ', space));
        variants.push(format!("{space}{base}{space}"));
    }
    variants.push(unique("Pad Cafe\u{0301} \u{1100}\u{1161}"));
    variants.push(unique("Pad Cafe\u{0301}\u{200D} \u{1100}\u{1161}\u{FE0F}"));
    variants.push(format!("{base}\u{2800}"));
    variants.push(format!("{base} \u{2800}"));

    let accepted: Vec<String> = variants
        .iter()
        .filter(|v| match client.create_virtual_output(v) {
            Ok(_) => true,
            Err(MidiError::DuplicateName(_)) => false,
            Err(e) => panic!("{}: 예상하지 못한 오류 {e:?}", v.escape_unicode()),
        })
        .map(|v| v.escape_unicode().to_string())
        .collect();
    assert!(
        accepted.is_empty(),
        "같은 모양의 이름이 따로 만들어졌다: {accepted:?}"
    );
}

/// 경계. 이름 변경과 해제가 생성과 같은 키 함수를 쓴다. 새 키 규칙(뺀 뒤 NFC, 공백 정리, 빈칸 모양
/// 문자 제거)으로만 같아지는 이름으로 바꾸면 자기 키라 `Ok`이고, 다른 포트는 그 이름으로 바꿀 수
/// 없으며, 해제하면 그 키가 정확히 풀린다.
#[test]
fn rename_and_drop_with_recomposed_key_variants_keep_keys_in_step() {
    let client = MidiClient::new("ks-review6-rename").unwrap();
    let base = unique("Caf\u{00E9} Keys");
    let mut a = client.create_virtual_output(&base).unwrap();
    let mut b = client.create_virtual_output(&unique("Bass")).unwrap();

    for own_variant in [
        unique("Cafe\u{FE00}\u{0301} Keys"),
        unique("Caf\u{00E9} \u{200C} Keys\u{2800}"),
        unique("Caf\u{00E9}\u{FFFB} Keys \u{FE0F}"),
    ] {
        assert_eq!(
            a.rename(&own_variant),
            Ok(()),
            "{}",
            own_variant.escape_unicode()
        );
        for other in [base.clone(), unique("Cafe\u{E0100}\u{0301}\u{00A0}Keys")] {
            assert!(
                matches!(b.rename(&other), Err(MidiError::DuplicateName(_))),
                "{}",
                other.escape_unicode()
            );
            assert_eq!(b.name(), unique("Bass"));
        }
        assert!(matches!(
            client.create_virtual_output(&base),
            Err(MidiError::DuplicateName(_))
        ));
    }

    drop(a);
    assert_eq!(b.rename(&unique("Cafe\u{FE00}\u{0301} Keys")), Ok(()));
    drop(b);
    let again = client.create_virtual_output(&base).unwrap();
    assert_eq!(again.name(), base);
}
