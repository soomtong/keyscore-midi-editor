//! S3-T2 적대적 리뷰 4회차(좁은 확인)의 깨뜨리기 시도(`docs/reports/s3-t2/review-4.md` 3절). 범위는 3회차 지적
//! 1·2, 제안 4·5의 수정(`fe5d3b5`)과 그 회귀다. 도우미는 `review_s3_t2_r3.rs`에서 복사했다. 통과한 시도도
//! 「못 깨뜨림」으로 남기기 위해 모두 둔다.
#![allow(
    dead_code,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::keymap;
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

// ---------- 도우미(review_s3_t2_r3.rs 복사) ----------

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rev32r4-{}-{name}", std::process::id()));
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

fn current_track(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<TrackId> {
    cx.read(|cx| view.read(cx).current_track())
}

fn track_id(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, index: usize) -> TrackId {
    cx.read(|cx| view.read(cx).document().read(cx).project().tracks()[index].id())
}

fn view_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> View {
    cx.read(|cx| view.read(cx).mode_state().view())
}

/// 트랙 0: 클립 A(0–4마디)에 C4 셋(0·960·1920). 트랙 1: 클립 C(4–5마디)에 G4 하나. 트랙 1의 0–4마디는 틈이다.
fn two_tracks() -> Project {
    let mut project = Project::new();
    let mut t0 = Track::new("T0", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    t0.insert_clip(clip(
        0,
        4 * BAR,
        &[note(0, 960, 60), note(960, 960, 60), note(1920, 960, 60)],
    ))
    .unwrap();
    project.push_track(t0).unwrap();
    let mut t1 = Track::new("T1", Channel::new(2).unwrap(), TrackColor::new(1).unwrap());
    t1.insert_clip(clip(4 * BAR, BAR, &[note(0, 960, 67)]))
        .unwrap();
    project.push_track(t1).unwrap();
    project
}

// ---------- 시도 1. 3회차 지적 1 수정의 회귀: 「지금 선택의 클립」 규칙이 트랙을 보지 않는다 ----------

/// 트랙 0의 C4@0을 선택하고(5.11 「선택을 유지한 채 다음 트랙」) `Tab`으로 트랙 1에 간다. 트랙 1의 틈(960)에서
/// `shift-l`: 기준 칸·커서가 모두 틈이라 새 규칙 「지금 선택의 클립」이 트랙 0의 클립 A를 고르고, 트랙 1 화면에서
/// 그은 직사각형 `[960, 2880) × C4`로 트랙 0의 C4@960·C4@1920을 선택에 더한다. 그 노트는 트랙 1의 피아노롤에
/// 그려지지 않는다(장면은 선택의 클립이 현재 클립일 때만 `selected`로 그린다). 트랙 0으로 돌아가 `Alt` 조작을
/// 하면 사용자가 고른 적 없는 노트가 함께 옮겨진다. 수정 전(첫 클립 규칙은 현재 트랙만 본다)에는 생기지 않았다.
#[gpui_kit::test]
fn brk4_shift_extend_on_other_track_gap_does_not_add_hidden_notes_of_selection_track(
    cx: &mut TestAppContext,
) {
    start(cx, "cross-track");
    no_preview(cx);
    let (handle, view) = open(cx, two_tracks());
    enter_piano_roll(cx, handle, &view);
    let t0 = track_id(cx, &view, 0);
    let t1 = track_id(cx, &view, 1);
    assert_eq!(current_track(cx, &view), Some(t0));
    select(cx, &view, &[(0, 960, 60)]);
    press(cx, handle, "tab");
    assert_eq!(current_track(cx, &view), Some(t1), "Tab은 다음 트랙");
    assert_eq!(
        selection_of(cx, &view),
        vec![(0, 960, 60)],
        "트랙을 바꿔도 선택은 남는다(5.11)"
    );
    // 트랙 1의 틈. 수식키 없는 이동으로 기준 칸을 지운 자리에서 넓힌다
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "l h");
    assert_eq!(cursor_of(cx, &view), (960, 60));
    assert_eq!(anchor_of(cx, &view), None);
    press(cx, handle, "shift-l");
    assert_eq!(cursor_of(cx, &view), (1920, 60));
    assert_eq!(
        selection_of(cx, &view),
        vec![(0, 960, 60)],
        "트랙 1에서 그은 직사각형이 트랙 0(보이지 않는 트랙)의 노트를 선택에 더했다"
    );
    // 트랙 0으로 돌아가 Alt 조작: 고른 적 없는 노트가 옮겨지지 않아야 한다
    press(cx, handle, "shift-tab");
    assert_eq!(current_track(cx, &view), Some(t0));
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "h l");
    press(cx, handle, "alt-k");
    assert_eq!(
        clip_a_notes(cx, &view),
        vec![(0, 960, 61), (960, 960, 60), (1920, 960, 60)],
        "C4@0만 골랐는데 다른 노트도 반음 올라갔다"
    );
}

