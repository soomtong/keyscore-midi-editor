//! S3-T7 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s3-t7/review-1.md`). 픽스처는 `tests/search.rs`를
//! 베꼈다. 실패하는 시험은 구현자가 고칠 때까지 실패한 채로 둔다.

use std::collections::BTreeSet;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{help, mode, note, search};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::search::{Query, QueryError};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const BAR: u64 = 3840;

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-s3t7-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-review-s3t7-{name}", std::process::id()),
        );
    });
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

/// `tests/search.rs`와 같은 트랙 둘(Lead, Bass Line).
fn project() -> Project {
    let mut project = Project::new();
    let mut lead = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    clip.insert_note(note(0, 480, 61).with_lyric("반")).unwrap();
    clip.insert_note(note(960, 480, 60).with_lyric("짝"))
        .unwrap();
    clip.insert_note(note(BAR, 480, 61)).unwrap();
    lead.insert_clip(clip).unwrap();
    project.push_track(lead).unwrap();
    let mut bass = Track::new(
        "Bass Line",
        Channel::new(2).unwrap(),
        TrackColor::new(1).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    clip.insert_note(note(480, 480, 37)).unwrap();
    clip.insert_note(note(BAR + 960, 480, 49).with_lyric("별"))
        .unwrap();
    bass.insert_clip(clip).unwrap();
    project.push_track(bass).unwrap();
    project
}

/// Lead(노트 0@61)와 노트 없는 클립 하나(BAR)만 든 「Pad」. 둘 다 이름에 `a`가 있다.
fn project_with_empty_track() -> Project {
    let mut project = Project::new();
    let mut lead = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, BAR).unwrap();
    clip.insert_note(note(0, 480, 61)).unwrap();
    lead.insert_clip(clip).unwrap();
    project.push_track(lead).unwrap();
    let mut pad = Track::new(
        "Pad",
        Channel::new(2).unwrap(),
        TrackColor::new(1).unwrap(),
    );
    pad.insert_clip(Clip::new(Tick::new(BAR), BAR).unwrap())
        .unwrap();
    project.push_track(pad).unwrap();
    project
}

fn open(
    cx: &mut TestAppContext,
    name: &str,
    preset: Preset,
    project: Project,
    piano_roll: bool,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    if piano_roll {
        press(cx, handle, "enter");
        assert_eq!(
            cx.read(|cx| view.read(cx).mode_state().view()),
            View::PianoRoll
        );
    }
    set_cursor(cx, &view, 0, 72);
    (handle, view)
}

fn track_ids(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<TrackId> {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .project()
            .tracks()
            .iter()
            .map(Track::id)
            .collect()
    })
}

fn current_track(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<TrackId> {
    cx.read(|cx| view.read(cx).current_track())
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

fn last_message(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .last()
            .map(ToString::to_string)
            .unwrap_or_default()
    })
}

fn bar_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).search_bar_open())
}

fn current_mode(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn focus_in_document(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> bool {
    let focus = cx.read(|cx| view.read(cx).document_focus_for_test());
    cx.update_window(handle, |_, window, _| focus.is_focused(window))
        .unwrap()
}

fn lead_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| {
        view.read(cx).document().read(cx).project().tracks()[0]
            .clips()
            .next()
            .unwrap()
            .1
            .notes()
            .len()
    })
}

fn search_for(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    press(cx, handle, "cmd-f");
    press(cx, handle, "cmd-a");
    type_text(cx, handle, text);
    press(cx, handle, "enter");
}

// ---------- 1. 노트 없는 트랙 결과에서 이전 결과 ----------

/// 결정 3절 3(노트 없는 트랙 결과의 음높이는 0)과 4절 1(그 결과로 가면 커서 음높이를 둔다)이 겹치면, 그 결과
/// 위의 커서는 (틱, 트랙, 커서 음높이)라 결과 자리 (틱, 트랙, 0)보다 「뒤」가 된다. 그래서 「이전」이 같은 결과를
/// 다시 골라 커서가 그 자리에 갇힌다.
#[gpui_kit::test]
fn search_prev_from_empty_track_hit_moves_to_earlier_hit(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "emptyprev", Preset::Default, project_with_empty_track(), true);
    let ids = track_ids(cx, &view);
    search_for(cx, handle, "\"a\"");
    assert_eq!(current_track(cx, &view), Some(ids[1]), "다음 결과는 Pad");
    assert_eq!(cursor(cx, &view), (BAR, 72), "노트 없는 트랙: 커서 음높이 그대로");
    press(cx, handle, "cmd-shift-g");
    assert_eq!(
        (current_track(cx, &view), cursor(cx, &view)),
        (Some(ids[0]), (0, 61)),
        "이전 결과는 Lead@0이어야 한다. 메시지: {}",
        last_message(cx, &view)
    );
}

