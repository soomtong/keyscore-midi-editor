//! S3-T6b 적대적 리뷰 3회차 시험. 실패하는 시험은 결함을 보인다(고치지 않는다). 헬퍼는 `review_s3_t6b_r2.rs`에서 복사.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
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
    let dir = std::env::temp_dir().join(format!("ks-review-t6b-r3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-review-t6b-r3-{name}", std::process::id()),
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

// ---------- 리뷰 3회차 깨뜨리기 ----------

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

use keyscore_app::pianoroll::mouse::{Axis, DragInput, Grip, drag_shift};
use keyscore_app::pianoroll::ops::plan_shift;

const S: i32 = PitchOffset::SEMITONE;

/// 노트에 오프셋을 붙인다. 절대 범위 밖이면 `None`.
fn with_off(n: Triple, units: i32) -> Option<Note> {
    note(n).with_offset(PitchOffset::new(units).ok()?).ok()
}

fn body_shift(
    meter: &keyscore_core::MeterMap,
    notes: &[Note],
    semitones: i32,
    axis: Option<Axis>,
) -> i32 {
    drag_shift(&DragInput {
        meter,
        clip_start: 0,
        notes,
        grabbed: &notes[0],
        grip: Grip::Body,
        ticks: 0.0,
        semitones,
        grid: 480,
        axis,
    })
    .pitch
}

/// 음높이 `p`(Δ 적용 뒤)가 노트에 받아들여지는가(`Note::with_pitch`의 검사, 키 조작과 같은 검사).
fn pitch_ok(n: &Note, delta: i32) -> bool {
    let p = i32::from(n.pitch().get()) + delta;
    u8::try_from(p)
        .ok()
        .and_then(|p| Pitch::new(p).ok())
        .is_some_and(|p| n.with_pitch(p).is_ok())
}

/// 지적 1 수정(`pitch_room`) 확인: 모든 음높이 0–127과 경계 오프셋(음수·0·양수, ±1단위, ±1반음, ±12반음)에서
/// 위·아래로 멀리 끈 자르기 결과가 (1) 받아들여지고 (2) 한 반음 더 가면 거부되는 최대값이며 (3) Δ 0 요청은 0이다.
#[test]
fn review3_pitch_room_clamp_is_valid_and_maximal_for_all_offsets() {
    let meter = keyscore_core::MeterMap::default();
    let offsets = [
        -PitchOffset::MAX,
        -S - 1,
        -S,
        -S + 1,
        -S / 2,
        -1,
        0,
        1,
        S / 2,
        S - 1,
        S,
        S + 1,
        PitchOffset::MAX,
    ];
    let mut checked = 0;
    for pitch in 0..=127u8 {
        for &units in &offsets {
            let Some(n) = with_off((0, 480, pitch), units) else {
                continue;
            };
            let notes = std::slice::from_ref(&n);
            assert_eq!(
                body_shift(&meter, notes, 0, None),
                0,
                "Δ 0: {pitch} {units}"
            );
            let up = body_shift(&meter, notes, 300, None);
            let down = body_shift(&meter, notes, -300, None);
            assert!(up >= 0 && down <= 0, "{pitch} {units}: {down}..{up}");
            assert!(
                pitch_ok(&n, up) && !pitch_ok(&n, up + 1),
                "위 {pitch} {units}: {up}"
            );
            assert!(
                pitch_ok(&n, down) && !pitch_ok(&n, down - 1),
                "아래 {pitch} {units}: {down}"
            );
            // 요청이 범위 안이면 그대로
            for k in [down, i32::midpoint(down, up), up] {
                assert_eq!(body_shift(&meter, notes, k, None), k, "{pitch} {units} {k}");
            }
            assert_eq!(body_shift(&meter, notes, 300, Some(Axis::Time)), 0);
            checked += 1;
        }
    }
    assert!(checked > 1500, "검사한 조합 {checked}");
}

/// 여러 노트의 자르기는 노트마다 범위의 교집합이고 `plan_shift`가 그 끝값을 받아들인다(키와 같은 계산).
#[test]
fn review3_pitch_room_intersects_notes_and_plan_shift_accepts_edges() {
    let a = with_off((0, 480, 100), PitchOffset::MAX).unwrap(); // 절대 112: 위로 15, 아래로 100
    let b = with_off((960, 480, 110), -PitchOffset::MAX).unwrap(); // 절대 98: 위로 17, 아래로 98
    let c = with_off((1920, 480, 64), -S / 2).unwrap(); // 절대 63.5: 아래로 63
    let notes = vec![a, b, c];
    let p = project_with(notes.clone());
    let t = &p.tracks()[0];
    let (tid, cid) = (t.id(), t.clips().next().unwrap().0);
    let meter = p.meter_map();
    for (ask, want) in [(300, 15), (-300, -63), (15, 15), (-63, -63)] {
        let got = body_shift(meter, &notes, ask, None);
        assert_eq!(got, want, "요청 {ask}");
        let shift = drag_shift(&DragInput {
            meter,
            clip_start: 0,
            notes: &notes,
            grabbed: &notes[0],
            grip: Grip::Body,
            ticks: 0.0,
            semitones: ask,
            grid: 480,
            axis: None,
        });
        let planned = plan_shift(&p, tid, cid, &notes, shift);
        assert!(
            matches!(planned, Ok(Some(_))),
            "요청 {ask}: {shift:?} → {planned:?}"
        );
    }
}

/// 음수 오프셋 노트(음높이 12 − 12반음 = 절대 0)를 아래로 끌면 아래 가장자리에서 멈추고 시간은 옮긴다
/// (2회차 시험은 위쪽만 봤다).
#[gpui_kit::test]
fn review3_body_drag_negative_offset_note_stops_at_bottom_edge(cx: &mut TestAppContext) {
    let low = with_off((960, 960, 12), -PitchOffset::MAX).unwrap();
    let (handle, view) = open_with(cx, "offset-bottom", project_with(vec![low]));
    show(cx, handle, &view, 960, 12);
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, (960, 960, 12));
    let to = moved_by(cx, &view, from, 960, -3);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    let shown = ghosts(cx, &view);
    let blocked = ghost_blocked(cx, &view);
    up(cx, handle, to, none());
    assert!(
        !blocked && shown == vec![(1920, 12)],
        "고스트 {shown:?}, 거부 {blocked}"
    );
    let after = lyrics_offsets(cx, &view);
    assert_eq!(after[0].0, (1920, 960, 12));
    assert_eq!(after[0].2, PitchOffset::new(-PitchOffset::MAX).unwrap());
    assert_eq!(undo_len(cx, &view), undo_before + 1);
}

