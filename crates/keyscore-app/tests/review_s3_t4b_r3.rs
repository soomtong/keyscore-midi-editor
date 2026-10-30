//! S3-T4b 적대적 리뷰 3회차의 깨뜨리기 시험(`docs/reports/s3-t4b/review-3.md`). 도우미는
//! `tests/review_s3_t4b_r2.rs`에서 옮겼다.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::actions::{edit, mode as mode_action, step};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::step::StepStatus;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const BAR: u64 = 960 * 4;
const WAIT: Duration = Duration::from_millis(300);

/// (절대 시작, 길이, 음높이)
type Abs = (u64, u32, u8);

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-t4b-r3-{}-{name}", std::process::id()));
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

#[allow(dead_code)]
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

// ---------- (a) 타이 대상 검사 ----------

/// 셋잇단으로 연 화음: `n Ctrl+c e`는 첫 음만 셋잇단(640)이고 둘째 음은 4분(960)이라 스텝(640)을 넘어 끝난다.
/// 결정 2026-10-29 5절은 닫은 화음을 타이 대상으로 하지만 2026-10-30 2절은 「모두 커서 이전에 끝날 때만」이다.
#[gpui_kit::test]
fn r3_tie_after_triplet_chord(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-triplet-chord-tie",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[(0, BAR, &[])]),
    );
    press(cx, handle, "n ctrl-c e enter");
    assert_eq!(notes(cx, &view), vec![(0, 640, 60), (0, 960, 64)]);
    assert_eq!(cursor(cx, &view), 640);
    let before = undo_len(cx, &view);
    press(cx, handle, "~");
    let msg = last_message(cx, &view);
    eprintln!("triplet chord tie: notes {:?}, msg {msg}", notes(cx, &view));
    // 결정 2절 문구대로라면 거부. 거부할 때 알림이 「바로 앞에 친 음이 없다」면 사실과 다르다.
    assert_eq!(undo_len(cx, &view), before);
    assert!(
        !msg.contains("바로 앞에 친 음이 없다"),
        "직전 스텝이 있는데 없다고 알린다: {msg}"
    );
}

/// 절반 음으로 닫은 화음 뒤 레가토(짧은 음만 커서까지 늘어남) 뒤 타이: 커서를 넘긴 노트가 없으니 2절 문구로 허용.
#[gpui_kit::test]
fn r3_tie_after_legato_that_stays_within_step(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-legato-within-tie",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 480, 67)])]),
    );
    // 커서 0에서 삽입: 기존 G(0)는 960으로 밀린다.
    press(cx, handle, "ctrl-c shift-e enter");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 480, 64), (0, 960, 60), (960, 480, 67)]
    );
    run(cx, handle, step::TieLegato);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (0, 960, 64), (960, 480, 67)]
    );
    press(cx, handle, "~");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 1920, 60), (0, 1920, 64), (1920, 480, 67)],
        "알림: {}",
        last_message(cx, &view)
    );
    assert_eq!(cursor(cx, &view), 1920);
}

/// 덮어쓰기에서 절반 음 화음 뒤 레가토가 커서를 넘기면 타이는 거부되고 문서·커서는 그대로다.
#[gpui_kit::test]
fn r3_overwrite_half_chord_legato_past_cursor_refuses_tie(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-over-legato-past",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(1500, 480, 67)])]),
    );
    run(cx, handle, mode_action::StepOverwrite);
    press(cx, handle, "ctrl-c shift-e enter");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 480, 64), (0, 960, 60), (1500, 480, 67)]
    );
    run(cx, handle, step::TieLegato);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 1500, 60), (0, 1500, 64), (1500, 480, 67)]
    );
    let before = undo_len(cx, &view);
    press(cx, handle, "~");
    assert_eq!(undo_len(cx, &view), before);
    assert_eq!(cursor(cx, &view), 960);
}

/// 덮어쓰기 타이가 절반 음 화음을 늘리면서 [커서, 커서 + 길이)에서 시작하는 노트만 지운다.
#[gpui_kit::test]
fn r3_overwrite_tie_on_half_chord(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-over-tie-half",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(960, 240, 67), (1920, 240, 69)])]),
    );
    run(cx, handle, mode_action::StepOverwrite);
    press(cx, handle, "ctrl-c shift-e enter ~");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 1440, 64), (0, 1920, 60), (1920, 240, 69)],
        "알림: {}",
        last_message(cx, &view)
    );
    assert_eq!(cursor(cx, &view), 1920);
}

