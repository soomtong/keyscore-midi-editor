//! S3-T6b 드래그 이동·조절의 순수 계산(REQ 7.4 MS-2·MS-3·MS-6, 5.7). 규칙은
//! `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 6절, 시험 목록은 `docs/reports/s3-t6b/plan.md`.

use keyscore_app::actions::args::{LengthCode, MouseSnap};
use keyscore_app::pianoroll::mouse::{
    Axis, DragInput, Grip, dominant_axis, drag_shift, grip_at, snap_grid, snap_nearest,
};
use keyscore_app::pianoroll::ops::{
    NoteOp, NoteShift, OpError, TimeShift, plan_note_op, plan_shift, plan_shift_copy,
};
use keyscore_app::settings::Settings;
use keyscore_app::step::StepLength;
use keyscore_core::{
    Channel, Clip, ClipId, Document, ModelError, Note, Pitch, Project, Tick, TimeSignature, Track,
    TrackColor, TrackId, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn triple(n: &Note) -> (u64, u32, u8) {
    (n.start().get(), n.length(), n.pitch().get())
}

/// 클립마다 (시작, 길이, 노트).
fn project(clips: &[(u64, u64, &[Note])]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    for (start, length, notes) in clips {
        let mut clip = Clip::new(Tick::new(*start), *length).unwrap();
        for n in *notes {
            clip.insert_note(n.clone()).unwrap();
        }
        track.insert_clip(clip).unwrap();
    }
    project.push_track(track).unwrap();
    project
}

fn ids(project: &Project) -> (TrackId, ClipId) {
    let track = &project.tracks()[0];
    (track.id(), track.clips().next().unwrap().0)
}

/// 편집을 적용한 뒤 첫 클립의 (길이, 노트).
fn applied(project: &Project, edits: Vec<keyscore_core::Edit>) -> (u64, Vec<(u64, u32, u8)>) {
    let mut doc = Document::new(project.clone(), 1 << 20).unwrap();
    doc.apply(edits, None).unwrap();
    let clip = doc.project().tracks()[0].clips().next().unwrap().1.clone();
    (clip.length(), clip.notes().iter().map(triple).collect())
}

// ---------- 일반 변환 ----------

#[test]
fn note_ops_equal_plan_shift_of_their_shift() {
    let step = 480;
    let expect = |time, pitch, start, end| NoteShift {
        time,
        pitch,
        start,
        end,
    };
    assert_eq!(
        NoteOp::NudgeEarlier.shift(step),
        expect(TimeShift::Ticks(-480), 0, 0, 0)
    );
    assert_eq!(
        NoteOp::NudgeLater.shift(step),
        expect(TimeShift::Ticks(480), 0, 0, 0)
    );
    assert_eq!(
        NoteOp::NudgeBarEarlier.shift(step),
        expect(TimeShift::BarEarlier, 0, 0, 0)
    );
    assert_eq!(
        NoteOp::NudgeBarLater.shift(step),
        expect(TimeShift::BarLater, 0, 0, 0)
    );
    assert_eq!(
        NoteOp::TransposeUp.shift(step),
        expect(TimeShift::Ticks(0), 1, 0, 0)
    );
    assert_eq!(
        NoteOp::TransposeDown.shift(step),
        expect(TimeShift::Ticks(0), -1, 0, 0)
    );
    assert_eq!(
        NoteOp::OctaveUp.shift(step),
        expect(TimeShift::Ticks(0), 12, 0, 0)
    );
    assert_eq!(
        NoteOp::OctaveDown.shift(step),
        expect(TimeShift::Ticks(0), -12, 0, 0)
    );
    assert_eq!(
        NoteOp::Lengthen.shift(step),
        expect(TimeShift::Ticks(0), 0, 0, 480)
    );
    assert_eq!(
        NoteOp::Shorten.shift(step),
        expect(TimeShift::Ticks(0), 0, 0, -480)
    );
    // 스텝 0은 1틱(지금 `plan_note_op`과 같다)
    assert_eq!(
        NoteOp::NudgeLater.shift(0),
        expect(TimeShift::Ticks(1), 0, 0, 0)
    );

    // 박자표가 바뀐 곡에서 열 가지 모두 같은 결과(마디 단위는 노트 자리의 마디 길이)
    let notes = [
        note(2400, 480, 60),
        note(4000, 240, 127),
        note(5000, 960, 0),
    ];
    let mut p = project(&[(0, 2 * BAR, &notes)]);
    p.meter_map_mut()
        .set(Tick::new(2400), TimeSignature::new(3, 4).unwrap());
    let (track, clip) = ids(&p);
    for op in NoteOp::ALL {
        for picked in [&notes[..1], &notes[..2], &notes[2..]] {
            assert_eq!(
                plan_note_op(&p, track, clip, picked, op, step),
                plan_shift(&p, track, clip, picked, op.shift(step)),
                "{op:?} {picked:?}"
            );
        }
    }
}

#[test]
fn plan_shift_keeps_notes_that_would_vanish() {
    let a = note(0, 240, 60);
    let b = note(960, 960, 62);
    let p = project(&[(0, BAR, &[a.clone(), b.clone()])]);
    let (track, clip) = ids(&p);
    let shift = |start, end| NoteShift {
        start,
        end,
        ..NoteShift::IDENTITY
    };
    let planned = plan_shift(&p, track, clip, &[a.clone(), b.clone()], shift(0, -480))
        .unwrap()
        .expect("b가 바뀜");
    assert_eq!(planned.moved.len(), 1);
    assert_eq!(triple(&planned.moved[0].0), (960, 960, 62));
    assert_eq!(triple(&planned.moved[0].1), (960, 480, 62));
    // 둘 다 1틱 미만이 되면 바뀌는 노트가 없다
    assert_eq!(
        plan_shift(&p, track, clip, &[a.clone(), b.clone()], shift(0, -960)),
        Ok(None)
    );
    // 시작 Δ(끝 고정)도 같은 규칙
    let planned = plan_shift(&p, track, clip, &[a.clone(), b.clone()], shift(240, 0))
        .unwrap()
        .expect("b가 바뀜");
    assert_eq!(planned.moved.len(), 1);
    assert_eq!(triple(&planned.moved[0].1), (1200, 720, 62));
    // 항등 변환은 바뀌는 노트가 없다
    assert_eq!(
        plan_shift(&p, track, clip, &[a, b], NoteShift::IDENTITY),
        Ok(None)
    );
}

#[test]
fn plan_shift_rejects_before_clip_start_pitch_range_and_next_clip() {
    let first = note(0, 480, 60);
    let high = note(960, 480, 127);
    let tail = note(2880, 960, 64);
    let p = project(&[
        (0, BAR, &[first.clone(), high.clone(), tail.clone()]),
        (BAR, BAR, &[]),
    ]);
    let (track, clip) = ids(&p);
    let ticks = |t| NoteShift {
        time: TimeShift::Ticks(t),
        ..NoteShift::IDENTITY
    };
    assert_eq!(
        plan_shift(&p, track, clip, std::slice::from_ref(&first), ticks(-1)),
        Err(OpError::Model(ModelError::NoteBeforeClipStart))
    );
    let up = NoteShift {
        pitch: 1,
        ..NoteShift::IDENTITY
    };
    assert_eq!(
        plan_shift(&p, track, clip, &[first.clone(), high], up),
        Err(OpError::PitchOutOfRange)
    );
    assert!(matches!(
        plan_shift(&p, track, clip, std::slice::from_ref(&tail), ticks(960)),
        Err(OpError::Model(ModelError::ClipOverlap { .. }))
    ));
    // 다음 클립이 없으면 마디 단위로 늘린다
    let alone = project(&[(0, BAR, &[first, tail.clone()])]);
    let (track, clip) = ids(&alone);
    let planned = plan_shift(&alone, track, clip, &[tail], ticks(960))
        .unwrap()
        .expect("바뀜");
    let (length, notes) = applied(&alone, planned.edits);
    assert_eq!(length, 2 * BAR);
    assert_eq!(notes, vec![(0, 480, 60), (3840, 960, 64)]);
}

#[test]
fn plan_shift_copy_inserts_copies_and_keeps_originals() {
    let a = note(0, 480, 60);
    let b = note(2880, 960, 62);
    let p = project(&[(0, BAR, &[a.clone(), b.clone()])]);
    let (track, clip) = ids(&p);
    let later = NoteShift {
        time: TimeShift::Ticks(960),
        ..NoteShift::IDENTITY
    };
    let planned = plan_shift_copy(&p, track, clip, &[a.clone(), b.clone()], later)
        .unwrap()
        .expect("사본");
    let pairs: Vec<_> = planned
        .moved
        .iter()
        .map(|(from, to)| (triple(from), triple(to)))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ((0, 480, 60), (960, 480, 60)),
            ((2880, 960, 62), (3840, 960, 62))
        ]
    );
    let (length, notes) = applied(&p, planned.edits);
    assert_eq!(length, 2 * BAR, "사본이 끝을 넘으면 마디 단위로 늘린다");
    assert_eq!(
        notes,
        vec![
            (0, 480, 60),
            (960, 480, 60),
            (2880, 960, 62),
            (3840, 960, 62)
        ]
    );
    assert_eq!(
        plan_shift_copy(
            &p,
            track,
            clip,
            std::slice::from_ref(&a),
            NoteShift::IDENTITY
        ),
        Ok(None)
    );
    let earlier = NoteShift {
        time: TimeShift::Ticks(-1),
        ..NoteShift::IDENTITY
    };
    assert_eq!(
        plan_shift_copy(&p, track, clip, &[a], earlier),
        Err(OpError::Model(ModelError::NoteBeforeClipStart))
    );
    // 다음 클립과 겹치면 거부(`insert_notes`)
    let blocked = project(&[(0, BAR, std::slice::from_ref(&b)), (BAR, BAR, &[])]);
    let (track, clip) = ids(&blocked);
    assert!(matches!(
        plan_shift_copy(&blocked, track, clip, &[b], later),
        Err(OpError::Model(ModelError::ClipOverlap { .. }))
    ));
}