/// `Alt` 복제도 같은 자르기를 쓴다: 오프셋 노트의 사본이 위 가장자리에서 멈추고 오프셋을 지킨다.
#[gpui_kit::test]
fn review3_alt_copy_offset_note_stops_at_top_edge(cx: &mut TestAppContext) {
    let high = with_off((960, 960, 115), PitchOffset::MAX).unwrap();
    let (handle, view) = open_with(cx, "offset-copy", project_with(vec![high]));
    show(cx, handle, &view, 960, 115);
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, (960, 960, 115));
    let to = moved_by(cx, &view, from, 960, 2);
    let alt = mods(false, false, true);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, alt);
    let blocked = ghost_blocked(cx, &view);
    up(cx, handle, to, alt);
    assert!(!blocked);
    let max = PitchOffset::new(PitchOffset::MAX).unwrap();
    let after: Vec<_> = lyrics_offsets(cx, &view)
        .into_iter()
        .map(|(t, _, o)| (t, o))
        .collect();
    assert_eq!(after, vec![((960, 960, 115), max), ((1920, 960, 115), max)]);
    assert_eq!(undo_len(cx, &view), undo_before + 1);
}

/// 사용자 키맵 파일로 드래그 취소를 `f9`에 옮기면(키맵 편집기의 `bind`가 쓰는 꼴: 새 키 + 프리셋 키 `null`)
/// `escape`는 더 취소하지 않고 `f9`가 취소한다(KM-1·KM-3, 2회차 지적 2 수정 확인).
#[gpui_kit::test]
fn review3_user_keymap_file_moves_cancel_key(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "user-rebind");
    cx.update(|cx| {
        let path = keyscore_app::keymap::user_path(cx);
        std::fs::write(
            &path,
            r#"[{"context": "Dragging", "bindings": {"f9": "edit::CancelDrag", "escape": null}}]"#,
        )
        .unwrap();
        keyscore_app::keymap::apply(cx);
        assert!(
            keyscore_app::keymap::errors(cx).is_empty(),
            "{:?}",
            keyscore_app::keymap::errors(cx)
        );
    });
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N2);
    let to = moved_by(cx, &view, from, 960, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    press(cx, handle, "escape");
    let after_escape = gesture_active(cx, &view);
    press(cx, handle, "shift-f9");
    let after_f9 = gesture_active(cx, &view);
    up(cx, handle, to, none());
    assert!(after_escape, "지운 escape가 여전히 드래그를 취소한다");
    assert!(!after_f9, "바꿔 묶은 f9(Shift를 쥔 채)가 취소하지 않는다");
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    assert_eq!(undo_len(cx, &view), undo_before);
}

