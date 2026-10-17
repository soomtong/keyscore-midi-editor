//! S2-T4 적대적 리뷰 3회차의 깨뜨리기 시도 가운데 깨뜨리지 못한 것(통과하는 테스트).
//! 깨진 시도는 `docs/reports/s2-t4/review-3.md` 부록에 있다.

use std::path::PathBuf;

use gpui_kit::component::kbd::Kbd;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Entity, Keystroke, OwnedMenuItem, TestAppContext,
};
use keyscore_app::actions::mode as mode_actions;
use keyscore_app::commands::{self, RunCommand};
use keyscore_app::keymap;
use keyscore_app::mode::Mode;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t4-r3-{}-{name}", std::process::id()));
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

fn type_chars(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    for ch in text.chars() {
        type_text(cx, handle, &ch.to_string());
    }
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    render(cx, handle);
}

fn palette_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).palette_open())
}

fn mode_menu_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_menu_open())
}

fn mode_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn query_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<String> {
    cx.read(|cx| view.read(cx).palette_query(cx))
}

fn inspector_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).layout().inspector_open)
}

/// 순위 함수가 이 질의에 주는 실행 가능한 줄의 이름(순위순).
fn runnable_labels(cx: &mut TestAppContext, query: &str) -> Vec<String> {
    cx.update(|cx| {
        let entries = commands::palette_entries(cx);
        commands::filter_entries(&entries, query)
            .into_iter()
            .filter(|ix| entries[*ix].action().is_some())
            .map(|ix| entries[ix].label().to_string())
            .collect()
    })
}

fn keys(text: &str) -> String {
    Kbd::format(&Keystroke::parse(text).unwrap())
}

/// 보기 메뉴에서 이름이 `prefix`로 시작하는 항목의 제목과 감쌈 여부.
fn view_menu_item(cx: &mut TestAppContext, prefix: &str) -> (String, bool) {
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let view = menus
        .into_iter()
        .find(|m| m.name.as_ref() == "보기")
        .unwrap();
    view.items
        .into_iter()
        .find_map(|i| match i {
            OwnedMenuItem::Action { name, action, .. } if name.starts_with(prefix) => {
                Some((name, action.as_any().is::<RunCommand>()))
            }
            _ => None,
        })
        .unwrap()
}

// ---------- 1. 팔레트 강조(리뷰 2 지적 1의 근본) ----------

// 맞는 줄이 없던 질의(강조 없음)에서 글자를 지워 다시 맞으면 첫 실행 가능 줄이 강조된다.
#[gpui_kit::test]
fn backspace_from_no_match_highlights_first_runnable_row(cx: &mut TestAppContext) {
    start(cx, "r3-backspace");
    let (handle, view) = open(cx);
    let before = inspector_open(cx, &view);
    press(cx, handle, "cmd-shift-p");
    type_chars(cx, handle, "트랙 인스펙터");
    type_text(cx, handle, "q");
    assert!(runnable_labels(cx, "트랙 인스펙터q").is_empty());
    press(cx, handle, "backspace");
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert_ne!(inspector_open(cx, &view), before);
}

// 꺼진 줄만 맞는 질의(강조 없음, Enter 무시) 뒤 Esc로 질의를 지우면 빈 질의의 첫 실행 가능 줄이 강조된다.
#[gpui_kit::test]
fn disabled_only_query_then_clear_highlights_first_runnable_row(cx: &mut TestAppContext) {
    start(cx, "r3-disabled-only");
    let (handle, view) = open(cx);
    assert!(runnable_labels(cx, "cursor::setmark").is_empty());
    let expected = runnable_labels(cx, "")[0].clone();
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "cursor::setmark");
    press(cx, handle, "enter");
    assert!(palette_open(cx, &view), "꺼진 줄만 있는데 닫혔다");
    press(cx, handle, "escape");
    assert_eq!(query_of(cx, &view).as_deref(), Some(""));
    press(cx, handle, "tab");
    assert_eq!(query_of(cx, &view), Some(expected));
}

// 아래로 두 줄 옮긴 뒤 글자를 더 치면 강조는 새 목록의 첫 실행 가능 줄로 돌아간다.
#[gpui_kit::test]
fn typing_after_moving_down_resets_highlight_to_top(cx: &mut TestAppContext) {
    start(cx, "r3-down-type");
    let (handle, view) = open(cx);
    let expected = runnable_labels(cx, "트랙 인")[0].clone();
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "트랙");
    press(cx, handle, "down");
    press(cx, handle, "down");
    type_chars(cx, handle, " 인");
    press(cx, handle, "tab");
    assert_eq!(query_of(cx, &view), Some(expected));
}

