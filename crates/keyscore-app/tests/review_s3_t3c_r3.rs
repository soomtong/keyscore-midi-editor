//! S3-T3c 적대적 리뷰 3회차의 깨뜨리기 시도(`docs/reports/s3-t3c/review-3.md`). 픽스처는 2회차
//! `review_s3_t3c_r2.rs`에서 복사했다. 실패하는 시험은 리뷰어가 기대한 동작을 단언한 지적의 재현이고, 통과하는 시험은
//! 「못 깨뜨림」 또는 지금 동작의 기록이다.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::clipboard;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::pianoroll::vim::{Operator, VisualKind};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const BAR: u64 = 3840;

type Triple = (u64, u32, u8);

const C4: Triple = (0, 960, 60);
const D4: Triple = (960, 960, 62);
const E4: Triple = (1920, 960, 64);
const G4: Triple = (1920, 960, 67);
const D4B: Triple = (2880, 960, 62);
const A4: Triple = (3840, 960, 69);
const ALL: [Triple; 6] = [C4, D4, E4, G4, D4B, A4];

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str, preset: Preset) {
    let dir = std::env::temp_dir().join(format!("ks-rvim3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-rvim3-{name}", std::process::id()),
        );
        if preset == Preset::Vim {
            keymap::set_preset(cx, Preset::Vim);
        }
    });
}

