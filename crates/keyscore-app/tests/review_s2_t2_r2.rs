//! S2-T2 적대적 리뷰 2회차: 리뷰 1 지적 1–9의 수정이 근본까지 닿았는지 주변 변형으로 깨뜨려 본다.
//!
//! 실패하는 테스트는 리뷰 2 지적 사항이고, 통과하는 테스트는 회귀 방지로 남긴다.

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
    IntoElement, KeyBindingContextPredicate, KeyContext, Keystroke, ParentElement as _, Render,
    Styled as _, TestAppContext, Window, div,
};
use keyscore_app::actions::cursor::ClipStart;
use keyscore_app::actions::view::ToggleInspector;
use keyscore_app::app_state::AppSettings;
use keyscore_app::init;
use keyscore_app::keymap::conflict::{ConflictKind, conflicts};
use keyscore_app::keymap::source::{ActionRef, Binding, Layer, parse};
use keyscore_app::keymap::{self, Preset, canonical_keys, load_layers};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review2-s2t2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start_in(cx: &mut TestAppContext, dir: &std::path::Path, user_keymap: Option<&str>) {
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    let path = dir.join("settings.json");
    cx.update(|cx| init(cx, path));
}

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = temp_dir(name);
    start_in(cx, &dir, user_keymap);
    dir
}

fn tick(cx: &mut TestAppContext) {
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
}

// ---------------------------------------------------------------- 탐침 뷰

/// 처리기 안에서 본 (Action 이름, 꺼낸 카운트).
type Seen = Rc<RefCell<Vec<(&'static str, Option<u32>)>>>;

/// 컨텍스트 스택을 겹친 div로 만들고 가장 안쪽이 포커스를 가진다. 키맵에 걸린 모든 Action을 받아
/// 처리한 것으로 친다(그래야 GPUI가 다음 후보로 넘어가지 않고 키 관찰자에 Action을 넘긴다).
/// `catch_all`이 거짓이면 아래 `handled`에 적힌 Action만 받는다.
struct Probe {
    stack: Vec<String>,
    focus: FocusHandle,
    catch_all: bool,
    /// 처리기 안에서 `take_count`로 꺼낸 카운트(카운트 테스트용).
    seen_counts: Seen,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let catch: Vec<Box<dyn Action>> = if self.catch_all {
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let mut seen_types = BTreeSet::<TypeId>::new();
            keymap
                .bindings()
                .filter(|b| seen_types.insert(b.action().as_any().type_id()))
                .map(|b| b.action().boxed_clone())
                .collect()
        } else {
            Vec::new()
        };
        let seen = self.seen_counts.clone();
        let seen2 = self.seen_counts.clone();
        let mut inner = div()
            .key_context(self.stack.last().unwrap().as_str())
            .track_focus(&self.focus)
            .size_full()
            .on_action(move |_: &ToggleInspector, _, cx| {
                let c = keymap::take_count(cx);
                seen.borrow_mut().push(("view::ToggleInspector", c));
            })
            .on_action(move |_: &ClipStart, _, cx| {
                let c = keymap::take_count(cx);
                seen2.borrow_mut().push(("cursor::ClipStart", c));
            });
        // 키맵의 모든 Action을 처리한 것으로 둔다(전파 멈춤이 기본값).
        for action in &catch {
            inner = inner.on_boxed_action(action.as_ref(), |_, _, _| {});
        }
        for name in self.stack.iter().rev().skip(1) {
            inner = div().key_context(name.as_str()).size_full().child(inner);
        }
        inner
    }
}

