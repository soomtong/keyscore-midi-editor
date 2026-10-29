//! S3-T3e 적대적 리뷰 1회차의 깨뜨리기 시험. 픽스처는 `quantize_sheet.rs`에서 복사했다.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::Opt;
use keyscore_app::actions::{edit, mode, transport};
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
    start_k(cx, name, None)
}

fn start_k(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rqsheet-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-rqsheet-{name}", std::process::id()),
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

// ---------- 깨뜨리기 ----------

/// 시도 1·2: 시트가 열린 채 `transport::SetTempo`(인자 없음, 템포 필드 포커스)를 디스패치한다. 처리기가 창 루트에
/// 있어 시트 포커스에서도 닿는다. 리뷰 1 때는 템포 필드가 포커스를 가져가고 시트가 열린 채 남았다(제안 1).
/// 수정(ee86cb0) 뒤에는 포커스 이탈로 시트가 취소로 닫힌다. 리뷰 2 제안 2로 이름과 단언을 그 동작에 맞췄다.
#[gpui_kit::test]
fn set_tempo_dispatch_while_sheet_open_cancels_sheet(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "tempo");
    select_all_notes(cx, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-q left");
    assert!(focus_in_sheet(cx, handle));
    run(cx, handle, transport::SetTempo(Opt(None)));
    let open_after_dispatch = sheet(cx, &view).is_some();
    let focus_after_dispatch = focus_in_sheet(cx, handle);
    assert!(!open_after_dispatch, "포커스가 나가면 시트는 닫힌다");
    assert!(!focus_after_dispatch);
    press(cx, handle, "escape");
    let open_after_escape = sheet(cx, &view).is_some();
    let doc_focus = focus_in_document(cx, handle, &view);
    press(cx, handle, "q");
    let notes = clip_notes(cx, &view);
    eprintln!(
        "open_after_dispatch={open_after_dispatch} focus_in_sheet={focus_after_dispatch} \
         open_after_escape={open_after_escape} doc_focus={doc_focus} notes={notes:?}"
    );
}

/// 시도 3: 전역 `ctrl-tab`(`mode::Cycle`). 모드는 바뀌지만 시트·포커스는 남아야 한다.
#[gpui_kit::test]
fn ctrl_tab_while_sheet_open_keeps_focus_in_sheet(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "ctrltab");
    let before = cx.read(|cx| view.read(cx).mode_state().mode());
    press(cx, handle, "shift-q left");
    press(cx, handle, "ctrl-tab");
    let after = cx.read(|cx| view.read(cx).mode_state().mode());
    eprintln!("mode {before:?} -> {after:?}");
    assert!(sheet(cx, &view).is_some());
    assert!(focus_in_sheet(cx, handle));
    assert_eq!(draft(cx, &view).grid, Grid::Eighth);
    press(cx, handle, "enter");
    assert_eq!(saved(cx).grid, Grid::Eighth);
    assert!(focus_in_document(cx, handle, &view));
}

/// 시도 4: 시트가 열린 채 모드 메뉴(`mode::ShowMenu`, 메뉴 막대)를 열면 시트는 취소로 닫히고, 모드 메뉴가 열린 채
/// 시트를 열면 모드 메뉴가 닫힌다(팝업은 하나).
#[gpui_kit::test]
fn mode_menu_and_sheet_are_exclusive(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "modemenu");
    press(cx, handle, "shift-q left");
    run(cx, handle, mode::ShowMenu);
    assert!(sheet(cx, &view).is_none());
    assert!(cx.read(|cx| view.read(cx).mode_menu_open()));
    assert_eq!(saved(cx), QuantizeSettings::default());
    run(cx, handle, edit::QuantizeSettings);
    assert!(!cx.read(|cx| view.read(cx).mode_menu_open()));
    assert!(sheet(cx, &view).is_some());
    assert!(focus_in_sheet(cx, handle));
    press(cx, handle, "escape");
    assert!(sheet(cx, &view).is_none());
    assert!(focus_in_document(cx, handle, &view));
}

/// 시도 5: 확인 버튼·배경 클릭 뒤 포커스가 문서로 돌아오는가(기존 시험은 닫힘만 본다).
#[gpui_kit::test]
fn mouse_close_returns_focus_to_document(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "mousefocus");
    press(cx, handle, "shift-q");
    click(cx, handle, "quantize-inc-2");
    click(cx, handle, "quantize-confirm");
    assert!(sheet(cx, &view).is_none());
    assert!(focus_in_document(cx, handle, &view), "확인 버튼 뒤");
    press(cx, handle, "shift-q");
    click(cx, handle, "quantize-cancel");
    assert!(focus_in_document(cx, handle, &view), "취소 버튼 뒤");
    press(cx, handle, "shift-q");
    click(cx, handle, "track-add");
    assert!(sheet(cx, &view).is_none());
    assert!(focus_in_document(cx, handle, &view), "배경 클릭 뒤");
}

/// 시도 6: 범위 끝(중간 값에서 큰 걸음).
#[gpui_kit::test]
fn large_steps_clamp_from_middle_values(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "clamp");
    cx.update(|cx| AppSettings::update(cx, |s| s.quantize = q(Grid::Quarter, true, 95, 70)));
    press(cx, handle, "shift-q");
    press(cx, handle, "left shift-left");
    assert_eq!(draft(cx, &view).grid, Grid::Quarter);
    press(cx, handle, "tab shift-right right");
    assert!(draft(cx, &view).triplet);
    press(cx, handle, "tab shift-right");
    assert_eq!(draft(cx, &view).strength, 100);
    press(cx, handle, "tab shift-right");
    assert_eq!(draft(cx, &view).swing, 75);
    press(cx, handle, "escape");
    cx.update(|cx| AppSettings::update(cx, |s| s.quantize = q(Grid::SixtyFourth, false, 5, 3)));
    press(cx, handle, "shift-q right shift-right");
    assert_eq!(draft(cx, &view).grid, Grid::SixtyFourth);
    press(cx, handle, "tab tab shift-left");
    assert_eq!(draft(cx, &view).strength, 0);
    press(cx, handle, "tab shift-left");
    assert_eq!(draft(cx, &view).swing, 0);
}

