//! S1-T6 적대적 리뷰 4(확인 리뷰)의 깨뜨리기 테스트(REQ IO-2, LYR-6).
//!
//! 결정 `docs/decisions/2026-10-16-s1-t6-review-limit.md`(A)에 따라 리뷰 3 지적 1–3의 이웃 입력만
//! 흔든다(`docs/reports/s1-t6/review-4.md`).

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

fn total_lyrics(song: &Song) -> usize {
    song.tracks().iter().map(|t| t.lyrics().len()).sum()
}

// ---------- A. 리뷰 3 지적 1의 이웃: 가사 트랙 여럿이 같은 틱을 나눔 ----------

/// 노트 `notes`개(틱 1, 2, ...)인 트랙 하나와, 틱 1..=`per_track`에 가사를 하나씩 둔 가사 트랙
/// `lyric_tracks`개. 가사 트랙끼리 같은 틱을 나눈다.
fn notes_and_shared_tick_lyric_tracks(
    notes: usize,
    lyric_tracks: usize,
    per_track: usize,
) -> Vec<u8> {
    let mut events: Vec<(u32, Vec<u8>)> = Vec::with_capacity(2 * notes);
    for _ in 0..notes {
        events.push((1, vec![0x90, 60, 100]));
        events.push((1, vec![0x80, 60, 0]));
    }
    // 노트는 틱 1, 3, 5, ...에 시작한다. 가사도 같은 틱에 둔다.
    let lyric_events: Vec<(u32, Vec<u8>)> = (0..per_track)
        .map(|i| (if i == 0 { 1 } else { 2 }, lyric_meta(b"la")))
        .collect();
    let lyric = track_body(&lyric_events);
    let mut tracks = vec![track_body(&events)];
    for _ in 0..lyric_tracks {
        tracks.push(lyric.clone());
    }
    smf(1, 960, &tracks)
}

/// 가사 트랙 1000개가 같은 틱 100개를 나눠도 가져오기는 (노트 + 가사)에 비례한다. 가사 총수가
/// 같은 기준(가사 트랙 1개 × 가사 10만 개)과 비교한다.
#[test]
fn many_lyric_tracks_sharing_ticks_import_in_bounded_time() {
    const NOTES: usize = 900_000;
    let small = notes_and_shared_tick_lyric_tracks(NOTES, 1, 100_000);
    let started = Instant::now();
    let song = song_of(&small);
    let base = started.elapsed();
    assert_eq!(song.tracks()[0].lyrics().len(), 100_000);

    let big = notes_and_shared_tick_lyric_tracks(NOTES, 1_000, 100);
    let started = Instant::now();
    let song = song_of(&big);
    let took = started.elapsed();
    // 모든 가사 트랙이 노트 트랙 하나에 맞는다.
    assert_eq!(song.tracks().len(), 1);
    assert_eq!(song.tracks()[0].lyrics().len(), 100_000);
    assert!(
        song.tracks()[0]
            .lyrics()
            .windows(2)
            .all(|w| w[0].0 <= w[1].0)
    );

    #[allow(clippy::cast_precision_loss)]
    let ratio = big.len() as f64 / small.len() as f64;
    let limit = base.mul_f64(10.0 * ratio).max(Duration::from_secs(20));
    eprintln!("공유 틱 가사 트랙 1000개: {took:?}, 기준 {base:?}");
    assert!(
        took < limit,
        "가사 트랙 1000개: {took:?}, 가사 트랙 1개: {base:?}, 입력 비율 {ratio:.3}, 상한 {limit:?}"
    );
}

/// 노트 트랙 `note_tracks`개(각각 채널 16개에 틱 `ticks`개의 노트)와 같은 틱에 가사를 둔 가사
/// 트랙 `lyric_tracks`개. `hit`가 거짓이면 가사를 노트가 없는 틱에 둔다(같은 입력 크기의 기준).
fn channel_split_tracks_and_lyric_tracks(
    note_tracks: usize,
    lyric_tracks: usize,
    ticks: usize,
    hit: bool,
) -> Vec<u8> {
    let mut events: Vec<(u32, Vec<u8>)> = Vec::new();
    for t in 0..ticks {
        for c in 0..16u8 {
            events.push((if c == 0 && t > 0 { 2 } else { 0 }, vec![0x90 | c, 60, 100]));
        }
        for c in 0..16u8 {
            events.push((u32::from(c == 0), vec![0x80 | c, 60, 0]));
        }
    }
    let notes = track_body(&events);
    // 노트는 틱 0, 3, 6, ...에 시작한다. 맞지 않는 기준은 틱 1, 4, 7, ...에 둔다.
    let lyric_events: Vec<(u32, Vec<u8>)> = (0..ticks)
        .map(|i| {
            let first = u32::from(!hit);
            (if i == 0 { first } else { 3 }, lyric_meta(b"la"))
        })
        .collect();
    let lyric = track_body(&lyric_events);
    let mut tracks = Vec::new();
    for _ in 0..note_tracks {
        tracks.push(notes.clone());
    }
    for _ in 0..lyric_tracks {
        tracks.push(lyric.clone());
    }
    smf(1, 960, &tracks)
}

