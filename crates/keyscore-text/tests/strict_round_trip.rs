//! 엄격 모드로 읽히는 텍스트는 내보낸 뒤에도 엄격 모드로 읽힌다(REQ 5.14 「그리드에 안 맞는 노트만
//! `~`」, S0-T5 리뷰 6 지적 2). 공개 API만 쓴다.
//!
//! 생성기가 만들지 않는 엄격 모드 꼴(알려진 한계): 쉼표가 낀 셋잇단(`n(c r e)`), 셋잇단 원소에서
//! 시작하는 타이, 16조각으로 적을 수 없는 타이, 같은 이름의 성부 블록들이 셋잇단 쉼표 간격으로
//! 엇갈리거나 같은 (시작, 길이, 음높이) 셋잇단 원소를 나누는 꼴(내보내기는 같은 시작·길이 노트를 한
//! 화음으로 묶는다). 내보내기는 이들을 `~`로 적는다. 그래서 셋잇단을 넣는 무작위 텍스트는 블록마다
//! 다른 이름(트랙마다 한 갈래)을 쓰고, 같은 이름 블록은 셋잇단 없이 따로 만든다(리뷰 7 지적 1).
//! 셋잇단이 든 여러 성부 블록은 고친 꼴을 결정적 테스트로 본다.

mod common;

use std::fmt::Write as _;

use common::notes;
use keyscore_text::{ExportTrack, Mode, Parsed, export, parse};
use proptest::prelude::*;

fn length() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec!["", "", "1", "2", "4", "8", "16", "32", "64"]),
        prop::sample::select(vec!["", "", ".", ".."]),
    )
        .prop_map(|(d, dots)| {
            if d.is_empty() {
                String::new()
            } else {
                format!("{d}{dots}")
            }
        })
}

fn head(drum: bool) -> BoxedStrategy<String> {
    if drum {
        prop::sample::select(vec!["bd", "sd", "ch", "oh", "cb", "n20", "n75"])
            .prop_map(str::to_owned)
            .boxed()
    } else {
        (
            prop::sample::select(vec!["c", "d", "e", "f", "g", "a", "b"]),
            prop::sample::select(vec!["", "", "+", "-", "#"]),
        )
            .prop_map(|(n, a)| format!("{n}{a}"))
            .boxed()
    }
}

fn lyric() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => Just(String::new()),
        1 => prop::sample::select(vec!["\"가\"", "\"a b\"", "\"\\\"\""]).prop_map(str::to_owned),
    ]
}

/// 음 하나 또는 화음 하나(접미 포함). 타이면 같은 머리를 한 번 더 적는다.
fn sounding(drum: bool) -> impl Strategy<Value = String> {
    let single = head(drum);
    let chord =
        prop::collection::vec(head(drum), 2..4).prop_map(|hs| format!("[{}]", hs.join(" ")));
    (
        prop_oneof![3 => single, 1 => chord],
        length(),
        lyric(),
        prop::option::weighted(0.25, (length(), prop::option::weighted(0.3, length()))),
    )
        .prop_map(|(h, len, ly, tie)| match tie {
            None => format!("{h}{len}{ly}"),
            Some((l2, None)) => format!("{h}{len}{ly}&{h}{l2}"),
            Some((l2, Some(l3))) => format!("{h}{len}{ly}&{h}{l2}&{h}{l3}"),
        })
}

fn token(drum: bool, triplets: bool) -> BoxedStrategy<String> {
    let triplet = prop::collection::vec(
        (
            prop_oneof![
                3 => head(drum),
                1 => prop::collection::vec(head(drum), 2..3).prop_map(|hs| format!("[{}]", hs.join(" "))),
            ],
            length(),
        ),
        3,
    )
    .prop_map(|xs| {
        let parts: Vec<String> = xs.into_iter().map(|(h, l)| format!("{h}{l}")).collect();
        format!("n({})", parts.join(" "))
    });
    let commands = if drum {
        prop::sample::select(vec!["l8", "l16", "l4.", "v90"]).boxed()
    } else {
        prop::sample::select(vec!["l8", "l16", "l4.", "v90", "<", ">", "o3", "o5", "o4"]).boxed()
    };
    prop_oneof![
        6 => sounding(drum),
        2 => length().prop_map(|l| format!("r{l}")),
        u32::from(triplets) => triplet,
        1 => commands.prop_map(str::to_owned),
    ]
    .boxed()
}

/// 이름 없는 블록 본문과 채널.
fn block() -> impl Strategy<Value = (u8, String)> {
    (
        any::<bool>(),
        prop::sample::select(vec!["l4", "l8", "l16", "l1", "l2."]),
    )
        .prop_flat_map(|(drum, l)| {
            let ch = if drum { 10 } else { 1 };
            prop::collection::vec(token(drum, true), 1..24)
                .prop_map(move |ts| (ch, format!("{l} {}\n", ts.join(" "))))
        })
}

fn strict_text() -> impl Strategy<Value = String> {
    const NAMES: [&str; 3] = ["A", "B", "\"가 나\""];
    (
        prop::sample::select(vec!["", "@time 3/4\n", "@time 7/8\n", "@tempo 97.5\n"]),
        prop::collection::vec(block(), 1..4),
    )
        .prop_map(|(h, bs)| {
            let mut t = h.to_owned();
            for (i, (ch, body)) in bs.into_iter().enumerate() {
                let _ = write!(t, "@track {} ch{ch}\n{body}", NAMES[i]);
            }
            t
        })
}

