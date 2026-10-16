//! S1-T6 적대적 리뷰 1의 깨뜨리기 테스트(REQ IO-2, SEQ-6, SEQ-9, CTL-10, LYR-6).
//!
//! 통과하는 테스트는 회귀 테스트로 남고, 실패하는 테스트는 리뷰 지적 사항을 보인다
//! (`docs/reports/s1-t6/review-1.md`).

use std::time::{Duration, Instant};

use keyscore_core::{
    CcPoint, MeterMap, Pitch, Tempo, TempoMap, Tick, TimeSignature, Track, Velocity,
};
use keyscore_midi::{SmfError, SmfFile, SmfFormat, Song, SongNote, SongTrack};
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

fn chunk(id: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend(u32::try_from(body.len()).unwrap().to_be_bytes());
    out.extend_from_slice(body);
    out
}

/// (델타, 바이트) 열로 트랙 본문을 만든다. End of Track을 붙인다.
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

// ---------- 1. 이차 시간 ----------

/// Note Off 순서가 시작 순서와 어긋나면 `SongTrack::add_note`의 정렬 삽입(`Vec::insert`)이 매번
/// 뒤쪽 노트를 민다. 45만 쌍(이벤트 180만 개, 상한 200만 안)이면 원소 이동이 약 10^11번이다.
/// 15만 쌍 3.5 s, 45만 쌍 31 s(디버그)로 3배 입력에 8.6배 시간이다.
#[test]
fn interleaved_note_off_order_imports_in_bounded_time() {
    const M: usize = 450_000;
    let mut events: Vec<(u32, Vec<u8>)> = Vec::with_capacity(4 * M);
    for _ in 0..M {
        events.push((1, vec![0x90, 60, 100]));
        events.push((0, vec![0x90, 61, 100]));
    }
    // 61을 먼저 모두 끄고(길이 M), 60을 나중에 끈다(길이 2M). 60의 노트는 정렬상 61 노트들 사이에
    // 끼어 들어간다.
    for _ in 0..M {
        events.push((1, vec![0x80, 61, 0]));
    }
    for _ in 0..M {
        events.push((1, vec![0x80, 60, 0]));
    }
    let bytes = smf(0, 960, &[track_body(&events)]);
    let started = Instant::now();
    let song = song_of(&bytes);
    assert_eq!(song.tracks()[0].notes().len(), 2 * M);
    within(started, Duration::from_secs(20), "노트 45만 쌍 가져오기");
}

/// 가사만 든 트랙의 가사 하나마다 모든 곡 트랙을 훑는다(`attach_lyrics`). SMF 트랙 1024개 × 채널
/// 16 = 곡 트랙 16384개와 가사 20만 개면 `has_note_at` 호출이 약 3×10^9번이다.
#[test]
fn orphan_lyrics_with_many_song_tracks_attach_in_bounded_time() {
    let mut tracks = Vec::new();
    for _ in 0..1023 {
        let events: Vec<(u32, Vec<u8>)> = (0u8..16).map(|c| (0, vec![0xB0 | c, 7, 100])).collect();
        tracks.push(track_body(&events));
    }
    let lyrics: Vec<(u32, Vec<u8>)> = (0..200_000)
        .map(|_| (1, vec![0xFF, 0x05, 0x01, b'a']))
        .collect();
    tracks.push(track_body(&lyrics));
    let bytes = smf(1, 960, &tracks);
    let started = Instant::now();
    let song = song_of(&bytes);
    assert_eq!(song.tracks().len(), 1023 * 16);
    within(started, Duration::from_secs(20), "고아 가사 20만 개 붙이기");
}

/// 한 마디 걸러 노트가 있으면 노트마다 클립이 하나 생긴다. `Track::insert_clip`의 겹침 검사가
/// 기존 클립을 모두 훑으므로 `to_project`가 클립 수의 제곱이다.
#[test]
fn sparse_bar_notes_build_many_clips_in_bounded_time() {
    const N: usize = 70_000;
    let mut events: Vec<(u32, Vec<u8>)> = Vec::with_capacity(2 * N);
    for i in 0..N {
        // 4/4 960 PPQ: 두 마디(7680틱)마다 1틱 노트.
        events.push((if i == 0 { 0 } else { 7679 }, vec![0x90, 60, 100]));
        events.push((1, vec![0x80, 60, 0]));
    }
    let bytes = smf(0, 960, &[track_body(&events)]);
    let song = song_of(&bytes);
    let started = Instant::now();
    let project = song.to_project().unwrap();
    assert_eq!(project.tracks()[0].clips().count(), N);
    within(started, Duration::from_secs(20), "클립 7만 개 만들기");
}