/// 채널로 나뉜 곡 트랙이 많고(500 × 16) 가사 트랙(500)이 모두 같은 틱에 맞으면 `attach_lyrics`의
/// 「맞은 수」는 (곡 트랙 × 가사 트랙 × 틱)이다. 입력 크기가 같고 맞지 않는 기준과 비교한다.
#[test]
fn many_channel_tracks_and_lyric_tracks_on_shared_ticks_import_in_bounded_time() {
    let base_bytes = channel_split_tracks_and_lyric_tracks(500, 500, 120, false);
    let started = Instant::now();
    let song = song_of(&base_bytes);
    let base = started.elapsed();
    assert_eq!(total_lyrics(&song), 500 * 120);

    let bytes = channel_split_tracks_and_lyric_tracks(500, 500, 120, true);
    assert_eq!(bytes.len(), base_bytes.len());
    let started = Instant::now();
    let song = song_of(&bytes);
    let took = started.elapsed();
    assert_eq!(song.tracks().len(), 500 * 16);
    assert_eq!(total_lyrics(&song), 500 * 120);

    let limit = base.mul_f64(10.0).max(Duration::from_secs(20));
    eprintln!("채널 트랙 8000 × 가사 트랙 500: {took:?}, 기준 {base:?}");
    assert!(
        took < limit,
        "맞는 틱: {took:?}, 맞지 않는 기준: {base:?}, 상한 {limit:?}"
    );
}

// ---------- B. 리뷰 3 지적 2의 이웃: 같은 길이로 잘린 화음 ----------

