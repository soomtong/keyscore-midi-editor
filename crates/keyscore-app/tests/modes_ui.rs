//! S2-T3 모드 FSM을 창에 붙인 동작(헤드리스). 키 입력이 컨텍스트를 바꾸고, 바뀐 컨텍스트의
//! 바인딩이 이어서 맞는지 본다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::mode::ShowMenu;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent, ModeState, View};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-modes-{}-{name}", std::process::id()));
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

/// (뷰, 모드, 선택, 고스트)
type Summary = (View, Mode, bool, bool);

fn summary(s: ModeState) -> Summary {
    (s.view(), s.mode(), s.has_selection(), s.has_ghost())
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> String {
    cx.update_window(handle, |_, window, _| {
        window.find(id).label().unwrap_or_default().to_owned()
    })
    .unwrap()
}

// ---------- 7. 키가 모드와 컨텍스트를 움직인다 ----------

#[gpui_kit::test]
fn keys_drive_modes_through_key_contexts(cx: &mut TestAppContext) {
    start(cx, "keys", None);
    let (handle, view) = open(cx);
    assert_eq!(state(cx, &view), ModeState::INITIAL);

    let steps: [(&str, Summary); 12] = [
        // v: 뷰 전환(5.10)
        ("v", (View::PianoRoll, Mode::Normal, false, false)),
        ("v", (View::Arrangement, Mode::Normal, false, false)),
        // 배치 뷰의 enter = clip::Open, 피아노롤의 enter = mode::Step
        ("enter", (View::PianoRoll, Mode::Normal, false, false)),
        ("enter", (View::PianoRoll, Mode::Step, false, false)),
        // ctrl-tab 순환(5.11)
        ("ctrl-tab", (View::PianoRoll, Mode::Sequence, false, false)),
        ("ctrl-tab", (View::PianoRoll, Mode::Play, false, false)),
        ("ctrl-tab", (View::PianoRoll, Mode::Normal, false, false)),
        (
            "ctrl-shift-tab",
            (View::PianoRoll, Mode::Play, false, false),
        ),
        ("ctrl-tab", (View::PianoRoll, Mode::Normal, false, false)),
        // cmd-r Sequence, cmd-k Play, a AI(5.5)
        ("cmd-r", (View::PianoRoll, Mode::Sequence, false, false)),
        (
            "ctrl-shift-tab",
            (View::PianoRoll, Mode::Step, false, false),
        ),
        (
            "ctrl-shift-tab",
            (View::PianoRoll, Mode::Normal, false, false),
        ),
    ];
    for (keys, expected) in steps {
        press(cx, handle, keys);
        assert_eq!(summary(state(cx, &view)), expected, "{keys}");
    }
    press(cx, handle, "cmd-k");
    assert_eq!(state(cx, &view).mode(), Mode::Play);
    // Play 컨텍스트에는 Normal의 v가 없다
    press(cx, handle, "v");
    assert_eq!(
        summary(state(cx, &view)),
        (View::PianoRoll, Mode::Play, false, false)
    );
    press(cx, handle, "ctrl-tab");
    press(cx, handle, "a");
    assert_eq!(state(cx, &view).mode(), Mode::Ai);
    // AI 컨텍스트의 escape = ai::Dismiss(프리셋) → Normal
    press(cx, handle, "escape");
    assert_eq!(
        summary(state(cx, &view)),
        (View::PianoRoll, Mode::Normal, false, false)
    );
}

// ---------- 8. Esc = 통상의 취소(A13 결정), 프리셋 바인딩 그대로 ----------

fn starts() -> Vec<Vec<ModeEvent>> {
    use ModeEvent::{Enter, OpenClip, Select, ShowGhost};
    vec![
        vec![OpenClip, Enter(Mode::Step), Select, ShowGhost],
        vec![Enter(Mode::Step), Select, ShowGhost],
        vec![Enter(Mode::Play), Select],
        vec![Enter(Mode::Sequence)],
        vec![OpenClip, Select],
        vec![Enter(Mode::Ai)],
        vec![Enter(Mode::Lyric)],
        vec![Enter(Mode::Controller), Select],
        vec![Select],
        vec![],
    ]
}

/// Esc 연타가 어느 상태에서든 Normal·선택 없음·고스트 없음으로 수렴하고, 뷰는 바꾸지 않는다.
fn escape_mashing_converges(cx: &mut TestAppContext, name: &str, preset: Preset) {
    start(cx, name, None);
    cx.update(|cx| keymap::set_preset(cx, preset));
    let (handle, view) = open(cx);
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
    for events in starts() {
        event(cx, handle, &view, ModeEvent::Enter(Mode::Normal));
        event(cx, handle, &view, ModeEvent::ClearSelection);
        if state(cx, &view).view() == View::PianoRoll {
            event(cx, handle, &view, ModeEvent::ToggleView);
        }
        assert_eq!(state(cx, &view), ModeState::INITIAL);
        for ev in &events {
            event(cx, handle, &view, *ev);
        }
        let from = state(cx, &view);
        for _ in 0..5 {
            press(cx, handle, "escape");
        }
        assert_eq!(
            summary(state(cx, &view)),
            (from.view(), Mode::Normal, false, false),
            "{name}: {from:?}"
        );
    }
}

#[gpui_kit::test]
fn escape_mashing_converges_with_default_preset(cx: &mut TestAppContext) {
    escape_mashing_converges(cx, "esc-default", Preset::Default);
}

#[gpui_kit::test]
fn escape_mashing_converges_with_vim_preset(cx: &mut TestAppContext) {
    escape_mashing_converges(cx, "esc-vim", Preset::Vim);
}

#[gpui_kit::test]
fn escape_steps_ghost_then_mode_then_selection(cx: &mut TestAppContext) {
    start(cx, "esc-steps", None);
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::OpenClip);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    event(cx, handle, &view, ModeEvent::Select);
    event(cx, handle, &view, ModeEvent::ShowGhost);
    // Step > Ghost가 Step보다 깊어서 이긴다(5.12: 고스트만 지우고 Step에 머문다)
    let expected = [
        (View::PianoRoll, Mode::Step, true, false),
        (View::PianoRoll, Mode::Normal, true, false),
        (View::PianoRoll, Mode::Normal, false, false),
        // 더 닫을 것이 없으면 아무 일도 없다(배치 뷰로 나가지 않는다)
        (View::PianoRoll, Mode::Normal, false, false),
    ];
    for want in expected {
        press(cx, handle, "escape");
        assert_eq!(summary(state(cx, &view)), want);
    }
}

