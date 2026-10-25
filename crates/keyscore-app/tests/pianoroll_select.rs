//! S3-T2 커서·선택·수식키 체계(REQ 5.3, 5.7, 5.8, PRE-1, UI-11). 5.7 표의 모든 칸을 테이블로, 선택·조작·되돌리기
//! 묶음·노트 단위 이동·마크는 헤드리스 창으로, 미리듣기와 커서부터 재생은 같은 프로세스의 CoreMIDI 수신기로 본다.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{cursor, note, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::ops::{NoteOp, OpError, plan_note_op};
use keyscore_app::pianoroll::scene::{self, Scene, SceneInput};
use keyscore_app::pianoroll::select::{
    bar_notes, clip_last_step, next_note, next_same_pitch, note_under, prev_note, prev_same_pitch,
    region_notes,
};
use keyscore_app::pianoroll::{Cursor, MaxLenCache, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Edit, Key, MeterMap, ModelError, Note, Pitch, Project, Tick,
    TimeSignature, Track, TrackColor, TrackId, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;
const WAIT: Duration = Duration::from_secs(2);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-sel-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext, project: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
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

fn pitch(value: u8) -> Pitch {
    Pitch::new(value).unwrap()
}

fn at(tick: u64, p: u8) -> Cursor {
    Cursor {
        tick: Tick::new(tick),
        pitch: pitch(p),
    }
}

fn clip(start: u64, length: u64, notes: &[Note]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    clip
}

/// 클립 A(0–4마디) 노트 8개와 클립 B(4마디부터 1마디)의 G4.
const A_NOTES: [(u64, u32, u8); 8] = [
    (0, 960, 60),
    (960, 960, 62),
    (1920, 480, 64),
    (3840, 960, 60),
    (3840, 960, 64),
    (3840, 960, 67),
    (7680, 1920, 69),
    (14400, 960, 65),
];
const D4: (u64, u32, u8) = (960, 960, 62);
const A4: (u64, u32, u8) = (7680, 1920, 69);

fn clip_a() -> Clip {
    let notes: Vec<Note> = A_NOTES.iter().map(|&(s, l, p)| note(s, l, p)).collect();
    clip(0, 4 * BAR, &notes)
}

fn clip_b() -> Clip {
    clip(4 * BAR, BAR, &[note(0, 960, 67)])
}

fn fixture_track() -> Track {
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip_a()).unwrap();
    track.insert_clip(clip_b()).unwrap();
    track
}

fn fixture() -> Project {
    let mut project = Project::new();
    project.push_track(fixture_track()).unwrap();
    project
}

/// 클립 A만 있는 프로젝트(클립 연장 시험).
fn fixture_single_clip() -> Project {
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip_a()).unwrap();
    project.push_track(track).unwrap();
    project
}

fn triple(n: &Note) -> (u64, u32, u8) {
    (n.start().get(), n.length(), n.pitch().get())
}

fn sorted(set: &BTreeSet<Note>) -> Vec<(u64, u32, u8)> {
    set.iter().map(triple).collect()
}

fn cursor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (u64, u8) {
    cx.read(|cx| {
        let c = view.read(cx).piano_roll().cursor();
        (c.tick.get(), c.pitch.get())
    })
}

fn set_cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64, p: u8) {
    cx.update(|cx| view.read(cx).piano_roll().set_cursor(at(tick, p)));
}

fn selection_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u32, u8)> {
    cx.read(|cx| {
        let s = view.read(cx).selection();
        let mut v: Vec<_> = s.notes().map(triple).collect();
        v.sort_unstable();
        v
    })
}

fn has_selection(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_state().has_selection())
}

/// 첫 트랙 첫 클립의 (트랙, 클립) 식별자.
fn ids(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (TrackId, ClipId) {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        (track.id(), track.clips().next().unwrap().0)
    })
}

/// 클립 A의 노트 몇 개를 선택으로 둔다(시험 준비. 모드 FSM도 함께 맞춘다).
fn select(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, notes: &[(u64, u32, u8)]) {
    let (track, clip) = ids(cx, view);
    let set: BTreeSet<Note> = notes.iter().map(|&(s, l, p)| note(s, l, p)).collect();
    view.update(cx, |this, cx| {
        let mut sel = Selection::default();
        sel.set(track, clip, set);
        this.set_selection(sel, cx);
    });
}

fn clip_a_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u32, u8)> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .next()
            .unwrap()
            .1
            .notes()
            .iter()
            .map(triple)
            .collect()
    })
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

fn enter_piano_roll(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) {
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
}

// ---------- 1. 선택 영역(순수) ----------

#[test]
fn region_notes_picks_notes_overlapping_cells_between_anchor_and_cursor() {
    let a = clip_a();
    let pick = |x: Cursor, y: Cursor| sorted(&region_notes(&a, x, y, QUARTER));
    assert_eq!(
        pick(at(960, 62), at(1920, 62)),
        vec![D4],
        "[960, 2880) × 62"
    );
    assert_eq!(pick(at(1920, 62), at(960, 62)), vec![D4], "순서 무관");
    assert_eq!(
        pick(at(0, 60), at(3840, 60)),
        vec![(0, 960, 60), (3840, 960, 60)],
        "[0, 4800) × 60"
    );
    assert_eq!(
        pick(at(3840, 55), at(3840, 67)),
        vec![(3840, 960, 60), (3840, 960, 64), (3840, 960, 67)],
        "음높이 범위는 양끝 포함"
    );
    assert_eq!(
        pick(at(8640, 69), at(8640, 69)),
        vec![A4],
        "긴 노트는 겹침으로 든다"
    );
    assert_eq!(
        pick(at(8640, 70), at(8640, 72)),
        vec![],
        "음높이 밖은 빠진다"
    );
    assert_eq!(
        pick(at(2400, 64), at(2400, 64)),
        vec![],
        "끝 틱(배타)에서는 걸치지 않는다"
    );
    // 커서 틱은 절대 틱이고 클립 B는 15360에서 시작한다
    let b = clip_b();
    assert_eq!(
        sorted(&region_notes(&b, at(15360, 67), at(15360, 67), QUARTER)),
        vec![(0, 960, 67)]
    );
    assert_eq!(
        sorted(&region_notes(&b, at(0, 67), at(960, 67), QUARTER)),
        vec![]
    );
}

