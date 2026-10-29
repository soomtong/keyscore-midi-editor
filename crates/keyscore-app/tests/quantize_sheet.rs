//! S3-T3e 퀀타이즈 설정 시트(REQ 5.8 `Shift+q`, EDT-2, 7.6). 헤드리스 창에서 키를 눌러 시트·설정·문서를 본다.
//! 규칙은 `docs/reports/s3-t3e/plan.md` 「끝의 정의」.

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::Opt;
use keyscore_app::actions::{edit, help, mode, palette, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::quantize_sheet::SheetRow;
use keyscore_app::settings::QuantizeSettings;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::transform::Grid;
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const BAR: u64 = 3840;

type Triple = (u64, u32, u8);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-qsheet-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-qsheet-{name}", std::process::id()),
        );
    });
    dir
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

fn triple(n: &Note) -> Triple {
    (n.start().get(), n.length(), n.pitch().get())
}

/// 클립 하나 [0, 2마디).
fn project_with(notes: &[Triple]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    for &(s, l, p) in notes {
        clip.insert_note(note(s, l, p)).unwrap();
    }
    track.insert_clip(clip).unwrap();
    project.push_track(track).unwrap();
    project
}

const OFF_GRID: [Triple; 2] = [(250, 100, 60), (1000, 100, 62)];

fn setup_with(
    cx: &mut TestAppContext,
    name: &str,
    preset: Preset,
) -> (AnyWindowHandle, Entity<ProjectWindow>, PathBuf) {
    let dir = start(cx, name);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let (handle, view) = cx
        .update(|cx| open_project_window(project_with(&OFF_GRID), cx))
        .unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    (handle, view, dir)
}

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view, _) = setup_with(cx, name, Preset::Default);
    (handle, view)
}

fn sheet(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
) -> Option<(SheetRow, QuantizeSettings)> {
    cx.read(|cx| view.read(cx).quantize_sheet())
}

fn draft(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> QuantizeSettings {
    sheet(cx, view).expect("시트가 열려 있지 않다").1
}

fn row(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> SheetRow {
    sheet(cx, view).expect("시트가 열려 있지 않다").0
}

fn saved(cx: &mut TestAppContext) -> QuantizeSettings {
    cx.read(|cx| AppSettings::get(cx).quantize)
}

/// 포커스가 시트 안(`Dialog > QuantizeSheet`)에 있는가.
fn focus_in_sheet(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| {
        let stack = window.context_stack();
        stack.iter().any(|c| c.contains("Dialog"))
            && stack.iter().any(|c| c.contains("QuantizeSheet"))
    })
    .unwrap()
}

/// 포커스가 문서(모드 키 컨텍스트 안쪽)에 있는가.
fn focus_in_document(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> bool {
    let focus = cx.read(|cx| view.read(cx).document_focus_for_test());
    cx.update_window(handle, |_, window, _| focus.is_focused(window))
        .unwrap()
}

fn clip_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
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

fn select_all_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) {
    let (track, clip, notes) = cx.read(|cx| {
        let t = &view.read(cx).document().read(cx).project().tracks()[0];
        let (clip_id, clip) = t.clips().next().unwrap();
        let notes: BTreeSet<Note> = clip.notes().iter().cloned().collect();
        (t.id(), clip_id, notes)
    });
    view.update(cx, |this, cx| {
        let mut sel = Selection::default();
        sel.set(track, clip, notes);
        this.set_selection(sel, cx);
    });
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

fn q(grid: Grid, triplet: bool, strength: u8, swing: u8) -> QuantizeSettings {
    QuantizeSettings {
        grid,
        triplet,
        strength,
        swing,
    }
}

// ---------- 1–3. 열기 ----------

#[gpui_kit::test]
fn shift_q_opens_sheet_with_current_settings(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "open");
    cx.update(|cx| AppSettings::update(cx, |s| s.quantize = q(Grid::ThirtySecond, true, 70, 20)));
    assert!(sheet(cx, &view).is_none());
    press(cx, handle, "shift-q");
    assert_eq!(
        sheet(cx, &view),
        Some((SheetRow::Grid, q(Grid::ThirtySecond, true, 70, 20)))
    );
    assert!(focus_in_sheet(cx, handle), "포커스가 시트 안에 있어야 한다");
    // 시트가 그려지고 줄마다 이름과 값을 보인다
    let label = cx
        .update_window(handle, |_, window, _| {
            window
                .try_find("quantize-row-0")
                .and_then(|e| e.label().map(str::to_owned))
        })
        .unwrap();
    assert_eq!(label.as_deref(), Some("그리드: 1/32"));
}

#[gpui_kit::test]
fn menu_action_opens_sheet_in_step_mode(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "step");
    run(cx, handle, mode::Step);
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    run(cx, handle, edit::QuantizeSettings);
    assert_eq!(row(cx, &view), SheetRow::Grid);
    assert!(focus_in_sheet(cx, handle));
    press(cx, handle, "escape");
    assert!(sheet(cx, &view).is_none());
    // 시트의 Esc는 시트만 닫는다: 모드는 그대로
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
}

