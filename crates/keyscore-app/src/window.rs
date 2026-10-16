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
    /// 크기 상태 구독. 상태를 새로 만들 때 함께 바꾼다.
    resize_subscriptions: [Subscription; 2],
    _focus_lost: Subscription,
}

impl ProjectWindow {
    pub fn new(
        document: Entity<ProjectDocument>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (horizontal, vertical, resize_subscriptions) = Self::fresh_states(cx);
        // 창 안에 포커스가 없으면 Action이 루트(`Root`)에서 디스패치되어 이 뷰의 처리기에 닿지 않는다
        // (gpui-component `Input`은 Esc에서 `window.blur()`를 부른다). 포커스를 잃으면 창의 루트
        // 핸들로 되돌린다(Zed Workspace와 같은 방식, S2-T1 리뷰 1 지적 3).
        let focus_lost = cx.on_focus_lost(window, |this, window, cx| {
            window.focus(&this.focus, cx);
        });
        ProjectWindow {
            document,
            layout: AppSettings::get(cx).layout,
            focus: cx.focus_handle(),
            horizontal,
            vertical,
            resize_subscriptions,
            _focus_lost: focus_lost,
        }
    }

    /// 새 크기 상태 둘과 그 구독. 끌기와 프로그램 조절 모두 `Resized`를 낸다(gpui-base
    /// `resize_panel`). 그때 크기를 남긴다.
    fn fresh_states(
        cx: &mut Context<Self>,
    ) -> (
        Entity<ResizableState>,
        Entity<ResizableState>,
        [Subscription; 2],
    ) {
        let horizontal = cx.new(|_| ResizableState::default());
        let vertical = cx.new(|_| ResizableState::default());
        let subscriptions = [
            cx.subscribe(&horizontal, |this, state, _: &ResizablePanelEvent, cx| {
                this.record_sizes(Group::Horizontal, &sizes_of(&state, cx), cx);
            }),
            cx.subscribe(&vertical, |this, state, _: &ResizablePanelEvent, cx| {
                this.record_sizes(Group::Vertical, &sizes_of(&state, cx), cx);
            }),
        ];
        (horizontal, vertical, subscriptions)
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

    /// 이 창의 레이아웃을 바꾸고, 바뀐 항목만 앱 설정에 남긴다. 창마다 레이아웃 전체를 쓰면 한 창의
    /// 토글이 다른 창에서 바꾼 너비를 되돌린다(리뷰 1 제안 5). 다음에 여는 창은 설정으로 열린다.
    fn change_layout(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut LayoutSettings)) {
        let before = self.layout;
        edit(&mut self.layout);
        let after = self.layout;
        // 접혀 있던 패널을 펴면 그 그룹의 크기 상태를 새로 만든다. gpui-base는 숨은 패널의 크기를
        // 0(최소값)으로 덮어써 두므로, 새 상태여야 저장된 크기로 다시 그린다(리뷰 1 지적 2).
        let reopened = (!before.track_list_open && after.track_list_open)
            || (!before.inspector_open && after.inspector_open)
            || (!before.bottom_open && after.bottom_open);
        if reopened {
            let (horizontal, vertical, subscriptions) = Self::fresh_states(cx);
            self.horizontal = horizontal;
            self.vertical = vertical;
            self.resize_subscriptions = subscriptions;
        }
        AppSettings::update(cx, |s| merge_changes(&mut s.layout, before, after));
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

/// `before`에서 `after`로 바뀐 항목만 `target`에 옮긴다.
// 「바뀌었는가」를 정확히 본다. 같은 값을 다시 쓴 것은 바뀐 것이 아니다.
#[allow(clippy::float_cmp)]
fn merge_changes(target: &mut LayoutSettings, before: LayoutSettings, after: LayoutSettings) {
    macro_rules! copy_changed {
        ($($field:ident),*) => {
            $(
                if before.$field != after.$field {
                    target.$field = after.$field;
                }
            )*
        };
    }
    copy_changed!(
        track_list_open,
        track_list_width,
        inspector_open,
        inspector_width,
        bottom_open,
        bottom_height
    );
}
