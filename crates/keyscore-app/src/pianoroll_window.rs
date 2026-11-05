//! 피아노롤을 창에 붙인 부분(S3-T1): Action 처리기, 그리기, 재생 헤드.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    Context, Div, InteractiveElement as _, IntoElement, ParentElement as _, Stateful,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window,
};

use crate::actions::{cursor, view};
use crate::engine::TransportState;
use crate::pianoroll::element::{Palette, PianoRollElement, Press};
use crate::pianoroll::vim::Motion;
use crate::pianoroll::{Cursor, ZOOM_FACTOR};
use crate::window::ProjectWindow;

impl ProjectWindow {
    pub(crate) fn piano_roll_handlers(
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let element = element
            .on_action(cx.listener(Self::cursor_left))
            .on_action(cx.listener(Self::cursor_right))
            .on_action(cx.listener(Self::cursor_up))
            .on_action(cx.listener(Self::cursor_down))
            .on_action(cx.listener(Self::cursor_prev_bar))
            .on_action(cx.listener(Self::cursor_next_bar))
            .on_action(cx.listener(Self::cursor_octave_up))
            .on_action(cx.listener(Self::cursor_octave_down))
            .on_action(cx.listener(Self::view_zoom_in))
            .on_action(cx.listener(Self::view_zoom_out))
            .on_action(cx.listener(Self::view_zoom_reset))
            .on_action(cx.listener(Self::view_center_cursor));
        let element = Self::selection_handlers(element, cx);
        let element = Self::vim_handlers(element, cx);
        let element = Self::note_edit_handlers(element, cx);
        Self::note_ops_handlers(element, cx)
    }

    /// 피아노롤에 그리는 재생 헤드 틱. 재생 중이면 지금 자리, 일시정지면 그 자리, 정지면 없음.
    #[must_use]
    pub fn piano_roll_playhead(&self) -> Option<keyscore_core::Tick> {
        match self.transport.state {
            TransportState::Playing => Some(self.transport.position()),
            TransportState::Paused(at) => Some(at),
            TransportState::Stopped => None,
        }
    }

    /// 가운데 주 뷰의 피아노롤.
    pub(crate) fn render_piano_roll(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let track_color = self
            .current_track()
            .and_then(|id| {
                self.document
                    .read(cx)
                    .project()
                    .track(id)
                    .map(keyscore_core::Track::color)
            })
            .map_or(theme.primary, |c| crate::palette::track_color(c, cx));
        let palette = Palette {
            row_light: theme.background,
            row_dark: theme.muted,
            bar_line: theme.border,
            beat_line: theme.border.opacity(0.45),
            note: track_color,
            note_border: theme.foreground.opacity(0.6),
            note_dim: track_color.opacity(0.25),
            selected: theme.selection,
            cursor: theme.primary,
            playhead: theme.danger,
        };
        let animating = self.transport.state == TransportState::Playing;
        let playhead = self.piano_roll_playhead();
        let element = PianoRollElement::new(
            self.pianoroll.clone(),
            self.document.clone(),
            self.current_track(),
            palette,
        )
        .playhead(playhead, animating)
        .on_press(cx.listener(|this, press: &Press, window, cx| {
            this.piano_roll_press(press, window, cx);
        }))
        .on_drag({
            let listener = cx.listener(
                |this, (at, modifiers): &((f32, f32), gpui_kit::Modifiers), _, cx| {
                    this.piano_roll_drag(*at, *modifiers, cx);
                },
            );
            move |at, modifiers, window, cx| listener(&(at, modifiers), window, cx)
        })
        .on_view_change({
            let listener = cx.listener(|this, (): &(), _, cx| {
                this.piano_roll_view_changed(cx);
            });
            move |window, cx| listener(&(), window, cx)
        })
        .on_release({
            let listener = cx.listener(
                |this, (at, modifiers): &((f32, f32), gpui_kit::Modifiers), _, cx| {
                    this.piano_roll_release(*at, *modifiers, cx);
                },
            );
            move |at, modifiers, window, cx| listener(&(at, modifiers), window, cx)
        })
        .on_right_click({
            let listener = cx.listener(
                |this, (cell, at): &(Cursor, gpui_kit::Point<gpui_kit::Pixels>), window, cx| {
                    this.open_piano_roll_menu(*cell, *at, window, cx);
                },
            );
            move |cell, at, window, cx| listener(&(cell, at), window, cx)
        });
        // 자리표시자와 같은 id·라벨을 쓴다(창 시험이 「main-view」로 찾는다). 둘레 4 px 여백은 패널 분할 손잡이
        // 히트박스가 놓일 자리다. 여백 없이는 손잡이가 맨 위 행·왼쪽 끝을 가려 누를 수 없었다(S3-T1 리뷰 1 제안 4).
        // `main-view`는 여백 안쪽이라 Element와 같은 크기다(창 시험이 이 경계로 좌표를 잡는다).
        gpui_kit::div().size_full().p(gpui_kit::px(4.)).child(
            gpui_kit::div()
                .id("main-view")
                .aria_label("피아노롤")
                .test_support()
                .size_full()
                .overflow_hidden()
                .child(element),
        )
    }

