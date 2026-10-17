//! S2-T3 적대적 리뷰 1회차: 모드 FSM을 창에 붙인 동작을 깨뜨려 본다.
//!
//! 이 파일에는 통과하는(깨뜨리지 못한) 시도만 회귀 방지로 남긴다. 실패하는 시도(지적 사항)는
//! `docs/reports/s2-t3/review-1.md` 부록에 코드로 있다.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, Focusable as _, TestAppContext};
use keyscore_app::actions::mode::ShowMenu;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent, ModeState, View};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx
        .update(|cx| open_project_window(Project::new(), cx))
        .unwrap();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn state(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> ModeState {
    cx.read(|cx| view.read(cx).mode_state())
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}

fn event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    ev: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| this.apply_mode_event(ev, window, cx));
    })
    .unwrap();
    render(cx, handle);
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    render(cx, handle);
}

fn menu_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_menu_open())
}

fn window_focused(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> bool {
    cx.update_window(handle, |_, window, cx| {
        view.read(cx).focus_handle(cx).is_focused(window)
    })
    .unwrap()
}

fn tick(cx: &mut TestAppContext) {
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
}

// ---------- 모드 메뉴: 닫힘과 포커스 복귀 ----------

#[gpui_kit::test]
fn mode_menu_outside_click_closes_and_returns_focus(cx: &mut TestAppContext) {
    start(cx, "outside", None);
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    assert!(menu_open(cx, &view));
    click(cx, handle, "main-view");
    assert!(!menu_open(cx, &view));
    assert!(window_focused(cx, handle, &view));
    press(cx, handle, "v");
    assert_eq!(state(cx, &view).view(), View::PianoRoll);
}

#[gpui_kit::test]
fn mode_menu_opened_twice_closes_with_one_escape(cx: &mut TestAppContext) {
    start(cx, "twice", None);
    let (handle, view) = open(cx);
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| {
            window.dispatch_action(Box::new(ShowMenu), cx);
        })
        .unwrap();
        render(cx, handle);
    }
    assert!(menu_open(cx, &view));
    press(cx, handle, "escape");
    assert!(!menu_open(cx, &view));
    assert!(window_focused(cx, handle, &view));
    press(cx, handle, "v");
    assert_eq!(state(cx, &view).view(), View::PianoRoll);
}

#[gpui_kit::test]
fn mode_indicator_clicked_twice_leaves_window_usable(cx: &mut TestAppContext) {
    start(cx, "click-twice", None);
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    click(cx, handle, "status-mode");
    press(cx, handle, "escape");
    assert!(!menu_open(cx, &view));
    assert!(window_focused(cx, handle, &view));
}

#[gpui_kit::test]
fn mode_menu_keeps_working_after_context_depth_change(cx: &mut TestAppContext) {
    start(cx, "menu-depth", None);
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    // 메뉴가 열린 채 컨텍스트 깊이가 2에서 1로 바뀐다(Normal > Arrangement → Step)
    press(cx, handle, "ctrl-tab");
    assert_eq!(state(cx, &view).mode(), Mode::Step);
    assert!(menu_open(cx, &view));
    press(cx, handle, "down");
    press(cx, handle, "enter");
    assert!(!menu_open(cx, &view));
    assert_eq!(state(cx, &view).mode(), Mode::Normal);
    assert!(window_focused(cx, handle, &view));
}

// ---------- 컨텍스트 깊이 변화와 대기 중인 시퀀스 ----------

#[gpui_kit::test]
fn pending_sequence_survives_context_depth_change(cx: &mut TestAppContext) {
    start(
        cx,
        "seq",
        Some(r#"[{"context": "Normal", "bindings": {"g p": "mode::Play"}}]"#),
    );
    let (handle, view) = open(cx);
    press(cx, handle, "g");
    // 키가 아닌 경로(S5 클립 끝, S3 선택)로 깊이가 2 → 2 → 3으로 바뀐다
    event(cx, handle, &view, ModeEvent::OpenClip);
    event(cx, handle, &view, ModeEvent::Select);
    press(cx, handle, "p");
    assert_eq!(state(cx, &view).mode(), Mode::Play);
}

#[gpui_kit::test]
fn normal_selection_predicate_matches_through_view_context(cx: &mut TestAppContext) {
    // 프리셋의 `Normal > Selection`(A13 대기)은 경로 Normal > Arrangement > Selection에서도 맞아야 한다
    start(
        cx,
        "sel",
        Some(r#"[{"context": "Normal > Selection", "bindings": {"escape": "select::Clear"}}]"#),
    );
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::Select);
    press(cx, handle, "escape");
    assert!(!state(cx, &view).has_selection());
    event(cx, handle, &view, ModeEvent::OpenClip);
    event(cx, handle, &view, ModeEvent::Select);
    press(cx, handle, "escape");
    assert!(!state(cx, &view).has_selection());
}

// ---------- 바탕 레이어 ----------

#[gpui_kit::test]
fn base_layer_survives_preset_switch_and_hot_reload(cx: &mut TestAppContext) {
    let dir = start(cx, "base", None);
    let _ = open(cx);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"bindings": {"f9": "mode::Play"}}]"#,
    )
    .unwrap();
    tick(cx);
    cx.update(|cx| {
        let km = cx.key_bindings();
        let km = km.borrow();
        let popup = km
            .bindings()
            .filter(|b| b.predicate().is_some_and(|p| p.to_string() == "PopupMenu"))
            .count();
        assert!(popup >= 6, "{popup}");
        assert!(
            km.bindings()
                .any(|b| b.action().name() == "mode::Play" && b.keystrokes()[0].unparse() == "f9")
        );
    });
}

