//! 모드 FSM(REQ P2, 5.5, 5.11). GPUI와 무관한 순수 상태 기계다.
//!
//! 창(`window.rs`)은 이 상태로 키 컨텍스트와 상태줄을 그리고, Action 처리기는 사건을 넣는다.
//! 선택·고스트는 유무만 다룬다. 실제 내용은 선택(S3)과 AI 어시스트(S7)가 채운다.

/// Normal 모드가 오가는 두 뷰(5.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum View {
    /// 클립을 블록으로 다루는 배치 뷰
    Arrangement,
    /// 클립 안을 편집하는 피아노롤 뷰
    PianoRoll,
}

/// 입력 모드.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Normal,
    Step,
    Sequence,
    Play,
    Ai,
    /// 가사 모드(5.15)
    Lyric,
    /// 컨트롤러 레인 전체 화면(5.9)
    Controller,
}

impl Mode {
    /// 모든 모드.
    pub const ALL: [Mode; 7] = [
        Mode::Normal,
        Mode::Step,
        Mode::Sequence,
        Mode::Play,
        Mode::Ai,
        Mode::Lyric,
        Mode::Controller,
    ];

    /// 상태줄 모드 메뉴의 항목. P2의 다섯 모드다.
    pub const MENU: [Mode; 5] = [
        Mode::Normal,
        Mode::Step,
        Mode::Sequence,
        Mode::Play,
        Mode::Ai,
    ];

    /// 상태줄에 보이는 이름.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Step => "STEP",
            Mode::Sequence => "SEQUENCE",
            Mode::Play => "PLAY",
            Mode::Ai => "AI",
            Mode::Lyric => "LYRIC",
            Mode::Controller => "CONTROLLER",
        }
    }

    /// 노트를 쓰는 입력 모드. 피아노롤에서만 일어난다.
    const fn writes_notes(self) -> bool {
        matches!(self, Mode::Step | Mode::Sequence | Mode::Lyric)
    }

    /// 순환(5.11)에서의 다음 모드. 순환 밖 모드(AI, Lyric, Controller)는 Normal처럼 센다.
    const fn cycled(self, forward: bool) -> Mode {
        const ORDER: [Mode; 4] = [Mode::Normal, Mode::Step, Mode::Sequence, Mode::Play];
        let at = match self {
            Mode::Step => 1,
            Mode::Sequence => 2,
            Mode::Play => 3,
            Mode::Normal | Mode::Ai | Mode::Lyric | Mode::Controller => 0,
        };
        ORDER[if forward { (at + 1) % 4 } else { (at + 3) % 4 }]
    }
}

/// FSM에 들어가는 사건. 각 사건을 부르는 Action은 `docs/reports/s2-t3/plan.md` 전이 표에 있다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModeEvent {
    /// 배치 뷰에서 클립 안으로(`clip::Open`)
    OpenClip,
    /// 피아노롤 ↔ 배치 뷰(`view::Toggle`)
    ToggleView,
    /// 그 모드로 들어간다
    Enter(Mode),
    /// 모드 순환 Normal → Step → Sequence → Play → Normal
    Cycle,
    /// 모드 역순환
    CycleBack,
    /// Normal로 돌아간다(선택·뷰는 그대로)
    ToNormal,
    /// 한 단계 나간다: 고스트 → 선택 → 모드 → 피아노롤 → (없음)
    Back,
    /// 선택 해제
    ClearSelection,
    /// 고스트만 지운다
    DismissGhost,
    /// 선택이 생겼다(S3)
    Select,
    /// 고스트가 나타났다(S7). Step에서만 의미가 있다
    ShowGhost,
}

impl ModeEvent {
    /// 모든 사건(`Enter`는 모드마다 하나).
    #[must_use]
    pub fn all() -> Vec<ModeEvent> {
        let mut all = vec![
            ModeEvent::OpenClip,
            ModeEvent::ToggleView,
            ModeEvent::Cycle,
            ModeEvent::CycleBack,
            ModeEvent::ToNormal,
            ModeEvent::Back,
            ModeEvent::ClearSelection,
            ModeEvent::DismissGhost,
            ModeEvent::Select,
            ModeEvent::ShowGhost,
        ];
        all.extend(Mode::ALL.map(ModeEvent::Enter));
        all
    }
}

/// 모드 FSM의 상태. 필드는 비공개이고 [`ModeState::apply`]로만 바뀐다.
///
/// 불변식: 고스트가 있으면 모드는 Step이다. 노트 입력 모드(Step, Sequence, Lyric)는 피아노롤에 있다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModeState {
    view: View,
    mode: Mode,
    selection: bool,
    ghost: bool,
}

impl Default for ModeState {
    fn default() -> Self {
        Self::INITIAL
    }
}

impl ModeState {
    /// 시작 상태이자 `Esc` 연타가 수렴하는 상태: Normal, 배치 뷰, 선택·고스트 없음.
    pub const INITIAL: ModeState = ModeState {
        view: View::Arrangement,
        mode: Mode::Normal,
        selection: false,
        ghost: false,
    };

    #[must_use]
    pub const fn view(self) -> View {
        self.view
    }

