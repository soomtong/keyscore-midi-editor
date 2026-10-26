//! S3-T3c 적대적 리뷰 2회차의 깨뜨리기 시도(`docs/reports/s3-t3c/review-2.md`). 픽스처는 1회차
//! `review_s3_t3c.rs`에서 복사했다. 실패하는 시험은 리뷰어가 기대한 동작을 단언한 지적의 재현이고, 통과하는 시험은
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
    let dir = std::env::temp_dir().join(format!("ks-rvim2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-rvim2-{name}", std::process::id()),
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

// ---------- 지적 1 수정(9ba9ab1): 키가 아닌 경로로 대기가 끝날 때 남은 대기 입력 ----------

/// `d i`(객체 접두 대기) 중 메뉴·팔레트 `select::All`: 수정이 오퍼레이터 대기를 끝내 컨텍스트가 `Selection`으로
/// 바뀌는데 GPUI의 대기 입력 `i`는 남는다. 다음 `n`에서 `i n`이 새 컨텍스트에 없으므로 `i`가 Normal의 `i`
/// (Step 모드)로 재생된다. 계획 2절 「대기 중 `i` `a` 단독은 아무 일도 하지 않는다」.
#[gpui_kit::test]
fn object_prefix_then_menu_select_all_then_n_stays_normal(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-all-n");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    assert!(pending(cx, &view).is_some());
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "n");
    assert_eq!(
        mode(cx, &view),
        Mode::Normal,
        "남은 대기 입력 i가 Step 모드로 재생됐다: path={:?}",
        context_path(cx, &view)
    );
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

/// 같은 상태에서 다음 키가 `Esc`: 키맵의 A13 가로채기가 대기 입력을 비우며 `i`를 재생한다.
#[gpui_kit::test]
fn object_prefix_then_menu_select_all_then_escape_stays_normal(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-all-esc");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    press(cx, handle, "escape");
    assert_eq!(
        mode(cx, &view),
        Mode::Normal,
        "Esc의 비우기가 i를 Step 모드로 재생했다"
    );
}

/// 같은 갈래(1회차 범위, 클릭 경로): `d i` 중 피아노롤 클릭으로 대기가 끝난 뒤 `n`.
#[gpui_kit::test]
fn object_prefix_then_click_then_n_stays_normal(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "di-click-n");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i");
    view.update(cx, |this, cx| {
        this.click_piano_roll_cell(
            Cursor {
                tick: Tick::new(0),
                pitch: Pitch::new(60).unwrap(),
            },
            cx,
        );
    });
    render(cx, handle);
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "n");
    assert_eq!(
        mode(cx, &view),
        Mode::Normal,
        "클릭 뒤 남은 대기 입력 i가 Step 모드로 재생됐다"
    );
}

/// 비주얼의 객체 접두 `v i` 중 팔레트 `select::All`(비주얼이 끝난다) 뒤 `n`.
#[gpui_kit::test]
fn visual_object_prefix_then_menu_select_all_then_n_stays_normal(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vi-all-n");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v i");
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    assert_eq!(visual(cx, &view), None);
    press(cx, handle, "n");
    assert_eq!(
        mode(cx, &view),
        Mode::Normal,
        "비주얼이 끝난 뒤 남은 i가 Step 모드로 재생됐다"
    );
}

/// 기준 칸만 있는 선택(빈 칸에서 팔레트 `select::ExtendRight`) 뒤 `d`: 선택 컨텍스트인데 대기에 든다. 키 컨텍스트
/// 경로에 `Operator`가 붙지 않아(`context_path`는 `Normal > PianoRoll`에서만 붙인다) 대기와 선택이 공존한다.
#[gpui_kit::test]
fn anchor_only_selection_then_d_pending_without_operator_context(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "anchor-d");
    set_cursor(cx, &view, 0, 50);
    run(
        cx,
        handle,
        Box::new(keyscore_app::actions::select::ExtendRight),
    );
    assert!(selection_of(cx, &view).is_empty());
    assert_eq!(
        context_path(cx, &view),
        vec!["Normal", "PianoRoll", "Selection"]
    );
    press(cx, handle, "d");
    let p = pending(cx, &view);
    let path = context_path(cx, &view);
    assert!(
        p.is_none() || path.contains(&"Operator"),
        "대기와 선택 컨텍스트가 공존한다: pending={p:?} path={path:?} status={}",
        status_mode(cx, handle)
    );
}

