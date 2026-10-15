//! S1-T1 적대적 리뷰 7회차(`docs/reports/s1-t1/review-7.md`)의 테스트.
//!
//! 대상은 중복 키에 CGJ를 남기는 수정(`midi: Keep letters split by joiners distinct in the duplicate
//! key`) 하나다. 깨뜨리지 못했으므로 모두 경계를 고정하는 통과 테스트다. CoreMIDI 상태는 시스템
//! 전역이라 포트 이름에 프로세스 id를 넣는다. 같은 바이너리의 테스트가 동시에 돌아도 프로세스
//! 상한(16)에 닿지 않도록 한 번에 쥐는 포트를 둘 이하로 둔다.

use keyscore_midi::{MidiClient, MidiError};

fn unique(name: &str) -> String {
    format!("KS review7 {} {name}", std::process::id())
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

/// 경계. 가장자리 문자(공백, 빈칸 모양 문자, 허용 예외 문자)만 있는 이름과 빈 이름은 키를 만들기
/// 전에 `InvalidName`으로 거부되고 패닉하지 않는다.
#[test]
fn create_virtual_output_empty_or_edge_only_name_fails_with_invalid_name() {
    let client = MidiClient::new("ks-review7-edge-only").unwrap();
    for name in [
        "",
        "\u{200D}",
        "\u{FE0F}\u{200C}",
        " \u{2800}\u{FE0F} ",
        "\u{E0041}\u{FFF9}\u{FFFB}",
        "\u{00A0}\u{1D159}\u{16FE4}\u{E0100}\u{3000}",
    ] {
        assert!(
            matches!(
                client.create_virtual_output(name),
                Err(MidiError::InvalidName)
            ),
            "{}",
            name.escape_unicode()
        );
    }
}

/// 경계. 이름 앞뒤에 가장자리 문자가 섞여 길게 붙어도 키에서는 모두 떨어진다. 마지막 글자 바로
/// 앞의 연속 묶음(인덱스가 `end - 1`까지 가는 경로)도 패닉 없이 결정대로 빠진다.
#[test]
fn create_virtual_output_edge_runs_and_run_before_last_letter_fail_with_duplicate() {
    let client = MidiClient::new("ks-review7-runs").unwrap();
    let base = unique("PadX");
    let _port = client.create_virtual_output(&base).unwrap();
    let variants = [
        format!("\u{200D} \u{2800}{base}\u{FE0F} \u{2800}\u{E0041}"),
        format!("\u{FFF9}\u{E0100}{base}\u{FFFB}\u{200C}"),
        unique("Pad\u{200D}\u{FE0F}\u{E0041}\u{FFF9}X"),
        unique("Pad\u{FFFA}\u{200C}\u{E0100}X\u{2800}"),
    ];
    for variant in &variants {
        assert!(
            matches!(
                client.create_virtual_output(variant),
                Err(MidiError::DuplicateName(_))
            ),
            "{}",
            variant.escape_unicode()
        );
    }
}

/// 경계(구현 메모 리뷰 6). 결합자를 품은 연속 묶음 뒤의 결합 문자, 아무 뺀 문자 뒤의 한글
/// 중성·종성 낱자는 CGJ로 갈라져 합성된 글자와 다른 이름이다. 묶음 안 순서와 구성(ZWJ·ZWNJ, 이체자
/// 선택자, 태그, U+FFF9–FFFB)이 달라도 같은 자리면 서로 같은 이름이다.
#[test]
fn create_virtual_output_mixed_run_before_mark_or_jamo_follows_split_rule() {
    let client = MidiClient::new("ks-review7-mixed-run").unwrap();
    let distinct = [
        ("Caf\u{00E9}", "Cafe\u{FE0F}\u{200D}\u{0301}"),
        ("Caf\u{00E9}", "Cafe\u{200C}\u{E0041}\u{0301}"),
        ("\u{AC01}", "\u{AC00}\u{FE0F}\u{FFF9}\u{11A8}"),
        ("\u{AC00}", "\u{1100}\u{E0100}\u{1161}"),
    ];
    for (base, variant) in distinct {
        assert!(
            accepted_as_distinct(&client, &unique(base), &unique(variant)),
            "{} ~ {}",
            variant.escape_unicode(),
            base.escape_unicode()
        );
    }
    let same = [
        (
            "Cafe\u{200D}\u{0301}",
            "Cafe\u{FE0F}\u{200C}\u{FFFA}\u{0301}",
        ),
        (
            "\u{AC00}\u{200D}\u{11A8}",
            "\u{AC00}\u{FE0F}\u{FFF9}\u{11A8}",
        ),
        ("Caf\u{00E9}", "Cafe\u{FE0F}\u{E0041}\u{FFFB}\u{0301}"),
    ];
    for (base, variant) in same {
        assert!(
            !accepted_as_distinct(&client, &unique(base), &unique(variant)),
            "{} ~ {}",
            variant.escape_unicode(),
            base.escape_unicode()
        );
    }
}

/// 경계. CGJ가 들어간 키도 생성, 이름 변경, 해제에서 같은 값으로 잡히고 풀린다.
#[test]
fn rename_and_drop_with_cgj_keys_keep_keys_in_step() {
    let client = MidiClient::new("ks-review7-cgj-step").unwrap();
    let split = unique("Step Cafe\u{200D}\u{0301}");
    let composed = unique("Step Caf\u{00E9}");

    let mut a = client.create_virtual_output(&split).unwrap();
    let mut b = client.create_virtual_output(&composed).unwrap();
    // 자기 키와 같은 변형으로는 바꿀 수 있다.
    a.rename(&unique("Step Cafe\u{200C}\u{FE0F}\u{0301}"))
        .unwrap();
    // 남의 키(CGJ 키)로는 못 바꾸고, 같은 키로 새로 만들 수도 없다.
    assert!(matches!(b.rename(&split), Err(MidiError::DuplicateName(_))));
    assert!(matches!(
        client.create_virtual_output(&unique("Step Cafe\u{E0041}\u{200D}\u{0301}")),
        Err(MidiError::DuplicateName(_))
    ));

    drop(a);
    // A의 CGJ 키가 정확히 풀렸으므로 B가 그 키로 옮겨 갈 수 있고, B의 옛 키도 풀린다.
    b.rename(&split).unwrap();
    let c = client.create_virtual_output(&composed).unwrap();
    assert!(matches!(
        client.create_virtual_output(&unique("Step Cafe\u{200C}\u{0301}")),
        Err(MidiError::DuplicateName(_))
    ));
    drop(c);
    drop(b);
    // 모두 해제한 뒤에는 두 키가 다 비어 있다.
    let _a = client.create_virtual_output(&split).unwrap();
    let _b = client.create_virtual_output(&composed).unwrap();
}
