//! 결정 Q1 개정(`//` 주석, `#`는 주석 아님) 적대적 리뷰 1회차의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5-comments/review-1.md` 7절),
//! 통과하는 테스트는 깨뜨리지 못한 시도를 회귀 테스트로 남긴 것이다.

use keyscore_core::{Channel, Clip, Key, Note, Pitch, Program, Tick, TimeSignature, Velocity};
use keyscore_text::{ExportTrack, Header, Mode, Parsed, export, parse};
use proptest::prelude::*;

type View = (u64, u32, u8, Option<String>);

fn view(c: &Clip) -> Vec<View> {
    let mut v: Vec<View> = c
        .notes()
        .iter()
        .map(|n| {
            (
                n.start().get(),
                n.length(),
                n.pitch().get(),
                n.lyric().map(str::to_owned),
            )
        })
        .collect();
    v.sort();
    v
}

fn ok(text: &str, mode: Mode) -> Parsed {
    match parse(text, mode) {
        Ok(p) => p,
        Err(e) => panic!("{text:?}: {:?}", e.diagnostic()),
    }
}

fn notes(text: &str, mode: Mode) -> Vec<View> {
    view(ok(text, mode).tracks()[0].clip())
}

fn plain(start: u64, len: u32, pitch: u8) -> View {
    (start, len, pitch, None)
}

/// 토큰 사이 trivia를 건너뛰는 모든 자리에 // 주석을 넣어도 주석이 없는 입력과 결과가 같다.
#[test]
fn comments_in_every_trivia_position_are_skipped() {
    let commented = "// 첫 줄\n\
        @tempo 120// a\n\
        @time 3/4 // b\n\
        @key F# minor// c\n\
        @track Lead ch1 // d\n\
        prog5 // e\n\
        c4 c c |// r\n\
        o4// f\n\
        l8// g\n\
        c// h\n\
        [c // i\n e // j\n g]4// k\n\
        n(c // l\n d // m\n e)// n\n\
        c#4&// o\n\
        c#4 d\"la // 글자\"// p\n\
        >// q\n\
        <c4.// s";
    let bare = "@tempo 120\n@time 3/4\n@key F# minor\n@track Lead ch1 prog5\n\
        c4 c c | o4 l8 c [c e g]4 n(c d e) c#4& c#4 d\"la // 글자\" > < c4.";
    for mode in [Mode::Strict, Mode::Lenient] {
        let a = ok(commented, mode);
        let b = ok(bare, mode);
        assert_eq!(a.header(), b.header(), "{mode:?}");
        assert_eq!(a.tracks(), b.tracks(), "{mode:?}");
        assert!(a.warnings().is_empty(), "{:?}", a.warnings());
    }
    let p = ok(commented, Mode::Strict);
    assert_eq!(p.header().time(), Some(TimeSignature::new(3, 4).unwrap()));
    assert!(matches!(p.header().key(), Some(Key::Minor(pc)) if pc.get() == 6));
    assert_eq!(p.tracks()[0].program(), Some(Program::new(5).unwrap()));
    assert!(
        view(p.tracks()[0].clip())
            .iter()
            .any(|n| n.3.as_deref() == Some("la // 글자"))
    );
}

/// 느슨 모드에서만 받는 ~ 보정 뒤와 드럼 라벨 뒤에도 주석이 붙는다.
#[test]
fn comments_after_offset_and_drum_labels_are_skipped() {
    assert_eq!(
        notes("@track A ch1\nc4~+13// x\nd4~+0:-20// y\n", Mode::Lenient),
        notes("@track A ch1\nc4~+13 d4~+0:-20", Mode::Lenient),
    );
    assert_eq!(
        notes(
            "@track D ch10\nbd4// x\nn36// y\n[bd sd// z\n]4",
            Mode::Strict
        ),
        notes("@track D ch10\nbd4 n36 [bd sd]4", Mode::Strict),
    );
}

