//! S3-T2 적대적 리뷰 1회차의 깨뜨리기 시도(`docs/reports/s3-t2/review-1.md` 3절). 도우미는
//! `pianoroll_select.rs`에서 복사했다. 통과한 시도도 「못 깨뜨림」으로 남기기 위해 모두 둔다.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, track};
use keyscore_app::commands::ex::{self, ExCommand, ExError};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::select::next_note;
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

// ---------- 도우미(pianoroll_select.rs 복사) ----------

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rev32-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: impl Into<gpui_kit::ElementId>) {
    let id = id.into();
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
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

fn second_track() -> Track {
    let mut track = Track::new("U", Channel::new(2).unwrap(), TrackColor::new(1).unwrap());
    track
        .insert_clip(clip(0, 4 * BAR, &[note(0, 960, 48)]))
        .unwrap();
    track
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

/// 트랙 `index`의 첫 클립 노트.
fn notes_of(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    index: usize,
) -> Vec<(u64, u32, u8)> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[index];
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

fn clip_a_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u32, u8)> {
    notes_of(cx, view, 0)
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
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

// ---------- 시도 1. 선택의 클립과 커서의 클립이 다를 때 Alt 조작(REQ 5.3) ----------

/// 클립 A에서 `shift-l`로 D4를 고른 뒤 `shift-n`으로 클립 B에 들어가 `alt-k`. 5.3은 「앞뒤 클립의 노트는
/// 흐리게 보이고 편집은 되지 않는다」이므로 A의 D4는 그대로여야 한다.
#[gpui_kit::test]
fn brk_alt_op_with_selection_in_other_clip_edits_dimmed_notes(cx: &mut TestAppContext) {
    start(cx, "other-clip");
    cx.update(|cx| {
        keyscore_app::app_state::AppSettings::update(cx, |s| s.preview.on_move = false);
    });
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "shift-l");
    assert_eq!(selection_of(cx, &view), vec![D4]);
    press(cx, handle, "shift-n");
    assert_eq!(cursor_of(cx, &view), (4 * BAR, 62), "커서는 클립 B");
    // 현재 클립은 B다. 선택은 A를 가리킨 채 남아 있다.
    assert!(has_selection(cx, &view));
    press(cx, handle, "alt-k");
    let notes = clip_a_notes(cx, &view);
    assert!(
        notes.contains(&D4),
        "흐리게 보이는 클립 A의 노트가 바뀌었다(5.3): {notes:?}, 선택 {:?}",
        selection_of(cx, &view)
    );
}

/// 같은 구조로 트랙을 바꾼 뒤(`cmd-down`) `alt-k`: 보이지 않는 트랙 0의 선택이 바뀌는가.
#[gpui_kit::test]
fn brk_alt_op_with_selection_on_other_track_edits_invisible_notes(cx: &mut TestAppContext) {
    start(cx, "other-track");
    cx.update(|cx| {
        keyscore_app::app_state::AppSettings::update(cx, |s| s.preview.on_move = false);
    });
    let mut project = fixture();
    project.push_track(second_track()).unwrap();
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "shift-l");
    assert_eq!(selection_of(cx, &view), vec![D4]);
    press(cx, handle, "cmd-down");
    let current = cx.read(|cx| view.read(cx).current_track());
    let second = cx.read(|cx| view.read(cx).document().read(cx).project().tracks()[1].id());
    assert_eq!(current, Some(second), "현재 트랙은 U");
    press(cx, handle, "alt-k");
    let notes = notes_of(cx, &view, 0);
    assert!(
        notes.contains(&D4),
        "보이지 않는 트랙 T의 노트가 바뀌었다: {notes:?}"
    );
    assert_eq!(
        notes_of(cx, &view, 1),
        vec![(0, 960, 48)],
        "현재 트랙 U는 그대로"
    );
}

// ---------- 시도 2. 기준 칸이 클립 사이 빈 틈일 때 Shift+이동 ----------