fn open_probe(cx: &mut TestAppContext, stack: &[&str], catch_all: bool) -> (AnyWindowHandle, Seen) {
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
                        catch_all,
                        seen_counts: seen_in,
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

/// 키로 실행된 Action을 모두 적는다.
fn record_actions(cx: &mut TestAppContext) -> Rc<RefCell<Vec<Box<dyn Action>>>> {
    let log: Rc<RefCell<Vec<Box<dyn Action>>>> = Rc::new(RefCell::new(Vec::new()));
    let log_in = log.clone();
    cx.update(|cx| {
        cx.observe_keystrokes(move |event, _, _| {
            if let Some(action) = &event.action {
                log_in.borrow_mut().push(action.boxed_clone());
            }
        })
        .detach();
    });
    log
}

// ---------------------------------------------------------------- 레이어 규칙으로 계산한 기대값

fn rank(layer: Layer) -> u8 {
    match layer {
        Layer::Standard => 0,
        Layer::Preset => 1,
        Layer::PresetOverlay => 2,
        Layer::User => 3,
    }
}

fn stack_of(context: &str) -> Option<Vec<KeyContext>> {
    context
        .split('>')
        .map(|s| {
            let s = s.trim();
            (!s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                .then(|| KeyContext::parse(s).unwrap())
        })
        .collect()
}

/// 카탈로그가 뜻하는 결과: 위 레이어가 아래 레이어를 이긴다(vim 프리셋은 기본 프리셋 위에 얹힌다).
/// 한 레이어 안에서는 깊은 컨텍스트, 같으면 뒤 항목이 이긴다. 전역 바인딩은 그 레이어에서 가장 얕다.
/// `null`은 아래 레이어의 같은 키를 지운다(같은 레이어의 얕은 바인딩은 남는다).
fn expected(
    all: &[Binding],
    stack: &[KeyContext],
    keys: &str,
) -> Option<(String, Option<serde_json::Value>)> {
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
                ActionRef::Named { name, args } => return Some((name.clone(), args.clone())),
                ActionRef::Unbind => hidden = true,
            }
        }
        if hidden {
            return None;
        }
    }
    None
}

fn all_bindings(preset: Preset) -> Vec<Binding> {
    let (layers, errors) = load_layers(preset, None);
    assert!(errors.is_empty(), "{errors:?}");
    layers.iter().cloned().collect()
}

/// 그 키에 그 Action을 거는 바탕 레이어 바인딩이 있는가.
fn from_base(cx: &mut TestAppContext, key: &str, action: &str) -> bool {
    cx.update(|cx| {
        keymap::base_bindings(cx).iter().any(|b| {
            b.action().name() == action
                && b.keystrokes()
                    .iter()
                    .map(gpui_kit::KeybindingKeystroke::unparse)
                    .collect::<Vec<_>>()
                    .join(" ")
                    == key
        })
    })
}

fn describe(action: &dyn Action) -> String {
    format!("{} {:?}", action.name(), action)
}

fn same(
    cx: &mut TestAppContext,
    got: &dyn Action,
    want: &(String, Option<serde_json::Value>),
) -> bool {
    let built = cx.update(|cx| cx.build_action(&want.0, want.1.clone()));
    built.is_ok_and(|w| w.partial_eq(got))
}

fn stacks_for(all: &[Binding]) -> Vec<Vec<String>> {
    let mut out: BTreeSet<Vec<String>> = [
        vec!["Normal"],
        vec!["Normal", "PianoRoll"],
        vec!["Normal", "Arrangement"],
        vec!["Step"],
    ]
    .into_iter()
    .map(|v| v.into_iter().map(str::to_string).collect())
    .collect();
    for b in all {
        if let Some(c) = &b.context
            && stack_of(c).is_some()
        {
            out.insert(c.split('>').map(|s| s.trim().to_string()).collect());
        }
    }
    out.into_iter().collect()
}

