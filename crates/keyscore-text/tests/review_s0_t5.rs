//! S0-T5 적대적 리뷰 1회차의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5/review-1.md` 7절), 통과하는 테스트는
//! 깨뜨리지 못한 시도를 회귀 테스트로 남긴 것이다.

use keyscore_core::{Channel, Clip, Note, Pitch, Tick, TimeSignature, Velocity};
use keyscore_text::{ExportTrack, Header, Mode, export, parse};

type View = (u64, u32, u8, Option<String>);

fn clip(length: u64, ns: &[(u64, u32, u8, Option<&str>)]) -> Clip {
    let mut c = Clip::new(Tick::ZERO, length).unwrap();
    for &(s, l, p, y) in ns {
        let n = Note::new(
            Tick::new(s),
            l,
            Pitch::new(p).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap();
        c.insert_note(match y {
            Some(y) => n.with_lyric(y),
            None => n,
        })
        .unwrap();
    }
    c
}

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

fn export_one(h: &Header, ch: u8, c: &Clip) -> String {
    export(
        h,
        &[ExportTrack {
            name: "T",
            channel: Channel::new(ch).unwrap(),
            program: None,
            bank: None,
            clip: c,
        }],
    )
}

/// export → parse가 노트를 그대로 돌려주고, 경고가 없고, 다시 내보내면 같은 문자열인지.
/// 다시 읽은 클립을 돌려준다.
fn round_trip(h: &Header, ch: u8, c: &Clip) -> Clip {
    let text = export_one(h, ch, c);
    let p = parse(&text, Mode::Lenient).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert!(p.warnings().is_empty(), "{:?}\n{text}", p.warnings());
    let got = p.tracks()[0].clip().clone();
    assert_eq!(view(&got), view(c), "{text}");
    assert_eq!(export_one(p.header(), ch, &got), text, "재내보내기 불안정");
    got
}

// ---- 깨뜨린 시도 ----

/// 결정(2026-10-12-ktext-implementation-details) 「차이는 15틱 미만」: 쉼표로 닿지 않는 클립 길이를
/// 줄일 때의 차이. 150틱 아래에서는 성립하지 않는다(135는 쉼표 합으로 못 만들고, 60틱 미만의
/// 커서 앞지름은 쉼표를 더할 자리가 없다).
#[test]
fn clip_length_shrink_stays_below_15_ticks_for_short_clips() {
    let h = Header::default();
    for (length, notes) in [
        (140u64, vec![]),
        (149, vec![]),
        (55, vec![(0u64, 3u32, 60u8, None)]),
    ] {
        let c = clip(length, &notes);
        let got = round_trip(&h, 1, &c);
        let diff = c.length().abs_diff(got.length());
        assert!(
            diff < 15,
            "클립 {length}틱 → {}틱 (차이 {diff})\n{}",
            got.length(),
            export_one(&h, 1, &c)
        );
    }
}

/// 범위 밖 ch·prog·bank 값의 오류 메시지는 쓴 값을 보여야 한다. 지금은 u8/u16 최대로 잘린 값을
/// 보여 준다(`ch300` → 「…: 255」). AI-6 재요청에 그대로 들어가는 메시지라 틀린 값이 된다.
#[test]
fn out_of_range_channel_program_bank_errors_show_written_value() {
    for (text, written) in [
        ("@track A ch300\nc", "300"),
        ("@track A ch1 prog300\nc", "300"),
        ("@track A ch1 bank70000\nc", "70000"),
    ] {
        let e = parse(text, Mode::Strict).unwrap_err();
        let msg = e.diagnostic().message().to_owned();
        assert!(msg.contains(written), "{text:?} → {msg}");
    }
}

// ---- 깨뜨리지 못한 시도 ----

#[test]
fn same_pitch_overlaps_and_exact_duplicates_round_trip() {
    let c = clip(
        4000,
        &[
            (0, 960, 60, None),
            (0, 960, 60, None),
            (0, 960, 60, Some("a")),
            (0, 960, 60, Some("a")),
            (480, 960, 60, None),
            (0, 1920, 60, None),
            (0, 960, 64, Some("b")),
            (10, 3, 60, None),
        ],
    );
    round_trip(&Header::default(), 1, &c);
    // 화음 안 중복 음
    let p = parse("@track T ch1\n[c c e]4\"x\"", Mode::Lenient).unwrap();
    round_trip(&Header::default(), 1, p.tracks()[0].clip());
}

#[test]
fn lengths_near_u32_max_round_trip() {
    for l in [u32::MAX, u32::MAX - 1, 3840 * 20_000, 3840 * 20_000 + 1440] {
        let c = clip(u64::from(l) + 7, &[(0, l, 127, Some("x"))]);
        let got = round_trip(&Header::default(), 1, &c);
        assert!(got.length().abs_diff(c.length()) < 15);
    }
    let c = clip(
        u64::from(u32::MAX) * 2,
        &[(u64::from(u32::MAX), u32::MAX, 0, None)],
    );
    round_trip(&Header::default(), 10, &c);
}

#[test]
fn negative_start_correction_near_clip_start() {
    // 50틱 노트는 64분음표(60)로 적혀 커서가 실제 끝을 앞지르고, 다음 노트는 음수 보정을 받는다.
    let c = clip(
        200,
        &[
            (0, 50, 60, None),
            (50, 50, 61, None),
            (100, 7, 0, None),
            (107, 1, 127, None),
        ],
    );
    let text = export_one(&Header::default(), 1, &c);
    assert!(text.contains("~-"), "{text}");
    round_trip(&Header::default(), 1, &c);
}

#[test]
fn lyrics_with_quotes_backslashes_hash_newlines_round_trip() {
    let c = clip(
        3840,
        &[
            (0, 480, 60, Some("\"")),
            (480, 480, 62, Some("\\")),
            (960, 480, 64, Some("a\\\"b\\")),
            (1440, 480, 65, Some("# 주석 아님")),
            (1920, 480, 67, Some("줄\n바꿈\r\n")),
            (2400, 480, 69, Some("& | ~+1 [ ] n( @track X ch1")),
        ],
    );
    round_trip(&Header::default(), 1, &c);
}

#[test]
fn drum_labelled_and_unlabelled_notes_mixed_round_trip() {
    let c = clip(
        3840 * 2,
        &[
            (0, 240, 36, None),
            (0, 240, 75, Some("x")),
            (0, 240, 0, None),
            (240, 160, 35, None),
            (400, 160, 42, None),
            (560, 160, 127, None),
            (720, 5000 - 720, 81, None),
            (720, 3840 * 2 - 720, 53, None),
        ],
    );
    round_trip(&Header::default(), 10, &c);
}

#[test]
fn bar_lines_with_voices_and_three_four_time_have_no_warnings() {
    let h = Header::new(None, Some(TimeSignature::new(3, 4).unwrap()), None);
    // 성부 둘(겹친 지속음), 셋잇단, 타이 사슬, 그리드 밖 노트
    let c = clip(
        2880 * 4,
        &[
            (0, 960, 60, None),
            (960, 320, 62, None),
            (1280, 320, 64, None),
            (1600, 320, 65, None),
            (1920, 2880 * 2 + 960, 67, None),
            (0, 2880 * 3, 48, None),
            (9600, 13, 72, None),
        ],
    );
    round_trip(&h, 1, &c);
    // 성부마다 마디선이 맞으면 경고가 없다.
    let p = parse(
        "@time 3/4\n@track A ch1\nc4 c4 c4 | c2. |\n@track A ch1\nc2. | c4 c4 c4 |",
        Mode::Strict,
    )
    .unwrap();
    assert!(p.warnings().is_empty());
}

#[test]
fn sixty_fourth_note_triplets_round_trip() {
    let c = clip(
        480,
        &[
            (0, 40, 60, None),
            (40, 40, 62, Some("a")),
            (80, 40, 64, None),
            (120, 40, 65, None),
            (160, 40, 67, None),
            (200, 40, 69, None),
            (270, 40, 60, None),
            (310, 40, 61, None),
            (350, 40, 62, None),
        ],
    );
    let text = export_one(&Header::default(), 1, &c);
    assert!(text.contains("n("), "{text}");
    round_trip(&Header::default(), 1, &c);
}

#[test]
fn grammar_decisions_hold() {
    let notes = |t: &str| view(parse(t, Mode::Lenient).unwrap().tracks()[0].clip());
    // o4 c = 60, Q1(2026-10-16 개정): 음 바로 뒤 #는 올림표, 주석은 // 뒤
    assert_eq!(notes("@track A ch1\no4 c")[0].2, 60);
    assert_eq!(
        notes("@track A ch1\nc#4 d4// e\nf"),
        vec![
            (0, 960, 61, None),
            (960, 960, 62, None),
            (1920, 960, 65, None)
        ]
    );
    // Q2: 각 음 길이의 2/3, ) 뒤 숫자 오류, 정확히 3개
    assert_eq!(
        notes("@track A ch1\nn(c8 d16 e16)")[1],
        (320, 160, 62, None)
    );
    assert!(parse("@track A ch1\nn(c d e)8", Mode::Lenient).is_err());
    assert!(parse("@track A ch1\nn(c d [e g] a)", Mode::Lenient).is_err());
    // Q4: 공백 생략, 드럼 최장 일치, 멜로디의 bd는 b d
    assert_eq!(notes("@track A ch1\nbd").len(), 2);
    assert_eq!(notes("@track D ch10\nrbrdrs").len(), 3);
    // Q5: 마디선은 틱에 영향 없음
    assert_eq!(notes("@track A ch1\nc2 | c2"), notes("@track A ch1\nc2 c2"));
    // Q6: 순서 고정, 엄격 모드는 ~·모르는 토큰·마디 불일치를 거부
    assert_eq!(
        notes("@track A ch1\nc8~+13:-20\"x\"&c8"),
        vec![(13, 947, 60, Some("x".to_owned()))]
    );
    for t in [
        "@track A ch1\nc~+1",
        "@track A ch1\nt120 c",
        "@track A ch1\nc2 |",
        "@track A ch1\nc&",
    ] {
        assert!(parse(t, Mode::Strict).is_err(), "{t}");
    }
}

#[test]
fn pathological_inputs_do_not_panic() {
    for t in [
        "@track A ch1\nc~+9223372036854775807",
        "@track A ch1\nc~-9223372036854775808",
        "@track A ch1\nc~+0:-9223372036854775807",
        "@track A ch1\nc~+0:+9223372036854775807",
        "@track A ch1\no99999999999999999999 c",
        "@track A ch1\nv99999999999999999999 c",
        "@tempo 999999999999.999999999999\n@track A ch1\nc",
        "@tempo 0.000000000001\n@track A ch1\nc",
        "@time 255/256\n@track A ch1\nc |",
        "@track A ch1\nn(c d e&)",
        "@track A ch1\n[c\n",
        "@track \"A\n",
    ] {
        let _ = parse(t, Mode::Lenient);
        let _ = parse(t, Mode::Strict);
    }
}