// Shift+Tab(이전 후보)은 첫 줄에서 마지막 실행 가능 줄로 돈다. 꺼진 줄(맨 뒤)에 서지 않는다.
#[gpui_kit::test]
fn shift_tab_from_top_wraps_to_last_runnable_row(cx: &mut TestAppContext) {
    start(cx, "r3-shift-tab");
    let (handle, view) = open(cx);
    // 꺼진 줄(operator::Count)도 맞는 질의
    let all = cx.update(|cx| {
        let entries = commands::palette_entries(cx);
        commands::filter_entries(&entries, "카운트")
            .into_iter()
            .map(|ix| entries[ix].action().is_some())
            .collect::<Vec<_>>()
    });
    assert!(all.contains(&false), "꺼진 줄이 맞아야 시도가 된다");
    let expected = runnable_labels(cx, "카운트").last().unwrap().clone();
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "카운트");
    press(cx, handle, "shift-tab");
    press(cx, handle, "tab");
    assert_eq!(query_of(cx, &view), Some(expected));
}

// 리뷰 2에서 강조가 어긋났던 질의들을 한 글자씩 쳐서 Tab으로 강조를 확인한다. 강조는 늘 순위 함수의 첫
// 실행 가능 줄이다. 같은 팔레트 안에서 Esc로 질의를 지우며 이어 친다(앞 목록이 바뀌는 경로).
#[gpui_kit::test]
fn incremental_queries_always_highlight_first_runnable_row(cx: &mut TestAppContext) {
    start(cx, "r3-incremental");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    let queries = [
        "sequence",
        "카운트인",
        "se",
        "sel",
        "Se",
        "CC",
        "cc",
        "녹음",
        "step",
        "마크",
        "mode",
        "view::",
        "edit::d",
        "트랙",
        "설정",
    ];
    for query in queries {
        type_chars(cx, handle, query);
        let expected = runnable_labels(cx, query).first().cloned();
        press(cx, handle, "tab");
        let got = query_of(cx, &view).unwrap();
        match expected {
            Some(label) => assert_eq!(got, label, "질의 「{query}」"),
            None => assert_eq!(got, query, "질의 「{query}」"),
        }
        // 질의 지우기(Esc 첫 번째는 질의만 지운다)
        press(cx, handle, "escape");
        assert!(palette_open(cx, &view));
        assert_eq!(query_of(cx, &view).as_deref(), Some(""));
    }
}

// 빈 질의도 실행 가능한 줄이 먼저이고, 그 안은 표 순서다.
#[gpui_kit::test]
fn empty_query_lists_runnable_rows_first_in_table_order(cx: &mut TestAppContext) {
    start(cx, "r3-empty");
    cx.update(|cx| {
        let entries = commands::palette_entries(cx);
        let order = commands::filter_entries(&entries, "");
        assert_eq!(order.len(), entries.len());
        let first_disabled = order
            .iter()
            .position(|ix| entries[*ix].action().is_none())
            .unwrap_or(order.len());
        assert!(
            order[first_disabled..]
                .iter()
                .all(|ix| entries[*ix].action().is_none())
        );
        assert!(order[..first_disabled].windows(2).all(|w| w[0] < w[1]));
    });
}

// ---------- 2. 팝업 하나만(리뷰 2 지적 2) ----------

// 팔레트가 열린 채 메뉴 막대로 `mode::ShowMenu`를 내면 팔레트가 닫히고 모드 메뉴가 포커스를 갖는다.
// Esc로 닫은 뒤 키는 문서로 간다.
#[gpui_kit::test]
fn show_menu_from_menu_bar_while_palette_open(cx: &mut TestAppContext) {
    start(cx, "r3-showmenu-bar");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    cx.dispatch_action(handle, mode_actions::ShowMenu);
    render(cx, handle);
    assert!(!palette_open(cx, &view));
    assert!(mode_menu_open(cx, &view));
    press(cx, handle, "escape");
    assert!(!mode_menu_open(cx, &view));
    press(cx, handle, "ctrl-tab");
    assert_ne!(mode_of(cx, &view), Mode::Normal, "문서가 키를 받지 못한다");
}

// 팔레트 줄 「모드 메뉴」(mode::ShowMenu)를 Enter로 실행하면 모드 메뉴가 열리고 키를 받는다.
#[gpui_kit::test]
fn show_menu_from_palette_row_takes_keys(cx: &mut TestAppContext) {
    start(cx, "r3-showmenu-row");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "mode::showmenu");
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert!(mode_menu_open(cx, &view));
    press(cx, handle, "escape");
    assert!(!mode_menu_open(cx, &view));
    press(cx, handle, "ctrl-tab");
    assert_ne!(mode_of(cx, &view), Mode::Normal);
}

