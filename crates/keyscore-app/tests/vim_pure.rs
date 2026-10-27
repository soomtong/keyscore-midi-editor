//! S3-T3c 순수 계산(`pianoroll::vim`): 읽기 순서 범위, 모션 목적지, 텍스트 객체, 비주얼 집합. 규칙은
//! `docs/reports/s3-t3c/plan.md` 「끝의 정의」 1·4·5·6절.

use std::collections::BTreeSet;

use keyscore_app::pianoroll::Cursor;
use keyscore_app::pianoroll::vim::{
    Motion, Reach, TextObject, VisualKind, motion_notes, motion_target, object_notes, reading_key,
    visual_notes,
};
use keyscore_core::{
    Channel, Clip, MeterMap, Note, Pitch, Tick, TimeSignature, Track, TrackColor, Velocity,
};

const BAR: u64 = 3840;
const STEP: u64 = 960;

type Triple = (u64, u32, u8);

const C4: Triple = (0, 960, 60);
const D4: Triple = (960, 960, 62);
const E4: Triple = (1920, 960, 64);
const G4: Triple = (1920, 960, 67);
const D4B: Triple = (2880, 960, 62);
const A4: Triple = (3840, 960, 69);

fn note(t: Triple) -> Note {
    Note::new(
        Tick::new(t.0),
        t.1,
        Pitch::new(t.2).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn cell(tick: u64, pitch: u8) -> Cursor {
    Cursor {
        tick: Tick::new(tick),
        pitch: Pitch::new(pitch).unwrap(),
    }
}

/// 클립 [`start`, `start` + 2마디)에 `notes`(클립 기준).
fn clip_at(start: u64, notes: &[Triple]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), 2 * BAR).unwrap();
    for &n in notes {
        clip.insert_note(note(n)).unwrap();
    }
    clip
}

fn fixture_clip() -> Clip {
    clip_at(0, &[C4, D4, E4, G4, D4B, A4])
}

fn track_with(clips: Vec<Clip>) -> Track {
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    for c in clips {
        track.insert_clip(c).unwrap();
    }
    track
}

fn triples(set: &BTreeSet<Note>) -> Vec<Triple> {
    let mut v: Vec<Triple> = set
        .iter()
        .map(|n| (n.start().get(), n.length(), n.pitch().get()))
        .collect();
    v.sort_unstable();
    v
}

fn sorted(mut v: Vec<Triple>) -> Vec<Triple> {
    v.sort_unstable();
    v
}

// ---------- 1. 읽기 순서 자리 ----------

#[test]
fn reading_key_uses_note_start_under_cell_or_cell() {
    let clip = fixture_clip();
    // 노트 시작 칸
    assert_eq!(reading_key(&clip, cell(960, 62)), (960, 62));
    // 노트 중간 칸은 그 노트의 시작
    assert_eq!(reading_key(&clip, cell(1440, 62)), (960, 62));
    // 노트가 없는 칸은 칸 자체
    assert_eq!(reading_key(&clip, cell(1440, 61)), (1440, 61));
    // 클립이 뒤에서 시작해도 절대 틱
    let late = clip_at(BAR, &[(0, 960, 60)]);
    assert_eq!(reading_key(&late, cell(BAR + 500, 60)), (BAR, 60));
}

// ---------- 2. 모션 범위 ----------

#[test]
fn motion_notes_exclusive_inclusive_and_chords() {
    let clip = fixture_clip();
    let from = cell(960, 62);
    // 배타: 앞으로 가면 커서 노트를 넣고 목적지를 뺀다
    assert_eq!(
        triples(&motion_notes(&clip, from, cell(1920, 64), Reach::Exclusive)),
        vec![D4]
    );
    // 포함: 목적지 노트도 넣는다
    assert_eq!(
        triples(&motion_notes(&clip, from, cell(1920, 64), Reach::Inclusive)),
        vec![D4, E4]
    );
    // 뒤로 가는 배타는 커서 노트를 뺀다
    assert_eq!(
        triples(&motion_notes(&clip, from, cell(0, 60), Reach::Exclusive)),
        vec![C4]
    );
    // 같은 틱의 낮은 음은 읽기 순서에서 앞이라 빠진다(결정 1 A의 글자)
    assert!(motion_notes(&clip, from, cell(0, 62), Reach::Exclusive).is_empty());
    // 화음 안: E4(1920, 64)에서 G4까지 배타는 E4만
    assert_eq!(
        triples(&motion_notes(
            &clip,
            cell(1920, 64),
            cell(1920, 67),
            Reach::Exclusive
        )),
        vec![E4]
    );
    // 노트 중간 칸에서 시작해도 그 노트부터
    assert_eq!(
        triples(&motion_notes(
            &clip,
            cell(1440, 62),
            cell(1920, 64),
            Reach::Exclusive
        )),
        vec![D4]
    );
    // 클립 처음부터 / 클립 끝까지
    assert_eq!(
        triples(&motion_notes(
            &clip,
            from,
            cell(0, 62),
            Reach::FromClipStart
        )),
        vec![C4, D4]
    );
    assert_eq!(
        triples(&motion_notes(&clip, from, cell(7680, 62), Reach::ToClipEnd)),
        vec![D4, E4, G4, D4B, A4]
    );
    // 음높이 모션은 범위가 없다
    assert!(motion_notes(&clip, from, cell(960, 63), Reach::Pitch).is_empty());
}

