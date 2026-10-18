//! S2-T6b 적대적 리뷰 1회차: `?` 키맵 오버레이를 깨뜨리려는 시도.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{help, mode, transport};
use keyscore_app::keymap;
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::ksq;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t6b-{}-{name}", std::process::id()));
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
    activate(cx, handle);
    (handle, view)
}

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
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

fn page(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<(usize, usize)> {
    cx.read(|cx| view.read(cx).overlay_page())
}

fn muted(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).project().tracks()[0].mute())
}

fn mode_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

/// 줄의 키 목록(`키, 키 · 제목 (이름)`의 앞부분).
fn keys_of<'a>(lines: &'a [String], action: &str) -> Vec<&'a str> {
    lines
        .iter()
        .filter(|l| l.ends_with(&format!("({action})")))
        .flat_map(|l| l.split(" · ").next().unwrap_or("").split(", "))
        .collect()
}

fn header(cx: &mut TestAppContext, handle: AnyWindowHandle) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find("keymap-overlay")
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

// ---------- 접두 시퀀스·프리셋 ----------

/// vim 프리셋에서도 `?`가 열고(5.8.1 충돌 메모), `g g` 시퀀스가 한 키로 보이며, Esc·Tab이 동작한다.
#[gpui_kit::test]
fn vim_preset_question_mark_opens_and_shows_sequences(cx: &mut TestAppContext) {
    start(cx, "vim", None);
    cx.update(|cx| keymap::set_preset(cx, keymap::Preset::Vim));
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    let l = lines(cx, &view).expect("vim에서 ?가 오버레이를 열지 않았다");
    assert!(keys_of(&l, "cursor::ClipStart").contains(&"g g"), "{l:?}");
    // vim이 null로 지운 기본 키(`z`)는 보이지 않는다
    assert!(
        !l.iter().any(|x| x
            .split(" · ")
            .next()
            .unwrap_or("")
            .split(", ")
            .any(|k| k == "z")),
        "{l:?}"
    );
    press(cx, handle, "tab");
    assert_eq!(page(cx, &view).map(|p| p.0), Some(1));
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
}

/// 수식키 교환(KM-11)을 켜면 오버레이의 `alt-k`가 `cmd-k`로 바뀐다.
#[gpui_kit::test]
fn swap_alt_cmd_is_reflected(cx: &mut TestAppContext) {
    start(cx, "swap", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Normal);
    press(cx, handle, "?");
    let before = lines(cx, &view).unwrap();
    let up = keys_of(&before, "note::TransposeUp");
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    cx.run_until_parked();
    render(cx, handle);
    let after = lines(cx, &view).unwrap();
    let up_after = keys_of(&after, "note::TransposeUp");
    if up.contains(&"alt-k") {
        assert!(up_after.contains(&"cmd-k"), "{up:?} -> {up_after:?}");
        assert!(!up_after.contains(&"alt-k"), "{up:?} -> {up_after:?}");
    }
}

// ---------- 컨텍스트: Selection, Ghost ----------

/// Selection에서 Esc는 `select::Clear`로 보이고, 가려진 Esc 바인딩은 보이지 않는다.
#[gpui_kit::test]
fn selection_context_shows_winning_escape(cx: &mut TestAppContext) {
    start(cx, "sel", None);
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::Select);
    run(cx, handle, help::KeymapOverlay);
    let l = lines(cx, &view).unwrap();
    assert!(keys_of(&l, "select::Clear").contains(&"escape"), "{l:?}");
    let escapes = l
        .iter()
        .filter(|x| {
            x.split(" · ")
                .next()
                .unwrap_or("")
                .split(", ")
                .any(|k| k == "escape")
        })
        .count();
    assert_eq!(escapes, 1, "{l:?}");
    // Esc는 오버레이만 닫고 선택은 그대로
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    assert!(cx.read(|cx| view.read(cx).mode_state().has_selection()));
}