/// 같은 이름(같은 채널)의 성부 블록 2–3개. 셋잇단은 넣지 않는다: 알려진 예외(셋잇단이 셋잇단 쉼표
/// 간격으로 엇갈리거나 같은 원소를 나누는 꼴)가 모두 셋잇단 꼴이라서다. 그 밖의 성부 배정(쉼표로
/// 못 채우는 간격에서 새 성부 열기, S0-T5 리뷰 7 지적 1)은 이 생성기가 본다.
fn voice_block_text() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec!["", "@time 3/4\n", "@time 7/8\n"]),
        any::<bool>(),
        prop::collection::vec(
            (
                prop::sample::select(vec!["l4", "l8", "l16", "l1", "l2.", "l64"]),
                prop::collection::vec(any::<prop::sample::Index>(), 1..16),
            ),
            2..4,
        ),
    )
        .prop_flat_map(|(h, drum, blocks)| {
            let ch = if drum { 10 } else { 1 };
            let bodies: Vec<BoxedStrategy<String>> = blocks
                .into_iter()
                .map(|(l, toks)| {
                    prop::collection::vec(token(drum, false), toks.len())
                        .prop_map(move |ts| format!("@track A ch{ch}\n{l} {}\n", ts.join(" ")))
                        .boxed()
                })
                .collect();
            bodies.prop_map(move |bs| format!("{h}{}", bs.concat()))
        })
}

fn export_parsed(p: &Parsed) -> String {
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
    export(p.header(), &tracks)
}

/// 여러 성부 블록(같은 이름)에서 고친 꼴: 셋잇단이 한 성부에 남고, 쉼표로 정확히 닿는 성부를 고르고,
/// 셋잇단 첫 원소가 표기 가능한 길이여도 잇고, 셋잇단 뒤 격자 밖 커서면 새 성부를 열고, 클립 끝을
/// 다른 성부의 커서로 정확히 맞춘다(S0-T5 리뷰 6 지적 2, proptest가 찾은 최소 사례들).
#[test]
fn strict_voice_blocks_export_to_strict_text() {
    for text in [
        "@track A ch1\nl4 n(c c c) n(c1 c c)\n",
        "@track A ch1\nl4 c8.&c4&c2.. n(c c c)\n",
        "@track A ch1\nl4 c8.. c32&c64..\n@track A ch1\nl4 c n(c c c)\n",
        "@track B ch1\nl4 c n(c c c8)\n@track B ch1\nl4 c1 n(c c c) c c\n",
        "@track B ch1\nl4 c1 c&c1 c c\n@track B ch1\nl4 r1. n(c c c8) l4. c c r\n",
        "@track B ch1\nl4 c1 c&c1 c c\n@track B ch1\nl4 r1. n(c c c8) l4. c c n(c2 c c)\n",
        "@track B ch1\nl4 c1 c&c1 c c\n@track B ch1\nl4 r n(c8 c64 c1) c1. r c\n",
        "@track B ch1\nl4 c1 n(c1. c c) c&c&c\n@track B ch1\nl4 c2. c2\n",
        "@track B ch1\nl4 c&c n(c2 c c) c\n@track B ch1\nl4 r1. c\n",
    ] {
        let p = parse(text, Mode::Strict).unwrap();
        let out = export_parsed(&p);
        let q = parse(&out, Mode::Strict)
            .unwrap_or_else(|e| panic!("{e}\n원문:\n{text}\n내보냄:\n{out}"));
        let (mut x, mut y) = (notes(p.tracks()[0].clip()), notes(q.tracks()[0].clip()));
        x.sort();
        y.sort();
        assert_eq!(x, y, "{out}");
        assert_eq!(
            p.tracks()[0].clip().length(),
            q.tracks()[0].clip().length(),
            "{out}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn strict_voice_block_text_exports_to_strict_text(text in voice_block_text()) {
        let Ok(p) = parse(&text, Mode::Strict) else {
            return Ok(());
        };
        let out = export_parsed(&p);
        let q = parse(&out, Mode::Strict);
        prop_assert!(q.is_ok(), "{:?}\n원문:\n{}\n내보냄:\n{}", q.err(), text, out);
        let q = q.unwrap();
        for (a, b) in p.tracks().iter().zip(q.tracks()) {
            let (mut x, mut y) = (notes(a.clip()), notes(b.clip()));
            x.sort();
            y.sort();
            prop_assert_eq!(x, y, "{}", out);
            prop_assert_eq!(a.clip().length(), b.clip().length(), "{}", out);
        }
        prop_assert_eq!(export_parsed(&q), out);
    }

    #[test]
    fn strict_text_exports_to_strict_text(text in strict_text()) {
        // 범위 밖 음 같은 오류가 나는 텍스트는 이 성질의 범위 밖이다.
        let Ok(p) = parse(&text, Mode::Strict) else {
            return Ok(());
        };
        let out = export_parsed(&p);
        let q = parse(&out, Mode::Strict);
        prop_assert!(q.is_ok(), "{:?}\n원문:\n{}\n내보냄:\n{}", q.err(), text, out);
        let q = q.unwrap();
        prop_assert_eq!(q.header(), p.header());
        prop_assert_eq!(q.tracks().len(), p.tracks().len());
        for (a, b) in p.tracks().iter().zip(q.tracks()) {
            let (mut x, mut y) = (notes(a.clip()), notes(b.clip()));
            x.sort();
            y.sort();
            prop_assert_eq!(x, y, "{}", out);
            prop_assert_eq!(a.clip().length(), b.clip().length(), "{}", out);
        }
    }
}
