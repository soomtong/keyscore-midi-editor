//! 키가 아닌 경로(명령 팔레트, 상태줄 모드 메뉴, 이후 툴바)의 실행 함수.
//!
//! 결정 `docs/decisions/2026-10-17-count-and-non-key-dispatch.md` C: 이 경로들은 모두 이 함수 하나를
//! 거치고, 이 함수가 쌓인 카운트를 먼저 비운다. 메뉴 막대는 GPUI가 Action을 직접 실행하므로 한계로 남는다.

use gpui_kit::{Action, App, Window};

use crate::keymap;

/// 카운트를 비우고 창의 포커스로 Action을 디스패치한다.
pub fn run_action(action: Box<dyn Action>, window: &mut Window, cx: &mut App) {
    keymap::clear_count(cx);
    window.dispatch_action(action, cx);
}
