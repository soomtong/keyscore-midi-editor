//! S2-T5b 적대적 리뷰 2(트랜스포트 바와 트랙 목록). 깨뜨리지 못한 시도와 진단을 남긴다.
//! 키는 `simulate_keystrokes`(키 바인딩을 거침)로 친다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, track, transport};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::Mode;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, Tempo, Tick, TrackId, ksq};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review2-t5b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
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

fn current(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<TrackId> {
    cx.read(|cx| view.read(cx).current_track())
}

fn tempo(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Tempo {
    project_of(cx, view).tempo_map().tempo_at(Tick::ZERO)
}

fn mode(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn muted(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, index: usize) -> bool {
    project_of(cx, view).tracks()[index].mute()
}

fn shown_tempo(cx: &mut TestAppContext, handle: AnyWindowHandle) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window.find("transport-tempo").value().map(str::to_owned)
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

fn three_tracks() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "A", "channel": 1}, {"name": "B", "channel": 2},
            {"name": "C", "channel": 3}]}"#,
    )
    .unwrap()
    .0
}

// ---------- 입력란의 글자·숫자 키(두 프리셋) ----------

/// 기본·vim 프리셋 모두 입력란에 친 글자·숫자가 텍스트가 되고 모드 키로 빠지지 않는다.
#[gpui_kit::test]
fn letters_and_digits_stay_in_field_both_presets(cx: &mut TestAppContext) {
    for (name, preset) in [("default", Preset::Default), ("vim", Preset::Vim)] {
        start(cx, &format!("letters-{name}"));
        cx.update(|cx| keymap::set_preset(cx, preset));
        let (handle, view) = open(cx, three_tracks());
        let before = ids(cx, &view);
        click(cx, handle, "transport-tempo");
        // m(뮤트), o(솔로·vim 새 트랙), t(템포), enter 전의 숫자
        press(cx, handle, "cmd-a backspace m o t x 1 2");
        assert_eq!(shown_tempo(cx, handle).as_deref(), Some("motx12"), "{name}");
        assert_eq!(ids(cx, &view), before, "{name}");
        assert!(!muted(cx, &view, 0), "{name}");
        assert_eq!(mode(cx, &view), Mode::Normal, "{name}");
        press(cx, handle, "cmd-a backspace 1 0 0 enter");
        assert_eq!(tempo(cx, &view), Tempo::from_bpm(100).unwrap(), "{name}");
        // Enter 뒤 포커스는 문서로 돌아오고 모드 키가 산다(기본 `m` 뮤트, vim `o` 새 트랙)
        assert!(!tempo_focused(cx, handle, &view), "{name}");
        if preset == Preset::Vim {
            press(cx, handle, "o");
            assert_eq!(ids(cx, &view).len(), 4, "{name}");
        } else {
            press(cx, handle, "m");
            assert!(muted(cx, &view, 0), "{name}");
        }
    }
}

/// 입력란에서 `cmd-a`·`cmd-c`·`cmd-v`가 입력란에 닿는다.
#[gpui_kit::test]
fn cmd_a_c_v_in_tempo_field(cx: &mut TestAppContext) {
    start(cx, "cmd-acv");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a cmd-c right cmd-v");
    assert_eq!(shown_tempo(cx, handle).as_deref(), Some("120.0120.0"));
    let _ = view;
}

// ---------- 버튼·메뉴 뒤 포커스 ----------

/// 문서에 포커스가 있을 때 트랜스포트 버튼을 눌러도 모드 키(m)가 산다.
#[gpui_kit::test]
fn buttons_keep_document_focus(cx: &mut TestAppContext) {
    start(cx, "buttons-focus");
    let (handle, view) = open(cx, three_tracks());
    for id in [
        "transport-play",
        "transport-stop",
        "transport-panic",
        "transport-tap",
    ] {
        click(cx, handle, id);
    }
    press(cx, handle, "m");
    assert!(muted(cx, &view, 0));
}