// ---------- 2. 다음·이전 노트(순수) ----------

#[test]
fn next_prev_note_walk_track_notes_in_start_pitch_order() {
    let track = fixture_track();
    let walk = |c: Cursor| next_note(&track, c).map(|(_, c)| (c.tick.get(), c.pitch.get()));
    let back = |c: Cursor| prev_note(&track, c).map(|(_, c)| (c.tick.get(), c.pitch.get()));
    assert_eq!(walk(at(0, 60)), Some((960, 62)));
    assert_eq!(
        walk(at(3840, 60)),
        Some((3840, 64)),
        "화음은 음높이 순으로 한 음씩"
    );
    assert_eq!(walk(at(3840, 67)), Some((7680, 69)));
    assert_eq!(
        walk(at(14400, 65)),
        Some((15360, 67)),
        "다음 클립으로 넘어간다"
    );
    assert_eq!(walk(at(15360, 67)), None, "끝에서는 없음");
    assert_eq!(
        walk(at(1000, 100)),
        Some((1920, 64)),
        "노트 사이에서는 그다음 노트"
    );
    assert_eq!(back(at(960, 62)), Some((0, 60)));
    assert_eq!(back(at(0, 60)), None);
    assert_eq!(back(at(3840, 64)), Some((3840, 60)));
    assert_eq!(
        back(at(15360, 67)),
        Some((14400, 65)),
        "앞 클립으로 넘어간다"
    );
    assert_eq!(back(at(1000, 100)), Some((960, 62)));
    // 같은 음높이(vim f / F)
    let same = |c: Cursor| next_same_pitch(&track, c).map(|c| (c.tick.get(), c.pitch.get()));
    let same_back = |c: Cursor| prev_same_pitch(&track, c).map(|c| (c.tick.get(), c.pitch.get()));
    assert_eq!(same(at(0, 60)), Some((3840, 60)));
    assert_eq!(same(at(3840, 60)), None);
    assert_eq!(same(at(0, 67)), Some((3840, 67)));
    assert_eq!(same(at(3840, 67)), Some((15360, 67)));
    assert_eq!(same_back(at(15360, 67)), Some((3840, 67)));
    assert_eq!(same_back(at(0, 60)), None);
    // 커서 위 노트: 커서 틱을 품고 음높이가 같은 노트
    let a = clip_a();
    assert_eq!(note_under(&a, at(1200, 62)).map(|n| triple(&n)), Some(D4));
    assert_eq!(note_under(&a, at(1920, 62)), None, "끝 틱은 밖");
    assert_eq!(note_under(&a, at(9000, 69)).map(|n| triple(&n)), Some(A4));
    assert_eq!(note_under(&a, at(9000, 70)), None);
}

// ---------- 3. 마디 선택과 클립 마지막 칸(순수) ----------

#[test]
fn bar_notes_and_clip_last_step() {
    let a = clip_a();
    let meter = MeterMap::default();
    assert_eq!(
        sorted(&bar_notes(&a, &meter, Tick::new(3840 + 960))),
        vec![(3840, 960, 60), (3840, 960, 64), (3840, 960, 67)]
    );
    assert_eq!(
        sorted(&bar_notes(&a, &meter, Tick::ZERO)),
        vec![(0, 960, 60), (960, 960, 62), (1920, 480, 64)]
    );
    assert_eq!(
        sorted(&bar_notes(&a, &meter, Tick::new(2 * BAR + 100))),
        vec![A4],
        "A4(7680–9600)는 2마디 안"
    );
    assert_eq!(
        sorted(&bar_notes(&a, &meter, Tick::new(10 * BAR))),
        vec![],
        "클립 밖 마디"
    );
    let waltz = MeterMap::new(TimeSignature::new(3, 4).unwrap());
    assert_eq!(
        sorted(&bar_notes(&a, &waltz, Tick::new(2880))),
        vec![(3840, 960, 60), (3840, 960, 64), (3840, 960, 67)],
        "3/4의 두 번째 마디 [2880, 5760)"
    );
    assert_eq!(clip_last_step(&a, QUARTER).get(), 14400);
    assert_eq!(clip_last_step(&clip(0, 960, &[]), QUARTER).get(), 0);
    assert_eq!(clip_last_step(&clip(0, 1000, &[]), QUARTER).get(), 960);
    assert_eq!(clip_last_step(&clip(0, 2000, &[]), QUARTER).get(), 1920);
    assert_eq!(
        clip_last_step(&clip(4 * BAR, BAR, &[]), QUARTER).get(),
        4 * BAR + 3 * QUARTER
    );
}

// ---------- 4. 조작 계획(순수) ----------

