//! 프로젝트 창(창 하나 = 프로젝트 하나). 7.1의 레이아웃 골격.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::{
    ActiveTheme as _, ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
    v_resizable,
};
use gpui_kit::{
    Action, Anchor, AnyElement, App, AppContext as _, Bounds, Context, DismissEvent, Div, Entity,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    Point, Render, Role, Stateful, StatefulInteractiveElement as _, Styled as _, Subscription,
    TestSupportExt as _, Window, anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};

use crate::actions::view::{ToggleBottomPanel, ToggleInspector, ToggleTrackList};
use crate::actions::{ai, clip, mode, palette, view};
use crate::app_state::AppSettings;
use crate::command_palette::{CommandPalette, PaletteEvent};
use crate::commands::RunCommand;
use crate::commands::ex::ExCommand;
use crate::dispatch::run_action;
use crate::document::ProjectDocument;
use crate::mode::{Mode, ModeEvent, ModeState, View};
use crate::settings::{LayoutSettings, PANEL_SIZE_RANGE};
use crate::transport::Transport;
use crate::transport_bar::TransportBar;

/// 명령 팔레트의 폭과 창 위쪽에서 떨어진 거리.
const PALETTE_WIDTH: Pixels = px(560.);
const PALETTE_TOP: Pixels = px(64.);

/// 크기를 저장하는 패널 그룹.
#[derive(Clone, Copy)]
enum Group {
    Horizontal,
    Vertical,
}

/// 창의 루트 뷰.
pub struct ProjectWindow {
    pub(crate) document: Entity<ProjectDocument>,
    pub(crate) layout: LayoutSettings,
    focus: FocusHandle,
    /// 가로 그룹(트랙 목록 | 메인 | 인스펙터)과 세로 그룹(위 | 하단 영역)의 크기 상태.
    horizontal: Entity<ResizableState>,
    vertical: Entity<ResizableState>,
    /// 크기 상태 구독. 상태를 새로 만들 때 함께 바꾼다.
    resize_subscriptions: [Subscription; 2],
    _focus_lost: Subscription,
    /// 모드 FSM 상태. 키 컨텍스트와 상태줄이 이것을 따른다.
    mode: ModeState,
    /// 열린 팝업 메뉴(상태줄 모드 메뉴, 조성, 출력 포트). 팝업은 하나만 띄운다.
    pub(crate) popup: Option<Popup>,
    /// 상태줄 모드 표시의 지난 프레임 위치. 모드 메뉴를 그 바로 위에 띄운다(근삿값 대신 실제 위치,
    /// S2-T3 리뷰 2 제안 3).
    indicator_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// 현재 트랙(S2-T5b). 인스펙터·포트 드롭다운·트랙 Action의 대상이다.
    current_track: Option<keyscore_core::TrackId>,
    /// 현재 트랙의 마지막으로 알려진 자리. undo 등으로 현재 트랙이 사라지면 그 자리의 이웃이 현재 트랙이
    /// 된다(S2-T5b 리뷰 1 지적 4).
    current_index: usize,
    /// 트랜스포트 바의 입력 상태(S2-T5b).
    pub(crate) transport_bar: TransportBar,
    /// 창의 한 줄 입력란(트랜스포트·인스펙터).
    pub(crate) fields: crate::fields::Fields,
    /// 트랙 인스펙터(S2-T5c).
    pub(crate) inspector: crate::inspector::Inspector,
    /// 열린 `?` 키맵 오버레이(S2-T6b).
    pub(crate) overlay: Option<crate::overlay::KeymapOverlay>,
    /// 열린 명령 팔레트와 그 사건 구독.
    pub(crate) palette: Option<(Entity<CommandPalette>, Subscription)>,
    /// 열린 퀀타이즈 설정 시트(S3-T3e).
    pub(crate) quantize_sheet: Option<crate::quantize_sheet::QuantizeSheet>,
    /// 트랜스포트 상태(S2-T5a).
    pub(crate) transport: Transport,
    /// undo 상한 알림을 이미 냈는가(창마다 한 번, EDT-1 결정 A).
    history_notice_shown: bool,
    /// 미리듣기 실패 알림을 이미 냈는가(창마다 한 번, S3-T2 리뷰 1 제안 11).
    pub(crate) preview_notice_shown: bool,
    /// 문서가 바뀌면 재생 중 스냅샷을 교체한다(TRN-3). 창이 닫히면 재생을 멈춘다.
    _document_changed: Subscription,
    _released: Subscription,
    /// 가로 패널 크기 상태가 지금 그리는 사이드 패널 너비(트랙 목록, 인스펙터)의 기준. 상태를 새로 만든 뒤
    /// 처음 그릴 때 요청한 너비로 채우고, 조절을 저장할 때마다 그 조절 뒤 너비로 옮긴다. 조절 사건에서 이 값과
    /// 다른 패널만 사람이 끈 패널로 본다. `None`이면 상태를 새로 만들어 아직 그리지 않았다.
    baseline: Option<(f32, f32)>,
    /// 창 크기 바뀜 구독(패널 크기 상태를 새로 만든다).
    _window_resized: Subscription,
    /// 키 입력 관찰(되돌리기 묶음 끊기, S3-T2).
    _keystrokes: Subscription,
    /// 음 키 대기의 키 가로채기(S3-T3b).
    _pitch_keys: Subscription,
    /// 누른 채 `Esc`로 마우스 조작 취소(S3-T6a). 다른 가로채기보다 먼저 단다(`Esc`는 가장 안쪽 하나만, 5.5).
    _mouse_keys: Subscription,
    /// vim 오퍼레이터 대기·`y` 접두의 키 가로채기(S3-T3c).
    _vim_keys: Subscription,
    /// GPUI 대기 입력 관찰(붙든 키 컨텍스트 경로를 놓는다, S3-T3c 리뷰 3).
    _pending_input: Subscription,
    /// 이 뷰를 담은 창(파일 흐름이 시트를 띄우고 창을 닫는다).
    handle: gpui_kit::AnyWindowHandle,
    /// 저장·닫기 흐름이 돌고 있다(시트가 떠 있을 수 있다). 그동안 새 흐름을 시작하지 않는다(S2-T7a).
    pub(crate) file_flow: bool,
    /// 자동 저장본 상태(S2-T7b).
    pub(crate) autosave: crate::autosave::WindowAutosave,
    /// 피아노롤 커서·뷰포트·선택(S3-T1).
    pub(crate) pianoroll: crate::pianoroll::PianoRollState,
    /// 음 키 대기·레가토 기억·마지막 편집(S3-T3b).
    pub(crate) note_ops: crate::note_ops::NoteOpsState,
    pub(crate) vim: crate::vim::VimState,
    /// Step 입력 길이·옥타브·조합(S3-T4a).
    pub(crate) step: crate::step::StepState,
}

