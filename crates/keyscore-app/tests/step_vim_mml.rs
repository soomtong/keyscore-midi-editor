//! S3-T4b vim Step 진입·덮어쓰기·`Shift` 조합, 계이름 MML 호환, 설정 창(REQ 5.1, 5.6, 5.8.1, 5.11). 규칙은
//! `docs/decisions/2026-10-30-s3-t4b-vim-step-mml.md`, 시험 목록은 `docs/reports/s3-t4b/plan.md`.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{Int, KeyLayout, LengthCode, Opt, SettingsSection};
use keyscore_app::actions::{settings, step, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::settings_window::{self, SettingsWindow};
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
    let dir = std::env::temp_dir().join(format!("ks-step-vim-{}-{name}", std::process::id()));
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

/// 피아노롤 Normal 창(커서 틱 0, C4). `preset`과 `layout`을 건다. 이동 시 미리듣기는 끈다.
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

/// `setup` 뒤 곧바로 Step 모드(커서 틱 0).
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

fn plain(code: LengthCode) -> StepLength {
    StepLength::plain(code)
}

/// 지금 키 컨텍스트에서 키 하나가 내는 Action 이름.
fn action_for(cx: &mut TestAppContext, handle: AnyWindowHandle, key: &str) -> Option<String> {
    cx.update_window(handle, |_, window, cx| {
        let stroke = gpui_kit::Keystroke::parse(key).unwrap();
        let stack = window.context_stack();
        let (found, _) = cx
            .key_bindings()
            .borrow()
            .bindings_for_input(&[stroke], &stack);
        found.first().map(|b| b.action().name().to_string())
    })
    .unwrap()
}

// ---------- 1–3. vim 진입 위치 ----------

#[gpui_kit::test]
fn vim_i_enters_step_at_cursor(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "vim-i",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 60), (960, 960, 62)])]),
    );
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "i");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(cursor(cx, &view), 960);
    assert!(!status(cx, &view).overwrite);
    press(cx, handle, "a");
    wait(cx, handle, WAIT);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (960, 960, 60), (1920, 960, 62)],
        "삽입: 커서 뒤 노트를 민다"
    );
}

#[gpui_kit::test]
fn vim_a_enters_after_cursor_note_or_at_cursor(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "vim-a",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 60), (1920, 480, 64)])]),
    );
    // 노트 위: 그 노트의 끝
    set_cursor(cx, &view, 480, 60);
    press(cx, handle, "a");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(cursor(cx, &view), 960);
    press(cx, handle, "escape");
    assert_eq!(mode(cx, &view), Mode::Normal);
    // 노트가 없는 칸: 그대로
    set_cursor(cx, &view, 480, 70);
    press(cx, handle, "a");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(cursor(cx, &view), 480);
    press(cx, handle, "escape");
    // 노트 시작 칸에서도 그 노트의 끝
    set_cursor(cx, &view, 1920, 64);
    press(cx, handle, "a");
    assert_eq!(cursor(cx, &view), 2400);
}

#[gpui_kit::test]
fn vim_shift_i_and_shift_a_enter_at_bar_start_and_end(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "vim-ia",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, 2 * BAR, &[])]),
    );
    set_cursor(cx, &view, BAR + 960, 60);
    press(cx, handle, "shift-i");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(cursor(cx, &view), BAR);
    press(cx, handle, "escape");
    set_cursor(cx, &view, BAR + 960, 60);
    press(cx, handle, "shift-a");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(cursor(cx, &view), 2 * BAR);
    press(cx, handle, "escape");
    // 마디 시작에서: `I`는 그 마디 시작, `A`는 그 마디 끝
    set_cursor(cx, &view, BAR, 60);
    press(cx, handle, "shift-i");
    assert_eq!(cursor(cx, &view), BAR);
    press(cx, handle, "escape");
    set_cursor(cx, &view, BAR, 60);
    press(cx, handle, "shift-a");
    assert_eq!(cursor(cx, &view), 2 * BAR);
    assert!(!status(cx, &view).overwrite);
}