/// 시도 7: 취소·같은 값 확인은 설정 파일을 쓰지 않는다.
#[gpui_kit::test]
fn cancel_and_unchanged_confirm_do_not_write_file(cx: &mut TestAppContext) {
    let (handle, _view, dir) = setup_with(cx, "nowrite", Preset::Default);
    cx.update(AppSettings::flush);
    let path = dir.join("settings.json");
    let before = std::fs::read_to_string(&path).ok();
    let _ = std::fs::remove_file(&path);
    press(cx, handle, "shift-q right escape");
    press(cx, handle, "shift-q right left enter");
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(5));
    cx.run_until_parked();
    eprintln!("before={before:?}");
    assert!(!path.exists(), "취소·같은 값 확인이 설정 파일을 썼다");
}

/// 시도 8: 창 둘. 한쪽 확인이 다른 쪽 초안을 바꾸지 않고, 마지막 확인이 남는다.
#[gpui_kit::test]
fn two_windows_last_confirm_wins(cx: &mut TestAppContext) {
    let (h1, v1) = setup(cx, "twowin");
    let (h2, v2) = cx
        .update(|cx| open_project_window(project_with(&OFF_GRID), cx))
        .unwrap();
    cx.update_window(h2, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, h2);
    press(cx, h2, "shift-q left");
    cx.update_window(h1, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    press(cx, h1, "shift-q right enter");
    assert_eq!(saved(cx).grid, Grid::ThirtySecond);
    assert_eq!(draft(cx, &v2).grid, Grid::Eighth, "다른 창의 초안은 그대로");
    assert!(sheet(cx, &v1).is_none());
    cx.update_window(h2, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    press(cx, h2, "enter");
    assert_eq!(saved(cx).grid, Grid::Eighth);
}

/// 시도 9: vim 프리셋에서 시트 안 키(Esc·Enter·화살표·Tab)가 시트로 간다.
#[gpui_kit::test]
fn vim_preset_sheet_keys(cx: &mut TestAppContext) {
    let (handle, view, _) = setup_with(cx, "vimkeys", Preset::Vim);
    press(cx, handle, "shift-q right tab right down down up");
    assert_eq!(row(cx, &view), SheetRow::Strength);
    assert_eq!(draft(cx, &view), q(Grid::ThirtySecond, true, 100, 0));
    press(cx, handle, "escape");
    assert!(sheet(cx, &view).is_none());
    assert_eq!(saved(cx), QuantizeSettings::default());
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        Mode::Normal
    );
    press(cx, handle, "shift-q right enter");
    assert_eq!(saved(cx).grid, Grid::ThirtySecond);
    assert!(focus_in_document(cx, handle, &view));
}

/// 시도 10: 사용자 키맵으로 시트 Action을 다른 키에 건다(UI-1).
#[gpui_kit::test]
fn user_keymap_rebinds_sheet_actions(cx: &mut TestAppContext) {
    start_k(
        cx,
        "userkm",
        Some(
            r#"[{"context": "QuantizeSheet", "bindings": {"j": "window::FocusNext", "k": "window::FocusPrev", "l": "dialog::Increase", "h": "dialog::Decrease", "shift-l": "dialog::IncreaseLarge"}}]"#,
        ),
    );
    let (handle, view) = cx
        .update(|cx| open_project_window(project_with(&OFF_GRID), cx))
        .unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    cx.update(|cx| AppSettings::update(cx, |s| s.quantize = q(Grid::Sixteenth, false, 50, 0)));
    press(cx, handle, "shift-q l j l j j k h shift-l");
    assert_eq!(row(cx, &view), SheetRow::Strength);
    assert_eq!(draft(cx, &view), q(Grid::ThirtySecond, true, 59, 0));
    let (_, d) = sheet(cx, &view).unwrap();
    eprintln!("draft={d:?}");
}

/// 시도 11: 시트가 열린 채 전역 `cmd-z`. 계획 3절은 전역 바인딩이 그대로 동작한다고 적었다. 동작을 기록한다.
#[gpui_kit::test]
fn cmd_z_while_sheet_open_undoes_document(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmdz");
    set_cursor(cx, &view, 250, 60);
    press(cx, handle, "+");
    assert_eq!(clip_notes(cx, &view)[0], (250, 100, 61));
    press(cx, handle, "shift-q");
    press(cx, handle, "cmd-z");
    let notes = clip_notes(cx, &view);
    eprintln!("after cmd-z: {notes:?} sheet={:?}", sheet(cx, &view));
    assert!(sheet(cx, &view).is_some());
    assert!(focus_in_sheet(cx, handle));
}

/// 시도 12: 시트가 열린 채 메뉴로 `edit::Quantize`(편집 > 퀀타이즈). 초안이 아니라 저장된 설정으로 퀀타이즈하는가.
#[gpui_kit::test]
fn menu_quantize_while_sheet_open_uses_saved_settings(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "menuq");
    select_all_notes(cx, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-q left");
    run(cx, handle, edit::Quantize);
    let notes = clip_notes(cx, &view);
    eprintln!(
        "menu q with sheet open: {notes:?} sheet={:?}",
        sheet(cx, &view)
    );
    assert!(sheet(cx, &view).is_some());
    assert!(focus_in_sheet(cx, handle));
}