// ---------- 9. 상태줄 모드 표시와 메뉴 ----------

#[gpui_kit::test]
fn status_bar_shows_mode_and_menu_switches_mode(cx: &mut TestAppContext) {
    start(
        cx,
        "status",
        Some(r#"[{"bindings": {"3": "operator::Count(3)"}}]"#),
    );
    let (handle, view) = open(cx);
    assert_eq!(label(cx, handle, "status-mode"), "NORMAL");
    press(cx, handle, "ctrl-tab");
    // S3-T4a: Step 모드 표시는 길이·옥타브를 함께 보인다(REQ 5.1 `STEP ♩ oct4`).
    assert_eq!(label(cx, handle, "status-mode"), "STEP ♩ oct4");

    // 클릭으로 열고 Esc로 닫는다(메뉴의 키는 gpui-component 바인딩)
    cx.update_window(handle, |_, window, cx| window.click("status-mode", cx))
        .unwrap();
    render(cx, handle);
    assert!(cx.read(|cx| view.read(cx).mode_menu_open()));
    press(cx, handle, "escape");
    assert!(!cx.read(|cx| view.read(cx).mode_menu_open()));
    assert_eq!(state(cx, &view).mode(), Mode::Step);

    // Action으로 열고 키보드로 고른다: 첫 항목 Normal
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(ShowMenu), cx);
    })
    .unwrap();
    render(cx, handle);
    assert!(cx.read(|cx| view.read(cx).mode_menu_open()));
    press(cx, handle, "down");
    press(cx, handle, "enter");
    assert!(!cx.read(|cx| view.read(cx).mode_menu_open()));
    assert_eq!(state(cx, &view).mode(), Mode::Normal);
    assert_eq!(label(cx, handle, "status-mode"), "NORMAL");

    // 마우스로 고르면 쌓인 카운트를 비운다(결정 C). 항목 3 = Play
    press(cx, handle, "3");
    assert_eq!(cx.read(keymap::pending_count), Some(3));
    cx.update_window(handle, |_, window, cx| window.click("status-mode", cx))
        .unwrap();
    render(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window
            .within("popup-menu")
            .click(gpui_kit::ElementId::Integer(3), cx);
    })
    .unwrap();
    render(cx, handle);
    assert_eq!(state(cx, &view).mode(), Mode::Play);
    assert_eq!(cx.read(keymap::pending_count), None);
    assert_eq!(label(cx, handle, "status-mode"), "PLAY");
}

