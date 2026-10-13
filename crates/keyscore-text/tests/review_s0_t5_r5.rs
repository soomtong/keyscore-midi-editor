//! S0-T5 적대적 리뷰 5회차(결정 2026-10-13 A 뒤 확인 리뷰)의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5/review-5.md` 7절), 통과하는 테스트는
//! 깨뜨리지 못한 시도를 회귀 테스트로 남긴 것이다.

use std::fmt::Write as _;

use keyscore_core::{Channel, Clip, Note, Pitch, Tick, Velocity};
use keyscore_text::{ExportTrack, Header, Mode, export, parse};

const W: u64 = 3840;
type View = (u64, u64, u8, Option<String>);

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

/// 내보내기 → 가져오기가 노트를 지키고, 경고가 없고, 클립 길이가 결정 상한 안이고, 다시 내보내도
/// 같은 문자열이고, `~` 시작 보정이 −480..=+59 안인지. 내보낸 텍스트를 돌려준다.
fn round_trip(header: &Header, ch: u8, c: &Clip) -> Result<String, String> {
    let text = export_with(header, ch, c);
    let p = parse(&text, Mode::Lenient).map_err(|e| format!("{e}\n{text}"))?;
    if !p.warnings().is_empty() {
        return Err(format!("경고 {:?}", p.warnings()));
    }
    let back = p.tracks()[0].clip();
    if view(back) != view(c) {
        return Err("노트가 다르다".to_owned());
    }
    let bound = match c.length() {
        150.. => 7,
        60..150 => 15,
        l => 60 - l,
    };
    if back.length().abs_diff(c.length()) > bound {
        return Err(format!("클립 길이 {} → {}", c.length(), back.length()));
    }
    if export_with(header, ch, back) != text {
        return Err("재내보내기가 다르다".to_owned());
    }
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
    Ok(text)
}

/// 한 블록 텍스트: `prefix` 뒤에 `unit`을 `n`번, 그 뒤에 서로 겹치는 길이 보정 노트 `k`개.
fn late_overlaps(prefix: &str, unit: &str, n: usize, k: u64) -> String {
    let mut text = format!("@track A ch1\n{prefix}");
    text.push_str(&unit.repeat(n));
    for i in 0..k {
        let _ = write!(text, " d64~+0:+{}", 60 * (k - i) + 1000);
    }
    text
}

fn export_ratio(text: &str) -> (usize, usize) {
    let p = parse(text, Mode::Lenient).unwrap();
    let out = export_with(p.header(), 1, p.tracks()[0].clip());
    (text.len(), out.len())
}

/// 지적 1. 결정 A의 완료 조건 「출력은 입력의 64배 이내」가 기본 길이(`l1..`)를 쓴 입력에서
/// 깨진다. `l1..` 뒤의 `r`는 1–2바이트로 1.75온음표를 나아가지만, 내보내기는 성부마다 그 구간을
/// 온음표마다 `r1 |`와 줄바꿈(5바이트)으로 다시 적는다. 겹친 노트 16개(면제 성부 16개)만으로
/// 16 × 1.75 × 5 = 140배에 다가간다(n = 30,000에서 139배). 공백을 넣은 평범한 꼴
/// (`l1.. r r r`)도 70배에 다가간다. 처음 기준은 결정 A와 리뷰 2–4의 64배였고, 결정 개정
/// (2026-10-13, 「선형 보장 + 상한 재기술」)에 따라 256배로 바꿨다(구현자 수정, 입력은 그대로).
#[test]
fn export_size_stays_within_256x_input_when_default_length_rests_feed_16_voices() {
    for unit in ["r", "r "] {
        let text = late_overlaps("l1.. ", unit, 6000, 16);
        let (i, o) = export_ratio(&text);
        assert!(
            o <= 256 * i,
            "단위 {unit:?}: 입력 {i}바이트 → 내보내기 {o}바이트({}배)",
            o / i
        );
    }
}

/// 지적 1. 같은 성질의 예산 쪽 경로. 예산은 사건마다, 그리고 마지막 시작까지의 온음표마다
/// 16온음표 쉼표를 허락하는데, `l1..` 뒤의 `c` 한 바이트는 사건 하나와 1.75온음표를 함께 만든다
/// (단위 2.75). 그래서 출력은 바이트당 약 16 × 2.75 × 5 = 220바이트에 다가간다(겹친 노트 64개,
/// n = 30,000에서 218배). 계수를 결정 범위(16–32)의 가장 아래로 두어도 64배 안에 둘 수 없다.
/// 결정 개정(2026-10-13)에 따라 기준을 256배로 바꿨다(구현자 수정, 입력은 그대로).
#[test]
fn export_size_stays_within_256x_input_when_default_length_notes_inflate_budget() {
    let text = late_overlaps("l1.. ", "c", 6000, 64);
    let (i, o) = export_ratio(&text);
    assert!(
        o <= 256 * i,
        "입력 {i}바이트 → 내보내기 {o}바이트({}배)",
        o / i
    );
}

