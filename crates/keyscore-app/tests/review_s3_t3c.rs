//! S3-T3c 적대적 리뷰 1회차의 깨뜨리기 시도(`docs/reports/s3-t3c/review-1.md`). 픽스처는 `vim_operators.rs`에서
//! 복사했다. 실패하는 시험은 리뷰어가 기대한 동작을 단언한 지적의 재현이고,
//! 통과하는 시험은 「못 깨뜨림」 또는 지금 동작의 기록이다.

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
    let dir = std::env::temp_dir().join(format!("ks-rvim-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(cx, &format!("ks-test-{}-rvim-{name}", std::process::id()));
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

/// 원래 노트를 가장 이른 시작 기준으로.
fn relative(notes: &[Triple]) -> Vec<Triple> {
    let first = notes.iter().map(|n| n.0).min().unwrap_or(0);
    let mut v: Vec<Triple> = notes.iter().map(|&(s, l, p)| (s - first, l, p)).collect();
    v.sort_unstable();
    v
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

// ---------- 불변식: 오퍼레이터 대기와 선택은 동시에 없다 ----------

/// 대기 중 메뉴·팔레트로 `select::All`을 실행하면 대기와 선택이 함께 남는다. 상태줄은 `NORMAL d`인데 키
/// 컨텍스트 경로에는 `Operator`가 없다(경로가 `Selection`이라). 그래서 `i`가 Step 모드로 새고, `w`는 선택이
/// 아니라 모션 범위를 지운다.
#[gpui_kit::test]
fn menu_select_all_while_pending_breaks_invariant(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "menu-all");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d");
    assert!(pending(cx, &view).is_some());
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    let p = pending(cx, &view);
    let s = selection_of(cx, &view);
    let path = context_path(cx, &view);
    assert!(
        p.is_none() || s.is_empty(),
        "대기와 선택이 공존한다: pending={p:?} selection={s:?} path={path:?} status={}",
        status_mode(cx, handle)
    );
}

/// 대기 중 메뉴로 `edit::Undo`: undo가 되살린 노트를 선택해 대기와 선택이 공존한다.
#[gpui_kit::test]
fn menu_undo_while_pending_breaks_invariant(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "menu-undo");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d w");
    assert_eq!(clip_a(cx, &view), without(&[D4]));
    press(cx, handle, "d");
    assert!(pending(cx, &view).is_some());
    run(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    let p = pending(cx, &view);
    let s = selection_of(cx, &view);
    assert!(
        p.is_none() || s.is_empty(),
        "대기와 선택이 공존한다: pending={p:?} selection={s:?} path={:?}",
        context_path(cx, &view)
    );
}

/// 대기 중 메뉴로 모션을 부르면 오퍼레이터가 작용한다(계획 4절 「메뉴·팔레트로 부른 모션은 한 번」의 읽기).
#[gpui_kit::test]
fn menu_motion_while_pending_applies_operator(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "menu-motion");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "3 d");
    run(
        cx,
        handle,
        Box::new(keyscore_app::actions::cursor::NextNote),
    );
    // 대기 앞 카운트 3은 키 입력이라 남는다: 3 × 1
    assert_eq!(clip_a(cx, &view), without(&[D4, E4, G4]));
    assert_eq!(pending(cx, &view), None);
}

// ---------- 비주얼과 클립 경계 ----------

/// `v` 뒤 커서가 클립 밖(틈)으로 나가면 선택은 기준 칸의 클립에 남지만 `d`는 「현재 클립 밖」으로 거부되고,
/// 거부하면서 비주얼과 선택까지 지운다(S3-T3a는 거부 때 선택을 남긴다).
#[gpui_kit::test]
fn visual_past_clip_end_then_delete_loses_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vis-gap");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v ctrl-l ctrl-l");
    assert_eq!(cursor(cx, &view), (2 * BAR, 62));
    let before = selection_of(cx, &view);
    assert_eq!(before, vec![D4, E4, G4, D4B, A4]);
    press(cx, handle, "d");
    let after_doc = clip_a(cx, &view);
    let after_sel = selection_of(cx, &view);
    let msg = last_message(cx, &view);
    // 리뷰어의 기대: 지우거나(결정 3 A 「선택에 바로 작용」), 거부하면 선택을 남긴다(S3-T3a)
    assert!(
        after_doc != ALL.to_vec() || after_sel == before,
        "지우지도 않고 선택도 잃었다: doc={after_doc:?} sel={after_sel:?} msg={msg}"
    );
}

