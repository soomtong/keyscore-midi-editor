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

// ---------- 리뷰 2 부록(지적 1–4): 고친 뒤 회귀 방지로 옮김 ----------

fn mode_menu_is_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_menu_open())
}

// A-1 (지적 1): 첫 줄이 꺼진 질의에서 Tab → Enter가 Tab으로 채운 명령이 아닌 둘째 줄을 실행한다.
// 「sequence」의 첫 줄은 꺼진 `sequence::Note`, 강조는 둘째 줄 「녹음」(mode::Sequence)이다.
#[gpui_kit::test]
fn tab_then_enter_after_disabled_top_row_runs_completed_command(cx: &mut TestAppContext) {
    start(cx, "r2-a1");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "sequence");
    press(cx, handle, "tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some("녹음")
    );
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert_eq!(
        mode_of(cx, &view),
        Mode::Sequence,
        "Tab으로 채운 「녹음」의 Enter가 mode::Sequence를 실행하지 않았다"
    );
}

// A-2 (지적 1): 한 글자 더 치면 강조가 앞 질의의 「첫 실행 가능 줄 위치」에 남는다(한글 질의).
#[gpui_kit::test]
fn typing_one_more_char_highlights_first_runnable_row(cx: &mut TestAppContext) {
    start(cx, "r2-a2");
    let (handle, view) = open(cx);
    let expected = cx.update(|cx| {
        let entries = commands::palette_entries(cx);
        let order = commands::filter_entries(&entries, "카운트인");
        let first = order
            .iter()
            .find(|ix| entries[**ix].action().is_some())
            .unwrap();
        entries[*first].label().to_string()
    });
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "카운트");
    type_text(cx, handle, "인");
    // Tab은 강조된 줄의 이름으로 질의를 채우므로 강조 위치가 드러난다
    press(cx, handle, "tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)),
        Some(expected),
        "「카운트」의 첫 줄(꺼진 operator::Count) 때문에 강조가 둘째 줄에 남았다"
    );
}

// A-3 (지적 2): 팔레트가 열린 채 상태줄 모드 표시를 누르면 모드 메뉴가 열리지만 포커스는 문서로 간다.
// 모드 메뉴는 키(아래·Enter·Esc)를 받지 못하고 Esc로 닫히지 않는다(P2).
#[gpui_kit::test]
fn mode_menu_opened_by_click_while_palette_open_takes_keys(cx: &mut TestAppContext) {
    start(cx, "r2-a3");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    click(cx, handle, "status-mode");
    assert!(!palette_open(cx, &view));
    assert!(mode_menu_is_open(cx, &view));
    press(cx, handle, "escape");
    assert!(
        !mode_menu_is_open(cx, &view),
        "모드 메뉴가 열린 채 남고 Esc로 닫히지 않는다"
    );
}

// A-3b (지적 2): 모드 메뉴가 열린 채 cmd-shift-p로 팔레트를 열면 모드 메뉴가 남는다. 팔레트를 닫은 뒤에도
// 모드 메뉴는 Esc로 닫히지 않는다.
#[gpui_kit::test]
fn opening_palette_over_mode_menu_does_not_orphan_it(cx: &mut TestAppContext) {
    start(cx, "r2-a3b");
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    assert!(mode_menu_is_open(cx, &view));
    press(cx, handle, "cmd-shift-p");
    press(cx, handle, "escape");
    press(cx, handle, "escape");
    assert!(!palette_open(cx, &view));
    assert!(
        !mode_menu_is_open(cx, &view),
        "팔레트를 닫은 뒤에도 모드 메뉴가 남고 Esc로 닫히지 않는다"
    );
}

// A-4 (지적 3): GPUI `create_menu_item`의 수식키 마스크는 cmd·ctrl·alt·shift만 본다. `fn-k`를 컨텍스트 없이
// 걸면 GPUI는 맨 `k`를 키 등가물로 굽는데, `commands::is_bare`는 fn을 수식키로 쳐서 감싸지 않는다.
#[gpui_kit::test]
fn fn_modifier_binding_is_not_baked_as_bare_menu_key(cx: &mut TestAppContext) {
    let dir = start(cx, "r2-a4");
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"bindings": {"fn-k": "view::ToggleKeyboard"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let view_menu = menus.iter().find(|m| m.name.as_ref() == "보기").unwrap();
    let (title, key) = cx.update(|cx| {
        view_menu
            .items
            .iter()
            .find_map(|i| match i {
                OwnedMenuItem::Action { name, action, .. } if name.starts_with("건반 뷰") => {
                    Some((
                        name.clone(),
                        commands::native_key_equivalent(action.as_ref(), cx),
                    ))
                }
                _ => None,
            })
            .unwrap()
    });
    // GPUI가 실제로 굽는 마스크(cmd·ctrl·alt·shift)로 본 맨 키
    let baked_bare = key.is_some_and(|k| {
        let m = k.modifiers;
        !(m.platform || m.control || m.alt) && k.key.len() == 1
    });
    assert!(!baked_bare, "「{title}」에 맨 키 k가 키 등가물로 구워진다");
}

// A-5 (지적 4): UI-11. REQ가 눈에 보이는 자리를 주지 않은 vim 마크(5.8.1 `m`, `` ` ``)는 지어낸 자리 대신 결정
// 요청이 있어야 한다. 「커서(클릭으로도 옮긴다)」는 마크를 설정하지도, 이름 붙은 마크로 가지도 않는다.
#[test]
fn marks_have_a_visible_place_or_a_decision_request() {
    let decisions = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/decisions");
    let text: String = std::fs::read_dir(decisions)
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap_or_default())
        .collect();
    let missing: Vec<String> = ["cursor::SetMark", "cursor::JumpMark"]
        .iter()
        .filter_map(|name| match commands::spec(name).unwrap().place() {
            commands::Place::Elsewhere { detail, .. } if !text.contains(name) => {
                Some(format!("{name} ({detail})"))
            }
            _ => None,
        })
        .collect();
    assert!(missing.is_empty(), "{}건: {:?}", missing.len(), missing);
}