/// Step > Ghost에서 tab·escape는 고스트 Action이다. Step의 `escape → mode::Normal`은 가려져 빠진다.
#[gpui_kit::test]
fn ghost_context_shadows_step_escape(cx: &mut TestAppContext) {
    start(cx, "ghost", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Step);
    event(cx, handle, &view, ModeEvent::ShowGhost);
    run(cx, handle, help::KeymapOverlay);
    let l = lines(cx, &view).unwrap();
    assert!(keys_of(&l, "ai::AcceptGhost").contains(&"tab"), "{l:?}");
    assert!(keys_of(&l, "ai::DismissGhost").contains(&"escape"), "{l:?}");
    assert!(!keys_of(&l, "mode::Normal").contains(&"escape"), "{l:?}");
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    assert!(
        cx.read(|cx| view.read(cx).mode_state().has_ghost()),
        "Esc가 고스트를 지웠다"
    );
}

// ---------- Esc·수식키·포커스 ----------

/// 수식키 붙은 Esc는 오버레이를 닫지 않고, Step을 나가게 하지도 않는다.
#[gpui_kit::test]
fn modified_escape_does_nothing(cx: &mut TestAppContext) {
    start(cx, "modesc", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Step);
    run(cx, handle, help::KeymapOverlay);
    for k in ["shift-escape", "alt-escape", "ctrl-escape"] {
        press(cx, handle, k);
        assert!(lines(cx, &view).is_some(), "{k}");
        assert_eq!(mode_of(cx, &view), Mode::Step, "{k}");
    }
}

