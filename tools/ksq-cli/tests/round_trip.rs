//! `ksq2txt → txt2ksq` 왕복 속성 테스트(S0-T8, 계획 테스트 9).
//!
//! 두 가지를 본다.
//! - 텍스트가 나르는 정보만 쓴 프로젝트는 클립 길이 반올림 말고는 손실 없이 돌아온다.
//! - 어떤 프로젝트든 손실 목록이 비어 있으면 되읽은 `.ksq`가 바이트까지 같다(손실 보고가 빠짐없다).

use keyscore_core::{
    Bank, Channel, Clip, Key, MeterMap, Note, Pitch, PitchClass, PitchOffset, Program, Project,
    Tempo, TempoMap, Tick, TimeSignature, Track, TrackColor, Velocity, ksq,
};
use keyscore_text::Mode;
use ksq_cli::{Loss, project_to_text, text_to_project};
use proptest::prelude::*;

#[derive(Debug, Clone)]
struct NoteSpec {
    start: u64,
    length: u32,
    pitch: u8,
    lyric: Option<String>,
    velocity: u8,
    offset: i32,
}

#[derive(Debug, Clone)]
struct ClipSpec {
    start: u64,
    /// 마지막 노트 끝 뒤에 더하는 길이.
    tail: u64,
    notes: Vec<NoteSpec>,
}

#[derive(Debug, Clone)]
struct TrackSpec {
    name: String,
    channel: u8,
    program: Option<u8>,
    bank: Option<u16>,
    clips: Vec<ClipSpec>,
    mute: bool,
}

#[derive(Debug, Clone)]
struct ProjectSpec {
    tempo: u32,
    meter: (u8, u16),
    key: Key,
    tempo_change: Option<(u64, u32)>,
    tracks: Vec<TrackSpec>,
}

fn grid_length() -> impl Strategy<Value = u32> {
    let mut grid = Vec::new();
    for d in [1u32, 2, 4, 8, 16, 32, 64] {
        let base = 3840 / d;
        grid.extend([base, base * 3 / 2, base * 7 / 4, base * 2 / 3]);
    }
    prop_oneof![3 => prop::sample::select(grid), 1 => 1u32..5000]
}

fn text_chars(max: usize) -> impl Strategy<Value = String> {
    let chars = prop::sample::select(vec![
        'a', 'Z', '0', ' ', '"', '\\', '#', '-', '|', '~', '&', '@', '가', '\n', '[', '(',
    ]);
    prop::collection::vec(chars, 0..max).prop_map(|v| v.into_iter().collect())
}

/// 텍스트가 나르는 정보만: 벨로시티 100, 오프셋 0.
fn plain_note() -> impl Strategy<Value = NoteSpec> {
    (
        prop_oneof![2 => (0u64..32).prop_map(|k| k * 240), 1 => 0u64..8000],
        grid_length(),
        0u8..128,
        prop_oneof![3 => Just(None), 1 => text_chars(4).prop_map(Some)],
    )
        .prop_map(|(start, length, pitch, lyric)| NoteSpec {
            start,
            length,
            pitch,
            lyric,
            velocity: 100,
            offset: 0,
        })
}

fn any_note() -> impl Strategy<Value = NoteSpec> {
    (
        plain_note(),
        prop_oneof![3 => Just(100u8), 1 => 0u8..128],
        prop_oneof![3 => Just(0i32), 1 => -1000i32..1000],
    )
        .prop_map(|(mut n, velocity, offset)| {
            n.velocity = velocity;
            n.offset = offset;
            n
        })
}

fn tail() -> impl Strategy<Value = u64> {
    prop_oneof![3 => Just(0u64), 1 => prop::sample::select(vec![60u64, 240, 960]), 1 => 1u64..1000]
}

fn header() -> impl Strategy<Value = (u32, (u8, u16), Key)> {
    let key = prop_oneof![
        Just(Key::None),
        (0u8..12).prop_map(|p| Key::Major(PitchClass::new(p).unwrap())),
        (0u8..12).prop_map(|p| Key::Minor(PitchClass::new(p).unwrap())),
    ];
    (
        prop_oneof![3 => 250_000u32..1_500_000, 1 => 1u32..=Tempo::MAX_MICROS],
        (1u8..=16, prop::sample::select(vec![1u16, 2, 4, 8, 16, 32])),
        key,
    )
}

fn unique_names(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, n) in names.into_iter().enumerate() {
        out.push(if out.contains(&n) {
            format!("{n}{i}")
        } else {
            n
        });
    }
    out
}

/// 텍스트가 나르는 정보만 쓴 프로젝트: 트랙마다 시작 0 클립 하나, 이름은 서로 다르다.
fn text_only_project() -> impl Strategy<Value = ProjectSpec> {
    let track = (
        text_chars(6),
        1u8..=16,
        prop::option::of(0u8..128),
        prop::option::of(0u16..=Bank::MAX),
        prop::collection::vec(plain_note(), 0..12),
        tail(),
    );
    (header(), prop::collection::vec(track, 0..4)).prop_map(|((tempo, meter, key), tracks)| {
        let names = unique_names(tracks.iter().map(|t| t.0.clone()).collect());
        ProjectSpec {
            tempo,
            meter,
            key,
            tempo_change: None,
            tracks: tracks
                .into_iter()
                .zip(names)
                .map(
                    |((_, channel, program, bank, notes, tail), name)| TrackSpec {
                        name,
                        channel,
                        program,
                        bank,
                        clips: vec![ClipSpec {
                            start: 0,
                            tail: tail.max(u64::from(notes.is_empty())),
                            notes,
                        }],
                        mute: false,
                    },
                )
                .collect(),
        }
    })
}