/// 조성 메뉴를 열고 고른 뒤에도 모드 키가 산다.
#[gpui_kit::test]
fn key_menu_returns_focus(cx: &mut TestAppContext) {
    start(cx, "key-menu-focus");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-key");
    cx.update_window(handle, |_, window, cx| {
        window
            .within("popup-menu")
            .click(gpui_kit::ElementId::Integer(3), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "m");
    assert!(muted(cx, &view, 0));
}

// ---------- 필드 동기화 ----------

/// 입력 중(글자를 친 뒤) 다른 경로로 문서가 바뀌면 입력 중인 글자는 덮이지 않는다.
#[gpui_kit::test]
fn typed_text_survives_document_change(cx: &mut TestAppContext) {
    start(cx, "typed-survives");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace 9 5");
    run(
        cx,
        handle,
        transport::SetTempo(keyscore_app::actions::args::Opt(Some(
            keyscore_app::actions::args::Real(100.0),
        ))),
    );
    assert_eq!(shown_tempo(cx, handle).as_deref(), Some("95"));
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(95).unwrap());
}

// ---------- 현재 트랙 이웃 ----------

/// 가운데 트랙을 지우고(이웃 C), undo/redo를 오가도 현재 트랙은 문서에 있고 이웃 자리다.
#[gpui_kit::test]
fn current_track_through_delete_undo_redo(cx: &mut TestAppContext) {
    start(cx, "neighbour-cycle");
    let (handle, view) = open(cx, three_tracks());
    let t = ids(cx, &view);
    run(cx, handle, track::Next);
    run(cx, handle, track::Delete);
    assert_eq!(current(cx, &view), Some(t[2]));
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view), t);
    assert!(current(cx, &view).is_some_and(|c| t.contains(&c)));
    run(cx, handle, edit::Redo);
    assert!(current(cx, &view).is_some_and(|c| ids(cx, &view).contains(&c)));
    // 마지막 트랙에서 새 트랙 둘 → 둘 다 undo → 원래 마지막 트랙
    run(cx, handle, keyscore_app::actions::cursor::LastTrack);
    let last = *ids(cx, &view).last().unwrap();
    run(cx, handle, track::NewBelow);
    run(cx, handle, track::NewBelow);
    run(cx, handle, edit::Undo);
    run(cx, handle, edit::Undo);
    assert_eq!(current(cx, &view), Some(last));
}

/// `t`(키보드)로 템포 입력란에 들어가 새 값을 치고 Enter. 두 프리셋.
#[gpui_kit::test]
fn t_key_enters_tempo_field_and_commits(cx: &mut TestAppContext) {
    for (name, preset) in [("default", Preset::Default), ("vim", Preset::Vim)] {
        start(cx, &format!("t-key-{name}"));
        cx.update(|cx| keymap::set_preset(cx, preset));
        let (handle, view) = open(cx, three_tracks());
        press(cx, handle, "t");
        assert!(tempo_focused(cx, handle, &view), "{name}");
        press(cx, handle, "8 5 enter");
        assert_eq!(tempo(cx, &view), Tempo::from_bpm(85).unwrap(), "{name}");
        assert!(!tempo_focused(cx, handle, &view), "{name}");
    }
}

/// 입력란에서 `ctrl-tab`(전역 모드 순환)은 모드를 돌린다. 기록용.
#[gpui_kit::test]
fn ctrl_tab_in_tempo_field_cycles_mode(cx: &mut TestAppContext) {
    start(cx, "ctrl-tab-exact");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "ctrl-tab");
    assert_eq!(mode(cx, &view), Mode::Step);
}

/// 박자표 입력란도 실제 키로 친다(vim 숫자, `/`).
#[gpui_kit::test]
fn vim_meter_field_with_keys(cx: &mut TestAppContext) {
    start(cx, "vim-meter");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-meter");
    press(cx, handle, "cmd-a backspace 6 / 8 enter");
    let sig = project_of(cx, &view).meter_map().signature_at(Tick::ZERO);
    assert_eq!(keyscore_app::tracks::format_meter(sig), "6/8");
}
