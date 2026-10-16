//! 파서 단위 테스트(S0-T5 계획 테스트 3, 5–15). 공개 API만 쓴다.

mod common;

use std::time::{Duration, Instant};

use common::{drums, err, melody, n, nl, notes, ok, one_track, render, warning_positions};
use keyscore_core::{Key, PitchClass, Tempo, TimeSignature};
use keyscore_text::{Mode, parse};

/// REQ 5.14의 예시 블록 원문.
const REQ_EXAMPLE: &str = r"// 주석은 // 뒤
@tempo 120          // 선택. 없으면 프로젝트 값
@time 4/4           // 선택
@key C major        // 선택. 'none' 가능

@track Lead ch1 prog0
o4 l8 c d e f g4 r4 | e d c4 c2 | [c e g]4 r4 c8&c8 d4 |

@track Bass ch2 prog33
o2 l4 c c g g | a a g2 |

@track Drums ch10
l16 [bd ch] ch [sd ch] ch [bd ch] ch [sd oh] r [bd ch] ch [sd ch] ch [bd ch] ch [sd oh] r |
n(ch ch ch) ch4 [bd cr]4 r8 sd8 sd8 |
";

#[test]
fn req_example_block_snapshot() {
    let p = ok(REQ_EXAMPLE, Mode::Lenient);
    // 결정 Q3: 사람이 REQ 예시를 마디가 맞게 고쳤으므로(2026-10-15) 경고가 없다.
    assert_eq!(warning_positions(&p), Vec::<(usize, usize)>::new());
    insta::assert_snapshot!(render(&p));
}

#[test]
fn invalid_lengths_are_errors_with_position() {
    let t = "@track T ch1\no4 c4 c3";
    for mode in [Mode::Lenient, Mode::Strict] {
        let (line, col, msg) = err(t, mode);
        assert_eq!((line, col), (2, 8));
        assert!(msg.contains('3'), "{msg}");
    }
    let (line, col, msg) = err("@track T ch1\n  c12", Mode::Lenient);
    assert_eq!((line, col), (2, 4));
    assert!(msg.contains("12"), "{msg}");
    // 쉼표·기본 길이·화음 길이도 같은 규칙이다.
    let at = |t: &str| {
        let (l, c, _) = err(t, Mode::Lenient);
        (l, c)
    };
    assert_eq!(at("@track T ch1\nr12"), (2, 2));
    assert_eq!(at("@track T ch1\nl3"), (2, 2));
    assert_eq!(at("@track T ch1\n[c e]3"), (2, 6));
    assert_eq!(at("@track T ch1\nc04"), (2, 2));
    // 점은 둘까지, 그리고 길이 숫자 뒤에만.
    assert_eq!(at("@track T ch1\nc4..."), (2, 5));
    assert_eq!(at("@track T ch1\nc."), (2, 2));
    // 허용 길이와 점의 틱(온음표 3840, 점 ×1.5, 겹점 ×1.75).
    assert_eq!(
        notes(&melody("c1 c2. c4.. c64 c64. c64..")),
        vec![
            n(0, 3840, 60),
            n(3840, 2880, 60),
            n(6720, 1680, 60),
            n(8400, 60, 60),
            n(8460, 90, 60),
            n(8550, 105, 60),
        ]
    );
    // 기본 길이 l은 스티키이고 점을 받는다.
    assert_eq!(
        notes(&melody("l8. c d l16 e")),
        vec![n(0, 720, 60), n(720, 720, 62), n(1440, 240, 64)]
    );
}

