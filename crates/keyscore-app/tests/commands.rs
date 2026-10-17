//! S2-T4 명령 표, 메뉴바, 명령 팔레트(UI-2, UI-11, KM-13, 7.6).

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::component::kbd::Kbd;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Entity, Focusable as _, Keystroke, OwnedMenu,
    OwnedMenuItem, TestAppContext,
};
use keyscore_app::actions::ALL_ACTION_NAMES;
use keyscore_app::commands::{self, MenuId, Place};
use keyscore_app::keymap;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-commands-{}-{name}", std::process::id()));
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

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    render(cx, handle);
}

fn label_of(cx: &mut TestAppContext, handle: AnyWindowHandle, id: String) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

/// 메뉴의 Action 항목을 하위 메뉴까지 (제목, Action) 으로 편다.
fn menu_actions(menus: &[OwnedMenu]) -> Vec<(String, Box<dyn Action>)> {
    fn walk(items: &[OwnedMenuItem], out: &mut Vec<(String, Box<dyn Action>)>) {
        for item in items {
            match item {
                OwnedMenuItem::Action { name, action, .. } => {
                    out.push((name.clone(), action.boxed_clone()));
                }
                OwnedMenuItem::Submenu(menu) => walk(&menu.items, out),
                OwnedMenuItem::Separator | OwnedMenuItem::SystemMenu(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    for menu in menus {
        walk(&menu.items, &mut out);
    }
    out
}

fn keys(text: &str) -> String {
    Kbd::format(&Keystroke::parse(text).unwrap())
}

// ---------- 1. 명령 표 ----------

#[test]
fn every_action_has_exactly_one_command_spec() {
    let names: Vec<&str> = commands::specs()
        .iter()
        .map(commands::CommandSpec::name)
        .collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "명령 표에 같은 Action이 두 줄");
    let registered: BTreeSet<&str> = ALL_ACTION_NAMES.iter().copied().collect();
    assert_eq!(unique, registered);
    for spec in commands::specs() {
        assert!(!spec.label().trim().is_empty(), "{}", spec.name());
        if let Place::Elsewhere(place) = spec.place() {
            assert!(!place.trim().is_empty(), "{}", spec.name());
        }
        assert_eq!(
            commands::spec(spec.name()).map(commands::CommandSpec::name),
            Some(spec.name())
        );
    }
    // 메뉴 이름 = 팔레트 이름이므로 이름이 겹치면 사람이 고를 수 없다
    let mut labels = BTreeSet::new();
    for spec in commands::specs() {
        assert!(
            labels.insert(spec.label()),
            "이름이 겹친다: {}",
            spec.label()
        );
    }
}

// ---------- 2. 단계 게이트: 등록 수 = 팔레트 = 메뉴 + 메뉴 밖 ----------

#[gpui_kit::test]
fn action_counts_match_palette_and_menus(cx: &mut TestAppContext) {
    start(cx, "counts", None);
    let registered = cx.update(|cx| {
        let all = cx.all_action_names();
        ALL_ACTION_NAMES.iter().filter(|n| all.contains(n)).count()
    });
    assert_eq!(registered, ALL_ACTION_NAMES.len());

    let palette: BTreeSet<&str> = cx.update(|cx| {
        commands::palette_entries(cx)
            .iter()
            .map(commands::PaletteEntry::name)
            .collect()
    });
    let menus = cx.update(|cx| cx.get_menus()).expect("메뉴가 걸려 있다");
    let in_menus: BTreeSet<String> = menu_actions(&menus)
        .iter()
        .map(|(_, a)| a.name().to_owned())
        .collect();
    let elsewhere: BTreeSet<String> = commands::specs()
        .iter()
        .filter(|s| matches!(s.place(), Place::Elsewhere(_)))
        .map(|s| s.name().to_owned())
        .collect();
    assert_eq!(palette.len(), registered);
    assert!(
        in_menus.is_disjoint(&elsewhere),
        "{:?}",
        in_menus.intersection(&elsewhere).collect::<Vec<_>>()
    );
    assert_eq!(in_menus.len() + elsewhere.len(), registered);
    let declared: BTreeSet<String> = commands::specs()
        .iter()
        .filter(|s| matches!(s.place(), Place::Menu { .. }))
        .map(|s| s.name().to_owned())
        .collect();
    // 최근 파일은 S2-T7까지 빈 하위 메뉴다
    let missing: Vec<_> = declared.difference(&in_menus).collect();
    assert_eq!(
        missing,
        ["file::OpenRecent"],
        "메뉴에 둔다고 했는데 없는 항목"
    );
}

// ---------- 3. 7.6 메뉴바 구성 ----------

// 7.6 메뉴 구성 표를 한눈에 보이게 나누지 않는다.
#[allow(clippy::too_many_lines)]
#[gpui_kit::test]
fn menus_follow_req_7_6(cx: &mut TestAppContext) {
    start(cx, "menus", None);
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let titles: Vec<&str> = menus.iter().map(|m| m.name.as_ref()).collect();
    assert_eq!(
        titles,
        [
            "KeyScore",
            "파일",
            "편집",
            "보기",
            "트랙",
            "재생",
            "AI",
            "도움말"
        ]
    );
    assert_eq!(
        MenuId::ALL.map(MenuId::title),
        [
            "KeyScore",
            "파일",
            "편집",
            "보기",
            "트랙",
            "재생",
            "AI",
            "도움말"
        ]
    );
    let expect: [(&str, &[&str]); 7] = [
        (
            "파일",
            &[
                "file::New",
                "file::Open",
                "file::Save",
                "file::SaveAs",
                "file::ImportSmf",
                "file::ExportSmf",
                "file::ImportText",
                "file::ExportText",
            ],
        ),
        (
            "편집",
            &[
                "edit::Undo",
                "edit::Redo",
                "edit::Cut",
                "edit::Copy",
                "edit::Paste",
                "edit::Duplicate",
                "edit::Repeat",
                "select::All",
                "edit::Split",
                "edit::Join",
                "edit::Quantize",
                "edit::Legato",
                "mode::Lyric",
                "lyric::Distribute",
            ],
        ),
        (
            "보기",
            &[
                "view::Toggle",
                "lane::Toggle",
                "view::ToggleInspector",
                "view::ToggleKeyboard",
                "view::ZoomIn",
                "view::ZoomOut",
                "view::SetTheme",
            ],
        ),
        (
            "트랙",
            &[
                "track::New",
                "track::Delete",
                "track::ToggleMute",
                "track::ToggleSolo",
                "track::MoveUp",
                "track::MoveDown",
                "track::SetChannel",
            ],
        ),
        (
            "재생",
            &[
                "transport::Play",
                "transport::Stop",
                "transport::PlayFromCursor",
                "transport::ToggleLoop",
                "mode::Sequence",
                "transport::ToggleMetronome",
                "transport::Panic",
            ],
        ),
        (
            "AI",
            &[
                "ai::OpenPanel",
                "ai::ToggleAssist",
                "ai::OpenLab",
                "ai::OpenTemplateFolder",
            ],
        ),
        (
            "도움말",
            &[
                "help::KeymapOverlay",
                "help::ReplayOnboarding",
                "help::OpenDocs",
            ],
        ),
    ];
    for (title, names) in expect {
        let menu = menus.iter().find(|m| m.name.as_ref() == title).unwrap();
        let have: BTreeSet<String> = menu_actions(std::slice::from_ref(menu))
            .iter()
            .map(|(_, a)| a.name().to_owned())
            .collect();
        for name in names {
            assert!(have.contains(*name), "{title} 메뉴에 {name}이 없다");
        }
    }
}

// ---------- 4. 메뉴 제목 = 팔레트 이름 ----------

#[gpui_kit::test]
fn menu_titles_equal_palette_names(cx: &mut TestAppContext) {
    start(cx, "titles", None);
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let entries = cx.update(|cx| commands::palette_entries(cx));
    for (title, action) in menu_actions(&menus) {
        let entry = entries
            .iter()
            .find(|e| e.action().is_some_and(|a| a.partial_eq(action.as_ref())))
            .unwrap_or_else(|| panic!("팔레트에 없는 메뉴 항목 {title}"));
        assert_eq!(entry.label().as_ref(), title);
    }
}

// ---------- 5. 단축키 표시는 현재 키맵 ----------

#[gpui_kit::test]
fn shortcut_display_follows_current_keymap(cx: &mut TestAppContext) {
    let dir = start(cx, "shortcuts", None);
    let (handle, _) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggle");
    assert_eq!(
        label_of(cx, handle, "palette-key-view::Toggle".into()),
        Some(keys("v"))
    );
    press(cx, handle, "escape");
    press(cx, handle, "escape");
    // 사용자 레이어(뒤 레이어가 이김)에서 같은 컨텍스트에 다른 키를 건다
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"context": "Normal", "bindings": {"f6": "view::Toggle", "v": null}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    render(cx, handle);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggle");
    assert_eq!(
        label_of(cx, handle, "palette-key-view::Toggle".into()),
        Some(keys("f6"))
    );
    // 키맵을 다시 걸면 메뉴도 다시 만든다(메뉴의 키 등가물은 메뉴를 걸 때 굽는다)
    assert!(cx.update(|cx| cx.get_menus()).is_some());
}

// ---------- 6. 인자 변형 ----------

#[gpui_kit::test]
fn every_variant_builds(cx: &mut TestAppContext) {
    start(cx, "variants", None);
    cx.update(|cx| {
        for spec in commands::specs() {
            for variant in spec.variants() {
                let args = serde_json::from_str(variant.args()).unwrap();
                let args = if args == serde_json::Value::Null {
                    None
                } else {
                    Some(args)
                };
                assert!(
                    cx.build_action(spec.name(), args).is_ok(),
                    "{} {}",
                    spec.name(),
                    variant.args()
                );
            }
        }
        let entries = commands::palette_entries(cx);
        let themes: Vec<_> = entries
            .iter()
            .filter(|e| e.name() == "view::SetTheme")
            .map(|e| e.label().to_string())
            .collect();
        assert_eq!(themes, ["시스템 테마", "라이트 테마", "다크 테마"]);
        assert!(
            entries
                .iter()
                .filter(|e| e.name() == "view::SetTheme")
                .all(|e| e.action().is_some())
        );
    });
}

// ---------- 7–9. 팔레트 ----------

fn inspector_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).layout().inspector_open)
}

