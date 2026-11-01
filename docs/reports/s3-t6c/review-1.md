# 적대적 리뷰: S3-T6c 트랜스포트 템포·박자표 필드 끌어서 조정 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 5e3ae13 (`s3/t6c-transport-drag`, `git diff main...HEAD`)
- 깨뜨리기 시험: `crates/keyscore-app/tests/review_s3_t6c_r1.rs` (22개 중 4개 실패, 실패한 채로 둠)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 클릭(문턱 미만): 떼면 템포 필드에 포커스·전체 선택, 바로 친 값이 Enter로 확정된다. 클릭 뒤 `Esc`는 S2-T5b 규칙대로 고치기를 취소한다.
2. 다른 필드(박자표)를 고치는 중(친 글자 있음)에 템포 필드를 누르면 박자표는 확정이 아니라 취소되고 템포 끌기가 된다(7절 9).
3. 끄는 중 포인터가 다른 입력란(박자표) 위에서 떼어지면 템포만 바뀌고 박자표 입력란은 포커스·글자를 받지 않는다.
4. 필드 누름의 캡처 단계 `stop_propagation`이 입력란 말고도 다른 캡처 단계 처리기(팔레트·팝업 메뉴의 `on_mouse_down_out`)를 막아, 열린 팔레트·메뉴가 닫히지 않을 수 있다.
5. 범위 끝: 999.9에서 위로, 분자 99·분모 32에서 위로는 편집 없음. 범위 밖 기준(5 BPM, 분모 128)은 바깥으로 안 가고 안쪽 첫 단계에 범위 끝.
6. 10분의 1 BPM으로 안 떨어지는 SMF 템포(µs 500 001): 끌었다 제자리면 µs 보존, 한 단계면 정확히 121.0.
7. `Cmd`를 쥔 채 끌다 `Cmd`를 놓으며 떼기: 뗌 처리의 재계산이 값을 튀게 하지 않는가. 박자표에서 `Cmd`는 뜻이 없는가.
8. `Esc` 뒤 계속 움직이면 끌기가 되살아나지 않는가. 문턱 전 `Esc`면 클릭이 취소되는가.
9. 문서 변경 관찰자는 `cx.notify()`만 보므로 실제로 안 바뀐 알림(빈 undo 스택의 `Cmd+Z`)도 끌기를 취소한다.
10. 더블클릭·오른쪽 단추는 끌기를 만들지 않는다.
11. 끄는 중 모드 전환(7절 8)에도 끌기가 이어진다. 끌기 편집은 `coalesce` 없는 단위라 키 묶음과 섞이지 않는다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 클릭 → 입력·Esc | 일부 | 클릭 → 입력은 `click_without_drag_enters_field_with_all_selected`. 클릭 뒤 Esc 없음 |
| 2 다른 필드 고치는 중 누름 | 없음 | 7절 9의 규칙인데 시험이 없다 |
| 3 다른 입력란 위에서 뗌 | 없음 | `drag_continues_outside_field`는 피아노롤 위만 |
| 4 팔레트·메뉴가 열린 채 누름 | 없음 | |
| 5 범위 끝·범위 밖 기준 | 순수 시험만 | UI 경로(편집 없음, undo 길이) 없음 |
| 6 SMF 템포 왕복 | 없음 | 보고서 「리뷰어에게」에 적었으나 시험 없음 |
| 7 `Cmd` 놓으며 뗌·박자표 `Cmd` | 일부 | `cmd_drag_fine_tunes_tempo_without_jump`는 움직임으로 놓음. 뗌에서 놓기·박자표 없음 |
| 8 Esc 뒤 움직임·문턱 전 Esc | 없음 | |
| 9 바뀌지 않은 문서 알림 | 없음 | |
| 10 더블클릭·우클릭 | 없음 | |
| 11 모드 전환·undo 단위 | 일부 | Step 모드에서 시작은 있음. 끄는 중 전환 없음 |

## 3. 깨뜨리기 시도

