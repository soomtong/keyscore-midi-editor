//! S3-T7 검색 막대와 결과 이동(REQ 5.8 `Cmd+F`, 5.8.1 `/`·`n`·`N`). 헤드리스 창에서 키를 눌러 막대·커서를
//! 본다. 규칙은 결정 `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md` 1–5절.

use std::collections::BTreeSet;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, mode, note, palette, search};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const BAR: u64 = 3840;

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-search-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-search-{name}", std::process::id()),
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

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
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

/// 트랙 둘. Lead: c#4 두 개(0, BAR)와 가사, Bass Line: c#2(480), c#3(BAR+960, 가사 「별」).
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

fn setup_with(
    cx: &mut TestAppContext,
    name: &str,
    preset: Preset,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    set_cursor(cx, &view, 0, 72);
    (handle, view)
}

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    setup_with(cx, name, Preset::Default)
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

fn bar_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).search_bar_open())
}

fn status(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<String> {
    cx.read(|cx| view.read(cx).search_status())
}

fn last_search(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<String> {
    cx.read(|cx| view.read(cx).last_search())
}

/// 포커스가 막대 입력란(`SearchBar > Input`)에 있는가.
fn focus_in_bar(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| {
        let stack = window.context_stack();
        stack.iter().any(|c| c.contains("SearchBar")) && stack.iter().any(|c| c.contains("Input"))
    })
    .unwrap()
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

// ---------- 10–12. 막대 ----------

#[gpui_kit::test]
fn cmd_f_opens_bar_and_enter_jumps_to_next_pitch(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "open");
    let ids = track_ids(cx, &view);
    press(cx, handle, "cmd-f");
    assert!(bar_open(cx, &view));
    assert!(focus_in_bar(cx, handle), "포커스가 막대 입력란에 있다");
    assert_eq!(status(cx, &view).as_deref(), Some(""));
    type_text(cx, handle, "c#");
    assert_eq!(status(cx, &view).as_deref(), Some("결과 4개"));
    type_text(cx, handle, "4");
    assert_eq!(status(cx, &view).as_deref(), Some("결과 2개"));
    assert_eq!(
        cursor(cx, &view),
        (0, 72),
        "치는 동안 커서는 움직이지 않는다"
    );
    // 커서 (0, 72)는 Lead@0 c#4(61)보다 뒤라 다음은 Lead@BAR
    press(cx, handle, "enter");
    assert!(!bar_open(cx, &view));
    assert!(focus_in_document(cx, handle, &view));
    assert_eq!(cursor(cx, &view), (BAR, 61));
    assert_eq!(current_track(cx, &view), Some(ids[0]));
    assert_eq!(last_message(cx, &view), "검색 c#4: 2/2");
    assert_eq!(last_search(cx, &view).as_deref(), Some("c#4"));

    // 다른 글: 범위 밖, 결과 없음
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "b9");
    assert_eq!(status(cx, &view).as_deref(), Some("음높이 범위 밖"));
    press(cx, handle, "cmd-a");
    type_text(cx, handle, "zzz");
    assert_eq!(status(cx, &view).as_deref(), Some("결과 없음"));
    press(cx, handle, "enter");
    assert_eq!(last_message(cx, &view), "검색 zzz: 결과 없음");
    assert_eq!(cursor(cx, &view), (BAR, 61), "결과가 없으면 커서는 그대로");
}

#[gpui_kit::test]
fn shift_enter_jumps_backward_and_wraps(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "back");
    let ids = track_ids(cx, &view);
    set_cursor(cx, &view, 0, 61);
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#");
    // Lead@0(61)에서 이전은 없으므로 끝(Bass@BAR+960, 49)으로 돈다
    press(cx, handle, "shift-enter");
    assert!(!bar_open(cx, &view));
    assert_eq!(cursor(cx, &view), (BAR + 960, 49));
    assert_eq!(current_track(cx, &view), Some(ids[1]));
    assert_eq!(
        last_message(cx, &view),
        "검색 c#: 4/4 · 처음에서 끝으로 돌아왔다"
    );
    // 앞으로 가면 끝에서 처음으로
    press(cx, handle, "cmd-g");
    assert_eq!(cursor(cx, &view), (0, 61));
    assert_eq!(current_track(cx, &view), Some(ids[0]));
    assert_eq!(
        last_message(cx, &view),
        "검색 c#: 1/4 · 끝에서 처음으로 돌아왔다"
    );
}

