//! 피아노롤 `Element`. 장면(`scene::Scene`)을 `paint_layer` 하나 안에서 `paint_quad`로 그린다.
//!
//! 레이어 밖에서 그리면 quad마다 `BoundsTree` 삽입이 일어나 노트 10,000개에서 paint가 6–8 ms다. 레이어
//! 안이면 같은 order로 삽입 순서대로 그려져 2 ms 아래다(`docs/spikes/sp2-element-instancing.md`).

use std::rc::Rc;

use gpui_kit::{
    App, BorderStyle, Bounds, ContentMask, DispatchPhase, Element, ElementId, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PinchEvent, Pixels, ScrollDelta,
    ScrollWheelEvent, Style, Window, fill, point, px, quad, relative, size,
};
use keyscore_core::{Tick, TrackId};

use super::scene::{self, LineKind, Rect, SceneInput, Shade};
use super::{Cursor, PianoRollState, ZOOM_FACTOR};
use crate::document::ProjectDocument;

/// 휠의 「줄」 단위를 픽셀로.
const LINE_PX: f32 = 20.0;

/// 그릴 때 쓰는 색.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub row_light: Hsla,
    pub row_dark: Hsla,
    pub bar_line: Hsla,
    pub beat_line: Hsla,
    pub note: Hsla,
    pub note_border: Hsla,
    pub note_dim: Hsla,
    pub selected: Hsla,
    pub cursor: Hsla,
    pub playhead: Hsla,
}

/// 왼쪽 단추 누름(S3-T6a).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    /// 뷰 기준 좌표.
    pub x: f32,
    pub y: f32,
    /// 눌린 칸. 뷰포트 밖이면 `None`.
    pub cell: Option<Cursor>,
    pub modifiers: Modifiers,
    /// 누른 횟수(더블클릭이면 2).
    pub click_count: usize,
}

/// 누름 콜백.
pub type PressHandler = Rc<dyn Fn(&Press, &mut Window, &mut App)>;
/// 끌기·뗌 콜백: 뷰 기준 좌표(피아노롤 밖일 수 있다)와 그때의 수식키(S3-T6b: 스냅 해제·한 축·복제).
pub type PointerHandler = Rc<dyn Fn((f32, f32), Modifiers, &mut Window, &mut App)>;
/// 휠·핀치로 뷰포트(스크롤·배율)가 바뀐 뒤의 콜백.
pub type ViewChangeHandler = Rc<dyn Fn(&mut Window, &mut App)>;
/// 우클릭 콜백: 눌린 칸과 창 좌표(메뉴를 띄울 자리).
pub type RightClickHandler = Rc<dyn Fn(Cursor, gpui_kit::Point<Pixels>, &mut Window, &mut App)>;

pub struct PianoRollElement {
    pub(crate) state: PianoRollState,
    pub(crate) document: gpui_kit::Entity<ProjectDocument>,
    pub(crate) track: Option<TrackId>,
    pub(crate) playhead: Option<Tick>,
    /// 재생 중이면 매 프레임 다시 그린다(UI-4).
    pub(crate) animating: bool,
    pub(crate) palette: Palette,
    pub(crate) on_press: Option<PressHandler>,
    pub(crate) on_drag: Option<PointerHandler>,
    pub(crate) on_release: Option<PointerHandler>,
    pub(crate) on_view_change: Option<ViewChangeHandler>,
    pub(crate) on_right_click: Option<RightClickHandler>,
}

