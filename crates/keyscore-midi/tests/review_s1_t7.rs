//! S1-T7 적대적 리뷰 1: `render`를 깨뜨리려는 시도. 통과하는 테스트는 회귀 테스트로 남는다.

use std::collections::HashMap;

use keyscore_core::{
    Bank, Channel, Clip, Note, Pitch, Program, Project, Tick, Track, TrackColor, Velocity,
};
use keyscore_midi::{ScheduledEvent, render};
use proptest::prelude::*;

fn ch(n: u8) -> Channel {
    Channel::new(n).unwrap()
}

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn track_with(name: &str, channel: u8, clips: &[(u64, u64, &[Note])]) -> Track {
    let mut t = Track::new(name, ch(channel), TrackColor::new(0).unwrap());
    for &(at, len, notes) in clips {
        let mut clip = Clip::new(Tick::new(at), len).unwrap();
        for n in notes {
            clip.insert_note(n.clone()).unwrap();
        }
        t.insert_clip(clip).unwrap();
    }
    t
}

fn project(tracks: Vec<Track>) -> Project {
    let mut p = Project::new();
    for t in tracks {
        p.push_track(t).unwrap();
    }
    p
}

fn flat(events: &[ScheduledEvent]) -> Vec<(u64, u16, Vec<u8>)> {
    events
        .iter()
        .map(|e| (e.tick().get(), e.port(), e.msg().bytes().to_vec()))
        .collect()
}

/// 이벤트를 순서대로 흘려 키(포트, 채널, 음높이)마다 켜짐 수가 음수가 되지 않고 끝에 0인지 본다.
fn assert_balanced(events: &[ScheduledEvent]) {
    let mut on = HashMap::<(u16, u8, u8), i64>::new();
    for e in events {
        let msg = e.msg();
        let b = msg.bytes();
        if b.len() != 3 {
            continue;
        }
        let key = (e.port(), b[0] & 0x0F, b[1]);
        match (b[0] & 0xF0, b[2]) {
            (0x90, v) if v > 0 => *on.entry(key).or_default() += 1,
            (0x80 | 0x90, _) => {
                let c = on.entry(key).or_default();
                *c -= 1;
                assert!(
                    *c >= 0,
                    "켜지 않은 노트를 끈다: {key:?} @ {}",
                    e.tick().get()
                );
            }
            _ => {}
        }
    }
    assert!(on.values().all(|&c| c == 0), "걸린 노트: {on:?}");
}

#[test]
fn render_mute_solo_all_combinations_follow_solo_then_mute_rule() {
    // 트랙 셋의 (뮤트, 솔로) 4^3 조합 전부.
    for mask in 0u32..64 {
        let states: Vec<(bool, bool)> = (0..3)
            .map(|i| {
                let s = (mask >> (i * 2)) & 3;
                (s & 1 != 0, s & 2 != 0)
            })
            .collect();
        let tracks = states
            .iter()
            .enumerate()
            .map(|(i, &(mute, solo))| {
                let i = u8::try_from(i).unwrap();
                let mut t = track_with("T", i + 1, &[(0, 3840, &[note(0, 480, 60 + i)])]);
                t.set_mute(mute);
                t.set_solo(solo);
                t
            })
            .collect();
        let p = project(tracks);
        let any_solo = states.iter().any(|s| s.1);
        let expected: Vec<u8> = states
            .iter()
            .enumerate()
            .filter(|&(_, &(mute, solo))| if any_solo { solo } else { !mute })
            .map(|(i, _)| 60 + u8::try_from(i).unwrap())
            .collect();
        let r = render(&p, |_, _| Some(0));
        let mut got: Vec<u8> = r
            .events()
            .iter()
            .filter(|e| e.msg().bytes()[0] & 0xF0 == 0x90)
            .map(|e| e.msg().bytes()[1])
            .collect();
        got.sort_unstable();
        assert_eq!(got, expected, "상태 {states:?}");
    }
}

#[test]
fn render_tail_off_precedes_same_pitch_on_at_next_clip_start() {
    // 분할로 꼬리가 생긴 노트(3000–4600)와, 오른쪽 클립 첫 틱의 같은 음높이 노트.
    let mut t = track_with("T", 1, &[(0, 7680, &[note(3000, 1600, 60)])]);
    let id = t.clips().next().unwrap().0;
    let right = t.split_clip(id, Tick::new(3840)).unwrap();
    t.insert_note(right, note(0, 480, 60)).unwrap();
    let p = project(vec![t]);
    let r = render(&p, |_, _| Some(0));
    let at: Vec<_> = flat(r.events())
        .into_iter()
        .filter(|e| e.0 == 3840)
        .map(|e| e.2[0] & 0xF0)
        .collect();
    assert_eq!(at, vec![0x80, 0x90], "같은 틱에서 Note Off가 먼저여야 한다");
    assert_balanced(r.events());
    assert_eq!(r.end(), Tick::new(3840 + 480));
}

#[test]
fn render_note_ending_exactly_at_clip_end_is_not_cut() {
    let t = track_with("T", 1, &[(960, 3840, &[note(2880, 960, 64)])]);
    let p = project(vec![t]);
    let r = render(&p, |_, _| Some(0));
    assert_eq!(
        flat(r.events()),
        vec![
            (960 + 2880, 0, vec![0x90, 64, 100]),
            (960 + 3840, 0, vec![0x80, 64, 0]),
        ]
    );
    assert_eq!(r.end(), Tick::new(4800));
}