#[gpui_kit::test]
fn escape_closes_without_changing_last_search(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "escape");
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    let after = cursor(cx, &view);
    press(cx, handle, "cmd-f");
    press(cx, handle, "cmd-a");
    type_text(cx, handle, "bass");
    press(cx, handle, "escape");
    assert!(!bar_open(cx, &view));
    assert!(focus_in_document(cx, handle, &view));
    assert_eq!(last_search(cx, &view).as_deref(), Some("c#4"));
    assert_eq!(cursor(cx, &view), after);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        Mode::Normal,
        "막대의 Esc는 막대만 닫는다"
    );
}

// ---------- 13–14. 막대 밖의 다음·이전 ----------

#[gpui_kit::test]
fn cmd_g_repeats_last_search_in_default_preset(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmdg");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    assert_eq!(cursor(cx, &view), (0, 61));
    press(cx, handle, "cmd-g");
    assert_eq!(cursor(cx, &view), (BAR, 61));
    press(cx, handle, "cmd-shift-g");
    assert_eq!(cursor(cx, &view), (0, 61));
    // 기본 프리셋의 n은 여전히 다음 노트
    press(cx, handle, "n");
    assert_eq!(cursor(cx, &view).0, 960, "n은 다음 노트(짝, 960)");
}

#[gpui_kit::test]
fn vim_slash_and_n_navigate(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "vim", Preset::Vim);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "/");
    assert!(bar_open(cx, &view));
    assert!(focus_in_bar(cx, handle));
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    assert_eq!(cursor(cx, &view), (0, 61));
    press(cx, handle, "n");
    assert_eq!(cursor(cx, &view), (BAR, 61));
    press(cx, handle, "shift-n");
    assert_eq!(cursor(cx, &view), (0, 61));
    press(cx, handle, "cmd-g");
    assert_eq!(cursor(cx, &view), (BAR, 61), "vim도 cmd-g를 물려받는다");
}

// ---------- 15–17 ----------

#[gpui_kit::test]
fn search_jumps_to_other_track_by_name_and_lyric(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "names");
    let ids = track_ids(cx, &view);
    assert_eq!(current_track(cx, &view), Some(ids[0]));
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "LINE");
    assert_eq!(status(cx, &view).as_deref(), Some("결과 1개"));
    press(cx, handle, "enter");
    assert_eq!(current_track(cx, &view), Some(ids[1]), "트랙 이름 결과");
    assert_eq!(cursor(cx, &view), (480, 37), "트랙의 첫 노트");
    press(cx, handle, "cmd-f");
    press(cx, handle, "cmd-a");
    type_text(cx, handle, "짝");
    press(cx, handle, "enter");
    assert_eq!(current_track(cx, &view), Some(ids[0]), "가사 결과");
    assert_eq!(cursor(cx, &view), (960, 60));
    // 따옴표로 감싸면 음높이 꼴도 글자: "a"는 트랙 이름 Bass Line·Lead
    press(cx, handle, "cmd-f");
    press(cx, handle, "cmd-a");
    type_text(cx, handle, "\"a\"");
    assert_eq!(status(cx, &view).as_deref(), Some("결과 2개"));
}

#[gpui_kit::test]
fn reopen_prefills_last_query(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "prefill");
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    assert_eq!(cursor(cx, &view), (BAR, 61));
    press(cx, handle, "cmd-f");
    assert_eq!(status(cx, &view).as_deref(), Some("결과 2개"));
    // 글자는 전부 골라져 있다: 바로 Enter면 같은 검색
    press(cx, handle, "enter");
    assert_eq!(cursor(cx, &view), (0, 61));
    // 치면 새 검색어로 바뀐다
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "별");
    press(cx, handle, "enter");
    assert_eq!(last_search(cx, &view).as_deref(), Some("별"));
    assert_eq!(cursor(cx, &view), (BAR + 960, 49));
}

