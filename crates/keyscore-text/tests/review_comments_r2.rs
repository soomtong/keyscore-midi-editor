//! 결정 Q1 개정(`//` 주석, `#`는 주석 아님) 적대적 리뷰 2회차(확인 리뷰)의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 리뷰 1 지적 1을 방법 (b)로 고친 결과를 확인한다. 첫 `@track` 앞의 모르는 토큰은 두 모드 모두
//! 오류이고 `#`면 「주석은 //」를 알린다. 트랙 블록 안의 `#`는 느슨 모드 경고, 엄격 모드 오류이고
//! 둘 다 같은 안내를 붙인다(`docs/reports/s0-t5-comments/review-2.md`).

use keyscore_text::{Mode, parse};

const HINT: &str = "(주석은 //)";
const MODES: [Mode; 2] = [Mode::Strict, Mode::Lenient];

fn err(text: &str, mode: Mode) -> (usize, usize, String) {
    match parse(text, mode) {
        Ok(_) => panic!("{text:?} {mode:?}: 오류여야 한다"),
        Err(e) => {
            let d = e.diagnostic();
            (d.line(), d.column(), d.message().to_owned())
        }
    }
}

/// 헤더 값 바로 뒤에 공백 없이 붙은 `#`도 헤더 영역의 모르는 토큰이다. 두 모드 모두 오류, 위치는
/// `#` 자리, 메시지에 안내.
#[test]
fn hash_glued_to_header_value_is_error_with_hint_in_both_modes() {
    for (text, col) in [
        ("@tempo 120#\n@track A ch1\nc", 11),
        ("@tempo 120# x\n@track A ch1\nc", 11),
        ("@time 4/4#\n@track A ch1\nc", 10),
        ("@key C major#\n@track A ch1\nc", 13),
        ("@key C# minor # x\n@track A ch1\nc", 15),
        ("#", 1),
    ] {
        for mode in MODES {
            let (line, column, msg) = err(text, mode);
            assert_eq!((line, column), (1, col), "{text:?} {mode:?}: {msg}");
            assert!(msg.contains("@track 블록 밖"), "{text:?} {mode:?}: {msg}");
            assert!(msg.ends_with(HINT), "{text:?} {mode:?}: {msg}");
        }
    }
}

/// 헤더 영역의 `#` 아닌 모르는 토큰(음, 홑 `/`, 글자)은 오류이되 `#` 안내를 붙이지 않는다.
#[test]
fn other_unknown_tokens_before_first_track_are_error_without_hint() {
    for text in [
        "x\n@track A ch1\nc",
        "c\n@track A ch1\nc",
        "/ x\n@track A ch1\nc",
    ] {
        for mode in MODES {
            let (line, column, msg) = err(text, mode);
            assert_eq!((line, column), (1, 1), "{text:?}");
            assert!(msg.contains("@track 블록 밖"), "{text:?}: {msg}");
            assert!(!msg.contains("//"), "{text:?}: {msg}");
        }
    }
}

/// 두 `@track` 블록 사이의 `#` 줄은 첫 트랙 본문 안이다. 느슨 모드 경고 후 건너뜀(뒤 글자는 각각
/// 모르는 토큰), 엄격 모드 오류. 둘 다 안내를 붙인다. `@track` 줄 끝의 `#`도 트랙 안이다.
#[test]
fn hash_between_tracks_and_after_track_header_is_inside_track() {
    let between = "@track A ch1\nc d\n# 메모\n@track B ch2\ne";
    let (line, column, msg) = err(between, Mode::Strict);
    assert_eq!((line, column), (3, 1));
    assert_eq!(msg, format!("모르는 토큰 '#'를 건너뛴다 {HINT}"));
    let p = parse(between, Mode::Lenient).expect("느슨 모드는 통과");
    assert_eq!(p.tracks().len(), 2);
    let w: Vec<String> = p.warnings().iter().map(ToString::to_string).collect();
    assert_eq!(
        w,
        [
            format!("줄 3, 열 1: 모르는 토큰 '#'를 건너뛴다 {HINT}"),
            "줄 3, 열 3: 모르는 토큰 '메'를 건너뛴다".to_owned(),
            "줄 3, 열 4: 모르는 토큰 '모'를 건너뛴다".to_owned(),
        ]
    );
    assert_eq!(p.tracks()[0].clip().notes().len(), 2);
    assert_eq!(p.tracks()[1].clip().notes().len(), 1);

    for (text, col) in [
        ("@track A ch1 # 메모\nc", 14),
        ("@track A ch1 #\nc", 14),
        ("@track A ch1 prog0#\nc", 19),
    ] {
        let (line, column, msg) = err(text, Mode::Strict);
        assert_eq!((line, column), (1, col), "{text:?}");
        assert!(msg.ends_with(HINT), "{text:?}: {msg}");
        let p = parse(text, Mode::Lenient).expect("느슨 모드는 통과");
        let first = &p.warnings()[0];
        assert_eq!((first.line(), first.column()), (1, col), "{text:?}");
        assert!(first.message().ends_with(HINT), "{text:?}");
        assert_eq!(p.tracks()[0].clip().notes().len(), 1, "{text:?}");
    }
}

