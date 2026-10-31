# 적대적 리뷰: S3-T6a 마우스 선택 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: a5b8643 (제품 수정은 d766e23, 시험은 242798c, 그 뒤는 보고서 커밋)

범위: 리뷰 1 지적 1–5의 수정(d766e23·242798c)이 실제로 고쳤는가, 그리고 수정이 닿는 길(`live_gesture`, `apply_mode_event`의 조작 버리기, `Anchor`·`band_between`, `band_rect`의 뷰포트 변환)에서 새 결함이 생겼는가.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show d766e23`의 제품 코드와 지금의 `pianoroll_mouse.rs`, `pianoroll/mouse.rs`, `element.rs`(휠·끌기·뗌 처리기), `window.rs` `apply_mode_event`, `mode.rs` `apply`만 읽고, 시험(`tests/mouse_select*.rs`, `tests/review_s3_t6a.rs`)을 열기 전에 적었다.

1. 누른 점이 내용(`Anchor`)에 붙었지만 선택은 `update_band`(끌기)에서만 다시 계산된다. 휠 처리기(`element.rs` 432–456)는 뷰포트만 옮기고 조작을 건드리지 않는다. 누른 채 스크롤하고 포인터를 움직이지 않은 채 떼면, 마지막으로 그린 직사각형(새 뷰포트로 다시 그림)과 최종 선택이 다른가. 리뷰 1 시험은 스크롤 뒤 1 px 끌어 이 틈을 덮는다.
2. `Cmd+휠` 줌 중 누른 점이 내용에 붙는가(`x_of(anchor.tick)`가 화면 밖으로 나가도 직사각형이 가장자리에서 잘리고 선택은 누른 점을 품는가).
3. 세로 스크롤: `Anchor.row`가 내용 행인가, `anchor_at`의 `last_row + 0.999` 상한이 스크롤 뒤에도 누른 행을 바꾸지 않는가.
4. `apply_mode_event`의 버리기 조건 「모드 또는 뷰가 바뀜」: 선택만 바뀌는 사건(`ClearSelection`을 `apply_mode_event`로 넣는 경우)은 조작을 버리지 않아야 한다(러버밴드가 스스로 선택 사건을 낸다).
5. 누른 채 모드를 다녀오면(Normal → Step → Normal) 조작이 버려져 뗌은 아무 일도 하지 않아야 한다(결정 2절 1의 고친 글). 노트 줄이기 대기·빈 곳 누름의 선택 비우기가 다시 살아나지 않는가.
6. `cancel_piano_roll_gesture`가 이제 `live_gesture`를 먼저 부른다. 문서 변경 취소(`selection_document_changed`)는 Normal 피아노롤에서 여전히 `Cancelled`를 남겨 뗌까지 끌기를 무시하는가(버려 `None`이 되면 다음 끌기는 무시되나 의미가 달라진다).
7. `band_between`의 폭 0 넓히기 `max(lo + 1)`: `lo`가 `tick_u`의 포화값(`u64::MAX`)이면 넘친다. 공개 함수다.
8. `update_band`가 `m::band`의 빈 뷰포트 검사(너비·높이·행 높이 ≤ 0이면 `None`)를 거치지 않고 `band_between`을 바로 부른다. 빈 뷰포트에서 한 칸짜리 밴드가 생기는가.
9. 클립 고르기의 `press`가 `cell.tick`에서 `tick_u(anchor.tick)`로 바뀌었다. `cell_at`도 스냅하지 않은 내림 틱이므로 같은 값인가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 스크롤 뒤 움직이지 않고 뗌 | 없음 | `wheel_scroll_during_band_keeps_press_point_on_content`(리뷰 1)는 스크롤 뒤 1 px 끈다 |
| 2 줌 중 누른 점 | 없음 | `cmd_wheel_zoom_stops_at_bounds`는 조작 없이 배율만 |
| 3 세로 스크롤 중 누른 행 | 없음 | |
| 4 선택 사건이 조작을 버리지 않음 | 간접 | 러버밴드 시험 전체가 `Select`를 거치며 통과한다. `apply_mode_event`로 넣는 선택 사건은 없음 |
| 5 모드 왕복 뒤 뗌 | 없음 | 리뷰 1 시험은 Step·배치 뷰로 간 채 끝난다 |
| 6 문서 변경 취소 뒤 끌기 | 있음 | `document_change_during_band_cancels_gesture` |
| 7 `band_between` 상한 | 없음 | |
| 8 빈 뷰포트 | 없음 | 창 시험으로 뷰포트를 0으로 만들 길을 찾지 못해 시도하지 않았다(3절에 적지 않음) |
| 9 `press` 틱 의미 | 간접 | `rubber_band_across_clips_picks_press_clip`, `band_clip_prefers_press_then_now_then_first_overlap` |
| 리뷰 1 지적 4의 시험 | 있음 | `escape_during_plain_press_cancels_release_action`, `shift_click_empty_without_base_is_plain_click`, `cmd_wheel_zoom_stops_at_bounds` |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t6a_r2.rs`에 있고 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t6a_r2`로 실행했다(8개 중 6 통과, 2 실패). 같은 트리에서 `review_s3_t6a` 10, `mouse_select` 26, `mouse_select_pure` 7 모두 통과(리뷰 1의 실패 시험 6개가 이제 통과한다).

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 러버밴드(틱 100 → 2000) 중 휠로 내용을 2000틱 옮기고 포인터를 움직이지 않은 채 뗀다 | 깨짐: 직사각형은 틱 100–4000으로 다시 그려져 N4·N5를 덮는데, 끄는 동안의 선택과 뗀 뒤 선택은 `[N1, N2, N3]`에 머문다. 화면이 보여 준 범위와 결과 선택이 다르다 | `wheel_scroll_then_release_without_move_selects_drawn_band` |
| 2 | 러버밴드 중 `Cmd+휠` 두 단계 확대(누른 점이 왼쪽 밖으로 나감) 뒤 1 px 끈다 | 못 깨뜨림: N1이 선택에 남는다 | `cmd_wheel_zoom_during_band_keeps_press_point_on_content` |
| 3 | 러버밴드(음 64 → 62) 중 세로로 4행 스크롤 뒤 1 px 끈다 | 못 깨뜨림: 누른 행의 N3이 남는다 | `vertical_scroll_during_band_keeps_press_row_on_content` |
| 4 | 선택된 노트를 누른 채(줄이기 대기) Step → Normal 왕복 뒤 뗌, 빈 곳 누름도 같은 왕복 뒤 뗌 | 못 깨뜨림: 조작이 버려지고 선택 `[N1, N2]`가 그대로다 | `mode_round_trip_during_press_makes_release_inert` |
| 5 | 러버밴드 중 `apply_mode_event(ClearSelection)` 뒤 끌기 | 못 깨뜨림: 조작이 살아 있고 끌기가 선택을 다시 만든다 | `selection_only_event_keeps_band_alive` |
| 6 | 러버밴드 중 `Cmd+Z`(문서 변경) 뒤 먼 곳으로 끌기 | 못 깨뜨림: 선택이 바뀌지 않고 뗀 뒤 조작이 없다 | `undo_during_band_then_drag_is_ignored` |
| 7 | `band_between`에 틱 상한 밖의 같은 두 점(`1e30`) | 깨짐: `lo + 1`이 넘쳐 디버그 빌드에서 패닉(`mouse.rs:53`). 창 경로로는 닿기 어렵다(첫 틱을 2^64까지 스크롤해야 함) | `band_between_at_tick_ceiling_does_not_overflow` |
| 8 | 폭 0(정수 틱 500) 두 점의 범위 | 못 깨뜨림: `[500, 501)`, 행 양끝 포함 | `band_between_zero_width_is_one_tick` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 뷰를 바꾸면 노트 선택이 비고 FSM 「선택 없음」(S3-T2) | 아니오 | 리뷰 1 시도 3이 이제 통과(`apply_mode_event`가 조작을 버리고, `live_gesture`가 배치 뷰의 `Esc`를 가로채지 않음) |
| 마우스 선택은 Normal 피아노롤에서만(결정 2절 1) | 아니오 | 끌기·뗌·`Esc`가 `live_gesture`를 거친다. 시도 4 |
| 진행 중 조작은 누름 하나에 하나, 뗄 때·모드 전환 때 없어짐 | 아니오 | 시도 4, 리뷰 1 시도 2 |
| 끄는 동안 선택 = 그린 직사각형 안의 노트(결정 3절 5, MS-1) | 예 | 시도 1: 휠·줌은 직사각형만 다시 그리고 선택은 다음 끌기까지 옛 범위다 |
| 선택은 한 클립 | 아니오 | 모든 선택이 `Selection::set`을 거친다 |
| 문서는 `apply`·undo로만 바뀐다 | 아니오 | 이번 수정은 편집 경로를 건드리지 않았다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.4 빈 곳 드래그(러버밴드), `Cmd+드래그` 추가 | 부분 충족 | 누른 점 고정은 고쳐졌다(`mouse.rs:24-63`, 시도 2·3). 그러나 누른 채 스크롤·줌하면 직사각형과 선택이 어긋난 채 끝날 수 있다(시도 1, `pianoroll_mouse.rs:191-207`, `element.rs:432-456`) |
| MS-1(러버밴드 미리보기) | 부분 충족 | 위와 같은 원인. 미리보기 강조가 그린 직사각형을 따르지 않는다 |
| P2 「`Esc`는 진행 중인 조작을 취소」·5.5 「가장 안쪽 하나만」 | 충족 | `pianoroll_mouse.rs:45-71`, `:83-89`. 리뷰 1 시도 3 통과 |
| 결정 2절 1(Normal 피아노롤에서만, 모드·뷰가 바뀌면 버림) | 충족 | `window.rs:318-322`, `pianoroll_mouse.rs:74-89`. 시도 4·5 |
| 결정 3절 5(누른 점은 내용 위치, 폭 0이면 한 틱) | 부분 충족 | 기하는 맞다(시도 2·3·8). 「끄는 동안 선택 강조가 실시간으로 바뀐다」가 스크롤·줌에서 지켜지지 않는다(시도 1) |
| 7.4 클릭·`Cmd+클릭`·`Shift+클릭`·더블클릭, 같은 Action·선택 상태 | 충족 | 이번 수정이 닿지 않았고 기존 시험 26개가 통과 |
| MS-7(`Cmd+휠`, 핀치 가로 줌) | 충족 | `element.rs:421-446`, `cmd_wheel_zoom_stops_at_bounds` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. UI 스레드만 바뀌었다.
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음. 다만 `band_between`의 `lo + 1`(`mouse.rs:53`)은 산술 넘침 패닉 지점이다(시도 7, 디버그 빌드).
- 에러가 삼켜지는 곳: 새로 생긴 것 없음. `live_gesture`가 조작을 조용히 버리는 것은 결정 2절 1에 적힌 동작이다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 1 지적 3의 수정이 반쪽이다. 누른 점은 내용에 붙었고 직사각형도 그릴 때 새 뷰포트로 바뀌지만, 선택은 `update_band`(끌기)에서만 다시 계산된다. 누른 채 휠 스크롤·`Cmd+휠`·핀치 줌을 하고 포인터를 움직이지 않은 채 떼면, 마지막으로 보인 직사각형과 최종 선택이 다르다(직사각형 틱 100–4000, 선택 `[N1, N2, N3]`, N4·N5 빠짐). 결정 3절 5 「끄는 동안 선택 강조가 실시간으로 바뀐다」·MS-1 미리보기와 어긋난다. 리뷰 1 시험은 스크롤 뒤 1 px 끌어 이 틈을 덮었다. 뷰포트가 바뀔 때(휠·줌 처리기 뒤) 진행 중 러버밴드를 `now`로 다시 계산하거나, 뗌에서 마지막 점으로 한 번 더 계산해야 한다. 시험: `wheel_scroll_then_release_without_move_selects_drawn_band` | `crates/keyscore-app/src/pianoroll_mouse.rs:191-207`, `:209-254`; `crates/keyscore-app/src/pianoroll/element.rs:421-456` |
| 2 | 제안 | `band_between`의 폭 0 넓히기 `tick_u(..).max(lo + 1)`이 `lo == u64::MAX`(포화)에서 넘쳐 디버그 빌드에서 패닉한다. 공개 함수지만 창 경로로는 사실상 닿지 않는다. `lo.saturating_add(1)`이면 된다. 시험: `band_between_at_tick_ceiling_does_not_overflow` | `crates/keyscore-app/src/pianoroll/mouse.rs:53` |
| 3 | 제안 | `update_band`가 `m::band`의 빈 뷰포트 검사를 거치지 않고 `band_between`을 바로 부른다(수정 전에는 너비·높이·행 높이가 0이면 `None`). 뷰포트가 비면 `first_tick`·`first_row` 한 칸 밴드가 노트를 고를 수 있다. 창 시험으로 재현하지 못했으므로 제안으로 둔다. `update_band` 앞에서 같은 검사를 하거나 `anchor_at`에 같은 가드를 둔다 | `crates/keyscore-app/src/pianoroll_mouse.rs:218-225` |

리뷰 1 지적 1(차단)·2·4와 제안 5는 고쳐졌다. 리뷰 1 시험 6개가 모두 통과하고, 바뀐 리뷰 시험 `drag_from_outside_after_return_does_not_resume_stale_band`의 단언 변경은 전제(조작이 남음)만 바꾸고 결함을 단언하던 뒤 단언을 그대로 두었다(정당). 새 시험 3개가 리뷰 1 지적 4의 빈 곳을 덮는다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(수정)을 고친 뒤 재리뷰한다. 재리뷰는 휠·줌 처리기와 뗌에서 러버밴드를 다시 계산하는 길로 좁힌다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(d766e23·242798c는 `pianoroll_mouse.rs`, `pianoroll/{mouse,mod,element}.rs`, `window.rs`의 조작 버리기 다섯 줄, 결정·계획 문구, 시험만 바꿨다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(242798c는 추가만, `review_s3_t6a.rs`는 전제 단언 한 줄 교체)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
