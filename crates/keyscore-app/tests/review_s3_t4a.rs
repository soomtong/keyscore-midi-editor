//! S3-T4a 적대적 리뷰 1회차(docs/reports/s3-t4a/review-1.md). 픽스처는 `step_input.rs`에서 옮겨 왔다.
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

// ---------- 리뷰 시도 ----------

/// 지적 1: `Shift+음`(절반) 뒤 `Backspace`는 방금 친 스텝(240틱)이 아니라 현재 길이 한 스텝(480틱)을 지운다.
/// 뒤 노트가 240틱 더 당겨져 원래 자리보다 앞서고, 커서는 앞 노트(C)의 한가운데로 간다.
#[gpui_kit::test]
fn backspace_after_half_note_restores_later_notes(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "rv-bs-half",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(960, 480, 72)])]),
    );
    press(cx, handle, "4 c enter");
    // C 0..480, 뒤 노트는 1440으로 밀렸다
    assert_eq!(notes(cx, &view), vec![(0, 480, 60), (1440, 480, 72)]);
    press(cx, handle, "shift-d enter");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 480, 60), (480, 240, 62), (1680, 480, 72)]
    );
    assert_eq!(cursor(cx, &view), 720);
    press(cx, handle, "backspace");
    // 방금 친 D를 지우면 D를 치기 전 상태(뒤 노트 1440, 커서 480)로 돌아가야 한다
    assert_eq!(notes(cx, &view), vec![(0, 480, 60), (1440, 480, 72)]);
    assert_eq!(cursor(cx, &view), 480);
}

/// 지적 1(셋잇단): `4na 4nb 4nc` 뒤 `Backspace`는 c(320틱)만 지우고 커서를 b 끝(640)에 둬야 한다.
#[gpui_kit::test]
fn backspace_after_triplets_lands_on_previous_step(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "rv-bs-trip",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(1920, 480, 72)])]),
    );
    press(cx, handle, "4 n a enter n b enter n c enter");
    assert_eq!(cursor(cx, &view), 960);
    assert_eq!(notes(cx, &view).last(), Some(&(1920 + 960, 480, 72)));
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 640, "커서는 b 끝(=c 시작)");
    assert_eq!(
        notes(cx, &view).last(),
        Some(&(1920 + 640, 480, 72)),
        "뒤 노트는 c 길이(320)만큼만 당겨져야 한다"
    );
}

/// 지적 2: 클립 안에서 친 쉼표가 클립 끝을 넘은 뒤의 음은 그 클립을 늘리지 않고 새 클립을 만든다
/// (REQ 5.10 「입력이 클립 끝을 넘으면 클립이 마디 단위로 늘어난다」).
#[gpui_kit::test]
fn rest_past_clip_end_then_note_extends_same_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "rv-rest-end",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 960, 60)])]),
    );
    set_cursor(cx, &view, 2880);
    press(cx, handle, "2 space");
    assert_eq!(cursor(cx, &view), BAR + 960);
    press(cx, handle, "c enter");
    assert_eq!(
        clips(cx, &view),
        vec![(0, 2 * BAR)],
        "이어 친 입력은 한 클립으로 늘어나야 한다"
    );
}

/// 지적 3: 결정 5절 「화음이 열린 채 다른 Step 동작이 오면 먼저 화음을 닫는다」. 옥타브 키 `]`는 다른 Step
/// 동작인데 화음이 열린 채 남는다.
#[gpui_kit::test]
fn octave_key_closes_open_chord(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-chord-oct", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "ctrl-c enter");
    assert!(status(cx, &view).chord_open);
    press(cx, handle, "]");
    assert!(
        !status(cx, &view).chord_open,
        "결정 5절: 다른 Step 동작이 오면 화음을 먼저 닫는다"
    );
    assert_eq!(cursor(cx, &view), 960);
}

/// 지적 3(`n`): 셋잇단 접두도 다른 Step 동작이다.
#[gpui_kit::test]
fn triplet_key_closes_open_chord(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-chord-n", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "ctrl-c enter n");
    assert!(
        !status(cx, &view).chord_open,
        "결정 5절: 다른 Step 동작이 오면 화음을 먼저 닫는다"
    );
}

/// 못 깨뜨림: 옥타브 0의 `c -`는 B-1(11)로 0–127 안이다. 옥타브 9 `g #`(128)은 쓰지 않고 커서도 그대로다.
/// `b #`은 다음 옥타브 C다.
#[gpui_kit::test]
fn solfege_accidental_out_of_range_is_refused(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-acc-range", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "[ [ [ [ c - enter");
    assert_eq!(notes(cx, &view), vec![(0, 960, 11)]);
    assert_eq!(cursor(cx, &view), 960);
    press(cx, handle, "] ] ] ] ] ] ] ] ] g # enter");
    assert_eq!(notes(cx, &view).len(), 1);
    assert_eq!(cursor(cx, &view), 960);
    assert_eq!(status(cx, &view).composing, None);
    // b # 는 다음 옥타브 C
    press(cx, handle, "[ [ [ [ [ b # enter");
    assert_eq!(notes(cx, &view)[1], (960, 960, 72), "oct4 b# = C5");
}

/// 못 깨뜨림: 낡은 타이머. 변화표로 세대가 바뀐 뒤 첫 타이머는 확정하지 않는다.
#[gpui_kit::test]
fn stale_timer_after_accidental_does_not_commit_early(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-stale", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c");
    wait(cx, handle, Duration::from_millis(200));
    press(cx, handle, "#");
    wait(cx, handle, Duration::from_millis(150));
    assert!(notes(cx, &view).is_empty(), "첫 타이머(300 ms)는 낡았다");
    press(cx, handle, "5");
    assert_eq!(notes(cx, &view), vec![(0, 960, 73)]);
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view).len(), 1);
}

/// 못 깨뜨림: 화음이 열린 채 Esc로 나가면 닫히고, 다시 들어와 친 음은 화음 뒤에 붙는다.
#[gpui_kit::test]
fn leaving_with_open_chord_closes_it(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-chord-esc", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "ctrl-c ctrl-e escape");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (0, 960, 64)]);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert!(!status(cx, &view).chord_open);
    assert_eq!(cursor(cx, &view), 960);
}

/// 못 깨뜨림: 건반형 조합 중 z/x 는 옛 옥타브로 확정하고 옥타브를 바꾼다. n 은 확정 후 셋잇단.
#[gpui_kit::test]
fn piano_pending_z_x_n_commit_first(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-piano-zxn", KeyLayout::Piano, project(&[]));
    press(cx, handle, "a x");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)]);
    assert_eq!(status(cx, &view).octave, 5);
    press(cx, handle, "a n s enter");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (960, 960, 72), (1920, 640, 74)]
    );
}

/// 관찰(제안 1): Step에서 `Cmd+Z`는 커서를 돌리지 않는다. 지운 음 자리에 구멍이 남은 채 다음 음이 그 뒤에 붙는다.
#[gpui_kit::test]
fn undo_in_step_leaves_cursor_after_removed_note(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-undo-cursor", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c enter d enter cmd-z");
    assert_eq!(cursor(cx, &view), 1920);
    press(cx, handle, "e enter");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (1920, 960, 64)]);
}

/// 관찰(제안 2): 첫 음을 `Backspace`로 지우면 그 음이 만든 빈 클립이 남는다.
#[gpui_kit::test]
fn backspace_first_note_leaves_empty_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rv-bs-empty", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c enter backspace");
    assert!(notes(cx, &view).is_empty());
    assert_eq!(clips(cx, &view), vec![(0, BAR)]);
}
