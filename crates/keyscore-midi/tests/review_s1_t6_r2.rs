//! S1-T6 적대적 리뷰 2의 깨뜨리기 테스트(REQ IO-2, SEQ-9, LYR-6).
//!
//! 리뷰 1 지적 1–4의 수정과 반영한 제안 5·7을 이웃 입력으로 다시 흔든다
//! (`docs/reports/s1-t6/review-2.md`).

use std::time::{Duration, Instant};

use keyscore_core::{
    Channel, Clip, MeterMap, Note, Pitch, Project, Tempo, TempoMap, Tick, TimeSignature, Track,
    TrackColor, Velocity,
};
use keyscore_midi::{MAX_SONG_TICKS, SmfError, SmfFile, SmfFormat, Song, SongNote, SongTrack};

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

fn within(started: Instant, limit: Duration, what: &str) {
    let took = started.elapsed();
    assert!(took < limit, "{what}: {took:?} (상한 {limit:?})");
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

fn lyric_meta(text: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, 0x05];
    out.extend(vlq(u32::try_from(text.len()).unwrap()));
    out.extend_from_slice(text);
    out
}

// ---------- A. 지적 1–4의 이웃 입력 ----------

/// 지적 1의 이웃: SMF 트랙 7개 × 채널 16에서 켜는 순서의 역순으로 끈다. 음높이가 128개를 돌아
/// 같은 키의 노트가 여러 개 열려 FIFO로 짝지어진다.
#[test]
fn reverse_note_off_order_across_channels_and_tracks_imports_in_bounded_time() {
    const M: usize = 7_500;
    let mut tracks = Vec::new();
    for _ in 0..7 {
        let mut events: Vec<(u32, Vec<u8>)> = Vec::with_capacity(32 * M);
        for i in 0..M {
            for c in 0u8..16 {
                let p = u8::try_from(i % 128).unwrap();
                events.push((u32::from(c == 0), vec![0x90 | c, p, 100]));
            }
        }
        for i in (0..M).rev() {
            for c in (0u8..16).rev() {
                let p = u8::try_from(i % 128).unwrap();
                events.push((u32::from(c == 15), vec![0x80 | c, p, 0]));
            }
        }
        tracks.push(track_body(&events));
    }
    let bytes = smf(1, 960, &tracks);
    let started = Instant::now();
    let song = song_of(&bytes);
    let project = song.to_project().unwrap();
    assert_eq!(song.tracks().len(), 7 * 16);
    assert!(song.tracks().iter().all(|t| t.notes().len() == M));
    assert!(
        song.tracks()
            .iter()
            .all(|t| t.notes().windows(2).all(|w| w[0] <= w[1]))
    );
    assert_eq!(project.tracks().len(), 7 * 16);
    within(started, Duration::from_secs(20), "역순 Note Off 84만 쌍");
}

/// 지적 1·3의 이웃: 한 틱에 노트 24만 개(같은 채널, 음높이는 내려가고 길이는 늘어난다). 곡 계층은
/// (시작, 길이, 음높이) 순으로 정렬하지만 core 클립은 (시작, 음높이, 길이) 순이라, `to_project`가
/// `Clip::insert_note`로 하나씩 넣으면 매번 앞쪽에 끼워 넣는다. 6만 개 2.1 s, 24만 개 35 s(디버그)로
/// 4배 입력에 17배 시간이다. 이벤트 상한(200만)이면 노트 90만 개다.
#[test]
fn huge_chord_at_one_tick_builds_clip_in_bounded_time() {
    const N: usize = 240_000;
    let pitch_of = |i: usize| u8::try_from(127 - i * 128 / N).unwrap();
    let mut events: Vec<(u32, Vec<u8>)> = Vec::with_capacity(2 * N);
    for i in 0..N {
        events.push((0, vec![0x90, pitch_of(i), 100]));
    }
    // 같은 음높이는 켠 순서대로(FIFO) 끈다. 노트 i의 길이는 i + 1틱이다.
    for i in 0..N {
        events.push((1, vec![0x80, pitch_of(i), 0]));
    }
    let bytes = smf(0, 960, &[track_body(&events)]);
    let song = song_of(&bytes);
    assert_eq!(song.tracks()[0].notes().len(), N);
    let started = Instant::now();
    let project = song.to_project().unwrap();
    let clips: Vec<_> = project.tracks()[0].clips().collect();
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].1.notes().len(), N);
    within(
        started,
        Duration::from_secs(20),
        "한 틱 노트 24만 개 클립 만들기",
    );
}

