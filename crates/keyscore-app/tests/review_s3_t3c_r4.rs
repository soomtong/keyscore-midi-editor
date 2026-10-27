//! S3-T3c 적대적 리뷰 4회차(좁은 확인 리뷰, 결정 2026-10-27-s3-t3c-review-limit A)의 깨뜨리기 시도
//! (`docs/reports/s3-t3c/review-4.md`). 범위는 리뷰 3 지적 1의 수정(4f2d301)뿐이다. 픽스처는 3회차
//! `review_s3_t3c_r3.rs`에서 복사했다.

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
    let dir = std::env::temp_dir().join(format!("ks-rvim4-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-rvim4-{name}", std::process::id()),
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

fn has_pending(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| window.has_pending_keystrokes())
        .unwrap()
}

/// 그리지 않는 클릭: 다음 키의 `dispatch_key_event`가 더러운 창을 먼저 그리므로, 그 그리기의 비우기 지연과 키
/// 가로채기의 비우기(`flush_held_then`)가 한 키 안에서 겹친다.
fn click_no_render(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64, pitch: u8) {
    view.update(cx, |this, cx| {
        this.click_piano_roll_cell(
            Cursor {
                tick: Tick::new(tick),
                pitch: Pitch::new(pitch).unwrap(),
            },
            cx,
        );
    });
}

fn assert_plain_stack(cx: &mut TestAppContext, handle: AnyWindowHandle, what: &str) {
    let stack = rendered_stack(cx, handle);
    assert!(
        !stack.contains("Operator") && !stack.contains("Visual"),
        "{what}: {stack}"
    );
}

// ---------- 정상 시퀀스: 붙든 경로가 지금 경로와 같으면 그리기는 비우지 않는다 ----------

/// `d i` 대기 중 여러 번 그려도 대기 입력은 남고, 이어 `n`이 `d i n`으로 맞는다.
#[gpui_kit::test]
fn delete_inner_note_survives_renders_between_keys(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "din-renders");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    render(cx, handle);
    render(cx, handle);
    cx.run_until_parked();
    assert!(has_pending(cx, handle), "그리기가 정상 대기 입력을 비웠다");
    assert!(rendered_stack(cx, handle).contains("Operator"));
    press(cx, handle, "n");
    assert_eq!(clip_a(cx, &view), without(&[D4]));
    assert_eq!(pending(cx, &view), None);
    assert_plain_stack(cx, handle, "d i n 뒤");
}

/// `v i` 중 그리기 뒤 `b`: 비주얼 객체가 마디를 고른다.
#[gpui_kit::test]
fn visual_inner_bar_survives_renders_between_keys(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vib-renders");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v i");
    render(cx, handle);
    cx.run_until_parked();
    assert!(has_pending(cx, handle));
    press(cx, handle, "b");
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
    assert_eq!(selection_of(cx, &view), vec![C4, D4, E4, G4, D4B]);
}

/// `g g`, `y y p`, `2 d i n`(카운트 뒤 시퀀스)이 그리기를 사이에 두고도 맞는다.
#[gpui_kit::test]
fn normal_sequences_with_renders_between_keys(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "seq-renders");
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, "g");
    render(cx, handle);
    press(cx, handle, "g");
    assert_eq!(cursor(cx, &view).0, 0, "g g");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y");
    render(cx, handle);
    press(cx, handle, "y");
    render(cx, handle);
    press(cx, handle, "p");
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(pending(cx, &view), None);
    let before = clip_a(cx, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "2 d i");
    render(cx, handle);
    press(cx, handle, "n");
    assert_eq!(pending(cx, &view), None);
    assert!(
        clip_a(cx, &view).len() < before.len(),
        "2 d i n이 지우지 않았다"
    );
    assert_plain_stack(cx, handle, "2 d i n 뒤");
}

/// `y` 접두 재전송(`y i n`): `y` 뒤 `i`는 삼켜져 `y`를 실행한 뒤 다시 보낸다. 붙든 경로가 낡게 남은 상태(`v i` 뒤
/// 메뉴 `select::All`로 비주얼이 끝남)에서 시작해도 커서 노트를 복사한다. (비주얼 중 클릭은 비주얼을 끝내지 않고
/// 커서만 옮긴다.)
#[gpui_kit::test]
fn yank_prefix_resend_after_stale_visual_prefix(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vi-all-yin");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v i");
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    assert_eq!(visual(cx, &view), None);
    assert!(
        !has_pending(cx, handle),
        "그리기가 낡은 접두를 비우지 않았다"
    );
    assert_plain_stack(cx, handle, "select::All 뒤");
    press(cx, handle, "escape");
    assert!(selection_of(cx, &view).is_empty());
    press(cx, handle, "y i n");
    assert_eq!(pending(cx, &view), None);
    assert_eq!(internal(cx), Some(vec![(0, 960, 62)]));
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