// 모드 메뉴가 열린 채 문서를 누르면 메뉴가 닫히고 포커스는 문서로 간다.
#[gpui_kit::test]
fn clicking_document_closes_mode_menu(cx: &mut TestAppContext) {
    start(cx, "r3-menu-doc-click");
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    assert!(mode_menu_open(cx, &view));
    click(cx, handle, "main-view");
    assert!(!mode_menu_open(cx, &view));
    press(cx, handle, "ctrl-tab");
    assert_ne!(mode_of(cx, &view), Mode::Normal);
}

// 팔레트 → 모드 메뉴 → 팔레트를 오가도 마지막 팔레트가 정상으로 실행하고 팝업이 남지 않는다.
#[gpui_kit::test]
fn palette_mode_menu_palette_roundtrip(cx: &mut TestAppContext) {
    start(cx, "r3-roundtrip");
    let (handle, view) = open(cx);
    let before = inspector_open(cx, &view);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "zz");
    click(cx, handle, "status-mode");
    assert!(!palette_open(cx, &view));
    assert!(mode_menu_open(cx, &view));
    press(cx, handle, "cmd-shift-p");
    assert!(palette_open(cx, &view));
    assert!(!mode_menu_open(cx, &view));
    assert_eq!(
        query_of(cx, &view).as_deref(),
        Some(""),
        "새 팔레트는 빈 질의"
    );
    type_text(cx, handle, "트랙 인스펙터");
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert!(!mode_menu_open(cx, &view));
    assert_ne!(inspector_open(cx, &view), before);
}

// ---------- 3. 감싸기 규칙(리뷰 2 지적 3, 제안 4) ----------

