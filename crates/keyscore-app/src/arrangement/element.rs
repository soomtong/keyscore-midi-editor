//! 배치 뷰 `Element`. 장면(`scene::Scene`)을 `paint_layer` 하나 안에서 `paint_quad`로 그리고, 블록의 이름·가사는
//! 같은 레이어 안에서 글로 그린다.
//!
//! 레이어 밖에서 그리면 quad마다 `BoundsTree` 삽입이 일어난다(`docs/spikes/sp2-element-instancing.md`). 피아노롤
//! Element(S3-T1)와 같은 구조다.

use std::rc::Rc;

use gpui_kit::{
    App, BorderStyle, Bounds, ContentMask, DispatchPhase, Element, ElementId, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, Modifiers,
    MouseButton, MouseDownEvent, MouseUpEvent, PinchEvent, Pixels, ScrollDelta, ScrollWheelEvent,
    SharedString, Style, TextAlign, TextRun, Window, fill, point, px, quad, relative, size,
};
use keyscore_core::{Tick, TrackId};

use super::scene::{self, LineKind, Rect, SceneInput, StepGhost};
use super::{ArrangementState, ZOOM_FACTOR};
use crate::document::ProjectDocument;

/// 휠의 「줄」 단위를 픽셀로.
const LINE_PX: f32 = 20.0;
/// 블록 라벨 글자 크기와 줄 높이(행 높이 28 px 안에 두 줄).
const LABEL_FONT_PX: f32 = 11.0;
const LABEL_LINE_PX: f32 = 13.0;
/// 라벨의 블록 안 여백.
const LABEL_PAD_PX: f32 = 3.0;

/// 그릴 때 쓰는 색.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub row_light: Hsla,
    pub row_dark: Hsla,
    /// 현재 트랙 행.
    pub row_current: Hsla,
    pub bar_line: Hsla,
    pub beat_line: Hsla,
    pub clip_border: Hsla,
    pub label: Hsla,
    /// 첫 마디 가사(흐리게).
    pub lyric: Hsla,
    pub selected: Hsla,
    pub cursor: Hsla,
    pub ghost: Hsla,
    pub ghost_blocked: Hsla,
    pub playhead: Hsla,
    /// 트랙 색 0–11의 테마 색(`crate::palette::track_color`).
    pub track_colors: [Hsla; 12],
}

/// 왼쪽 단추 누름.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    /// 뷰 기준 좌표.
    pub x: f32,
    pub y: f32,
    /// 눌린 칸(트랙 행, 스텝으로 내린 틱). 뷰포트·트랙 행 밖이면 `None`.
    pub cell: Option<(usize, Tick)>,
    pub modifiers: Modifiers,
    /// 누른 횟수(더블클릭이면 2).
    pub click_count: usize,
}

/// 누름 콜백.
pub type PressHandler = Rc<dyn Fn(&Press, &mut Window, &mut App)>;
/// 휠·핀치로 뷰포트(스크롤·배율)가 바뀐 뒤의 콜백.
pub type ViewChangeHandler = Rc<dyn Fn(&mut Window, &mut App)>;
/// 우클릭 콜백: 눌린 칸과 창 좌표(메뉴를 띄울 자리).
pub type RightClickHandler =
    Rc<dyn Fn((usize, Tick), gpui_kit::Point<Pixels>, &mut Window, &mut App)>;

pub struct ArrangementElement {
    pub(crate) state: ArrangementState,
    pub(crate) document: gpui_kit::Entity<ProjectDocument>,
    pub(crate) current_track: Option<TrackId>,
    /// 창의 커서 틱(피아노롤 커서와 같다).
    pub(crate) cursor: Tick,
    pub(crate) step_ticks: u64,
    pub(crate) ghost: Option<StepGhost>,
    pub(crate) playhead: Option<Tick>,
    /// 재생 중이면 매 프레임 다시 그린다(UI-4).
    pub(crate) animating: bool,
    pub(crate) palette: Palette,
    pub(crate) on_press: Option<PressHandler>,
    pub(crate) on_view_change: Option<ViewChangeHandler>,
    pub(crate) on_right_click: Option<RightClickHandler>,
}

impl ArrangementElement {
    #[must_use]
    pub fn new(
        state: ArrangementState,
        document: gpui_kit::Entity<ProjectDocument>,
        current_track: Option<TrackId>,
        cursor: Tick,
        step_ticks: u64,
        palette: Palette,
    ) -> Self {
        ArrangementElement {
            state,
            document,
            current_track,
            cursor,
            step_ticks,
            ghost: None,
            playhead: None,
            animating: false,
            palette,
            on_press: None,
            on_view_change: None,
            on_right_click: None,
        }
    }

    #[must_use]
    pub fn playhead(mut self, playhead: Option<Tick>, animating: bool) -> Self {
        self.playhead = playhead;
        self.animating = animating;
        self
    }

    /// Step 모드의 고스트 클립(클립 밖에서 입력하면 생길 클립).
    #[must_use]
    pub fn ghost(mut self, ghost: Option<StepGhost>) -> Self {
        self.ghost = ghost;
        self
    }

