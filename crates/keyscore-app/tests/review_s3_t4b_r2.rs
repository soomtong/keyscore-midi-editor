//! S3-T4b 적대적 리뷰 2회차의 깨뜨리기 시험(`docs/reports/s3-t4b/review-2.md`). 도우미는
//! `tests/review_s3_t4b.rs`에서 옮겼다.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, LengthCode};
use keyscore_app::actions::{mode as mode_action, step};
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
    let dir = std::env::temp_dir().join(format!("ks-review-t4b-r2-{}-{name}", std::process::id()));
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

// ---------- 리뷰 2: 타이 대상 검사 ----------

/// 결정 2026-10-29 5절 「화음도 모두 늘린다」. `Ctrl+c`로 연 화음을 `Shift+e`(절반)로 닫으면 화음 노트 길이가 섞인다.
/// 리뷰 1 지적 1의 수정(모든 노트가 커서에서 끝나야 함)이 이 화음의 타이를 막는가.
#[gpui_kit::test]
fn r2_tie_after_chord_closed_by_half_note(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r2-chord-half-tie",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[(0, BAR, &[])]),
    );
    press(cx, handle, "ctrl-c shift-e enter");
    assert_eq!(notes(cx, &view), vec![(0, 480, 64), (0, 960, 60)]);
    assert_eq!(cursor(cx, &view), 960);
    press(cx, handle, "~");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 1440, 64), (0, 1920, 60)],
        "닫은 화음의 타이가 거부됐다. 알림: {}",
        last_message(cx, &view)
    );
}

// ---------- 리뷰 2: 레이아웃 변경 때의 MML 대기 확정(그리기 중) ----------

/// `l`만 친 채(숫자 없음) 건반형으로 바꾸면 그리기 중 확정이 알림을 내고 길이는 그대로다.
#[gpui_kit::test]
fn r2_layout_switch_settles_bare_mml_with_notice(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r2-layout-bare",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "l");
    set_layout(cx, handle, KeyLayout::Piano);
    assert!(!path(cx, &view).contains(&"Mml"));
    assert_eq!(status(cx, &view).composing, None);
    assert_eq!(
        status(cx, &view).length,
        StepLength::plain(LengthCode::Quarter)
    );
    assert!(
        last_message(cx, &view).contains("MML"),
        "{}",
        last_message(cx, &view)
    );
    // 건반형의 `l`은 음 키다
    press(cx, handle, "a");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)]);
}

/// `l 1 6` 대기 중 건반형으로: 16분으로 확정되고 이어지는 건반형 숫자는 길이 코드다.
#[gpui_kit::test]
fn r2_layout_switch_settles_l16(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r2-layout-l16",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "l 1 6");
    set_layout(cx, handle, KeyLayout::Piano);
    assert_eq!(
        status(cx, &view).length,
        StepLength::plain(LengthCode::Sixteenth)
    );
    press(cx, handle, "4 a");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 480, 60)]);
}

// ---------- 리뷰 2: MML 숫자 범위 ----------

#[gpui_kit::test]
fn r2_mml_numbers_out_of_range(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r2-mml-range",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    // 세 자리(164), 0, 앞 0(016)은 알리고 길이를 바꾸지 않는다
    for keys in ["l 1 6 4", "l 0", "l 0 1 6", "l 1 2 8 8"] {
        press(cx, handle, keys);
        press(cx, handle, "enter");
        assert_eq!(
            status(cx, &view).length,
            StepLength::plain(LengthCode::Quarter),
            "{keys}"
        );
        assert!(last_message(cx, &view).contains("MML"), "{keys}");
    }
    // `l.4`: 점 뒤 숫자는 쌓지 않는다
    press(cx, handle, "l . 4 enter");
    assert_eq!(
        status(cx, &view).length,
        StepLength::plain(LengthCode::Quarter)
    );
    let at = cursor(cx, &view);
    // 점64분 = 90
    press(cx, handle, "l 6 4 . c");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view).last().copied(), Some((at, 90, 60)));
    // 팔레트 MmlDigit(10)·(-1)은 무시하고 대기는 남는다
    press(cx, handle, "o");
    run(
        cx,
        handle,
        step::MmlDigit(keyscore_app::actions::args::Int(10)),
    );
    run(
        cx,
        handle,
        step::MmlDigit(keyscore_app::actions::args::Int(-1)),
    );
    assert_eq!(status(cx, &view).composing.as_deref(), Some("o_"));
    press(cx, handle, "9 b");
    wait(cx, handle, WAIT);
    assert!(
        last_message(cx, &view).contains("0–127"),
        "{}",
        last_message(cx, &view)
    );
    press(cx, handle, "o 0 c");
    wait(cx, handle, WAIT);
    assert!(
        notes(cx, &view).iter().any(|n| n.2 == 12),
        "{:?}",
        notes(cx, &view)
    );
}

