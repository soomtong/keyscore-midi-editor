//! 피아노롤 `Element`. 장면(`scene::Scene`)을 `paint_layer` 하나 안에서 `paint_quad`로 그린다.
//!
//! 레이어 밖에서 그리면 quad마다 `BoundsTree` 삽입이 일어나 노트 10,000개에서 paint가 6–8 ms다. 레이어
//! 안이면 같은 order로 삽입 순서대로 그려져 2 ms 아래다(`docs/spikes/sp2-element-instancing.md`).

use std::rc::Rc;

use gpui_kit::{
    App, BorderStyle, Bounds, ContentMask, DispatchPhase, Element, ElementId, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, MouseButton,
    MouseDownEvent, MouseUpEvent, Pixels, ScrollDelta, ScrollWheelEvent, Style, Window, fill,
    point, px, quad, relative, size,
};
use keyscore_core::{Tick, TrackId};

use super::scene::{self, LineKind, Rect, SceneInput, Shade};
use super::{Cursor, PianoRollState};
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

/// 클릭 콜백: 눌린 칸.
pub type ClickHandler = Rc<dyn Fn(Cursor, &mut Window, &mut App)>;
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
    pub(crate) on_click: Option<ClickHandler>,
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
            on_click: None,
            on_right_click: None,
        }
    }

    #[must_use]
    pub fn playhead(mut self, playhead: Option<Tick>, animating: bool) -> Self {
        self.playhead = playhead;
        self.animating = animating;
        self
    }

    #[must_use]
    pub fn on_click(mut self, handler: impl Fn(Cursor, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
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
                            h: f32::from(bounds.size.height),
                        };
                        window.paint_quad(fill(Self::rect(origin, r), palette.playhead));
                    }
                }
            }
        }
        painted
    }

    /// 클릭: 그 칸으로 커서. 휠: 스크롤(Shift+휠은 가로).
    fn register_mouse(&self, bounds: Bounds<Pixels>, hitbox: &Hitbox, window: &mut Window) {
        let origin = bounds.origin;
        let state = self.state.clone();
        let hit = hitbox.clone();
        let wheel_hit = hitbox.clone();
        if let Some(on_click) = self.on_click.clone() {
            let click_state = state.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble
                    || event.button != MouseButton::Left
                    || !hit.is_hovered(window)
                {
                    return;
                }
                let local = event.position - origin;
                if let Some(cell) = click_state
                    .viewport()
                    .cell_at(f32::from(local.x), f32::from(local.y))
                {
                    on_click(cell, window, cx);
                }
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
        // `is_hovered`는 마지막 입력이 키보드면 거짓이고 휠은 입력 종류를 바꾸지 않는다. 키를 친 뒤 포인터를
        // 움직이지 않고 굴리는 흔한 순서가 모두 무시되므로 GPUI `Div`처럼 `should_handle_scroll`을 쓴다
        // (리뷰 1 지적 2).
        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble || !wheel_hit.should_handle_scroll(window) {
                return;
            }
            let (dx, dy) = match event.delta {
                ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y)),
                ScrollDelta::Lines(l) => (l.x * LINE_PX, l.y * LINE_PX),
            };
            // 휠 델타는 내용이 움직이는 방향(자연 스크롤)이라 뷰포트는 반대로 간다.
            let (dx, dy) = if event.modifiers.shift && dx == 0.0 {
                (-dy, 0.0)
            } else {
                (-dx, -dy)
            };
            state.scroll_by(dx, dy);
            window.refresh();
        });
    }
}