/// 지적 2의 이웃: 곡 트랙이 많고(1021 × 16 CC 트랙) 한 틱에 노트가 많은 트랙이 있으며, 가사만 든
/// 트랙이 둘(1절·2절)이다. 둘째 가사 트랙은 첫째와 같은 틱에 붙으므로 `add_lyric`이 매번 가운데에
/// 끼워 넣는다. 15만 개 7.3 s, 30만 개 28 s(디버그)로 2배 입력에 3.8배 시간이다.
#[test]
fn two_lyric_only_verses_with_many_tracks_attach_in_bounded_time() {
    const L: usize = 300_000;
    let mut tracks = Vec::new();
    for _ in 0..1021 {
        let events: Vec<(u32, Vec<u8>)> = (0u8..16).map(|c| (0, vec![0xB0 | c, 7, 100])).collect();
        tracks.push(track_body(&events));
    }
    // 노트 트랙: 틱 0에 5000음 화음, 그 뒤 가사 틱마다 노트 하나.
    let mut notes: Vec<(u32, Vec<u8>)> = Vec::new();
    for i in 0..5_000 {
        notes.push((0, vec![0x90, u8::try_from(i % 128).unwrap(), 100]));
    }
    for i in 0..5_000 {
        notes.push((0, vec![0x80, u8::try_from(i % 128).unwrap(), 0]));
    }
    for _ in 0..L {
        notes.push((1, vec![0x90, 60, 100]));
        notes.push((0, vec![0x80, 60, 0]));
    }
    tracks.push(track_body(&notes));
    for _ in 0..2 {
        let mut lyrics: Vec<(u32, Vec<u8>)> = vec![(0, lyric_meta(b"x"))];
        lyrics.extend((0..L - 1).map(|_| (1, lyric_meta(b"a"))));
        tracks.push(track_body(&lyrics));
    }
    let bytes = smf(1, 960, &tracks);
    let started = Instant::now();
    let song = song_of(&bytes);
    let with_lyrics: Vec<_> = song
        .tracks()
        .iter()
        .filter(|t| !t.lyrics().is_empty())
        .collect();
    assert_eq!(with_lyrics.len(), 1);
    assert_eq!(with_lyrics[0].lyrics().len(), 2 * L);
    within(
        started,
        Duration::from_secs(20),
        "가사 트랙 둘 × 30만 개 붙이기",
    );
}

/// 지적 3의 이웃: 마디마다 박자가 3/4·4/4로 바뀌고 두 마디마다 노트가 있어 클립이 많다.
#[test]
fn sparse_clips_with_meter_change_every_bar_build_in_bounded_time() {
    const N: usize = 60_000;
    let mut conductor: Vec<(u32, Vec<u8>)> = Vec::with_capacity(2 * N);
    let mut notes: Vec<(u32, Vec<u8>)> = Vec::with_capacity(2 * N);
    let mut tick = 0u32;
    let mut last_note = 0u32;
    for bar in 0..2 * N {
        let three = bar % 2 == 0;
        let delta = match (bar, three) {
            (0, _) => 0,
            (_, true) => 3840,
            (_, false) => 2880,
        };
        tick += delta;
        conductor.push((
            delta,
            vec![0xFF, 0x58, 0x04, if three { 3 } else { 4 }, 2, 24, 8],
        ));
        if three {
            notes.push((tick - last_note, vec![0x90, 60, 100]));
            notes.push((1, vec![0x80, 60, 0]));
            last_note = tick + 1;
        }
    }
    let bytes = smf(1, 960, &[track_body(&conductor), track_body(&notes)]);
    let song = song_of(&bytes);
    let started = Instant::now();
    let project = song.to_project().unwrap();
    let clips: Vec<_> = project.tracks()[0].clips().collect();
    assert_eq!(clips.len(), N);
    // 클립은 3/4 마디 하나(2880틱)다.
    assert!(clips.iter().all(|(_, c)| c.length() == 2880));
    within(
        started,
        Duration::from_secs(20),
        "박자 12만 번 바뀌는 곡의 클립 6만 개",
    );
}

