//! 노트 정보 팝업(S3-T7, REQ 5.8 `i` 「커서 노트 정보 팝업(틱·길이·채널)」, 5.8.1 vim `K`).
//!
//! 규칙은 결정 `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md` 6절이다.

use keyscore_core::{ClipId, Note, PitchOffset, Project, Tick, TrackId};

use crate::selection::pitch_name;

/// 팝업의 한 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoLine {
    pub label: &'static str,
    pub value: String,
}

/// `track`의 `clip`에 든 `note`의 정보 줄(결정 6절 2). 트랙·클립이 없으면 빈 목록.
#[must_use]
pub fn info_lines(project: &Project, track: TrackId, clip: ClipId, note: &Note) -> Vec<InfoLine> {
    let Some(t) = project.track(track) else {
        return Vec::new();
    };
    let Some(c) = t.clip(clip) else {
        return Vec::new();
    };
    let start = Tick::new(c.start().get().saturating_add(note.start().get()));
    let pos = project.meter_map().position(start);
    let mut lines = vec![
        InfoLine {
            label: "음높이",
            value: pitch_text(note),
        },
        InfoLine {
            label: "시작",
            value: format!(
                "{}.{}.{} ({}틱)",
                pos.bar() + 1,
                pos.beat() + 1,
                pos.tick(),
                start.get()
            ),
        },
        InfoLine {
            label: "길이",
            value: match length_name(note.length()) {
                Some(name) => format!("{}틱 ({name})", note.length()),
                None => format!("{}틱", note.length()),
            },
        },
        InfoLine {
            label: "채널",
            value: t.channel().get().to_string(),
        },
        InfoLine {
            label: "벨로시티",
            value: note.velocity().to_midi1().to_string(),
        },
    ];
    if let Some(lyric) = note.lyric() {
        lines.push(InfoLine {
            label: "가사",
            value: lyric.to_owned(),
        });
    }
    lines.push(InfoLine {
        label: "트랙",
        value: t.name().to_owned(),
    });
    lines
}

/// 「C#4 (61)」, 오프셋이 있으면 「 +25 cent」를 덧붙인다(반음 = 100 cent, 반올림).
fn pitch_text(note: &Note) -> String {
    let pitch = note.pitch();
    let text = format!("{} ({})", pitch_name(pitch), pitch.get());
    let units = i64::from(note.offset().units());
    if units == 0 {
        return text;
    }
    let semitone = i64::from(PitchOffset::SEMITONE);
    // 가장 가까운 0.1 cent(절반은 0에서 먼 쪽). 정수 cent로 반올림하면 작은 오프셋이 「+0 cent」로 보인다
    // (S3-T7 리뷰 2 제안 4)
    let tenths = (units * 2000 + units.signum() * semitone) / (2 * semitone);
    // 부호는 반올림 전 값에서 정한다. 0.05 cent 미만은 0이 아니라 「0.1 cent 미만」이다(리뷰 3 지적 3)
    let sign = if units < 0 { '-' } else { '+' };
    let (whole, frac) = (tenths.abs() / 10, tenths.abs() % 10);
    if tenths == 0 {
        format!("{text} {sign}<0.1 cent")
    } else if frac == 0 {
        format!("{text} {sign}{whole} cent")
    } else {
        format!("{text} {sign}{whole}.{frac} cent")
    }
}

/// 길이 코드(온음표–64분음표, 점, 셋잇단)와 같은 틱이면 그 이름.
fn length_name(ticks: u32) -> Option<String> {
    const NAMES: [&str; 7] = [
        "온음표",
        "2분음표",
        "4분음표",
        "8분음표",
        "16분음표",
        "32분음표",
        "64분음표",
    ];
    NAMES.iter().enumerate().find_map(|(i, name)| {
        let plain = WHOLE >> i;
        if ticks == plain {
            Some((*name).to_owned())
        } else if ticks == plain + plain / 2 {
            Some(format!("점{name}"))
        } else if u64::from(ticks) * 3 == u64::from(plain) * 2 {
            Some(format!("{name} 셋잇단"))
        } else {
            None
        }
    })
}

/// 온음표 틱(PPQ 960).
const WHOLE: u32 = 3840;

