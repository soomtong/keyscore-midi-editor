//! S3-T4a 적대적 리뷰 3회차(docs/reports/s3-t4a/review-3.md). 픽스처는 `review_s3_t4a_r2.rs`에서 옮겨 왔다.
#![allow(dead_code, unused_imports)]

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, LengthCode};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::step::{StepLength, StepStatus};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;
const WAIT: Duration = Duration::from_millis(300);

/// (절대 시작, 길이, 음높이)
type Abs = (u64, u32, u8);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-step-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
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

/// 트랙 하나, 클립들(시작, 길이, 노트(클립 기준)).
fn project(clips: &[(u64, u64, &[Abs])]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    for &(s, l, notes) in clips {
        let mut clip = Clip::new(Tick::new(s), l).unwrap();
        for &(ns, nl, np) in notes {
            clip.insert_note(note(ns, nl, np)).unwrap();
        }
        track.insert_clip(clip).unwrap();
    }
    project.push_track(track).unwrap();
    project
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(handle, key);
        cx.run_until_parked();
        render(cx, handle);
    }
}

fn wait(cx: &mut TestAppContext, handle: AnyWindowHandle, d: Duration) {
    cx.executor().advance_clock(d);
    cx.run_until_parked();
    render(cx, handle);
}

fn event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    e: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| this.apply_mode_event(e, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn set_layout(cx: &mut TestAppContext, handle: AnyWindowHandle, layout: KeyLayout) {
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = layout));
    cx.run_until_parked();
    render(cx, handle);
}

fn set_cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(60).unwrap(),
        });
    });
}

/// 피아노롤 뷰의 Step 모드 창. 커서는 틱 0.
fn setup(
    cx: &mut TestAppContext,
    name: &str,
    layout: KeyLayout,
    p: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    set_layout(cx, handle, layout);
    event(cx, handle, &view, ModeEvent::ToggleView);
    set_cursor(cx, &view, 0);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

/// 트랙 0의 모든 노트(절대 시작), 정렬.
fn notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Abs> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        let mut v: Vec<Abs> = track
            .clips()
            .flat_map(|(_, c)| {
                c.notes().iter().map(move |n| {
                    (
                        c.start().get() + n.start().get(),
                        n.length(),
                        n.pitch().get(),
                    )
                })
            })
            .collect();
        v.sort_unstable();
        v
    })
}

/// 트랙 0의 클립(시작, 길이).
fn clips(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u64)> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .map(|(_, c)| (c.start().get(), c.length()))
            .collect()
    })
}

fn cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> u64 {
    cx.read(|cx| view.read(cx).piano_roll().cursor().tick.get())
}

fn status(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> StepStatus {
    cx.read(|cx| view.read(cx).step_status())
}

fn label(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| view.read(cx).mode_label())
}

fn path(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<&'static str> {
    cx.read(|cx| view.read(cx).context_path().to_vec())
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
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

fn mode(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn len(code: LengthCode, dotted: bool) -> StepLength {
    StepLength { code, dotted }
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl gpui_kit::Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

/// 빈 트랙 둘.
fn two_tracks() -> Project {
    let mut project = Project::new();
    for (name, ch) in [("A", 1), ("B", 2)] {
        let track = Track::new(name, Channel::new(ch).unwrap(), TrackColor::new(0).unwrap());
        project.push_track(track).unwrap();
    }
    project
}

fn current_index(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<usize> {
    cx.read(|cx| {
        let v = view.read(cx);
        let id = v.current_track()?;
        v.document()
            .read(cx)
            .project()
            .tracks()
            .iter()
            .position(|t| t.id() == id)
    })
}

// ---------- 리뷰 3 시도 ----------

/// 지적 2 수정의 결과: 다음 클립 시작을 넘은 쉼표는 다음 클립의 노트를 밀지 않으므로 그 노트들이 쉼표 스텝 안에
/// 놓인다. 바로 `Backspace`(쉼표 스텝 되돌리기)를 치면 쉼표가 건드리지 않은 다음 클립의 노트가 지워진다.
/// 제안 1은 거절했다(보고서 「알려진 한계」, undo로 되돌린다). 이 시험은 지금 동작을 기록한다.
#[gpui_kit::test]
fn r3_rest_crossing_next_clip_then_backspace_deletes_next_clip_notes(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r3-rest-bs",
        KeyLayout::Solfege,
        project(&[
            (0, BAR, &[]),
            (BAR + 960, BAR, &[(0, 960, 67), (960, 960, 69)]),
        ]),
    );
    set_cursor(cx, &view, 2880);
    press(cx, handle, "1 space");
    assert_eq!(cursor(cx, &view), 2880 + BAR);
    assert_eq!(clips(cx, &view), vec![(0, BAR + 960), (BAR + 960, BAR)]);
    let before = notes(cx, &view);
    assert_eq!(before, vec![(4800, 960, 67), (5760, 960, 69)]);
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 2880);
    assert_eq!(
        notes(cx, &view),
        Vec::<(u64, u32, u8)>::new(),
        "알려진 한계: 쉼표 스텝 안의 다음 클립 노트가 지워진다"
    );
    press(cx, handle, "cmd-z");
    assert_eq!(notes(cx, &view), before, "undo로 되돌린다");
}

/// 클립 끝(끝 = 커서)에서 친 쉼표도 다음 클립 시작에서 연장이 잘리고 커서는 쉼표 길이만큼 간다.
#[gpui_kit::test]
fn r3_rest_from_clip_end_cut_at_next_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r3-rest-end-cut",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[]), (BAR + 960, BAR, &[(0, 960, 67)])]),
    );
    set_cursor(cx, &view, BAR);
    press(cx, handle, "1 space");
    assert_eq!(cursor(cx, &view), 2 * BAR);
    assert_eq!(clips(cx, &view), vec![(0, BAR + 960), (BAR + 960, BAR)]);
    assert_eq!(notes(cx, &view), vec![(4800, 960, 67)]);
}

