//! S3-T7 적대적 리뷰 4회차(좁은 확인 리뷰)의 깨뜨리기 시험(`docs/reports/s3-t7/review-4-confirm.md`).
//! 리뷰 3 뒤 수정(`search::walk`, 노트 정보 cent 표시)만 본다. 실패하는 시험은 실패한 채로 둔다.

use keyscore_app::note_info::info_lines;
use keyscore_app::search::{Direction, Hit, Position, Query, find_hits, step, walk};
use keyscore_core::{
    Channel, Clip, Note, Pitch, PitchOffset, Project, Tick, Track, TrackColor, Velocity,
};
use proptest::prelude::*;

const BAR: u64 = 3840;

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 트랙 하나의 꼴: 이름이 「pad」를 품는가, 노트(틱, 음높이, 가사 「pad」인가), 노트 없을 때 클립 시작(없으면
/// 클립 없음).
type TrackSpec = (bool, Vec<(u64, u8, bool)>, Option<u64>);

fn build(specs: &[TrackSpec]) -> Project {
    let mut project = Project::new();
    for (i, (named, notes, empty_clip)) in specs.iter().enumerate() {
        let name = if *named {
            format!("Pad {i}")
        } else {
            format!("Lead {i}")
        };
        let mut t = Track::new(name, Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
        if notes.is_empty() {
            if let Some(start) = empty_clip {
                t.insert_clip(Clip::new(Tick::new(*start), BAR).unwrap())
                    .unwrap();
            }
        } else {
            let mut clip = Clip::new(Tick::ZERO, 4 * BAR).unwrap();
            for &(tick, pitch, lyric) in notes {
                let n = note(tick, 1, pitch);
                clip.insert_note(if lyric { n.with_lyric("PAD") } else { n })
                    .unwrap();
            }
            t.insert_clip(clip).unwrap();
        }
        project.push_track(t).unwrap();
    }
    project
}

fn track_spec() -> impl Strategy<Value = TrackSpec> {
    (
        any::<bool>(),
        proptest::collection::btree_map((0u64..2 * BAR, 58u8..63), any::<bool>(), 0..6)
            .prop_map(|m| m.into_iter().map(|((t, p), l)| (t, p, l)).collect()),
        proptest::option::of(0u64..2 * BAR),
    )
}

/// 키를 `count`번 누른 흉내(GUI `search_jump`와 같다: 노트 없는 결과는 커서 음높이를 둔다).
fn repeat(
    project: &Project,
    hits: &[Hit],
    at0: Position,
    direction: Direction,
    count: u64,
) -> (usize, bool, Pitch) {
    let mut pos = at0;
    let mut wrapped = false;
    let mut index = 0;
    for _ in 0..count {
        let f = step(project, hits, pos, direction).unwrap();
        wrapped |= f.wrapped;
        index = f.index;
        let h = hits[index];
        pos = Position {
            track_index: project.track_index(h.track).unwrap(),
            tick: h.tick,
            pitch: if h.has_note { h.pitch } else { pos.pitch },
        };
    }
    (index, wrapped, pos.pitch)
}

fn walked(
    project: &Project,
    hits: &[Hit],
    at0: Position,
    direction: Direction,
    count: u32,
) -> (usize, bool, Pitch) {
    let w = walk(project, hits, at0, direction, count).unwrap();
    (
        w.index,
        w.wrapped,
        w.pitch_from.map_or(at0.pitch, |i| hits[i].pitch),
    )
}

/// 시작 자리: 결과 위(노트 없는 결과 위에서는 다른 음높이)거나 임의의 자리.
fn start_at(
    project: &Project,
    hits: &[Hit],
    on_hit: Option<usize>,
    free: (usize, u64, u8),
) -> Position {
    match on_hit {
        Some(i) if !hits.is_empty() => {
            let h = hits[i % hits.len()];
            Position {
                track_index: project.track_index(h.track).unwrap(),
                tick: h.tick,
                pitch: if h.has_note {
                    h.pitch
                } else {
                    Pitch::new(free.2).unwrap()
                },
            }
        }
        _ => Position {
            track_index: free.0 % project.tracks().len(),
            tick: Tick::new(free.1),
            pitch: Pitch::new(free.2).unwrap(),
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 구현자의 `walk_equals_repeated_steps`는 노트 결과가 많아야 하나(가사 없는 노트, 트랙 이름 결과 하나)다.
    /// 여기서는 트랙 1–5개를 아무 순서로 섞고, 노트 결과(가사)·트랙 이름 결과(노트 있음/없음/클립 없음)가 같은
    /// 틱에 겹치게 하며, 결과 위·임의 자리에서 시작한다.
    #[test]
    fn walk_equals_repeated_steps_on_mixed_track_orders(
        specs in proptest::collection::vec(track_spec(), 1..6),
        count in 1u32..40,
        on_hit in proptest::option::of(0usize..64),
        free in (0usize..8, 0u64..3 * BAR, 40u8..80),
        forward in any::<bool>(),
    ) {
        let project = build(&specs);
        let hits = find_hits(&project, &Query::Text("pad".into()));
        prop_assume!(!hits.is_empty());
        let direction = if forward { Direction::Forward } else { Direction::Backward };
        let at0 = start_at(&project, &hits, on_hit, free);
        prop_assert_eq!(
            walked(&project, &hits, at0, direction, count),
            repeat(&project, &hits, at0, direction, u64::from(count))
        );
    }

    /// u32 끝의 카운트와 결과 수의 배수. 첫 걸음 뒤 걷기는 결과 수를 주기로 되풀이되므로, 결과 수보다 큰 카운트는
    /// `(count - 1) % n + 1 + n`걸음과 같다(그만큼은 실제로 `step`을 되풀이해 본다).
    #[test]
    fn walk_with_huge_or_multiple_counts_matches_period(
        specs in proptest::collection::vec(track_spec(), 1..5),
        big in prop_oneof![
            (u32::MAX - 64..=u32::MAX).prop_map(Some),
            Just(None),
        ],
        multiple in 1u32..5,
        on_hit in proptest::option::of(0usize..64),
        free in (0usize..8, 0u64..3 * BAR, 40u8..80),
        forward in any::<bool>(),
    ) {
        let project = build(&specs);
        let hits = find_hits(&project, &Query::Text("pad".into()));
        prop_assume!(!hits.is_empty());
        let n = u32::try_from(hits.len()).unwrap();
        let count = big.unwrap_or(n * multiple);
        let direction = if forward { Direction::Forward } else { Direction::Backward };
        let at0 = start_at(&project, &hits, on_hit, free);
        let steps = if count > n {
            u64::from((count - 1) % n + 1 + n)
        } else {
            u64::from(count)
        };
        prop_assert_eq!(
            walked(&project, &hits, at0, direction, count),
            repeat(&project, &hits, at0, direction, steps)
        );
    }

    /// cent 표시는 부호가 오프셋의 부호와 같고, 보인 값이 참값에서 0.05 cent 안이다. 0이 아닌 오프셋은 0으로
    /// 보이지 않는다.
    #[test]
    fn cent_text_is_signed_and_within_half_tenth(units in -PitchOffset::MAX..=PitchOffset::MAX) {
        prop_assume!(units != 0);
        let text = pitch_line(units);
        let rest = text.strip_prefix("C#4 (61) ").unwrap();
        let rest = rest.strip_suffix(" cent").unwrap();
        let (sign, body) = rest.split_at(1);
        prop_assert_eq!(sign, if units < 0 { "-" } else { "+" });
        let true_milli = i128::from(units).abs() * 100_000 / i128::from(PitchOffset::SEMITONE);
        if let Some(below) = body.strip_prefix('<') {
            prop_assert_eq!(below, "0.1");
            prop_assert!(true_milli < 50, "{} 단위가 {}", units, text);
        } else {
            let shown_milli = if let Some((w, f)) = body.split_once('.') {
                prop_assert_eq!(f.len(), 1);
                w.parse::<i128>().unwrap() * 1000 + f.parse::<i128>().unwrap() * 100
            } else {
                body.parse::<i128>().unwrap() * 1000
            };
            prop_assert!(shown_milli > 0, "0이 아닌 오프셋이 0: {}", text);
            prop_assert!((shown_milli - true_milli).abs() <= 50, "{} 단위가 {}", units, text);
        }
    }
}

fn pitch_line(offset_units: i32) -> String {
    let n = note(0, 480, 61)
        .with_offset(PitchOffset::new(offset_units).unwrap())
        .unwrap();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, BAR).unwrap();
    clip.insert_note(n.clone()).unwrap();
    let clip_id = t.insert_clip(clip).unwrap();
    let track_id = t.id();
    let mut project = Project::new();
    project.push_track(t).unwrap();
    info_lines(&project, track_id, clip_id, &n)
        .into_iter()
        .find(|l| l.label == "음높이")
        .unwrap()
        .value
}

/// 0.05 cent 경계(반음 = 2^25 단위이므로 0.05 cent = 16777.216 단위)와 99.95 cent 경계.
#[test]
fn cent_rounding_boundaries() {
    assert_eq!(pitch_line(16_777), "C#4 (61) +<0.1 cent");
    assert_eq!(pitch_line(16_778), "C#4 (61) +0.1 cent");
    assert_eq!(pitch_line(-16_777), "C#4 (61) -<0.1 cent");
    assert_eq!(pitch_line(-16_778), "C#4 (61) -0.1 cent");
    // 99.95 cent = 33537654.784 단위
    assert_eq!(pitch_line(33_537_654), "C#4 (61) +99.9 cent");
    assert_eq!(pitch_line(33_537_655), "C#4 (61) +100 cent");
    assert_eq!(pitch_line(-33_537_655), "C#4 (61) -100 cent");
}

/// 결과 하나: 그 위에서 `n`은 돌아 제자리, 앞에서 `1 n`은 돌지 않고 `2 n`부터 돈다. u32 끝에서도 넘치지 않는다.
#[test]
fn walk_single_hit_wrap_and_huge_count() {
    let project = build(&[(true, vec![], Some(BAR))]);
    let hits = find_hits(&project, &Query::Text("pad".into()));
    assert_eq!(hits.len(), 1);
    let before = Position {
        track_index: 0,
        tick: Tick::ZERO,
        pitch: Pitch::new(70).unwrap(),
    };
    for direction in [Direction::Forward, Direction::Backward] {
        let w = walk(&project, &hits, before, direction, 1).unwrap();
        assert_eq!(w.index, 0);
        assert_eq!(w.wrapped, direction == Direction::Backward);
        assert_eq!(w.pitch_from, None);
        for count in [2, u32::MAX, 0] {
            let w = walk(&project, &hits, before, direction, count).unwrap();
            assert_eq!(w.index, 0);
            assert_eq!(
                w.wrapped,
                count > 1 || direction == Direction::Backward,
                "{direction:?} {count}"
            );
            assert_eq!(w.pitch_from, None);
        }
    }
    assert!(walk(&project, &[], before, Direction::Forward, 3).is_none());
}