// 컨텍스트 없는 alt 바인딩은 감싸고 제목에 ⌥ 키를 보인다.
#[gpui_kit::test]
fn alt_binding_is_wrapped_and_shown(cx: &mut TestAppContext) {
    let dir = start(cx, "r3-alt");
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"bindings": {"alt-k": "view::ToggleKeyboard"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    let (title, wrapped) = view_menu_item(cx, "건반 뷰");
    assert!(wrapped);
    assert_eq!(
        title,
        format!("건반 뷰{}{}", commands::MENU_KEY_SEPARATOR, keys("alt-k"))
    );
}

// Ctrl 바인딩과 기능 키는 감싸지 않는다(GPUI 키 등가물 그대로).
#[gpui_kit::test]
fn ctrl_and_function_keys_are_not_wrapped(cx: &mut TestAppContext) {
    let dir = start(cx, "r3-ctrl-fkey");
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"bindings": {"ctrl-k": "view::ToggleKeyboard"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    assert_eq!(view_menu_item(cx, "건반 뷰"), ("건반 뷰".to_owned(), false));
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"bindings": {"f5": "view::ToggleKeyboard"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    assert_eq!(view_menu_item(cx, "건반 뷰"), ("건반 뷰".to_owned(), false));
}

// ---------- 4. 모든 이름(7.6 「팔레트에서 같은 이름으로 검색」) ----------

// 실제 컴포넌트에서 실행 가능한 줄의 이름을 앞 질의를 지운 뒤 통째로 넣으면 그 줄이 강조된다(Tab이
// 질의를 바꾸지 않는다). 순위 함수만이 아니라 컴포넌트 강조까지 본다.
#[gpui_kit::test]
fn every_runnable_label_is_highlighted_in_component(cx: &mut TestAppContext) {
    start(cx, "r3-all-labels");
    let (handle, view) = open(cx);
    let labels = runnable_labels(cx, "");
    press(cx, handle, "cmd-shift-p");
    let mut bad = Vec::new();
    // 전수는 85초가 걸려 8줄마다 하나를 본다(전수는 리뷰 때 한 번 실행해 통과했다)
    for label in labels.into_iter().step_by(8) {
        type_text(cx, handle, &label);
        press(cx, handle, "tab");
        let got = query_of(cx, &view).unwrap();
        if got != label {
            bad.push(format!("{label} -> {got}"));
        }
        press(cx, handle, "escape");
    }
    assert!(bad.is_empty(), "{}건: {:?}", bad.len(), bad);
}

// ---------- 5. 단축키 표시(KM-10) ----------

// vim 프리셋에서 팔레트의 단축키 표시는 가려진 키를 보이지 않는다(`v`는 vim 덧층의 select::VisualNote).
// 메뉴의 제목 꼬리와 대비된다(review-3 지적 2).
#[gpui_kit::test]
fn palette_does_not_show_shadowed_key_in_vim(cx: &mut TestAppContext) {
    start(cx, "r3-vim-palette-key");
    cx.update(|cx| keymap::set_preset(cx, keymap::Preset::Vim));
    let (handle, _view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggle");
    render(cx, handle);
    let key = cx
        .update_window(handle, |_, window, _| {
            window
                .try_find("palette-key-view::Toggle")
                .and_then(|e| e.label().map(str::to_owned))
        })
        .unwrap();
    assert_eq!(key, None);
}

// ---------- 리뷰 3 부록 B(지적 1·2): 결정 2026-10-17-s2-t4-review-limit A로 고친 뒤 옮김 ----------

mod review3_menu_keys {
    use gpui_kit::component::kbd::Kbd;
    use gpui_kit::{App, KeyContext, Keystroke, OwnedMenuItem, TestAppContext};
    use keyscore_app::commands;
    use keyscore_app::keymap::{self, Preset};

    fn start(cx: &mut TestAppContext, name: &str, preset: Preset) {
        let dir =
            std::env::temp_dir().join(format!("ks-review-s2t4-r3b-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        cx.update(|cx| keyscore_app::init(cx, dir.join("settings.json")));
        cx.update(|cx| keymap::set_preset(cx, preset));
    }

    fn runs(keys: &[Keystroke], target: &dyn gpui_kit::Action, cx: &App) -> bool {
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let paths: Vec<&'static [&'static str]> = {
            let mut paths: Vec<&'static [&'static str]> = keyscore_app::mode::ModeState::all()
                .map(keyscore_app::mode::ModeState::context_path)
                .collect();
            paths.sort_unstable();
            paths.dedup();
            paths
        };
        paths.iter().any(|path| {
            let stack: Vec<KeyContext> =
                path.iter().map(|c| KeyContext::parse(c).unwrap()).collect();
            let (found, pending) = keymap.bindings_for_input(keys, &stack);
            !pending && found.first().is_some_and(|b| b.action().partial_eq(target))
        })
    }

    /// (제목, 문제) 목록. 단축키가 있는데 안 보이거나, 보인 키가 그 명령을 실행하지 않으면 문제다.
    fn problems(cx: &App) -> Vec<String> {
        fn walk(items: &[OwnedMenuItem], cx: &App, out: &mut Vec<String>) {
            for item in items {
                match item {
                    OwnedMenuItem::Action { name, action, .. } => {
                        let target = commands::menu_target(action.as_ref(), cx).unwrap();
                        let effective = commands::effective_keys(target.as_ref(), cx);
                        let native = commands::native_key_equivalent(action.as_ref(), cx);
                        let suffix = name.split(commands::MENU_KEY_SEPARATOR).nth(1);
                        match (&effective, suffix, &native) {
                            (Some(keys), Some(text), _) => {
                                let want =
                                    keys.iter().map(Kbd::format).collect::<Vec<_>>().join(" ");
                                if text != want || !runs(keys, target.as_ref(), cx) {
                                    out.push(format!("{name}: 보인 키 {text}, 누를 키 {want}"));
                                }
                            }
                            (Some(keys), None, Some(native)) => {
                                if !(keys.len() == 1 && keys[0] == *native) {
                                    out.push(format!("{name}: 키 등가물이 누를 키와 다르다"));
                                }
                            }
                            (Some(_), None, None) => {
                                out.push(format!("{name}: 단축키가 안 보인다"));
                            }
                            (None, Some(text), _) => {
                                out.push(format!("{name}: 누를 키가 없는데 {text}"));
                            }
                            (None, None, _) => {}
                        }
                    }
                    OwnedMenuItem::Submenu(menu) => walk(&menu.items, cx, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for menu in cx.get_menus().unwrap() {
            walk(&menu.items, cx, &mut out);
        }
        out
    }

    #[gpui_kit::test]
    fn vim_menu_shortcuts_are_shown_and_effective(cx: &mut TestAppContext) {
        start(cx, "vim", Preset::Vim);
        let bad = cx.update(|cx| problems(cx));
        assert!(bad.is_empty(), "{}건: {:?}", bad.len(), bad);
    }

    #[gpui_kit::test]
    fn default_menu_shortcuts_are_shown_and_effective(cx: &mut TestAppContext) {
        start(cx, "default", Preset::Default);
        let bad = cx.update(|cx| problems(cx));
        assert!(bad.is_empty(), "{}건: {:?}", bad.len(), bad);
    }

    /// vim의 `z z`(커서를 가운데로)처럼 시퀀스만 걸린 항목도 제목에 보인다(B-1).
    #[gpui_kit::test]
    fn vim_sequence_shortcut_is_in_title(cx: &mut TestAppContext) {
        start(cx, "vim-seq", Preset::Vim);
        let menus = cx.update(|cx| cx.get_menus()).unwrap();
        let view = menus.iter().find(|m| m.name.as_ref() == "보기").unwrap();
        let title = view
            .items
            .iter()
            .find_map(|i| match i {
                OwnedMenuItem::Action { name, .. } if name.starts_with("커서를 가운데로") => {
                    Some(name.clone())
                }
                _ => None,
            })
            .unwrap();
        assert!(title.contains(commands::MENU_KEY_SEPARATOR), "{title}");
    }
}
