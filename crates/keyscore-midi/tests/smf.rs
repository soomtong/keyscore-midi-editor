//! S1-T6 SMF 입출력 테스트(REQ IO-2, SEQ-6, SEQ-9, CTL-10, LYR-6).

use std::collections::BTreeMap;
use std::path::Path;

use keyscore_core::{
    Bank, Channel, Clip, MeterMap, Note, Pitch, Program, Project, Tempo, TempoMap, Tick, Track,
    TrackColor, Velocity,
};
use keyscore_midi::{SmfError, SmfEventKind, SmfFile, SmfFormat, SmfLimits, Song};
use proptest::prelude::*;

// ---------- SMF 바이트 만들기 ----------

fn vlq(mut n: u32) -> Vec<u8> {
    let mut out = vec![u8::try_from(n & 0x7F).unwrap()];
    n >>= 7;
    while n > 0 {
        out.insert(0, u8::try_from(n & 0x7F).unwrap() | 0x80);
        n >>= 7;
    }
    out
}

/// (델타, 바이트) 열로 `MTrk` 청크를 만든다. End of Track을 붙인다.
fn track(events: &[(u32, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (delta, bytes) in events {
        body.extend(vlq(*delta));
        body.extend_from_slice(bytes);
    }
    body.extend([0x00, 0xFF, 0x2F, 0x00]);
    chunk(*b"MTrk", &body)
}

fn chunk(id: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend(u32::try_from(body.len()).unwrap().to_be_bytes());
    out.extend_from_slice(body);
    out
}

fn smf(format: u16, ppq: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend(format.to_be_bytes());
    header.extend(u16::try_from(tracks.len()).unwrap().to_be_bytes());
    header.extend(ppq.to_be_bytes());
    let mut out = chunk(*b"MThd", &header);
    for t in tracks {
        out.extend_from_slice(t);
    }
    out
}

fn song_of(bytes: &[u8]) -> Song {
    Song::from_smf(&SmfFile::read(bytes).unwrap()).unwrap()
}

fn ch(n: u8) -> Channel {
    Channel::new(n).unwrap()
}

fn pitch(n: u8) -> Pitch {
    Pitch::new(n).unwrap()
}

fn vel(n: u8) -> Velocity {
    Velocity::from_midi1(n).unwrap()
}

/// 채널 메시지를 (틱, 바이트) 목록으로 모은다(트랙 구분 없이).
fn channel_messages(file: &SmfFile) -> Vec<(u64, Vec<u8>)> {
    let mut out = Vec::new();
    for t in file.tracks() {
        for e in t.events() {
            if let SmfEventKind::Channel(m) = e.kind() {
                out.push((e.tick(), m.bytes().to_vec()));
            }
        }
    }
    out.sort();
    out
}

/// 채널별 이벤트 집합(Type 0으로 합치면 트랙 경계와 이름이 사라지므로 채널 단위로 비교한다). 채널은
/// 1–16이라 키 0은 가사 몫이다.
fn by_channel(song: &Song) -> BTreeMap<u8, Vec<String>> {
    let mut out: BTreeMap<u8, Vec<String>> = BTreeMap::new();
    for t in song.tracks() {
        let list = out.entry(t.channel().get()).or_default();
        // 노트는 켜기·끄기 이벤트로 적는다. 같은 채널·음높이의 노트가 겹치면 Type 0 한 줄에서 어느
        // Note Off가 어느 Note On의 짝인지 남지 않는다(MIDI 1.0의 한계). 이벤트 집합은 그대로다.
        for n in t.notes() {
            let end = n.start().get() + u64::from(n.length());
            list.push(format!(
                "on {:?} {:?} {:?}",
                n.start(),
                n.pitch(),
                n.velocity()
            ));
            list.push(format!("off {end} {:?}", n.pitch()));
        }
        for p in t.programs() {
            list.push(format!("prog {p:?}"));
        }
        for (cc, points) in t.cc() {
            for p in points {
                list.push(format!("cc {cc} {p:?}"));
            }
        }
        for p in t.bend() {
            list.push(format!("bend {p:?}"));
        }
        for p in t.pressure() {
            list.push(format!("pressure {p:?}"));
        }
    }
    // 가사는 같은 틱에 노트가 시작하는 채널 트랙에 붙는다. Type 0으로 합치면 다른 SMF 트랙의 채널에
    // 붙을 수 있어, 채널과 무관한 하나의 집합(키 0)으로 비교한다.
    let lyrics = out.entry(0).or_default();
    for t in song.tracks() {
        for l in t.lyrics() {
            lyrics.push(format!("lyric {l:?}"));
        }
    }
    for list in out.values_mut() {
        list.sort();
    }
    out
}

// ---------- 1. 공개 샘플 왕복 ----------

#[test]
fn public_samples_round_trip_event_sets() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/smf");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "mid"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 10, "공개 샘플 10개: {files:?}");
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        let first = song_of(&bytes);
        assert!(
            first.tracks().iter().any(|t| !t.notes().is_empty()),
            "{path:?}: 노트가 없다"
        );
        // Type 1로 다시 쓰면 곡이 그대로다.
        let again = song_of(&first.to_smf(SmfFormat::Parallel).unwrap().write());
        assert_eq!(again, first, "{path:?}: Type 1 왕복");
        // Type 0으로 합치면 채널별 이벤트 집합과 템포·박자표가 그대로다.
        let single = song_of(&first.to_smf(SmfFormat::Single).unwrap().write());
        assert_eq!(
            by_channel(&single),
            by_channel(&first),
            "{path:?}: Type 0 왕복"
        );
        assert_eq!(single.tempo(), first.tempo());
        assert_eq!(single.meter(), first.meter());
    }
}

