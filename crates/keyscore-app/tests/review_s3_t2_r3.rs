//! S3-T2 적대적 리뷰 3회차의 깨뜨리기 시도(`docs/reports/s3-t2/review-3.md` 3절). 2회차 수정(지적 1–6, 제안
//! 8·9)을 다른 경로로 다시 깨뜨린다. 도우미는 `review_s3_t2_r2.rs`에서 복사했다. 통과한 시도도 「못 깨뜨림」으로
//! 남기기 위해 모두 둔다.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;
/// 키맵 편집기 캡처 확정 대기(1 s)보다 조금 길게.
const SETTLE: Duration = Duration::from_millis(1100);

// ---------- 도우미(review_s3_t2_r2.rs 복사) ----------

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rev32r3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn no_preview(cx: &mut TestAppContext) {
    cx.update(|cx| {
        keyscore_app::app_state::AppSettings::update(cx, |s| s.preview.on_move = false);
    });
}

fn open(cx: &mut TestAppContext, project: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    activate(cx, handle);
    (handle, view)
}

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    cx.run_until_parked();
    render(cx, handle);
}

#[allow(dead_code)]
fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn settle(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.executor().advance_clock(SETTLE);
    cx.run_until_parked();
    render(cx, handle);
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

fn pitch(value: u8) -> Pitch {
    Pitch::new(value).unwrap()
}

fn at(tick: u64, p: u8) -> Cursor {
    Cursor {
        tick: Tick::new(tick),
        pitch: pitch(p),
    }
}

fn clip(start: u64, length: u64, notes: &[Note]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    clip
}

/// 클립 A(0–4마디) 노트 8개와 클립 B(4마디부터 1마디)의 G4. `pianoroll_select.rs`와 같다.
const A_NOTES: [(u64, u32, u8); 8] = [
    (0, 960, 60),
    (960, 960, 62),
    (1920, 480, 64),
    (3840, 960, 60),
    (3840, 960, 64),
    (3840, 960, 67),
    (7680, 1920, 69),
    (14400, 960, 65),
];
const D4: (u64, u32, u8) = (960, 960, 62);

fn clip_a() -> Clip {
    let notes: Vec<Note> = A_NOTES.iter().map(|&(s, l, p)| note(s, l, p)).collect();
    clip(0, 4 * BAR, &notes)
}

fn clip_b() -> Clip {
    clip(4 * BAR, BAR, &[note(0, 960, 67)])
}

fn fixture_track() -> Track {
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip_a()).unwrap();
    track.insert_clip(clip_b()).unwrap();
    track
}

fn fixture() -> Project {
    let mut project = Project::new();
    project.push_track(fixture_track()).unwrap();
    project
}

/// 한 클립(4마디)에 `notes`만 든 프로젝트.
fn single_clip(notes: &[(u64, u32, u8)]) -> Project {
    let notes: Vec<Note> = notes.iter().map(|&(s, l, p)| note(s, l, p)).collect();
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip(0, 4 * BAR, &notes)).unwrap();
    project.push_track(track).unwrap();
    project
}

/// 트랙 하나에 클립 여럿.
fn clips(list: Vec<Clip>) -> Project {
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    for c in list {
        track.insert_clip(c).unwrap();
    }
    project.push_track(track).unwrap();
    project
}

fn triple(n: &Note) -> (u64, u32, u8) {
    (n.start().get(), n.length(), n.pitch().get())
}

fn cursor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (u64, u8) {
    cx.read(|cx| {
        let c = view.read(cx).piano_roll().cursor();
        (c.tick.get(), c.pitch.get())
    })
}

fn anchor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<(u64, u8)> {
    cx.read(|cx| {
        view.read(cx)
            .piano_roll()
            .anchor()
            .map(|c| (c.tick.get(), c.pitch.get()))
    })
}

fn set_cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64, p: u8) {
    cx.update(|cx| view.read(cx).piano_roll().set_cursor(at(tick, p)));
}

fn selection_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u32, u8)> {
    cx.read(|cx| {
        let s = view.read(cx).selection();
        let mut v: Vec<_> = s.notes().map(triple).collect();
        v.sort_unstable();
        v
    })
}

fn selection_clip(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<ClipId> {
    cx.read(|cx| view.read(cx).selection().clip())
}

fn has_selection(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_state().has_selection())
}

