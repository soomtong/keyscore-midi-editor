//! S3-T4a 적대적 리뷰 2회차(docs/reports/s3-t4a/review-2.md). 픽스처는 `review_s3_t4a.rs`에서 옮겨 왔다.
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

// ---------- 리뷰 2 시도 ----------

/// 지적 1 수정 확인: 절반 뒤 `Backspace`는 방금 친 스텝만 지우고 뒤 노트를 원래 자리로 돌린다.
#[gpui_kit::test]
fn r2_backspace_after_half_restores(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-bs-half",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(960, 480, 72)])]),
    );
    press(cx, handle, "4 c enter shift-d enter backspace");
    assert_eq!(cursor(cx, &view), 480);
    assert_eq!(notes(cx, &view), vec![(0, 480, 60), (1440, 480, 72)]);
}

/// 셋잇단 셋 → `Shift+Tab` 둘 → `Backspace`: 자취를 따라 c 하나만 지운다.
#[gpui_kit::test]
fn r2_shift_tab_then_backspace_follows_trail(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r2-trail", KeyLayout::Solfege, project(&[]));
    // 4분 셋잇단 = 640
    press(
        cx,
        handle,
        "n c enter n d enter n e enter shift-tab shift-tab",
    );
    assert_eq!(cursor(cx, &view), 640);
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(notes(cx, &view), vec![(0, 640, 62), (640, 640, 64)]);
}

/// 타이로 늘린 스텝: `Backspace`가 타이까지 한 스텝으로 지운다.
#[gpui_kit::test]
fn r2_backspace_after_tie_removes_tied_note(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r2-tie", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c enter ~ d enter backspace");
    assert_eq!(cursor(cx, &view), 1920);
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 0);
    assert!(notes(cx, &view).is_empty());
}

/// 낡은 자취: 4분 셋잇단 셋(640씩)을 친 뒤 undo로 모두 되돌리면 문서에는 셋잇단이 없는데도 `Backspace`가 낡은
/// 자취(1280–1920)를 따라 640만 뒤로 가 아무것도 지우지 않고 뒤 노트를 640 당겨 격자에서 어긋나게 한다. 자취가
/// 없을 때의 규칙(현재 길이 한 스텝)이면 커서 960, 그 자리 노트(62)를 지우고 뒤 노트를 960 당긴다.
#[gpui_kit::test]
fn r2_stale_trail_after_undo_misplaces_backspace(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-stale-undo",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 960, 60), (960, 960, 62), (1920, 960, 64)])]),
    );
    press(cx, handle, "n f enter n g enter n a enter");
    assert_eq!(cursor(cx, &view), 1920);
    press(cx, handle, "cmd-z cmd-z cmd-z");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (960, 960, 62), (1920, 960, 64)]
    );
    assert_eq!(cursor(cx, &view), 1920);
    press(cx, handle, "backspace");
    let after = notes(cx, &view);
    assert_eq!(cursor(cx, &view), 960, "notes: {after:?}");
    assert_eq!(after, vec![(0, 960, 60), (960, 960, 64)]);
}

/// 낡은 자취: Step을 나갔다 들어와도 자취가 남는다. 나간 뒤 커서를 옮겼다가 같은 틱에 돌아오면 `Shift+Tab`이
/// 현재 길이가 아니라 예전 셋잇단 길이만큼 간다.
#[gpui_kit::test]
fn r2_stale_trail_survives_mode_exit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r2-stale-exit", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c enter c enter n c enter");
    assert_eq!(cursor(cx, &view), 2560);
    event(cx, handle, &view, ModeEvent::ToNormal);
    set_cursor(cx, &view, 0);
    set_cursor(cx, &view, 2560);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    press(cx, handle, "shift-tab");
    // 현재 길이(4분) 한 스텝이면 1600
    assert_eq!(cursor(cx, &view), 1600);
}