// ---------- 2. 가사 ----------

#[test]
fn lyrics_round_trip_through_smf_and_project() {
    let name: &[u8] = &[0xFF, 0x03, 0x05, b'V', b'o', b'c', b'a', b'l'];
    let do_: &[u8] = &[0xFF, 0x05, 0x03, 0xEB, 0x8F, 0x84]; // "도"(UTF-8)
    let latin: &[u8] = &[0xFF, 0x05, 0x02, b'f', 0xE9]; // "fé"(Latin-1)
    let bytes = smf(
        1,
        480,
        &[track(&[
            (0, name),
            (0, do_),
            (0, &[0x90, 60, 100]),
            (480, &[0x80, 60, 0]),
            (0, latin),
            (0, &[0x90, 62, 90]),
            (480, &[0x80, 62, 0]),
        ])],
    );
    let song = song_of(&bytes);
    let t = &song.tracks()[0];
    assert_eq!(t.name(), Some("Vocal"));
    assert_eq!(
        t.lyrics(),
        &[
            (Tick::ZERO, "도".to_string()),
            (Tick::new(960), "fé".to_string())
        ]
    );
    // 모델로 옮기면 가사가 노트에 붙고, 되돌리면 같은 가사가 나온다.
    let project = song.to_project().unwrap();
    let lyrics: Vec<_> = project.tracks()[0]
        .clips()
        .flat_map(|(_, c)| {
            c.notes()
                .iter()
                .filter_map(|n| n.lyric().map(str::to_string))
        })
        .collect();
    assert_eq!(lyrics, ["도", "fé"]);
    let back = Song::from_project(&project);
    let again = song_of(&back.to_smf(SmfFormat::Parallel).unwrap().write());
    assert_eq!(again.tracks()[0].lyrics(), t.lyrics());
    assert_eq!(again.tracks()[0].notes(), t.notes());
}

// ---------- 3. 벨로시티·CC·벤드 ----------

#[test]
fn velocity_and_cc_survive_round_trip() {
    let mut events: Vec<(u32, Vec<u8>)> = Vec::new();
    for v in [1u8, 64, 100, 127] {
        events.push((0, vec![0x90, 60, v]));
        events.push((10, vec![0x80, 60, 0]));
    }
    for (cc, v) in [(0u8, 0u8), (1, 1), (64, 64), (7, 127), (121, 0)] {
        events.push((5, vec![0xB0, cc, v]));
    }
    for (lsb, msb) in [(0u8, 0u8), (0, 0x40), (0x7F, 0x7F), (1, 0x20)] {
        events.push((5, vec![0xE0, lsb, msb]));
    }
    events.push((5, vec![0xD0, 99]));
    events.push((5, vec![0xC0, 5]));
    let refs: Vec<(u32, &[u8])> = events.iter().map(|(d, b)| (*d, b.as_slice())).collect();
    let bytes = smf(1, 960, &[track(&refs)]);
    let original = SmfFile::read(&bytes).unwrap();
    let song = Song::from_smf(&original).unwrap();
    let written = SmfFile::read(&song.to_smf(SmfFormat::Parallel).unwrap().write()).unwrap();
    assert_eq!(channel_messages(&written), channel_messages(&original));
    // 곡 계층의 값은 32비트다: CC 127은 꼭대기, 벤드 가운데는 0x8000_0000이다.
    let t = &song.tracks()[0];
    assert_eq!(t.cc()[&7][0].value(), u32::MAX);
    assert_eq!(t.cc()[&0][0].value(), 0);
    assert_eq!(t.bend()[1].value(), 0x8000_0000);
    assert_eq!(t.notes()[3].velocity(), Velocity::new(0xFFFF));
}