// ---------- 4–8. 덮어쓰기 ----------

#[gpui_kit::test]
fn vim_shift_r_overwrites_without_pushing(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "vim-r",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(
            0,
            BAR,
            &[
                (0, 960, 60),
                (960, 480, 62),
                (1440, 480, 64),
                (1920, 960, 65),
            ],
        )]),
    );
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "shift-r");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert!(status(cx, &view).overwrite);
    assert_eq!(label(cx, &view), "STEP R ♩ oct4");
    let before = undo_len(cx, &view);
    press(cx, handle, "g");
    wait(cx, handle, WAIT);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (960, 960, 67), (1920, 960, 65)],
        "[960, 1920)에서 시작하는 노트만 바뀌고 뒤 노트는 그대로"
    );
    assert_eq!(cursor(cx, &view), 1920);
    assert_eq!(undo_len(cx, &view), before + 1);
}

#[gpui_kit::test]
fn overwrite_rest_tie_and_backspace_do_not_shift(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "over-rest",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(
            0,
            BAR,
            &[
                (0, 960, 60),
                (960, 960, 62),
                (1920, 960, 64),
                (2880, 960, 65),
            ],
        )]),
    );
    press(cx, handle, "shift-r");
    press(cx, handle, "space");
    assert_eq!(
        notes(cx, &view),
        vec![(960, 960, 62), (1920, 960, 64), (2880, 960, 65)],
        "쉼표가 그 스텝의 노트를 지운다"
    );
    assert_eq!(cursor(cx, &view), 960);
    press(cx, handle, "a");
    wait(cx, handle, WAIT);
    assert_eq!(
        notes(cx, &view),
        vec![(960, 960, 60), (1920, 960, 64), (2880, 960, 65)]
    );
    press(cx, handle, "~");
    assert_eq!(
        notes(cx, &view),
        vec![(960, 1920, 60), (2880, 960, 65)],
        "타이가 늘리고 덮인 노트를 지운다"
    );
    assert_eq!(cursor(cx, &view), 2880);
    press(cx, handle, "backspace");
    assert_eq!(
        notes(cx, &view),
        vec![(2880, 960, 65)],
        "지우기만 하고 당기지 않는다"
    );
    assert_eq!(cursor(cx, &view), 960);
    assert_eq!(clips(cx, &view), vec![(0, BAR)]);
}

#[gpui_kit::test]
fn overwrite_chord_removes_range_once(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "over-chord",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 60), (960, 960, 62)])]),
    );
    press(cx, handle, "shift-r");
    press(cx, handle, "ctrl-a ctrl-d g");
    wait(cx, handle, WAIT);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (0, 960, 64), (0, 960, 67), (960, 960, 62)]
    );
    assert_eq!(cursor(cx, &view), 960);
}

#[gpui_kit::test]
fn overwrite_ends_when_leaving_step_and_plain_entry_inserts(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "over-leave",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 62)])]),
    );
    press(cx, handle, "shift-r");
    assert!(status(cx, &view).overwrite);
    press(cx, handle, "escape");
    assert!(!status(cx, &view).overwrite);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "i a");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (960, 960, 62)]);
    // Ctrl+Tab으로 들어가도 삽입
    press(cx, handle, "escape shift-r escape ctrl-tab");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert!(!status(cx, &view).overwrite);
}

#[gpui_kit::test]
fn overwrite_extends_clip_at_end(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "over-extend",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 60)])]),
    );
    set_cursor(cx, &view, BAR, 60);
    press(cx, handle, "shift-r a");
    wait(cx, handle, WAIT);
    assert_eq!(clips(cx, &view), vec![(0, 2 * BAR)]);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (BAR, 960, 60)]);
}

// ---------- 9–13. vim Step 표의 Shift 조합 ----------