// ---------- 시도 2. 3회차 제안 5 수정의 회귀: 기준 칸만 남은 상태가 뷰 전환 뒤에 산다 ----------

/// 빈 칸을 넓혀 기준 칸만 남긴(이제 `Selection` 컨텍스트) 뒤 `v`. FSM은 선택을 비우지만 `apply_mode_event`는 실제
/// 선택이 비어 있으면 기준 칸을 지우지 않는다. 그 뒤 배치 뷰에서 문서가 바뀌면(`m`, 뮤트) `selection_document_changed`
/// → `sync_selection_mode`가 기준 칸을 보고 `Select`를 넣어 배치 뷰가 `Normal > Arrangement > Selection`(노트 선택
/// 없음)이 된다. 피아노롤로 돌아가 `shift-l`은 옛 기준 칸에서 넓힌다. 계획 1 「뷰를 바꾸면 … 비운다」, 보고서
/// 「기준 칸 생명주기(… 뷰 전환이 지움)」와 어긋난다.
#[gpui_kit::test]
fn brk4_view_toggle_with_anchor_only_clears_anchor_and_keeps_arrangement_without_selection(
    cx: &mut TestAppContext,
) {
    start(cx, "anchor-only-view");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 10 * QUARTER, 40);
    press(cx, handle, "shift-l shift-l");
    assert!(selection_of(cx, &view).is_empty());
    assert_eq!(anchor_of(cx, &view), Some((10 * QUARTER, 40)));
    assert!(has_selection(cx, &view), "제안 5: 기준 칸만 있어도 선택 중");
    press(cx, handle, "v");
    assert_eq!(view_of(cx, &view), View::Arrangement);
    assert!(!has_selection(cx, &view));
    let anchor_after_toggle = anchor_of(cx, &view);
    // 배치 뷰에서 문서를 바꾼다(뮤트)
    press(cx, handle, "m");
    let arrangement_selection = has_selection(cx, &view);
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-l");
    let anchor_after_extend = anchor_of(cx, &view);
    assert_eq!(
        (
            anchor_after_toggle,
            arrangement_selection,
            anchor_after_extend
        ),
        (None, false, Some((0, 60))),
        "(뷰 전환 뒤 기준 칸, 배치 뷰의 선택 컨텍스트, 돌아와 넓힌 기준 칸)"
    );
}

// ---------- 시도 3. 3회차 제안 5 확인: 기준 칸만 남은 상태의 진입·해제 ----------

/// 기준 칸만 남은 상태에서 (a) 수식키 없는 이동은 기준 칸과 `Selection` 컨텍스트를 함께 지우고, (b) `Esc`는 기준
/// 칸을 지우고 뷰를 바꾸지 않으며, 다음 `shift-l`은 옮기기 전 커서 칸에서 넓힌다. (c) 기준 칸만 남은 상태의 `Esc`
/// 두 번째는 아무 일도 없다(5.8).
#[gpui_kit::test]
fn brk4_anchor_only_state_is_left_by_plain_move_and_escape(cx: &mut TestAppContext) {
    start(cx, "anchor-only-exit");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 10 * QUARTER, 40);
    press(cx, handle, "shift-l");
    assert!(has_selection(cx, &view));
    press(cx, handle, "l");
    assert_eq!(anchor_of(cx, &view), None);
    assert!(
        !has_selection(cx, &view),
        "수식키 없는 이동 뒤 컨텍스트가 남았다"
    );
    press(cx, handle, "shift-l");
    assert_eq!(anchor_of(cx, &view), Some((12 * QUARTER, 40)));
    press(cx, handle, "escape");
    assert_eq!(anchor_of(cx, &view), None);
    assert!(!has_selection(cx, &view));
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    press(cx, handle, "escape");
    assert_eq!(
        view_of(cx, &view),
        View::PianoRoll,
        "두 번째 Esc가 뷰를 바꿨다"
    );
    // D4(960, 62)를 품는 자리에서 다시 넓히면 옛 기준 칸이 아니라 옮기기 전 커서 칸이 기준이다
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "shift-l");
    assert_eq!(anchor_of(cx, &view), Some((960, 62)));
    assert_eq!(selection_of(cx, &view), vec![D4]);
}