impl ProjectWindow {
    pub fn new(
        document: Entity<ProjectDocument>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (horizontal, vertical, resize_subscriptions) = Self::fresh_states(cx);
        // 창 안에 포커스가 없으면 Action이 루트(`Root`)에서 디스패치되어 이 뷰의 처리기에 닿지 않는다
        // (팝업·패널이 닫히며 포커스한 요소가 사라질 때 등). 포커스를 잃으면 창의 루트
        // 핸들로 되돌린다(Zed Workspace와 같은 방식, S2-T1 리뷰 1 지적 3).
        let focus_lost = cx.on_focus_lost(window, |this, window, cx| {
            window.focus(&this.focus, cx);
        });
        let focus = cx.focus_handle();
        let document_changed = cx.observe_in(&document, window, |this, _, window, cx| {
            this.keep_current_track(cx);
            this.selection_document_changed(cx);
            this.sync_fields(window, cx);
            this.transport_document_changed(cx);
            this.notice_history_limit(cx);
            this.refresh_file_state(window, cx);
            this.autosave_document_changed(cx);
        });
        let current_track = document
            .read(cx)
            .project()
            .tracks()
            .first()
            .map(keyscore_core::Track::id);
        let transport_bar = TransportBar::new();
        let fields = crate::fields::Fields::new(window, cx);
        let inspector = crate::inspector::Inspector::new(window, cx);
        let released = cx.on_release(|_, cx| crate::transport::release(cx));
        // 창 크기가 바뀌면 gpui-base가 모든 패널을 비율로 늘리거나 줄인다. 사이드·아래 패널은 저장된 크기를 지키고
        // 메인 영역이 늘고 주는 것이 맞다(7.1). 크기 상태를 새로 만들면 패널은 저장된 크기로, 메인은 나머지로
        // 그려지고, 다음 조절이 비율로 줄어든 값을 저장하지 않는다(S2-T1 리뷰 3 제안 1).
        let window_resized = cx.observe_window_bounds(window, |this, _, cx| {
            this.reset_panel_states(cx);
        });
        // 되돌리기 묶음(5.7): 같은 조작의 키가 아닌 키 입력이 끼면 묶음을 끊는다. 메뉴·팔레트 실행은 키 입력이
        // 아니라 여기 오지 않는다. 다른 창의 키는 그 창의 일이다(S3-T2).
        let keystrokes = cx.observe_keystrokes(|this, event, window, cx| {
            if window.window_handle() != this.handle {
                return;
            }
            let same_op = event
                .action
                .as_deref()
                .is_some_and(crate::selection::is_coalescing_action);
            if !same_op {
                this.seal_history(cx);
            }
            this.vim_after_keystroke(event.action.as_deref(), cx);
        });
        let mouse_keys = Self::intercept_mouse_keys(window, cx);
        let pitch_keys = Self::intercept_pitch_keys(window, cx);
        let vim_keys = Self::intercept_vim_keys(window, cx);
        let pending_input = cx.observe_pending_input(window, Self::vim_pending_input_changed);
        let me = cx.entity().downgrade();
        if cx.has_global::<OpenWindows>() {
            let list = &mut cx.global_mut::<OpenWindows>().0;
            list.retain(|w| w.upgrade().is_some());
            list.push(me);
        } else {
            cx.set_global(OpenWindows(vec![me]));
        }
        let mut this = ProjectWindow {
            document,
            layout: AppSettings::get(cx).layout,
            focus,
            horizontal,
            vertical,
            resize_subscriptions,
            _focus_lost: focus_lost,
            mode: ModeState::INITIAL,
            popup: None,
            indicator_bounds: Rc::new(Cell::new(Bounds::default())),
            current_track,
            current_index: 0,
            transport_bar,
            fields,
            inspector,
            overlay: None,
            quantize_sheet: None,
            palette: None,
            transport: Transport::default(),
            history_notice_shown: false,
            preview_notice_shown: false,
            _document_changed: document_changed,
            _released: released,
            baseline: None,
            _window_resized: window_resized,
            _keystrokes: keystrokes,
            _pitch_keys: pitch_keys,
            _mouse_keys: mouse_keys,
            _vim_keys: vim_keys,
            _pending_input: pending_input,
            handle: window.window_handle(),
            file_flow: false,
            autosave: crate::autosave::WindowAutosave::new(),
            pianoroll: crate::pianoroll::PianoRollState::new(),
            note_ops: crate::note_ops::NoteOpsState::default(),
            vim: crate::vim::VimState::default(),
            step: crate::step::StepState::new(AppSettings::get(cx).input.step_default_length),
        };
        this.refresh_fields(window, cx);
        this.refresh_file_state(window, cx);
        // 창 닫기 단추와 `window::Close`는 같은 확인을 거친다. 바뀐 문서는 시트로 묻고, 닫을 때 재생을 멈춘다
        // (S2-T5a 이월: 뷰 해제를 기다리지 않는다).
        let me = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            me.upgrade()
                .is_none_or(|view| crate::files::request_close(&view, cx))
        });
        this
    }

    /// 패널 크기 상태를 새로 만든다. 새 상태는 각 패널의 저장된 크기로 시작한다.
    fn reset_panel_states(&mut self, cx: &mut Context<Self>) {
        let (horizontal, vertical, subscriptions) = Self::fresh_states(cx);
        self.horizontal = horizontal;
        self.vertical = vertical;
        self.resize_subscriptions = subscriptions;
        self.baseline = None;
        cx.notify();
    }

    /// 새 크기 상태 둘과 그 구독. 끌기와 프로그램 조절 모두 `Resized`를 낸다(gpui-base
    /// `resize_panel`). 그때 크기를 남긴다.
    fn fresh_states(
        cx: &mut Context<Self>,
    ) -> (
        Entity<ResizableState>,
        Entity<ResizableState>,
        [Subscription; 2],
    ) {
        let horizontal = cx.new(|_| ResizableState::default());
        let vertical = cx.new(|_| ResizableState::default());
        let subscriptions = [
            cx.subscribe(&horizontal, |this, state, _: &ResizablePanelEvent, cx| {
                this.record_sizes(Group::Horizontal, &sizes_of(&state, cx), cx);
            }),
            cx.subscribe(&vertical, |this, state, _: &ResizablePanelEvent, cx| {
                this.record_sizes(Group::Vertical, &sizes_of(&state, cx), cx);
            }),
        ];
        (horizontal, vertical, subscriptions)
    }

    /// 가로 패널 그룹의 크기 상태. 자식은 펴진 패널뿐이고 순서는 트랙 목록, 메인, 인스펙터다(접힌
    /// 패널은 빠진다).
    #[must_use]
    pub fn horizontal_state(&self) -> &Entity<ResizableState> {
        &self.horizontal
    }

    /// 세로 패널 그룹의 크기 상태. 위 영역, 그리고 펴져 있으면 하단 영역.
    #[must_use]
    pub fn vertical_state(&self) -> &Entity<ResizableState> {
        &self.vertical
    }

    #[must_use]
    pub fn document(&self) -> &Entity<ProjectDocument> {
        &self.document
    }

    /// 이 뷰를 담은 창.
    #[must_use]
    pub fn window_handle(&self) -> gpui_kit::AnyWindowHandle {
        self.handle
    }

    /// 모드 FSM의 현재 상태.
    #[must_use]
    pub const fn mode_state(&self) -> ModeState {
        self.mode
    }

    /// 모드 사건을 넣는다. Action 처리기와 이후 단계(선택 S3, 클립 끝 S5, 고스트 S7)가 부른다.
    pub fn apply_mode_event(
        &mut self,
        event: ModeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = self.mode.apply(event);
        // 모드를 나가면 조합 중인 입력은 현재 옥타브로 확정한다(REQ 5.6, A13).
        if self.mode.mode() == Mode::Step && next.mode() != Mode::Step {
            self.step_settle(cx);
            // 지나온 스텝의 자취는 이번 Step 입력의 것이다(리뷰 2 지적 1).
            self.step.clear_trail();
            // 덮어쓰기는 Step 진입마다 정한다(S3-T4b 결정 1절).
            self.step.overwrite = false;
        }
        if next != self.mode {
            // 오퍼레이터 대기와 비주얼은 Normal 피아노롤의 것이다. 모드·뷰가 바뀌면 끝낸다(S3-T3c). 비주얼만 끝내고
            // 선택·기준 칸은 FSM 규칙(아래)대로 둔다.
            self.clear_operator(cx);
            self.pianoroll.set_visual(None);
            // 마우스 조작도 Normal 피아노롤의 것이다. 모드·뷰가 바뀌면 버린다(뗌이 오지 않을 수 있다, S3-T6a 리뷰 1
            // 지적 1). 선택만 바뀐 FSM 사건(`Select`)은 러버밴드 자신이 내므로 버리지 않는다.
            if next.mode() != self.mode.mode() || next.view() != self.mode.view() {
                self.pianoroll.drop_gesture();
            }
            let entered_piano_roll =
                next.view() == View::PianoRoll && self.mode.view() != View::PianoRoll;
            let sequence_changed =
                (self.mode.mode() == Mode::Sequence) != (next.mode() == Mode::Sequence);
            self.mode = next;
            // Sequence 모드는 곡 끝에서 멈추지 않는다(S3-T3d). 재생 중이면 곡 끝 유무를 바꾼 스냅샷을 넣는다.
            if sequence_changed {
                self.transport_refresh(cx);
            }
            if entered_piano_roll {
                self.entered_piano_roll(cx);
            }
            // FSM이 선택을 비웠으면(뷰 전환, 선택 해제) 실제 선택도 비운다(S3-T2). 기준 칸만 남은 상태도 선택 중이라
            // 함께 지운다. 남기면 문서가 바뀔 때 노트 없는 `Selection` 컨텍스트가 다시 켜진다(리뷰 4 지적 3).
            if !next.has_selection()
                && (!self.pianoroll.selection().is_empty() || self.pianoroll.anchor().is_some())
            {
                self.pianoroll
                    .set_selection(crate::pianoroll::Selection::default());
                self.pianoroll.set_anchor(None);
            }
            // 입력란에서 전역 키(`ctrl-tab`)로 모드가 바뀌면 문서로 포커스를 옮긴다. 그대로 두면 상태줄은
            // 새 모드인데 키는 입력란 글자가 된다(리뷰 2 제안 1).
            if self.field_focused(window, cx) {
                self.cancel_open_fields(window, cx);
                window.focus(&self.focus, cx);
            }
            cx.notify();
        }
    }

    /// 피아노롤 상태(S3-T1).
    #[must_use]
    pub fn piano_roll(&self) -> &crate::pianoroll::PianoRollState {
        &self.pianoroll
    }

    /// 지금 선택(S3-T2).
    #[must_use]
    pub fn selection(&self) -> crate::pianoroll::Selection {
        self.pianoroll.selection()
    }

    /// 선택을 바꾸고 모드 FSM의 선택 유무를 맞춘다(S3-T2).
    pub fn set_selection(
        &mut self,
        selection: crate::pianoroll::Selection,
        cx: &mut Context<Self>,
    ) {
        self.pianoroll.set_selection(selection);
        self.sync_selection_mode(cx);
        cx.notify();
    }

    /// 선택 유무만 바꾸는 FSM 사건(`Select`·`ClearSelection`). 모드·뷰는 바뀌지 않아 입력란 처리가 없다.
    pub(crate) fn apply_selection_event(&mut self, event: ModeEvent) {
        self.mode = self.mode.apply(event);
    }

    /// 피아노롤 우클릭 메뉴가 떠 있는가(S3-T2).
    #[must_use]
    pub fn context_menu_open(&self) -> bool {
        self.popup
            .as_ref()
            .is_some_and(|p| p.kind == PopupKind::PianoRollMenu)
    }

    /// 피아노롤의 가로·세로 뷰포트. 인스펙터·레인이 같은 가로축을 그릴 때 읽는다(가로 동기 훅).
    #[must_use]
    pub fn piano_roll_viewport(&self) -> crate::pianoroll::Viewport {
        self.pianoroll.viewport()
    }

    /// `e`(음높이 바꾸기) 뒤 음 키를 기다리는 중인가(S3-T3b).
    #[must_use]
    pub const fn pitch_pending(&self) -> bool {
        self.note_ops.pitch_pending
    }

    /// 현재 트랙(S2-T5b). 트랙이 없으면 `None`.
    #[must_use]
    pub const fn current_track(&self) -> Option<keyscore_core::TrackId> {
        self.current_track
    }

    /// 마지막으로 연 포트 드롭다운의 선택지(가상 포트 먼저, 그다음 OS 목적지).
    #[must_use]
    pub fn port_choices(&self) -> Vec<keyscore_core::OutputPort> {
        self.transport_bar.port_choices.clone()
    }

    pub(crate) fn set_current_track(
        &mut self,
        track: Option<keyscore_core::TrackId>,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = track.and_then(|id| {
            self.document
                .read(cx)
                .project()
                .tracks()
                .iter()
                .position(|t| t.id() == id)
        }) {
            self.current_index = index;
        }
        if self.current_track != track {
            self.current_track = track;
            // 오퍼레이터 대기는 그 트랙의 커서에 걸린 것이다(S3-T3c)
            self.clear_operator(cx);
            cx.notify();
        }
    }

    /// 문서가 바뀐 뒤 현재 트랙이 사라졌으면(undo 등) 그 자리의 이웃(없으면 앞 트랙)으로, 트랙이 없으면
    /// 없음으로 둔다.
    fn keep_current_track(&mut self, cx: &mut Context<Self>) {
        let tracks = self.document.read(cx).project().tracks();
        if let Some(index) = self
            .current_track
            .and_then(|id| tracks.iter().position(|t| t.id() == id))
        {
            self.current_index = index;
        } else {
            let index = self.current_index.min(tracks.len().saturating_sub(1));
            self.current_track = tracks.get(index).map(keyscore_core::Track::id);
            self.current_index = index;
        }
    }

    /// 문서를 편집한다(undo 한 단위). 재생 중 반영(TRN-3)을 위해 문서를 알린다. 실패하면 메시지를 남기고
    /// `false`.
    pub(crate) fn edit_document(
        &mut self,
        edits: Vec<keyscore_core::Edit>,
        cx: &mut Context<Self>,
    ) -> bool {
        let result = self.document.update(cx, |doc, cx| {
            let result = doc.apply(edits);
            if result.is_ok() {
                cx.notify();
            }
            result
        });
        match result {
            Ok(()) => {
                cx.notify();
                true
            }
            Err(e) => {
                self.say(format!("편집하지 못했다: {e}"), cx);
                false
            }
        }
    }

    /// undo 기록이 상한을 넘어 오래된 단계를 처음 버렸으면 상태줄에 한 번 알린다(EDT-1, 결정
    /// `docs/decisions/2026-10-19-undo-history-memory-limit.md` A).
    fn notice_history_limit(&mut self, cx: &mut Context<Self>) {
        if self.history_notice_shown || self.document.read(cx).dropped_undo_units() == 0 {
            return;
        }
        self.history_notice_shown = true;
        self.say(
            format!(
                "되돌리기 기록이 메모리 상한({} MiB)을 넘어 가장 오래된 단계부터 버렸다",
                crate::document::HISTORY_LIMIT_BYTES / (1024 * 1024)
            ),
            cx,
        );
    }

    /// 창 메시지를 남긴다(상태줄 표시는 S2-T5c).
    pub(crate) fn say(
        &mut self,
        message: impl Into<gpui_kit::SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.transport.say(message);
        cx.notify();
    }

    /// undo·redo 전의 현재 트랙(뒤에 나타난 노트를 찾는다).
    fn current_track_snapshot(&self, cx: &Context<Self>) -> Option<keyscore_core::Track> {
        let id = self.current_track()?;
        self.document.read(cx).project().track(id).cloned()
    }

    /// undo·redo 뒤 선택(S3-T3a, S3-T2 리뷰 1 제안 8): 현재 트랙에 새로 나타난 노트가 모두 한 클립에 있으면 선택을
    /// 그 노트로 바꾼다(삭제를 되돌리면 되살아난 노트, 조작을 되돌리면 원래 노트). 아니면 문서 관찰자가 클립에 없는
    /// 노트만 뺀다.
    ///
    /// 피아노롤 뷰에서만 한다. 배치 뷰에서 만들면 화면에 없는 노트 선택이 생기고 `Selection` 컨텍스트가 켜진다
    /// (S3-T2: 뷰 전환이 노트 선택을 비운다, 리뷰 1 지적 3).
    fn select_appeared(&mut self, before: Option<keyscore_core::Track>, cx: &mut Context<Self>) {
        if self.mode.view() != View::PianoRoll {
            return;
        }
        let Some(before) = before else {
            return;
        };
        let found = {
            let doc = self.document.read(cx);
            doc.project()
                .track(before.id())
                .and_then(|after| crate::pianoroll::edit::appeared(&before, after))
        };
        if let Some((clip, notes)) = found {
            self.pianoroll.set_anchor(None);
            self.replace_selection(before.id(), clip, notes, cx);
        }
    }

    /// 포커스가 텍스트 입력란(gpui-component `Input` 컨텍스트) 안에 있는가.
    pub(crate) fn text_input_focused(window: &Window) -> bool {
        window.context_stack().iter().any(|c| c.contains("Input"))
    }

    /// 문서(모드 키 컨텍스트 안쪽)의 포커스 핸들.
    pub(crate) fn document_focus(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// 문서로 포커스를 돌린다(필드 확정 뒤).
    pub(crate) fn focus_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
    }

    fn edit_undo(
        &mut self,
        _: &crate::actions::edit::Undo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 포커스가 입력란(트랜스포트 필드, 명령 팔레트 질의, 이후 인스펙터)에 있으면 그 입력란을 되돌린다
        // (KM-14). 키로 왔든 메뉴로 왔든 입력란의 `input::Undo`로 보낸다(리뷰 3 지적 1·2).
        if Self::text_input_focused(window) {
            if let Ok(action) = cx.build_action("input::Undo", None) {
                window.dispatch_action(action, cx);
            }
            return;
        }
        // 조합 중이면 확정한 뒤 되돌린다(결정 2026-10-29-s3-t4-step-input-details 3절).
        self.step_settle(cx);
        let before = self.current_track_snapshot(cx);
        let result = self.document.update(cx, |doc, cx| {
            let r = doc.undo();
            cx.notify();
            r
        });
        match result {
            Ok(true) => self.select_appeared(before, cx),
            Ok(false) => {}
            Err(e) => self.say(format!("되돌리지 못했다: {e}"), cx),
        }
        cx.notify();
    }

    fn edit_redo(
        &mut self,
        _: &crate::actions::edit::Redo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 포커스가 입력란(트랜스포트 필드, 명령 팔레트 질의, 이후 인스펙터)에 있으면 그 입력란을 되돌린다
        // (KM-14). 키로 왔든 메뉴로 왔든 입력란의 `input::Redo`로 보낸다(리뷰 3 지적 1·2).
        if Self::text_input_focused(window) {
            if let Ok(action) = cx.build_action("input::Redo", None) {
                window.dispatch_action(action, cx);
            }
            return;
        }
        self.step_settle(cx);
        let before = self.current_track_snapshot(cx);
        let result = self.document.update(cx, |doc, cx| {
            let r = doc.redo();
            cx.notify();
            r
        });
        match result {
            Ok(true) => self.select_appeared(before, cx),
            Ok(false) => {}
            Err(e) => self.say(format!("다시 실행하지 못했다: {e}"), cx),
        }
        cx.notify();
    }

    /// 템포 입력란에 포커스가 있는가(입력 중).
    #[must_use]
    pub fn transport_tempo_focused(&self, window: &Window, cx: &App) -> bool {
        use gpui_kit::Focusable as _;
        self.field_state(crate::fields::Field::Tempo)
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
    }

    /// 트랜스포트 상태(S2-T5a).
    #[must_use]
    pub const fn transport_state(&self) -> crate::engine::TransportState {
        self.transport.state
    }

    /// 표시하는 재생 위치: 재생 중이면 지금 자리, 일시정지면 그 자리, 정지면 처음(S3-T3d).
    #[must_use]
    pub fn transport_position(&self) -> keyscore_core::Tick {
        self.transport.position()
    }

    /// 메트로놈이 켜져 있는가(TRN-2).
    #[must_use]
    pub const fn metronome_on(&self) -> bool {
        self.transport.metronome
    }

    /// 재생 중인데 메트로놈 포트를 찾지 못해 클릭이 나지 않는가(MET-1).
    #[must_use]
    pub const fn metronome_port_missing(&self) -> bool {
        self.transport.metronome_missing
    }

    /// 창이 남긴 메시지(엔진 없음, 없는 포트, MIDI 오류). 상태줄 표시는 S2-T5c다.
    #[must_use]
    pub fn messages(&self) -> &[gpui_kit::SharedString] {
        &self.transport.messages
    }

    /// 명령 팔레트가 열려 있는가.
    #[must_use]
    pub const fn palette_open(&self) -> bool {
        self.palette.is_some()
    }

    /// 열린 팔레트의 질의.
    #[must_use]
    pub fn palette_query(&self, cx: &App) -> Option<String> {
        self.palette.as_ref().map(|(p, _)| p.read(cx).query(cx))
    }

    /// 명령 팔레트를 연다(`palette::Open`, UI-2). 이미 열려 있으면 질의로 포커스만 돌린다.
    fn open_palette(&mut self, _: &palette::Open, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette_with(None, window, cx);
    }

    /// `:`(A12): 팔레트를 `:` 접두가 채워진 채 연다. 이어 치는 글자는 ex 명령이다.
    fn open_command_line(
        &mut self,
        _: &palette::OpenCommandLine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_palette_with(Some(":"), window, cx);
    }

    fn open_palette_with(
        &mut self,
        initial: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((palette, _)) = &self.palette {
            palette.update(cx, |palette, cx| {
                if let Some(text) = initial {
                    palette.set_query_text(text, window, cx);
                }
                palette.focus_query(window, cx);
            });
            return;
        }
        // 팝업은 하나만 띄운다. 다른 팝업·키맵 오버레이가 열려 있으면 닫는다(포커스는 곧 팔레트로 간다,
        // S2-T6b 리뷰 1 지적 1).
        self.popup = None;
        self.overlay = None;
        self.quantize_sheet = None;
        let target = self.focus.clone();
        let palette = cx.new(|cx| CommandPalette::new(target, window, cx));
        let events = cx.subscribe_in(&palette, window, |this, _, event, window, cx| {
            this.close_palette(window, cx);
            match event {
                PaletteEvent::Run(action) => run_action(action.boxed_clone(), window, cx),
                PaletteEvent::Ex(cmd) => this.run_ex(cmd.clone(), window, cx),
                PaletteEvent::Close => {}
            }
        });
        palette.update(cx, |palette, cx| {
            if let Some(text) = initial {
                palette.set_query_text(text, window, cx);
            }
            palette.focus_query(window, cx);
        });
        self.palette = Some((palette, events));
        cx.notify();
    }

    /// ex 명령(A12, REQ 7.5 vim 대응 열)을 실행한다. 인자 없는 명령은 같은 Action을 실행 함수로 낸다.
    fn run_ex(&mut self, cmd: ExCommand, window: &mut Window, cx: &mut Context<Self>) {
        use crate::actions::args::Opt;
        use crate::actions::{file, settings, window as win};
        match cmd {
            ExCommand::Write(None) => run_action(Box::new(file::Save), window, cx),
            ExCommand::Write(Some(name)) => self.file_save_as_named(name, cx),
            ExCommand::Quit => run_action(Box::new(win::Close), window, cx),
            ExCommand::Edit(None) => run_action(Box::new(file::Open), window, cx),
            ExCommand::Edit(Some(path)) => self.open_ex_path(&path, cx),
            ExCommand::New => run_action(Box::new(file::New), window, cx),
            ExCommand::Set(section) => {
                run_action(Box::new(settings::Open(Opt(section))), window, cx);
            }
        }
    }

    /// 팔레트를 닫고 문서로 포커스를 돌린다. 실행할 Action은 그 뒤에 문서 포커스로 디스패치된다.
    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((palette, _)) = self.palette.take() {
            // 포커스가 아직 팔레트에 있을 때만 문서로 돌린다. 팔레트 밖을 눌러 닫혔고 그 클릭이 다른
            // 팝업(상태줄 모드 메뉴)을 열었다면 포커스는 그 팝업의 것이다(리뷰 2 지적 2).
            let focused_inside = palette
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx);
            if focused_inside || window.focused(cx).is_none() {
                window.focus(&self.focus, cx);
            }
            cx.notify();
        }
    }

    /// 열린 팔레트. 모드 메뉴처럼 컨텍스트 겹 바깥에 띄운다. 문서 컨텍스트 키가 팔레트 입력에 섞이지
    /// 않게 하기 위해서다(S2-T3 리뷰 1 지적 2와 같은 이유).
    fn palette_overlay(&self, window: &Window) -> Option<impl IntoElement> {
        let (palette, _) = self.palette.as_ref()?;
        let left = ((window.viewport_size().width - PALETTE_WIDTH) / 2.).max(px(0.));
        Some(deferred(
            anchored()
                .position(point(left, PALETTE_TOP))
                .child(palette.clone()),
        ))
    }

    /// 상태줄 모드 메뉴가 열려 있는가.
    #[must_use]
    pub fn mode_menu_open(&self) -> bool {
        self.popup
            .as_ref()
            .is_some_and(|p| p.kind == PopupKind::Mode)
    }

    /// 팝업 메뉴를 연다. 팝업은 하나만 띄운다: 팔레트와 다른 팝업을 닫는다(포커스는 곧 새 메뉴로 간다).
    /// 위치는 `at`(창 좌표)이고, 모드 메뉴는 상태줄 위로(아래쪽 기준), 나머지는 그 점 아래로 연다.
    pub(crate) fn open_popup(
        &mut self,
        kind: PopupKind,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
        build: impl FnOnce(PopupMenu) -> PopupMenu + 'static,
    ) {
        self.open_popup_with(kind, at, window, cx, |menu, _, _| build(menu));
    }

    /// [`Self::open_popup`]과 같되 하위 메뉴를 만들 수 있게 창·컨텍스트를 넘긴다(S3-T2 마크 메뉴).
    pub(crate) fn open_popup_with(
        &mut self,
        kind: PopupKind,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
        build: impl FnOnce(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) {
        self.palette = None;
        self.overlay = None;
        self.quantize_sheet = None;
        let menu = PopupMenu::build(window, cx, build);
        let dismissed = cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, window, cx| {
            this.popup = None;
            window.focus(&this.focus, cx);
            cx.notify();
        });
        window.focus(&menu.focus_handle(cx), cx);
        self.popup = Some(Popup {
            kind,
            menu,
            at,
            _dismissed: dismissed,
        });
        cx.notify();
    }

    /// 상태줄 모드 메뉴를 연다(7.6 「모드 전환」: 모드 표시 클릭, `mode::ShowMenu`).
    fn open_mode_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.mode.mode();
        // 위치는 그릴 때마다 모드 표시에서 다시 구한다(`popup_overlay`)
        let at = self.indicator_bounds.get().origin;
        self.open_popup(PopupKind::Mode, at, window, cx, move |mut menu| {
            for mode in Mode::MENU {
                menu = menu.item(
                    PopupMenuItem::new(mode.label())
                        .checked(mode == current)
                        .on_click(move |_, window, cx| {
                            // 키가 아닌 실행 경로는 실행 함수 하나를 거친다(결정 C, 카운트를 비운다).
                            run_action(mode_action(mode), window, cx);
                        }),
                );
            }
            menu
        });
    }

    fn show_mode_menu(&mut self, _: &mode::ShowMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.open_mode_menu(window, cx);
    }

    /// Action `A`가 오면 사건 `event`를 넣는 처리기를 단다.
    fn on_mode<A: Action>(
        element: Stateful<Div>,
        event: ModeEvent,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element.on_action(cx.listener(move |this, _: &A, window, cx| {
            this.apply_mode_event(event, window, cx);
        }))
    }

    /// 모드 FSM에 사건을 넣는 Action 처리기들(전이 표: `docs/reports/s2-t3/plan.md`).
    fn mode_handlers(element: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        use ModeEvent::{
            Cycle, CycleBack, DismissGhost, Enter, LeaveAi, OpenClip, ToNormal, ToggleView,
        };
        let e = element;
        let e = Self::on_mode::<clip::Open>(e, OpenClip, cx);
        let e = Self::on_mode::<view::Toggle>(e, ToggleView, cx);
        // `mode::Step`·`StepAfterNote`·`StepAtBarStart`·`StepAtBarEnd`·`StepOverwrite`는 진입 위치를 정하는
        // 처리기가 `step_handlers`에 있다(S3-T4b).
        let e = Self::on_mode::<mode::Sequence>(e, Enter(Mode::Sequence), cx);
        let e = Self::on_mode::<mode::Play>(e, Enter(Mode::Play), cx);
        let e = Self::on_mode::<mode::Lyric>(e, Enter(Mode::Lyric), cx);
        let e = Self::on_mode::<mode::Controller>(e, Enter(Mode::Controller), cx);
        let e = Self::on_mode::<mode::Cycle>(e, Cycle, cx);
        let e = Self::on_mode::<mode::CycleBack>(e, CycleBack, cx);
        let e = Self::on_mode::<mode::Normal>(e, ToNormal, cx);
        let e = Self::on_mode::<ai::OpenPanel>(e, Enter(Mode::Ai), cx);
        let e = Self::on_mode::<ai::Accept>(e, LeaveAi, cx);
        let e = Self::on_mode::<ai::Dismiss>(e, LeaveAi, cx);
        let e = Self::on_mode::<ai::DismissGhost>(e, DismissGhost, cx);
        let e = Self::piano_roll_handlers(e, cx);
        let e = Self::step_handlers(e, cx);
        e.on_action(cx.listener(Self::show_mode_menu))
            .on_action(cx.listener(Self::transport_play))
            .on_action(cx.listener(Self::transport_play_from_cursor))
            .on_action(cx.listener(Self::transport_toggle))
            .on_action(cx.listener(Self::transport_stop))
            .on_action(cx.listener(Self::transport_pause))
            .on_action(cx.listener(Self::transport_panic))
            .on_action(cx.listener(Self::transport_toggle_metronome))
            .on_action(cx.listener(Self::transport_set_tempo))
            .on_action(cx.listener(Self::transport_tap_tempo))
            .on_action(cx.listener(Self::track_new))
            .on_action(cx.listener(Self::track_new_above))
            .on_action(cx.listener(Self::track_new_below))
            .on_action(cx.listener(Self::track_delete))
            .on_action(cx.listener(Self::track_toggle_mute))
            .on_action(cx.listener(Self::track_toggle_solo))
            .on_action(cx.listener(Self::track_move_up))
            .on_action(cx.listener(Self::track_move_down))
            .on_action(cx.listener(Self::track_set_channel))
            .on_action(cx.listener(Self::track_rename))
            .on_action(cx.listener(Self::track_set_color))
            .on_action(cx.listener(Self::track_set_port))
            .on_action(cx.listener(Self::track_set_program))
            .on_action(cx.listener(Self::track_set_default_velocity))
            .on_action(cx.listener(Self::track_set_lyric_lang))
            .on_action(cx.listener(Self::midi_send_test_note))
            .on_action(cx.listener(Self::midi_map_missing_ports))
            .on_action(cx.listener(Self::track_next))
            .on_action(cx.listener(Self::track_prev))
            .on_action(cx.listener(Self::track_first))
            .on_action(cx.listener(Self::track_last))
            .on_action(cx.listener(Self::edit_undo))
            .on_action(cx.listener(Self::edit_redo))
            .on_action(cx.listener(Self::open_palette))
            .on_action(cx.listener(Self::open_command_line))
            .on_action(cx.listener(Self::help_keymap_overlay))
            .on_action(cx.listener(Self::open_quantize_sheet))
            .on_action(cx.listener(Self::file_save))
            .on_action(cx.listener(Self::file_save_as))
            .on_action(cx.listener(Self::window_close))
            .on_action(cx.listener(Self::file_import_smf))
            .on_action(cx.listener(Self::file_export_smf))
            .on_action(cx.listener(Self::file_import_text))
            .on_action(cx.listener(Self::file_export_text))
            // 맨 키를 피하려고 감싼 메뉴 항목(`commands::RunCommand`). 실행 함수로 실제 Action을 낸다.
            .on_action(cx.listener(|_, run: &RunCommand, window, cx| {
                if let Ok(action) = cx.build_action(&run.name, run.args.clone()) {
                    run_action(action, window, cx);
                }
            }))
    }

    /// 키 컨텍스트 경로대로 `div`를 겹친다. GPUI는 요소 하나에 컨텍스트 하나를 쌓아 깊이를 만들고,
    /// `Normal > PianoRoll` 같은 술어는 그 깊이로 맞춘다. 가장 안쪽이 창 포커스를 잡아야 경로 전체가
    /// 디스패치 경로가 된다.
    fn wrap_in_contexts(&self, content: AnyElement, window: &Window) -> AnyElement {
        let path = self.key_context_path(window);
        let (innermost, outer) = path.split_last().expect("컨텍스트 경로는 비지 않는다");
        let mut element = div()
            .size_full()
            .key_context(*innermost)
            .track_focus(&self.focus)
            .child(content)
            .into_any_element();
        for context in outer.iter().rev() {
            element = div()
                .size_full()
                .key_context(*context)
                .child(element)
                .into_any_element();
        }
        element
    }

    /// 끄는 중 수식키만 바꿔도(`Cmd` 스냅 해제·`Shift` 한 축·`Alt` 복제) 고스트를 다시 계산한다(S3-T6b). 수식키
    /// 사건은 포커스 경로로 오므로 피아노롤이 아니라 바깥 요소에 단다.
    fn watch_modifiers(root: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        root.on_modifiers_changed(cx.listener(
            |this, e: &gpui_kit::ModifiersChangedEvent, _, cx| {
                this.piano_roll_modifiers(e.modifiers, cx);
            },
        ))
    }

    /// gpui-component `Root`의 Tab 포커스 이동(`root::Tab`, `root::TabPrev`)을 창 안에서 삼킨다. 우리
    /// 키맵에 `tab`이 없는 모드(Lyric 등)나 팝업에서 그 바인딩이 맞으면 창 포커스가 트랜스포트 입력란으로
    /// 넘어가 모든 모드 키가 끊긴다(S2-T3 리뷰 1 제안 2가 경고한 일). 다음 필드로의 `Tab`(5.11)은
    /// 대화상자·설정 화면의 일이고, 그 화면은 이 창 밖(Root 레이어)에 뜬다. 타입이 공개되지 않아 이름으로
    /// 만든다.
    fn swallow_root_tab(root: Stateful<Div>, cx: &App) -> Stateful<Div> {
        ["root::Tab", "root::TabPrev"]
            .iter()
            .filter_map(|name| cx.build_action(name, None).ok())
            .fold(root, |el, action| {
                el.on_boxed_action(action.as_ref(), |_, _, _| {})
            })
    }

    /// 상태줄 왼쪽의 모드 표시. 누르면 모드 메뉴가 열린다.
    fn mode_indicator(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = self.mode_label();
        let bounds = self.indicator_bounds.clone();
        div()
            .id("status-mode")
            .role(Role::Button)
            .aria_label(label.clone())
            .test_support()
            .relative()
            .px_1()
            .cursor_pointer()
            .child(label)
            .child(
                gpui_kit::canvas(move |b, _, _| bounds.set(b), |_, (), _, _| {})
                    .absolute()
                    .inset_0(),
            )
            .on_click(cx.listener(|this, _, window, cx| this.open_mode_menu(window, cx)))
    }

    /// 겹 바깥 자리(트랜스포트 바·패널·상태줄)를 누르면(어느 버튼이든) 문서로 포커스를 옮기는 처리기. 겹이
    /// 창 전체를 감쌀 때 GPUI 포커스 요소(`track_focus`, 모든 버튼)가 하던 일이다(S2-T5c 리뷰 1 지적 1, 리뷰 2
    /// 지적 1·2).
    /// - 아무도 누름을 받지 않았으면(빈 자리) 문서로 옮긴다.
    /// - 안쪽이 받았고 포커스가 텍스트 입력란에 있으면 그대로 둔다. 입력란을 누른 것이다. 버튼의 왼쪽 누름도
    ///   여기에 든다(gpui-component 버튼은 포커스를 받지 않고 `prevent_default`만 한다). 그 뒤는 버튼의 클릭
    ///   처리기가 문서로 돌린다.
    /// - 안쪽이 받았는데 포커스가 입력란이 아니면 문서로 되돌린다. 오른쪽 누름에서는 버튼·토글이 포커스를
    ///   가져가고, 겹 바깥 버튼에 포커스가 남으면 모드 키가 모두 끊긴다(리뷰 3 지적 1).
    fn focus_document_on_press(
        cx: &mut Context<Self>,
    ) -> impl Fn(&gpui_kit::MouseDownEvent, &mut Window, &mut App) + 'static {
        cx.listener(|this, _, window, cx| {
            if window.default_prevented() && Self::text_input_focused(window) {
                return;
            }
            this.focus_document(window, cx);
            window.prevent_default();
        })
    }

    /// 상태줄(UI-5, 7.1 아래): 왼쪽 모드, 가운데 현재 트랙·포트·채널, 오른쪽 키맵 오류(KM-6) 또는 마지막
    /// 메시지. 길이 코드·옥타브는 Step 입력(S5)과 함께 넣는다.
    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let track = self
            .current_track()
            .map(|id| {
                crate::tracks::status_track_text(
                    self.document.read(cx).project(),
                    id,
                    &crate::engine::virtual_port_name(cx),
                )
            })
            .unwrap_or_default();
        // 현재 트랙의 이름 포트가 지금 없으면 알린다(OUT-3). 감시가 없으면 모른다.
        let port_missing = self
            .current_track()
            .and_then(|id| {
                self.document
                    .read(cx)
                    .project()
                    .tracks()
                    .iter()
                    .find(|t| t.id() == id)
            })
            .and_then(|t| match t.port() {
                keyscore_core::OutputPort::Named(name) => crate::ports::port_present(cx, name),
                keyscore_core::OutputPort::Virtual => None,
            })
            == Some(false);
        let message = self
            .messages()
            .last()
            .map(ToString::to_string)
            .unwrap_or_default();
        let keymap_error = crate::keymap::errors(cx)
            .first()
            .map(|e| format!("키맵 오류 {}", crate::keymap::describe(e)));
        let right = if let Some(error) = keymap_error {
            div()
                .id("status-keymap-error")
                .aria_label(error.clone())
                .test_support()
                .text_color(cx.theme().warning)
                .child(error)
        } else {
            div()
                .id("status-message")
                .aria_label(message.clone())
                .test_support()
                .text_color(cx.theme().muted_foreground)
                .child(message)
        };
        div()
            .id("status-bar")
            .test_support()
            .flex_none()
            .on_any_mouse_down(Self::focus_document_on_press(cx))
            .child(
                StatusBar::new()
                    .left(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(self.mode_indicator(cx))
                            .child(
                                div()
                                    .id("status-track")
                                    .aria_label(track.clone())
                                    .test_support()
                                    .child(track),
                            )
                            .children(port_missing.then(|| {
                                div()
                                    .id("status-port-missing")
                                    .aria_label("포트 없음")
                                    .test_support()
                                    .text_color(cx.theme().warning)
                                    .child("포트 없음")
                            })),
                    )
                    .right(right),
            )
    }

    /// 열린 모드 메뉴. 컨텍스트 겹 바깥(루트의 자식)에 그린다. 안쪽에 그리면 메뉴 포커스의 디스패치
    /// 경로에 문서 컨텍스트(`Normal > Arrangement` 등)가 들어가, 메뉴가 걸지 않은 키(`v`, `a`)가 메뉴
    /// 뒤의 문서로 간다(S2-T3 리뷰 1 지적 2). 위치는 상태줄 왼쪽 모드 표시 위다.
    fn mode_menu_overlay(&self, _: &Window) -> Option<impl IntoElement> {
        let popup = self.popup.as_ref()?;
        // 모드 메뉴는 모드 표시 바로 위에 아래쪽을 맞춘다. 창 크기가 바뀌어도 따라간다.
        let (at, corner) = if popup.kind == PopupKind::Mode {
            (self.indicator_bounds.get().origin, Anchor::BottomLeft)
        } else {
            (popup.at, Anchor::TopLeft)
        };
        Some(deferred(
            anchored()
                .position(at)
                .anchor(corner)
                .child(popup.menu.clone()),
        ))
    }

    #[must_use]
    pub const fn layout(&self) -> LayoutSettings {
        self.layout
    }

    fn toggle_track_list(&mut self, _: &ToggleTrackList, _: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(cx, |l| l.track_list_open = !l.track_list_open);
    }

    fn toggle_inspector(&mut self, _: &ToggleInspector, _: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(cx, |l| l.inspector_open = !l.inspector_open);
    }

    fn toggle_bottom(&mut self, _: &ToggleBottomPanel, _: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(cx, |l| l.bottom_open = !l.bottom_open);
    }

    /// 이 창의 레이아웃을 바꾸고, 바뀐 항목만 앱 설정에 남긴다. 창마다 레이아웃 전체를 쓰면 한 창의
    /// 토글이 다른 창에서 바꾼 너비를 되돌린다(리뷰 1 제안 5). 다음에 여는 창은 설정으로 열린다.
    fn change_layout(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut LayoutSettings)) {
        let before = self.layout;
        edit(&mut self.layout);
        let after = self.layout;
        // 패널을 접거나 펴면 크기 상태를 새로 만든다. 그룹의 자식은 펴진 패널뿐이라(render) 자식 수가
        // 바뀌고, 새 상태는 각 패널의 저장된 크기로 시작한다. 숨은 패널을 상태에 남기면 gpui-base가
        // 그 크기를 합에 넣어 형제 패널의 크기가 틀어진다(리뷰 1 지적 2, 리뷰 2 지적 1).
        let shape_changed = before.track_list_open != after.track_list_open
            || before.inspector_open != after.inspector_open
            || before.bottom_open != after.bottom_open;
        if shape_changed {
            self.reset_panel_states(cx);
        }
        AppSettings::update(cx, |s| merge_changes(&mut s.layout, before, after));
        cx.notify();
    }

    /// 패널 그룹의 크기가 바뀌었다(사용자가 끌었다). 크기 목록은 자식 순서를 따른다. 접힌 패널이 있어
    /// 목록 길이가 다르면 그때는 저장하지 않는다(다음 조절에서 맞춘다).
    fn record_sizes(&mut self, group: Group, sizes: &[f32], cx: &mut Context<Self>) {
        let keep = |v: f32| PANEL_SIZE_RANGE.contains(&v);
        let l = self.layout;
        // 그룹의 자식은 펴진 패널뿐이다(접힌 패널은 크기 상태에 없다). 자식 순서대로 인덱스를 매긴다.
        match group {
            Group::Horizontal => {
                let expected = 1 + usize::from(l.track_list_open) + usize::from(l.inspector_open);
                if sizes.len() != expected {
                    return;
                }
                // 사람이 끈 패널만 저장한다. 창이 좁아 줄여 그린 다른 패널 값을 저장하면 안 된다(S2-T6f 리뷰 1
                // 지적 1). 그릴 때 요청한 너비와 다른 패널이 끈 패널이다.
                // 메인이 최소 너비에 닿은 채 한쪽 경계를 끌면 gpui-base가 반대쪽 패널을 밀어낸다. 둘 다 바뀌었으면
                // 더 많이 바뀐 쪽이 끈 패널이고 밀린 쪽은 저장하지 않는다.
                let last = sizes.len() - 1;
                let (asked_track, asked_inspector) =
                    self.baseline.unwrap_or((sizes[0], sizes[last]));
                let track_delta = if l.track_list_open {
                    (asked_track - sizes[0]).abs()
                } else {
                    0.0
                };
                let inspector_delta = if l.inspector_open {
                    (asked_inspector - sizes[last]).abs()
                } else {
                    0.0
                };
                let track = (track_delta > SIZE_EPSILON && track_delta >= inspector_delta)
                    .then(|| sizes[0]);
                let inspector = (inspector_delta > SIZE_EPSILON && inspector_delta > track_delta)
                    .then(|| sizes[last]);
                // 다음 조절의 기준은 이 조절 뒤 그린 너비다(줄여 그리는 동안 저장값과 다르다, 리뷰 2 지적 2)
                self.baseline = Some((sizes[0], sizes[last]));
                self.change_layout(cx, |l| {
                    if let Some(v) = track.filter(|&v| keep(v)) {
                        l.track_list_width = v;
                    }
                    if let Some(v) = inspector.filter(|&v| keep(v)) {
                        l.inspector_width = v;
                    }
                });
            }
            Group::Vertical => {
                if !l.bottom_open || sizes.len() != 2 {
                    return;
                }
                let bottom = sizes[1];
                if keep(bottom) {
                    self.change_layout(cx, |l| l.bottom_height = bottom);
                }
            }
        }
    }

    /// 자리표시자 영역. 내용은 S2-T5 이후 채운다.
    fn region(id: &'static str, label: &'static str, cx: &App) -> impl IntoElement {
        div()
            .id(id)
            .aria_label(label)
            .test_support()
            .size_full()
            .p_2()
            .text_color(cx.theme().muted_foreground)
            .child(label)
    }
}

