//! S3-T4b 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s3-t4b/review-1.md`). 도우미는
//! `tests/step_vim_mml.rs`에서 옮겼다.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, LengthCode};
use keyscore_app::actions::{ai, edit, mode as mode_action, step};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::step::{StepLength, StepStatus};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const BAR: u64 = 960 * 4;
const WAIT: Duration = Duration::from_millis(300);

/// (절대 시작, 길이, 음높이)
type Abs = (u64, u32, u8);

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-t4b-{}-{name}", std::process::id()));
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
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

fn set_cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64, pitch: u8) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(pitch).unwrap(),
        });
    });
}

fn setup(
    cx: &mut TestAppContext,
    name: &str,
    preset: Preset,
    layout: KeyLayout,
    p: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.input.key_layout = layout;
            s.preview.on_move = false;
        });
        keymap::set_preset(cx, preset);
    });
    cx.run_until_parked();
    render(cx, handle);
    event(cx, handle, &view, ModeEvent::ToggleView);
    set_cursor(cx, &view, 0, 60);
    render(cx, handle);
    (handle, view)
}

fn setup_step(
    cx: &mut TestAppContext,
    name: &str,
    preset: Preset,
    layout: KeyLayout,
    p: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = setup(cx, name, preset, layout, p);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

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

fn set_layout(cx: &mut TestAppContext, handle: AnyWindowHandle, layout: KeyLayout) {
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = layout));
    cx.run_until_parked();
    render(cx, handle);
}

// ---------- 덮어쓰기 ----------

/// 결정 2절: 덮어쓰기 `Shift+Backspace`는 지우기만 하고 당기지 않는다. undo 한 번이 그 지움만 되살린다.
#[gpui_kit::test]
fn review_overwrite_shift_backspace_does_not_pull(cx: &mut TestAppContext) {
    let original: &[Abs] = &[
        (0, 960, 60),
        (960, 960, 62),
        (1920, 960, 64),
        (2880, 960, 65),
    ];
    let (handle, view) = setup(
        cx,
        "over-bar-back",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, original)]),
    );
    press(cx, handle, "shift-r g h");
    wait(cx, handle, WAIT);
    assert_eq!(
        notes(cx, &view),
        vec![
            (0, 960, 67),
            (960, 960, 69),
            (1920, 960, 64),
            (2880, 960, 65)
        ]
    );
    assert_eq!(cursor(cx, &view), 1920);
    let before = undo_len(cx, &view);
    press(cx, handle, "shift-backspace");
    assert_eq!(
        notes(cx, &view),
        vec![(1920, 960, 64), (2880, 960, 65)],
        "덮어쓰기는 당기지 않는다"
    );
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(undo_len(cx, &view), before + 1, "한 단위");
    run(cx, handle, edit::Undo);
    assert_eq!(
        notes(cx, &view),
        vec![
            (0, 960, 67),
            (960, 960, 69),
            (1920, 960, 64),
            (2880, 960, 65)
        ]
    );
}

/// 덮어쓰기 타이가 클립 끝을 넘으면 클립을 늘리고, 밀지 않는다.
#[gpui_kit::test]
fn review_overwrite_tie_past_clip_end_extends_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "over-tie-end",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(2880, 960, 65)])]),
    );
    set_cursor(cx, &view, 2880, 60);
    press(cx, handle, "shift-r a");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(2880, 960, 60)]);
    press(cx, handle, "~");
    assert_eq!(notes(cx, &view), vec![(2880, 1920, 60)]);
    assert_eq!(clips(cx, &view), vec![(0, 2 * BAR)]);
    assert_eq!(cursor(cx, &view), BAR + 960);
}

/// 덮어쓰기 `Backspace`는 자취를 따라 한 스텝씩 돌아가며(절반·셋잇단 길이 그대로) 당기지 않는다.
#[gpui_kit::test]
fn review_overwrite_backspace_follows_trail_without_pull(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "over-trail",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 60), (960, 960, 62), (1920, 960, 64)])]),
    );
    // 절반 음(480) + 셋잇단 쉼표(640)
    press(cx, handle, "shift-r shift-g n space");
    assert_eq!(cursor(cx, &view), 1120);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 480, 67), (1920, 960, 64)],
        "[0,480)·[480,1120)에서 시작하는 노트를 지운다"
    );
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 480, "셋잇단 쉼표 스텝만큼");
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(notes(cx, &view), vec![(1920, 960, 64)], "당기지 않는다");
}

/// 결정 1절: Step 안에서 팔레트로 `mode::Step`을 다시 고르면 덮어쓰기가 꺼지고 조합은 확정된다.
#[gpui_kit::test]
fn review_palette_reentry_toggles_overwrite_and_commits(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "reentry",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 62)])]),
    );
    press(cx, handle, "shift-r a");
    run(cx, handle, mode_action::Step);
    assert!(!status(cx, &view).overwrite);
    assert_eq!(status(cx, &view).composing, None);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)], "덮어쓰기로 확정");
    press(cx, handle, "s");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (960, 960, 62)]);
    run(cx, handle, mode_action::StepOverwrite);
    assert!(status(cx, &view).overwrite);
}

// ---------- Shift 조합 ----------

/// 화음이 열린 채 `Shift+Backspace`: 화음을 닫고(전진) 마디 시작까지 지운다. 결과에 화음이 남지 않는다.
#[gpui_kit::test]
fn review_shift_backspace_with_open_chord_removes_chord(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "chord-bar-back",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 60)])]),
    );
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "ctrl-a ctrl-d");
    wait(cx, handle, WAIT);
    assert!(status(cx, &view).chord_open);
    press(cx, handle, "shift-backspace");
    assert!(!status(cx, &view).chord_open);
    assert_eq!(notes(cx, &view), vec![], "마디 시작까지 다 지운다");
    assert_eq!(cursor(cx, &view), 0);
}

