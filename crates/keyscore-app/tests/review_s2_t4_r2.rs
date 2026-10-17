//! S2-T4 적대적 리뷰 2회차의 깨뜨리기 시도 가운데 깨뜨리지 못한 것(통과하는 테스트).
//! 깨진 시도는 `docs/reports/s2-t4/review-2.md` 부록에 있다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Entity, OwnedMenu, OwnedMenuItem, TestAppContext,
};
use keyscore_app::commands::{self, RunCommand};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t4-r2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
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

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    render(cx, handle);
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    render(cx, handle);
}

/// 메뉴 막대의 항목 선택을 흉내 낸다. GPUI는 항목의 Action을 `App::dispatch_action`으로 내고, 그것은 활성 창의
/// `Window::dispatch_action`(포커스 노드)이다. 헤드리스에는 활성 창이 없어 창에 직접 낸다.
fn menu_click(cx: &mut TestAppContext, handle: AnyWindowHandle, action: RunCommand) {
    cx.dispatch_action(handle, action);
    render(cx, handle);
}

fn wrapped(name: &str) -> RunCommand {
    RunCommand {
        name: name.to_owned(),
        args: None,
    }
}

fn palette_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).palette_open())
}

fn mode_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn view_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> View {
    cx.read(|cx| view.read(cx).mode_state().view())
}

fn bare_menu_keys(menus: &[OwnedMenu], cx: &App) -> Vec<String> {
    fn walk(items: &[OwnedMenuItem], cx: &App, bad: &mut Vec<String>) {
        for item in items {
            match item {
                OwnedMenuItem::Action { name, action, .. }
                    if commands::native_key_equivalent(action.as_ref(), cx)
                        .is_some_and(|k| commands::is_bare(&k)) =>
                {
                    bad.push(name.clone());
                }
                OwnedMenuItem::Submenu(menu) => walk(&menu.items, cx, bad),
                _ => {}
            }
        }
    }
    let mut bad = Vec::new();
    for menu in menus {
        walk(&menu.items, cx, &mut bad);
    }
    bad
}

// 감싼 메뉴 항목(배치 뷰 `v`)이 팔레트에 포커스가 있을 때도 창 루트의 처리기에 닿아 실제 Action을 낸다.
#[gpui_kit::test]
fn wrapped_menu_item_runs_while_palette_has_focus(cx: &mut TestAppContext) {
    start(cx, "wrapped-palette");
    let (handle, view) = open(cx);
    let before = view_of(cx, &view);
    press(cx, handle, "cmd-shift-p");
    assert!(palette_open(cx, &view));
    menu_click(cx, handle, wrapped("view::Toggle"));
    assert_ne!(view_of(cx, &view), before);
}

// 감싼 메뉴 항목이 모드 메뉴(PopupMenu)에 포커스가 있을 때도 실행된다.
#[gpui_kit::test]
fn wrapped_menu_item_runs_while_mode_menu_has_focus(cx: &mut TestAppContext) {
    start(cx, "wrapped-mode-menu");
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    assert!(cx.read(|cx| view.read(cx).mode_menu_open()));
    menu_click(cx, handle, wrapped("mode::Lyric"));
    assert_eq!(mode_of(cx, &view), Mode::Lyric);
}

// 감싼 메뉴 항목은 실행 함수를 거치므로 쌓인 카운트를 비운다(결정 C).
#[gpui_kit::test]
fn wrapped_menu_item_clears_pending_count(cx: &mut TestAppContext) {
    start(cx, "wrapped-count");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, _view) = open(cx);
    press(cx, handle, "3");
    assert_eq!(cx.read(keymap::pending_count), Some(3));
    menu_click(cx, handle, wrapped("view::Toggle"));
    assert_eq!(cx.read(keymap::pending_count), None);
}

// 존재하지 않는 Action 이름을 감싼 항목(사용자 키맵으로도 만들 수 있다)은 아무 일도 하지 않고 패닉하지 않는다.
#[gpui_kit::test]
fn wrapped_unknown_action_is_ignored(cx: &mut TestAppContext) {
    start(cx, "wrapped-unknown");
    let (handle, view) = open(cx);
    menu_click(cx, handle, wrapped("nope::Nothing"));
    assert_eq!(mode_of(cx, &view), Mode::Normal);
}