impl Focusable for ProjectWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ProjectWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_step_layout(cx);
        let layout = self.layout;
        // 창이 저장된 사이드 패널 합보다 좁으면 우리가 먼저 비율로 줄여 요청한다(gpui-base가 줄이면 어느 패널을
        // 사람이 끌었는지 가릴 수 없다). 메인 영역에는 최소 크기를 남긴다.
        let (track_w, inspector_w) =
            fit_side_panels(layout, f32::from(window.viewport_size().width));
        // 새 상태는 요청한 너비로 그려진다(패널 최소보다 작게 요청하지 않는다). 그것이 첫 기준이다
        if self.baseline.is_none() {
            self.baseline = Some((track_w, inspector_w));
        }
        let range = px(*PANEL_SIZE_RANGE.start())..px(*PANEL_SIZE_RANGE.end());
        let transport_bar = self.render_transport_bar(window, cx).into_any_element();
        let track_list = self.render_track_list(cx).into_any_element();
        let inspector = self.render_inspector(window, cx).into_any_element();
        let status_bar = self.render_status_bar(cx).into_any_element();
        // 모드 키 컨텍스트 겹(포커스 핸들 포함)은 가운데 주 뷰만 감싼다. 키 디스패치는 포커스 자리에서
        // 시작하므로 모드 키는 그대로이고, 겹 바깥의 트랜스포트 바·인스펙터 입력란에 친 `space`·글자·숫자는
        // 모드 바인딩(재생 토글, vim 카운트)으로 빠지지 않는다(S2-T5b 리뷰 1 지적 1). 팔레트·팝업과 같은
        // 이유다.
        let main_view = match self.mode.view() {
            View::Arrangement => match self.arrangement_step(cx) {
                Some(shown) => Self::render_arrangement_step(shown, cx).into_any_element(),
                None => Self::region("main-view", "배치 뷰", cx).into_any_element(),
            },
            View::PianoRoll => self.render_piano_roll(window, cx).into_any_element(),
        };
        self.release_stale_prefix(window, cx);
        let main_view = self.wrap_in_contexts(main_view, window);
        let top = div()
            .id("top")
            .on_any_mouse_down(Self::focus_document_on_press(cx))
            .h(px(40.))
            .flex_none()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(transport_bar);
        // 겹 바깥 패널의 빈 자리를 눌러도 문서로 포커스가 온다(`focus_document_on_press`).
        let content = div()
            .id("content")
            .on_any_mouse_down(Self::focus_document_on_press(cx))
            .flex_1()
            .min_h_0()
            .child(
                v_resizable("main-vertical")
                    .with_state(&self.vertical)
                    .child(
                        resizable_panel().child(
                            h_resizable("main-horizontal")
                                .with_state(&self.horizontal)
                                .when(layout.track_list_open, |g| {
                                    g.child(
                                        resizable_panel()
                                            .size(px(track_w))
                                            .size_range(range.clone())
                                            .child(track_list),
                                    )
                                })
                                .child(resizable_panel().child(main_view))
                                .when(layout.inspector_open, |g| {
                                    g.child(
                                        resizable_panel()
                                            .size(px(inspector_w))
                                            .size_range(range.clone())
                                            .child(inspector),
                                    )
                                }),
                        ),
                    )
                    .when(layout.bottom_open, |g| {
                        g.child(
                            resizable_panel()
                                .size(px(layout.bottom_height))
                                .size_range(range.clone())
                                .child(match self.keyboard_view(cx) {
                                    // 하단 영역은 Step이면 건반 뷰, 그 밖에는 컨트롤러 레인 자리(S6)다(REQ 7.1)
                                    Some(caps) => {
                                        self.render_keyboard_view(&caps, cx).into_any_element()
                                    }
                                    None => {
                                        Self::region("bottom-panel", "컨트롤러 레인 / 건반 뷰", cx)
                                            .into_any_element()
                                    }
                                }),
                        )
                    }),
            );
        // 처리기는 바깥 요소에 단다. 디스패치는 포커스(가장 안쪽)에서 바깥으로 올라온다.
        let root = div()
            .id("project-window")
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::toggle_track_list))
            .on_action(cx.listener(Self::toggle_inspector))
            .on_action(cx.listener(Self::toggle_bottom));
        let root = Self::watch_modifiers(Self::swallow_root_tab(root, cx), cx);
        Self::mode_handlers(root, cx)
            .child(top)
            .child(content)
            .child(status_bar)
            .children(self.mode_menu_overlay(window))
            .children(self.palette_overlay(window))
            .children(self.render_overlay(cx))
            .children(self.render_quantize_sheet(cx))
    }
}