#[gpui_kit::test]
fn vim_preset_shift_q_opens_sheet(cx: &mut TestAppContext) {
    let (handle, view, _) = setup_with(cx, "vim", Preset::Vim);
    press(cx, handle, "shift-q");
    assert!(sheet(cx, &view).is_some());
    assert!(focus_in_sheet(cx, handle));
}

// ---------- 4–5. 줄 이동과 값 ----------

#[gpui_kit::test]
fn tab_and_arrows_move_rows_and_wrap(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rows");
    press(cx, handle, "shift-q");
    press(cx, handle, "tab");
    assert_eq!(row(cx, &view), SheetRow::Triplet);
    press(cx, handle, "down");
    assert_eq!(row(cx, &view), SheetRow::Strength);
    press(cx, handle, "tab down");
    assert_eq!(row(cx, &view), SheetRow::Grid, "스윙 다음은 그리드");
    press(cx, handle, "shift-tab");
    assert_eq!(row(cx, &view), SheetRow::Swing, "그리드 이전은 스윙");
    press(cx, handle, "up");
    assert_eq!(row(cx, &view), SheetRow::Strength);
    assert!(
        focus_in_sheet(cx, handle),
        "줄 이동이 포커스를 시트 밖으로 내보내지 않는다"
    );
}

#[gpui_kit::test]
fn arrows_change_each_value_and_stop_at_ends(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "values");
    press(cx, handle, "shift-q");
    // 그리드: 1/16에서 → 1/32, 1/64, 끝에서 멈춤. ← 1/4까지, 끝에서 멈춤
    press(cx, handle, "right");
    assert_eq!(draft(cx, &view).grid, Grid::ThirtySecond);
    press(cx, handle, "right right");
    assert_eq!(draft(cx, &view).grid, Grid::SixtyFourth);
    press(cx, handle, "left left left");
    assert_eq!(draft(cx, &view).grid, Grid::Eighth);
    press(cx, handle, "shift-left left");
    assert_eq!(
        draft(cx, &view).grid,
        Grid::Quarter,
        "Shift도 그리드는 한 칸"
    );
    press(cx, handle, "shift-right");
    assert_eq!(draft(cx, &view).grid, Grid::Eighth);
    // 셋잇단: → 켬, ← 끔, 같은 쪽을 다시 눌러도 그대로
    press(cx, handle, "tab right");
    assert!(draft(cx, &view).triplet);
    press(cx, handle, "right");
    assert!(draft(cx, &view).triplet);
    press(cx, handle, "left left");
    assert!(!draft(cx, &view).triplet);
    press(cx, handle, "shift-right");
    assert!(draft(cx, &view).triplet);
    // 강도: 100에서 ↑ 막힘, ← 1씩, Shift+← 10씩, 0에서 멈춤
    press(cx, handle, "tab right shift-right");
    assert_eq!(draft(cx, &view).strength, 100);
    press(cx, handle, "left");
    assert_eq!(draft(cx, &view).strength, 99);
    press(cx, handle, "shift-left");
    assert_eq!(draft(cx, &view).strength, 89);
    for _ in 0..9 {
        press(cx, handle, "shift-left");
    }
    assert_eq!(draft(cx, &view).strength, 0);
    press(cx, handle, "right");
    assert_eq!(draft(cx, &view).strength, 1);
    // 스윙: 0에서 ← 막힘, Shift+→ 10씩, 75에서 멈춤
    press(cx, handle, "tab left shift-left");
    assert_eq!(draft(cx, &view).swing, 0);
    press(cx, handle, "shift-right right");
    assert_eq!(draft(cx, &view).swing, 11);
    for _ in 0..7 {
        press(cx, handle, "shift-right");
    }
    assert_eq!(draft(cx, &view).swing, 75);
    press(cx, handle, "right");
    assert_eq!(draft(cx, &view).swing, 75);
    press(cx, handle, "left");
    assert_eq!(draft(cx, &view).swing, 74);
    // 초안만 바뀌고 설정은 그대로다
    assert_eq!(saved(cx), QuantizeSettings::default());
}

// ---------- 6–8. 확인·취소 ----------