fn ids(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (TrackId, ClipId) {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        (track.id(), track.clips().next().unwrap().0)
    })
}

/// 트랙 0의 `index`번째 클립.
fn clip_id(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, index: usize) -> ClipId {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track.clips().nth(index).unwrap().0
    })
}

fn select(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, notes: &[(u64, u32, u8)]) {
    let (track, clip) = ids(cx, view);
    let set: BTreeSet<Note> = notes.iter().map(|&(s, l, p)| note(s, l, p)).collect();
    view.update(cx, |this, cx| {
        let mut sel = Selection::default();
        sel.set(track, clip, set);
        this.set_selection(sel, cx);
    });
}

/// 트랙 0의 첫 클립 노트.
fn clip_a_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u32, u8)> {
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

fn enter_piano_roll(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) {
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
}

fn last_message(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<String> {
    cx.read(|cx| view.read(cx).messages().last().map(ToString::to_string))
}

// ---------- 키맵 편집기 도우미(settings_keymap.rs 복사) ----------

fn run_settings(cx: &mut TestAppContext) {
    cx.update(|cx| settings_window::open(cx, None));
    cx.run_until_parked();
}

fn settings_view(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<SettingsWindow>) {
    let found = cx.update(|cx| settings_window::current(cx));
    let (handle, view) = found.expect("설정 창이 없다");
    render(cx, handle);
    (handle, view)
}

/// 목록을 검색으로 좁혀 그 Action을 고른다(키보드만).
fn select_action(cx: &mut TestAppContext, handle: AnyWindowHandle, query: &str) {
    press(cx, handle, "/");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, query);
    press(cx, handle, "enter");
}

fn user_file(cx: &mut TestAppContext) -> String {
    let path = cx.update(|cx| keymap::user_path(cx));
    std::fs::read_to_string(path).unwrap_or_default()
}

fn f20_errors(cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| {
        keymap::errors(cx)
            .iter()
            .map(keymap::describe)
            .filter(|m| m.to_lowercase().contains("f20"))
            .collect()
    })
}

// ---------- 시도 1. 지적 2 우회: 기준 칸이 틈이면 고르는 클립이 A → B → A로 뒤집힌다 ----------

/// 클립 A(1–2마디, C4 960), B(3–4마디, C4 480). 틈(0)에서 `shift-l`로 A 안 → A 뒤 틈(첫 클립 규칙으로 A) → B 안
/// (커서 클립 규칙으로 B, A의 선택은 떨어진다) → B 뒤 틈. 마지막은 기준 칸·커서가 모두 틈이라 「직사각형의 첫
/// 클립」 = A가 골라져 선택이 B의 노트에서 A의 노트로 바뀐다. 커서를 한 스텝 앞으로 넓혔는데 선택이 다른 클립의
/// 노트로 바뀌어 치는 것이다(「넓히기는 좁히지 않는다」와도 어긋남: B의 선택이 사라진다).
#[gpui_kit::test]
fn brk3_shift_extend_from_gap_flips_picked_clip_a_b_a(cx: &mut TestAppContext) {
    start(cx, "gap-flip");
    no_preview(cx);
    let (handle, view) = open(
        cx,
        clips(vec![
            clip(BAR, BAR, &[note(0, 960, 60)]),
            clip(3 * BAR, BAR, &[note(0, 480, 60)]),
        ]),
    );
    enter_piano_roll(cx, handle, &view);
    let a = clip_id(cx, &view, 0);
    let b = clip_id(cx, &view, 1);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-l shift-l shift-l shift-l");
    assert_eq!(cursor_of(cx, &view), (BAR, 60), "A의 시작");
    assert_eq!(selection_of(cx, &view), vec![(0, 960, 60)]);
    assert_eq!(selection_clip(cx, &view), Some(a));
    press(cx, handle, "shift-l shift-l shift-l shift-l");
    assert_eq!(cursor_of(cx, &view), (2 * BAR, 60), "A 뒤의 틈");
    assert_eq!(selection_clip(cx, &view), Some(a), "첫 클립 규칙: A");
    press(cx, handle, "shift-l shift-l shift-l shift-l");
    assert_eq!(cursor_of(cx, &view), (3 * BAR, 60), "B의 시작");
    assert_eq!(selection_clip(cx, &view), Some(b), "커서 클립 규칙: B");
    assert_eq!(selection_of(cx, &view), vec![(0, 480, 60)]);
    press(cx, handle, "shift-l shift-l shift-l shift-l");
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 60), "B 뒤의 틈");
    assert_eq!(
        selection_clip(cx, &view),
        Some(b),
        "한 스텝 넓혔는데 선택 클립이 B에서 {:?}로 바뀌었다: {:?}",
        selection_clip(cx, &view),
        selection_of(cx, &view)
    );
}

