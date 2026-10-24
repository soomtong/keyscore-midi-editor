//! 피아노롤을 창에 붙인 부분(S3-T1): Action 처리기, 그리기, 재생 헤드.

use gpui_kit::{Context, Div, IntoElement, Stateful, Window};

use crate::actions::{cursor, view};
use crate::pianoroll::CursorMove;
use crate::window::ProjectWindow;

impl ProjectWindow {
    pub(crate) fn piano_roll_handlers(
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let _ = (element, cx);
        todo!()
    }

    /// 커서를 옮기고 뷰포트가 따라가게 한다.
    pub(crate) fn move_piano_roll_cursor(&mut self, delta: CursorMove, cx: &mut Context<Self>) {
        let _ = (delta, cx);
        todo!()
    }

    /// 가운데 주 뷰의 피아노롤.
    pub(crate) fn render_piano_roll(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let _ = (window, cx);
        gpui_kit::div()
    }

    /// 피아노롤로 들어갈 때 커서를 클립 안에 둔다.
    pub(crate) fn entered_piano_roll(&mut self, cx: &mut Context<Self>) {
        let _ = cx;
        todo!()
    }

    pub(crate) fn cursor_left(&mut self, _: &cursor::Left, _: &mut Window, cx: &mut Context<Self>) {
        self.move_piano_roll_cursor(CursorMove::StepBack, cx);
    }

    pub(crate) fn cursor_right(
        &mut self,
        _: &cursor::Right,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_piano_roll_cursor(CursorMove::StepForward, cx);
    }

    pub(crate) fn cursor_up(&mut self, _: &cursor::Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_piano_roll_cursor(CursorMove::SemitoneUp, cx);
    }

    pub(crate) fn cursor_down(&mut self, _: &cursor::Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_piano_roll_cursor(CursorMove::SemitoneDown, cx);
    }

    pub(crate) fn cursor_prev_bar(
        &mut self,
        _: &cursor::PrevBar,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_piano_roll_cursor(CursorMove::BarBack, cx);
    }

    pub(crate) fn cursor_next_bar(
        &mut self,
        _: &cursor::NextBar,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_piano_roll_cursor(CursorMove::BarForward, cx);
    }

    pub(crate) fn cursor_octave_up(
        &mut self,
        _: &cursor::OctaveUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_piano_roll_cursor(CursorMove::OctaveUp, cx);
    }

    pub(crate) fn cursor_octave_down(
        &mut self,
        _: &cursor::OctaveDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_piano_roll_cursor(CursorMove::OctaveDown, cx);
    }

    pub(crate) fn view_zoom_in(
        &mut self,
        _: &view::ZoomIn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = cx;
        todo!()
    }

    pub(crate) fn view_zoom_out(
        &mut self,
        _: &view::ZoomOut,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = cx;
        todo!()
    }

    pub(crate) fn view_zoom_reset(
        &mut self,
        _: &view::ZoomReset,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = cx;
        todo!()
    }

    pub(crate) fn view_center_cursor(
        &mut self,
        _: &view::CenterCursor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = cx;
        todo!()
    }
}