/// 위 상태의 결과: `d i b`(커서 마디의 노트)가 객체 삭제가 되지 않고 「오퍼레이터를 취소했다」가 된다. `i`가
/// `Operator`의 `null`이 아니라 Normal의 Step 진입과 맞아 허용 밖 키로 취소되기 때문이다. 기준 칸만 있는 선택은
/// 빈 칸 직사각형이라 커서 칸에 노트가 없으므로 `i n`이 아니라 `i b`로 본다.
#[gpui_kit::test]
fn anchor_only_selection_then_d_i_b(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "anchor-dib");
    set_cursor(cx, &view, 0, 50);
    run(
        cx,
        handle,
        Box::new(keyscore_app::actions::select::ExtendRight),
    );
    press(cx, handle, "d i b");
    eprintln!(
        "anchor d i b: mode={:?} msg={} clip={:?}",
        mode(cx, &view),
        last_message(cx, &view),
        clip_a(cx, &view)
    );
    assert_eq!(mode(cx, &view), Mode::Normal, "d i b가 Step으로 샜다");
    assert_eq!(pending(cx, &view), None, "대기가 남았다");
    assert_eq!(
        clip_a(cx, &view),
        vec![A4],
        "d i b가 커서 마디의 노트를 지우지 않았다: {}",
        last_message(cx, &view)
    );
}

/// 대기 중 팔레트 `edit::Paste`(선택을 만들지 않는 편집): 붙여넣기는 실행되고 대기가 남는지 기록.
#[gpui_kit::test]
fn pending_then_menu_paste_records(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "pend-paste");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "y y");
    set_cursor(cx, &view, 4 * BAR + 960, 50);
    press(cx, handle, "d");
    assert!(pending(cx, &view).is_some());
    run(cx, handle, Box::new(keyscore_app::actions::edit::Paste));
    let p = pending(cx, &view);
    let s = selection_of(cx, &view);
    let path = context_path(cx, &view);
    eprintln!("paste 뒤: pending={p:?} selection={s:?} path={path:?}");
    assert!(p.is_none() || s.is_empty(), "대기와 선택 공존: {p:?} {s:?}");
}

/// 메뉴 모션이 대기에 작용하는 1회차 동작(시도 3)이 수정 뒤에도 그대로인가: `d` 뒤 팔레트 `cursor::NextNote`.
#[gpui_kit::test]
fn menu_motion_still_applies_after_fix(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "menu-motion");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d");
    run(
        cx,
        handle,
        Box::new(keyscore_app::actions::cursor::NextNote),
    );
    assert_eq!(clip_a(cx, &view), without(&[D4]));
    assert_eq!(pending(cx, &view), None);
}

/// 비주얼 `c`(선택이 있어 바로)·`d w`(대기 → 모션)가 수정 뒤에도 그대로인가. `cursor_moved`가 부르는
/// `sync_selection_mode`가 오퍼레이터 처리 도중 대기를 먼저 지우지 않는지 본다.
#[gpui_kit::test]
fn change_word_still_works_after_fix(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cw");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "3 d w");
    assert_eq!(clip_a(cx, &view), without(&[D4, E4, G4]));
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "u");
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v w c");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(clip_a(cx, &view), without(&[D4, E4]));
}

/// 오퍼레이터 대기 없이 빠른 `y`(대기 입력 `y`) 중 팔레트 `select::All`, 이어 `w`: 기록.
#[gpui_kit::test]
fn yank_prefix_then_menu_select_all_then_w_records(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-all-w");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y");
    run(cx, handle, Box::new(keyscore_app::actions::select::All));
    press(cx, handle, "w");
    eprintln!(
        "y <All> w: clipboard={:?} pending={:?} cursor={:?} sel={}",
        internal(cx),
        pending(cx, &view),
        cursor(cx, &view),
        selection_of(cx, &view).len()
    );
    assert_eq!(pending(cx, &view), None);
}

// ---------- 지적 2 수정(df2f893) ----------

/// `d` 대기 중 `y escape`: 재생된 `y`는 무시되고 `d` 대기가 남는다(A13). 이어 `w`가 한 노트를 지운다.
#[gpui_kit::test]
fn delete_pending_y_escape_keeps_delete(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "d-y-esc");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d y escape");
    assert_eq!(pending(cx, &view), Some((Operator::Delete, None)));
    assert_eq!(internal(cx), None);
    press(cx, handle, "w");
    assert_eq!(clip_a(cx, &view), without(&[D4]));
    assert_eq!(internal(cx), None);
}

/// `y` 대기(대기 시간 뒤) 중 `y escape` 다음 바로 `y`: 무시 표시가 남아 정상 `y`(겹친 오퍼레이터)를 삼키지 않는가.
#[gpui_kit::test]
fn yank_pending_y_escape_then_y_still_works(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-y-esc-y");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y");
    wait(cx, handle);
    press(cx, handle, "y escape");
    assert_eq!(pending(cx, &view), Some((Operator::Yank, None)));
    press(cx, handle, "y");
    wait(cx, handle);
    assert_eq!(internal(cx), Some(relative(&[D4])));
    assert_eq!(pending(cx, &view), None);
}

