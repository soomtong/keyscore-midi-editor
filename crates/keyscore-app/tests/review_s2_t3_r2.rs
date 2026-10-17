//! S2-T3 적대적 리뷰 2회차: 리뷰 1 대응(aee4fbf)을 깨뜨려 본다.
//!
//! 이 파일에는 통과하는(깨뜨리지 못한) 시도만 회귀 방지로 남긴다. 실패하는 시도(지적 사항)는
//! `docs/reports/s2-t3/review-2.md` 부록에 코드로 있다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Entity, Focusable as _, TestAppContext, px, size,
};
use keyscore_app::actions::mode::ShowMenu;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent, ModeState, View};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t3r2-{}-{name}", std::process::id()));
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

fn show_menu(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(ShowMenu), cx);
    })
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

/// (메뉴, 모드 표시, 창 크기)의 경계.
fn menu_geometry(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
) -> (
    gpui_kit::Bounds<gpui_kit::Pixels>,
    gpui_kit::Bounds<gpui_kit::Pixels>,
    gpui_kit::Size<gpui_kit::Pixels>,
) {
    cx.update_window(handle, |_, window, _| {
        (
            window.find("popup-menu").bounds(),
            window.find("status-mode").bounds(),
            window.viewport_size(),
        )
    })
    .unwrap()
}

// ---------- 지적 2 대응: 컨텍스트 겹 바깥의 모드 메뉴 ----------

#[gpui_kit::test]
fn mode_menu_sits_above_indicator_and_follows_resize(cx: &mut TestAppContext) {
    start(cx, "geom", None);
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    assert!(menu_open(cx, &view));
    for _ in 0..2 {
        let (menu, indicator, viewport) = menu_geometry(cx, handle);
        // 메뉴는 창 안에 있고, 모드 표시를 가리지 않으며, 모드 표시 바로 위에 붙는다.
        assert!(menu.origin.y >= px(0.), "{menu:?}");
        assert!(menu.bottom() <= viewport.height, "{menu:?} {viewport:?}");
        assert!(
            menu.bottom() <= indicator.top() + px(1.),
            "{menu:?} {indicator:?}"
        );
        assert!(
            indicator.top() - menu.bottom() < px(8.),
            "{menu:?} {indicator:?}"
        );
        assert!(
            menu.left() <= indicator.left() + px(8.),
            "{menu:?} {indicator:?}"
        );
        cx.simulate_window_resize(handle, size(px(900.), px(420.)));
        render(cx, handle);
        assert!(menu_open(cx, &view), "창 크기 변경이 메뉴를 닫았다");
    }
}

#[gpui_kit::test]
fn mode_menu_open_keeps_mode_keys_out_in_every_mode(cx: &mut TestAppContext) {
    // 메뉴가 열린 채 그 모드의 컨텍스트 키(Step의 space·tab, Play의 tab, Normal의 v·a·w)가
    // 메뉴 뒤로 새지 않는다.
    start(cx, "keys-every-mode", None);
    let (handle, view) = open(cx);
    for mode in [Mode::Normal, Mode::Step, Mode::Play, Mode::Lyric] {
        event(cx, handle, &view, ModeEvent::Enter(mode));
        let before = state(cx, &view);
        show_menu(cx, handle);
        assert!(menu_open(cx, &view));
        for key in ["v", "a", "w", "shift-c", "cmd-r", "cmd-k", "space", "1"] {
            press(cx, handle, key);
            assert_eq!(state(cx, &view), before, "{mode:?}에서 메뉴 뒤로 {key}");
            assert!(menu_open(cx, &view), "{mode:?}에서 {key}가 메뉴를 닫았다");
        }
        press(cx, handle, "escape");
        assert!(!menu_open(cx, &view));
        assert!(window_focused(cx, handle, &view));
    }
}

#[gpui_kit::test]
fn mode_menu_open_normal_context_cmd_keys_stay_out(cx: &mut TestAppContext) {
    // `cmd-i`(view::ToggleInspector)는 `Normal` 컨텍스트 바인딩이다. 처리기는 메뉴의 조상(루트)에
    // 있지만 바인딩이 맞지 않아야 한다.
    start(cx, "cmd-i", None);
    let (handle, view) = open(cx);
    let before = cx.read(|cx| view.read(cx).layout().inspector_open);
    click(cx, handle, "status-mode");
    press(cx, handle, "cmd-i");
    assert_eq!(cx.read(|cx| view.read(cx).layout().inspector_open), before);
    assert!(menu_open(cx, &view));
}