#[gpui_kit::test]
fn enter_saves_and_q_uses_new_values(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "enter");
    press(cx, handle, "shift-q");
    // 1/8, 셋잇단 켬, 강도 50
    press(cx, handle, "left down right down");
    for _ in 0..5 {
        press(cx, handle, "shift-left");
    }
    press(cx, handle, "enter");
    assert!(sheet(cx, &view).is_none());
    assert_eq!(saved(cx), q(Grid::Eighth, true, 50, 0));
    assert!(focus_in_document(cx, handle, &view));
    assert!(
        last_message(cx, &view).contains("1/8, 셋잇단 켬, 강도 50%, 스윙 0%"),
        "{}",
        last_message(cx, &view)
    );
    // 바꾼 값으로 q: 셋잇단 1/8 그리드 320틱. 250 → 320의 50% = 285, 1000 → 960의 50% = 980
    select_all_notes(cx, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "q");
    assert_eq!(clip_notes(cx, &view), vec![(285, 100, 60), (980, 100, 62)]);
}

#[gpui_kit::test]
fn enter_writes_settings_file(cx: &mut TestAppContext) {
    let (handle, view, dir) = setup_with(cx, "file", Preset::Default);
    press(cx, handle, "shift-q");
    press(cx, handle, "right up shift-right");
    assert_eq!(draft(cx, &view), q(Grid::ThirtySecond, false, 100, 10));
    press(cx, handle, "enter");
    cx.update(AppSettings::flush);
    let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        json["quantize"],
        serde_json::json!({"grid": "1/32", "triplet": false, "strength": 100, "swing": 10})
    );
}

#[gpui_kit::test]
fn escape_discards_changes(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "escape");
    press(cx, handle, "shift-q");
    press(cx, handle, "left tab right");
    let before = last_message(cx, &view);
    press(cx, handle, "escape");
    assert!(sheet(cx, &view).is_none());
    assert_eq!(saved(cx), QuantizeSettings::default());
    assert!(focus_in_document(cx, handle, &view));
    assert_eq!(
        last_message(cx, &view),
        before,
        "취소는 메시지를 남기지 않는다"
    );
    // 다시 열면 초안은 지금 설정에서 시작한다
    press(cx, handle, "shift-q");
    assert_eq!(
        sheet(cx, &view),
        Some((SheetRow::Grid, QuantizeSettings::default()))
    );
}

// ---------- 9–10. 문서와 분리 ----------

#[gpui_kit::test]
fn document_keys_do_not_reach_document_while_open(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "isolate");
    select_all_notes(cx, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-q");
    press(cx, handle, "q x l u h j k");
    assert_eq!(clip_notes(cx, &view), OFF_GRID.to_vec());
    assert_eq!(cursor(cx, &view), (0, 60));
    assert_eq!(undo_len(cx, &view), 0);
    assert!(sheet(cx, &view).is_some(), "글자 키가 시트를 닫지 않는다");
    assert!(focus_in_sheet(cx, handle));
}

#[gpui_kit::test]
fn sheet_does_not_touch_undo_or_repeat(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "repeat");
    // 노트 하나를 반음 올리고(마지막 편집), 시트를 확인·취소한 뒤 `.`
    set_cursor(cx, &view, 250, 60);
    press(cx, handle, "+");
    assert_eq!(clip_notes(cx, &view)[0], (250, 100, 61));
    assert_eq!(undo_len(cx, &view), 1);
    set_cursor(cx, &view, 250, 61);
    press(cx, handle, "shift-q left enter");
    press(cx, handle, "shift-q right escape");
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, ".");
    assert_eq!(
        clip_notes(cx, &view)[0],
        (250, 100, 62),
        "`.`는 시트 전 편집을 반복한다"
    );
}

// ---------- 11–12. 다른 팝업 ----------

#[gpui_kit::test]
fn palette_and_overlay_close_sheet_and_sheet_closes_them(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "popups");
    // 시트에서 팔레트를 열면 시트는 취소로 닫힌다
    press(cx, handle, "shift-q left");
    run(cx, handle, palette::Open);
    assert!(sheet(cx, &view).is_none());
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    assert_eq!(saved(cx), QuantizeSettings::default(), "초안을 버린다");
    // 팔레트가 열린 채 시트를 열면 팔레트가 닫힌다
    run(cx, handle, edit::QuantizeSettings);
    assert!(!cx.read(|cx| view.read(cx).palette_open()));
    assert!(sheet(cx, &view).is_some());
    assert!(focus_in_sheet(cx, handle));
    // 오버레이도 같다
    run(cx, handle, help::KeymapOverlay);
    assert!(sheet(cx, &view).is_none());
    assert!(cx.read(|cx| view.read(cx).overlay_lines(cx)).is_some());
    run(cx, handle, edit::QuantizeSettings);
    assert!(cx.read(|cx| view.read(cx).overlay_lines(cx)).is_none());
    assert!(sheet(cx, &view).is_some());
}

