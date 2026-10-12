//! 「내보내기 → 가져오기 = 원본」 속성 테스트(S0-T5 게이트, 계획 테스트 1·2)와 파서 퍼즈.
//! 공개 API만 쓴다.

mod common;

use common::{NoteView, notes};
use keyscore_core::{
    Bank, Channel, Clip, Key, Note, Pitch, PitchClass, PitchOffset, Program, Tempo, Tick,
    TimeSignature, Track, TrackColor, Velocity,
};
use keyscore_text::{ExportTrack, Header, Mode, export, parse};
use proptest::prelude::*;

/// 표기 가능한 길이(1·2·4·8·16·32·64분음표와 점·겹점).
fn notated() -> Vec<u32> {
    let mut v = Vec::new();
    for d in [1, 2, 4, 8, 16, 32, 64] {
        let base = 3840 / d;
        v.extend([base, base * 3 / 2, base * 7 / 4]);
    }
    v
}

fn length() -> impl Strategy<Value = u32> {
    let grid = prop::sample::select(notated());
    let triplet = prop::sample::select(notated()).prop_map(|l| l * 2 / 3);
    prop_oneof![
        3 => grid,
        2 => triplet,
        3 => 1u32..4000,
        1 => (1u32..4, prop::sample::select(notated())).prop_map(|(k, r)| k * 3840 + r),
        1 => 4000u32..20_000,
    ]
}

fn start() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => (0u64..64).prop_map(|k| k * 240),
        1 => (0u64..48).prop_map(|k| k * 320),
        2 => 0u64..16_000,
    ]
}

fn text_chars(max: usize) -> impl Strategy<Value = String> {
    let chars = prop::sample::select(vec![
        'a', 'Z', '0', ' ', '"', '\\', '#', '-', '|', '~', '&', '@', '가', '\n', '[', ')',
    ]);
    prop::collection::vec(chars, 0..max).prop_map(|v| v.into_iter().collect())
}

fn lyric() -> impl Strategy<Value = Option<String>> {
    prop_oneof![3 => Just(None), 1 => text_chars(5).prop_map(Some)]
}

#[derive(Debug, Clone)]
struct NoteSpec {
    start: u64,
    length: u32,
    pitch: u8,
    velocity: u8,
    offset: i32,
    lyric: Option<String>,
}

fn note_spec() -> impl Strategy<Value = NoteSpec> {
    (
        start(),
        length(),
        0u8..128,
        0u8..128,
        prop_oneof![3 => Just(0), 1 => -1000i32..1000],
        lyric(),
    )
        .prop_map(|(start, length, pitch, velocity, offset, lyric)| NoteSpec {
            start,
            length,
            pitch,
            velocity,
            offset,
            lyric,
        })
}

/// 같은 시작·길이의 화음을 더 자주 만든다.
fn notes_spec() -> impl Strategy<Value = Vec<NoteSpec>> {
    (
        prop::collection::vec(note_spec(), 0..24),
        prop::collection::vec((0usize..24, 0u8..128, lyric()), 0..6),
    )
        .prop_map(|(mut ns, extra)| {
            for (i, pitch, lyric) in extra {
                if let Some(base) = ns.get(i % ns.len().max(1)).cloned() {
                    ns.push(NoteSpec {
                        pitch,
                        lyric,
                        ..base
                    });
                }
            }
            ns
        })
}

/// 클립 길이를 정하는 방법.
#[derive(Debug, Clone)]
enum LengthMode {
    /// 마지막 노트 끝.
    NoteEnd,
    /// 노트 끝보다 긴, 쉼표로 닿는 길이(15틱 단위, 195 이상).
    Padded(u64),
    /// 더 긴 클립을 노트 안쪽에서 분할해 숨김 꼬리를 만든다.
    Split(usize, u32),
    /// 아무 길이(재내보내기 안정성용).
    Any(u64),
}

