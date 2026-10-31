//! S3-T6b 적대적 리뷰 2회차 시험. 실패하는 시험은 결함을 보인다(고치지 않는다). 헬퍼는 `review_s3_t6b.rs`에서 복사.
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
    let dir = std::env::temp_dir().join(format!("ks-review-t6b-r2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-review-t6b-r2-{name}", std::process::id()),
        );
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

// ---------- 리뷰 2회차 깨뜨리기 ----------

use keyscore_core::PitchOffset;

/// `project`로 창을 열고 피아노롤에 들어가 가로 배율을 0.05 px/틱으로 줄인다.
fn open_with(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    cx.update(|cx| view.read(cx).piano_roll().zoom(0.5));
    render(cx, handle);
    (handle, view)
}

/// 클립 A [0, 2마디) 하나에 `notes`를 둔 프로젝트.
fn project_with(notes: Vec<Note>) -> Project {
    let mut p = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut c = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    for n in notes {
        c.insert_note(n).unwrap();
    }
    track.insert_clip(c).unwrap();
    p.push_track(track).unwrap();
    p
}

/// 첫 클립의 (가사, 오프셋)을 시작 순으로.
fn lyrics_offsets(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
) -> Vec<(Triple, Option<String>, PitchOffset)> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        let (_, c) = track.clips().next().unwrap();
        c.notes()
            .iter()
            .map(|n| (triple(n), n.lyric().map(str::to_string), n.offset()))
            .collect()
    })
}

/// 커서를 `(tick, pitch)`로 옮겨 그 행이 보이게 한다.
fn show(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    tick: u64,
    pitch: u8,
) {
    cx.update(|cx| {
        let roll = view.read(cx).piano_roll();
        roll.set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(pitch).unwrap(),
        });
        roll.center_cursor();
    });
    render(cx, handle);
}

/// 피치 오프셋이 있는 노트(음높이 115 + 오프셋 +12반음 = 절대 127)를 위로 끌면, 음높이 Δ를 자르는
/// `drag_shift`가 오프셋을 보지 않아 결과가 `plan_shift`에서 절대 피치 범위로 거부된다(결정 6절 5 「모든
/// 노트가 범위 안에 있게 자른다」의 의도와 어긋남). 시간 Δ까지 함께 버려진다.
#[test]
fn review2_drag_shift_with_pitch_offset_is_accepted_by_plan_shift() {
    use keyscore_app::pianoroll::mouse::{DragInput, Grip, drag_shift};
    use keyscore_app::pianoroll::ops::plan_shift;
    let high = note((960, 960, 115))
        .with_offset(PitchOffset::new(PitchOffset::MAX).unwrap())
        .unwrap();
    let p = project_with(vec![high.clone()]);
    let t = &p.tracks()[0];
    let (tid, cid) = (t.id(), t.clips().next().unwrap().0);
    let shift = drag_shift(&DragInput {
        meter: p.meter_map(),
        clip_start: 0,
        notes: std::slice::from_ref(&high),
        grabbed: &high,
        grip: Grip::Body,
        ticks: 960.0,
        semitones: 3,
        grid: 480,
        axis: None,
    });
    let planned = plan_shift(&p, tid, cid, std::slice::from_ref(&high), shift);
    assert!(
        planned.is_ok(),
        "자른 드래그 양이 거부됐다: {shift:?} → {planned:?}"
    );
    let moved = planned.unwrap().unwrap().moved;
    assert_eq!(
        triple(&moved[0].1),
        (1920, 960, 115),
        "시간은 옮기고 음높이는 가장자리에서 멈춘다"
    );
}