#[test]
fn drum_label_on_melody_track_is_error() {
    let at = |t: &str| {
        let (l, c, _) = err(t, Mode::Lenient);
        (l, c)
    };
    // 음으로 읽을 수 없는 라벨은 멜로디 트랙에서 오류다(결정 Q4).
    assert_eq!(at("@track T ch1\nc ch"), (2, 3));
    assert_eq!(at("@track T ch1\nsd"), (2, 1));
    assert_eq!(at("@track T ch1\noh"), (2, 1));
    assert_eq!(at("@track T ch1\n[c ch]"), (2, 4));
    assert_eq!(at("@track T ch1\nlt"), (2, 1));
    // 멜로디 트랙의 n<번호>도 드럼 전용이다.
    assert_eq!(at("@track T ch1\nn60"), (2, 1));
    // bd는 b d로 읽힌다(결정 Q4의 단점).
    assert_eq!(notes(&melody("bd")), vec![n(0, 960, 71), n(960, 960, 62)]);
    // 드럼 트랙은 라벨을 최장 일치로 읽고 n<번호>(35–81 밖도 0–127)를 받는다.
    assert_eq!(
        notes(&drums("bd sd ch n81 rd8 r8 rs n0 n127")),
        vec![
            n(0, 960, 36),
            n(960, 960, 38),
            n(1920, 960, 42),
            n(2880, 960, 81),
            n(3840, 480, 51),
            n(4800, 960, 37),
            n(5760, 960, 0),
            n(6720, 960, 127),
        ]
    );
    assert_eq!(
        notes(&drums("[BD Ch]8 cb")),
        vec![n(0, 480, 36), n(0, 480, 42), n(480, 960, 56)]
    );
    // 드럼 트랙에서 음이름과 범위 밖 번호는 오류다.
    assert_eq!(at("@track D ch10\nc"), (2, 1));
    assert_eq!(at("@track D ch10\nbd e"), (2, 4));
    assert_eq!(at("@track D ch10\nn128"), (2, 1));
}

#[test]
fn hash_is_sharp_after_note_and_comment_starts_with_slashes() {
    // 결정 Q1: 음이름 또는 앞선 변화표 바로 뒤의 #만 올림표다. 주석은 //부터다(2026-10-16 개정).
    assert_eq!(
        notes(&melody("c# d //e f\ng")),
        vec![n(0, 960, 61), n(960, 960, 62), n(1920, 960, 67)]
    );
    assert_eq!(
        notes(&melody("c+# c-# d#+ e--")),
        vec![
            n(0, 960, 62),
            n(960, 960, 60),
            n(1920, 960, 64),
            n(2880, 960, 62)
        ]
    );
    // 주석은 //부터 줄 끝까지다. 길이 바로 뒤, 공백 뒤, 줄 첫머리 어디서든 된다(결정 Q1 개정).
    assert_eq!(
        notes(&melody("c4// d\n// e\nf")),
        vec![n(0, 960, 60), n(960, 960, 65)]
    );
    assert_eq!(
        notes(&melody("c //d\ne")),
        vec![n(0, 960, 60), n(960, 960, 64)]
    );
    // 따옴표 안의 #와 //는 글자다.
    assert_eq!(notes(&melody("c\"#1\"")), vec![nl(0, 960, 60, "#1")]);
    assert_eq!(notes(&melody("c\"//1\"")), vec![nl(0, 960, 60, "//1")]);
}

#[test]
fn hash_is_not_a_comment() {
    // 결정 Q1 개정: #는 음이름·변화표 바로 뒤의 올림표일 뿐이다. 토큰 자리의 #는 모르는 토큰이다.
    for text in [
        "@track T ch1\nc # d",
        "@track T ch1\nc4# d",
        "@track T ch1\n# e",
    ] {
        let (line, _, msg) = err(text, Mode::Strict);
        assert_eq!(line, 2, "{text}");
        assert!(msg.contains('#'), "{msg}");
        // 느슨 모드는 # 한 글자를 건너뛰고 경고한다. 뒤의 음은 그대로 읽는다.
        let p = ok(text, Mode::Lenient);
        assert_eq!(p.warnings().len(), 1, "{text}");
    }
    assert_eq!(
        notes(&melody("c # d")),
        vec![n(0, 960, 60), n(960, 960, 62)]
    );
    // 홑 /도 모르는 토큰이다.
    let p = ok("@track T ch1\nc / d", Mode::Lenient);
    assert_eq!(warning_positions(&p), vec![(2, 3)]);
    // 올림표 #는 그대로다.
    assert_eq!(
        notes(&melody("c# c+# d#+")),
        vec![n(0, 960, 61), n(960, 960, 62), n(1920, 960, 64)]
    );
}

#[test]
fn track_name_may_contain_hash_and_ends_at_comment() {
    let p = ok("@track Lead#2 ch3\nc", Mode::Strict);
    assert_eq!(p.tracks()[0].name(), "Lead#2");
    let p = ok("@track Lead// 멜로디\nch3\nc", Mode::Lenient);
    assert_eq!(p.tracks()[0].name(), "Lead");
}