// vim 프리셋으로 바꾸면 편집 메뉴로 옮긴 vim 키(`p`, `P`, `.` 등)가 맨 키가 된다. 메뉴를 다시 걸어 감싼다.
#[gpui_kit::test]
fn preset_switch_rebuilds_menus_without_bare_keys(cx: &mut TestAppContext) {
    start(cx, "preset-bare");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let bad = cx.update(|cx| bare_menu_keys(&menus, cx));
    assert!(bad.is_empty(), "맨 키 등가물: {bad:?}");
    cx.update(|cx| keymap::set_preset(cx, Preset::Default));
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let bad = cx.update(|cx| bare_menu_keys(&menus, cx));
    assert!(bad.is_empty(), "맨 키 등가물: {bad:?}");
}

// 사용자 키맵 핫리로드(`keymap::apply`)로 컨텍스트 없는 맨 키를 걸어도 메뉴가 다시 감싼다.
#[gpui_kit::test]
fn user_contextless_bare_key_is_wrapped_after_reload(cx: &mut TestAppContext) {
    let dir = start(cx, "reload-bare");
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"bindings": {"k": "view::ToggleKeyboard"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let bad = cx.update(|cx| bare_menu_keys(&menus, cx));
    assert!(bad.is_empty(), "맨 키 등가물: {bad:?}");
}

// 팔레트 안(질의 입력)을 눌러도 팔레트는 닫히지 않는다(`on_mouse_down_out`은 밖만 본다).
#[gpui_kit::test]
fn clicking_inside_palette_keeps_it_open(cx: &mut TestAppContext) {
    start(cx, "click-inside");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "zoom");
    // 왼쪽 위 모서리 근처(질의 입력)를 누른다. 가운데는 줄이라 누르면 실행된다.
    cx.update_window(handle, |_, window, cx| {
        window.click_at(
            "command-palette",
            gpui_kit::point(gpui_kit::px(20.), gpui_kit::px(12.)),
            cx,
        );
    })
    .unwrap();
    render(cx, handle);
    assert!(palette_open(cx, &view));
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some("zoom")
    );
}

// 질의를 바꿀 때 강조가 첫 실행 가능 줄로 간다(첫 줄이 실행 가능한 흔한 경우): 「트랙 인스펙터」를 한 글자씩
// 쳐도 Enter가 그 명령을 실행한다.
#[gpui_kit::test]
fn typing_label_char_by_char_runs_that_command(cx: &mut TestAppContext) {
    start(cx, "type-chars");
    let (handle, view) = open(cx);
    let before = cx.read(|cx| view.read(cx).layout().inspector_open);
    press(cx, handle, "cmd-shift-p");
    for ch in "트랙 인스펙터".chars() {
        type_text(cx, handle, &ch.to_string());
    }
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert_ne!(cx.read(|cx| view.read(cx).layout().inspector_open), before);
}

// 맞는 줄이 없는 질의의 Enter는 아무것도 하지 않고 팔레트를 닫지 않는다.
#[gpui_kit::test]
fn enter_with_no_match_does_nothing(cx: &mut TestAppContext) {
    start(cx, "no-match");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "zzzzqqqq");
    press(cx, handle, "enter");
    assert!(palette_open(cx, &view));
    assert_eq!(mode_of(cx, &view), Mode::Normal);
}

// 팔레트의 줄을 누르면 `on_mouse_down_out`이 먼저 닫지 않고 그 줄을 실행한다.
#[gpui_kit::test]
fn clicking_a_palette_row_runs_it(cx: &mut TestAppContext) {
    start(cx, "click-row");
    let (handle, view) = open(cx);
    let before = cx.read(|cx| view.read(cx).layout().inspector_open);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "트랙 인스펙터");
    click(cx, handle, "palette-row-view::ToggleInspector");
    assert!(!palette_open(cx, &view));
    assert_ne!(
        cx.read(|cx| view.read(cx).layout().inspector_open),
        before,
        "줄 클릭이 실행하지 않았다"
    );
}

// 순위 함수는 대소문자와 앞뒤 공백을 무시한다(Action 이름으로 찾기).
#[gpui_kit::test]
fn ranking_ignores_case_and_surrounding_spaces(cx: &mut TestAppContext) {
    start(cx, "rank-case");
    cx.update(|cx| {
        let entries = commands::palette_entries(cx);
        let a = commands::filter_entries(&entries, "VIEW::TOGGLEINSPECTOR");
        let b = commands::filter_entries(&entries, "  view::toggleinspector  ");
        assert_eq!(a, b);
        assert_eq!(entries[a[0]].name(), "view::ToggleInspector");
    });
}
