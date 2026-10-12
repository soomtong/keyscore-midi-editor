//! S0-T5 적대적 리뷰 2회차의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 리뷰가 찾은 결함이고(`docs/reports/s0-t5/review-2.md` 7절), 통과하는 테스트는
//! 깨뜨리지 못한 시도를 회귀 테스트로 남긴 것이다.

use keyscore_core::{Channel, Clip, Note, Pitch, Tick, Velocity};
use keyscore_text::{ExportTrack, Header, Mode, export, parse};

type View = (u64, u64, u8, Option<String>);

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

fn export_one(ch: u8, c: &Clip) -> String {
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

/// 결정 문서의 클립 길이 차이 상한.
fn bound(length: u64) -> u64 {
    match length {
        150.. => 7,
        60..150 => 15,
        _ => 60 - length,
    }
}

/// 내보낸 텍스트의 `~` 시작 보정 값들.
fn start_offsets(text: &str) -> Vec<i64> {
    text.split('~')
        .skip(1)
        .map(|s| {
            let end = s.find([':', ' ', '\n', '"']).unwrap_or(s.len());
            s[..end].parse::<i64>().unwrap()
        })
        .collect()
}

/// 내보내기 → 가져오기: 노트 일치, 경고 없음, 클립 길이 차이가 결정의 상한 안, 시작 보정이
/// −480..+59 안, 다시 내보내면 같은 문자열. 내보낸 텍스트를 돌려준다.
fn check(ch: u8, c: &Clip) -> String {
    let text = export_one(ch, c);
    let p = parse(&text, Mode::Lenient).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert!(p.warnings().is_empty(), "{:?}\n{text}", p.warnings());
    let got = p.tracks()[0].clip().clone();
    assert_eq!(view(&got), view(c), "{text}");
    let diff = got.length().abs_diff(c.length());
    assert!(
        diff <= bound(c.length()),
        "클립 {} → {}\n{text}",
        c.length(),
        got.length()
    );
    for so in start_offsets(&text) {
        assert!((-480..=59).contains(&so), "시작 보정 {so}\n{text}");
    }
    assert_eq!(export_one(ch, &got), text, "재내보내기 불안정");
    text
}

/// 결정적 의사 난수(테스트 재현용).
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

// ---- 깨뜨린 시도 ----

/// 결정(ktext-implementation-details) 「`~` 보정 한도 … 작은 입력으로 내보낼 수 없는 거대한
/// 클립을 만들지 못하게」와 `parse.rs`의 `MAX_LENGTH_OFFSET` 주석 「증폭이 생기지 않는다」.
/// 길이 보정 한도가 `u32::MAX`라 18바이트 토큰 하나가 온음표 1,118,481개짜리 타이 사슬(약 5 MB)로
/// 내보내진다. 노트 수에 비례해 커지므로 몇 KB 입력이 GB 단위 내보내기가 된다
/// (release 측정: 157바이트 → 40 MB, 1.2초).
#[test]
fn length_offset_does_not_amplify_export_size() {
    let text = "@track A ch1\nc4~+0:+4294963200 c4~+0:+4294963200";
    let p = parse(text, Mode::Lenient).unwrap();
    let c = p.tracks()[0].clip();
    assert_eq!(c.notes().len(), 2);
    let out = export_one(1, c);
    assert!(
        out.len() <= 64 * text.len(),
        "입력 {}바이트 → 내보내기 {}바이트",
        text.len(),
        out.len()
    );
}

// ---- 깨뜨리지 못한 시도 ----

/// 노트가 클립 아무 곳에나 있고(숨김 꼬리 포함) 클립 길이가 노트 끝 근처 위아래에 있을 때
/// 클립 길이 상한·재내보내기 안정·시작 보정 범위. 구현자 테스트는 시작 0의 노트만 본다.
#[test]
fn clip_length_bound_and_offsets_hold_for_random_clips() {
    let mut r = Lcg(7);
    for case in 0..3000u64 {
        let n = r.below(20);
        let mut ns = Vec::new();
        for _ in 0..n {
            let (s, l) = match case % 4 {
                0 => (r.below(3000), 1 + r.below(300)),
                1 => (r.below(300), 1 + r.below(5)),
                2 => (
                    r.below(16) * 60 + r.below(3),
                    [
                        1, 2, 40, 59, 60, 61, 80, 105, 119, 135, 140, 3840, 3900, 7680,
                    ][usize::try_from(r.below(14)).unwrap()],
                ),
                _ => (r.below(100), 1 + r.below(10_000)),
            };
            let lyric = (r.below(5) == 0).then_some("a");
            ns.push((
                s,
                u32::try_from(l).unwrap(),
                u8::try_from(r.below(128)).unwrap(),
                lyric,
            ));
        }
        let last_start = ns.iter().map(|n| n.0).max().unwrap_or(0);
        let max_end = ns.iter().map(|n| n.0 + u64::from(n.1)).max().unwrap_or(1);
        let length = match r.below(3) {
            // 숨김 꼬리: 마지막 시작 뒤, 노트 끝 앞
            0 => last_start + 1 + r.below(max_end.saturating_sub(last_start).max(1)),
            1 => max_end + r.below(200),
            _ => last_start + 1 + r.below(300),
        };
        let mut c = Clip::new(Tick::ZERO, length).unwrap();
        for &(s, l, p, y) in &ns {
            let note = Note::new(
                Tick::new(s),
                l,
                Pitch::new(p).unwrap(),
                Velocity::from_midi1(100).unwrap(),
            )
            .unwrap();
            // 같은 노트 중복 등으로 거부되면 건너뛴다(core 규칙).
            let _ = c.insert_note(match y {
                Some(y) => note.with_lyric(y),
                None => note,
            });
        }
        check(if case % 3 == 0 { 10 } else { 1 }, &c);
    }
}

/// `~` 한도 경계: 정확히 ±3840은 받고 그 바깥은 두 모드 모두 오류, 길이 보정 ±`u32::MAX` 경계,
/// i64 끝값. 패닉 없이 위치가 `~`인 오류다.
#[test]
fn offset_cap_boundaries() {
    let ok = |body: &str| {
        let p = parse(&format!("@track A ch1\n{body}"), Mode::Lenient)
            .unwrap_or_else(|e| panic!("{body}: {e}"));
        p.tracks()[0]
            .clip()
            .notes()
            .iter()
            .map(|n| (n.start().get(), n.length()))
            .collect::<Vec<_>>()
    };
    assert_eq!(ok("c~+3840"), vec![(3840, 960)]);
    assert_eq!(ok("r1 c~-3840"), vec![(0, 960)]);
    assert_eq!(ok("c~-0:+0"), vec![(0, 960)]);
    assert_eq!(ok("c64~+0:+4294967235"), vec![(0, u32::MAX)]);
    assert_eq!(ok("c~+3840:+4294966335"), vec![(3840, u32::MAX)]);
    for body in [
        "c~+3841",
        "r1 r1 c~-3841",
        "c~-3840",
        "c~+4294967295",
        "c~-9223372036854775807",
        "c~-9223372036854775808",
        "c~+0:+4294967296",
        "c~+0:-9223372036854775807",
        "c~+0:+18446744073709551615",
        "c64~+0:+4294967236",
        "c~+0:-960",
    ] {
        let text = format!("@track A ch1\n{body}");
        let e = parse(&text, Mode::Lenient).unwrap_err();
        assert_eq!(e.diagnostic().line(), 2, "{body}");
        assert!(parse(&text, Mode::Strict).is_err(), "{body}");
    }
}

/// 1틱 노트가 매 틱 이어지거나(커서가 노트마다 59틱 앞지름), 같은 시작에 길이 1·2·3 노트가
/// 매 틱 있을 때. 시작 보정은 −480 이상으로 묶이고 왕복한다. 성부는 겹침(3)의 약 60배(176개)까지
/// 늘지만 입력 노트 수와 클립 길이에 선형이다(관찰, 결함 아님).
#[test]
fn dense_short_notes_keep_offsets_in_cap_and_round_trip() {
    let ns: Vec<_> = (0..600).map(|i| (i, 1, 60, None)).collect();
    check(1, &clip(600, &ns));
    let ns: Vec<_> = (0..300u64)
        .flat_map(|i| [(i, 1, 60, None), (i, 2, 61, None), (i, 3, 62, Some("x"))])
        .collect();
    let text = check(1, &clip(303, &ns));
    assert!(text.matches("@track").count() <= 64 * 3, "{text}");
    // 두 덩어리 사이 큰 간격: 성부마다 간격을 쉼표로 채운다.
    let mut ns: Vec<_> = (0..600).map(|i| (i, 1, 60, None)).collect();
    ns.extend((0..600).map(|i| (1_000_000 + i, 1, 60, None)));
    check(1, &clip(1_000_600, &ns));
}