// ---------- 10. 컴포넌트 바인딩 보존 ----------

#[gpui_kit::test]
fn component_bindings_survive_keymap_apply(cx: &mut TestAppContext) {
    start(
        cx,
        "component",
        Some(r#"[{"context": "PopupMenu", "bindings": {"escape": "mode::Normal"}}]"#),
    );
    cx.update(|cx| {
        keymap::apply(cx);
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let popup: Vec<_> = keymap
            .bindings()
            .filter(|b| b.predicate().is_some_and(|p| p.to_string() == "PopupMenu"))
            .map(|b| (b.keystrokes()[0].unparse(), b.action().name()))
            .collect();
        assert!(
            popup.contains(&("down".into(), "ui::SelectDown")),
            "{popup:?}"
        );
        // 바탕 레이어가 먼저(낮은 우선순위), 사용자 오버라이드가 뒤
        let escapes: Vec<_> = popup.iter().filter(|(k, _)| k == "escape").collect();
        assert_eq!(
            escapes.last().map(|(_, a)| *a),
            Some("mode::Normal"),
            "{popup:?}"
        );
        assert!(escapes.len() >= 2, "{popup:?}");
    });
}

// ---------- 11. 메인 영역 문구 ----------

#[gpui_kit::test]
fn main_view_placeholder_names_current_view(cx: &mut TestAppContext) {
    start(cx, "main-view", None);
    let (handle, _) = open(cx);
    assert_eq!(label(cx, handle, "main-view"), "배치 뷰");
    press(cx, handle, "v");
    assert_eq!(label(cx, handle, "main-view"), "피아노롤");
}

// ---------- 바탕 레이어의 root::Tab이 창 포커스를 빼앗지 않는다 ----------

#[gpui_kit::test]
fn unbound_tab_keeps_window_focus(cx: &mut TestAppContext) {
    start(cx, "tab-focus", None);
    let (handle, view) = open(cx);
    // Lyric·Sequence 컨텍스트에는 우리 tab 바인딩이 없어 gpui-component의 root::Tab이 맞는다
    for mode in [Mode::Lyric, Mode::Sequence, Mode::Ai] {
        event(cx, handle, &view, ModeEvent::Enter(mode));
        press(cx, handle, "tab");
        press(cx, handle, "shift-tab");
        let focused = cx
            .update_window(handle, |_, window, cx| {
                gpui_kit::Focusable::focus_handle(view.read(cx), cx).is_focused(window)
            })
            .unwrap();
        assert!(focused, "{mode:?}");
        event(cx, handle, &view, ModeEvent::ToNormal);
    }
}