/// 같은 결함의 화면 쪽: 고스트가 사라지고(거부 색 고스트조차 없음) 놓으면 편집 없이 거부 문구만 남는다.
#[gpui_kit::test]
fn review2_body_drag_offset_note_stops_at_top_edge(cx: &mut TestAppContext) {
    let high = note((960, 960, 115))
        .with_offset(PitchOffset::new(PitchOffset::MAX).unwrap())
        .unwrap();
    let (handle, view) = open_with(cx, "offset-edge", project_with(vec![high]));
    show(cx, handle, &view, 960, 115);
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, (960, 960, 115));
    let to = moved_by(cx, &view, from, 960, 3);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    let shown = ghosts(cx, &view);
    let blocked = ghost_blocked(cx, &view);
    up(cx, handle, to, none());
    let after = lyrics_offsets(cx, &view);
    let message = last_message(cx, &view);
    assert!(
        !blocked && shown == vec![(1920, 115)],
        "고스트 {shown:?}, 거부 {blocked}, 문구 「{message}」"
    );
    assert_eq!(after[0].0, (1920, 960, 115));
    assert_eq!(after[0].2, PitchOffset::new(PitchOffset::MAX).unwrap());
    assert_eq!(undo_len(cx, &view), undo_before + 1);
}

/// 지적 1 수정 확인: 본체 이동·`Alt` 복제·끝 조절 모두 가사와 피치 오프셋을 지킨다.
#[gpui_kit::test]
fn review2_move_copy_and_resize_keep_lyric_and_offset(cx: &mut TestAppContext) {
    let off = PitchOffset::new(10).unwrap();
    let sung = note(N2).with_lyric("라").with_offset(off).unwrap();
    let (handle, view) = open_with(cx, "keep-all", project_with(vec![sung]));
    // 본체 이동 한 박
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 0);
    drag(cx, handle, from, to, none());
    let moved = (1920, 960, 62);
    assert_eq!(
        lyrics_offsets(cx, &view),
        vec![(moved, Some("라".into()), off)]
    );
    // `Alt` 복제 한 박 뒤로
    let from = mid(cx, handle, &view, moved);
    let to = moved_by(cx, &view, from, 960, 0);
    let alt = mods(false, false, true);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, alt);
    up(cx, handle, to, alt);
    let copy = (2880, 960, 62);
    assert_eq!(
        lyrics_offsets(cx, &view),
        vec![
            (moved, Some("라".into()), off),
            (copy, Some("라".into()), off)
        ]
    );
    // 사본의 오른쪽 끝을 반 박 늘림(끝 영역 6 px 안: 끝에서 20틱 = 1 px)
    let edge = spot(cx, handle, &view, copy.0 + u64::from(copy.1) - 20, copy.2);
    let to = moved_by(cx, &view, edge, 480, 0);
    drag(cx, handle, edge, to, none());
    assert_eq!(
        lyrics_offsets(cx, &view),
        vec![
            (moved, Some("라".into()), off),
            ((2880, 1440, 62), Some("라".into()), off)
        ]
    );
}

/// `Cmd+러버밴드` 중 `cmd-escape`는 러버밴드를 취소하고 누르기 전 선택으로 되돌린다(지적 2 수정의 다른 조작).
#[gpui_kit::test]
fn review2_cmd_escape_during_cmd_band_restores_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmd-band-esc");
    let p1 = mid(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    assert_eq!(selection_of(cx, &view), vec![N1]);
    // N4·N5 위 빈 곳(음높이 72)에서 아래로 끌어 N4·N5를 덮는다
    let a = spot(cx, handle, &view, 3840 - 100, 72);
    let b = spot(cx, handle, &view, 3840 + 500, 58);
    down(cx, handle, a, cmd());
    drag_to(cx, handle, b, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N4, N5]);
    press(cx, handle, "cmd-escape");
    assert!(!gesture_active(cx, &view));
    assert_eq!(selection_of(cx, &view), vec![N1], "누르기 전 선택");
    up(cx, handle, b, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1]);
}

/// 조작이 끝난 뒤 `Esc`는 가로채이지 않고 평소 바인딩(선택 해제)으로 간다.
#[gpui_kit::test]
fn review2_escape_after_drag_reaches_normal_binding(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "esc-after");
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    drag(cx, handle, from, to, none());
    assert_eq!(selection_of(cx, &view), vec![(1920, 960, 60)]);
    press(cx, handle, "escape");
    assert_eq!(selection_of(cx, &view), Vec::<Triple>::new());
}

