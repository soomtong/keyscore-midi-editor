//! 건반 뷰의 창 쪽(S3-T5): 하단 영역에 그리기와 옥타브 버튼. 계산은 `keyboard_view`, 규칙은
//! `docs/reports/s3-t5/plan.md` 「끝의 정의」.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::{
    Action, App, ClickEvent, Context, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, div,
    hsla, prelude::FluentBuilder as _, px, relative,
};

use crate::actions::step;
use crate::dispatch::run_action;
use crate::keyboard_view::{KEY_MAX_HEIGHT, KeyCap, WHITE_KEY_MAX_WIDTH, keys};
use crate::mode::Mode;
use crate::window::ProjectWindow;

/// 건반 높이에 대한 검은건반 높이.
const BLACK_HEIGHT: f32 = 0.6;
/// 흰건반 너비에 대한 검은건반 너비.
const BLACK_WIDTH: f32 = 0.6;

/// 건반 색(명도). 조성 안은 밝게, 밖은 어둡게(REQ 2 「조성 표시」). 흰·검은건반 구분은 그대로 둔다.
fn key_fill(cap: &KeyCap, has_key: bool) -> Hsla {
    let lightness = match (cap.black, has_key, cap.out_of_key) {
        (false, _, false) => 0.96,
        (false, _, true) => 0.68,
        (true, false, _) => 0.2,
        (true, true, false) => 0.36,
        (true, true, true) => 0.08,
    };
    hsla(0., 0., lightness, 1.)
}

fn key_text(cap: &KeyCap) -> Hsla {
    if cap.black {
        hsla(0., 0., 0.92, 1.)
    } else {
        hsla(0., 0., 0.12, 1.)
    }
}

/// 건반 색에 조성 명암을 더하는가. 드럼은 조성이 있어도 조성 없음과 똑같이 그린다(결정
/// `2026-10-31-key-shading-off-and-drums.md` 2).
fn shades_key(key: keyscore_core::Key, drum: bool) -> bool {
    !drum && key != keyscore_core::Key::None
}

/// 키 라벨의 키캡 상자. 음 이름(상자 밖 작은 글자)과 모양으로 가른다(결정
/// `2026-10-31-keyboard-view-direct-labels.md` 2).
fn keycap(i: usize, key: String, cap: &KeyCap) -> impl IntoElement {
    let edge = if cap.black {
        hsla(0., 0., 0.7, 1.)
    } else {
        hsla(0., 0., 0.45, 1.)
    };
    div()
        .id(ElementId::Name(format!("keyboard-keycap-{i}").into()))
        .test_support()
        .mb_0p5()
        .px_1()
        .min_w(px(16.))
        .flex()
        .justify_center()
        .border_1()
        .border_color(edge)
        .rounded_sm()
        .child(key)
}

