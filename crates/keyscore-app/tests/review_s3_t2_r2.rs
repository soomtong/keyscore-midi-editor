//! S3-T2 적대적 리뷰 2회차의 깨뜨리기 시도(`docs/reports/s3-t2/review-2.md` 3절). 1회차 수정(지적 1–5, 제안
//! 7·10·11)을 다른 경로로 다시 깨뜨린다. 도우미는 `review_s3_t2.rs`에서 복사했다. 통과한 시도도 「못 깨뜨림」으로
//! 남기기 위해 모두 둔다.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

// ---------- 도우미(review_s3_t2.rs 복사) ----------

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rev32r2-{}-{name}", std::process::id()));
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
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
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

fn triple(n: &Note) -> (u64, u32, u8) {
    (n.start().get(), n.length(), n.pitch().get())
}

fn cursor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (u64, u8) {
    cx.read(|cx| {
        let c = view.read(cx).piano_roll().cursor();
        (c.tick.get(), c.pitch.get())
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

fn has_selection(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_state().has_selection())
}

fn ids(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (TrackId, ClipId) {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        (track.id(), track.clips().next().unwrap().0)
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

// ---------- 시도 1. 지적 1 확인: 선택이 다른 클립이면 거부·알림·선택 유지, 돌아오면 다시 됨 ----------

#[gpui_kit::test]
fn brk2_alt_op_with_selection_in_other_clip_rejects_and_keeps_selection(cx: &mut TestAppContext) {
    start(cx, "other-clip-reject");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "shift-l");
    assert_eq!(selection_of(cx, &view), vec![D4]);
    press(cx, handle, "shift-n");
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 62));
    let undo_before = cx.read(|cx| view.read(cx).document().read(cx).undo_len());
    press(cx, handle, "alt-k");
    assert!(clip_a_notes(cx, &view).contains(&D4), "A의 D4가 바뀌었다");
    assert_eq!(
        selection_of(cx, &view),
        vec![D4],
        "선택은 남아야 한다(5.11)"
    );
    assert!(has_selection(cx, &view));
    let msg = last_message(cx, &view).unwrap_or_default();
    assert!(msg.contains("선택이 현재 클립에 없다"), "{msg}");
    assert_eq!(
        cx.read(|cx| view.read(cx).document().read(cx).undo_len()),
        undo_before,
        "거부는 문서를 바꾸지 않는다"
    );
    // 선택의 클립으로 돌아오면(Shift+p는 앞 클립 A의 시작) 같은 선택에 조작이 된다
    press(cx, handle, "shift-p");
    assert_eq!(cursor_of(cx, &view), (0, 62));
    press(cx, handle, "alt-k");
    assert_eq!(selection_of(cx, &view), vec![(960, 960, 63)]);
    assert!(clip_a_notes(cx, &view).contains(&(960, 960, 63)));
}

// ---------- 시도 2. 지적 1 우회: 커서가 클립 사이 틈(현재 클립 없음)일 때 ----------

#[gpui_kit::test]
fn brk2_alt_op_with_selection_while_cursor_in_gap_rejects(cx: &mut TestAppContext) {
    start(cx, "gap-reject");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    select(cx, &view, &[D4]);
    // 클립 B(4–5마디) 뒤의 빈 자리
    set_cursor(cx, &view, 6 * BAR, 62);
    press(cx, handle, "alt-k");
    assert!(
        clip_a_notes(cx, &view).contains(&D4),
        "커서가 틈인데 보이지 않는 선택이 바뀌었다"
    );
    assert_eq!(selection_of(cx, &view), vec![D4]);
    let msg = last_message(cx, &view).unwrap_or_default();
    assert!(msg.contains("선택이 현재 클립에 없다"), "{msg}");
}

// ---------- 시도 3. 지적 1·2 교차: Shift+l로 A의 경계를 넘어 커서만 B에 든 뒤 Alt 조작 ----------

/// 기준 칸은 A(14400, F4), `shift-l`로 커서는 B의 시작(15360). 선택은 A의 F4(기준 칸의 클립), 현재 클립은 B.
/// 5.3상 흐린 클립의 노트라 거부가 맞고 문서는 그대로여야 한다. B의 G4(다른 음높이)는 들지 않는다.
#[gpui_kit::test]
fn brk2_shift_extend_across_boundary_selects_anchor_clip_and_alt_op_is_rejected(
    cx: &mut TestAppContext,
) {
    start(cx, "across-boundary");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 14400, 65);
    press(cx, handle, "shift-l");
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 65), "커서는 B의 시작");
    assert_eq!(
        selection_of(cx, &view),
        vec![(14400, 960, 65)],
        "기준 칸의 클립 A에서 고른다"
    );
    let before = clip_a_notes(cx, &view);
    press(cx, handle, "alt-k");
    assert_eq!(clip_a_notes(cx, &view), before, "흐린 A의 노트가 바뀌었다");
    assert_eq!(selection_of(cx, &view), vec![(14400, 960, 65)]);
    assert!(
        last_message(cx, &view)
            .unwrap_or_default()
            .contains("선택이 현재 클립에 없다")
    );
}

