# Task 계획: S3-T6c 트랜스포트 템포·박자표 끌어서 조정

- 브랜치: `s3/t6c-transport-drag`
- 관련 REQ: 7.6(「트랜스포트의 필드(클릭해 입력, 끌어서 조정)」), TRN-4, P2(`Esc`는 진행 중 조작 취소), EDT-1(undo 한 단위)
- 선행 Task: S2-T5b(트랜스포트 입력란, `fields.rs` 규칙), S3-T6b(`edit::CancelDrag` 가로채기)
- 관련 스파이크: 없음
- 결정: `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절(이 Task 전에 적음, 에이전트 해석)

## 끝의 정의 (내 말로)

트랜스포트의 템포·박자표 필드를 세로로 끌어 값을 바꾼다. 누른 뒤 움직이지 않고 떼면 지금처럼 입력란에 들어가 바로 칠 수 있다(전체 선택). 끄는 동안 필드가 새 값을 보이고, 떼면 편집 하나(undo 한 단위)로 문서에 들어간다. `Esc`(키맵의 `edit::CancelDrag`)와 문서 변경은 취소다. 규칙은 결정 7절 1–9이고, 여기서는 구현 단위로 적는다.

1. 순수 계산(`transport_drag.rs`, `pub`): 세로 픽셀 → 단계 수(`drag_steps`, 4 px마다, 0 쪽으로 내림), 템포(10분의 1 BPM 정수로 계산, `dragged_tempo`), 박자표(`dragged_meter`, 분자 1–99, 분모 1–32 두 배·절반), 박자표의 바꿀 쪽(`meter_part_at`, 가로 가운데 기준). 템포 ↔ 10분의 1 BPM은 `format_tempo`·`parse_tempo`와 같은 반올림이다(`tempo_tenths`, `tempo_from_tenths`).
2. 상태: `TransportBar.drag: Option<FieldDrag>`(필드, 박자표의 쪽, 누른 점, 기준 y·기준 값, 미세 조정 여부, 문턱을 넘었는가, 지금 값). 필드 둘레 상자는 그리기 때 기록한다(`canvas`, 박자표의 쪽을 가르는 데 쓴다).
3. 누름: 필드 상자의 캡처 단계 누름(`capture_any_mouse_down`, 왼쪽 단추)에서 그 필드에 포커스가 없으면 `FieldDrag`를 만들고 전파를 멈춘다(입력란·겹 바깥 누름 처리기가 받지 않는다). 포커스는 문서로 옮긴다(고치던 다른 입력란은 지금처럼 취소, 7절 9). 되돌리기 묶음을 끊는다(클릭은 키 입력이 아니다). 포커스가 있으면 아무것도 하지 않는다(입력란 보통 동작).
4. 움직임·뗌: 끄는 동안 포인터가 필드 밖으로 나가도 따르도록 창 전체 마우스 사건(`window.on_mouse_event`)을 그리기 때 등록한다(피아노롤 Element와 같은 방식). 움직임마다 문턱(3 px, `pianoroll::mouse::DRAG_THRESHOLD`)을 보고, 넘었으면 단계 수로 값을 다시 계산해 필드 글자로 보인다. `Cmd`가 바뀌면 그 자리를 새 기준점으로 삼는다(템포만).
5. 뗌: 문턱을 넘지 않았으면 클릭이다: 필드에 포커스를 주고 전체 선택. 넘었으면 지금 값이 문서와 다를 때 `edit_document`로 편집 하나를 적용하고, 필드를 문서 값으로 되돌린다. 포커스는 문서에 둔다.
6. 취소: `intercept_mouse_keys`와 `cancel_drag_action`이 피아노롤 조작과 함께 트랜스포트 끌기도 취소한다(`cancel_transport_drag`). 문서 변경 관찰자는 `sync_fields` 전에 끌기를 취소한다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/transport_drag.rs
pub const DRAG_STEP_PX: f32 = 4.0;
pub const NUMERATOR_RANGE: RangeInclusive<u8> = 1..=99;
pub const DENOMINATOR_MAX: u16 = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterPart { Numerator, Denominator }
/// 위로 끈 거리(px, 위가 양수) → 단계 수. 4 px마다, 0 쪽으로 내림.
pub fn drag_steps(up_px: f32) -> i32;
pub fn tempo_tenths(tempo: Tempo) -> u32;
pub fn tempo_from_tenths(tenths: u32) -> Option<Tempo>;
/// 기준(10분의 1 BPM)에서 단계만큼. 미세면 단계 0.1 BPM, 아니면 1 BPM. 100–9999에서 멈춘다.
pub fn dragged_tempo(base_tenths: u32, steps: i32, fine: bool) -> u32;
pub fn dragged_meter(base: TimeSignature, part: MeterPart, steps: i32) -> TimeSignature;
/// 필드 왼쪽에서 잰 x, 필드 폭 → 바꿀 쪽.
pub fn meter_part_at(x: f32, width: f32) -> MeterPart;

// crates/keyscore-app/src/window.rs (시험용)
impl ProjectWindow {
    pub fn tempo_field_text(&self, cx: &App) -> String;
    pub fn meter_field_text(&self, cx: &App) -> String;
    pub fn transport_drag_active(&self) -> bool;
}
~~~

## 테스트 목록

순수 시험 `crates/keyscore-app/tests/transport_drag_pure.rs`, UI 시험 `crates/keyscore-app/tests/transport_drag.rs`(헤드리스 창, MIDI 없음, `simulate_event`로 누름·이동·뗌).

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `drag_steps_counts_four_px_toward_zero` | 단위 | 0·3·4·7·8 px, 음수 대칭 |
| 2 | `dragged_tempo_adds_steps_and_clamps` | 단위 | 1 BPM·0.1 BPM 단계, 소수 유지, 10.0·999.9에서 멈춤, 범위 밖 기준(5.0·1200.0 BPM)은 바깥으로 안 감 |
| 3 | `tempo_tenths_round_trips_with_field_format` | 전수 | 100–9999 모든 값이 `tempo_from_tenths` → `tempo_tenths`로 돌아오고 `format_tempo`와 같다 |
| 4 | `dragged_meter_changes_numerator_and_denominator` | 단위 | 분자 ±, 1·99에서 멈춤. 분모 두 배·절반, 1·32에서 멈춤. 범위 밖 기준(분자 120, 분모 128)은 바깥으로 안 가고 안쪽 한 단계에 범위 끝 |
| 5 | `meter_part_splits_at_middle` | 단위 | 왼쪽 절반 분자, 가운데·오른쪽 분모 |
| 6 | `dragging_tempo_up_sets_tempo_with_one_undo` | UI | 위로 40 px → +10 BPM, 끄는 중 문서 그대로·필드 글자 바뀜, 뗀 뒤 undo 하나로 원래대로 |
| 7 | `cmd_drag_fine_tunes_tempo_without_jump` | UI | `Cmd` 쥔 채 위로 40 px → +1.0 BPM. 끄는 중 `Cmd`를 쥐면 그 자리에서 값이 튀지 않음 |
| 8 | `drag_down_stops_at_tempo_minimum` | UI | 아주 아래로 → 10.0 |
| 9 | `meter_drag_left_half_numerator_right_half_denominator` | UI | 왼쪽에서 위로 → 5/4, 오른쪽에서 위로 → 4/8 |
| 10 | `click_without_drag_enters_field_with_all_selected` | UI | 누르고 그대로 떼기 → 문서 그대로, 바로 「90」 Enter로 90 BPM |
| 11 | `small_move_under_threshold_is_click` | UI | 2 px 움직이고 떼기 → 클릭 |
| 12 | `escape_cancels_tempo_drag` | UI | 끄는 중 `escape` → 필드가 문서 값, 떼어도 편집 없음, undo 쌓이지 않음 |
| 13 | `rebound_cancel_drag_key_cancels_tempo_drag` | UI | `edit::CancelDrag`를 `f9`로 묶으면 `f9`가 취소 |
| 14 | `document_change_during_drag_cancels` | UI | 끄는 중 `cmd-z`(앞서 만든 편집) → 끌기 끝, 떼어도 편집 없음 |
| 15 | `drag_back_to_start_makes_no_edit` | UI | 위로 끌었다 제자리로 와서 떼기 → undo 쌓이지 않음 |
| 16 | `pressing_focused_field_does_not_drag` | UI | 고치는 중인 템포 필드를 위로 끌어도 값 조정 없음(입력란 동작) |
| 17 | `drag_continues_outside_field` | UI | 필드 밖(피아노롤 위)까지 끌고 떼도 값이 바뀌고 피아노롤은 아무 일 없음 |
| 18 | `drag_works_in_step_mode` | UI | Step 모드에서도 같다(7절 8) |
| 19 | `release_away_without_move_uses_release_position` | UI | 움직임 사건 없이 다른 자리에서 떼면 그 자리로 계산(구현 중 더함) |

기존 `transport_bar.rs`·`review_s2_t5b*.rs`의 필드 시험(클릭 → `cmd-a` → 입력 → Enter)은 그대로 통과해야 한다.

## 불변식

- 문서는 뗄 때 한 번만 바뀐다(편집 하나). 끄는 중·취소에는 바뀌지 않는다. 같은 값이면 편집하지 않는다.
- 편집은 필드 확정과 같은 `Edit::SetTempo`·`Edit::SetMeter`(틱 0)다. 끌기 전용 편집 경로가 없다.
- 고치는 중인(포커스가 있는) 입력란의 마우스 동작은 바뀌지 않는다.
- 실시간 스레드와 무관하다.

## 범위 밖

- 끄는 동안 재생 템포를 실시간으로 바꾸기(7절 5).
- 필드 위 포인터 모양(위아래 화살표) 같은 호버 표시(REQ에 없다. 피아노롤 끝 영역과 같은 태도).
- 인스펙터 필드(뱅크·기본 벨로시티)의 끌기(7.6이 트랜스포트 필드만 적는다).
- 휠로 값 바꾸기(REQ에 없다).

## 리스크·결정 요청

- 결정 7절은 에이전트 해석이다(4 px 단계, `Cmd` 미세, 분자·분모를 누른 쪽으로 가르기, 범위 1–99·1–32, 뗄 때 적용). 사람이 다르게 보면 고친다.
- gpui 입력란이 캡처 단계 누름을 막으면 받지 않는다는 가정은 gpui-component의 꺼진 입력란(`capture_any_mouse_down` + `stop_propagation`)과 같은 방식이라 기댄다.
