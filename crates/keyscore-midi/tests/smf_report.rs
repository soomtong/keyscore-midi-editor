//! S2-T7c SMF 가져오기·내보내기 결과 보고(S1-T6 이월: 버린 항목을 알린다). 보고 함수는 기존 변환과 같은 결과를
//! 내고, 버린 것을 센다.

use std::path::Path;

use keyscore_core::{
    Channel, Clip, Note, Pitch, PitchOffset, Program, Project, Tick, Track, TrackColor, Velocity,
    ksq,
};
use keyscore_midi::{SmfFile, SmfFormat, Song, smf_export, smf_import};

fn vlq(mut n: u32) -> Vec<u8> {
    let mut out = vec![u8::try_from(n & 0x7F).unwrap()];
    n >>= 7;
    while n > 0 {
        out.insert(0, u8::try_from(n & 0x7F).unwrap() | 0x80);
        n >>= 7;
    }
    out
}

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

fn smf(format: u16, tracks: &[Vec<u8>]) -> SmfFile {
    let mut header = Vec::new();
    header.extend(format.to_be_bytes());
    header.extend(u16::try_from(tracks.len()).unwrap().to_be_bytes());
    header.extend(960u16.to_be_bytes());
    let mut out = chunk(*b"MThd", &header);
    for t in tracks {
        out.extend_from_slice(t);
    }
    SmfFile::read(&out).unwrap()
}

fn samples() -> Vec<SmfFile> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/smf");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "mid") {
            out.push(SmfFile::read(&std::fs::read(&path).unwrap()).unwrap());
        }
    }
    assert!(out.len() >= 10);
    out
}

#[test]
fn import_matches_existing_conversion_on_samples() {
    for file in samples() {
        let expected = Song::from_smf(&file).unwrap().to_project().unwrap();
        let (project, report) = smf_import(&file).unwrap();
        assert_eq!(ksq::to_string(&project), ksq::to_string(&expected));
        let notes: usize = project
            .tracks()
            .iter()
            .flat_map(|t| t.clips())
            .map(|(_, c)| c.notes().len())
            .sum();
        assert_eq!(report.notes, notes);
        assert_eq!(report.tracks, project.tracks().len());
    }
}

#[test]
fn export_matches_existing_conversion_on_samples() {
    for file in samples() {
        let project = Song::from_smf(&file).unwrap().to_project().unwrap();
        let expected = Song::from_project(&project)
            .to_smf(SmfFormat::Parallel)
            .unwrap()
            .write();
        let (written, _) = smf_export(&project, SmfFormat::Parallel).unwrap();
        assert_eq!(written.write(), expected);
    }
}

#[test]
fn import_report_counts_dropped_items() {
    let file = smf(
        1,
        &[
            // 곡 트랙: 노트 하나, CC 둘, 벤드 하나, 채널 프레셔 하나, 프로그램 둘, 폴리 프레셔 하나, SysEx 하나,
            // 텍스트 메타 하나, 노트 시작이 아닌 틱의 가사 하나.
            track(&[
                (0, &[0xC0, 0x05]),
                (0, &[0x90, 60, 100]),
                (0, &[0xB0, 7, 100]),
                (0, &[0xB0, 10, 64]),
                (0, &[0xE0, 0, 64]),
                (0, &[0xD0, 30]),
                (0, &[0xA0, 60, 20]),
                (0, &[0xF0, 0x02, 0x7E, 0xF7]),
                (0, &[0xFF, 0x01, 0x02, b'h', b'i']),
                (240, &[0xFF, 0x05, 0x01, b'x']),
                (240, &[0x80, 60, 0]),
                (0, &[0xC0, 0x06]),
            ]),
        ],
    );
    let (_, report) = smf_import(&file).unwrap();
    assert_eq!(report.tracks, 1);
    assert_eq!(report.notes, 1);
    assert_eq!(report.lane_points, 4, "CC 둘 + 벤드 + 채널 프레셔");
    assert_eq!(report.extra_programs, 1);
    assert_eq!(
        report.skipped_events, 3,
        "폴리 프레셔 + SysEx + 텍스트 메타"
    );
    assert_eq!(report.unattached_lyrics, 1);
    assert_eq!(report.orphan_lyrics, 0);
    assert!(!report.is_lossless());
}