#[gpui_kit::test]
fn mode_menu_open_tab_then_escape_returns_focus(cx: &mut TestAppContext) {
    // 메뉴 포커스에서 `tab`은 바탕 레이어의 `Root` 바인딩(root::Tab)에 닿는다. 그 뒤에도 메뉴를
    // 닫을 수 있고 창이 포커스를 되찾는다.
    start(cx, "tab", None);
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    press(cx, handle, "tab");
    assert_eq!(state(cx, &view), ModeState::INITIAL);
    press(cx, handle, "escape");
    assert!(!menu_open(cx, &view));
    assert!(window_focused(cx, handle, &view));
    press(cx, handle, "v");
    assert_eq!(state(cx, &view).view(), View::PianoRoll);
}

#[gpui_kit::test]
fn mode_menu_click_on_track_list_closes_and_returns_focus(cx: &mut TestAppContext) {
    // 메뉴 자리가 바뀌었으니 바깥 클릭을 다른 영역(트랙 목록, 트랜스포트)으로 다시 본다.
    start(cx, "outside-2", None);
    let (handle, view) = open(cx);
    for outside in ["track-list", "transport", "inspector"] {
        click(cx, handle, "status-mode");
        assert!(menu_open(cx, &view));
        click(cx, handle, outside);
        assert!(!menu_open(cx, &view), "{outside} 클릭");
        assert!(window_focused(cx, handle, &view), "{outside} 클릭");
    }
}

#[gpui_kit::test]
fn mode_menu_pick_keeps_view_and_selection(cx: &mut TestAppContext) {
    // 메뉴로 모드를 고르는 경로도 「모드만 바뀐다」(지적 1 대응)를 따른다.
    start(cx, "menu-pick", None);
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::Select);
    click(cx, handle, "status-mode");
    // 첫 down이 첫 항목(NORMAL)을 고른다. 두 번이면 STEP.
    press(cx, handle, "down");
    press(cx, handle, "down");
    press(cx, handle, "enter");
    let s = state(cx, &view);
    assert_eq!(s.mode(), Mode::Step);
    assert_eq!(s.view(), View::Arrangement);
    assert!(s.has_selection());
    assert!(window_focused(cx, handle, &view));
}

// ---------- 지적 3 대응: LeaveAi ----------

#[test]
fn leave_ai_only_leaves_ai_and_keeps_view_and_selection() {
    for s in ModeState::all() {
        let next = s.apply(ModeEvent::LeaveAi);
        if s.mode() == Mode::Ai {
            assert_eq!(next.mode(), Mode::Normal, "{s:?}");
            assert_eq!(next.view(), s.view(), "{s:?}");
            assert_eq!(next.has_selection(), s.has_selection(), "{s:?}");
        } else {
            assert_eq!(next, s, "{s:?}");
        }
    }
}

#[gpui_kit::test]
fn ai_accept_and_dismiss_leave_ai_by_action(cx: &mut TestAppContext) {
    use keyscore_app::actions::ai;
    start(cx, "ai-leave", None);
    let (handle, view) = open(cx);
    for action in [
        Box::new(ai::Accept) as Box<dyn gpui_kit::Action>,
        Box::new(ai::Dismiss),
    ] {
        press(cx, handle, "a");
        assert_eq!(state(cx, &view).mode(), Mode::Ai);
        cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
            .unwrap();
        render(cx, handle);
        assert_eq!(state(cx, &view), ModeState::INITIAL);
    }
}

// ---------- 제안 1 대응: install 두 번 ----------

