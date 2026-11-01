//! S3-T7 검색·노트 정보의 순수 계산. 규칙은 결정 `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md`
//! 2·3·6절.

use keyscore_app::note_info::info_lines;
use keyscore_app::search::{Direction, Found, Hit, Position, Query, QueryError, find_hits, step};
use keyscore_core::{
    Channel, Clip, ClipId, Note, Pitch, PitchOffset, Project, Tick, Track, TrackColor, TrackId,
    Velocity,
};
use proptest::prelude::*;

const BAR: u64 = 3840;

fn p(v: u8) -> Pitch {
    Pitch::new(v).unwrap()
}

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        p(pitch),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 트랙 하나: (이름, 채널, [(클립 시작, 클립 길이, 노트들)]).
type ClipSpec = (u64, u64, Vec<Note>);

fn track(name: &str, channel: u8, clips: Vec<ClipSpec>) -> (Track, Vec<ClipId>) {
    let mut t = Track::new(
        name,
        Channel::new(channel).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut ids = Vec::new();
    for (start, len, notes) in clips {
        let mut clip = Clip::new(Tick::new(start), len).unwrap();
        for n in notes {
            clip.insert_note(n).unwrap();
        }
        ids.push(t.insert_clip(clip).unwrap());
    }
    (t, ids)
}

fn project(tracks: Vec<Track>) -> (Project, Vec<TrackId>) {
    let mut project = Project::new();
    let ids = tracks.iter().map(Track::id).collect();
    for t in tracks {
        project.push_track(t).unwrap();
    }
    (project, ids)
}

fn hit(track: TrackId, tick: u64, pitch: u8) -> Hit {
    Hit {
        track,
        tick: Tick::new(tick),
        pitch: p(pitch),
        has_note: true,
    }
}

// ---------- 검색어 ----------

#[test]
fn parse_pitch_forms_cover_accidentals_and_octaves() {
    assert_eq!(Query::parse("c#4"), Ok(Query::Pitch(p(61))));
    assert_eq!(Query::parse("C4"), Ok(Query::Pitch(p(60))));
    assert_eq!(Query::parse("Eb3"), Ok(Query::Pitch(p(51))));
    assert_eq!(Query::parse("b-1"), Ok(Query::Pitch(p(11))));
    assert_eq!(Query::parse("c-1"), Ok(Query::Pitch(p(0))));
    assert_eq!(Query::parse("g9"), Ok(Query::Pitch(p(127))));
    // 변화표가 옥타브 경계를 넘는다: Cb4 = B3
    assert_eq!(Query::parse("cb4"), Ok(Query::Pitch(p(59))));
    assert_eq!(Query::parse("B#3"), Ok(Query::Pitch(p(60))));
    assert_eq!(Query::parse("  a4 "), Ok(Query::Pitch(p(69))));
}

#[test]
fn parse_pitch_without_octave_is_pitch_class() {
    assert_eq!(Query::parse("a"), Ok(Query::PitchClass(9)));
    assert_eq!(Query::parse("C#"), Ok(Query::PitchClass(1)));
    assert_eq!(Query::parse("cb"), Ok(Query::PitchClass(11)));
    assert_eq!(Query::parse("b#"), Ok(Query::PitchClass(0)));
    assert_eq!(Query::parse("eb"), Ok(Query::PitchClass(3)));
}

#[test]
fn parse_out_of_range_and_empty_and_quoted() {
    assert_eq!(Query::parse("g#9"), Err(QueryError::PitchOutOfRange));
    assert_eq!(Query::parse("b9"), Err(QueryError::PitchOutOfRange));
    assert_eq!(Query::parse("cb-1"), Err(QueryError::PitchOutOfRange));
    assert_eq!(Query::parse(""), Err(QueryError::Empty));
    assert_eq!(Query::parse("   "), Err(QueryError::Empty));
    assert_eq!(Query::parse("\"\""), Err(QueryError::Empty));
    assert_eq!(Query::parse("\"a\""), Ok(Query::Text("a".into())));
    assert_eq!(Query::parse("\" C4 \""), Ok(Query::Text(" c4 ".into())));
    assert_eq!(Query::parse("Bass"), Ok(Query::Text("bass".into())));
    assert_eq!(Query::parse("c10"), Ok(Query::Text("c10".into())));
    assert_eq!(Query::parse("c##4"), Ok(Query::Text("c##4".into())));
    assert_eq!(Query::parse("h4"), Ok(Query::Text("h4".into())));
    assert_eq!(Query::parse("사랑"), Ok(Query::Text("사랑".into())));
    // 한쪽 따옴표만이면 그대로 글자다
    assert_eq!(Query::parse("\"a"), Ok(Query::Text("\"a".into())));
}

// ---------- 결과 ----------

#[test]
fn find_hits_pitch_matches_exact_and_class() {
    let offset = note(0, 240, 61)
        .with_offset(PitchOffset::new(PitchOffset::SEMITONE / 2).unwrap())
        .unwrap();
    let (a, _) = track(
        "Lead",
        1,
        vec![
            (0, BAR, vec![note(0, 240, 61), note(480, 240, 60)]),
            (2 * BAR, BAR, vec![note(960, 240, 73), offset]),
        ],
    );
    let (b, _) = track("Bass", 2, vec![(BAR, BAR, vec![note(0, 240, 49)])]);
    let (project, ids) = project(vec![a, b]);

    let exact = find_hits(&project, &Query::Pitch(p(61)));
    assert_eq!(
        exact,
        vec![hit(ids[0], 0, 61), hit(ids[0], 2 * BAR, 61)],
        "오프셋은 보지 않고 클립 시작을 더한다"
    );
    let class = find_hits(&project, &Query::PitchClass(1));
    assert_eq!(
        class,
        vec![
            hit(ids[0], 0, 61),
            hit(ids[1], BAR, 49),
            hit(ids[0], 2 * BAR, 61),
            hit(ids[0], 2 * BAR + 960, 73),
        ]
    );
    assert!(find_hits(&project, &Query::Pitch(p(62))).is_empty());
}

#[test]
fn find_hits_text_matches_track_names_and_lyrics() {
    let (lead, _) = track(
        "Lead Vocal",
        1,
        vec![(
            BAR,
            BAR,
            vec![
                note(480, 240, 64).with_lyric("사랑"),
                note(0, 240, 62).with_lyric("LA"),
            ],
        )],
    );
    // 이름이 맞는 트랙: 첫 노트(가사 없음)의 자리
    let (vocal2, _) = track("vocal 2", 2, vec![(0, BAR, vec![note(240, 240, 70)])]);
    // 노트가 없는 트랙: 첫 클립 시작, 음높이 0
    let (vocal3, _) = track("VOCAL empty", 3, vec![(2 * BAR, BAR, vec![])]);
    // 클립도 없는 트랙: 0, 음높이 0
    let (vocal4, _) = track("vocal none", 4, vec![]);
    let (project, ids) = project(vec![lead, vocal2, vocal3, vocal4]);

    let hits = find_hits(&project, &Query::Text("vocal".into()));
    assert_eq!(
        hits,
        vec![
            Hit {
                track: ids[3],
                tick: Tick::ZERO,
                pitch: p(0),
                has_note: false,
            },
            hit(ids[1], 240, 70),
            // Lead Vocal의 첫 노트는 가사 LA(62)
            hit(ids[0], BAR, 62),
            Hit {
                track: ids[2],
                tick: Tick::new(2 * BAR),
                pitch: p(0),
                has_note: false,
            },
        ]
    );

    let lyric = find_hits(&project, &Query::Text("사랑".into()));
    assert_eq!(lyric, vec![hit(ids[0], BAR + 480, 64)]);
    let la = find_hits(&project, &Query::Text("la".into()));
    assert_eq!(la, vec![hit(ids[0], BAR, 62)], "대소문자 무시");

    // 트랙 이름과 첫 노트 가사가 같은 자리면 하나로 센다
    let (both, _) = track(
        "la la",
        1,
        vec![(0, BAR, vec![note(0, 240, 60).with_lyric("la")])],
    );
    let (project, ids) = self::project(vec![both]);
    assert_eq!(
        find_hits(&project, &Query::Text("la".into())),
        vec![hit(ids[0], 0, 60)]
    );
}

#[test]
fn find_hits_are_ordered_by_tick_track_pitch() {
    // 같은 틱에 두 트랙·두 음높이: 틱 → 트랙 순서 → 낮은 음 먼저
    let (a, _) = track(
        "A",
        1,
        vec![(
            0,
            BAR,
            vec![note(480, 240, 72), note(480, 240, 60), note(0, 240, 84)],
        )],
    );
    let (b, _) = track(
        "B",
        2,
        vec![(0, BAR, vec![note(480, 240, 48), note(0, 240, 36)])],
    );
    let (project, ids) = project(vec![a, b]);
    let hits = find_hits(&project, &Query::PitchClass(0));
    assert_eq!(
        hits,
        vec![
            hit(ids[0], 0, 84),
            hit(ids[1], 0, 36),
            hit(ids[0], 480, 60),
            hit(ids[0], 480, 72),
            hit(ids[1], 480, 48),
        ]
    );
}

// ---------- 다음·이전 ----------

fn three_hits() -> (Project, Vec<TrackId>, Vec<Hit>) {
    let (a, _) = track(
        "A",
        1,
        vec![(0, 2 * BAR, vec![note(0, 240, 60), note(BAR, 240, 60)])],
    );
    let (b, _) = track("B", 2, vec![(0, BAR, vec![note(480, 240, 60)])]);
    let (project, ids) = project(vec![a, b]);
    let hits = find_hits(&project, &Query::Pitch(p(60)));
    assert_eq!(hits.len(), 3);
    (project, ids, hits)
}

fn at(track_index: usize, tick: u64, pitch: u8) -> Position {
    Position {
        track_index,
        tick: Tick::new(tick),
        pitch: p(pitch),
    }
}

// 비교 대상이 `Option<Found>`라 감싼 채로 돌려준다
#[allow(clippy::unnecessary_wraps)]
fn found(index: usize, wrapped: bool) -> Option<Found> {
    Some(Found { index, wrapped })
}

#[test]
fn step_forward_backward_and_wrap() {
    let (project, _, hits) = three_hits();
    // hits: (A, 0), (B, 480), (A, BAR)
    let fwd = |pos| step(&project, &hits, pos, Direction::Forward);
    let back = |pos| step(&project, &hits, pos, Direction::Backward);

    assert_eq!(fwd(at(0, 0, 60)), found(1, false), "결과 위면 다음 것");
    assert_eq!(
        fwd(at(0, 0, 59)),
        found(0, false),
        "같은 틱·트랙의 낮은 음이면 그 결과가 뒤다"
    );
    assert_eq!(fwd(at(1, 0, 60)), found(1, false));
    assert_eq!(
        fwd(at(0, 480, 60)),
        found(1, false),
        "같은 틱, 트랙 A가 B보다 앞"
    );
    assert_eq!(fwd(at(1, 480, 60)), found(2, false));
    assert_eq!(fwd(at(0, BAR, 60)), found(0, true), "끝에서 처음으로");
    assert_eq!(fwd(at(1, 5 * BAR, 0)), found(0, true));

    assert_eq!(back(at(0, BAR, 60)), found(1, false));
    assert_eq!(
        back(at(0, BAR, 61)),
        found(2, false),
        "같은 자리 위의 음이면 그 결과가 앞"
    );
    assert_eq!(back(at(0, 0, 60)), found(2, true), "처음에서 끝으로");
    assert_eq!(back(at(0, 100, 0)), found(0, false));

    // 결과가 하나이고 그 위에 있으면 그 자리에 머물고 돎으로 센다
    let one = &hits[..1];
    assert_eq!(
        step(&project, one, at(0, 0, 60), Direction::Forward),
        found(0, true)
    );
    assert_eq!(
        step(&project, one, at(0, 0, 60), Direction::Backward),
        found(0, true)
    );
    assert_eq!(step(&project, &[], at(0, 0, 60), Direction::Forward), None);
}

#[test]
fn step_uses_current_track_order() {
    // 트랙 순서는 프로젝트의 지금 순서다: 결과의 트랙이 목록에서 몇 번째인지로 비교한다
    let (project, ids, hits) = three_hits();
    let (b, a) = (ids[1], ids[0]);
    let mut swapped = Project::new();
    swapped
        .push_track(project.track(b).unwrap().clone())
        .unwrap();
    swapped
        .push_track(project.track(a).unwrap().clone())
        .unwrap();
    // 같은 틱 480: 이제 B가 0번이라 (0, 480)에서의 다음은 A의 BAR가 아니라 B 480 뒤
    assert_eq!(
        step(&swapped, &hits, at(0, 480, 60), Direction::Forward),
        found(2, false)
    );
    assert_eq!(
        step(&swapped, &hits, at(0, 480, 59), Direction::Forward),
        found(1, false)
    );
}

proptest! {
    #[test]
    fn step_order_property_visits_every_hit_once(
        notes in proptest::collection::btree_set((0u64..4 * BAR, 0u8..3, 58u8..63), 1..30),
        start_tick in 0u64..5 * BAR,
        start_pitch in 50u8..70,
    ) {
        let mut a = Vec::new();
        let mut b = Vec::new();
        for (tick, which, pitch) in notes {
            let n = note(tick, 1, pitch);
            if which == 0 { a.push(n); } else { b.push(n); }
        }
        let (ta, _) = track("A", 1, vec![(0, 5 * BAR, a)]);
        let (tb, _) = track("B", 2, vec![(0, 5 * BAR, b)]);
        let (project, ids) = project(vec![ta, tb]);
        let hits = find_hits(&project, &Query::PitchClass(0).clone());
        let hits = if hits.is_empty() { find_hits(&project, &Query::Text("a".into())) } else { hits };
        prop_assume!(!hits.is_empty());
        let index_of = |id: TrackId| ids.iter().position(|t| *t == id).unwrap();
        for direction in [Direction::Forward, Direction::Backward] {
            let mut pos = at(0, start_tick, start_pitch);
            let mut seen = std::collections::BTreeSet::new();
            let mut wraps = 0;
            for _ in 0..hits.len() {
                let f = step(&project, &hits, pos, direction).unwrap();
                prop_assert!(seen.insert(f.index), "같은 결과를 두 번 골랐다");
                wraps += usize::from(f.wrapped);
                let h = hits[f.index];
                pos = at(index_of(h.track), h.tick.get(), h.pitch.get());
            }
            prop_assert_eq!(seen.len(), hits.len());
            prop_assert!(wraps <= 1);
        }
    }
}

// ---------- 노트 정보 ----------

fn line(lines: &[keyscore_app::note_info::InfoLine], label: &str) -> String {
    lines
        .iter()
        .find(|l| l.label == label)
        .unwrap_or_else(|| panic!("{label} 줄이 없다: {lines:?}"))
        .value
        .clone()
}

#[test]
fn info_lines_show_pitch_start_length_channel() {
    let lyric = note(480, 480, 61).with_lyric("별");
    let bent = note(0, 1440, 60)
        .with_offset(PitchOffset::new(PitchOffset::SEMITONE / 4).unwrap())
        .unwrap();
    let odd = note(1000, 100, 127);
    let triplet = note(2000, 320, 0);
    let (t, clips) = track(
        "Lead",
        10,
        vec![(
            BAR,
            BAR,
            vec![lyric.clone(), bent.clone(), odd.clone(), triplet.clone()],
        )],
    );
    let (project, ids) = project(vec![t]);

    let lines = info_lines(&project, ids[0], clips[0], &lyric);
    assert_eq!(line(&lines, "음높이"), "C#4 (61)");
    assert_eq!(line(&lines, "시작"), "2.1.480 (4320틱)");
    assert_eq!(line(&lines, "길이"), "480틱 (8분음표)");
    assert_eq!(line(&lines, "채널"), "10");
    assert_eq!(line(&lines, "벨로시티"), "100");
    assert_eq!(line(&lines, "가사"), "별");
    assert_eq!(line(&lines, "트랙"), "Lead");
    let labels: Vec<_> = lines.iter().map(|l| l.label).collect();
    assert_eq!(
        labels,
        ["음높이", "시작", "길이", "채널", "벨로시티", "가사", "트랙"]
    );

    let lines = info_lines(&project, ids[0], clips[0], &bent);
    assert_eq!(line(&lines, "음높이"), "C4 (60) +25 cent");
    assert_eq!(line(&lines, "시작"), "2.1.0 (3840틱)");
    assert_eq!(line(&lines, "길이"), "1440틱 (점4분음표)");
    assert!(
        lines.iter().all(|l| l.label != "가사"),
        "가사 없으면 줄 없음"
    );

    let lines = info_lines(&project, ids[0], clips[0], &odd);
    assert_eq!(line(&lines, "음높이"), "G9 (127)");
    assert_eq!(line(&lines, "시작"), "2.2.40 (4840틱)");
    assert_eq!(line(&lines, "길이"), "100틱");

    let lines = info_lines(&project, ids[0], clips[0], &triplet);
    assert_eq!(line(&lines, "음높이"), "C-1 (0)");
    assert_eq!(line(&lines, "길이"), "320틱 (8분음표 셋잇단)");

    // 없는 트랙·클립
    let (other, other_clips) = track("X", 1, vec![(0, BAR, vec![])]);
    assert!(info_lines(&project, other.id(), other_clips[0], &lyric).is_empty());
}
