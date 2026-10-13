//! 내보내기 테스트(S0-T5 계획 테스트 4와 보조 테스트). 공개 API만 쓴다.

mod common;

use common::{NoteView, n, nl, notes, ok};
use keyscore_core::{
    Channel, Clip, Key, Note, Pitch, PitchClass, Program, Tempo, Tick, TimeSignature, Velocity,
};
use keyscore_text::{ExportTrack, Header, Mode, export};

fn clip(length: u64, ns: &[NoteView]) -> Clip {
    let mut c = Clip::new(Tick::ZERO, length).unwrap();
    for (s, l, p, lyric) in ns {
        let note = Note::new(
            Tick::new(*s),
            *l,
            Pitch::new(*p).unwrap(),
            Velocity::from_midi1(80).unwrap(),
        )
        .unwrap();
        let note = match lyric {
            Some(t) => note.with_lyric(t.clone()),
            None => note,
        };
        c.insert_note(note).unwrap();
    }
    c
}

fn track<'a>(name: &'a str, ch: u8, c: &'a Clip) -> ExportTrack<'a> {
    ExportTrack {
        name,
        channel: Channel::new(ch).unwrap(),
        program: None,
        bank: None,
        clip: c,
    }
}

/// 내보낸 뒤 다시 읽은 첫 트랙의 (노트, 클립 길이).
fn reparse(text: &str) -> (Vec<NoteView>, u64) {
    let p = ok(text, Mode::Lenient);
    assert!(p.warnings().is_empty(), "{:?}\n{text}", p.warnings());
    let c = p.tracks()[0].clip();
    (notes(c), c.length())
}

#[test]
fn export_snapshot() {
    let lead = clip(
        15_360,
        &[
            nl(0, 480, 60, "사"),
            n(480, 480, 62),
            n(960, 960, 64),
            n(1920, 960, 60),
            n(1920, 960, 64),
            nl(1920, 960, 67, "랑"),
            // 녹음된 비그리드 노트.
            n(2893, 455, 65),
            // 겹친 지속음은 성부를 나눈다.
            n(0, 3840, 48),
            // 8분 셋잇단.
            n(3840, 320, 60),
            n(4160, 320, 62),
            n(4480, 320, 64),
            // 온음표 둘 길이.
            n(5760, 7680, 72),
        ],
    );
    let drums = clip(
        1920,
        &[
            n(0, 240, 36),
            n(0, 240, 42),
            n(240, 240, 42),
            n(480, 240, 38),
            n(720, 240, 75),
        ],
    );
    let header = Header::new(
        Some(Tempo::from_bpm(120).unwrap()),
        Some(TimeSignature::COMMON),
        Some(Key::Major(PitchClass::new(0).unwrap())),
    );
    let mut lead_track = track("Lead", 1, &lead);
    lead_track.program = Some(Program::new(0).unwrap());
    let text = export(&header, &[lead_track, track("Drums", 10, &drums)]);
    insta::assert_snapshot!(text);
    let p = ok(&text, Mode::Lenient);
    assert!(p.warnings().is_empty(), "{:?}", p.warnings());
    assert_eq!(notes(p.tracks()[0].clip()), notes(&lead));
    assert_eq!(notes(p.tracks()[1].clip()), notes(&drums));
}

#[test]
fn export_grid_notes_have_no_offset_suffix() {
    let c = clip(
        3840,
        &[
            n(0, 960, 60),
            n(960, 480, 62),
            n(1440, 720, 64),
            n(2400, 240, 65),
            n(2880, 960, 67),
        ],
    );
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert!(!text.contains('~'), "{text}");
    assert_eq!(reparse(&text), (notes(&c), 3840));
}

#[test]
fn export_long_note_uses_ties_and_low_pitch_uses_octave_below_zero() {
    let c = clip(12_000, &[nl(0, 7680 + 960, 0, "길"), n(8640, 3360, 127)]);
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert!(text.contains('&'), "{text}");
    assert!(text.contains("o0 <"), "{text}");
    assert_eq!(reparse(&text), (notes(&c), 12_000));
}