/// running status만으로 이벤트 상한 가까이 채워도 선형으로 읽는다(회귀).
#[test]
fn running_status_flood_reads_in_linear_time() {
    let mut body = vec![0x00, 0x90, 60, 100];
    for _ in 0..1_000_000 {
        body.extend([0x00, 60, 0]);
    }
    body.extend([0x00, 0xFF, 0x2F, 0x00]);
    let bytes = smf(0, 960, &[body]);
    let started = Instant::now();
    let file = SmfFile::read(&bytes).unwrap();
    let song = Song::from_smf(&file).unwrap();
    assert_eq!(file.tracks()[0].events().len(), 1_000_001);
    assert_eq!(song.tracks()[0].notes().len(), 1);
    within(started, Duration::from_secs(20), "running status 100만 개");
}

// ---------- 2. 출력 폭증 ----------

/// PPQ 1에서 최대 델타(2^28 - 1)는 960배 틱이 되고, 쓰기는 28비트마다 빈 텍스트 메타를 하나씩 넣는다.
/// 입력 바이트당 약 960배의 출력이 나온다. 이벤트 상한(200만)까지 채운 14 MB 파일은 13 GB를 쓴다.
/// 공개 API로 `Tick` 이 u64 끝 가까이인 노트를 넣으면 채움 메타가 2^36개(약 480 GB)다.
#[test]
fn low_ppq_huge_delta_does_not_amplify_written_size() {
    let mut events: Vec<(u32, Vec<u8>)> = (0..2000)
        .map(|_| (0x0FFF_FFFF, vec![0xFF, 0x01, 0x00]))
        .collect();
    events.push((0, vec![0x90, 60, 100]));
    events.push((1, vec![0x80, 60, 0]));
    let input = smf(0, 1, &[track_body(&events)]);
    // 곡 길이 상한(`MAX_SONG_TICKS`)을 넘는 곡은 읽기에서 `TooLong`이다(구현자 수정: 이 리뷰의 제안대로
    // 상한을 두었다). 상한 안이면 쓰기가 불어나지 않아야 한다.
    let file = keyscore_midi::SmfFile::read(&input).unwrap();
    let song = match keyscore_midi::Song::from_smf(&file) {
        Err(keyscore_midi::SmfError::TooLong { .. }) => return,
        other => other.unwrap(),
    };
    let written = song.to_smf(SmfFormat::Parallel).unwrap().write();
    assert!(
        written.len() <= 100 * input.len(),
        "입력 {} B → 출력 {} B",
        input.len(),
        written.len()
    );
}

// ---------- 3. 해상도 확장 ----------

/// MIDI 2.0 명세의 min-center-max 확장을 그대로 옮긴 기준 구현.
fn reference_upscale(src: u32, src_bits: u32) -> u32 {
    let scale_bits = 32 - src_bits;
    let mut shifted = src << scale_bits;
    let center = 1u32 << (src_bits - 1);
    if src <= center {
        return shifted;
    }
    let repeat_bits = src_bits - 1;
    let mut repeat = src & ((1u32 << repeat_bits) - 1);
    if scale_bits > repeat_bits {
        repeat <<= scale_bits - repeat_bits;
    } else {
        repeat >>= repeat_bits - scale_bits;
    }
    while repeat != 0 {
        shifted |= repeat;
        repeat >>= repeat_bits;
    }
    shifted
}

