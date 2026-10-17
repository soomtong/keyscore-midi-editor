//! S2-T3 적대적 리뷰 3회차: A13 결정 반영(4925281)과 모드 진입 뷰 결정을 깨뜨려 본다.
//!
//! 이 파일에는 통과하는(깨뜨리지 못한) 시도만 회귀 방지로 남긴다. 실패하는 시도(지적 사항)는
//! `docs/reports/s2-t3/review-3.md` 부록에 코드로 있다.

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, Focusable as _, TestAppContext};
use keyscore_app::actions::ALL_ACTION_NAMES;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent, ModeState, View};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t3r3-{}-{name}", std::process::id()));
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

fn summary(s: ModeState) -> (View, Mode, bool, bool) {
    (s.view(), s.mode(), s.has_selection(), s.has_ghost())
}

/// 어떤 상태에서든 시작 상태로 되돌린다(키를 쓰지 않는다).
fn reset(cx: &mut TestAppContext, handle: AnyWindowHandle, view: &Entity<ProjectWindow>) {
    event(cx, handle, view, ModeEvent::ToNormal);
    event(cx, handle, view, ModeEvent::ClearSelection);
    if state(cx, view).view() == View::PianoRoll {
        event(cx, handle, view, ModeEvent::ToggleView);
    }
    assert_eq!(state(cx, view), ModeState::INITIAL);
}

/// 시작 상태에서 `target`까지 사건으로 간다.
fn reach(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    target: ModeState,
) {
    reset(cx, handle, view);
    if target.view() == View::PianoRoll {
        event(cx, handle, view, ModeEvent::OpenClip);
    }
    if target.mode() != Mode::Normal {
        event(cx, handle, view, ModeEvent::Enter(target.mode()));
    }
    if target.has_selection() {
        event(cx, handle, view, ModeEvent::Select);
    }
    if target.has_ghost() {
        event(cx, handle, view, ModeEvent::ShowGhost);
    }
    assert_eq!(state(cx, view), target);
}

/// A13 결정 해석 표(`docs/decisions/2026-10-17-keymap-conflicts.md`)대로 `Esc` 한 번의 결과.
fn a13_once(s: ModeState) -> (View, Mode, bool, bool) {
    let (v, m, sel, ghost) = summary(s);
    match m {
        Mode::Step if ghost => (v, m, sel, false),
        Mode::Normal => (v, m, false, false),
        _ => (v, Mode::Normal, sel, false),
    }
}

// ---------- A13: 모든 상태에서 Esc 한 번, 연타, 뷰·레이아웃 불변 ----------

fn escape_once_in_every_state(cx: &mut TestAppContext, name: &str, preset: Preset) {
    start(cx, name, None);
    cx.update(|cx| keymap::set_preset(cx, preset));
    let (handle, view) = open(cx);
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
    let layout = cx.read(|cx| view.read(cx).layout());
    let all: Vec<ModeState> = ModeState::all().collect();
    assert_eq!(all.len(), 28);
    for target in all {
        reach(cx, handle, &view, target);
        press(cx, handle, "escape");
        let once = summary(state(cx, &view));
        assert_eq!(once, a13_once(target), "{preset:?} {target:?}: Esc 한 번");
        // 연타: 세 번 안에 Normal·선택 없음·고스트 없음, 그 뒤로는 아무 일도 없다
        for _ in 0..5 {
            press(cx, handle, "escape");
        }
        assert_eq!(
            summary(state(cx, &view)),
            (target.view(), Mode::Normal, false, false),
            "{preset:?} {target:?}: 연타"
        );
        assert!(window_focused(cx, handle, &view), "{preset:?} {target:?}");
        assert_eq!(cx.read(|cx| view.read(cx).layout()), layout, "{target:?}");
    }
}

#[gpui_kit::test]
fn escape_once_matches_a13_table_in_every_state_default(cx: &mut TestAppContext) {
    escape_once_in_every_state(cx, "a13-default", Preset::Default);
}