// ---------- 잡는 자리·스냅 ----------

#[test]
fn grip_edge_zone_is_six_px_or_third_of_narrow_note() {
    assert_eq!(grip_at(100.0, 60.0, 100.0), Grip::Start);
    assert_eq!(grip_at(100.0, 60.0, 105.9), Grip::Start);
    assert_eq!(grip_at(100.0, 60.0, 106.0), Grip::Body);
    assert_eq!(grip_at(100.0, 60.0, 153.9), Grip::Body);
    assert_eq!(grip_at(100.0, 60.0, 154.0), Grip::End);
    assert_eq!(grip_at(100.0, 60.0, 159.9), Grip::End);
    // 12 px 노트는 끝 영역이 4 px
    assert_eq!(grip_at(0.0, 12.0, 3.9), Grip::Start);
    assert_eq!(grip_at(0.0, 12.0, 4.0), Grip::Body);
    assert_eq!(grip_at(0.0, 12.0, 7.9), Grip::Body);
    assert_eq!(grip_at(0.0, 12.0, 8.0), Grip::End);
}

#[test]
fn dominant_axis_prefers_time_on_tie() {
    assert_eq!(dominant_axis(5.0, 5.0), Axis::Time);
    assert_eq!(dominant_axis(3.0, -4.0), Axis::Pitch);
    assert_eq!(dominant_axis(-6.0, 5.0), Axis::Time);
}