#[gpui_kit::test]
fn user_global_tab_beats_base_root_tab(cx: &mut TestAppContext) {
    // 컨텍스트 없는 사용자 바인딩이 더 깊은 `Root` 컨텍스트의 바탕 바인딩에 지지 않는다
    start(
        cx,
        "root-tab",
        Some(r#"[{"bindings": {"tab": "mode::Play"}}]"#),
    );
    let (handle, view) = open(cx);
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
    event(cx, handle, &view, ModeEvent::Enter(Mode::Lyric));
    press(cx, handle, "tab");
    assert_eq!(state(cx, &view).mode(), Mode::Play);
}

// ---------- 리뷰 1 부록(지적 1–3, 제안 1): 고친 뒤 회귀 방지로 옮김 ----------

#[test]
fn cycle_round_trip_restores_state() {
    for start in ModeState::all().filter(|s| s.mode() == Mode::Normal) {
        let mut s = start;
        for _ in 0..4 {
            s = s.apply(ModeEvent::Cycle);
        }
        assert_eq!(s, start, "Ctrl+Tab 한 바퀴");
        for _ in 0..4 {
            s = s.apply(ModeEvent::CycleBack);
        }
        assert_eq!(s, start, "Ctrl+Shift+Tab 한 바퀴");
    }
}

#[gpui_kit::test]
fn ctrl_tab_round_trip_restores_state(cx: &mut TestAppContext) {
    start(cx, "cycle-ui", None);
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::Select);
    let before = state(cx, &view);
    for _ in 0..4 {
        press(cx, handle, "ctrl-tab");
    }
    assert_eq!(state(cx, &view), before);
}

#[gpui_kit::test]
fn mode_menu_open_keeps_document_keys_out(cx: &mut TestAppContext) {
    start(cx, "menu-keys", None);
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    assert!(menu_open(cx, &view));
    press(cx, handle, "v");
    assert_eq!(
        state(cx, &view),
        ModeState::INITIAL,
        "메뉴가 열린 채 v가 뷰를 바꿨다"
    );
    press(cx, handle, "a");
    assert_eq!(
        state(cx, &view).mode(),
        Mode::Normal,
        "메뉴가 열린 채 a가 AI로 갔다"
    );
    assert!(menu_open(cx, &view));
}

#[gpui_kit::test]
fn ai_accept_dismiss_outside_ai_keep_mode(cx: &mut TestAppContext) {
    use keyscore_app::actions::ai;
    start(cx, "ai-outside", None);
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Play));
    for action in [
        Box::new(ai::Dismiss) as Box<dyn gpui_kit::Action>,
        Box::new(ai::Accept),
    ] {
        cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
            .unwrap();
        render(cx, handle);
        assert_eq!(state(cx, &view).mode(), Mode::Play);
    }
}

#[gpui_kit::test]
fn install_twice_does_not_freeze_user_layer(cx: &mut TestAppContext) {
    let dir = start(
        cx,
        "install-twice",
        Some(r#"[{"bindings": {"f9": "mode::Play"}}]"#),
    );
    let path = dir.join("keymap.json");
    cx.update(|cx| keymap::install(cx, path.clone(), keymap::options(cx), |_| {}));
    std::fs::remove_file(&path).unwrap();
    cx.update(keymap::apply);
    cx.update(|cx| {
        let km = cx.key_bindings();
        let km = km.borrow();
        assert!(
            !km.bindings()
                .any(|b| b.action().name() == "mode::Play" && b.keystrokes()[0].unparse() == "f9"),
            "지운 사용자 바인딩이 남았다"
        );
    });
}