#[test]
fn cc_pressure_and_bend_upscale_match_midi2_reference_for_every_value() {
    let mut events: Vec<(u32, Vec<u8>)> = Vec::new();
    for v in 0u8..128 {
        events.push((1, vec![0xB0, 1, v]));
        events.push((0, vec![0xD0, v]));
    }
    for v in 0u16..16384 {
        let [lsb, msb] = [
            u8::try_from(v & 0x7F).unwrap(),
            u8::try_from(v >> 7).unwrap(),
        ];
        events.push((1, vec![0xE0, lsb, msb]));
    }
    let song = song_of(&smf(0, 960, &[track_body(&events)]));
    let t = &song.tracks()[0];
    let cc: Vec<u32> = t.cc()[&1].iter().map(CcPoint::value).collect();
    let pressure: Vec<u32> = t.pressure().iter().map(CcPoint::value).collect();
    let bend: Vec<u32> = t.bend().iter().map(CcPoint::value).collect();
    let want7: Vec<u32> = (0..128).map(|v| reference_upscale(v, 7)).collect();
    let want14: Vec<u32> = (0..16384).map(|v| reference_upscale(v, 14)).collect();
    assert_eq!(cc, want7);
    assert_eq!(pressure, want7);
    assert_eq!(bend, want14);
    assert_eq!(bend[0x2000], 0x8000_0000);
    assert_eq!(bend[0x3FFF], 0xFFFF_FFFF);
    // 다시 쓰면 7·14비트 원래 값이다.
    let again = song_of(&song.to_smf(SmfFormat::Single).unwrap().write());
    assert_eq!(again, song);
}

// ---------- 4. 마디 클립 ----------

fn song_with(meter: MeterMap, notes: &[(u64, u32)]) -> Song {
    let mut song = Song::new(TempoMap::new(Tempo::DEFAULT), meter);
    let mut t = SongTrack::new(None, keyscore_core::Channel::new(1).unwrap());
    for &(start, len) in notes {
        t.add_note(SongNote::new(
            Tick::new(start),
            len,
            Pitch::new(60).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        ));
    }
    song.push_track(t);
    song
}

fn clip_spans(song: &Song) -> Vec<(u64, u64, usize)> {
    let project = song.to_project().unwrap();
    project.tracks()[0]
        .clips()
        .map(|(_, c)| (c.start().get(), c.length(), c.notes().len()))
        .collect()
}

#[test]
fn short_bar_from_mid_bar_meter_change_groups_clips_on_bar_lines() {
    // 4/4에서 1920틱(마디 중간)에 3/4로 바뀌면 마디 0은 [0, 1920)짜리 짧은 마디다. 마디 1은
    // [1920, 4800), 마디 2는 [4800, 7680), 마디 3은 [7680, 10560).
    let meter = MeterMap::from_changes(
        TimeSignature::COMMON,
        [(Tick::new(1920), TimeSignature::new(3, 4).unwrap())],
    );
    // 마디 0과 1에 걸친 노트, 마디 3의 노트(마디 2는 비어 있음).
    let song = song_with(meter, &[(1800, 200), (7680, 10)]);
    assert_eq!(clip_spans(&song), vec![(0, 4800, 1), (7680, 2880, 1)]);
}

#[test]
fn note_ending_exactly_on_bar_line_does_not_join_next_bar() {
    // 마디 0을 꽉 채운 노트와 마디 1의 노트는 인접 마디라 한 클립, 마디 3의 노트는 따로다.
    let song = song_with(MeterMap::default(), &[(0, 3840), (11520, 3840)]);
    assert_eq!(clip_spans(&song), vec![(0, 3840, 1), (11520, 3840, 1)]);
    let joined = song_with(MeterMap::default(), &[(0, 3840), (3840, 1)]);
    assert_eq!(clip_spans(&joined), vec![(0, 7680, 2)]);
}