fn split_choir(notes: &[(u32, u8, u8, &str)]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new("Choir", ch(1), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for &(len, p, v, text) in notes {
        let note = Note::new(Tick::new(3000), len, pitch(p), vel(v)).unwrap();
        // 빈 문자열은 가사 없는 노트다.
        let note = if text.is_empty() {
            note
        } else {
            note.with_lyric(text)
        };
        clip.insert_note(note).unwrap();
    }
    let id = track.insert_clip(clip).unwrap();
    track.split_clip(id, Tick::new(3200)).unwrap();
    project.push_track(track).unwrap();
    project
}

fn round_trip_notes(project: &Project, format: SmfFormat) -> Vec<(u8, u16, Option<String>)> {
    let bytes = Song::from_project(project).to_smf(format).unwrap().write();
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
    got
}

/// 같은 음높이 셋이 모두 길이 200으로 잘리고, 모델 길이 순서와 벨로시티 순서가 거꾸로다. 위에
/// 가사 없는 높은 음도 있어 빈 가사 자리가 앞에 낀다.
#[test]
fn three_equal_cut_length_notes_with_reversed_velocity_keep_lyrics() {
    let project = split_choir(&[
        (820, 60, 30, "x"),
        (780, 60, 60, "y"),
        (740, 60, 90, "z"),
        (650, 55, 40, "low"),
        (400, 72, 80, ""),
    ]);
    for format in [SmfFormat::Parallel, SmfFormat::Single] {
        assert_eq!(
            round_trip_notes(&project, format),
            vec![
                (55, vel(40).get(), Some("low".to_string())),
                (60, vel(30).get(), Some("x".to_string())),
                (60, vel(60).get(), Some("y".to_string())),
                (60, vel(90).get(), Some("z".to_string())),
                (72, vel(80).get(), None),
            ],
            "{format:?}"
        );
    }
}

/// 같은 길이로 잘리고 벨로시티까지 같은 두 노트는 SMF에서 구별할 수 없다(보고서에 적힌 한계).
/// 가사 둘이 사라지거나 한 노트에 몰리지 않고 두 노트에 하나씩 남는지만 본다.
#[test]
fn equal_cut_length_and_velocity_notes_keep_both_lyrics() {
    let project = split_choir(&[(800, 60, 64, "a"), (700, 60, 64, "b")]);
    for format in [SmfFormat::Parallel, SmfFormat::Single] {
        let got = round_trip_notes(&project, format);
        let mut lyrics: Vec<Option<String>> = got.into_iter().map(|g| g.2).collect();
        lyrics.sort();
        assert_eq!(
            lyrics,
            vec![Some("a".to_string()), Some("b".to_string())],
            "{format:?}"
        );
    }
}

// ---------- C. 리뷰 3 지적 3의 이웃: 채널 메시지가 남지 않는 트랙 ----------

/// Type 0 한 트랙에 채널 1 노트, 채널 2 폴리 프레셔·짝 없는 Note Off, 가사. 채널 2 트랙은 비어
/// 버려지지만 가사는 채널 1 트랙에 남는다.
#[test]
fn type0_stray_channel_with_lyrics_keeps_lyrics() {
    let body = track_body(&[
        (0, vec![0xA1, 60, 10]),
        (0, vec![0x81, 64, 0]),
        (0, lyric_meta(b"hel")),
        (0, vec![0x90, 60, 100]),
        (960, vec![0x80, 60, 0]),
        (0, lyric_meta(b"lo")),
        (0, vec![0x90, 62, 100]),
        (960, vec![0x80, 62, 0]),
        (480, lyric_meta(b"tail")),
    ]);
    let input = SmfFile::read(&smf(0, 960, &[body])).unwrap();
    let song = Song::from_smf(&input).unwrap();
    assert_eq!(song.tracks().len(), 1);
    for format in [SmfFormat::Parallel, SmfFormat::Single] {
        let output = song.to_smf(format).unwrap();
        assert_eq!(lyric_events(&output), lyric_events(&input), "{format:?}");
    }
}

/// Type 1: 채널 메시지만 남지 않는 트랙 여럿(폴리 프레셔, 짝 없는 Note Off, 둘 다)이 같은 틱의
/// 가사를 나누고, 가사만 든 트랙도 섞인다. 모든 가사가 남는다.
#[test]
fn type1_several_stray_channel_tracks_with_shared_lyric_ticks_keep_lyrics() {
    let notes = track_body(&[
        (0, vec![0x90, 60, 100]),
        (960, vec![0x80, 60, 0]),
        (0, vec![0x90, 62, 100]),
        (960, vec![0x80, 62, 0]),
    ]);
    let poly = track_body(&[
        (0, vec![0xA0, 60, 10]),
        (0, lyric_meta(b"one")),
        (960, lyric_meta(b"two")),
    ]);
    let off = track_body(&[
        (0, lyric_meta(b"uno")),
        (0, vec![0x83, 70, 0]),
        (960, lyric_meta(b"dos")),
    ]);
    let both = track_body(&[
        (0, lyric_meta(b"eins")),
        (480, vec![0xA5, 1, 1]),
        (480, vec![0x85, 1, 0]),
        (0, lyric_meta(b"zwei")),
        (100, lyric_meta(b"drei")),
    ]);
    let pure = track_body(&[(0, lyric_meta(b"ha")), (960, lyric_meta(b"na"))]);
    let input = SmfFile::read(&smf(1, 960, &[notes, poly, off, both, pure])).unwrap();
    let song = Song::from_smf(&input).unwrap();
    assert_eq!(song.tracks().len(), 1);
    assert_eq!(total_lyrics(&song), 9);
    for format in [SmfFormat::Parallel, SmfFormat::Single] {
        let output = song.to_smf(format).unwrap();
        assert_eq!(lyric_events(&output), lyric_events(&input), "{format:?}");
    }
}

/// 노트가 하나도 없는 Type 0(폴리 프레셔와 가사만)은 곡 트랙이 남지 않는다. `attach_lyrics`의
/// 문서대로 가사를 버린다. 오류 없이 읽히는지와 현재 동작을 기록한다(제안, S2-T7 이월 후보).
#[test]
fn type0_without_notes_reads_and_has_no_tracks() {
    let body = track_body(&[(0, vec![0xA0, 60, 10]), (0, lyric_meta(b"solo"))]);
    let song = song_of(&smf(0, 960, &[body]));
    assert!(song.tracks().is_empty());
    let output = song.to_smf(SmfFormat::Single).unwrap();
    assert!(lyric_events(&output).is_empty());
}
