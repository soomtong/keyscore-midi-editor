//! S3-T6b 적대적 리뷰 1회차 시험. 실패하는 시험은 결함을 보인다(고치지 않는다).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    dead_code,
    unused_imports
)]

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Capslock, Entity, Modifiers, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
};
use keyscore_app::actions::args::{LengthCode, MouseSnap, Opt, SettingsSection};
use keyscore_app::actions::settings;
use keyscore_app::app_state::AppSettings;
use keyscore_app::clipboard;
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::{Cursor, Viewport};
use keyscore_app::settings_window;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

type Triple = (u64, u32, u8);

// 클립 A [0, 2마디), 틈 [2, 3마디), 클립 B [3, 4마디). 클립 기준 시작.
const N1: Triple = (0, 960, 60);
const N2: Triple = (960, 960, 62);
const N3: Triple = (1920, 480, 64);
const N4: Triple = (3840, 960, 60);
const N5: Triple = (3840, 960, 67);
const N6: Triple = (5760, 960, 65);
/// 좁은 노트: 0.05 px/틱에서 6 px라 끝 영역이 2 px다.
const N7: Triple = (6720, 120, 70);
const A_NOTES: [Triple; 7] = [N1, N2, N3, N4, N5, N6, N7];
const B_START: u64 = 3 * BAR;
const B1: Triple = (0, 960, 60);
const B2: Triple = (960, 960, 64);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-t6b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(cx, &format!("ks-test-{}-review-t6b-{name}", std::process::id()));
        // 스냅 격자(길이 코드): 8분. 미리듣기는 MIDI 없이 돈다.
        AppSettings::update(cx, |s| s.input.step_default_length = LengthCode::Eighth);
    });
}

fn note((start, len, pitch): Triple) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn clip(start: u64, length: u64, notes: &[Triple]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(note(*n)).unwrap();
    }
    clip
}

fn project() -> Project {
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip(0, 2 * BAR, &A_NOTES)).unwrap();
    track.insert_clip(clip(B_START, BAR, &[B1, B2])).unwrap();
    project.push_track(track).unwrap();
    project
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

/// 피아노롤에 들어가 가로 배율을 0.05 px/틱(4분 = 48 px, 8분 = 24 px)으로 줄인다.
fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    cx.update(|cx| view.read(cx).piano_roll().zoom(0.5));
    render(cx, handle);
    let vp = viewport(cx, &view);
    assert!((vp.px_per_tick - 0.05).abs() < 1e-9, "{vp:?}");
    assert!(vp.width >= 800.0, "창이 좁다: {vp:?}");
    (handle, view)
}

fn viewport(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Viewport {
    cx.read(|cx| view.read(cx).piano_roll().viewport())
}

fn main_view_bounds(cx: &mut TestAppContext, handle: AnyWindowHandle) -> gpui_kit::Bounds<Pixels> {
    cx.update_window(handle, |_, window, _| window.find("main-view").bounds())
        .unwrap()
}

/// 절대 틱·음높이 행 가운데의 창 좌표.
fn spot(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    tick: u64,
    pitch: u8,
) -> Point<Pixels> {
    let vp = viewport(cx, view);
    let x = ((tick as f64 - vp.first_tick) * vp.px_per_tick) as f32;
    let row = Viewport::row_of(Pitch::new(pitch).unwrap());
    let y = (row - vp.first_row + 0.5) * vp.row_height;
    assert!(
        (0.0..vp.width).contains(&x) && (0.0..vp.height).contains(&y),
        "뷰포트 밖: {tick} {pitch} {vp:?}"
    );
    main_view_bounds(cx, handle).origin + point(px(x), px(y))
}

/// 노트 가운데(본체).
fn mid(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    n: Triple,
) -> Point<Pixels> {
    spot(cx, handle, view, n.0 + u64::from(n.1) / 2, n.2)
}

/// 같은 행에서 `dt`틱, `dp`반음 떨어진 자리.
fn moved_by(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    from: Point<Pixels>,
    dt: i64,
    dp: i32,
) -> Point<Pixels> {
    let vp = viewport(cx, view);
    from + point(
        px((dt as f64 * vp.px_per_tick) as f32),
        px(-(dp as f32) * vp.row_height),
    )
}

fn mods(platform: bool, shift: bool, alt: bool) -> Modifiers {
    Modifiers {
        platform,
        shift,
        alt,
        ..Modifiers::default()
    }
}

fn none() -> Modifiers {
    Modifiers::default()
}

fn cmd() -> Modifiers {
    mods(true, false, false)
}

fn down(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, modifiers: Modifiers) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers,
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn drag_to(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    modifiers: Modifiers,
) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: Some(MouseButton::Left),
        modifiers,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn up(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>, modifiers: Modifiers) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers,
        click_count: 1,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn click(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: Point<Pixels>,
    modifiers: Modifiers,
) {
    down(cx, handle, at, modifiers);
    up(cx, handle, at, modifiers);
}

/// 누르고, 문턱을 넘겨 `to`까지 끌고, 뗀다.
fn drag(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    from: Point<Pixels>,
    to: Point<Pixels>,
    modifiers: Modifiers,
) {
    down(cx, handle, from, none());
    drag_to(cx, handle, to, modifiers);
    up(cx, handle, to, modifiers);
}

fn modifiers_changed(cx: &mut TestAppContext, handle: AnyWindowHandle, modifiers: Modifiers) {
    VisualTestContext::from_window(handle, cx).simulate_event(ModifiersChangedEvent {
        modifiers,
        capslock: Capslock::default(),
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn triple(n: &Note) -> Triple {
    (n.start().get(), n.length(), n.pitch().get())
}

fn selection_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    cx.read(|cx| {
        let s = view.read(cx).selection();
        let mut v: Vec<_> = s.notes().map(triple).collect();
        v.sort_unstable();
        v
    })
}

fn sorted(mut v: Vec<Triple>) -> Vec<Triple> {
    v.sort_unstable();
    v
}

fn cursor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Cursor {
    cx.read(|cx| view.read(cx).piano_roll().cursor())
}

/// 클립 `index`의 (시작, 길이, 노트).
fn clip_of(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    index: usize,
) -> (u64, u64, Vec<Triple>) {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        let (_, c) = track.clips().nth(index).unwrap();
        (
            c.start().get(),
            c.length(),
            c.notes().iter().map(triple).collect(),
        )
    })
}

fn a_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    sorted(clip_of(cx, view, 0).2)
}

