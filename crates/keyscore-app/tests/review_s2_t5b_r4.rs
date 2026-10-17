//! S2-T5b 적대적 리뷰 4(리뷰 3 지적 한정 확인 리뷰). 깨뜨리지 못한 시도를 남긴다. 깨진 시도는
//! `docs/reports/s2-t5b/review-4.md` 부록에 있다.
//! 키는 `simulate_keystrokes`(키 바인딩을 거침), 클릭은 `window.click`, 메뉴 막대는 포커스로의 Action 디스패치다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{Opt, Real};
use keyscore_app::actions::{edit, track, transport};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, Tempo, Tick, TrackId, ksq};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review4-t5b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    // 시험 창은 활성이 아니면 포커스·Blur 사건이 나지 않는다(gpui window.rs 3416-3426). 실제 앱처럼
    // 활성으로 둔다.
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

fn click_menu_item(cx: &mut TestAppContext, handle: AnyWindowHandle, index: u64) {
    cx.update_window(handle, |_, window, cx| {
        window
            .within("popup-menu")
            .click(gpui_kit::ElementId::Integer(index), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}
fn project_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Project {
    cx.read(|cx| view.read(cx).document().read(cx).project().clone())
}

fn ids(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<TrackId> {
    project_of(cx, view)
        .tracks()
        .iter()
        .map(keyscore_core::Track::id)
        .collect()
}

fn tempo(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Tempo {
    project_of(cx, view).tempo_map().tempo_at(Tick::ZERO)
}

fn shown(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window.find(id).value().map(str::to_owned)
    })
    .unwrap()
}

fn query(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<String> {
    cx.read(|cx| view.read(cx).palette_query(cx))
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

fn set_tempo(bpm: f64) -> transport::SetTempo {
    transport::SetTempo(Opt(Some(Real(bpm))))
}

fn three_tracks() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "A", "channel": 1}, {"name": "B", "channel": 2},
            {"name": "C", "channel": 3}]}"#,
    )
    .unwrap()
    .0
}

fn presets() -> [(&'static str, Preset); 2] {
    [("default", Preset::Default), ("vim", Preset::Vim)]
}

// ---------- 지적 1: 팔레트 질의·트랜스포트 필드의 cmd-z ----------

#[gpui_kit::test]
fn palette_query_cmd_z_and_redo_edit_query_only(cx: &mut TestAppContext) {
    for (name, preset) in presets() {
        start(cx, &format!("pal-{name}"));
        cx.update(|cx| keymap::set_preset(cx, preset));
        let (handle, view) = open(cx, three_tracks());
        run(cx, handle, track::Delete);
        press(cx, handle, "cmd-shift-p x");
        assert_eq!(query(cx, &view).as_deref(), Some("x"), "{name}");
        press(cx, handle, "cmd-z");
        assert_eq!(query(cx, &view).as_deref(), Some(""), "{name}");
        assert_eq!(ids(cx, &view).len(), 2, "{name}");
        press(cx, handle, "cmd-shift-z");
        assert_eq!(query(cx, &view).as_deref(), Some("x"), "{name}");
        assert_eq!(ids(cx, &view).len(), 2, "{name}");
    }
}

/// 질의가 비어도 입력란이 포커스를 가지므로 문서는 그대로다(재귀 없이 끝나야 한다).
#[gpui_kit::test]
fn palette_empty_query_cmd_z_keeps_document_and_terminates(cx: &mut TestAppContext) {
    start(cx, "pal-empty");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    press(cx, handle, "cmd-shift-p cmd-z cmd-shift-z cmd-z");
    assert_eq!(query(cx, &view).as_deref(), Some(""));
    assert_eq!(ids(cx, &view).len(), 2);
}

#[gpui_kit::test]
fn palette_menu_undo_and_redo_edit_query_only(cx: &mut TestAppContext) {
    start(cx, "pal-menu");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    press(cx, handle, "cmd-shift-p a b");
    run(cx, handle, edit::Undo);
    assert_eq!(query(cx, &view).as_deref(), Some(""));
    run(cx, handle, edit::Redo);
    assert_eq!(query(cx, &view).as_deref(), Some("ab"));
    assert_eq!(ids(cx, &view).len(), 2);
}

/// 팔레트를 닫으면 문서 undo가 돌아온다(키·메뉴, 두 프리셋).
#[gpui_kit::test]
fn document_undo_works_after_palette_closed(cx: &mut TestAppContext) {
    for (name, preset) in presets() {
        start(cx, &format!("pal-close-{name}"));
        cx.update(|cx| keymap::set_preset(cx, preset));
        let (handle, view) = open(cx, three_tracks());
        run(cx, handle, track::Delete);
        press(cx, handle, "cmd-shift-p x escape escape");
        press(cx, handle, "cmd-z");
        assert_eq!(ids(cx, &view).len(), 3, "{name}");
        press(cx, handle, "cmd-shift-z");
        assert_eq!(ids(cx, &view).len(), 2, "{name}");
        run(cx, handle, edit::Undo);
        assert_eq!(ids(cx, &view).len(), 3, "{name}");
        run(cx, handle, edit::Redo);
        assert_eq!(ids(cx, &view).len(), 2, "{name}");
    }
}

/// 입력란이 없으면 키·메뉴 모두 문서 undo(두 프리셋).
#[gpui_kit::test]
fn document_undo_without_input_keys_and_menu(cx: &mut TestAppContext) {
    for (name, preset) in presets() {
        start(cx, &format!("doc-{name}"));
        cx.update(|cx| keymap::set_preset(cx, preset));
        let (handle, view) = open(cx, three_tracks());
        run(cx, handle, track::Delete);
        run(cx, handle, track::Delete);
        press(cx, handle, "cmd-z");
        assert_eq!(ids(cx, &view).len(), 2, "{name}");
        run(cx, handle, edit::Undo);
        assert_eq!(ids(cx, &view).len(), 3, "{name}");
        run(cx, handle, edit::Redo);
        press(cx, handle, "cmd-shift-z");
        assert_eq!(ids(cx, &view).len(), 1, "{name}");
    }
}

/// 박자표 필드: 키·메뉴 undo/redo가 필드 글자만 바꾼다.
#[gpui_kit::test]
fn meter_field_key_and_menu_undo_edit_text_only(cx: &mut TestAppContext) {
    start(cx, "meter-undo");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    click(cx, handle, "transport-meter");
    press(cx, handle, "cmd-a 3 / 4");
    press(cx, handle, "cmd-z");
    assert_eq!(ids(cx, &view).len(), 2);
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 2);
    run(cx, handle, edit::Redo);
    run(cx, handle, edit::Redo);
    assert_eq!(ids(cx, &view).len(), 2);
    // 글자 undo를 끝까지 해도 문서는 그대로
    press(cx, handle, "cmd-z cmd-z cmd-z cmd-z cmd-z cmd-z");
    assert_eq!(ids(cx, &view).len(), 2);
}