/// 창 너비에 맞춘 사이드 패널 너비(트랙 목록, 인스펙터). 펴진 패널 합과 메인 최소 너비가 창보다 크면 둘을 같은
/// 비율로 줄인다. 저장값은 바꾸지 않는다.
fn fit_side_panels(layout: LayoutSettings, width: f32) -> (f32, f32) {
    let min = *PANEL_SIZE_RANGE.start();
    let track = if layout.track_list_open {
        layout.track_list_width
    } else {
        0.0
    };
    let inspector = if layout.inspector_open {
        layout.inspector_width
    } else {
        0.0
    };
    let room = (width - MAIN_MIN_WIDTH).max(0.0);
    let total = track + inspector;
    if total <= room || total <= 0.0 || width <= 0.0 {
        return (layout.track_list_width, layout.inspector_width);
    }
    // 같은 비율로 줄이되 펴진 패널은 패널 최소(gpui-base가 그 아래로 그리지 않는다)보다 작게 요청하지 않는다.
    // 한쪽이 최소에 걸리면 남는 자리를 다른 쪽에 준다(리뷰 2 지적 1)
    let scale = room / total;
    let (mut t, mut i) = (track * scale, inspector * scale);
    if layout.track_list_open && t < min {
        t = min;
        if layout.inspector_open {
            i = (room - t).max(min);
        }
    }
    if layout.inspector_open && i < min {
        i = min;
        if layout.track_list_open {
            t = (room - i).max(min);
        }
    }
    (
        if layout.track_list_open {
            t
        } else {
            layout.track_list_width
        },
        if layout.inspector_open {
            i
        } else {
            layout.inspector_width
        },
    )
}