#[test]
fn plan_note_op_rules() {
    let project = fixture();
    let track = project.tracks()[0].id();
    let clip_id = project.tracks()[0].clips().next().unwrap().0;
    let n = |t: (u64, u32, u8)| note(t.0, t.1, t.2);
    let plan = |p: &Project, notes: &[(u64, u32, u8)], op: NoteOp| {
        let notes: Vec<Note> = notes.iter().map(|t| n(*t)).collect();
        let clip_id = p.tracks()[0].clips().next().unwrap().0;
        plan_note_op(p, p.tracks()[0].id(), clip_id, &notes, op, QUARTER)
    };
    // 한 스텝 미룸
    let planned = plan(&project, &[D4], NoteOp::NudgeLater)
        .unwrap()
        .expect("바뀜");
    assert_eq!(planned.moved.len(), 1);
    assert_eq!(triple(&planned.moved[0].1), (1920, 960, 62));
    match &planned.edits[..] {
        [
            Edit::ReplaceNotes {
                track: t,
                clip: c,
                length,
                pairs,
            },
        ] => {
            assert_eq!((*t, *c), (track, clip_id));
            assert_eq!(*length, 4 * BAR, "클립 안이면 길이 그대로");
            assert_eq!(pairs.len(), 1);
        }
        other => panic!("{other:?}"),
    }
    // 한 마디
    let planned = plan(&project, &[A4], NoteOp::NudgeBarLater)
        .unwrap()
        .unwrap();
    assert_eq!(triple(&planned.moved[0].1), (11520, 1920, 69));
    let planned = plan(&project, &[A4], NoteOp::NudgeBarEarlier)
        .unwrap()
        .unwrap();
    assert_eq!(triple(&planned.moved[0].1), (3840, 1920, 69));
    // 클립 시작 앞은 거부
    assert!(matches!(
        plan(&project, &[(0, 960, 60)], NoteOp::NudgeEarlier),
        Err(OpError::Model(ModelError::NoteBeforeClipStart))
    ));
    assert!(plan(&project, &[(0, 960, 60)], NoteOp::NudgeBarEarlier).is_err());
    // 늘리기가 클립 끝을 넘으면 마디 단위 연장. 다음 클립과 겹치면 거부
    assert!(matches!(
        plan(&project, &[(14400, 960, 65)], NoteOp::Lengthen),
        Err(OpError::Model(ModelError::ClipOverlap { .. }))
    ));
    let single = fixture_single_clip();
    let planned = plan(&single, &[(14400, 960, 65)], NoteOp::Lengthen)
        .unwrap()
        .unwrap();
    match &planned.edits[..] {
        [Edit::ReplaceNotes { length, .. }] => assert_eq!(*length, 5 * BAR),
        other => panic!("{other:?}"),
    }
    assert_eq!(triple(&planned.moved[0].1), (14400, 1920, 65));
    // 줄이기: 한 스텝 이하는 그대로. 모두 그대로면 None
    assert!(plan(&project, &[D4], NoteOp::Shorten).unwrap().is_none());
    let planned = plan(&project, &[D4, A4], NoteOp::Shorten).unwrap().unwrap();
    assert_eq!(planned.moved.len(), 1);
    assert_eq!(triple(&planned.moved[0].1), (7680, 960, 69));
    // 음높이 범위 밖은 거부(접지 않는다)
    let mut high = Project::new();
    let mut t = Track::new("H", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    t.insert_clip(clip(
        0,
        BAR,
        &[note(0, 960, 127), note(960, 960, 120), note(1920, 960, 3)],
    ))
    .unwrap();
    high.push_track(t).unwrap();
    assert!(plan(&high, &[(0, 960, 127)], NoteOp::TransposeUp).is_err());
    assert!(plan(&high, &[(960, 960, 120)], NoteOp::OctaveUp).is_err());
    assert!(plan(&high, &[(1920, 960, 3)], NoteOp::OctaveDown).is_err());
    assert!(
        plan(&high, &[(0, 960, 127), (1920, 960, 3)], NoteOp::TransposeUp).is_err(),
        "하나라도 밖이면 전부 거부"
    );
    let planned = plan(&high, &[(960, 960, 120)], NoteOp::TransposeDown)
        .unwrap()
        .unwrap();
    assert_eq!(triple(&planned.moved[0].1), (960, 960, 119));
    let planned = plan(&project, &[A4], NoteOp::OctaveUp).unwrap().unwrap();
    assert_eq!(triple(&planned.moved[0].1), (7680, 1920, 81));
    // 클립에 없는 노트는 거부
    assert!(plan(&project, &[(5, 5, 5)], NoteOp::NudgeLater).is_err());
    // 조작마다 다른 묶음 키
    let keys: BTreeSet<u64> = NoteOp::ALL.iter().map(|op| op.coalesce_key()).collect();
    assert_eq!(keys.len(), NoteOp::ALL.len());
}

// ---------- 6. 5.7 표 전체 ----------

#[derive(Debug, Clone)]
enum Expect {
    Cursor(u64, u8),
    CursorAndSelection((u64, u8), Vec<(u64, u32, u8)>),
    Replaced {
        from: (u64, u32, u8),
        to: (u64, u32, u8),
    },
    Nothing,
}

struct Cell {
    keys: [&'static str; 2],
    cursor: (u64, u8),
    selection: &'static [(u64, u32, u8)],
    expect: Expect,
}

// 5.7 표 한 칸이 한 줄이라 표 길이만큼 길다.
#[allow(clippy::too_many_lines)]
fn table() -> Vec<Cell> {
    use Expect::{Cursor as C, CursorAndSelection as CS, Nothing, Replaced};
    let cell = |keys, cursor, selection, expect| Cell {
        keys,
        cursor,
        selection,
        expect,
    };
    let chord: &'static [(u64, u32, u8)] = &[(3840, 960, 60), (3840, 960, 64), (3840, 960, 67)];
    vec![
        // (없음) 커서
        cell(["h", "left"], (1920, 60), &[], C(960, 60)),
        cell(["l", "right"], (1920, 60), &[], C(2880, 60)),
        cell(["j", "down"], (1920, 60), &[], C(1920, 59)),
        cell(["k", "up"], (1920, 60), &[], C(1920, 61)),
        // Shift 선택 넓히기
        cell(
            ["shift-h", "shift-left"],
            (1920, 62),
            &[],
            CS((960, 62), vec![D4]),
        ),
        cell(
            ["shift-l", "shift-right"],
            (960, 62),
            &[],
            CS((1920, 62), vec![D4]),
        ),
        cell(
            ["shift-j", "shift-down"],
            (960, 62),
            &[],
            CS((960, 61), vec![D4]),
        ),
        cell(
            ["shift-k", "shift-up"],
            (960, 61),
            &[],
            CS((960, 62), vec![D4]),
        ),
        // Ctrl 큰 커서 이동
        cell(
            ["ctrl-h", "ctrl-left"],
            (2 * BAR + 960, 60),
            &[],
            C(2 * BAR, 60),
        ),
        cell(["ctrl-l", "ctrl-right"], (960, 60), &[], C(BAR, 60)),
        cell(["ctrl-j", "ctrl-down"], (1920, 60), &[], C(1920, 48)),
        cell(["ctrl-k", "ctrl-up"], (1920, 60), &[], C(1920, 72)),
        // Ctrl+Shift 크게 선택(「한 마디 뒤까지」는 Ctrl+h와 같은 읽기)
        cell(
            ["ctrl-shift-h", "ctrl-shift-left"],
            (BAR + 960, 60),
            &[],
            CS((BAR, 60), vec![(3840, 960, 60)]),
        ),
        cell(
            ["ctrl-shift-l", "ctrl-shift-right"],
            (0, 60),
            &[],
            CS((BAR, 60), vec![(0, 960, 60), (3840, 960, 60)]),
        ),
        cell(
            ["ctrl-shift-j", "ctrl-shift-down"],
            (3840, 67),
            &[],
            CS((3840, 55), chord.to_vec()),
        ),
        cell(
            ["ctrl-shift-k", "ctrl-shift-up"],
            (3840, 60),
            &[],
            CS((3840, 72), chord.to_vec()),
        ),
        // Alt 선택 노트 밀기
        cell(
            ["alt-h", "alt-left"],
            (0, 60),
            &[D4],
            Replaced {
                from: D4,
                to: (0, 960, 62),
            },
        ),
        cell(
            ["alt-l", "alt-right"],
            (0, 60),
            &[D4],
            Replaced {
                from: D4,
                to: (1920, 960, 62),
            },
        ),
        cell(
            ["alt-j", "alt-down"],
            (0, 60),
            &[D4],
            Replaced {
                from: D4,
                to: (960, 960, 61),
            },
        ),
        cell(
            ["alt-k", "alt-up"],
            (0, 60),
            &[D4],
            Replaced {
                from: D4,
                to: (960, 960, 63),
            },
        ),
        // Alt+Shift 길이·옥타브
        cell(
            ["alt-shift-h", "alt-shift-left"],
            (0, 60),
            &[A4],
            Replaced {
                from: A4,
                to: (7680, 960, 69),
            },
        ),
        cell(
            ["alt-shift-l", "alt-shift-right"],
            (0, 60),
            &[A4],
            Replaced {
                from: A4,
                to: (7680, 2880, 69),
            },
        ),
        cell(
            ["alt-shift-j", "alt-shift-down"],
            (0, 60),
            &[A4],
            Replaced {
                from: A4,
                to: (7680, 1920, 57),
            },
        ),
        cell(
            ["alt-shift-k", "alt-shift-up"],
            (0, 60),
            &[A4],
            Replaced {
                from: A4,
                to: (7680, 1920, 81),
            },
        ),
        // Alt+Ctrl 크게 밀기. j/k는 (예약)
        cell(
            ["ctrl-alt-h", "ctrl-alt-left"],
            (0, 60),
            &[A4],
            Replaced {
                from: A4,
                to: (3840, 1920, 69),
            },
        ),
        cell(
            ["ctrl-alt-l", "ctrl-alt-right"],
            (0, 60),
            &[A4],
            Replaced {
                from: A4,
                to: (11520, 1920, 69),
            },
        ),
        cell(["ctrl-alt-j", "ctrl-alt-down"], (0, 60), &[A4], Nothing),
        cell(["ctrl-alt-k", "ctrl-alt-up"], (0, 60), &[A4], Nothing),
    ]
}

#[gpui_kit::test]
fn modifier_table_5_7_every_cell(cx: &mut TestAppContext) {
    start(cx, "table");
    let mut failures = Vec::new();
    for cell in table() {
        for key in cell.keys {
            let (handle, view) = open(cx, fixture());
            enter_piano_roll(cx, handle, &view);
            set_cursor(cx, &view, cell.cursor.0, cell.cursor.1);
            if !cell.selection.is_empty() {
                select(cx, &view, cell.selection);
            }
            let before_notes = clip_a_notes(cx, &view);
            let before_sel = selection_of(cx, &view);
            press(cx, handle, key);
            let got_cursor = cursor_of(cx, &view);
            let got_sel = selection_of(cx, &view);
            let got_notes = clip_a_notes(cx, &view);
            let ok = match &cell.expect {
                Expect::Cursor(t, p) => got_cursor == (*t, *p) && got_sel == before_sel,
                Expect::CursorAndSelection(c, sel) => {
                    let mut want = sel.clone();
                    want.sort_unstable();
                    got_cursor == *c && got_sel == want && has_selection(cx, &view)
                }
                Expect::Replaced { from, to } => {
                    got_notes.contains(to)
                        && !got_notes.contains(from)
                        && got_notes.len() == before_notes.len()
                        && got_sel == vec![*to]
                        && undo_len(cx, &view) == 1
                        && got_cursor == cell.cursor
                }
                Expect::Nothing => {
                    got_notes == before_notes
                        && got_sel == before_sel
                        && got_cursor == cell.cursor
                        && undo_len(cx, &view) == 0
                }
            };
            if !ok {
                failures.push(format!(
                    "{key}: 기대 {:?}, 커서 {got_cursor:?}, 선택 {got_sel:?}, 노트 {got_notes:?}",
                    cell.expect
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---------- 7. 선택 없으면 커서 노트 하나 ----------

#[gpui_kit::test]
fn alt_operations_without_selection_apply_to_cursor_note(cx: &mut TestAppContext) {
    start(cx, "alt-cursor");
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 1200, 62);
    press(cx, handle, "alt-k");
    let notes = clip_a_notes(cx, &view);
    assert!(
        notes.contains(&(960, 960, 63)) && !notes.contains(&D4),
        "{notes:?}"
    );
    assert_eq!(cursor_of(cx, &view), (1200, 63), "커서가 노트를 따라간다");
    assert!(selection_of(cx, &view).is_empty(), "선택은 생기지 않는다");
    assert!(!has_selection(cx, &view));
    press(cx, handle, "alt-k");
    assert!(clip_a_notes(cx, &view).contains(&(960, 960, 64)));
    assert_eq!(cursor_of(cx, &view), (1200, 64));
    press(cx, handle, "alt-shift-l");
    assert!(clip_a_notes(cx, &view).contains(&(960, 1920, 64)));
    assert_eq!(
        cursor_of(cx, &view),
        (1200, 64),
        "길이는 커서를 옮기지 않는다"
    );
    press(cx, handle, "alt-l");
    assert!(clip_a_notes(cx, &view).contains(&(1920, 1920, 64)));
    assert_eq!(cursor_of(cx, &view), (2160, 64));
    // 다른 노트는 그대로
    let notes = clip_a_notes(cx, &view);
    for n in A_NOTES.iter().filter(|n| **n != D4) {
        assert!(notes.contains(n), "{n:?}");
    }
    // 커서 아래 노트가 없으면 아무 일도 없고 알림만
    set_cursor(cx, &view, 2 * BAR + 100, 40);
    let before = clip_a_notes(cx, &view);
    press(cx, handle, "alt-k");
    assert_eq!(clip_a_notes(cx, &view), before);
    assert_eq!(cursor_of(cx, &view), (2 * BAR + 100, 40));
    let last = cx.read(|cx| view.read(cx).messages().last().cloned());
    assert!(
        last.as_deref().is_some_and(|m| m.contains("노트")),
        "{last:?}"
    );
}

// ---------- 8. 되돌리기 묶음 ----------

#[gpui_kit::test]
fn repeated_same_operation_is_one_undo_and_other_key_breaks_group(cx: &mut TestAppContext) {
    start(cx, "undo-group");
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    select(cx, &view, &[A4]);
    assert_eq!(undo_len(cx, &view), 0);
    for _ in 0..5 {
        press(cx, handle, "alt-shift-l");
    }
    assert!(clip_a_notes(cx, &view).contains(&(7680, 1920 + 5 * 960, 69)));
    assert_eq!(undo_len(cx, &view), 1, "연타는 한 단위");
    press(cx, handle, "cmd-z");
    assert!(
        clip_a_notes(cx, &view).contains(&A4),
        "{:?}",
        clip_a_notes(cx, &view)
    );
    assert_eq!(undo_len(cx, &view), 0);
    // 다른 키가 끼면 끊긴다
    select(cx, &view, &[A4]);
    press(cx, handle, "alt-shift-l");
    press(cx, handle, "l");
    press(cx, handle, "alt-shift-l");
    assert_eq!(undo_len(cx, &view), 2, "사이에 커서 키");
    // 다른 조작은 다른 묶음
    press(cx, handle, "alt-shift-h");
    assert_eq!(undo_len(cx, &view), 3);
    press(cx, handle, "alt-shift-h");
    assert_eq!(undo_len(cx, &view), 3);
    // 바인딩 없는 키도 끊는다
    press(cx, handle, "f19");
    press(cx, handle, "alt-shift-h");
    assert_eq!(undo_len(cx, &view), 4);
    // 메뉴·팔레트 경로(키 입력 아님)는 끊지 않는다
    run(cx, handle, note::Shorten);
    assert_eq!(undo_len(cx, &view), 4);
    // 선택 노트 밀기도 같은 규칙
    press(cx, handle, "alt-k alt-k alt-k");
    assert_eq!(undo_len(cx, &view), 5);
    press(cx, handle, "cmd-z");
    assert_eq!(undo_len(cx, &view), 4);
    assert!(
        clip_a_notes(cx, &view).iter().any(|n| n.2 == 69),
        "{:?}",
        clip_a_notes(cx, &view)
    );
}

// ---------- 9. 노트·클립 단위 이동과 Cmd+화살표 ----------

#[gpui_kit::test]
fn n_p_shift_n_shift_p_cmd_arrows_move_cursor(cx: &mut TestAppContext) {
    start(cx, "n-p");
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    let steps = [
        (960, 62),
        (1920, 64),
        (3840, 60),
        (3840, 64),
        (3840, 67),
        (7680, 69),
        (14400, 65),
        (15360, 67),
    ];
    for want in steps {
        press(cx, handle, "n");
        assert_eq!(cursor_of(cx, &view), want);
    }
    press(cx, handle, "n");
    assert_eq!(cursor_of(cx, &view), (15360, 67), "끝에서는 그대로");
    press(cx, handle, "p");
    assert_eq!(cursor_of(cx, &view), (14400, 65));
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "p");
    assert_eq!(cursor_of(cx, &view), (0, 60));
    // 클립
    press(cx, handle, "shift-n");
    assert_eq!(
        cursor_of(cx, &view),
        (4 * BAR, 60),
        "다음 클립 시작, 음높이 유지"
    );
    press(cx, handle, "shift-n");
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 60));
    press(cx, handle, "shift-p");
    assert_eq!(cursor_of(cx, &view), (0, 60));
    press(cx, handle, "shift-p");
    assert_eq!(cursor_of(cx, &view), (0, 60));
    // 클립 처음·끝
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "cmd-right");
    assert_eq!(cursor_of(cx, &view), (14400, 60), "마지막 스텝 칸");
    press(cx, handle, "cmd-left");
    assert_eq!(cursor_of(cx, &view), (0, 60));
    set_cursor(cx, &view, 4 * BAR + 100, 60);
    press(cx, handle, "cmd-right");
    assert_eq!(cursor_of(cx, &view), (4 * BAR + 3 * QUARTER, 60));
    set_cursor(cx, &view, 7 * BAR, 60);
    press(cx, handle, "cmd-right cmd-left");
    assert_eq!(cursor_of(cx, &view), (7 * BAR, 60), "클립 밖이면 그대로");
    // 첫·마지막 트랙(커서 틱 유지)
    let mut project = fixture();
    for name in ["U", "V"] {
        project
            .push_track(Track::new(
                name,
                Channel::new(2).unwrap(),
                TrackColor::new(1).unwrap(),
            ))
            .unwrap();
    }
    let ids: Vec<TrackId> = project.tracks().iter().map(Track::id).collect();
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "cmd-down");
    assert_eq!(cx.read(|cx| view.read(cx).current_track()), Some(ids[2]));
    assert_eq!(cursor_of(cx, &view), (960, 62));
    press(cx, handle, "cmd-up");
    assert_eq!(cx.read(|cx| view.read(cx).current_track()), Some(ids[0]));
    assert_eq!(cursor_of(cx, &view), (960, 62));
}

// ---------- 10. b / Shift+b / Cmd+A / Esc ----------

#[gpui_kit::test]
fn b_shift_b_cmd_a_select_and_escape_clears(cx: &mut TestAppContext) {
    start(cx, "b-select");
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 3840 + 960, 60);
    press(cx, handle, "b");
    assert_eq!(
        selection_of(cx, &view),
        vec![(3840, 960, 60), (3840, 960, 64), (3840, 960, 67)]
    );
    assert!(has_selection(cx, &view));
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().context_path()),
        &["Normal", "PianoRoll", "Selection"]
    );
    let mut all: Vec<_> = A_NOTES.to_vec();
    all.sort_unstable();
    press(cx, handle, "shift-b");
    assert_eq!(selection_of(cx, &view), all);
    press(cx, handle, "escape");
    assert!(selection_of(cx, &view).is_empty());
    assert!(!has_selection(cx, &view));
    assert_eq!(
        cursor_of(cx, &view),
        (3840 + 960, 60),
        "Esc는 커서를 바꾸지 않는다"
    );
    press(cx, handle, "escape");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll,
        "두 번째 Esc는 아무 일도 없다"
    );
    press(cx, handle, "cmd-a");
    assert_eq!(selection_of(cx, &view), all);
    press(cx, handle, "v");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    assert!(
        selection_of(cx, &view).is_empty(),
        "뷰를 바꾸면 선택을 비운다"
    );
    press(cx, handle, "v");
    set_cursor(cx, &view, 6 * BAR, 60);
    press(cx, handle, "b");
    assert!(
        selection_of(cx, &view).is_empty(),
        "클립 밖에서는 선택이 생기지 않는다"
    );
    assert!(!has_selection(cx, &view));
    // 선택이 있어도 수식키 없는 이동은 선택을 지키고, 다음 Shift 이동은 기준 칸을 새로 잡는다
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "b");
    assert_eq!(selection_of(cx, &view).len(), 3);
    press(cx, handle, "l");
    assert_eq!(
        selection_of(cx, &view).len(),
        3,
        "커서 이동은 선택을 지운다 않는다"
    );
    press(cx, handle, "shift-l");
    assert_eq!(cursor_of(cx, &view), (2880, 62));
    // 넓히기는 좁히지 않는다(리뷰 2 지적 1): [1920, 3840) × 62에 노트가 없어도 마디 선택 3개는 남는다
    assert_eq!(
        selection_of(cx, &view).len(),
        3,
        "[1920, 3840) × 62에는 노트가 없지만 넓히기는 선택을 지우지 않는다"
    );
    assert!(has_selection(cx, &view));
}