/// 예산 식의 정확한 경계(못 깨뜨림). `bars`마디 뒤 그리드 패드 17음(16분음표 간격, 온음표 8개
/// 길이)의 마지막 시작은 `bars` + 1온음표라 예산은 16 × (17 + `bars` + 1) + 64다. 성부 16개가
/// 각각 `bars`개의 쉼표를 쓰면 352가 남고, 17번째 성부는 `bars` + 1개가 든다. 그래서 351마디 뒤면
/// 17번째 성부도 열려 `~`가 없고, 352마디 뒤면 겹쳐 적는다. 둘 다
/// 왕복·재내보내기 안정·시작 보정 범위를 지킨다. 결정 문서의 식(계수 16, 기본량 64, 시작 기준)을
/// 그대로 고정한다.
#[test]
fn rest_budget_exact_boundary_opens_17th_voice_at_351_bars_and_folds_at_352() {
    let late_pad = |bars: u64, k: u64| {
        let s = bars * W;
        let ns: Vec<Note> = (0..k)
            .map(|i| note(s + i * 240, 8 * W, u8::try_from(40 + i * 2).unwrap(), None))
            .collect();
        clip(s + (k - 1) * 240 + 8 * W, &ns)
    };
    let header = Header::default();
    let fits = round_trip(&header, 1, &late_pad(351, 17)).unwrap();
    assert_eq!(fits.matches("@track").count(), 17);
    assert!(!fits.contains('~'));
    assert!(parse(&fits, Mode::Strict).is_ok());
    let folds = round_trip(&header, 1, &late_pad(352, 17)).unwrap();
    assert_eq!(folds.matches("@track").count(), 16, "{folds}");
    assert!(folds.contains("64~+0:+"));
}

/// 성부 17개 이상이 늦은 시각에 몰리는 무작위 클립(못 깨뜨림): 긴 쉼표, 그리드·비그리드 시작,
/// 1틱–u32 끝 길이, 화음·가사·드럼, 박자표 넷, 앞쪽에 흩어진 노트(면제 성부가 예산을 먼저
/// 쓰게). 예산 경계 근처의 겹쳐 적기와 거슬러 접기를 지나며 왕복, 경고 0, 클립 길이, 재내보내기
/// 안정, 시작 보정 −480..=+59를 본다(release에서 시드를 바꿔 20,000 사례도 돌렸다).
#[test]
fn random_late_clusters_beyond_16_voices_round_trip_with_offsets_in_range() {
    struct Lcg(u64);
    impl Lcg {
        fn below(&mut self, n: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % n.max(1)
        }
    }
    let headers: Vec<Header> = ["4/4", "3/4", "7/8", "5/16"]
        .iter()
        .map(|t| {
            *parse(&format!("@time {t}\n@track T ch1\nc"), Mode::Lenient)
                .unwrap()
                .header()
        })
        .collect();
    let mut r = Lcg(11);
    let mut folded = 0;
    for case in 0..200 {
        let header = headers[usize::try_from(r.below(4)).unwrap()];
        let ch = if r.below(5) == 0 { 10 } else { 1 };
        let bars = 1 + r.below(500);
        let g = bars * W + r.below(4) * 60;
        let k = 14 + r.below(40);
        let mut ns = Vec::new();
        for _ in 0..r.below(20) {
            ns.push(note(r.below(g), 1 + r.below(8000), 60, None));
        }
        for i in 0..k {
            let start = g + match r.below(3) {
                0 => i * 240,
                1 => r.below(1000),
                _ => r.below(4) * 640,
            };
            let len = match r.below(4) {
                0 => (1 + r.below(10)) * W,
                1 => 1 + r.below(100),
                2 if r.below(6) == 0 => u64::from(u32::MAX) - start - r.below(1000),
                _ => 1 + r.below(30_000),
            };
            let pitch = u8::try_from(if ch == 10 {
                35 + r.below(47)
            } else {
                r.below(128)
            })
            .unwrap();
            let lyric = if r.below(4) == 0 { Some("가") } else { None };
            ns.push(note(start, len, pitch, lyric));
        }
        let max_end = ns.iter().map(Note::end).max().unwrap();
        // 노트 끝 뒤 여유는 짧은 클립에만 둔다. u32 끝 근처 노트 뒤의 여유는 클립 끝까지 쉼표 약
        // 110만 개가 되어(리뷰 3 시도 8, 클립 길이에 선형) 시험만 느려진다.
        let pad = if max_end < 1 << 24 { r.below(50) } else { 0 };
        let c = clip(max_end + pad, &ns);
        let text = round_trip(&header, ch, &c).unwrap_or_else(|e| panic!("사례 {case}: {e}"));
        if text.contains("64~+") {
            folded += 1;
        }
    }
    assert!(folded > 30, "겹쳐 적기 경로를 지난 사례가 적다: {folded}");
}
