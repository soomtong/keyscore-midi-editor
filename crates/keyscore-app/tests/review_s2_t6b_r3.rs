//! S2-T6b 적대적 리뷰 3회차: 리뷰 2 수정(배경 `occlude`, 키맵 원문에서 찾은 인자 라벨, `overlay_lines` 캐시,
//! `delete` 기호)을 깨뜨리려는 시도.
//!
//! 인자 라벨은 엔진이 실제로 그 키에 건 Action과 대조한다. 라벨의 인자로 Action을 다시 만들어 엔진의 승자와
//! 같은지 본다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, AsKeystroke as _, Entity, KeyContext, TestAppContext,
};
use keyscore_app::actions::{help, mode};
use keyscore_app::keymap;
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::ksq;
use serde_json::Value;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review3-t6b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let p = ksq::from_str(r#"{"tracks": [{"name": "A", "channel": 1}]}"#)
        .unwrap()
        .0;
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
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
    cx.run_until_parked();
    render(cx, handle);
}

fn lines(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<Vec<String>> {
    cx.read(|cx| view.read(cx).overlay_lines(cx))
}

fn muted(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).project().tracks()[0].mute())
}

/// 줄의 키 목록에 그 키(시퀀스)가 있는가.
fn has_key(line: &str, key: &str) -> bool {
    line.split_once(" · ")
        .is_some_and(|(k, _)| k.split(", ").any(|x| x == key))
}

fn reload(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update(keymap::apply);
    cx.run_until_parked();
    render(cx, handle);
}

