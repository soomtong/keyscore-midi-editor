# Task 계획: S2-T3 모드 FSM과 Enter/Esc/Tab 규약

- 브랜치: `s2/t3-modes`
- 관련 REQ: P2, 5.1·5.2·5.3·5.4(진입·종료), 5.5, 5.10(뷰 전환), 5.11(모드 순환), 5.12(고스트), 7.6 「모드 전환」 행, UI-5(상태줄 모드 표시)
- 선행 Task: S2-T1, S2-T2
- 관련 결정: `2026-10-17-keymap-conflicts.md` A13(Esc의 의미, 결정 대기), `2026-10-17-count-and-non-key-dispatch.md`(C)

## 끝의 정의 (내 말로)

1. 모드 FSM(`src/mode.rs`, GPUI 없는 순수 상태 기계)
   - 상태 `ModeState { view, mode, selection, ghost }`
     - `view`: 배치 뷰 `Arrangement` / 피아노롤 `PianoRoll`
     - `mode`: `Normal`, `Step`, `Sequence`, `Play`, `Ai`(P2의 다섯) + `Lyric`(5.15), `Controller`(5.9). 카탈로그에 진입 Action이 있어 상태로만 둔다.
     - `selection`: 선택 유무. 실제 선택 내용은 S3이 채운다.
     - `ghost`: AI 고스트 표시 유무. 실제 고스트는 S7이 채운다.
   - 사건(`ModeEvent`)과 전이를 한 함수 `ModeState::apply(self, ModeEvent) -> ModeState`로 정의한다. 전이 표는 아래 「전이 표」.
2. 창 연결(`window.rs`)
   - 키 컨텍스트를 상태에서 만든다(`ModeState::context_path`). 지금은 루트의 `Normal` 고정이다.
     - Normal: `Normal > Arrangement` 또는 `Normal > PianoRoll`, 선택이 있으면 그 아래 `Selection`
     - Step: `Step`, 고스트가 있으면 그 아래 `Ghost`
     - 그 밖: `Sequence`, `Play`, `AI`, `Lyric`, `Controller`
     - GPUI는 컨텍스트 하나에 식별자 하나를 깊이로 쌓아야 `A > B`가 맞는다. 그래서 경로 길이만큼 `div`를 겹치고 가장 안쪽이 창 포커스를 잡는다.
   - 처리기를 단다: `mode::*`, `view::Toggle`, `clip::Open`(뷰 전환만), `select::Clear`, `ai::OpenPanel`·`Accept`·`Dismiss`·`DismissGhost`.
   - 공개 메서드 `mode_state()`와 `apply_mode_event()`. 이후 단계(선택 S3, 클립 끝 S5, 고스트 S7)가 사건을 넣는다.
3. 상태줄
   - 왼쪽에 모드 이름(`NORMAL`, `STEP`, `SEQUENCE`, `PLAY`, `AI`, `LYRIC`, `CONTROLLER`)을 늘 표시한다. 길이·옥타브(`STEP ♩ oct4`)는 S3이 붙인다.
   - 모드 표시를 누르거나 `mode::ShowMenu`를 실행하면 모드 메뉴(gpui-component `PopupMenu`)가 열린다. 항목은 P2의 다섯 모드이고 현재 모드에 체크가 있다.
   - 메뉴로 고르면 카운트를 먼저 비운다(결정 C: 키가 아닌 실행 경로). S2-T4가 실행 함수를 하나로 모을 때 이 경로도 그 함수로 옮긴다.
4. 메인 영역 자리표시자는 현재 뷰 이름(「배치 뷰」/「피아노롤」)을 보인다.
5. 키맵 엔진 보완: `keymap::apply`의 `clear_key_bindings`가 gpui-component가 `init`에서 건 바인딩(`PopupMenu`의 `up`/`down`/`enter`/`escape`, `Input`의 편집 키 등)까지 지운다. 모드 메뉴를 키보드로 조작하려면 필요하므로, `install` 때 이미 걸린 바인딩을 「바탕 레이어」로 떠 두고 매 적용 때 가장 먼저(가장 낮은 우선순위로) 다시 건다.

## 전이 표