// ---------- 11. 이동 시 미리듣기(PRE-1) ----------

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn loopback(name: &str) -> Loopback {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-sel-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let data = p.data();
                let mut i = 0;
                while i < data.len() {
                    let len = match data[i] & 0xF0 {
                        0xC0 | 0xD0 => 2,
                        0xF0 => 1,
                        _ => 3,
                    };
                    let _ = tx.send(data[i..(i + len).min(data.len())].to_vec());
                    i += len;
                }
            }
        })
        .unwrap();
    let deadline = Instant::now() + WAIT;
    let source = loop {
        if let Some(s) = coremidi::Source::from_name(name) {
            break s;
        }
        assert!(
            Instant::now() < deadline,
            "가상 포트 {name}이 보이지 않는다"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    input.connect_source(&source).unwrap();
    Loopback {
        rx,
        _input: input,
        _client: client,
    }
}

impl Loopback {
    fn drain_for(&self, limit: Duration) -> Vec<Vec<u8>> {
        let mut got = Vec::new();
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(m) => got.push(m),
                Err(_) => return got,
            }
        }
    }

    fn wait_for(&self, pred: impl Fn(&[u8]) -> bool) -> Option<Vec<Vec<u8>>> {
        let mut got = Vec::new();
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(m) => {
                    let hit = pred(&m);
                    got.push(m);
                    if hit {
                        return Some(got);
                    }
                }
                Err(_) => return None,
            }
        }
    }
}

