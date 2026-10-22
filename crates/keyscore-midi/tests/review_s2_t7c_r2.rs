//! S2-T7c 적대적 리뷰 2회차 시험(SMF 보고). 깨뜨리지 못한 시도만 남긴다. 깨진 재현은
//! `docs/reports/s2-t7c/review-2.md` 부록에 있다.

use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use keyscore_midi::{ExportReport, ImportReport, SmfFile, SmfFormat, smf_export, smf_import};

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

fn note(start: u64, len: u32, pitch: u8, v: u16) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::new(v),
    )
    .unwrap()
}

fn v100() -> u16 {
    Velocity::from_midi1(100).unwrap().get()
}

fn clips(spec: &[(u64, u64, Vec<Note>)]) -> Project {
    let mut p = Project::new();
    let mut t = Track::new("V", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    for (start, len, notes) in spec {
        let mut clip = Clip::new(Tick::new(*start), *len).unwrap();
        for n in notes {
            clip.insert_note(n.clone()).unwrap();
        }
        t.insert_clip(clip).unwrap();
    }
    p.push_track(t).unwrap();
    p
}

fn round_trip(p: &Project) -> (Vec<(u64, u32)>, ImportReport, ExportReport) {
    let (file, ex) = smf_export(p, SmfFormat::Parallel).unwrap();
    let back = SmfFile::read(&file.write()).unwrap();
    let (q, report) = smf_import(&back).unwrap();
    let mut notes: Vec<(u64, u32)> = q.tracks()[0]
        .clips()
        .flat_map(|(_, c)| {
            c.notes()
                .iter()
                .map(|n| (c.start().get() + n.start().get(), n.length()))
                .collect::<Vec<_>>()
        })
        .collect();
    notes.sort_unstable();
    (notes, report, ex)
}

/// 리뷰 1 지적 5 회귀: 겹친 같은 음은 센다. 길이가 실제로 바뀐다.
#[test]
fn overlapping_same_pitch_that_changes_is_reported() {
    let p = clips(&[(
        0,
        3840,
        vec![note(0, 1920, 60, v100()), note(480, 480, 60, v100())],
    )]);
    let (notes, _, ex) = round_trip(&p);
    assert_ne!(notes, vec![(0, 1920), (480, 480)]);
    assert!(ex.overlapping_notes > 0, "{ex:?}");
}

/// 숨김 꼬리는 클립 끝에서 자른 뒤 겹침을 본다. 잘린 꼬리가 다음 클립의 같은 음과 겹치지 않으면 세지 않는다.
#[test]
fn hidden_tail_cut_at_clip_end_is_not_overlap() {
    let mut p = Project::new();
    let mut t = Track::new("V", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 1920).unwrap();
    clip.insert_note(note(0, 1920, 60, v100())).unwrap();
    clip.insert_note(note(960, 480, 60, v100())).unwrap();
    let id = t.insert_clip(clip).unwrap();
    t.split_clip(id, Tick::new(960)).unwrap();
    p.push_track(t).unwrap();
    let (notes, _, ex) = round_trip(&p);
    assert_eq!(notes, vec![(0, 960), (960, 480)]);
    assert_eq!(ex.overlapping_notes, 0, "{ex:?}");
}

/// 맞닿은 같은 음(끝 = 시작)은 겹침이 아니다.
#[test]
fn adjacent_same_pitch_is_not_overlap() {
    let p = clips(&[(
        0,
        3840,
        vec![note(0, 480, 60, v100()), note(480, 480, 60, v100())],
    )]);
    let (notes, _, ex) = round_trip(&p);
    assert_eq!(notes, vec![(0, 480), (480, 480)]);
    assert!(ex.is_lossless(), "{ex:?}");
}

/// 다른 트랙(같은 채널)의 같은 음은 SMF 트랙이 달라 짝이 섞이지 않는다. 세지 않는다.
#[test]
fn same_pitch_in_other_track_is_not_overlap() {
    let mut p = clips(&[(0, 3840, vec![note(0, 1920, 60, v100())])]);
    let mut t = Track::new("W", Channel::new(1).unwrap(), TrackColor::new(1).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    clip.insert_note(note(480, 480, 60, v100())).unwrap();
    t.insert_clip(clip).unwrap();
    p.push_track(t).unwrap();
    let (notes, _, ex) = round_trip(&p);
    assert_eq!(notes, vec![(0, 1920)]);
    assert!(ex.is_lossless(), "{ex:?}");
}

/// 리뷰 1 제안 6 회귀: 벨로시티 1(MIDI 1.0 최소)은 손실이 아니고, 0은 센다.
#[test]
fn velocity_one_is_lossless_zero_is_counted() {
    let one = Velocity::from_midi1(1).unwrap().get();
    let p = clips(&[(0, 3840, vec![note(0, 480, 60, one), note(480, 480, 62, 0)])]);
    let (_, _, ex) = round_trip(&p);
    assert_eq!(ex.velocity_rounded, 1, "{ex:?}");
}

/// 리뷰 1 지적 3 회귀: 빈 가사를 세지 않아도, 빈 것 아닌 붙지 않은 가사는 여전히 센다.
#[test]
fn nonempty_unattached_lyric_is_still_counted() {
    let file = smf(
        1,
        &[track(&[
            (0, &[0x90, 60, 100]),
            (0, &[0xFF, 0x05, 0x00]),
            (240, &[0xFF, 0x05, 0x01, b'x']),
            (240, &[0x80, 60, 0]),
        ])],
    );
    let (_, report) = smf_import(&file).unwrap();
    assert_eq!(report.unattached_lyrics, 1, "{report:?}");
}

/// Type 1 곡 트랙의 둘째 이름, 가사만 담은 트랙의 이름은 버린 것으로 센다. 첫 이름은 세지 않는다.
#[test]
fn second_name_and_lyric_track_name_are_counted() {
    let file = smf(
        1,
        &[
            track(&[
                (0, &[0xFF, 0x03, 0x01, b'A']),
                (0, &[0xFF, 0x03, 0x01, b'B']),
                (0, &[0x90, 60, 100]),
                (480, &[0x80, 60, 0]),
            ]),
            track(&[
                (0, &[0xFF, 0x03, 0x01, b'L']),
                (0, &[0xFF, 0x05, 0x01, b'a']),
            ]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tracks()[0].name(), "A");
    assert_eq!(report.skipped_events, 2, "{report:?}");
}
