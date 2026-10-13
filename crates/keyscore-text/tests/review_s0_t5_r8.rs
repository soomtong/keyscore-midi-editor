//! S0-T5 적대적 리뷰 8회차(리뷰 7 수정 `665b9e2`·`d80fc1b`·`030c76a` 뒤 확인 리뷰)의 깨뜨리기 시도.
//! 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5/review-8.md` 7절), 통과하는 테스트는
//! 깨뜨리지 못한 시도를 경계 확인용 회귀 테스트로 남긴 것이다.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use keyscore_core::{Channel, Clip, Note, Pitch, Tick, Velocity};
use keyscore_text::{ExportTrack, Header, Mode, Parsed, export, parse};

/// 겹점 온음표의 틱.
const DDW: u64 = 6720;
type View = (u64, u64, u8, Option<String>);

/// 파서가 받는 표기 길이 21개(분음표 1–64 × 점 없음·점·겹점).
fn notations() -> Vec<u64> {
    [1, 2, 4, 8, 16, 32, 64]
        .iter()
        .flat_map(|d| {
            let b = 3840 / d;
            [b, b * 3 / 2, b * 7 / 4]
        })
        .collect()
}

fn notation_text(ticks: u64) -> String {
    for d in [1u64, 2, 4, 8, 16, 32, 64] {
        let b = 3840 / d;
        for (t, dots) in [(b, ""), (b * 3 / 2, "."), (b * 7 / 4, "..")] {
            if t == ticks {
                return format!("{d}{dots}");
            }
        }
    }
    panic!("표기 길이가 아니다: {ticks}");
}

struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n.max(1)
    }

    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[usize::try_from(self.below(xs.len() as u64)).unwrap()]
    }
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

fn export_clip(c: &Clip) -> String {
    export(
        &Header::default(),
        &[ExportTrack {
            name: "T",
            channel: Channel::new(1).unwrap(),
            program: None,
            bank: None,
            clip: c,
        }],
    )
}

