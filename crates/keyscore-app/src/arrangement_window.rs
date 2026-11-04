//! 배치 뷰를 창에 붙인 부분(S4-T1): Action 처리기, 그리기, 클릭·우클릭, 뷰 진입.
//!
//! 커서는 창에 하나다(틱은 `pianoroll`, 트랙은 `current_track`). 배치 뷰 상태(`arrangement`)는 뷰포트와 클립
//! 선택만 갖는다.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::{
    App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point,
    Stateful, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window, div, px,
};
use keyscore_core::{Tick, TrackColor};

use crate::actions::cursor;
use crate::arrangement::element::{ArrangementElement, Palette, Press};
use crate::arrangement::{Cell, clip_boundary_after, clip_boundary_before};
use crate::dispatch::run_action;
use crate::engine::TransportState;
use crate::mode::View;
use crate::pianoroll::Cursor;
use crate::selection::MARK_NAMES;
use crate::window::{PopupKind, ProjectWindow};

impl ProjectWindow {
    pub(crate) fn arrangement_handlers(
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element
            .on_action(cx.listener(Self::cursor_prev_clip_boundary))
            .on_action(cx.listener(Self::cursor_next_clip_boundary))
            .on_action(cx.listener(Self::cursor_track_up))
            .on_action(cx.listener(Self::cursor_track_down))
    }

    /// Normal 배치 뷰인가(줌·가운데 처리기가 뷰를 가른다).
    pub(crate) fn in_arrangement(&self) -> bool {
        self.mode_state().view() == View::Arrangement
    }

    /// 배치 뷰 커서 칸: 창의 커서 틱, Step 길이, 현재 트랙 행.
    pub(crate) fn arrangement_cell(&self, cx: &App) -> Cell {
        Cell {
            tick: self.pianoroll.cursor().tick,
            step_ticks: self.pianoroll.step_ticks(),
            row: self.current_index(cx),
        }
    }

    /// 배치 뷰로 들어올 때: 커서가 보이면 스크롤 자리를 지키고, 안 보이면 가운데로.
    pub(crate) fn entered_arrangement(&mut self, cx: &mut Context<Self>) {
        let cell = self.arrangement_cell(cx);
        self.arrangement.reveal_on_entry(cell);
    }

    /// 배치 뷰에서 커서(틱·트랙)가 바뀐 뒤: 선택 상태를 맞추고 뷰포트가 커서 칸을 따라간다
    /// (`cursor_moved_quietly`가 배치 뷰 따라가기까지 한다). 미리듣기는 피아노롤 노트의 것이라 하지 않는다.
    fn arrangement_cursor_moved(&mut self, cx: &mut Context<Self>) {
        self.cursor_moved_quietly(false, cx);
    }

    /// 현재 트랙의 클립 경계로 커서 틱을 옮긴다. 경계가 없으면 그대로.
    fn jump_to_boundary(&mut self, backward: bool, cx: &mut Context<Self>) {
        let cursor = self.pianoroll.cursor();
        let target = {
            let project = self.document.read(cx).project();
            let track = self.current_track().and_then(|id| project.track(id));
            track.and_then(|t| {
                if backward {
                    clip_boundary_before(t, cursor.tick)
                } else {
                    clip_boundary_after(t, cursor.tick)
                }
            })
        };
        let Some(tick) = target else {
            return;
        };
        self.pianoroll.set_cursor(Cursor { tick, ..cursor });
        self.arrangement_cursor_moved(cx);
    }

    pub(crate) fn cursor_prev_clip_boundary(
        &mut self,
        _: &cursor::PrevClipBoundary,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_boundary(true, cx);
    }

    pub(crate) fn cursor_next_clip_boundary(
        &mut self,
        _: &cursor::NextClipBoundary,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_boundary(false, cx);
    }

    /// 이웃 트랙으로. 틱은 그대로다(REQ 5.11 「시간 위치는 유지」).
    fn move_track(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(index) = self.current_index(cx) else {
            return;
        };
        let target = index.saturating_add_signed(delta);
        self.select_track_at(target, cx);
        self.arrangement_cursor_moved(cx);
    }

    pub(crate) fn cursor_track_up(
        &mut self,
        _: &cursor::TrackUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_track(-1, cx);
    }

    pub(crate) fn cursor_track_down(
        &mut self,
        _: &cursor::TrackDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_track(1, cx);
    }