// ---------- 4–7. 읽기 규칙 ----------

#[test]
fn note_on_velocity_zero_reads_as_note_off() {
    let bytes = smf(
        0,
        960,
        &[track(&[(0, &[0x90, 60, 100]), (240, &[0x90, 60, 0])])],
    );
    let song = song_of(&bytes);
    let notes = song.tracks()[0].notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].start(), Tick::ZERO);
    assert_eq!(notes[0].length(), 240);
}

#[test]
fn running_status_and_meta_between_reads() {
    // running status로 Note On 둘, 사이에 메타와 SysEx. 그 뒤의 데이터 바이트도 앞 상태를 잇는다
    // (명세는 메타·SysEx가 running status를 끊는다고 하지만, 그렇게 쓴 파일이 많아 너그럽게 읽는다).
    let bytes = smf(
        0,
        960,
        &[track(&[
            (0, &[0x90, 60, 100]),
            (0, &[62, 100]),
            (0, &[0xFF, 0x01, 0x01, b'x']),
            (0, &[0xF0, 0x02, 0x7E, 0xF7]),
            (100, &[60, 0]),
            (0, &[62, 0]),
        ])],
    );
    let file = SmfFile::read(&bytes).unwrap();
    let kinds: Vec<_> = file.tracks()[0]
        .events()
        .iter()
        .map(keyscore_midi::SmfEvent::kind)
        .collect();
    assert_eq!(kinds.len(), 6);
    assert!(matches!(kinds[2], SmfEventKind::Meta { kind: 0x01, .. }));
    assert!(matches!(
        kinds[3],
        SmfEventKind::SysEx { escape: false, .. }
    ));
    let song = Song::from_smf(&file).unwrap();
    assert_eq!(song.tracks()[0].notes().len(), 2);
    assert!(song.tracks()[0].notes().iter().all(|n| n.length() == 100));
    // 첫 이벤트가 데이터 바이트면 상태가 없다.
    let bad = smf(0, 960, &[track(&[(0, &[60, 100])])]);
    assert!(matches!(
        SmfFile::read(&bad),
        Err(SmfError::MissingStatus { .. })
    ));
}

#[test]
fn ppq_rescale_to_960_is_rounded_integer() {
    for (ppq, tick, want) in [
        (96u16, 48u32, 480u64),
        (480, 1, 2),
        (1000, 1, 1),
        (7, 3, 411),
        (960, 12345, 12345),
    ] {
        let bytes = smf(
            0,
            ppq,
            &[track(&[(tick, &[0x90, 60, 100]), (tick, &[0x80, 60, 0])])],
        );
        let song = song_of(&bytes);
        assert_eq!(
            song.tracks()[0].notes()[0].start(),
            Tick::new(want),
            "ppq {ppq}"
        );
    }
}

#[test]
fn type0_splits_by_channel_and_type1_by_track() {
    let type0 = smf(
        0,
        960,
        &[track(&[
            (0, &[0x91, 60, 100]),
            (0, &[0x90, 64, 100]),
            (10, &[0x81, 60, 0]),
            (0, &[0x80, 64, 0]),
        ])],
    );
    let song = song_of(&type0);
    let channels: Vec<u8> = song.tracks().iter().map(|t| t.channel().get()).collect();
    assert_eq!(channels, [1, 2]);
    // Type 1: 템포 트랙은 곡 트랙이 아니고, 채널이 둘인 트랙은 둘로 나뉜다.
    let tempo: &[u8] = &[0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20];
    let type1 = smf(
        1,
        960,
        &[
            track(&[(0, tempo)]),
            track(&[
                (0, &[0xFF, 0x03, 0x01, b'A']),
                (0, &[0x92, 60, 1]),
                (5, &[0x82, 60, 0]),
            ]),
            track(&[
                (0, &[0x93, 60, 1]),
                (0, &[0x94, 61, 1]),
                (5, &[0x83, 60, 0]),
                (0, &[0x84, 61, 0]),
            ]),
        ],
    );
    let song = song_of(&type1);
    let shape: Vec<(Option<&str>, u8)> = song
        .tracks()
        .iter()
        .map(|t| (t.name(), t.channel().get()))
        .collect();
    assert_eq!(shape, [(Some("A"), 3), (None, 4), (None, 5)]);
    assert_eq!(
        song.tempo().tempo_at(Tick::ZERO),
        Tempo::from_micros_per_quarter(500_000).unwrap()
    );
}