/// 지적 4의 경계(읽기): 끝 틱이 정확히 `MAX_SONG_TICKS`면 받고, 하나 넘으면 `TooLong`이다. PPQ 1은
/// 960배 뒤 경계로 본다.
#[test]
fn read_accepts_song_ending_exactly_at_limit_and_rejects_one_past() {
    let gap = || vec![0xFF, 0x01, 0x00];
    // PPQ 960: 델타 (2^28 - 1) 16번 + 15 = 2^32 - 1.
    let mut events: Vec<(u32, Vec<u8>)> = (0..16).map(|_| (0x0FFF_FFFF, gap())).collect();
    events.push((15, vec![0x90, 60, 100]));
    let at_limit = smf(0, 960, &[track_body(&events)]);
    let song = song_of(&at_limit);
    // 끝나지 않은 노트는 트랙 끝(= 상한)에서 1틱으로 끝난다.
    let n = song.tracks()[0].notes()[0];
    assert_eq!(n.start().get(), MAX_SONG_TICKS);
    // 그 곡은 다시 쓸 수 없다(노트 끝 = 상한 + 1). 조용히 자르지 않는다.
    assert!(matches!(
        song.to_smf(SmfFormat::Parallel),
        Err(SmfError::TooLong { .. })
    ));
    events.push((1, vec![0x80, 60, 0]));
    let past = smf(0, 960, &[track_body(&events)]);
    let file = SmfFile::read(&past).unwrap();
    assert!(matches!(
        Song::from_smf(&file),
        Err(SmfError::TooLong { tick }) if tick == MAX_SONG_TICKS + 1
    ));
    // PPQ 1: 960 × 4473924 ≤ 2^32 - 1 < 960 × 4473925.
    let ppq1 = |end: u32| {
        let ev = vec![(0, vec![0x90, 60, 100]), (end, vec![0x80, 60, 0])];
        Song::from_smf(&SmfFile::read(&smf(0, 1, &[track_body(&ev)])).unwrap())
    };
    assert_eq!(
        ppq1(4_473_924).unwrap().tracks()[0].notes()[0].length(),
        4_473_924 * 960
    );
    assert!(matches!(ppq1(4_473_925), Err(SmfError::TooLong { .. })));
}

/// 지적 4의 경계(쓰기): 노트 끝(시작 + 길이), 템포·박자 변화가 상한을 넘는지 모두 본다. 상한에서
/// 정확히 끝나는 곡은 쓰고 다시 읽으면 같다.
#[test]
fn to_smf_checks_note_end_and_conductor_against_limit() {
    let note_song = |start: u64, length: u32| {
        let mut song = Song::new(TempoMap::default(), MeterMap::default());
        let mut t = SongTrack::new(None, ch(1));
        t.add_note(SongNote::new(Tick::new(start), length, pitch(60), vel(100)));
        song.push_track(t);
        song
    };
    // 시작 0, 길이 u32::MAX: 끝이 정확히 상한.
    let edge = note_song(0, u32::MAX);
    for format in [SmfFormat::Parallel, SmfFormat::Single] {
        let bytes = edge.to_smf(format).unwrap().write();
        // 채움 메타는 트랙당 16개 이하라 출력이 작다.
        assert!(bytes.len() < 300, "{format:?}: {} B", bytes.len());
        assert_eq!(
            song_of(&bytes).tracks()[0].notes(),
            edge.tracks()[0].notes()
        );
    }
    // 시작은 상한 안이지만 끝이 넘는다.
    for (start, length) in [(1, u32::MAX), (MAX_SONG_TICKS - 5, 6), (MAX_SONG_TICKS, 1)] {
        assert!(
            matches!(
                note_song(start, length).to_smf(SmfFormat::Parallel),
                Err(SmfError::TooLong { .. })
            ),
            "{start} + {length}"
        );
    }
    // u64 끝 가까운 시작도 넘침 없이 `Err`.
    assert!(matches!(
        note_song(u64::MAX - 1, u32::MAX).to_smf(SmfFormat::Single),
        Err(SmfError::TooLong { .. })
    ));
    // 템포·박자 변화만 상한 뒤에 있어도 `Err`, 상한에 정확히 있으면 쓴다.
    let tempo = Tempo::from_micros_per_quarter(400_000).unwrap();
    let sig = TimeSignature::new(3, 4).unwrap();
    for (at, ok) in [(MAX_SONG_TICKS, true), (MAX_SONG_TICKS + 1, false)] {
        let t = Song::new(
            TempoMap::from_changes(Tempo::DEFAULT, [(Tick::new(at), tempo)]),
            MeterMap::default(),
        );
        assert_eq!(t.to_smf(SmfFormat::Parallel).is_ok(), ok, "템포 {at}");
        let m = Song::new(
            TempoMap::default(),
            MeterMap::from_changes(TimeSignature::COMMON, [(Tick::new(at), sig)]),
        );
        assert_eq!(m.to_smf(SmfFormat::Single).is_ok(), ok, "박자 {at}");
    }
}