/// 밀린 노트가 다음 클립에 닿는 쉼표는 여전히 거부(결정 1절).
#[gpui_kit::test]
fn r3_rest_pushing_note_into_next_clip_is_refused(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r3-rest-push-refused",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(2880, 960, 60)]), (BAR + 960, BAR, &[])]),
    );
    set_cursor(cx, &view, 1920);
    press(cx, handle, "2 space");
    assert_eq!(cursor(cx, &view), 1920);
    assert_eq!(notes(cx, &view), vec![(2880, 960, 60)]);
    assert_eq!(clips(cx, &view), vec![(0, BAR), (BAR + 960, BAR)]);
}

/// 화음(셋잇단 길이)은 음마다 apply가 하나씩이다. `rekey`가 이어 가므로 `Backspace`는 화음 스텝(640)을 지운다.
#[gpui_kit::test]
fn r3_triplet_chord_backspace_follows_trail(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r3-chord-bs", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "n ctrl-c enter ctrl-e enter ctrl-g enter tab");
    assert_eq!(cursor(cx, &view), 640);
    press(cx, handle, "c enter");
    assert_eq!(cursor(cx, &view), 1600);
    press(cx, handle, "backspace backspace");
    assert_eq!(cursor(cx, &view), 0);
    assert!(notes(cx, &view).is_empty());
}

/// 트랙을 바꾸면 자취를 버린다(결정 1절): 다른 트랙의 `Shift+Tab`은 현재 길이.
#[gpui_kit::test]
fn r3_track_switch_drops_trail(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r3-track", KeyLayout::Solfege, two_tracks());
    press(cx, handle, "n c enter n d enter");
    assert_eq!(cursor(cx, &view), 1280);
    run(cx, handle, keyscore_app::actions::track::Next);
    assert_eq!(current_index(cx, &view), Some(1));
    assert_eq!(mode(cx, &view), Mode::Step);
    press(cx, handle, "shift-tab");
    assert_eq!(cursor(cx, &view), 320);
}

/// 트랙을 바꿨다 돌아오면(편집 없음) 자취가 그대로 쓰인다. 제안 2를 받아 결정 1절 문장을 코드에 맞췄다(바꾼 트랙에서
/// Step 동작을 해야 버린다. 돌아오면 그대로 맞는 기록이다).
#[gpui_kit::test]
fn r3_track_round_trip_keeps_trail(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r3-track-rt", KeyLayout::Solfege, two_tracks());
    press(cx, handle, "n c enter n d enter");
    run(cx, handle, keyscore_app::actions::track::Next);
    run(cx, handle, keyscore_app::actions::track::Prev);
    assert_eq!(current_index(cx, &view), Some(0));
    press(cx, handle, "shift-tab");
    // 자취가 남아 지나온 셋잇단 스텝(d)의 시작 640으로. 자취가 없으면 현재 길이(4분)만큼 320
    assert_eq!(cursor(cx, &view), 640);
}

/// Step 안 undo 뒤 새 음: 자취는 새 음부터. 그 앞은 현재 길이.
#[gpui_kit::test]
fn r3_undo_in_step_then_note_restarts_trail(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r3-undo-note", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "n c enter n d enter cmd-z");
    assert_eq!(cursor(cx, &view), 1280);
    press(cx, handle, "n e enter");
    assert_eq!(cursor(cx, &view), 1920);
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 1280);
    assert_eq!(notes(cx, &view), vec![(0, 640, 60)]);
    press(cx, handle, "shift-tab");
    assert_eq!(cursor(cx, &view), 320);
}