/// 지적 2 회귀: 클립 안에서 친 쉼표가 다음 클립 시작을 넘으면 쉼표 자체가 거부되어 커서가 움직이지 않는다.
/// 고치기 전에는 커서가 쉼표 길이만큼 갔다(노트가 없으면 편집 없음).
#[gpui_kit::test]
fn r2_rest_crossing_next_clip_is_refused(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-rest-next",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 960, 60)]), (BAR + 960, BAR, &[])]),
    );
    set_cursor(cx, &view, 2880);
    press(cx, handle, "1 space");
    let msg = last_message(cx, &view);
    assert_eq!(cursor(cx, &view), 2880 + BAR, "message: {msg}");
}

/// 지적 2 확인: 쉼표가 클립 끝을 넘은 뒤 음은 같은 클립에 들어간다.
#[gpui_kit::test]
fn r2_rest_past_end_then_note_same_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-rest-end",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[])]),
    );
    set_cursor(cx, &view, 2880);
    press(cx, handle, "2 space c enter");
    assert_eq!(clips(cx, &view), vec![(0, 2 * BAR)]);
    assert_eq!(notes(cx, &view), vec![(4800, 1920, 60)]);
    // 쉼표의 연장은 한 undo 단위
    press(cx, handle, "cmd-z");
    assert_eq!(clips(cx, &view), vec![(0, 2 * BAR)]);
    press(cx, handle, "cmd-z");
    assert_eq!(clips(cx, &view), vec![(0, BAR)]);
}

/// 지적 2: 커서가 앞 클립 끝이자 붙은 다음 클립 시작이면 다음 클립 안의 쉼표(밀기)로 본다.
#[gpui_kit::test]
fn r2_rest_at_adjacent_boundary_pushes_next_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-rest-adj",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[]), (BAR, BAR, &[(0, 960, 67)])]),
    );
    set_cursor(cx, &view, BAR);
    press(cx, handle, "space");
    assert_eq!(cursor(cx, &view), BAR + 960);
    assert_eq!(notes(cx, &view), vec![(BAR + 960, 960, 67)]);
    assert_eq!(clips(cx, &view), vec![(0, BAR), (BAR, BAR)]);
}

/// 지적 3 확인: 화음 중 `[` `z`(건반형)·`n`이 화음을 닫고, 그 뒤 길이·옥타브가 적용된다.
#[gpui_kit::test]
fn r2_octave_and_triplet_close_chord(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r2-chord", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "ctrl-c enter ctrl-e enter [ g enter");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (0, 960, 64), (960, 960, 55)]
    );
    press(cx, handle, "ctrl-c n ctrl-e enter tab");
    // 화음(c) 하나는 n이 닫고, n은 다음 음(e) 하나에
    assert_eq!(
        notes(cx, &view),
        vec![
            (0, 960, 60),
            (0, 960, 64),
            (960, 960, 55),
            (1920, 960, 48),
            (2880, 640, 52)
        ]
    );
}

#[gpui_kit::test]
fn r2_piano_z_closes_chord(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r2-piano-chord", KeyLayout::Piano, project(&[]));
    press(cx, handle, "ctrl-a enter z");
    assert!(!status(cx, &view).chord_open);
    assert_eq!(cursor(cx, &view), 960);
}

/// 제안 6 확인: 표시. 건반형은 음이름, 계이름 변화표 여럿.
#[gpui_kit::test]
fn r2_status_shows_composition(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "r2-status", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c # #");
    assert_eq!(label(cx, &view), "STEP ♩ oct4 c##_");
    press(cx, handle, "enter shift-e");
    assert_eq!(label(cx, &view), "STEP ♩ oct4 e_");
    press(cx, handle, "enter 4 n");
    set_layout(cx, handle, KeyLayout::Piano);
    press(cx, handle, "w");
    assert_eq!(label(cx, &view), "STEP ♪ oct4 4nc#_");
}