/// 수식키를 뺀 모양 맞추기가 조작이 없을 때 다른 바인딩을 가로채지 않는다: `cmd-escape`·`f9`에 다른 Action을
/// 묶으면 조작이 없을 때 그 Action이 실행된다.
#[gpui_kit::test]
fn review3_bare_match_does_not_steal_keys_without_gesture(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "no-steal");
    cx.update(|cx| {
        cx.bind_keys([
            gpui_kit::KeyBinding::new(
                "f9",
                keyscore_app::actions::edit::CancelDrag,
                Some("Dragging"),
            ),
            gpui_kit::KeyBinding::new("cmd-escape", keyscore_app::actions::select::Clear, None),
            gpui_kit::KeyBinding::new("alt-f9", keyscore_app::actions::select::Clear, None),
        ]);
    });
    let p1 = mid(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    assert_eq!(selection_of(cx, &view), vec![N1]);
    press(cx, handle, "cmd-escape");
    assert_eq!(
        selection_of(cx, &view),
        Vec::<Triple>::new(),
        "cmd-escape 바인딩"
    );
    click(cx, handle, p1, none());
    press(cx, handle, "alt-f9");
    assert_eq!(
        selection_of(cx, &view),
        Vec::<Triple>::new(),
        "alt-f9 바인딩"
    );
}

/// 바꿔 묶은 취소 키는 `Cmd+러버밴드`도 취소하고 누르기 전 선택으로 되돌린다.
#[gpui_kit::test]
fn review3_rebound_key_cancels_band_and_restores_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rebound-band");
    cx.update(|cx| {
        cx.bind_keys([gpui_kit::KeyBinding::new(
            "f9",
            keyscore_app::actions::edit::CancelDrag,
            Some("Dragging"),
        )]);
    });
    let p1 = mid(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let a = spot(cx, handle, &view, 3840 - 100, 72);
    let b = spot(cx, handle, &view, 3840 + 500, 58);
    down(cx, handle, a, cmd());
    drag_to(cx, handle, b, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N4, N5]);
    press(cx, handle, "cmd-f9");
    assert!(!gesture_active(cx, &view));
    assert_eq!(selection_of(cx, &view), vec![N1]);
    up(cx, handle, b, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1]);
}