// ---------- 창 쪽 ----------

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    Context, FocusHandle, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _,
    Window, deferred, div, px,
};

use crate::actions::{dialog, note};
use crate::window::ProjectWindow;

/// 열린 노트 정보 팝업. 내용은 연 때의 사본이다. 문서가 바뀌면 닫는다(결정 6절 4).
pub(crate) struct NoteInfoPopup {
    focus: FocusHandle,
    lines: Vec<InfoLine>,
    /// 창이 활성인 동안 포커스가 팝업 밖으로 나가면 닫는다(시트와 같다).
    _focus_out: Subscription,
}

impl ProjectWindow {
    /// 열린 노트 정보 팝업의 줄. 닫혀 있으면 `None`.
    #[must_use]
    pub fn note_info(&self) -> Option<Vec<InfoLine>> {
        self.note_info.as_ref().map(|p| p.lines.clone())
    }

    /// `note::ShowInfo`: 커서 노트의 정보 팝업을 연다(결정 6절). 모든 모드에서 연다.
    pub(crate) fn show_note_info(
        &mut self,
        _: &note::ShowInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((track, clip, n)) = self.note_under_cursor(cx) else {
            self.say("커서에 노트가 없다", cx);
            return;
        };
        let lines = info_lines(self.document.read(cx).project(), track, clip, &n);
        // 팝업은 하나만 띄운다
        self.palette = None;
        self.overlay = None;
        self.popup = None;
        self.quantize_sheet = None;
        self.search_bar = None;
        let focus = cx.focus_handle();
        let focus_out = cx.on_focus_out(&focus, window, |this, _, window, cx| {
            if window.is_window_active() {
                this.close_note_info(window, cx);
            }
        });
        window.focus(&focus, cx);
        self.note_info = Some(NoteInfoPopup {
            focus,
            lines,
            _focus_out: focus_out,
        });
        cx.notify();
    }

    /// 팝업을 닫는다. 포커스가 팝업에 있었으면 문서로 돌린다.
    pub(crate) fn close_note_info(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(popup) = self.note_info.take() else {
            return;
        };
        if popup.focus.contains_focused(window, cx) || window.focused(cx).is_none() {
            self.focus_document(window, cx);
        }
        cx.notify();
    }

    /// 문서가 바뀌면 내용이 낡으므로 닫는다(결정 6절 4).
    pub(crate) fn note_info_document_changed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_note_info(window, cx);
    }

    /// 열린 팝업. 창 루트의 자식으로 그린다.
    pub(crate) fn render_note_info(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let popup = self.note_info.as_ref()?;
        let rows = popup.lines.iter().enumerate().map(|(i, line)| {
            let text = format!("{}: {}", line.label, line.value);
            div()
                .id(SharedString::from(format!("note-info-{i}")))
                .aria_label(text)
                .test_support()
                .flex()
                .gap_2()
                .child(
                    div()
                        .w(px(64.))
                        .text_color(cx.theme().muted_foreground)
                        .child(line.label),
                )
                .child(line.value.clone())
        });
        let rows: Vec<_> = rows.collect();
        Some(deferred(
            div()
                .id("note-info-backdrop")
                // 바깥 클릭은 닫기만 하고 아래 요소에 닿지 않는다(시트와 같다)
                .occlude()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_note_info(window, cx)),
                )
                .child(
                    div()
                        .id("note-info-dialog")
                        .key_context("Dialog")
                        .absolute()
                        .top(px(44.))
                        .right(px(16.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .id("note-info")
                                .aria_label("노트 정보")
                                .test_support()
                                .key_context("NoteInfo")
                                .track_focus(&popup.focus)
                                .on_action(cx.listener(|this, _: &dialog::Confirm, window, cx| {
                                    this.close_note_info(window, cx);
                                }))
                                .on_action(cx.listener(|this, _: &dialog::Cancel, window, cx| {
                                    this.close_note_info(window, cx);
                                }))
                                .w(px(260.))
                                .p_3()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .text_sm()
                                .bg(cx.theme().popover)
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(div().pb_1().child("노트 정보"))
                                .children(rows)
                                .child(
                                    div()
                                        .pt_1()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Esc·Enter 닫기"),
                                ),
                        ),
                ),
        ))
    }
}
