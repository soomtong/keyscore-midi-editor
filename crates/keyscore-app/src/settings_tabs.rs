//! 설정 창의 입력·메트로놈·AI 구역(S2-T6c, MET-1, P3, 7.6).
//!
//! 값은 모두 키보드로 고르는 선택 상자(`Select`, 검색 가능)다. `Tab`/`Shift+Tab`으로 옮겨 가고(REQ 5.11 표
//! 「키맵 편집 · 설정 · 대화상자: 다음 필드」), `Enter`나 `space`로 열고 화살표나 검색어로 고른다. 고르면 곧바로
//! 설정에 남는다(`AppSettings`, 1 s 디바운스 저장). 메트로놈 값은 재생 중인 창에도 바로 반영된다.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::IndexPath;
use gpui_kit::component::button::Button;
use gpui_kit::component::searchable_list::SearchableVec;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::{
    App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _, Window, div,
};
use keyscore_core::OutputPort;

use crate::actions::args::KeyLayout;
use crate::actions::settings;
use crate::app_state::AppSettings;
use crate::gm::percussion_label;
use crate::settings::{InputSettings, MetronomeSettings};
use crate::settings_window::SettingsWindow;

/// 메트로놈 클릭 길이 목록(ms). 파일에 손으로 넣은 다른 값은 목록에 덧붙여 보인다.
const LENGTHS_MS: [u32; 10] = [10, 20, 30, 50, 80, 100, 150, 200, 300, 500];

type Choice = SelectState<SearchableVec<SharedString>>;

/// 구역 안 컨트롤의 이름. 요소 id이고 시험이 포커스를 묻는 이름이다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Control {
    KeyLayout,
    Port,
    Channel,
    Accent,
    Beat,
    Length,
}

impl Control {
    const ALL: [Control; 6] = [
        Control::KeyLayout,
        Control::Port,
        Control::Channel,
        Control::Accent,
        Control::Beat,
        Control::Length,
    ];

    const fn id(self) -> &'static str {
        match self {
            Control::KeyLayout => "settings-key-layout",
            Control::Port => "settings-metronome-port",
            Control::Channel => "settings-metronome-channel",
            Control::Accent => "settings-metronome-accent",
            Control::Beat => "settings-metronome-beat",
            Control::Length => "settings-metronome-length",
        }
    }
}

/// 입력·메트로놈 구역의 상태.
pub(crate) struct Tabs {
    key_layout: Entity<Choice>,
    port: Entity<Choice>,
    channel: Entity<Choice>,
    accent: Entity<Choice>,
    beat: Entity<Choice>,
    length: Entity<Choice>,
    /// 보정 마법사 버튼을 감싼 칸의 포커스(Tab 정지점).
    calibrate: FocusHandle,
    /// 포트 선택 상자의 항목(같은 자리).
    ports: Vec<OutputPort>,
    /// 길이 선택 상자의 항목(같은 자리).
    lengths: Vec<u32>,
    /// 포트 목록을 다시 읽어야 한다(구역을 열 때, 포트가 오갈 때).
    ports_stale: bool,
    /// 선택 상자가 지금 보이는 설정. 다르면(팔레트·파일 등 밖에서 바뀜) 선택을 맞춘다.
    shown: Option<(InputSettings, MetronomeSettings)>,
    _subscriptions: Vec<Subscription>,
}

fn labels(items: impl IntoIterator<Item = String>) -> SearchableVec<SharedString> {
    SearchableVec::new(
        items
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    )
}

const fn layout_label(layout: KeyLayout) -> &'static str {
    match layout {
        KeyLayout::Piano => "건반형",
        KeyLayout::Solfege => "계이름형",
    }
}

fn length_label(ms: u32) -> String {
    format!("{ms} ms")
}

fn port_choice_label(port: &OutputPort, virtual_name: &str) -> String {
    match port {
        OutputPort::Virtual => format!("{virtual_name} · 가상 포트"),
        OutputPort::Named(name) => name.clone(),
    }
}

