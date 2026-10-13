//! S0-T5 적대적 리뷰 6회차(결정 2026-10-13 개정 「선형 보장 + 상한 재기술」 뒤 확인 리뷰)의
//! 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5/review-6.md` 7절), 통과하는 테스트는
//! 깨뜨리지 못한 시도를 회귀 테스트로 남긴 것이다.

use std::fmt::Write as _;

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

fn clip(length: u64, notes: &[Note]) -> Clip {
    let mut c = Clip::new(Tick::ZERO, length).unwrap();
    for n in notes {
        // 같은 노트 중복은 core가 받지 않을 수 있으므로 건너뛴다.
        let _ = c.insert_note(n.clone());
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

fn export_with(header: &Header, ch: u8, c: &Clip) -> String {
    export(
        header,
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

/// 가져온 결과 `p`를 내보내고 다시 읽어, 트랙마다 노트·속성·클립 길이(상한 안)가 같고, 경고가
/// 없고, 재내보내기가 같고, 시작 보정이 범위 안인지. 내보낸 텍스트를 돌려준다.
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
        if a.name() != b.name()
            || a.channel() != b.channel()
            || a.program() != b.program()
            || a.bank() != b.bank()
        {
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

fn ratio_of(text: &str) -> (usize, usize) {
    let p = parse(text, Mode::Lenient).unwrap();
    (text.len(), export_parsed(&p).len())
}

/// 이름이 `name_len`바이트인 트랙 한 블록에 서로 겹치는 64분음표 길이 보정 노트 `k`개.
/// 노트는 60틱 간격으로 시작해 모두 클립 끝까지 울리므로 성부 `k`개가 필요하다.
fn long_name_overlaps(name_len: usize, k: u64) -> String {
    let mut t = format!("@track {} ch1\nl64", "x".repeat(name_len));
    for i in 0..k {
        let _ = write!(t, " c~+0:+{}", 100_000 - i);
    }
    t
}

/// 지적 1. 결정 개정의 보장 「출력은 입력에 선형이고, 상수는 256배 이내」가 긴 트랙 이름에서
/// 깨진다. 내보내기는 성부 블록마다 `@track` 줄(이름 포함)을 다시 적고, 성부 수는 겹친 노트 수에
/// 비례하는데, 이름은 입력에 한 번만 나온다. 성부를 여는 비용은 시작까지의 온음표 쉼표만 세므로
/// 클립 앞쪽(온음표 안)의 겹침은 예산을 쓰지 않는다. 그래서 출력은 이름 길이 × 성부 수다.
/// release: 이름 6,000바이트 + 노트 1,000개(18 KB) → 6.1 MB(337배), 이름 12,000 + 노트 2,000
/// (36 KB) → 20.2 MB(561배).
#[test]
fn export_size_stays_within_256x_input_when_long_track_name_repeats_per_voice() {
    let text = long_name_overlaps(6000, 1000);
    let (i, o) = ratio_of(&text);
    assert!(
        o <= 256 * i,
        "입력 {i}바이트 → 내보내기 {o}바이트({}배)",
        o / i
    );
}

/// 지적 1. 같은 입력 꼴에서 이름과 노트 수를 함께 두 배로 하면 입력은 두 배, 출력은 약 네 배다
/// (제곱). 선형이라면 출력/입력 비는 거의 그대로여야 한다. 기준은 비가 25% 넘게 늘지 않는 것이다.
#[test]
fn export_size_ratio_stays_flat_when_long_track_name_and_overlaps_double() {
    let (i1, o1) = ratio_of(&long_name_overlaps(1500, 250));
    let (i2, o2) = ratio_of(&long_name_overlaps(3000, 500));
    assert!(
        o2 * i1 * 4 <= o1 * i2 * 5,
        "{i1} → {o1}({}배), {i2} → {o2}({}배)",
        o1 / i1,
        o2 / i2
    );
}

/// 지적 2. 엄격 모드로 읽히는 그리드 노트의 타이(`c4&c16`처럼 온음표보다 짧고 한 표기로 안 되는
/// 합)가 `c4~+0:+240`으로 나가 엄격 모드로 다시 읽히지 않는다. REQ 5.14는 `~`를 「그리드에 안
/// 맞는 노트」에 쓴다고 하고 `&`를 문법에 둔다. 내보내기의 타이 사슬은 온음표보다 긴 노트만
/// 다룬다(`export.rs`의 `event`). `c2&c8 d4`는 `c2~+0:+480 r8 d4`로, 음이 울리는 자리에 쉼표가
/// 적힌다.
#[test]
fn export_keeps_strict_text_when_grid_note_is_short_tie() {
    for src in [
        "@track A ch1\nc4&c16",
        "@track A ch1\nc2&c8 d4",
        "@track A ch1\nc4.&c4",
        "@track A ch1\nc8&c32 r4",
        "@track A ch10\nbd4&bd16 sd4",
        "@track A ch1\n[c e g]4&[c e g]16",
    ] {
        let p = parse(src, Mode::Strict).unwrap();
        let out = check_parsed(&p).unwrap();
        assert!(
            parse(&out, Mode::Strict).is_ok(),
            "{src:?} → {out:?}가 엄격 모드로 읽히지 않는다"
        );
    }
}

/// 지적 2. 점·겹점 64분쉼표(90·105틱) 뒤의 음. 엄격 모드 문법으로 적은 시작인데, 내보내기는
/// 간격을 60틱 배수 쉼표로만 채워(`fill_gap`) 나머지를 `~+30`·`~+45`로 적는다.
#[test]
fn export_keeps_strict_text_when_dotted_64th_rest_precedes_note() {
    for src in [
        "@track A ch1\nr64.. c8",
        "@track A ch1\nr64. c8",
        "@track A ch1\nr32.. c8",
        "@track A ch10\nr64. bd8",
    ] {
        let p = parse(src, Mode::Strict).unwrap();
        let out = check_parsed(&p).unwrap();
        assert!(
            parse(&out, Mode::Strict).is_ok(),
            "{src:?} → {out:?}가 엄격 모드로 읽히지 않는다"
        );
    }
}

/// 지적 2. 8분음표 뒤의 16분 셋잇단. 8분음표(480틱)도 점8분(720)의 2/3라, 셋잇단 찾기
/// (`is_triplet`)가 앞에서부터 탐욕으로 `c8 d e`를 셋잇단으로 묶는다. 그래서 평범한
/// `c8 n(d16 e16 f16) g8`이 `n(o4 c8. d16 e16) f32.~+0:-20 g8~-20`으로 나간다(왕복은 맞다).
#[test]
fn export_keeps_triplet_grouping_when_eighth_precedes_sixteenth_triplet() {
    for src in [
        "@track A ch1\nc8 n(d16 e16 f16) g8",
        "@track A ch1\nn(c8 d8 e8) c8 n(d16 e16 f16)",
        "@track A ch1\nc64 n(d16 e16. f32)",
    ] {
        let p = parse(src, Mode::Strict).unwrap();
        let out = check_parsed(&p).unwrap();
        assert!(
            parse(&out, Mode::Strict).is_ok(),
            "{src:?} → {out:?}가 엄격 모드로 읽히지 않는다"
        );
    }
}

/// 압축 입력의 다른 변형(못 깨뜨림). 구현자가 잰 꼴(`l1..` 뒤 `c`) 밖에서 기본 길이, 화음,
/// 셋잇단, 타이, 옥타브, 시작 보정 `~+0`·`~+59`, 박자표, 끝의 겹친 노트(화음·`~+59`·`~-3840`)를
/// 바꿔 본다. release에서 박자표 11 × 기본 길이 3 × 단위 13 × 꼬리 5 × 겹침 5가지(10,725 사례,
/// n = 3,000)를 돌린 최고는 196배였다(`l1..` 뒤 `c`, 겹침 32). 여기서는 n = 6,000의 대표 꼴만 둔다.
#[test]
fn export_size_stays_within_256x_input_when_compressed_variants_feed_voices() {
    let cases = [
        ("", "l1.. ", "c", "d64~+0:+{}", 32),
        ("@time 2/2\n", "l1.. ", "c", "d64~+0:+{}", 64),
        ("@time 7/4\n", "l1.. ", "c~+59", "d64~+59:+{}", 32),
        ("", "l1.. ", "[ce]", "[df]64~+0:+{}", 64),
        ("", "l1.. ", ">c<c", "d64~-3840:+{}", 32),
        ("@time 1/64\n", "l1. ", "cr", "d64~+13:+{}", 17),
        ("", "l1.. ", "n(ccc)", "d64~+0:+{}", 64),
    ];
    for (time, pre, unit, tail, k) in cases {
        let mut t = format!("{time}@track A ch1\n{pre}");
        t.push_str(&unit.repeat(6000));
        for i in 0..k {
            let _ = write!(
                t,
                " {}",
                tail.replace("{}", &(60 * (k - i) + 1000).to_string())
            );
        }
        let (i, o) = ratio_of(&t);
        assert!(
            o <= 256 * i,
            "{time:?} {pre:?} {unit:?} {tail:?} k={k}: 입력 {i} → {o}({}배)",
            o / i
        );
    }
}

/// 무작위 느슨 모드 텍스트(못 깨뜨림): 가져온 결과를 내보내 다시 가져오면 같고, 경고 0,
/// 재내보내기 같음, 시작 보정 −480..=+59. 음·쉼표·기본 길이·점·옥타브·화음·셋잇단·타이·가사·
/// 시작/길이 보정·드럼 라벨·같은 이름 블록(성부)·박자표를 섞는다. 읽히지 않는 텍스트는 건너뛴다.
/// release에서 시드를 바꿔 20,000 사례도 돌렸다.
#[test]
fn random_lenient_text_round_trips_through_export_with_offsets_in_range() {
    let accepted = random_text_cases(7, 400);
    assert!(accepted > 150, "읽힌 사례가 적다: {accepted}");
}

fn random_text_cases(seed: u64, cases: usize) -> usize {
    let mut r = Lcg(seed);
    let mut accepted = 0;
    for case in 0..cases {
        let text = random_text(&mut r);
        let Ok(p) = parse(&text, Mode::Lenient) else {
            continue;
        };
        accepted += 1;
        if let Err(e) = check_parsed(&p) {
            panic!("사례 {case}: {e}\n원문:\n{text}");
        }
    }
    accepted
}

fn random_len(r: &mut Lcg) -> String {
    let d = r.pick(&["", "", "1", "2", "4", "8", "16", "32", "64"]);
    if d.is_empty() {
        return String::new();
    }
    format!("{d}{}", r.pick(&["", "", ".", ".."]))
}

fn random_text(r: &mut Lcg) -> String {
    let mut t = String::new();
    let _ = write!(
        t,
        "{}",
        r.pick(&[
            "",
            "@time 3/4\n",
            "@time 7/8\n",
            "@time 5/16\n",
            "@tempo 97.5\n"
        ])
    );
    let drum = r.below(5) == 0;
    let ch = if drum { 10 } else { 1 };
    for _ in 0..=r.below(4) {
        let name = r.pick(&["A", "B", "\"가 나\""]);
        let _ = writeln!(t, "@track {name} ch{ch}");
        let _ = write!(t, "l{} ", r.pick(&["4", "8", "16", "1..", "64"]));
        for _ in 0..=r.below(30) {
            random_token(r, &mut t, drum);
            t.push(' ');
        }
        t.push('\n');
    }
    t
}

fn random_head(r: &mut Lcg, drum: bool) -> String {
    if drum {
        r.pick(&["bd", "sd", "ch", "oh", "n38", "crash"]).to_owned()
    } else {
        format!(
            "{}{}",
            r.pick(&["c", "d", "e", "f", "g", "a", "b"]),
            r.pick(&["", "", "+", "-"])
        )
    }
}

fn random_suffix(r: &mut Lcg, t: &mut String) {
    t.push_str(&random_len(r));
    if r.below(4) == 0 {
        let s = i64::try_from(r.below(800)).unwrap() - 400;
        let _ = write!(t, "~{s:+}");
        if r.below(2) == 0 {
            let l = i64::try_from(r.below(20_000)).unwrap() - 50;
            let _ = write!(t, ":{l:+}");
        }
    }
    if r.below(6) == 0 {
        t.push_str("\"가\"");
    }
}

fn random_token(r: &mut Lcg, t: &mut String, drum: bool) {
    match r.below(12) {
        0..=4 => {
            let head = random_head(r, drum);
            t.push_str(&head);
            random_suffix(r, t);
            if r.below(5) == 0 {
                // 같은 음과 잇는 타이. 짧은 합(`c4&c16`)이 자주 나오게 한다.
                let _ = write!(t, "&{head}{}", random_len(r));
            }
        }
        5 => {
            let _ = write!(t, "r{}", random_len(r));
        }
        6 => {
            let _ = write!(t, "l{}", random_len(r).trim_start_matches('.'));
            if t.ends_with('l') {
                t.push('8');
            }
        }
        7 if !drum => t.push_str(r.pick(&["<", ">", "o3", "o5", "o0", "o9"])),
        8 => {
            t.push('[');
            for _ in 0..2 + r.below(3) {
                let head = random_head(r, drum);
                let _ = write!(t, "{head} ");
            }
            t.push(']');
            random_suffix(r, t);
        }
        9 => {
            t.push_str("n(");
            for _ in 0..3 {
                let head = random_head(r, drum);
                let _ = write!(t, "{head}{} ", random_len(r));
            }
            t.push(')');
        }
        10 => t.push('|'),
        _ => {
            let head = random_head(r, drum);
            let _ = write!(t, "{head}64~+0:+{}", r.below(30_000));
        }
    }
}

/// 무작위 클립(못 깨뜨림, 리뷰 5와 다른 시드와 분포): 앞쪽의 짧은 타이 길이(온음표보다 짧은
/// 두 표기의 합), 셋잇단, 1틱 노트, 늦게 몰린 겹침, 화음·가사·드럼, 박자표 다섯. 왕복, 경고 0,
/// 클립 길이 상한, 재내보내기, 시작 보정 −480..=+59를 본다. release에서 20,000 사례도 돌렸다.
#[test]
fn random_clips_with_short_tie_lengths_round_trip_with_offsets_in_range() {
    random_clip_cases(2026, 200);
}

fn random_clip_cases(seed: u64, cases: usize) {
    let headers: Vec<Header> = ["4/4", "3/4", "7/8", "5/16", "2/2"]
        .iter()
        .map(|t| {
            *parse(&format!("@time {t}\n@track T ch1\nc"), Mode::Lenient)
                .unwrap()
                .header()
        })
        .collect();
    let notated: Vec<u64> = [1, 2, 4, 8, 16, 32, 64]
        .iter()
        .flat_map(|d| {
            let b = W / d;
            [b, b * 3 / 2, b * 7 / 4]
        })
        .collect();
    let mut r = Lcg(seed);
    for case in 0..cases {
        let header = headers[usize::try_from(r.below(5)).unwrap()];
        let ch = if r.below(5) == 0 { 10 } else { 1 };
        let mut ns = Vec::new();
        let mut at = 0;
        let pick = |r: &mut Lcg| notated[usize::try_from(r.below(21)).unwrap()];
        for _ in 0..r.below(40) {
            let len = match r.below(5) {
                0 => pick(&mut r) + pick(&mut r),
                1 => pick(&mut r) * 2 / 3,
                2 => 1 + r.below(100),
                3 => pick(&mut r),
                _ => 1 + r.below(20_000),
            };
            at += match r.below(3) {
                0 => 0,
                1 => len,
                _ => r.below(2000),
            };
            let pitch = u8::try_from(if ch == 10 {
                35 + r.below(47)
            } else {
                r.below(128)
            })
            .unwrap();
            let lyric = if r.below(5) == 0 { Some("가") } else { None };
            ns.push(note(at, len, pitch, lyric));
        }
        let g = at + r.below(200) * W;
        for i in 0..r.below(30) {
            ns.push(note(g + i * 60, 1 + r.below(10_000), 60, None));
        }
        let max_end = ns.iter().map(Note::end).max().unwrap_or(0);
        let c = clip(max_end.max(1) + r.below(50), &ns);
        let text = export_with(&header, ch, &c);
        let p = parse(&text, Mode::Lenient).unwrap_or_else(|e| panic!("사례 {case}: {e}"));
        assert!(
            view(p.tracks()[0].clip()) == view(&c),
            "사례 {case}: 노트가 다르다\n{text}"
        );
        if let Err(e) = check_parsed(&p) {
            panic!("사례 {case}: {e}");
        }
        let back = p.tracks()[0].clip().length();
        assert!(
            back.abs_diff(c.length()) <= length_bound(c.length()),
            "사례 {case}: 클립 길이 {} → {back}",
            c.length()
        );
    }
}
