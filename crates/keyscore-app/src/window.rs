//! 프로젝트 창(창 하나 = 프로젝트 하나). 7.1의 레이아웃 골격.

use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::{ActiveTheme as _, h_resizable, resizable_panel, v_resizable};
use gpui_kit::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::actions::{ToggleBottomPanel, ToggleInspector, ToggleTrackList};
use crate::app_state::AppSettings;
use crate::document::ProjectDocument;
use crate::settings::{LayoutSettings, PANEL_SIZE_RANGE};

/// 창의 루트 뷰.
pub struct ProjectWindow {
    document: Entity<ProjectDocument>,
    layout: LayoutSettings,
    focus: FocusHandle,
}

impl ProjectWindow {
    pub fn new(document: Entity<ProjectDocument>, cx: &mut Context<Self>) -> Self {
        ProjectWindow {
            document,
            layout: AppSettings::get(cx).layout,
            focus: cx.focus_handle(),
        }
    }

    #[must_use]
    pub fn document(&self) -> &Entity<ProjectDocument> {
        &self.document
    }

    #[must_use]
    pub const fn layout(&self) -> LayoutSettings {
        self.layout
    }

    fn toggle_track_list(&mut self, _: &ToggleTrackList, _: &mut Window, cx: &mut Context<Self>) {
        let _ = cx;
        todo!()
    }

    fn toggle_inspector(&mut self, _: &ToggleInspector, _: &mut Window, cx: &mut Context<Self>) {
        let _ = cx;
        todo!()
    }

    fn toggle_bottom(&mut self, _: &ToggleBottomPanel, _: &mut Window, cx: &mut Context<Self>) {
        let _ = cx;
        todo!()
    }

    /// 자리표시자 영역. 내용은 S2-T5 이후 채운다.
    fn region(id: &'static str, label: &'static str, cx: &App) -> impl IntoElement {
        div()
            .id(id)
            .test_support()
            .size_full()
            .p_2()
            .text_color(cx.theme().muted_foreground)
            .child(label)
    }
}

impl Focusable for ProjectWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ProjectWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = self.layout;
        let range = px(*PANEL_SIZE_RANGE.start())..px(*PANEL_SIZE_RANGE.end());
        div()
            .id("project-window")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::toggle_track_list))
            .on_action(cx.listener(Self::toggle_inspector))
            .on_action(cx.listener(Self::toggle_bottom))
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .h(px(40.))
                    .flex_none()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Self::region("transport", "트랜스포트", cx)),
            )
            .child(
                div().flex_1().min_h_0().child(
                    v_resizable("main-vertical")
                        .child(
                            resizable_panel().child(
                                h_resizable("main-horizontal")
                                    .child(
                                        resizable_panel()
                                            .visible(layout.track_list_open)
                                            .size(px(layout.track_list_width))
                                            .size_range(range.clone())
                                            .child(Self::region("track-list", "트랙 목록", cx)),
                                    )
                                    .child(resizable_panel().child(Self::region(
                                        "main-view",
                                        "피아노롤 / 배치 뷰",
                                        cx,
                                    )))
                                    .child(
                                        resizable_panel()
                                            .visible(layout.inspector_open)
                                            .size(px(layout.inspector_width))
                                            .size_range(range.clone())
                                            .child(Self::region("inspector", "트랙 인스펙터", cx)),
                                    ),
                            ),
                        )
                        .child(
                            resizable_panel()
                                .visible(layout.bottom_open)
                                .size(px(layout.bottom_height))
                                .size_range(range)
                                .child(Self::region("bottom-panel", "컨트롤러 레인 / 건반 뷰", cx)),
                        ),
                ),
            )
            .child(
                div()
                    .id("status-bar")
                    .test_support()
                    .flex_none()
                    .when(true, |d| d.child(StatusBar::new().left("NORMAL"))),
            )
    }
}