/// 같은 원인: 노트 없는 트랙 결과가 유일한 결과일 때 「이전」은 돎 문구를 남겨야 한다(결정 3절 5).
#[gpui_kit::test]
fn search_prev_on_sole_empty_track_hit_says_wrapped(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "emptysole", Preset::Default, project_with_empty_track(), true);
    search_for(cx, handle, "pad");
    assert_eq!(cursor(cx, &view), (BAR, 72));
    press(cx, handle, "cmd-shift-g");
    assert_eq!(
        last_message(cx, &view),
        "검색 pad: 1/1 · 처음에서 끝으로 돌아왔다"
    );
}

// ---------- 2. vim 오퍼레이터 대기 중의 n ----------

/// 결정 4절 4: 오퍼레이터 대기 중에 검색 이동이 오면 대기를 끝내고 이동만 한다.
#[gpui_kit::test]
fn vim_n_while_operator_pending_ends_wait_and_moves(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "opn", Preset::Vim, project(), true);
    set_cursor(cx, &view, 0, 60);
    search_for(cx, handle, "c#4");
    assert_eq!(cursor(cx, &view), (0, 61));
    let notes = lead_notes(cx, &view);
    press(cx, handle, "d");
    assert!(cx.read(|cx| view.read(cx).pending_operator().is_some()));
    press(cx, handle, "n");
    assert!(cx.read(|cx| view.read(cx).pending_operator().is_none()));
    assert_eq!(lead_notes(cx, &view), notes, "d n 은 지우지 않는다");
    assert_eq!(
        cursor(cx, &view),
        (BAR, 61),
        "대기를 끝내고 이동한다. 메시지: {}",
        last_message(cx, &view)
    );
}

/// 같은 규칙, `cmd-g`(vim도 물려받는 키).
#[gpui_kit::test]
fn vim_cmd_g_while_operator_pending_ends_wait_and_moves(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "opcmdg", Preset::Vim, project(), true);
    set_cursor(cx, &view, 0, 60);
    search_for(cx, handle, "c#4");
    press(cx, handle, "d");
    press(cx, handle, "cmd-g");
    assert!(cx.read(|cx| view.read(cx).pending_operator().is_none()));
    assert_eq!(cursor(cx, &view), (BAR, 61));
}

// ---------- 3. 막대가 열린 채 모드가 바뀜 ----------

/// 막대는 Normal에서만 열리지만 전역 `ctrl-tab`(모드 순환)은 막대 입력란에서도 닿는다. Step으로 바뀐 뒤 막대의
/// `Enter`가 Step 커서(입력 위치)를 옮기면 결정 1절 2를 어긴다.
#[gpui_kit::test]
fn bar_enter_after_mode_cycle_does_not_move_step_cursor(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "modecycle", Preset::Default, project(), true);
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "ctrl-tab");
    if current_mode(cx, &view) == Mode::Normal {
        // 막대 안에서 모드가 바뀌지 않으면 깨뜨리지 못한 것이다
        return;
    }
    let before = cursor(cx, &view);
    press(cx, handle, "enter");
    assert_eq!(
        cursor(cx, &view),
        before,
        "Normal이 아닌 모드({:?})에서 검색이 커서를 옮겼다. 막대 열림 {}",
        current_mode(cx, &view),
        bar_open(cx, &view)
    );
}

// ---------- 4. 배치 뷰 ----------

#[gpui_kit::test]
fn search_in_arrangement_keeps_view_and_switches_track(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "arrange", Preset::Default, project(), false);
    let ids = track_ids(cx, &view);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    search_for(cx, handle, "별");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    assert_eq!(current_track(cx, &view), Some(ids[1]));
    assert_eq!(cursor(cx, &view), (BAR + 960, 49));
    assert!(focus_in_document(cx, handle, &view));
}

// ---------- 5. vim 비주얼 중 n ----------

#[gpui_kit::test]
fn vim_visual_n_recomputes_selection_to_new_cursor(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "visual", Preset::Vim, project(), true);
    set_cursor(cx, &view, 0, 60);
    search_for(cx, handle, "c#4");
    assert_eq!(cursor(cx, &view), (0, 61));
    press(cx, handle, "v");
    press(cx, handle, "n");
    assert_eq!(cursor(cx, &view), (BAR, 61));
    let starts: BTreeSet<u64> = cx.read(|cx| {
        view.read(cx)
            .piano_roll()
            .selection()
            .notes()
            .map(|n| n.start().get())
            .collect()
    });
    assert!(
        starts.contains(&0) && starts.contains(&BAR),
        "비주얼 선택이 새 커서까지 넓어진다: {starts:?}"
    );
}

// ---------- 6. 검색어 문법 경계 ----------