// ---------- 시도 2. 지적 1 모순: 선택을 유지한 채 틈에서 넓히면 선택이 비워진다 ----------

/// `b`로 1마디 노트 3개를 선택하고(5.11 흐름대로 선택을 유지한 채) 커서를 B 뒤의 틈으로 옮긴 뒤 `shift-l`.
/// 직사각형이 어느 클립에도 걸치지 않아 `picked`가 `None`이고 선택이 비워진다. 「넓히기는 좁히지 않는다」가
/// 클립이 없는 자리에서는 서지 않는다.
#[gpui_kit::test]
fn brk3_shift_extend_in_gap_with_kept_selection_clears_it(cx: &mut TestAppContext) {
    start(cx, "gap-clears");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "b");
    assert_eq!(selection_of(cx, &view).len(), 3);
    // 수식키 없는 이동은 선택을 유지한다(5.11). B(4–5마디) 뒤의 틈까지
    set_cursor(cx, &view, 6 * BAR, 60);
    press(cx, handle, "l");
    assert_eq!(
        selection_of(cx, &view).len(),
        3,
        "수식키 없는 이동은 선택을 유지한다"
    );
    press(cx, handle, "shift-l");
    assert_eq!(
        selection_of(cx, &view).len(),
        3,
        "넓히기가 선택을 비웠다(직사각형이 어느 클립에도 걸치지 않음)"
    );
    assert!(has_selection(cx, &view));
}

// ---------- 시도 3. 지적 1 확인: 같은 축으로 되돌아와도, b 뒤 조작 뒤에도 선택이 줄지 않는다 ----------

#[gpui_kit::test]
fn brk3_shift_extend_back_and_after_bar_select_does_not_shrink(cx: &mut TestAppContext) {
    start(cx, "extend-back");
    no_preview(cx);
    let (handle, view) = open(
        cx,
        single_clip(&[
            (0, 960, 60),
            (960, 960, 60),
            (1920, 960, 60),
            (2880, 960, 60),
        ]),
    );
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-l shift-l shift-l");
    assert_eq!(selection_of(cx, &view).len(), 4);
    // 같은 축으로 되돌아온다: 규칙대로면 유지(DAW 관례의 「줄이기」는 아니다)
    press(cx, handle, "shift-h shift-h shift-h");
    assert_eq!(cursor_of(cx, &view), (0, 60));
    assert_eq!(
        selection_of(cx, &view).len(),
        4,
        "되돌아오는 넓히기가 선택을 줄였다"
    );
    // 다른 축으로: 음높이 61로 넓혀도 4개 유지
    press(cx, handle, "shift-k");
    assert_eq!(selection_of(cx, &view).len(), 4);
    // Esc로 비우고 b(기준 칸 없음) → alt-k(기준 칸 없어 옮길 것 없음) → shift-l: 커서는 shift-k 뒤 (0, 61)
    // 그대로이고(선택 조작은 커서를 옮기지 않는다) 직사각형 [0, 1920) × 61의 노트 2개와 선택 4개가 합쳐져 4개다
    press(cx, handle, "escape");
    assert!(selection_of(cx, &view).is_empty());
    assert_eq!(anchor_of(cx, &view), None);
    press(cx, handle, "b");
    assert_eq!(selection_of(cx, &view).len(), 4);
    press(cx, handle, "alt-k");
    assert_eq!(
        selection_of(cx, &view),
        vec![
            (0, 960, 61),
            (960, 960, 61),
            (1920, 960, 61),
            (2880, 960, 61)
        ]
    );
    assert_eq!(
        cursor_of(cx, &view),
        (0, 61),
        "선택 조작은 커서를 옮기지 않는다"
    );
    press(cx, handle, "shift-l");
    assert_eq!(
        selection_of(cx, &view).len(),
        4,
        "b 뒤 조작 뒤 넓히기가 선택을 줄였다"
    );
    assert_eq!(anchor_of(cx, &view), Some((0, 61)));
}