#[test]
fn triplet_group_rules() {
    // 결정 Q2(문법): 각 원소 길이(없으면 l)의 2/3.
    assert_eq!(
        notes(&melody("l8 n(c d e) f")),
        vec![
            n(0, 320, 60),
            n(320, 320, 62),
            n(640, 320, 64),
            n(960, 480, 65)
        ]
    );
    assert_eq!(
        notes(&melody("n(c8 d16 e16) g")),
        vec![
            n(0, 320, 60),
            n(320, 160, 62),
            n(480, 160, 64),
            n(640, 960, 67)
        ]
    );
    // 원소는 음·쉼표·화음.
    let c = melody("n(c r [e g])");
    assert_eq!(
        notes(&c),
        vec![n(0, 640, 60), n(1280, 640, 64), n(1280, 640, 67)]
    );
    assert_eq!(c.length(), 1920);
    assert_eq!(
        notes(&drums("l16 n(ch ch ch) ch4")),
        vec![
            n(0, 160, 42),
            n(160, 160, 42),
            n(320, 160, 42),
            n(480, 960, 42)
        ]
    );
    let at = |t: &str| {
        let (l, c, _) = err(t, Mode::Lenient);
        (l, c)
    };
    // 정확히 3개가 아니면 n( 위치에서 오류, ) 뒤 숫자도 오류.
    assert_eq!(at("@track T ch1\nn(c d)"), (2, 1));
    assert_eq!(at("@track T ch1\nr n(c d e f)"), (2, 3));
    assert_eq!(at("@track T ch1\nn(c d e)4"), (2, 9));
    assert_eq!(at("@track T ch1\nn(c d e"), (2, 1));
    assert_eq!(at("@track T ch1\nn(c n(d e f) g)"), (2, 5));
}

#[test]
fn voice_blocks_merge_into_one_clip() {
    let p = ok(
        "@track A ch1\nc1\n@track B ch2\nd4\n@track A ch1\ne4 e4\n",
        Mode::Lenient,
    );
    let names: Vec<&str> = p
        .tracks()
        .iter()
        .map(keyscore_text::ParsedTrack::name)
        .collect();
    assert_eq!(names, vec!["A", "B"]);
    let a = p.tracks()[0].clip();
    // 성부는 모두 클립 시작에서 시작한다. 클립 길이는 가장 긴 성부다.
    assert_eq!(
        notes(a),
        vec![n(0, 3840, 60), n(0, 960, 64), n(960, 960, 64)]
    );
    assert_eq!(a.length(), 3840);
    assert_eq!(notes(p.tracks()[1].clip()), vec![n(0, 960, 62)]);
    // 성부 블록마다 옥타브·기본 길이 상태가 처음으로 돌아간다.
    let c = one_track("@track A ch1\no5 l8 c\n@track A ch1\nc", Mode::Lenient);
    assert_eq!(notes(&c), vec![n(0, 960, 60), n(0, 480, 72)]);
    // 같은 이름인데 속성이 다르면 오류(어느 쪽이 맞는지 정할 수 없다).
    let (l, c, _) = err("@track A ch1\nc\n@track A ch2\nd", Mode::Lenient);
    assert_eq!((l, c), (3, 1));
    let (l, c, _) = err("@track A ch1 prog1\nc\n@track A ch1\nd", Mode::Lenient);
    assert_eq!((l, c), (3, 1));
}

#[test]
fn offset_suffix_start_and_length() {
    // 결정 Q3: ~<±시작>[:<±길이>]. 커서는 명목 길이로 나아간다.
    assert_eq!(
        notes(&melody("r4 c4~+13:-20 d4")),
        vec![n(973, 940, 60), n(1920, 960, 62)]
    );
    // REQ의 시작만 보정. 클립 길이는 노트 끝(493)이다(커서 480이 아니라).
    let c = melody("c8~+13");
    assert_eq!(notes(&c), vec![n(13, 480, 60)]);
    assert_eq!(c.length(), 493);
    assert_eq!(notes(&melody("r4 c4~-10")), vec![n(950, 960, 60)]);
    assert_eq!(notes(&melody("c64~+0:-59")), vec![n(0, 1, 60)]);
    // 길이 보정으로 다음 음과 겹칠 수 있다.
    assert_eq!(
        notes(&melody("c4~+0:+100 d4")),
        vec![n(0, 1060, 60), n(960, 960, 62)]
    );
    // 화음은 ] 뒤에 붙는다.
    assert_eq!(
        notes(&melody("[c e]8~+5:+1")),
        vec![n(5, 481, 60), n(5, 481, 64)]
    );
    let at = |t: &str| {
        let (l, c, _) = err(t, Mode::Lenient);
        (l, c)
    };
    // 시작이 0 미만, 길이가 1 미만, 부호 없음, 숫자 없음은 ~ 위치에서 오류.
    assert_eq!(at("@track T ch1\nc4~-10"), (2, 3));
    assert_eq!(at("@track T ch1\nc64~+0:-60"), (2, 4));
    assert_eq!(at("@track T ch1\nc4~13"), (2, 3));
    assert_eq!(at("@track T ch1\nc4~+"), (2, 3));
    assert_eq!(at("@track T ch1\nc4~+1:"), (2, 3));
}

