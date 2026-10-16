//! 다크·라이트 테마(UI-7).

use gpui_kit::App;
use gpui_kit::component::{Theme, ThemeMode};

use crate::app_state::AppSettings;
use crate::settings::ThemeChoice;

/// 테마를 고르고 설정에 남긴다.
pub fn choose(choice: ThemeChoice, cx: &mut App) {
    AppSettings::update(cx, |s| s.theme = choice);
    apply(choice, cx);
}

/// 테마를 적용한다. 시스템이면 macOS 외관을 따른다.
pub fn apply(choice: ThemeChoice, cx: &mut App) {
    match choice {
        ThemeChoice::System => Theme::sync_system_appearance(None, cx),
        ThemeChoice::Light => Theme::change(ThemeMode::Light, None, cx),
        ThemeChoice::Dark => Theme::change(ThemeMode::Dark, None, cx),
    }
    cx.refresh_windows();
}
