//! 결정 Q1 2차 개정(레거시 `#` 주석 줄) 적대적 리뷰 1회차의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 규칙: 레거시 지원 모드에서 줄 첫머리(앞 공백 무시)의 `#`부터 줄 끝까지는 주석, 노트 바로 뒤의
//! `#`는 올림표, 그 밖의 `#`는 1차 개정의 오류·경고 규칙(`docs/reports/s0-t5-comments/review-legacy-1.md`).

use std::time::{Duration, Instant};

use keyscore_core::{Key, PitchClass};
use keyscore_text::{
    ExportTrack, Mode, ParseError, ParseOptions, Parsed, export, parse, parse_with,
};
use proptest::prelude::*;

const MODES: [Mode; 2] = [Mode::Strict, Mode::Lenient];
const LEGACY: ParseOptions = ParseOptions {
    legacy_hash_comments: true,
};

fn legacy(text: &str, mode: Mode) -> Result<Parsed, ParseError> {
    parse_with(text, mode, LEGACY)
}

fn pitches(p: &Parsed) -> Vec<(u64, u8)> {
    p.tracks()[0]
        .clip()
        .notes()
        .iter()
        .map(|n| (n.start().get(), n.pitch().get()))
        .collect()
}

#[test]
fn legacy_hash_after_crlf_and_indent_is_comment_in_both_modes() {
    let text = "# 머리\r\n@track A ch1\r\nc\r\n# x y\r\n  \t# z\r\n\u{3000}# 전각 공백 뒤\r\nd\r\n";
    for mode in MODES {
        let p = legacy(text, mode).unwrap();
        assert!(p.warnings().is_empty(), "{mode:?}: {:?}", p.warnings());
        assert_eq!(pitches(&p), vec![(0, 60), (960, 62)]);
    }
}

#[test]
fn legacy_hash_at_next_line_start_after_note_is_comment_not_sharp() {
    // `c` 뒤 줄바꿈 다음 줄 첫머리의 `#d`는 올림표가 아니라 주석이다.
    for mode in MODES {
        let p = legacy("@track A ch1\nc\n#d e\nf", mode).unwrap();
        assert_eq!(pitches(&p), vec![(0, 60), (960, 65)]);
        // 같은 줄에 붙은 `#`는 올림표다. 여러 개도 모두 올림표다.
        let p = legacy("@track A ch1\nc##\nd", mode).unwrap();
        assert_eq!(pitches(&p), vec![(0, 62), (960, 62)]);
    }
}

#[test]
fn legacy_hash_line_inside_chord_and_triplet_is_comment() {
    for mode in MODES {
        let p = legacy("@track A ch1\n[c e\n# ] 가짜 닫기\n g]", mode).unwrap();
        assert_eq!(pitches(&p), vec![(0, 60), (0, 64), (0, 67)]);
        let p = legacy("@track A ch1\nn(c d\n  # x )\ne) f", mode).unwrap();
        assert_eq!(
            pitches(&p),
            vec![(0, 60), (640, 62), (1280, 64), (1920, 65)]
        );
    }
}

#[test]
fn legacy_hash_inside_lyric_spanning_lines_stays_text() {
    for mode in MODES {
        let p = legacy("@track A ch1\nc\"가\n# 나 // 다\"\nd", mode).unwrap();
        let clip = p.tracks()[0].clip();
        assert_eq!(clip.notes()[0].lyric(), Some("가\n# 나 // 다"));
        assert_eq!(clip.notes().len(), 2);
    }
}

#[test]
fn legacy_key_sharp_at_line_start_is_header_not_comment() {
    for mode in MODES {
        let p = legacy("@key F# minor\n# @key C major\n@track A ch1\nc", mode).unwrap();
        assert_eq!(
            p.header().key(),
            Some(Key::Minor(PitchClass::new(6).unwrap()))
        );
        // `@key F` 다음 줄 첫머리의 `# minor`는 주석이라 조가 완성되지 않는다(규칙대로 오류).
        assert!(legacy("@key F\n# minor\n@track A ch1\nc", mode).is_err());
        // 헤더와 값 사이의 주석 줄은 건너뛴다.
        let p = legacy("@tempo\n# 옛 주석\n90\n@track A ch1\nc", mode).unwrap();
        assert!(p.header().tempo().is_some());
    }
}

#[test]
fn legacy_track_name_with_hash_mid_line_is_name() {
    for mode in MODES {
        let p = legacy("@track #1 ch1\nc", mode).unwrap();
        assert_eq!(p.tracks()[0].name(), "#1");
        // 이름 자리가 다음 줄 첫머리의 `#`면 주석이라 이름이 없다(규칙대로 오류).
        assert!(legacy("@track\n#1 ch1\nc", mode).is_err());
    }
}

#[test]
fn legacy_bom_is_not_whitespace_at_library_level() {
    // BOM은 `txt2ksq`가 떼므로 라이브러리에서는 BOM 뒤 `#`가 줄 첫머리가 아니다(문서화용).
    assert!(legacy("\u{feff}# x\n@track A ch1\nc", Mode::Lenient).is_err());
}