// ---------- 8–9. 모델 변환 ----------

fn note_song(notes: &[(u64, u32)]) -> Song {
    let mut events: Vec<(u64, Vec<u8>)> = Vec::new();
    for &(start, len) in notes {
        events.push((start, vec![0x90, 60, 100]));
        events.push((start + u64::from(len), vec![0x80, 60, 0]));
    }
    events.sort_by_key(|e| (e.0, e.1[0] == 0x90));
    let mut last = 0;
    let deltas: Vec<(u32, Vec<u8>)> = events
        .into_iter()
        .map(|(t, b)| {
            let d = u32::try_from(t - last).unwrap();
            last = t;
            (d, b)
        })
        .collect();
    let refs: Vec<(u32, &[u8])> = deltas.iter().map(|(d, b)| (*d, b.as_slice())).collect();
    song_of(&smf(0, 960, &[track(&refs)]))
}

#[test]
fn to_project_groups_notes_into_bar_clips() {
    // 4/4, 한 마디 3840틱. 마디 0, 1(마디 2로 넘어감), 4에 노트.
    let song = note_song(&[(0, 960), (3840 + 3000, 1000), (4 * 3840, 960)]);
    let project = song.to_project().unwrap();
    let t = &project.tracks()[0];
    assert_eq!(t.channel(), ch(1));
    let clips: Vec<(u64, u64, Vec<u64>)> = t
        .clips()
        .map(|(_, c)| {
            (
                c.start().get(),
                c.length(),
                c.notes().iter().map(|n| n.start().get()).collect(),
            )
        })
        .collect();
    assert_eq!(
        clips,
        [(0, 3 * 3840, vec![0, 6840]), (4 * 3840, 3840, vec![0])]
    );
}

#[test]
fn from_project_flattens_clips_and_cuts_hidden_tails() {
    let mut project = Project::new();
    let mut track = Track::new("Lead", ch(3), TrackColor::new(0).unwrap());
    track.set_program(Some(Program::new(5).unwrap()));
    track.set_bank(Some(Bank::new(130).unwrap()));
    let mut clip = Clip::new(Tick::new(3840), 3840).unwrap();
    clip.insert_note(Note::new(Tick::new(3000), 800, pitch(60), vel(90)).unwrap())
        .unwrap();
    clip.insert_note(Note::new(Tick::ZERO, 960, pitch(64), vel(100)).unwrap())
        .unwrap();
    let id = track.insert_clip(clip).unwrap();
    // 3840 + 3200에서 나누면 틱 3000의 노트(끝 3800)는 왼쪽 클립의 숨김 꼬리가 된다.
    track.split_clip(id, Tick::new(3840 + 3200)).unwrap();
    project.push_track(track).unwrap();
    let song = Song::from_project(&project);
    let t = &song.tracks()[0];
    assert_eq!(t.name(), Some("Lead"));
    assert_eq!(t.channel(), ch(3));
    let notes: Vec<(u64, u32, u8)> = t
        .notes()
        .iter()
        .map(|n| (n.start().get(), n.length(), n.pitch().get()))
        .collect();
    assert_eq!(notes, [(3840, 960, 64), (3840 + 3000, 200, 60)]);
    // 뱅크(MSB 1, LSB 2)와 프로그램이 틱 0에 이 순서로 나간다.
    let file = song.to_smf(SmfFormat::Parallel).unwrap();
    let first: Vec<Vec<u8>> = channel_messages(&file)
        .into_iter()
        .filter(|(t, _)| *t == 0)
        .map(|(_, b)| b)
        .collect();
    let track_order: Vec<Vec<u8>> = file
        .tracks()
        .iter()
        .flat_map(keyscore_midi::SmfTrack::events)
        .filter(|e| e.tick() == 0)
        .filter_map(|e| match e.kind() {
            SmfEventKind::Channel(m) => Some(m.bytes().to_vec()),
            _ => None,
        })
        .collect();
    assert_eq!(first.len(), 3);
    assert_eq!(
        track_order,
        [vec![0xB2, 0, 1], vec![0xB2, 32, 2], vec![0xC2, 5]]
    );
}