/// 클립 A(0–1마디), 클립 B(2마디–3마디, 시작에 C4). 틈(1마디 + 2박)에서 `shift-l` 세 번 → 커서는 B 안
/// (2마디 + 1박). 직사각형 `[5760, 9600) × 60`에 B의 C4가 든다. 7.4 러버밴드와 같은 기준이면 골라야 한다.
#[gpui_kit::test]
fn brk_shift_extend_started_in_gap_never_selects(cx: &mut TestAppContext) {
    start(cx, "gap");
    cx.update(|cx| {
        keyscore_app::app_state::AppSettings::update(cx, |s| s.preview.on_move = false);
    });
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
    assert_eq!(
        selection_of(cx, &view),
        vec![(0, 960, 60)],
        "틈에서 시작한 Shift+이동은 클립 B에 들어가도 아무것도 고르지 않는다"
    );
}

// ---------- 시도 3. 선택 없이 커서 노트 줄이기 연타 ----------

/// 노트 `[0, 3840)`, 커서 3000. `alt-shift-h` 한 번 → `[0, 2880)`. 커서(3000)는 이제 노트 밖이다. 5.7의
/// 「같은 조작을 연속으로」가 성립하려면 두 번째 `alt-shift-h`도 같은 노트를 줄여야 한다.
#[gpui_kit::test]
fn brk_shorten_repeat_on_cursor_note_fails_when_cursor_falls_off(cx: &mut TestAppContext) {
    start(cx, "shorten");
    cx.update(|cx| {
        keyscore_app::app_state::AppSettings::update(cx, |s| s.preview.on_move = false);
    });
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track
        .insert_clip(clip(0, 4 * BAR, &[note(0, 3840, 60)]))
        .unwrap();
    project.push_track(track).unwrap();
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 3000, 60);
    press(cx, handle, "alt-shift-h");
    assert_eq!(clip_a_notes(cx, &view), vec![(0, 2880, 60)]);
    press(cx, handle, "alt-shift-h");
    assert_eq!(
        clip_a_notes(cx, &view),
        vec![(0, 1920, 60)],
        "두 번째 줄이기가 적용되지 않았다. 커서 {:?}, 알림 {:?}",
        cursor_of(cx, &view),
        last_message(cx, &view)
    );
}

// ---------- 시도 4. Shift+p가 클립 중간에서 가는 곳 ----------

/// 커서가 클립 B 중간. REQ 5.8 「`Shift+p` 이전 클립으로」면 A의 시작(0)이어야 한다.
#[gpui_kit::test]
fn brk_shift_p_from_clip_middle_goes_to_current_clip_start(cx: &mut TestAppContext) {
    start(cx, "shift-p");
    cx.update(|cx| {
        keyscore_app::app_state::AppSettings::update(cx, |s| s.preview.on_move = false);
    });
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 4 * BAR + 480, 60);
    press(cx, handle, "shift-p");
    assert_eq!(
        cursor_of(cx, &view),
        (0, 60),
        "클립 B 중간에서 Shift+p는 「이전 클립」 A가 아니라 B의 시작으로 간다"
    );
}

// ---------- 시도 5. 기준 칸이 낡았을 때 Shift+이동이 선택을 줄인다 ----------

/// `shift-l`로 D4 선택 → `alt-k`(D4 → 63, 선택은 바뀐 노트) → `shift-l`로 더 넓히면 기준 칸(음높이 62)
/// 직사각형에 63 노트가 들지 않아 선택이 사라진다. 「넓히기」가 선택을 비우면 안 된다.
#[gpui_kit::test]
fn brk_extend_after_alt_op_drops_moved_note_from_selection(cx: &mut TestAppContext) {
    start(cx, "stale-anchor");
    cx.update(|cx| {
        keyscore_app::app_state::AppSettings::update(cx, |s| s.preview.on_move = false);
    });
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "shift-l");
    assert_eq!(selection_of(cx, &view), vec![D4]);
    press(cx, handle, "alt-k");
    assert_eq!(selection_of(cx, &view), vec![(960, 960, 63)]);
    press(cx, handle, "shift-l");
    assert_eq!(cursor_of(cx, &view), (2880, 62));
    assert!(
        selection_of(cx, &view).contains(&(960, 960, 63)),
        "넓히기가 방금 옮긴 노트를 선택에서 떨어뜨렸다: {:?}",
        selection_of(cx, &view)
    );
}

// ---------- 시도 6. 입력란의 Cmd+A ----------

