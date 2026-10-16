//! 프로젝트 창(창 하나 = 프로젝트 하나). 7.1의 레이아웃 골격.

use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::{
    ActiveTheme as _, ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
    v_resizable,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, TestSupportExt as _,
    Window, div, prelude::FluentBuilder as _, px,
};

use crate::actions::{ToggleBottomPanel, ToggleInspector, ToggleTrackList};
use crate::app_state::AppSettings;
use crate::document::ProjectDocument;
use crate::settings::{LayoutSettings, PANEL_SIZE_RANGE};

/// 크기를 저장하는 패널 그룹.
#[derive(Clone, Copy)]
enum Group {
    Horizontal,
    Vertical,
}

/// 창의 루트 뷰.
pub struct ProjectWindow {
    document: Entity<ProjectDocument>,
    layout: LayoutSettings,
    focus: FocusHandle,
    /// 가로 그룹(트랙 목록 | 메인 | 인스펙터)과 세로 그룹(위 | 하단 영역)의 크기 상태.
    horizontal: Entity<ResizableState>,
    vertical: Entity<ResizableState>,
    _subscriptions: [Subscription; 2],
}

impl ProjectWindow {
    pub fn new(document: Entity<ProjectDocument>, cx: &mut Context<Self>) -> Self {
        let horizontal = cx.new(|_| ResizableState::default());
        let vertical = cx.new(|_| ResizableState::default());
        // 끌기와 프로그램 조절 모두 `Resized`를 낸다(gpui-base `resize_panel`). 그때 크기를 남긴다.
        let subscriptions = [
            cx.subscribe(&horizontal, |this, state, _: &ResizablePanelEvent, cx| {
                this.record_sizes(Group::Horizontal, &sizes_of(&state, cx), cx);
            }),
            cx.subscribe(&vertical, |this, state, _: &ResizablePanelEvent, cx| {
                this.record_sizes(Group::Vertical, &sizes_of(&state, cx), cx);
            }),
        ];
        ProjectWindow {
            document,
            layout: AppSettings::get(cx).layout,
            focus: cx.focus_handle(),
            horizontal,
            vertical,
            _subscriptions: subscriptions,
        }
    }

    /// 가로 패널 그룹의 크기 상태(트랙 목록 0, 메인 1, 인스펙터 2).
    #[must_use]
    pub fn horizontal_state(&self) -> &Entity<ResizableState> {
        &self.horizontal
    }

    /// 세로 패널 그룹의 크기 상태(위 0, 하단 영역 1).
    #[must_use]
    pub fn vertical_state(&self) -> &Entity<ResizableState> {
        &self.vertical
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
        self.change_layout(cx, |l| l.track_list_open = !l.track_list_open);
    }

    fn toggle_inspector(&mut self, _: &ToggleInspector, _: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(cx, |l| l.inspector_open = !l.inspector_open);
    }

    fn toggle_bottom(&mut self, _: &ToggleBottomPanel, _: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(cx, |l| l.bottom_open = !l.bottom_open);
    }

    /// 이 창의 레이아웃을 바꾸고 앱 설정에 남긴다. 다음에 여는 창이 이 레이아웃으로 열린다.
    fn change_layout(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut LayoutSettings)) {
        edit(&mut self.layout);
        let layout = self.layout;
        AppSettings::update(cx, |s| s.layout = layout);
        cx.notify();
    }

    /// 패널 그룹의 크기가 바뀌었다(사용자가 끌었다). 크기 목록은 자식 순서를 따른다. 접힌 패널이 있어
    /// 목록 길이가 다르면 그때는 저장하지 않는다(다음 조절에서 맞춘다).
    fn record_sizes(&mut self, group: Group, sizes: &[f32], cx: &mut Context<Self>) {
        let keep = |v: f32| PANEL_SIZE_RANGE.contains(&v);
        match (group, sizes) {
            (Group::Horizontal, &[track, _, inspector]) => self.change_layout(cx, |l| {
                if l.track_list_open && keep(track) {
                    l.track_list_width = track;
                }
                if l.inspector_open && keep(inspector) {
                    l.inspector_width = inspector;
                }
            }),
            (Group::Vertical, &[_, bottom]) => self.change_layout(cx, |l| {
                if l.bottom_open && keep(bottom) {
                    l.bottom_height = bottom;
                }
            }),
            _ => {}
        }
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
                        .with_state(&self.vertical)
                        .child(
                            resizable_panel().child(
                                h_resizable("main-horizontal")
                                    .with_state(&self.horizontal)
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

fn sizes_of(state: &Entity<ResizableState>, cx: &App) -> Vec<f32> {
    state
        .read(cx)
        .sizes()
        .iter()
        .map(|p| f32::from(*p))
        .collect()
}
