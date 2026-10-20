//! `?` 키맵 오버레이(S2-T6b, UI-6, KM-10).
//!
//! 지금 모드의 키 컨텍스트 경로에서 실제로 동작하는 키를 보인다. 엔진에 걸린 바인딩마다 그 경로에서 맞고,
//! 그 키를 눌렀을 때 GPUI가 고르는 첫 바인딩이 그것인 것만 넣는다. 그래서 사용자 키맵·프리셋·수식키 교환이
//! 그대로 반영된다(KM-10 「항상 현재 키맵」). 키맵 판이나 모드가 바뀌면 다시 구하므로 열린 동안 키맵이나
//! 모드(전역 `ctrl-tab`)가 바뀌어도 따라간다.
//!
//! 오버레이는 모드 키 컨텍스트 겹 바깥(팔레트처럼)에 그리고 포커스를 가진다. 컨텍스트는 `Overlay`다.

use std::rc::Rc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::{
    App, AsKeystroke as _, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyContext,
    KeymapVersion, Keystroke, MouseButton, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, TestSupportExt as _, Window, deferred, div, prelude::FluentBuilder as _, px,
};

use crate::actions::help;
use crate::window::ProjectWindow;

/// 한 쪽의 줄 수.
const LINES_PER_PAGE: usize = 24;

/// 구한 줄과 그때의 (키맵 판, 모드 경로).
type Cached = (KeymapVersion, &'static [&'static str], Rc<Vec<Line>>);

/// 열린 오버레이.
pub(crate) struct KeymapOverlay {
    focus: FocusHandle,
    page: usize,
    /// 마지막으로 구한 줄과 그때의 (키맵 판, 모드 경로). 그릴 때마다 모든 바인딩을 다시 훑지 않는다(리뷰 1
    /// 제안 5). 키맵이 바뀌거나 모드가 바뀌면 다시 구한다.
    cache: Option<Cached>,
}

/// 오버레이 한 줄: 명령(인자 있으면 변형), 제목, 키들.
pub(crate) struct Line {
    name: String,
    title: String,
    keys: Vec<Vec<Keystroke>>,
}