fn is_note_on(m: &[u8], pitch: u8) -> bool {
    m.len() == 3 && m[0] & 0xF0 == 0x90 && m[1] == pitch && m[2] > 0
}

fn is_note_off(m: &[u8], pitch: u8) -> bool {
    m.len() == 3 && m[1] == pitch && (m[0] & 0xF0 == 0x80 || (m[0] & 0xF0 == 0x90 && m[2] == 0))
}

fn start_engine(cx: &mut TestAppContext, name: &str) -> Loopback {
    start(cx, name);
    let port = format!("KS sel {} {name}", std::process::id());
    let options = EngineOptions {
        client_name: format!("ks-sel-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    loopback(&port)
}

#[gpui_kit::test]
fn cursor_move_onto_note_sends_preview_on_track_port_channel(cx: &mut TestAppContext) {
    let rx = start_engine(cx, "preview");
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 62);
    let _ = rx.drain_for(Duration::from_millis(100));
    press(cx, handle, "l");
    assert_eq!(cursor_of(cx, &view), (960, 62));
    let got = rx.wait_for(|m| is_note_on(m, 62)).expect("D4 Note On");
    let on = got.last().unwrap();
    assert_eq!(on[0], 0x90, "트랙 채널 1");
    assert_eq!(on[2], 100, "노트의 벨로시티");
    rx.wait_for(|m| is_note_off(m, 62))
        .expect("D4 Note Off(기본 200 ms)");
    press(cx, handle, "l");
    assert_eq!(cursor_of(cx, &view), (1920, 62));
    let quiet = rx.drain_for(Duration::from_millis(300));
    assert!(
        !quiet.iter().any(|m| m[0] & 0xF0 == 0x90 && m[2] > 0),
        "노트 없는 칸: {quiet:?}"
    );
    press(cx, handle, "k k");
    assert_eq!(cursor_of(cx, &view), (1920, 64));
    rx.wait_for(|m| is_note_on(m, 64)).expect("E4 Note On");
    rx.wait_for(|m| is_note_off(m, 64)).expect("E4 Note Off");
    press(cx, handle, "l");
    assert_eq!(cursor_of(cx, &view), (2880, 64));
    let quiet = rx.drain_for(Duration::from_millis(300));
    assert!(
        !quiet.iter().any(|m| m[0] & 0xF0 == 0x90 && m[2] > 0),
        "빈 칸에서는 소리가 없다: {quiet:?}"
    );
    // n 으로 노트 위로 가도 들린다
    press(cx, handle, "n");
    assert_eq!(cursor_of(cx, &view), (3840, 60));
    rx.wait_for(|m| is_note_on(m, 60)).expect("n 뒤 미리듣기");
    let _ = rx.drain_for(Duration::from_millis(300));
    // 설정을 끄면 들리지 않고, note::Audition은 설정과 무관하다
    cx.update(|cx| AppSettings::update(cx, |s| s.preview.on_move = false));
    press(cx, handle, "k");
    assert_eq!(cursor_of(cx, &view), (3840, 61));
    press(cx, handle, "j");
    assert_eq!(cursor_of(cx, &view), (3840, 60));
    let quiet = rx.drain_for(Duration::from_millis(300));
    assert!(
        !quiet.iter().any(|m| m[0] & 0xF0 == 0x90 && m[2] > 0),
        "설정이 꺼지면 소리가 없다: {quiet:?}"
    );
    run(cx, handle, note::Audition);
    rx.wait_for(|m| is_note_on(m, 60)).expect("note::Audition");
    // 길이 설정이 Note Off 시점을 정한다(50 ms)
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.preview.on_move = true;
            s.preview.length_ms = 50;
        });
    });
    let _ = rx.drain_for(Duration::from_millis(300));
    press(cx, handle, "k k k k");
    assert_eq!(cursor_of(cx, &view), (3840, 64));
    let t0 = Instant::now();
    rx.wait_for(|m| is_note_on(m, 64))
        .expect("E4(화음) Note On");
    rx.wait_for(|m| is_note_off(m, 64)).expect("Note Off");
    assert!(
        t0.elapsed() < Duration::from_millis(180),
        "{:?}",
        t0.elapsed()
    );
    run(cx, handle, transport::Stop);
}