/// 각 스택에서 걸린 모든 단일 키를 실제로 쳐서 디스패치된 Action이 레이어 규칙의 기대값과 같은지 본다.
fn check_single_keys(cx: &mut TestAppContext, preset: Preset, name: &str) -> String {
    start(cx, name, None);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let all = all_bindings(preset);
    let log = record_actions(cx);
    let mut report = String::new();
    for stack in stacks_for(&all) {
        let refs: Vec<&str> = stack.iter().map(String::as_str).collect();
        let kstack: Vec<KeyContext> = refs.iter().map(|s| KeyContext::parse(s).unwrap()).collect();
        let keys: BTreeSet<String> = all
            .iter()
            .map(|b| canonical_keys(&b.keys))
            .filter(|k| !k.contains(' '))
            .collect();
        let mut handle = open_probe(cx, &refs, true).0;
        for key in keys {
            let want = expected(&all, &kstack, &key);
            log.borrow_mut().clear();
            cx.simulate_keystrokes(handle, &key);
            tick(cx);
            let got = log.borrow_mut().drain(..).collect::<Vec<_>>();
            // 자기 바인딩이 없는 접두(`m`, `g`, `z`)는 GPUI가 시간 제한 없이 기다린다(제안 지적 참고).
            // 다음 키에 섞이지 않게 창을 새로 연다.
            let pending = cx
                .update_window(handle, |_, window, _| window.has_pending_keystrokes())
                .unwrap();
            if pending {
                cx.update_window(handle, |_, window, _| window.remove_window())
                    .unwrap();
                handle = open_probe(cx, &refs, true).0;
            }
            let ok = match (&want, got.as_slice()) {
                (None, []) => true,
                // 우리 레이어가 비운 키에는 바탕 레이어(gpui-component 바인딩, S2-T3에서 보존)가 남는다.
                (None, [g]) => from_base(cx, &key, g.name()),
                (Some(w), [g]) => same(cx, g.as_ref(), w),
                _ => false,
            };
            if !ok {
                let _ = writeln!(
                    report,
                    "{stack:?} '{key}': 기대 {want:?}, 실제 {:?}",
                    got.iter().map(|g| describe(g.as_ref())).collect::<Vec<_>>()
                );
            }
        }
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
    }
    report
}

#[gpui_kit::test]
fn every_single_key_dispatches_layer_rule_action_default(cx: &mut TestAppContext) {
    let report = check_single_keys(cx, Preset::Default, "sys-default");
    assert!(report.is_empty(), "\n{report}");
}

#[gpui_kit::test]
fn every_single_key_dispatches_layer_rule_action_vim(cx: &mut TestAppContext) {
    let report = check_single_keys(cx, Preset::Vim, "sys-vim");
    assert!(report.is_empty(), "\n{report}");
}

/// 여러 키 시퀀스(`g g`, `y y`, `z l`, `' a`, `a c`)를 그 컨텍스트에서 끝까지 쳤을 때 그 Action이
/// 한 번만 나와야 한다(KM-5: 짧은 쪽은 대기 시간 뒤에 실행되고 긴 시퀀스는 닿는다).
fn check_sequences(cx: &mut TestAppContext, preset: Preset, name: &str) -> String {
    start(cx, name, None);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let all = all_bindings(preset);
    let log = record_actions(cx);
    let mut report = String::new();
    let mut seen = BTreeSet::new();
    for b in &all {
        let keys = canonical_keys(&b.keys);
        if !keys.contains(' ') || matches!(b.action, ActionRef::Unbind) {
            continue;
        }
        let stack: Vec<String> = match &b.context {
            None => vec!["Normal".to_string()],
            Some(c) if stack_of(c).is_some() => {
                c.split('>').map(|s| s.trim().to_string()).collect()
            }
            Some(_) => continue,
        };
        if !seen.insert((stack.clone(), keys.clone())) {
            continue;
        }
        let refs: Vec<&str> = stack.iter().map(String::as_str).collect();
        let kstack: Vec<KeyContext> = refs.iter().map(|s| KeyContext::parse(s).unwrap()).collect();
        let Some(want) = expected(&all, &kstack, &keys) else {
            continue;
        };
        let (handle, _) = open_probe(cx, &refs, true);
        log.borrow_mut().clear();
        cx.simulate_keystrokes(handle, &keys);
        tick(cx);
        let got = log.borrow_mut().drain(..).collect::<Vec<_>>();
        let ok = matches!(got.as_slice(), [g] if same(cx, g.as_ref(), &want));
        if !ok {
            let _ = writeln!(
                report,
                "{stack:?} '{keys}': 기대 {want:?}, 실제 {:?}",
                got.iter().map(|g| describe(g.as_ref())).collect::<Vec<_>>()
            );
        }
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
    }
    report
}

