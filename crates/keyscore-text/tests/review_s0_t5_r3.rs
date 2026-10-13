//! S0-T5 적대적 리뷰 3회차의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5/review-3.md` 7절), 통과하는 테스트는
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

/// 리뷰 3 지적 1. 긴 쉼표 G 뒤에 서로 겹치는 긴 노트 V개를 둔다. 원문은 한 블록에 길이 보정으로
/// 겹침을 적어 입력이 G + V에 비례하지만, 내보내기는 겹친 노트마다 성부를 나누고 성부마다 처음부터
/// G까지 쉼표를 다시 적어 출력이 G × V에 비례한다(입력을 두 배로 하면 출력이 네 배).
/// 리뷰 2의 기준(입력의 64배)을 그대로 쓴다.
#[test]
fn export_size_stays_within_64x_input_when_overlapping_notes_follow_long_rest() {
    let (gap_rests, overlapping) = (300u64, 300u64);
    let mut text = String::from("@track A ch1\n");
    for _ in 0..gap_rests {
        text.push_str("r1.. ");
    }
    for i in 0..overlapping {
        // 60틱 간격으로 시작하고 모두 마지막 노트 시작 뒤까지 이어진다.
        let _ = write!(text, "c64~+0:+{} ", 60 * (overlapping - i) + 1000);
    }
    let p = parse(&text, Mode::Lenient).unwrap();
    let out = export_with(p.header(), 1, p.tracks()[0].clip());
    assert!(
        out.len() <= 64 * text.len(),
        "입력 {}바이트 → 내보내기 {}바이트",
        text.len(),
        out.len()
    );
}

