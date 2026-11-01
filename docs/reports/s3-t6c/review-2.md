# 적대적 리뷰: S3-T6c 트랜스포트 템포·박자표 필드 끌어서 조정 (리뷰 2회차)

- 리뷰어 세션: 구현자와도 1회차 리뷰어와도 다른 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: dff1c9c (`s3/t6c-transport-drag`, `git diff main...HEAD`). 1회차 이후 제품 변경은 `e2e2de0`(필드 누름이 `stop_propagation` 대신 `prevent_default`), `33612e2`(끄는 값이 바뀔 때만 끌기 취소)
- 깨뜨리기 시험: `crates/keyscore-app/tests/review_s3_t6c_r2.rs` (13개 중 2개 실패, 실패한 채로 둠)

## 0. 1회차 지적의 수정 확인

| 1회차 # | 지적 | 수정 | 확인 |
| --- | --- | --- | --- |
| 1 (수정) | 필드 누름의 캡처 단계 `stop_propagation`이 팔레트·팝업 메뉴의 `on_mouse_down_out`을 막아 닫히지 않음 | `e2e2de0`: `window.prevent_default()`로 바꿈(`transport_drag.rs:237-239`) | 해결. 1회차 실패 시험 3개(`field_drag_with_key_menu_open_closes_menu`, `field_drag_with_palette_open_closes_palette`, `field_click_with_palette_open_closes_palette`)가 통과한다. 이번 회차에 클릭 뒤 바로 친 값이 확정되는지(메뉴·팔레트 닫힘의 포커스 되돌림이 클릭 진입을 늦게 뺏지 않는지)까지 확인했다(시도 9·10) |
| 2 (제안) | 문서 알림만으로 끌기 취소(빈 undo도 취소) | `33612e2`: 틱 0의 끄는 값(템포는 10분의 1 BPM, 박자표)이 누를 때와 다를 때만 취소(`transport_drag.rs:355-373`), 결정 7절 7 문구도 고침 | 해결. `undo_with_empty_history_keeps_drag` 통과. 다른 필드 변경·redo·`SetTempo`도 이번 회차에 확인(시도 1–4) |
| 3 (수정) | 7절 9, 다른 입력란 위 뗌, 범위 끝, SMF 왕복 시험 없음 | 1회차 시험 파일을 PR에 둠 | 해결. `review_s3_t6c_r1.rs` 22개 모두 통과 |

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `prevent_default`로 입력란 `on_mouse_down`이 일찍 돌아가므로 `GlobalState::suppress_text_selection`도 불리지 않는다. Root의 창 수준 글자 선택(`TextSelectionLayer`, 버블 단계)이 이 누름에서 시작돼 끄는 동안 지나간 글자를 선택하지 않는가.
2. 전파가 이어져 겹 바깥 `focus_document_on_press`가 같은 누름을 받는다. 다른 입력란을 고치던 중이면 그 입력이 확정되거나, 클릭 진입한 필드 포커스가 뺏기지 않는가.
3. 팝업 메뉴의 `DismissEvent`(문서로 포커스)와 팔레트의 `close_palette`가 누름 뒤 효과 비우기에서 돈다. 클릭(뗌)으로 필드에 들어간 뒤 바로 친 값이 확정되는가.
4. 끄는 값이 아닌 변경(템포를 끄는 중 박자표 undo)에서 끌기는 이어지는가. 관찰자의 `sync_fields`가 끄는 필드의 글자(끌어서 고른 값)를 문서 값으로 덮지 않는가.
5. 끄는 값이 바뀌는 다른 길: redo, `transport::SetTempo`(값 있음), 탭 템포. 모두 취소되는가.
6. 비교가 10분의 1 BPM으로 반올림되므로 µs 500 001 → 500 000 같은 변경은 취소하지 않는다. 그때 뗌이 그 변경을 말없이 덮는가.
7. 끄는 중 그 필드에 포커스가 들어오면(`transport::SetTempo` 값 없음) 필드는 「고치는 중」(7절 2)인데, 움직임의 `set_value`가 친 글자를 덮고 뗌의 `reset_field`가 버리지 않는가.
8. 뗌 사건을 잃은 끌기: 창 전체 처리기는 `dragging()`인 움직임만 보고, 다른 곳의 새 누름은 남은 끌기를 지우지 않는다. 다음 아무 클릭의 뗌이 남은 끌기의 뗌이 되어 템포를 바꾸지 않는가.
9. 끄는 중 키로 팔레트·모드 메뉴를 열고 그 위(deferred 겹)에서 떼면 뗌이 창 전체 처리기에 닿는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 창 수준 글자 선택 | 없음 | |
| 2 바깥 처리기와 다른 입력란 | 일부 | r1 `pressing_tempo_while_editing_meter_cancels_meter_and_drags`는 끌기만. 클릭·undo 길이 없음 |
| 3 메뉴·팔레트 닫힘 뒤 클릭 진입 | 일부 | r1은 팔레트 클릭 뒤 포커스만 봄. 메뉴 클릭·친 값 확정 없음 |
| 4 끄는 값이 아닌 변경 | 일부 | r1 `undo_with_empty_history_keeps_drag`는 문서 무변경. 실제 다른 값 변경·끄는 글자 유지 없음 |
| 5 redo·SetTempo·탭 | 일부 | `document_change_during_drag_cancels`는 undo만 |
| 6 반올림이 같은 변경 | 없음 | |
| 7 끄는 중 필드 포커스 | 없음 | |
| 8 뗌을 잃은 끌기 | 없음 | |
| 9 deferred 겹 위의 뗌 | 없음 | r1 `release_over_meter_field_applies_tempo_only`는 같은 겹의 입력란 위 |