    /// 왼쪽 단추 누름(배치 뷰 위에서만).
    #[must_use]
    pub fn on_press(mut self, handler: impl Fn(&Press, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Rc::new(handler));
        self
    }

    /// 휠·핀치가 뷰포트를 바꾼 뒤.
    #[must_use]
    pub fn on_view_change(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_view_change = Some(Rc::new(handler));
        self
    }

    /// 우클릭(마크 메뉴, UI-11).
    #[must_use]
    pub fn on_right_click(
        mut self,
        handler: impl Fn((usize, Tick), gpui_kit::Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_right_click = Some(Rc::new(handler));
        self
    }

    fn rect(origin: gpui_kit::Point<Pixels>, r: Rect) -> Bounds<Pixels> {
        Bounds::new(
            point(origin.x + px(r.x), origin.y + px(r.y)),
            size(px(r.w), px(r.h)),
        )
    }
}

impl IntoElement for ArrangementElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ArrangementElement {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        let doc = self.document.read(cx);
        let project = doc.project();
        let mut guard = self.state.inner().borrow_mut();
        let inner = &mut *guard;
        inner.set_size(
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
            project.tracks().len(),
        );
        // 문서가 바뀌었으면 없어진 클립을 선택에서 뺀다(불변식: 선택은 존재하는 클립만). 모드 FSM의 선택 유무는
        // 창이 문서 변경 때 맞춘다.
        if inner.revision != Some(doc.revision()) {
            inner.revision = Some(doc.revision());
            inner.selection.retain_existing(project);
        }
        let viewport = inner.viewport();
        let super::Inner {
            scene, selection, ..
        } = inner;
        scene::build(
            &SceneInput {
                project,
                current_track: self.current_track,
                cursor: self.cursor,
                step_ticks: self.step_ticks,
                selection,
                ghost: self.ghost,
                playhead: self.playhead,
                viewport,
            },
            scene,
        );
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = self.state.clone();
        let painted = {
            let inner = state.inner().borrow();
            let scene = &inner.scene;
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                window.paint_layer(bounds, |window| self.paint_scene(scene, bounds, window, cx))
            })
        };
        state.inner().borrow_mut().painted = painted;
        if self.animating {
            window.request_animation_frame();
        }
        self.register_mouse(bounds, hitbox, window);
    }
}