#[gpui_kit::test]
fn every_sequence_reaches_its_action_default(cx: &mut TestAppContext) {
    let report = check_sequences(cx, Preset::Default, "seq-default");
    assert!(report.is_empty(), "\n{report}");
}

#[gpui_kit::test]
fn every_sequence_reaches_its_action_vim(cx: &mut TestAppContext) {
    let report = check_sequences(cx, Preset::Vim, "seq-vim");
    assert!(report.is_empty(), "\n{report}");
}

// ---------------------------------------------------------------- KM-7: US 자판의 Shift 기호

const US_SHIFTED: &[(&str, &str)] = &[
    ("1", "!"),
    ("2", "@"),
    ("3", "#"),
    ("4", "$"),
    ("5", "%"),
    ("6", "^"),
    ("7", "&"),
    ("8", "*"),
    ("9", "("),
    ("0", ")"),
    ("-", "_"),
    ("=", "+"),
    ("[", "{"),
    ("]", "}"),
    ("\\", "|"),
    (";", ":"),
    ("'", "\""),
    (",", "<"),
    (".", ">"),
    ("/", "?"),
    ("`", "~"),
];

/// 모든 `shift-<숫자·기호>` 바인딩을 macOS(US)가 실제로 만드는 입력(Shift 없이 기호 글자)으로 쳤을 때
/// 같은 Action이 나와야 한다. 두 프리셋 모두.
fn check_shifted(cx: &mut TestAppContext, preset: Preset, name: &str) -> String {
    start(cx, name, None);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let all = all_bindings(preset);
    let log = record_actions(cx);
    let mut report = String::new();
    let mut checked = 0;
    for b in &all {
        let keys = canonical_keys(&b.keys);
        if keys.contains(' ') {
            continue;
        }
        let Ok(stroke) = Keystroke::parse(&keys) else {
            continue;
        };
        if !stroke.modifiers.shift {
            continue;
        }
        let Some((_, sym)) = US_SHIFTED.iter().find(|(base, _)| *base == stroke.key) else {
            continue;
        };
        let stack: Vec<String> = match &b.context {
            None => vec!["Normal".to_string()],
            Some(c) if stack_of(c).is_some() => {
                c.split('>').map(|s| s.trim().to_string()).collect()
            }
            Some(_) => continue,
        };
        let refs: Vec<&str> = stack.iter().map(String::as_str).collect();
        let kstack: Vec<KeyContext> = refs.iter().map(|s| KeyContext::parse(s).unwrap()).collect();
        let Some(want) = expected(&all, &kstack, &keys) else {
            continue;
        };
        let mut mac = stroke.clone();
        mac.modifiers.shift = false;
        mac.key = (*sym).to_string();
        mac.key_char = Some((*sym).to_string());
        let (handle, _) = open_probe(cx, &refs, true);
        log.borrow_mut().clear();
        cx.simulate_keystrokes(handle, &mac.unparse());
        tick(cx);
        checked += 1;
        let got = log.borrow_mut().drain(..).collect::<Vec<_>>();
        if !matches!(got.as_slice(), [g] if same(cx, g.as_ref(), &want)) {
            let _ = writeln!(
                report,
                "{stack:?} '{keys}' → macOS '{}': 기대 {want:?}, 실제 {:?}",
                mac.unparse(),
                got.iter().map(|g| describe(g.as_ref())).collect::<Vec<_>>()
            );
        }
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
    }
    assert!(checked > 0, "Shift 기호 바인딩을 하나도 찾지 못했다");
    report
}

#[gpui_kit::test]
fn shifted_symbol_bindings_match_us_keystrokes_default(cx: &mut TestAppContext) {
    let report = check_shifted(cx, Preset::Default, "shift-default");
    assert!(report.is_empty(), "\n{report}");
}