/// 사이드 패널을 줄일 때 메인 영역에 남기는 너비(논리 픽셀).
const MAIN_MIN_WIDTH: f32 = 120.0;

/// 패널 크기를 같다고 볼 차이(논리 픽셀). 비율 계산의 부동소수 오차를 「조절」로 보지 않는다.
const SIZE_EPSILON: f32 = 0.5;

fn sizes_of(state: &Entity<ResizableState>, cx: &App) -> Vec<f32> {
    state
        .read(cx)
        .sizes()
        .iter()
        .map(|p| f32::from(*p))
        .collect()
}

/// `before`에서 `after`로 바뀐 항목만 `target`에 옮긴다.
// 「바뀌었는가」를 정확히 본다. 같은 값을 다시 쓴 것은 바뀐 것이 아니다.
#[allow(clippy::float_cmp)]
fn merge_changes(target: &mut LayoutSettings, before: LayoutSettings, after: LayoutSettings) {
    macro_rules! copy_changed {
        ($($field:ident),*) => {
            $(
                if before.$field != after.$field {
                    target.$field = after.$field;
                }
            )*
        };
    }
    copy_changed!(
        track_list_open,
        track_list_width,
        inspector_open,
        inspector_width,
        bottom_open,
        bottom_height
    );
}

/// 상태줄 모드 메뉴 항목이 실행하는 Action. 키·팔레트와 같은 처리기를 거친다.
fn mode_action(mode: Mode) -> Box<dyn Action> {
    match mode {
        Mode::Normal => Box::new(mode::Normal),
        Mode::Step => Box::new(mode::Step),
        Mode::Sequence => Box::new(mode::Sequence),
        Mode::Play => Box::new(mode::Play),
        Mode::Ai => Box::new(ai::OpenPanel),
        Mode::Lyric => Box::new(mode::Lyric),
        Mode::Controller => Box::new(mode::Controller),
    }
}

