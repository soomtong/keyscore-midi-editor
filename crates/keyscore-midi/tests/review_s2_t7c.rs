//! S2-T7c 적대적 리뷰 1회차 시험(SMF 보고). 깨뜨리지 못한 시도만 남긴다. 깨진 재현은
//! `docs/reports/s2-t7c/review-1.md` 부록에 있다.

use keyscore_midi::{SmfFile, smf_import};

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

/// 가사만 담은 트랙의 가사가 곡 트랙의 노트 시작에 붙으면 버린 것으로 세지 않는다.
#[test]
fn lyric_track_attached_to_song_track_is_not_counted() {
    let file = smf(
        1,
        &[
            track(&[
                (0, &[0x90, 60, 100]),
                (480, &[0x80, 60, 0]),
                (0, &[0x90, 62, 100]),
                (480, &[0x80, 62, 0]),
            ]),
            track(&[
                (0, &[0xFF, 0x05, 0x01, b'a']),
                (480, &[0xFF, 0x05, 0x01, b'b']),
            ]),
        ],
    );
    let (_, report) = smf_import(&file).unwrap();
    assert_eq!(report.orphan_lyrics, 0);
    assert_eq!(report.unattached_lyrics, 0);
    assert!(report.is_lossless(), "{report:?}");
}

/// CC만 있는 트랙은 트랙으로 남고 그 CC는 레인 포인트로 센다. 뱅크 CC 0·32는 세지 않는다.
#[test]
fn cc_only_track_counts_lane_points_but_not_bank() {
    let file = smf(
        1,
        &[
            track(&[(0, &[0x90, 60, 100]), (480, &[0x80, 60, 0])]),
            track(&[
                (0, &[0xB1, 0, 1]),
                (0, &[0xB1, 32, 2]),
                (0, &[0xB1, 7, 100]),
            ]),
        ],
    );
    let (project, report) = smf_import(&file).unwrap();
    assert_eq!(report.tracks, project.tracks().len());
    assert_eq!(report.tracks, 2);
    assert_eq!(report.lane_points, 1);
}

/// Type 0의 채널별 트랙: 노트·트랙 수가 모델과 같다.
#[test]
fn type0_split_counts_match_model() {
    let file = smf(
        0,
        &[track(&[
            (0, &[0x90, 60, 100]),
            (0, &[0x91, 40, 100]),
            (0, &[0x99, 36, 100]),
            (480, &[0x80, 60, 0]),
            (0, &[0x81, 40, 0]),
            (0, &[0x89, 36, 0]),
        ])],
    );
    let (project, report) = smf_import(&file).unwrap();
    assert_eq!(report.tracks, 3);
    assert_eq!(project.tracks().len(), 3);
    assert_eq!(report.notes, 3);
}

// ---------- 리뷰 1 부록 B(수정 확인) ----------

// 아래 시험은 crates/keyscore-midi/tests/review_s2_t7c.rs 끝에 붙인다. 쓰는 이름을 더 들여온다.
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use keyscore_midi::{SmfFormat, smf_export};

fn note(start: u64, len: u32, pitch: u8, v: u16) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::new(v),
    )
    .unwrap()
}

fn one_clip(notes: Vec<Note>) -> Project {
    let mut p = Project::new();
    let mut t = Track::new("V", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for n in notes {
        clip.insert_note(n).unwrap();
    }
    t.insert_clip(clip).unwrap();
    p.push_track(t).unwrap();
    p
}

fn reimport(p: &Project) -> (Project, keyscore_midi::ImportReport, keyscore_midi::ExportReport) {
    let (file, ex) = smf_export(p, SmfFormat::Parallel).unwrap();
    let back = SmfFile::read(&file.write()).unwrap();
    let (q, report) = smf_import(&back).unwrap();
    (q, report, ex)
}

/// 지적 3: 화음의 아래 음에만 가사가 있으면 내보내기가 빈 가사로 자리를 채운다. 다시 가져오면 가사는 그대로인데
/// 빈 가사를 「노트 시작에 붙지 않은 가사」로 센다(과대 보고).
#[test]
fn reimport_of_chord_lyric_reports_nothing_unattached() {
    let v = Velocity::from_midi1(100).unwrap().get();
    let p = one_clip(vec![note(0, 480, 60, v).with_lyric("la"), note(0, 480, 64, v)]);
    let (q, report, ex) = reimport(&p);
    assert!(ex.is_lossless(), "{ex:?}");
    let lyrics: Vec<String> = q.tracks()[0]
        .clips()
        .flat_map(|(_, c)| c.notes().iter().filter_map(|n| n.lyric().map(str::to_owned)).collect::<Vec<_>>())
        .collect();
    assert_eq!(lyrics, vec!["la".to_owned()], "가사는 그대로 왕복한다");
    assert_eq!(report.unattached_lyrics, 0, "{report:?}");
}

/// 지적 5: 같은 트랙에서 같은 음이 겹치면 SMF에서 먼저 켠 것을 먼저 끄므로 길이가 바뀐다. 보고는 손실 없음.
#[test]
fn overlapping_same_pitch_export_is_counted() {
    let v = Velocity::from_midi1(100).unwrap().get();
    let p = one_clip(vec![note(0, 1920, 60, v), note(480, 480, 60, v)]);
    let (q, _, ex) = reimport(&p);
    let notes: Vec<(u64, u32)> = q.tracks()[0]
        .clips()
        .flat_map(|(_, c)| c.notes().iter().map(|n| (n.start().get(), n.length())).collect::<Vec<_>>())
        .collect();
    assert!(
        notes == vec![(0, 1920), (480, 480)] || !ex.is_lossless(),
        "겹친 같은 음이 {notes:?}로 바뀌었는데 손실 없음"
    );
}

/// 지적 6: 벨로시티 0은 Note On 1로 써서 바뀌는데 세지 않는다.
#[test]
fn velocity_zero_export_is_counted() {
    let p = one_clip(vec![note(0, 480, 60, 0)]);
    let (q, _, ex) = reimport(&p);
    let got = q.tracks()[0].clips().next().unwrap().1.notes()[0].velocity().get();
    assert!(got == 0 || ex.velocity_rounded == 1, "0이 {got}로 바뀌었는데 세지 않았다");
}

/// 지적 6: 지휘 트랙의 곡 제목, 0 µs 템포, 분모 2^20 박자표는 버리지만 세지 않는다.
#[test]
fn dropped_title_and_bad_meta_are_counted() {
    let file = smf(
        1,
        &[
            track(&[
                (0, &[0xFF, 0x03, 0x05, b'T', b'i', b't', b'l', b'e']),
                (0, &[0xFF, 0x51, 0x03, 0, 0, 0]),
                (0, &[0xFF, 0x58, 0x04, 4, 20, 24, 8]),
            ]),
            track(&[(0, &[0x90, 60, 100]), (480, &[0x80, 60, 0])]),
        ],
    );
    let (_, report) = smf_import(&file).unwrap();
    assert!(!report.is_lossless(), "{report:?}");
}