// ---------- 시도 4. 3회차 지적 1 확인: 같은 트랙에서 틈 → A → 틈 → B → 틈 → 되돌아오기 ----------

/// 3회차 시도 1의 경로 뒤에 B 뒤 틈에서 `shift-l`을 더 눌러도(직사각형이 B를 덮은 채) 선택은 B이고, 노트 없는 틈으로
/// 더 나가도 B의 선택이 줄지 않는다(노트 시작은 클립 기준 틱).
#[gpui_kit::test]
fn brk4_extend_past_b_into_gap_keeps_b_selection(cx: &mut TestAppContext) {
    start(cx, "keep-b");
    no_preview(cx);
    let (handle, view) = open(
        cx,
        clips(vec![
            clip(BAR, BAR, &[note(0, 960, 60)]),
            clip(3 * BAR, BAR, &[note(0, 480, 60)]),
        ]),
    );
    enter_piano_roll(cx, handle, &view);
    let b = clip_id(cx, &view, 1);
    set_cursor(cx, &view, 0, 60);
    for _ in 0..16 {
        press(cx, handle, "shift-l");
    }
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 60));
    assert_eq!(selection_clip(cx, &view), Some(b));
    for _ in 0..8 {
        press(cx, handle, "shift-l");
    }
    assert_eq!(cursor_of(cx, &view), (6 * BAR, 60));
    assert_eq!(selection_clip(cx, &view), Some(b));
    assert_eq!(selection_of(cx, &view), vec![(0, 480, 60)]);
    assert!(has_selection(cx, &view));
}

// ---------- 시도 5. 3회차 지적 2 수정의 우회: 「이미 파일에 있는 키」는 다른 Action에도 새로 걸린다 ----------

/// 손으로 쓴 파일에 `f20: view::Toggle`(Normal)이 있다. 편집기 API로 `track::ToggleMute`(Normal > PianoRoll)에
/// `f20`을 새로 걸고 `save_user`. 새 검사는 키 글자만 보고(`already.contains(&b.keys)`) 「파일에 이미 있는
/// 키」면 통과시키므로, 새로 넣는 예약 키 바인딩이 파일에 쓰이고 `Ok`(거짓 성공)이며 KM-6 오류가 하나 늘어난다.
/// 2회차 지적 6(편집기가 f20을 파일에 씀)이 이 조건에서 다시 열린다.
#[gpui_kit::test]
fn brk4_save_user_rejects_new_f20_binding_even_if_file_already_has_f20(cx: &mut TestAppContext) {
    start(cx, "f20-already");
    let path = cx.update(|cx| keymap::user_path(cx));
    let hand = "// 손으로 쓴 키맵\n[{\"context\": \"Normal\", \"bindings\": {\"f20\": \"view::Toggle\"}}]\n";
    std::fs::write(&path, hand).unwrap();
    cx.update(keymap::apply);
    assert_eq!(f20_errors(cx).len(), 1);
    let (layers, user) = cx.update(|cx| keymap::current_layers(cx));
    let mut user = user.expect("읽히는 사용자 레이어");
    let eff = keymap::user::effective(&layers);
    user.bind("track::ToggleMute", Some("Normal > PianoRoll"), "f20", &eff);
    let saved = cx.update(|cx| keymap::save_user(cx, &user));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        saved.is_err() && !text.contains("track::ToggleMute"),
        "새 f20 바인딩이 쓰였다: {saved:?} / 파일: {text} / KM-6 f20 오류 {}개",
        f20_errors(cx).len()
    );
}

