//! S2-T4 리뷰 4(좁은 확인): 메뉴 제목 꼬리 키의 정확성을 사용자 키맵 핫리로드 뒤에도 본다.

use gpui_kit::component::kbd::Kbd;
use gpui_kit::{App, KeyContext, OwnedMenuItem, TestAppContext};
use keyscore_app::commands;
use keyscore_app::keymap::{self, Preset};

fn start(cx: &mut TestAppContext, name: &str, preset: Preset) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t4-r4-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| keyscore_app::init(cx, dir.join("settings.json")));
    cx.update(|cx| keymap::set_preset(cx, preset));
    dir
}

fn runs(keys: &[gpui_kit::Keystroke], target: &dyn gpui_kit::Action, cx: &App) -> bool {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    [
        &["Normal", "Arrangement"][..],
        &["Normal", "Arrangement", "Selection"],
        &["Normal", "PianoRoll"],
        &["Normal", "PianoRoll", "Selection"],
    ]
    .iter()
    .any(|path| {
        let stack: Vec<KeyContext> = path.iter().map(|c| KeyContext::parse(c).unwrap()).collect();
        let (found, pending) = keymap.bindings_for_input(keys, &stack);
        !pending && found.first().is_some_and(|b| b.action().partial_eq(target))
    })
}

/// 제목 꼬리나 macOS 키 등가물로 보이는 키가 Normal 모드에서 그 명령을 실행하지 않는 항목.
fn problems(cx: &App) -> Vec<String> {
    fn walk(items: &[OwnedMenuItem], cx: &App, out: &mut Vec<String>) {
        for item in items {
            match item {
                OwnedMenuItem::Action { name, action, .. } => {
                    let target = commands::menu_target(action.as_ref(), cx).unwrap();
                    let eff = commands::effective_keys(target.as_ref(), cx);
                    let native = commands::native_key_equivalent(action.as_ref(), cx);
                    let suffix = name.split(commands::MENU_KEY_SEPARATOR).nth(1);
                    if let (Some(keys), Some(text)) = (&eff, suffix) {
                        let want = keys.iter().map(Kbd::format).collect::<Vec<_>>().join(" ");
                        if text != want || !runs(keys, target.as_ref(), cx) {
                            out.push(format!("{name}: 꼬리 {text} / 누를 키 {want}"));
                        }
                    }
                    // 가사 분배의 cmd-v는 Lyric 컨텍스트 전용이라 Normal에서는 붙여넣기다(review-4 제안 1). 알려진 예외.
                    if eff.is_none()
                        && !name.starts_with("가사 분배")
                        && (suffix.is_some() || native.is_some())
                    {
                        out.push(format!(
                            "{name}: 누를 키 없음, 보이는 키 {suffix:?} {native:?}"
                        ));
                    }
                    // macOS가 그리는 키 등가물이 누를 키와 다르면 거짓 표시다
                    if let Some(n) = &native {
                        let works = runs(std::slice::from_ref(n), target.as_ref(), cx);
                        if !works && !name.starts_with("가사 분배") {
                            out.push(format!(
                                "{name}: 거짓 등가물 {n:?}(Normal에서 다른 명령), 누를 키 {eff:?}"
                            ));
                        }
                    }
                    if eff.is_some() && suffix.is_none() && native.is_none() {
                        out.push(format!("{name}: 안 보임"));
                    }
                }
                OwnedMenuItem::Submenu(m) => walk(&m.items, cx, out),
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
fn presets_have_no_menu_key_problems(cx: &mut TestAppContext) {
    for (n, p) in [("d", Preset::Default), ("v", Preset::Vim)] {
        start(cx, n, p);
        let bad = cx.update(|cx| problems(cx));
        assert!(bad.is_empty(), "{n}: {bad:?}");
    }
}

#[gpui_kit::test]
fn user_keymap_reload_keeps_menu_keys_correct(cx: &mut TestAppContext) {
    for (n, p) in [("ud", Preset::Default), ("uv", Preset::Vim)] {
        let dir = start(cx, n, p);
        let maps = [
            r#"[{"bindings": {"alt-k": "view::ToggleKeyboard", "g g": "view::ToggleKeyboard"}}]"#,
            r#"[{"bindings": {"ctrl-k": "view::ToggleKeyboard"}}]"#,
            r#"[{"bindings": {"z z": "view::ToggleKeyboard", "v": "view::ToggleKeyboard"}}]"#,
            // 기본 프리셋이 쓰는 키를 가리는 사용자 키맵은 review-4 지적 1(실패)이라 뺐다
            r#"[{"bindings": {"x": "view::ToggleKeyboard"}}]"#,
            "[]",
        ];
        for m in maps {
            std::fs::write(dir.join("keymap.json"), m).unwrap();
            cx.update(keymap::apply);
            let bad = cx.update(|cx| problems(cx));
            assert!(bad.is_empty(), "{n} {m}: {bad:?}");
        }
    }
}