// ---------- 10–12. 악의적 입력, 형식, 결정적 쓰기 ----------

#[test]
fn malicious_smf_returns_err_without_panic() {
    // 거짓 청크 길이.
    let mut lying = smf(0, 960, &[]);
    lying.extend(b"MTrk");
    lying.extend(u32::MAX.to_be_bytes());
    lying.extend([0x00, 0x90, 60, 100]);
    let header_ntrks_one = {
        let mut b = lying.clone();
        b[11] = 1;
        b
    };
    assert!(matches!(
        SmfFile::read(&header_ntrks_one),
        Err(SmfError::Truncated { .. })
    ));
    // 트랙 수 과다(헤더 65535).
    let mut many = smf(1, 960, &[track(&[(0, &[0x90, 60, 1])])]);
    many[10] = 0xFF;
    many[11] = 0xFF;
    assert!(matches!(
        SmfFile::read(&many),
        Err(SmfError::TooManyTracks { .. })
    ));
    // 5바이트 가변 길이 수.
    let mut long_vlq = smf(0, 960, &[]);
    long_vlq[11] = 1;
    long_vlq.extend(chunk(
        *b"MTrk",
        &[0x81, 0x81, 0x81, 0x81, 0x01, 0x90, 60, 1],
    ));
    assert!(matches!(
        SmfFile::read(&long_vlq),
        Err(SmfError::BadVarLen { .. })
    ));
    // 이벤트 수 상한.
    let events: Vec<(u32, &[u8])> = (0..20).map(|_| (0, &[0x90u8, 60, 1][..])).collect();
    let bytes = smf(0, 960, &[track(&events)]);
    assert!(matches!(
        SmfFile::read_with(&bytes, SmfLimits::new(16, 10)),
        Err(SmfError::TooManyEvents { max: 10 })
    ));
    // 빈 입력, 헤더만, 잘린 헤더.
    assert!(matches!(SmfFile::read(&[]), Err(SmfError::NotSmf)));
    assert!(SmfFile::read(b"MThd").is_err());
    // 메타 길이가 트랙 끝을 넘는다.
    let mut meta = smf(0, 960, &[]);
    meta[11] = 1;
    meta.extend(chunk(*b"MTrk", &[0x00, 0xFF, 0x05, 0x7F, b'a']));
    assert!(matches!(
        SmfFile::read(&meta),
        Err(SmfError::Truncated { .. })
    ));
}

proptest! {
    #[test]
    fn malicious_smf_random_bytes_never_panic(
        tail in proptest::collection::vec(any::<u8>(), 0..512),
        format in 0u16..3,
        ppq in any::<u16>(),
    ) {
        // 아무 바이트, 그리고 올바른 헤더 뒤의 아무 트랙 바이트.
        let _ = SmfFile::read(&tail);
        let mut bytes = smf(format, ppq, &[]);
        bytes[11] = 1;
        bytes.extend(chunk(*b"MTrk", &tail));
        if let Ok(file) = SmfFile::read(&bytes) {
            let _ = Song::from_smf(&file).map(|s| s.to_project());
        }
    }
}

#[test]
fn smpte_division_and_type2_are_rejected() {
    let t = track(&[(0, &[0x90, 60, 1])]);
    let mut smpte = smf(0, 960, std::slice::from_ref(&t));
    smpte[12] = 0xE7;
    smpte[13] = 0x28;
    assert_eq!(SmfFile::read(&smpte), Err(SmfError::SmpteDivision));
    assert_eq!(
        SmfFile::read(&smf(2, 960, std::slice::from_ref(&t))),
        Err(SmfError::UnsupportedFormat(2))
    );
    assert_eq!(
        SmfFile::read(&smf(0, 0, std::slice::from_ref(&t))),
        Err(SmfError::ZeroDivision)
    );
    assert_eq!(
        SmfFile::read(&smf(0, 960, &[t.clone(), t])),
        Err(SmfError::SingleTrackCount(2))
    );
}