#[test]
fn snap_nearest_rounds_to_bar_relative_grid() {
    let meter = keyscore_core::MeterMap::new(TimeSignature::COMMON);
    let at = |t: u64, g: u64| snap_nearest(&meter, Tick::new(t), g).get();
    assert_eq!(at(700, 480), 480);
    assert_eq!(at(720, 480), 480, "같은 거리면 앞쪽");
    assert_eq!(at(721, 480), 960);
    assert_eq!(at(3800, 960), 3840);
    // 마디 길이가 격자의 배수가 아니면 다음 마디 시작이 격자선이다
    assert_eq!(at(3700, 1100), 3840);
    assert_eq!(at(777, 0), 777);
    // 2400에서 3/4로 바뀌면 그 앞 마디는 2400에서 끝난다
    let mut changed = keyscore_core::MeterMap::new(TimeSignature::COMMON);
    changed.set(Tick::new(2400), TimeSignature::new(3, 4).unwrap());
    assert_eq!(snap_nearest(&changed, Tick::new(2300), 960).get(), 2400);
    assert_eq!(snap_nearest(&changed, Tick::new(3500), 960).get(), 3360);
}

#[test]
fn snap_grid_follows_setting() {
    let eighth = StepLength {
        code: LengthCode::Eighth,
        dotted: false,
    };
    let dotted = StepLength {
        code: LengthCode::Quarter,
        dotted: true,
    };
    assert_eq!(snap_grid(MouseSnap::Length, eighth), 480);
    assert_eq!(snap_grid(MouseSnap::Length, dotted), 1440);
    assert_eq!(snap_grid(MouseSnap::Quarter, eighth), 960);
    assert_eq!(snap_grid(MouseSnap::Eighth, dotted), 480);
    assert_eq!(snap_grid(MouseSnap::Sixteenth, eighth), 240);
    assert_eq!(snap_grid(MouseSnap::ThirtySecond, eighth), 120);
    assert_eq!(snap_grid(MouseSnap::SixtyFourth, eighth), 60);
}