fn single_note_clip(length: u64) -> Clip {
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
    c
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

/// 엄격 모드로 읽히는 `text`를 내보내고 엄격 모드로 다시 읽어, 노트·클립 길이가 같고 재내보내기가
/// 같은지. 내보낸 텍스트를 돌려준다.
fn strict_round_trip(text: &str) -> Result<String, String> {
    let p = parse(text, Mode::Strict).map_err(|e| format!("원문이 엄격 모드로 안 읽힘: {e}"))?;
    let out = export_parsed(&p);
    let q = parse(&out, Mode::Strict).map_err(|e| format!("{e}\n원문:\n{text}\n내보냄:\n{out}"))?;
    for (a, b) in p.tracks().iter().zip(q.tracks()) {
        if view(a.clip()) != view(b.clip()) || a.clip().length() != b.clip().length() {
            return Err(format!("노트·길이가 다르다\n원문:\n{text}\n내보냄:\n{out}"));
        }
    }
    if export_parsed(&q) != out {
        return Err(format!("재내보내기가 다르다\n{out}"));
    }
    Ok(out)
}

/// 못 깨뜨림(`d80fc1b`): 15틱 배수의 모든 노트 길이(겹점 온음표 17개까지)에서, 내보낸 타이 사슬의
/// 조각 수가 표기 길이 21개로 나눌 수 있는 가장 적은 조각 수와 같고, 그 수가 16 이하이면 `~` 없이
/// 엄격 모드로 다시 읽히며, 넘으면 `~`로 나간다. 단위 테스트(`lengths.rs`)와 달리 공개 API의 출력에서
/// 센다. 16/17조각 경계는 겹점 온음표 16개(107,520틱, 16조각)와 거기에 64분음표를 더한 길이(17조각)다.
#[test]
fn tie_export_uses_fewest_pieces_for_every_length_up_to_17_double_dotted_wholes() {
    let ns = notations();
    let limit = 17 * DDW;
    let size = usize::try_from(limit / 15).unwrap() + 1;
    let mut best = vec![u32::MAX; size];
    best[0] = 0;
    for i in 1..size {
        let v = u64::try_from(i).unwrap() * 15;
        for &n in &ns {
            if n <= v {
                let r = best[usize::try_from((v - n) / 15).unwrap()];
                if r != u32::MAX {
                    best[i] = best[i].min(r + 1);
                }
            }
        }
    }
    let mut checked_17 = false;
    for (i, &b) in best.iter().enumerate().skip(1) {
        let v = u64::try_from(i).unwrap() * 15;
        // 쉼표 없이 노트 하나만 있는 클립이라 다른 성부·보정이 끼지 않는다.
        let text = export_clip(&single_note_clip(v));
        let body: String = text.lines().skip(1).collect::<Vec<_>>().join(" ");
        if b <= 16 {
            let pieces = u32::try_from(body.matches('&').count()).unwrap() + 1;
            assert_eq!(pieces, b, "{v}틱: {text}");
            assert!(!text.contains('~'), "{v}틱: {text}");
            assert!(parse(&text, Mode::Strict).is_ok(), "{v}틱: {text}");
        } else {
            assert!(!body.contains('&') && text.contains('~'), "{v}틱: {text}");
            checked_17 |= v == 16 * DDW + 60;
        }
    }
    assert!(checked_17);
    assert_eq!(best[usize::try_from(16 * DDW / 15).unwrap()], 16);
    assert_eq!(best[usize::try_from((16 * DDW + 60) / 15).unwrap()], 17);
}

/// 못 깨뜨림(`d80fc1b` × `030c76a`): 16조각 타이가 같은 이름의 다른 블록에서, 앞 성부 커서와 쉼표로
/// 못 채우는 간격(30·45·135틱)을 두고 시작해도, 새 성부(`fresh`)가 열려 `~` 없이 엄격 모드로
/// 왕복한다. 새 성부 열기 조건의 「그리드 길이」에 타이 길이(`floor_preferred`)가 들어가는지 본다.
#[test]
fn export_keeps_strict_text_when_16_piece_tie_meets_voice_at_unrestable_gap() {
    let tie16 = format!("{}c1..", "c1..&".repeat(15));
    let tie_mixed = format!("{}c2.&c64..", "c1..&".repeat(14));
    for (first, rest) in [("c64.", "r32"), ("c64", "r64.."), ("c2 c64..", "r2 r16")] {
        for tie in [&tie16, &tie_mixed] {
            let text = format!("@track A ch1\n{first}\n@track A ch1\n{rest} {tie}\n");
            let out = strict_round_trip(&text).unwrap_or_else(|e| panic!("{e}"));
            assert!(!out.contains('~'), "{text}\n{out}");
        }
    }
}

fn random_len(r: &mut Lcg) -> String {
    let d = r.pick(&["", "1", "2", "4", "8", "16", "32", "64", "64", "32"]);
    if d.is_empty() {
        return String::new();
    }
    format!("{d}{}", r.pick(&["", "", ".", ".."]))
}

/// 무작위 엄격 모드 텍스트(셋잇단 없음): 같은 이름 블록 `1..=blocks`개, 3조각 이하 짧은 타이,
/// 8–16조각 긴 타이(겹점·점 온음표), 점·겹점 64분, 화음, 가사, 드럼, 박자표(1/64 포함), 긴 쉼표.
fn random_same_name_strict_text(r: &mut Lcg, blocks: u64) -> String {
    let mut t = r
        .pick(&[
            "",
            "@time 3/4\n",
            "@time 7/8\n",
            "@time 5/16\n",
            "@time 1/64\n",
        ])
        .to_owned();
    let drum = r.below(5) == 0;
    let ch = if drum { 10 } else { 1 };
    for _ in 0..=r.below(blocks) {
        let _ = writeln!(t, "@track A ch{ch}");
        let _ = write!(t, "l{} ", r.pick(&["4", "8", "16", "1", "2.", "64", "64."]));
        for _ in 0..r.below(20) {
            let h = if drum {
                r.pick(&["bd", "sd", "ch"])
            } else {
                r.pick(&["c", "d", "e", "g", "a"])
            };
            match r.below(12) {
                0..=3 => {
                    let ly = if r.below(6) == 0 { "\"x\"" } else { "" };
                    let _ = write!(t, "{h}{}{ly}", random_len(r));
                    for _ in 0..r.below(4).saturating_sub(1) {
                        let _ = write!(t, "&{h}{}", random_len(r));
                    }
                }
                4 | 5 => {
                    let _ = write!(t, "r{}", random_len(r));
                }
                6 => {
                    let other = if drum { "sd" } else { "g" };
                    let _ = write!(t, "[{h} {other}]{}", random_len(r));
                }
                7 => {
                    let count = 8 + r.below(9);
                    for i in 0..count {
                        let piece = r.pick(&["1..", "1..", "1.", "1", "2.."]);
                        let amp = if i + 1 < count { "&" } else { "" };
                        let _ = write!(t, "{h}{piece}{amp}");
                    }
                }
                8 => {
                    for _ in 0..r.below(40) {
                        t.push_str("r1 ");
                    }
                }
                9 if !drum => t.push_str(r.pick(&["<", ">", "o3", "o5"])),
                _ => {
                    let _ = write!(t, "l{}", r.pick(&["4", "8", "16", "64", "32.", "64.."]));
                }
            }
            t.push(' ');
        }
        t.push('\n');
    }
    t
}

/// 못 깨뜨림(`030c76a`): 셋잇단 없는 같은 이름 블록(1–24개)의 엄격 텍스트는 내보낸 뒤 엄격 모드로
/// 다시 읽히고, 노트·클립 길이·재내보내기가 같다. 구현자의 proptest(블록 2–3개, 긴 타이·긴 쉼표
/// 없음)보다 넓게 블록 수를 16 넘게, 타이를 16조각까지, 박자표 1/64를 넣었다. release에서 블록
/// 1–6개 시드 8·9 각 40,000 사례(79,024건 읽힘), 1–12개·1–24개 각 20,000 사례를 돌려 실패 0이었다.
#[test]
fn random_same_name_strict_blocks_without_triplets_export_to_strict_text() {
    let mut r = Lcg(808);
    let mut accepted = 0;
    for case in 0..1500 {
        let blocks = [6, 12, 24][case % 3];
        let text = random_same_name_strict_text(&mut r, blocks);
        if parse(&text, Mode::Strict).is_err() {
            continue;
        }
        accepted += 1;
        if let Err(e) = strict_round_trip(&text) {
            panic!("사례 {case}: {e}");
        }
    }
    assert!(accepted > 1000, "읽힌 사례가 적다: {accepted}");
}

/// 무작위 느슨 모드 텍스트: 같은 이름 블록 1–9개, 시작·길이 보정, 1–59틱 노트, 긴 길이 보정,
/// 압축 쉼표, 타이.
fn random_same_name_lenient_text(r: &mut Lcg) -> String {
    let mut t = r.pick(&["", "@time 3/4\n", "@time 1/64\n"]).to_owned();
    for _ in 0..=r.below(8) {
        let _ = write!(
            t,
            "@track A ch1\nl{} ",
            r.pick(&["4", "64", "64.", "1..", "16"])
        );
        for _ in 0..r.below(30) {
            let h = r.pick(&["c", "d", "e"]);
            match r.below(10) {
                0..=2 => {
                    let _ = write!(t, "{h}{}", random_len(r));
                }
                3 | 4 => {
                    let so = i64::try_from(r.below(8)).unwrap() * 15 - 45;
                    let lo = i64::try_from(r.below(7)).unwrap() * 15 - 59;
                    let _ = write!(t, "{h}{}~{so:+}:{lo:+}", random_len(r));
                }
                5 => {
                    let _ = write!(t, "{h}~+0:-{}", 1 + r.below(59));
                }
                6 => {
                    let _ = write!(t, "r{}", random_len(r));
                }
                7 => {
                    for _ in 0..r.below(30) {
                        t.push_str("r ");
                    }
                }
                8 => {
                    let _ = write!(t, "{h}~+0:+{}", r.below(200_000));
                }
                _ => {
                    let _ = write!(t, "{h}&{h}{}&{h}1..", random_len(r));
                }
            }
            t.push(' ');
        }
        t.push('\n');
    }
    t
}

/// 못 깨뜨림(`030c76a`): 새 성부 열기가 바뀐 뒤에도 느슨 모드의 같은 이름 블록(쉼표로 못 채우는
/// 간격을 만드는 시작·길이 보정과 1–59틱 노트를 섞음)에서 왕복, 경고 0, 재내보내기 같음, 시작 보정
/// −480..=+59, 출력 ≤ 입력 256배. release에서 시드 1·2·3 각 20,000 사례(43,133건)도 실패 0이었다.
#[test]
fn random_same_name_lenient_blocks_round_trip_with_offsets_in_range() {
    let mut r = Lcg(8080);
    let mut accepted = 0;
    for case in 0..800 {
        let text = random_same_name_lenient_text(&mut r);
        let Ok(p) = parse(&text, Mode::Lenient) else {
            continue;
        };
        if !p.warnings().is_empty() {
            continue;
        }
        accepted += 1;
        let out = export_parsed(&p);
        let q = parse(&out, Mode::Lenient).unwrap_or_else(|e| panic!("사례 {case}: {e}\n{out}"));
        assert!(q.warnings().is_empty(), "사례 {case}: 경고\n{out}");
        assert!(
            view(p.tracks()[0].clip()) == view(q.tracks()[0].clip()),
            "사례 {case}: 노트가 다르다\n{text}\n{out}"
        );
        assert_eq!(export_parsed(&q), out, "사례 {case}: 재내보내기");
        if let Err(e) = start_offsets_in_range(&out) {
            panic!("사례 {case}: {e}\n{out}");
        }
        assert!(out.len() <= 256 * text.len(), "사례 {case}: 크기");
    }
    assert!(accepted > 400, "읽힌 사례가 적다: {accepted}");
}

/// 지적 2. 셋잇단 없는 같은 이름 블록 17개가 긴 쉼표 뒤 같은 자리에서 서로 다른 길이로 겹치면,
/// 성부 16개 면제 뒤 예산이 바닥나 17번째 음이 겹쳐 적혀(`~+0:+…`) 엄격 모드로 다시 읽히지 않는다.
/// 예산 = 16 × (사건 17 + 시작 400온음표) + 64 = 6,736인데 면제 성부 16개가 쉼표 6,400을 쓰고,
/// 17번째 성부는 400이 든다. 동작은 결정 A(성부 예산)대로지만, 결정
/// `ktext-implementation-details.md`의 「엄격 모드 왕복」은 이번에 「같은 이름 성부 블록이라도
/// 셋잇단이 없으면 예외가 아니다」라고 적었고, 알려진 예외 넷에 이 꼴이 없다. 16개까지는 면제라
/// 통과한다. 결정이 이 꼴을 예외에 더하면 그 결정에 따라 기대값을 바꾼다.
#[test]
fn export_keeps_strict_text_when_17_same_name_blocks_overlap_after_long_rest() {
    let lens: Vec<u64> = notations();
    let text = |k: usize| {
        let mut t = String::new();
        for &l in lens.iter().take(k) {
            let _ = writeln!(
                t,
                "@track A ch1\nl1 {}c{}",
                "r ".repeat(400),
                notation_text(l)
            );
        }
        t
    };
    let sixteen = strict_round_trip(&text(16)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(sixteen.matches("@track").count(), 16);
    if let Err(e) = strict_round_trip(&text(17)) {
        panic!("{}", e.lines().next().unwrap_or_default());
    }
}

fn best_export_time(text: &str) -> Duration {
    let p = parse(text, Mode::Lenient).unwrap();
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

/// 지적 1. `d80fc1b`의 `tie_pieces`는 조각 벡터를 다 만든 뒤에야 16조각 한도로 거른다. `by_wholes`는
/// `vec![3840; 길이 / 3840]`, `by_double_dotted_wholes`는 후보 넷마다 `vec![6720; 길이 / 6720 − …]`를
/// 만든다. 그 전에는 조각 수를 먼저 보고 한도를 넘으면 할당 없이 `None`이었다. `tie_pieces`는
/// `floor_preferred`·`grid_exact`·`event`를 거쳐 사건마다 여러 번 불리므로, 길이 보정 하나로
/// u32 끝까지 늘린 노트(`c~+0:+4294000000`, 18바이트)마다 약 1.1M + 4 × 0.64M 원소를 채운다.
/// 측정: debug 200개(3.4 KB) 3.5 s(`665b9e2` 5 ms), release 8,000개(136 KB) 1.7–2.0 s(23 ms).
/// 같은 수의 짧은 길이 보정 노트(`c~+0:+4294`)는 debug 1.8 ms다. 출력은 같다.
#[test]
fn export_time_stays_flat_when_note_lengths_reach_u32_max() {
    let n = 100;
    let huge = best_export_time(&format!("@track A ch1\n{}", "c~+0:+4294000000 ".repeat(n)));
    let small = best_export_time(&format!("@track A ch1\n{}", "c~+0:+4294 ".repeat(n)));
    assert!(
        huge <= small * 10 + Duration::from_millis(50),
        "노트 {n}개: 긴 길이 {huge:?}, 짧은 길이 {small:?}"
    );
}

/// 같은 길이(120틱) 노트를 150틱마다 둔 클립. 앞 성부 커서에서 다음 음까지 30틱(쉼표로 못 채움)이라
/// 사건마다 `good`의 `clean` 탐색과 `fresh` 판단을 거친다.
fn alternating_gap_clip(n: u64) -> Clip {
    let mut c = Clip::new(Tick::ZERO, n * 150).unwrap();
    for k in 0..n {
        c.insert_note(
            Note::new(
                Tick::new(k * 150),
                120,
                Pitch::new(60).unwrap(),
                Velocity::from_midi1(100).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
    c
}

/// 못 깨뜨림(`030c76a`, `665b9e2`): 쉼표로 못 채우는 간격이 사건마다 나와 새 성부 판단을 매번 타는
/// 입력에서도 시간은 입력에 거의 선형이다. 성부는 둘로 번갈아 쓰이고 `~`는 없다.
#[test]
fn export_time_stays_near_linear_when_unrestable_gaps_repeat() {
    let time = |n: u64| {
        let c = alternating_gap_clip(n);
        (0..3)
            .map(|_| {
                let t = Instant::now();
                let out = export_clip(&c);
                let d = t.elapsed();
                assert!(!out.contains('~'), "{out}");
                assert_eq!(out.matches("@track").count(), 2);
                d
            })
            .min()
            .unwrap()
    };
    let small = time(4000);
    let large = time(8000);
    assert!(
        large < small * 3,
        "입력 두 배에 시간 {small:?} → {large:?}(제곱이면 네 배)"
    );
}
