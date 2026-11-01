//! 퀀타이즈 설정 시트(S3-T3e, REQ 5.8 `Shift+q`, EDT-2, 7.6 「설정 시트(그리드·강도·스윙)」).
//!
//! 시트는 프로젝트 창 안의 패널이다(REQ 7절: OS 네이티브 외의 모달 다이얼로그는 두지 않는다). 팔레트·`?`
//! 오버레이처럼 모드 키 컨텍스트 겹 바깥에 그리고 포커스를 가진다. 그래서 열린 동안 문서 키는 동작하지 않는다.
//! 컨텍스트는 `Dialog > QuantizeSheet`라 `Dialog`의 `Enter`·`Esc`·`Tab` 바인딩을 그대로 쓴다.
//!
//! 바꾸는 것은 초안이다. 확인해야 앱 설정 `quantize`에 들어간다(결정 `2026-10-26-s3-t3b-note-edit-details.md` 3-A).
//! 규칙은 `docs/reports/s3-t3e/plan.md`.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::{
    ClickEvent, Context, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    TestSupportExt as _, Window, deferred, div, prelude::FluentBuilder as _, px,
};

use crate::actions::{dialog, edit};
use crate::app_state::AppSettings;
use crate::settings::{GRIDS, QuantizeSettings};
use crate::window::ProjectWindow;

/// 강도·스윙의 「크게」 한 칸.
const LARGE_STEP: i16 = 10;

/// 시트의 줄(위에서부터).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetRow {
    Grid,
    Triplet,
    Strength,
    Swing,
}

impl SheetRow {
    const ALL: [SheetRow; 4] = [
        SheetRow::Grid,
        SheetRow::Triplet,
        SheetRow::Strength,
        SheetRow::Swing,
    ];

    fn index(self) -> usize {
        match self {
            SheetRow::Grid => 0,
            SheetRow::Triplet => 1,
            SheetRow::Strength => 2,
            SheetRow::Swing => 3,
        }
    }

    /// `delta`줄 옮긴다. 양 끝에서 돈다.
    fn moved(self, delta: isize) -> SheetRow {
        let n = Self::ALL.len().cast_signed();
        Self::ALL[(self.index().cast_signed() + delta)
            .rem_euclid(n)
            .cast_unsigned()]
    }
}

/// 열린 시트.
pub(crate) struct QuantizeSheet {
    focus: FocusHandle,
    row: SheetRow,
    draft: QuantizeSettings,
    /// 포커스가 시트 밖으로 나가면 취소로 닫는다(리뷰 1 제안 1). 루트 처리기가 포커스를 옮기는 Action(트랜스포트
    /// 필드)을 받으면 시트가 포커스 없이 남아 문서 키가 시트 뒤에서 동작했다.
    _focus_out: Subscription,
}

/// 값 한 칸의 크기.
#[derive(Clone, Copy)]
enum Step {
    One,
    Large,
}

/// `row`의 값을 한 칸(`up`이면 늘리고 아니면 줄인다) 옮긴 초안. 모든 값은 끝에서 멈춘다.
fn stepped(mut q: QuantizeSettings, row: SheetRow, up: bool, step: Step) -> QuantizeSettings {
    let by = |value: u8, max: u8| {
        let size = match step {
            Step::One => 1,
            Step::Large => LARGE_STEP,
        };
        let next = i16::from(value) + if up { size } else { -size };
        u8::try_from(next.clamp(0, i16::from(max))).expect("0..=max 안")
    };
    match row {
        // 그리드·셋잇단은 「크게」도 한 칸이다
        SheetRow::Grid => {
            let i = GRIDS.iter().position(|(g, _)| *g == q.grid).unwrap_or(2);
            let j = if up {
                (i + 1).min(GRIDS.len() - 1)
            } else {
                i.saturating_sub(1)
            };
            q.grid = GRIDS[j].0;
        }
        SheetRow::Triplet => q.triplet = up,
        SheetRow::Strength => q.strength = by(q.strength.min(100), 100),
        SheetRow::Swing => q.swing = by(q.swing.min(75), 75),
    }
    q
}

fn grid_name(q: QuantizeSettings) -> &'static str {
    GRIDS
        .iter()
        .find(|(g, _)| *g == q.grid)
        .map_or("1/16", |(_, name)| name)
}