#[test]
fn written_smf_is_byte_stable() {
    let song = note_song(&[(0, 960), (100, 50), (5000, 1)]);
    let a = song.to_smf(SmfFormat::Parallel).unwrap().write();
    let b = song.clone().to_smf(SmfFormat::Parallel).unwrap().write();
    assert_eq!(a, b);
    assert_eq!(&a[..4], b"MThd");
    // 960 PPQ로 쓴다.
    assert_eq!(SmfFile::read(&a).unwrap().ppq(), 960);
    // 템포·박자표 기본값도 쓴다(첫 트랙).
    let file = SmfFile::read(&a).unwrap();
    assert_eq!(file.format(), SmfFormat::Parallel);
    let song2 = Song::from_smf(&file).unwrap();
    assert_eq!(song2.tempo(), &TempoMap::new(Tempo::DEFAULT));
    assert_eq!(song2.meter(), &MeterMap::default());
}

// ---------- 13. 곡 왕복 속성 ----------

fn arb_events() -> impl Strategy<Value = Vec<(u32, Vec<u8>)>> {
    let msg = prop_oneof![
        (0u8..4, 0u8..128, 0u8..128).prop_map(|(c, p, v)| vec![0x90 | c, p, v]),
        (0u8..4, 0u8..128).prop_map(|(c, p)| vec![0x80 | c, p, 0]),
        (0u8..4, 0u8..128, 0u8..128).prop_map(|(c, n, v)| vec![0xB0 | c, n, v]),
        (0u8..4, 0u8..128, 0u8..128).prop_map(|(c, l, m)| vec![0xE0 | c, l, m]),
        (0u8..4, 0u8..128).prop_map(|(c, v)| vec![0xD0 | c, v]),
        (0u8..4, 0u8..128).prop_map(|(c, v)| vec![0xC0 | c, v]),
        Just(vec![0xFF, 0x51, 0x03, 0x09, 0x27, 0xC0]),
        Just(vec![0xFF, 0x58, 0x04, 0x03, 0x02, 0x18, 0x08]),
        Just(vec![0xFF, 0x05, 0x02, b'l', b'a']),
    ];
    proptest::collection::vec((0u32..500, msg), 0..60)
}

proptest! {
    #[test]
    fn song_round_trip_is_lossless_property(events in arb_events(), ppq in 1u16..2000) {
        let refs: Vec<(u32, &[u8])> = events.iter().map(|(d, b)| (*d, b.as_slice())).collect();
        let bytes = smf(1, ppq, &[track(&refs)]);
        let first = song_of(&bytes);
        let again = song_of(&first.to_smf(SmfFormat::Parallel).unwrap().write());
        prop_assert_eq!(&again, &first);
        let single = song_of(&first.to_smf(SmfFormat::Single).unwrap().write());
        prop_assert_eq!(by_channel(&single), by_channel(&first));
    }
}