/// 타이 사슬 16/17조각 경계와 그 주변(남는 조각이 점음표·64분음표·표기 불가, u32 끝)을 화음·드럼
/// 라벨·드럼 번호·가사·시작 보정·숨김 꼬리·뒤따르는 노트와 엮는다. 못 깨뜨림.
#[test]
fn tie_chain_cap_boundaries_round_trip_with_chords_drums_lyrics_and_tails() {
    let lengths = [
        15 * W + 60,
        15 * W + 1920,
        15 * W + 3360,
        16 * W,
        16 * W + 60,
        16 * W + 2880,
        17 * W,
        15 * W + 100,
        16 * W + 100,
        6720,
        7200,
        u32::MAX - 6720,
        u32::MAX / W * W,
        u32::MAX,
    ];
    let shapes: [(u8, &[u8]); 6] = [
        (1, &[60]),
        (1, &[60, 64, 67]),
        (1, &[0, 127]),
        (10, &[36]),
        (10, &[36, 38]),
        (10, &[20]),
    ];
    let header = Header::default();
    let mut failures = Vec::new();
    for &len in &lengths {
        for start in [0u64, 1, 59, 3840] {
            for &(ch, pitches) in &shapes {
                for lyric in [None, Some("la")] {
                    let top = pitches.len() - 1;
                    let ns: Vec<Note> = pitches
                        .iter()
                        .enumerate()
                        .map(|(i, &p)| note(start, len, p, lyric.filter(|_| i == top)))
                        .collect();
                    let end = start + u64::from(len);
                    let mut cases = vec![clip(end, &ns)];
                    for cut in [start + 1, start + 6721, start + 16 * 3840, end - 1] {
                        if cut > start && cut < end {
                            cases.push(split_at(clip(end, &ns), cut));
                        }
                    }
                    if len < u32::MAX - 6720 {
                        // 같은 성부로 이어지는 다음 노트(가사 있음).
                        let mut more = ns.clone();
                        more.push(note(end, 960, pitches[0], Some("x")));
                        cases.push(clip(end + 960, &more));
                    }
                    for c in &cases {
                        match round_trip(&header, ch, c) {
                            // 노트 하나·화음 하나의 출력은 한도 덕에 상수다(16조각 + 뒤 노트).
                            Ok(text) if text.len() > 1000 => failures.push(format!(
                                "len {len} start {start} ch{ch}: {}바이트",
                                text.len()
                            )),
                            Ok(_) => {}
                            Err(e) => failures.push(format!("len {len} start {start} ch{ch}: {e}")),
                        }
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures[0]);
}

/// 긴 노트(14–18 온음표 + 나머지, u32 끝 근처)를 섞은 무작위 클립: 박자표 넷, 드럼, 가사(빈 가사,
/// 따옴표·역슬래시), 숨김 꼬리, 클립 길이 위쪽 여유. 노트 왕복, 경고 0, 클립 길이 상한, 재내보내기
/// 안정, 노트당 출력 상수(4 KB)를 본다. 못 깨뜨림(release 100,000 사례에서도).
#[test]
fn random_clips_with_long_notes_round_trip_and_stay_small() {
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
    let rems = [
        0u64, 60, 90, 105, 120, 180, 210, 240, 360, 420, 480, 720, 840, 960, 1440, 1680, 1920,
        2880, 3360,
    ];
    let whole = u64::from(W);
    let mut r = Lcg(42);
    for case in 0..3000 {
        let header = headers[usize::try_from(r.below(4)).unwrap()];
        let ch = if r.below(5) == 0 { 10 } else { 1 };
        let k = r.below(8) + 1;
        let mut ns = Vec::new();
        for _ in 0..k {
            let start = match r.below(3) {
                0 => r.below(20) * 960,
                1 => r.below(80_000),
                _ => r.below(4) * whole + r.below(3) * 60,
            };
            let len = match r.below(4) {
                0 => (14 + r.below(5)) * whole + r.pick(&rems),
                1 => r.below(20) * whole + r.pick(&rems) + r.below(2),
                2 => 1 + r.below(200),
                _ if r.below(10) == 0 => u64::from(u32::MAX) - r.below(10_000),
                // u32 끝 근처의 온음표 배수 + 표기 길이: 한도가 없으면 타이 백만 조각이 된다.
                _ if r.below(10) == 0 => {
                    u64::from(u32::MAX / W - 1 - u32::try_from(r.below(4)).unwrap()) * whole
                        + r.pick(&rems)
                }
                _ => 17 * whole + r.below(5000),
            }
            .max(1);
            let pitch = if ch == 10 {
                35 + r.below(47)
            } else {
                r.below(128)
            };
            let lyric = r.pick(&[Some("la"), Some(""), Some("a\\\"b \\"), None, None, None]);
            ns.push(note(
                start,
                u32::try_from(len).unwrap(),
                u8::try_from(pitch).unwrap(),
                lyric,
            ));
        }
        let max_end = ns.iter().map(Note::end).max().unwrap();
        let last_start = ns.iter().map(|n| n.start().get()).max().unwrap();
        let pad = if r.below(3) == 0 && max_end < 1_000_000 {
            r.below(100)
        } else {
            0
        };
        let mut c = Clip::new(Tick::ZERO, max_end + pad).unwrap();
        for n in &ns {
            // 같은 노트 중복은 core가 받지 않을 수 있으므로 건너뛴다.
            let _ = c.insert_note(n.clone());
        }
        if r.below(3) == 0 && max_end > last_start + 1 {
            c = split_at(c, last_start + 1 + r.below(max_end - last_start - 1));
        }
        let text = round_trip(&header, ch, &c).unwrap_or_else(|e| panic!("사례 {case}: {e}"));
        assert!(
            text.len() < 4000 * usize::try_from(k).unwrap(),
            "사례 {case}: {}바이트",
            text.len()
        );
    }
}

/// 타이 한도의 부수 효과 관찰: 타이 16조각 안에 드는 그리드 노트는 타이로(`~` 없이) 나가 엄격
/// 모드로도 읽히지만, 16조각으로 적을 수 없는 노트는 `~+0:+차이`로 나가 엄격 모드가 읽지 못한다
/// (리뷰 3 지적 2, 제안).
///
/// (구현자 수정, S0-T5 리뷰 7 지적 2) 처음에는 16온음표(타이)와 17온음표(`~`)를 비교했다. 리뷰 7
/// 지적 2에 따라 타이는 조각 수가 가장 적은 분해(겹점 온음표 포함)를 쓰므로 17온음표도 10조각 타이가
/// 된다. 경계를 겹점 온음표 16개(16조각, 타이)와 거기에 4분음표를 더한 길이(17조각, `~`)로 옮겼다.
/// 검사하는 성질은 그대로다.
#[test]
fn grid_note_beyond_16_tie_pieces_exports_with_offset_unlike_16_pieces() {
    let header = Header::default();
    let sixteen = clip(16 * 3840, &[note(0, 16 * W, 60, None)]);
    let text = round_trip(&header, 1, &sixteen).unwrap();
    assert!(!text.contains('~'), "{text}");
    assert!(parse(&text, Mode::Strict).is_ok(), "{text}");
    let at_cap = clip(16 * 6720, &[note(0, 16 * 6720, 60, None)]);
    let text = round_trip(&header, 1, &at_cap).unwrap();
    assert_eq!(text.matches('&').count(), 15, "{text}");
    assert!(parse(&text, Mode::Strict).is_ok(), "{text}");

    let beyond = clip(16 * 6720 + 960, &[note(0, 16 * 6720 + 960, 60, None)]);
    let text = round_trip(&header, 1, &beyond).unwrap();
    assert!(text.contains("c1..~+0:+101760"), "{text}");
    assert!(parse(&text, Mode::Strict).is_err(), "{text}");
}