impl Tabs {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<SettingsWindow>) -> Self {
        let make = |items: SearchableVec<SharedString>,
                    window: &mut Window,
                    cx: &mut Context<SettingsWindow>| {
            cx.new(|cx| SelectState::new(items, None, window, cx).searchable(true))
        };
        let key_layout = make(
            labels([KeyLayout::Piano, KeyLayout::Solfege].map(|l| layout_label(l).to_owned())),
            window,
            cx,
        );
        let port = make(labels([]), window, cx);
        let channel = make(labels((1..=16u8).map(|c| c.to_string())), window, cx);
        let accent = make(labels((0..=127u8).map(percussion_label)), window, cx);
        let beat = make(labels((0..=127u8).map(percussion_label)), window, cx);
        let length = make(labels(LENGTHS_MS.map(length_label)), window, cx);
        let mut subscriptions = Vec::new();
        for (control, state) in [
            (Control::KeyLayout, &key_layout),
            (Control::Port, &port),
            (Control::Channel, &channel),
            (Control::Accent, &accent),
            (Control::Beat, &beat),
            (Control::Length, &length),
        ] {
            subscriptions.push(cx.subscribe_in(
                state,
                window,
                move |this, state, event: &SelectEvent<SearchableVec<SharedString>>, window, cx| {
                    let SelectEvent::Confirm(Some(_)) = event else {
                        return;
                    };
                    let Some(index) = state.read(cx).selected_index(cx).map(|i| i.row) else {
                        return;
                    };
                    this.tabs_chosen(control, index, window, cx);
                },
            ));
        }
        Tabs {
            key_layout,
            port,
            channel,
            accent,
            beat,
            length,
            calibrate: cx.focus_handle().tab_stop(true),
            ports: Vec::new(),
            lengths: LENGTHS_MS.to_vec(),
            ports_stale: true,
            shown: None,
            _subscriptions: subscriptions,
        }
    }

    fn state(&self, control: Control) -> &Entity<Choice> {
        match control {
            Control::KeyLayout => &self.key_layout,
            Control::Port => &self.port,
            Control::Channel => &self.channel,
            Control::Accent => &self.accent,
            Control::Beat => &self.beat,
            Control::Length => &self.length,
        }
    }

    /// 포커스를 가진 컨트롤의 요소 id.
    pub(crate) fn focused(&self, window: &Window, cx: &App) -> Option<&'static str> {
        if self.calibrate.is_focused(window) {
            return Some("settings-calibrate");
        }
        Control::ALL
            .into_iter()
            .find(|c| {
                self.state(*c)
                    .read(cx)
                    .focus_handle(cx)
                    .contains_focused(window, cx)
            })
            .map(Control::id)
    }

    /// 구역의 첫 컨트롤에 포커스를 준다. 그 구역에 컨트롤이 없으면 `false`.
    pub(crate) fn focus_first(
        &mut self,
        section: crate::actions::args::SettingsSection,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        use crate::actions::args::SettingsSection as S;
        let first = match section {
            S::Input => Control::KeyLayout,
            S::Metronome => {
                self.ports_stale = true;
                Control::Port
            }
            S::MidiOutput | S::Keymap | S::Ai => return false,
        };
        let handle = self.state(first).read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        true
    }

    /// 포트가 오갔다. 메트로놈 구역을 다음에 그릴 때 목록을 다시 읽는다.
    pub(crate) fn ports_changed(&mut self) {
        self.ports_stale = true;
    }

    /// 선택 상자의 항목·선택을 지금 설정에 맞춘다. 그릴 때 부른다(선택을 바꾸려면 `Window`가 필요하다).
    fn sync(&mut self, window: &mut Window, cx: &mut App) {
        let settings = AppSettings::get(cx);
        let now = (settings.input, settings.metronome.clone());
        let mut force = false;
        if self.ports_stale {
            self.ports_stale = false;
            let mut ports = vec![OutputPort::Virtual];
            ports.extend(
                keyscore_midi::destinations()
                    .into_iter()
                    .map(|d| OutputPort::Named(d.name().to_owned())),
            );
            // 지금 값이 목록에 없으면(포트가 사라짐) 그대로 보이게 넣는다
            if !ports.contains(&now.1.port) {
                ports.push(now.1.port.clone());
            }
            if ports != self.ports {
                let virtual_name = crate::engine::virtual_port_name(cx);
                let items = labels(ports.iter().map(|p| port_choice_label(p, &virtual_name)));
                self.port.update(cx, |s, cx| s.set_items(items, window, cx));
                self.ports = ports;
                force = true;
            }
        }
        if !self.lengths.contains(&now.1.length_ms) {
            let mut lengths = LENGTHS_MS.to_vec();
            lengths.push(now.1.length_ms);
            lengths.sort_unstable();
            let items = labels(lengths.iter().map(|ms| length_label(*ms)));
            self.length
                .update(cx, |s, cx| s.set_items(items, window, cx));
            self.lengths = lengths;
            force = true;
        }
        if !force && self.shown.as_ref() == Some(&now) {
            return;
        }
        let (input, m) = &now;
        let rows = [
            (
                Control::KeyLayout,
                Some(usize::from(input.key_layout == KeyLayout::Solfege)),
            ),
            (Control::Port, self.ports.iter().position(|p| *p == m.port)),
            (
                Control::Channel,
                Some(usize::from(m.channel.saturating_sub(1))),
            ),
            (Control::Accent, Some(usize::from(m.accent))),
            (Control::Beat, Some(usize::from(m.beat))),
            (
                Control::Length,
                self.lengths.iter().position(|l| *l == m.length_ms),
            ),
        ];
        for (control, row) in rows {
            self.state(control).update(cx, |s, cx| {
                s.set_selected_index(row.map(IndexPath::new), window, cx);
            });
        }
        self.shown = Some(now);
    }
}