/// `from`의 노트를 `to`로 바꾼 클립 A 노트(정렬).
fn a_with(changes: &[(Triple, Triple)]) -> Vec<Triple> {
    let mut v: Vec<Triple> = A_NOTES.to_vec();
    for (from, to) in changes {
        let i = v.iter().position(|n| n == from).unwrap();
        v[i] = *to;
    }
    sorted(v)
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

fn gesture_active(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).piano_roll().gesture_active())
}

/// 고스트 사각형마다 (절대 틱, 음높이). 사각형 왼쪽 위를 내용 위치로 되돌린다.
fn ghosts(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u8)> {
    let vp = viewport(cx, view);
    let rects = cx.read(|cx| view.read(cx).piano_roll().ghost_rects());
    let mut v: Vec<(u64, u8)> = rects
        .iter()
        .map(|r| {
            let tick = (vp.first_tick + f64::from(r.x) / vp.px_per_tick).round() as u64;
            let row = (vp.first_row + r.y / vp.row_height).round() as u8;
            (tick, 127 - row)
        })
        .collect();
    v.sort_unstable();
    v
}

fn ghost_blocked(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).piano_roll().ghost_blocked())
}

fn last_message(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .last()
            .map(ToString::to_string)
            .unwrap_or_default()
    })
}

/// N1·N2·N3을 클릭과 `Cmd+클릭`으로 고른다.
fn select_three(cx: &mut TestAppContext, handle: AnyWindowHandle, view: &Entity<ProjectWindow>) {
    let p1 = mid(cx, handle, view, N1);
    click(cx, handle, p1, none());
    let p2 = mid(cx, handle, view, N2);
    click(cx, handle, p2, cmd());
    let p3 = mid(cx, handle, view, N3);
    click(cx, handle, p3, cmd());
    assert_eq!(selection_of(cx, view), vec![N1, N2, N3]);
}

// ---------- 본체 이동 ----------

// ---------- 리뷰 1회차 깨뜨리기 ----------

/// 지적 1: 5.7 키 `Alt` 조작이 노트의 가사(LYR-1)·피치 오프셋·모르는 키를 버린다. `shift_note`가 `Note::new`로
/// 새 노트를 만들기 때문이다(main의 `plan_note_op`은 `with_start`·`with_pitch`·`with_length`로 보존했다).
#[test]
fn review_key_alt_ops_keep_lyric_and_pitch_offset() {
    use keyscore_app::pianoroll::ops::{NoteOp, plan_note_op};
    use keyscore_core::PitchOffset;
    let sung = note((960, 960, 62))
        .with_lyric("라")
        .with_offset(PitchOffset::new(10).unwrap())
        .unwrap();
    let mut p = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut c = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    c.insert_note(sung.clone()).unwrap();
    track.insert_clip(c).unwrap();
    p.push_track(track).unwrap();
    let t = &p.tracks()[0];
    let (tid, cid) = (t.id(), t.clips().next().unwrap().0);
    for op in NoteOp::ALL {
        let planned = plan_note_op(&p, tid, cid, std::slice::from_ref(&sung), op, 480)
            .unwrap()
            .unwrap();
        let to = &planned.moved[0].1;
        assert_eq!(to.lyric(), Some("라"), "{op:?}: 가사가 사라졌다");
        assert_eq!(to.offset(), sung.offset(), "{op:?}: 피치 오프셋이 사라졌다");
    }
}