/// `v }`(다음 클립으로) 뒤 `d`도 같다.
#[gpui_kit::test]
fn visual_to_next_clip_then_delete_refused(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vis-next-clip");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v }");
    assert_eq!(cursor(cx, &view), (4 * BAR, 62));
    assert_eq!(selection_of(cx, &view), vec![D4, E4, G4, D4B, A4]);
    press(cx, handle, "d");
    let after_doc = clip_a(cx, &view);
    let after_sel = selection_of(cx, &view);
    assert!(
        after_doc != ALL.to_vec() || !after_sel.is_empty(),
        "지우지도 않고 선택도 잃었다: msg={}",
        last_message(cx, &view)
    );
}

// ---------- y 접두 ----------

/// 대기 시간 뒤의 `y` 대기에서 `y escape`: Esc가 시퀀스(`y`)를 비우며 `y`를 재생해 겹친 오퍼레이터(커서 노트
/// 복사)가 실행된다. Normal의 `y escape`(시험 11)는 취소인데 여기는 복사다.
#[gpui_kit::test]
fn yank_pending_then_y_escape_copies(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-wait-y-esc");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y");
    wait(cx, handle);
    assert_eq!(pending(cx, &view), Some((Operator::Yank, None)));
    press(cx, handle, "y escape");
    // 구현자 주(리뷰 1 지적 2 수정): A13과 계획 2절대로 시퀀스 중 Esc는 시퀀스(`y`)만 취소하고 `y` 대기는 남긴다
    // (`d i escape`와 같다, `vim_operators::other_key_cancels_operator_without_running`). 다음 Esc가 대기를 끝낸다.
    assert_eq!(pending(cx, &view), Some((Operator::Yank, None)));
    assert_eq!(internal(cx), None, "Esc가 복사를 실행했다");
    press(cx, handle, "escape");
    assert_eq!(pending(cx, &view), None);
    assert_eq!(internal(cx), None);
}

#[gpui_kit::test]
fn yank_prefix_then_pitch_motion_cancels(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-j");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y j");
    assert_eq!(cursor(cx, &view), (960, 62));
    assert_eq!(internal(cx), None);
    assert_eq!(pending(cx, &view), None);
    assert_eq!(mode(cx, &view), Mode::Normal);
}

#[gpui_kit::test]
fn yank_prefix_then_g_g(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-gg");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y g g");
    assert_eq!(internal(cx), Some(relative(&[C4, D4])));
    assert_eq!(cursor(cx, &view), (0, 62));
    assert_eq!(pending(cx, &view), None);
}

/// `y` 접두 뒤 `2 i n`: 숫자·`i` 재전송이 차례대로 가야 한다.
#[gpui_kit::test]
fn yank_prefix_count_then_object(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-2in");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y 2 i n");
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(internal(cx), Some(relative(&[D4])));
    assert_eq!(pending(cx, &view), None);
}

/// `d` 대기 중 빠른 `y y`(`edit::Copy`): 복사하지 않고 취소.
#[gpui_kit::test]
fn delete_pending_then_y_y_cancels(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "d-yy");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d y y");
    assert_eq!(internal(cx), None);
    assert_eq!(pending(cx, &view), None);
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    // 그 뒤 w는 이동만
    press(cx, handle, "w");
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

/// `y y p`는 그대로 `edit::Repeat`(복제, REQ 7.x `yyp`류)다. 오퍼레이터 대기로 새지 않고, 직전 `d w`의
/// `.` 기억도 바꾸지 않는다.
#[gpui_kit::test]
fn yyp_still_duplicates(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "yyp");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "d w");
    assert_eq!(clip_a(cx, &view), without(&[C4]));
    set_cursor(cx, &view, 1920, 64);
    press(cx, handle, "y y p");
    assert!(clip_a(cx, &view).contains(&(2880, 960, 64)));
    assert_eq!(pending(cx, &view), None);
    assert_eq!(mode(cx, &view), Mode::Normal);
}

// ---------- 카운트·비주얼·반복 ----------