// ---------- 드래그 양 ----------

/// 클립 시작 1마디, 잡은 노트 (480, 960, 60), 다른 노트 (1920, 240, 64). 4/4, 8분 격자.
struct Fixture {
    meter: keyscore_core::MeterMap,
    notes: Vec<Note>,
}

impl Fixture {
    fn new() -> Self {
        Fixture {
            meter: keyscore_core::MeterMap::new(TimeSignature::COMMON),
            notes: vec![note(480, 960, 60), note(1920, 240, 64)],
        }
    }

    fn shift(
        &self,
        grip: Grip,
        ticks: f64,
        semitones: i32,
        step: u64,
        axis: Option<Axis>,
    ) -> NoteShift {
        drag_shift(&DragInput {
            meter: &self.meter,
            clip_start: BAR,
            notes: &self.notes,
            grabbed: &self.notes[0],
            grip,
            ticks,
            semitones,
            grid: step,
            axis,
        })
    }
}

fn moved(time: i64, pitch: i32) -> NoteShift {
    NoteShift {
        time: TimeShift::Ticks(time),
        pitch,
        ..NoteShift::IDENTITY
    }
}

fn resized(start: i64, end: i64) -> NoteShift {
    NoteShift {
        start,
        end,
        ..NoteShift::IDENTITY
    }
}

#[test]
fn drag_shift_body_snaps_grabbed_start() {
    let f = Fixture::new();
    // 잡은 노트 시작 4320 + 700 = 5020 → 가까운 8분 격자 4800
    assert_eq!(f.shift(Grip::Body, 700.0, 2, 480, None), moved(480, 2));
    // 4320 + 240 = 4560: 4320과 4800의 가운데 → 앞쪽
    assert_eq!(f.shift(Grip::Body, 240.0, 0, 480, None), moved(0, 0));
    assert!(f.shift(Grip::Body, 0.4, 0, 480, None).is_identity());
    assert_eq!(f.shift(Grip::Body, -100.0, -1, 480, None), moved(0, -1));
}

#[test]
fn drag_shift_free_rounds_raw_ticks() {
    let f = Fixture::new();
    assert_eq!(f.shift(Grip::Body, 700.6, -1, 0, None), moved(701, -1));
    assert_eq!(f.shift(Grip::Body, -0.4, 0, 0, None), moved(0, 0));
}

#[test]
fn drag_shift_clamps_clip_start_and_pitch_range() {
    let f = Fixture::new();
    // 가장 이른 노트(480)가 클립 시작에서 멈춘다
    assert_eq!(f.shift(Grip::Body, -2000.0, 0, 480, None), moved(-480, 0));
    assert_eq!(f.shift(Grip::Body, -2000.0, 0, 0, None), moved(-480, 0));
    // 가장 높은 노트(64)가 127에서, 가장 낮은 노트(60)가 0에서 멈춘다
    assert_eq!(f.shift(Grip::Body, 0.0, 70, 480, None), moved(0, 63));
    assert_eq!(f.shift(Grip::Body, 0.0, -70, 480, None), moved(0, -60));
}

