//! S2-T5b 적대적 리뷰 3(트랜스포트 바와 트랙 목록). 깨뜨리지 못한 시도를 남긴다. 깨진 시도는
//! `docs/reports/s2-t5b/review-3.md` 부록에 있다.
//! 키는 `simulate_keystrokes`(키 바인딩을 거침), 클릭은 실제 마우스 사건(`window.click`)이다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{Opt, Real};
use keyscore_app::actions::{edit, track, transport};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::Mode;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, Tempo, Tick, TrackId, ksq};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review3-t5b-{}-{name}", std::process::id()));
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

fn meter(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    keyscore_app::tracks::format_meter(project_of(cx, view).meter_map().signature_at(Tick::ZERO))
}

fn mode(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn muted(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, index: usize) -> bool {
    project_of(cx, view).tracks()[index].mute()
}

fn shown(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window.find(id).value().map(str::to_owned)
    })
    .unwrap()
}

fn tempo_focused(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> bool {
    cx.update_window(handle, |_, window, cx| {
        view.read(cx).transport_tempo_focused(window, cx)
    })
    .unwrap()
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

// ---------- 리뷰 2 지적 2: 입력란 cmd-z / cmd-shift-z (KM-14) ----------

/// 입력란의 `cmd-z`는 입력란 글자를 되돌리고 `cmd-shift-z`는 다시 한다. 문서는 그대로다.
#[gpui_kit::test]
fn cmd_z_and_cmd_shift_z_edit_field_text_only(cx: &mut TestAppContext) {
    start(cx, "field-undo-redo");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    let after_delete = ids(cx, &view);
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("9"));
    press(cx, handle, "cmd-z");
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("120.0")
    );
    assert_eq!(ids(cx, &view), after_delete);
    press(cx, handle, "cmd-shift-z");
    assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("9"));
    assert_eq!(ids(cx, &view), after_delete);
    // 되돌려 원래 글자가 되면 「고치지 않은 필드」라 Enter가 편집을 만들지 않는다
    press(cx, handle, "cmd-z enter");
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 3);
}

/// 입력란 포커스가 없으면 `cmd-z`는 문서를 되돌린다(Enter·Esc 뒤).
#[gpui_kit::test]
fn cmd_z_after_leaving_field_undoes_document(cx: &mut TestAppContext) {
    start(cx, "undo-after-leave");
    let (handle, view) = open(cx, three_tracks());
    press(cx, handle, "t 9 0 enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
    press(cx, handle, "cmd-z");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("120.0")
    );
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 7 escape cmd-shift-z");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
}

// ---------- 리뷰 2 지적 4: Esc ----------

/// 고친 입력란의 `Esc`는 입력을 버리고 문서로 돌아간다(두 프리셋, 두 필드). 그 뒤 모드 키가 산다.
#[gpui_kit::test]
fn escape_cancels_both_fields_both_presets(cx: &mut TestAppContext) {
    for (name, preset) in [("default", Preset::Default), ("vim", Preset::Vim)] {
        start(cx, &format!("esc-{name}"));
        cx.update(|cx| keymap::set_preset(cx, preset));
        let (handle, view) = open(cx, three_tracks());
        press(cx, handle, "t 9 9 escape");
        assert_eq!(tempo(cx, &view), Tempo::DEFAULT, "{name}");
        assert_eq!(
            shown(cx, handle, "transport-tempo").as_deref(),
            Some("120.0"),
            "{name}"
        );
        assert!(!tempo_focused(cx, handle, &view), "{name}");
        click(cx, handle, "transport-meter");
        press(cx, handle, "cmd-a 3 / 4 escape");
        assert_eq!(meter(cx, &view), "4/4", "{name}");
        assert_eq!(
            shown(cx, handle, "transport-meter").as_deref(),
            Some("4/4"),
            "{name}"
        );
        assert_eq!(mode(cx, &view), Mode::Normal, "{name}");
        if preset == Preset::Vim {
            press(cx, handle, "o");
            assert_eq!(ids(cx, &view).len(), 4, "{name}");
        } else {
            press(cx, handle, "m");
            assert!(muted(cx, &view, 0), "{name}");
        }
    }
}

/// 수식키 붙은 `Esc`(`shift-escape`)는 가로채지 않는다. 입력란에 머문다.
#[gpui_kit::test]
fn shift_escape_is_not_cancel(cx: &mut TestAppContext) {
    start(cx, "shift-esc");
    let (handle, view) = open(cx, three_tracks());
    press(cx, handle, "t 9 9 shift-escape");
    assert!(tempo_focused(cx, handle, &view));
    assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("99"));
}