#[test]
fn many_tempo_and_lyric_events_import_without_quadratic_time() {
    // 템포·가사 변화가 20만 개씩이어도 한 번에 만든다(항목마다 다시 계산하면 수 분이 걸린다).
    let tempo: &[u8] = &[0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20];
    let lyric: &[u8] = &[0xFF, 0x05, 0x01, b'a'];
    let mut events: Vec<(u32, &[u8])> = Vec::new();
    for _ in 0..100_000 {
        events.push((1, tempo));
        events.push((0, lyric));
        events.push((0, &[0x90, 60, 100]));
        events.push((1, &[0x80, 60, 0]));
    }
    let bytes = smf(0, 960, &[track(&events)]);
    let started = std::time::Instant::now();
    let song = song_of(&bytes);
    let project = song.to_project().unwrap();
    assert_eq!(song.tracks()[0].notes().len(), 100_000);
    assert!(project.tempo_map().changes().count() > 99_000);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn lyrics_only_track_attaches_to_matching_voice() {
    // LilyPond(bach)는 가사를 채널 메시지 없는 트랙에, aloha-oe는 노트 트랙에 둔다. 둘 다 가사를 잃지
    // 않고, 모델에서는 노트에 붙는다(LYR-6).
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/smf");
    for (file, count) in [("bach-sheep-safely-graze.mid", 110), ("aloha-oe.mid", 73)] {
        let song = song_of(&std::fs::read(dir.join(file)).unwrap());
        let kept: usize = song.tracks().iter().map(|t| t.lyrics().len()).sum();
        assert_eq!(kept, count, "{file}");
        let project = song.to_project().unwrap();
        let on_notes: usize = project
            .tracks()
            .iter()
            .flat_map(Track::clips)
            .map(|(_, c)| c.notes().iter().filter(|n| n.lyric().is_some()).count())
            .sum();
        assert!(
            on_notes * 10 >= count * 9,
            "{file}: 노트에 붙은 가사 {on_notes}/{count}"
        );
    }
}

#[test]
fn song_longer_than_limit_is_rejected_on_read_and_write() {
    // PPQ 1에서 델타 (2^28 - 1) 다섯 번 = 960배로 약 1.3 × 10^12틱 > MAX_SONG_TICKS.
    let gap: &[u8] = &[0xFF, 0x01, 0x00];
    let events: Vec<(u32, &[u8])> = (0..5).map(|_| (0x0FFF_FFFF, gap)).collect();
    let file = SmfFile::read(&smf(0, 1, &[track(&events)])).unwrap();
    assert!(matches!(
        Song::from_smf(&file),
        Err(SmfError::TooLong { .. })
    ));
    // 공개 API로 상한 뒤 노트를 넣어도 쓰기는 조용히 자르지 않고 `Err`다.
    let mut song = Song::new(TempoMap::default(), MeterMap::default());
    let mut t = keyscore_midi::SongTrack::new(None, ch(1));
    t.add_note(keyscore_midi::SongNote::new(
        Tick::new(keyscore_midi::MAX_SONG_TICKS),
        10,
        pitch(60),
        vel(100),
    ));
    song.push_track(t);
    assert!(matches!(
        song.to_smf(SmfFormat::Parallel),
        Err(SmfError::TooLong { .. })
    ));
}

#[test]
fn chord_lyrics_round_trip_through_smf_to_same_notes() {
    // 화음 C·E·G 가운데 C와 G에 가사가 있으면, 모델 → SMF → 모델에서 같은 노트에 돌아온다.
    let mut project = Project::new();
    let mut track = Track::new("Choir", ch(1), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for (p, lyric) in [(60, Some("lo")), (64, None), (67, Some("hi"))] {
        let mut n = Note::new(Tick::ZERO, 960, pitch(p), vel(100)).unwrap();
        if let Some(l) = lyric {
            n = n.with_lyric(l);
        }
        clip.insert_note(n).unwrap();
    }
    track.insert_clip(clip).unwrap();
    project.push_track(track).unwrap();
    let bytes = Song::from_project(&project)
        .to_smf(SmfFormat::Parallel)
        .unwrap()
        .write();
    let back = song_of(&bytes).to_project().unwrap();
    let lyrics: Vec<(u8, Option<String>)> = back.tracks()[0]
        .clips()
        .flat_map(|(_, c)| c.notes().iter())
        .map(|n| (n.pitch().get(), n.lyric().map(str::to_string)))
        .collect();
    let want: Vec<(u8, Option<String>)> = vec![
        (60, Some("lo".to_string())),
        (64, None),
        (67, Some("hi".to_string())),
    ];
    assert_eq!(lyrics, want);
    // 아래 음 하나에만 가사가 있어도 그 음으로 돌아온다(빈 가사가 위 음의 자리를 채운다).
    let mut project = Project::new();
    let mut track = Track::new("Low", ch(1), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    clip.insert_note(
        Note::new(Tick::ZERO, 960, pitch(48), vel(100))
            .unwrap()
            .with_lyric("bass"),
    )
    .unwrap();
    clip.insert_note(Note::new(Tick::ZERO, 960, pitch(72), vel(100)).unwrap())
        .unwrap();
    track.insert_clip(clip).unwrap();
    project.push_track(track).unwrap();
    let bytes = Song::from_project(&project)
        .to_smf(SmfFormat::Parallel)
        .unwrap()
        .write();
    let back = song_of(&bytes).to_project().unwrap();
    let lyrics: Vec<(u8, Option<String>)> = back.tracks()[0]
        .clips()
        .flat_map(|(_, c)| c.notes().iter())
        .map(|n| (n.pitch().get(), n.lyric().map(str::to_string)))
        .collect();
    assert_eq!(lyrics, vec![(48, Some("bass".to_string())), (72, None)]);
}