/// 템포 입력란에 포커스가 있을 때 `cmd-a`는 글자 전체 선택이어야 하고 피아노롤 선택은 생기지 않아야 한다.
#[gpui_kit::test]
fn brk_cmd_a_in_tempo_field_selects_text_not_notes(cx: &mut TestAppContext) {
    start(cx, "cmd-a-field");
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a");
    assert!(
        selection_of(cx, &view).is_empty(),
        "입력란의 cmd-a가 노트를 골랐다"
    );
    assert!(!has_selection(cx, &view));
    press(cx, handle, "backspace");
    let shown = cx
        .update_window(handle, |_, window, _| {
            window.find("transport-tempo").value().map(str::to_owned)
        })
        .unwrap();
    assert_eq!(
        shown.as_deref(),
        Some(""),
        "cmd-a가 글자 전체를 고르지 않았다(backspace 뒤 남은 글자)"
    );
    press(cx, handle, "escape");
}

// ---------- 시도 7. 사용자 키맵이 f20을 걸면 시퀀스 대기 중 Esc가 그 Action을 실행한다 ----------

#[gpui_kit::test]
fn brk_user_keymap_f20_runs_on_escape_during_pending_sequence(cx: &mut TestAppContext) {
    start(cx, "f20");
    let path = cx.update(|cx| keymap::user_path(cx));
    std::fs::write(
        &path,
        r#"[{"context": "Normal", "bindings": {"f20": "view::Toggle"}}]"#,
    )
    .unwrap();
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    // 수정(리뷰 1 지적 5): f20은 앱이 예약한 키라 사용자 키맵이 걸면 KM-6 오류로 거부된다
    let errors = cx.update(|cx| keymap::errors(cx).to_vec());
    assert!(
        errors.iter().any(|e| keymap::describe(e).contains("f20")),
        "{errors:?}"
    );
    let (handle, view) = open(cx, fixture());
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    cx.simulate_keystrokes(handle, "g");
    press(cx, handle, "escape");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll,
        "Esc가 사용자의 f20 바인딩(view::Toggle)을 실행했다"
    );
}

// ---------- 시도 8. 선택의 트랙이 지워지면 선택과 FSM이 비는가 ----------

#[gpui_kit::test]
fn brk_track_delete_clears_selection_pointing_to_it(cx: &mut TestAppContext) {
    start(cx, "track-delete");
    let mut project = fixture();
    project.push_track(second_track()).unwrap();
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 960, 62);
    select(cx, &view, &[D4]);
    assert!(has_selection(cx, &view));
    run(cx, handle, track::Delete);
    assert_eq!(
        cx.read(|cx| view.read(cx).document().read(cx).project().tracks().len()),
        1
    );
    assert!(
        selection_of(cx, &view).is_empty(),
        "지운 트랙의 선택이 남았다"
    );
    assert!(
        !has_selection(cx, &view),
        "FSM이 Selection 컨텍스트에 남았다"
    );
    press(cx, handle, "escape");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
}

// ---------- 시도 9. 메뉴·팔레트 경로의 undo 뒤 같은 조작 ----------

/// `alt-shift-l` ×2, `l`, `alt-shift-l` ×2 → undo 2단위. 키 입력이 아닌 `edit::Undo`(메뉴 경로)로 하나를
/// 되돌린 뒤 `alt-shift-l`: 맨 위 단위가 같은 키인데 `seal`을 거치지 않았다. core가 undo에서 스스로 봉인하지
/// 않으면 첫 묶음에 붙어 undo 한 번에 세 번이 풀린다.
#[gpui_kit::test]
fn brk_undo_via_menu_then_same_op_starts_new_unit(cx: &mut TestAppContext) {
    start(cx, "undo-menu");
    cx.update(|cx| {
        keyscore_app::app_state::AppSettings::update(cx, |s| s.preview.on_move = false);
    });
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    select(cx, &view, &[(7680, 1920, 69)]);
    press(cx, handle, "alt-shift-l alt-shift-l");
    press(cx, handle, "l");
    press(cx, handle, "alt-shift-l alt-shift-l");
    assert_eq!(undo_len(cx, &view), 2);
    run(cx, handle, edit::Undo);
    assert_eq!(undo_len(cx, &view), 1);
    assert!(clip_a_notes(cx, &view).contains(&(7680, 1920 + 2 * 960, 69)));
    // undo 뒤 선택은 되살아난 노트다(S3-T3a, S3-T2 리뷰 1 제안 8). 묶음 질문만 보기 위해 키 입력 없이
    // 다시 고른다(`set_selection`은 seal을 거치지 않는다).
    assert_eq!(
        selection_of(cx, &view),
        vec![(7680, 1920 + 2 * 960, 69)],
        "undo 뒤 선택은 되살아난 노트"
    );
    select(cx, &view, &[(7680, 1920 + 2 * 960, 69)]);
    press(cx, handle, "alt-shift-l");
    assert_eq!(undo_len(cx, &view), 2, "undo 뒤의 같은 조작은 새 단위다");
    press(cx, handle, "cmd-z");
    assert!(
        clip_a_notes(cx, &view).contains(&(7680, 1920 + 2 * 960, 69)),
        "{:?}",
        clip_a_notes(cx, &view)
    );
}

