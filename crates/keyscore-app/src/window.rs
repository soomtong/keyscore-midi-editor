//! 프로젝트 창(창 하나 = 프로젝트 하나). 7.1의 레이아웃 골격.

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::{
    ActiveTheme as _, ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
    v_resizable,
};
use gpui_kit::{
    Action, Anchor, AnyElement, App, AppContext as _, Context, DismissEvent, Div, Entity,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    Render, Role, Stateful, StatefulInteractiveElement as _, Styled as _, Subscription,
    TestSupportExt as _, Window, anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};

use crate::actions::view::{ToggleBottomPanel, ToggleInspector, ToggleTrackList};
use crate::actions::{ai, clip, mode, palette, select, view};
use crate::app_state::AppSettings;
use crate::command_palette::{CommandPalette, PaletteEvent};
use crate::dispatch::run_action;
use crate::document::ProjectDocument;
use crate::mode::{Mode, ModeEvent, ModeState, View};
use crate::settings::{LayoutSettings, PANEL_SIZE_RANGE};

/// 상태줄 높이의 근삿값. 모드 메뉴를 그 위에 띄울 때 쓴다.
const STATUS_BAR_HEIGHT: Pixels = px(28.);

/// 명령 팔레트의 폭과 창 위쪽에서 떨어진 거리.
const PALETTE_WIDTH: Pixels = px(560.);
const PALETTE_TOP: Pixels = px(64.);

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
    /// 모드 FSM 상태. 키 컨텍스트와 상태줄이 이것을 따른다.
    mode: ModeState,
    /// 열린 상태줄 모드 메뉴와 그 닫힘 구독.
    mode_menu: Option<(Entity<PopupMenu>, Subscription)>,
    /// 열린 명령 팔레트와 그 사건 구독.
    palette: Option<(Entity<CommandPalette>, Subscription)>,
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
            mode: ModeState::INITIAL,
            mode_menu: None,
            palette: None,
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

    /// 가로 패널 그룹의 크기 상태. 자식은 펴진 패널뿐이고 순서는 트랙 목록, 메인, 인스펙터다(접힌
    /// 패널은 빠진다).
    #[must_use]
    pub fn horizontal_state(&self) -> &Entity<ResizableState> {
        &self.horizontal
    }

    /// 세로 패널 그룹의 크기 상태. 위 영역, 그리고 펴져 있으면 하단 영역.
    #[must_use]
    pub fn vertical_state(&self) -> &Entity<ResizableState> {
        &self.vertical
    }

    #[must_use]
    pub fn document(&self) -> &Entity<ProjectDocument> {
        &self.document
    }

    /// 모드 FSM의 현재 상태.
    #[must_use]
    pub const fn mode_state(&self) -> ModeState {
        self.mode
    }

    /// 모드 사건을 넣는다. Action 처리기와 이후 단계(선택 S3, 클립 끝 S5, 고스트 S7)가 부른다.
    pub fn apply_mode_event(
        &mut self,
        event: ModeEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = self.mode.apply(event);
        if next != self.mode {
            self.mode = next;
            cx.notify();
        }
    }

    /// 명령 팔레트가 열려 있는가.
    #[must_use]
    pub const fn palette_open(&self) -> bool {
        self.palette.is_some()
    }

    /// 열린 팔레트의 질의.
    #[must_use]
    pub fn palette_query(&self, cx: &App) -> Option<String> {
        self.palette.as_ref().map(|(p, _)| p.read(cx).query(cx))
    }

    /// 명령 팔레트를 연다(`palette::Open`, UI-2). 이미 열려 있으면 질의로 포커스만 돌린다.
    fn open_palette(&mut self, _: &palette::Open, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((palette, _)) = &self.palette {
            palette.update(cx, |palette, cx| palette.focus_query(window, cx));
            return;
        }
        let target = self.focus.clone();
        let palette = cx.new(|cx| CommandPalette::new(target, window, cx));
        let events = cx.subscribe_in(&palette, window, |this, _, event, window, cx| {
            this.close_palette(window, cx);
            if let PaletteEvent::Run(action) = event {
                run_action(action.boxed_clone(), window, cx);
            }
        });
        palette.update(cx, |palette, cx| palette.focus_query(window, cx));
        self.palette = Some((palette, events));
        cx.notify();
    }

    /// 팔레트를 닫고 문서로 포커스를 돌린다. 실행할 Action은 그 뒤에 문서 포커스로 디스패치된다.
    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// 열린 팔레트. 모드 메뉴처럼 컨텍스트 겹 바깥에 띄운다. 문서 컨텍스트 키가 팔레트 입력에 섞이지
    /// 않게 하기 위해서다(S2-T3 리뷰 1 지적 2와 같은 이유).
    fn palette_overlay(&self, window: &Window) -> Option<impl IntoElement> {
        let (palette, _) = self.palette.as_ref()?;
        let left = ((window.viewport_size().width - PALETTE_WIDTH) / 2.).max(px(0.));
        Some(deferred(
            anchored()
                .position(point(left, PALETTE_TOP))
                .child(palette.clone()),
        ))
    }

    /// 상태줄 모드 메뉴가 열려 있는가.
    #[must_use]
    pub const fn mode_menu_open(&self) -> bool {
        self.mode_menu.is_some()
    }

    /// 상태줄 모드 메뉴를 연다(7.6 「모드 전환」: 모드 표시 클릭, `mode::ShowMenu`).
    fn open_mode_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.mode.mode();
        let menu = PopupMenu::build(window, cx, |mut menu, _, _| {
            for mode in Mode::MENU {
                menu = menu.item(
                    PopupMenuItem::new(mode.label())
                        .checked(mode == current)
                        .on_click(move |_, window, cx| {
                            // 키가 아닌 실행 경로는 실행 함수 하나를 거친다(결정 C, 카운트를 비운다).
                            run_action(mode_action(mode), window, cx);
                        }),
                );
            }
            menu
        });
        let dismissed = cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, window, cx| {
            this.mode_menu = None;
            window.focus(&this.focus, cx);
            cx.notify();
        });
        window.focus(&menu.focus_handle(cx), cx);
        self.mode_menu = Some((menu, dismissed));
        cx.notify();
    }

    fn show_mode_menu(&mut self, _: &mode::ShowMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.open_mode_menu(window, cx);
    }

    /// Action `A`가 오면 사건 `event`를 넣는 처리기를 단다.
    fn on_mode<A: Action>(
        element: Stateful<Div>,
        event: ModeEvent,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element.on_action(cx.listener(move |this, _: &A, window, cx| {
            this.apply_mode_event(event, window, cx);
        }))
    }

    /// 모드 FSM에 사건을 넣는 Action 처리기들(전이 표: `docs/reports/s2-t3/plan.md`).
    fn mode_handlers(element: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        use ModeEvent::{
            ClearSelection, Cycle, CycleBack, DismissGhost, Enter, LeaveAi, OpenClip, ToNormal,
            ToggleView,
        };
        let e = element;
        let e = Self::on_mode::<clip::Open>(e, OpenClip, cx);
        let e = Self::on_mode::<view::Toggle>(e, ToggleView, cx);
        let e = Self::on_mode::<mode::Step>(e, Enter(Mode::Step), cx);
        let e = Self::on_mode::<mode::StepAfterNote>(e, Enter(Mode::Step), cx);
        let e = Self::on_mode::<mode::StepAtBarStart>(e, Enter(Mode::Step), cx);
        let e = Self::on_mode::<mode::StepAtBarEnd>(e, Enter(Mode::Step), cx);
        let e = Self::on_mode::<mode::StepOverwrite>(e, Enter(Mode::Step), cx);
        let e = Self::on_mode::<mode::Sequence>(e, Enter(Mode::Sequence), cx);
        let e = Self::on_mode::<mode::Play>(e, Enter(Mode::Play), cx);
        let e = Self::on_mode::<mode::Lyric>(e, Enter(Mode::Lyric), cx);
        let e = Self::on_mode::<mode::Controller>(e, Enter(Mode::Controller), cx);
        let e = Self::on_mode::<mode::Cycle>(e, Cycle, cx);
        let e = Self::on_mode::<mode::CycleBack>(e, CycleBack, cx);
        let e = Self::on_mode::<mode::Normal>(e, ToNormal, cx);
        let e = Self::on_mode::<select::Clear>(e, ClearSelection, cx);
        let e = Self::on_mode::<ai::OpenPanel>(e, Enter(Mode::Ai), cx);
        let e = Self::on_mode::<ai::Accept>(e, LeaveAi, cx);
        let e = Self::on_mode::<ai::Dismiss>(e, LeaveAi, cx);
        let e = Self::on_mode::<ai::DismissGhost>(e, DismissGhost, cx);
        e.on_action(cx.listener(Self::show_mode_menu))
            .on_action(cx.listener(Self::open_palette))
    }

    /// 키 컨텍스트 경로대로 `div`를 겹친다. GPUI는 요소 하나에 컨텍스트 하나를 쌓아 깊이를 만들고,
    /// `Normal > PianoRoll` 같은 술어는 그 깊이로 맞춘다. 가장 안쪽이 창 포커스를 잡아야 경로 전체가
    /// 디스패치 경로가 된다.
    fn wrap_in_contexts(&self, content: AnyElement) -> AnyElement {
        let path = self.mode.context_path();
        let (innermost, outer) = path.split_last().expect("컨텍스트 경로는 비지 않는다");
        let mut element = div()
            .size_full()
            .key_context(*innermost)
            .track_focus(&self.focus)
            .child(content)
            .into_any_element();
        for context in outer.iter().rev() {
            element = div()
                .size_full()
                .key_context(*context)
                .child(element)
                .into_any_element();
        }
        element
    }

    /// 상태줄 왼쪽의 모드 표시. 누르면 모드 메뉴가 열린다.
    fn mode_indicator(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = self.mode.mode().label();
        div()
            .id("status-mode")
            .role(Role::Button)
            .aria_label(label)
            .test_support()
            .px_1()
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(|this, _, window, cx| this.open_mode_menu(window, cx)))
    }

    /// 열린 모드 메뉴. 컨텍스트 겹 바깥(루트의 자식)에 그린다. 안쪽에 그리면 메뉴 포커스의 디스패치
    /// 경로에 문서 컨텍스트(`Normal > Arrangement` 등)가 들어가, 메뉴가 걸지 않은 키(`v`, `a`)가 메뉴
    /// 뒤의 문서로 간다(S2-T3 리뷰 1 지적 2). 위치는 상태줄 왼쪽 모드 표시 위다.
    fn mode_menu_overlay(&self, window: &Window) -> Option<impl IntoElement> {
        let (menu, _) = self.mode_menu.as_ref()?;
        let origin = point(px(4.), window.viewport_size().height - STATUS_BAR_HEIGHT);
        Some(deferred(
            anchored()
                .position(origin)
                .anchor(Anchor::BottomLeft)
                .child(menu.clone()),
        ))
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
        // 패널을 접거나 펴면 크기 상태를 새로 만든다. 그룹의 자식은 펴진 패널뿐이라(render) 자식 수가
        // 바뀌고, 새 상태는 각 패널의 저장된 크기로 시작한다. 숨은 패널을 상태에 남기면 gpui-base가
        // 그 크기를 합에 넣어 형제 패널의 크기가 틀어진다(리뷰 1 지적 2, 리뷰 2 지적 1).
        let shape_changed = before.track_list_open != after.track_list_open
            || before.inspector_open != after.inspector_open
            || before.bottom_open != after.bottom_open;
        if shape_changed {
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
        let l = self.layout;
        // 그룹의 자식은 펴진 패널뿐이다(접힌 패널은 크기 상태에 없다). 자식 순서대로 인덱스를 매긴다.
        match group {
            Group::Horizontal => {
                let expected = 1 + usize::from(l.track_list_open) + usize::from(l.inspector_open);
                if sizes.len() != expected {
                    return;
                }
                let track = l.track_list_open.then(|| sizes[0]);
                let inspector = l.inspector_open.then(|| sizes[sizes.len() - 1]);
                self.change_layout(cx, |l| {
                    if let Some(v) = track.filter(|&v| keep(v)) {
                        l.track_list_width = v;
                    }
                    if let Some(v) = inspector.filter(|&v| keep(v)) {
                        l.inspector_width = v;
                    }
                });
            }
            Group::Vertical => {
                if !l.bottom_open || sizes.len() != 2 {
                    return;
                }
                let bottom = sizes[1];
                if keep(bottom) {
                    self.change_layout(cx, |l| l.bottom_height = bottom);
                }
            }
        }
    }

    /// 자리표시자 영역. 내용은 S2-T5 이후 채운다.
    fn region(id: &'static str, label: &'static str, cx: &App) -> impl IntoElement {
        div()
            .id(id)
            .aria_label(label)
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = self.layout;
        let range = px(*PANEL_SIZE_RANGE.start())..px(*PANEL_SIZE_RANGE.end());
        let main_label = match self.mode.view() {
            View::Arrangement => "배치 뷰",
            View::PianoRoll => "피아노롤",
        };
        let content = div()
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
                                    .when(layout.track_list_open, |g| {
                                        g.child(
                                            resizable_panel()
                                                .size(px(layout.track_list_width))
                                                .size_range(range.clone())
                                                .child(Self::region("track-list", "트랙 목록", cx)),
                                        )
                                    })
                                    .child(resizable_panel().child(Self::region(
                                        "main-view",
                                        main_label,
                                        cx,
                                    )))
                                    .when(layout.inspector_open, |g| {
                                        g.child(
                                            resizable_panel()
                                                .size(px(layout.inspector_width))
                                                .size_range(range.clone())
                                                .child(Self::region(
                                                    "inspector",
                                                    "트랙 인스펙터",
                                                    cx,
                                                )),
                                        )
                                    }),
                            ),
                        )
                        .when(layout.bottom_open, |g| {
                            g.child(
                                resizable_panel()
                                    .size(px(layout.bottom_height))
                                    .size_range(range.clone())
                                    .child(Self::region(
                                        "bottom-panel",
                                        "컨트롤러 레인 / 건반 뷰",
                                        cx,
                                    )),
                            )
                        }),
                ),
            )
            .child(
                div()
                    .id("status-bar")
                    .test_support()
                    .flex_none()
                    .child(StatusBar::new().left(self.mode_indicator(cx))),
            )
            .into_any_element();
        // 처리기는 바깥 요소에 단다. 디스패치는 포커스(가장 안쪽)에서 바깥으로 올라온다.
        let root = div()
            .id("project-window")
            .size_full()
            .on_action(cx.listener(Self::toggle_track_list))
            .on_action(cx.listener(Self::toggle_inspector))
            .on_action(cx.listener(Self::toggle_bottom));
        Self::mode_handlers(root, cx)
            .child(self.wrap_in_contexts(content))
            .children(self.mode_menu_overlay(window))
            .children(self.palette_overlay(window))
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

/// 상태줄 모드 메뉴 항목이 실행하는 Action. 키·팔레트와 같은 처리기를 거친다.
fn mode_action(mode: Mode) -> Box<dyn Action> {
    match mode {
        Mode::Normal => Box::new(mode::Normal),
        Mode::Step => Box::new(mode::Step),
        Mode::Sequence => Box::new(mode::Sequence),
        Mode::Play => Box::new(mode::Play),
        Mode::Ai => Box::new(ai::OpenPanel),
        Mode::Lyric => Box::new(mode::Lyric),
        Mode::Controller => Box::new(mode::Controller),
    }
}
