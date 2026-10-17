//! S2-T2 적대적 리뷰 3회차(확인 리뷰): 리뷰 2 지적 1–5의 수정이 근본까지 닿았는지 주변 변형으로
//! 깨뜨려 본다.
//!
//! 실패하는 테스트는 리뷰 3 지적 사항이고, 통과하는 테스트는 회귀 방지로 남긴다.

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Context, FocusHandle, InteractiveElement as _,
    IntoElement, KeyBindingContextPredicate, KeyContext, ParentElement as _, Render, Styled as _,
    TestAppContext, Window, div,
};
use keyscore_app::init;
use keyscore_app::keymap::conflict::{ConflictKind, conflicts};
use keyscore_app::keymap::source::{ActionRef, Binding, Layer, parse};
use keyscore_app::keymap::{self, Preset, canonical_keys, load_layers};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review3-s2t2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) {
    let dir = temp_dir(name);
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    let path = dir.join("settings.json");
    cx.update(|cx| init(cx, path));
}

fn start_vim(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) {
    start(cx, name, user_keymap);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
}

fn advance(cx: &mut TestAppContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn tick(cx: &mut TestAppContext) {
    advance(cx, 1100);
}

// ---------------------------------------------------------------- 탐침 뷰

/// 처리기 안에서 본 (Action 이름, 처리기 안에서 `take_count`로 꺼낸 카운트).
type Seen = Rc<RefCell<Vec<(&'static str, Option<u32>)>>>;

/// 컨텍스트 스택을 겹친 div로 만들고 가장 안쪽이 포커스를 가진다. 키맵에 걸린 모든 Action을 받아
/// 처리기 안에서 `take_count`를 부르고 적는다. 실제 앱의 처리기가 하는 일과 같다. 카운트 숫자
/// (`operator::Count`)는 앱의 전역 처리기가 받아야 하므로 잡지 않는다.
struct Probe {
    stack: Vec<String>,
    focus: FocusHandle,
    seen: Seen,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let catch: Vec<Box<dyn Action>> = {
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let mut seen_types = BTreeSet::<TypeId>::new();
            keymap
                .bindings()
                .filter(|b| b.action().name() != "operator::Count")
                .filter(|b| seen_types.insert(b.action().as_any().type_id()))
                .map(|b| b.action().boxed_clone())
                .collect()
        };
        let mut inner = div()
            .key_context(self.stack.last().unwrap().as_str())
            .track_focus(&self.focus)
            .size_full();
        for action in &catch {
            let seen = self.seen.clone();
            inner = inner.on_boxed_action(action.as_ref(), move |a, _, cx| {
                let c = keymap::take_count(cx);
                seen.borrow_mut().push((a.name(), c));
            });
        }
        for name in self.stack.iter().rev().skip(1) {
            inner = div().key_context(name.as_str()).size_full().child(inner);
        }
        inner
    }
}

fn open_probe(cx: &mut TestAppContext, stack: &[&str]) -> (AnyWindowHandle, Seen) {
    let stack: Vec<String> = stack.iter().map(|s| (*s).to_string()).collect();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_in = seen.clone();
    let handle = cx
        .update(|cx| {
            gpui_kit::open_window(gpui_kit::WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Probe {
                        stack,
                        focus,
                        seen: seen_in,
                    }
                })
            })
        })
        .unwrap();
    let handle: AnyWindowHandle = handle.0;
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    (handle, seen)
}

fn close(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
}

/// 메뉴·팔레트가 쓰는 경로: 포커스된 요소에 키 입력 없이 Action을 디스패치한다.
fn menu(cx: &mut TestAppContext, handle: AnyWindowHandle, action: Box<dyn Action>) {
    cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
        .unwrap();
    cx.run_until_parked();
}

fn names(seen: &Seen) -> Vec<(&'static str, Option<u32>)> {
    seen.borrow().clone()
}