    /// 피아노롤 칸을 눌렀을 때(포커스 옮기기 전까지): 커서를 그 칸으로. 비주얼 중이면 선택을 다시 계산하고,
    /// 오퍼레이터 대기는 끝낸다(S3-T3c).
    pub fn click_piano_roll_cell(&mut self, cell: Cursor, cx: &mut Context<Self>) {
        // 클릭은 키 입력이 아니므로 되돌리기 묶음을 여기서 끊는다(5.7 「다른 키가 끼면」)
        self.seal_history(cx);
        self.clear_operator(cx);
        self.pianoroll.set_cursor(cell);
        self.cursor_moved(false, cx);
    }

    /// 피아노롤로 들어갈 때 커서를 클립 안에 둔다.
    pub(crate) fn entered_piano_roll(&mut self, cx: &mut Context<Self>) {
        let doc = self.document.read(cx);
        let track = self.current_track().and_then(|id| doc.project().track(id));
        // 옮기지 않았어도 커서가 뷰포트 밖일 수 있다(배치 뷰에 있는 동안 바뀐 경우). 보이면 스크롤 자리를
        // 지키고, 안 보이면 가운데로. 크기를 아직 모르면 첫 그리기에서 적용된다.
        let _ = self.pianoroll.place_cursor_in_clip(track);
        self.pianoroll.reveal_cursor_on_entry();
    }

    pub(crate) fn cursor_left(
        &mut self,
        _: &cursor::Left,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::Left, window, cx);
    }

    pub(crate) fn cursor_right(
        &mut self,
        _: &cursor::Right,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::Right, window, cx);
    }

    pub(crate) fn cursor_up(
        &mut self,
        _: &cursor::Up,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::Up, window, cx);
    }

    pub(crate) fn cursor_down(
        &mut self,
        _: &cursor::Down,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::Down, window, cx);
    }

    pub(crate) fn cursor_prev_bar(
        &mut self,
        _: &cursor::PrevBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::PrevBar, window, cx);
    }

    pub(crate) fn cursor_next_bar(
        &mut self,
        _: &cursor::NextBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::NextBar, window, cx);
    }

    pub(crate) fn cursor_octave_up(
        &mut self,
        _: &cursor::OctaveUp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::OctaveUp, window, cx);
    }

    pub(crate) fn cursor_octave_down(
        &mut self,
        _: &cursor::OctaveDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::OctaveDown, window, cx);
    }

    // 줌·가운데 처리기는 창에 걸려 있어 현재 뷰에 따라 나눈다(S3-T1 리뷰 1 제안 7, S4-T1 이월): 배치 뷰에서는
    // 배치 뷰의 배율만 바뀌고 숨은 피아노롤은 그대로다.

    pub(crate) fn view_zoom_in(
        &mut self,
        _: &view::ZoomIn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.in_arrangement() {
            self.arrangement
                .zoom(ZOOM_FACTOR, self.arrangement_cell(cx));
            cx.notify();
            return;
        }
        self.pianoroll.zoom(ZOOM_FACTOR);
        self.piano_roll_view_changed(cx);
        cx.notify();
    }

    pub(crate) fn view_zoom_out(
        &mut self,
        _: &view::ZoomOut,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.in_arrangement() {
            self.arrangement
                .zoom(1.0 / ZOOM_FACTOR, self.arrangement_cell(cx));
            cx.notify();
            return;
        }
        self.pianoroll.zoom(1.0 / ZOOM_FACTOR);
        self.piano_roll_view_changed(cx);
        cx.notify();
    }

    pub(crate) fn view_zoom_reset(
        &mut self,
        _: &view::ZoomReset,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.in_arrangement() {
            self.arrangement.zoom_reset(self.arrangement_cell(cx));
            cx.notify();
            return;
        }
        self.pianoroll.zoom_reset();
        self.piano_roll_view_changed(cx);
        cx.notify();
    }

    pub(crate) fn view_center_cursor(
        &mut self,
        _: &view::CenterCursor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.in_arrangement() {
            self.arrangement.center(self.arrangement_cell(cx));
            cx.notify();
            return;
        }
        self.pianoroll.center_cursor();
        self.piano_roll_view_changed(cx);
        cx.notify();
    }
}