    #[must_use]
    pub const fn mode(self) -> Mode {
        self.mode
    }

    #[must_use]
    pub const fn has_selection(self) -> bool {
        self.selection
    }

    #[must_use]
    pub const fn has_ghost(self) -> bool {
        self.ghost
    }

    /// 불변식을 지키는 모든 상태.
    pub fn all() -> impl Iterator<Item = ModeState> {
        let views = [View::Arrangement, View::PianoRoll];
        Mode::ALL.into_iter().flat_map(move |mode| {
            views.into_iter().flat_map(move |view| {
                [false, true].into_iter().flat_map(move |selection| {
                    [false, true].into_iter().filter_map(move |ghost| {
                        let s = ModeState {
                            view,
                            mode,
                            selection,
                            ghost,
                        };
                        s.is_valid().then_some(s)
                    })
                })
            })
        })
    }

    /// 불변식: 고스트는 Step에만, 노트 입력 모드는 피아노롤에만.
    const fn is_valid(self) -> bool {
        (!self.ghost || matches!(self.mode, Mode::Step))
            && (!self.mode.writes_notes() || matches!(self.view, View::PianoRoll))
    }

    /// 사건 하나를 적용한 다음 상태.
    #[must_use]
    pub fn apply(self, event: ModeEvent) -> ModeState {
        match event {
            ModeEvent::OpenClip if self.is_normal_in(View::Arrangement) => {
                self.with_view(View::PianoRoll)
            }
            ModeEvent::ToggleView if self.mode == Mode::Normal => self.with_view(match self.view {
                View::Arrangement => View::PianoRoll,
                View::PianoRoll => View::Arrangement,
            }),
            ModeEvent::Enter(mode) => self.with_mode(mode),
            ModeEvent::Cycle => self.with_mode(self.mode.cycled(true)),
            ModeEvent::CycleBack => self.with_mode(self.mode.cycled(false)),
            ModeEvent::ToNormal => self.with_mode(Mode::Normal),
            ModeEvent::Back => self.back(),
            ModeEvent::ClearSelection => ModeState {
                selection: false,
                ..self
            },
            ModeEvent::DismissGhost => ModeState {
                ghost: false,
                ..self
            },
            ModeEvent::Select => ModeState {
                selection: true,
                ..self
            },
            ModeEvent::ShowGhost if self.mode == Mode::Step => ModeState {
                ghost: true,
                ..self
            },
            ModeEvent::OpenClip | ModeEvent::ToggleView | ModeEvent::ShowGhost => self,
        }
    }

    fn is_normal_in(self, view: View) -> bool {
        self.mode == Mode::Normal && self.view == view
    }

    /// 뷰를 바꾼다. 피아노롤의 노트 선택과 배치 뷰의 클립 선택은 다른 것이라 선택을 비운다.
    fn with_view(self, view: View) -> ModeState {
        if view == self.view {
            return self;
        }
        ModeState {
            view,
            selection: false,
            ..self
        }
    }

    /// 모드를 바꾼다. 고스트는 Step에만 산다. 노트 입력 모드는 피아노롤로 옮겨 간다(5.5에서
    /// 모드를 나오면 피아노롤이다).
    fn with_mode(self, mode: Mode) -> ModeState {
        if mode == self.mode {
            return self;
        }
        let s = if mode.writes_notes() {
            self.with_view(View::PianoRoll)
        } else {
            self
        };
        ModeState {
            mode,
            ghost: false,
            ..s
        }
    }

    /// `Esc` 한 단계(5.5): 고스트 → 선택 → 모드 → 피아노롤 → (더 나갈 곳 없음).
    fn back(self) -> ModeState {
        if self.ghost {
            ModeState {
                ghost: false,
                ..self
            }
        } else if self.selection {
            ModeState {
                selection: false,
                ..self
            }
        } else if self.mode != Mode::Normal {
            self.with_mode(Mode::Normal)
        } else {
            self.with_view(View::Arrangement)
        }
    }

    /// 창의 키 컨텍스트 경로(바깥 → 안). 한 요소에 식별자 하나씩 겹쳐야 `A > B`가 맞는다.
    #[must_use]
    pub fn context_path(self) -> &'static [&'static str] {
        match (self.mode, self.view, self.selection, self.ghost) {
            (Mode::Normal, View::Arrangement, false, _) => &["Normal", "Arrangement"],
            (Mode::Normal, View::Arrangement, true, _) => &["Normal", "Arrangement", "Selection"],
            (Mode::Normal, View::PianoRoll, false, _) => &["Normal", "PianoRoll"],
            (Mode::Normal, View::PianoRoll, true, _) => &["Normal", "PianoRoll", "Selection"],
            (Mode::Step, _, _, false) => &["Step"],
            (Mode::Step, _, _, true) => &["Step", "Ghost"],
            (Mode::Sequence, ..) => &["Sequence"],
            (Mode::Play, ..) => &["Play"],
            (Mode::Ai, ..) => &["AI"],
            (Mode::Lyric, ..) => &["Lyric"],
            (Mode::Controller, ..) => &["Controller"],
        }
    }
}
