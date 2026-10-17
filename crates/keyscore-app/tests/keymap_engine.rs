//! S2-T2 키맵 엔진을 GPUI 위에서(헤드리스).

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    Keystroke, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext, Window, div,
};
use keyscore_app::actions::ALL_ACTION_NAMES;
use keyscore_app::actions::view::{ToggleInspector, ToggleTrackList};
use keyscore_app::keymap::conflict::{ConflictKind, conflicts};
use keyscore_app::keymap::{self, Preset, load_layers, swap_alt_cmd};
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-keymap-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 앱을 초기화한다. 사용자 키맵을 먼저 써 두면 부트 때 읽힌다.
fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = temp_dir(name);
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    let settings = dir.join("settings.json");
    cx.update(|cx| init(cx, settings));
    dir
}

fn open(cx: &mut TestAppContext) -> AnyWindowHandle {
    let (handle, _) = cx
        .update(|cx| open_project_window(Project::new(), cx))
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    handle
}

fn inspector_open(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |root, _, cx| {
        let root = root.downcast::<gpui_kit::component::Root>().unwrap();
        let view = root.read(cx).view().clone();
        view.downcast::<keyscore_app::window::ProjectWindow>()
            .unwrap()
            .read(cx)
            .layout()
            .inspector_open
    })
    .unwrap()
}

// ---------- 11–12. 프리셋과 Action 레지스트리 ----------

#[gpui_kit::test]
fn every_catalog_action_is_registered(cx: &mut TestAppContext) {
    start(cx, "registry", None);
    cx.update(|cx| {
        let registered = cx.all_action_names();
        for name in ALL_ACTION_NAMES {
            assert!(registered.contains(name), "등록되지 않은 Action {name}");
        }
    });
}

#[gpui_kit::test]
fn presets_have_no_conflicts_and_only_known_actions(cx: &mut TestAppContext) {
    start(cx, "presets", None);
    for preset in [Preset::Default, Preset::Vim] {
        let (layers, errors) = load_layers(preset, None);
        assert!(errors.is_empty(), "{preset:?}: {errors:?}");
        let (_, build_errors) = cx.update(|cx| {
            keymap::build(
                &layers,
                keymap::KeymapOptions {
                    preset,
                    swap_alt_cmd: false,
                },
                cx,
            )
        });
        assert!(build_errors.is_empty(), "{preset:?}: {build_errors:?}");
        let all: Vec<_> = layers.iter().cloned().collect();
        let found = conflicts(&all);
        let same: Vec<_> = found
            .iter()
            .filter(|c| c.kind == ConflictKind::SameKeys)
            .collect();
        assert!(same.is_empty(), "{preset:?}: {same:?}");
        // 접두는 vim의 y / y y / y y p(REQ 5.8.1이 요구)뿐이다.
        for c in found.iter().filter(|c| c.kind == ConflictKind::Prefix) {
            assert!(
                c.first.0.starts_with('y') && c.second.0.starts_with('y'),
                "{c:?}"
            );
        }
    }
}

// ---------- 7, 10. 표준 단축키와 수식키 교환 ----------

#[gpui_kit::test]
fn cmd_shortcuts_are_preset_invariant(cx: &mut TestAppContext) {
    start(cx, "cmd", None);
    let (default, _) = load_layers(Preset::Default, None);
    let (vim, _) = load_layers(Preset::Vim, None);
    assert_eq!(default.standard, vim.standard);
    assert!(!default.standard.is_empty());
    // 프리셋은 표준 레이어의 키를 다시 정의하지 않는다(cmd-j 같은 표준 밖 Cmd 조합은 쓴다).
    let standard: Vec<&str> = default.standard.iter().map(|b| b.keys.as_str()).collect();
    for layers in [&default, &vim] {
        for b in &layers.preset {
            assert!(
                !standard.contains(&b.keys.as_str()),
                "프리셋이 표준 키를 덮음 {b:?}"
            );
        }
    }
    // 수식키 교환을 켜도 표준 레이어의 cmd-s는 그대로다.
    let save = keyscore_app::actions::file::Save;
    for swap in [false, true] {
        let keys = cx.update(|cx| {
            keymap::set_swap_alt_cmd(cx, swap);
            cx.key_bindings()
                .borrow()
                .bindings_for_action(&save)
                .map(|b| {
                    b.keystrokes()
                        .iter()
                        .map(|k| k.inner().unparse())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
        });
        assert!(keys.contains(&"cmd-s".to_string()), "{swap}: {keys:?}");
    }
}

#[test]
fn swap_alt_cmd_option_rewrites_preset_bindings() {
    assert_eq!(swap_alt_cmd("alt-shift-l"), "cmd-shift-l");
    assert_eq!(swap_alt_cmd("cmd-j g alt-k"), "alt-j g cmd-k");
    // 키 이름 자체가 수식키 이름과 같아도 바꾸지 않는다.
    assert_eq!(swap_alt_cmd("-"), "-");
    assert_eq!(swap_alt_cmd("shift--"), "shift--");
}

// ---------- 3. 물리 키(KM-7) ----------

#[gpui_kit::test]
fn same_physical_key_fires_same_action_in_korean_input(cx: &mut TestAppContext) {
    start(
        cx,
        "korean",
        Some(r#"[{"bindings": {"h": "view::ToggleInspector"}}]"#),
    );
    let handle = open(cx);
    assert!(inspector_open(cx, handle));
    // 한글 입력 중: 키 이름은 자판의 h, 입력 글자는 ㅗ다(GPUI macOS가 이렇게 만든다).
    cx.dispatch_keystroke(
        handle,
        Keystroke {
            modifiers: Modifiers::default(),
            key: "h".into(),
            key_char: Some("ㅗ".into()),
        },
    );
    assert!(!inspector_open(cx, handle));
    // 영문 입력 중에도 같은 Action이다.
    cx.dispatch_keystroke(
        handle,
        Keystroke {
            modifiers: Modifiers::default(),
            key: "h".into(),
            key_char: Some("h".into()),
        },
    );
    assert!(inspector_open(cx, handle));
}

// ---------- 5. 컨텍스트 우선순위 ----------

/// `Normal` 안에 `Arrangement`가 있는 작은 뷰. 어떤 Action이 왔는지 센다.
struct Nested {
    focus: FocusHandle,
    fired: Vec<&'static str>,
}

impl Render for Nested {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Normal")
            .size_full()
            .on_action(cx.listener(|this, _: &ToggleTrackList, _, _| {
                this.fired.push("normal");
            }))
            .child(
                div()
                    .key_context("Arrangement")
                    .track_focus(&self.focus)
                    .size_full()
                    .on_action(cx.listener(|this, _: &ToggleInspector, _, _| {
                        this.fired.push("arrangement");
                    })),
            )
    }
}

#[gpui_kit::test]
fn more_specific_context_wins(cx: &mut TestAppContext) {
    start(
        cx,
        "context",
        Some(
            r#"[
  {"context": "Normal > Arrangement", "bindings": {"enter": "view::ToggleInspector"}},
  {"context": "Normal", "bindings": {"enter": "view::ToggleTrackList"}}
]"#,
        ),
    );
    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(gpui_kit::WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Nested {
                        focus,
                        fired: Vec::new(),
                    }
                })
            })
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    // 더 깊은 컨텍스트가 파일에서 앞에 있어도 이긴다.
    cx.simulate_keystrokes(handle, "enter");
    assert_eq!(cx.read(|cx| view.read(cx).fired.clone()), ["arrangement"]);
}

