//! S0-T5 적대적 리뷰 9회차(리뷰 8 수정 `71f136a`·`7d98c26` 뒤 확인 리뷰)의 깨뜨리기 시도.
//! 공개 API만 쓴다.
//!
//! 이번 회차는 깨뜨리지 못했다. 아래 테스트는 모두 통과하며, 경계를 확인하는 회귀 테스트로 남긴다
//! (`docs/reports/s0-t5/review-9.md` 3절).

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use keyscore_core::{Channel, Clip, Note, Pitch, Tick, Velocity};
use keyscore_text::{ExportTrack, Header, Mode, Parsed, export, parse};

/// 겹점 온음표의 틱.
const DDW: u64 = 6720;
type View = (u64, u64, u8, Option<String>);

/// 파서가 받는 표기 길이 21개(분음표 1–64 × 점 없음·점·겹점)와 그 표기.
fn notations() -> Vec<(u64, String)> {
    [1u64, 2, 4, 8, 16, 32, 64]
        .iter()
        .flat_map(|d| {
            let b = 3840 / d;
            [
                (b, format!("{d}")),
                (b * 3 / 2, format!("{d}.")),
                (b * 7 / 4, format!("{d}..")),
            ]
        })
        .collect()
}

fn view(c: &Clip) -> Vec<View> {
    let mut v: Vec<View> = c
        .notes()
        .iter()
        .map(|n| {
            (
                n.start().get(),
                n.end().min(c.length()) - n.start().get(),
                n.pitch().get(),
                n.lyric().map(str::to_owned),
            )
        })
        .collect();
    v.sort();
    v
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

fn single_note_export(length: u64) -> String {
    let mut c = Clip::new(Tick::ZERO, length).unwrap();
    c.insert_note(
        Note::new(
            Tick::ZERO,
            u32::try_from(length).unwrap(),
            Pitch::new(60).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    export(
        &Header::default(),
        &[ExportTrack {
            name: "T",
            channel: Channel::new(1).unwrap(),
            program: None,
            bank: None,
            clip: &c,
        }],
    )
}

/// 내보낸 텍스트의 `~` 시작 보정이 모두 −480..=+59 안인지.
fn start_offsets_in_range(text: &str) -> Result<(), String> {
    for tok in text.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']')) {
        if let Some(i) = tok.find('~') {
            let rest = &tok[i + 1..];
            let end = rest.find([':', '"', '&']).unwrap_or(rest.len());
            let so: i64 = rest[..end].parse().map_err(|e| format!("{tok}: {e}"))?;
            if !(-480..=59).contains(&so) {
                return Err(format!("시작 보정 범위 밖: {tok}"));
            }
        }
    }
    Ok(())
}

/// 표기 길이 21개로 `limit`까지 15틱 배수 길이를 나누는 가장 적은 조각 수(전수 동적 계획법).
/// 칸 `v / 15`. 나눌 수 없으면 `u32::MAX`.
fn fewest_pieces_table(limit: u64) -> Vec<u32> {
    let lens: Vec<u64> = notations().into_iter().map(|(t, _)| t).collect();
    let size = usize::try_from(limit / 15).unwrap() + 1;
    let mut best = vec![u32::MAX; size];
    best[0] = 0;
    for i in 1..size {
        let v = u64::try_from(i).unwrap() * 15;
        for &l in &lens {
            if l <= v {
                let r = best[usize::try_from((v - l) / 15).unwrap()];
                if r != u32::MAX {
                    best[i] = best[i].min(r + 1);
                }
            }
        }
    }
    best
}

/// 못 깨뜨림(`71f136a`): 일찍 돌려주는 조건 `ticks / 6720 > 16`은 17 × 6720 = 114,240틱부터
/// 참이 된다. 리뷰 8의 전수 테스트는 겹점 온음표 17개까지라 새 분기를 한 점(114,240)에서만 지난다.
/// 여기서는 겹점 온음표 16–20개 사이 15틱 배수 모든 길이(1,793개)를 노트 하나 클립으로 내보내,
/// 최소 조각 수가 16 이하이면 그 수만큼의 타이(`&` = 조각 − 1)로 `~` 없이 엄격 모드로 읽히고,
/// 넘으면 `&` 없이 `~`로 나가 느슨 모드로 왕복하는지 본다. 일찍 돌려주는 분기 바로 앞뒤의 15틱
/// 배수가 아닌 길이(±1..=±14)도 `~`로 나가는지 본다. 조건을 `>=`로 바꾼 사본에서는 107,520틱
/// (16조각)에서 실패한다.
#[test]
fn tie_export_matches_fewest_pieces_across_early_return_from_16_to_20_double_dotted_wholes() {
    let limit = 20 * DDW;
    let best = fewest_pieces_table(limit);
    let mut lengths: Vec<u64> = (16 * DDW..=limit).step_by(15).collect();
    for d in 1..15 {
        lengths.push(17 * DDW + d);
        lengths.push(17 * DDW - d);
        lengths.push(16 * DDW + d);
    }
    for t in lengths {
        let out = single_note_export(t);
        let fewest = if t % 15 == 0 {
            best[usize::try_from(t / 15).unwrap()]
        } else {
            u32::MAX
        };
        if fewest <= 16 {
            let ties = u32::try_from(out.matches('&').count()).unwrap();
            assert_eq!(ties + 1, fewest, "{t}틱\n{out}");
            assert!(!out.contains('~'), "{t}틱\n{out}");
            let q = parse(&out, Mode::Strict).unwrap_or_else(|e| panic!("{t}틱: {e}\n{out}"));
            assert_eq!(q.tracks()[0].clip().notes()[0].end(), t, "{out}");
        } else {
            assert!(!out.contains('&'), "{t}틱\n{out}");
            assert!(out.contains('~'), "{t}틱\n{out}");
            let q = parse(&out, Mode::Lenient).unwrap_or_else(|e| panic!("{t}틱: {e}\n{out}"));
            assert_eq!(q.tracks()[0].clip().notes()[0].end(), t, "{out}");
            assert_eq!(q.tracks()[0].clip().length(), t, "{out}");
        }
    }
}

fn best_export_time(text: &str) -> Duration {
    let p = parse(text, Mode::Lenient).unwrap_or_else(|e| panic!("{e}\n{text}"));
    (0..3)
        .map(|_| {
            let t = Instant::now();
            let out = export_parsed(&p);
            let d = t.elapsed();
            assert!(!out.is_empty());
            d
        })
        .min()
        .unwrap()
}

/// 못 깨뜨림(`71f136a`): 리뷰 8 지적 1의 회귀가 노트 하나 말고 다른 경로에도 남았는지. u32 끝까지
/// 늘린 길이를 화음, 시작 보정이 붙은 음, 뒤 음이 음수 시작 보정으로 겹치는 음, 셋잇단 원소와 그
/// 이웃, 같은 이름 성부 블록 20개에 넣고 같은 꼴의 짧은 길이 보정과 시간을 비교한다. 측정(debug)은
/// 모두 짧은 쪽의 다섯 배 안이었다. 클립 끝 채우기는 쉼표가 출력 자체라 이 회귀와 무관하다.
#[test]
fn export_time_stays_flat_for_u32_long_lengths_in_chords_offsets_triplets_and_blocks() {
    let n = 100;
    let shapes = [
        "[ceg]~+0:+{L} ",
        "c~+30:+{L} ",
        "c1~+0:+{L} c~-30 ",
        "n(c~+0:+{L} e g) ",
        "n(c e g) c~+0:+{L} ",
    ];
    for shape in shapes {
        let huge = format!(
            "@track A ch1\n{}",
            shape.replace("{L}", "4294000000").repeat(n)
        );
        let small = format!("@track A ch1\n{}", shape.replace("{L}", "4294").repeat(n));
        let (h, s) = (best_export_time(&huge), best_export_time(&small));
        assert!(
            h <= s * 10 + Duration::from_millis(50),
            "{shape}: 긴 길이 {h:?}, 짧은 길이 {s:?}"
        );
    }
    let blocks = |l: &str| {
        let mut t = String::new();
        for k in 0..20 {
            let _ = writeln!(
                t,
                "@track A ch1\n{}{}",
                "r ".repeat(k),
                format!("c~+0:+{l} ").repeat(10)
            );
        }
        t
    };
    let (h, s) = (
        best_export_time(&blocks("4294000000")),
        best_export_time(&blocks("4294")),
    );
    assert!(
        h <= s * 10 + Duration::from_millis(50),
        "같은 이름 블록: 긴 길이 {h:?}, 짧은 길이 {s:?}"
    );
}

/// 같은 이름 블록 `k`개. 블록마다 `l1` 뒤 온음표 쉼표 `rests`개 다음 서로 다른 길이의 노트 하나
/// (표기 21개, 온음표·점 온음표 타이 21개씩).
fn same_name_blocks(k: usize, rests: usize) -> String {
    let lens = notations();
    let mut names: Vec<String> = lens.iter().map(|(_, s)| format!("c{s}")).collect();
    names.extend(lens.iter().map(|(_, s)| format!("c1&c{s}")));
    names.extend(lens.iter().map(|(_, s)| format!("c1.&c{s}")));
    let mut t = String::new();
    for s in names.iter().take(k) {
        let _ = writeln!(t, "@track A ch1\nl1 {}{s}", "r ".repeat(rests));
    }
    t
}

/// 못 깨뜨림(`7d98c26`): 알려진 예외(결정 A의 겹쳐 적기)가 리뷰 8의 한 사례(블록 17개, 쉼표 400개)
/// 밖에서도 결정이 약속한 성질을 지키는지. 블록 17–63개 × 쉼표 100·400·1,000개에서 겹쳐 적기가
/// 일어나면 `~`는 모두 64분음표 명목 길이의 겹쳐 적기 토큰(`c64~`, 화음이면 `[…]64~`)뿐이고, 느슨 모드로 경고 없이
/// 왕복하며(트랙 수, 노트, 클립 길이), 재내보내기가 같고, 시작 보정이 −480..=+59 안이다.
/// 예산이 남는 쉼표 5개에서는 블록 63개(성부 57개)도 `~` 없이 엄격 모드로 왕복한다. 곧 예외는
/// 예산이 바닥난 꼴로만 좁혀져 있다.
#[test]
fn export_folds_only_over_budget_and_round_trips_leniently_for_17_to_63_same_name_blocks() {
    for k in [17usize, 22, 30, 45, 63] {
        let src = same_name_blocks(k, 5);
        let p = parse(&src, Mode::Strict).unwrap();
        let out = export_parsed(&p);
        assert!(!out.contains('~'), "블록 {k}개, 쉼표 5개\n{out}");
        let q = parse(&out, Mode::Strict).unwrap_or_else(|e| panic!("{e}\n{out}"));
        assert_eq!(view(p.tracks()[0].clip()), view(q.tracks()[0].clip()));
        assert_eq!(export_parsed(&q), out);

        for rests in [100usize, 400, 1000] {
            let src = same_name_blocks(k, rests);
            let p = parse(&src, Mode::Strict).unwrap();
            let out = export_parsed(&p);
            for tok in out.split_whitespace().filter(|w| w.contains('~')) {
                assert!(
                    tok.split_once('~').is_some_and(|(h, _)| h.ends_with("64")),
                    "블록 {k}개, 쉼표 {rests}개: {tok}"
                );
            }
            let q = parse(&out, Mode::Lenient).unwrap_or_else(|e| panic!("{e}\n{out}"));
            assert!(q.warnings().is_empty(), "{:?}", q.warnings());
            assert_eq!(p.tracks().len(), q.tracks().len());
            for (a, b) in p.tracks().iter().zip(q.tracks()) {
                assert_eq!(view(a.clip()), view(b.clip()), "블록 {k}개, 쉼표 {rests}개");
                assert_eq!(a.clip().length(), b.clip().length());
            }
            assert_eq!(export_parsed(&q), out, "블록 {k}개, 쉼표 {rests}개");
            start_offsets_in_range(&out).unwrap_or_else(|e| panic!("{e}"));
        }
    }
}