#[test]
fn motion_reach_table() {
    use Motion::*;
    for m in [
        Left, Right, PrevBar, NextBar, NextNote, PrevNote, NextBeat, PrevBeat, NextClip, PrevClip,
    ] {
        assert_eq!(m.reach(), Reach::Exclusive, "{m:?}");
    }
    for m in [NoteEnd, LastNoteEndInBar, NextSamePitch, PrevSamePitch] {
        assert_eq!(m.reach(), Reach::Inclusive, "{m:?}");
    }
    assert_eq!(ClipStart.reach(), Reach::FromClipStart);
    assert_eq!(ClipEnd.reach(), Reach::ToClipEnd);
    for m in [Up, Down, OctaveUp, OctaveDown] {
        assert_eq!(m.reach(), Reach::Pitch, "{m:?}");
    }
    assert!(!ClipStart.counts() && !ClipEnd.counts());
    assert!(NextNote.counts() && Left.counts());
}

// ---------- 3. 모션 목적지 ----------

#[test]
fn beat_targets_follow_meter() {
    let track = track_with(vec![fixture_clip()]);
    let meter = MeterMap::default();
    let t =
        |from: u64, m| motion_target(&track, &meter, STEP, cell(from, 62), m).map(|c| c.tick.get());
    assert_eq!(t(960, Motion::NextBeat), Some(1920));
    assert_eq!(t(1000, Motion::NextBeat), Some(1920));
    // 박 시작이면 앞 박, 박 중간이면 그 박 시작
    assert_eq!(t(960, Motion::PrevBeat), Some(0));
    assert_eq!(t(1000, Motion::PrevBeat), Some(960));
    assert_eq!(t(0, Motion::PrevBeat), None);
    // 박자표 변화 지점도 박 시작이다: 4/4 첫 마디 뒤 틱 3840에 3/8
    let mut meter = MeterMap::default();
    meter.set(Tick::new(BAR), TimeSignature::new(3, 8).unwrap());
    assert_eq!(
        motion_target(&track, &meter, STEP, cell(BAR, 62), Motion::NextBeat).map(|c| c.tick.get()),
        Some(BAR + 480)
    );
    assert_eq!(
        motion_target(&track, &meter, STEP, cell(BAR, 62), Motion::PrevBeat).map(|c| c.tick.get()),
        Some(BAR - 960)
    );
}

#[test]
fn note_end_targets() {
    let track = track_with(vec![fixture_clip()]);
    let meter = MeterMap::default();
    let t = |from: Cursor, m| motion_target(&track, &meter, STEP, from, m);
    // 커서 노트 D4의 마지막 칸은 960이고 커서가 이미 거기라 다음 노트 E4(1920, 64)의 마지막 칸
    assert_eq!(t(cell(960, 62), Motion::NoteEnd), Some(cell(1920, 64)));
    // 노트가 없는 칸: 읽기 순서 다음 노트의 마지막 칸
    assert_eq!(t(cell(0, 61), Motion::NoteEnd), Some(cell(960, 62)));
    // 긴 노트: 마지막 칸 = 시작 + ⌊(길이 − 1) / 스텝⌋ × 스텝
    let long = track_with(vec![clip_at(0, &[(0, 2000, 60)])]);
    assert_eq!(
        motion_target(&long, &meter, STEP, cell(0, 60), Motion::NoteEnd),
        Some(cell(1920, 60))
    );
    // E: 마디 0에서 시작하는 노트 가운데 읽기 순서 마지막(D4B)의 마지막 칸
    assert_eq!(
        t(cell(960, 62), Motion::LastNoteEndInBar),
        Some(cell(2880, 62))
    );
    // 이미 거기면 다음 마디(A4)
    assert_eq!(
        t(cell(2880, 62), Motion::LastNoteEndInBar),
        Some(cell(3840, 69))
    );
    // 더 없으면 없다
    assert_eq!(t(cell(3840, 69), Motion::LastNoteEndInBar), None);
}