/// 같은 우회를 설정 창으로: 손으로 쓴 `f20`(Dialog 컨텍스트) 파일에서 `track::ToggleMute`를 골라 press-to-bind로
/// `f20`을 누른다. 거부 메시지가 나와야 하고 파일에 `track::ToggleMute`의 f20이 없어야 한다.
#[gpui_kit::test]
fn brk4_press_to_bind_f20_is_rejected_even_if_file_already_has_f20(cx: &mut TestAppContext) {
    start(cx, "f20-already-ui");
    let path = cx.update(|cx| keymap::user_path(cx));
    std::fs::write(
        &path,
        "// 손으로 쓴 키맵\n[{\"context\": \"Dialog\", \"bindings\": {\"f20\": \"dialog::Confirm\"}}]\n",
    )
    .unwrap();
    cx.update(keymap::apply);
    assert_eq!(f20_errors(cx).len(), 1);
    run_settings(cx);
    let (handle, view) = settings_view(cx);
    activate(cx, handle);
    select_action(cx, handle, "track::togglemute");
    press(cx, handle, "enter");
    assert!(cx.read(|cx| view.read(cx).capturing()));
    press(cx, handle, "f20");
    settle(cx, handle);
    let message = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    let text = user_file(cx);
    assert!(
        !text.contains("track::ToggleMute") && message.contains("예약"),
        "편집기가 f20을 새로 걸었다. 메시지: {message} / 파일: {text} / KM-6 f20 오류 {}개",
        f20_errors(cx).len()
    );
}

// ---------- 시도 6. 3회차 지적 2 확인: 손으로 쓴 예약 키 표기 여럿 + 다른 편집 ----------

/// 손으로 쓴 파일에 `F20`(대문자)·`shift-f20`·`g f20` 셋. 편집기 API로 다른 Action에 `ctrl-alt-u`를 걸면 쓰이고,
/// 손으로 쓴 셋은 파일에 그대로 남아 KM-6 오류 셋으로만 보이며, 손으로 쓴 원본은 `.bak`에 남는다. 새로 넣는
/// `F20`은 여전히 거부된다.
#[gpui_kit::test]
fn brk4_hand_written_f20_spellings_survive_other_edit_and_new_f20_is_rejected(
    cx: &mut TestAppContext,
) {
    start(cx, "f20-spellings");
    let path = cx.update(|cx| keymap::user_path(cx));
    let hand = "// 손으로 쓴 키맵\n[{\"context\": \"Normal\", \"bindings\": {\"F20\": \"view::Toggle\", \"shift-f20\": \"track::Next\", \"g f20\": \"track::Prev\"}}]\n";
    std::fs::write(&path, hand).unwrap();
    cx.update(keymap::apply);
    assert_eq!(f20_errors(cx).len(), 3);
    let (layers, user) = cx.update(|cx| keymap::current_layers(cx));
    let mut user = user.expect("읽히는 사용자 레이어");
    let eff = keymap::user::effective(&layers);
    user.bind("track::ToggleMute", Some("Normal"), "ctrl-alt-u", &eff);
    let saved = cx.update(|cx| keymap::save_user(cx, &user));
    assert!(saved.is_ok(), "{saved:?}");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("ctrl-alt-u"), "{text}");
    for k in ["F20", "shift-f20", "g f20"] {
        assert!(text.contains(k), "손으로 쓴 {k}가 사라졌다: {text}");
    }
    assert_eq!(f20_errors(cx).len(), 3);
    assert_eq!(
        std::fs::read_to_string(path.with_extension("json.bak")).unwrap(),
        hand
    );
    // 새로 넣는 대문자 F20은 다른 컨텍스트라도 거부
    let (layers, user) = cx.update(|cx| keymap::current_layers(cx));
    let mut user = user.expect("편집기가 쓴 파일");
    let eff = keymap::user::effective(&layers);
    user.bind(
        "track::ToggleSolo",
        Some("Normal > PianoRoll"),
        "alt-F20",
        &eff,
    );
    let before = std::fs::read_to_string(&path).unwrap();
    let saved = cx.update(|cx| keymap::save_user(cx, &user));
    assert!(saved.is_err(), "{saved:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
}