impl ArrangementElement {
    /// 레이어 안에서 장면을 그린다. 그리는 순서가 곧 겹침 순서다. 실제로 그린 클립 블록 수를 돌려준다.
    fn paint_scene(
        &self,
        scene: &scene::Scene,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> usize {
        let palette = self.palette;
        let origin = bounds.origin;
        let height = f32::from(bounds.size.height);
        let mut painted = 0;
        for row in &scene.rows {
            let color = if row.current {
                palette.row_current
            } else if row.index % 2 == 0 {
                palette.row_light
            } else {
                palette.row_dark
            };
            window.paint_quad(fill(Self::rect(origin, row.rect), color));
        }
        for line in &scene.lines {
            let color = match line.kind {
                LineKind::Bar => palette.bar_line,
                LineKind::Beat => palette.beat_line,
            };
            let r = Rect {
                x: line.x,
                y: 0.0,
                w: 1.0,
                h: height,
            };
            window.paint_quad(fill(Self::rect(origin, r), color));
        }
        for block in &scene.clips {
            let base = palette.track_colors[usize::from(block.color.index()) % 12];
            let (background, border, border_w) = if block.selected {
                (base.opacity(0.9), palette.selected, 2.0)
            } else {
                (base.opacity(0.7), palette.clip_border, 1.0)
            };
            let block_bounds = Self::rect(origin, block.rect);
            window.paint_quad(quad(
                block_bounds,
                px(3.),
                background,
                px(border_w),
                border,
                BorderStyle::Solid,
            ));
            painted += 1;
            if block.name.is_some() || block.lyric.is_some() {
                self.paint_labels(block, block_bounds, window, cx);
            }
        }
        if let Some(ghost) = scene.ghost {
            let color = if ghost.blocked {
                palette.ghost_blocked
            } else {
                palette.ghost
            };
            window.paint_quad(quad(
                Self::rect(origin, ghost.rect),
                px(3.),
                color.opacity(0.25),
                px(1.5),
                color,
                BorderStyle::Dashed,
            ));
        }
        if let Some(cursor) = scene.cursor {
            window.paint_quad(quad(
                Self::rect(origin, cursor),
                px(2.),
                palette.cursor.opacity(0.25),
                px(1.5),
                palette.cursor,
                BorderStyle::Solid,
            ));
        }
        if let Some(x) = scene.playhead {
            let r = Rect {
                x,
                y: 0.0,
                w: 2.0,
                h: height,
            };
            window.paint_quad(fill(Self::rect(origin, r), palette.playhead));
        }
        painted
    }

    /// 블록 안 왼쪽 위에 이름, 그 아래에 첫 마디 가사(흐리게). 블록 폭 안에서 자른다.
    fn paint_labels(
        &self,
        block: &scene::ClipBlock,
        block_bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let style = window.text_style();
        let font = style.font();
        let lines = [
            (block.name.as_deref(), self.palette.label),
            (block.lyric.as_deref(), self.palette.lyric),
        ];
        let mut y = block_bounds.origin.y + px(LABEL_PAD_PX - 1.0);
        window.with_content_mask(
            Some(ContentMask {
                bounds: block_bounds,
            }),
            |window| {
                for (text, color) in lines {
                    let Some(text) = text else {
                        continue;
                    };
                    let run = TextRun {
                        len: text.len(),
                        font: font.clone(),
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped = window.text_system().shape_line(
                        SharedString::from(text.to_owned()),
                        px(LABEL_FONT_PX),
                        &[run],
                        None,
                    );
                    let at = point(block_bounds.origin.x + px(LABEL_PAD_PX), y);
                    // 글꼴이 없어 못 그리는 경우는 블록만 남긴다(라벨은 보조 정보)
                    let _ = shaped.paint(at, px(LABEL_LINE_PX), TextAlign::Left, None, window, cx);
                    y += px(LABEL_LINE_PX);
                }
            },
        );
    }

    /// 누름·우클릭, 그리고 휠·핀치(`register_view_input`).
    fn register_mouse(&self, bounds: Bounds<Pixels>, hitbox: &Hitbox, window: &mut Window) {
        let origin = bounds.origin;
        let state = self.state.clone();
        let step = self.step_ticks;
        if let Some(on_press) = self.on_press.clone() {
            let press_state = state.clone();
            let hit = hitbox.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble
                    || event.button != MouseButton::Left
                    || !hit.is_hovered(window)
                {
                    return;
                }
                let local = event.position - origin;
                let (x, y) = (f32::from(local.x), f32::from(local.y));
                let cell = {
                    let inner = press_state.inner().borrow();
                    inner.viewport().cell_at(x, y, step, inner.rows)
                };
                let press = Press {
                    x,
                    y,
                    cell,
                    modifiers: event.modifiers,
                    click_count: event.click_count,
                };
                on_press(&press, window, cx);
            });
        }
        // 우클릭은 뗄 때 연다(피아노롤과 같은 이유: 누를 때 열면 겹 바깥 누름 처리기가 포커스를 돌려 메뉴가
        // `Esc`로 닫히지 않는다).
        if let Some(on_right_click) = self.on_right_click.clone() {
            let right_state = state.clone();
            let right_hit = hitbox.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble
                    || event.button != MouseButton::Right
                    || !right_hit.is_hovered(window)
                {
                    return;
                }
                let local = event.position - origin;
                let cell = {
                    let inner = right_state.inner().borrow();
                    inner.viewport().cell_at(
                        f32::from(local.x),
                        f32::from(local.y),
                        step,
                        inner.rows,
                    )
                };
                if let Some(cell) = cell {
                    on_right_click(cell, event.position, window, cx);
                }
            });
        }
        self.register_view_input(origin, hitbox, window);
    }

    /// 휠(스크롤, `Cmd+휠` 줌)과 핀치(줌). 바뀐 뒤 `on_view_change`를 부른다. 피아노롤(S3-T6a)과 같은 규칙.
    fn register_view_input(
        &self,
        origin: gpui_kit::Point<Pixels>,
        hitbox: &Hitbox,
        window: &mut Window,
    ) {
        let state = self.state.clone();
        let wheel_hit = hitbox.clone();
        let pinch_hit = hitbox.clone();
        let pinch_state = state.clone();
        let pinch_changed = self.on_view_change.clone();
        let wheel_changed = self.on_view_change.clone();
        window.on_mouse_event(move |event: &PinchEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !pinch_hit.should_handle_scroll(window) {
                return;
            }
            let x = f32::from((event.position - origin).x);
            pinch_state.zoom_at(1.0 + f64::from(event.delta.max(-0.9)), x);
            if let Some(f) = &pinch_changed {
                f(window, cx);
            }
            window.refresh();
        });
        // `is_hovered`는 마지막 입력이 키보드면 거짓이므로 휠은 `should_handle_scroll`로 본다(S3-T1 리뷰 1 지적 2).
        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !wheel_hit.should_handle_scroll(window) {
                return;
            }
            let (dx, dy) = match event.delta {
                ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y)),
                ScrollDelta::Lines(l) => (l.x * LINE_PX, l.y * LINE_PX),
            };
            if event.modifiers.platform {
                let x = f32::from((event.position - origin).x);
                state.zoom_at(ZOOM_FACTOR.powf(f64::from(dy / LINE_PX)), x);
            } else {
                // 휠 델타는 내용이 움직이는 방향(자연 스크롤)이라 뷰포트는 반대로 간다.
                let (dx, dy) = if event.modifiers.shift && dx == 0.0 {
                    (-dy, 0.0)
                } else {
                    (-dx, -dy)
                };
                state.scroll_by(dx, dy);
            }
            if let Some(f) = &wheel_changed {
                f(window, cx);
            }
            window.refresh();
        });
    }
}
