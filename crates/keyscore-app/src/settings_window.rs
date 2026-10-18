//! 설정 창(S2-T6a): 구역 목록과 키맵 편집기(KM-3·4·8·9).

use gpui_kit::{AnyWindowHandle, App, Context, Entity, IntoElement, Render, Window, div};

use crate::actions::args::SettingsSection;

/// 설정 창.
pub struct SettingsWindow {
    section: SettingsSection,
}

/// 설정 창을 연다(이미 있으면 앞으로, 구역만 바꾼다).
pub fn open(cx: &mut App, section: Option<SettingsSection>) {
    let _ = (cx, section);
    todo!()
}

/// 열린 설정 창.
#[must_use]
pub fn current(cx: &App) -> Option<(AnyWindowHandle, Entity<SettingsWindow>)> {
    let _ = cx;
    todo!()
}

impl SettingsWindow {
    #[must_use]
    pub fn section(&self) -> SettingsSection {
        self.section
    }

    #[must_use]
    pub fn selected_action(&self, cx: &App) -> Option<String> {
        let _ = cx;
        todo!()
    }

    #[must_use]
    pub fn capturing(&self) -> bool {
        todo!()
    }

    #[must_use]
    pub fn sheet_open(&self) -> bool {
        todo!()
    }

    #[must_use]
    pub fn message(&self) -> Option<String> {
        todo!()
    }

    #[must_use]
    pub fn search_focused_for_test(&self) -> bool {
        todo!()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