// ---------- 리뷰 2 지적 3·5: 고친 필드 판단, 같은 값 ----------

/// 고쳤지만 문서와 같은 값(`120`)의 Enter는 편집을 만들지 않는다. 박자표도 같다.
#[gpui_kit::test]
fn same_value_commit_makes_no_undo_unit(cx: &mut TestAppContext) {
    start(cx, "same-value");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 1 2 0 enter");
    click(cx, handle, "transport-meter");
    press(cx, handle, "cmd-a 4 / 4 enter");
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 3);
}

/// 고치지 않은 필드는 포커스가 있어도 문서 변경(`SetTempo`)을 따라가고, 그 뒤 고친 값은 확정된다.
#[gpui_kit::test]
fn untouched_focused_field_follows_then_commits_typed(cx: &mut TestAppContext) {
    start(cx, "follow-then-type");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    run(cx, handle, set_tempo(100.0));
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("100.0")
    );
    press(cx, handle, "cmd-a 1 2 0 enter");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

/// 박자표를 고치는 동안 템포가 바뀌면 템포 필드는 바로 따라간다(리뷰 2 제안 2).
#[gpui_kit::test]
fn tempo_field_follows_while_meter_dirty(cx: &mut TestAppContext) {
    start(cx, "per-field");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-meter");
    press(cx, handle, "cmd-a 3");
    run(cx, handle, set_tempo(100.0));
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("100.0")
    );
    assert_eq!(shown(cx, handle, "transport-meter").as_deref(), Some("3"));
    press(cx, handle, "/ 4 enter");
    assert_eq!(meter(cx, &view), "3/4");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(100).unwrap());
}

// ---------- 리뷰 2 지적 1: 버튼·토글·메뉴 뒤 포커스 ----------

/// 메트로놈을 켜고 끈 뒤, 그리고 고치던 입력란에서 메트로놈을 누른 뒤에도 모드 키가 산다.
#[gpui_kit::test]
fn metronome_twice_and_from_field_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "metronome-twice");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-metronome");
    click(cx, handle, "transport-metronome");
    assert!(!cx.read(|cx| view.read(cx).metronome_on()));
    press(cx, handle, "m");
    assert!(muted(cx, &view, 0));
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    click(cx, handle, "transport-metronome");
    assert!(cx.read(|cx| view.read(cx).metronome_on()));
    assert!(!tempo_focused(cx, handle, &view));
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("120.0")
    );
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    press(cx, handle, "m");
    assert!(!muted(cx, &view, 0));
}

/// 입력란에 있다가 재생·정지·패닉·탭 버튼을 눌러도 포커스가 문서로 와서 모드 키가 산다.
#[gpui_kit::test]
fn buttons_from_field_return_to_document(cx: &mut TestAppContext) {
    start(cx, "buttons-from-field");
    let (handle, view) = open(cx, three_tracks());
    for id in [
        "transport-play",
        "transport-stop",
        "transport-panic",
        "transport-tap",
    ] {
        click(cx, handle, "transport-tempo");
        click(cx, handle, id);
        assert!(!tempo_focused(cx, handle, &view), "{id}");
    }
    press(cx, handle, "m");
    assert!(muted(cx, &view, 0));
}

/// 꺼진 버튼(녹음·AI)을 눌러도 포커스가 버튼에 남지 않는다.
#[gpui_kit::test]
fn disabled_buttons_keep_document_focus(cx: &mut TestAppContext) {
    start(cx, "disabled-buttons");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-record");
    click(cx, handle, "transport-ai");
    assert_eq!(mode(cx, &view), Mode::Normal);
    press(cx, handle, "m");
    assert!(muted(cx, &view, 0));
}

/// 포트 메뉴에서 고른 뒤에도 모드 키가 산다.
#[gpui_kit::test]
fn port_menu_returns_focus(cx: &mut TestAppContext) {
    start(cx, "port-menu-focus");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-port");
    click_menu_item(cx, handle, 0);
    press(cx, handle, "m");
    assert!(muted(cx, &view, 0));
}

// ---------- 리뷰 2 제안 1: 모드 변경 ----------