impl PianoRollElement {
    #[must_use]
    pub fn new(
        state: PianoRollState,
        document: gpui_kit::Entity<ProjectDocument>,
        track: Option<TrackId>,
        palette: Palette,
    ) -> Self {
        PianoRollElement {
            state,
            document,
            track,
            playhead: None,
            animating: false,
            palette,
            on_press: None,
            on_drag: None,
            on_release: None,
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

    /// 왼쪽 단추 누름(피아노롤 위에서만).
    #[must_use]
    pub fn on_press(mut self, handler: impl Fn(&Press, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Rc::new(handler));
        self
    }

    /// 왼쪽 단추를 누른 채 움직임. 포인터가 피아노롤 밖이어도 부른다(러버밴드가 가장자리에서 멈추게).
    #[must_use]
    pub fn on_drag(
        mut self,
        handler: impl Fn((f32, f32), Modifiers, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_drag = Some(Rc::new(handler));
        self
    }

    /// 휠·핀치가 뷰포트를 바꾼 뒤. 러버밴드처럼 뷰포트에 따라 결과가 바뀌는 조작이 다시 계산한다(S3-T6a 리뷰 2
    /// 지적 1).
    #[must_use]
    pub fn on_view_change(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_view_change = Some(Rc::new(handler));
        self
    }

    /// 왼쪽 단추 뗌. 피아노롤 밖에서 떼어도 부른다(조작이 끝나지 않은 채 남지 않게).
    #[must_use]
    pub fn on_release(
        mut self,
        handler: impl Fn((f32, f32), Modifiers, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_release = Some(Rc::new(handler));
        self
    }

    /// 우클릭(S3-T2: 마크 메뉴, UI-11).
    #[must_use]
    pub fn on_right_click(
        mut self,
        handler: impl Fn(Cursor, gpui_kit::Point<Pixels>, &mut Window, &mut App) + 'static,
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

impl IntoElement for PianoRollElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PianoRollElement {
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
        let track = self.track.and_then(|id| project.track(id));
        let mut guard = self.state.inner().borrow_mut();
        let inner = &mut *guard;
        inner.set_size(f32::from(bounds.size.width), f32::from(bounds.size.height));
        inner.cache.refresh(track, doc.revision());
        let current = track.and_then(|t| {
            let tick = inner.cursor.tick.get();
            t.clips()
                .find(|(_, c)| c.start().get() <= tick && tick < c.end())
                .map(|(id, _)| id)
        });
        let viewport = inner.viewport();
        let super::Inner {
            scene,
            cache,
            selection,
            cursor,
            step_ticks,
            gesture,
            ..
        } = inner;
        scene::build(
            &SceneInput {
                track,
                current,
                cache,
                meter: project.meter_map(),
                key: project.key(),
                cursor: *cursor,
                step_ticks: *step_ticks,
                selection,
                playhead: self.playhead,
                viewport,
            },
            scene,
        );
        scene.band = gesture.as_ref().and_then(|g| g.band_rect(&viewport));
        scene.ghost_blocked = if let Some(g) = gesture {
            g.ghost_rects(&viewport, &mut scene.ghosts)
        } else {
            scene.ghosts.clear();
            false
        };
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
        _: &mut App,
    ) {
        let state = self.state.clone();
        let painted = {
            let inner = state.inner().borrow();
            let scene = &inner.scene;
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                window.paint_layer(bounds, |window| self.paint_scene(scene, bounds, window))
            })
        };
        state.inner().borrow_mut().painted = painted;
        if self.animating {
            window.request_animation_frame();
        }
        self.register_mouse(bounds, hitbox, window);
    }
}

impl PianoRollElement {
    /// 레이어 안에서 장면을 그린다. 그리는 순서가 곧 겹침 순서다(같은 레이어 안은 삽입 순). 실제로 그린
    /// 노트 수를 돌려준다(리뷰 1 제안 1: 장면의 노트 수와 따로 센다).
    fn paint_scene(
        &self,
        scene: &scene::Scene,
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) -> usize {
        let palette = self.palette;
        let origin = bounds.origin;
        let mut painted = 0;
        {
            {
                {
                    for row in &scene.rows {
                        let color = match row.shade {
                            Shade::Light => palette.row_light,
                            Shade::Dark => palette.row_dark,
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
                            h: f32::from(bounds.size.height),
                        };
                        window.paint_quad(fill(Self::rect(origin, r), color));
                    }
                    for note in &scene.notes {
                        let background = if note.selected {
                            palette.selected
                        } else if note.dim {
                            palette.note_dim
                        } else {
                            // 벨로시티가 낮을수록 옅게
                            palette
                                .note
                                .opacity(0.55 + 0.45 * f32::from(note.velocity) / 127.0)
                        };
                        let border = if note.dim {
                            palette.note_dim
                        } else {
                            palette.note_border
                        };
                        window.paint_quad(quad(
                            Self::rect(origin, note.rect),
                            px(2.),
                            background,
                            px(1.),
                            border,
                            BorderStyle::Solid,
                        ));
                        painted += 1;
                    }
                    // 고스트(MS-1): 결과 자리의 테두리. 거부면 재생 헤드 색
                    let ghost = if scene.ghost_blocked {
                        palette.playhead
                    } else {
                        palette.selected
                    };
                    for rect in &scene.ghosts {
                        window.paint_quad(quad(
                            Self::rect(origin, *rect),
                            px(2.),
                            ghost.opacity(0.3),
                            px(1.5),
                            ghost,
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
                    if let Some(band) = scene.band {
                        window.paint_quad(quad(
                            Self::rect(origin, band),
                            px(0.),
                            palette.selected.opacity(0.15),
                            px(1.),
                            palette.selected,
                            BorderStyle::Solid,
                        ));
                    }
                    if let Some(x) = scene.playhead {
                        let r = Rect {
                            x,
                            y: 0.0,
                            w: 2.0,
                            h: f32::from(bounds.size.height),
                        };
                        window.paint_quad(fill(Self::rect(origin, r), palette.playhead));
                    }
                }
            }
        }
        painted
    }

    /// 누름·끌기·뗌·우클릭, 그리고 휠·핀치(`register_view_input`).
    fn register_mouse(&self, bounds: Bounds<Pixels>, hitbox: &Hitbox, window: &mut Window) {
        let origin = bounds.origin;
        let state = self.state.clone();
        let hit = hitbox.clone();
        if let Some(on_press) = self.on_press.clone() {
            let press_state = state.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble
                    || event.button != MouseButton::Left
                    || !hit.is_hovered(window)
                {
                    return;
                }
                let local = event.position - origin;
                let (x, y) = (f32::from(local.x), f32::from(local.y));
                let press = Press {
                    x,
                    y,
                    cell: press_state.viewport().cell_at(x, y),
                    modifiers: event.modifiers,
                    click_count: event.click_count,
                };
                on_press(&press, window, cx);
            });
        }
        if let Some(on_drag) = self.on_drag.clone() {
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || !event.dragging() {
                    return;
                }
                let local = event.position - origin;
                on_drag(
                    (f32::from(local.x), f32::from(local.y)),
                    event.modifiers,
                    window,
                    cx,
                );
            });
        }
        if let Some(on_release) = self.on_release.clone() {
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                    return;
                }
                let local = event.position - origin;
                on_release(
                    (f32::from(local.x), f32::from(local.y)),
                    event.modifiers,
                    window,
                    cx,
                );
            });
        }
        // 우클릭은 뗄 때 연다. 누를 때는 창의 겹 바깥 누름 처리기가 포커스를 문서로 돌리므로, 누를 때 메뉴를
        // 열면 메뉴가 포커스를 잃어 Esc로 닫히지 않는다.
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
                if let Some(cell) = right_state
                    .viewport()
                    .cell_at(f32::from(local.x), f32::from(local.y))
                {
                    on_right_click(cell, event.position, window, cx);
                }
            });
        }
        self.register_view_input(origin, hitbox, window);
    }

    /// 휠(스크롤, `Cmd+휠` 줌)과 핀치(줌). 바뀐 뒤 `on_view_change`를 부른다.
    fn register_view_input(
        &self,
        origin: gpui_kit::Point<Pixels>,
        hitbox: &Hitbox,
        window: &mut Window,
    ) {
        let state = self.state.clone();
        let wheel_hit = hitbox.clone();
        // `is_hovered`는 마지막 입력이 키보드면 거짓이고 휠은 입력 종류를 바꾸지 않는다. 키를 친 뒤 포인터를
        // 움직이지 않고 굴리는 흔한 순서가 모두 무시되므로 GPUI `Div`처럼 `should_handle_scroll`을 쓴다
        // (리뷰 1 지적 2).
        let pinch_hit = hitbox.clone();
        let pinch_state = state.clone();
        let pinch_changed = self.on_view_change.clone();
        let wheel_changed = self.on_view_change.clone();
        window.on_mouse_event(move |event: &PinchEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !pinch_hit.should_handle_scroll(window) {
                return;
            }
            // 핀치는 가로 줌이다(MS-7). 포인터 아래 틱을 제자리에 둔다.
            let x = f32::from((event.position - origin).x);
            pinch_state.zoom_at(1.0 + f64::from(event.delta.max(-0.9)), x);
            if let Some(f) = &pinch_changed {
                f(window, cx);
            }
            window.refresh();
        });
        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !wheel_hit.should_handle_scroll(window) {
                return;
            }
            let (dx, dy) = match event.delta {
                ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y)),
                ScrollDelta::Lines(l) => (l.x * LINE_PX, l.y * LINE_PX),
            };
            if event.modifiers.platform {
                // `Cmd+휠`은 가로 줌(7.4). 한 줄마다 한 단계, 위로 굴리면(내용이 아래로) 확대다.
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