/// 열린 프로젝트 창(포트 감시가 창마다 알린다).
struct OpenWindows(Vec<gpui_kit::WeakEntity<ProjectWindow>>);

impl gpui_kit::Global for OpenWindows {}

/// 열린 프로젝트 창들.
pub(crate) fn project_windows(cx: &App) -> Vec<Entity<ProjectWindow>> {
    if !cx.has_global::<OpenWindows>() {
        return Vec::new();
    }
    cx.global::<OpenWindows>()
        .0
        .iter()
        .filter_map(gpui_kit::WeakEntity::upgrade)
        .collect()
}

/// 열린 프로젝트 창들(시험과 파일 흐름이 쓴다).
#[must_use]
pub fn open_project_windows(cx: &App) -> Vec<Entity<ProjectWindow>> {
    project_windows(cx)
}

/// 열린 모든 창에 메시지를 남긴다.
pub(crate) fn say_everywhere(cx: &mut App, message: &str) {
    for window in project_windows(cx) {
        window.update(cx, |w, cx| w.say(message.to_owned(), cx));
    }
}

/// 팝업 메뉴의 종류.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PopupKind {
    /// 상태줄 모드 메뉴
    Mode,
    /// 트랜스포트의 조성 메뉴
    Key,
    /// 출력 포트 메뉴(트랜스포트·인스펙터)
    Port,
    /// 인스펙터의 색 메뉴
    Color,
    /// 인스펙터의 채널 메뉴
    Channel,
    /// 인스펙터의 가사 언어 메뉴
    LyricLang,
    /// 피아노롤 우클릭 메뉴(마크, S3-T2)
    PianoRollMenu,
}

/// 열린 팝업 메뉴.
pub(crate) struct Popup {
    kind: PopupKind,
    menu: Entity<PopupMenu>,
    at: Point<Pixels>,
    _dismissed: Subscription,
}
