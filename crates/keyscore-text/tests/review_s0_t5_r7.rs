//! S0-T5 적대적 리뷰 7회차(리뷰 6 수정 `fff52be`·`76b3e84` 뒤 확인 리뷰)의 깨뜨리기 시도.
//! 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5/review-7.md` 7절), 통과하는 테스트는
//! 깨뜨리지 못한 시도를 회귀 테스트로 남긴 것이다.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use keyscore_core::{Channel, Clip, Note, Pitch, Tick, Velocity};
use keyscore_text::{ExportTrack, Header, Mode, Parsed, export, parse};

const W: u64 = 3840;
type View = (u64, u64, u8, Option<String>);

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

fn note(start: u64, length: u64, pitch: u8, lyric: Option<&str>) -> Note {
    let n = Note::new(
        Tick::new(start),
        u32::try_from(length).unwrap(),
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap();
    match lyric {
        Some(y) => n.with_lyric(y),
        None => n,
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

fn export_with(ch: u8, c: &Clip) -> String {
    export(
        &Header::default(),
        &[ExportTrack {
            name: "T",
            channel: Channel::new(ch).unwrap(),
            program: None,
            bank: None,
            clip: c,
        }],
    )
}

/// 결정 `2026-10-12-ktext-implementation-details.md`의 클립 길이 상한.
fn length_bound(length: u64) -> u64 {
    match length {
        150.. => 7,
        60..150 => 15,
        l => 60 - l,
    }
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

/// 가져온 결과를 내보내고 느슨 모드로 다시 읽어, 노트·속성·클립 길이(상한 안)가 같고, 경고가
/// 없고, 재내보내기가 같고, 시작 보정이 범위 안인지.
fn check_parsed(p: &Parsed) -> Result<String, String> {
    let text = export_parsed(p);
    let q = parse(&text, Mode::Lenient).map_err(|e| format!("{e}\n{text}"))?;
    if !q.warnings().is_empty() {
        return Err(format!("경고 {:?}\n{text}", q.warnings()));
    }
    if q.header() != p.header() || q.tracks().len() != p.tracks().len() {
        return Err(format!("헤더·트랙 수가 다르다\n{text}"));
    }
    for (a, b) in p.tracks().iter().zip(q.tracks()) {
        if a.name() != b.name() || a.channel() != b.channel() {
            return Err(format!("트랙 속성이 다르다: {}", a.name()));
        }
        if view(a.clip()) != view(b.clip()) {
            return Err(format!("노트가 다르다: {}\n{text}", a.name()));
        }
        let (la, lb) = (a.clip().length(), b.clip().length());
        if la.abs_diff(lb) > length_bound(la) {
            return Err(format!("클립 길이 {la} → {lb}"));
        }
    }
    if export_parsed(&q) != text {
        return Err(format!("재내보내기가 다르다\n{text}"));
    }
    start_offsets_in_range(&text)?;
    Ok(text)
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

/// 지적 1. 같은 이름의 성부 블록이 셋잇단 없이 만나는데, 앞 성부의 커서에서 다음 음까지가
/// 쉼표로 채울 수 없는 15틱 배수(15·30·45·75·135)이면 내보내기가 그 음을 앞 성부에 넣고 `~`를
/// 붙인다. 원문의 두 블록처럼 새 성부(클립 시작에서 쉼표로 정확히 닿음)를 열면 `~`가 없다.
/// `voices`의 `fresh`는 커서가 15틱 격자 밖(셋잇단 뒤)일 때만 새 성부를 연다. 결정
/// `ktext-implementation-details.md`의 「알려진 예외」(셋잇단 꼴만)에 없는 꼴이다. `76b3e84` 전에도
/// 같았다(이번 수정이 고치지 않은 경로).
#[test]
fn export_keeps_strict_text_when_voice_blocks_meet_at_unrestable_gap() {
    for text in [
        // c64.(0–90) 뒤 30틱 쉬고 d4(120–). 겹치지도 않는다.
        "@track A ch1\nc64.\n@track A ch1\nr32 d4\n",
        "@track A ch1\nc64\n@track A ch1\nr64.. d4\n",
        "@track A ch1\nc2 c64.\n@track A ch1\nr2 r32 d4\n",
        "@track A ch10\nbd64.\n@track A ch10\nr32 sd4\n",
    ] {
        if let Err(e) = strict_round_trip(text) {
            panic!("{e}");
        }
    }
}

/// 지적 2. 온음표 17개보다 짧은 노트도, 엄격 모드로 16조각 이하의 타이로 적혀 있어도, 내보내기의
/// 타이 분해(온음표들 + 나머지의 최소 분해)가 17조각 이상이면 `~+0:+…`로 나간다.
/// 16.5온음표(63,360틱)는 `c1..` 아홉 개와 `c2.`(10조각)로 적히지만 내보내기는 온음표 16개 + 2분
/// 점음표(17조각)로 나눠 `c1..~+0:+56640`을 쓴다. 15온음표 + 30틱(57,630틱)은 11조각으로 적히지만
/// 나머지 30틱을 나눌 수 없어 온음표 14개 + 3조각(17조각)이 된다. 결정 문서의 「온음표 17개보다
/// 긴 노트는 `~`」, 「17조각 이상 타이」(입력 조각 수로 읽힌다)와 맞지 않는다.
#[test]
fn export_keeps_strict_text_when_tie_under_17_wholes_needs_17_split_pieces() {
    let texts = [
        format!("@track A ch1\n{}c2.\n", "c1..&".repeat(9)),
        format!("@track A ch1\n{}c2..&c16..&c64.\n", "c1..&".repeat(8)),
    ];
    for text in &texts {
        let p = parse(text, Mode::Strict).unwrap();
        let len = p.tracks()[0].clip().notes()[0].length();
        assert!(u64::from(len) < 17 * W, "{len}");
        if let Err(e) = strict_round_trip(text) {
            panic!("{e}");
        }
    }
}

fn lyric_triplet_blocks(n: usize) -> String {
    "@track A ch1\nn(c4\"a\" c4\"a\" c4\"a\")\n".repeat(n)
}

fn best_export_time(text: &str) -> Duration {
    let p = parse(text, Mode::Strict).unwrap();
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

/// 지적 3. 성부 배정이 사건 수의 제곱으로 느려진다. 가사가 붙은 같은 노트는 화음으로 묶이지 않아
/// 사건이 따로 서고(`events`), `voices`의 `triplet_starts`는 시작마다 셋잇단 길이를 중복째 쌓는다.
/// `needs_prev`는 사건마다 그 목록을 끝까지 훑으므로(뒤에 셋잇단이 이어지지 않으면 `any`가 일찍
/// 끝나지 않는다) 같은 자리의 가사 셋잇단 N개는 N²이다. 엄격 모드 텍스트(같은 이름 블록 N개)로도
/// 닿는다. release: 블록 2,000/4,000/8,000(68/136/272 KB) → 36/139/545 ms, core API로 같은 노트
/// 16,000쌍 → 2.2 s. 목록에서 중복을 빼면 8,000개 10 ms로 선형이 된다. `voices` doc의
/// 「n log n」과 맞지 않는다.
#[test]
fn export_time_stays_near_linear_when_lyric_triplet_duplicates_double() {
    let small = best_export_time(&lyric_triplet_blocks(2000));
    let large = best_export_time(&lyric_triplet_blocks(4000));
    assert!(
        large < small * 3,
        "입력 두 배에 시간 {small:?} → {large:?}(제곱이면 네 배)"
    );
}

/// 못 깨뜨림: 한 블록의 엄격 텍스트에서 `76b3e84`가 고친 꼴 둘레(셋잇단 앞뒤 타이, 가사·화음
/// 셋잇단, 64분 셋잇단 뒤 점 64분쉼표, 이어진 셋잇단, 점8분 셋잇단).
#[test]
fn export_keeps_strict_text_when_single_block_mixes_triplets_ties_and_dotted_rests() {
    for text in [
        "@track A ch1\nl4 c4&n(c8 d8 e8)\n",
        "@track A ch1\nl4 c n(c8\"a\" d8\"b\" e8\"c\") d\n",
        "@track A ch1\nl4 n([c e]8 [d f]8 [e g]8) d\n",
        "@track A ch1\nl4 n(c64 d64 e64) n(c64 d64 e64) c8\n",
        "@track A ch1\nl4 n(c64 d64 e64) c8 n(c64 d64 e64) r64 c8\n",
        "@track A ch1\nl4 n(c32 d32 e32) r64. c8\n",
        "@track A ch1\nl4 c1&c64.\n",
        "@track A ch1\nl4 c64..&c64..&c64..&c64..\n",
        "@track A ch1\nl4 r64.. c r64.. d\n",
        "@track A ch1\nl4 c r64..\n",
        "@track D ch10\nl4 n(bd8 sd8 bd8) r64. sd\n",
        "@track A ch1\nl4 n(c2 d4 e4) n(c8 d8 e8)\n",
        "@track A ch1\nl4 c8 n(d8 e8 f8) g8 n(a16 b16 c16)\n",
        "@track A ch1\nl4 c16 n(d16 e16 f16) n(d16 e16 f16) c16\n",
        "@track A ch1\nl4 n(c8. d8. e8.) n(c16 d16 e16)\n",
        "@track A ch1\nl4 c1.&c64\n",
    ] {
        let out = strict_round_trip(text).unwrap_or_else(|e| panic!("{e}"));
        assert!(!out.contains('~'), "{text}\n{out}");
    }
}

fn random_len(r: &mut Lcg) -> String {
    let d = r.pick(&["", "1", "2", "4", "8", "16", "32", "64", "64", "32"]);
    if d.is_empty() {
        return String::new();
    }
    format!("{d}{}", r.pick(&["", "", ".", ".."]))
}

/// 무작위 엄격 모드 텍스트: 같은 이름 블록 `1..=blocks`개, 짧은 타이, 점·겹점 64분, 화음, 가사,
/// 셋잇단(`triplets`), 드럼.
fn random_strict_text(r: &mut Lcg, triplets: bool, blocks: u64) -> String {
    let mut t = r
        .pick(&["", "@time 3/4\n", "@time 7/8\n", "@time 5/16\n"])
        .to_owned();
    let drum = r.below(5) == 0;
    let ch = if drum { 10 } else { 1 };
    for _ in 0..=r.below(blocks) {
        let _ = writeln!(t, "@track A ch{ch}");
        let _ = write!(t, "l{} ", r.pick(&["4", "8", "16", "1", "2.", "64"]));
        for _ in 0..r.below(25) {
            let h = if drum {
                r.pick(&["bd", "sd", "ch", "n75"])
            } else {
                r.pick(&["c", "d", "e", "f+", "g", "a", "b-"])
            };
            match r.below(10) {
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
                7 if triplets => {
                    let lens = [random_len(r), random_len(r), random_len(r)];
                    let _ = write!(t, "n({h}{} {h}{} {h}{})", lens[0], lens[1], lens[2]);
                }
                8 if !drum => t.push_str(r.pick(&["<", ">", "o3", "o5"])),
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

/// 못 깨뜨림: 블록 하나의 무작위 엄격 텍스트(셋잇단·짧은 타이·점 64분·화음·가사·드럼)는 내보낸
/// 뒤 엄격 모드로 다시 읽히고, 노트·클립 길이·재내보내기가 같다. release에서 150,000 사례(시드
/// 777)와 다른 이름 두 트랙 30,000 사례도 돌렸다.
#[test]
fn random_strict_single_block_text_exports_to_strict_text() {
    let mut r = Lcg(777);
    let mut accepted = 0;
    for case in 0..3000 {
        let text = random_strict_text(&mut r, true, 1);
        if parse(&text, Mode::Strict).is_err() {
            continue;
        }
        accepted += 1;
        if let Err(e) = strict_round_trip(&text) {
            panic!("사례 {case}: {e}");
        }
    }
    assert!(accepted > 1500, "읽힌 사례가 적다: {accepted}");
}

/// 못 깨뜨림: 같은 이름 블록 여러 개(셋잇단 포함)를 느슨 모드로 읽어 내보내도 왕복, 경고 0, 클립
/// 길이 상한, 재내보내기, 시작 보정 −480..=+59가 지켜진다. release에서 60,000 사례도 돌렸다.
#[test]
fn random_voice_block_text_round_trips_with_offsets_in_range() {
    let mut r = Lcg(9);
    for case in 0..600 {
        let text = random_strict_text(&mut r, case % 2 == 0, 6);
        let Ok(p) = parse(&text, Mode::Lenient) else {
            continue;
        };
        if let Err(e) = check_parsed(&p) {
            panic!("사례 {case}: {e}\n원문:\n{text}");
        }
    }
}

/// 셋잇단 길이(표기의 2/3)와 짧은 타이 길이, 짧은 노트, 셋잇단 격자(40틱 배수) 시작, 가사 붙은 같은
/// 노트 중복을 섞은 클립.
fn triplet_clip(r: &mut Lcg) -> Clip {
    let notated: Vec<u64> = [1, 2, 4, 8, 16, 32, 64]
        .iter()
        .flat_map(|d| {
            let b = W / d;
            [b, b * 3 / 2, b * 7 / 4]
        })
        .collect();
    let pick = |r: &mut Lcg| notated[usize::try_from(r.below(21)).unwrap()];
    let mut ns = Vec::new();
    let mut at = 0;
    for _ in 0..r.below(80) {
        let len = match r.below(6) {
            0 | 1 => pick(r) * 2 / 3,
            2 => pick(r) + pick(r),
            3 => pick(r),
            4 => 1 + r.below(200),
            _ => pick(r) * 2 / 3 + pick(r),
        };
        at += match r.below(5) {
            0 => 0,
            1 | 2 => len,
            3 => pick(r) * 2 / 3,
            _ => r.below(4) * W + 40 * r.below(30),
        };
        let pitch = u8::try_from(40 + r.below(40)).unwrap();
        let lyric = if r.below(6) == 0 { Some("x") } else { None };
        ns.push(note(at, len, pitch, lyric));
        if r.below(4) == 0 {
            ns.push(note(at, len, pitch, Some("y")));
        }
    }
    let max_end = ns.iter().map(Note::end).max().unwrap_or(0);
    let mut c = Clip::new(Tick::ZERO, max_end.max(1) + r.below(100)).unwrap();
    for n in ns {
        c.insert_note(n).unwrap();
    }
    c
}

/// 못 깨뜨림: 셋잇단이 많은 무작위 클립(바뀐 성부 배정의 `good`·`fresh`·`needs_prev` 경로)에서
/// 왕복, 클립 길이 상한, 재내보내기, 시작 보정 −480..=+59, 패닉 없음. release에서 30,000 사례도
/// 돌렸다.
#[test]
fn random_triplet_clips_round_trip_with_offsets_in_range() {
    let mut r = Lcg(4242);
    for case in 0..400 {
        let c = triplet_clip(&mut r);
        let ch = if r.below(6) == 0 { 10 } else { 1 };
        let text = export_with(ch, &c);
        let p = parse(&text, Mode::Lenient).unwrap_or_else(|e| panic!("사례 {case}: {e}"));
        assert!(
            view(p.tracks()[0].clip()) == view(&c),
            "사례 {case}: 노트가 다르다\n{text}"
        );
        let back = p.tracks()[0].clip().length();
        assert!(
            back.abs_diff(c.length()) <= length_bound(c.length()),
            "사례 {case}: 클립 길이 {} → {back}",
            c.length()
        );
        if let Err(e) = check_parsed(&p) {
            panic!("사례 {case}: {e}");
        }
    }
}

/// 못 깨뜨림: 선형 256배 상한. 압축 입력(`l1..` 뒤 단위 3,000개)에 셋잇단·짧은 타이·쉼표 낀
/// 셋잇단 단위, 트랙 이름 1·50·300바이트, 박자표 넷, 겹친 꼬리(길이 보정 음, 길이 보정 셋잇단)
/// 0–300개를 섞어 1,152꼴을 쟀다(release). 최고 164배(이름 1바이트, 단위 `c`, 꼬리 `c~+0:+N` 64개).
/// 대표 꼴을 남긴다.
#[test]
fn export_size_stays_within_256x_input_when_triplets_ties_and_long_names_mix() {
    let names = ["A".to_owned(), "x".repeat(50), "x".repeat(300)];
    for name in &names {
        for (unit, tail, k) in [
            ("c", "l64 c~+0:+{N}", 64),
            ("n(ccc)", "l64 n(c~+0:+{N} c c)", 300),
            ("c&c", "l64 c~+0:+{N}", 17),
            ("n(c r c)", "l64 c", 300),
            ("c64&c", "l64 n(c~+0:+{N} c c)", 64),
        ] {
            let mut t = format!("@track {name} ch1\nl1.. ");
            t.push_str(&unit.repeat(3000));
            t.push(' ');
            for i in 0..k {
                let _ = write!(t, " {}", tail.replace("{N}", &(400_000 - i).to_string()));
            }
            let p = parse(&t, Mode::Lenient).unwrap();
            let out = export_parsed(&p);
            assert!(
                out.len() <= 256 * t.len(),
                "{} → {} ({unit}, {tail}, {k})",
                t.len(),
                out.len()
            );
        }
    }
}