/// 키맵(`assets/keymaps/default.json` 「Dragging」 컨텍스트)과 Action 목록은 드래그 취소를 `edit::CancelDrag`로
/// 적는데, 그 Action에는 처리기가 없다. 키맵 편집(KM-1·KM-3)으로 다른 키에 묶어도 드래그가 취소되지 않는다.
#[gpui_kit::test]
fn review2_cancel_drag_action_cancels_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cancel-action");
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    assert!(gesture_active(cx, &view));
    run(cx, handle, keyscore_app::actions::edit::CancelDrag);
    let active = gesture_active(cx, &view);
    up(cx, handle, to, none());
    assert!(!active, "edit::CancelDrag가 드래그를 취소해야 한다");
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    assert_eq!(undo_len(cx, &view), undo_before);
}

/// 끄는 중 키 편집(`Alt+k`)은 문서를 바꾸므로 드래그를 취소한다. 뗌은 아무 일도 하지 않는다(결정 6절 11).
#[gpui_kit::test]
fn review2_key_edit_during_drag_cancels_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "key-during");
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    press(cx, handle, "alt-k");
    assert!(!gesture_active(cx, &view));
    up(cx, handle, to, none());
    assert_eq!(a_notes(cx, &view), a_with(&[(N1, (0, 960, 61))]));
    assert_eq!(undo_len(cx, &view), undo_before + 1);
}

/// vim 프리셋에서 시퀀스 대기(`g`) 중 끄는 동안 `Esc`: 첫 `Esc`가 무엇을 취소하든, 두 번째 `Esc`까지 누르면
/// 드래그가 취소돼야 한다(취소할 길이 막히지 않는가).
#[gpui_kit::test]
fn review2_vim_pending_sequence_then_escape_cancels_drag(cx: &mut TestAppContext) {
    use keyscore_app::keymap::{self, Preset};
    start(cx, "vim-pending");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    cx.update(|cx| view.read(cx).piano_roll().zoom(0.5));
    render(cx, handle);
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    press(cx, handle, "g");
    press(cx, handle, "escape");
    let after_first = gesture_active(cx, &view);
    if after_first {
        press(cx, handle, "escape");
    }
    assert!(
        !gesture_active(cx, &view),
        "두 번째 Esc 뒤에도 드래그가 살아 있다"
    );
    up(cx, handle, to, none());
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    assert_eq!(undo_len(cx, &view), undo_before);
    // 첫 Esc가 시퀀스만 취소했는지 기록한다(결함 아님)
    eprintln!("첫 Esc 뒤 드래그 살아 있음: {after_first}");
}

/// 제안 기록: 대체 격자선 규칙(리뷰 1 제안 3 수락)은 「안쪽으로 끌었는데 길어짐」을 대체 경로에서만 막는다.
/// 가장 가까운 격자선 스냅 자체는 안쪽으로 끈 끝을 바깥 격자선으로 보낼 수 있다(지금 동작 기록, 통과).
#[test]
fn review2_inward_edge_drags_can_still_lengthen_by_nearest_snap() {
    use keyscore_app::pianoroll::mouse::{DragInput, Grip, drag_shift};
    use keyscore_core::MeterMap;
    let meter = MeterMap::default();
    let input = |n: &Note, grip, ticks| {
        drag_shift(&DragInput {
            meter: &meter,
            clip_start: 0,
            notes: std::slice::from_ref(n),
            grabbed: n,
            grip,
            ticks,
            semitones: 0,
            grid: 480,
            axis: None,
        })
    };
    // [0, 300)의 오른쪽 끝을 10틱 왼쪽으로: 290 → 가장 가까운 격자선 480, 끝 Δ +180
    let a = note((0, 300, 60));
    assert_eq!(input(&a, Grip::End, -10.0).end, 180);
    // [10, 110)의 왼쪽 끝을 50틱 오른쪽으로: 60 → 격자선 0, 시작 Δ −10(왼쪽으로 늘어남)
    let b = note((10, 100, 60));
    assert_eq!(input(&b, Grip::Start, 50.0).start, -10);
    // 대체 경로(리뷰 1 제안 3): 시작 이상으로 끈 왼쪽 끝은 원래 시작보다 앞으로 가지 않는다
    assert_eq!(input(&b, Grip::Start, 300.0).start, 0);
}