#[gpui_kit::test]
fn next_without_query_says_so(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "noquery");
    press(cx, handle, "cmd-g");
    assert_eq!(last_message(cx, &view), "검색어가 없다");
    assert_eq!(cursor(cx, &view), (0, 72));
    // 빈 검색어의 Enter는 닫기만 한다
    press(cx, handle, "cmd-f");
    press(cx, handle, "enter");
    assert!(!bar_open(cx, &view));
    assert_eq!(last_search(cx, &view), None);
    assert_eq!(cursor(cx, &view), (0, 72));
}

// ---------- 18–22 ----------

#[gpui_kit::test]
fn search_only_in_normal_mode(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "mode");
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    let before = cursor(cx, &view);
    run(cx, handle, mode::Step);
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    let step_cursor = cursor(cx, &view);
    press(cx, handle, "cmd-f");
    assert!(!bar_open(cx, &view));
    assert_eq!(last_message(cx, &view), "검색은 Normal 모드에서 한다");
    run(cx, handle, search::Next);
    assert_eq!(cursor(cx, &view), step_cursor);
    assert_eq!(last_message(cx, &view), "검색은 Normal 모드에서 한다");
    let _ = before;
}

#[gpui_kit::test]
fn document_keys_do_not_reach_document_while_bar_open(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "keys");
    set_cursor(cx, &view, 0, 61);
    let notes = lead_notes(cx, &view);
    let undo = undo_len(cx, &view);
    press(cx, handle, "cmd-f");
    press(cx, handle, "x l j");
    assert!(bar_open(cx, &view));
    assert_eq!(lead_notes(cx, &view), notes);
    assert_eq!(cursor(cx, &view), (0, 61));
    assert_eq!(undo_len(cx, &view), undo);
    // 글자는 입력란으로 갔다
    assert_eq!(status(cx, &view).as_deref(), Some("결과 없음"));
}

#[gpui_kit::test]
fn bar_buttons_navigate_and_close(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "buttons");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    click(cx, handle, "search-next");
    assert!(bar_open(cx, &view), "버튼은 막대를 두고 이동한다");
    assert_eq!(cursor(cx, &view), (0, 61));
    assert_eq!(last_search(cx, &view).as_deref(), Some("c#4"));
    click(cx, handle, "search-next");
    assert_eq!(cursor(cx, &view), (BAR, 61));
    click(cx, handle, "search-prev");
    assert_eq!(cursor(cx, &view), (0, 61));
    assert!(bar_open(cx, &view));
    click(cx, handle, "search-close");
    assert!(!bar_open(cx, &view));
    assert!(focus_in_document(cx, handle, &view));
}

#[gpui_kit::test]
fn bar_and_other_popups_close_each_other(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "popups");
    press(cx, handle, "cmd-f");
    run(cx, handle, palette::Open);
    assert!(!bar_open(cx, &view));
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    press(cx, handle, "escape");
    press(cx, handle, "cmd-f");
    assert!(bar_open(cx, &view));
    run(cx, handle, edit::QuantizeSettings);
    assert!(!bar_open(cx, &view));
    assert!(cx.read(|cx| view.read(cx).quantize_sheet().is_some()));
    run(cx, handle, search::Open);
    assert!(bar_open(cx, &view));
    assert!(cx.read(|cx| view.read(cx).quantize_sheet().is_none()));
    // 정보 팝업을 열면 막대가 닫히고, 막대를 열면 정보 팝업이 닫힌다
    set_cursor(cx, &view, 0, 61);
    run(cx, handle, note::ShowInfo);
    assert!(!bar_open(cx, &view));
    assert!(cx.read(|cx| view.read(cx).note_info().is_some()));
    run(cx, handle, search::Open);
    assert!(bar_open(cx, &view));
    assert!(cx.read(|cx| view.read(cx).note_info().is_none()));
}