/// 지적 1(마우스 쪽): 가사가 붙은 노트를 끌어 옮기면 가사가 사라진다.
#[gpui_kit::test]
fn review_body_drag_keeps_lyric(cx: &mut TestAppContext) {
    start(cx, "lyric");
    let mut p = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut c = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    c.insert_note(note(N2).with_lyric("라")).unwrap();
    track.insert_clip(c).unwrap();
    p.push_track(track).unwrap();
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    cx.update(|cx| view.read(cx).piano_roll().zoom(0.5));
    render(cx, handle);
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 0);
    drag(cx, handle, from, to, none());
    let lyrics: Vec<Option<String>> = cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        let (_, c) = track.clips().next().unwrap();
        c.notes()
            .iter()
            .map(|n| n.lyric().map(str::to_string))
            .collect()
    });
    assert_eq!(clip_of(cx, &view, 0).2, vec![(1920, 960, 62)], "옮겨졌다");
    assert_eq!(lyrics, vec![Some("라".to_string())], "가사가 남아야 한다");
}

/// 지적 2: 수식키를 누른 채(`Alt` 복제·`Cmd` 자유·`Shift` 한 축) 끄는 중 `Esc`를 누르면 취소되지 않는다.
/// `Esc` 가로채기가 수식키 없는 `escape`만 받는다.
#[gpui_kit::test]
fn review_escape_with_alt_held_cancels_copy_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "alt-esc");
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    let alt = mods(false, false, true);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, alt);
    assert_eq!(ghosts(cx, &view), vec![(1920, 60)]);
    press(cx, handle, "alt-escape");
    assert!(!gesture_active(cx, &view), "Esc가 조작을 끝내야 한다");
    up(cx, handle, to, alt);
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()), "원위치");
    assert_eq!(undo_len(cx, &view), undo_before);
}

#[gpui_kit::test]
fn review_escape_with_shift_held_cancels_axis_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "shift-esc");
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 0);
    let shift = mods(false, true, false);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, shift);
    press(cx, handle, "shift-escape");
    up(cx, handle, to, shift);
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()), "원위치");
    assert_eq!(undo_len(cx, &view), undo_before);
}

/// 키(`Cmd+A`)로 만든 선택을 마우스로 끈다(7.4 「`v`로 고른 뒤 마우스로 끌어도 된다」).
#[gpui_kit::test]
fn review_key_selection_then_drag_moves_all(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmd-a");
    let p2 = mid(cx, handle, &view, N2);
    click(cx, handle, p2, none());
    press(cx, handle, "cmd-a");
    assert_eq!(selection_of(cx, &view), sorted(A_NOTES.to_vec()));
    let undo_before = undo_len(cx, &view);
    let to = moved_by(cx, &view, p2, 0, 1);
    drag(cx, handle, p2, to, none());
    let up1: Vec<Triple> = A_NOTES.iter().map(|&(s, l, p)| (s, l, p + 1)).collect();
    assert_eq!(a_notes(cx, &view), sorted(up1));
    assert_eq!(undo_len(cx, &view), undo_before + 1);
}

/// 끌었다가 누른 자리로 돌아와 놓으면 편집이 없다.
#[gpui_kit::test]
fn review_drag_back_to_origin_makes_no_edit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "back");
    select_three(cx, handle, &view);
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N2);
    let away = moved_by(cx, &view, from, 960, 2);
    down(cx, handle, from, none());
    drag_to(cx, handle, away, none());
    drag_to(cx, handle, from, mods(false, false, true));
    assert!(ghosts(cx, &view).is_empty(), "항등이면 고스트 없음");
    up(cx, handle, from, mods(false, false, true));
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    assert_eq!(undo_len(cx, &view), undo_before);
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
}

/// 끄는 중 `Alt`를 눌렀다 떼고 놓으면 복제가 아니라 이동이다(놓는 순간의 수식키, 결정 6절 8).
#[gpui_kit::test]
fn review_alt_released_before_drop_moves(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "alt-off");
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    modifiers_changed(cx, handle, mods(false, false, true));
    modifiers_changed(cx, handle, none());
    up(cx, handle, to, none());
    assert_eq!(a_notes(cx, &view), a_with(&[(N1, (1920, 960, 60))]));
}

/// 지적 3(제안): 격자에 있지 않은 짧은 노트의 오른쪽 끝을 왼쪽으로 끌면 노트가 오히려 길어진다
/// (결정 6절 3 「시작 뒤 첫 격자선」의 글자 그대로). 지금 동작을 기록한다.
#[test]
fn review_end_grip_left_drag_on_short_offgrid_note_lengthens() {
    use keyscore_app::pianoroll::mouse::{DragInput, Grip, drag_shift};
    use keyscore_core::MeterMap;
    let meter = MeterMap::default();
    let short = note((10, 100, 60)); // [10, 110), 8분 격자 480
    let shift = drag_shift(&DragInput {
        meter: &meter,
        clip_start: 0,
        notes: std::slice::from_ref(&short),
        grabbed: &short,
        grip: Grip::End,
        ticks: -20.0,
        semitones: 0,
        grid: 480,
        axis: None,
    });
    // 끝을 20틱 왼쪽으로 끌었는데 끝 Δ가 +370(끝 480)이다
    assert_eq!(shift.end, 370);
}