/// 고치던 입력란에서 `ctrl-tab`: 모드가 바뀌고, 입력은 버려지고, 포커스는 문서로 간다.
#[gpui_kit::test]
fn ctrl_tab_from_dirty_field_cancels_and_leaves(cx: &mut TestAppContext) {
    start(cx, "ctrl-tab-dirty");
    let (handle, view) = open(cx, three_tracks());
    press(cx, handle, "t 9 0 ctrl-tab");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert!(!tempo_focused(cx, handle, &view));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("120.0")
    );
    // Step에서 Esc는 Normal로(A13). 키가 문서에 닿는다
    press(cx, handle, "escape");
    assert_eq!(mode(cx, &view), Mode::Normal);
}

/// 입력란에서 모드 메뉴로 모드를 바꿔도 포커스가 입력란으로 돌아가지 않는다.
#[gpui_kit::test]
fn mode_menu_from_field_leaves_field(cx: &mut TestAppContext) {
    start(cx, "mode-menu-from-field");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    click(cx, handle, "status-mode");
    click_menu_item(cx, handle, 1);
    assert_ne!(mode(cx, &view), Mode::Normal);
    assert!(!tempo_focused(cx, handle, &view));
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

// ---------- 포커스를 잃은 필드(활성 창) ----------

/// 고치던 입력란에서 다른 곳(버튼·다른 입력란·트랙 목록)을 누르면 입력은 버려지고 필드는 문서 값을 보인다.
/// 비활성 시험 창에서는 Blur가 나지 않아 이 경로가 돌지 않는다(`open`이 창을 활성으로 두는 이유).
#[gpui_kit::test]
fn leaving_dirty_field_by_click_reverts_it(cx: &mut TestAppContext) {
    for target in [
        "transport-tap",
        "transport-metronome",
        "transport-meter",
        "transport-play",
        "track-list",
    ] {
        start(cx, &format!("leave-{target}"));
        let (handle, view) = open(cx, three_tracks());
        click(cx, handle, "transport-tempo");
        press(cx, handle, "cmd-a 9");
        click(cx, handle, target);
        assert!(!tempo_focused(cx, handle, &view), "{target}");
        assert_eq!(
            shown(cx, handle, "transport-tempo").as_deref(),
            Some("120.0"),
            "{target}"
        );
        assert_eq!(tempo(cx, &view), Tempo::DEFAULT, "{target}");
    }
}

// ---------- 리뷰 3 부록 C(지적 1-3, 제안 4) ----------

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

#[gpui_kit::test]
fn cmd_z_in_palette_query_does_not_undo_document(cx: &mut TestAppContext) {
    start(cx, "palette-cmd-z");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    press(cx, handle, "cmd-shift-p x");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some("x")
    );
    press(cx, handle, "cmd-z");
    assert_eq!(
        ids(cx, &view).len(),
        2,
        "팔레트 질의의 cmd-z가 문서를 되돌렸다"
    );
}

#[gpui_kit::test]
fn menu_undo_in_dirty_field_undoes_field_text(cx: &mut TestAppContext) {
    start(cx, "menu-undo-field");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    // 메뉴 막대는 포커스로 `edit::Undo`를 디스패치한다
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 2);
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("120.0"),
        "메뉴 되돌리기가 입력란 글자를 되돌리지 않는다"
    );
    run(cx, handle, edit::Redo);
    assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("9"));
    assert_eq!(ids(cx, &view).len(), 2);
}

#[gpui_kit::test]
fn key_menu_same_key_makes_no_undo_unit(cx: &mut TestAppContext) {
    start(cx, "key-same");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    let before = undo_len(cx, &view);
    click(cx, handle, "transport-key");
    click_menu_item(cx, handle, 0); // 지금 조성(조성 없음)
    assert_eq!(undo_len(cx, &view), before);
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 3);
}

#[gpui_kit::test]
fn port_menu_same_port_makes_no_undo_unit(cx: &mut TestAppContext) {
    start(cx, "port-same");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    let before = undo_len(cx, &view);
    click(cx, handle, "transport-port");
    click_menu_item(cx, handle, 0); // 지금 포트(가상 포트)
    assert_eq!(undo_len(cx, &view), before);
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 3);
}

/// 제안 4. 고치던 필드가 있는 동안 문서 값이 바뀌어도, 옛 값과 같은 글자를 친 확정은 적용된다.
#[gpui_kit::test]
fn typed_old_value_after_document_change_commits(cx: &mut TestAppContext) {
    start(cx, "stale-shown");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    run(cx, handle, set_tempo(100.0));
    press(cx, handle, "cmd-a 1 2 0 . 0 enter");
    assert_eq!(
        tempo(cx, &view),
        Tempo::DEFAULT,
        "친 120.0이 확정되지 않았다"
    );
}