/// 지금 경로에서 엔진이 실제로 거는(그 키의 첫 승자) 값 열린 인자 바인딩마다, 오버레이에 그 키를 가진 그 이름의
/// 줄이 있고 그 줄 제목의 인자로 만든 Action이 엔진의 Action과 같은지. 어긋난 것을 돌려준다.
fn arg_mismatches(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<String> {
    cx.read(|cx| {
        let lines = view
            .read(cx)
            .overlay_lines(cx)
            .expect("오버레이가 닫혀 있다");
        let path = view.read(cx).mode_state().context_path();
        let stack: Vec<KeyContext> = path.iter().map(|c| KeyContext::parse(c).unwrap()).collect();
        let entries = keyscore_app::commands::palette_entries(cx);
        let km = cx.key_bindings();
        let km = km.borrow();
        let mut bad = Vec::new();
        for b in km.bindings() {
            let a = b.action();
            let name = a.name();
            if keyscore_app::commands::spec(name).is_none() {
                continue;
            }
            if b.predicate().is_some_and(|p| p.depth_of(&stack).is_none()) {
                continue;
            }
            let (w, _) = km.bindings_for_input(b.keystrokes(), &stack);
            if !w.first().is_some_and(|w| w.action().partial_eq(a)) {
                continue;
            }
            // 팔레트 변형이 있거나 인자 없는 Action은 이 검사 밖
            if entries
                .iter()
                .any(|e| e.action().is_some_and(|x| x.partial_eq(a)))
                || cx.build_action(name, None).is_ok()
            {
                continue;
            }
            let keys = b
                .keystrokes()
                .iter()
                .map(|k| k.as_keystroke().unparse())
                .collect::<Vec<_>>()
                .join(" ");
            let suffix = format!(" ({name})");
            let ok = lines
                .iter()
                .filter_map(|l| l.strip_suffix(&suffix))
                .filter_map(|l| l.split_once(" · "))
                .filter(|(k, _)| k.split(", ").any(|x| x == keys))
                .any(|(_, title)| {
                    let Some((_, arg)) = title.rsplit_once(" · ") else {
                        return false;
                    };
                    let v = serde_json::from_str(arg).unwrap_or(Value::String(arg.to_owned()));
                    cx.build_action(name, Some(v))
                        .is_ok_and(|x| x.partial_eq(a))
                });
            if !ok {
                let shown: Vec<_> = lines.iter().filter(|l| l.ends_with(&suffix)).collect();
                bad.push(format!("{path:?} {keys} -> {name}: 줄 {shown:?}"));
            }
        }
        bad
    })
}

/// 모든 모드·보기·선택·고스트 경로를 돌며 인자 라벨을 대조한다. 대조한 바인딩 수도 센다.
fn walk_all_paths(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> Vec<String> {
    let mut bad = Vec::new();
    let mut check = |cx: &mut TestAppContext| {
        bad.extend(arg_mismatches(cx, view));
    };
    event(cx, handle, view, ModeEvent::Enter(Mode::Normal));
    check(cx);
    event(cx, handle, view, ModeEvent::Select);
    check(cx);
    event(cx, handle, view, ModeEvent::ClearSelection);
    event(cx, handle, view, ModeEvent::ToggleView);
    check(cx);
    event(cx, handle, view, ModeEvent::Select);
    check(cx);
    event(cx, handle, view, ModeEvent::ClearSelection);
    event(cx, handle, view, ModeEvent::ToggleView);
    for m in [
        Mode::Step,
        Mode::Sequence,
        Mode::Play,
        Mode::Ai,
        Mode::Lyric,
        Mode::Controller,
    ] {
        event(cx, handle, view, ModeEvent::Enter(Mode::Normal));
        event(cx, handle, view, ModeEvent::Enter(m));
        check(cx);
        if m == Mode::Step {
            event(cx, handle, view, ModeEvent::ShowGhost);
            check(cx);
            event(cx, handle, view, ModeEvent::DismissGhost);
        }
    }
    event(cx, handle, view, ModeEvent::Enter(Mode::Normal));
    bad
}

// ---------- 지적 2 수정: 인자 라벨이 엔진과 맞는가 ----------

/// 기본·vim 프리셋, 수식키 교환 켬/끔에서 모든 경로의 값 열린 인자 줄이 엔진이 그 키에 건 인자와 같다.
#[gpui_kit::test]
fn argument_labels_match_engine_across_presets_and_swap(cx: &mut TestAppContext) {
    start(cx, "presets", None);
    let (handle, view) = open(cx);
    run(cx, handle, help::KeymapOverlay);
    let mut bad = Vec::new();
    for preset in [keymap::Preset::Default, keymap::Preset::Vim] {
        for swap in [false, true] {
            cx.update(|cx| {
                keymap::set_preset(cx, preset);
                keymap::set_swap_alt_cmd(cx, swap);
            });
            cx.run_until_parked();
            render(cx, handle);
            assert!(lines(cx, &view).is_some(), "오버레이가 닫혔다");
            for b in walk_all_paths(cx, handle, &view) {
                bad.push(format!("{preset:?} swap={swap}: {b}"));
            }
            let all = lines(cx, &view).unwrap();
            assert!(!all.iter().any(|l| l.contains("dyn Action")), "{all:?}");
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// vim Step의 `{`/`}`는 방향이 다른 두 줄이고, vim Normal의 숫자는 숫자마다 한 줄이다.
#[gpui_kit::test]
fn vim_octave_once_and_count_keep_direction(cx: &mut TestAppContext) {
    start(cx, "vim", None);
    let (handle, view) = open(cx);
    cx.update(|cx| keymap::set_preset(cx, keymap::Preset::Vim));
    cx.run_until_parked();
    run(cx, handle, mode::Step);
    run(cx, handle, help::KeymapOverlay);
    let l = lines(cx, &view).unwrap();
    let oct: Vec<_> = l
        .iter()
        .filter(|x| x.ends_with("(step::OctaveOnce)"))
        .collect();
    assert!(
        oct.iter()
            .any(|x| x.starts_with("{ · ") && x.contains(" · -1 (")),
        "{oct:?}"
    );
    assert!(
        oct.iter()
            .any(|x| x.starts_with("} · ") && x.contains(" · 1 (")),
        "{oct:?}"
    );
    run(cx, handle, mode::Normal);
    let l = lines(cx, &view).unwrap();
    let cnt: Vec<_> = l
        .iter()
        .filter(|x| x.ends_with("(operator::Count)"))
        .collect();
    assert_eq!(cnt.len(), 9, "{cnt:?}");
}

/// 사용자 키맵이 같은 자리를 다른 인자로 덮고, 시퀀스 키에 걸고, `zed::Unbind`로 끄고, 수식키 교환으로 키가
/// 바뀌어도 라벨이 엔진과 맞는다.
#[gpui_kit::test]
fn argument_labels_follow_user_overrides(cx: &mut TestAppContext) {
    let user = r#"[{"context": "Play", "bindings": {
        "f1": "play::SelectTrack(5)",
        "f2": ["zed::Unbind", "play::SelectTrack"],
        "g 3": "play::SelectTrack(3)",
        "alt-f9": "play::SelectTrack(7)"
    }}]"#;
    start(cx, "user", Some(user));
    let (handle, view) = open(cx);
    run(cx, handle, mode::Play);
    run(cx, handle, help::KeymapOverlay);
    let l = lines(cx, &view).unwrap();
    let sel: Vec<_> = l
        .iter()
        .filter(|x| x.ends_with("(play::SelectTrack)"))
        .cloned()
        .collect();
    assert!(
        sel.iter().any(|x| has_key(x, "f1") && x.contains(" · 5 (")),
        "{sel:?}"
    );
    assert!(!sel.iter().any(|x| has_key(x, "f2")), "{sel:?}");
    assert!(
        sel.iter()
            .any(|x| has_key(x, "g 3") && x.contains(" · 3 (")),
        "{sel:?}"
    );
    assert!(
        arg_mismatches(cx, &view).is_empty(),
        "{:#?}",
        arg_mismatches(cx, &view)
    );
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    cx.run_until_parked();
    render(cx, handle);
    let l = lines(cx, &view).unwrap();
    assert!(
        l.iter()
            .any(|x| has_key(x, "cmd-f9") && x.contains(" · 7 (")),
        "{l:?}"
    );
    assert!(
        arg_mismatches(cx, &view).is_empty(),
        "{:#?}",
        arg_mismatches(cx, &view)
    );
}

/// 키맵 편집기 경로(`UserKeymap::bind` → `save_user`)로 값 열린 인자 Action의 키를 바꿔도 라벨이 엔진과 맞는다.
/// 수식키 교환을 켠 채로도.
#[gpui_kit::test]
fn argument_labels_follow_editor_rebind(cx: &mut TestAppContext) {
    start(cx, "editor", None);
    let (handle, view) = open(cx);
    for (swap, keys) in [(false, "f9"), (true, "cmd-f10")] {
        cx.update(|cx| keymap::set_swap_alt_cmd(cx, swap));
        cx.run_until_parked();
        cx.update(|cx| {
            let (layers, user) = keymap::current_layers(cx);
            let mut user = user.unwrap();
            let eff = keymap::user::effective_with(&layers, swap);
            user.bind("play::SelectTrack", Some("Play"), keys, &eff);
            keymap::save_user(cx, &user).unwrap();
        });
        cx.run_until_parked();
        render(cx, handle);
        run(cx, handle, mode::Play);
        run(cx, handle, help::KeymapOverlay);
        let l = lines(cx, &view).unwrap();
        let sel: Vec<_> = l
            .iter()
            .filter(|x| x.ends_with("(play::SelectTrack)"))
            .cloned()
            .collect();
        assert!(sel.iter().any(|x| has_key(x, keys)), "swap={swap}: {sel:?}");
        assert!(
            arg_mismatches(cx, &view).is_empty(),
            "swap={swap}: {:#?}",
            arg_mismatches(cx, &view)
        );
        run(cx, handle, help::KeymapOverlay);
        run(cx, handle, mode::Normal);
    }
}

// ---------- 지적 1 수정: 배경 occlude ----------

/// 배경이 마우스를 가려도 안쪽 클릭(줄)은 닫지 않고, 오버레이 위에서의 클릭 뒤에도 키 포커스가 남는다(Tab).
/// 바깥 클릭 뒤에는 문서 키가 다시 산다.
#[gpui_kit::test]
fn inside_click_keeps_overlay_and_focus(cx: &mut TestAppContext) {
    start(cx, "inside", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    cx.update_window(handle, |_, window, cx| {
        window.click(("overlay-row", 0usize), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(lines(cx, &view).is_some(), "안쪽 클릭이 닫았다");
    press(cx, handle, "tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).overlay_page()).map(|p| p.0),
        Some(1),
        "안쪽 클릭 뒤 Tab이 쪽을 넘기지 않았다"
    );
    press(cx, handle, "m");
    assert!(!muted(cx, &view), "열린 동안 m이 문서에 닿았다");
    cx.update_window(handle, |_, window, cx| window.click("track-mute-0", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(lines(cx, &view).is_none());
    assert!(!muted(cx, &view), "바깥 클릭이 뮤트를 눌렀다");
    press(cx, handle, "m");
    assert!(
        muted(cx, &view),
        "바깥 클릭으로 닫은 뒤 문서 포커스가 돌아오지 않았다"
    );
}

/// 오버레이 위에 마우스를 올려도(아래 버튼 자리) 닫힌 뒤 그 버튼 클릭은 정상이다(가림이 남지 않는다).
#[gpui_kit::test]
fn occlusion_does_not_outlive_overlay(cx: &mut TestAppContext) {
    start(cx, "outlive", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    cx.update_window(handle, |_, window, cx| window.hover("track-mute-0", cx))
        .unwrap();
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    cx.update_window(handle, |_, window, cx| window.click("track-mute-0", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(muted(cx, &view), "닫힌 뒤 뮤트 버튼이 눌리지 않았다");
}

// ---------- 제안 3: overlay_lines 캐시 ----------

/// 그린 줄과 `overlay_lines`가 키맵 다시 읽기 직후(다시 그리기 전)에도 지금 키맵을 따른다.
#[gpui_kit::test]
fn overlay_lines_is_fresh_before_redraw(cx: &mut TestAppContext) {
    let dir = start(cx, "fresh", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"context": "Normal", "bindings": {"ctrl-alt-shift-z": "track::ToggleMute"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    // 다시 그리지 않은 채로 본다
    let l = lines(cx, &view).unwrap();
    assert!(
        l.iter()
            .any(|x| x.contains("ctrl-alt-shift-z") && x.ends_with("(track::ToggleMute)")),
        "캐시가 낡은 줄을 돌려줬다"
    );
    reload(cx, handle);
    assert!(lines(cx, &view).is_some());
}

// ---------- S2-T6f: 리뷰 3 부록 C1·C2를 저장소 테스트로 ----------

/// 손으로 쓴 전역 `f1` → `play::SelectTrack(2)`가 Play의 `f1`(1)과 겹칠 때, 라벨은 엔진이 실제로 고른 것과 같다.
#[gpui_kit::test]
fn argument_label_with_global_override(cx: &mut TestAppContext) {
    start(
        cx,
        "global",
        Some(r#"[{"bindings": {"f1": "play::SelectTrack(2)"}}]"#),
    );
    let (handle, view) = open(cx);
    run(cx, handle, mode::Play);
    run(cx, handle, help::KeymapOverlay);
    let bad = arg_mismatches(cx, &view);
    assert!(bad.is_empty(), "{bad:#?}");
}

/// vim: 편집기로 Normal의 카운트 키 `1`을 `2`로 옮기면 Normal의 `2`는 `Count(1)`, `PianoRoll`의 `2`는 그대로
/// `Count(2)`다. 두 경로의 라벨이 엔진이 고른 것과 같다.
#[gpui_kit::test]
fn argument_label_editor_rebind_shadowed_by_deeper_context(cx: &mut TestAppContext) {
    start(cx, "deeper", None);
    let (handle, view) = open(cx);
    cx.update(|cx| keymap::set_preset(cx, keymap::Preset::Vim));
    cx.run_until_parked();
    cx.update(|cx| {
        let (layers, user) = keymap::current_layers(cx);
        let mut user = user.unwrap();
        let eff = keymap::user::effective_with(&layers, false);
        user.bind("operator::Count", Some("Normal"), "2", &eff);
        keymap::save_user(cx, &user).unwrap();
    });
    cx.run_until_parked();
    render(cx, handle);
    run(cx, handle, help::KeymapOverlay);
    let normal = arg_mismatches(cx, &view);
    event(cx, handle, &view, ModeEvent::ToggleView);
    let roll = arg_mismatches(cx, &view);
    assert!(
        normal.is_empty() && roll.is_empty(),
        "{normal:#?} {roll:#?}"
    );
}