// ---------- 시도 4. 지적 2 우회: 기준 칸이 틈이고 커서가 클립을 지나 다시 틈으로 ----------

/// 클립 A(0–1마디), B(2–3마디, 시작에 C4). 틈(1마디+2박)에서 `shift-l` ×3으로 B 안(C4 선택) → `shift-l` ×4로
/// B의 끝을 지나 틈(3마디+1박). 직사각형 `[5760, 13440) × 60`은 여전히 B의 C4를 덮는다. 「넓히기」가 선택을
/// 비우면 안 된다(지적 2·4와 같은 종류).
#[gpui_kit::test]
fn brk2_shift_extend_from_gap_past_clip_end_drops_selection(cx: &mut TestAppContext) {
    start(cx, "gap-to-gap");
    no_preview(cx);
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track
        .insert_clip(clip(0, BAR, &[note(0, 960, 60)]))
        .unwrap();
    track
        .insert_clip(clip(2 * BAR, BAR, &[note(0, 960, 60)]))
        .unwrap();
    project.push_track(track).unwrap();
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, BAR + 2 * QUARTER, 60);
    press(cx, handle, "shift-l shift-l shift-l");
    assert_eq!(cursor_of(cx, &view), (2 * BAR + QUARTER, 60));
    assert_eq!(selection_of(cx, &view), vec![(0, 960, 60)], "지적 2 수정");
    press(cx, handle, "shift-l shift-l shift-l shift-l");
    assert_eq!(
        cursor_of(cx, &view),
        (3 * BAR + QUARTER, 60),
        "커서는 B 뒤의 틈"
    );
    assert_eq!(
        selection_of(cx, &view),
        vec![(0, 960, 60)],
        "직사각형이 여전히 B의 C4를 덮는데 넓히기가 선택을 비웠다"
    );
}

// ---------- 시도 5. 지적 3 우회: 당긴 커서가 스텝 격자를 벗어난다 ----------

/// 노트 `[0, 3840)`, 커서는 격자 위(2880). `<` 한 번 → 노트 `[0, 2880)`, 커서는 `end − 1` = 2879로 당겨진다.
/// 5.3의 커서는 「(틱, 음높이) 한 칸」이고 `h`/`l`은 `tick ± step`이라 그 뒤의 모든 이동이 격자 밖이다.
#[gpui_kit::test]
fn brk2_shorten_pulls_cursor_off_the_step_grid(cx: &mut TestAppContext) {
    start(cx, "off-grid");
    no_preview(cx);
    let (handle, view) = open(cx, single_clip(&[(0, 3840, 60)]));
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 2880, 60);
    press(cx, handle, "alt-shift-h");
    assert_eq!(clip_a_notes(cx, &view), vec![(0, 2880, 60)]);
    let (tick, _) = cursor_of(cx, &view);
    assert_eq!(
        tick % QUARTER,
        0,
        "줄이기 뒤 커서 틱 {tick}이 스텝 격자(960의 배수) 밖이다"
    );
    press(cx, handle, "l");
    let (tick, _) = cursor_of(cx, &view);
    assert_eq!(tick % QUARTER, 0, "그 뒤의 l도 격자 밖({tick})");
}