#[gpui_kit::test]
fn shifted_symbol_bindings_match_us_keystrokes_vim(cx: &mut TestAppContext) {
    let report = check_shifted(cx, Preset::Vim, "shift-vim");
    assert!(report.is_empty(), "\n{report}");
}

// ---------------------------------------------------------------- KM-5 카운트

/// 메뉴(키가 아닌 경로)로 실행된 Action 뒤에도 카운트가 남으면, `3` → 메뉴 「인스펙터」 → 한참 뒤
/// 다음 키 Action이 3을 받는다. 리뷰 1 지적 4의 「숫자 외 Action이 지나가도 남는다」와 같은 문제다.
#[gpui_kit::test]
fn count_is_cleared_by_menu_dispatched_action(cx: &mut TestAppContext) {
    start(cx, "count-menu", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, seen) = open_probe(cx, &["Normal"], false);
    cx.simulate_keystrokes(handle, "3");
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(keyscore_app::actions::edit::Undo), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(seen.borrow().is_empty());
    assert_eq!(
        cx.update(keymap::take_count),
        None,
        "메뉴 Action 뒤에 카운트가 남았다"
    );
}

/// 카운트는 시퀀스 대기 중에는 남고(`3 g g`의 처리기가 3을 받는다), 시퀀스 Action 뒤에는 비워진다.
#[gpui_kit::test]
fn count_survives_pending_sequence_and_reaches_its_handler(cx: &mut TestAppContext) {
    start(cx, "count-seq", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, seen) = open_probe(cx, &["Normal"], false);
    cx.simulate_keystrokes(handle, "1 2 g g");
    assert_eq!(seen.borrow().as_slice(), [("cursor::ClipStart", Some(12))]);
    assert_eq!(cx.update(keymap::take_count), None);
    // 대기 시간 동안 다음 키가 없을 때도: `4 g` 뒤 1 s → `g`는 단일 바인딩이 없어 아무 일도 없다.
    cx.simulate_keystrokes(handle, "4 g");
    tick(cx);
    cx.simulate_keystrokes(handle, "g g");
    assert_eq!(
        seen.borrow().last().copied(),
        Some(("cursor::ClipStart", Some(4))),
        "대기 시간이 끝난 시퀀스 뒤 카운트 처리"
    );
}

/// `3`, `Esc`, `x`: 리뷰 1 지적 4가 든 예다. vim Normal에서 `escape`는 아직 비어 있어(A13 결정 대기)
/// Action이 없으므로 카운트가 남는다. vim은 Esc로 카운트를 버린다.
#[gpui_kit::test]
fn count_is_cleared_by_unbound_escape(cx: &mut TestAppContext) {
    start(cx, "count-esc", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, _) = open_probe(cx, &["Normal"], false);
    cx.simulate_keystrokes(handle, "3 escape");
    assert_eq!(
        cx.update(keymap::take_count),
        None,
        "Esc 뒤에 카운트가 남았다"
    );
}

// ---------------------------------------------------------------- KM-4 충돌 정규화

/// 충돌 비교는 키도 GPUI 키 입력 단위로 정규화한다고 보고서가 적었다. 수식키 순서·대소문자·별칭이
/// 다른 같은 키는 같은 키다.
#[test]
fn conflicts_detected_across_key_spellings() {
    for (a, b) in [
        ("cmd-shift-k", "shift-cmd-k"),
        ("alt-k", "Alt-k"),
        ("cmd-k", "super-k"),
        ("ctrl-alt-h", "alt-ctrl-h"),
    ] {
        let text = format!(
            r#"[{{"context": "Normal", "bindings": {{"{a}": "edit::Undo", "{b}": "edit::Redo"}}}}]"#
        );
        let (bindings, errors) = parse(&text, Layer::User);
        assert!(errors.is_empty(), "{errors:?}");
        let c = conflicts(&bindings);
        assert!(
            c.iter().any(|c| c.kind == ConflictKind::SameKeys),
            "'{a}'와 '{b}'의 충돌을 놓쳤다: {c:?}"
        );
    }
}

