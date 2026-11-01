# Task 보고: S3-T6c 트랜스포트 템포·박자표 끌어서 조정

- PR: 브랜치 `s3/t6c-transport-drag`
- 계획 파일: docs/reports/s3-t6c/plan.md
- 계획 대비 달라진 점:
  - 뗌 처리가 뗀 자리로 한 번 더 계산한다(움직임 사건 없이 다른 자리에서 뗄 수 있다). 시험 19를 더했다.
  - 템포 기준 값이 필드 범위(10.0–999.9) 밖일 때도 박자표와 같은 규칙(바깥으로 안 감, 안쪽 첫 단계에 범위 끝)을 쓴다. 결정 7절 4에 한 문장을 더했다.
  - `FieldDrag`에 누를 때의 값(`start`)을 더했다(위 결함).
  - 시험 3은 `proptest` 대신 100–9999 전수 시험이다(앱 crate에 `proptest`가 없고 범위가 작다).

## 구현 요약

- `crates/keyscore-app/src/transport_drag.rs`(새 파일): 순수 계산 `drag_steps`(4 px마다, 0 쪽으로 내림), `tempo_tenths`·`tempo_from_tenths`(필드 표기와 같은 반올림), `dragged_tempo`(1 BPM·`Cmd` 0.1 BPM, 10.0–999.9), `dragged_meter`(분자 1–99, 분모 1–32 지수 단계), `meter_part_at`. 범위 밖 기준의 규칙은 `step_within` 하나가 맡는다. 창 처리기: 필드 상자의 캡처 단계 누름(`transport_field_press`), 창 전체 움직임·뗌을 받는 빈 그림(`transport_drag_listener`), 움직임(문턱 3 px, `Cmd`가 바뀌면 기준점 옮김), 뗌(클릭이면 필드에 포커스·전체 선택, 끌기면 `Edit::SetTempo`·`SetMeter` 하나), 취소(`cancel_transport_drag`).
- `transport_bar.rs`: 템포·박자표 입력란을 감싼 상자에 캡처 누름 처리기와 상자 기록 그림을 달고, 바 끝에 끌기 수신 그림을 둔다. `TransportBar`에 `drag`·`tempo_box`·`meter_box`.
- `pianoroll_mouse.rs`: `edit::CancelDrag`의 키 가로채기와 Action 처리기가 트랜스포트 끌기도 취소한다.
- `window.rs`: 문서 변경 관찰자가 필드를 맞추기 전에 트랜스포트 끌기를 취소한다.
- `fields.rs`: `reset_field`를 crate 안에 연다.
- 결정 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 7절(계획 전에 적음).

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 2689 passed, 1 failed(아래 고침 전의 시험 7)
고친 뒤 cargo test -p keyscore-app --no-fail-fast: 1718 passed, 0 failed
(다른 crate는 바뀌지 않아 위 전체 실행의 결과 그대로. 합계 2690 통과)
~~~

전체 실행 중 시험 7이 실패해 결함을 찾았다: `Cmd`를 바꿀 때 옮기는 기준 값을 「누를 때의 값」 비교에도 써서, `Cmd`를 놓으며 떼면 편집이 사라졌다. 누를 때의 값(`start`)을 따로 두어 고쳤다. 인계 문서의 흔들리는 시험(`review_s2_t5a_r2::pause_position_does_not_drift_with_many_live_edits`)은 첫 실행에서 한 번 실패했고 `--no-fail-fast` 재실행에서는 통과했다(이 Task와 무관한 재생 경로).

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `transport_drag_pure::drag_steps_counts_four_px_toward_zero` | 통과 |
| 2 | `transport_drag_pure::dragged_tempo_adds_steps_and_clamps` | 통과 |
| 3 | `transport_drag_pure::tempo_tenths_round_trips_with_field_format` | 통과 |
| 4 | `transport_drag_pure::dragged_meter_changes_numerator_and_denominator` | 통과 |
| 5 | `transport_drag_pure::meter_part_splits_at_middle` | 통과 |
| 6 | `transport_drag::dragging_tempo_up_sets_tempo_with_one_undo` | 통과 |
| 7 | `transport_drag::cmd_drag_fine_tunes_tempo_without_jump` | 통과 |
| 8 | `transport_drag::drag_down_stops_at_tempo_minimum` | 통과 |
| 9 | `transport_drag::meter_drag_left_half_numerator_right_half_denominator` | 통과 |
| 10 | `transport_drag::click_without_drag_enters_field_with_all_selected` | 통과 |
| 11 | `transport_drag::small_move_under_threshold_is_click` | 통과 |
| 12 | `transport_drag::escape_cancels_tempo_drag` | 통과 |
| 13 | `transport_drag::rebound_cancel_drag_key_cancels_tempo_drag` | 통과 |
| 14 | `transport_drag::document_change_during_drag_cancels` | 통과 |
| 15 | `transport_drag::drag_back_to_start_makes_no_edit` | 통과 |
| 16 | `transport_drag::pressing_focused_field_does_not_drag` | 통과 |
| 17 | `transport_drag::drag_continues_outside_field` | 통과 |
| 18 | `transport_drag::drag_works_in_step_mode` | 통과 |
| 19 | `transport_drag::release_away_without_move_uses_release_position` | 통과 |

계획에 있었지만 쓰지 않은 테스트와 이유: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`, `scripts/check-deps.test.sh`(16 통과), `scripts/bench-check.test.sh`(8 통과)

## 알려진 한계

- 끄는 동안 재생 템포는 바뀌지 않는다(뗄 때 적용, 결정 7절 5).
- 필드 위 포인터 모양(위아래 화살표)은 바꾸지 않았다(REQ에 없음).
- 끄는 동안 키 `t`(`transport::SetTempo` 값 없음)로 템포 필드에 포커스가 가면, 끌기는 이어지고 뗄 때 필드를 문서 값으로 되돌리고 포커스를 문서로 돌린다(친 글자는 사라진다). 마우스를 쥔 채 키로 필드에 들어가는 드문 조합이라 그대로 두었다.
- 창이 비활성이 되는 등으로 뗌 사건을 놓치면 끌기가 남는다. 다음 누름이 새 끌기로 덮는다(피아노롤과 같다).

## 리뷰어에게

- 입력란이 누름을 받지 않게 하는 방법: 감싼 상자의 캡처 단계 누름에서 `cx.stop_propagation()`. gpui-component의 꺼진 입력란과 같은 방식이다. 입력란 안쪽 글자 요소가 창 전체 움직임 처리기(`on_drag_move`)를 가지는데, 누름을 받지 않아 「선택 중」 상태가 아니므로 글자 선택이 생기지 않는다고 보았다. 실제 창에서 확인이 필요하다.
- 겹 바깥 누름 처리기(`focus_document_on_press`, 버블 단계)도 전파 정지로 불리지 않는다. 그래서 누름에서 직접 `focus_document`를 부른다.
- `Cmd`를 쥐거나 놓기만 하고 움직이지 않으면 다음 움직임에서 「지난 자리」(마지막 움직임의 y)를 기준점으로 삼는다. 값은 튀지 않는다.
- 기준 값과 같아지면 편집하지 않는다. 문서 템포가 10분의 1 BPM으로 딱 떨어지지 않을 때(SMF의 µs 값) 끌었다 제자리로 오면 문서 µs 값을 지킨다.