// ---------- 6. 프리셋 전환 ----------

#[gpui_kit::test]
fn preset_switch_keeps_user_overrides(cx: &mut TestAppContext) {
    start(
        cx,
        "preset-switch",
        Some(r#"[{"bindings": {"f5": "view::ToggleInspector"}}]"#),
    );
    let handle = open(cx);
    for preset in [Preset::Vim, Preset::Default, Preset::Vim] {
        cx.update(|cx| keymap::set_preset(cx, preset));
        let before = inspector_open(cx, handle);
        cx.simulate_keystrokes(handle, "f5");
        assert_ne!(inspector_open(cx, handle), before, "{preset:?}");
    }
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
}

// ---------- 8. 시퀀스와 카운트 ----------

#[gpui_kit::test]
fn multi_key_sequence_and_count_prefix(cx: &mut TestAppContext) {
    start(
        cx,
        "sequence",
        Some(
            r#"[{"bindings": {
  "g g": "view::ToggleInspector",
  "1": "operator::Count(1)",
  "2": "operator::Count(2)",
  "3": "operator::Count(3)"
}}]"#,
        ),
    );
    let handle = open(cx);
    cx.simulate_keystrokes(handle, "g g");
    assert!(!inspector_open(cx, handle));
    cx.simulate_keystrokes(handle, "3");
    assert_eq!(drain_count(cx), Some(3));
    assert_eq!(drain_count(cx), None);
    cx.simulate_keystrokes(handle, "1 2");
    assert_eq!(drain_count(cx), Some(12));
}

// ---------- 9. 핫리로드 ----------

#[gpui_kit::test]
fn user_keymap_hot_reloads_and_reports_errors(cx: &mut TestAppContext) {
    let dir = start(cx, "reload", None);
    let handle = open(cx);
    let path = dir.join("keymap.json");
    std::fs::write(&path, r#"[{"bindings": {"f6": "view::ToggleInspector"}}]"#).unwrap();
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
    cx.simulate_keystrokes(handle, "f6");
    assert!(!inspector_open(cx, handle));
    // 항목 하나가 잘못된 파일: 오류를 줄과 함께 남기고 나머지는 건다.
    std::fs::write(
        &path,
        "[{\"bindings\": {\n  \"f6\": \"view::ToggleInspector\",\n  \"f7\": \"no::SuchAction\"\n}}]",
    )
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
    let errors = cx.update(|cx| keymap::errors(cx).to_vec());
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].pos.line, 3);
    cx.simulate_keystrokes(handle, "f6");
    assert!(inspector_open(cx, handle));
}

#[gpui_kit::test]
fn syntax_error_in_user_keymap_keeps_last_good_layer(cx: &mut TestAppContext) {
    let dir = start(
        cx,
        "keep-last",
        Some(r#"[{"bindings": {"f8": "view::ToggleInspector"}}]"#),
    );
    let handle = open(cx);
    std::fs::write(dir.join("keymap.json"), "[{\"bindings\": {\"f8\" ").unwrap();
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| keymap::errors(cx).len()), 1);
    cx.simulate_keystrokes(handle, "f8");
    assert!(
        !inspector_open(cx, handle),
        "편집 중 문법 오류로 사용자 키가 사라졌다"
    );
}

#[test]
fn duplicate_fields_in_a_section_are_reported() {
    let (b, e) = keymap::source::parse(
        r#"[{"context": "Normal", "context": "Step", "bindings": {"x": "edit::Undo"}}]"#,
        keymap::source::Layer::User,
    );
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!(b[0].context.as_deref(), Some("Normal"));
}

/// 쌓인 카운트를 보고 비운다. `take_count`는 키 디스패치 안에서만 주므로(S2-T2 리뷰 2 지적 2·3)
/// 처리기 밖의 테스트는 이 도우미를 쓴다.
fn drain_count(cx: &mut TestAppContext) -> Option<u32> {
    cx.update(|cx| {
        let c = keymap::pending_count(cx);
        keymap::clear_count(cx);
        c
    })
}