/// 결정 5절(2026-10-29): 타이는 직전 스텝의 노트가 커서에서 끝날 때만. 레가토 뒤에는 노트가 커서에서 끝나지 않는다.
#[gpui_kit::test]
fn review_tie_after_legato_is_refused(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "legato-tie",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(1920, 960, 64)])]),
    );
    press(cx, handle, "a");
    wait(cx, handle, WAIT);
    run(cx, handle, step::TieLegato);
    assert_eq!(notes(cx, &view), vec![(0, 2880, 60), (2880, 960, 64)]);
    assert_eq!(cursor(cx, &view), 960);
    press(cx, handle, "~");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 2880, 60), (2880, 960, 64)],
        "레가토로 늘어난 노트는 커서에서 끝나지 않으므로 타이는 알리고 아무것도 하지 않아야 한다. 커서 {}",
        cursor(cx, &view)
    );
}

// ---------- MML ----------

/// 결정 4절: 다른 Step 동작(`[`, `n`, 쉼표, `Tab`), undo가 MML 대기를 먼저 확정한다.
#[gpui_kit::test]
fn review_mml_wait_settles_on_every_step_action(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-paths",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "l 8 [");
    assert_eq!(
        status(cx, &view).length,
        StepLength::plain(LengthCode::Eighth)
    );
    assert_eq!(status(cx, &view).octave, 3);
    assert_eq!(status(cx, &view).composing, None);
    press(cx, handle, "l 1 6 space");
    assert_eq!(cursor(cx, &view), 240, "쉼표는 확정한 16분");
    press(cx, handle, "l 2 tab");
    assert_eq!(cursor(cx, &view), 240 + 1920, "Tab은 확정한 길이만큼 전진");
    press(cx, handle, "c");
    wait(cx, handle, WAIT);
    let before = notes(cx, &view);
    assert_eq!(before.len(), 1);
    press(cx, handle, "l 4");
    run(cx, handle, edit::Undo);
    assert_eq!(
        status(cx, &view).length,
        StepLength::plain(LengthCode::Quarter)
    );
    assert_eq!(notes(cx, &view), vec![]);
    press(cx, handle, "o 5 l 8 ctrl-tab");
    assert_eq!(mode(cx, &view), Mode::Sequence);
    assert_eq!(
        status(cx, &view).length,
        StepLength::plain(LengthCode::Eighth)
    );
    assert_eq!(status(cx, &view).octave, 5);
}

/// 결정 4절 「MML은 계이름 레이아웃」. MML 대기 중 레이아웃을 건반형으로 바꾸면 그 대기는 받을 바인딩이 없는
/// 컨텍스트(`Step > Piano > Mml`)에 남으면 안 된다.
#[gpui_kit::test]
fn review_mml_wait_does_not_survive_layout_switch(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-layout",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "l 1");
    set_layout(cx, handle, KeyLayout::Piano);
    let p = path(cx, &view);
    assert!(
        !p.contains(&"Mml"),
        "건반형에 MML 대기가 남았다: {p:?}, 상태 {:?}",
        status(cx, &view).composing
    );
}

/// 결정 4절: MML은 계이름 레이아웃의 문법이다. 건반형에서 팔레트로 `step::MmlOctave`를 불러도 숫자를 받을 수 없는
/// 대기를 열면 안 된다.
#[gpui_kit::test]
fn review_mml_palette_in_piano_layout_does_not_open_dead_wait(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-piano-palette",
        Preset::Default,
        KeyLayout::Piano,
        project(&[]),
    );
    run(cx, handle, step::MmlOctave);
    let p = path(cx, &view);
    // 이어서 친 `5`가 무엇이 되는지 기록한다.
    press(cx, handle, "5");
    let after = (
        status(cx, &view).octave,
        status(cx, &view).length,
        last_message(cx, &view),
    );
    assert!(
        !p.contains(&"Mml"),
        "건반형에서 MML 대기가 열렸다: {p:?}, `5` 뒤 (옥타브, 길이, 알림) = {after:?}"
    );
}

/// `n l8 c`: 셋잇단 접두는 MML 길이를 지나 다음 음에 적용되는가, 상태줄에 보이는가.
#[gpui_kit::test]
fn review_triplet_prefix_survives_mml_length(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-triplet",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "n l 8");
    let shown = status(cx, &view).composing;
    press(cx, handle, "c");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 320, 60)], "셋잇단 8분");
    assert!(
        shown.as_deref().is_some_and(|s| s.contains('n')),
        "셋잇단이 적용될 것인데 상태줄에 `n`이 없다: {shown:?}"
    );
}

// ---------- 진입 ----------

/// 결정 1절(리뷰 1 지적 4로 고침): 모드 FSM은 AI 패널에서도 Step 진입을 받는다(기존 `Enter(Step)`과 같다).
/// 진입 위치 Action도 같은 FSM을 따라 커서를 옮기고 들어간다.
#[gpui_kit::test]
fn review_entry_from_ai_panel_follows_fsm(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "ai-entry",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, 2 * BAR, &[])]),
    );
    set_cursor(cx, &view, 960, 60);
    run(cx, handle, ai::OpenPanel);
    assert_eq!(mode(cx, &view), Mode::Ai);
    run(cx, handle, mode_action::StepAtBarEnd);
    assert_eq!((mode(cx, &view), cursor(cx, &view)), (Mode::Step, BAR));
}