/// 접두 충돌도 같은 정규화를 거쳐야 한다(`shift-cmd-k`와 `cmd-shift-k g`).
#[test]
fn prefix_conflicts_detected_across_key_spellings() {
    let (bindings, _) = parse(
        r#"[{"context": "Normal", "bindings": {"shift-cmd-k": "edit::Undo", "cmd-shift-k g": "edit::Redo"}}]"#,
        Layer::User,
    );
    let c = conflicts(&bindings);
    assert!(c.iter().any(|c| c.kind == ConflictKind::Prefix), "{c:?}");
}

/// 컨텍스트 표기 차이(공백, 괄호 없는 같은 술어)는 같은 그룹이다(회귀).
#[test]
fn conflicts_detected_across_context_whitespace() {
    let (bindings, _) = parse(
        r#"[{"context": "Normal  >PianoRoll", "bindings": {"x": "edit::Undo"}},
            {"context": "Normal> PianoRoll", "bindings": {"x": "edit::Redo"}}]"#,
        Layer::User,
    );
    assert_eq!(conflicts(&bindings).len(), 1);
}

// ---------------------------------------------------------------- 프리셋·교환 저장(재시작)

#[gpui_kit::test]
fn preset_and_swap_survive_restart(cx_a: &mut TestAppContext, cx_b: &mut TestAppContext) {
    let dir = temp_dir("restart");
    start_in(cx_a, &dir, None);
    cx_a.update(|cx| {
        keymap::set_preset(cx, Preset::Vim);
        keymap::set_swap_alt_cmd(cx, true);
    });
    // 저장 디바운스(1 s)가 지나면 디스크에 있어야 한다.
    tick(cx_a);
    start_in(cx_b, &dir, None);
    let options = cx_b.update(|cx| keymap::options(cx));
    assert_eq!(options.preset, Preset::Vim);
    assert!(options.swap_alt_cmd);
    // 실제로 vim 키맵이 걸렸는가: Normal의 `b`는 vim에서 이전 노트다.
    let log = record_actions(cx_b);
    let (handle, _) = open_probe(cx_b, &["Normal", "PianoRoll"], true);
    cx_b.simulate_keystrokes(handle, "b");
    let got: Vec<_> = log.borrow().iter().map(|a| a.name()).collect();
    assert_eq!(got, ["cursor::PrevNote"]);
    assert_eq!(
        cx_b.update(|cx| AppSettings::get(cx).keymap.preset),
        Preset::Vim
    );
}

// ---------------------------------------------------------------- 잘못된 context 절

#[gpui_kit::test]
fn bad_context_sections_never_leak_bindings(cx: &mut TestAppContext) {
    start(
        cx,
        "bad-context",
        Some(
            r#"[
  {"context": "", "bindings": {"f1": "view::ToggleInspector"}},
  {"context": "Normal >", "bindings": {"f2": "view::ToggleInspector"}},
  {"context": ["Normal"], "bindings": {"f3": "view::ToggleInspector"}},
  {"context": "&& Normal", "bindings": {"f4": "view::ToggleInspector"}},
  {"context": "Normal", "context": 5, "bindings": {"f5": "view::ToggleInspector"}},
  {"context": 5, "context": "Normal", "bindings": {"f6": "view::ToggleInspector"}},
  {"bindings": {"f7": "view::ToggleInspector"}, "context": {}}
]"#,
        ),
    );
    let errors = cx.update(|cx| keymap::errors(cx).to_vec());
    let (handle, seen) = open_probe(cx, &["Normal"], false);
    for key in ["f1", "f2", "f3", "f4", "f6", "f7"] {
        cx.simulate_keystrokes(handle, key);
        assert!(seen.borrow().is_empty(), "{key}가 걸렸다. 오류: {errors:?}");
    }
    // 각 절이 적어도 하나의 오류를 낸다.
    assert!(errors.len() >= 7, "{errors:?}");
}

// ---------------------------------------------------------------- KM-12 표준 키 보호와 별칭