규칙
- 뷰가 바뀌면 선택을 비운다. 피아노롤의 노트 선택과 배치 뷰의 클립 선택은 다른 것이라서다.
- 고스트는 Step에서만 산다. Step을 떠나면 고스트를 지운다.
- 모드 전환은 모드만 바꾼다. 뷰와 선택은 그대로다(5.11 「현재 커서 위치를 유지한 채 모드만 바뀐다」). 다만 Lyric·Controller는 피아노롤로 옮겨 들어간다(5.15, 5.9). 결정 `docs/decisions/2026-10-17-mode-entry-view.md`(리뷰 1 지적 1, 리뷰 2 지적 1).
- 모드 진입 Action은 어느 모드에서든 그 모드로 간다(팔레트·메뉴가 Normal을 거치지 않고 부를 수 있다). 기본 키맵은 5.5대로 Normal 컨텍스트에만 건다.
- 모드 순환은 Normal → Step → Sequence → Play → Normal(5.11). 순환에 없는 모드(AI, Lyric, Controller)에서는 Normal에 있는 것처럼 센다.

| 사건 | Action | 전이 |
| --- | --- | --- |
| `OpenClip` | `clip::Open`(배치 뷰 `enter`) | Normal·배치 뷰에서만 피아노롤로. 그 밖은 그대로 |
| `ToggleView` | `view::Toggle`(`v`) | Normal에서만 뷰를 바꾼다 |
| `Enter(m)` | `mode::Step`·`StepAfterNote`·`StepAtBarStart`·`StepAtBarEnd`·`StepOverwrite`, `mode::Sequence`, `mode::Play`, `mode::Lyric`, `mode::Controller`, `ai::OpenPanel` | 모드를 `m`으로 |
| `Cycle` / `CycleBack` | `mode::Cycle`(`ctrl-tab`) / `mode::CycleBack` | 순환 |
| `ToNormal` | `mode::Normal`, 클립 끝(S5) | 모드를 Normal로. 선택·뷰는 그대로 |
| `LeaveAi` | `ai::Accept`, `ai::Dismiss` | AI 모드에서만 Normal로. 그 밖은 그대로(리뷰 1 지적 3) |
| `ClearSelection` | `select::Clear` | 선택을 비운다 |
| `DismissGhost` | `ai::DismissGhost` | 고스트를 지운다 |
| `Select` | (S3의 선택 Action) | 선택이 생긴다 |
| `ShowGhost` | (S7의 어시스트) | Step에서만 고스트가 생긴다 |

## Esc 바인딩과 A13

A13은 「통상의 취소」로 결정됐다(`2026-10-17-keymap-conflicts.md` A13). 프리셋 `escape`는 다음과 같다. 깊은 컨텍스트가 이겨 한 번에 가장 안쪽 것 하나만 닫는다.
- `Step > Ghost` → `ai::DismissGhost`(Step에 머묾)
- `Step`, `Sequence`, `Play`, `Lyric`, `Controller` → `mode::Normal`(뷰·선택 유지)
- `AI` → `ai::Dismiss`(이미 있음)
- `Normal > Selection` → `select::Clear`
- 선택 없는 Normal → 없음(No-op). 피아노롤에서 배치 뷰로 나가지 않는다. `mode::Back`과 사건 `Back`은 뺐다.

`Esc` 연타는 어느 상태에서든 「Normal·선택 없음·고스트 없음」으로 수렴하고 뷰는 바꾸지 않는다. 단계 파일의 「배치 뷰·선택 없음으로 수렴」은 이 결정으로 「뷰 그대로·선택 없음」으로 읽는다.

## 공개 인터페이스 초안