/// 텍스트가 나르지 않는 정보(여러 클립, 템포 변화, 벨로시티, 오프셋, 뮤트, 같은 이름)도 섞는다.
fn any_project() -> impl Strategy<Value = ProjectSpec> {
    let clip = (
        prop_oneof![1 => Just(0u64), 1 => 0u64..20_000],
        prop::collection::vec(any_note(), 0..6),
        tail(),
    );
    let track = (
        prop::sample::select(vec!["A".to_owned(), "A (2)".to_owned(), "B".to_owned()]),
        1u8..=16,
        prop::option::of(0u8..128),
        prop::option::of(0u16..=Bank::MAX),
        prop::collection::vec(clip, 0..3),
        prop::bool::weighted(0.1),
    );
    (
        header(),
        prop::option::of((1u64..20_000, 250_000u32..1_500_000)),
        prop::collection::vec(track, 0..4),
    )
        .prop_map(|((tempo, meter, key), tempo_change, tracks)| ProjectSpec {
            tempo,
            meter,
            key,
            tempo_change,
            tracks: tracks
                .into_iter()
                .map(|(name, channel, program, bank, clips, mute)| TrackSpec {
                    name,
                    channel,
                    program,
                    bank,
                    clips: clips
                        .into_iter()
                        .map(|(start, notes, tail)| ClipSpec {
                            start,
                            tail: tail.max(u64::from(notes.is_empty())),
                            notes,
                        })
                        .collect(),
                    mute,
                })
                .collect(),
        })
}

fn build_clip(spec: &ClipSpec) -> Clip {
    let end = spec
        .notes
        .iter()
        .map(|n| n.start + u64::from(n.length))
        .max()
        .unwrap_or(0);
    let mut clip = Clip::new(Tick::new(spec.start), end + spec.tail).unwrap();
    for n in &spec.notes {
        let mut note = Note::new(
            Tick::new(n.start),
            n.length,
            Pitch::new(n.pitch).unwrap(),
            Velocity::from_midi1(n.velocity).unwrap(),
        )
        .unwrap();
        if n.offset != 0 {
            match note
                .clone()
                .with_offset(PitchOffset::new(n.offset).unwrap())
            {
                Ok(with) => note = with,
                Err(_) => continue,
            }
        }
        if let Some(l) = &n.lyric {
            note = note.with_lyric(l.clone());
        }
        clip.insert_note(note).unwrap();
    }
    clip
}

fn build(spec: &ProjectSpec) -> Project {
    let mut p = Project::new();
    *p.tempo_map_mut() = TempoMap::new(Tempo::from_micros_per_quarter(spec.tempo).unwrap());
    if let Some((at, us)) = spec.tempo_change {
        p.tempo_map_mut()
            .set(Tick::new(at), Tempo::from_micros_per_quarter(us).unwrap());
    }
    *p.meter_map_mut() = MeterMap::new(TimeSignature::new(spec.meter.0, spec.meter.1).unwrap());
    p.set_key(spec.key);
    for (i, t) in spec.tracks.iter().enumerate() {
        let color = TrackColor::new(u8::try_from(i % 12).unwrap()).unwrap();
        let mut track = Track::new(t.name.clone(), Channel::new(t.channel).unwrap(), color);
        track.set_program(t.program.map(|v| Program::new(v).unwrap()));
        track.set_bank(t.bank.map(|v| Bank::new(v).unwrap()));
        track.set_mute(t.mute);
        // 겹치는 클립은 건너뛴다(생성기가 겹침을 피하지 않는다).
        for c in &t.clips {
            let _ = track.insert_clip(build_clip(c));
        }
        p.push_track(track).unwrap();
    }
    p
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn txt2ksq_then_ksq2txt_round_trips_random_projects(spec in text_only_project()) {
        let original = build(&spec);

        let (text, losses) = project_to_text(&original).unwrap();
        let (back, _) = text_to_project(&text, Mode::Lenient).unwrap();

        // 클립 길이 반올림만 허용하고, 그 길이로 바꾼 원본과 바이트까지 같아야 한다.
        let mut expected_spec = spec.clone();
        for loss in &losses {
            match loss {
                Loss::ClipLength { track, from, to, .. } => {
                    let c = &mut expected_spec.tracks[*track].clips[0];
                    prop_assert_eq!(build_clip(c).length(), *from);
                    c.tail = c.tail + *to - *from;
                }
                other => prop_assert!(false, "텍스트만 쓴 프로젝트에서 손실: {other:?}\n{text}"),
            }
        }
        prop_assert_eq!(ksq::to_string(&back), ksq::to_string(&build(&expected_spec)), "{}", text);
    }

    #[test]
    fn ksq2txt_without_losses_round_trips_any_project_bytes(spec in any_project()) {
        let original = build(&spec);

        let (text, losses) = project_to_text(&original).unwrap();
        let (back, _) = text_to_project(&text, Mode::Lenient).unwrap();

        prop_assert!(
            !losses.iter().any(|l| matches!(l, Loss::Mismatch { .. })),
            "{losses:?}\n{text}"
        );
        if losses.is_empty() {
            prop_assert_eq!(ksq::to_string(&back), ksq::to_string(&original), "{}", text);
        }
        // 손실이 있어도 트랙 수·순서·이름 수는 그대로다.
        prop_assert_eq!(back.tracks().len(), original.tracks().len());
    }
}
