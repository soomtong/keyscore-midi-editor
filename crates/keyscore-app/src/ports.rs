//! 출력 포트 감시와 재연결(S2-T5d, OUT-1, OUT-3).

use std::time::Duration;

use gpui_kit::App;

/// 감시 주기.
pub const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 감시를 시작한다. 엔진이 없으면 아무 일도 하지 않는다.
pub fn start_monitor(cx: &mut App) {
    let _ = cx;
    todo!()
}

/// 감시가 돌고 있는가.
#[must_use]
pub fn is_monitoring(cx: &App) -> bool {
    let _ = cx;
    todo!()
}
