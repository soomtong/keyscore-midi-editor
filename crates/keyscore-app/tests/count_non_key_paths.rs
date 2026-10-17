//! 키가 아닌 실행 경로와 카운트(결정 `docs/decisions/2026-10-17-count-and-non-key-dispatch.md` C).
//!
//! S2-T2 리뷰 3이 미뤄 둔 수용 테스트(`tests/deferred/count_non_key_paths.rs`)를 S2-T4가 결정에 맞게 고쳐
//! 켰다. 팔레트·상태줄 모드 메뉴·툴바는 실행 함수 `keyscore_app::dispatch::run_action` 하나를 거치고, 그
//! 함수가 카운트를 먼저 비운다. 메뉴 막대 경로는 결정 C의 한계라 실행 함수 경로로 바꿨다.

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Action, AnyWindowHandle, AppContext as _, Context, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, TestAppContext, Window, div,
};
use keyscore_app::init;
use keyscore_app::keymap::{self, Preset};

fn start_vim(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) {
    let dir = std::env::temp_dir().join(format!("ks-count-paths-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
}

fn advance(cx: &mut TestAppContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

/// 키가 아닌 경로의 실행(팔레트 확정과 같은 함수).
fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: Box<dyn Action>) {
    cx.update_window(handle, |_, window, cx| {
        keyscore_app::dispatch::run_action(action, window, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

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

fn names(seen: &Seen) -> Vec<(&'static str, Option<u32>)> {
    seen.borrow().clone()
}

/// 리뷰 2 지적 2의 시나리오를 실행 함수 경로로: `3` → 팔레트(실행 함수)로 실행 취소 → 한참 뒤 키 Action. 키 Action이 3을
/// 받으면 지적이 고쳐지지 않은 것이다. 리뷰 2 테스트는 처리기 밖 `take_count`가 늘 `None`이 되는
/// 의미 변경만으로 통과한다.
#[gpui_kit::test]
fn count_typed_before_non_key_run_does_not_reach_next_key_action(cx: &mut TestAppContext) {
    start_vim(cx, "run-then-key", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3");
    run(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(
        names(&seen),
        [("edit::Undo", None)],
        "실행 함수로 실행한 처리기(전제)"
    );
    advance(cx, 5000);
    cx.simulate_keystrokes(handle, "g g");
    assert_eq!(
        names(&seen).last().copied(),
        Some(("cursor::ClipStart", None)),
        "실행 함수 앞에 친 카운트가 다음 키 Action에 붙었다"
    );
}

/// 시퀀스 대기 중(`3 g`)에 실행 함수로 실행된 처리기도 카운트를 받지 않는다(보고서: 「메뉴·팔레트 처리기는
/// 받지 않는다」). 대기 키는 키 관찰자를 부르지 않아 `in_key_dispatch`가 참으로 남는다.
#[gpui_kit::test]
fn non_key_run_during_pending_sequence_gets_no_count(cx: &mut TestAppContext) {
    start_vim(cx, "run-pending", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3 g");
    run(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(
        names(&seen),
        [("edit::Undo", None)],
        "대기 중 실행 함수로 실행한 처리기가 카운트를 받았다"
    );
}