fn note(t: Triple) -> Note {
    Note::new(
        Tick::new(t.0),
        t.1,
        Pitch::new(t.2).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn triple(n: &Note) -> Triple {
    (n.start().get(), n.length(), n.pitch().get())
}

/// 클립 A [0, 2마디): C4 D4 (E4+G4) D4B | A4. 클립 B [4마디, 5마디): A4.
fn fixture() -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut a = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    for n in ALL {
        a.insert_note(note(n)).unwrap();
    }
    track.insert_clip(a).unwrap();
    let mut b = Clip::new(Tick::new(4 * BAR), BAR).unwrap();
    b.insert_note(note((0, 960, 69))).unwrap();
    track.insert_clip(b).unwrap();
    project.push_track(track).unwrap();
    project
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

/// 창을 열고 피아노롤로 들어간다(`enter` = `clip::Open`).
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(fixture(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    (handle, view)
}

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name, Preset::Vim);
    open(cx)
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

/// 시퀀스 대기 시간(1 s)을 넘긴다.
fn wait(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.executor().advance_clock(Duration::from_millis(1100));
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

fn cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (u64, u8) {
    cx.read(|cx| {
        let c = view.read(cx).piano_roll().cursor();
        (c.tick.get(), c.pitch.get())
    })
}

fn clip_a(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .next()
            .unwrap()
            .1
            .notes()
            .iter()
            .map(triple)
            .collect()
    })
}

fn selection_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    cx.read(|cx| {
        let mut v: Vec<_> = view.read(cx).selection().notes().map(triple).collect();
        v.sort_unstable();
        v
    })
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

fn pending(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
) -> Option<(Operator, Option<u32>)> {
    cx.read(|cx| view.read(cx).pending_operator())
}

fn visual(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<VisualKind> {
    cx.read(|cx| view.read(cx).piano_roll().visual())
}

fn status_mode(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    cx.update_window(handle, |_, window, _| {
        window
            .find("status-mode")
            .label()
            .unwrap_or_default()
            .to_owned()
    })
    .unwrap()
}

/// 내부 클립보드의 노트(가장 이른 노트 기준).
fn internal(cx: &mut TestAppContext) -> Option<Vec<Triple>> {
    cx.read(|cx| {
        clipboard::internal(cx).map(|c| {
            let mut v: Vec<Triple> = c.notes().iter().map(triple).collect();
            v.sort_unstable();
            v
        })
    })
}

fn without(gone: &[Triple]) -> Vec<Triple> {
    ALL.iter().copied().filter(|n| !gone.contains(n)).collect()
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: Box<dyn gpui_kit::Action>) {
    cx.update_window(handle, |_, window, cx| {
        keyscore_app::dispatch::run_action(action, window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn context_path(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<&'static str> {
    cx.read(|cx| view.read(cx).context_path().to_vec())
}

/// 그려진 창의 디스패치 경로(포커스 노드의 컨텍스트 스택). GPUI는 이 경로로 키를 맞춘다.
fn rendered_stack(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    cx.update_window(handle, |_, window, _| {
        format!("{:?}", window.context_stack())
    })
    .unwrap()
}

fn click(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    tick: u64,
    pitch: u8,
) {
    view.update(cx, |this, cx| {
        this.click_piano_roll_cell(
            Cursor {
                tick: Tick::new(tick),
                pitch: Pitch::new(pitch).unwrap(),
            },
            cx,
        );
    });
    cx.run_until_parked();
    render(cx, handle);
}

// ---------- 지적 1 수정(f4b0b32): 붙든 경로에서 비운 뒤 다시 보낸 키 ----------

/// `d i` 중 클릭으로 대기가 끝난 뒤 `i`: 새 키이므로 Normal의 `i`(Step 모드)여야 한다(계획 2절 「그 키를 새로
/// 보낸다」). 다시 보내는 키는 비우기의 알림 뒤 다시 그리기 전에 디스패치되므로 붙든 `Operator` 경로로 맞춰질 수
/// 있다.
#[gpui_kit::test]
fn object_prefix_then_click_then_i_enters_step(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-click-i");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    click(cx, handle, &view, 0, 60);
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "i");
    assert_eq!(
        mode(cx, &view),
        Mode::Step,
        "클릭 뒤 새로 누른 i가 Step으로 가지 않았다: stack={}",
        rendered_stack(cx, handle)
    );
}

/// 같은 상태에서 `a`(커서 노트 뒤 Step).
#[gpui_kit::test]
fn object_prefix_then_menu_select_all_then_a_enters_step(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-all-a");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    press(cx, handle, "escape");
    press(cx, handle, "a");
    assert_eq!(
        mode(cx, &view),
        Mode::Step,
        "Esc 뒤 a가 Step으로 가지 않았다"
    );
}

/// `d i` 중 클릭 뒤 `3 l`: 다시 보낸 카운트가 다음 이동에 산다.
#[gpui_kit::test]
fn object_prefix_then_click_then_count_motion(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-click-3l");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    click(cx, handle, &view, 0, 60);
    press(cx, handle, "3 l");
    assert_eq!(cursor(cx, &view), (2880, 60));
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

/// `d i` 중 클릭 뒤 `d w`: 다시 보낸 `d`가 대기에 들고 `w`가 지운다.
#[gpui_kit::test]
fn object_prefix_then_click_then_delete_word(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-click-dw");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    click(cx, handle, &view, 0, 60);
    press(cx, handle, "d w");
    assert_eq!(clip_a(cx, &view), without(&[C4]));
    assert_eq!(pending(cx, &view), None);
}

// ---------- 불변식: `Operator`·`Visual`은 Normal + 피아노롤일 때만 ----------

/// `d i` 중 메뉴 `view::Toggle`(배치 뷰): 대기 입력이 남아 있어 창이 붙든 `Normal > PianoRoll > Operator`로
/// 배치 뷰를 그리는가. 계획 「불변식」 「키 컨텍스트 경로의 `Operator`·`Visual`은 Normal + 피아노롤일 때만 붙는다」.
#[gpui_kit::test]
fn object_prefix_then_menu_view_toggle_draws_arrangement_context(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-toggle-stack");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    run(cx, handle, Box::new(keyscore_app::actions::view::Toggle));
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    let stack = rendered_stack(cx, handle);
    assert!(
        !stack.contains("Operator") && !stack.contains("PianoRoll"),
        "배치 뷰가 피아노롤 대기 경로로 그려졌다: {stack}"
    );
}

/// 같은 상태에서 배치 뷰의 첫 키 `enter`(`clip::Open`): 붙든 피아노롤 경로로 맞춰지면 `note::Audition`이 되어
/// 피아노롤로 들어가지 않는다.
#[gpui_kit::test]
fn object_prefix_then_menu_view_toggle_then_enter_opens_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-toggle-enter");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    run(cx, handle, Box::new(keyscore_app::actions::view::Toggle));
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll,
        "배치 뷰의 enter가 clip::Open으로 가지 않았다"
    );
}

/// `d i` 중 메뉴 `mode::Step`: 대기 입력이 남은 동안 Step 모드가 `Normal > PianoRoll > Operator`로 그려진다.
/// 다음 키는 붙든 경로에서 비운 뒤 다시 그린 경로로 다시 보내므로 키 동작은 맞다(아래 `enter` 시험과 같은 길).
#[gpui_kit::test]
fn object_prefix_then_menu_step_draws_operator_context_records(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-step-stack");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    run(cx, handle, Box::new(keyscore_app::actions::mode::Step));
    assert_eq!(mode(cx, &view), Mode::Step);
    let stack = rendered_stack(cx, handle);
    // 구현자 주(리뷰 3 지적 1 나 수정): 상태가 키 밖에서 끝나면 그리기가 곧바로 붙든 경로에서 대기 입력을 비우고
    // 붙들기를 놓는다. 그래서 Step 모드는 `Operator`로 그려지지 않는다(기록 단언을 수정 뒤 동작으로 뒤집음).
    assert!(
        !stack.contains("Operator"),
        "Step 모드가 Operator로 그려진다: {stack}"
    );
    assert!(
        !cx.update_window(handle, |_, window, _| window.has_pending_keystrokes())
            .unwrap(),
        "대기 입력이 남았다"
    );
    press(cx, handle, "escape");
    let stack = rendered_stack(cx, handle);
    assert!(!stack.contains("Operator"), "Esc 뒤: {stack}");
}

/// `d i` 중 클릭 뒤 수식키만 누른 입력(`shift` 단독, macOS에서 Shift를 눌렀다 떼면 GPUI가 만든다). 가로채기는
/// 수식키를 거르므로 붙든 경로의 비우기·다시 그리기가 돌지 않고, GPUI가 접두 `i`를 붙든 경로에서 비운다(알림 없음).
/// 그린 경로가 `Operator`로 남으면 다음 `i`가 그 경로의 `null`·접두로 맞춰져 Step으로 가지 않는다.
#[gpui_kit::test]
fn object_prefix_then_click_then_shift_then_i_enters_step(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-click-shift-i");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    click(cx, handle, &view, 0, 60);
    cx.simulate_keystrokes(handle, "shift");
    cx.run_until_parked();
    let has_pending = cx
        .update_window(handle, |_, window, _| window.has_pending_keystrokes())
        .unwrap();
    let stack = rendered_stack(cx, handle);
    cx.simulate_keystrokes(handle, "i");
    cx.run_until_parked();
    assert_eq!(
        mode(cx, &view),
        Mode::Step,
        "shift 뒤 i가 Step으로 가지 않았다(shift 뒤 대기 입력 {has_pending}, 그린 경로 {stack}, 지금 {})",
        rendered_stack(cx, handle)
    );
}

/// 같은 갈래, 비주얼: `v i` 중 메뉴 `select::All`(비주얼 끝) 뒤 `shift` 단독, 이어 `escape`(못 깨뜨림: 그린 경로
/// `Visual`에서도 Esc는 선택을 지운다).
#[gpui_kit::test]
fn visual_object_prefix_then_menu_select_all_then_shift_then_escape_clears(
    cx: &mut TestAppContext,
) {
    let (handle, view) = setup(cx, "vi-all-shift-i");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v i");
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    cx.simulate_keystrokes(handle, "shift");
    cx.run_until_parked();
    let stack = rendered_stack(cx, handle);
    cx.simulate_keystrokes(handle, "escape");
    cx.run_until_parked();
    assert!(
        selection_of(cx, &view).is_empty(),
        "shift 뒤 Esc가 선택을 지우지 않았다(그린 경로 {stack})"
    );
}

// ---------- 정상 시퀀스 회귀 ----------

#[gpui_kit::test]
fn visual_then_g_g_still_moves(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-gg");
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, "v g g");
    assert_eq!(cursor(cx, &view).0, 0);
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
    assert!(selection_of(cx, &view).contains(&D4B));
}

#[gpui_kit::test]
fn delete_g_g_still_deletes(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "d-gg");
    set_cursor(cx, &view, 1920, 64);
    press(cx, handle, "d g g");
    assert_eq!(pending(cx, &view), None);
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert!(!clip_a(cx, &view).contains(&C4));
    assert!(!clip_a(cx, &view).contains(&D4));
}

#[gpui_kit::test]
fn normal_sequences_after_held_path(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "seq-after-held");
    set_cursor(cx, &view, 960, 62);
    // 붙든 경로를 남긴다: v i n(비주얼 객체) → escape
    press(cx, handle, "v i n");
    press(cx, handle, "escape");
    assert_eq!(visual(cx, &view), None);
    assert_eq!(context_path(cx, &view), vec!["Normal", "PianoRoll"]);
    press(cx, handle, "m x");
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, "' x");
    assert_eq!(cursor(cx, &view), (960, 62), "' x");
    press(cx, handle, "z z");
    press(cx, handle, "2 g g");
    assert_eq!(cursor(cx, &view).0, 0, "2 g g");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y y p");
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "i");
    assert_eq!(mode(cx, &view), Mode::Step);
}

