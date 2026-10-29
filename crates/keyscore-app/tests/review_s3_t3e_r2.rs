//! S3-T3e 적대적 리뷰 2회차의 깨뜨리기 시험(커밋 ee86cb0, 포커스 이탈 취소). 픽스처는 `review_s3_t3e.rs`에서 복사했다.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::Opt;
use keyscore_app::actions::{dialog, edit, mode, palette, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
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
    let dir = std::env::temp_dir().join(format!("ks-r2qsheet-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-r2qsheet-{name}", std::process::id()),
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

/// 입력란(트랜스포트 필드, 팔레트 질의)에 포커스가 있는가.
fn focus_in_input(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| {
        window.context_stack().iter().any(|c| c.contains("Input"))
    })
    .unwrap()
}

fn message_count(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).messages().len())
}

/// 창이 활성인 동안 「시트가 열려 있다 ⇔ 포커스가 시트 안」(ee86cb0이 지키려는 성질).
fn assert_sheet_iff_focus(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    at: &str,
) {
    let open = sheet(cx, view).is_some();
    let focus = focus_in_sheet(cx, handle);
    assert_eq!(
        open, focus,
        "{at}: 시트 열림={open}, 포커스 시트 안={focus}"
    );
}

// ---------- 깨뜨리기 ----------

/// 시도 1: 창 비활성·재활성. 다른 창을 활성으로 만들어 이 창의 포커스 경로가 비어도 시트는 남고, 돌아오면 초안 그대로
/// 이어 쓴다(입력란과 같은 관례).
#[gpui_kit::test]
fn deactivate_and_reactivate_keeps_sheet_and_draft(cx: &mut TestAppContext) {
    let (h1, v1) = setup(cx, "deact");
    press(cx, h1, "shift-q left tab tab shift-left");
    let before = sheet(cx, &v1);
    let (h2, _v2) = cx
        .update(|cx| open_project_window(project_with(&OFF_GRID), cx))
        .unwrap();
    cx.update_window(h2, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, h2);
    render(cx, h1);
    cx.run_until_parked();
    render(cx, h1);
    let active = cx
        .update_window(h1, |_, window, _| window.is_window_active())
        .unwrap();
    assert!(!active, "h1이 비활성이 되지 않았다(시험 전제)");
    assert_eq!(sheet(cx, &v1), before, "창 비활성으로 시트가 닫혔다");
    cx.update_window(h1, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, h1);
    assert_eq!(sheet(cx, &v1), before);
    assert!(focus_in_sheet(cx, h1), "재활성 뒤 포커스가 시트에 없다");
    press(cx, h1, "enter");
    assert_eq!(saved(cx), q(Grid::Eighth, false, 90, 0));
    assert!(focus_in_document(cx, h1, &v1));
}

/// 시도 2: 닫고 그릴 틈 없이 다시 연다. 앞 시트의 구독이 남아 새 시트를 닫지 않는가.
#[gpui_kit::test]
fn close_and_reopen_without_frame_keeps_new_sheet(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "reopenburst");
    press(cx, handle, "shift-q left escape shift-q");
    assert_eq!(
        sheet(cx, &view),
        Some((SheetRow::Grid, QuantizeSettings::default()))
    );
    render(cx, handle);
    assert_sheet_iff_focus(cx, handle, &view, "Esc 뒤 다시 열기");
    press(cx, handle, "left enter shift-q");
    assert_eq!(saved(cx).grid, Grid::Eighth);
    assert!(sheet(cx, &view).is_some());
    assert_sheet_iff_focus(cx, handle, &view, "Enter 뒤 다시 열기");
    // 한 update 안에서 취소와 열기를 잇는다(렌더 없이)
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(dialog::Cancel), cx);
        window.dispatch_action(Box::new(edit::QuantizeSettings), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    render(cx, handle);
    assert!(
        sheet(cx, &view).is_some(),
        "다시 연 시트가 앞 구독에 닫혔다"
    );
    assert_sheet_iff_focus(cx, handle, &view, "한 update 안 취소+열기");
    // 열고 닫기를 여러 번 되풀이한 뒤에도 시트 밖 동작에 구독이 끼지 않는다
    for _ in 0..5 {
        press(cx, handle, "escape shift-q");
    }
    press(cx, handle, "escape");
    assert!(sheet(cx, &view).is_none());
    assert!(focus_in_document(cx, handle, &view));
    let messages = message_count(cx, &view);
    run(cx, handle, transport::SetTempo(Opt(None)));
    assert!(
        focus_in_input(cx, handle),
        "닫힌 시트의 구독이 템포 필드 포커스를 빼앗았다"
    );
    assert_eq!(message_count(cx, &view), messages);
}