#[gpui_kit::test]
fn count_before_visual_does_not_leak(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "3v");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "3 v w");
    assert_eq!(cursor(cx, &view), (960, 62));
    assert_eq!(selection_of(cx, &view), vec![C4, D4]);
}

#[gpui_kit::test]
fn visual_cmd_a_ends_visual(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-cmda");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v cmd-a");
    assert_eq!(visual(cx, &view), None);
    let mut all = ALL.to_vec();
    all.sort_unstable();
    assert_eq!(selection_of(cx, &view), all);
    assert_eq!(status_mode(cx, handle), "NORMAL");
}

/// `v w d` 뒤 `.`은 「선택(없으면 커서 노트)」 대상이라 커서가 빈 칸이면 아무것도 지우지 않는다(vim은 같은
/// 길이를 지운다). 계획 8절의 글자 그대로다.
#[gpui_kit::test]
fn dot_after_visual_delete_targets_cursor_note(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-dot");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v w d");
    assert_eq!(clip_a(cx, &view), without(&[D4, E4]));
    assert_eq!(cursor(cx, &view), (1920, 64));
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), without(&[D4, E4]));
    assert!(last_message(cx, &view).contains("없다"));
}

/// 비주얼 → 배치 뷰 → 피아노롤: 비주얼도 선택도 끝난다. 그 뒤 `d`는 대기에 든다.
#[gpui_kit::test]
fn visual_view_round_trip_clears_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-view");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v w");
    press(cx, handle, "g a");
    press(cx, handle, "g a");
    assert_eq!(visual(cx, &view), None);
    assert_eq!(status_mode(cx, handle), "NORMAL");
    assert_eq!(selection_of(cx, &view), Vec::<Triple>::new());
    press(cx, handle, "d");
    assert!(pending(cx, &view).is_some());
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

/// 마지막 클립의 마지막 노트에서 `d w`: 다음 노트가 없어 「대상이 없다」(vim `dw`는 지운다).
#[gpui_kit::test]
fn delete_word_on_last_note_does_nothing(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dw-last");
    set_cursor(cx, &view, 4 * BAR, 69);
    press(cx, handle, "d w");
    let b_notes = cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track.clips().nth(1).unwrap().1.notes().len()
    });
    assert_eq!(b_notes, 1);
    assert!(last_message(cx, &view).contains("대상이 없다"));
}

/// 대기 중 `cmd-z`: 계획 2절대로 실행하지 않고 취소한다(표준 키도 삼킨다).
#[gpui_kit::test]
fn pending_swallows_standard_keys(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "d-cmdz");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d w");
    press(cx, handle, "d cmd-z");
    assert_eq!(clip_a(cx, &view), without(&[D4]));
    assert_eq!(pending(cx, &view), None);
}

/// 비주얼 중 `u`: undo가 되살린 노트를 선택해도 비주얼 불변식(기준 칸)이 유지되는가.
#[gpui_kit::test]
fn visual_delete_undo_state(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-x-u");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v w x");
    assert_eq!(clip_a(cx, &view), without(&[D4, E4]));
    assert_eq!(visual(cx, &view), None);
    press(cx, handle, "u");
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    assert_eq!(visual(cx, &view), None);
    assert_eq!(status_mode(cx, handle), "NORMAL");
}

/// 빈 칸에서 `c c`(지울 노트 없음)도 `.` 기억을 바꾼다. 계획 8절 「실패한 오퍼레이터(대상 없음)는 바꾸지
/// 않는다」의 「대상 없음」에 드는지 계획이 정하지 않았다. 지금은 바꾼다(기록).
#[gpui_kit::test]
fn change_on_empty_cell_replaces_last_edit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cc-empty");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "d w");
    assert_eq!(clip_a(cx, &view), without(&[C4]));
    set_cursor(cx, &view, 480, 50);
    press(cx, handle, "c c");
    assert_eq!(mode(cx, &view), Mode::Step);
    press(cx, handle, "escape");
    assert_eq!(mode(cx, &view), Mode::Normal);
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, ".");
    // `.`은 `d w`가 아니라 `c c`(커서 노트 지우고 Step)다
    assert_eq!(clip_a(cx, &view), without(&[C4, D4B]));
    assert_eq!(mode(cx, &view), Mode::Step);
}