// ---------- 지적 2: 메뉴 경로 ----------

#[gpui_kit::test]
fn menu_undo_and_redo_in_dirty_tempo_field(cx: &mut TestAppContext) {
    start(cx, "menu-tempo");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    click(cx, handle, "transport-tempo");
    // gpui 입력란은 글자 하나씩 되돌린다
    press(cx, handle, "cmd-a 9 0");
    run(cx, handle, edit::Undo);
    run(cx, handle, edit::Undo);
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("120.0")
    );
    run(cx, handle, edit::Redo);
    run(cx, handle, edit::Redo);
    assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("90"));
    assert_eq!(ids(cx, &view).len(), 2);
    // 확정은 되돌린 뒤에도 정상
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
}

// ---------- 지적 3: 같은 값 메뉴 ----------

#[gpui_kit::test]
fn key_and_port_menu_different_value_still_undoable_same_value_not(cx: &mut TestAppContext) {
    start(cx, "menu-values");
    let (handle, view) = open(cx, three_tracks());
    let n0 = undo_len(cx, &view);
    click(cx, handle, "transport-key");
    click_menu_item(cx, handle, 0);
    assert_eq!(undo_len(cx, &view), n0);
    click(cx, handle, "transport-key");
    click_menu_item(cx, handle, 1);
    assert_eq!(undo_len(cx, &view), n0 + 1);
    // 방금 고른 조성을 다시 고르면 단위가 늘지 않는다
    click(cx, handle, "transport-key");
    click_menu_item(cx, handle, 1);
    assert_eq!(undo_len(cx, &view), n0 + 1);
    run(cx, handle, edit::Undo);
    assert_eq!(undo_len(cx, &view), n0);
    // 포트
    click(cx, handle, "transport-port");
    click_menu_item(cx, handle, 0);
    assert_eq!(undo_len(cx, &view), n0);
    click(cx, handle, "transport-port");
    click_menu_item(cx, handle, 0);
    assert_eq!(undo_len(cx, &view), n0);
}

// ---------- 제안 4: 고친 필드 기준 이동 ----------

#[gpui_kit::test]
fn typed_old_value_after_document_change_commits(cx: &mut TestAppContext) {
    start(cx, "stale-old");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    run(cx, handle, set_tempo(100.0));
    assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("9"));
    press(cx, handle, "cmd-a 1 2 0 . 0 enter");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

#[gpui_kit::test]
fn dirty_field_escape_and_blur_restore_new_document_value(cx: &mut TestAppContext) {
    for how in ["escape", "blur"] {
        start(cx, &format!("restore-{how}"));
        let (handle, view) = open(cx, Project::new());
        click(cx, handle, "transport-tempo");
        press(cx, handle, "cmd-a 9");
        run(cx, handle, set_tempo(100.0));
        assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("9"));
        if how == "escape" {
            press(cx, handle, "escape");
        } else {
            click(cx, handle, "track-list");
        }
        assert_eq!(
            shown(cx, handle, "transport-tempo").as_deref(),
            Some("100.0"),
            "{how}"
        );
        assert_eq!(tempo(cx, &view), Tempo::from_bpm(100).unwrap(), "{how}");
    }
}

/// 새 문서 값과 똑같은 글자를 치고 Enter: 이미 그 값이므로 편집을 만들지 않는다.
#[gpui_kit::test]
fn typing_new_document_value_then_enter_makes_no_unit(cx: &mut TestAppContext) {
    start(cx, "type-new");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    run(cx, handle, set_tempo(100.0));
    let n = undo_len(cx, &view);
    press(cx, handle, "cmd-a 1 0 0 . 0 enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(100).unwrap());
    assert_eq!(undo_len(cx, &view), n);
}

/// 고치지 않은 필드는 따라가고, 고친 필드는 글자를 지키며 서로 독립이다.
#[gpui_kit::test]
fn untouched_follows_dirty_keeps_text_independently(cx: &mut TestAppContext) {
    start(cx, "indep");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-meter");
    press(cx, handle, "cmd-a 3 / 4");
    run(cx, handle, set_tempo(100.0));
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("100.0")
    );
    assert_eq!(shown(cx, handle, "transport-meter").as_deref(), Some("3/4"));
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(100).unwrap());
    assert_eq!(
        keyscore_app::tracks::format_meter(
            project_of(cx, &view).meter_map().signature_at(Tick::ZERO)
        ),
        "3/4"
    );
}
