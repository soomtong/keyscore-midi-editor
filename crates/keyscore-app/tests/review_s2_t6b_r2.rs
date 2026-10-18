//! S2-T6b 적대적 리뷰 2회차: 리뷰 1 수정(모드 추종, 줄 캐시, 변형, Kbd, 팔레트·`?` 닫기)을 깨뜨리려는 시도.
//!
//! `overlay_lines`는 캐시를 거치지 않고 매번 다시 구한다. 그래서 캐시가 낡는지는 실제로 그린 줄(`overlay-row`의
//! 접근성 이름)과 머리말로 본다.

use std::path::PathBuf;
use std::time::Instant;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{help, mode};
use keyscore_app::keymap;
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::ksq;

const PER_PAGE: usize = 24;

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review2-t6b-{}-{name}", std::process::id()));
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

fn mode_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn header(cx: &mut TestAppContext, handle: AnyWindowHandle) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find("keymap-overlay")
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

/// 지난 프레임에 실제로 그린 줄(캐시를 거친 것).
fn rendered(cx: &mut TestAppContext, handle: AnyWindowHandle) -> Vec<String> {
    cx.update_window(handle, |_, window, _| {
        (0..PER_PAGE)
            .filter_map(|i| {
                window
                    .try_find(("overlay-row", i))
                    .and_then(|e| e.label().map(str::to_owned))
            })
            .collect()
    })
    .unwrap()
}

/// 그린 줄·머리말이 지금 모드·지금 키맵으로 새로 구한 줄과 같은가. 캐시가 낡으면 어긋난다.
fn assert_fresh(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    what: &str,
) {
    let fresh = lines(cx, view).expect("오버레이가 닫혀 있다");
    let (page, pages) = cx.read(|cx| view.read(cx).overlay_page()).unwrap();
    let path = cx.read(|cx| view.read(cx).mode_state().context_path().join(" > "));
    let h = header(cx, handle).unwrap_or_default();
    assert!(
        h.contains(&format!("키맵 · {path} · {}/{pages} 쪽", page + 1)),
        "{what}: 머리말이 지금 경로({path})·쪽과 다르다: {h}"
    );
    let expected: Vec<String> = fresh
        .iter()
        .skip(page * PER_PAGE)
        .take(PER_PAGE)
        .cloned()
        .collect();
    assert_eq!(rendered(cx, handle), expected, "{what}: 그린 줄이 낡았다");
}

// ---------- 캐시: 모드·컨텍스트·키맵 변화 ----------