/// undo로 직전 스텝을 되돌린 뒤 타이: 문서에 없는 노트를 늘리지 않는다(알림만).
#[gpui_kit::test]
fn r3_tie_after_undo_does_not_edit(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-undo-tie",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[(0, BAR, &[])]),
    );
    press(cx, handle, "c ctrl-d shift-f enter");
    assert_eq!(cursor(cx, &view), 1920);
    run(cx, handle, edit::Undo);
    let after_undo = notes(cx, &view);
    let before = undo_len(cx, &view);
    press(cx, handle, "~");
    eprintln!("undo tie msg: {}", last_message(cx, &view));
    assert_eq!(notes(cx, &view), after_undo);
    assert_eq!(undo_len(cx, &view), before);
    // redo로 되살리면 다시 타이 대상이다.
    run(cx, handle, edit::Redo);
    press(cx, handle, "~");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (960, 1440, 65), (960, 1920, 62)],
        "알림: {}",
        last_message(cx, &view)
    );
}

/// 같은 커서에서 타이를 두 번: 둘째 타이도 섞인 길이 화음을 모두 늘린다.
#[gpui_kit::test]
fn r3_double_tie_on_half_chord(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-double-tie",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[(0, BAR, &[])]),
    );
    press(cx, handle, "ctrl-c shift-e enter ~ ~");
    assert_eq!(notes(cx, &view), vec![(0, 2400, 64), (0, 2880, 60)]);
    assert_eq!(cursor(cx, &view), 2880);
}

// ---------- (c) 「커서가 든 클립 안만」 ----------

/// 클립 A [0, BAR)의 끝 = 커서 = 클립 B 시작. 덮어쓰기 음은 B(커서가 든 클립)의 노트를 바꾼다.
#[gpui_kit::test]
fn r3_overwrite_note_at_boundary_uses_next_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r3-over-note-at-boundary",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[]), (BAR, BAR, &[(0, 960, 62)])]),
    );
    set_cursor(cx, &view, BAR, 60);
    press(cx, handle, "shift-r a");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(BAR, 960, 60)]);
}

/// 같은 경계에서 덮어쓰기 타이: 직전 스텝은 A에 있고 커서가 든 클립(삽입 규칙)은 B다. 결정 2절 「지우는 노트는 커서가
/// 든 클립 안의 것뿐」이면 B의 노트(BAR 시작)를 지우거나, 적어도 늘어난 노트와 B의 노트가 겹쳐 남지 않아야 한다.
#[gpui_kit::test]
fn r3_overwrite_tie_at_clip_boundary(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r3-over-tie-boundary",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[]), (BAR, BAR, &[(0, 960, 62)])]),
    );
    set_cursor(cx, &view, BAR - 960, 60);
    press(cx, handle, "shift-r a");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(BAR - 960, 960, 60), (BAR, 960, 62)]);
    assert_eq!(cursor(cx, &view), BAR);
    let before = undo_len(cx, &view);
    press(cx, handle, "~");
    let after = notes(cx, &view);
    eprintln!(
        "overwrite tie boundary: notes {after:?}, clips {:?}, msg {}",
        clips(cx, &view),
        last_message(cx, &view)
    );
    let changed = undo_len(cx, &view) != before;
    // 타이가 적용됐다면 B의 BAR 노트는 덮여 지워져야 한다(같은 커서의 덮어쓰기 음과 같은 결과).
    if changed {
        assert!(
            !after.contains(&(BAR, 960, 62)),
            "덮어쓰기 타이가 늘어난 범위의 다음 클립 노트를 남겼다: {after:?}"
        );
    }
}

/// 삽입 타이가 클립 경계에서: 직전 스텝은 A, 커서가 든 클립은 B. 기록만(밀기 대상 클립).
#[gpui_kit::test]
fn r3_insert_tie_at_clip_boundary(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r3-ins-tie-boundary",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[]), (BAR, BAR, &[(0, 960, 62)])]),
    );
    set_cursor(cx, &view, BAR - 960, 60);
    press(cx, handle, "i a");
    wait(cx, handle, WAIT);
    let before = undo_len(cx, &view);
    press(cx, handle, "~");
    eprintln!(
        "insert tie boundary: notes {:?}, clips {:?}, undo +{}, msg {}",
        notes(cx, &view),
        clips(cx, &view),
        undo_len(cx, &view) - before,
        last_message(cx, &view)
    );
}

