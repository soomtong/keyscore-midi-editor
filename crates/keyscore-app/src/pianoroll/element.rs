//! 피아노롤 `Element`. 장면(`scene::Scene`)을 `paint_layer` 하나 안에서 `paint_quad`로 그린다.

use gpui_kit::{
    App, Bounds, Element, ElementId, GlobalElementId, Hsla, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use keyscore_core::{Key, MeterMap, Tick, TrackId};

use super::{Cursor, PianoRollState};
use crate::document::ProjectDocument;

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

/// 클릭·휠 콜백.
pub type ClickHandler = Box<dyn Fn(Cursor, &mut Window, &mut App)>;

pub struct PianoRollElement {
    pub(crate) state: PianoRollState,
    pub(crate) document: gpui_kit::Entity<ProjectDocument>,
    pub(crate) track: Option<TrackId>,
    pub(crate) playhead: Option<Tick>,
    /// 재생 중이면 매 프레임 다시 그린다(UI-4).
    pub(crate) animating: bool,
    pub(crate) palette: Palette,
    pub(crate) on_click: Option<ClickHandler>,
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
        }
    }

    #[must_use]
    pub fn playhead(mut self, playhead: Option<Tick>, animating: bool) -> Self {
        self.playhead = playhead;
        self.animating = animating;
        self
    }

    #[must_use]
    pub fn on_click(mut self, handler: ClickHandler) -> Self {
        self.on_click = Some(handler);
        self
    }

    fn meter_and_key(&self, cx: &App) -> (MeterMap, Key) {
        let doc = self.document.read(cx);
        (doc.project().meter_map().clone(), doc.project().key())
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
    type PrepaintState = Option<gpui_kit::Hitbox>;

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
        todo!()
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<gpui_kit::Hitbox> {
        todo!()
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Option<gpui_kit::Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        todo!()
    }
}