#[test]
fn import_report_counts_lyrics_without_song_tracks() {
    // 가사만 있는 파일: 붙일 곡 트랙이 없어 모두 버린다.
    let file = smf(
        1,
        &[track(&[
            (0, &[0xFF, 0x05, 0x01, b'a']),
            (480, &[0xFF, 0x05, 0x01, b'b']),
        ])],
    );
    let (project, report) = smf_import(&file).unwrap();
    assert!(project.tracks().is_empty());
    assert_eq!(report.orphan_lyrics, 2);
}

#[test]
fn clean_import_is_lossless() {
    let file = smf(
        1,
        &[track(&[
            (0, &[0xFF, 0x03, 0x04, b'L', b'e', b'a', b'd']),
            (0, &[0x90, 60, 100]),
            (0, &[0xFF, 0x05, 0x01, b'a']),
            (480, &[0x80, 60, 0]),
        ])],
    );
    let (_, report) = smf_import(&file).unwrap();
    assert!(report.is_lossless(), "{report:?}");
}

fn note(start: u64, length: u32, pitch: u8, velocity: u16) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::new(velocity),
    )
    .unwrap()
}

#[test]
fn export_report_counts_losses() {
    let mut p = Project::new();
    let mut lead = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::new(0), 960).unwrap();
    // 7비트로 정확히 나타나는 벨로시티 하나, 바뀌는 벨로시티 하나.
    let exact = Velocity::from_midi1(100).unwrap().get();
    clip.insert_note(note(0, 240, 60, exact)).unwrap();
    clip.insert_note(note(240, 240, 62, exact + 1)).unwrap();
    // 피치 오프셋.
    clip.insert_note(
        note(480, 240, 64, exact)
            .with_offset(PitchOffset::new(10).unwrap())
            .unwrap(),
    )
    .unwrap();
    // 숨김 꼬리: 클립을 노트 가운데서 나누면 앞 클립에 꼬리가 남는다.
    clip.insert_note(note(720, 240, 65, exact)).unwrap();
    let id = lead.insert_clip(clip).unwrap();
    lead.split_clip(id, Tick::new(840)).unwrap();
    p.push_track(lead).unwrap();
    // 노트·프로그램·뱅크가 없는 트랙.
    p.push_track(Track::new(
        "Empty",
        Channel::new(2).unwrap(),
        TrackColor::new(1).unwrap(),
    ))
    .unwrap();
    // 프로그램만 있는 트랙은 다른 프로그램도 읽는다.
    let mut pad = Track::new("Pad", Channel::new(3).unwrap(), TrackColor::new(2).unwrap());
    pad.set_program(Some(Program::new(5).unwrap()));
    p.push_track(pad).unwrap();
    let (_, report) = smf_export(&p, SmfFormat::Parallel).unwrap();
    assert_eq!(report.velocity_rounded, 1);
    assert_eq!(report.pitch_offsets, 1);
    assert_eq!(report.hidden_tails, 1);
    assert_eq!(report.empty_tracks, 1);
    assert!(!report.is_lossless());
}

#[test]
fn imported_project_exports_losslessly() {
    for file in samples() {
        let (project, _) = smf_import(&file).unwrap();
        let (_, report) = smf_export(&project, SmfFormat::Parallel).unwrap();
        assert_eq!(
            report.velocity_rounded, 0,
            "7비트에서 온 벨로시티는 그대로 돌아간다"
        );
        assert_eq!(report.pitch_offsets, 0);
    }
}