#[test]
fn strict_mode_rejects_what_lenient_warns() {
    let cases: [(&str, (usize, usize)); 6] = [
        // 모르는 토큰(다른 방언의 명령).
        ("@track T ch1\nt120 c4", (2, 1)),
        ("@track T ch1\nc4 % d4", (2, 4)),
        ("@foo bar\n@track T ch1\nc", (1, 1)),
        // 마디 길이 불일치(결정 Q5).
        ("@track T ch1\nc4 |", (2, 4)),
        // 타이 뒤가 같은 음이 아님.
        ("@track T ch1\nc4& d4", (2, 3)),
        ("@track T ch1\nc4&", (2, 3)),
    ];
    for (text, pos) in cases {
        let p = ok(text, Mode::Lenient);
        assert_eq!(warning_positions(&p).first(), Some(&pos), "{text}");
        let (l, c, _) = err(text, Mode::Strict);
        assert_eq!((l, c), pos, "{text}");
    }
    // 느슨 모드는 모르는 토큰만 건너뛴다.
    assert_eq!(
        notes(&melody("t120 c4 q8 d4")),
        vec![n(0, 960, 60), n(960, 960, 62)]
    );
    // 타이가 맞지 않으면 따로 둔다.
    assert_eq!(
        notes(&melody("c4& d4")),
        vec![n(0, 960, 60), n(960, 960, 62)]
    );
    // ~ 보정은 느슨 모드에서 경고 없이 받고, 엄격 모드에서는 오류다.
    let p = ok("@track T ch1\nc4~+1", Mode::Lenient);
    assert!(p.warnings().is_empty());
    let (l, c, _) = err("@track T ch1\nc4~+1", Mode::Strict);
    assert_eq!((l, c), (2, 3));
    // 마디가 맞으면 엄격 모드도 받는다.
    let c = one_track("@time 3/4\n@track T ch1\nc4 d e | f2. |", Mode::Strict);
    assert_eq!(c.length(), 5760);
}