/// `@time 4//4`는 4/4도, `4/` 뒤 주석도 아니다. 두 모드 모두 오류.
#[test]
fn time_with_double_slash_is_error() {
    for mode in [Mode::Strict, Mode::Lenient] {
        let e = parse("@time 4//4\n@track A ch1\nc", mode).unwrap_err();
        assert_eq!(e.diagnostic().line(), 1, "{mode:?}");
        assert!(e.diagnostic().message().contains("@time"), "{mode:?}");
    }
    // 4/4 바로 뒤의 //는 주석이다.
    let p = ok("@time 4/4//x\n@track A ch1\nc", Mode::Strict);
    assert_eq!(p.header().time(), Some(TimeSignature::new(4, 4).unwrap()));
}

/// 파일 끝의 주석(줄바꿈 없음), `///`, 빈 `//`.
#[test]
fn comment_edges_at_eof_triple_slash_and_empty() {
    for text in [
        "@track A ch1\nc// 끝",
        "@track A ch1\nc//",
        "@track A ch1\nc///",
        "@track A ch1\n/// d\nc",
        "@track A ch1\n//\nc",
        "///@track B ch2\n@track A ch1\nc",
    ] {
        let p = ok(text, Mode::Strict);
        assert_eq!(p.tracks().len(), 1, "{text}");
        assert_eq!(
            view(p.tracks()[0].clip()),
            vec![plain(0, 960, 60)],
            "{text}"
        );
    }
    // 주석만 있는 문서는 트랙이 없다.
    assert!(ok("// 비었다", Mode::Strict).tracks().is_empty());
}

/// 홑 `/`는 모르는 토큰이다. 파일 끝, 화음·셋잇단 안, `/ /`(공백 낀 두 개)도.
#[test]
fn lone_slash_is_unknown_token_everywhere() {
    for (text, count) in [
        ("@track A ch1\nc /", 1),
        ("@track A ch1\nc/", 1),
        ("@track A ch1\nc / / d", 2),
        ("@track A ch1\n[c / e]", 1),
        ("@track A ch1\nn(c / d e)", 1),
    ] {
        assert!(parse(text, Mode::Strict).is_err(), "{text}");
        let p = ok(text, Mode::Lenient);
        assert_eq!(p.warnings().len(), count, "{text}: {:?}", p.warnings());
    }
}

/// 토큰 자리의 `#`: 엄격 모드 오류, 느슨 모드 경고 한 개 후 계속(트랙 블록 안).
#[test]
fn hash_in_token_positions_inside_track_is_unknown() {
    for text in [
        "@track A ch1\n[c # e]",
        "@track A ch1\nn(c # d e)",
        "@track A ch1\nc1 | # d",
        "@track A ch1\nc& # c",
        "@track A ch1\nc\"x\"#",
        "@track A ch1\nc4.#",
        "@track A ch1 #\nc",
        "@track A ch1 prog0 bank0#\nc",
    ] {
        let e = parse(text, Mode::Strict).unwrap_err();
        assert!(e.diagnostic().message().contains('#'), "{text}");
        let p = ok(text, Mode::Lenient);
        assert_eq!(p.warnings().len(), 1, "{text}: {:?}", p.warnings());
    }
    // 올림표 #와 그 뒤 // 주석.
    assert_eq!(
        notes("@track A ch1\nc#// x\nd-#4//y", Mode::Strict),
        vec![plain(0, 960, 61), plain(960, 960, 62)]
    );
    // 음이름 뒤의 # 연속은 올림표 여럿이다(주석이 아니다).
    assert_eq!(
        notes("@track A ch1\nc## d", Mode::Strict),
        vec![plain(0, 960, 62), plain(960, 960, 62)]
    );
}