// ---------- 그리기의 비우기가 재생하는 Action ----------

/// `v y`(비주얼에서 `y`는 `y y`의 접두라 대기, 단독 바인딩 `operator::Yank`) 중 메뉴 `select::All`로 비주얼이
/// 끝나면 그리기가 붙든 `Visual` 경로에서 `y`를 재생한다. 재생된 Yank가 새면 전체 선택을 복사한다.
#[gpui_kit::test]
fn visual_yank_prefix_then_menu_select_all_does_not_copy(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vy-all");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v y");
    assert!(has_pending(cx, handle), "v y가 대기하지 않았다(시험 전제)");
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(internal(cx), None, "재생된 y가 복사했다");
    assert_eq!(pending(cx, &view), None);
    assert!(!has_pending(cx, handle));
    assert_plain_stack(cx, handle, "select::All 뒤");
    press(cx, handle, "escape");
    press(cx, handle, "d");
    assert_eq!(
        pending(cx, &view),
        Some((Operator::Delete, None)),
        "무시 표시가 남았다"
    );
}

/// 같은 갈래, `v y` 중 메뉴 `view::Toggle`(배치 뷰). 재생된 Yank가 「오퍼레이터는 피아노롤의 Normal에서」를
/// 알리거나 대기에 들면 안 된다. 이어 배치 뷰의 `enter`는 `clip::Open`.
#[gpui_kit::test]
fn visual_yank_prefix_then_menu_view_toggle_then_enter(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vy-toggle-enter");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v y");
    let before = last_message(cx, &view);
    run(cx, handle, Box::new(keyscore_app::actions::view::Toggle));
    render(cx, handle);
    assert_eq!(last_message(cx, &view), before, "재생된 y가 알렸다");
    assert_eq!(internal(cx), None);
    assert_plain_stack(cx, handle, "배치 뷰");
    assert!(!rendered_stack(cx, handle).contains("PianoRoll"));
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
}

/// `d g`(단독 바인딩 없는 접두) 중 메뉴 `view::Toggle`: 그리기의 비우기가 아무것도 실행하지 않고, 커서·문서 그대로.
#[gpui_kit::test]
fn delete_g_prefix_then_menu_view_toggle_flushes_quietly(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dg-toggle");
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, "d g");
    run(cx, handle, Box::new(keyscore_app::actions::view::Toggle));
    render(cx, handle);
    assert!(!has_pending(cx, handle));
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    run(cx, handle, Box::new(keyscore_app::actions::view::Toggle));
    assert_eq!(cursor(cx, &view), (2880, 62));
    press(cx, handle, "l");
    assert_eq!(
        cursor(cx, &view).0,
        3840,
        "카운트·대기가 새지 않았다면 한 칸"
    );
}

/// `d 3 i`(카운트 뒤 접두) 중 클릭, 이어 `l`: 비우기·카운트 비우기 뒤 한 칸.
#[gpui_kit::test]
fn delete_count_object_prefix_then_click_then_l_moves_once(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "d3i-click-l");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d 3 i");
    click(cx, handle, &view, 0, 60);
    assert!(!has_pending(cx, handle));
    press(cx, handle, "l");
    assert_eq!(cursor(cx, &view), (960, 60));
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

// ---------- 그리기와 키가 한 디스패치 안에서 겹침 ----------

/// 클릭 뒤 그리지 않고 곧바로 `a`: 키의 디스패치가 먼저 그리면서 그리기의 비우기 지연을 걸고, 가로채기도
/// `flush_held_then`으로 비우기·재전송을 건다. 두 비우기가 겹쳐도 `a`는 한 번, Normal의 `a`(Step)로 간다.
#[gpui_kit::test]
fn object_prefix_then_click_without_render_then_a_enters_step(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-click-nr-a");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    click_no_render(cx, &view, 0, 60);
    press(cx, handle, "a");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert!(!has_pending(cx, handle));
}