#[test]
fn lyrics_chord_tie_order() {
    // LYR-2: 음 토큰 바로 뒤.
    assert_eq!(
        notes(&melody("l8 c\"사\" d\"랑\" e4\"-\" f4\"해\"")),
        vec![
            nl(0, 480, 60, "사"),
            nl(480, 480, 62, "랑"),
            nl(960, 960, 64, "-"),
            nl(1920, 960, 65, "해"),
        ]
    );
    // 결정 Q6: 화음 가사는 ] 뒤 길이 다음, 가장 높은 음에 저장.
    assert_eq!(
        notes(&melody("[g c e]4\"사\"")),
        vec![n(0, 960, 60), n(0, 960, 64), nl(0, 960, 67, "사")]
    );
    // 순서 음이름 변화표 길이 점 ~ "가사" &.
    assert_eq!(
        notes(&melody("c+8.~+13:-1\"사\"&c+8")),
        vec![nl(13, 1187, 61, "사")]
    );
    assert_eq!(notes(&melody("c8\"사\"&c8")), vec![nl(0, 960, 60, "사")]);
    // 화음의 타이는 같은 음 집합과 잇는다.
    assert_eq!(
        notes(&melody("[c e]4&[e c]4 |")),
        vec![n(0, 1920, 60), n(0, 1920, 64)]
    );
    // 따옴표 안의 공백·기호는 그대로, \"와 \\는 이스케이프.
    assert_eq!(
        notes(&melody(r#"c"a \"b\" c\\d #|~&""#)),
        vec![nl(0, 960, 60, r#"a "b" c\d #|~&"#)]
    );
    // 순서가 다르면 받지 않는다: 가사 뒤 ~는 모르는 토큰.
    let p = ok("@track T ch1\nc8\"사\"~+13", Mode::Lenient);
    assert_eq!(warning_positions(&p).first(), Some(&(2, 6)));
    assert_eq!(notes(p.tracks()[0].clip()), vec![nl(0, 480, 60, "사")]);
    // 떨어진 가사는 붙지 않는다.
    let p = ok("@track T ch1\nc \"x\"", Mode::Lenient);
    assert_eq!(warning_positions(&p), vec![(2, 3)]);
    assert_eq!(notes(p.tracks()[0].clip()), vec![n(0, 960, 60)]);
    // 타이로 이어지는 쪽에 가사가 있으면 어느 쪽인지 정할 수 없어 오류.
    assert_eq!(err("@track T ch1\nc4&c4\"x\"", Mode::Lenient).0, 2);
    // 닫히지 않은 따옴표.
    let (l, c, _) = err("@track T ch1\nc\"abc", Mode::Lenient);
    assert_eq!((l, c), (2, 2));
}

#[test]
fn octave_reference_o4_c_is_60() {
    // 결정 Q1(내보내기 규칙): o4 c = 60. 옥타브를 적지 않으면 o4다.
    assert_eq!(notes(&melody("o4 c")), vec![n(0, 960, 60)]);
    assert_eq!(notes(&melody("c")), vec![n(0, 960, 60)]);
    assert_eq!(notes(&melody("o0 c")), vec![n(0, 960, 12)]);
    assert_eq!(notes(&melody("o0 c-")), vec![n(0, 960, 11)]);
    assert_eq!(notes(&melody("o9 g")), vec![n(0, 960, 127)]);
    assert_eq!(notes(&melody("> c")), vec![n(0, 960, 72)]);
    assert_eq!(notes(&melody("O3 < C")), vec![n(0, 960, 36)]);
    // 옥타브 명령은 스티키다.
    assert_eq!(
        notes(&melody("o5 c > c < < c")),
        vec![n(0, 960, 72), n(960, 960, 84), n(1920, 960, 60)]
    );
    // o0 아래는 < 로만 간다(MIDI 0–11).
    assert_eq!(notes(&melody("o0 < c")), vec![n(0, 960, 0)]);
    let at = |t: &str| {
        let (l, c, _) = err(t, Mode::Lenient);
        (l, c)
    };
    // 범위 밖 음은 음 위치에서 오류.
    assert_eq!(at("@track T ch1\no9 g+"), (2, 4));
    assert_eq!(at("@track T ch1\no9 b"), (2, 4));
    assert_eq!(at("@track T ch1\no0 < c-"), (2, 6));
    assert_eq!(at("@track T ch1\no10 c"), (2, 1));
    assert_eq!(at("@track T ch1\no c"), (2, 1));
}

#[test]
fn parse_is_linear_in_input_size() {
    const UNIT: &str =
        "o4 c8 d16. e32~+3 [c e g]4\"가\" n(c d e) r8 > f+2 < < g-1 o4 v90 l16 a b\n";
    let text = |reps: usize| format!("@track T ch1\n{}", UNIT.repeat(reps));
    let time = |t: &str| {
        (0..3)
            .map(|_| {
                let start = Instant::now();
                let p = parse(t, Mode::Lenient);
                let elapsed = start.elapsed();
                assert!(p.is_ok());
                elapsed
            })
            .min()
            .unwrap_or(Duration::ZERO)
    };
    let small = text(1_000);
    let large = text(10_000);
    let _ = time(&small);
    let (ts, tl) = (time(&small), time(&large));
    let ratio = tl.as_secs_f64() / ts.as_secs_f64().max(1e-9);
    assert!(ratio < 30.0, "10배 입력에 {ratio:.1}배 ({ts:?} → {tl:?})");

    // 여러 성부가 섞여도 같다(노트 삽입이 정렬 위치를 찾느라 제곱이 되지 않는다).
    let voices = |reps: usize| {
        let mut s = String::new();
        for _ in 0..reps {
            s.push_str("@track T ch1\nc4 d4 e4\n");
        }
        s
    };
    let (ts, tl) = (time(&voices(1_000)), time(&voices(10_000)));
    let ratio = tl.as_secs_f64() / ts.as_secs_f64().max(1e-9);
    assert!(ratio < 30.0, "성부 10배에 {ratio:.1}배 ({ts:?} → {tl:?})");
}

#[test]
fn headers_and_track_attributes() {
    let p = ok(
        "@tempo 120.5\n@time 6/8\n@key Db major\n@track \"Lead Vox\" ch3 prog5 bank300\nc",
        Mode::Strict,
    );
    let header = p.header();
    // 60,000,000 / 120.5 = 497925.31… → 497925 µs.
    assert_eq!(header.tempo().map(Tempo::micros_per_quarter), Some(497_925));
    assert_eq!(header.time(), Some(TimeSignature::new(6, 8).unwrap()));
    assert_eq!(header.key(), Some(Key::Major(PitchClass::new(1).unwrap())));
    let track = &p.tracks()[0];
    assert_eq!(track.name(), "Lead Vox");
    assert_eq!(track.channel().get(), 3);
    assert_eq!(track.program().map(keyscore_core::Program::get), Some(5));
    assert_eq!(track.bank().map(keyscore_core::Bank::get), Some(300));

    // 정수 BPM은 Tempo::from_bpm과 같다. 대소문자 무시.
    let p = ok("@TEMPO 7\n@KEY f# MINOR\n@track a CH16\nc", Mode::Lenient);
    assert_eq!(p.header().tempo(), Some(Tempo::from_bpm(7).unwrap()));
    assert_eq!(
        p.header().key(),
        Some(Key::Minor(PitchClass::new(6).unwrap()))
    );
    assert_eq!(p.tracks()[0].channel().get(), 16);
    assert_eq!(p.tracks()[0].program(), None);
    assert_eq!(p.tracks()[0].bank(), None);

    // @key none은 「적지 않음」과 다르다.
    let p = ok("@key none\n@track a ch1\nc", Mode::Lenient);
    assert_eq!(p.header().key(), Some(Key::None));
    let p = ok("@track a ch1\nc", Mode::Lenient);
    assert_eq!(*p.header(), keyscore_text::Header::default());
    let p = ok("@track \"a \\\"q\\\"\" ch1 bank0\nc", Mode::Lenient);
    assert_eq!(p.tracks()[0].name(), "a \"q\"");
    assert_eq!(p.tracks()[0].bank().map(keyscore_core::Bank::get), Some(0));

    let at = |text: &str| {
        let (line, column, _) = err(text, Mode::Lenient);
        (line, column)
    };
    assert_eq!(at("@time 3/5\n@track a ch1\nc"), (1, 7));
    assert_eq!(at("@tempo 0\n@track a ch1\nc"), (1, 8));
    assert_eq!(at("@key H major\n@track a ch1\nc"), (1, 6));
    assert_eq!(at("@track a ch17\nc"), (1, 10));
    assert_eq!(at("@track a ch1 prog128\nc"), (1, 14));
    assert_eq!(at("@track a ch1 bank16384\nc"), (1, 14));
    assert_eq!(at("@track a\nc"), (2, 1));
    // 헤더는 첫 @track 앞에만, 한 번씩만.
    assert_eq!(at("@track a ch1\nc\n@tempo 90"), (3, 1));
    assert_eq!(at("@tempo 90\n@tempo 91\n@track a ch1\nc"), (2, 1));
    // @track 밖의 음, 내용이 없는 블록.
    assert_eq!(at("c d e"), (1, 1));
    assert_eq!(at("@track a ch1\n"), (1, 1));
}

#[test]
fn offset_suffix_is_capped_in_both_modes() {
    // 시작 보정은 ±3840틱, 길이 보정은 ±u32::MAX틱까지(S0-T5 리뷰 1 지적 3).
    assert_eq!(notes(&melody("r1 c~-3840")), vec![n(0, 960, 60)]);
    assert_eq!(notes(&melody("c~+3840")), vec![n(3840, 960, 60)]);
    assert_eq!(
        notes(&melody("c64~+0:+4294967235")),
        vec![n(0, u32::MAX, 60)]
    );
    for text in [
        "@track T ch1\nc~+3841",
        "@track T ch1\nr1 r1 c~-3841",
        "@track T ch1\nc~+9000000000000000000",
        "@track T ch1\nc~+0:+4294967296",
        "@track T ch1\nc~+0:-4294967296",
    ] {
        let (line, column, _) = err(text, Mode::Lenient);
        assert_eq!(line, 2, "{text}");
        assert!(
            text.lines().nth(1).unwrap().chars().nth(column - 1) == Some('~'),
            "{text}"
        );
        // 엄격 모드는 ~ 자체를 거부한다.
        assert!(parse(text, Mode::Strict).is_err());
    }
}