#[test]
fn parse_edge_forms_follow_decision_grammar() {
    let pitch = |v: u8| Ok(Query::Pitch(Pitch::new(v).unwrap()));
    assert_eq!(Query::parse("cb-1"), Err(QueryError::PitchOutOfRange));
    assert_eq!(Query::parse("B#9"), Err(QueryError::PitchOutOfRange));
    assert_eq!(Query::parse("b#-1"), pitch(12));
    assert_eq!(Query::parse("G9"), pitch(127));
    assert_eq!(Query::parse("  \tC-1\u{3000}"), pitch(0));
    assert_eq!(Query::parse("bb"), Ok(Query::PitchClass(10)));
    // 음높이 꼴이 아닌 것은 글자다(공황 없이)
    for text in ["c##4", "c#10", "c4x", "c가", "가", "é", "c#٤", "h", "-1", "\"", "\"a"] {
        assert!(
            matches!(Query::parse(text), Ok(Query::Text(_))),
            "{text:?} → {:?}",
            Query::parse(text)
        );
    }
    assert_eq!(Query::parse("\"\""), Err(QueryError::Empty));
    assert_eq!(Query::parse("\"C#4\""), Ok(Query::Text("c#4".to_owned())));
}

// ---------- 7. 팝업 하나와 포커스 ----------

#[gpui_kit::test]
fn bar_and_keymap_overlay_close_each_other(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "overlay", Preset::Default, project(), true);
    press(cx, handle, "cmd-f");
    run(cx, handle, help::KeymapOverlay);
    assert!(!bar_open(cx, &view));
    assert!(cx.read(|cx| view.read(cx).overlay_lines(cx).is_some()));
    run(cx, handle, search::Open);
    assert!(bar_open(cx, &view));
    assert!(cx.read(|cx| view.read(cx).overlay_lines(cx).is_none()));
    press(cx, handle, "escape");
    assert!(!bar_open(cx, &view));
    assert!(focus_in_document(cx, handle, &view));
}

/// 결정 1절 7: 창이 활성인 동안 포커스가 막대 밖으로 나가면 닫는다.
#[gpui_kit::test]
fn bar_closes_when_focus_moves_to_document(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "focusout", Preset::Default, project(), true);
    press(cx, handle, "cmd-f");
    assert!(bar_open(cx, &view));
    let focus = cx.read(|cx| view.read(cx).document_focus_for_test());
    cx.update_window(handle, |_, window, cx| window.focus(&focus, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(!bar_open(cx, &view));
}

/// 결정 6절 5: 정보 팝업은 모든 모드에서 연다(팔레트·메뉴 경로). Step에서 닫으면 포커스는 문서로 돌아온다.
#[gpui_kit::test]
fn note_info_opens_in_step_mode_and_returns_focus(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "infostep", Preset::Default, project(), true);
    run(cx, handle, mode::Step);
    assert_eq!(current_mode(cx, &view), Mode::Step);
    set_cursor(cx, &view, 0, 61);
    run(cx, handle, note::ShowInfo);
    assert!(cx.read(|cx| view.read(cx).note_info().is_some()));
    press(cx, handle, "escape");
    assert!(cx.read(|cx| view.read(cx).note_info().is_none()));
    assert_eq!(current_mode(cx, &view), Mode::Step, "팝업의 Esc는 모드를 바꾸지 않는다");
    assert!(focus_in_document(cx, handle, &view));
}

// ---------- 8. 1번의 순수 계산판(GPUI 없이 원인을 보인다) ----------

#[test]
fn step_prev_from_empty_track_hit_position_picks_earlier_hit() {
    use keyscore_app::search::{Direction, Position, find_hits, step};
    let project = project_with_empty_track();
    let hits = find_hits(&project, &Query::parse("\"a\"").unwrap());
    assert_eq!(hits.len(), 2);
    // 창이 Pad 결과로 옮긴 뒤의 자리: 노트가 없어 커서 음높이 72를 그대로 둔다(결정 4절 1)
    let at = Position {
        track_index: 1,
        tick: Tick::new(BAR),
        pitch: Pitch::new(72).unwrap(),
    };
    let found = step(&project, &hits, at, Direction::Backward).unwrap();
    assert_eq!(
        hits[found.index].tick,
        Tick::ZERO,
        "이전은 Lead@0이어야 한다: {found:?} {:?}",
        hits[found.index]
    );
}

#[test]
fn step_on_sole_empty_track_hit_wraps_both_ways() {
    use keyscore_app::search::{Direction, Position, find_hits, step};
    let project = project_with_empty_track();
    let hits = find_hits(&project, &Query::parse("pad").unwrap());
    assert_eq!(hits.len(), 1);
    let at = Position {
        track_index: 1,
        tick: Tick::new(BAR),
        pitch: Pitch::new(72).unwrap(),
    };
    let back = step(&project, &hits, at, Direction::Backward).unwrap();
    let fwd = step(&project, &hits, at, Direction::Forward).unwrap();
    assert_eq!(
        (back.wrapped, fwd.wrapped),
        (true, true),
        "유일한 결과 위에서는 앞뒤 모두 돎 문구(결정 3절 5)"
    );
}