#[test]
fn user_override_of_standard_shortcut_warns_for_aliases() {
    for keys in ["super-s", "Cmd-s", "shift-cmd-z", "win-z", "cmd-shift-S"] {
        let text = format!(r#"[{{"bindings": {{"{keys}": "view::ToggleInspector"}}}}]"#);
        let (_, errors) = load_layers(Preset::Default, Some(&text));
        assert!(
            errors
                .iter()
                .any(|e| e.layer == Layer::User && e.message.contains("표준")),
            "'{keys}': {errors:?}"
        );
    }
}

/// 사용자 `null`로 표준 키를 지우는 것도 표준 단축키를 바꾸는 일이다.
#[test]
fn user_unbinding_standard_shortcut_warns() {
    let (_, errors) = load_layers(Preset::Default, Some(r#"[{"bindings": {"cmd-s": null}}]"#));
    assert!(
        errors.iter().any(|e| e.message.contains("표준")),
        "{errors:?}"
    );
}

/// 수식키 교환을 켜면 사용자 `alt-s`가 실제로는 `cmd-s`(저장)를 덮는다. 경고가 없다면 KM-12의
/// 「경고를 띄운다」가 교환 옵션 아래에서 빠진다.
#[gpui_kit::test]
fn user_override_of_standard_shortcut_warns_under_swap(cx: &mut TestAppContext) {
    start(
        cx,
        "swap-warn",
        Some(r#"[{"bindings": {"alt-s": "view::ToggleInspector"}}]"#),
    );
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let errors = cx.update(|cx| keymap::errors(cx).to_vec());
    let log = record_actions(cx);
    let (handle, _) = open_probe(cx, &["Normal"], true);
    cx.simulate_keystrokes(handle, "cmd-s");
    let got: Vec<_> = log.borrow().iter().map(|a| a.name()).collect();
    assert_eq!(
        got,
        ["view::ToggleInspector"],
        "교환 후 사용자 키가 cmd-s를 덮는다(전제)"
    );
    assert!(
        errors.iter().any(|e| e.message.contains("표준")),
        "교환으로 표준 키를 덮는데 경고가 없다: {errors:?}"
    );
}

/// 교환을 켜도 프리셋은 표준 키를 덮지 못한다(KM-12). 프리셋의 `alt-` 키가 교환 뒤 표준 `cmd-` 키와
/// 같아지는 것이 없는지 모든 프리셋 바인딩으로 본다(회귀).
#[test]
fn swapped_preset_keys_never_hit_standard_keys() {
    for preset in [Preset::Default, Preset::Vim] {
        let (layers, _) = load_layers(preset, None);
        let standard: BTreeSet<String> = layers
            .standard
            .iter()
            .map(|b| canonical_keys(&b.keys))
            .collect();
        for b in &layers.preset {
            let swapped = canonical_keys(&keymap::swap_alt_cmd(&b.keys));
            assert!(
                !standard.contains(&swapped),
                "{preset:?} '{}' → '{swapped}'",
                b.keys
            );
        }
    }
}

// ---------------------------------------------------------------- 제안 10, 11, 13

/// 부트 때부터 문법 오류면 사용자 레이어는 비고 오류가 보고된다. 고치면 바로 적용된다.
#[gpui_kit::test]
fn syntax_error_at_boot_then_fixed(cx: &mut TestAppContext) {
    let dir = start(cx, "boot-syntax", Some("[{\"bindings\": {\"f8\" "));
    assert_eq!(cx.update(|cx| keymap::errors(cx).len()), 1);
    let (handle, seen) = open_probe(cx, &["Normal"], false);
    cx.simulate_keystrokes(handle, "f8");
    assert!(seen.borrow().is_empty());
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"bindings": {"f8": "view::ToggleInspector"}}]"#,
    )
    .unwrap();
    tick(cx);
    cx.simulate_keystrokes(handle, "f8");
    assert_eq!(seen.borrow().len(), 1);
}

/// 좋은 파일 → 문법 오류(마지막 좋은 레이어 유지) → 다른 좋은 파일(새 레이어로 바뀜) → 빈 배열(모두 지움).
#[gpui_kit::test]
fn last_good_user_layer_is_replaced_and_cleared(cx: &mut TestAppContext) {
    let dir = start(
        cx,
        "last-good",
        Some(r#"[{"bindings": {"f8": "view::ToggleInspector"}}]"#),
    );
    let (handle, seen) = open_probe(cx, &["Normal"], false);
    let write = |text: &str| {
        // 같은 크기·같은 초의 수정 시각이면 감시가 놓칠 수 있어 크기를 바꾼다.
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    };
    write("[{\"bindings\": {\"f8\"");
    tick(cx);
    cx.simulate_keystrokes(handle, "f8");
    assert_eq!(
        seen.borrow().len(),
        1,
        "문법 오류 중에도 마지막 좋은 레이어"
    );
    write(r#"[{"bindings": {"f9": "view::ToggleInspector"}}]   "#);
    tick(cx);
    cx.simulate_keystrokes(handle, "f8 f9");
    assert_eq!(seen.borrow().len(), 2, "새 레이어: f8은 빠지고 f9가 걸린다");
    write("[]");
    tick(cx);
    cx.simulate_keystrokes(handle, "f9");
    assert_eq!(seen.borrow().len(), 2, "빈 배열은 사용자 레이어를 비운다");
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
}

/// UTF-8이 아닌 파일은 읽기 오류로 보고되고 마지막 좋은 레이어가 남는다.
#[gpui_kit::test]
fn unreadable_user_keymap_is_reported(cx: &mut TestAppContext) {
    let dir = start(
        cx,
        "unreadable",
        Some(r#"[{"bindings": {"f8": "view::ToggleInspector"}}]"#),
    );
    std::fs::write(
        dir.join("keymap.json"),
        b"[{\"bindings\": {\"f8\": \"\xff\xfe\"}}]",
    )
    .unwrap();
    tick(cx);
    let errors = cx.update(|cx| keymap::errors(cx).to_vec());
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].layer, Layer::User);
    let (handle, seen) = open_probe(cx, &["Normal"], false);
    cx.simulate_keystrokes(handle, "f8");
    assert_eq!(seen.borrow().len(), 1);
    // 디렉터리로 바뀐 경로도 읽기 오류다.
    std::fs::remove_file(dir.join("keymap.json")).unwrap();
    std::fs::create_dir(dir.join("keymap.json")).unwrap();
    tick(cx);
    assert_eq!(cx.update(|cx| keymap::errors(cx).len()), 1);
}

/// 같은 절의 중복 `bindings`는 보고되고 앞 것이 남는다.
#[test]
fn duplicate_bindings_field_is_reported_and_first_kept() {
    let (b, e) = parse(
        r#"[{"bindings": {"x": "edit::Undo"}, "bindings": {"y": "edit::Redo"}}]"#,
        Layer::User,
    );
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].keys, "x");
}

/// 자기 바인딩이 없는 접두(vim `m`, `g`, `z`, `'`, `` ` ``)는 GPUI가 대기 시간 없이 다음 키를 기다린다
/// (`pending_has_binding`이 거짓이면 타이머를 걸지 않는다). KM-5의 「시퀀스 대기 1 s」와 다르다.
/// 리뷰 2 제안 지적이며, 지금 동작을 고정한다(받아들이면 기대값을 `search::Next`로 바꾼다).
#[gpui_kit::test]
fn vim_prefix_without_own_binding_waits_past_timeout(cx: &mut TestAppContext) {
    start(cx, "prefix-wait", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let log = record_actions(cx);
    let (handle, _) = open_probe(cx, &["Normal"], true);
    cx.simulate_keystrokes(handle, "m");
    cx.executor().advance_clock(Duration::from_secs(5));
    cx.run_until_parked();
    cx.simulate_keystrokes(handle, "n");
    let got: Vec<_> = log.borrow().iter().map(|a| a.name()).collect();
    assert_eq!(got, ["cursor::SetMark"]);
}