/// 열린 채 `ctrl-tab`/`ctrl-shift-tab`으로 모드를 돌리면 그린 줄과 머리말이 매번 지금 모드를 따른다.
#[gpui_kit::test]
fn rendered_rows_follow_mode_cycle(cx: &mut TestAppContext) {
    start(cx, "cycle", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    assert_fresh(cx, handle, &view, "열기");
    let mut seen = vec![mode_of(cx, &view)];
    for _ in 0..8 {
        press(cx, handle, "ctrl-tab");
        assert!(lines(cx, &view).is_some(), "ctrl-tab이 오버레이를 닫았다");
        let m = mode_of(cx, &view);
        assert_fresh(cx, handle, &view, &format!("{m:?}"));
        seen.push(mode_of(cx, &view));
    }
    press(cx, handle, "ctrl-shift-tab");
    assert_fresh(cx, handle, &view, "ctrl-shift-tab");
    seen.dedup();
    assert!(seen.len() > 2, "모드가 돌지 않았다: {seen:?}");
}

/// 열린 채 메뉴·팔레트 경로의 모드 Action(포커스는 오버레이)을 내도 오버레이가 따라가고, Esc는 오버레이만 닫는다.
#[gpui_kit::test]
fn rendered_rows_follow_mode_actions(cx: &mut TestAppContext) {
    start(cx, "modeact", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    run(cx, handle, mode::Step);
    assert_eq!(mode_of(cx, &view), Mode::Step);
    assert_fresh(cx, handle, &view, "Step");
    run(cx, handle, mode::Lyric);
    assert_fresh(cx, handle, &view, "Lyric");
    run(cx, handle, mode::Normal);
    assert_fresh(cx, handle, &view, "Normal");
    run(cx, handle, mode::Step);
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    assert_eq!(mode_of(cx, &view), Mode::Step, "Esc가 Step도 나갔다");
}

/// 열린 채 선택·고스트가 켜지고 꺼져도(같은 모드, 다른 경로 조각) 그린 줄이 따라간다. 마지막 쪽에서도.
#[gpui_kit::test]
fn rendered_rows_follow_selection_and_ghost(cx: &mut TestAppContext) {
    start(cx, "selghost", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    for _ in 0..10 {
        press(cx, handle, "tab");
    }
    event(cx, handle, &view, ModeEvent::Select);
    assert_fresh(cx, handle, &view, "선택 켬");
    event(cx, handle, &view, ModeEvent::ClearSelection);
    assert_fresh(cx, handle, &view, "선택 끔");
    run(cx, handle, mode::Step);
    event(cx, handle, &view, ModeEvent::ShowGhost);
    assert_fresh(cx, handle, &view, "고스트 켬");
    event(cx, handle, &view, ModeEvent::DismissGhost);
    assert_fresh(cx, handle, &view, "고스트 끔");
}

/// 열린 채 키맵이 바뀌면(사용자 파일 다시 읽기, 수식키 교환, 프리셋) 그린 줄이 따라간다.
#[gpui_kit::test]
fn rendered_rows_follow_keymap_changes(cx: &mut TestAppContext) {
    let dir = start(cx, "reload", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    // 뮤트 줄이 있는 쪽으로 가서, 다시 읽은 뒤 그 쪽의 그린 줄이 실제로 바뀌는지 본다
    let idx = lines(cx, &view)
        .unwrap()
        .iter()
        .position(|l| l.ends_with("(track::ToggleMute)"))
        .unwrap();
    for _ in 0..idx / PER_PAGE {
        press(cx, handle, "tab");
    }
    let before = rendered(cx, handle);
    assert!(before.iter().any(|l| l.starts_with("m · ")), "{before:?}");
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"context": "Normal", "bindings": {"m": null, "ctrl-shift-alt-q": "track::ToggleMute"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    cx.run_until_parked();
    render(cx, handle);
    assert_fresh(cx, handle, &view, "사용자 키맵");
    let after = rendered(cx, handle);
    assert!(!after.iter().any(|l| l.starts_with("m · ")), "{after:?}");
    assert!(
        lines(cx, &view)
            .unwrap()
            .iter()
            .any(|l| l.contains("ctrl-alt-shift-q") && l.ends_with("(track::ToggleMute)")),
        "새 키가 없다"
    );
    // 같은 내용으로 다시 읽어도(판만 오름) 결과는 같다
    cx.update(keymap::apply);
    cx.run_until_parked();
    render(cx, handle);
    assert_fresh(cx, handle, &view, "같은 파일 다시 읽기");
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    cx.run_until_parked();
    render(cx, handle);
    assert_fresh(cx, handle, &view, "수식키 교환");
    cx.update(|cx| keymap::set_preset(cx, keymap::Preset::Vim));
    cx.run_until_parked();
    render(cx, handle);
    assert_fresh(cx, handle, &view, "vim 프리셋");
}

// ---------- `?` 토글, 배타 ----------

/// `?`로 열고 닫기를 여러 번 되풀이해도 매번 맞게 바뀌고, 닫힌 뒤에는 모드 키가 산다.
#[gpui_kit::test]
fn question_mark_toggles_repeatedly(cx: &mut TestAppContext) {
    start(cx, "toggle", None);
    let (handle, view) = open(cx);
    for i in 0..4 {
        press(cx, handle, "?");
        assert!(lines(cx, &view).is_some(), "{i}: 열리지 않았다");
        press(cx, handle, "tab");
        press(cx, handle, "?");
        assert!(lines(cx, &view).is_none(), "{i}: 닫히지 않았다");
    }
    // 다시 열면 첫 쪽부터
    press(cx, handle, "?");
    assert_eq!(
        cx.read(|cx| view.read(cx).overlay_page()).map(|p| p.0),
        Some(0)
    );
    press(cx, handle, "?");
    press(cx, handle, "m");
    assert!(muted(cx, &view));
}

/// `?`가 Normal에만 걸린 키라 Step에서는 오버레이를 열지 않는다(메뉴로 연 뒤의 `?`는 닫는다).
#[gpui_kit::test]
fn question_mark_in_step_does_not_open_but_closes(cx: &mut TestAppContext) {
    start(cx, "stepq", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Step);
    press(cx, handle, "?");
    assert!(lines(cx, &view).is_none(), "Step의 ?가 오버레이를 열었다");
    run(cx, handle, help::KeymapOverlay);
    assert!(lines(cx, &view).is_some());
    press(cx, handle, "?");
    assert!(lines(cx, &view).is_none());
    assert_eq!(mode_of(cx, &view), Mode::Step);
}

/// 모드 메뉴가 열린 채 메뉴의 「키맵 보기」를 내면 팝업이 닫히고 오버레이만 남는다. 반대로 오버레이 위 팔레트는
/// 오버레이를 닫는다. 어느 쪽이든 둘이 함께 뜨지 않는다.
#[gpui_kit::test]
fn popup_palette_overlay_are_exclusive(cx: &mut TestAppContext) {
    start(cx, "excl", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::ShowMenu);
    assert!(cx.read(|cx| view.read(cx).mode_menu_open()));
    run(cx, handle, help::KeymapOverlay);
    assert!(lines(cx, &view).is_some());
    assert!(
        !cx.read(|cx| view.read(cx).mode_menu_open()),
        "팝업과 오버레이가 함께 떴다"
    );
    press(cx, handle, "m");
    assert!(!muted(cx, &view));
    press(cx, handle, "cmd-shift-p");
    assert!(lines(cx, &view).is_none());
    assert!(cx.read(|cx| view.read(cx).palette_query(cx).is_some()));
    // 팔레트가 열린 채 메뉴로 키맵 보기
    run(cx, handle, help::KeymapOverlay);
    assert!(lines(cx, &view).is_some());
    assert!(
        cx.read(|cx| view.read(cx).palette_query(cx).is_none()),
        "팔레트와 오버레이가 함께 떴다"
    );
    press(cx, handle, "escape");
    assert!(lines(cx, &view).is_none());
    press(cx, handle, "m");
    assert!(muted(cx, &view), "닫은 뒤 포커스가 문서로 돌아오지 않았다");
}

// ---------- Kbd ----------

/// Step 오버레이는 키를 `Kbd`로 그린다(`space`, 시퀀스가 아닌 숫자 키).
#[gpui_kit::test]
fn step_keys_are_painted_as_kbd(cx: &mut TestAppContext) {
    start(cx, "kbd", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Step);
    run(cx, handle, help::KeymapOverlay);
    let l = lines(cx, &view).unwrap();
    let first_page: Vec<_> = l.iter().take(PER_PAGE).collect();
    let mut vcx = gpui_kit::VisualTestContext::from_window(handle, cx);
    let mut checked = 0;
    for key in ["space", "1", "escape"] {
        let on_page = first_page.iter().any(|x| {
            x.split(" · ")
                .next()
                .unwrap_or("")
                .split(", ")
                .any(|k| k == key)
        });
        if on_page {
            let sel: &'static str = Box::leak(format!("kbd:{key}").into_boxed_str());
            assert!(
                vcx.debug_bounds(sel).is_some(),
                "{key}가 Kbd로 그려지지 않았다"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "첫 쪽에 검사할 키가 없다: {first_page:?}");
}

// ---------- 성능 ----------

/// 큰 사용자 키맵에서 열린 채 다시 그리기가 캐시를 써서 싸다.
#[gpui_kit::test]
fn cached_render_is_cheap_with_big_keymap(cx: &mut TestAppContext) {
    use std::fmt::Write as _;
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
    let (handle, _view) = open(cx);
    let t = Instant::now();
    render(cx, handle);
    let closed = t.elapsed();
    let t = Instant::now();
    press(cx, handle, "?");
    let opened = t.elapsed();
    let t = Instant::now();
    for _ in 0..5 {
        render(cx, handle);
    }
    let cached = t.elapsed() / 5;
    let t = Instant::now();
    press(cx, handle, "ctrl-tab");
    let after_mode = t.elapsed();
    eprintln!(
        "closed={closed:?} open={opened:?} cached_render={cached:?} mode_change={after_mode:?}"
    );
    assert!(
        cached < closed * 3 + std::time::Duration::from_millis(20),
        "캐시가 안 듣는다: {cached:?} vs {closed:?}"
    );
}