// ---------- C. 반영한 제안 5·7 ----------

fn chord_project(notes: &[(u8, u32, Option<&str>)]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new("Choir", ch(1), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for &(p, len, lyric) in notes {
        let mut n = Note::new(Tick::ZERO, len, pitch(p), vel(100)).unwrap();
        if let Some(l) = lyric {
            n = n.with_lyric(l);
        }
        clip.insert_note(n).unwrap();
    }
    track.insert_clip(clip).unwrap();
    project.push_track(track).unwrap();
    project
}

fn lyrics_of(project: &Project) -> Vec<(u8, u32, Option<String>)> {
    let mut out: Vec<_> = project.tracks()[0]
        .clips()
        .flat_map(|(_, c)| c.notes().iter())
        .map(|n| (n.pitch().get(), n.length(), n.lyric().map(str::to_string)))
        .collect();
    out.sort();
    out
}

fn round_trip(project: &Project, format: SmfFormat) -> Project {
    let bytes = Song::from_project(project).to_smf(format).unwrap().write();
    song_of(&bytes).to_project().unwrap()
}

/// 제안 5: 화음의 가사가 모델 → SMF(Type 0·1) → 모델에서 같은 노트로 돌아온다. 여러 모양을 본다.
#[test]
fn chord_lyrics_shapes_round_trip_in_both_formats() {
    let shapes: [&[(u8, u32, Option<&str>)]; 4] = [
        &[
            (60, 960, Some("c")),
            (64, 960, Some("e")),
            (67, 960, Some("g")),
        ],
        &[
            (48, 960, Some("lo")),
            (60, 960, None),
            (64, 960, None),
            (72, 960, None),
        ],
        &[(48, 960, None), (60, 960, Some("mid")), (72, 960, None)],
        &[(60, 960, Some("only"))],
    ];
    for shape in shapes {
        let project = chord_project(shape);
        for format in [SmfFormat::Parallel, SmfFormat::Single] {
            assert_eq!(
                lyrics_of(&round_trip(&project, format)),
                lyrics_of(&project),
                "{shape:?} {format:?}"
            );
        }
    }
}

/// 제안 5의 이웃: 같은 틱에 같은 음높이 노트가 둘(길이만 다름)이면 `from_project`는 자리 채움 빈
/// 가사를 노트마다 내지만, `bar_groups`는 음높이를 중복 제거해 짝짓는다. 아래 음의 가사가 밀려
/// 사라진다.
#[test]
fn chord_lyrics_with_duplicate_pitch_round_trip() {
    let project = chord_project(&[(67, 960, None), (67, 480, None), (60, 960, Some("lo"))]);
    assert_eq!(
        lyrics_of(&round_trip(&project, SmfFormat::Parallel)),
        lyrics_of(&project)
    );
}

/// 제안 7: Type 1은 곡 트랙 65534개(지휘 트랙 포함 65535)까지 쓰고, 그보다 많으면 `TooManyTracks`다.
/// Type 0은 한 트랙으로 합치므로 트랙 수와 상관없다.
#[test]
fn to_smf_track_count_limit_is_exact() {
    let mut song = Song::new(TempoMap::default(), MeterMap::default());
    for _ in 0..65_534 {
        song.push_track(SongTrack::new(None, ch(1)));
    }
    let file = song.to_smf(SmfFormat::Parallel).unwrap();
    assert_eq!(file.tracks().len(), 65_535);
    song.push_track(SongTrack::new(None, ch(1)));
    assert!(matches!(
        song.to_smf(SmfFormat::Parallel),
        Err(SmfError::TooManyTracks { .. })
    ));
    assert_eq!(song.to_smf(SmfFormat::Single).unwrap().tracks().len(), 1);
}