/// 시도 3: 포커스 이탈 취소 뒤 템포 필드가 포커스를 그대로 갖고, 메시지·undo가 없으며, `Esc`·`q`가 저장된 설정(기본값)을
/// 쓴다(초안 1/8이 새지 않는다).
#[gpui_kit::test]
fn focus_out_cancel_keeps_field_focus_and_discards_draft(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "tempofield");
    select_all_notes(cx, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-q left");
    let messages = message_count(cx, &view);
    let undo = undo_len(cx, &view);
    run(cx, handle, transport::SetTempo(Opt(None)));
    assert!(sheet(cx, &view).is_none());
    assert!(
        focus_in_input(cx, handle),
        "템포 필드가 포커스를 잃었다(시트 닫기가 문서로 돌렸다)"
    );
    assert_eq!(
        message_count(cx, &view),
        messages,
        "취소인데 메시지를 남겼다"
    );
    assert_eq!(undo_len(cx, &view), undo);
    press(cx, handle, "escape");
    assert!(focus_in_document(cx, handle, &view));
    press(cx, handle, "q");
    assert_eq!(clip_notes(cx, &view), vec![(240, 100, 60), (960, 100, 62)]);
}

/// 시도 4: 시트 안·배경 아래를 오른쪽 클릭한다. 버튼·루트 처리기가 포커스를 가져가 시트가 닫히거나 고아가 되는가.
#[gpui_kit::test]
fn right_clicks_do_not_orphan_sheet(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rclick");
    press(cx, handle, "shift-q");
    for id in [
        "quantize-row-2",
        "quantize-inc-2",
        "quantize-dec-0",
        "quantize-confirm",
        "quantize-cancel",
        "quantize-sheet",
        "track-add",
    ] {
        cx.update_window(handle, |_, window, cx| window.right_click(id, cx))
            .unwrap();
        cx.run_until_parked();
        render(cx, handle);
        render(cx, handle);
        eprintln!(
            "{id}: open={} focus_in_sheet={}",
            sheet(cx, &view).is_some(),
            focus_in_sheet(cx, handle)
        );
        assert_sheet_iff_focus(cx, handle, &view, id);
        assert_eq!(
            saved(cx),
            QuantizeSettings::default(),
            "{id}: 오른쪽 클릭이 확인했다"
        );
        if sheet(cx, &view).is_none() {
            press(cx, handle, "shift-q");
        }
    }
    // 시트가 남았다면 키보드로 이어 쓴다
    press(cx, handle, "right enter");
    assert_eq!(saved(cx).grid, Grid::ThirtySecond);
}

/// 시도 5: 시트에서 팔레트·모드 메뉴를 열면 시트는 닫히고, 몇 프레임 뒤에도 포커스는 그쪽에 남는다(시트 닫기가 문서로
/// 포커스를 되돌려 팔레트·메뉴를 닫지 않는다). 확인 버튼 클릭은 포커스 이탈보다 먼저 닫히지 않는다.
#[gpui_kit::test]
fn palette_and_mode_menu_keep_focus_after_sheet_closes(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "popupfocus");
    press(cx, handle, "shift-q left");
    run(cx, handle, palette::Open);
    render(cx, handle);
    render(cx, handle);
    assert!(sheet(cx, &view).is_none());
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    assert!(focus_in_input(cx, handle), "팔레트 질의가 포커스를 잃었다");
    press(cx, handle, "escape");
    assert!(!cx.read(|cx| view.read(cx).palette_open()));
    press(cx, handle, "shift-q left");
    run(cx, handle, mode::ShowMenu);
    render(cx, handle);
    render(cx, handle);
    assert!(sheet(cx, &view).is_none());
    assert!(
        cx.read(|cx| view.read(cx).mode_menu_open()),
        "모드 메뉴가 닫혔다"
    );
    assert_eq!(saved(cx), QuantizeSettings::default());
    run(cx, handle, edit::QuantizeSettings);
    assert_sheet_iff_focus(cx, handle, &view, "메뉴에서 다시 열기");
    // 버튼 클릭: ‹ 뒤 확인이 초안을 저장한다(포커스 이탈 취소가 먼저 닫지 않는다)
    click(cx, handle, "quantize-dec-0");
    assert_sheet_iff_focus(cx, handle, &view, "‹ 클릭 뒤");
    click(cx, handle, "quantize-confirm");
    assert_eq!(saved(cx).grid, Grid::Eighth, "확인 버튼이 취소로 처리됐다");
    assert!(focus_in_document(cx, handle, &view));
}