모든 시험은 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t6c_r1`로 실행했다. 구현자의 `transport_drag`(14)·`transport_drag_pure`(5)도 같은 빌드에서 통과를 확인했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 박자표에 「7/8」을 친 채 템포를 끌기 | 못 깨뜨림 | `pressing_tempo_while_editing_meter_cancels_meter_and_drags` |
| 2 | 템포를 끌어 박자표 위에서 떼고 바로 「9」 입력 | 못 깨뜨림 | `release_over_meter_field_applies_tempo_only` |
| 3 | 조성 메뉴가 열린 채 템포 필드 끌기 | 깨짐: 끌기 편집은 들어가지만 메뉴가 열린 채 남는다(포커스는 문서) | `field_drag_with_key_menu_open_closes_menu` (실패) |
| 4 | 팔레트가 열린 채 템포 필드 끌기 | 깨짐: 팔레트가 남는다 | `field_drag_with_palette_open_closes_palette` (실패) |
| 5 | 팔레트가 열린 채 템포 필드 클릭 | 깨짐: 팔레트가 남은 채 필드에 포커스가 간다 | `field_click_with_palette_open_closes_palette` (실패) |
| 6 | 대조: 같은 상태에서 주 뷰를 누름 | 팔레트·메뉴 모두 닫힘(밖 누름은 닫는 것이 기존 동작) | `control_palette_closes_on_outside_press`, `control_key_menu_closes_on_outside_press` |
| 7 | 999.9에서 위로 끌기 | 못 깨뜨림(편집 없음) | `drag_up_at_tempo_maximum_makes_no_edit` |
| 8 | 5 BPM(µs 12 000 000)에서 아래로, 위로 한 단계 | 못 깨뜨림 | `out_of_range_slow_tempo_enters_range_on_first_step` |
| 9 | µs 500 001에서 끌었다 제자리, 한 단계 위 | 못 깨뜨림 | `smf_tempo_off_tenths_round_trips_and_steps` |
| 10 | 99/32에서 양쪽 위로, 3/128에서 분모 아래로 | 못 깨뜨림 | `meter_ends_stop_without_edit`, `meter_out_of_range_denominator_enters_at_32` |
| 11 | `Cmd` 쥐고 끌다 `Cmd` 놓으며 뗌 | 못 깨뜨림(120.2) | `release_with_cmd_lifted_does_not_jump` |
| 12 | 박자표 끌기 중 `Cmd` 쥐고 놓기 | 못 깨뜨림 | `cmd_has_no_effect_on_meter` |
| 13 | Esc 뒤 계속 움직이고 뗌, 문턱 전 Esc 뒤 뗌 | 못 깨뜨림 | `moves_after_escape_do_nothing`, `escape_before_threshold_cancels_click` |
| 14 | 빈 undo 스택에서 끄는 중 `Cmd+Z` | 깨짐: 문서가 바뀌지 않았는데 끌기가 취소된다 | `undo_with_empty_history_keeps_drag` (실패) |
| 15 | 클릭 → 「77」 → Esc | 못 깨뜨림 | `click_then_escape_cancels_field` |
| 16 | 더블클릭, 오른쪽 단추 끌기 | 못 깨뜨림 | `double_click_makes_no_edit`, `right_button_does_not_drag` |
| 17 | 피아노롤에서 끄는 중 Enter로 Step 전환 | 못 깨뜨림 | `mode_change_during_drag_keeps_drag` |
| 18 | 끌기 두 번 → undo 한 번 | 못 깨뜨림 | `two_drags_are_two_undo_units` |

원인(시도 3–5): `transport_field_press`가 캡처 단계에서 `cx.stop_propagation()`을 부른다(`crates/keyscore-app/src/transport_drag.rs:237`). 팔레트(`command_palette.rs:240`)와 gpui-component 팝업 메뉴는 `on_mouse_down_out`(캡처 단계)으로 밖 누름을 받아 닫히는데, 그 처리기가 불리지 않는다. 계획은 입력란과 겹 바깥 누름 처리기(`focus_document_on_press`)만 막는 것으로 보았고, 버블 단계 처리기만 대신 불렀다(`focus_document`). main에서는 필드 누름이 입력란에 닿기 전에 이 캡처 처리기들이 돌아 팔레트·메뉴를 닫았다(대조 시험 6과 같은 경로). 이 Task가 들인 회귀다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 문서는 뗄 때 한 번만 바뀐다 | 아니오 | 끄는 중 문서 무변경(시도 1–2, 구현자 시험 6), 취소·제자리 편집 없음(시도 7·9·13) |
| 편집은 필드 확정과 같은 `Edit::SetTempo`·`SetMeter`(틱 0) | 아니오 | `transport_drag.rs:327-342`, `edit_document`(coalesce 없음) |
| 같은 값이면 편집하지 않음 | 아니오 | 시도 7·9·10 |
| 고치는 중인 입력란의 마우스 동작 불변 | 아니오 | 구현자 시험 16, 시도 16 |
| 모델 불변식(박자표 분모 2의 거듭제곱, 템포 범위) | 아니오 | `TimeSignature::new`·`Tempo` 생성자를 거치고 실패는 기준 값으로 돌아감(`transport_drag.rs:62,105-123`) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.6 「클릭해 입력, 끌어서 조정」 | 충족 | 클릭 `transport_drag.rs:315-323`, 끌기 `:273-309`, 뗌 `:325-347` |
| TRN-4 (곡 전체 하나, 틱 0) | 충족 | `transport_drag.rs:330-338` |
| P2 (`Esc`는 진행 중 조작 취소, 가장 안쪽 하나) | 충족 | `pianoroll_mouse.rs:53-61`, `:73-75`. 단 문서 무변경 알림도 취소함(지적 2) |
| EDT-1 (드래그 한 번 = undo 한 단위) | 충족 | `window.rs:457-468` coalesce 없음, 시도 18 |

7절 9(다른 입력란 취소)·7절 8(모드)은 결정 문서의 규칙이며 시도 1·17로 확인했다. 팔레트·메뉴가 남는 것(지적 1)은 REQ 항목 위반이라기보다 기존 동작(팝업 밖 누름은 닫음, UI-2 팔레트)의 회귀로 본다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 UI 스레드만 바꾼다. 끄는 중 엔진에 보내는 것이 없다(뗄 때 `edit_document` 하나).
- 제품 코드의 `unwrap`/`expect`: 없음(`transport_drag.rs`는 `unwrap_or`·`map_or`만).
- 에러가 삼켜지는 곳: `transport_drag.rs:200,209`의 `let _ = on_move.update(..)` — 창 엔티티가 사라진 경우뿐이라 정당. `edit_document` 실패는 상태줄로 알린다(`window.rs:474-476`).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 팔레트나 팝업 메뉴(조성·포트·모드 등)가 열린 채 템포·박자표 필드를 누르면(클릭이든 끌기든) 그것이 닫히지 않는다. 캡처 단계 `stop_propagation`이 팔레트·메뉴의 `on_mouse_down_out`까지 막는다. 끌기 뒤 포커스는 문서로 가는데 메뉴는 화면에 남고, 클릭이면 팔레트가 남은 채 필드에 포커스가 간다. main에서는 같은 누름이 닫았다(회귀). 시험 `field_drag_with_key_menu_open_closes_menu`, `field_drag_with_palette_open_closes_palette`, `field_click_with_palette_open_closes_palette` | `crates/keyscore-app/src/transport_drag.rs:237` |
| 2 | 제안 | 문서 변경 관찰자가 `cx.notify()`만 보고 끌기를 취소해, 되돌릴 것이 없는 `Cmd+Z`·`Cmd+Shift+Z`도 끌기를 취소한다(`edit_undo`·`edit_redo`는 결과와 상관없이 알린다). 7절 7은 「문서가 바뀌면」이다. 결정 문구를 「문서 알림이 오면」으로 고치거나, 리비전이 바뀐 때만 취소하라. 시험 `undo_with_empty_history_keeps_drag` | `crates/keyscore-app/src/window.rs:135-136`, `:570-573` |
| 3 | 수정 | 결정 7절 9(다른 입력란을 고치는 중 누름은 그 입력란 취소), 다른 입력란 위에서 뗌, UI 경로의 범위 끝·범위 밖 기준, SMF 템포 왕복(보고서가 리뷰어에게 주장한 동작)에 시험이 없었다. 이 리뷰의 통과한 시험이 그 자리를 채우므로 PR에 넣으면 된다 | `crates/keyscore-app/tests/transport_drag.rs` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다 (새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