#[test]
fn drag_shift_end_snaps_and_keeps_grabbed_length() {
    let f = Fixture::new();
    // 잡은 노트 끝 5280. +100 → 5380 → 5280, +300 → 5580 → 5760
    assert_eq!(f.shift(Grip::End, 100.0, 0, 480, None), resized(0, 0));
    assert_eq!(f.shift(Grip::End, 300.0, 3, 480, None), resized(0, 480));
    // 시작(4320) 이하로 끌면 시작 뒤 첫 격자선(4800)
    assert_eq!(f.shift(Grip::End, -1000.0, 0, 480, None), resized(0, -480));
    // 스냅 없음이면 시작 + 1
    assert_eq!(f.shift(Grip::End, -1000.0, 0, 0, None), resized(0, -959));
    assert_eq!(f.shift(Grip::End, 123.4, 0, 0, None), resized(0, 123));
}

#[test]
fn drag_shift_start_keeps_end_and_clamps() {
    let f = Fixture::new();
    // 잡은 노트 시작 4320. +300 → 4620 → 4800
    assert_eq!(f.shift(Grip::Start, 300.0, 0, 480, None), resized(480, 0));
    // 끝(5280) 이상이면 끝 앞 마지막 격자선(4800)
    assert_eq!(f.shift(Grip::Start, 1200.0, 0, 480, None), resized(480, 0));
    assert_eq!(f.shift(Grip::Start, 1200.0, 0, 0, None), resized(959, 0));
    // 격자 밖 짧은 노트를 안쪽으로 끌어도 길어지지 않는다(리뷰 1 제안 3)
    let short = note(10, 100, 60);
    let input = |grip, ticks| DragInput {
        meter: &f.meter,
        clip_start: 0,
        notes: std::slice::from_ref(&short),
        grabbed: &short,
        grip,
        ticks,
        semitones: 0,
        grid: 480,
        axis: None,
    };
    assert_eq!(drag_shift(&input(Grip::Start, 300.0)), resized(0, 0));
    assert_eq!(drag_shift(&input(Grip::End, -20.0)), resized(0, 0));
    // 클립 시작 앞으로는 가지 않는다(가장 이른 노트 480)
    assert_eq!(
        f.shift(Grip::Start, -1000.0, 0, 480, None),
        resized(-480, 0)
    );
}

#[test]
fn drag_shift_axis_lock_zeroes_other_axis() {
    let f = Fixture::new();
    assert_eq!(
        f.shift(Grip::Body, 700.0, 2, 480, Some(Axis::Time)),
        moved(480, 0)
    );
    assert_eq!(
        f.shift(Grip::Body, 700.0, 2, 480, Some(Axis::Pitch)),
        moved(0, 2)
    );
    // 끝 조절에는 뜻이 없다
    assert_eq!(
        f.shift(Grip::End, 300.0, 2, 480, Some(Axis::Pitch)),
        resized(0, 480)
    );
}

// ---------- 설정 ----------

fn temp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-mouse-snap-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("settings.json")
}

#[test]
fn mouse_snap_setting_round_trips_and_rejects_bad_values() {
    let path = temp_path("rw");
    std::fs::write(&path, r#"{"input": {"mouse_snap": "1/16"}}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(s.input.mouse_snap, MouseSnap::Sixteenth);
    s.save(&path).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["input"]["mouse_snap"], "1/16");

    let path = temp_path("bad");
    std::fs::write(&path, r#"{"input": {"mouse_snap": "1/3"}}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s.input.mouse_snap, MouseSnap::Length);
    assert_eq!(warnings.len(), 1, "{warnings:?}");

    let (s, _) = Settings::load(&temp_path("none"));
    assert_eq!(s.input.mouse_snap, MouseSnap::Length);
}