proptest! {
    /// 아무 노트·박자 변화로 만든 곡도 `to_project`가 패닉하지 않고, 성공하면 모델 불변식(클립 겹침
    /// 없음, 노트가 클립 안)과 노트 수가 지켜진다.
    #[test]
    fn random_song_to_project_keeps_invariants(
        notes in proptest::collection::vec((0u64..2_000_000, 1u32..50_000), 0..80),
        changes in proptest::collection::vec((0u64..2_000_000, 1u8..13, 0u32..9), 0..6),
    ) {
        let meter = MeterMap::from_changes(
            TimeSignature::COMMON,
            changes
                .iter()
                .map(|&(t, n, d)| (Tick::new(t), TimeSignature::new(n, 1 << d).unwrap())),
        );
        let song = song_with(meter, &notes);
        let project = song.to_project().unwrap();
        let track: &Track = &project.tracks()[0];
        let mut prev_end = 0u64;
        let mut count = 0usize;
        for (_, c) in track.clips() {
            prop_assert!(c.start().get() >= prev_end);
            prev_end = c.end();
            for n in c.notes() {
                prop_assert!(n.end() <= c.length());
                count += 1;
            }
        }
        prop_assert_eq!(count, notes.len());
        // 모델 → 곡이 같은 노트 집합이다(꼬리 없음).
        let back = Song::from_project(&project);
        prop_assert_eq!(back.tracks()[0].notes(), song.tracks()[0].notes());
    }
}

// ---------- 5. 악의적 헤더·청크 ----------

#[test]
fn malicious_header_and_chunk_variants_return_err() {
    let ok_track = track_body(&[(0, vec![0x90, 60, 1])]);
    // 헤더 길이 거짓(4 GB).
    let mut huge_header = smf(1, 960, std::slice::from_ref(&ok_track));
    huge_header[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(
        SmfFile::read(&huge_header),
        Err(SmfError::Truncated { .. })
    ));
    // 선언한 트랙보다 실제 청크가 적다.
    let mut fewer = smf(1, 960, std::slice::from_ref(&ok_track));
    fewer[11] = 3;
    assert!(matches!(
        SmfFile::read(&fewer),
        Err(SmfError::Truncated { .. })
    ));
    // 모르는 청크가 수만 개 끼어 있어도 선형으로 건너뛴다.
    let mut junk = smf(1, 960, &[]);
    junk[11] = 1;
    for _ in 0..50_000 {
        junk.extend(chunk(*b"XFIH", &[]));
    }
    junk.extend(chunk(*b"MTrk", &ok_track));
    assert_eq!(SmfFile::read(&junk).unwrap().tracks().len(), 1);
    // 상태 바이트 없이 데이터 바이트로 시작.
    let no_status = smf(0, 960, &[vec![0x00, 60, 100, 0x00, 0xFF, 0x2F, 0x00]]);
    assert!(matches!(
        SmfFile::read(&no_status),
        Err(SmfError::MissingStatus { .. })
    ));
    // 시스템 공통 상태(0xF4)와 채널 메시지 안의 8비트 데이터.
    let bad_status = smf(0, 960, &[vec![0x00, 0xF4, 0x00, 0xFF, 0x2F, 0x00]]);
    assert!(matches!(
        SmfFile::read(&bad_status),
        Err(SmfError::BadStatus { status: 0xF4, .. })
    ));
    let bad_data = smf(0, 960, &[vec![0x00, 0x90, 0x80, 1, 0x00, 0xFF, 0x2F, 0x00]]);
    assert!(matches!(
        SmfFile::read(&bad_data),
        Err(SmfError::BadData { .. })
    ));
    // SysEx 길이 거짓(최대 가변 길이 수).
    let sysex = smf(0, 960, &[vec![0x00, 0xF0, 0xFF, 0xFF, 0xFF, 0x7F, 0xF7]]);
    assert!(matches!(
        SmfFile::read(&sysex),
        Err(SmfError::Truncated { .. })
    ));
}

/// 결정적 쓰기: SMF → 곡 → SMF 바이트가 두 번째부터 고정점이다.
#[test]
fn smf_song_smf_reaches_byte_fixed_point() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/smf");
    for name in ["aloha-oe.mid", "terzetto-012.mid", "gm-128-sounds-demo.mid"] {
        let first = song_of(&std::fs::read(dir.join(name)).unwrap());
        for format in [SmfFormat::Parallel, SmfFormat::Single] {
            let a = first.to_smf(format).unwrap().write();
            let b = song_of(&a).to_smf(format).unwrap().write();
            let c = song_of(&b).to_smf(format).unwrap().write();
            assert_eq!(b, c, "{name} {format:?}");
        }
    }
}