~~~rust
pub mod mode {
    pub enum View { Arrangement, PianoRoll }
    pub enum Mode { Normal, Step, Sequence, Play, Ai, Lyric, Controller }
    pub struct ModeState { /* view, mode, selection, ghost: 비공개 */ }
    pub enum ModeEvent { OpenClip, ToggleView, Enter(Mode), Cycle, CycleBack, ToNormal, LeaveAi,
                         ClearSelection, DismissGhost, Select, ShowGhost }
    impl ModeState {
        pub const INITIAL: ModeState;            // Normal·배치 뷰·선택 없음
        pub fn view(self) -> View; pub fn mode(self) -> Mode;
        pub fn has_selection(self) -> bool; pub fn has_ghost(self) -> bool;
        pub fn apply(self, ev: ModeEvent) -> ModeState;
        pub fn context_path(self) -> &'static [&'static str];
        pub fn all() -> impl Iterator<Item = ModeState>; // 불변식을 지키는 모든 상태(테스트용)
    }
    impl Mode { pub fn label(self) -> &'static str; pub const MENU: [Mode; 5]; }
}
impl ProjectWindow {
    pub fn mode_state(&self) -> ModeState;
    pub fn apply_mode_event(&mut self, ev: ModeEvent, window: &mut Window, cx: &mut Context<Self>);
    pub fn mode_menu_open(&self) -> bool;
}
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `transition_table_matches_req_5_5` | 단위(테이블) | 위 전이 표의 각 행(시작 상태, 사건, 기대 상태) |
| 2 | `every_state_upholds_invariants_after_any_event` | 단위(전수) | 모든 상태 × 모든 사건에서 고스트 ⇒ Step, 결과가 `all()` 안 |
| 3 | `escape_cancels_innermost_thing_once` | 단위(테이블) | A13 결정의 `escape` 한 번이 가장 안쪽 것 하나만 닫는다(고스트 → 입력 모드 → 선택 → No-op) |
| 4 | `escape_mashing_converges_to_normal_without_changing_view` | 단위(전수) | 모든 상태에서 `escape` 연타가 3번 안에 Normal·선택 없음·고스트 없음, 뷰는 그대로 |
| 5 | `cycle_visits_normal_step_sequence_play` | 단위 | `Cycle` 4번이 한 바퀴, `CycleBack`이 역순, 순환 밖 모드의 순환 |
| 6 | `context_path_per_state` | 단위(테이블) | 상태별 키 컨텍스트 경로 |
| 7 | `keys_drive_modes_through_key_contexts` | UI | 기본 프리셋으로 `v`, `enter`, `ctrl-tab`, `cmd-r`, `cmd-k`, `a`가 표대로 상태를 바꾸고, 바뀐 컨텍스트의 바인딩이 이어서 맞는다(예: Arrangement의 `enter`=`clip::Open`, PianoRoll의 `enter`=`mode::Step`) |
| 8 | `escape_mashing_converges_with_default_preset`, `_with_vim_preset`, `escape_steps_ghost_then_mode_then_selection` | UI | 프리셋 `escape`로 여러 시작 상태에서 연타 → Normal·선택 없음, 뷰 그대로. 고스트 → 모드 → 선택 순서 |
| 9 | `status_bar_shows_mode_and_menu_switches_mode` | UI | 상태줄 모드 이름이 모드를 따른다. 모드 표시 클릭과 `mode::ShowMenu`로 메뉴가 열리고, 키보드(`down`, `enter`)로 고르면 그 모드가 되고 카운트가 비워진다 |
| 10 | `component_bindings_survive_keymap_apply` | UI | `keymap::apply` 뒤에도 `PopupMenu` 컨텍스트의 `escape`·`enter` 바인딩이 남아 있고, 사용자 키맵보다 우선순위가 낮다 |
| 11 | `main_view_placeholder_names_current_view` | UI | 메인 영역 문구가 뷰를 따른다 |

## 불변식

- 고스트 ⇒ 모드가 Step.
- 상태 필드는 비공개이고 `apply`로만 바뀐다. 그래서 불변식을 깨는 상태를 밖에서 만들 수 없다.
- 창의 키 컨텍스트는 늘 `context_path(mode_state())`와 같다.
- 모드 메뉴가 열려 있으면 문서 컨텍스트 키는 메뉴 뒤로 가지 않는다. 컨텍스트 없는 전역 바인딩(`ctrl-tab`, `cmd-` 표준 키)은 메뉴가 열려 있어도 실행된다(의도, 리뷰 2 제안 4).

## 범위 밖

- 각 모드의 실제 동작(Step 입력 S3, Sequence·Play S5, 가사·컨트롤러 S6, AI S7), 길이·옥타브 상태줄 표시(S3), 레이아웃 하위 컨텍스트(`Step > Piano` 등, S3)
- 실제 선택·클립 열기(S3·S4). 여기서는 선택 유무와 뷰만 다룬다.
- `escape` 프리셋 바인딩(A13 결정 뒤)
- 실행 함수의 통합(S2-T4)

## 리스크·결정 요청

- A13 결정 대기. 위 「Esc 바인딩과 A13」처럼 FSM은 선택지와 무관하게 만들고, 프리셋의 `escape`만 비워 둔다. 결정이 나면 프리셋 몇 줄과 테스트 하나를 고친다.
- 위 전이 표의 「규칙」 네 줄은 REQ가 직접 적지 않은 세부다. 5.5·5.10·5.11의 문장에서 읽어 낸 것이고 의미를 바꾸지 않는다고 본다.