#[gpui_kit::test]
fn escape_once_matches_a13_table_in_every_state_vim(cx: &mut TestAppContext) {
    escape_once_in_every_state(cx, "a13-vim", Preset::Vim);
}

#[gpui_kit::test]
fn vim_layer_does_not_change_any_escape_binding(cx: &mut TestAppContext) {
    // vim 오버레이가 escape를 가리거나(null) 다른 Action으로 바꾸는 곳이 없다.
    fn escape_bindings(cx: &mut TestAppContext) -> BTreeSet<(String, String)> {
        cx.update(|cx| {
            let km = cx.key_bindings();
            let km = km.borrow();
            km.bindings()
                .filter(|b| {
                    b.keystrokes()
                        .iter()
                        .any(|k| gpui_kit::KeybindingKeystroke::unparse(k) == "escape")
                })
                .map(|b| {
                    (
                        b.predicate().map(|p| p.to_string()).unwrap_or_default(),
                        b.action().name().to_string(),
                    )
                })
                .collect()
        })
    }
    start(cx, "vim-escape", None);
    let default = escape_bindings(cx);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let vim = escape_bindings(cx);
    assert_eq!(default, vim);
    for ctx in ["Step", "Sequence", "Play", "Lyric", "Controller", "AI"] {
        assert!(
            vim.contains(&(ctx.to_string(), "mode::Normal".to_string()))
                || (ctx == "AI" && vim.contains(&(ctx.to_string(), "ai::Dismiss".to_string()))),
            "{ctx}: {vim:?}"
        );
    }
    assert!(
        !vim.iter().any(|(c, _)| c == "Normal"
            || c == "Normal > Arrangement"
            || c == "Normal > PianoRoll"),
        "선택 없는 Normal에 escape가 걸렸다: {vim:?}"
    );
}

// ---------- A13: 모드 메뉴(패널)가 열려 있으면 Esc는 메뉴만 닫는다 ----------

#[gpui_kit::test]
fn escape_with_mode_menu_open_closes_only_the_menu(cx: &mut TestAppContext) {
    start(cx, "menu-escape", None);
    let (handle, view) = open(cx);
    for target in ModeState::all() {
        reach(cx, handle, &view, target);
        click(cx, handle, "status-mode");
        assert!(menu_open(cx, &view), "{target:?}");
        press(cx, handle, "escape");
        assert!(!menu_open(cx, &view), "{target:?}");
        assert_eq!(state(cx, &view), target, "메뉴를 닫는 Esc가 상태도 바꿨다");
        assert!(window_focused(cx, handle, &view), "{target:?}");
        // 다음 Esc는 표대로 한 단계
        press(cx, handle, "escape");
        assert_eq!(summary(state(cx, &view)), a13_once(target), "{target:?}");
    }
}

// ---------- A13: 카운트(vim)와 Esc ----------

#[gpui_kit::test]
fn vim_count_then_escape_clears_count_and_does_one_thing(cx: &mut TestAppContext) {
    start(cx, "vim-count-escape", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::Select);
    press(cx, handle, "3");
    assert_eq!(cx.update(|cx| keymap::pending_count(cx)), Some(3));
    press(cx, handle, "escape");
    assert_eq!(cx.update(|cx| keymap::pending_count(cx)), None);
    assert_eq!(state(cx, &view), ModeState::INITIAL);
    press(cx, handle, "3");
    press(cx, handle, "escape");
    assert_eq!(cx.update(|cx| keymap::pending_count(cx)), None);
    assert_eq!(state(cx, &view), ModeState::INITIAL);
}

// ---------- A13: 다른 영역을 누른 뒤에도 Esc가 입력 모드를 나간다 ----------