/// 첫 @track 앞(헤더 영역)의 #는 두 모드 모두 오류이고 「주석은 //」를 알린다. 옛 # 주석 파일(옛 REQ
/// 예시, 옛 픽스처 원본)의 첫 줄이 바로 이 자리다. 처음에는 결정 문구(느슨 모드 경고)와 어긋나는 결함으로
/// 적었으나, 리뷰의 방법 (b)에 따라 결정 문구를 이 동작으로 좁혔다(구현자 수정: 주석만).
#[test]
fn hash_before_first_track_is_error_with_slash_hint() {
    for text in [
        "# 옛 주석\n@track A ch1\nc",
        "@tempo 120          # 선택\n@track A ch1\nc",
        "@key C major # 선택\n@track A ch1\nc",
    ] {
        // 구현자 수정: 리뷰가 낸 방법 (b)를 택했다. 헤더 영역의 모르는 토큰은 두 모드 모두 오류이고,
        // `#`면 「주석은 //」를 알린다(결정 Q1 개정 문구를 그에 맞게 좁혔다).
        for mode in [Mode::Strict, Mode::Lenient] {
            let e = parse(text, mode).unwrap_err();
            assert!(e.diagnostic().message().contains("//"), "{text}: {e}");
        }
    }
}

/// 따옴표 없는 트랙 이름: `#`·홑 `/`는 이름 글자, `//`에서 끝난다.
#[test]
fn bare_track_names_with_slash_and_hash() {
    for (text, name) in [
        ("@track A/ ch1\nc", "A/"),
        ("@track / ch1\nc", "/"),
        ("@track #/# ch1\nc", "#/#"),
        ("@track A/B// x\nch1\nc", "A/B"),
        ("@track Lead#2// ch9\nch1\nc", "Lead#2"),
        ("@track \"A // B\" ch1\nc", "A // B"),
        ("@track \"#\" ch1\nc", "#"),
    ] {
        let p = ok(text, Mode::Strict);
        assert_eq!(p.tracks()[0].name(), name, "{text}");
        assert_eq!(p.tracks()[0].channel(), Channel::new(1).unwrap(), "{text}");
    }
    // 이름이 //로 시작하면 이름이 비어 오류다.
    assert!(parse("@track //x ch1\nc", Mode::Lenient).is_err());
}

fn one_note_clip(lyric: Option<&str>) -> Clip {
    let mut c = Clip::new(Tick::ZERO, 3840).unwrap();
    let n = Note::new(
        Tick::ZERO,
        960,
        Pitch::new(60).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap();
    c.insert_note(match lyric {
        Some(y) => n.with_lyric(y),
        None => n,
    })
    .unwrap();
    c
}

fn name_chars() -> impl Strategy<Value = String> {
    let chars = prop::sample::select(vec![
        'a', '/', '#', ' ', '"', '\\', '가', '@', 'c', 'h', '1', '\t', '\n',
    ]);
    prop::collection::vec(chars, 0..8).prop_map(|v| v.into_iter().collect())
}

proptest! {
    /// `/`·`#`가 섞인 트랙 이름과 가사가 export → parse(엄격)로 그대로 돌아오고, 다시 내보내면 같다.
    #[test]
    fn names_and_lyrics_with_slashes_round_trip(name in name_chars(), lyric in name_chars()) {
        let c = one_note_clip(Some(&lyric));
        let t = ExportTrack {
            name: &name,
            channel: Channel::new(1).unwrap(),
            program: None,
            bank: None,
            clip: &c,
        };
        let text = export(&Header::default(), &[t]);
        let p = parse(&text, Mode::Strict).map_err(|e| {
            TestCaseError::fail(format!("{text:?}: {:?}", e.diagnostic()))
        })?;
        prop_assert_eq!(p.tracks().len(), 1);
        prop_assert_eq!(p.tracks()[0].name(), name.as_str());
        prop_assert_eq!(view(p.tracks()[0].clip()), view(&c));
        let again = export(
            p.header(),
            &[ExportTrack {
                name: p.tracks()[0].name(),
                channel: p.tracks()[0].channel(),
                program: None,
                bank: None,
                clip: p.tracks()[0].clip(),
            }],
        );
        prop_assert_eq!(again, text);
    }
}