#[gpui_kit::test]
fn reopen_while_open_keeps_draft(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "reopen");
    press(cx, handle, "shift-q left tab tab shift-left");
    let before = sheet(cx, &view);
    assert_eq!(
        before,
        Some((SheetRow::Strength, q(Grid::Eighth, false, 90, 0)))
    );
    run(cx, handle, edit::QuantizeSettings);
    assert_eq!(sheet(cx, &view), before);
    assert!(focus_in_sheet(cx, handle));
}

/// 루트 처리기가 포커스를 옮기는 Action(템포 필드)을 받으면 시트는 취소로 닫힌다. 시트가 보이는 채 문서 키가
/// 시트 뒤에서 동작하지 않는다(리뷰 1 제안 1).
#[gpui_kit::test]
fn focus_leaving_sheet_cancels_it(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "blur");
    press(cx, handle, "shift-q left");
    run(cx, handle, transport::SetTempo(Opt(None)));
    assert!(sheet(cx, &view).is_none());
    assert_eq!(saved(cx), QuantizeSettings::default(), "초안을 버린다");
    assert!(!focus_in_sheet(cx, handle));
}

// ---------- 13. 마우스 ----------

#[gpui_kit::test]
fn buttons_and_backdrop_click(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "mouse");
    press(cx, handle, "shift-q");
    // 셋잇단 줄을 누르면 그 줄로, › 버튼은 그 줄의 늘리기
    click(cx, handle, "quantize-row-1");
    assert_eq!(row(cx, &view), SheetRow::Triplet);
    click(cx, handle, "quantize-inc-1");
    assert!(draft(cx, &view).triplet);
    click(cx, handle, "quantize-dec-0");
    assert_eq!(draft(cx, &view).grid, Grid::Eighth);
    assert_eq!(row(cx, &view), SheetRow::Grid, "버튼도 그 줄로 옮긴다");
    assert!(
        focus_in_sheet(cx, handle),
        "버튼 클릭 뒤에도 키보드로 이어 쓴다"
    );
    click(cx, handle, "quantize-confirm");
    assert!(sheet(cx, &view).is_none());
    assert_eq!(saved(cx), q(Grid::Eighth, true, 100, 0));
    // 취소 버튼
    press(cx, handle, "shift-q right");
    click(cx, handle, "quantize-cancel");
    assert!(sheet(cx, &view).is_none());
    assert_eq!(saved(cx), q(Grid::Eighth, true, 100, 0));
    // 바깥 클릭은 취소이고 아래 버튼(트랙 추가)에 닿지 않는다
    let tracks = cx.read(|cx| view.read(cx).document().read(cx).project().tracks().len());
    press(cx, handle, "shift-q right");
    click(cx, handle, "track-add");
    assert!(sheet(cx, &view).is_none());
    assert_eq!(saved(cx), q(Grid::Eighth, true, 100, 0));
    assert_eq!(
        cx.read(|cx| view.read(cx).document().read(cx).project().tracks().len()),
        tracks
    );
}

// ---------- 14. 키맵 ----------

#[gpui_kit::test]
fn sheet_actions_are_bound_in_quantize_sheet_context(cx: &mut TestAppContext) {
    start(cx, "keymap");
    for (keys, name) in [
        ("right", "dialog::Increase"),
        ("left", "dialog::Decrease"),
        ("shift-right", "dialog::IncreaseLarge"),
        ("shift-left", "dialog::DecreaseLarge"),
        ("down", "window::FocusNext"),
        ("up", "window::FocusPrev"),
        ("tab", "window::FocusNext"),
        ("enter", "dialog::Confirm"),
        ("escape", "dialog::Cancel"),
    ] {
        let found = cx.update(|cx| {
            let stack = ["Dialog", "QuantizeSheet"]
                .iter()
                .map(|c| gpui_kit::KeyContext::parse(c).unwrap())
                .collect::<Vec<_>>();
            let keystrokes = vec![gpui_kit::Keystroke::parse(keys).unwrap()];
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let (bindings, _) = keymap.bindings_for_input(&keystrokes, &stack);
            bindings.first().map(|b| b.action().name().to_owned())
        });
        assert_eq!(found.as_deref(), Some(name), "{keys}");
    }
}
