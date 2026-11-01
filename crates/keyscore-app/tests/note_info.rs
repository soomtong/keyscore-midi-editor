//! S3-T7 노트 정보 팝업(REQ 5.8 `i`, 5.8.1 `K`). 헤드리스 창에서 키를 눌러 팝업을 본다. 규칙은 결정
//! `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md` 6절.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{mode, note, palette};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const BAR: u64 = 3840;

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-info-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-info-{name}", std::process::id()),
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

fn info(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<Vec<(String, String)>> {
    cx.read(|cx| {
        view.read(cx).note_info().map(|lines| {
            lines
                .into_iter()
                .map(|l| (l.label.to_owned(), l.value))
                .collect()
        })
    })
}

fn focus_in_info(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| {
        let stack = window.context_stack();
        stack.iter().any(|c| c.contains("Dialog")) && stack.iter().any(|c| c.contains("NoteInfo"))
    })
    .unwrap()
}

fn value(lines: &[(String, String)], label: &str) -> String {
    lines.iter().find(|(l, _)| l == label).map_or_else(
        || panic!("{label} 줄이 없다: {lines:?}"),
        |(_, v)| v.clone(),
    )
}

// ---------- 23–27 ----------

#[gpui_kit::test]
fn i_opens_note_info_for_cursor_note(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "open");
    set_cursor(cx, &view, 100, 61);
    press(cx, handle, "i");
    let lines = info(cx, &view).expect("팝업이 열린다");
    assert_eq!(value(&lines, "음높이"), "C#4 (61)");
    assert_eq!(value(&lines, "시작"), "1.1.0 (0틱)");
    assert_eq!(value(&lines, "길이"), "480틱 (8분음표)");
    assert_eq!(value(&lines, "채널"), "1");
    assert_eq!(value(&lines, "가사"), "반");
    assert!(focus_in_info(cx, handle), "포커스가 팝업에 있다");
    // 그려진 줄
    let label = cx
        .update_window(handle, |_, window, _| {
            window
                .try_find("note-info-0")
                .and_then(|e| e.label().map(str::to_owned))
        })
        .unwrap();
    assert_eq!(label.as_deref(), Some("음높이: C#4 (61)"));
    press(cx, handle, "escape");
    assert!(info(cx, &view).is_none());
    assert!(focus_in_document(cx, handle, &view));
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        Mode::Normal
    );
    press(cx, handle, "i");
    assert!(info(cx, &view).is_some());
    press(cx, handle, "enter");
    assert!(info(cx, &view).is_none());
    assert!(focus_in_document(cx, handle, &view));
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        Mode::Normal,
        "팝업의 Enter는 Step으로 들어가지 않는다"
    );
}

#[gpui_kit::test]
fn i_without_note_says_so(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "empty");
    set_cursor(cx, &view, 100, 90);
    press(cx, handle, "i");
    assert!(info(cx, &view).is_none());
    assert_eq!(last_message(cx, &view), "커서에 노트가 없다");
}

#[gpui_kit::test]
fn vim_shift_k_opens_note_info(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "vim", Preset::Vim);
    // Bass Line 트랙의 노트(채널 2)
    press(cx, handle, "tab");
    set_cursor(cx, &view, BAR + 960, 49);
    press(cx, handle, "shift-k");
    let lines = info(cx, &view).expect("팝업이 열린다");
    assert_eq!(value(&lines, "채널"), "2");
    assert_eq!(value(&lines, "시작"), "2.2.0 (4800틱)");
    assert_eq!(value(&lines, "트랙"), "Bass Line");
}

#[gpui_kit::test]
fn note_info_closes_on_document_change_and_outside_click(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "close");
    set_cursor(cx, &view, 0, 61);
    press(cx, handle, "x");
    assert_eq!(lead_notes(cx, &view), 2);
    press(cx, handle, "cmd-z");
    assert_eq!(lead_notes(cx, &view), 3);
    press(cx, handle, "i");
    assert!(info(cx, &view).is_some());
    // 열린 채 전역 cmd-z(문서가 바뀐다) → 닫힘
    press(cx, handle, "cmd-shift-z");
    assert_eq!(lead_notes(cx, &view), 2, "다시 실행이 문서를 바꿨다");
    assert!(info(cx, &view).is_none());
    // 바깥 클릭은 닫기만 한다
    press(cx, handle, "cmd-z");
    press(cx, handle, "i");
    assert!(info(cx, &view).is_some());
    // 바깥(트랙 추가 버튼 자리)을 누르면 닫히고 그 누름은 아래에 닿지 않는다
    let tracks = cx.read(|cx| view.read(cx).document().read(cx).project().tracks().len());
    click(cx, handle, "track-add");
    assert!(info(cx, &view).is_none());
    assert_eq!(
        cx.read(|cx| view.read(cx).document().read(cx).project().tracks().len()),
        tracks
    );
    assert!(focus_in_document(cx, handle, &view));
    // 팔레트를 열면 닫힌다
    press(cx, handle, "i");
    run(cx, handle, palette::Open);
    assert!(info(cx, &view).is_none());
}

#[gpui_kit::test]
fn note_info_keys_do_not_reach_document(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "keys");
    set_cursor(cx, &view, 0, 61);
    let undo = undo_len(cx, &view);
    press(cx, handle, "i");
    press(cx, handle, "x l j i");
    assert!(info(cx, &view).is_some());
    assert_eq!(lead_notes(cx, &view), 3);
    assert_eq!(cursor(cx, &view), (0, 61));
    assert_eq!(undo_len(cx, &view), undo);
    // 메뉴·팔레트 경로는 다른 모드에서도 연다
    press(cx, handle, "escape");
    run(cx, handle, mode::Step);
    set_cursor(cx, &view, 0, 61);
    run(cx, handle, note::ShowInfo);
    assert!(info(cx, &view).is_some());
}
