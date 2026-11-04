//! 트랙 목록과 트랙 Action(S2-T5b, SEQ-7, SEQ-8, 5.11 `Tab`).
//!
//! 트랙 편집은 각각 undo 한 단위다. 트랙 목록에는 탭 정지점을 두지 않는다. `Tab`은 트랙 이동이다.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle};
use gpui_kit::{
    Action, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use keyscore_core::{Channel, Edit, OutputPort, Track, TrackColor, TrackId, TrackProp};

use crate::actions::{cursor, track};
use crate::dispatch::run_action;
use crate::palette::track_color;
use crate::tracks::{lowest_free_channel, port_label, program_conflicts};
use crate::window::ProjectWindow;

/// 드럼 채널(SEQ-7: 10번 채널은 드럼 트랙으로 표시한다).
const DRUM_CHANNEL: u8 = 10;

impl ProjectWindow {
    pub(crate) fn track_index(&self, id: TrackId, cx: &gpui_kit::App) -> Option<usize> {
        self.document
            .read(cx)
            .project()
            .tracks()
            .iter()
            .position(|t| t.id() == id)
    }

    pub(crate) fn current_index(&self, cx: &gpui_kit::App) -> Option<usize> {
        self.current_track().and_then(|id| self.track_index(id, cx))
    }

    /// 현재 트랙을 그 자리의 트랙으로 바꾼다(범위 밖이면 가장 가까운 끝).
    pub(crate) fn select_track_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let id = {
            let tracks = self.document.read(cx).project().tracks();
            tracks
                .get(index.min(tracks.len().saturating_sub(1)))
                .map(Track::id)
        };
        self.set_current_track(id, cx);
    }

    /// `index` 자리에 새 트랙을 넣고 현재 트랙으로 삼는다. 채널은 기본 가상 포트의 빈 채널 가운데 가장
    /// 낮은 것이고(SEQ-7), 16채널이 다 찼으면 1을 쓰고 다른 포트를 권한다(SEQ-8).
    fn insert_new_track(&mut self, index: usize, cx: &mut Context<Self>) {
        let (count, channel) = {
            let project = self.document.read(cx).project();
            (
                project.tracks().len(),
                lowest_free_channel(project, &OutputPort::Virtual),
            )
        };
        let channel = if let Some(c) = channel {
            c
        } else {
            self.say(
                "기본 가상 포트의 16채널이 모두 쓰이고 있다. 새 트랙은 채널 1을 함께 쓴다. 다른 포트를 고르기를 권한다",
                cx,
            );
            Channel::new(1).unwrap_or(Channel::DRUMS)
        };
        // 팔레트 12색을 차례로 쓴다(0–11은 늘 유효하다)
        let Ok(color) = TrackColor::new(u8::try_from(count % 12).unwrap_or(0)) else {
            return;
        };
        let track = Track::new(format!("트랙 {}", count + 1), channel, color);
        let id = track.id();
        if self.edit_document(
            vec![Edit::InsertTrack {
                index: index.min(count),
                track: Box::new(track),
            }],
            cx,
        ) {
            self.set_current_track(Some(id), cx);
        }
    }

    pub(crate) fn track_new(&mut self, _: &track::New, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.document.read(cx).project().tracks().len();
        self.insert_new_track(count, cx);
    }

    pub(crate) fn track_new_above(
        &mut self,
        _: &track::NewAbove,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = self.current_index(cx).unwrap_or(0);
        self.insert_new_track(at, cx);
    }

    pub(crate) fn track_new_below(
        &mut self,
        _: &track::NewBelow,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = self.current_index(cx).map_or_else(
            || self.document.read(cx).project().tracks().len(),
            |i| i + 1,
        );
        self.insert_new_track(at, cx);
    }

    pub(crate) fn track_delete(
        &mut self,
        _: &track::Delete,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.current_index(cx) else {
            return;
        };
        let Some(id) = self.current_track() else {
            return;
        };
        if self.edit_document(vec![Edit::RemoveTrack { track: id }], cx) {
            // 지운 자리의 트랙(없으면 앞 트랙)이 현재 트랙이 된다
            self.select_track_at(index, cx);
        }
    }

    fn toggle_prop(&mut self, prop: impl Fn(&Track) -> TrackProp, cx: &mut Context<Self>) {
        let Some(id) = self.current_track() else {
            return;
        };
        let prop = {
            let project = self.document.read(cx).project();
            project.tracks().iter().find(|t| t.id() == id).map(prop)
        };
        if let Some(prop) = prop {
            self.edit_document(vec![Edit::SetTrackProp { track: id, prop }], cx);
        }
    }

    pub(crate) fn track_toggle_mute(
        &mut self,
        _: &track::ToggleMute,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_prop(|t| TrackProp::Mute(!t.mute()), cx);
    }

    pub(crate) fn track_toggle_solo(
        &mut self,
        _: &track::ToggleSolo,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_prop(|t| TrackProp::Solo(!t.solo()), cx);
    }

    /// 트랙 순서 바꾸기. 모델에 순서 편집이 없어 지우기와 넣기를 한 undo 단위로 묶는다.
    fn move_current(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some(index) = self.current_index(cx) else {
            return;
        };
        let (count, track) = {
            let tracks = self.document.read(cx).project().tracks();
            (tracks.len(), tracks[index].clone())
        };
        let to = if up {
            index.checked_sub(1)
        } else {
            (index + 1 < count).then_some(index + 1)
        };
        let Some(to) = to else { return };
        self.edit_document(
            vec![
                Edit::RemoveTrack { track: track.id() },
                Edit::InsertTrack {
                    index: to,
                    track: Box::new(track),
                },
            ],
            cx,
        );
    }

    pub(crate) fn track_move_up(
        &mut self,
        _: &track::MoveUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_current(true, cx);
    }

    pub(crate) fn track_move_down(
        &mut self,
        _: &track::MoveDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_current(false, cx);
    }

    pub(crate) fn track_set_channel(
        &mut self,
        action: &track::SetChannel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.current_track() else {
            return;
        };
        match u8::try_from(action.0.0)
            .ok()
            .and_then(|c| Channel::new(c).ok())
        {
            Some(channel) => {
                self.edit_document(
                    vec![Edit::SetTrackProp {
                        track: id,
                        prop: TrackProp::Channel(channel),
                    }],
                    cx,
                );
            }
            None => self.say(format!("채널은 1–16이다({})", action.0.0), cx),
        }
    }

    /// `track::Next`/`Prev`(5.11 `Tab`/`Shift+Tab`). 끝에서는 멈춘다.
    pub(crate) fn track_next(&mut self, _: &track::Next, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self.current_index(cx) {
            self.select_track_at(i + 1, cx);
        }
    }

    pub(crate) fn track_prev(&mut self, _: &track::Prev, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self.current_index(cx) {
            self.select_track_at(i.saturating_sub(1), cx);
        }
    }

    pub(crate) fn track_first(
        &mut self,
        _: &cursor::FirstTrack,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_track_at(0, cx);
    }

    pub(crate) fn track_last(
        &mut self,
        _: &cursor::LastTrack,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_track_at(usize::MAX, cx);
    }

    /// 트랙 목록(7.1 왼쪽): 줄마다 색·이름·「포트 · ch N」(드럼 표시)·SEQ-8 경고·M/S. 아래에 `+`.
    // 줄 하나의 모양을 한곳에서 읽히게 나누지 않는다.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn render_track_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        struct Row {
            id: TrackId,
            name: String,
            routing: String,
            color: TrackColor,
            mute: bool,
            solo: bool,
            conflict: bool,
        }
        let virtual_name = crate::engine::virtual_port_name(cx);
        let current = self.current_track();
        let rows: Vec<Row> = {
            let project = self.document.read(cx).project();
            let conflicts = program_conflicts(project);
            project
                .tracks()
                .iter()
                .map(|t| {
                    let drum = t.channel().get() == DRUM_CHANNEL;
                    Row {
                        id: t.id(),
                        name: t.name().to_owned(),
                        routing: format!(
                            "{} · ch {}{}",
                            port_label(t.port(), &virtual_name),
                            t.channel().get(),
                            if drum { " · 드럼" } else { "" }
                        ),
                        color: t.color(),
                        mute: t.mute(),
                        solo: t.solo(),
                        conflict: conflicts.contains(&t.id()),
                    }
                })
                .collect()
        };
        let me = cx.entity().downgrade();
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().accent;
        let warning = cx.theme().warning;
        let elements: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                let target = row.id;
                let button = |kind: &'static str, text: &'static str, on: bool, action: Box<dyn Action>| {
                    let me = me.clone();
                    Toggle::new(format!("track-{kind}-{i}"))
                        .label(text)
                        .checked(on)
                        .on_click(move |_, window, cx| {
                            // 버튼은 그 줄의 트랙을 현재 트랙으로 삼고 Action을 실행 함수로 낸다(결정 C)
                            let _ = me.update(cx, |this, cx| this.set_current_track(Some(target), cx));
                            run_action(action.boxed_clone(), window, cx);
                        })
                };
                let select = me.clone();
                div()
                    .id(format!("track-row-{i}"))
                    .aria_label(format!("{} · {}", row.name, row.routing))
                    .test_support()
                    .w_full()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(current == Some(target), |d| d.bg(accent))
                    .on_click(move |_: &ClickEvent, _, cx| {
                        let _ = select.update(cx, |this, cx| this.set_current_track(Some(target), cx));
                    })
                    .child(div().w(px(4.)).h(px(20.)).bg(track_color(row.color, cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(row.name)
                            .child(div().text_xs().text_color(muted).child(row.routing)),
                    )
                    .when(row.conflict, |d| {
                        d.child(
                            div()
                                .id(format!("track-warning-{i}"))
                                .aria_label(
                                    "같은 포트·채널을 쓰는 트랙과 프로그램이 다르다(SEQ-8). 한 신디에서 음색이 서로 덮인다",
                                )
                                .test_support()
                                .text_color(warning)
                                .child("⚠"),
                        )
                    })
                    .child(button("mute", "M", row.mute, Box::new(track::ToggleMute)))
                    .child(button("solo", "S", row.solo, Box::new(track::ToggleSolo)))
            })
            .collect();
        // 목록 안을 왼쪽 버튼으로 누르면(줄·버튼 포함) 늘 문서로 포커스를 돌린다. 목록에는 입력란이 없다.
        // gpui-component 버튼은 왼쪽 누름에서 포커스를 받지 않고 `prevent_default`만 하므로, 창의 공통
        // 처리기(`focus_document_on_press`)는 고치던 입력란을 그대로 둔다. 이 처리기가 그 입력을 끝낸다.
        div()
            .id("track-list")
            .test_support()
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus_document(window, cx)),
            )
            .size_full()
            .flex()
            .flex_col()
            .children(elements)
            .child(Button::new("track-add").label("+").ghost().on_click(
                |_: &ClickEvent, window, cx| {
                    run_action(Box::new(track::New), window, cx);
                },
            ))
    }
}
