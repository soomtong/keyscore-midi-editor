//! S1-T7 모델 → 스케줄러 이벤트 변환 테스트.

use keyscore_core::{
    Bank, Channel, Clip, Note, Pitch, Program, Project, Tick, Track, TrackColor, Velocity,
};
use keyscore_midi::{ScheduledEvent, render};

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

/// 클립 하나(시작 `at`, 한 마디)에 노트들을 담은 트랙.
fn track(name: &str, channel: u8, at: u64, notes: &[Note]) -> Track {
    let mut t = Track::new(name, ch(channel), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::new(at), 3840).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    t.insert_clip(clip).unwrap();
    t
}

fn project(tracks: Vec<Track>) -> Project {
    let mut p = Project::new();
    for t in tracks {
        p.push_track(t).unwrap();
    }
    p
}

/// (틱, 포트, 바이트) 목록.
fn flat(events: &[ScheduledEvent]) -> Vec<(u64, u16, Vec<u8>)> {
    let mut v: Vec<_> = events
        .iter()
        .map(|e| (e.tick().get(), e.port(), e.msg().bytes().to_vec()))
        .collect();
    v.sort();
    v
}

fn all_on_port_zero() -> impl FnMut(usize, &Track) -> Option<u16> {
    |_, _| Some(0)
}

#[test]
fn render_skips_muted_tracks_and_honours_solo() {
    let a = track("A", 1, 0, &[note(0, 480, 60)]);
    let mut b = track("B", 2, 0, &[note(0, 480, 62)]);
    b.set_mute(true);
    let c = track("C", 3, 0, &[note(0, 480, 64)]);
    let r = render(
        &project(vec![a.clone(), b.clone(), c.clone()]),
        all_on_port_zero(),
    );
    let channels: Vec<u8> = flat(r.events()).iter().map(|e| e.2[0] & 0x0F).collect();
    assert!(channels.iter().all(|&c| c == 0 || c == 2), "{channels:?}");
    // 솔로가 있으면 솔로 트랙만. 뮤트이면서 솔로인 트랙은 솔로가 이긴다(흔한 DAW 규칙).
    let mut c_solo = c;
    c_solo.set_solo(true);
    let r = render(&project(vec![a, b, c_solo]), all_on_port_zero());
    let channels: Vec<u8> = flat(r.events()).iter().map(|e| e.2[0] & 0x0F).collect();
    assert!(
        !channels.is_empty() && channels.iter().all(|&c| c == 2),
        "{channels:?}"
    );
}

#[test]
fn render_cuts_hidden_tail_at_clip_end() {
    let mut t = track("T", 1, 3840, &[note(3000, 800, 60), note(0, 100, 64)]);
    let id = t.clips().next().unwrap().0;
    // 3840 + 3200에서 나누면 틱 3000의 노트(끝 3800)는 왼쪽 클립의 꼬리다. 클립 끝(3840 + 3200)에서 끈다.
    t.split_clip(id, Tick::new(3840 + 3200)).unwrap();
    let r = render(&project(vec![t]), all_on_port_zero());
    let events = flat(r.events());
    assert!(events.contains(&(3840 + 3000, 0, vec![0x90, 60, 100])));
    assert!(
        events.contains(&(3840 + 3200, 0, vec![0x80, 60, 0])),
        "{events:?}"
    );
    assert!(events.contains(&(3840 + 100, 0, vec![0x80, 64, 0])));
    assert_eq!(r.end(), Tick::new(3840 + 3200));
}

#[test]
fn render_sends_bank_then_program_at_tick_zero() {
    let mut t = track("T", 5, 0, &[note(0, 10, 60)]);
    t.set_bank(Some(Bank::new(130).unwrap()));
    t.set_program(Some(Program::new(7).unwrap()));
    let r = render(&project(vec![t]), all_on_port_zero());
    let first: Vec<Vec<u8>> = r
        .events()
        .iter()
        .filter(|e| e.tick() == Tick::ZERO)
        .map(|e| e.msg().bytes().to_vec())
        .collect();
    // 같은 틱 안에서는 넣은 순서(뱅크 MSB, LSB, 프로그램)가 스냅샷 정렬에서도 지켜진다.
    assert_eq!(
        first,
        [
            vec![0xB4, 0, 1],
            vec![0xB4, 32, 2],
            vec![0xC4, 7],
            vec![0x94, 60, 100]
        ]
    );
}

#[test]
fn render_uses_track_channel_and_port_mapping() {
    let a = track("A", 1, 0, &[note(0, 10, 60)]);
    let b = track("B", 16, 0, &[note(0, 10, 61)]);
    let c = track("C", 2, 0, &[note(0, 10, 62)]);
    let r = render(&project(vec![a, b, c]), |i, t| match (i, t.name()) {
        (0, "A") => Some(3),
        (1, "B") => Some(7),
        _ => None,
    });
    let events = flat(r.events());
    assert_eq!(
        events,
        [
            (0, 3, vec![0x90, 60, 100]),
            (0, 7, vec![0x9F, 61, 100]),
            (10, 3, vec![0x80, 60, 0]),
            (10, 7, vec![0x8F, 61, 0]),
        ]
    );
}

#[test]
fn render_end_is_last_note_off() {
    let a = track("A", 1, 0, &[note(0, 960, 60), note(100, 2000, 62)]);
    let b = track("B", 2, 7680, &[note(10, 5, 64)]);
    let r = render(&project(vec![a, b]), all_on_port_zero());
    assert_eq!(r.end(), Tick::new(7680 + 15));
    let ons = r
        .events()
        .iter()
        .filter(|e| e.msg().bytes()[0] & 0xF0 == 0x90)
        .count();
    let offs = r
        .events()
        .iter()
        .filter(|e| e.msg().bytes()[0] & 0xF0 == 0x80)
        .count();
    assert_eq!((ons, offs), (3, 3));
    assert_eq!(
        render(&Project::new(), all_on_port_zero()).end(),
        Tick::ZERO
    );
}