    /// 가운데 주 뷰의 배치 뷰.
    pub(crate) fn render_arrangement(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let track_colors: [gpui_kit::Hsla; 12] = std::array::from_fn(|i| {
            // 0–11은 늘 유효한 트랙 색이다.
            TrackColor::new(u8::try_from(i).unwrap_or(0))
                .map_or(theme.primary, |c| crate::palette::track_color(c, cx))
        });
        let palette = Palette {
            row_light: theme.background,
            row_dark: theme.muted.opacity(0.5),
            row_current: theme.primary.opacity(0.08),
            bar_line: theme.border,
            beat_line: theme.border.opacity(0.45),
            clip_border: theme.foreground.opacity(0.6),
            label: theme.foreground,
            lyric: theme.muted_foreground.opacity(0.8),
            selected: theme.selection,
            cursor: theme.primary,
            ghost: theme.selection,
            ghost_blocked: theme.danger,
            playhead: theme.danger,
            track_colors,
        };
        let animating = self.transport.state == TransportState::Playing;
        let playhead = self.piano_roll_playhead();
        let ghost = self.step_ghost(cx);
        let shown = self.arrangement_step(cx);
        let cursor = self.pianoroll.cursor().tick;
        let element = ArrangementElement::new(
            self.arrangement.clone(),
            self.document.clone(),
            self.current_track(),
            cursor,
            self.pianoroll.step_ticks(),
            palette,
        )
        .playhead(playhead, animating)
        .ghost(ghost)
        .on_press(cx.listener(|this, press: &Press, _, cx| {
            this.arrangement_press(press, cx);
        }))
        .on_view_change({
            let listener = cx.listener(|_, (): &(), _, cx| cx.notify());
            move |window, cx| listener(&(), window, cx)
        })
        .on_right_click({
            let listener = cx.listener(
                |this, (cell, at): &((usize, Tick), Point<Pixels>), window, cx| {
                    this.open_arrangement_menu(*cell, *at, window, cx);
                },
            );
            move |cell, at, window, cx| listener(&(cell, at), window, cx)
        });
        // 같은 id·라벨(창 시험이 「main-view」로 찾는다). 4 px 여백은 패널 분할 손잡이 히트박스 자리다(S3-T1 리뷰 1
        // 제안 4). 피아노롤과 달리 `main-view`가 여백 바깥(패널 크기)이고 Element는 그 안쪽이다: 패널 최소 너비를 재는
        // 창 시험(S2-T6f 리뷰 3)이 배치 뷰의 `main-view`를 패널로 본다. 배치 뷰 좌표 시험은 여백만큼 더한다.
        let mut main = div()
            .id("main-view")
            .aria_label("배치 뷰")
            .test_support()
            .relative()
            .size_full()
            .p(px(4.))
            .overflow_hidden()
            .child(element);
        // Step 위치 글 표시(S3-T4c)는 격자 위의 커서·고스트로 바뀌었다. 접근성 이름으로만 남긴다(크기 0).
        if let Some(shown) = shown {
            main = main
                .child(
                    div()
                        .id("arrangement-step-cursor")
                        .aria_label(shown.cursor)
                        .test_support()
                        .absolute()
                        .size_0(),
                )
                .child(
                    div()
                        .id("arrangement-step-clip")
                        .aria_label(shown.clip)
                        .test_support()
                        .absolute()
                        .size_0(),
                );
        }
        main
    }

    /// 배치 뷰 칸을 눌렀을 때: 그 행의 트랙을 현재 트랙으로, 커서 틱을 그 칸으로.
    pub(crate) fn arrangement_press(&mut self, press: &Press, cx: &mut Context<Self>) {
        let Some((row, tick)) = press.cell else {
            return;
        };
        // 클릭은 키 입력이 아니므로 되돌리기 묶음을 여기서 끊는다(5.7 「다른 키가 끼면」)
        self.seal_history(cx);
        self.clear_operator(cx);
        self.select_track_at(row, cx);
        let cursor = self.pianoroll.cursor();
        self.pianoroll.set_cursor(Cursor { tick, ..cursor });
        self.arrangement_cursor_moved(cx);
    }

    /// 배치 뷰 우클릭 메뉴(UI-11, 결정 `2026-10-17-vim-marks-visible-ui` a): 마크 설정·마크로 이동. 우클릭한 칸으로
    /// 트랙·커서를 옮긴다(피아노롤 `open_piano_roll_menu`와 같은 흐름).
    pub(crate) fn open_arrangement_menu(
        &mut self,
        (row, tick): (usize, Tick),
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.seal_history(cx);
        self.clear_operator(cx);
        self.select_track_at(row, cx);
        let cursor = self.pianoroll.cursor();
        self.pianoroll.set_cursor(Cursor { tick, ..cursor });
        self.arrangement_cursor_moved(cx);
        self.open_popup_with(
            PopupKind::ArrangementMenu,
            at,
            window,
            cx,
            |menu, window, cx| {
                menu.submenu("마크 설정", window, cx, |menu, _, _| {
                    MARK_NAMES.chars().fold(menu, |menu, c| {
                        let name = c.to_string();
                        menu.item(PopupMenuItem::new(name.clone()).on_click(
                            move |_, window, cx| {
                                run_action(Box::new(cursor::SetMark(name.clone())), window, cx);
                            },
                        ))
                    })
                })
                .submenu("마크로 이동", window, cx, |menu, _, _| {
                    MARK_NAMES.chars().fold(menu, |menu, c| {
                        let name = c.to_string();
                        menu.item(PopupMenuItem::new(name.clone()).on_click(
                            move |_, window, cx| {
                                run_action(Box::new(cursor::JumpMark(name.clone())), window, cx);
                            },
                        ))
                    })
                })
            },
        );
    }
}
