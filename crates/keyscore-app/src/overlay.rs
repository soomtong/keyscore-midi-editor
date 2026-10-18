//! `?` 키맵 오버레이(S2-T6b, UI-6, KM-10).
//!
//! 지금 모드의 키 컨텍스트 경로에서 실제로 동작하는 키를 보인다. 엔진에 걸린 바인딩마다 그 경로에서 맞고,
//! 그 키를 눌렀을 때 GPUI가 고르는 첫 바인딩이 그것인 것만 넣는다. 그래서 사용자 키맵·프리셋·수식키 교환이
//! 그대로 반영된다(KM-10 「항상 현재 키맵」). 그릴 때마다 다시 구하므로 열린 동안 키맵이 바뀌어도 따라간다.
//!
//! 오버레이는 모드 키 컨텍스트 겹 바깥(팔레트처럼)에 그리고 포커스를 가진다. 컨텍스트는 `Overlay`다.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyContext, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window,
    deferred, div, px,
};

use crate::actions::help;
use crate::window::ProjectWindow;

/// 한 쪽의 줄 수.
const LINES_PER_PAGE: usize = 24;

/// 열린 오버레이.
pub(crate) struct KeymapOverlay {
    focus: FocusHandle,
    page: usize,
    pages: usize,
    /// 연 때의 모드 키 컨텍스트 경로
    path: &'static [&'static str],
}

/// 오버레이 한 줄: Action 이름, 명령 제목, 키들.
struct Line {
    name: String,
    title: String,
    keys: Vec<String>,
}

impl Line {
    fn text(&self) -> String {
        format!("{} · {} ({})", self.keys.join(", "), self.title, self.name)
    }
}

/// 그 경로에서 실제로 동작하는 키를 명령 표 순서로 모은다.
fn lines_for(path: &[&str], cx: &App) -> Vec<Line> {
    let stack: Vec<KeyContext> = path
        .iter()
        .filter_map(|c| KeyContext::parse(c).ok())
        .collect();
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let mut lines: Vec<Line> = Vec::new();
    for binding in keymap.bindings() {
        let name = binding.action().name();
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
            .is_some_and(|w| w.action().partial_eq(binding.action()))
        {
            continue;
        }
        let keys = binding
            .keystrokes()
            .iter()
            .map(gpui_kit::KeybindingKeystroke::unparse)
            .collect::<Vec<_>>()
            .join(" ");
        match lines.iter_mut().find(|l| l.name == name) {
            Some(line) => {
                if !line.keys.contains(&keys) {
                    line.keys.push(keys);
                }
            }
            None => lines.push(Line {
                name: name.to_owned(),
                title: spec.label().to_owned(),
                keys: vec![keys],
            }),
        }
    }
    let order = |name: &str| {
        crate::commands::specs()
            .iter()
            .position(|s| s.name() == name)
            .unwrap_or(usize::MAX)
    };
    lines.sort_by_key(|l| order(&l.name));
    lines
}

fn page_count(lines: usize) -> usize {
    lines.div_ceil(LINES_PER_PAGE).max(1)
}

impl ProjectWindow {
    /// 열린 키맵 오버레이의 모든 줄(`키, 키 · 제목 (이름)`). 닫혀 있으면 `None`.
    #[must_use]
    pub fn overlay_lines(&self, cx: &App) -> Option<Vec<String>> {
        let overlay = self.overlay.as_ref()?;
        Some(lines_for(overlay.path, cx).iter().map(Line::text).collect())
    }

    /// 열린 키맵 오버레이의 (지금 쪽, 쪽 수). 쪽은 0부터 센다.
    #[must_use]
    pub fn overlay_page(&self) -> Option<(usize, usize)> {
        self.overlay.as_ref().map(|o| (o.page, o.pages))
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
        let path = self.mode_state().context_path();
        let pages = page_count(lines_for(path, cx).len());
        self.overlay = Some(KeymapOverlay {
            focus,
            page: 0,
            pages,
            path,
        });
        cx.notify();
    }

    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.take().is_some() {
            self.focus_document(window, cx);
            cx.notify();
        }
    }

    fn turn_page(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(o) = &mut self.overlay {
            let last = o.pages.saturating_sub(1);
            o.page = o.page.saturating_add_signed(delta).min(last);
            cx.notify();
        }
    }

    /// 열린 오버레이. 모드 키 컨텍스트 겹 바깥(창 루트의 자식)에 그린다.
    pub(crate) fn render_overlay(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let lines = lines_for(self.overlay.as_ref()?.path, cx);
        let overlay = self.overlay.as_mut()?;
        // 키맵이 바뀌어 줄 수가 바뀌었으면 쪽 수를 맞춘다
        overlay.pages = page_count(lines.len());
        overlay.page = overlay.page.min(overlay.pages - 1);
        let (page, pages) = (overlay.page, overlay.pages);
        let header = format!(
            "키맵 · {} · {}/{} 쪽 (Tab 다음, Shift+Tab 이전, Esc 닫기)",
            overlay.path.join(" > "),
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
                        div()
                            .w(px(180.))
                            .text_color(cx.theme().accent_foreground)
                            .child(line.keys.join(", ")),
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