#[gpui_kit::test]
fn shift_space_rests_half_length_once(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "half-rest",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(0, 960, 60)])]),
    );
    press(cx, handle, "shift-space");
    assert_eq!(cursor(cx, &view), 480);
    assert_eq!(notes(cx, &view), vec![(480, 960, 60)], "삽입이라 민다");
    press(cx, handle, "space");
    assert_eq!(cursor(cx, &view), 1440, "다음 쉼표는 원래 길이");
    press(cx, handle, "n shift-space");
    assert_eq!(cursor(cx, &view), 1760, "셋잇단 절반 = 320");
    assert_eq!(status(cx, &view).length, plain(LengthCode::Quarter));
}

#[gpui_kit::test]
fn shift_backspace_deletes_to_bar_start_and_pulls(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "bar-back",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(
            0,
            2 * BAR,
            &[
                (0, 960, 60),
                (960, 960, 62),
                (1920, 960, 64),
                (2880, 960, 65),
                (BAR, 960, 67),
                (BAR + 960, 960, 69),
            ],
        )]),
    );
    set_cursor(cx, &view, BAR + 960, 60);
    press(cx, handle, "i");
    // 조합은 버린다(노트를 쓰지 않는다)
    press(cx, handle, "a shift-backspace");
    assert_eq!(status(cx, &view).composing, None);
    assert_eq!(
        notes(cx, &view),
        vec![
            (0, 960, 60),
            (960, 960, 62),
            (1920, 960, 64),
            (2880, 960, 65),
            (BAR, 960, 69)
        ],
        "마디 중간: 그 마디 시작까지 지우고 당긴다"
    );
    assert_eq!(cursor(cx, &view), BAR);
    press(cx, handle, "shift-backspace");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 69)],
        "마디 시작: 앞 마디 시작까지"
    );
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(clips(cx, &view), vec![(0, 2 * BAR)], "클립은 줄이지 않는다");
}

#[gpui_kit::test]
fn braces_shift_composing_note_octave_once(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "octave-once",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[]),
    );
    press(cx, handle, "a }");
    assert_eq!(label(cx, &view), "STEP ♩ oct4 c'_");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 960, 72)]);
    assert_eq!(status(cx, &view).octave, 4, "스티키 옥타브는 그대로");
    press(cx, handle, "a { {");
    assert_eq!(label(cx, &view), "STEP ♩ oct4 c,,_");
    wait(cx, handle, WAIT);
    press(cx, handle, "a");
    wait(cx, handle, WAIT);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 72), (960, 960, 36), (1920, 960, 60)]
    );
    // 조합이 없으면 알리고 아무것도 하지 않는다
    press(cx, handle, "}");
    assert!(last_message(cx, &view).contains("입력 중인 음"));
    assert_eq!(status(cx, &view).octave, 4);
    // 0 아래로는 가지 않는다
    press(cx, handle, "[ [ [ [ a {");
    assert!(last_message(cx, &view).contains("옥타브"));
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view).last(), Some(&(2880, 960, 12)));
}

#[gpui_kit::test]
fn tie_legato_from_palette_connects_last_step(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "tie-legato",
        Preset::Vim,
        KeyLayout::Piano,
        project(&[(0, BAR, &[(1920, 960, 64)])]),
    );
    press(cx, handle, "a");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (2880, 960, 64)]);
    let before = undo_len(cx, &view);
    run(cx, handle, step::TieLegato);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 2880, 60), (2880, 960, 64)],
        "다음 노트 시작에 붙인다"
    );
    assert_eq!(undo_len(cx, &view), before + 1);
    run(cx, handle, step::TieLegato);
    assert!(last_message(cx, &view).contains("레가토"));
    assert_eq!(undo_len(cx, &view), before + 1);
    press(cx, handle, "space");
    run(cx, handle, step::TieLegato);
    assert!(last_message(cx, &view).contains("바로 앞에 친 음이 없다"));
    // 조합 중이면 확정한 뒤 그 음을 레가토로
    // (쉼표가 64를 3840으로, D4가 4800으로 밀었다)
    press(cx, handle, "s");
    run(cx, handle, step::TieLegato);
    assert!(
        notes(cx, &view).contains(&(1920, 2880, 62)),
        "{:?}",
        notes(cx, &view)
    );
}