#[gpui_kit::test]
fn visual_object_prefix_escape_escape_then_i_enters_step(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vi-esc-esc-i");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v i");
    press(cx, handle, "escape");
    assert_eq!(
        visual(cx, &view),
        Some(VisualKind::Note),
        "첫 Esc는 시퀀스만"
    );
    press(cx, handle, "escape");
    assert_eq!(visual(cx, &view), None);
    press(cx, handle, "i");
    assert_eq!(mode(cx, &view), Mode::Step);
}

/// 기본 프리셋(멀티키 시퀀스 없음): `cmd-left`, `x` 뒤에도 그린 경로에 `Operator`·`Visual`이 붙지 않는다.
#[gpui_kit::test]
fn default_preset_keys_unaffected(cx: &mut TestAppContext) {
    start(cx, "default-gg", Preset::Default);
    let (handle, view) = open(cx);
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, "cmd-left");
    assert_eq!(cursor(cx, &view).0, 0);
    press(cx, handle, "x");
    assert_eq!(context_path(cx, &view), vec!["Normal", "PianoRoll"]);
    let stack = rendered_stack(cx, handle);
    assert!(
        !stack.contains("Operator") && !stack.contains("Visual"),
        "{stack}"
    );
}

// ---------- 지적 2·3 수정 ----------