/// 리뷰 1 제안 6: `{` `}`도 MML 대기를 먼저 확정한다(vim 프리셋 키).
#[gpui_kit::test]
fn r2_braces_settle_mml_first(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r2-braces-mml",
        Preset::Vim,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "l 8 }");
    assert_eq!(
        status(cx, &view).length,
        StepLength::plain(LengthCode::Eighth)
    );
    assert_eq!(status(cx, &view).composing, None);
    assert!(!path(cx, &view).contains(&"Mml"));
}

/// 리뷰 1 제안 5의 같은 갈래: `n l 8` 뒤 `Backspace`로 대기를 버리면 표시에서 `n`이 사라진다. 셋잇단 접두는
/// 버려지는가, 남는가. 남는다면 다음 조합 표시에 `n`이 보여야 한다.
#[gpui_kit::test]
fn r2_triplet_prefix_after_discarded_mml(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r2-mml-triplet-discard",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "n l 8");
    assert_eq!(status(cx, &view).composing.as_deref(), Some("nl8_"));
    press(cx, handle, "backspace c");
    let shown = status(cx, &view).composing;
    wait(cx, handle, WAIT);
    let written = notes(cx, &view);
    // 일관성: 셋잇단이 적용되면 표시에도 `n`이 있어야 하고, 표시에 없으면 셋잇단이 아니어야 한다.
    let triplet = written == vec![(0, 640, 60)];
    assert_eq!(
        shown.as_deref().is_some_and(|s| s.starts_with('n')),
        triplet,
        "표시 {shown:?}, 쓴 노트 {written:?}"
    );
}

// ---------- 리뷰 2: 덮어쓰기와 클립 경계 ----------

/// 결정 2절(리뷰 2 제안 2로 문구를 분명히 함): 덮어쓰기가 지우는 노트는 커서가 든 클립 안의 것뿐이다(삽입·`Backspace`와
/// 같은 클립 규칙). 쉼표가 다음 클립으로 걸쳐도 다음 클립의 노트는 남는다.
#[gpui_kit::test]
fn r2_overwrite_rest_across_clip_boundary(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-over-rest-boundary",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[]), (BAR, BAR, &[(0, 960, 62)])]),
    );
    set_cursor(cx, &view, BAR - 480, 60);
    press(cx, handle, "shift-r space");
    assert_eq!(cursor(cx, &view), BAR + 480);
    assert_eq!(notes(cx, &view), vec![(BAR, 960, 62)]);
}

/// 덮어쓰기 음이 다음 클립에 닿으면 삽입처럼 거부되고 문서는 그대로다.
#[gpui_kit::test]
fn r2_overwrite_note_into_next_clip_is_refused(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-over-note-boundary",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[]), (BAR, BAR, &[(0, 960, 62)])]),
    );
    set_cursor(cx, &view, BAR - 480, 60);
    let before = undo_len(cx, &view);
    press(cx, handle, "shift-r a");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(BAR, 960, 62)]);
    assert_eq!(undo_len(cx, &view), before);
    assert_eq!(cursor(cx, &view), BAR - 480);
}

// ---------- 리뷰 2: 설정 창 ----------

fn settings_view(
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    Entity<keyscore_app::settings_window::SettingsWindow>,
) {
    let (sh, sv) = cx
        .update(|cx| keyscore_app::settings_window::current(cx))
        .expect("설정 창");
    cx.update_window(sh, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, sh);
    (sh, sv)
}