// ---------- 시도 6. 지적 4 우회: 변위가 커서 쪽을 향하면 직사각형이 줄어든다 ----------

/// 노트 4개(0·960·1920·2880, C4). 커서 0에서 `shift-l` ×3 → 4개 선택(기준 칸 0, 커서 2880). `alt-l` ×3 →
/// 노트는 2880·3840·4800·5760, 기준 칸은 2880으로 따라가지만 커서는 2880에 남는다. 다음 `shift-l`의 직사각형
/// `[2880, 4800)`은 4800·5760 노트를 덮지 않는다. 1회차 지적 4(「넓히기가 선택을 줄인다」)의 다른 경로다.
#[gpui_kit::test]
fn brk2_shift_extend_after_nudge_later_drops_notes_from_selection(cx: &mut TestAppContext) {
    start(cx, "nudge-then-extend");
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
    press(cx, handle, "alt-l alt-l alt-l");
    assert_eq!(
        selection_of(cx, &view),
        vec![
            (2880, 960, 60),
            (3840, 960, 60),
            (4800, 960, 60),
            (5760, 960, 60)
        ]
    );
    assert_eq!(
        cursor_of(cx, &view),
        (2880, 60),
        "선택 조작은 커서를 옮기지 않는다"
    );
    press(cx, handle, "shift-l");
    assert_eq!(cursor_of(cx, &view), (3840, 60));
    assert_eq!(
        selection_of(cx, &view).len(),
        4,
        "넓히기가 방금 옮긴 노트를 떨어뜨렸다: {:?}",
        selection_of(cx, &view)
    );
}

/// 같은 구조를 음높이로: 화음(60·61·62)을 `shift-k` ×2로 선택(기준 칸 60, 커서 62) → `alt-k` ×2(62·63·64, 기준
/// 칸 62, 커서 62) → `shift-k`의 직사각형 `62..=63`이 64를 덮지 않는다.
#[gpui_kit::test]
fn brk2_shift_extend_after_transpose_up_drops_notes_from_selection(cx: &mut TestAppContext) {
    start(cx, "transpose-then-extend");
    no_preview(cx);
    let (handle, view) = open(cx, single_clip(&[(0, 960, 60), (0, 960, 61), (0, 960, 62)]));
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-k shift-k");
    assert_eq!(selection_of(cx, &view).len(), 3);
    press(cx, handle, "alt-k alt-k");
    assert_eq!(
        selection_of(cx, &view),
        vec![(0, 960, 62), (0, 960, 63), (0, 960, 64)]
    );
    press(cx, handle, "shift-k");
    assert_eq!(cursor_of(cx, &view), (0, 63));
    assert_eq!(
        selection_of(cx, &view).len(),
        3,
        "넓히기가 방금 올린 노트를 떨어뜨렸다: {:?}",
        selection_of(cx, &view)
    );
}

// ---------- 시도 7. 조작 뒤 선택 갱신: 줄이기에서 한 스텝 이하 노트가 선택에서 사라진다 ----------