// ---------- 시도 4. 지적 1 교차: 뷰 전환이 선택과 기준 칸을 함께 지우고, 빈 선택의 기준 칸은 Esc가 못 지운다 ----------

#[gpui_kit::test]
fn brk3_view_toggle_clears_anchor_but_escape_with_empty_selection_keeps_it(
    cx: &mut TestAppContext,
) {
    start(cx, "anchor-life");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "shift-l");
    assert_eq!(selection_of(cx, &view), vec![D4]);
    assert_eq!(anchor_of(cx, &view), Some((960, 62)));
    press(cx, handle, "v");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    assert!(
        selection_of(cx, &view).is_empty(),
        "뷰 전환은 선택을 비운다"
    );
    assert_eq!(anchor_of(cx, &view), None, "뷰 전환은 기준 칸도 지운다");
    enter_piano_roll(cx, handle, &view);
    // 빈 칸을 넓힌다: 선택은 비고 기준 칸만 남는다. Esc(Selection 컨텍스트 없음)는 기준 칸을 지우지 않는다
    set_cursor(cx, &view, 10 * QUARTER, 40);
    press(cx, handle, "shift-l shift-l");
    assert!(selection_of(cx, &view).is_empty());
    assert_eq!(anchor_of(cx, &view), Some((10 * QUARTER, 40)));
    press(cx, handle, "escape");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll,
        "Esc가 뷰를 바꿨다"
    );
    assert_eq!(
        anchor_of(cx, &view),
        None,
        "빈 선택의 기준 칸이 Esc 뒤에도 남아 다음 Shift+이동이 옛 기준 칸에서 넓힌다"
    );
}

// ---------- 시도 5. 지적 3 확인: Shorten 연타가 격자를 지키며 계속 적용되고, 끝에서 거부·커서 유지 ----------

#[gpui_kit::test]
fn brk3_shorten_repeat_keeps_grid_until_one_step_then_rejects(cx: &mut TestAppContext) {
    start(cx, "shorten-grid");
    no_preview(cx);
    let (handle, view) = open(cx, single_clip(&[(0, 3840, 60)]));
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 2880, 60);
    press(cx, handle, "alt-shift-h");
    assert_eq!(clip_a_notes(cx, &view), vec![(0, 2880, 60)]);
    assert_eq!(cursor_of(cx, &view), (1920, 60));
    press(cx, handle, "alt-shift-h");
    assert_eq!(clip_a_notes(cx, &view), vec![(0, 1920, 60)]);
    assert_eq!(cursor_of(cx, &view), (960, 60));
    press(cx, handle, "alt-shift-h");
    assert_eq!(clip_a_notes(cx, &view), vec![(0, 960, 60)]);
    assert_eq!(cursor_of(cx, &view), (0, 60));
    press(cx, handle, "alt-shift-h");
    assert_eq!(
        clip_a_notes(cx, &view),
        vec![(0, 960, 60)],
        "한 스텝 이하는 그대로"
    );
    assert_eq!(cursor_of(cx, &view), (0, 60));
    assert!(
        last_message(cx, &view)
            .unwrap_or_default()
            .contains("한 스텝 이하")
    );
    // 연타 네 번(세 번 적용)이 undo 한 단위
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a_notes(cx, &view), vec![(0, 3840, 60)]);
    press(cx, handle, "l");
    assert_eq!(cursor_of(cx, &view), (960, 60));
}

// ---------- 시도 6. 지적 3 경계: 스텝보다 짧은 노트와 격자 밖 시작 ----------