/// `y` 대기 중 `y y escape`(대기 입력 둘, `y y p`의 접두): 키맵의 비우기가 `y y`(`edit::Copy`)를 실행한다. 기록.
#[gpui_kit::test]
fn yank_pending_y_y_escape_records(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-yy-esc");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y");
    wait(cx, handle);
    press(cx, handle, "y y escape");
    eprintln!(
        "y(wait) y y escape: clipboard={:?} pending={:?}",
        internal(cx),
        pending(cx, &view)
    );
}

/// Normal의 `y escape` 뒤 무시 표시가 다음 키 입력까지 새지 않는가: 바로 `y y`(빠름)가 복사한다.
#[gpui_kit::test]
fn normal_y_escape_then_y_y_copies(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-esc-yy");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y escape");
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "y y");
    wait(cx, handle);
    assert_eq!(internal(cx), Some(relative(&[D4])));
}

// ---------- 지적 3 수정(aeb202d) ----------

/// 거부 뒤 선택·비주얼이 남고, 커서를 선택의 클립으로 되돌려 `d`하면 지운다.
#[gpui_kit::test]
fn visual_refused_then_back_into_clip_deletes(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-refuse-back");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v }");
    let before = selection_of(cx, &view);
    assert!(!before.is_empty());
    press(cx, handle, "d");
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
    assert_eq!(selection_of(cx, &view), before);
    assert_eq!(pending(cx, &view), None);
    assert_eq!(status_mode(cx, handle), "VISUAL");
    press(cx, handle, "{");
    let back = selection_of(cx, &view);
    press(cx, handle, "d");
    assert_eq!(visual(cx, &view), None);
    let left = clip_a(cx, &view);
    assert!(
        back.iter().all(|n| !left.contains(n)),
        "되돌린 선택이 지워지지 않았다: back={back:?} left={left:?}"
    );
}

/// 거부된 `c`: Step으로 가지 않고 `.` 기억도 바뀌지 않는다.
#[gpui_kit::test]
fn visual_refused_change_keeps_state(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-refuse-c");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "d w");
    assert_eq!(clip_a(cx, &view), without(&[C4]));
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v }");
    press(cx, handle, "c");
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
    press(cx, handle, "escape");
    assert_eq!(visual(cx, &view), None);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), without(&[C4, D4]), "`.`이 d w가 아니다");
}

/// 거부된 비주얼에서 `y`는 클립 밖이어도 복사하고 비주얼을 끝낸다(복사는 문서를 바꾸지 않는다).
#[gpui_kit::test]
fn visual_outside_then_yank_copies(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-out-y");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v }");
    let sel = selection_of(cx, &view);
    press(cx, handle, "y");
    wait(cx, handle);
    assert_eq!(internal(cx), Some(relative(&sel)));
    assert_eq!(visual(cx, &view), None);
}

/// 틈에서 시작한 비주얼(기준 칸·커서 모두 클립 밖) `d`: 「클립 밖」 거부 뒤 비주얼이 남는다. Esc로 끝난다.
#[gpui_kit::test]
fn visual_in_gap_then_delete_keeps_visual(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "v-gap");
    set_cursor(cx, &view, 2 * BAR + 960, 60);
    press(cx, handle, "v l");
    press(cx, handle, "d");
    eprintln!(
        "gap v d: visual={:?} msg={} status={}",
        visual(cx, &view),
        last_message(cx, &view),
        status_mode(cx, handle)
    );
    press(cx, handle, "escape");
    assert_eq!(visual(cx, &view), None);
    assert_eq!(context_path(cx, &view), vec!["Normal", "PianoRoll"]);
}

/// 같은 갈래(키가 아닌 경로로 끝난 대기의 카운트): `d 3` 뒤 클릭, 이어 `l`. 클릭이 대기를 끝내도 오퍼레이터에
/// 붙은 키맵 카운트 3이 남아 다음 `l`이 세 칸 간다(`cancel_operator`·`operator_cancel`은 카운트를 비운다).
#[gpui_kit::test]
fn delete_count_then_click_then_l_moves_once(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "d3-click-l");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d 3");
    view.update(cx, |this, cx| {
        this.click_piano_roll_cell(
            Cursor {
                tick: Tick::new(0),
                pitch: Pitch::new(60).unwrap(),
            },
            cx,
        );
    });
    render(cx, handle);
    press(cx, handle, "l");
    assert_eq!(
        cursor(cx, &view),
        (960, 60),
        "대기의 카운트가 클릭 뒤 이동으로 샜다"
    );
}