fn build_clip(ns: &[NoteSpec], mode: &LengthMode) -> Clip {
    let max_end = ns
        .iter()
        .map(|n| n.start + u64::from(n.length))
        .max()
        .unwrap_or(0);
    let length = match mode {
        LengthMode::NoteEnd => max_end.max(1),
        LengthMode::Padded(k) => max_end.max(195).div_ceil(15) * 15 + k * 15,
        LengthMode::Split(..) => max_end.max(1),
        LengthMode::Any(k) => max_end.max(1) + k,
    };
    let mut c = Clip::new(Tick::ZERO, length).unwrap();
    for s in ns {
        let note = Note::new(
            Tick::new(s.start),
            s.length,
            Pitch::new(s.pitch).unwrap(),
            Velocity::from_midi1(s.velocity).unwrap(),
        )
        .unwrap();
        let note = note
            .clone()
            .with_offset(PitchOffset::new(s.offset).unwrap())
            .unwrap_or(note);
        let note = match &s.lyric {
            Some(l) => note.with_lyric(l.clone()),
            None => note,
        };
        c.insert_note(note).unwrap();
    }
    if let LengthMode::Split(i, frac) = mode {
        // 노트 하나의 안쪽에서 나눠 그 노트가 숨김 꼬리가 되게 한다.
        if let Some(s) = ns.get(i % ns.len().max(1)).filter(|s| s.length >= 2) {
            let at = s.start + 1 + u64::from(*frac % (s.length - 1));
            let mut t = Track::new("x", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
            let id = t.insert_clip(c.clone()).unwrap();
            t.split_clip(id, Tick::new(at)).unwrap();
            return t.clip(id).unwrap().clone();
        }
    }
    c
}

fn length_mode() -> impl Strategy<Value = LengthMode> {
    prop_oneof![
        2 => Just(LengthMode::NoteEnd),
        2 => (0u64..600).prop_map(LengthMode::Padded),
        2 => (0usize..24, any::<u32>()).prop_map(|(i, f)| LengthMode::Split(i, f)),
    ]
}

#[derive(Debug, Clone)]
struct TrackSpec {
    name: String,
    channel: u8,
    program: Option<u8>,
    bank: Option<u16>,
    notes: Vec<NoteSpec>,
    mode: LengthMode,
}

fn track_spec(mode: impl Strategy<Value = LengthMode>) -> impl Strategy<Value = TrackSpec> {
    (
        text_chars(6),
        prop_oneof![3 => 1u8..17, 1 => Just(10u8)],
        prop::option::of(0u8..128),
        prop::option::of(0u16..16_384),
        notes_spec(),
        mode,
    )
        .prop_map(|(name, channel, program, bank, notes, mode)| TrackSpec {
            name,
            channel,
            program,
            bank,
            notes,
            mode,
        })
}

fn header() -> impl Strategy<Value = Header> {
    let time = (1u8..=255, 0u32..9).prop_map(|(n, e)| TimeSignature::new(n, 1 << e).unwrap());
    let key = prop_oneof![
        Just(Key::None),
        (0u8..12).prop_map(|p| Key::Major(PitchClass::new(p).unwrap())),
        (0u8..12).prop_map(|p| Key::Minor(PitchClass::new(p).unwrap())),
    ];
    (
        prop::option::of(
            (1..=Tempo::MAX_MICROS).prop_map(|us| Tempo::from_micros_per_quarter(us).unwrap()),
        ),
        prop::option::of(time),
        prop::option::of(key),
    )
        .prop_map(|(t, s, k)| Header::new(t, s, k))
}

struct Built {
    name: String,
    channel: Channel,
    program: Option<Program>,
    bank: Option<Bank>,
    clip: Clip,
}

/// 트랙 이름이 겹치면 가져올 때 한 클립의 성부로 합쳐지므로 번호를 붙여 서로 다르게 한다.
fn build(specs: &[TrackSpec]) -> Vec<Built> {
    specs
        .iter()
        .enumerate()
        .map(|(i, s)| Built {
            name: format!("{}{i}", s.name),
            channel: Channel::new(s.channel).unwrap(),
            program: s.program.map(|p| Program::new(p).unwrap()),
            bank: s.bank.map(|b| Bank::new(b).unwrap()),
            clip: build_clip(&s.notes, &s.mode),
        })
        .collect()
}

fn export_tracks(b: &[Built]) -> Vec<ExportTrack<'_>> {
    b.iter()
        .map(|t| ExportTrack {
            name: &t.name,
            channel: t.channel,
            program: t.program,
            bank: t.bank,
            clip: &t.clip,
        })
        .collect()
}