// ---------- 16. 마크와 우클릭 메뉴 ----------

#[gpui_kit::test]
fn marks_set_and_jump_and_context_menu_opens(cx: &mut TestAppContext) {
    start(cx, "marks");
    let mut project = fixture();
    project
        .push_track(Track::new(
            "U",
            Channel::new(2).unwrap(),
            TrackColor::new(1).unwrap(),
        ))
        .unwrap();
    let ids: Vec<TrackId> = project.tracks().iter().map(Track::id).collect();
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    run(cx, handle, cursor::SetMark("a".to_owned()));
    press(cx, handle, "l l k");
    press(cx, handle, "cmd-down");
    assert_eq!(cx.read(|cx| view.read(cx).current_track()), Some(ids[1]));
    run(cx, handle, cursor::JumpMark("a".to_owned()));
    assert_eq!(cursor_of(cx, &view), (960, 62));
    assert_eq!(cx.read(|cx| view.read(cx).current_track()), Some(ids[0]));
    run(cx, handle, cursor::JumpMark("q".to_owned()));
    assert_eq!(cursor_of(cx, &view), (960, 62));
    let last = cx.read(|cx| view.read(cx).messages().last().cloned());
    assert!(
        last.as_deref().is_some_and(|m| m.contains("마크")),
        "{last:?}"
    );
    // 우클릭 메뉴
    assert!(!cx.read(|cx| view.read(cx).context_menu_open()));
    cx.update_window(handle, |_, window, cx| window.right_click("main-view", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(cx.read(|cx| view.read(cx).context_menu_open()));
    press(cx, handle, "escape");
    assert!(!cx.read(|cx| view.read(cx).context_menu_open()));
}

// ---------- 17. 선택은 문서를 따라간다 ----------

#[gpui_kit::test]
fn selection_follows_document_changes(cx: &mut TestAppContext) {
    start(cx, "sel-doc");
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    select(cx, &view, &[D4]);
    assert!(has_selection(cx, &view));
    press(cx, handle, "alt-l");
    assert_eq!(selection_of(cx, &view), vec![(1920, 960, 62)]);
    press(cx, handle, "cmd-z");
    assert!(clip_a_notes(cx, &view).contains(&D4));
    // S3-T3a(S3-T2 리뷰 1 제안 8): 되돌리기로 되살아난 원래 노트가 선택이 된다
    assert_eq!(
        selection_of(cx, &view),
        vec![D4],
        "되돌리기 뒤 선택은 되살아난 노트"
    );
    assert!(has_selection(cx, &view));
    // 다른 편집이 선택 노트의 일부만 없애면 남은 것만 남는다
    select(cx, &view, &[D4, A4]);
    let (track, clip_id) = ids(cx, &view);
    view.update(cx, |this, cx| {
        this.document().update(cx, |d, cx| {
            d.apply(vec![Edit::RemoveNote {
                track,
                clip: clip_id,
                note: note(D4.0, D4.1, D4.2),
            }])
            .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(selection_of(cx, &view), vec![A4]);
    assert!(has_selection(cx, &view));
}

// ---------- 18. 커서부터 재생 ----------

#[gpui_kit::test]
fn play_from_cursor_starts_at_piano_roll_cursor(cx: &mut TestAppContext) {
    let _rx = start_engine(cx, "play-cursor");
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    cx.update(|cx| AppSettings::update(cx, |s| s.preview.on_move = false));
    press(cx, handle, "ctrl-l ctrl-l");
    assert_eq!(cursor_of(cx, &view), (2 * BAR, 60));
    press(cx, handle, "shift-space");
    assert_eq!(
        cx.read(|cx| view.read(cx).transport_state()),
        TransportState::Playing
    );
    std::thread::sleep(Duration::from_millis(60));
    render(cx, handle);
    let head = cx
        .read(|cx| view.read(cx).piano_roll_playhead())
        .expect("재생 중");
    assert!(head.get() >= 2 * BAR, "{head:?}");
    assert!(head.get() < 3 * BAR, "{head:?}");
    run(cx, handle, transport::Stop);
}

// ---------- 20. 장면: 선택은 선택한 클립에서만 ----------

#[test]
fn scene_marks_selected_only_in_selected_clip() {
    let track = fixture_track();
    let (a_id, b_id) = {
        let mut it = track.clips();
        (it.next().unwrap().0, it.next().unwrap().0)
    };
    let mut selection = Selection::default();
    selection.set(track.id(), a_id, [note(0, 960, 60)].into_iter().collect());
    let mut cache = MaxLenCache::default();
    cache.refresh(Some(&track), 0);
    let meter = MeterMap::default();
    let vp = keyscore_app::pianoroll::Viewport {
        first_tick: 0.0,
        px_per_tick: 0.1,
        width: 2000.0,
        first_row: keyscore_app::pianoroll::Viewport::row_of(pitch(72)),
        row_height: 12.0,
        height: 400.0,
    };
    let build = |current: ClipId| {
        let mut scene = Scene::default();
        scene::build(
            &SceneInput {
                track: Some(&track),
                current: Some(current),
                cache: &cache,
                meter: &meter,
                key: Key::None,
                cursor: at(0, 60),
                step_ticks: QUARTER,
                selection: &selection,
                playhead: None,
                viewport: vp,
            },
            &mut scene,
        );
        scene
    };
    let scene = build(a_id);
    let c4 = scene.notes.iter().find(|n| n.rect.x == 0.0).unwrap();
    assert!(c4.selected && !c4.dim);
    assert_eq!(scene.notes.iter().filter(|n| n.selected).count(), 1);
    let scene = build(b_id);
    let c4 = scene.notes.iter().find(|n| n.rect.x == 0.0).unwrap();
    assert!(
        !c4.selected && c4.dim,
        "다른 클립이 현재면 선택을 그리지 않는다"
    );
    assert!(selection.contains(a_id, &note(0, 960, 60)));
    assert!(!selection.contains(b_id, &note(0, 960, 60)));
}