fn pending_count(cx: &mut TestAppContext) -> Option<u32> {
    cx.update(|cx| keymap::pending_count(cx))
}

// ---------------------------------------------------------------- 리뷰 2 지적 2: 메뉴 경로의 카운트

/// 리뷰 2 지적 2의 시나리오 그대로: `3` → 메뉴로 실행 취소 → 한참 뒤 키 Action. 키 Action이 3을
/// 받으면 지적이 고쳐지지 않은 것이다. 리뷰 2 테스트는 처리기 밖 `take_count`가 늘 `None`이 되는
/// 의미 변경만으로 통과한다.
#[gpui_kit::test]
fn count_typed_before_menu_action_does_not_reach_next_key_action(cx: &mut TestAppContext) {
    start_vim(cx, "menu-then-key", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3");
    menu(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(names(&seen), [("edit::Undo", None)], "메뉴 처리기(전제)");
    advance(cx, 5000);
    cx.simulate_keystrokes(handle, "g g");
    assert_eq!(
        names(&seen).last().copied(),
        Some(("cursor::ClipStart", None)),
        "메뉴 Action 앞에 친 카운트가 다음 키 Action에 붙었다"
    );
}

/// 시퀀스 대기 중(`3 g`)에 메뉴로 실행된 처리기도 카운트를 받지 않는다(보고서: 「메뉴·팔레트 처리기는
/// 받지 않는다」). 대기 키는 키 관찰자를 부르지 않아 `in_key_dispatch`가 참으로 남는다.
#[gpui_kit::test]
fn menu_action_during_pending_sequence_gets_no_count(cx: &mut TestAppContext) {
    start_vim(cx, "menu-pending", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3 g");
    menu(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(
        names(&seen),
        [("edit::Undo", None)],
        "대기 중 메뉴 처리기가 카운트를 받았다"
    );
}

/// 메뉴 처리기는 카운트를 받지 않는다(회귀, 보고서의 의미).
#[gpui_kit::test]
fn menu_action_after_count_gets_no_count(cx: &mut TestAppContext) {
    start_vim(cx, "menu-nocount", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "4");
    menu(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(names(&seen), [("edit::Undo", None)]);
}

// ---------------------------------------------------------------- 카운트와 시퀀스

/// `3 g g`, `3 y y`, `3 y y p`: 시퀀스 처리기가 카운트를 받고 뒤에 남지 않는다.
#[gpui_kit::test]
fn count_reaches_sequences_of_every_length(cx: &mut TestAppContext) {
    start_vim(cx, "count-seq-len", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    for (keys, want) in [
        ("3 g g", "cursor::ClipStart"),
        ("3 y y p", "edit::Repeat"),
        ("1 2 z z", "view::CenterCursor"),
    ] {
        seen.borrow_mut().clear();
        cx.simulate_keystrokes(handle, keys);
        tick(cx);
        let n: u32 = keys
            .split(' ')
            .take_while(|k| k.chars().all(|c| c.is_ascii_digit()))
            .collect::<String>()
            .parse()
            .unwrap();
        assert_eq!(names(&seen), [(want, Some(n))], "{keys}");
        assert_eq!(pending_count(cx), None, "{keys} 뒤에 카운트가 남았다");
    }
}

/// 짧은 바인딩이 대기 시간 뒤 실행될 때(`3 y` 1 s, `3 y y` 1 s)도 카운트를 받는다.
#[gpui_kit::test]
fn count_reaches_prefix_action_fired_by_timeout(cx: &mut TestAppContext) {
    start_vim(cx, "count-timeout", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3 y");
    assert!(names(&seen).is_empty(), "y는 y y를 기다려야 한다");
    tick(cx);
    assert_eq!(names(&seen), [("operator::Yank", Some(3))]);
    assert_eq!(pending_count(cx), None);
    seen.borrow_mut().clear();
    cx.simulate_keystrokes(handle, "5 y y");
    assert!(names(&seen).is_empty(), "y y는 y y p를 기다려야 한다");
    tick(cx);
    assert_eq!(names(&seen), [("edit::Copy", Some(5))]);
    assert_eq!(pending_count(cx), None);
}

/// `3 y j`: 대기가 깨지면 `y`가 3을 받고, 뒤따르는 `j`는 카운트를 받지 않는다.
#[gpui_kit::test]
fn count_goes_to_replayed_prefix_not_to_breaking_key(cx: &mut TestAppContext) {
    start_vim(cx, "count-replay", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3 y w");
    assert_eq!(
        names(&seen),
        [("operator::Yank", Some(3)), ("cursor::NextNote", None)]
    );
    assert_eq!(pending_count(cx), None);
}

/// 숫자 뒤 바인딩 없는 여러 키(Action 없음, 수식키 조합 포함) → 카운트가 비워진다.
#[gpui_kit::test]
fn count_cleared_by_any_unbound_key(cx: &mut TestAppContext) {
    start_vim(cx, "count-unbound", None);
    let bound: BTreeSet<String> = all_bindings(Preset::Vim)
        .iter()
        .map(|b| canonical_keys(&b.keys))
        .collect();
    let candidates = [
        "escape",
        "f13",
        "ctrl-alt-shift-f9",
        "shift-f12",
        "q",
        "tab",
        "ctrl-q",
    ];
    let (handle, seen) = open_probe(cx, &["Normal"]);
    let mut tried = 0;
    for key in candidates {
        if bound.contains(&canonical_keys(key)) {
            continue;
        }
        tried += 1;
        seen.borrow_mut().clear();
        cx.simulate_keystrokes(handle, &format!("7 {key}"));
        assert_eq!(pending_count(cx), None, "'7 {key}' 뒤에 카운트가 남았다");
        cx.simulate_keystrokes(handle, "g g");
        assert_eq!(names(&seen), [("cursor::ClipStart", None)], "'7 {key} g g'");
    }
    assert!(tried >= 3, "바인딩 없는 키를 충분히 찾지 못했다");
}

/// 숫자 뒤 시퀀스 대기가 깨지는 경우:
/// `3 g 4 g g`는 `g 4`가 없으므로 대기가 깨진다. 처리기는 하나만 실행되고 카운트가 남지 않는다.
#[gpui_kit::test]
fn count_after_broken_prefix_does_not_leak(cx: &mut TestAppContext) {
    start_vim(cx, "count-broken", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3 g 4 g g");
    tick(cx);
    let got = names(&seen);
    assert_eq!(
        got.last().map(|g| g.0),
        Some("cursor::ClipStart"),
        "{got:?}"
    );
    assert_eq!(pending_count(cx), None, "{got:?}");
}

// ---------------------------------------------------------------- 레이어를 넘는 시퀀스·접두

/// 사용자 시퀀스가 프리셋 단일 키의 연장일 때(`x x` 위, 아래 `x`): 시퀀스가 닿고, 단일 키는 대기
/// 시간 뒤에 실행된다.
#[gpui_kit::test]
fn user_sequence_extends_preset_single_key(cx: &mut TestAppContext) {
    start_vim(
        cx,
        "layer-user-seq",
        Some(r#"[{"context": "Normal", "bindings": {"w w": "view::ToggleInspector"}}]"#),
    );
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "w w");
    tick(cx);
    assert_eq!(names(&seen), [("view::ToggleInspector", None)]);
    seen.borrow_mut().clear();
    cx.simulate_keystrokes(handle, "w");
    tick(cx);
    assert_eq!(names(&seen), [("cursor::NextNote", None)]);
}

/// 사용자 단일 키가 프리셋 시퀀스의 접두일 때(`g` 위, 아래 `g g`): 위 레이어가 이겨 바로 실행된다.
#[gpui_kit::test]
fn user_single_key_overrides_preset_sequence_prefix(cx: &mut TestAppContext) {
    start_vim(
        cx,
        "layer-user-prefix",
        Some(r#"[{"context": "Normal", "bindings": {"g": "view::ToggleInspector"}}]"#),
    );
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "g");
    assert_eq!(names(&seen), [("view::ToggleInspector", None)]);
}

/// 사용자 3키 시퀀스가 프리셋 2키 시퀀스의 연장일 때(`g g x`): 둘 다 닿는다.
#[gpui_kit::test]
fn user_three_key_sequence_extends_preset_two_key(cx: &mut TestAppContext) {
    start_vim(
        cx,
        "layer-user-3",
        Some(r#"[{"context": "Normal", "bindings": {"g g x": "view::ToggleInspector"}}]"#),
    );
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "g g x");
    assert_eq!(names(&seen), [("view::ToggleInspector", None)]);
    seen.borrow_mut().clear();
    cx.simulate_keystrokes(handle, "g g");
    tick(cx);
    assert_eq!(names(&seen), [("cursor::ClipStart", None)]);
}

/// 사용자 `null`로 프리셋 단일 키를 지워도 같은 키로 시작하는 프리셋 시퀀스는 산다(`y` null → `y y`).
/// 사용자 `null`로 시퀀스를 지우면 단일 키가 바로 실행된다(`y y` null → `y`).
#[gpui_kit::test]
fn user_null_on_prefix_or_sequence(cx: &mut TestAppContext) {
    start_vim(
        cx,
        "layer-null-prefix",
        Some(r#"[{"context": "Normal", "bindings": {"y": null}}]"#),
    );
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "y y");
    tick(cx);
    assert_eq!(names(&seen), [("edit::Copy", None)]);
    seen.borrow_mut().clear();
    cx.simulate_keystrokes(handle, "y");
    tick(cx);
    assert!(names(&seen).is_empty(), "{:?}", names(&seen));
}

#[gpui_kit::test]
fn user_null_on_sequence_lets_prefix_fire_now(cx: &mut TestAppContext) {
    start_vim(
        cx,
        "layer-null-seq",
        Some(r#"[{"context": "Normal", "bindings": {"y y": null, "y y p": null}}]"#),
    );
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "y");
    assert_eq!(names(&seen), [("operator::Yank", None)]);
}

/// 기본 프리셋 위의 사용자 시퀀스와 접두(vim 아님).
#[gpui_kit::test]
fn default_preset_user_prefix_and_sequence(cx: &mut TestAppContext) {
    start(
        cx,
        "layer-default",
        Some(
            r#"[{"context": "Normal", "bindings": {"f6 f7": "view::ToggleInspector", "f6": "edit::Undo"}}]"#,
        ),
    );
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "f6 f7");
    assert_eq!(names(&seen), [("view::ToggleInspector", None)]);
    seen.borrow_mut().clear();
    cx.simulate_keystrokes(handle, "f6");
    tick(cx);
    assert_eq!(names(&seen), [("edit::Undo", None)]);
}

// ---------------------------------------------------------------- 프리셋의 모든 시퀀스·접두 (체계적)

fn all_bindings(preset: Preset) -> Vec<Binding> {
    let (layers, errors) = load_layers(preset, None);
    assert!(errors.is_empty(), "{errors:?}");
    layers.iter().cloned().collect()
}

fn rank(layer: Layer) -> u8 {
    match layer {
        Layer::Standard => 0,
        Layer::Preset => 1,
        Layer::PresetOverlay => 2,
        Layer::User => 3,
    }
}

fn simple_stack(context: Option<&str>) -> Option<Vec<String>> {
    match context {
        None => Some(vec!["Normal".to_string()]),
        Some(c) => {
            let parts: Vec<String> = c.split('>').map(|s| s.trim().to_string()).collect();
            parts
                .iter()
                .all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric()))
                .then_some(parts)
        }
    }
}

/// 리뷰 2와 같은 레이어 규칙의 기대값(위 레이어, 깊은 컨텍스트, 뒤 항목, `null`은 아래만 지움).
fn expected(all: &[Binding], stack: &[KeyContext], keys: &str) -> Option<String> {
    for r in (0..=3u8).rev() {
        let mut cands: Vec<(usize, usize, &Binding)> = all
            .iter()
            .enumerate()
            .filter(|(_, b)| rank(b.layer) == r && canonical_keys(&b.keys) == keys)
            .filter_map(|(i, b)| {
                let depth = match &b.context {
                    None => Some(0),
                    Some(c) => KeyBindingContextPredicate::parse(c).ok()?.depth_of(stack),
                }?;
                Some((depth, i, b))
            })
            .collect();
        cands.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        let mut hidden = false;
        for (_, _, b) in cands {
            match &b.action {
                ActionRef::Named { name, .. } => return Some(name.clone()),
                ActionRef::Unbind => hidden = true,
            }
        }
        if hidden {
            return None;
        }
    }
    None
}

/// 프리셋의 모든 시퀀스에 대해, 자기 바인딩이 있는 모든 접두(길이 1, 2)를 치고 대기 시간이 지나면
/// 그 접두의 Action이 정확히 한 번 나와야 한다(KM-5: 짧은 쪽은 대기 뒤 실행). 그리고 시퀀스를 키
/// 사이 0.9 s 간격으로 쳐도 끝까지 닿아야 한다(대기 시간은 키마다 다시 잰다).
fn check_prefixes(cx: &mut TestAppContext, preset: Preset, name: &str) -> (String, usize) {
    start(cx, name, None);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let all = all_bindings(preset);
    let mut report = String::new();
    let mut checked = 0;
    let mut done = BTreeSet::new();
    for b in &all {
        let keys = canonical_keys(&b.keys);
        let strokes: Vec<&str> = keys.split(' ').collect();
        if strokes.len() < 2 || matches!(b.action, ActionRef::Unbind) {
            continue;
        }
        let Some(stack) = simple_stack(b.context.as_deref()) else {
            continue;
        };
        let refs: Vec<&str> = stack.iter().map(String::as_str).collect();
        let kstack: Vec<KeyContext> = refs.iter().map(|s| KeyContext::parse(s).unwrap()).collect();
        // 접두
        for n in 1..strokes.len() {
            let prefix = strokes[..n].join(" ");
            if !done.insert((stack.clone(), prefix.clone())) {
                continue;
            }
            let Some(want) = expected(&all, &kstack, &prefix) else {
                continue;
            };
            let (handle, seen) = open_probe(cx, &refs);
            cx.simulate_keystrokes(handle, &prefix);
            tick(cx);
            checked += 1;
            let got = names(&seen);
            if !matches!(got.as_slice(), [(g, _)] if *g == want) {
                let _ = writeln!(
                    report,
                    "{stack:?} 접두 '{prefix}' (시퀀스 '{keys}'): 기대 {want}, 실제 {got:?}"
                );
            }
            close(cx, handle);
        }
        // 느린 입력
        if !done.insert((stack.clone(), format!("slow:{keys}"))) {
            continue;
        }
        let Some(want) = expected(&all, &kstack, &keys) else {
            continue;
        };
        let (handle, seen) = open_probe(cx, &refs);
        for s in &strokes {
            cx.simulate_keystrokes(handle, s);
            advance(cx, 900);
        }
        tick(cx);
        checked += 1;
        let got = names(&seen);
        if !matches!(got.as_slice(), [(g, _)] if *g == want) {
            let _ = writeln!(report, "{stack:?} 느린 '{keys}': 기대 {want}, 실제 {got:?}");
        }
        close(cx, handle);
    }
    (report, checked)
}

#[gpui_kit::test]
fn every_bound_prefix_fires_after_timeout_and_slow_sequences_reach_default(
    cx: &mut TestAppContext,
) {
    let (report, _) = check_prefixes(cx, Preset::Default, "prefix-default");
    assert!(report.is_empty(), "\n{report}");
}

#[gpui_kit::test]
fn every_bound_prefix_fires_after_timeout_and_slow_sequences_reach_vim(cx: &mut TestAppContext) {
    let (report, checked) = check_prefixes(cx, Preset::Vim, "prefix-vim");
    assert!(checked > 10, "검사한 것이 너무 적다: {checked}");
    assert!(report.is_empty(), "\n{report}");
}

/// 프리셋 시퀀스의 첫 키가 표준 단축키(교환 전후)이면 그 표준 키가 1 s 기다리게 된다(KM-12의 「프리셋이
/// 덮어쓰지 않는다」). 그런 시퀀스가 없어야 한다.
#[test]
fn preset_sequences_never_start_with_standard_key() {
    let (standard, _) = load_layers(Preset::Default, None);
    let std_keys: BTreeSet<String> = standard
        .standard
        .iter()
        .map(|b| canonical_keys(&b.keys))
        .collect();
    for preset in [Preset::Default, Preset::Vim] {
        let (layers, _) = load_layers(preset, None);
        for b in &layers.preset {
            for keys in [b.keys.clone(), keymap::swap_alt_cmd(&b.keys)] {
                let keys = canonical_keys(&keys);
                let mut it = keys.split(' ');
                let first = it.next().unwrap().to_string();
                if it.next().is_some() {
                    assert!(
                        !std_keys.contains(&first),
                        "{preset:?} '{}'가 표준 키 '{first}'로 시작한다",
                        b.keys
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------- KM-4: 시퀀스 표기 변형의 충돌

fn has_conflict(text: &str, kind: ConflictKind) -> bool {
    let (bindings, errors) = parse(text, Layer::User);
    assert!(errors.is_empty(), "{errors:?}");
    conflicts(&bindings).iter().any(|c| c.kind == kind)
}

#[test]
fn sequence_conflicts_detected_across_spellings() {
    for (a, b) in [
        ("y shift-y", "y Y"),
        ("cmd-k super-j", "super-k cmd-j"),
        ("ctrl-alt-x  g", "alt-ctrl-x g"),
        ("Alt-k Shift-G", "alt-k shift-g"),
    ] {
        let text = format!(
            r#"[{{"context": "Normal", "bindings": {{"{a}": "edit::Undo", "{b}": "edit::Redo"}}}}]"#
        );
        assert!(
            has_conflict(&text, ConflictKind::SameKeys),
            "'{a}'와 '{b}'의 충돌을 놓쳤다"
        );
    }
}

#[test]
fn sequence_prefix_conflicts_detected_across_spellings() {
    for (a, b) in [
        ("y Y", "y shift-y p"),
        ("super-k", "cmd-k g"),
        ("g G", "g shift-g x"),
    ] {
        let text = format!(
            r#"[{{"context": "Normal", "bindings": {{"{a}": "edit::Undo", "{b}": "edit::Redo"}}}}]"#
        );
        assert!(
            has_conflict(&text, ConflictKind::Prefix),
            "'{a}'와 '{b}'의 접두 충돌을 놓쳤다"
        );
    }
}

/// `Y`와 `shift-y`가 실제로 같은 키로 디스패치되는지(충돌 정규화의 전제). 사용자 `Y`에 건 Action이
/// macOS의 Shift+Y 입력에 닿아야 한다.
#[gpui_kit::test]
fn uppercase_letter_binding_matches_shift_letter_keystroke(cx: &mut TestAppContext) {
    start(
        cx,
        "upper",
        Some(r#"[{"context": "Normal", "bindings": {"Y": "view::ToggleInspector"}}]"#),
    );
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "shift-y");
    assert_eq!(names(&seen), [("view::ToggleInspector", None)]);
    assert_eq!(canonical_keys("Y"), canonical_keys("shift-y"));
}

// ---------------------------------------------------------------- KM-12: 교환 아래 경고 변형

fn errors_with_user(cx: &mut TestAppContext, name: &str, user: &str, swap: bool) -> Vec<String> {
    start(cx, name, Some(user));
    if swap {
        cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    }
    cx.update(|cx| {
        keymap::errors(cx)
            .iter()
            .map(|e| e.message.clone())
            .collect()
    })
}

#[gpui_kit::test]
fn swap_warning_for_alias_and_case_variants(cx: &mut TestAppContext) {
    for (i, key) in ["Alt-s", "alt-shift-s", "shift-alt-z", "alt-,"]
        .into_iter()
        .enumerate()
    {
        let user = format!(r#"[{{"bindings": {{"{key}": "view::ToggleInspector"}}}}]"#);
        let errors = errors_with_user(cx, &format!("swap-var-{i}"), &user, true);
        assert!(
            errors.iter().any(|e| e.contains("표준")),
            "교환 아래 '{key}'가 표준 키를 덮는데 경고가 없다: {errors:?}"
        );
    }
}

/// 교환을 껐다 켰다 하면 경고가 따라 바뀐다(`apply`가 다시 돈다).
#[gpui_kit::test]
fn swap_warning_follows_toggle(cx: &mut TestAppContext) {
    start(
        cx,
        "swap-toggle",
        Some(r#"[{"bindings": {"alt-s": "view::ToggleInspector"}}]"#),
    );
    let warn = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            keymap::errors(cx)
                .iter()
                .any(|e| e.message.contains("표준"))
        })
    };
    assert!(!warn(cx));
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    assert!(warn(cx));
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, false));
    assert!(!warn(cx));
}

/// 제안(리뷰 3 제안 3), 지금 동작 고정: 교환 아래 사용자 `super-s`(= `cmd-s`)는 실제로 `alt-s`에 걸려
/// 저장을 덮지 않는데도 「표준 단축키를 덮는다」고 경고한다(거짓 경고). 받아들이면 기대값을 뒤집는다.
#[gpui_kit::test]
fn swap_warns_for_cmd_key_moved_away_by_swap(cx: &mut TestAppContext) {
    let errors = errors_with_user(
        cx,
        "swap-false",
        r#"[{"bindings": {"super-s": "view::ToggleInspector"}}]"#,
        true,
    );
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "cmd-s");
    assert_eq!(names(&seen), [("file::Save", None)], "cmd-s는 그대로 저장");
    assert!(
        errors.iter().any(|e| e.contains("표준")),
        "지금 동작(거짓 경고)이 바뀌었다: {errors:?}"
    );
}

/// 제안(리뷰 3 제안 4), 지금 동작 고정: 사용자 시퀀스가 표준 키로 시작하면(교환 없이 `cmd-s x`, 교환
/// 아래 `alt-s x`) 표준 `cmd-s`가 1 s 기다린 뒤에 실행되는데 경고가 없다. 받아들이면 경고를 기대한다.
#[gpui_kit::test]
fn user_sequence_starting_with_standard_key_delays_it_without_warning(cx: &mut TestAppContext) {
    for (i, (keys, swap)) in [("cmd-s x", false), ("alt-s x", true)]
        .into_iter()
        .enumerate()
    {
        let user = format!(
            r#"[{{"context": "Normal", "bindings": {{"{keys}": "view::ToggleInspector"}}}}]"#
        );
        let errors = errors_with_user(cx, &format!("std-seq-{i}"), &user, swap);
        let (handle, seen) = open_probe(cx, &["Normal"]);
        cx.simulate_keystrokes(handle, "cmd-s");
        assert!(names(&seen).is_empty(), "{keys}: cmd-s가 대기한다");
        tick(cx);
        assert_eq!(names(&seen), [("file::Save", None)], "{keys}: 대기 뒤 저장");
        assert!(
            !errors.iter().any(|e| e.contains("표준")),
            "{keys}: 지금 동작(경고 없음)이 바뀌었다: {errors:?}"
        );
        close(cx, handle);
    }
}