/// 선택 {C4 960, D4 1920}에 `<`. D4만 줄어들고 C4는 「그대로」여야 하는데(계획 2), 선택이 `planned.moved`로
/// 갱신되어 C4가 선택에서 빠진다. 다음 `>`는 D4에만 적용된다.
#[gpui_kit::test]
fn brk2_shorten_drops_unchanged_short_note_from_selection(cx: &mut TestAppContext) {
    start(cx, "shorten-selection");
    no_preview(cx);
    let (handle, view) = open(cx, single_clip(&[(0, 960, 60), (960, 1920, 62)]));
    enter_piano_roll(cx, handle, &view);
    select(cx, &view, &[(0, 960, 60), (960, 1920, 62)]);
    press(cx, handle, "alt-shift-h");
    assert_eq!(clip_a_notes(cx, &view), vec![(0, 960, 60), (960, 960, 62)]);
    assert_eq!(
        selection_of(cx, &view),
        vec![(0, 960, 60), (960, 960, 62)],
        "한 스텝 이하라 그대로 둔 노트가 선택에서 사라졌다"
    );
}

// ---------- 시도 8. 지적 5 우회: 대문자 `F20` ----------

/// `build`는 `k == "f20"`으로 비교하지만 GPUI `Keystroke::parse`는 이름 키를 소문자로 바꾼다. 사용자 키맵의
/// `"F20"`은 검사를 지나 `f20`에 걸리고, `g` 뒤 `escape`가 그 Action을 실행한다.
#[gpui_kit::test]
fn brk2_uppercase_f20_user_binding_bypasses_reservation(cx: &mut TestAppContext) {
    start(cx, "f20-upper");
    let path = cx.update(|cx| keymap::user_path(cx));
    std::fs::write(
        &path,
        r#"[{"context": "Normal", "bindings": {"F20": "view::Toggle"}}]"#,
    )
    .unwrap();
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let errors = cx.update(|cx| keymap::errors(cx).to_vec());
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    cx.simulate_keystrokes(handle, "g");
    press(cx, handle, "escape");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll,
        "Esc가 사용자의 F20 바인딩(view::Toggle)을 실행했다. 키맵 오류 {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| keymap::describe(e).to_lowercase().contains("f20")),
        "F20이 예약 키 검사를 지났다: {errors:?}"
    );
}

// ---------- 시도 9. 지적 5 여파: 키맵 편집기 경로(`UserKeymap::bind` → `save_user`)는 f20을 거부하지 않는다 ----------

/// press-to-bind는 `UserKeymap::bind`로 항목을 넣고 `save_user`로 쓴 뒤 「걸었다」를 말한다. `build`만 거부하므로
/// 파일에는 f20이 남고(매번 KM-6 오류), 편집기 메시지는 거짓 성공이다.
#[gpui_kit::test]
fn brk2_editor_bind_path_writes_f20_and_reports_success(cx: &mut TestAppContext) {
    start(cx, "f20-editor");
    let (layers, user) = cx.update(|cx| keymap::current_layers(cx));
    let mut user = user.expect("빈 사용자 레이어");
    let eff = keymap::user::effective(&layers);
    user.bind("view::Toggle", Some("Normal"), "f20", &eff);
    assert!(user.to_json().contains("f20"));
    let saved = cx.update(|cx| keymap::save_user(cx, &user));
    let errors = cx.update(|cx| keymap::errors(cx).to_vec());
    let path = cx.update(|cx| keymap::user_path(cx));
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        saved.is_err(),
        "편집기 경로가 f20을 파일에 쓰고 성공을 돌려줬다. 파일: {on_disk} / build 오류: {errors:?}"
    );
}

// ---------- 시도 10. 제안 7: Shift+p의 다른 자리들 ----------

#[gpui_kit::test]
fn brk2_shift_p_first_clip_middle_stays_and_gap_goes_to_previous_clip(cx: &mut TestAppContext) {
    start(cx, "shift-p-edges");
    no_preview(cx);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    // 첫 클립 중간: 앞 클립이 없으니 그대로(수정 전에는 클립 시작으로 갔다)
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "shift-p");
    assert_eq!(cursor_of(cx, &view), (960, 60));
    // B 뒤의 틈: 바로 앞 클립 B의 시작
    set_cursor(cx, &view, 6 * BAR, 60);
    press(cx, handle, "shift-p");
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 60));
    // 인접 클립(A.end == B.start)의 B 시작: A
    press(cx, handle, "shift-p");
    assert_eq!(cursor_of(cx, &view), (0, 60));
    // Shift+n은 대칭: A 중간에서 B
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "shift-n");
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 60));
}

