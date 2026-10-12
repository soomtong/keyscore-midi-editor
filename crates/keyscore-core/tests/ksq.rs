//! `.ksq` 왕복과 파일 쓰기의 통합 테스트(S0-T6 게이트). 공개 API만 쓴다.

use std::fs;
use std::path::PathBuf;

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{
    Bank, Channel, Clip, Key, Note, OutputPort, Pitch, PitchClass, PitchOffset, Program, Project,
    Tempo, Tick, TimeSignature, Track, TrackColor, Velocity,
};
use proptest::prelude::*;

/// id를 뺀 내용. 왕복 비교는 이것으로 한다(`TrackId`·`ClipId`는 읽을 때 새로 발급된다).
#[derive(Debug, PartialEq)]
struct Content {
    key: Key,
    tempo: Vec<(Tick, Tempo)>,
    meter: Vec<(Tick, TimeSignature)>,
    tracks: Vec<TrackContent>,
}

/// (시작, 길이, 이름, 색, 노트).
type ClipContent = (Tick, u64, Option<String>, Option<TrackColor>, Vec<Note>);

#[derive(Debug, PartialEq)]
struct TrackContent {
    name: String,
    color: TrackColor,
    port: OutputPort,
    channel: Channel,
    bank: Option<Bank>,
    program: Option<Program>,
    default_velocity: Velocity,
    mute: bool,
    solo: bool,
    lyric_lang: Option<String>,
    clips: Vec<ClipContent>,
}