#[test]
fn export_drum_note_without_label_uses_number() {
    let c = clip(960, &[n(0, 480, 75), n(480, 480, 36)]);
    let text = export(&Header::default(), &[track("D", 10, &c)]);
    assert!(text.contains("n75"), "{text}");
    assert!(text.contains("bd"), "{text}");
    assert_eq!(reparse(&text), (notes(&c), 960));
}

#[test]
fn export_tempo_uses_shortest_decimal_that_round_trips() {
    let header = Header::new(
        Some(Tempo::from_micros_per_quarter(497_925).unwrap()),
        Some(TimeSignature::new(7, 8).unwrap()),
        Some(Key::None),
    );
    let c = clip(960, &[n(0, 960, 60)]);
    let text = export(&header, &[track("T", 1, &c)]);
    assert!(text.contains("@tempo 120.5\n"), "{text}");
    assert!(text.contains("@time 7/8\n"), "{text}");
    assert!(text.contains("@key none\n"), "{text}");
    assert_eq!(*ok(&text, Mode::Lenient).header(), header);
}

#[test]
fn export_clip_length_beyond_last_note_is_kept_with_rests() {
    // 클립 끝까지 쉼표를 써서 가져온 클립 길이가 원본과 같다.
    let c = clip(7680, &[n(0, 100, 60)]);
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert_eq!(reparse(&text), (notes(&c), 7680));
    // 노트가 없는 클립도 쉼표로 길이를 지킨다.
    let empty = clip(3840 + 195, &[]);
    let text = export(&Header::default(), &[track("T", 1, &empty)]);
    assert_eq!(reparse(&text), (vec![], 3840 + 195));
}

#[test]
fn export_unreachable_clip_length_uses_nearest_reachable() {
    // 쉼표 합(15틱 단위)으로도, 노트 끝으로도 닿지 않는 클립 길이는 문법으로 적을 수 없다.
    // 내보내기는 위아래 가운데 가장 가까운 닿는 길이를 쓴다(같은 거리면 짧은 쪽).
    let c = clip(1000, &[n(0, 960, 60)]);
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert_eq!(reparse(&text), (notes(&c), 1005));
    let c = clip(997, &[n(0, 960, 60)]);
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert_eq!(reparse(&text), (notes(&c), 990));
    // 노트가 없고 60틱보다 짧으면 가장 짧은 쉼표(60틱)로 늘린다.
    let tiny = clip(7, &[]);
    let text = export(&Header::default(), &[track("T", 1, &tiny)]);
    assert_eq!(reparse(&text), (vec![], 60));
}

/// 클립 길이 차이의 상한: 150틱 이상은 7틱 이하, 60–149틱은 15틱 이하, 60틱 미만은 60 − 길이 이하
/// (60틱 미만에서 노트 끝보다 긴 길이는 쉼표 하나(60틱)로만 닿는다).
#[test]
fn export_clip_length_error_is_bounded() {
    let bound = |l: u64| match l {
        150.. => 7,
        60..150 => 15,
        _ => 60 - l,
    };
    for length in 1..2000u64 {
        let mut cases = vec![clip(length, &[])];
        if length >= 3 {
            cases.push(clip(length, &[n(0, 3, 60)]));
        }
        if length >= 50 {
            cases.push(clip(length, &[n(0, 50, 60), n(0, 25, 64)]));
        }
        for c in cases {
            let text = export(&Header::default(), &[track("T", 1, &c)]);
            let (got, got_length) = reparse(&text);
            assert_eq!(got, notes(&c), "{text}");
            let diff = got_length.abs_diff(length);
            assert!(diff <= bound(length), "{length} → {got_length}\n{text}");
        }
    }
}

#[test]
fn export_start_offsets_stay_within_parser_cap() {
    // 1틱 노트가 이어지면 노트마다 커서가 59틱씩 앞지른다. 앞지름이 쌓여 파서의 시작 보정
    // 한도(3840)를 넘지 않게 성부를 나눈다.
    let ns: Vec<NoteView> = (0..500).map(|i| n(i, 1, 60)).collect();
    let c = clip(500, &ns);
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    let worst = text
        .split('~')
        .skip(1)
        .filter_map(|s| {
            let end = s.find([':', ' ', '\n', '"']).unwrap_or(s.len());
            s[..end].parse::<i64>().ok()
        })
        .min()
        .unwrap_or(0);
    assert!(worst >= -480, "가장 작은 시작 보정 {worst}");
    assert_eq!(reparse(&text), (notes(&c), 500));
}

