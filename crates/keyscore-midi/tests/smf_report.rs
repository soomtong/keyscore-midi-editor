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
            .flat_map(Track::clips)
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

// ---------------------------------------------------------------- S2-T7e: S2-T7c 리뷰 2 이월(부록 B)

fn v100() -> u16 {
    Velocity::from_midi1(100).unwrap().get()
}

/// 채널 `channel`의 트랙 하나에 클립 하나(길이 3840)와 노트들.
fn track_with(name: &str, channel: u8, color: u8, notes: &[Note]) -> Track {
    let mut t = Track::new(
        name,
        Channel::new(channel).unwrap(),
        TrackColor::new(color).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    t.insert_clip(clip).unwrap();
    t
}

fn one_track(notes: &[Note]) -> Project {
    let mut p = Project::new();
    p.push_track(track_with("V", 1, 0, notes)).unwrap();
    p
}

/// 내보내고 다시 읽은 모든 트랙의 (시작, 길이)와 내보내기 보고.
fn reread(p: &Project, format: SmfFormat) -> (Vec<(u64, u32)>, keyscore_midi::ExportReport) {
    let (file, ex) = smf_export(p, format).unwrap();
    let back = SmfFile::read(&file.write()).unwrap();
    let (q, _) = smf_import(&back).unwrap();
    let mut notes: Vec<(u64, u32)> = q
        .tracks()
        .iter()
        .flat_map(|t| {
            t.clips()
                .flat_map(|(_, c)| {
                    c.notes()
                        .iter()
                        .map(|n| (c.start().get() + n.start().get(), n.length()))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    notes.sort_unstable();
    (notes, ex)
}

/// 끝이 같은 같은 음 겹침은 다시 읽어도 그대로다. 세지 않는다.
#[test]
fn same_end_overlap_is_unchanged_and_not_counted() {
    let p = one_track(&[note(0, 960, 60, v100()), note(480, 480, 60, v100())]);
    let (notes, ex) = reread(&p, SmfFormat::Parallel);
    assert_eq!(notes, vec![(0, 960), (480, 480)], "다시 읽은 노트");
    assert_eq!(ex.overlapping_notes, 0, "바뀌지 않는데 셌다: {ex:?}");
    assert!(ex.is_lossless(), "{ex:?}");
}

/// 시작이 같은 같은 음(길이만 다름)도 그대로다. 세지 않는다.
#[test]
fn same_start_overlap_is_unchanged_and_not_counted() {
    let p = one_track(&[note(0, 480, 60, v100()), note(0, 1920, 60, v100())]);
    let (notes, ex) = reread(&p, SmfFormat::Parallel);
    assert_eq!(notes, vec![(0, 480), (0, 1920)], "다시 읽은 노트");
    assert_eq!(ex.overlapping_notes, 0, "바뀌지 않는데 셌다: {ex:?}");
}

/// 겹침 하나로 두 노트의 길이가 바뀌면 2다.
#[test]
fn changed_lengths_match_overlap_count() {
    let p = one_track(&[note(0, 1920, 60, v100()), note(480, 480, 60, v100())]);
    let (notes, ex) = reread(&p, SmfFormat::Parallel);
    let changed = [(0u64, 1920u32), (480, 480)]
        .iter()
        .filter(|n| !notes.contains(n))
        .count();
    assert_eq!(changed, 2, "{notes:?}");
    assert_eq!(ex.overlapping_notes, changed, "{ex:?}");
}

/// 세 노트가 겹치면 바뀌는 노트만 센다: (0–1920)·(480–960)·(960–1440)는 다시 읽으면 (0–960)·(480–1440)·
/// (960–1920)이 되어 셋 다 바뀐다. 같은 음이 아닌 노트는 세지 않는다.
#[test]
fn three_overlapping_notes_count_each_changed_note() {
    let p = one_track(&[
        note(0, 1920, 60, v100()),
        note(480, 480, 60, v100()),
        note(960, 480, 60, v100()),
        note(0, 1920, 62, v100()),
    ]);
    let (notes, ex) = reread(&p, SmfFormat::Parallel);
    let original = [(0u64, 1920u32), (480, 480), (960, 480), (0, 1920)];
    let mut rest = notes.clone();
    let changed = original
        .iter()
        .filter(|n| {
            rest.iter()
                .position(|m| m == *n)
                .map(|i| rest.remove(i))
                .is_none()
        })
        .count();
    assert_eq!(changed, 3, "{notes:?}");
    assert_eq!(ex.overlapping_notes, 3, "{ex:?}");
}

/// Type 0은 모든 트랙이 한 SMF 트랙이라 같은 채널의 다른 트랙과 짝이 섞인다. Type 1은 트랙마다 따로다.
#[test]
fn type0_pools_same_channel_tracks_for_overlap_count() {
    let mut p = one_track(&[note(0, 1920, 60, v100())]);
    p.push_track(track_with("W", 1, 1, &[note(480, 480, 60, v100())]))
        .unwrap();
    let (_, parallel) = reread(&p, SmfFormat::Parallel);
    assert_eq!(parallel.overlapping_notes, 0, "{parallel:?}");
    let (notes, single) = reread(&p, SmfFormat::Single);
    let changed = [(0u64, 1920u32), (480, 480)]
        .iter()
        .filter(|n| !notes.contains(n))
        .count();
    assert_eq!(changed, 2, "{notes:?}");
    assert_eq!(single.overlapping_notes, 2, "{single:?}");
    // 채널이 다르면 Type 0에서도 섞이지 않는다
    let mut q = one_track(&[note(0, 1920, 60, v100())]);
    q.push_track(track_with("X", 2, 1, &[note(480, 480, 60, v100())]))
        .unwrap();
    let (_, other) = reread(&q, SmfFormat::Single);
    assert_eq!(other.overlapping_notes, 0, "{other:?}");
}

/// 같은 틱의 서로 다른 템포 둘은 하나만 남는다. 버린 하나를 센다.
#[test]
fn same_tick_tempos_drop_one_and_are_counted() {
    let file = smf(
        1,
        &[
            track(&[(0, &[0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20])]),
            track(&[
                (0, &[0xFF, 0x51, 0x03, 0x05, 0x16, 0x15]),
                (0, &[0x90, 60, 100]),
                (480, &[0x80, 60, 0]),
            ]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tempo_map().changes().count(), 1);
    assert_eq!(report.skipped_events, 1, "{report:?}");
    assert!(
        !report.is_lossless(),
        "템포 하나를 버렸는데 손실 없음: {report:?}"
    );
}

/// 같은 틱의 같은 템포·박자표(Type 1 파일이 여러 트랙에 겹쳐 적는 흔한 경우)는 손실이 아니다. PPQ 재배율로
/// 다른 틱이 같은 틱이 되는 경우도 같다. 같은 틱에 셋이면 마지막과 다른 것만 센다.
#[test]
fn same_tick_equal_tempos_are_not_counted() {
    let tempo120: &[u8] = &[0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20];
    let tempo125: &[u8] = &[0xFF, 0x51, 0x03, 0x07, 0x53, 0x00];
    let common: &[u8] = &[0xFF, 0x58, 0x04, 0x04, 0x02, 0x18, 0x08];
    let file = smf(
        1,
        &[
            track(&[(0, tempo120), (0, common)]),
            track(&[
                (0, tempo120),
                (0, common),
                (0, &[0x90, 60, 100]),
                (480, &[0x80, 60, 0]),
            ]),
        ],
    );
    let (_, report) = smf_import(&file).unwrap();
    assert!(report.is_lossless(), "{report:?}");
    // 같은 틱 셋: 120, 125, 120 → 125 하나만 버린다
    let file = smf(
        1,
        &[
            track(&[(0, tempo120), (0, tempo125), (0, tempo120)]),
            track(&[(0, &[0x90, 60, 100]), (480, &[0x80, 60, 0])]),
        ],
    );
    let (_, report) = smf_import(&file).unwrap();
    assert_eq!(report.skipped_events, 1, "{report:?}");
    // PPQ 9600에서는 틱 0과 틱 1이 960으로 재배율하면 둘 다 0이 된다. 다른 틱에 적힌 다른 템포 둘이 같은 틱이
    // 되어 하나가 버려진다
    let mut header = Vec::new();
    header.extend(1u16.to_be_bytes());
    header.extend(2u16.to_be_bytes());
    header.extend(9600u16.to_be_bytes());
    let mut bytes = chunk(*b"MThd", &header);
    bytes.extend(track(&[(0, tempo120), (1, tempo125)]));
    bytes.extend(track(&[(0, &[0x90, 60, 100]), (4800, &[0x80, 60, 0])]));
    let (p, report) = smf_import(&SmfFile::read(&bytes).unwrap()).unwrap();
    assert_eq!(p.tempo_map().changes().count(), 1, "재배율로 같은 틱");
    assert_eq!(report.skipped_events, 1, "{report:?}");
}

/// 폴리 프레셔만 있는 Type 1 트랙은 곡 트랙이 되지 않아 이름도 버려진다. 이름을 센다.
#[test]
fn name_of_vanished_track_is_counted() {
    let file = smf(
        1,
        &[
            track(&[(0, &[0x90, 60, 100]), (480, &[0x80, 60, 0])]),
            track(&[(0, &[0xFF, 0x03, 0x01, b'P']), (0, &[0xA0, 60, 10])]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tracks().len(), 1);
    assert_eq!(
        report.skipped_events, 2,
        "폴리 프레셔 1 + 이름 1: {report:?}"
    );
    // 짝 없는 Note Off뿐인 트랙도 같다. 프로그램만 있는 트랙은 남는다(이름을 센 적 없음)
    let file = smf(
        1,
        &[
            track(&[(0, &[0xFF, 0x03, 0x01, b'Q']), (0, &[0x80, 60, 0])]),
            track(&[(0, &[0xFF, 0x03, 0x01, b'R']), (0, &[0xC0, 5])]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tracks().len(), 1);
    assert_eq!(report.skipped_events, 1, "이름 Q만: {report:?}");
}