/// 왕복 비교 기준: 숨김 꼬리를 클립 끝에서 자른 (시작, 길이, 음높이, 가사).
fn expected_notes(c: &Clip) -> Vec<NoteView> {
    let mut v: Vec<NoteView> = notes(c)
        .into_iter()
        .map(|(s, l, p, y)| {
            let end = (s + u64::from(l)).min(c.length());
            (s, u32::try_from(end - s).unwrap(), p, y)
        })
        .collect();
    v.sort();
    v
}

fn sorted(mut v: Vec<NoteView>) -> Vec<NoteView> {
    v.sort();
    v
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn export_parse_round_trip_random_clips(
        h in header(),
        specs in prop::collection::vec(track_spec(length_mode()), 1..4),
    ) {
        let built = build(&specs);
        let text = export(&h, &export_tracks(&built));
        let p = parse(&text, Mode::Lenient);
        prop_assert!(p.is_ok(), "{:?}\n{}", p.err(), text);
        let p = p.unwrap();
        prop_assert!(p.warnings().is_empty(), "{:?}\n{}", p.warnings(), text);
        prop_assert_eq!(p.header(), &h);
        prop_assert_eq!(p.tracks().len(), built.len());
        for (got, want) in p.tracks().iter().zip(&built) {
            prop_assert_eq!(got.name(), want.name.as_str());
            prop_assert_eq!(got.channel(), want.channel);
            prop_assert_eq!(got.program(), want.program);
            prop_assert_eq!(got.bank(), want.bank);
            prop_assert_eq!(sorted(notes(got.clip())), expected_notes(&want.clip), "{}", text);
            prop_assert_eq!(got.clip().length(), want.clip.length(), "{}", text);
        }
    }

    #[test]
    fn export_is_deterministic_and_reparse_stable(
        h in header(),
        specs in prop::collection::vec(
            track_spec(prop_oneof![
                length_mode(),
                (0u64..5000).prop_map(LengthMode::Any),
            ]),
            1..4,
        ),
    ) {
        let built = build(&specs);
        let tracks = export_tracks(&built);
        let first = export(&h, &tracks);
        prop_assert_eq!(&export(&h, &tracks), &first);
        let p = parse(&first, Mode::Lenient);
        prop_assert!(p.is_ok(), "{:?}\n{}", p.err(), first);
        let p = p.unwrap();
        let again: Vec<ExportTrack<'_>> = p
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
        prop_assert_eq!(export(p.header(), &again), first);
    }

    /// 파서는 어떤 문자열에도 패닉하지 않는다(계획 「불변식」).
    #[test]
    fn parse_never_panics_on_arbitrary_text(
        prefix in prop_oneof![
            Just(String::new()),
            Just("@track T ch1\n".to_owned()),
            Just("@track D ch10\n".to_owned()),
        ],
        body in prop::collection::vec(
            prop::sample::select(vec![
                "a", "c", "r", "o", "l", "v", "n", "b", "d", "h", "s", "t", "q", "1", "2", "4", "9",
                "0", " ", "\n", "#", "+", "-", ".", "~", ":", "&", "\"", "\\", "[", "]", "(", ")",
                "|", "<", ">", "@", "@track", "@tempo", "ch", "prog", "가", "99999999999999999999",
            ]),
            0..80,
        ),
    ) {
        let text = format!("{prefix}{}", body.concat());
        for mode in [Mode::Lenient, Mode::Strict] {
            if let Ok(p) = parse(&text, mode) {
                for t in p.tracks() {
                    prop_assert!(t.clip().length() >= 1);
                }
            }
        }
    }
}