/// `[0, 480)`에 `>` `alt-l` `<` `>` `<`: 커서는 늘 노트 안이고 격자 위다. 격자 밖 시작 `[100, 1500)`에서 커서
/// 960으로 `<`: 한 스텝 물러난 0이 시작 앞이라 시작(100)으로 간다(계획 2 「그래도 시작 앞이면 시작」). 그 뒤 `l`은
/// 1060이다. `n`/`p`도 노트 시작으로 가므로 노트가 격자 밖이면 커서도 밖이다. 설계대로인지 확인한다.
#[gpui_kit::test]
fn brk3_cursor_follow_with_short_note_and_off_grid_start(cx: &mut TestAppContext) {
    start(cx, "short-offgrid");
    no_preview(cx);
    let (handle, view) = open(cx, single_clip(&[(0, 480, 60), (BAR + 100, 1400, 64)]));
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "alt-shift-l");
    assert!(clip_a_notes(cx, &view).contains(&(0, 1440, 60)));
    assert_eq!(cursor_of(cx, &view), (0, 60));
    press(cx, handle, "alt-l");
    assert!(clip_a_notes(cx, &view).contains(&(960, 1440, 60)));
    assert_eq!(cursor_of(cx, &view), (960, 60));
    press(cx, handle, "alt-shift-h");
    assert!(clip_a_notes(cx, &view).contains(&(960, 480, 60)));
    assert_eq!(cursor_of(cx, &view), (960, 60), "노트 [960, 1440) 안");
    press(cx, handle, "alt-shift-h");
    assert!(
        clip_a_notes(cx, &view).contains(&(960, 480, 60)),
        "한 스텝 이하 거부"
    );
    assert_eq!(cursor_of(cx, &view), (960, 60));
    press(cx, handle, "alt-h");
    assert!(clip_a_notes(cx, &view).contains(&(0, 480, 60)));
    assert_eq!(
        cursor_of(cx, &view),
        (0, 60),
        "변위가 음수여도 노트 안·격자 위"
    );
    // 격자 밖 시작: [3940, 5340) × 64, 커서 4800(격자 위, 노트 안)
    set_cursor(cx, &view, BAR + 960, 64);
    press(cx, handle, "alt-shift-h");
    assert!(clip_a_notes(cx, &view).contains(&(BAR + 100, 440, 64)));
    assert_eq!(
        cursor_of(cx, &view),
        (BAR + 100, 64),
        "한 스텝 물러난 3840은 시작 앞이라 시작으로"
    );
    press(cx, handle, "l");
    assert_eq!(
        cursor_of(cx, &view),
        (BAR + 100 + 960, 64),
        "그 뒤 l은 격자 밖(설계대로)"
    );
}

// ---------- 시도 7. 지적 4 확인: 그대로 둔 노트가 남고 다음 조작이 둘에 가며, 전무 거부 뒤 선택 불변 ----------

#[gpui_kit::test]
fn brk3_shorten_keeps_short_note_then_lengthen_hits_both_and_all_or_nothing_keeps_selection(
    cx: &mut TestAppContext,
) {
    start(cx, "shorten-both");
    no_preview(cx);
    let (handle, view) = open(
        cx,
        single_clip(&[(0, 960, 60), (960, 1920, 62), (0, 960, 120)]),
    );
    enter_piano_roll(cx, handle, &view);
    select(cx, &view, &[(0, 960, 60), (960, 1920, 62)]);
    press(cx, handle, "alt-shift-h");
    assert_eq!(selection_of(cx, &view), vec![(0, 960, 60), (960, 960, 62)]);
    press(cx, handle, "alt-shift-l");
    assert_eq!(
        selection_of(cx, &view),
        vec![(0, 1920, 60), (960, 1920, 62)],
        "그대로 둔 C4에도 다음 조작이 가야 한다"
    );
    assert!(clip_a_notes(cx, &view).contains(&(0, 1920, 60)));
    assert!(clip_a_notes(cx, &view).contains(&(960, 1920, 62)));
    // 전무 거부: 120은 옥타브 위가 밖. 선택·문서 불변
    select(cx, &view, &[(0, 1920, 60), (0, 960, 120)]);
    let before = clip_a_notes(cx, &view);
    press(cx, handle, "alt-shift-k");
    assert_eq!(clip_a_notes(cx, &view), before);
    assert_eq!(selection_of(cx, &view), vec![(0, 960, 120), (0, 1920, 60)]);
}

// ---------- 시도 8. 지적 5 우회: 수식키 조합·시퀀스·key_char 표기·수식키 교환 ----------