fn palette_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).palette_open())
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

#[gpui_kit::test]
fn palette_opens_filters_runs_and_closes(cx: &mut TestAppContext) {
    start(cx, "palette-run", None);
    let (handle, view) = open(cx);
    let before = inspector_open(cx, &view);

    press(cx, handle, "cmd-shift-p");
    assert!(palette_open(cx, &view));
    type_text(cx, handle, "view::toggleinspector");
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert_ne!(
        inspector_open(cx, &view),
        before,
        "Enter가 Action을 실행하지 않았다"
    );
    assert!(
        window_focused(cx, handle, &view),
        "닫힌 뒤 포커스가 문서로 돌아오지 않았다"
    );

    // `:`도 팔레트(UI-2). Esc는 질의를 먼저 지우고, 빈 질의에서 닫는다(A13)
    press(cx, handle, ":");
    assert!(palette_open(cx, &view));
    type_text(cx, handle, "zoom");
    press(cx, handle, "escape");
    assert!(palette_open(cx, &view));
    press(cx, handle, "escape");
    assert!(!palette_open(cx, &view));
    assert!(window_focused(cx, handle, &view));
}

#[gpui_kit::test]
fn palette_rows_show_name_description_shortcut(cx: &mut TestAppContext) {
    start(cx, "palette-rows", None);
    let (handle, _) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggle");
    assert_eq!(
        label_of(cx, handle, "palette-row-view::Toggle".into()).as_deref(),
        Some("배치 뷰")
    );
    assert_eq!(
        label_of(cx, handle, "palette-desc-view::Toggle".into()).as_deref(),
        Some("피아노롤 ↔ 배치 뷰")
    );
    assert_eq!(
        label_of(cx, handle, "palette-key-view::Toggle".into()),
        Some(keys("v"))
    );
    // 표준 키는 컨텍스트 없이 걸려 있다
    press(cx, handle, "escape");
    press(cx, handle, "escape");
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "edit::undo");
    assert_eq!(
        label_of(cx, handle, "palette-key-edit::Undo".into()),
        Some(keys("cmd-z"))
    );
}

#[gpui_kit::test]
fn open_argument_actions_are_listed_but_not_runnable(cx: &mut TestAppContext) {
    start(cx, "palette-args", None);
    let entries = cx.update(|cx| commands::palette_entries(cx));
    let tempo = entries
        .iter()
        .find(|e| e.name() == "transport::SetTempo")
        .unwrap();
    assert!(tempo.action().is_none());
    assert!(
        tempo.description().contains("인자 필요"),
        "{}",
        tempo.description()
    );

    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "transport::settempo");
    press(cx, handle, "enter");
    assert!(
        palette_open(cx, &view),
        "실행할 수 없는 줄이 팔레트를 닫았다"
    );
}