#[test]
fn step_and_note_targets_reuse_s3_rules() {
    let track = track_with(vec![fixture_clip(), clip_at(4 * BAR, &[(0, 960, 69)])]);
    let meter = MeterMap::default();
    let t = |from: Cursor, m| motion_target(&track, &meter, STEP, from, m);
    assert_eq!(t(cell(960, 62), Motion::Right), Some(cell(1920, 62)));
    assert_eq!(t(cell(960, 62), Motion::Left), Some(cell(0, 62)));
    assert_eq!(t(cell(960, 62), Motion::Up), Some(cell(960, 63)));
    assert_eq!(t(cell(960, 62), Motion::OctaveDown), Some(cell(960, 50)));
    assert_eq!(t(cell(960, 62), Motion::NextNote), Some(cell(1920, 64)));
    assert_eq!(t(cell(960, 62), Motion::PrevNote), Some(cell(0, 60)));
    assert_eq!(
        t(cell(960, 62), Motion::NextSamePitch),
        Some(cell(2880, 62))
    );
    assert_eq!(t(cell(960, 62), Motion::PrevSamePitch), None);
    assert_eq!(t(cell(960, 62), Motion::NextClip), Some(cell(4 * BAR, 62)));
    assert_eq!(t(cell(960, 62), Motion::NextBar), Some(cell(BAR, 62)));
    assert_eq!(t(cell(960, 62), Motion::ClipStart), Some(cell(0, 62)));
    assert_eq!(
        t(cell(960, 62), Motion::ClipEnd),
        Some(cell(2 * BAR - 960, 62))
    );
    // 클립 밖에서는 클립 처음·끝이 없다
    assert_eq!(t(cell(3 * BAR, 62), Motion::ClipStart), None);
}

// ---------- 4. 텍스트 객체 ----------

#[test]
fn object_notes_inner_note_bar_and_clip() {
    let clip = fixture_clip();
    let meter = MeterMap::default();
    let o = |c: Cursor, obj| triples(&object_notes(&clip, &meter, c, obj));
    assert_eq!(o(cell(1440, 62), TextObject::InnerNote), vec![D4]);
    assert!(o(cell(1440, 61), TextObject::InnerNote).is_empty());
    assert_eq!(
        o(cell(960, 62), TextObject::InnerBar),
        vec![C4, D4, E4, G4, D4B]
    );
    assert_eq!(o(cell(3840, 50), TextObject::InnerBar), vec![A4]);
    assert_eq!(
        o(cell(960, 62), TextObject::AroundClip),
        sorted(vec![C4, D4, E4, G4, D4B, A4])
    );
}

// ---------- 5. 비주얼 ----------

#[test]
fn visual_notes_by_kind() {
    let clip = fixture_clip();
    let v = |a: Cursor, c: Cursor, k| triples(&visual_notes(&clip, a, c, STEP, k));
    // v: 읽기 순서 양끝 포함
    assert_eq!(
        v(cell(960, 62), cell(1920, 64), VisualKind::Note),
        vec![D4, E4]
    );
    // 거꾸로 넓혀도 같다
    assert_eq!(
        v(cell(1920, 64), cell(960, 62), VisualKind::Note),
        vec![D4, E4]
    );
    // 처음 한 칸: 칸 위 노트만
    assert_eq!(v(cell(960, 62), cell(960, 62), VisualKind::Note), vec![D4]);
    // 노트 중간에서 시작해도 그 노트부터
    assert_eq!(
        v(cell(1440, 62), cell(1920, 67), VisualKind::Note),
        vec![D4, E4, G4]
    );
    // V: 시간 범위 [min, max + 스텝)의 모든 음높이
    assert_eq!(
        v(cell(960, 30), cell(1920, 30), VisualKind::Time),
        vec![D4, E4, G4]
    );
    // Ctrl+v: 직사각형
    assert_eq!(
        v(cell(960, 62), cell(1920, 64), VisualKind::Block),
        vec![D4, E4]
    );
    assert_eq!(
        v(cell(960, 60), cell(2880, 62), VisualKind::Block),
        vec![D4, D4B]
    );
}
