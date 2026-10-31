# 적대적 리뷰: S3-T6a 마우스 선택 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 2bc6646 (제품 수정은 3318d83, 그 뒤는 보고서 커밋)

범위: 리뷰 2 지적 1(수정)과 제안 2·3의 수정(3318d83)이 실제로 고쳤는가, 그리고 수정이 닿는 길(휠·`Cmd+휠`·핀치 → `on_view_change` → `piano_roll_view_changed`, 뗌의 `update_band`, `register_view_input`으로 옮긴 휠 처리, `band_between`의 `saturating_add`, `update_band`의 빈 뷰포트 검사)에서 회귀가 생겼는가.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show 3318d83`과 지금의 `pianoroll_mouse.rs`(누름·끌기·뗌·`piano_roll_view_changed`·`update_band`·`live_gesture`·`cancel_piano_roll_gesture`), `pianoroll/element.rs`(`register_mouse`, `register_view_input`), `pianoroll/mouse.rs`(`anchor_at`, `band_between`, `Gesture::band_rect`), `pianoroll/mod.rs`(`zoom`, `zoom_at`, `scroll_by`, `drop_gesture`), `window.rs` `apply_mode_event`·`set_selection`만 읽고, 시험을 열기 전에 적었다.

1. 핀치 처리기가 `on_view_change`를 부르는가. `zoom_at`은 포인터 아래 틱을 제자리에 두므로 보통은 범위가 바뀌지 않는다. 바뀌는 경우는 첫 틱이 0에 걸리는 축소다. 이때 포인터를 움직이지 않아도 선택이 새 직사각형을 따르는가.
2. `Cmd+휠` 줌 가지가 `return`을 잃고 스크롤 가지와 합쳐졌다. 줌 뒤에도 같은 콜백을 부르고, 줌 뒤 스크롤이 함께 일어나지 않는가(예전 `return`이 하던 일).
3. 뷰포트를 바꾸는 길은 휠·핀치만이 아니다. 키 줌(`cmd-=`·`cmd--`·`cmd-0`, vim `z l`·`z h`·`z z`)은 `view_zoom_*` 처리기에서 `zoom`을 부르고 `on_view_change`를 거치지 않는다. 누른 채 키로 줌하면 직사각형은 다시 그려지는데 끄는 동안의 선택은 옛 범위에 머무는가. 뗌의 재계산이 마지막 선택만은 맞추는가.
4. `Esc`(취소 상태 `Cancelled`) 뒤 휠·핀치: `piano_roll_view_changed`는 `Gesture::Band`만 고르므로 되살리지 않아야 한다. 뗌도 `_ => return`이어야 한다.
5. 문턱 전(`Gesture::Empty`)에 휠: 다시 계산하지 않고, 뗌은 빈 곳 클릭(선택 비우기)이어야 한다. `Empty`가 밴드로 바뀌면 안 된다.
6. `Cmd+드래그`(`add`) 중 여러 번 다시 계산: 매번 `before`에서 새로 합치는가. 스크롤로 넓혔다 되돌리면 넓힌 동안 들어온 노트가 빠지는가(누적되지 않는가).
7. 조작이 없거나 노트 누름(`Gesture::Note`) 중 휠은 선택을 바꾸지 않아야 하고, 노트 누름의 뗌(줄이기)은 그대로여야 한다.
8. 모드가 바뀐 뒤(Step 피아노롤에도 같은 Element가 있다) 휠: `live_gesture`가 조작을 버리고 선택을 바꾸지 않는가. 돌아온 뒤 휠·뗌도 아무 일도 하지 않는가.
9. 뗌의 `update_band`: 맨 위에서 `gesture = None`을 한 뒤 `update_band`가 `Gesture::Band`를 다시 넣고 `drop_gesture`로 지운다. 빈 뷰포트로 일찍 돌아오면 조작이 남지 않는가(순서상 `drop_gesture`가 늘 불리므로 남지 않을 것).
10. `band_between`의 `saturating_add(1)`: 넘침은 사라지지만 상한에서는 `[MAX, MAX)`로 빈 범위다(「폭 0이면 한 틱」과 다름). 창 경로로 닿는가.
11. `register_view_input`으로 옮긴 휠·핀치가 예전 동작(`should_handle_scroll`, `Shift+휠` 가로, `Cmd+휠` 경계)을 그대로 지키는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 핀치 뒤 움직이지 않고 뗌 | 없음 | `pinch_zooms_around_pointer`(`mouse_select.rs`)는 조작 없이 배율만 |
| 2 `Cmd+휠` 줌 뒤 움직이지 않고 뗌 | 없음 | 리뷰 2 `cmd_wheel_zoom_during_band_keeps_press_point_on_content`는 줌 뒤 1 px 끈다. 경계·스크롤 혼입은 `cmd_wheel_zoom_stops_at_bounds` |
| 3 키 줌 중 미리보기 | 없음 | |
| 4 `Esc` 뒤 휠 | 없음 | `Esc` 시험은 휠 없이 끝난다 |
| 5 문턱 전 휠 | 없음 | |
| 6 `Cmd+드래그` 중 스크롤 왕복 | 없음 | |
| 7 조작 없음·노트 누름 중 휠 | 간접 | 휠 시험 다수가 조작 없이 돌지만 선택을 단언하지 않음 |
| 8 Step에서 휠 | 없음 | 리뷰 2 `mode_round_trip_during_press_makes_release_inert`는 휠 없음 |
| 9 뗌 뒤 조작이 남지 않음 | 간접 | 러버밴드 시험들이 뗀 뒤 `gesture_active`를 본다 |
| 10 상한 빈 범위 | 있음(다른 단언) | 리뷰 2 `band_between_at_tick_ceiling_does_not_overflow`는 패닉만 본다 |
| 11 옮긴 휠 처리 | 있음 | `mouse_select.rs` 휠·핀치 시험, `review_s3_t6a.rs` |
| 리뷰 2 지적 1 | 있음 | `wheel_scroll_then_release_without_move_selects_drawn_band`(이제 통과) |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t6a_r3.rs`에 있고 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t6a_r3`로 실행했다(9개 중 7 통과, 2 실패). 같은 트리에서 `review_s3_t6a_r2` 8, `review_s3_t6a` 10, `mouse_select` 26, `mouse_select_pure` 7, `pianoroll` 13 모두 통과(리뷰 2의 실패 시험 2개가 이제 통과한다). 새 시험 파일은 `cargo fmt --all -- --check`와 `cargo clippy -p keyscore-app --test review_s3_t6a_r3 -- -D warnings`를 통과한다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 러버밴드(틱 100 → 2000) 중 첫 틱 0에서 핀치 축소(`delta -0.5`), 포인터를 움직이지 않고 뗌 | 못 깨뜨림: 핀치 직후 선택이 `[N1..N5]`로 넓어지고 뗀 뒤도 같다 | `pinch_zoom_out_during_band_recomputes_without_move` |
| 2 | 같은 밴드에서 `Cmd+휠` 축소 세 단계 뒤 움직이지 않고 뗌 | 못 깨뜨림: 줌 직후·뗀 뒤 모두 `[N1..N5]`, 직사각형 오른쪽 약 3906 | `cmd_wheel_zoom_out_during_band_recomputes_without_move` |
| 3 | 같은 밴드에서 누른 채 `cmd--`(`view::ZoomOut`) 세 번 | 깨짐(끄는 동안만): 직사각형은 틱 100–3906으로 다시 그려지는데 선택은 `[N1, N2, N3]`에 머문다. 뗀 뒤 선택은 `[N1..N5]`로 맞다(뗌의 재계산) | `keyboard_zoom_during_band_updates_preview` |
| 4 | N6 선택 → 밴드 → `Esc` → 휠·핀치 → 뗌 | 못 깨뜨림: 선택이 `[N6]`에 머물고 직사각형이 없다 | `wheel_after_escape_does_not_revive_band` |
| 5 | 빈 곳을 누르고 끌지 않은 채 휠 200 px 뒤 뗌 | 못 깨뜨림: 휠 중 밴드 없음, 뗀 뒤 선택이 비고 FSM도 「선택 없음」 | `wheel_before_threshold_then_release_is_plain_click` |
| 6 | N6 선택 뒤 `Cmd+드래그` 밴드, 휠로 넓혔다 되돌리고 뗌 | 못 깨뜨림: 넓힐 때 `[N1..N6]`, 되돌리면 `[N1, N2, N3, N6]`, 뗀 뒤도 같다 | `cmd_band_scroll_round_trip_does_not_accumulate` |
| 7 | 밴드를 뗀 뒤 휠 왕복, 선택된 노트를 누른 채 세로 휠 뒤 뗌 | 못 깨뜨림: 휠이 선택을 바꾸지 않고, 뗌은 N2 하나로 줄인다 | `wheel_without_band_leaves_selection` |
| 8 | 밴드 중 Step으로 가서 휠, Normal로 돌아와 휠 뒤 뗌 | 못 깨뜨림: 선택이 Step에 들어갈 때의 것에 머물고 조작이 없다 | `wheel_in_step_after_band_does_not_select` |
| 9 | `band_between`에 틱 상한 밖의 같은 두 점(`1e30`) | 깨짐(형식적): 패닉은 없어졌지만 `[MAX, MAX)` 빈 범위다. 창 경로로는 닿지 않고 그 틱에 노트도 있을 수 없다 | `band_between_at_tick_ceiling_is_not_empty` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 끄는 동안 선택 = 그린 직사각형 안의 노트(결정 3절 5, MS-1) | 휠·핀치로는 아니오, 키 줌으로는 예(끄는 동안만) | 시도 1·2 통과, 시도 3 깨짐. 뗀 뒤 선택은 모든 길에서 직사각형과 같다 |
| 뗄 때 선택 = 그 순간 그린 직사각형의 것(결정 3절 5 고친 글) | 아니오 | 뗌이 `update_band`를 한 번 더 부른다(`pianoroll_mouse.rs:203-214`). 시도 1·2·3 |
| 취소된 조작은 뗄 때까지 아무 일도 하지 않는다 | 아니오 | `piano_roll_view_changed`가 `Gesture::Band`만 고른다. 시도 4 |
| 마우스 선택은 Normal 피아노롤에서만(결정 2절 1) | 아니오 | `piano_roll_view_changed`도 `live_gesture`를 거친다. 시도 8 |
| 진행 중 조작은 누름 하나에 하나, 뗄 때 없어짐 | 아니오 | 뗌이 `drop_gesture`로 끝난다. 시도 4·7·8 |
| 선택은 한 클립 | 아니오 | 모든 선택이 `Selection::set`을 거친다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.4 빈 곳 드래그(러버밴드), `Cmd+드래그` 추가 | 충족 | 리뷰 2 지적 1이 고쳐졌다(`pianoroll_mouse.rs:203-231`, `element.rs:433-486`). 리뷰 2 시도 1, 이번 시도 1·2·6 |
| MS-1(러버밴드 미리보기) | 충족 | 마우스 입력(끌기·휠·핀치) 중에는 미리보기가 직사각형을 따른다. 키 줌 중의 어긋남(시도 3)은 마우스 밖 입력이 섞인 경우이고 뗌에서 맞춰진다(7절 제안 1) |
| MS-7(`Cmd+휠`, 핀치 가로 줌) | 충족 | `element.rs:448-486`. `mouse_select` 휠·핀치 시험과 시도 1·2 |
| 결정 3절 5(누른 점은 내용 위치, 폭 0이면 한 틱, 스크롤·줌 뒤 다시 계산) | 충족 | 고친 글이 「휠·핀치」로 적었고 그 길은 지켜진다. 상한의 빈 범위(시도 9)는 닿지 않는 값이다 |
| 결정 2절 1(Normal 피아노롤에서만) | 충족 | `pianoroll_mouse.rs:221-231`이 `live_gesture`를 거친다. 시도 8 |
| P2 「`Esc`는 진행 중인 조작을 취소」 | 충족 | 시도 4 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. UI 스레드의 Element 처리기와 창 엔티티만 바뀌었다.
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음. `band_between`의 넘침 패닉 지점은 `saturating_add`로 없어졌다(리뷰 2 시도 7 통과).
- 에러가 삼켜지는 곳: 새로 생긴 것 없음. `update_band`의 빈 뷰포트 조기 반환은 그리기 전에만 생기고, 뗌에서는 그 뒤 `drop_gesture`가 늘 불려 조작이 남지 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 뷰포트를 바꾸는 키(`cmd-=`·`cmd--`·`cmd-0`, vim `z l`·`z h`·`z z`)는 `on_view_change`를 거치지 않는다. 누른 채 키로 줌하면 직사각형은 새 뷰포트로 다시 그려지는데 끄는 동안의 선택 강조는 다음 끌기까지 옛 범위다(틱 100–3906 직사각형, 선택 `[N1, N2, N3]`). 뗀 뒤 선택은 맞다. 고친 결정 글이 「휠·핀치」로 범위를 적었고, 마우스를 누른 채 키로 줌하는 조합은 이번 수정의 길 밖이라 제안으로 둔다. 고친다면 `view_zoom_*`·`view_center_cursor` 처리기 끝에서 `piano_roll_view_changed`를 부르면 된다. 시험: `keyboard_zoom_during_band_updates_preview` | `crates/keyscore-app/src/pianoroll_window.rs:223-261`; `crates/keyscore-app/src/pianoroll_mouse.rs:221-231` |
| 2 | 제안 | `band_between`의 폭 0 넓히기가 상한에서 `[MAX, MAX)` 빈 범위다(넘침은 고쳐짐). 그 틱에 노트가 있을 수 없고 창 경로로 닿지 않으므로 결과는 같다. 거절해도 된다. 시험: `band_between_at_tick_ceiling_is_not_empty` | `crates/keyscore-app/src/pianoroll/mouse.rs:53` |

리뷰 2 지적 1(수정)과 제안 2·3은 고쳐졌다. 리뷰 2 실패 시험 2개(`wheel_scroll_then_release_without_move_selects_drawn_band`, `band_between_at_tick_ceiling_does_not_overflow`)가 통과하고, 휠·`Cmd+휠`·핀치 세 길 모두 포인터를 움직이지 않아도 선택이 직사각형을 따른다(리뷰 2 시도 1, 이번 시도 1·2). `register_view_input`으로 옮긴 휠 처리는 예전 `Cmd+휠`의 `return`을 `else`로 바꿔 줌과 스크롤이 섞이지 않고, 기존 휠·핀치 시험이 모두 통과한다. 취소·문턱 전·노트 누름·다른 모드에서는 새 콜백이 아무 일도 하지 않는다(시도 4·5·7·8).

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

수정·차단 지적이 없다. 제안 1·2는 구현자가 받거나 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(3318d83은 `pianoroll/{element,mouse}.rs`, `pianoroll_mouse.rs`, `pianoroll_window.rs`의 콜백 연결 여섯 줄, 결정 3절 5 한 문단만 바꿨다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(3318d83은 시험을 바꾸지 않았다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
