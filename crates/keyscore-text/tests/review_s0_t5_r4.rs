//! S0-T5 적대적 리뷰 4회차의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5/review-4.md` 7절), 통과하는 테스트는
//! 깨뜨리지 못한 시도를 회귀 테스트로 남긴 것이다.

use std::fmt::Write as _;

use keyscore_core::{Channel, Clip, Note, Pitch, Tick, Track, TrackColor, Velocity};
use keyscore_text::{ExportTrack, Header, Mode, export, parse};

const W: u32 = 3840;
type View = (u64, u64, u8, Option<String>);

fn note(start: u64, length: u32, pitch: u8, lyric: Option<&str>) -> Note {
    let n = Note::new(
        Tick::new(start),
        length,
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
        c.insert_note(n.clone()).unwrap();
    }
    c
}

/// 클립을 `at`에서 나눠 앞 조각을 돌려준다. `at`을 지나는 노트는 숨김 꼬리가 된다.
fn split_at(c: Clip, at: u64) -> Clip {
    let mut t = Track::new("x", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let id = t.insert_clip(c).unwrap();
    t.split_clip(id, Tick::new(at)).unwrap();
    t.clip(id).unwrap().clone()
}

/// 노트 비교 기준: 숨김 꼬리는 클립 끝에서 자른 길이로 본다(내보내기 문서의 예외).
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
/// 같은 문자열인지. 내보낸 텍스트를 돌려준다.
fn round_trip(header: &Header, ch: u8, c: &Clip) -> Result<String, String> {
    let text = export_with(header, ch, c);
    let p = parse(&text, Mode::Lenient).map_err(|e| format!("{e}\n{text}"))?;
    if !p.warnings().is_empty() {
        return Err(format!("경고 {:?}\n{text}", p.warnings()));
    }
    let back = p.tracks()[0].clip();
    if view(back) != view(c) {
        return Err(format!("노트가 다르다\n{text}"));
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
        return Err(format!("재내보내기가 다르다\n{text}"));
    }
    Ok(text)
}

/// 내보낸 텍스트의 `~` 시작 보정이 모두 −480..=+59 안인지(성부 커서 앞지름 한도와 쉼표 채움).
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

/// 리뷰 3 지적 1의 입력(긴 쉼표 G 뒤 서로 겹친 노트 V개).
fn overlap_after_rest(prefix: &str, gap_rests: u64, overlapping: u64) -> String {
    let mut text = String::from("@track A ch1\n");
    text.push_str(prefix);
    for _ in 0..gap_rests {
        text.push_str("r1.. ");
    }
    for i in 0..overlapping {
        let _ = write!(text, "c64~+0:+{} ", 60 * (overlapping - i) + 1000);
    }
    text
}

/// `bars` 마디 쉰 뒤 4분음표 간격으로 `k`음이 차례로 들어와 함께 끝나는 패드. 모두 그리드 위다.
fn late_build_up(bars: u64, k: u64) -> Clip {
    let s = bars * u64::from(W);
    let ns: Vec<Note> = (0..k)
        .map(|i| {
            note(
                s + i * 960,
                4 * W - u32::try_from(i * 960).unwrap(),
                u8::try_from(60 + i * 3).unwrap(),
                None,
            )
        })
        .collect();
    clip(s + 4 * u64::from(W), &ns)
}

/// 리뷰 4 지적 1. 성부 예산은 「마지막 노트 끝까지의 온음표 수」에 비례하는데, 느슨 모드에서는
/// 길이 보정 노트 하나(`c1~+0:+4294000000`, 18바이트)로 이 수를 약 110만으로 만들 수 있다. 그러면
/// 리뷰 3 입력에서 예산이 바닥나지 않아 겹친 노트마다 성부를 열고 쉼표를 다시 적는다(제곱 증폭이
/// 그대로 돌아온다). 같은 입력에 18바이트를 더했을 뿐이다. 기준은 리뷰 2·3과 같은 64배다.
#[test]
fn export_size_stays_within_64x_input_when_long_note_inflates_rest_budget() {
    let plain = overlap_after_rest("", 300, 300);
    let inflated = overlap_after_rest("c1~+0:+4294000000 ", 300, 300);
    for text in [plain, inflated] {
        let p = parse(&text, Mode::Lenient).unwrap();
        let out = export_with(p.header(), 1, p.tracks()[0].clip());
        assert!(
            out.len() <= 64 * text.len(),
            "입력 {}바이트 → 내보내기 {}바이트(성부 {}개)",
            text.len(),
            out.len(),
            out.matches("@track").count()
        );
    }
}

/// 리뷰 4 지적 2. 그리드 위 노트만 있는 클립(100마디 쉰 뒤 5음 패드)이 예산에 닿아 겹쳐 적기가
/// 일어나고, 그리드 노트에 `~`가 붙어 엄격 모드로 읽히지 않는다. 99마디면 5성부 타이로 나간다.
/// REQ 5.14는 `~`를 「그리드에 안 맞는 노트」에 쓴다고 하고, 결정 문서는 「평범한 곡은 예산에
/// 닿지 않는다」고 적었다.
#[test]
fn export_keeps_grid_notes_free_of_offsets_when_chord_enters_after_100_bars() {
    let header = Header::default();
    for bars in [99, 100, 400] {
        let text = round_trip(&header, 1, &late_build_up(bars, 5)).unwrap();
        assert!(!text.contains('~'), "{bars}마디: 그리드 노트에 ~\n{text}");
        assert!(
            parse(&text, Mode::Strict).is_ok(),
            "{bars}마디: 엄격 모드 실패"
        );
    }
}

/// 예산 경계 바로 아래(99마디)와 위(100마디): 둘 다 왕복하고, 위쪽은 실제로 겹쳐 적으며, 시작
/// 보정은 −480..=+59 안이다. 못 깨뜨림.
#[test]
fn rest_budget_boundary_round_trips_with_start_offsets_in_range() {
    let header = Header::default();
    let below = round_trip(&header, 1, &late_build_up(99, 5)).unwrap();
    assert_eq!(below.matches("@track").count(), 5, "{below}");
    let above = round_trip(&header, 1, &late_build_up(100, 5)).unwrap();
    assert!(
        above.contains("64~+0:+"),
        "겹쳐 적기가 일어나지 않았다\n{above}"
    );
    for text in [below, above] {
        start_offsets_in_range(&text).unwrap();
    }
}

/// 늦은 시각에 겹친 사건이 몰린 무작위 클립: 1틱–u32 끝 길이, 셋잇단 후보(640 간격), 타이 사슬,
/// 화음·가사·드럼(라벨·`n<번호>`), 숨김 꼬리, 박자표 넷, 클립 길이 여유. 예산 밖 겹쳐 적기와 마지막
/// 사건 거슬러 접기를 지나며 노트 왕복, 경고 0, 클립 길이 상한, 재내보내기 안정, 시작 보정
/// −480..=+59를 본다. 못 깨뜨림(release 17,000 사례에서도).
#[test]
fn random_late_overlap_clusters_round_trip_with_start_offsets_in_range() {
    struct Lcg(u64);
    impl Lcg {
        fn below(&mut self, n: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % n.max(1)
        }
        fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
            xs[usize::try_from(self.below(xs.len() as u64)).unwrap()]
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
    let whole = u64::from(W);
    let mut r = Lcg(7);
    let mut folded = 0;
    for case in 0..300 {
        let header = headers[usize::try_from(r.below(4)).unwrap()];
        let ch = if r.below(5) == 0 { 10 } else { 1 };
        let k = r.below(30) + 3;
        let g = (r.below(40) + 1) * whole + r.below(4) * 60 + r.below(3);
        let mut ns = Vec::new();
        for _ in 0..k {
            let start = match r.below(5) {
                0 => r.below(g),
                1 => g + r.below(20) * 60,
                2 => g + r.below(600),
                3 => g + r.below(8) * 640,
                _ => g + r.below(3),
            };
            let len: u64 = match r.below(8) {
                0 => 1 + r.below(70),
                1 => r.pick(&[640u64, 320, 1280, 160, 2560]),
                2 => r.pick(&[60u64, 240, 960, 3840, 6720, 1440]),
                3 => (1 + r.below(18)) * whole + r.pick(&[0u64, 60, 960, 100]),
                4 => 1 + r.below(20_000),
                5 if r.below(8) == 0 => u64::from(u32::MAX) - start - r.below(1000),
                _ => 100 + r.below(5000),
            }
            .max(1);
            let pitch = if ch == 10 {
                r.pick(&[36u64, 38, 42, 20, 0, 127])
            } else {
                r.below(128)
            };
            let lyric = r.pick(&[Some("la"), Some(""), Some("a\\\"b"), None, None, None]);
            let reps = if r.below(4) == 0 { 2 + r.below(3) } else { 1 };
            for j in 0..reps {
                ns.push(note(
                    start,
                    u32::try_from(len).unwrap(),
                    u8::try_from((pitch + j * 4) % 128).unwrap(),
                    if j + 1 == reps { lyric } else { None },
                ));
            }
        }
        let max_end = ns.iter().map(Note::end).max().unwrap();
        let last_start = ns.iter().map(|n| n.start().get()).max().unwrap();
        let pad = if r.below(3) == 0 { r.below(100) } else { 0 };
        let mut c = Clip::new(Tick::ZERO, max_end + pad).unwrap();
        for n in &ns {
            // 같은 노트 중복은 core가 받지 않을 수 있으므로 건너뛴다.
            let _ = c.insert_note(n.clone());
        }
        if r.below(4) == 0 && max_end > last_start + 1 {
            c = split_at(c, last_start + 1 + r.below(max_end - last_start - 1));
        }
        let text = round_trip(&header, ch, &c).unwrap_or_else(|e| panic!("사례 {case}: {e}"));
        start_offsets_in_range(&text).unwrap_or_else(|e| panic!("사례 {case}: {e}\n{text}"));
        if text.contains("64~+") {
            folded += 1;
        }
    }
    assert!(folded > 50, "겹쳐 적기 경로를 지난 사례가 적다: {folded}");
}
