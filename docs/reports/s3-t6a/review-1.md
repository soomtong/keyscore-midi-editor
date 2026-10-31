# 적대적 리뷰: S3-T6a 마우스 선택 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 1660a5c (제품 코드는 c01c466, 그 뒤는 보고서 커밋)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일(`tests/mouse_select*.rs`)을 열기 전에 `pianoroll_mouse.rs`, `pianoroll/mouse.rs`, `pianoroll/element.rs`, `pianoroll_window.rs`, `selection.rs`·`window.rs`·`note_edit.rs` diff만 읽고 적었다.

1. 진행 중 조작(`Inner.gesture`)의 수명: 누를 때 생기고 뗄 때(`piano_roll_release`) 없어진다. 뗌은 피아노롤 Element가 그려질 때만 등록되므로, 누른 채 뷰가 배치 뷰로 바뀌면 뗌이 닿지 않아 조작이 남는가. 남으면 다음 `Esc`(가로채기)와 다음 끌기(`on_drag`는 누른 자리를 묻지 않는다)가 무엇을 하는가.
2. 결정 2절 1(마우스 선택은 Normal에서만): `mouse_edits`는 누름에서만 본다. `piano_roll_drag`·`piano_roll_release`는 모드를 보지 않는다. 누른 채 Step으로 가면 끌기가 러버밴드를 이어 가는가.
3. 러버밴드의 「누른 점」은 뷰 기준 px(`origin`)로 저장되고, 클립 고르기의 `press`만 틱이다. 누른 채 휠 스크롤·`Cmd+휠` 줌을 하면 직사각형의 한 끝이 내용에서 미끄러지는가(결정 3절 5 「누른 점과 지금 점이 마주 보는 직사각형」).
4. `band`의 틱 범위 `[floor, ceil)`: 두 x가 같은 정수 틱이면 `[t, t)`라 빈 범위다. 세로로만 끈 러버밴드가 가로지르는 노트를 고르는가.
5. `Esc` 가로채기: 러버밴드뿐 아니라 끌지 않은 누름(`Gesture::Note`·`Empty`)에서도 `Esc`를 먹는다. 뗀 뒤의 `Esc`는 평소대로 선택 해제로 가는가.
6. 더블클릭 삭제의 `window.defer`: 같은 Action(`edit::Delete`)이 vim 프리셋에서도 한 undo 단위로 그 노트만 지우는가. `Cmd` 더블클릭에서 첫 누름이 그 노트를 선택에서 뺀 뒤 둘째 누름이 다른 선택 노트까지 지우지 않는가.
7. 마우스 전용 편집 경로가 있는가: 삭제는 `run_action(edit::Delete)`, 생성은 `plan_paste`+`apply_placed`(붙여넣기와 같은 계산·적용). 선택은 `set_selection`·`replace_selection`만 쓰는가(`sync_selection_mode`를 거치는가).
8. 4 px 여백: 여백 누름은 커서·조작을 만들지 않고, 바로 안쪽 첫 픽셀은 눌리는가.
9. `zoom_at`: 배율 상·하한, NaN·무한대, `first_tick` 0 밑 자르기, 커서를 따라가지 않음.
10. 문서 변경 취소(`selection_document_changed` → `cancel_piano_roll_gesture(false)`)가 낡은 `before`를 다시 쓰지 않는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 피아노롤을 떠날 때 조작 수명 | 없음 | `release_outside_piano_roll_finishes_band`는 피아노롤이 그려진 채 밖에서 떼는 경우뿐 |
| 2 누른 채 모드 변경 | 없음 | `step_mode_click_only_moves_cursor`는 Step에서 누르는 경우뿐 |
| 3 누른 채 스크롤·줌 | 없음 | |
| 4 폭 0 러버밴드 | 없음 | `band_covers_ticks_floor_ceil_and_rows_inclusive`는 폭이 있는 경우뿐 |
| 5 끌지 않은 누름의 `Esc`, 뗀 뒤 `Esc` | 일부 | 러버밴드 중 `Esc`만 있다(`escape_during_rubber_band_restores_selection_and_keeps_mode`). 노트·빈 곳 누름 중 `Esc` 뒤 뗌의 결과는 시험이 없다 |
| 6 더블클릭 삭제의 Action·defer | 일부 | 기본 프리셋 한 경우(`double_click_note_deletes_it_with_one_undo`). vim·`Cmd` 경우 없음 |
| 7 같은 Action·상태 | 있음 | `mouse_selection_then_alt_k_transposes`, `rubber_band_from_gap_..._alt_k_works`, 더블클릭 undo 시험 |
| 8 4 px 여백 | 일부 | 안쪽 첫 픽셀만(`edge_pixel_of_piano_roll_is_clickable`). 여백 누름이 아무것도 하지 않음은 없음 |
| 9 줌 경계 | 일부 | 배율·포인터 고정만. 상·하한·0 자르기 없음 |
| 10 문서 변경 취소 | 있음 | `document_change_during_band_cancels_gesture` |
| `Shift+클릭`(빈 곳), `Cmd+드래그`의 선택 클립 우선(UI) | 없음 | 순수 시험 `band_clip_prefers_...`가 `preferred`를 덮는다 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t6a.rs`에 있고 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t6a`로 실행했다(10개 중 4 통과, 6 실패). 기존 `mouse_select`(23)·`mouse_select_pure`(7)는 같은 트리에서 모두 통과.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 빈 곳을 누른 채 Step으로 바꾸고 노트 위로 끈다 | 깨짐: Step에서 선택 `[N2, N3]`가 생긴다(결정 2절 1 위반) | `band_drag_after_entering_step_mode_does_not_select` |
| 2 | 러버밴드 중 배치 뷰로 바꾸고 뗀다 | 깨짐: 뗌이 닿지 않아 `gesture_active()`가 참으로 남는다 | `gesture_does_not_survive_leaving_piano_roll` |
| 3 | 2에 이어 배치 뷰에서 `Esc` | 깨짐: 남은 러버밴드가 `Esc`를 가로채 누르기 전 노트 선택 `[N1]`을 되살리고, 배치 뷰 FSM이 「선택 있음」이 된다(Esc 전에는 거짓). 뷰 전환이 노트 선택을 비운다는 S3-T2 규칙이 마우스 경로로 깨진다 | `escape_in_arrangement_is_not_taken_by_stale_band` |
| 4 | 2에 이어 피아노롤로 돌아와 피아노롤 밖(트랜스포트 쪽)에서 눌러 노트 위로 끈다 | 깨짐: 피아노롤이 받지 않은 끌기가 남은 러버밴드를 이어 클립 A 노트 6개를 모두 고른다. 4 px 여백에서 시작하면 분할 손잡이가 끌기를 가져가 재현되지 않는다 | `drag_from_outside_after_return_does_not_resume_stale_band` |
| 5 | 러버밴드(틱 100 → 2000) 중 내용을 1000틱 가로 스크롤하고 1 px 더 끈다 | 깨짐: 누른 점이 화면 px에 붙어 있어 틱 1100으로 미끄러지고 N1이 선택에서 빠진다 | `wheel_scroll_during_band_keeps_press_point_on_content` |
| 6 | 세로로만 끈 러버밴드(x가 정수 틱 500) | 깨짐: `Band { lo: 500, hi: 500 }`이라 가로지르는 노트 `[0, 960)`을 고르지 않는다 | `vertical_band_on_integer_tick_selects_crossed_note` |
| 7 | 4 px 여백 누름 | 못 깨뜨림: 커서·조작이 생기지 않고 안쪽 1 px은 눌린다 | `press_in_margin_is_inert` |
| 8 | vim 프리셋 더블클릭 삭제 | 못 깨뜨림: 그 노트만 지워지고 undo 하나, 선택 비움 | `vim_double_click_note_deletes_with_one_undo` |
| 9 | 선택된 두 노트 중 하나를 `Cmd` 더블클릭 | 못 깨뜨림: 그 노트만 지워지고 다른 노트는 남는다 | `cmd_double_click_selected_note_deletes_only_it` |
| 10 | 뗀 뒤 `Esc` | 못 깨뜨림: 가로채기가 먹지 않고 선택 해제로 간다 | `escape_after_release_clears_selection` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 선택은 한 클립 | 아니오 | 모든 선택이 `Selection::set`(한 클립)을 거친다 |
| 뷰를 바꾸면 노트 선택이 비고 FSM 「선택 없음」(S3-T2, `apply_mode_event`) | 예 | 시도 3: 러버밴드 중 `ToggleView` → 뗌 → 배치 뷰 `Esc` → `cancel_piano_roll_gesture(true)`가 `set_selection(before)` → 배치 뷰 FSM `Select` |
| 마우스 선택은 Normal에서만(결정 2절 1) | 예 | 시도 1: `piano_roll_drag`가 모드를 보지 않는다 |
| 오퍼레이터 대기와 선택은 함께 없다(S3-T3c) | 아니오 | 누름이 `click_piano_roll_cell` → `clear_operator`, 선택 변경은 모두 `set_selection` → `sync_selection_mode` |
| 비주얼은 기준 칸이 있어야 한다 | 아니오 | 누름이 `set_anchor(None)`(비주얼도 끝) |
| 문서는 `apply`·undo로만 바뀐다(마우스 전용 편집 경로 없음) | 아니오 | 삭제 `run_action(edit::Delete)`, 생성 `plan_paste`+`apply_placed` |
| 진행 중 조작은 누름 하나에 하나, 뗄 때 없어짐 | 예 | 시도 2: 뗌이 그려지지 않은 Element에 닿지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.4 클릭(그 노트만) | 충족 | `pianoroll_mouse.rs:117-121`, `:180-183` |
| 7.4 `Cmd+클릭` 추가/제거 | 충족 | `pianoroll_mouse.rs:274-286` |
| 7.4 `Shift+클릭` 마지막 선택부터 시간 범위 | 충족 | `pianoroll_mouse.rs:289-333` |
| 7.4 빈 곳 드래그(러버밴드), `Cmd+드래그` 추가 | 부분 충족 | `pianoroll_mouse.rs:194-240`. 누른 채 스크롤하면 누른 점이 내용에서 미끄러진다(시도 5), 폭 0 밴드(시도 6) |
| 7.4 더블클릭 빈 곳 생성 / 노트 삭제 | 충족 | `pianoroll_mouse.rs:95-108`, `:336-376` |
| 7.4 「마우스 조작은 키 조작과 같은 Action을 호출, 선택 상태 공유」 | 충족 | `pianoroll_mouse.rs:100-102`(`edit::Delete`), 선택은 `set_selection`. 시험 `mouse_selection_then_alt_k_transposes` |
| 7.4 「마우스 선택은 `v` 비주얼 모드와 같은 선택 상태」 | 충족 | `selection.rs` `sync_selection_mode` 경유 |
| MS-1(러버밴드 미리보기) | 충족 | `pianoroll_mouse.rs:231-239`, `element.rs` 직사각형 그리기 |
| MS-5 | 충족 | `pianoroll_mouse.rs:90`(`click_piano_roll_cell`) |
| MS-7(`Cmd+휠`, 핀치 가로 줌) | 충족 | `element.rs:421-431`, `:440-446`, `pianoroll/mod.rs:647-657` |
| P2 「`Esc`는 진행 중인 조작을 취소」·5.5 「가장 안쪽 하나만」 | 부분 충족 | 피아노롤 안에서는 맞다. 그러나 남은 조작이 배치 뷰의 `Esc`를 먹고 노트 선택을 되살린다(시도 3, `pianoroll_mouse.rs:24-68`) |
| 결정 2절 1(Normal에서만) | 부분 충족 | 누름만 검사(`pianoroll_mouse.rs:83`), 끌기·뗌은 검사 없음(`:144-191`) |
| 결정 5절(4 px 여백) | 충족 | `pianoroll_window.rs` 래퍼 `p(4px)`, 시도 7 |
| PRE-1(더블클릭 생성 미리듣기) | 충족(코드 확인만) | `pianoroll_mouse.rs:374`(`cursor_moved` → `preview_cursor_note`). MIDI 시험은 돌리지 않았다 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 UI 스레드만 건드린다.
- 제품 코드의 `unwrap`/`expect`: 없음. `unwrap_or(false)`(`pianoroll_mouse.rs:36`, 창 엔티티가 사라졌으면 가로채지 않음)와 `unwrap_or(u32::MAX)`(`:351`, 길이 코드 틱은 u32 안이라 닿지 않음)뿐이다.
- 에러가 삼켜지는 곳: `extend_by_time`에서 클립을 못 찾으면 조용히 돌아간다(`pianoroll_mouse.rs:318-320`, 방금 커서가 찾은 클립이라 닿지 않음). `create_note_at`의 `planned == None`(트랙 없음)도 조용하다(`:367`). 생성 실패(`plan_paste` 에러)는 상태줄로 알린다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 진행 중 마우스 조작이 피아노롤·Normal을 떠나도 끝나지 않는다. 러버밴드 중 배치 뷰로 가면 뗌이 닿지 않아 조작이 살아 남고, (가) 배치 뷰의 첫 `Esc`를 가로채 누르기 전 노트 선택을 되살려 배치 뷰 FSM을 「선택 있음」으로 만들고(뷰 전환이 노트 선택을 비운다는 S3-T2 규칙 위반), (나) 피아노롤로 돌아온 뒤 피아노롤 밖에서 시작한 끌기가 그 러버밴드를 이어 선택을 바꾼다. `on_drag`·`on_release`가 창의 모든 왼쪽 끌기·뗌을 받는데 「이 누름이 피아노롤에서 시작했는가」를 조작 수명이 보장하지 못한다. 모드·뷰 전환(`apply_mode_event`)에서 조작을 버리는 길이 없다. 시험: `gesture_does_not_survive_leaving_piano_roll`, `escape_in_arrangement_is_not_taken_by_stale_band`, `drag_from_outside_after_return_does_not_resume_stale_band` | `crates/keyscore-app/src/pianoroll_mouse.rs:24-68`, `:144-191`; `crates/keyscore-app/src/window.rs:313-320`; `crates/keyscore-app/src/pianoroll/element.rs:378-396` |
| 2 | 수정 | 결정 2절 1(마우스 선택은 Normal에서만) 불일치: 빈 곳을 누른 채 Step으로 가면 끌기가 러버밴드를 이어 Step에서 선택을 만든다. 누름만 `mouse_edits`를 보고 끌기·뗌은 보지 않는다. 지적 1과 같은 뿌리(모드 전환이 조작을 끝내지 않음)다. 시험: `band_drag_after_entering_step_mode_does_not_select` | `crates/keyscore-app/src/pianoroll_mouse.rs:144-174` |
| 3 | 수정 | 러버밴드의 누른 점이 뷰 기준 px(`origin`)로 저장되어, 누른 채 휠 스크롤(또는 `Cmd+휠`·핀치 줌)하면 직사각형 한 끝이 내용에서 미끄러진다. 클립 고르기는 틱(`press`)을 쓰므로 둘이 어긋난다. 결정 3절 5 「누른 점과 지금 점이 마주 보는 직사각형」과 다르다. 누른 점을 (틱, 행)으로 두고 매번 뷰 px로 바꿔야 한다. 시험: `wheel_scroll_during_band_keeps_press_point_on_content` | `crates/keyscore-app/src/pianoroll_mouse.rs:132-137`, `:194-218`; `crates/keyscore-app/src/pianoroll/mouse.rs:26-52` |
| 4 | 수정 | 누락 시험: 끌지 않은 누름(노트 위 `Gesture::Note`, 빈 곳 `Gesture::Empty`) 중 `Esc` 뒤 뗌의 결과(선택 유지, 접기 없음), 빈 곳 `Shift+클릭`(결정 3절 4 「누른 틱 한 틱」), 줌 상·하한. 동작은 코드에 있으나 시험이 고정하지 않는다 | `crates/keyscore-app/tests/mouse_select.rs` |
| 5 | 제안 | 세로로만 끈 러버밴드의 x가 정수 틱이면 `[t, t)`라 화면에서 가로지르는 노트를 고르지 않는다(f32 좌표라 드물다). 결정 3절 5의 「내림·올림」을 문자 그대로 따른 결과라 제안으로 둔다. `hi`를 `max(ceil, lo + 1)`로 하거나 결정 문서에 「폭 0은 비어 있다」를 적는다. 시험: `vertical_band_on_integer_tick_selects_crossed_note` | `crates/keyscore-app/src/pianoroll/mouse.rs:46-48` |
| 6 | 제안 | 보고서는 UI 시험을 창 처리기 뒤에 썼다고 적었다(시험 먼저 규칙과 다름). 숨기지 않은 것은 좋으나, 이번 수정(지적 1–3)은 위 리뷰 시험이 먼저 실패한 상태에서 고치면 규칙을 지킬 수 있다 | `docs/reports/s3-t6a/report.md` 「테스트 결과」 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(차단)과 2·3·4(수정)를 고친 뒤 재리뷰한다. `review_s3_t6a.rs`의 실패 시험 6개 가운데 지적 5의 시험은 제안을 거절하면 기대를 「폭 0은 빈 선택」으로 바꿔 남긴다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`vim.rs`의 시그니처 변경은 `window.rs` clippy 줄 수 상한 때문이라 보고서에 적혀 있고 동작 변화 없음)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