/// `Keystroke::parse`가 `key == "f20"`으로 읽는 모든 표기가 거부되고, `g` 뒤 Esc는 아무 Action도 내지 않는다.
#[gpui_kit::test]
fn brk3_every_spelling_of_f20_is_rejected_even_with_swap(cx: &mut TestAppContext) {
    start(cx, "f20-spellings");
    let path = cx.update(|cx| keymap::user_path(cx));
    std::fs::write(
        &path,
        r#"[{"context": "Normal", "bindings": {
            "shift-f20": "view::Toggle",
            "ctrl-F20": "view::Toggle",
            "fn-f20": "view::Toggle",
            "cmd-f20": "view::Toggle",
            "alt-F20": "view::Toggle",
            "g F20": "view::Toggle",
            "f20->x": "view::Toggle"
        }}]"#,
    )
    .unwrap();
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let errors = f20_errors(cx);
    assert_eq!(errors.len(), 7, "{errors:#?}");
    assert!(
        errors.iter().all(|e| e.contains("예약")),
        "잘못된 키가 아니라 예약 키 거부여야 한다: {errors:#?}"
    );
    // 수식키 교환을 켜도 같다(키 이름은 바뀌지 않는다)
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    assert_eq!(f20_errors(cx).len(), 7);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    cx.simulate_keystrokes(handle, "g");
    press(cx, handle, "escape");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    // 뷰는 피아노롤 그대로이고 `g a`(vim view::Toggle)는 여전히 산다
    press(cx, handle, "g a");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
}

// ---------- 시도 9. 지적 6 확인: save_user 거부는 파일·.bak·엔진 오류를 건드리지 않는다 ----------