/// 기준 칸만 있는 선택에서 `3 d w`: 기준 칸을 지우고 대기에 들어 카운트대로 지운다.
#[gpui_kit::test]
fn anchor_only_selection_then_count_delete_word(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "anchor-3dw");
    set_cursor(cx, &view, 0, 50);
    run(
        cx,
        handle,
        Box::new(keyscore_app::actions::select::ExtendRight),
    );
    assert!(selection_of(cx, &view).is_empty(), "빈 칸만 덮어야 한다");
    press(cx, handle, "3 d");
    assert_eq!(pending(cx, &view), Some((Operator::Delete, Some(3))));
    assert_eq!(
        context_path(cx, &view),
        vec!["Normal", "PianoRoll", "Operator"]
    );
    assert!(cx.read(|cx| view.read(cx).piano_roll().anchor().is_none()));
    assert_eq!(status_mode(cx, handle), "NORMAL 3d");
}

/// 기준 칸만 있는 선택에서 `y`(대기 시간 뒤) `y`: 커서 노트를 복사한다.
#[gpui_kit::test]
fn anchor_only_selection_then_yank_line(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "anchor-y-y");
    set_cursor(cx, &view, 0, 50);
    run(
        cx,
        handle,
        Box::new(keyscore_app::actions::select::ExtendRight),
    );
    press(cx, handle, "y");
    wait(cx, handle);
    assert_eq!(pending(cx, &view), Some((Operator::Yank, None)));
    press(cx, handle, "y");
    wait(cx, handle);
    assert_eq!(pending(cx, &view), None);
    assert_eq!(internal(cx), None, "빈 칸 커서라 복사할 노트가 없다");
    assert_eq!(context_path(cx, &view), vec!["Normal", "PianoRoll"]);
}