#[gpui_kit::test]
fn preview_from_bar_leaves_step_and_plays_from_bar_start(cx: &mut TestAppContext) {
    start(cx, "preview-bar");
    let options = EngineOptions {
        client_name: "ks-step-vim-preview".to_owned(),
        port_name: format!("KS step vim {} preview", std::process::id()),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let (handle, view) = cx
        .update(|cx| open_project_window(project(&[(0, 2 * BAR, &[(0, 960, 60)])]), cx))
        .unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        AppSettings::update(cx, |s| s.preview.on_move = false);
        keymap::set_preset(cx, Preset::Vim);
    });
    event(cx, handle, &view, ModeEvent::ToggleView);
    set_cursor(cx, &view, BAR + 960, 60);
    press(cx, handle, "i a shift-enter");
    assert_eq!(mode(cx, &view), Mode::Normal, "입력을 마치고 Normal로");
    assert!(
        notes(cx, &view).contains(&(BAR + 960, 960, 60)),
        "조합 확정"
    );
    assert_eq!(
        cx.read(|cx| view.read(cx).transport_state()),
        TransportState::Playing
    );
    std::thread::sleep(Duration::from_millis(60));
    render(cx, handle);
    let head = cx
        .read(|cx| view.read(cx).piano_roll_playhead())
        .expect("재생 중");
    assert!(head.get() >= BAR, "{head:?}");
    assert!(head.get() < BAR + 960, "{head:?}");
    run(cx, handle, transport::Stop);
}

// ---------- 14–19. 계이름 MML 호환 ----------

#[gpui_kit::test]
fn mml_octave_sets_sticky_octave(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-o",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "o 5 c");
    wait(cx, handle, WAIT);
    assert_eq!(status(cx, &view).octave, 5);
    press(cx, handle, "c o 3 c");
    wait(cx, handle, WAIT);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 72), (960, 960, 72), (1920, 960, 48)],
        "`c o3`의 c는 앞 옥타브"
    );
    assert_eq!(status(cx, &view).octave, 3);
}

#[gpui_kit::test]
fn mml_length_reads_denominators_and_dot(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-l",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "l 8 c");
    wait(cx, handle, WAIT);
    assert_eq!(status(cx, &view).length, plain(LengthCode::Eighth));
    // 두 자리는 늦게 쳐도 하나로 읽는다(대기 만료 없음)
    press(cx, handle, "l 1");
    wait(cx, handle, Duration::from_secs(2));
    press(cx, handle, "6 c");
    wait(cx, handle, WAIT);
    assert_eq!(status(cx, &view).length, plain(LengthCode::Sixteenth));
    press(cx, handle, "l 4 . c");
    wait(cx, handle, WAIT);
    assert_eq!(
        status(cx, &view).length,
        StepLength {
            code: LengthCode::Quarter,
            dotted: true
        }
    );
    press(cx, handle, "l 1 c");
    wait(cx, handle, WAIT);
    assert_eq!(status(cx, &view).length, plain(LengthCode::Whole));
    press(cx, handle, "l 3 c");
    assert!(
        last_message(cx, &view).contains("l 뒤"),
        "잘못된 분모는 알림"
    );
    wait(cx, handle, WAIT);
    assert_eq!(status(cx, &view).length, plain(LengthCode::Whole));
    assert_eq!(
        notes(cx, &view),
        vec![
            (0, 480, 60),
            (480, 240, 60),
            (720, 1440, 60),
            (2160, 3840, 60),
            (6000, 3840, 60)
        ]
    );
}

#[gpui_kit::test]
fn mml_rest_is_space(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-r",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "c r c");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (1920, 960, 60)]);
    assert_eq!(cursor(cx, &view), 2880);
}