impl SettingsWindow {
    /// 선택 상자에서 값을 골랐다.
    fn tabs_chosen(
        &mut self,
        control: Control,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if control == Control::KeyLayout {
            let layout = if index == 0 {
                KeyLayout::Piano
            } else {
                KeyLayout::Solfege
            };
            // 팔레트와 같은 Action으로 바꾼다(처리기는 앱 전역)
            window.dispatch_action(Box::new(settings::SetKeyLayout(layout)), cx);
            return;
        }
        let tabs = self.tabs();
        let mut metronome = AppSettings::get(cx).metronome.clone();
        match control {
            Control::Port => match tabs.ports.get(index) {
                Some(port) => metronome.port = port.clone(),
                None => return,
            },
            Control::Channel => match u8::try_from(index + 1) {
                Ok(c) if (1..=16).contains(&c) => metronome.channel = c,
                _ => return,
            },
            Control::Accent => match u8::try_from(index) {
                Ok(n) if n <= 127 => metronome.accent = n,
                _ => return,
            },
            Control::Beat => match u8::try_from(index) {
                Ok(n) if n <= 127 => metronome.beat = n,
                _ => return,
            },
            Control::Length => match tabs.lengths.get(index) {
                Some(ms) => metronome.length_ms = *ms,
                None => return,
            },
            Control::KeyLayout => return,
        }
        if metronome == AppSettings::get(cx).metronome {
            return;
        }
        AppSettings::update(cx, |s| s.metronome = metronome);
        crate::engine::metronome_changed(cx);
        cx.notify();
    }