fn on_off(on: bool) -> &'static str {
    if on { "켬" } else { "끔" }
}

/// 줄의 (이름, 값).
fn row_text(q: QuantizeSettings, row: SheetRow) -> (&'static str, String) {
    match row {
        SheetRow::Grid => ("그리드", grid_name(q).to_owned()),
        SheetRow::Triplet => ("셋잇단", on_off(q.triplet).to_owned()),
        SheetRow::Strength => ("강도", format!("{}%", q.strength)),
        SheetRow::Swing => ("스윙", format!("{}%", q.swing)),
    }
}

/// 상태줄의 요약(확인 뒤).
fn summary(q: QuantizeSettings) -> String {
    format!(
        "퀀타이즈 설정: {}, 셋잇단 {}, 강도 {}%, 스윙 {}%",
        grid_name(q),
        on_off(q.triplet),
        q.strength,
        q.swing
    )
}

impl ProjectWindow {
    /// 열린 시트의 (포커스 줄, 초안). 닫혀 있으면 `None`.
    #[must_use]
    pub fn quantize_sheet(&self) -> Option<(SheetRow, QuantizeSettings)> {
        self.quantize_sheet.as_ref().map(|s| (s.row, s.draft))
    }

    /// 문서 포커스 핸들(테스트용).
    #[must_use]
    pub fn document_focus_for_test(&self) -> FocusHandle {
        self.document_focus()
    }

    /// `edit::QuantizeSettings`: 시트를 연다. 이미 열려 있으면 초안을 두고 포커스만 돌린다.
    pub(crate) fn open_quantize_sheet(
        &mut self,
        _: &edit::QuantizeSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 팝업은 하나만 띄운다
        self.palette = None;
        self.overlay = None;
        self.popup = None;
        self.search_bar = None;
        self.note_info = None;
        if self.quantize_sheet.is_none() {
            let focus = cx.focus_handle();
            // 창이 비활성이 되어 잃는 포커스는 취소가 아니다(입력란과 같은 macOS 관례, `fields.rs`)
            let focus_out = cx.on_focus_out(&focus, window, |this, _, window, cx| {
                if window.is_window_active() {
                    this.close_quantize_sheet(false, window, cx);
                }
            });
            self.quantize_sheet = Some(QuantizeSheet {
                focus,
                row: SheetRow::Grid,
                draft: AppSettings::get(cx).quantize,
                _focus_out: focus_out,
            });
        }
        let Some(sheet) = &self.quantize_sheet else {
            return;
        };
        window.focus(&sheet.focus, cx);
        cx.notify();
    }

    /// 시트를 닫는다. `save`면 초안을 앱 설정에 넣는다. 열려 있지 않으면 아무것도 하지 않는다.
    pub(crate) fn close_quantize_sheet(
        &mut self,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.quantize_sheet.take() else {
            return;
        };
        if save {
            AppSettings::update(cx, |s| s.quantize = sheet.draft);
            self.say(summary(sheet.draft), cx);
        }
        // 포커스가 아직 시트에 있을 때만 문서로 돌린다. 다른 팝업을 열며 닫혔으면 포커스는 그쪽 것이다
        if sheet.focus.contains_focused(window, cx) || window.focused(cx).is_none() {
            self.focus_document(window, cx);
        }
        cx.notify();
    }

    fn sheet_move_row(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(sheet) = &mut self.quantize_sheet {
            sheet.row = sheet.row.moved(delta);
            window.focus(&sheet.focus, cx);
            cx.notify();
        }
    }

    fn sheet_step(
        &mut self,
        row: Option<SheetRow>,
        up: bool,
        step: Step,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(sheet) = &mut self.quantize_sheet {
            if let Some(row) = row {
                sheet.row = row;
            }
            sheet.draft = stepped(sheet.draft, sheet.row, up, step);
            window.focus(&sheet.focus, cx);
            cx.notify();
        }
    }

    fn sheet_select_row(&mut self, row: SheetRow, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(sheet) = &mut self.quantize_sheet {
            sheet.row = row;
            window.focus(&sheet.focus, cx);
            cx.notify();
        }
    }