#[gpui_kit::test]
fn mml_wait_shows_status_and_backspace_discards(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-status",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(cx, handle, "l 1 6");
    assert_eq!(label(cx, &view), "STEP ♩ oct4 l16_");
    assert_eq!(path(cx, &view), vec!["Step", "Solfege", "Mml"]);
    press(cx, handle, "backspace");
    assert_eq!(status(cx, &view).composing, None);
    assert_eq!(status(cx, &view).length, plain(LengthCode::Quarter));
    assert_eq!(path(cx, &view), vec!["Step", "Solfege"]);
    press(cx, handle, "l 1 6 enter");
    assert_eq!(status(cx, &view).length, plain(LengthCode::Sixteenth));
    assert_eq!(notes(cx, &view), vec![]);
    assert_eq!(cursor(cx, &view), 0);
    press(cx, handle, "o");
    assert_eq!(label(cx, &view), "STEP 1/16 oct4 o_");
    press(cx, handle, "enter");
    assert!(last_message(cx, &view).contains("옥타브"));
    assert_eq!(status(cx, &view).octave, 4);
    // 모드를 나가면 확정한다
    press(cx, handle, "o 6 l 2 escape");
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(status(cx, &view).octave, 6);
    assert_eq!(status(cx, &view).length, plain(LengthCode::Half));
}

#[gpui_kit::test]
fn req_5_1_twinkle_in_mml_commands(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-twinkle",
        Preset::Default,
        KeyLayout::Solfege,
        project(&[]),
    );
    press(
        cx,
        handle,
        "o 4 l 4 c c g g a a l 2 g l 4 f f e e d d l 2 c",
    );
    wait(cx, handle, WAIT);
    let q = 960u32;
    let h = 1920u32;
    let mut want = Vec::new();
    let mut at = 0u64;
    for (len, pitch) in [
        (q, 60),
        (q, 60),
        (q, 67),
        (q, 67),
        (q, 69),
        (q, 69),
        (h, 67),
        (q, 65),
        (q, 65),
        (q, 64),
        (q, 64),
        (q, 62),
        (q, 62),
        (h, 60),
    ] {
        want.push((at, len, pitch));
        at += u64::from(len);
    }
    assert_eq!(notes(cx, &view), want);
    assert_eq!(cursor(cx, &view), 4 * BAR);
}

#[gpui_kit::test]
fn mml_keys_only_in_solfege_layout(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "mml-piano",
        Preset::Default,
        KeyLayout::Piano,
        project(&[]),
    );
    assert_eq!(action_for(cx, handle, "o").as_deref(), Some("step::Note"));
    assert_eq!(action_for(cx, handle, "l").as_deref(), Some("step::Note"));
    press(cx, handle, "o");
    assert_eq!(path(cx, &view), vec!["Step", "Piano", "Pending"]);
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 960, 73)]);
}

// ---------- 20–21. 설정 ----------

fn settings_view(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<SettingsWindow>) {
    let (sh, sv) = cx
        .update(|cx| settings_window::current(cx))
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

#[gpui_kit::test]
fn settings_input_has_step_length_and_wait_choices(cx: &mut TestAppContext) {
    let (handle, _view) = setup(
        cx,
        "settings-ui",
        Preset::Default,
        KeyLayout::Piano,
        project(&[]),
    );
    run(
        cx,
        handle,
        settings::Open(Opt(Some(SettingsSection::Input))),
    );
    let (sh, view) = settings_view(cx);
    let focused = |cx: &mut TestAppContext| {
        cx.update_window(sh, |_, window, cx| {
            view.read(cx).focused_control(window, cx)
        })
        .unwrap()
    };
    assert!(
        label_of(cx, sh, "settings-step-length").is_some_and(|l| l.contains("4분음표")),
        "{:?}",
        label_of(cx, sh, "settings-step-length")
    );
    assert!(label_of(cx, sh, "settings-octave-wait").is_some_and(|l| l.contains("300 ms")));
    // 레이턴시 보정 뒤에 Step 입력의 두 상자가 선다
    press(cx, sh, "tab tab tab tab");
    assert_eq!(focused(cx).as_deref(), Some("settings-step-length"));
    press(cx, sh, "enter down enter");
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).input.step_default_length),
        LengthCode::Eighth
    );
    press(cx, sh, "tab");
    assert_eq!(focused(cx).as_deref(), Some("settings-octave-wait"));
    press(cx, sh, "enter down down enter");
    assert_eq!(cx.read(|cx| AppSettings::get(cx).input.octave_wait_ms), 500);
    assert!(label_of(cx, sh, "settings-octave-wait").is_some_and(|l| l.contains("500 ms")));
    assert!(label_of(cx, sh, "settings-step-length").is_some_and(|l| l.contains("8분음표")));
}