// ---------- 시도 11. 제안 10: `:e` 틸드와 기본 폴더 ----------

/// 없는 파일을 열어 실패 메시지(`<풀린 경로>: <이유>`)로 경로가 어떻게 풀렸는지 본다. 홈을 건드리지 않는다.
#[gpui_kit::test]
fn brk2_ex_e_expands_tilde_and_resolves_relative_to_default_dir(cx: &mut TestAppContext) {
    start(cx, "ex-e-tilde");
    no_preview(cx);
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap();
    let (handle, view) = open(cx, fixture());
    let missing = format!("ks-review-r2-missing-{}.ksq", std::process::id());
    press(cx, handle, ":");
    type_text(cx, handle, &format!("e ~/{missing}"));
    press(cx, handle, "enter");
    let msg = last_message(cx, &view).unwrap_or_default();
    let want = home.join(&missing).display().to_string();
    assert!(msg.starts_with(&want), "틸드가 홈으로 풀리지 않았다: {msg}");
    assert!(!msg.contains('~'), "{msg}");
    // 경로 없는 문서의 상대 경로는 저장 시트와 같은 기본 폴더(~/Documents, 없으면 ~)
    let documents = home.join("Documents");
    let base = if documents.is_dir() {
        documents
    } else {
        home.clone()
    };
    press(cx, handle, ":");
    type_text(cx, handle, &format!("e {missing}"));
    press(cx, handle, "enter");
    let msg = last_message(cx, &view).unwrap_or_default();
    let want = base.join(&missing).display().to_string();
    assert!(
        msg.starts_with(&want),
        "상대 경로의 기준이 기본 폴더가 아니다: {msg}"
    );
    // `~` 단독은 홈 그대로(파일이 아니라 실패)
    press(cx, handle, ":");
    type_text(cx, handle, "e ~");
    press(cx, handle, "enter");
    let msg = last_message(cx, &view).unwrap_or_default();
    assert!(msg.starts_with(&home.display().to_string()), "{msg}");
}

// ---------- 시도 12. 제안 11: 미리듣기 실패 알림은 창마다 한 번, 두 번째 창은 따로 ----------

#[gpui_kit::test]
fn brk2_preview_failure_notice_once_per_window(cx: &mut TestAppContext) {
    start(cx, "preview-notice");
    // 엔진이 없는 시험 환경: 커서가 노트 위로 가면 NoEngine
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "n");
    assert_eq!(cursor_of(cx, &view), (960, 62));
    let first = last_message(cx, &view).unwrap_or_default();
    assert!(
        first.contains("미리듣기") && first.contains("창마다 한 번"),
        "{first}"
    );
    let count_before = cx.read(|cx| view.read(cx).messages().len());
    press(cx, handle, "n n n");
    assert_eq!(
        cx.read(|cx| view.read(cx).messages().len()),
        count_before,
        "두 번째 이후 이동에서 다시 알렸다"
    );
    // 다른 창은 자기 플래그
    let (handle2, view2) = open(cx, fixture());
    enter_piano_roll(cx, handle2, &view2);
    set_cursor(cx, &view2, 0, 60);
    press(cx, handle2, "n");
    let second = last_message(cx, &view2).unwrap_or_default();
    assert!(second.contains("미리듣기"), "{second}");
    // 커서 노트 조작 뒤 커서 따라가기(cursor_moved)는 더 알리지 않는다
    let count_before = cx.read(|cx| view.read(cx).messages().len());
    press(cx, handle, "alt-k");
    assert_eq!(
        cx.read(|cx| view.read(cx).messages().len()),
        count_before,
        "조작 뒤 커서 따라가기가 알림을 또 냈다"
    );
}