    /// 시트의 한 줄: 이름, ‹, 값, ›.
    fn sheet_row(
        sheet: &QuantizeSheet,
        row: SheetRow,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let i = row.index();
        let (name, value) = row_text(sheet.draft, row);
        let label = format!("{name}: {value}");
        let focused = row == sheet.row;
        div()
            .id(SharedString::from(format!("quantize-row-{i}")))
            .aria_label(label)
            .test_support()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .when(focused, |d| d.bg(cx.theme().accent))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| this.sheet_select_row(row, window, cx)),
            )
            .child(div().w(px(80.)).child(name))
            .child(
                Button::new(SharedString::from(format!("quantize-dec-{i}")))
                    .label("‹")
                    .ghost()
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.sheet_step(Some(row), false, Step::One, window, cx);
                    })),
            )
            .child(div().w(px(64.)).flex().justify_center().child(value))
            .child(
                Button::new(SharedString::from(format!("quantize-inc-{i}")))
                    .label("›")
                    .ghost()
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.sheet_step(Some(row), true, Step::One, window, cx);
                    })),
            )
    }

    /// 시트 안의 키 처리기(계획 「3. 키」).
    fn sheet_handlers<E: gpui_kit::InteractiveElement>(element: E, cx: &mut Context<Self>) -> E {
        element
            .on_action(cx.listener(|this, _: &dialog::Confirm, window, cx| {
                this.close_quantize_sheet(true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &dialog::Cancel, window, cx| {
                this.close_quantize_sheet(false, window, cx);
            }))
            .on_action(
                cx.listener(|this, _: &crate::actions::window::FocusNext, window, cx| {
                    this.sheet_move_row(1, window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::actions::window::FocusPrev, window, cx| {
                    this.sheet_move_row(-1, window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &dialog::Increase, window, cx| {
                this.sheet_step(None, true, Step::One, window, cx);
            }))
            .on_action(cx.listener(|this, _: &dialog::Decrease, window, cx| {
                this.sheet_step(None, false, Step::One, window, cx);
            }))
            .on_action(cx.listener(|this, _: &dialog::IncreaseLarge, window, cx| {
                this.sheet_step(None, true, Step::Large, window, cx);
            }))
            .on_action(cx.listener(|this, _: &dialog::DecreaseLarge, window, cx| {
                this.sheet_step(None, false, Step::Large, window, cx);
            }))
    }

    /// 열린 시트. 모드 키 컨텍스트 겹 바깥(창 루트의 자식)에 그린다.
    pub(crate) fn render_quantize_sheet(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let sheet = self.quantize_sheet.as_ref()?;
        let rows = SheetRow::ALL.map(|row| Self::sheet_row(sheet, row, cx));
        let focus = sheet.focus.clone();
        Some(deferred(
            div()
                .id("quantize-sheet-backdrop")
                // 바깥 클릭은 취소만 하고 아래 요소에 닿지 않는다(키맵 오버레이와 같다)
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(cx.theme().overlay)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_quantize_sheet(false, window, cx)),
                )
                .child(
                    div()
                        .id("quantize-sheet-dialog")
                        .key_context("Dialog")
                        // 안쪽 클릭은 닫지 않는다
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Self::sheet_handlers(
                                div()
                                    .id("quantize-sheet")
                                    .aria_label("퀀타이즈 설정")
                                    .test_support()
                                    .key_context("QuantizeSheet")
                                    .track_focus(&focus),
                                cx,
                            )
                            .w(px(320.))
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .text_sm()
                            .bg(cx.theme().popover)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(div().pb_1().child("퀀타이즈 설정"))
                            .children(rows)
                            .child(
                                div()
                                    .pt_1()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("↑↓ 줄 · ←→ 값(Shift 10씩) · Enter 확인 · Esc 취소"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .pt_1()
                                    .child(Button::new("quantize-confirm").label("확인").on_click(
                                        cx.listener(|this, _: &ClickEvent, window, cx| {
                                            this.close_quantize_sheet(true, window, cx);
                                        }),
                                    ))
                                    .child(
                                        Button::new("quantize-cancel")
                                            .label("취소")
                                            .ghost()
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, window, cx| {
                                                    this.close_quantize_sheet(false, window, cx);
                                                },
                                            )),
                                    ),
                            ),
                        ),
                ),
        ))
    }
}