impl Line {
    fn keys_text(&self) -> Vec<String> {
        self.keys
            .iter()
            .map(|seq| {
                seq.iter()
                    .map(Keystroke::unparse)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    fn text(&self) -> String {
        format!(
            "{} · {} ({})",
            self.keys_text().join(", "),
            self.title,
            self.name
        )
    }
}

/// 그 경로에서 실제로 동작하는 키를 명령 표 순서로 모은다. 인자 있는 Action은 명령 표의 변형(`Step 길이 ·
/// 온음표`)마다 따로 둔다(리뷰 1 지적 3).
fn lines_for(path: &[&str], cx: &App) -> Vec<Line> {
    let stack: Vec<KeyContext> = path
        .iter()
        .filter_map(|c| KeyContext::parse(c).ok())
        .collect();
    let entries = crate::commands::palette_entries(cx);
    // 변형이 없는(값이 열린) 인자의 표기는 GPUI Action에서 꺼낼 수 없어(`Debug`는 이름만 찍는다) 키맵 원문에서
    // 찾는다(리뷰 2 지적 2). 원문 항목은 인자로 Action을 다시 만들어 엔진의 Action과 맞춘다.
    let source = {
        let (layers, _) = crate::keymap::current_layers(cx);
        crate::keymap::user::effective_with(&layers, crate::keymap::options(cx).swap_alt_cmd)
    };
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    // (순서, 묶음 키, 줄). 묶음 키는 팔레트 줄 번호(변형까지 같은 Action) 또는 Action 표기다
    let mut lines: Vec<(usize, String, Line)> = Vec::new();
    for binding in keymap.bindings() {
        let action = binding.action();
        let name = action.name();
        // 지우기·지정 해제와 명령 표에 없는 것(gpui-component 바탕 레이어)은 보이지 않는다
        let Some(spec) = crate::commands::spec(name) else {
            continue;
        };
        if binding
            .predicate()
            .is_some_and(|p| p.depth_of(&stack).is_none())
        {
            continue;
        }
        let (winners, _) = keymap.bindings_for_input(binding.keystrokes(), &stack);
        if !winners
            .first()
            .is_some_and(|w| w.action().partial_eq(action))
        {
            continue;
        }
        // 팔레트 줄(변형마다 한 줄)에서 같은 Action을 찾아 그 제목을 쓴다. 없으면(값이 열린 인자) 명령 제목과
        // Action 표기
        let found = entries
            .iter()
            .position(|e| e.action().is_some_and(|a| a.partial_eq(action)));
        let (order, title, key) = if let Some(i) = found {
            (i, entries[i].label().to_string(), format!("#{i}"))
        } else {
            // 키·이름으로 찾으면 같은 키가 다른 컨텍스트에서 다른 인자로 걸린 경우 엔진이 고르지 않은 항목을
            // 집는다. 항목의 인자로 Action을 만들어 엔진이 고른 Action과 같은 것을 쓴다(S2-T6b 리뷰 3 제안)
            let label = source
                .iter()
                .find(|e| {
                    e.name == name
                        && cx
                            .build_action(&e.name, e.args.clone())
                            .is_ok_and(|built| built.partial_eq(action))
                })
                .map_or_else(|| name.to_owned(), |e| e.action.clone());
            let args = label
                .strip_prefix(name)
                .and_then(|rest| rest.strip_prefix('('))
                .and_then(|rest| rest.strip_suffix(')'));
            let title = match args {
                Some(args) => format!("{} · {args}", spec.label()),
                None => spec.label().to_owned(),
            };
            let order = entries
                .iter()
                .position(|e| e.name() == name)
                .unwrap_or(usize::MAX);
            (order, title, label)
        };
        let keys: Vec<Keystroke> = binding
            .keystrokes()
            .iter()
            .map(|k| k.as_keystroke().clone())
            .collect();
        match lines.iter_mut().find(|(_, k, _)| *k == key) {
            Some((_, _, line)) => {
                if !line.keys.contains(&keys) {
                    line.keys.push(keys);
                }
            }
            None => lines.push((
                order,
                key,
                Line {
                    name: name.to_owned(),
                    title,
                    keys: vec![keys],
                },
            )),
        }
    }
    lines.sort_by_key(|(order, _, _)| *order);
    lines.into_iter().map(|(_, _, line)| line).collect()
}

/// 키 하나의 표시. gpui-component `Kbd`는 macOS에서 `delete`(앞으로 지우기)도 `⌫`로 그려 `backspace`와 같아
/// 보인다. `delete`만 `⌦`로 그린다(리뷰 2 제안 4).
fn key_element(k: &Keystroke) -> gpui_kit::AnyElement {
    if k.key == "delete" && !k.modifiers.modified() {
        return div().px_1().child("⌦").into_any_element();
    }
    Kbd::new(k.clone()).into_any_element()
}

fn page_count(lines: usize) -> usize {
    lines.div_ceil(LINES_PER_PAGE).max(1)
}

impl ProjectWindow {
    /// 열린 키맵 오버레이의 모든 줄(`키, 키 · 제목 (이름)`). 닫혀 있으면 `None`. 지금 모드의 경로를 쓴다.
    #[must_use]
    pub fn overlay_lines(&self, cx: &App) -> Option<Vec<String>> {
        let overlay = self.overlay.as_ref()?;
        let path = self.mode_state().context_path();
        let version = cx.key_bindings().borrow().version();
        // 그려진 것과 같은 캐시를 본다(리뷰 2 제안 3). 캐시가 지금 판·경로가 아니면 새로 구한다
        let lines = match &overlay.cache {
            Some((v, p, lines)) if *v == version && std::ptr::eq(*p, path) => lines.clone(),
            _ => Rc::new(lines_for(path, cx)),
        };
        Some(lines.iter().map(Line::text).collect())
    }

    /// 열린 키맵 오버레이의 (지금 쪽, 쪽 수). 쪽은 0부터 센다.
    #[must_use]
    pub fn overlay_page(&self) -> Option<(usize, usize)> {
        let overlay = self.overlay.as_ref()?;
        let pages = overlay
            .cache
            .as_ref()
            .map_or(1, |(_, _, lines)| page_count(lines.len()));
        Some((overlay.page.min(pages - 1), pages))
    }

    /// 지금 키맵·지금 모드의 줄. 키맵 판이나 모드 경로가 바뀌었을 때만 다시 구한다. 오버레이가 열린 동안에도
    /// 전역 키(`ctrl-tab`)로 모드가 바뀔 수 있어, 연 때의 경로가 아니라 지금 경로를 쓴다(리뷰 1 지적 2).
    fn overlay_cache(&mut self, cx: &App) -> Option<Rc<Vec<Line>>> {
        let path = self.mode_state().context_path();
        let version = cx.key_bindings().borrow().version();
        let overlay = self.overlay.as_mut()?;
        if let Some((v, p, lines)) = &overlay.cache
            && *v == version
            && std::ptr::eq(*p, path)
        {
            return Some(lines.clone());
        }
        let lines = Rc::new(lines_for(path, cx));
        overlay.cache = Some((version, path, lines.clone()));
        Some(lines)
    }

    /// `help::KeymapOverlay`: 연다. 열려 있으면 닫는다.
    pub(crate) fn help_keymap_overlay(
        &mut self,
        _: &help::KeymapOverlay,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.overlay.is_some() {
            self.close_overlay(window, cx);
            return;
        }
        // 팝업은 하나만 띄운다
        self.palette = None;
        self.popup = None;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.overlay = Some(KeymapOverlay {
            focus,
            page: 0,
            cache: None,
        });
        self.overlay_cache(cx);
        cx.notify();
    }

    /// 오버레이를 닫는다(팔레트를 열 때도 부른다).
    pub(crate) fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.take().is_some() {
            self.focus_document(window, cx);
            cx.notify();
        }
    }

    fn turn_page(&mut self, delta: isize, cx: &mut Context<Self>) {
        let pages = self
            .overlay_cache(cx)
            .map_or(1, |lines| page_count(lines.len()));
        if let Some(o) = &mut self.overlay {
            o.page = o.page.saturating_add_signed(delta).min(pages - 1);
            cx.notify();
        }
    }

    /// 열린 오버레이. 모드 키 컨텍스트 겹 바깥(창 루트의 자식)에 그린다.
    pub(crate) fn render_overlay(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let lines = self.overlay_cache(cx)?;
        let path = self.mode_state().context_path();
        let overlay = self.overlay.as_mut()?;
        // 키맵이나 모드가 바뀌어 줄 수가 줄었으면 쪽을 맞춘다
        let pages = page_count(lines.len());
        overlay.page = overlay.page.min(pages - 1);
        let page = overlay.page;
        let header = format!(
            "키맵 · {} · {}/{} 쪽 (Tab 다음, Shift+Tab 이전, Esc 닫기)",
            path.join(" > "),
            page + 1,
            pages
        );
        let rows = lines
            .iter()
            .skip(page * LINES_PER_PAGE)
            .take(LINES_PER_PAGE)
            .enumerate()
            .map(|(i, line)| {
                let text = line.text();
                div()
                    .id(("overlay-row", i))
                    .aria_label(text)
                    .test_support()
                    .flex()
                    .gap_3()
                    .child(
                        // 키는 `Kbd`로(11절 컴포넌트 표, 리뷰 1 지적 4). 시퀀스는 키를 잇대고 다른 키는 쉼표로
                        div().w(px(220.)).flex().flex_wrap().gap_1().children(
                            line.keys.iter().enumerate().map(|(j, seq)| {
                                div()
                                    .flex()
                                    .gap_0p5()
                                    .when(j > 0, |d| d.child(", "))
                                    .children(seq.iter().map(key_element))
                            }),
                        ),
                    )
                    .child(div().child(line.title.clone()))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(line.name.clone()),
                    )
            })
            .collect::<Vec<_>>();
        let focus = overlay.focus.clone();
        Some(deferred(
            div()
                .id("keymap-overlay-backdrop")
                // 바깥 클릭은 닫기만 하고 아래 요소(버튼)에 닿지 않는다(리뷰 2 지적 1)
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(cx.theme().overlay)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_overlay(window, cx)),
                )
                .child(
                    div()
                        .id("keymap-overlay")
                        .aria_label(header.clone())
                        .test_support()
                        .key_context("Overlay")
                        .track_focus(&focus)
                        .on_action(cx.listener(|this, _: &help::NextPage, _, cx| {
                            this.turn_page(1, cx);
                        }))
                        .on_action(cx.listener(|this, _: &help::PrevPage, _, cx| {
                            this.turn_page(-1, cx);
                        }))
                        .on_action(cx.listener(|this, _: &help::CloseOverlay, window, cx| {
                            this.close_overlay(window, cx);
                        }))
                        // 안쪽 클릭은 닫지 않는다
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .w(px(720.))
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_sm()
                        .bg(cx.theme().popover)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(div().pb_1().child(header))
                        .children(rows),
                ),
        ))
    }
}
