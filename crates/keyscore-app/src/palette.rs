//! 12색 트랙 팔레트(SEQ-7, `TrackColor` 0–11).
//!
//! 색값을 앱에 적지 않고 gpui-kit 테마의 색 토큰 12개에 대응시킨다. 다크·라이트를 바꾸면 트랙 색도
//! 테마에 맞게 함께 바뀐다(gpui-kit 코딩 가이드: 앱 코드에 원시 색값을 쓰지 않는다).

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Hsla};
use keyscore_core::TrackColor;

/// 팔레트 순서. 이웃한 번호가 서로 멀리 떨어진 색이 되게 기본 여섯 색 다음에 밝은 여섯 색을 둔다.
pub const PALETTE_NAMES: [&str; 12] = [
    "red",
    "yellow",
    "green",
    "cyan",
    "blue",
    "magenta",
    "red_light",
    "yellow_light",
    "green_light",
    "cyan_light",
    "blue_light",
    "magenta_light",
];

/// 트랙 색 번호의 현재 테마 색.
#[must_use]
pub fn track_color(color: TrackColor, cx: &App) -> Hsla {
    let _ = (color, cx.theme());
    todo!()
}