#[test]
fn render_overlapping_same_pitch_notes_stay_balanced() {
    // 같은 클립 안에서 같은 음높이가 겹친다(0–1920, 960–1440, 960–2880).
    let t = track_with(
        "T",
        1,
        &[(
            0,
            3840,
            &[note(0, 1920, 60), note(960, 480, 60), note(960, 1920, 60)],
        )],
    );
    let p = project(vec![t]);
    let r = render(&p, |_, _| Some(0));
    assert_balanced(r.events());
    assert_eq!(r.end(), Tick::new(2880));
}

#[test]
fn render_empty_and_program_only_projects_end_at_zero() {
    let r = render(&Project::new(), |_, _| Some(0));
    assert!(r.events().is_empty());
    assert_eq!(r.end(), Tick::ZERO);
    let mut t = Track::new("P", ch(1), TrackColor::new(0).unwrap());
    t.set_program(Some(Program::new(5).unwrap()));
    let r = render(&project(vec![t]), |_, _| Some(0));
    assert_eq!(flat(r.events()), vec![(0, 0, vec![0xC0, 5])]);
    assert_eq!(r.end(), Tick::ZERO);
}

#[test]
fn render_bank_program_per_track_stay_grouped_before_tick_zero_notes() {
    // 같은 채널의 두 트랙. 각 트랙의 MSB·LSB·프로그램이 붙어 있고 Note On보다 앞이다.
    let mut a = track_with("A", 1, &[(0, 3840, &[note(0, 480, 60)])]);
    a.set_bank(Some(Bank::new(129).unwrap()));
    a.set_program(Some(Program::new(1).unwrap()));
    let mut b = track_with("B", 2, &[(0, 3840, &[note(0, 480, 62)])]);
    b.set_bank(Some(Bank::new(2).unwrap()));
    b.set_program(Some(Program::new(7).unwrap()));
    let r = render(&project(vec![a, b]), |_, _| Some(0));
    let bytes: Vec<Vec<u8>> = flat(r.events()).into_iter().map(|e| e.2).collect();
    assert_eq!(
        bytes,
        vec![
            vec![0xB0, 0, 1],
            vec![0xB0, 32, 1],
            vec![0xC0, 1],
            vec![0xB1, 0, 0],
            vec![0xB1, 32, 2],
            vec![0xC1, 7],
            vec![0x90, 60, 100],
            vec![0x91, 62, 100],
            vec![0x80, 60, 0],
            vec![0x81, 62, 0],
        ]
    );
}

#[test]
fn render_port_of_gets_absolute_track_index_when_earlier_tracks_are_muted() {
    let mut a = track_with("A", 1, &[(0, 3840, &[note(0, 480, 60)])]);
    a.set_mute(true);
    let b = track_with("B", 2, &[(0, 3840, &[note(0, 480, 62)])]);
    let mut seen = Vec::new();
    let r = render(&project(vec![a, b]), |i, t| {
        seen.push((i, t.name().to_string()));
        Some(u16::try_from(i).unwrap())
    });
    assert_eq!(seen, vec![(1, "B".to_string())]);
    assert!(r.events().iter().all(|e| e.port() == 1));
}

#[test]
fn render_smallest_velocity_is_still_note_on() {
    let n = Note::new(Tick::ZERO, 480, Pitch::new(60).unwrap(), Velocity::new(1)).unwrap();
    let t = track_with("T", 1, &[(0, 3840, &[n])]);
    let r = render(&project(vec![t]), |_, _| Some(0));
    let on = r.events()[0].msg().bytes().to_vec();
    assert_eq!(on[0], 0x90);
    assert!(on[2] >= 1, "벨로시티 0 Note On은 Note Off로 읽힌다: {on:?}");
}

proptest! {
    /// 임의의 클립·노트(분할로 생긴 꼬리 포함)에서 켜고 끄기가 맞고, 모든 이벤트가 클립 범위 안이다.
    #[test]
    fn render_any_project_is_balanced_and_inside_clips(
        notes in prop::collection::vec((0u64..7680, 1u32..4000, 58u8..62), 0..24),
        split in 1u64..7680,
        gap in 0u64..2000,
    ) {
        let mut t = Track::new("T", ch(1), TrackColor::new(0).unwrap());
        let mut clip = Clip::new(Tick::new(gap), 7680 + 4000).unwrap();
        for &(s, l, p) in &notes {
            clip.insert_note(note(s, l, p)).unwrap();
        }
        let id = t.insert_clip(clip).unwrap();
        // 분할로 숨김 꼬리를 만든다(결정 Q1).
        let _ = t.split_clip(id, Tick::new(gap + split));
        let bounds: Vec<(u64, u64)> = t.clips().map(|(_, c)| (c.start().get(), c.end())).collect();
        let p = project(vec![t]);
        let r = render(&p, |_, _| Some(0));
        assert_balanced(r.events());
        for e in r.events() {
            let tick = e.tick().get();
            prop_assert!(bounds.iter().any(|&(s, end)| s <= tick && tick <= end), "{tick} {bounds:?}");
        }
        let sorted = r.events().windows(2).all(|w| w[0].tick() <= w[1].tick());
        prop_assert!(sorted);
        prop_assert_eq!(r.end(), r.events().last().map_or(Tick::ZERO, ScheduledEvent::tick));
    }
}