#[gpui_kit::test]
fn escape_leaves_input_mode_after_clicking_other_areas(cx: &mut TestAppContext) {
    start(cx, "click-escape", None);
    let (handle, view) = open(cx);
    for area in ["track-list", "transport", "inspector", "main-view"] {
        reset(cx, handle, &view);
        press(cx, handle, "ctrl-tab");
        assert_eq!(state(cx, &view).mode(), Mode::Step);
        click(cx, handle, area);
        press(cx, handle, "escape");
        assert_eq!(state(cx, &view).mode(), Mode::Normal, "{area} 클릭 뒤 Esc");
    }
}

// ---------- 모드 진입 뷰 결정: 실제 키 ----------

#[gpui_kit::test]
fn entry_view_rule_by_real_keys(cx: &mut TestAppContext) {
    // Step·Sequence·Play·AI는 배치 뷰 그대로(선택도 그대로), Lyric·Controller는 피아노롤(선택 비움).
    start(cx, "entry-keys", None);
    let (handle, view) = open(cx);
    for (key, mode, view_after, keeps_sel) in [
        ("ctrl-tab", Mode::Step, View::Arrangement, true),
        ("cmd-r", Mode::Sequence, View::Arrangement, true),
        ("cmd-k", Mode::Play, View::Arrangement, true),
        ("a", Mode::Ai, View::Arrangement, true),
        ("w", Mode::Lyric, View::PianoRoll, false),
        ("shift-c", Mode::Controller, View::PianoRoll, false),
    ] {
        reset(cx, handle, &view);
        event(cx, handle, &view, ModeEvent::Select);
        press(cx, handle, key);
        assert_eq!(
            summary(state(cx, &view)),
            (view_after, mode, keeps_sel, false),
            "{key}"
        );
        // Esc는 뷰를 되돌리지 않는다(A13)
        press(cx, handle, "escape");
        assert_eq!(
            summary(state(cx, &view)),
            (view_after, Mode::Normal, keeps_sel, false),
            "{key} 뒤 Esc"
        );
    }
    // vim `i`도 Ctrl+Tab과 같은 규칙
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    reset(cx, handle, &view);
    press(cx, handle, "i");
    assert_eq!(
        summary(state(cx, &view)),
        (View::Arrangement, Mode::Step, false, false)
    );
}

#[test]
fn lyric_and_controller_never_leave_piano_roll() {
    // 어느 상태에서 어떤 사건을 몇 번 넣어도 Lyric·Controller는 피아노롤에 있다.
    for s in ModeState::all() {
        for a in ModeEvent::all() {
            for b in ModeEvent::all() {
                let n = s.apply(a).apply(b);
                if matches!(n.mode(), Mode::Lyric | Mode::Controller) {
                    assert_eq!(n.view(), View::PianoRoll, "{s:?} {a:?} {b:?}");
                }
            }
        }
    }
}

// ---------- mode::Back 제거의 파급 ----------

#[gpui_kit::test]
fn mode_back_is_gone_and_old_user_keymap_reports_one_error(cx: &mut TestAppContext) {
    assert!(!ALL_ACTION_NAMES.contains(&"mode::Back"));
    start(
        cx,
        "old-back",
        Some(
            r#"[{"context": "Normal", "bindings": {"escape": "mode::Back", "ctrl-x": "mode::Play"}}]"#,
        ),
    );
    let (handle, view) = open(cx);
    let errors = cx.update(|cx| keymap::errors(cx).len());
    assert_eq!(errors, 1);
    // 같은 파일의 다른 바인딩은 살아 있고, 선택 없는 Normal의 Esc는 여전히 아무 일도 없다.
    event(cx, handle, &view, ModeEvent::OpenClip);
    press(cx, handle, "escape");
    assert_eq!(
        summary(state(cx, &view)),
        (View::PianoRoll, Mode::Normal, false, false)
    );
    press(cx, handle, "ctrl-x");
    assert_eq!(state(cx, &view).mode(), Mode::Play);
}