/// 덮어쓰기 `Backspace`가 클립 경계(커서 = A 끝 = B 시작)에서: 커서 바로 앞 틱의 클립(A)에서 지운다. 같은 자리의
/// 덮어쓰기 음·쉼표는 B(커서가 든 클립)를 쓴다.
#[gpui_kit::test]
fn r3_overwrite_backspace_at_boundary_uses_previous_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r3-over-bs-boundary",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(2880, 960, 64)]), (BAR, BAR, &[(0, 960, 62)])]),
    );
    set_cursor(cx, &view, BAR, 60);
    press(cx, handle, "shift-r backspace");
    assert_eq!(notes(cx, &view), vec![(BAR, 960, 62)]);
    assert_eq!(cursor(cx, &view), BAR - 960);
}

// ---------- (b) `n` 접두와 버림 ----------

/// `n l 8` 뒤 `Shift+Backspace`: MML 대기를 버리고 마디 처음까지 지운다. 남은 `n`은 다음 음에 적용되고 표시에도 있다.
#[gpui_kit::test]
fn r3_triplet_survives_shift_backspace_discard(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r3-triplet-shift-bs",
        Preset::Vim,
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 960, 67)])]),
    );
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "i n l 8");
    assert_eq!(status(cx, &view).composing.as_deref(), Some("nl8_"));
    press(cx, handle, "shift-backspace");
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(notes(cx, &view), vec![]);
    press(cx, handle, "c");
    let shown = status(cx, &view).composing;
    wait(cx, handle, WAIT);
    let written = notes(cx, &view);
    eprintln!("shift-bs: shown {shown:?}, notes {written:?}");
    let triplet = written == vec![(0, 640, 60)];
    assert_eq!(
        shown.as_deref().is_some_and(|s| s.starts_with('n')),
        triplet,
        "표시 {shown:?}, 쓴 노트 {written:?}"
    );
    // 길이는 l8 대기를 버렸으니 그대로(4분).
    assert_eq!(written.first().map(|n| n.1), Some(640));
}

/// `n c` 조합을 `Backspace`로 버리면 셋잇단도 함께 버려진다(조합이 소비했다). 다음 음은 표시·적용 모두 셋잇단 아님.
#[gpui_kit::test]
fn r3_triplet_discarded_with_composition(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-triplet-compose-bs",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "n c");
    assert_eq!(status(cx, &view).composing.as_deref(), Some("nc_"));
    press(cx, handle, "backspace d");
    let shown = status(cx, &view).composing;
    wait(cx, handle, WAIT);
    assert_eq!(shown.as_deref(), Some("d_"));
    assert_eq!(notes(cx, &view), vec![(0, 960, 62)]);
}

/// `n n l 8 backspace c`: `n`을 두 번 쳐도 셋잇단은 한 번(다음 음 하나)이다. 표시는?
#[gpui_kit::test]
fn r3_double_n_then_mml_discard(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-double-n",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "n n l 8 backspace c");
    let shown = status(cx, &view).composing;
    wait(cx, handle, WAIT);
    eprintln!("double n: shown {shown:?}, notes {:?}", notes(cx, &view));
    assert_eq!(notes(cx, &view), vec![(0, 640, 60)]);
}

/// `n 3 l 1 6 backspace c`: 길이 코드 `3`(4분)은 이미 스티키로 적용됐다. 대기를 버린 뒤 표시는 `n`만 남고, 적용은
/// 셋잇단 4분(640)이다.
#[gpui_kit::test]
fn r3_length_code_then_mml_discard(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-code-then-mml",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "n 3 l 1 6");
    assert_eq!(status(cx, &view).composing.as_deref(), Some("nl16_"));
    press(cx, handle, "backspace c");
    let shown = status(cx, &view).composing;
    wait(cx, handle, WAIT);
    assert_eq!(shown.as_deref(), Some("nc_"));
    assert_eq!(notes(cx, &view), vec![(0, 640, 60)]);
}

/// `n` 뒤 Step을 나갔다 들어오면: 남은 셋잇단이 표시 없이 적용되지 않는가.
#[gpui_kit::test]
fn r3_triplet_across_step_exit(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r3-triplet-exit",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "n l 8 backspace");
    event(cx, handle, &view, ModeEvent::ToNormal);
    assert_eq!(mode(cx, &view), Mode::Normal);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    press(cx, handle, "c");
    let shown = status(cx, &view).composing;
    wait(cx, handle, WAIT);
    let written = notes(cx, &view);
    eprintln!("exit: shown {shown:?}, notes {written:?}");
    let triplet = written == vec![(0, 640, 60)];
    assert_eq!(
        shown.as_deref().is_some_and(|s| s.starts_with('n')),
        triplet,
        "표시 {shown:?}, 쓴 노트 {written:?}"
    );
}