/// 오버레이가 열린 동안 Normal의 `?`는 문서로 새지 않는다(닫히지도 않는다: Overlay 컨텍스트에 `?` 없음).
#[gpui_kit::test]
fn question_mark_inside_overlay_does_not_leak(cx: &mut TestAppContext) {
    start(cx, "qq", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    press(cx, handle, "?");
    // 관찰: 열린 채로 남는다
    assert!(lines(cx, &view).is_some());
    press(cx, handle, "m");
    assert!(!muted(cx, &view));
}

/// 안쪽 클릭은 닫지 않고, 바깥(배경) 클릭은 닫고 문서로 포커스를 돌린다.
#[gpui_kit::test]
fn click_inside_keeps_click_outside_closes(cx: &mut TestAppContext) {
    start(cx, "click", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    cx.update_window(handle, |_, window, cx| window.click("keymap-overlay", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(lines(cx, &view).is_some(), "안쪽 클릭이 닫았다");
    press(cx, handle, "m");
    assert!(!muted(cx, &view), "안쪽 클릭 뒤 키가 문서로 샜다");
    gpui_kit::VisualTestContext::from_window(handle, cx).simulate_click(
        gpui_kit::point(gpui_kit::px(3.), gpui_kit::px(3.)),
        gpui_kit::Modifiers::none(),
    );
    cx.run_until_parked();
    render(cx, handle);
    assert!(lines(cx, &view).is_none(), "바깥 클릭이 닫지 않았다");
    press(cx, handle, "m");
    assert!(muted(cx, &view), "닫은 뒤 포커스가 문서로 돌아오지 않았다");
}

/// 모드 메뉴(팝업)를 열면 오버레이가 닫히고, 팝업을 닫으면 모드 키가 산다.
#[gpui_kit::test]
fn mode_menu_replaces_overlay(cx: &mut TestAppContext) {
    start(cx, "menu", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    run(cx, handle, mode::ShowMenu);
    assert!(lines(cx, &view).is_none());
    assert!(cx.read(|cx| view.read(cx).mode_menu_open()));
    press(cx, handle, "escape");
    press(cx, handle, "m");
    assert!(muted(cx, &view));
}

/// Step에서 팔레트로 「키맵 보기」를 실행하면 Step 키 오버레이가 포커스를 갖고 열린다.
#[gpui_kit::test]
fn palette_opens_overlay_in_step(cx: &mut TestAppContext) {
    start(cx, "palstep", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Step);
    press(cx, handle, "cmd-shift-p");
    cx.simulate_input(handle, "키맵 보기");
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    let l = lines(cx, &view).expect("팔레트가 오버레이를 열지 않았다");
    assert!(keys_of(&l, "step::Rest").contains(&"space"), "{l:?}");
    assert!(header(cx, handle).is_some_and(|h| h.contains("Step")));
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    assert_eq!(mode_of(cx, &view), Mode::Step);
}

/// 두 창: 한 창의 오버레이는 다른 창과 무관하고, 창을 오가도 Esc가 닫는다.
#[gpui_kit::test]
fn two_windows_are_independent(cx: &mut TestAppContext) {
    start(cx, "two", None);
    let (a, va) = open(cx);
    press(cx, a, "?");
    let (b, vb) = open(cx);
    assert!(lines(cx, &vb).is_none());
    press(cx, b, "m");
    assert!(muted(cx, &vb));
    assert!(lines(cx, &va).is_some());
    activate(cx, a);
    press(cx, a, "escape");
    assert!(lines(cx, &va).is_none());
    press(cx, a, "m");
    assert!(muted(cx, &va));
}

/// 재생 중 열고 닫아도 문제없다.
#[gpui_kit::test]
fn overlay_while_playing(cx: &mut TestAppContext) {
    start(cx, "play", None);
    let (handle, view) = open(cx);
    run(cx, handle, transport::Play);
    press(cx, handle, "?");
    for _ in 0..5 {
        render(cx, handle);
    }
    assert!(lines(cx, &view).is_some());
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    run(cx, handle, transport::Stop);
}

// ---------- 키맵이 열린 동안 바뀜 ----------

/// 마지막 쪽에서 키맵이 줄어 쪽 수가 줄면 다음 그리기에 쪽이 맞춰진다.
#[gpui_kit::test]
fn page_clamps_when_keymap_shrinks(cx: &mut TestAppContext) {
    let dir = start(cx, "shrink", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    let (_, total) = page(cx, &view).unwrap();
    for _ in 0..total {
        press(cx, handle, "tab");
    }
    assert_eq!(page(cx, &view), Some((total - 1, total)));
    // Normal의 한 글자 키를 모두 지운다
    let l = lines(cx, &view).unwrap();
    let mut nulls = Vec::new();
    for line in &l {
        for k in line.split(" · ").next().unwrap_or("").split(", ") {
            if k.chars().count() == 1 && k != "?" {
                nulls.push(format!("{k:?}: null"));
            }
        }
    }
    let text = format!(
        r#"[{{"context": "Normal", "bindings": {{ {} }}}}]"#,
        nulls.join(", ")
    );
    std::fs::write(dir.join("keymap.json"), text).unwrap();
    cx.update(keymap::apply);
    cx.run_until_parked();
    render(cx, handle);
    let (p, t) = page(cx, &view).unwrap();
    assert!(t < total, "{t} < {total}");
    assert_eq!(p, t - 1);
    let l2 = lines(cx, &view).unwrap();
    assert!(keys_of(&l2, "track::ToggleMute").is_empty(), "{l2:?}");
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
}

// ---------- 성능 ----------

/// 그릴 때마다 모든 바인딩 × `bindings_for_input`(전체 키맵)을 다시 돈다. 기본 키맵과 큰 사용자 키맵에서 잰다.
#[gpui_kit::test]
fn overlay_line_cost(cx: &mut TestAppContext) {
    let mut big = String::from(r#"[{"context": "Normal", "bindings": {"#);
    let letters = "abcdefghijklmnopqrstuvwxyz";
    let mut first = true;
    for a in letters.chars() {
        for b in letters.chars() {
            if !first {
                big.push(',');
            }
            first = false;
            let _ = write!(big, r#""ctrl-alt-shift-{a} {b}": "track::ToggleMute""#);
        }
    }
    big.push_str("}}]");
    start(cx, "perf", Some(&big));
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    let n = cx.read(|cx| cx.key_bindings().borrow().bindings().count());
    let t = Instant::now();
    let l = lines(cx, &view).unwrap();
    let el = t.elapsed();
    eprintln!("bindings={n} lines={} lines_for={el:?}", l.len());
    let t = Instant::now();
    render(cx, handle);
    eprintln!("render_frame={:?}", t.elapsed());
}