fn label_of(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

/// 파일에서 바뀐 목록 밖 값(250 ms)은 덧붙여 보이고, 그 자리에서 아래로 한 칸 고르면 다음 값(300)이 된다. 팔레트로
/// 바꾼 기본 길이도 열린 설정 창에 보인다.
#[gpui_kit::test]
fn r2_settings_choices_follow_outside_values(cx: &mut TestAppContext) {
    use keyscore_app::actions::args::{Opt, SettingsSection};
    use keyscore_app::actions::settings;
    let (handle, _view) = setup(
        cx,
        "r2-settings",
        Preset::Default,
        KeyLayout::Piano,
        project(&[]),
    );
    run(
        cx,
        handle,
        settings::Open(Opt(Some(SettingsSection::Input))),
    );
    let (sh, _sv) = settings_view(cx);
    cx.update(|cx| AppSettings::update(cx, |s| s.input.octave_wait_ms = 250));
    cx.update(gpui_kit::App::refresh_windows);
    cx.run_until_parked();
    render(cx, sh);
    assert!(
        label_of(cx, sh, "settings-octave-wait").is_some_and(|l| l.contains("250 ms")),
        "{:?}",
        label_of(cx, sh, "settings-octave-wait")
    );
    press(cx, sh, "tab tab tab tab tab");
    press(cx, sh, "enter down enter");
    assert_eq!(cx.read(|cx| AppSettings::get(cx).input.octave_wait_ms), 300);
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(
            Box::new(settings::SetStepDefaultLength(LengthCode::SixtyFourth)),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, sh);
    assert!(
        label_of(cx, sh, "settings-step-length").is_some_and(|l| l.contains("64분음표")),
        "{:?}",
        label_of(cx, sh, "settings-step-length")
    );
    // 0 ms(파일에서만 가능한 하한)도 덧붙여 보인다
    cx.update(|cx| AppSettings::update(cx, |s| s.input.octave_wait_ms = 0));
    cx.update(gpui_kit::App::refresh_windows);
    cx.run_until_parked();
    render(cx, sh);
    assert!(
        label_of(cx, sh, "settings-octave-wait").is_some_and(|l| l.contains(": 0 ms")),
        "{:?}",
        label_of(cx, sh, "settings-octave-wait")
    );
}

// ---------- 리뷰 2: 진입 처리기와 배치 뷰 ----------

/// 배치 뷰(Normal)에서 vim `A`: 진입 처리기가 `step_handlers`로 옮겨진 뒤에도 Step으로 들어가는가, 커서는 어디인가.
#[gpui_kit::test]
fn r2_entry_from_arrangement_view(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-arrangement-entry",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, 2 * BAR, &[(0, 960, 60)])]),
    );
    event(cx, handle, &view, ModeEvent::ToggleView);
    set_cursor(cx, &view, 960, 60);
    render(cx, handle);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        keyscore_app::mode::View::Arrangement
    );
    press(cx, handle, "shift-a");
    assert_eq!((mode(cx, &view), cursor(cx, &view)), (Mode::Step, BAR));
    press(cx, handle, "escape");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "a");
    assert_eq!((mode(cx, &view), cursor(cx, &view)), (Mode::Step, 960));
}

/// 메뉴·팔레트 경로: Step 안에서 덮어쓰기 중 `mode::StepAtBarStart`를 고르면 덮어쓰기가 꺼지고 마디 시작으로 간다.
/// 직전 스텝이 커서에서 끝나지 않으므로 타이는 거부된다.
#[gpui_kit::test]
fn r2_reentry_moves_and_drops_tie_target(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "r2-reentry-tie",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[])]),
    );
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "shift-r a");
    run(cx, handle, mode_action::StepAtBarStart);
    assert!(!status(cx, &view).overwrite);
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(notes(cx, &view), vec![(960, 960, 60)]);
    press(cx, handle, "~");
    assert_eq!(notes(cx, &view), vec![(960, 960, 60)]);
    assert!(last_message(cx, &view).contains("타이"));
}

/// 리뷰 1 지적 7: `Shift+Backspace`의 자취 처리. 타이로 마디를 넘긴 스텝 뒤 마디 처음까지 지우면 그 스텝 기록은
/// 버려지고, 이어지는 `Backspace`는 현재 길이 한 스텝이다. undo 한 번은 그 지움만 되살린다.
#[gpui_kit::test]
fn r2_shift_backspace_after_tie_across_bar(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "r2-bar-back-tie",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, 2 * BAR, &[])]),
    );
    set_cursor(cx, &view, 2880, 60);
    press(cx, handle, "a");
    wait(cx, handle, WAIT);
    press(cx, handle, "~ s");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(2880, 1920, 60), (4800, 960, 62)]);
    assert_eq!(cursor(cx, &view), 5760);
    let before = undo_len(cx, &view);
    press(cx, handle, "shift-backspace");
    assert_eq!(cursor(cx, &view), BAR);
    assert_eq!(notes(cx, &view), vec![(2880, 1920, 60)]);
    assert_eq!(undo_len(cx, &view), before + 1);
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 2880);
    assert_eq!(notes(cx, &view), vec![]);
}