## 3. 깨뜨리기 시도

모든 시험은 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t6c_r2`로 실행했다. 같은 빌드에서 `transport_drag`(14)·`transport_drag_pure`(5)·`review_s3_t6c_r1`(22)도 모두 통과를 확인했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 박자표를 5/4로 끈 뒤, 템포를 끄는 중 `Cmd+Z`(박자표 되돌림) | 못 깨뜨림: 끌기 유지, 템포 글자 「122.0」 유지, 박자표 필드는 4/4, 뗌에서 123.0 | `tempo_drag_survives_meter_undo_and_keeps_text` |
| 2 | 박자표를 끄는 중 템포 편집 `Cmd+Z` | 못 깨뜨림 | `meter_drag_survives_tempo_undo` |
| 3 | 템포를 끄는 중 `edit::Redo`로 템포가 바뀜 | 못 깨뜨림: 취소, 뗌 편집 없음 | `redo_of_tempo_during_drag_cancels` |
| 4 | 템포를 끄는 중 `transport::SetTempo(90)` | 못 깨뜨림: 취소, 90 유지 | `set_tempo_action_during_drag_cancels` |
| 5 | µs 500 001에서 끄는 중 `SetTempo(120.0)`(µs 500 000, 같은 10분의 1) 뒤 제자리 뗌 | 못 깨뜨림: 취소되지 않지만 제자리 뗌은 편집이 없어 SetTempo가 남는다. 다른 값으로 떼면 그 위에 새 undo 단위로 쌓일 뿐 덮지 않는다 | `sub_tenth_change_does_not_cancel_and_is_not_lost` |
| 6 | 끄는 중 `transport::SetTempo`(값 없음)로 템포 필드에 들어가 「90」을 치고 더 끈 뒤 뗌 | 깨짐: 다음 움직임이 친 「90」을 「123.0」으로 덮고, 뗌은 123.0을 넣고 필드를 리셋해 친 글자가 말없이 사라진다 | `field_entered_during_drag_keeps_typed_text` (실패) |
| 7 | 템포를 끌다 뗌 없이(뗌 사건을 잃음) 주 뷰를 클릭 | 깨짐: 주 뷰 클릭의 뗌이 남은 템포 끌기의 뗌이 되어 템포가 105.0(µs 571 429)이 된다. 같은 클릭은 피아노롤 클릭으로도 처리된다 | `stale_drag_does_not_apply_on_later_click_elsewhere` (실패) |
| 8 | 템포 필드를 눌러 트랜스포트·주 뷰를 가로질러 상태줄 쪽까지 끌기 | 못 깨뜨림: 창 수준 글자 선택이 생기지 않는다(트랜스포트·주 뷰에 선택 참여 글자가 없어서다. 선택 참여 글자가 생기면 다시 볼 일) | `field_drag_does_not_start_window_text_selection` |
| 9 | 조성 메뉴가 열린 채 템포 필드 클릭 → 「90」 Enter | 못 깨뜨림: 메뉴 닫힘, 필드 진입, 90 확정 | `field_click_with_key_menu_open_enters_field` |
| 10 | 팔레트가 열린 채 템포 필드 클릭 → 「90」 Enter | 못 깨뜨림 | `field_click_with_palette_open_then_type_commits` |
| 11 | 박자표에 「3/4」를 친 채 템포 필드 클릭(전파가 바깥 처리기까지 감) | 못 깨뜨림: 박자표 취소(확정 아님), undo 길이 그대로, 템포 필드 진입 | `press_reaches_outer_handler_without_committing_other_field` |
| 12 | 끄는 중 키로 팔레트를 열고 팔레트 위에서 뗌 | 못 깨뜨림: 끌기 끝남 | `release_over_palette_opened_mid_drag_ends_drag` |
| 13 | 끄는 중 `mode::ShowMenu`로 모드 메뉴를 열고 메뉴 위에서 뗌 | 못 깨뜨림 | `release_over_mode_menu_opened_mid_drag_ends_drag` |

원인(시도 6): `transport_drag_move`(`transport_drag.rs:275-311`)는 필드 포커스를 보지 않고 매 움직임마다 `set_value`로 필드 글자를 쓴다. 뗌(`:313-350`)은 `reset_field`와 `focus_document`로 필드를 비운다. 누를 때만 「포커스가 있으면 입력란의 일」(7절 2)을 보고(`:233-236`), 끄는 중 포커스가 들어오는 경우는 다루지 않는다. 기본 키 묶음은 없지만(`commands/table.rs:1895`) 사용자가 키에 묶거나(KM-1) 끄는 중 팔레트로 부를 수 있다.

원인(시도 7): 창 전체 처리기(`transport_drag.rs:196-214`)는 버튼을 쥔 움직임과 왼쪽 뗌만 보고, 남은 끌기를 지우는 것은 뗌·`Esc`·끄는 값 변경뿐이다. 다른 곳의 왼쪽 누름이나 버튼을 쥐지 않은 움직임(뗌을 잃었다는 신호)은 끌기를 끝내지 않는다. 피아노롤은 새 누름이 제스처를 갈아 끼우므로(`pianoroll/element.rs:393-409`) 같은 상황에서 묵은 제스처가 다음 뗌에 붙지 않는다. 뗌을 잃는 길은 드물다(끄는 중 키로 띄운 시스템 모달 패널이 뗌을 가져감, 창 비활성 등). 시험은 뗌 없이 다음 누름을 보내는 것으로 그 상태를 만들었다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 문서는 뗄 때 한 번만, 끄는 값이 바뀌면 취소 | 아니오 | 시도 1–5 |
| 같은 값이면 편집하지 않음 | 아니오 | 시도 5, r1 시험 |
| 끄는 중 다른 필드·문서 표시가 어긋나지 않음 | 아니오 | 시도 1·2(끄는 글자 유지, 다른 필드는 문서 따라감) |
| 고치는 중 필드의 글자는 사람의 것(7절 2, S2-T5b) | 예 | 시도 6: 끄는 중 포커스가 들어온 필드의 글자를 움직임이 덮고 뗌이 버린다 |
| 템포 편집은 사람이 고른 끌기에서만 생긴다 | 예(드묾) | 시도 7: 뗌을 잃은 끌기가 다른 곳 클릭에 템포 편집을 만든다 |
| 모델 불변식(분모 2의 거듭제곱, 템포 범위) | 아니오 | 생성자 경유(`transport_drag.rs:57-63,94-124`) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.6 「클릭해 입력, 끌어서 조정」 | 충족 | 누름 `transport_drag.rs:223-273`, 끌기 `:275-311`, 뗌·클릭 `:313-350`. 시도 9·10으로 팝업이 열린 상태의 클릭 진입도 확인 |
| TRN-4 (곡 전체 하나, 틱 0) | 충족 | `transport_drag.rs:327-342` |
| P2 (`Esc`는 진행 중 조작 취소, 팝업·팔레트 닫기) | 충족 | `pianoroll_mouse.rs:46-62,67-75`. 필드 누름이 열린 팝업·팔레트를 닫음(지적 1회차 1 해결) |
| EDT-1 (드래그 한 번 = undo 한 단위) | 충족 | `window.rs:457-468`, r1 `two_drags_are_two_undo_units`, 시도 3 |

시도 6·7은 REQ 항목의 미충족이 아니라 결정 7절 2(고치는 중 필드)와 끌기 수명 관리의 결함으로 본다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. UI 스레드만 바뀐다. 끄는 중 엔진에 보내는 것이 없다.
- 제품 코드의 `unwrap`/`expect`: 없음(`unwrap_or`·`map_or`만).
- 에러가 삼켜지는 곳: `transport_drag.rs:200,209`의 `let _ = on_move.update(..)` — 창 엔티티가 사라진 경우뿐이라 정당. `edit_document` 실패는 상태줄로 알린다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 뗌 사건을 잃은 끌기가 남아 다음 다른 곳 클릭의 뗌에서 템포·박자표 편집이 들어간다(시도 7: 주 뷰 클릭으로 120 → 105 BPM, 같은 클릭이 피아노롤 클릭으로도 처리됨). 남은 끌기를 끝낼 신호가 없다. 예: 버튼을 쥐지 않은 움직임(`!event.dragging()`)이나 필드 밖의 새 왼쪽 누름이 오면 끌기를 취소한다. 시험 `stale_drag_does_not_apply_on_later_click_elsewhere` | `crates/keyscore-app/src/transport_drag.rs:196-214` |
| 2 | 제안 | 끄는 중 그 필드에 포커스가 들어오면(`transport::SetTempo` 값 없음을 키·팔레트로) 움직임이 친 글자를 덮고 뗌이 버린다. 7절 2의 「포커스가 있으면 입력란의 일」과 어긋난다. 끄는 필드가 포커스를 받으면 끌기를 취소하거나(문서 값으로 되돌리고 필드는 고치는 중으로 둠), 결정에 「끄는 중 필드 진입은 무시」를 적어라. 시험 `field_entered_during_drag_keeps_typed_text` | `crates/keyscore-app/src/transport_drag.rs:306-309`, `:347-348` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

1회차 지적은 모두 해결됐다. 지적 1(수정)은 드문 조건이지만 사람이 고르지 않은 편집이 문서에 들어가는 결함이고 고치는 범위가 작다. 지적 1을 고치고 지적 2를 받아들이거나 거절 이유를 보고서에 적은 뒤 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다 (새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