/// 접근성 라벨: `<음 이름> <키 라벨>`, 둘 중 없는 쪽은 뺀다. 없는 건반은 `없음`.
fn accessible(cap: &KeyCap) -> String {
    if cap.pitch.is_none() {
        return "없음".to_owned();
    }
    [cap.name.as_deref(), cap.key.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
}

impl ProjectWindow {
    /// 지금 그리는 건반 뷰. Step이 아니거나 하단 영역이 접혔으면 `None`. 시험·접근성용.
    #[must_use]
    pub fn keyboard_view(&self, cx: &App) -> Option<Vec<KeyCap>> {
        if !self.layout.bottom_open || self.mode_state().mode() != Mode::Step {
            return None;
        }
        Some(keys(
            self.step.layout,
            self.step.octave,
            self.document.read(cx).project().key(),
            self.current_track_is_drum(cx),
        ))
    }

    /// 현재 트랙이 드럼 트랙(채널 10, REQ 5.13)인가.
    fn current_track_is_drum(&self, cx: &App) -> bool {
        let project = self.document.read(cx).project();
        self.current_track()
            .and_then(|id| project.track(id))
            .is_some_and(keyscore_core::Track::is_drum)
    }

    /// 옥타브 버튼과 현재 옥타브(REQ 5.4 「건반 뷰의 옥타브 버튼」).
    fn render_octave_controls(&self) -> impl IntoElement {
        // 버튼은 키와 같은 Action을 낸다(처리기가 조합 확정·범위 메시지를 맡는다). 하단 영역은 모드 키
        // 컨텍스트 밖이라 누른 뒤 포커스를 문서로 돌린다(트랜스포트 바와 같다).
        let doc = self.document_focus();
        // 접근성 라벨은 글자(`+`)가 아니라 동작을 말한다(리뷰 1 제안 3)
        let octave_button = |id: &'static str,
                             label: &'static str,
                             spoken: &'static str,
                             action: Box<dyn Action>| {
            let doc = doc.clone();
            Button::new(id)
                .label(label)
                .accessibility_label(spoken)
                .ghost()
                .on_click(move |_: &ClickEvent, window, cx| {
                    run_action(action.boxed_clone(), window, cx);
                    window.focus(&doc, cx);
                })
        };
        let octave = format!("oct{}", self.step.octave);
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .px_2()
            .child(octave_button(
                "keyboard-octave-up",
                "+",
                "옥타브 올리기",
                Box::new(step::OctaveUp),
            ))
            .child(
                div()
                    .id("keyboard-octave")
                    .aria_label(octave.clone())
                    .test_support()
                    .text_sm()
                    .child(octave),
            )
            .child(octave_button(
                "keyboard-octave-down",
                "−",
                "옥타브 내리기",
                Box::new(step::OctaveDown),
            ))
    }

    /// 하단 영역의 건반 뷰. 왼쪽에 옥타브 버튼, 오른쪽에 24건반.
    pub(crate) fn render_keyboard_view(
        &self,
        caps: &[KeyCap],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let drum = self.current_track_is_drum(cx);
        let has_key = shades_key(self.document.read(cx).project().key(), drum);
        let controls = self.render_octave_controls();
        let whites = caps.iter().filter(|c| !c.black).count().max(1);
        #[allow(clippy::cast_precision_loss)] // 건반 수는 24다
        let white_width = 1. / whites as f32;
        let mut white_index = 0usize;
        let mut white_keys = Vec::new();
        let mut black_keys = Vec::new();
        for (i, cap) in caps.iter().enumerate() {
            let label = accessible(cap);
            // 음 이름은 C 건반에만 그린다(좁은 건반에 글이 넘치지 않게). 드럼 라벨은 모두 그린다
            let drawn_name = cap
                .name
                .clone()
                .filter(|_| drum || cap.pitch.is_some_and(|p| p.get() % 12 == 0));
            let body = div()
                .id(ElementId::Name(format!("keyboard-key-{i}").into()))
                .aria_label(label)
                .test_support()
                .flex()
                .flex_col()
                .justify_end()
                .items_center()
                .pb_1()
                .text_xs()
                .text_color(key_text(cap))
                .border_1()
                .border_color(cx.theme().border)
                .children(cap.key.clone().map(|k| keycap(i, k, cap)))
                // 음 이름 자리는 이름이 없는 건반에도 비워 둔다. 키캡이 같은 높이에 줄 선다
                .child(
                    div()
                        .id(ElementId::Name(format!("keyboard-name-{i}").into()))
                        .test_support()
                        .h_4()
                        // 검은건반은 좁아 세 글자 드럼 라벨(`n58`)이 넘친다. 글씨를 줄인다(S3-T5c)
                        .when(cap.black, |d| d.text_size(px(9.)))
                        .children(drawn_name),
                );
            let body = if cap.pitch.is_some() {
                body.bg(key_fill(cap, has_key))
            } else {
                body.bg(cx.theme().muted)
            };
            if cap.black {
                // 앞 흰건반 경계에 가운데를 맞춘다
                #[allow(clippy::cast_precision_loss)]
                let left = white_index as f32 * white_width - white_width * BLACK_WIDTH / 2.;
                black_keys.push(
                    body.absolute()
                        .top_0()
                        .left(relative(left))
                        .w(relative(white_width * BLACK_WIDTH))
                        .h(relative(BLACK_HEIGHT)),
                );
            } else {
                white_index += 1;
                white_keys.push(body.flex_1().h_full());
            }
        }
        // 흰건반 너비·건반 높이에 상한을 둔다. 넘는 자리는 오른쪽에 비운다(결정 2026-10-31 3)
        #[allow(clippy::cast_precision_loss)]
        let max_width = whites as f32 * WHITE_KEY_MAX_WIDTH;
        let board = div()
            .relative()
            .flex_1()
            .max_w(px(max_width))
            .h_full()
            .max_h(px(KEY_MAX_HEIGHT))
            .flex()
            .children(white_keys)
            .children(black_keys);
        div()
            .id("keyboard-view")
            .aria_label("건반 뷰")
            .test_support()
            .size_full()
            .flex()
            .items_center()
            .p_1()
            .bg(cx.theme().background)
            .child(controls)
            .child(board.min_w(px(0.)))
    }
}

#[cfg(test)]
mod tests {
    use keyscore_core::{Key, PitchClass};

    use super::*;

    #[test]
    fn drum_track_draws_like_no_key() {
        let d_major = Key::Major(PitchClass::new(2).unwrap());
        assert!(shades_key(d_major, false));
        assert!(!shades_key(d_major, true));
        assert!(!shades_key(Key::None, false));
        // 드럼의 검은건반은 조성 없음과 같은 색
        let black = KeyCap {
            pitch: keyscore_core::Pitch::new(37).ok(),
            black: true,
            key: None,
            name: None,
            out_of_key: false,
        };
        assert_eq!(
            key_fill(&black, shades_key(d_major, true)),
            key_fill(&black, shades_key(Key::None, true))
        );
    }
}