#[test]
fn export_tie_chain_is_capped_at_16_pieces() {
    // 16조각(온음표 15개 + 2분음표)까지는 타이, 넘으면 가장 가까운 표기 + 길이 보정 하나.
    let tied = clip(3840 * 15 + 1920, &[n(0, 3840 * 15 + 1920, 60)]);
    let text = export(&Header::default(), &[track("T", 1, &tied)]);
    assert_eq!(text.matches('&').count(), 15, "{text}");
    assert_eq!(reparse(&text), (notes(&tied), 3840 * 15 + 1920));
    let long = clip(3840 * 17, &[n(0, 3840 * 17, 60)]);
    let text = export(&Header::default(), &[track("T", 1, &long)]);
    assert!(!text.contains('&'), "{text}");
    assert!(text.contains("c1..~+0:+58560"), "{text}");
    assert_eq!(reparse(&text), (notes(&long), 3840 * 17));
}

#[test]
fn export_folds_overlaps_after_long_rest_and_stays_linear() {
    // 긴 쉼표 뒤의 겹친 노트를 성부마다 쉼표로 다시 채우지 않고 커서가 늦은 성부에 겹쳐 적는다
    // (S0-T5 리뷰 3 지적 1). 왕복, 경고 없음, 클립 길이, 시작 보정 −480..+59, 재내보내기 안정,
    // 노트 수와 마디 수에 선형인 크기를 본다.
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
    let mut r = Lcg(5);
    let mut folded = 0;
    for case in 0..60 {
        let gap = (50 + r.below(400)) * 3840 + r.below(3840);
        let count = 20 + r.below(200);
        let ch = if case % 4 == 0 { 10 } else { 1 };
        let mut ns: Vec<NoteView> = (0..r.below(8)).map(|i| n(i * 480, 480, 60)).collect();
        let mut at = gap;
        for _ in 0..count {
            at += r.below(200);
            let len = u32::try_from(1 + r.below(30_000)).unwrap();
            let pitch = u8::try_from(r.below(128)).unwrap();
            ns.push(if r.below(4) == 0 {
                nl(at, len, pitch, "가")
            } else {
                n(at, len, pitch)
            });
        }
        let max_end = ns.iter().map(|x| x.0 + u64::from(x.1)).max().unwrap();
        let length = max_end + r.below(100);
        let mut c = Clip::new(Tick::ZERO, length).unwrap();
        for (s, l, p, y) in &ns {
            let note = Note::new(
                Tick::new(*s),
                *l,
                Pitch::new(*p).unwrap(),
                Velocity::from_midi1(80).unwrap(),
            )
            .unwrap();
            let _ = c.insert_note(match y {
                Some(t) => note.with_lyric(t.clone()),
                None => note,
            });
        }
        let text = export(&Header::default(), &[track("T", ch, &c)]);
        let p = ok(&text, Mode::Lenient);
        assert!(p.warnings().is_empty(), "사례 {case}");
        let back = p.tracks()[0].clip();
        let mut want = notes(&c);
        want.sort();
        let mut got = notes(back);
        got.sort();
        assert_eq!(got, want, "사례 {case}");
        assert!(back.length().abs_diff(length) <= 7, "사례 {case}");
        for s in text.split('~').skip(1) {
            let e = s.find([':', ' ', '\n', '"']).unwrap_or(s.len());
            let so: i64 = s[..e].parse().unwrap();
            assert!((-480..=59).contains(&so), "사례 {case}: 시작 보정 {so}");
        }
        assert_eq!(export(&Header::default(), &[track("T", ch, back)]), text);
        let units = ns.len() + usize::try_from(length / 3840).unwrap();
        assert!(
            text.len() <= 200 * units,
            "사례 {case}: {}바이트, 단위 {units}",
            text.len()
        );
        if text.contains("64~+") {
            folded += 1;
        }
    }
    // 예산이 실제로 바닥나 겹쳐 적기 경로를 지나는지(지금은 60사례 모두).
    assert!(folded >= 50, "겹쳐 적은 사례 {folded}");
}