/// 홑 `/`의 경고에는 `#` 안내가 붙지 않는다.
#[test]
fn lone_slash_inside_track_has_no_hash_hint() {
    let p = parse("@track A ch1\nc /", Mode::Lenient).expect("느슨 모드는 통과");
    assert_eq!(p.warnings().len(), 1);
    assert_eq!(p.warnings()[0].message(), "모르는 토큰 '/'를 건너뛴다");
}

/// BOM은 keyscore-text가 떼지 않는다(파일 인코딩은 ksq-cli가 뗀다). BOM + `#`는 BOM 자리 오류이고
/// 원인이 BOM이므로 `#` 안내가 붙지 않는다. ksq-cli `txt2ksq`는 BOM을 떼고 `#` 안내 오류를 낸다
/// (리뷰 2 시도 5에서 바이너리로 확인).
#[test]
fn bom_then_hash_is_error_at_bom_without_hint() {
    for text in [
        "\u{feff}# 옛 주석\n@track A ch1\nc",
        "\u{feff}@track A ch1\n# x\nc",
    ] {
        for mode in MODES {
            let (line, column, msg) = err(text, mode);
            assert_eq!((line, column), (1, 1), "{text:?}");
            assert!(msg.contains("@track 블록 밖"), "{text:?}: {msg}");
            assert!(!msg.contains("//"), "{text:?}: {msg}");
        }
    }
}

/// 헤더 값 자리(값 대신 `#`)의 오류는 그 헤더의 형식 오류이고 안내가 없다. 결정 문구는 「토큰 자리」의
/// `#`만 다루므로 어긋나지 않는다(리뷰 2 제안 1).
#[test]
fn hash_in_header_value_position_is_header_error_without_hint() {
    for (text, expect) in [
        (
            "@tempo # 120\n@track A ch1\nc",
            "@tempo 뒤에는 BPM 숫자를 쓴다",
        ),
        (
            "@key C #\n@track A ch1\nc",
            "@key의 으뜸음 뒤에는 major 또는 minor를 쓴다",
        ),
    ] {
        for mode in MODES {
            let (_, _, msg) = err(text, mode);
            assert_eq!(msg, expect, "{text:?}");
        }
    }
}

/// 트랙 본문이 `#` 하나뿐이면 느슨 모드는 `#` 경고 대신 「빈 블록」 오류로 끝나 안내가 사라진다
/// (리뷰 2 제안 2). 동작 기록용.
#[test]
fn track_with_only_hash_lenient_reports_empty_block_without_hint() {
    for text in ["@track A ch1\n#", "@track A ch1\n#\n@track B ch2\ne"] {
        let (line, _, msg) = err(text, Mode::Lenient);
        assert_eq!(line, 1, "{text:?}");
        assert!(msg.contains("음이나 쉼표가 없다"), "{text:?}: {msg}");
        let (line, column, msg) = err(text, Mode::Strict);
        assert_eq!((line, column), (2, 1), "{text:?}");
        assert!(msg.ends_with(HINT), "{text:?}: {msg}");
    }
}