#[gpui_kit::test]
fn install_twice_keeps_one_copy_of_each_layer(cx: &mut TestAppContext) {
    let dir = start(cx, "install-twice-copies", None);
    let count = |cx: &mut TestAppContext, keys: &'static str, name: &'static str| {
        cx.update(|cx| {
            let km = cx.key_bindings();
            let km = km.borrow();
            km.bindings()
                .filter(|b| {
                    b.action().name() == name
                        && b.keystrokes()
                            .iter()
                            .map(gpui_kit::KeybindingKeystroke::unparse)
                            .collect::<Vec<_>>()
                            .join(" ")
                            == keys
                })
                .count()
        })
    };
    let ctrl_tab = count(cx, "ctrl-tab", "mode::Cycle");
    let popup_escape = cx.update(|cx| {
        keymap::base_bindings(cx)
            .iter()
            .filter(|b| b.predicate().is_some_and(|p| p.to_string() == "PopupMenu"))
            .count()
    });
    cx.update(|cx| {
        keymap::install(cx, dir.join("keymap.json"), keymap::options(cx), |_| {});
    });
    assert_eq!(count(cx, "ctrl-tab", "mode::Cycle"), ctrl_tab);
    let popup_after = cx.update(|cx| {
        keymap::base_bindings(cx)
            .iter()
            .filter(|b| b.predicate().is_some_and(|p| p.to_string() == "PopupMenu"))
            .count()
    });
    assert_eq!(popup_after, popup_escape);
    assert!(
        cx.update(|cx| !keymap::base_bindings(cx)
            .iter()
            .any(|b| b.action().name() == "mode::Cycle")),
        "우리 레이어가 바탕 레이어에 섞였다"
    );
}

#[gpui_kit::test]
fn install_twice_count_prefix_counts_once(cx: &mut TestAppContext) {
    // install은 카운트 처리기와 키 관찰자를 다시 단다. 두 번 달린 처리기가 숫자를 두 번 세지 않는다.
    let dir = start(cx, "install-twice-count", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    cx.update(|cx| {
        keymap::install(cx, dir.join("keymap.json"), keymap::options(cx), |_| {});
    });
    let (handle, _view) = open(cx);
    press(cx, handle, "3");
    assert_eq!(cx.update(|cx| keymap::pending_count(cx)), Some(3));
}

// ---------- 메뉴와 대기 중인 시퀀스 ----------

#[gpui_kit::test]
fn pending_sequence_does_not_cross_into_mode_menu(cx: &mut TestAppContext) {
    // `g`를 누른 채 메뉴를 열면 대기 중인 `g`가 메뉴 포커스로 넘어가 `p`와 맞으면 안 된다.
    start(
        cx,
        "pend",
        Some(r#"[{"context": "Normal", "bindings": {"g p": "mode::Play"}}]"#),
    );
    let (handle, view) = open(cx);
    press(cx, handle, "g");
    click(cx, handle, "status-mode");
    press(cx, handle, "p");
    assert_eq!(state(cx, &view), ModeState::INITIAL);
    assert!(menu_open(cx, &view));
    press(cx, handle, "escape");
    press(cx, handle, "p");
    assert_eq!(state(cx, &view), ModeState::INITIAL);
}

// ---------- 리뷰 2 부록 A1(지적 1): 결정 2026-10-17-mode-entry-view 뒤 회귀 방지로 옮김 ----------

#[gpui_kit::test]
fn lyric_and_controller_from_arrangement_open_in_piano_roll(cx: &mut TestAppContext) {
    // REQ 5.15 「커서 위 노트부터 시작」「피아노롤은 가사 입력 중인 노트를 강조」,
    // 5.9 Controller 모드 = 「피아노롤 아래」 레인을 화면 전체로 키운 상태.
    start(cx, "arr-lyric", None);
    let (handle, view) = open(cx);
    for (key, mode) in [("w", Mode::Lyric), ("shift-c", Mode::Controller)] {
        event(cx, handle, &view, ModeEvent::ToNormal);
        if state(cx, &view).view() == View::PianoRoll {
            event(cx, handle, &view, ModeEvent::ToggleView);
        }
        assert_eq!(state(cx, &view), ModeState::INITIAL);
        press(cx, handle, key);
        let s = state(cx, &view);
        assert_eq!(s.mode(), mode);
        assert_eq!(s.view(), View::PianoRoll, "{key}: 배치 뷰에서 {mode:?}");
        // Esc는 Normal로 돌아가고 뷰는 그대로(A13)
        press(cx, handle, "escape");
        assert_eq!(state(cx, &view).mode(), Mode::Normal);
        assert_eq!(state(cx, &view).view(), View::PianoRoll);
    }
}