/// 같은 겹침에서 다시 보낸 키가 오퍼레이터(`d w`): 무시 표시가 다시 보낸 `d`를 삼키지 않는다.
#[gpui_kit::test]
fn object_prefix_then_click_without_render_then_delete_word(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-click-nr-dw");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    click_no_render(cx, &view, 0, 60);
    press(cx, handle, "d w");
    assert_eq!(clip_a(cx, &view), without(&[C4]));
    assert_eq!(pending(cx, &view), None);
}

/// 비주얼 `y` 접두가 낡게 남은 채(`v y` 중 메뉴 `view::Toggle` 두 번, 그리지 않음) `d`: 키의 디스패치가 먼저 그리며
/// 거는 비우기가 재생하는 `y`는 무시되고, 다시 보낸 `d`는 대기에 든다(무시 표시가 다시 보낸 오퍼레이터까지 삼키지
/// 않는다).
#[gpui_kit::test]
fn visual_yank_prefix_then_toggles_without_render_then_delete(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vy-toggle-nr-d");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v y");
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| {
            keyscore_app::dispatch::run_action(
                Box::new(keyscore_app::actions::view::Toggle),
                window,
                cx,
            );
        })
        .unwrap();
    }
    assert_eq!(
        visual(cx, &view),
        None,
        "뷰 전환이 비주얼을 끝낸다(시험 전제)"
    );
    press(cx, handle, "d");
    assert_eq!(internal(cx), None, "재생된 y가 복사했다");
    assert_eq!(pending(cx, &view), Some((Operator::Delete, None)));
    assert!(!has_pending(cx, handle));
    press(cx, handle, "w");
    assert_eq!(
        clip_a(cx, &view).len(),
        ALL.len() - 1,
        "d w가 하나를 지운다"
    );
}

// ---------- (가) 대기 입력 관찰 ----------

/// `v y` 대기 시간 초과: GPUI가 대기 입력을 비우며(관찰 알림) `y`를 재생해 선택을 복사하고 비주얼이 끝난다.
/// 붙든 경로는 놓여 다음 `i`는 Step.
#[gpui_kit::test]
fn visual_yank_prefix_timeout_releases_held_path(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vy-timeout");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v y");
    wait(cx, handle);
    assert!(!has_pending(cx, handle));
    assert_eq!(internal(cx), Some(vec![(0, 960, 62)]));
    assert_eq!(visual(cx, &view), None);
    assert_plain_stack(cx, handle, "시간 초과 뒤");
    press(cx, handle, "i");
    assert_eq!(mode(cx, &view), Mode::Step);
}

/// 대기 중(`d i`, 붙든 경로 = 지금 경로) `shift` 단독: 접두가 비워지고 대기는 남는다. 그린 경로는 `Operator`이고
/// 대기 입력은 없다. 이어 `w`가 `d w`로 지운다(관찰이 대기 자체를 건드리지 않는다).
#[gpui_kit::test]
fn object_prefix_shift_keeps_operator_pending(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-shift-w");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    cx.simulate_keystrokes(handle, "shift");
    cx.run_until_parked();
    render(cx, handle);
    assert!(!has_pending(cx, handle));
    assert_eq!(pending(cx, &view), Some((Operator::Delete, None)));
    assert!(rendered_stack(cx, handle).contains("Operator"));
    press(cx, handle, "w");
    assert_eq!(clip_a(cx, &view), without(&[D4]));
}

// ---------- 다른 창 ----------

/// 두 창. 창 A의 `d i` 접두가 메뉴로 낡게 남아 그리기에서 비워질 때, 창 B의 정상 대기 입력(`d i`)은 남고 B의
/// `n`이 B에서만 지운다.
#[gpui_kit::test]
fn stale_prefix_release_does_not_touch_other_window(cx: &mut TestAppContext) {
    let (a, va) = setup(cx, "two-windows");
    let (b, vb) = open(cx);
    set_cursor(cx, &vb, 960, 62);
    press(cx, b, "d i");
    assert!(has_pending(cx, b));
    cx.update_window(a, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    set_cursor(cx, &va, 960, 62);
    press(cx, a, "d i");
    run(cx, a, Box::new(keyscore_app::actions::view::Toggle));
    render(cx, a);
    assert!(!has_pending(cx, a), "A의 낡은 접두가 남았다");
    assert!(has_pending(cx, b), "A의 비우기가 B의 대기 입력을 비웠다");
    assert_eq!(clip_a(cx, &va), ALL.to_vec());
    cx.update_window(b, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    press(cx, b, "n");
    assert_eq!(clip_a(cx, &vb), without(&[D4]));
    assert_eq!(clip_a(cx, &va), ALL.to_vec());
}
