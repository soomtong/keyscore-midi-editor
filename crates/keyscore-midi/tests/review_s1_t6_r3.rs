//! S1-T6 적대적 리뷰 3의 깨뜨리기 테스트(REQ IO-2, SEQ-9, LYR-6).
//!
//! 리뷰 2 지적 1–3의 근본 수정 여부와 화음 가사 자리의 왕복을 다시 흔든다
//! (`docs/reports/s1-t6/review-3.md`).

use std::time::{Duration, Instant};

use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use keyscore_midi::{SmfFile, SmfFormat, Song};

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

fn chunk(id: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend(u32::try_from(body.len()).unwrap().to_be_bytes());
    out.extend_from_slice(body);
    out
}

fn track_body(events: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (delta, bytes) in events {
        body.extend(vlq(*delta));
        body.extend_from_slice(bytes);
    }
    body.extend([0x00, 0xFF, 0x2F, 0x00]);
    body
}

fn smf(format: u16, ppq: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend(format.to_be_bytes());
    header.extend(u16::try_from(tracks.len()).unwrap().to_be_bytes());
    header.extend(ppq.to_be_bytes());
    let mut out = chunk(*b"MThd", &header);
    for body in tracks {
        out.extend(chunk(*b"MTrk", body));
    }
    out
}

fn song_of(bytes: &[u8]) -> Song {
    Song::from_smf(&SmfFile::read(bytes).unwrap()).unwrap()
}

fn lyric_meta(text: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, 0x05];
    out.extend(vlq(u32::try_from(text.len()).unwrap()));
    out.extend_from_slice(text);
    out
}

fn pitch(v: u8) -> Pitch {
    Pitch::new(v).unwrap()
}

fn vel(v: u8) -> Velocity {
    Velocity::from_midi1(v).unwrap()
}

fn ch(v: u8) -> Channel {
    Channel::new(v).unwrap()
}

/// 파일의 모든 Lyric 메타(틱, 바이트). 순서와 상관없이 비교하도록 정렬한다.
fn lyric_events(file: &SmfFile) -> Vec<(u64, Vec<u8>)> {
    let mut out: Vec<(u64, Vec<u8>)> = file
        .tracks()
        .iter()
        .flat_map(|t| t.events().iter())
        .filter_map(|e| match e.kind() {
            keyscore_midi::SmfEventKind::Meta { kind: 0x05, data } => {
                Some((e.tick(), data.clone()))
            }
            _ => None,
        })
        .collect();
    out.sort();
    out
}

// ---------- A. 남은 이차 경로 ----------

/// 노트 `notes`개(서로 다른 틱)인 트랙 하나와 가사 하나씩만 든 트랙 `orphans`개.
fn notes_and_orphan_lyric_tracks(notes: usize, orphans: usize) -> Vec<u8> {
    let mut events: Vec<(u32, Vec<u8>)> = Vec::with_capacity(2 * notes);
    for _ in 0..notes {
        events.push((1, vec![0x90, 60, 100]));
        events.push((1, vec![0x80, 60, 0]));
    }
    let mut tracks = vec![track_body(&events)];
    for _ in 0..orphans {
        tracks.push(track_body(&[(1, lyric_meta(b"la"))]));
    }
    smf(1, 960, &tracks)
}

/// `attach_lyrics`는 호출 하나가 (모든 곡 트랙의 노트 수 + 대상 가사 수)에 비례하지만, 가사만 든
/// 트랙마다 한 번씩 불린다. 그래서 가져오기 전체는 (가사 트랙 수 × 노트 수)다. 가사 트랙은 한 개에
/// 이벤트 하나(약 20 B)라 입력 크기는 거의 그대로인데 시간은 가사 트랙 수에 비례해 는다.
#[test]
fn many_single_lyric_tracks_with_many_notes_import_in_bounded_time() {
    const NOTES: usize = 990_000;
    let small = notes_and_orphan_lyric_tracks(NOTES, 1);
    let started = Instant::now();
    let song = song_of(&small);
    let base = started.elapsed();
    assert_eq!(song.tracks()[0].lyrics().len(), 1);

    let big = notes_and_orphan_lyric_tracks(NOTES, 1_000);
    let started = Instant::now();
    let song = song_of(&big);
    let took = started.elapsed();
    assert_eq!(song.tracks()[0].lyrics().len(), 1_000);

    // 선형 외삽: 입력 크기 비율만큼. 넉넉히 10배와 20 s 중 큰 쪽을 상한으로 둔다.
    #[allow(clippy::cast_precision_loss)]
    let ratio = big.len() as f64 / small.len() as f64;
    let limit = base.mul_f64(10.0 * ratio).max(Duration::from_secs(20));
    assert!(
        took < limit,
        "가사 트랙 1000개: {took:?}, 가사 트랙 1개: {base:?}, 입력 비율 {ratio:.3}, 상한 {limit:?}"
    );
}