    /// 입력 구역.
    pub(crate) fn render_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.tabs_mut().sync(window, cx);
        let layout = AppSettings::get(cx).input.key_layout;
        let tabs = self.tabs();
        let calibrate = tabs.calibrate.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(row(
                "키 레이아웃",
                Control::KeyLayout,
                format!("키 레이아웃: {}", layout_label(layout)),
                &tabs.key_layout,
                cx,
            ))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("건반형은 A S D F … 가 흰건반, W E T … 가 검은건반이다. 계이름형은 c d e f g a b로 친다(P3)."),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(gpui_kit::px(120.)).child("레이턴시 보정"))
                    .child(
                        div()
                            .id("settings-calibrate")
                            .aria_label("보정 마법사")
                            .test_support()
                            .track_focus(&calibrate)
                            .rounded_md()
                            .border_1()
                            .border_color(if calibrate.is_focused(window) {
                                cx.theme().ring
                            } else {
                                cx.theme().transparent
                            })
                            .on_key_down(cx.listener(|_, event: &KeyDownEvent, window, cx| {
                                let k = &event.keystroke;
                                if !k.modifiers.modified() && (k.key == "enter" || k.key == "space") {
                                    window.dispatch_action(Box::new(settings::Calibrate), cx);
                                    cx.stop_propagation();
                                }
                            }))
                            .child(
                                Button::new("settings-calibrate-button")
                                    .label("보정 마법사")
                                    .tab_stop(false)
                                    .on_click(|_: &ClickEvent, window, cx| {
                                        window.dispatch_action(Box::new(settings::Calibrate), cx);
                                    }),
                            ),
                    ),
            )
    }

    /// 메트로놈 구역(MET-1).
    pub(crate) fn render_metronome(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.tabs_mut().sync(window, cx);
        let m = AppSettings::get(cx).metronome.clone();
        let virtual_name = crate::engine::virtual_port_name(cx);
        let tabs = self.tabs();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(row(
                "포트",
                Control::Port,
                format!("포트: {}", port_choice_label(&m.port, &virtual_name)),
                &tabs.port,
                cx,
            ))
            .child(row(
                "채널",
                Control::Channel,
                format!("채널: {}", m.channel),
                &tabs.channel,
                cx,
            ))
            .child(row(
                "강박 노트",
                Control::Accent,
                format!("강박 노트: {}", percussion_label(m.accent)),
                &tabs.accent,
                cx,
            ))
            .child(row(
                "약박 노트",
                Control::Beat,
                format!("약박 노트: {}", percussion_label(m.beat)),
                &tabs.beat,
                cx,
            ))
            .child(row(
                "길이",
                Control::Length,
                format!("길이: {}", length_label(m.length_ms)),
                &tabs.length,
                cx,
            ))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("재생 중에는 트랜스포트의 메트로놈 버튼에 따른다. 바꾼 값은 재생 중에도 바로 들린다."),
            )
    }
}

/// 제목 + 선택 상자 한 줄. 감싼 칸의 id와 접근성 이름이 지금 값을 알린다.
fn row(
    title: &'static str,
    control: Control,
    value: String,
    state: &Entity<Choice>,
    cx: &App,
) -> impl IntoElement {
    let focus = state.read(cx).focus_handle(cx);
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(div().w(gpui_kit::px(120.)).child(title))
        .child(
            div()
                .id(control.id())
                .aria_label(value)
                .test_support()
                .w(gpui_kit::px(260.))
                // 닫힌 선택 상자는 `space`로도 연다(macOS 팝업 버튼 관례, 계획 5). 선택 상자의 키 바인딩은
                // `enter`만 열므로 같은 Action을 낸다. 열린 동안은 검색란이 포커스라 공백을 글자로 받는다.
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let k = &event.keystroke;
                    if k.key == "space" && !k.modifiers.modified() && focus.is_focused(window) {
                        window.dispatch_action(
                            Box::new(gpui_kit::base::actions::Confirm { secondary: false }),
                            cx,
                        );
                        cx.stop_propagation();
                    }
                })
                .child(Select::new(state).search_placeholder("검색")),
        )
}

/// AI 구역: S7까지 비활성 자리(7.6 대응표의 항목 이름만).
pub(crate) fn render_ai(cx: &App) -> impl IntoElement {
    let items = [
        ("settings-ai-endpoint", "엔드포인트(베이스 URL)"),
        ("settings-ai-models", "모델 두 자리(어시스트·생성)"),
        ("settings-ai-key", "API 키(키체인)"),
        ("settings-ai-templates", "템플릿 폴더 열기"),
        ("settings-ai-taste", "취향 지시"),
    ];
    div()
        .flex()
        .flex_col()
        .gap_2()
        .text_color(cx.theme().muted_foreground)
        .child("AI 설정은 S7에서 켠다. 지금은 바꿀 수 없다.")
        .children(items.map(|(id, title)| {
            div()
                .id(id)
                .aria_label(format!("{title}: S7에서 켠다(비활성)"))
                .test_support()
                .opacity(0.5)
                .child(title)
        }))
}
