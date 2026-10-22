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