/// `d 3` 뒤 메뉴 `view::Toggle` 두 번, 이어 `l`: 뷰 전환으로 끝난 대기의 카운트도 비운다.
#[gpui_kit::test]
fn delete_count_then_view_toggle_then_l_moves_once(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "d3-toggle-l");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "d 3");
    run(cx, handle, Box::new(keyscore_app::actions::view::Toggle));
    run(cx, handle, Box::new(keyscore_app::actions::view::Toggle));
    assert_eq!(pending(cx, &view), None);
    let start = cursor(cx, &view);
    press(cx, handle, "l");
    assert_eq!(cursor(cx, &view).0, start.0 + 960);
}

/// `3 d` 뒤 메뉴 `select::All`(선택이 생겨 대기가 끝남), `escape`, `l`: 카운트가 새지 않는다.
#[gpui_kit::test]
fn count_delete_then_menu_select_all_then_l_moves_once(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "3d-all-l");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "3 d");
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "escape");
    press(cx, handle, "l");
    assert_eq!(cursor(cx, &view), (960, 60));
}

/// 위 시험의 이어짐: `shift` 뒤 `i n`. 남은 `Operator` 그린 경로에서 `i n`이 `object::InnerNote`로 맞아 Step으로 가지
/// 않고 「텍스트 객체는 오퍼레이터나 비주얼 뒤에 쓴다」를 알린다. Normal이면 `i`(Step) 뒤 `n`(셋잇단)이다.
#[gpui_kit::test]
fn object_prefix_then_click_then_shift_then_i_n_enters_step(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-click-shift-in");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    click(cx, handle, &view, 0, 60);
    cx.simulate_keystrokes(handle, "shift");
    cx.run_until_parked();
    press(cx, handle, "i n");
    let m = mode(cx, &view);
    let sel = selection_of(cx, &view);
    let notes = clip_a(cx, &view);
    let msg = last_message(cx, &view);
    assert_eq!(
        (m, notes.len()),
        (Mode::Step, ALL.len()),
        "mode={m:?} sel={sel:?} notes={notes:?} msg={msg}"
    );
}

/// `d i` 중 메뉴 `view::Toggle`(배치 뷰), `shift` 단독, 이어 `enter`: 배치 뷰의 `enter`는 `clip::Open`이다. 붙든
/// 피아노롤 경로가 그린 채 남으면 `note::Audition`으로 맞아 피아노롤로 들어가지 않는다.
#[gpui_kit::test]
fn object_prefix_then_menu_view_toggle_then_shift_then_enter_opens_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-toggle-shift-enter");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    run(cx, handle, Box::new(keyscore_app::actions::view::Toggle));
    cx.simulate_keystrokes(handle, "shift");
    cx.run_until_parked();
    let stack = rendered_stack(cx, handle);
    cx.simulate_keystrokes(handle, "enter");
    cx.run_until_parked();
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll,
        "배치 뷰의 enter가 clip::Open으로 가지 않았다(그린 경로 {stack})"
    );
}