// ---------- 시도 10. ex 파서 가장자리 ----------

#[test]
fn brk_ex_parse_edge_cases() {
    assert_eq!(ex::parse(": w"), Ok(ExCommand::Write(None)), "접두 뒤 공백");
    assert_eq!(
        ex::parse(":w a b.ksq"),
        Ok(ExCommand::Write(Some("a b.ksq".to_owned()))),
        "공백 포함 이름"
    );
    assert_eq!(
        ex::parse(":e ~/x.ksq"),
        Ok(ExCommand::Edit(Some("~/x.ksq".to_owned()))),
        "틸드는 파서가 풀지 않는다(열기에서 그대로 경로가 된다)"
    );
    assert!(matches!(
        ex::parse(":set Input"),
        Err(ExError::UnknownSection(_))
    ));
    assert!(matches!(ex::parse(":W"), Err(ExError::Unknown(_))));
    assert!(matches!(ex::parse(":q!"), Err(ExError::Unknown(_))));
    assert!(matches!(
        ex::parse(":q x"),
        Err(ExError::UnexpectedArgument(_))
    ));
    assert!(matches!(ex::parse("::"), Err(ExError::Unknown(_))));
    assert!(matches!(ex::parse(":"), Err(ExError::Empty)));
    assert!(matches!(ex::parse("w"), Err(ExError::NotCommandLine)));
    assert!(ex::is_command_line("  :w"));
    assert!(!ex::is_command_line("w:"));
}

// ---------- 시도 11. n이 꼬리 노트를 지나고 다른 트랙은 보지 않는가(순수) ----------

#[test]
fn brk_next_note_walks_tail_note_and_only_this_track() {
    // 꼬리 노트는 분할로만 생긴다: 2마디 클립을 1마디에서 가르면 분할점을 가로지르는 E4가 왼쩍 클립의 꼬리다.
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track
        .insert_clip(clip(
            0,
            2 * BAR,
            &[
                note(0, 480, 60),
                note(BAR - 480, 1920, 64),
                note(BAR + 960, 960, 67),
            ],
        ))
        .unwrap();
    project.push_track(track).unwrap();
    let track_id = project.tracks()[0].id();
    let clip_id = project.tracks()[0].clips().next().unwrap().0;
    let mut document = keyscore_core::Document::new(project, usize::MAX).unwrap();
    document
        .apply(
            vec![keyscore_core::Edit::SplitClip {
                track: track_id,
                id: clip_id,
                at: Tick::new(BAR),
            }],
            None,
        )
        .unwrap();
    let track = &document.project().tracks()[0];
    let left = track.clips().next().unwrap().1;
    assert_eq!(left.length(), BAR);
    assert!(
        left.notes().iter().any(|n| n.end() > left.length()),
        "꼬리가 생겼다"
    );
    let step =
        |t: u64, p: u8| next_note(track, at(t, p)).map(|(_, c)| (c.tick.get(), c.pitch.get()));
    assert_eq!(step(0, 60), Some((BAR - 480, 64)), "꼬리 노트도 지난다");
    assert_eq!(step(BAR - 480, 64), Some((BAR + 960, 67)), "다음 클립으로");
    assert_eq!(step(BAR + 960, 67), None);
}

// ---------- 시도 12. 배치 뷰의 Cmd+A ----------

#[gpui_kit::test]
fn brk_cmd_a_in_arrangement_view_does_nothing(cx: &mut TestAppContext) {
    start(cx, "cmd-a-arr");
    let (handle, view) = open(cx, fixture());
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    press(cx, handle, "cmd-a");
    assert!(selection_of(cx, &view).is_empty());
    assert!(!has_selection(cx, &view));
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().context_path()),
        &["Normal", "Arrangement"]
    );
}