#[test]
fn legacy_trailing_hash_comment_words_become_notes_in_lenient() {
    // 옛 기록의 줄 끝 `#` 주석은 줄 첫머리가 아니라 규칙상 모르는 토큰이다. 느슨 모드는 `#` 하나만
    // 건너뛰고 뒤의 글자를 음으로 읽는다(경고는 `#` 하나뿐).
    let p = legacy("@track A ch1\nc d # e f", Mode::Lenient).unwrap();
    assert_eq!(p.warnings().len(), 1);
    assert_eq!(
        pitches(&p),
        vec![(0, 60), (960, 62), (1920, 64), (2880, 65)]
    );
    assert!(legacy("@track A ch1\nc d # e f", Mode::Strict).is_err());
}

#[test]
fn legacy_many_hashes_on_one_long_line_is_linear() {
    // `at_line_start`가 뒤로 걷는다. 긴 줄에 `#`가 많아도 첫 공백 아닌 글자에서 멈춰야 한다.
    let mut text = String::from("@track A ch1\nc");
    for _ in 0..20_000 {
        text.push_str("   #");
    }
    text.push('\n');
    text.push_str(&" ".repeat(200_000));
    text.push_str("# 긴 들여쓰기 뒤 주석\nd");
    let started = Instant::now();
    let p = legacy(&text, Mode::Lenient).unwrap();
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(p.warnings().len(), 20_000);
    assert_eq!(pitches(&p), vec![(0, 60), (960, 62)]);
}

#[test]
fn legacy_export_never_writes_hash_at_line_start_and_rereads_same() {
    let text = "@key F# minor\n@track #A ch1\nc# d#\"x\n# y\" [c# e]";
    let p = parse(text, Mode::Lenient).unwrap();
    let tracks: Vec<ExportTrack<'_>> = p
        .tracks()
        .iter()
        .map(|t| ExportTrack {
            name: t.name(),
            channel: t.channel(),
            program: t.program(),
            bank: t.bank(),
            clip: t.clip(),
        })
        .collect();
    let out = export(p.header(), &tracks);
    // 가사 안의 줄바꿈 뒤 `#`만 있을 수 있고, 그것은 따옴표 안이다.
    let outside: Vec<&str> = out
        .lines()
        .filter(|l| l.trim_start().starts_with('#'))
        .collect();
    assert_eq!(outside.len(), 1, "{out}");
    assert!(outside[0].starts_with("# y\""), "{out}");
    for mode in MODES {
        assert_eq!(parse(&out, mode), legacy(&out, mode), "{out}");
    }
}

/// 줄 첫머리(앞 공백 무시)에 `#`가 있는가. 따옴표를 가리지 않고 글자로만 본다.
fn has_line_start_hash(text: &str) -> bool {
    text.split('\n')
        .any(|line| line.trim_start().starts_with('#'))
}

/// 줄 첫머리의 `#` 하나를 `//`로 바꾼다.
fn hash_lines_to_slash(text: &str) -> String {
    replace_line_start_hash(text, "//")
}

/// 줄 첫머리의 `#` 하나를 `with`로 바꾼다.
fn replace_line_start_hash(text: &str, with: &str) -> String {
    text.split('\n')
        .map(|line| {
            let body = line.trim_start();
            if let Some(rest) = body.strip_prefix('#') {
                format!("{}{with}{rest}", &line[..line.len() - body.len()])
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn token() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "c", "d", "e", "#", "##", " ", "\t", "\n", "\r\n", "[", "]", "n(", ")", "4", "8", "&", "|",
        "o5", "<", ">", "r", "// x", "@key F", "@key F#", " minor", "\u{3000}", "/",
    ])
}

fn body() -> impl Strategy<Value = String> {
    prop::collection::vec(token(), 0..40).prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 줄 첫머리 `#`가 없으면 레거시 모드는 결과(오류 위치 포함)를 바꾸지 않는다.
    #[test]
    fn legacy_without_line_start_hash_equals_plain(b in body()) {
        // 줄 첫머리 `#`를 음 `c`로 바꿔 없앤다(`##`의 둘째 `#`는 올림표가 된다).
        let text = replace_line_start_hash(&format!("@track A ch1\n{b}"), "c");
        prop_assert!(!has_line_start_hash(&text));
        for mode in MODES {
            prop_assert_eq!(parse(&text, mode), legacy(&text, mode));
        }
    }

    /// 레거시 모드에서 줄 첫머리 `#` 줄은 그 자리를 `//`로 바꾼 텍스트와 똑같이 읽힌다
    /// (따옴표가 없는 입력). 경고·오류 위치도 같다.
    #[test]
    fn legacy_hash_line_equals_slash_line(b in body()) {
        let text = format!("@track A ch1\n{b}");
        let slashed = hash_lines_to_slash(&text);
        for mode in MODES {
            prop_assert_eq!(legacy(&text, mode), parse(&slashed, mode));
        }
    }
}