#[gpui_kit::test]
fn search_does_not_touch_undo_or_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "undo");
    let (track, clip, notes) = cx.read(|cx| {
        let t = &view.read(cx).document().read(cx).project().tracks()[0];
        let (clip_id, clip) = t.clips().next().unwrap();
        let notes: BTreeSet<Note> = clip.notes().iter().take(1).cloned().collect();
        (t.id(), clip_id, notes)
    });
    view.update(cx, |this, cx| {
        let mut sel = Selection::default();
        sel.set(track, clip, notes.clone());
        this.set_selection(sel, cx);
    });
    let undo = undo_len(cx, &view);
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    press(cx, handle, "cmd-g");
    assert_eq!(undo_len(cx, &view), undo);
    let selected: BTreeSet<Note> = cx.read(|cx| {
        view.read(cx)
            .piano_roll()
            .selection()
            .notes()
            .cloned()
            .collect()
    });
    assert_eq!(selected, notes, "검색은 선택을 바꾸지 않는다");
}

/// 리뷰 1 지적 3: 막대가 열린 동안 `ctrl-tab`은 모드를 바꾸지 않고, 다른 길(메뉴)로 모드가 바뀌면 `Enter`가
/// 커서를 옮기지 않고 막대를 닫는다.
#[gpui_kit::test]
fn bar_does_not_search_after_mode_change(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "modechange");
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "ctrl-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        Mode::Normal
    );
    assert!(bar_open(cx, &view));
    run(cx, handle, mode::Step);
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    let before = cursor(cx, &view);
    press(cx, handle, "enter");
    assert_eq!(cursor(cx, &view), before);
    assert!(!bar_open(cx, &view));
    assert_eq!(last_message(cx, &view), "검색은 Normal 모드에서 한다");
    assert_eq!(last_search(cx, &view), None);
}

/// 리뷰 1 지적 4: 지난 검색어가 잘못된 것이어도 오퍼레이터 대기를 끝낸다.
#[gpui_kit::test]
fn invalid_last_query_still_ends_operator_wait(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "badquery", Preset::Vim);
    press(cx, handle, "/");
    type_text(cx, handle, "b9");
    press(cx, handle, "enter");
    assert_eq!(last_message(cx, &view), "검색 b9: 음높이 범위 밖");
    press(cx, handle, "d");
    assert!(cx.read(|cx| view.read(cx).pending_operator().is_some()));
    press(cx, handle, "n");
    assert!(cx.read(|cx| view.read(cx).pending_operator().is_none()));
    assert_eq!(last_message(cx, &view), "검색 b9: 음높이 범위 밖");
}

/// 리뷰 1 지적 5: 팝업 메뉴와 막대는 하나만 열린다.
#[gpui_kit::test]
fn bar_and_popup_menu_close_each_other(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "menu");
    press(cx, handle, "cmd-f");
    run(cx, handle, keyscore_app::actions::mode::ShowMenu);
    assert!(!bar_open(cx, &view), "모드 메뉴를 열면 막대가 닫힌다");
    run(cx, handle, search::Open);
    assert!(bar_open(cx, &view));
    assert!(
        focus_in_bar(cx, handle),
        "막대를 열면 메뉴가 닫히고 포커스는 막대"
    );
}

/// 리뷰 1 지적 5: 막대가 열린 채 문서가 바뀌면 결과 수를 다시 센다.
#[gpui_kit::test]
fn bar_recounts_when_document_changes(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "recount");
    set_cursor(cx, &view, 0, 61);
    press(cx, handle, "x");
    assert_eq!(lead_notes(cx, &view), 2);
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    assert_eq!(status(cx, &view).as_deref(), Some("결과 1개"));
    // 막대 입력란이 포커스를 가진 채 다른 길(재생 중 편집 등)로 문서가 바뀐다
    view.update(cx, |this, cx| {
        this.document().update(cx, |doc, cx| {
            doc.undo().unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(status(cx, &view).as_deref(), Some("결과 2개"));
}
