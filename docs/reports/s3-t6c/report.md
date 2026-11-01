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
- 창이 비활성이 되는 등으로 뗌 사건을 놓치면 끌기가 남지만, 단추 없는 움직임이나 새 누름이 취소한다(리뷰 2 지적 1).

## 리뷰어에게

- 입력란이 누름을 받지 않게 하는 방법: 감싼 상자의 캡처 단계 누름에서 `cx.stop_propagation()`. gpui-component의 꺼진 입력란과 같은 방식이다. 입력란 안쪽 글자 요소가 창 전체 움직임 처리기(`on_drag_move`)를 가지는데, 누름을 받지 않아 「선택 중」 상태가 아니므로 글자 선택이 생기지 않는다고 보았다. 실제 창에서 확인이 필요하다.
- 겹 바깥 누름 처리기(`focus_document_on_press`, 버블 단계)도 전파 정지로 불리지 않는다. 그래서 누름에서 직접 `focus_document`를 부른다.
- `Cmd`를 쥐거나 놓기만 하고 움직이지 않으면 다음 움직임에서 「지난 자리」(마지막 움직임의 y)를 기준점으로 삼는다. 값은 튀지 않는다.
- 기준 값과 같아지면 편집하지 않는다. 문서 템포가 10분의 1 BPM으로 딱 떨어지지 않을 때(SMF의 µs 값) 끌었다 제자리로 오면 문서 µs 값을 지킨다.

## 리뷰 1 대응

판정 「수정 후 재리뷰」(`review-1.md`, 시험 `tests/review_s3_t6c_r1.rs` 22개 중 4개 실패).

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 수정: 필드 누름의 `stop_propagation`이 팔레트·메뉴의 바깥 누름 닫기까지 막음(회귀) | 받아들임. 전파를 멈추지 않고 `window.prevent_default()`로 바꿨다. 입력란의 누름 처리기와 GPUI의 누름 포커스는 기본 동작이 막힌 누름을 받지 않는다. 실패 시험 3개 통과 | `e2e2de0` |
| 2 제안: 되돌릴 것이 없는 `Cmd+Z`도 끌기를 취소 | 받아들임. 문서 변경 관찰자가 끄는 값(틱 0 템포·박자표)이 누를 때와 달라졌을 때만 취소한다. 결정 7절 7 문구를 고쳤다 | `33612e2` |
| 3 수정: 7절 9, 다른 입력란 위 뗌, 화면 경로의 범위 끝·범위 밖 기준, SMF 템포 왕복에 시험이 없음 | 받아들임. 리뷰 시험 파일을 PR에 넣었다(22개 모두 통과) | `56cd25e` |

## 리뷰 2 대응

판정 「수정 후 재리뷰」(`review-2.md`, 시험 `tests/review_s3_t6c_r2.rs` 13개 중 2개 실패).

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 수정: 뗌을 놓친 끌기가 남아 다른 곳 클릭의 뗌으로 적용됨 | 받아들임. 단추를 쥐지 않은 움직임, 필드 밖의 새 누름(창 전체 캡처), 필드 안의 새 누름이 남은 끌기를 취소한다(결정 7절 10). 고치는 중 필드 상자 기록 그림이 입력란 아래(정적 자리)에 놓여 있던 결함도 드러나 `top_0`·`left_0`을 더했다. 그동안은 박자표 쪽을 가르는 x만 써서 겉으로 드러나지 않았다 | `5525d08` |
| 2 제안: 끄는 중 키로 필드에 들어가 친 글자를 움직임·뗌이 덮거나 버림 | 받아들임. 끄는 중 그 필드가 포커스를 받으면 끌기를 놓는다. 친 글자는 Enter로 확정하거나 `Esc`로 취소한다(결정 7절 11). 보고서 「알려진 한계」의 같은 항목은 이것으로 없어졌다 | `c39958d` |

## 리뷰 3 대응

판정 「병합 가능」(`review-3.md`, 시험 `tests/review_s3_t6c_r3.rs` 12개 중 3개가 제안의 동작으로 실패).

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 제안: 뗌의 `focus_document`가 끄는 동안 키로 옮긴 포커스(다른 필드, 퀀타이즈 시트)를 뺏음 | 받아들임. 포커스가 문서에 있거나 아무 데도 없을 때만 문서로 돌린다. 결정 7절 6에 한 문장을 더했다 | `06b9e92` |
| 2 제안: 겹이 필드를 가리면 그 자리 누름이 남은 끌기를 끝내지 않음 | 받아들임. 창 전체 누름 처리기가 상자 좌표 대신 「이 누름으로 막 만든 끌기」 표시(`fresh`)로 가른다. 필드 누름은 캡처 단계에서 이 처리기보다 먼저 불린다(필드가 먼저 그려진다) | `e215fb6` |

리뷰 시험 세 파일(22·13·12)과 구현 시험(14·5), 트랜스포트 바 시험(6)이 모두 통과한다. 리뷰 3의 범위 밖 관찰(고치는 중인 필드를 가장자리에서 누르면 포커스를 잃음)은 `main`에서도 같아 이 Task 밖이다. 사람이 실제 앱에서 확인할 항목으로 인계 문서에 남긴다.

## 최종 게이트(병합 전)

- fmt, clippy `-D warnings`, deny: 통과
- `cargo test -p keyscore-app --no-fail-fast`(최종 코드): 1765 통과, 0 실패
- `cargo test --workspace --no-fail-fast`(리뷰 2 수정 시점, 그 뒤로 앱 crate만 바뀜): 2725 통과, 0 실패
- `scripts/check-deps.sh`·`check-deps.test.sh`·`bench-check.test.sh`: 통과(의존성 변경 없음)