/// `edit::CancelDrag` 디스패치(팔레트 경로)는 러버밴드에서도 누르기 전 선택으로 되돌린다.
#[gpui_kit::test]
fn review3_cancel_drag_action_during_band_restores_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "action-band");
    let p1 = mid(cx, handle, &view, N1);
    click(cx, handle, p1, none());
    let a = spot(cx, handle, &view, 3840 - 100, 72);
    let b = spot(cx, handle, &view, 3840 + 500, 58);
    down(cx, handle, a, cmd());
    drag_to(cx, handle, b, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1, N4, N5]);
    run(cx, handle, keyscore_app::actions::edit::CancelDrag);
    assert!(!gesture_active(cx, &view));
    assert_eq!(selection_of(cx, &view), vec![N1]);
    up(cx, handle, b, cmd());
    assert_eq!(selection_of(cx, &view), vec![N1]);
}

/// 조작이 없을 때 `edit::CancelDrag`는 해가 없다(선택·커서·문서·undo·상태줄 그대로). 그 뒤 새 드래그도
/// 평소대로 된다(취소 상태가 남지 않는다).
#[gpui_kit::test]
fn review3_cancel_drag_action_without_gesture_is_harmless(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "action-idle");
    select_three(cx, handle, &view);
    let cursor = cursor_of(cx, &view);
    let undo_before = undo_len(cx, &view);
    let message = last_message(cx, &view);
    run(cx, handle, keyscore_app::actions::edit::CancelDrag);
    assert_eq!(selection_of(cx, &view), vec![N1, N2, N3]);
    assert_eq!(cursor_of(cx, &view), cursor);
    assert_eq!(undo_len(cx, &view), undo_before);
    assert_eq!(last_message(cx, &view), message);
    assert!(!gesture_active(cx, &view));
    // 새 드래그: 세 노트를 한 박 뒤로, undo 하나
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 960, 0);
    drag(cx, handle, from, to, none());
    assert_eq!(
        a_notes(cx, &view),
        a_with(&[
            (N3, (2880, 480, 64)),
            (N2, (1920, 960, 62)),
            (N1, (960, 960, 60)),
        ])
    );
    assert_eq!(undo_len(cx, &view), undo_before + 1);
}

/// 액션 취소 뒤 떼고 다시 끌면 평소대로 옮겨진다(`Gesture::Cancelled`가 다음 누름을 막지 않는다).
#[gpui_kit::test]
fn review3_drag_after_action_cancel_works(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "action-again");
    let undo_before = undo_len(cx, &view);
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    run(cx, handle, keyscore_app::actions::edit::CancelDrag);
    up(cx, handle, to, none());
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
    drag(cx, handle, from, to, none());
    assert_eq!(a_notes(cx, &view), a_with(&[(N1, (1920, 960, 60))]));
    assert_eq!(undo_len(cx, &view), undo_before + 1);
}

/// 기록(제안): 취소 키 맞추기는 바인딩의 컨텍스트를 보지 않는다. 다른 컨텍스트(「Step」)에 묶은
/// `edit::CancelDrag` 키도 Normal 피아노롤의 드래그를 취소한다(지금 동작, 통과).
#[gpui_kit::test]
fn review3_cancel_key_in_other_context_still_cancels_drag(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "other-context");
    cx.update(|cx| {
        cx.bind_keys([gpui_kit::KeyBinding::new(
            "f8",
            keyscore_app::actions::edit::CancelDrag,
            Some("Step"),
        )]);
    });
    let from = mid(cx, handle, &view, N1);
    let to = moved_by(cx, &view, from, 1920, 0);
    down(cx, handle, from, none());
    drag_to(cx, handle, to, none());
    press(cx, handle, "f8");
    let active = gesture_active(cx, &view);
    up(cx, handle, to, none());
    assert!(!active, "다른 컨텍스트의 키도 취소한다(기록)");
    assert_eq!(a_notes(cx, &view), sorted(A_NOTES.to_vec()));
}
