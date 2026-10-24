//! S2-T7e 적대적 리뷰 1회차 시험(SMF 보고, S2-T7c 리뷰 2 이월). 깨뜨리지 못한 시도만 남긴다. 깨진 재현은
//! `docs/reports/s2-t7e/review-1.md` 부록에 있다.
//!
//! 겨냥: `changed_by_pairing`의 전제(틱 순 시작 ↔ 틱 순 끝)가 `split_track`의 FIFO와 어긋나는 입력,
//! `dropped_same_tick`·`yields_song_track`이 `Song::from_smf`의 실제 버림과 어긋나는 입력.

use std::collections::BTreeMap;

use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use keyscore_midi::{ExportReport, SmfFile, SmfFormat, smf_export, smf_import};
use proptest::prelude::*;

// ---------------------------------------------------------------- SMF 바이트

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

fn smf_bytes(format: u16, ppq: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
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

fn smf(format: u16, tracks: &[Vec<u8>]) -> SmfFile {
    SmfFile::read(&smf_bytes(format, 960, tracks)).unwrap()
}

fn tempo_bytes(micros: u32) -> Vec<u8> {
    let [_, a, b, c] = micros.to_be_bytes();
    vec![0xFF, 0x51, 0x03, a, b, c]
}

fn meter_bytes(nn: u8, dd: u8) -> Vec<u8> {
    vec![0xFF, 0x58, 0x04, nn, dd, 24, 8]
}

// ---------------------------------------------------------------- 모델

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

/// 트랙마다 (채널, 노트). 클립은 하나(길이 7680).
fn project_of(tracks: &[(u8, Vec<Note>)]) -> Project {
    let mut p = Project::new();
    for (i, (channel, notes)) in tracks.iter().enumerate() {
        let mut t = Track::new(
            format!("T{i}"),
            Channel::new(*channel).unwrap(),
            TrackColor::new(u8::try_from(i % 8).unwrap()).unwrap(),
        );
        let mut clip = Clip::new(Tick::ZERO, 7680).unwrap();
        for n in notes {
            clip.insert_note(n.clone()).unwrap();
        }
        t.insert_clip(clip).unwrap();
        p.push_track(t).unwrap();
    }
    p
}

/// 트랙 번호와 펼친 (채널, 음높이, 시작, 끝). 숨김 꼬리는 클립 끝에서 자른다(내보내기와 같다).
fn spans_of(p: &Project) -> Vec<(usize, u8, u8, u64, u64)> {
    let mut out = Vec::new();
    for (i, t) in p.tracks().iter().enumerate() {
        for (_, c) in t.clips() {
            for n in c.notes() {
                let start = c.start().get() + n.start().get();
                let end = c.start().get() + n.end().min(c.length());
                out.push((i, t.channel().get(), n.pitch().get(), start, end));
            }
        }
    }
    out
}

/// 내보내고 다시 읽은 프로젝트와 보고.
fn reread(p: &Project, format: SmfFormat) -> (Project, ExportReport) {
    let (file, ex) = smf_export(p, format).unwrap();
    let back = SmfFile::read(&file.write()).unwrap();
    let (q, _) = smf_import(&back).unwrap();
    (q, ex)
}

/// 다시 읽은 뒤 사라진(같은 묶음에 같은 (시작, 끝)이 없는) 원래 노트 수. 묶음은 Type 1이면 (트랙, 음높이),
/// Type 0이면 (채널, 음높이)다. 트랙마다 노트가 하나 이상이라 Type 1의 트랙 번호가 밀리지 않는다.
fn actually_changed(before: &Project, after: &Project, format: SmfFormat) -> usize {
    let key = |s: &(usize, u8, u8, u64, u64)| match format {
        SmfFormat::Parallel => (s.0, 0, s.2),
        SmfFormat::Single => (0, s.1, s.2),
    };
    let mut pool: BTreeMap<(usize, u8, u8), Vec<(u64, u64)>> = BTreeMap::new();
    for s in spans_of(after) {
        pool.entry(key(&s)).or_default().push((s.3, s.4));
    }
    let mut changed = 0;
    for s in spans_of(before) {
        let list = pool.entry(key(&s)).or_default();
        match list.iter().position(|x| *x == (s.3, s.4)) {
            Some(i) => {
                list.remove(i);
            }
            None => changed += 1,
        }
    }
    changed
}

// ---------------------------------------------------------------- 겹침 셈(제안 1)

/// 트랙 1–3개(채널 1·2, 음높이 60·62, 벨로시티 0 포함)에 같은 음 겹침을 아무렇게나 두고, 클립을 쪼개 숨김
/// 꼬리와 클립 경계를 만든다. 두 형식 모두 보고의 `overlapping_notes`가 실제로 길이가 바뀐 노트 수와 같다.
fn arb_notes() -> impl Strategy<Value = Vec<Note>> {
    proptest::collection::vec(
        (
            0u64..3600,
            1u32..2400,
            prop_oneof![Just(60u8), Just(62u8)],
            0u16..=0xFFFF,
        ),
        1..6,
    )
    .prop_map(|list| {
        list.into_iter()
            .map(|(s, l, p, v)| note(s, l, p, v))
            .collect()
    })
}

fn arb_project() -> impl Strategy<Value = Project> {
    (
        proptest::collection::vec((prop_oneof![Just(1u8), Just(2u8)], arb_notes()), 1..4),
        proptest::option::of(1u64..3600),
    )
        .prop_map(|(tracks, split)| {
            let mut p = project_of(&tracks);
            if let Some(at) = split {
                let ids: Vec<_> = p
                    .tracks()
                    .iter()
                    .map(|t| t.clips().next().unwrap().0)
                    .collect();
                for (i, id) in ids.into_iter().enumerate() {
                    let _ = p.track_mut(i).unwrap().split_clip(id, Tick::new(at));
                }
            }
            p
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn overlap_count_equals_notes_changed_by_rereading(p in arb_project()) {
        for format in [SmfFormat::Parallel, SmfFormat::Single] {
            let (q, ex) = reread(&p, format);
            let changed = actually_changed(&p, &q, format);
            prop_assert_eq!(
                ex.overlapping_notes,
                changed,
                "{:?}: 보고 {} ≠ 실제 {} ({:?})",
                format,
                ex.overlapping_notes,
                changed,
                spans_of(&p)
            );
        }
    }
}

/// 같은 틱에 같은 음 셋(길이 다름)은 FIFO가 틱 순 끝과 차례로 짝지어 그대로다. 벨로시티 0(1로 나간다)도 같다.
#[test]
fn same_tick_same_pitch_chord_is_unchanged() {
    let p = project_of(&[(
        1,
        vec![
            note(0, 480, 60, 0),
            note(0, 960, 60, v100()),
            note(0, 1920, 60, 1),
        ],
    )]);
    for format in [SmfFormat::Parallel, SmfFormat::Single] {
        let (q, ex) = reread(&p, format);
        assert_eq!(actually_changed(&p, &q, format), 0, "{format:?}");
        assert_eq!(ex.overlapping_notes, 0, "{format:?} {ex:?}");
    }
}

/// 클립 경계에서 잘린 숨김 꼬리가 다음 클립의 같은 음과 맞닿으면(끝 = 시작) 바뀌지 않는다. 다음 클립의 같은 음이
/// 꼬리 안에서 시작하면 꼬리를 자른 뒤 보므로 그것도 겹침이 아니다.
#[test]
fn hidden_tail_cut_before_pairing_is_not_counted() {
    let mut p = project_of(&[(
        1,
        vec![note(0, 3000, 60, v100()), note(1920, 480, 60, v100())],
    )]);
    let id = p.tracks()[0].clips().next().unwrap().0;
    p.track_mut(0)
        .unwrap()
        .split_clip(id, Tick::new(1920))
        .unwrap();
    assert_eq!(
        spans_of(&p),
        vec![(0, 1, 60, 0, 1920), (0, 1, 60, 1920, 2400)]
    );
    let (q, ex) = reread(&p, SmfFormat::Parallel);
    assert_eq!(actually_changed(&p, &q, SmfFormat::Parallel), 0);
    assert_eq!(ex.overlapping_notes, 0, "{ex:?}");
}

/// Type 1에서 같은 채널의 다른 트랙은 SMF 트랙이 달라 섞이지 않는다. 노트가 없는 트랙(이름만)이 앞에 있어도
/// 같다.
#[test]
fn type1_same_channel_tracks_stay_separate_even_after_empty_track() {
    let mut p = Project::new();
    p.push_track(Track::new(
        "empty",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    ))
    .unwrap();
    let rest = project_of(&[
        (1, vec![note(0, 1920, 60, v100())]),
        (1, vec![note(480, 480, 60, v100())]),
    ]);
    for t in rest.tracks() {
        p.push_track(t.clone()).unwrap();
    }
    let (_, ex) = reread(&p, SmfFormat::Parallel);
    assert_eq!(ex.overlapping_notes, 0, "{ex:?}");
    assert_eq!(ex.empty_tracks, 1, "{ex:?}");
    let (q, single) = reread(&p, SmfFormat::Single);
    assert_eq!(single.overlapping_notes, 2, "{single:?}");
    // Type 0은 빈 트랙이 사라지고 같은 채널이 한 트랙이 된다
    assert_eq!(q.tracks().len(), 1);
}

// ---------------------------------------------------------------- 같은 틱 메타(제안 2a)

/// 재배율 틱. `Rescale`과 같은 식(SEQ-6 반올림 정수 산술).
fn rescaled(ppq: u16, t: u64) -> u64 {
    let ppq = u128::from(ppq.max(1));
    u64::try_from((u128::from(t) * 960 + ppq / 2) / ppq).unwrap()
}

#[derive(Debug, Clone)]
enum MetaEvent {
    Tempo(u32),
    Meter(u8, u8),
}

fn arb_meta() -> impl Strategy<Value = MetaEvent> {
    prop_oneof![
        prop_oneof![
            Just(500_000u32),
            Just(480_000),
            Just(400_000),
            Just(0),
            Just(0x00FF_FFFF)
        ]
        .prop_map(MetaEvent::Tempo),
        (prop_oneof![Just(4u8), Just(3), Just(0)], 0u8..4)
            .prop_map(|(n, d)| MetaEvent::Meter(n, d)),
    ]
}

/// 트랙 1–3개에 템포·박자표를 아무 틱(델타 0–3)에 적는다. 잘못된 값(0 µs, 분자 0)도 섞는다. PPQ는 1–20000.
fn arb_meta_file() -> impl Strategy<Value = (u16, Vec<Vec<(u32, MetaEvent)>>)> {
    (
        1u16..20000,
        proptest::collection::vec(proptest::collection::vec((0u32..4, arb_meta()), 1..5), 1..4),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// 올바른 템포·박자표마다 재배율 틱의 실제 맵 값과 다르면 버려진 것이다. 잘못된 것은 그대로 센다. 그 합이
    /// `skipped_events`다.
    #[test]
    fn same_tick_meta_count_matches_actual_maps((ppq, tracks) in arb_meta_file()) {
        let mut encoded: Vec<Vec<u8>> = Vec::new();
        let mut invalid = 0usize;
        // (재배율 틱, 값), 트랙 순서·이벤트 순서대로
        let mut tempos: Vec<(u64, u32)> = Vec::new();
        let mut meters: Vec<(u64, (u8, u16))> = Vec::new();
        for events in &tracks {
            let mut bytes: Vec<(u32, Vec<u8>)> = Vec::new();
            let mut at = 0u64;
            for (delta, e) in events {
                at += u64::from(*delta);
                match e {
                    MetaEvent::Tempo(us) => {
                        if (1..=0x00FF_FFFF).contains(us) {
                            tempos.push((rescaled(ppq, at), *us));
                        } else {
                            invalid += 1;
                        }
                        bytes.push((*delta, tempo_bytes(*us)));
                    }
                    MetaEvent::Meter(nn, dd) => {
                        if *nn > 0 {
                            meters.push((rescaled(ppq, at), (*nn, 1u16 << dd)));
                        } else {
                            invalid += 1;
                        }
                        bytes.push((*delta, meter_bytes(*nn, *dd)));
                    }
                }
            }
            let refs: Vec<(u32, &[u8])> = bytes.iter().map(|(d, b)| (*d, b.as_slice())).collect();
            encoded.push(track(&refs));
        }
        encoded.push(track(&[(0, &[0x90, 60, 100]), (4, &[0x80, 60, 0])]));
        let file = SmfFile::read(&smf_bytes(1, ppq, &encoded)).unwrap();
        let (p, report) = smf_import(&file).unwrap();
        let tempo_at: BTreeMap<u64, u32> = p
            .tempo_map()
            .changes()
            .map(|(t, v)| (t.get(), v.micros_per_quarter()))
            .collect();
        let meter_at: BTreeMap<u64, (u8, u16)> = p
            .meter_map()
            .changes()
            .map(|(t, s)| (t.get(), (s.numerator(), s.denominator())))
            .collect();
        let dropped_tempos = tempos
            .iter()
            .filter(|(t, v)| tempo_at.get(t) != Some(v))
            .count();
        let dropped_meters = meters
            .iter()
            .filter(|(t, v)| meter_at.get(t) != Some(v))
            .count();
        prop_assert_eq!(
            report.skipped_events,
            invalid + dropped_tempos + dropped_meters,
            "ppq {} tempos {:?} meters {:?} map {:?}/{:?} report {:?}",
            ppq,
            tempos,
            meters,
            tempo_at,
            meter_at,
            report
        );
        prop_assert_eq!(report.is_lossless(), report.skipped_events == 0 && report.lane_points == 0);
    }
}

/// 잘못된 템포(0 µs)와 올바른 템포가 같은 틱에 있으면 잘못된 것만 한 번 센다(이중 셈 없음).
#[test]
fn invalid_tempo_at_same_tick_is_counted_once() {
    let file = smf(
        1,
        &[
            track(&[(0, &tempo_bytes(0)), (0, &tempo_bytes(500_000))]),
            track(&[(0, &[0x90, 60, 100]), (480, &[0x80, 60, 0])]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tempo_map().changes().count(), 1);
    assert_eq!(report.skipped_events, 1, "{report:?}");
}

/// 같은 틱의 박자표가 트랙을 건너 다르면 뒤 트랙 것이 남고 앞 것이 버려진다. 보고와 맵이 같은 쪽을 가리킨다.
#[test]
fn same_tick_meters_across_tracks_keep_last_track() {
    let file = smf(
        1,
        &[
            track(&[(0, &meter_bytes(3, 2))]),
            track(&[
                (0, &meter_bytes(4, 2)),
                (0, &[0x90, 60, 100]),
                (480, &[0x80, 60, 0]),
            ]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.meter_map().signature_at(Tick::ZERO).numerator(), 4);
    assert_eq!(report.skipped_events, 1, "{report:?}");
}

// ---------------------------------------------------------------- 사라지는 트랙의 이름(제안 2b)

/// 가사·이름·폴리 프레셔만 있는 Type 1 트랙: 곡 트랙이 남지 않아 이름은 버려지고(센다), 가사는 다른 트랙의
/// 노트에 붙는다(손실 아님). 폴리 프레셔 1 + 이름 1.
#[test]
fn name_of_lyric_and_poly_pressure_track_is_counted_lyric_is_not() {
    let file = smf(
        1,
        &[
            track(&[(0, &[0x90, 60, 100]), (480, &[0x80, 60, 0])]),
            track(&[
                (0, &[0xFF, 0x03, 0x01, b'P']),
                (0, &[0xFF, 0x05, 0x02, b'l', b'a']),
                (0, &[0xA0, 60, 10]),
            ]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tracks().len(), 1);
    assert_eq!(report.skipped_events, 2, "{report:?}");
    assert_eq!(report.orphan_lyrics, 0, "{report:?}");
    assert_eq!(report.unattached_lyrics, 0, "{report:?}");
}

/// 가사만 있는 이름 트랙(채널 메시지 없음)의 이름도 센다. 붙을 노트가 없으면 가사는 버린 것으로 센다.
#[test]
fn name_of_lyric_only_track_is_counted() {
    let file = smf(
        1,
        &[track(&[
            (0, &[0xFF, 0x03, 0x01, b'L']),
            (0, &[0xFF, 0x05, 0x02, b'l', b'a']),
        ])],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tracks().len(), 0);
    assert_eq!(report.skipped_events, 1, "{report:?}");
    assert_eq!(report.orphan_lyrics, 1, "{report:?}");
}

/// CC만, 채널 프레셔만, 벤드만 있는 트랙은 곡 트랙이 남아 이름이 유지된다(세지 않는다). 레인 포인트만 센다.
#[test]
fn names_of_lane_only_tracks_are_kept() {
    let file = smf(
        1,
        &[
            track(&[(0, &[0xFF, 0x03, 0x01, b'C']), (0, &[0xB0, 7, 100])]),
            track(&[(0, &[0xFF, 0x03, 0x01, b'D']), (0, &[0xD0, 50])]),
            track(&[(0, &[0xFF, 0x03, 0x01, b'E']), (0, &[0xE0, 0, 0x40])]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    let names: Vec<&str> = p.tracks().iter().map(keyscore_core::Track::name).collect();
    assert_eq!(names, vec!["C", "D", "E"]);
    assert_eq!(report.skipped_events, 0, "{report:?}");
    assert_eq!(report.lane_points, 3, "{report:?}");
}

/// Type 0의 이름은 곡 트랙이 있어도 버린다(제목). 둘째 이름도 센다.
#[test]
fn type0_names_are_counted_even_with_song_track() {
    let file = smf(
        0,
        &[track(&[
            (0, &[0xFF, 0x03, 0x01, b'T']),
            (0, &[0xFF, 0x03, 0x01, b'U']),
            (0, &[0x90, 60, 100]),
            (480, &[0x80, 60, 0]),
        ])],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tracks().len(), 1);
    assert_ne!(p.tracks()[0].name(), "T");
    assert_eq!(report.skipped_events, 2, "{report:?}");
}

/// 끝나지 않은 Note On만 있는 트랙은 곡 트랙이 남는다(트랙 끝에서 끝낸다). 이름을 세지 않는다.
#[test]
fn name_of_unterminated_note_track_is_kept() {
    let file = smf(
        1,
        &[track(&[
            (0, &[0xFF, 0x03, 0x01, b'N']),
            (0, &[0x90, 60, 100]),
            (480, &[0xB0, 7, 100]),
        ])],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tracks().len(), 1);
    assert_eq!(p.tracks()[0].name(), "N");
    assert_eq!(report.skipped_events, 0, "{report:?}");
    assert_eq!(report.notes, 1, "{report:?}");
}