// ---------- B. 화음 가사 자리 ----------

/// 숨김 꼬리가 클립 끝에서 잘려 같은 (틱, 음높이, 내보낸 길이)가 된 두 노트. 내보내기는 클립 순서
/// (모델 길이)로 가사를 내고, 가져오기는 곡 순서(길이 다음 벨로시티)로 짝짓는다. 모델 길이가 긴 쪽의
/// 벨로시티가 낮으면 가사가 서로 바뀐다.
#[test]
fn hidden_tail_chord_lyrics_with_equal_cut_length_stay_on_their_notes() {
    let mut project = Project::new();
    let mut track = Track::new("Choir", ch(1), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    // 둘 다 틱 3000에서 시작해 3200에서 나눈 뒤 잘린 길이가 200이다.
    clip.insert_note(
        Note::new(Tick::new(3000), 800, pitch(60), vel(50))
            .unwrap()
            .with_lyric("a"),
    )
    .unwrap();
    clip.insert_note(
        Note::new(Tick::new(3000), 700, pitch(60), vel(100))
            .unwrap()
            .with_lyric("b"),
    )
    .unwrap();
    let id = track.insert_clip(clip).unwrap();
    track.split_clip(id, Tick::new(3200)).unwrap();
    project.push_track(track).unwrap();

    for format in [SmfFormat::Parallel, SmfFormat::Single] {
        let bytes = Song::from_project(&project).to_smf(format).unwrap().write();
        let back = song_of(&bytes).to_project().unwrap();
        let mut by_velocity: Vec<(u16, Option<String>)> = back.tracks()[0]
            .clips()
            .flat_map(|(_, c)| c.notes().iter())
            .map(|n| (n.velocity().get(), n.lyric().map(str::to_string)))
            .collect();
        by_velocity.sort();
        assert_eq!(
            by_velocity,
            vec![
                (vel(50).get(), Some("a".to_string())),
                (vel(100).get(), Some("b".to_string())),
            ],
            "{format:?}"
        );
    }
}

/// 숨김 꼬리가 다른 길이로 잘리는 화음(같은 음높이 둘, 다른 음높이 하나)은 가사가 제자리다.
#[test]
fn hidden_tail_chord_lyrics_with_distinct_cut_lengths_round_trip() {
    let mut project = Project::new();
    let mut track = Track::new("Choir", ch(1), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for (len, p, v, text) in [
        (800u32, 60u8, 50u8, "long"),
        (100, 60, 100, "short"),
        (840, 55, 70, "low"),
    ] {
        clip.insert_note(
            Note::new(Tick::new(3000), len, pitch(p), vel(v))
                .unwrap()
                .with_lyric(text),
        )
        .unwrap();
    }
    let id = track.insert_clip(clip).unwrap();
    track.split_clip(id, Tick::new(3200)).unwrap();
    project.push_track(track).unwrap();
    for format in [SmfFormat::Parallel, SmfFormat::Single] {
        let bytes = Song::from_project(&project).to_smf(format).unwrap().write();
        let back = song_of(&bytes).to_project().unwrap();
        let mut got: Vec<(u8, u16, Option<String>)> = back.tracks()[0]
            .clips()
            .flat_map(|(_, c)| c.notes().iter())
            .map(|n| {
                (
                    n.pitch().get(),
                    n.velocity().get(),
                    n.lyric().map(str::to_string),
                )
            })
            .collect();
        got.sort();
        assert_eq!(
            got,
            vec![
                (55, vel(70).get(), Some("low".to_string())),
                (60, vel(50).get(), Some("long".to_string())),
                (60, vel(100).get(), Some("short".to_string())),
            ],
            "{format:?}"
        );
    }
}

/// SMF → 곡 → SMF가 Lyric 메타 집합을 지킨다. 가사와 짝 없는 Note Off(또는 폴리 프레셔)만 든
/// 트랙은 채널 메시지가 있어 가사만 든 트랙으로 취급되지 않고, 나눈 채널 트랙은 비어 버려진다.
#[test]
fn lyric_track_with_stray_channel_message_keeps_lyrics() {
    let notes = track_body(&[
        (0, vec![0x90, 60, 100]),
        (960, vec![0x80, 60, 0]),
        (0, vec![0x90, 62, 100]),
        (960, vec![0x80, 62, 0]),
    ]);
    let lyrics = track_body(&[
        (0, vec![0xA0, 60, 10]),
        (0, lyric_meta(b"hel")),
        (960, lyric_meta(b"lo")),
    ]);
    let input = SmfFile::read(&smf(1, 960, &[notes, lyrics])).unwrap();
    let song = Song::from_smf(&input).unwrap();
    let output = song.to_smf(SmfFormat::Parallel).unwrap();
    assert_eq!(lyric_events(&output), lyric_events(&input));
}