#[gpui_kit::test]
fn brk3_save_user_with_f20_leaves_file_bak_and_errors_untouched(cx: &mut TestAppContext) {
    start(cx, "f20-save");
    let path = cx.update(|cx| keymap::user_path(cx));
    // 손으로 쓴 파일(주석이 있어 편집기가 쓴 것이 아니다)
    let hand = "// 손으로 쓴 키맵\n[{\"context\": \"Normal\", \"bindings\": {\"ctrl-alt-u\": \"track::ToggleMute\"}}]\n";
    std::fs::write(&path, hand).unwrap();
    cx.update(keymap::apply);
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
    let (layers, user) = cx.update(|cx| keymap::current_layers(cx));
    let mut user = user.expect("읽히는 사용자 레이어");
    let eff = keymap::user::effective(&layers);
    user.bind("view::Toggle", Some("Normal"), "F20", &eff);
    let saved = cx.update(|cx| keymap::save_user(cx, &user));
    assert!(saved.is_err(), "{saved:?}");
    assert!(saved.unwrap_err().contains("f20"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        hand,
        "파일이 바뀌었다"
    );
    assert!(
        !path.with_extension("json.bak").exists(),
        "거부인데 .bak을 남겼다"
    );
    assert!(!path.with_extension("json.tmp").exists());
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
    // 가져오기 경로(`import_from` = parse → save_user): F20이 든 파일은 거부되고 아무것도 쓰지 않는다
    let imported = keymap::user::UserKeymap::parse(
        r#"[{"context": "Normal", "bindings": {"F20": "view::Toggle", "ctrl-alt-m": "track::ToggleMute"}}]"#,
    )
    .expect("문법은 맞다(예약 키는 parse가 아니라 build·save_user가 거부)");
    let saved = cx.update(|cx| keymap::save_user(cx, &imported));
    assert!(saved.is_err(), "{saved:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), hand);
}

// ---------- 시도 10. 지적 6 여파: 손으로 쓴 f20 항목 하나가 편집기의 모든 쓰기를 막는다 ----------

/// 사용자가 손으로 `f20`을 걸어 둔 파일(핫리로드는 KM-6 오류 한 줄). 수정 전(`8cc5d26`)에는 편집기로 다른 Action을
/// 바꿀 수 있었다(f20 줄은 그대로 오류). 수정 뒤 `save_user`가 파일 전체를 훑어 거부하므로 press-to-bind·지우기가
/// 모두 「키 'f20'는 쓸 수 없다」로 실패하고, 그 f20 항목은 실효 바인딩이 아니라 `clear`로도 없앨 수 없다.
#[gpui_kit::test]
fn brk3_hand_written_f20_blocks_every_editor_write(cx: &mut TestAppContext) {
    start(cx, "f20-blocks-editor");
    let path = cx.update(|cx| keymap::user_path(cx));
    std::fs::write(
        &path,
        "// 손으로 쓴 키맵\n[{\"context\": \"Normal\", \"bindings\": {\"f20\": \"view::Toggle\"}}]\n",
    )
    .unwrap();
    cx.update(keymap::apply);
    assert_eq!(f20_errors(cx).len(), 1, "핫리로드의 KM-6 오류 한 줄");
    run_settings(cx);
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    // press-to-bind: track::ToggleMute에 ctrl-alt-u
    select_action(cx, handle, "track::togglemute");
    press(cx, handle, "enter");
    assert!(cx.read(|cx| view.read(cx).capturing()));
    press(cx, handle, "ctrl-alt-u");
    settle(cx, handle);
    let message = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    let text = user_file(cx);
    assert!(
        text.contains("ctrl-alt-u") && text.contains("track::ToggleMute"),
        "다른 Action의 바인딩이 파일의 f20 항목 때문에 거부됐다. 메시지: {message} / 파일: {text}"
    );
}

/// 같은 파일에서 빠져나오는 길: 그 Action(`view::Toggle`)을 골라 지우기(`backspace`) 또는 복원(`r`). 실효 바인딩은
/// `last_user`(읽힌 레이어, build가 뺀 것 포함)로 계산하므로 `clear`도 f20 항목을 찾아 지운다. 둘 다 파일에서
/// f20이 사라지고 저장이 된다. 다른 Action을 고친 채로는 못 빠져나온다(앞 시험).
#[gpui_kit::test]
fn brk3_hand_written_f20_is_removed_by_clear_or_restore_of_that_action(cx: &mut TestAppContext) {
    start(cx, "f20-clear-restore");
    let path = cx.update(|cx| keymap::user_path(cx));
    let hand = "// 손으로 쓴 키맵\n[{\"context\": \"Normal\", \"bindings\": {\"f20\": \"view::Toggle\"}}]\n";
    std::fs::write(&path, hand).unwrap();
    cx.update(keymap::apply);
    run_settings(cx);
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    select_action(cx, handle, "view::toggle");
    assert_eq!(
        cx.read(|cx| view.read(cx).selected_action(cx)).as_deref(),
        Some("view::Toggle")
    );
    press(cx, handle, "backspace");
    let message = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    let text = user_file(cx);
    assert!(
        !text.contains("f20"),
        "clear가 f20 항목을 못 없앴다: {message} / {text}"
    );
    assert!(message.contains("지웠다"), "{message}");
    assert!(f20_errors(cx).is_empty());
    // 복원도 같다
    std::fs::write(&path, hand).unwrap();
    cx.update(keymap::apply);
    assert_eq!(f20_errors(cx).len(), 1);
    press(cx, handle, "r");
    let text = user_file(cx);
    assert!(
        !text.contains("f20"),
        "restore가 f20 항목을 못 없앴다: {text}"
    );
    assert!(f20_errors(cx).is_empty());
}

// ---------- 시도 11. 제안 9 확인: 거부 메시지의 안내와 계획의 한 줄 ----------

#[gpui_kit::test]
fn brk3_rejection_message_names_shift_p_and_cmd_left(cx: &mut TestAppContext) {
    start(cx, "reject-message");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 14400, 65);
    press(cx, handle, "shift-l");
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 65));
    assert_eq!(selection_of(cx, &view), vec![(14400, 960, 65)]);
    press(cx, handle, "alt-k");
    let msg = last_message(cx, &view).unwrap_or_default();
    assert!(msg.contains("Shift+p") && msg.contains("Cmd+←"), "{msg}");
    // 안내대로 Shift+p로 돌아가면 된다
    press(cx, handle, "shift-p");
    assert_eq!(cursor_of(cx, &view), (0, 65));
    press(cx, handle, "alt-k");
    assert_eq!(selection_of(cx, &view), vec![(14400, 960, 66)]);
    let plan = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports/s3-t2/plan.md"),
    )
    .unwrap();
    assert!(plan.contains("넓히기는 좁히지 않는다"));
    assert!(plan.contains("직사각형의 시간 범위에 걸치는 첫 클립"));
    assert!(plan.contains("커서가 그 클립 밖이면 `Alt` 조작은 거부된다"));
}