#[gpui_kit::test]
fn settings_actions_set_step_default_length_and_wait(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(
        cx,
        "settings-actions",
        Preset::Default,
        KeyLayout::Piano,
        project(&[]),
    );
    run(
        cx,
        handle,
        settings::SetStepDefaultLength(LengthCode::Eighth),
    );
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).input.step_default_length),
        LengthCode::Eighth
    );
    assert_eq!(
        status(cx, &view).length,
        plain(LengthCode::Quarter),
        "열린 창의 지금 길이는 그대로"
    );
    press(cx, handle, "0");
    assert_eq!(status(cx, &view).length, plain(LengthCode::Eighth));
    run(cx, handle, settings::SetOctaveWait(Int(1000)));
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).input.octave_wait_ms),
        1000
    );
    run(cx, handle, settings::SetOctaveWait(Int(5000)));
    run(cx, handle, settings::SetOctaveWait(Int(-1)));
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).input.octave_wait_ms),
        1000
    );
    press(cx, handle, "a");
    wait(cx, handle, Duration::from_millis(999));
    assert!(status(cx, &view).composing.is_some());
    wait(cx, handle, Duration::from_millis(1));
    assert_eq!(notes(cx, &view), vec![(0, 480, 60)]);
}

// ---------- 22. 키맵 ----------

#[gpui_kit::test]
fn step_shift_keys_have_no_conflicts_in_vim_preset(cx: &mut TestAppContext) {
    let (handle, view) = setup_step(cx, "conflicts", Preset::Vim, KeyLayout::Piano, project(&[]));
    for layout in [KeyLayout::Piano, KeyLayout::Solfege] {
        cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = layout));
        cx.run_until_parked();
        render(cx, handle);
        for (key, want) in [
            ("{", "step::OctaveOnce"),
            ("}", "step::OctaveOnce"),
            ("shift-space", "step::HalfRest"),
            ("shift-backspace", "step::BackToBarStart"),
            ("shift-enter", "step::PreviewFromBar"),
        ] {
            assert_eq!(
                action_for(cx, handle, key).as_deref(),
                Some(want),
                "{layout:?} {key}"
            );
        }
        if layout == KeyLayout::Solfege {
            for (key, want) in [
                ("o", "step::MmlOctave"),
                ("l", "step::MmlLength"),
                ("r", "step::Rest"),
            ] {
                assert_eq!(action_for(cx, handle, key).as_deref(), Some(want), "{key}");
            }
            press(cx, handle, "l");
            assert_eq!(path(cx, &view), vec!["Step", "Solfege", "Mml"]);
            for key in ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"] {
                assert_eq!(
                    action_for(cx, handle, key).as_deref(),
                    Some("step::MmlDigit"),
                    "{key}"
                );
            }
            assert_eq!(action_for(cx, handle, ".").as_deref(), Some("step::MmlDot"));
            for key in ["c", "r", "space", "enter", "backspace"] {
                let got = action_for(cx, handle, key);
                assert!(
                    got.as_deref().is_some_and(|a| a.starts_with("step::")),
                    "{key}: {got:?}"
                );
            }
            press(cx, handle, "backspace");
        }
    }
}