fn content(p: &Project) -> Content {
    Content {
        key: p.key(),
        tempo: p.tempo_map().changes().collect(),
        meter: p.meter_map().changes().collect(),
        tracks: p
            .tracks()
            .iter()
            .map(|t| TrackContent {
                name: t.name().to_owned(),
                color: t.color(),
                port: t.port().clone(),
                channel: t.channel(),
                bank: t.bank(),
                program: t.program(),
                default_velocity: t.default_velocity(),
                mute: t.mute(),
                solo: t.solo(),
                lyric_lang: t.lyric_lang().map(str::to_owned),
                clips: t
                    .clips()
                    .map(|(_, c)| {
                        (
                            c.start(),
                            c.length(),
                            c.name().map(str::to_owned),
                            c.color(),
                            c.notes().to_vec(),
                        )
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[derive(Debug, Clone)]
struct NoteSpec {
    start: u64,
    length: u32,
    pitch: u8,
    velocity: u16,
    offset_semis_x8: i8,
    lyric: Option<String>,
}

#[derive(Debug, Clone)]
struct ClipSpec {
    gap: u64,
    length: u64,
    name: Option<String>,
    notes: Vec<NoteSpec>,
    split_at: Option<u64>,
}

#[derive(Debug, Clone)]
struct TrackSpec {
    name: String,
    channel: u8,
    color: u8,
    program: Option<u8>,
    bank: Option<u16>,
    mute: bool,
    named_port: Option<String>,
    lyric_lang: Option<String>,
    clips: Vec<ClipSpec>,
}

fn note_spec() -> impl Strategy<Value = NoteSpec> {
    (
        0..4_000u64,
        1..3_000u32,
        0..=127u8,
        any::<u16>(),
        -96..=96i8,
        prop::option::of("[가-힣a-z \"\\\\]{1,3}"),
    )
        .prop_map(
            |(start, length, pitch, velocity, offset_semis_x8, lyric)| NoteSpec {
                start,
                length,
                pitch,
                velocity,
                offset_semis_x8,
                lyric,
            },
        )
}

fn clip_spec() -> impl Strategy<Value = ClipSpec> {
    (
        0..5_000u64,
        1..6_000u64,
        prop::option::of("[가-힣A-Za-z0-9 ]{0,6}"),
        prop::collection::vec(note_spec(), 0..12),
        prop::option::of(1..6_000u64),
    )
        .prop_map(|(gap, length, name, notes, split_at)| ClipSpec {
            gap,
            length,
            name,
            notes,
            split_at,
        })
}

fn track_spec() -> impl Strategy<Value = TrackSpec> {
    (
        "[A-Za-z가-힣 ]{0,8}",
        1..=16u8,
        0..12u8,
        prop::option::of(0..=127u8),
        prop::option::of(0..=16_383u16),
        any::<bool>(),
        prop::option::of("[A-Za-z0-9 ]{1,8}"),
        prop::option::of("ko|en|ja"),
        prop::collection::vec(clip_spec(), 0..4),
    )
        .prop_map(
            |(name, channel, color, program, bank, mute, named_port, lyric_lang, clips)| {
                TrackSpec {
                    name,
                    channel,
                    color,
                    program,
                    bank,
                    mute,
                    named_port,
                    lyric_lang,
                    clips,
                }
            },
        )
}

/// 명세로 프로젝트를 만든다. 모델이 거부하는 값(클립 밖 노트, 범위 밖 절대 피치)은 건너뛴다.
fn build(
    tracks: &[TrackSpec],
    tempos: &[(u64, u32)],
    meters: &[(u64, u8, u8)],
    key: Option<(u8, bool)>,
) -> Project {
    let mut p = Project::new();
    if let Some((tonic, major)) = key {
        let pc = PitchClass::new(tonic % 12).unwrap();
        p.set_key(if major {
            Key::Major(pc)
        } else {
            Key::Minor(pc)
        });
    }
    for &(tick, bpm) in tempos {
        p.tempo_map_mut()
            .set(Tick::new(tick), Tempo::from_bpm(bpm).unwrap());
    }
    for &(tick, num, den_pow) in meters {
        p.meter_map_mut().set(
            Tick::new(tick),
            TimeSignature::new(num, 1 << den_pow).unwrap(),
        );
    }
    for spec in tracks {
        let mut t = Track::new(
            spec.name.clone(),
            Channel::new(spec.channel).unwrap(),
            TrackColor::new(spec.color).unwrap(),
        );
        t.set_program(spec.program.map(|v| Program::new(v).unwrap()));
        t.set_bank(spec.bank.map(|v| Bank::new(v).unwrap()));
        t.set_mute(spec.mute);
        if let Some(port) = &spec.named_port {
            t.set_port(OutputPort::Named(port.clone()));
        }
        t.set_lyric_lang(spec.lyric_lang.clone());
        let mut at = 0u64;
        for cs in &spec.clips {
            at += cs.gap;
            let mut clip = Clip::new(Tick::new(at), cs.length).unwrap();
            if let Some(name) = &cs.name {
                clip.set_name(name.clone());
            }
            for ns in &cs.notes {
                let Ok(mut n) = Note::new(
                    Tick::new(ns.start),
                    ns.length,
                    Pitch::new(ns.pitch).unwrap(),
                    Velocity::new(ns.velocity),
                ) else {
                    continue;
                };
                let offset =
                    PitchOffset::new(i32::from(ns.offset_semis_x8) * (PitchOffset::SEMITONE / 8))
                        .unwrap();
                n = match n.with_offset(offset) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                if let Some(l) = &ns.lyric {
                    n = n.with_lyric(l.clone());
                }
                let _ = clip.insert_note(n);
            }
            let end = at + cs.length;
            let id = t.insert_clip(clip).unwrap();
            // 분할로 숨김 꼬리를 만든다. 꼬리도 왕복되어야 한다.
            if let Some(split) = cs.split_at {
                let _ = t.split_clip(id, Tick::new(at + split));
            }
            at = end;
        }
        p.push_track(t).unwrap();
    }
    p
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn ksq_round_trip_random_projects(
        tracks in prop::collection::vec(track_spec(), 0..4),
        tempos in prop::collection::vec((0..20_000u64, 20..300u32), 0..4),
        meters in prop::collection::vec((0..20_000u64, 1..=16u8, 0..7u8), 0..4),
        key in prop::option::of((0..12u8, any::<bool>())),
        gzip in any::<bool>(),
    ) {
        let p = build(&tracks, &tempos, &meters, key);
        let compression = if gzip { Compression::Gzip } else { Compression::None };
        let bytes = ksq::to_bytes(&p, compression).unwrap();
        let (back, warnings) = ksq::from_bytes(&bytes).unwrap();
        prop_assert!(warnings.is_empty());
        prop_assert_eq!(content(&back), content(&p));
        prop_assert_eq!(ksq::to_string(&back), ksq::to_string(&p));
    }
}

/// 테스트마다 따로 쓰는 임시 디렉터리.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("keyscore-ksq-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = set_readonly(&self.0, false);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn set_readonly(path: &std::path::Path, readonly: bool) -> std::io::Result<()> {
    let mut perm = fs::metadata(path)?.permissions();
    perm.set_readonly(readonly);
    fs::set_permissions(path, perm)
}

fn sample() -> Project {
    build(
        &[TrackSpec {
            name: "Lead".into(),
            channel: 1,
            color: 0,
            program: Some(0),
            bank: None,
            mute: false,
            named_port: None,
            lyric_lang: None,
            clips: vec![ClipSpec {
                gap: 0,
                length: 3840,
                name: None,
                notes: vec![NoteSpec {
                    start: 0,
                    length: 480,
                    pitch: 60,
                    velocity: 1000,
                    offset_semis_x8: 0,
                    lyric: None,
                }],
                split_at: None,
            }],
        }],
        &[],
        &[],
        None,
    )
}

#[test]
fn ksq_write_file_and_read_back() {
    let dir = TempDir::new("rw");
    for (name, compression) in [("a.ksq", Compression::None), ("a.ksqz", Compression::Gzip)] {
        let path = dir.0.join(name);
        ksq::write_file(&path, &sample(), compression).unwrap();
        let (back, _) = ksq::read_file(&path).unwrap();
        assert_eq!(content(&back), content(&sample()));
    }
    // 임시 파일이 남지 않는다.
    let mut names: Vec<String> = fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, vec!["a.ksq", "a.ksqz"]);
}

#[test]
fn ksq_write_file_failure_keeps_original() {
    let dir = TempDir::new("fail");
    let path = dir.0.join("song.ksq");
    ksq::write_file(&path, &sample(), Compression::None).unwrap();
    let original = fs::read(&path).unwrap();

    // 디렉터리에 쓸 수 없으면 임시 파일을 만들지 못해 실패한다. 원본은 그대로다.
    set_readonly(&dir.0, true).unwrap();
    let result = ksq::write_file(&path, &Project::new(), Compression::None);
    set_readonly(&dir.0, false).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), original);

    // 대상이 디렉터리라 이름 바꾸기가 실패한다. 임시 파일은 지워진다.
    let target = dir.0.join("is_dir.ksq");
    fs::create_dir(&target).unwrap();
    assert!(ksq::write_file(&target, &sample(), Compression::None).is_err());
    let leftovers: Vec<String> = fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n != "song.ksq" && n != "is_dir.ksq")
        .collect();
    assert!(leftovers.is_empty(), "임시 파일이 남았다: {leftovers:?}");
    assert_eq!(fs::read(&path).unwrap(), original);
}
