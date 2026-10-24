# 적대적 리뷰: S3-T1 피아노롤 Element (리뷰 2회차)

- 리뷰어 세션: 구현 세션·1회차 리뷰 세션과 다른 세션(구현 맥락 없이 코드·계획·REQ·1회차 산출물만 읽고 시작). 서브 에이전트.
- 대상 커밋: `bd12505` (브랜치 `s3/t1-pianoroll-element`, `main..HEAD` 11개 커밋. 수정 커밋은 `9e6ffcf`, `ba8f51a`, `3a54904`, `ebae157`)
- 범위: 1회차 지적 3건·제안 7건의 수정이 맞는지, 그 수정이 회귀를 만들지 않았는지. 1회차가 다루지 않은 새 영역은 결함·결정 위반·회귀만 「수정」으로 올리고 그 밖은 「제안」.
- 추가한 시험: `crates/keyscore-app/tests/review_s3_t1_r2.rs` (10개, 전부 통과)
- 실행: `cargo test -p keyscore-app --test review_s3_t1_r2 --test review_s3_t1 --test pianoroll` → `pianoroll` 13 통과, `review_s3_t1` 19 통과(1회차 실패 3개 포함), `review_s3_t1_r2` 10 통과. `cargo test -p keyscore-app --release --test pianoroll frame_with_10k -- --nocapture` → 중앙값 1.55 ms, 최대 1.92 ms, 그린 노트 9,874. `cargo clippy -p keyscore-app --test review_s3_t1_r2 -- -D warnings` 통과, `cargo fmt --all -- --check` 통과.
- 되돌림 확인: `git worktree add --detach /tmp/ks-r2-base 34d1709`(수정 전 마지막 커밋)에서 별도 target으로 `review_s3_t1`의 4개를 돌렸다. `entering_piano_roll_from_far_clip_scrolls_viewport_to_cursor`·`toggling_view_into_far_clip_scrolls_viewport_to_cursor`·`wheel_after_keyboard_input_without_pointer_move_scrolls` 실패(1회차가 적은 증상 그대로: `first_tick 0`, 커서 192,000틱 / 휠 뒤 뷰포트 불변), 대조 `wheel_after_pointer_move_scrolls_rows_and_shift_wheel_scrolls_ticks` 통과. HEAD에서는 셋 다 통과하므로 통과는 수정 커밋 때문이다. 시험 파일 자체는 수정 커밋에서 바뀌지 않았다(`git show ba8f51a 9e6ffcf --stat`에 `tests/` 없음).

## 1. 코드만 읽고 적은 「내가 검사할 것」

수정 커밋 4개의 diff와 `pianoroll/{mod.rs,scene.rs,element.rs}`, `pianoroll_window.rs`, `window.rs` 변경분, GPUI 0.3.8의 `Hitbox::should_handle_scroll`·`dispatch_event`·`render_frame`을 읽고 `tests/review_s3_t1_r2.rs`를 쓰기 전에 적었다.

1. `pending_follow`는 `set_size`의 「처음 크기」 분기(`width <= 0 || height <= 0`였다가 둘 다 양수)에서만 적용된다(`mod.rs:289-300`). 창 크기 변경이나 패널 접기로 주 뷰가 0이 됐다 돌아오는 경로가 실제로 있는가. 있다면 그때 `center_rows`가 다시 돌아 세로 스크롤 자리를 되돌리는가, 0인 동안 누른 커서 키의 따라가기가 돌아올 때 적용되는가.
2. 플래그가 소비되는가: 처음 크기에서 적용된 뒤 휠로 멀리 가면 다음 프레임이 커서로 되돌리지 않아야 한다(`ensure_cursor_visible` 첫 줄에서 `pending_follow = false`).
3. `entered_piano_roll`이 옮기지 않았어도 늘 `ensure_cursor_visible`을 부른다(`pianoroll_window.rs:115-116`). 배치 뷰에서 돌아올 때 (a) 커서가 아직 보이면 스크롤 자리를 지키는가(보이면 아무것도 안 하는 함수여야 함), (b) 커서를 화면 밖으로 스크롤해 둔 채 `v v`하면 커서로 되돌아간다. (b)가 계획 2 「커서가 뷰포트 밖으로 가면 뷰포트가 따라간다」·REQ 5.3·5.10과 어긋나는지.
4. `should_handle_scroll`은 `mouse_hit_test.ids.contains`뿐이고(`gpui-pre-0.3.8/src/window.rs:819-821`) `ScrollWheel`은 `mouse_position`을 갱신한 뒤 히트 테스트를 다시 한다(`window.rs:5508-5512`, `5793-5795`). 그러면 인스펙터·트랙 목록·상태줄 위의 휠은 피아노롤 히트박스에 들어가지 않아야 한다. 또 휠 → 키 → 휠(포인터 이동 없음)에서 둘째 휠도 처리돼야 한다.
5. 클릭 처리기는 여전히 `is_hovered`다(`element.rs:297`). `MouseDown`은 모달리티를 Mouse로 바꾸므로(`window.rs:5465`) 키 입력 직후 클릭은 된다. 1회차 시도 16이 이미 `enter` 뒤 클릭을 봤다.
6. release 프레임 시험(`pianoroll.rs:656-694`): `render_frame`이 `refresh()` 뒤 `draw()`라(`gpui-kit/crates/kit/src/test.rs:348-351`) 매 호출이 레이아웃·prepaint·paint 전체를 다시 한다. 시간이 0에 가까운 「더티 아님」 프레임을 재는 게 아닌지. 20프레임 중앙값이 60 fps 게이트로 충분한지, debug에서 단언이 없으면 자동 게이트(`cargo test --workspace`, debug)가 이 시험을 「통과」로 세는 것이 맞는지. 배율 하한(0.0005 px/틱)에서는 노트가 전부 1 px 폭이다(`scene.rs:238`의 `max(1.0)`) — CPU 비용은 같지만 「10,000 노트 화면」의 대표성.
7. 제안 1 수정: `paint_scene`이 노트 루프에서 세어 돌려주고 `paint_layer` 클로저가 그 값을 돌려준다(`element.rs:189, 259, 283`). `painted`가 `scene.notes.len()`과 따로 세어지는가. `RefCell` 빌림 순서(`borrow()` → 블록 끝 → `borrow_mut()`)가 그대로인가.
8. 제안 6 수정: 크기를 모를 때 `zoom`은 배율만 바꾸고 `ensure_cursor_visible`을 부르지 않는다(`mod.rs:453-456`). 그래서 `pending_follow`도 켜지 않는다. 배치 뷰에서 `cmd-=`(전역 standard 키, 제안 7)로 먼저 줌한 뒤 먼 클립으로 들어가면 `entered_piano_roll`의 `ensure_cursor_visible`이 플래그를 켜 주는가.
9. `place_cursor_in_clip`이 옮기지 않는 경로(커서가 이미 클립 안인데 화면 밖)에서도 처음 들어갈 때 따라가는가. 배치 뷰에서 `ctrl-l`(Normal 공통 바인딩, `default.json:44`)로 커서를 옮겨 두고 `enter`.
10. 먼 클립으로 들어갈 때 `ensure_cursor_visible`의 가로 규칙은 `fit = tick + step - width/px`(`mod.rs:270-271`)라 커서 칸이 오른쪽 끝에 붙는다. 들어간 직후 클립 안쪽이 한 스텝(960틱)만 보인다. 계획 2의 문구로는 만족이지만 REQ 5.3 「클립 하나의 안쪽을 … 보인다」와 견줘 결함인지.
11. 1회차 제안 2·3·4·5·7의 문서 반영이 실제로 있는가(`plan.md:84`, `13-S3:15,17,31,70`, `14-S4` 이월 줄, `mod.rs:180-181`).
12. 수정 커밋에 Task 범위 밖 변경·기존 시험 삭제·`#[ignore]`·새 의존성이 없는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 주 뷰 0 → 양수 재진입 | 없음 | 헤드리스 창을 1×1로 줄여도 주 뷰가 100×100으로 남는다(아래 시도 8). 0 경로는 첫 레이아웃뿐 |
| 2. 플래그 소비 | 없음 | 시도 3 |
| 3. 배치 뷰 왕복의 스크롤 자리 | 없음 | 시도 6·7 |
| 4. 다른 요소 위의 휠, 휠·키·휠 | 없음 | 1회차 시험은 주 뷰 가운데만 | 
| 5. 키 뒤 클릭 | 있음(1회차 시도 16) | `click_moves_cursor_to_cell_under_pointer`가 `enter` 뒤 클릭 |
| 6. release 프레임 측정 방식 | 있음(`frame_with_10k_notes_fits_60fps_budget_in_release`) | 측정은 맞다. 게이트 운용은 제안 3 |
| 7. `painted` 따로 세기 | 있음(`element_paints_every_visible_note_once`) | 이제 동어반복이 아니다(paint 루프 카운터 vs 시험이 따로 만든 장면) |
| 8. 크기 모를 때 줌 뒤 진입 | 없음 | 시도 4 |
| 9. 옮기지 않는 경로의 따라가기 | 없음 | 시도 5 |
| 10. 진입 시 보이는 클립 범위 | 없음 | 시도 10(수치만 기록) |
| 11·12. 문서·범위 | 코드로 확인 | 체크리스트 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t1_r2.rs`에서 실제로 실행했다(시도 11·12는 워크트리·release 실행).

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `enter` 뒤 포인터 이동 없이 인스펙터·트랙 목록·상태줄 가운데에서 휠(−120 px), 대조로 주 뷰 가운데 휠 | 못 깨뜨림. 세 패널 위 휠은 뷰포트 불변, 주 뷰 위 휠은 첫 행 +10 | `wheel_over_side_panels_after_keyboard_does_not_scroll_piano_roll` |
| 2 | 휠 → `l` → 휠(포인터 이동 없음) | 못 깨뜨림. 둘째 휠도 +10행 | `wheel_key_wheel_without_pointer_move_scrolls_twice` |
| 3 | 먼 클립(50마디)으로 들어가 따라간 뒤 `scroll_by(-50000, 300)`로 멀리 가고 `render_frame` 두 번 | 못 깨뜨림. 뷰포트가 그대로다(플래그 소비됨) | `first_layout_follow_is_consumed_and_does_not_snap_back_later` |
| 4 | 배치 뷰(피아노롤 한 번도 안 그림, `width 0`)에서 `cmd-=` 5번 → `enter`(50마디 클립) → `cmd-0` | 못 깨뜨림. 배율만 올라가고(`first_tick` 불변) 들어간 직후·되돌린 뒤 커서 보임 | `zoom_before_first_layout_then_enter_far_clip_keeps_cursor_visible` |
| 5 | 배치 뷰에서 `ctrl-l` 15번(커서 15마디, 20마디 클립 안) → `enter` | 못 깨뜨림. 옮기지 않았고(`place_cursor_in_clip` 거짓) 커서 보임 | `cursor_moved_in_arrangement_before_first_layout_is_visible_on_entry` |
| 6 | `enter l l l l`, `scroll_by(100, 36)`(커서 여전히 보임) → `v` → `v` | 못 깨뜨림. 뷰포트가 스크롤한 자리 그대로 | `returning_from_arrangement_keeps_scroll_when_cursor_still_visible` |
| 7 | `enter`, `scroll_by(50000, 600)`(커서 화면 밖) → `v v` | 못 깨뜨림(현재 구현대로 커서로 되돌아간다). 설계 선택이라 제안 2로 | `returning_from_arrangement_after_scrolling_away_snaps_to_cursor` |
| 8 | 창을 1×1로 → `ctrl-l` 15번 → 1280×800으로 | 못 깨뜨림. 1×1에서도 주 뷰는 100×100이라 0 경로를 타지 않았고, 커서 따라가기는 키 경로가 바로 적용. 되돌린 뒤 커서 보임, 패닉 없음 | `window_shrink_then_regrow_keeps_cursor_visible` |
| 9 | 휠로 세로 스크롤 뒤 창을 1000×700으로 | 못 깨뜨림. `first_row`·`first_tick` 불변(「처음」 분기가 다시 타지 않는다) | `ordinary_resize_keeps_vertical_scroll` |
| 10 | 50마디 클립(4마디 길이)으로 들어간 직후 커서 오른쪽의 보이는 틱 수 | 깨뜨리지 못함(계획 2 만족). 커서 뒤로 960틱(한 스텝)만 보인다. 제안 1 | `entering_far_clip_shows_only_one_step_of_the_clip` |
| 11 | 수정 전 커밋 `34d1709` 워크트리에서 1회차 실패 시험 3개 + 대조 1개 | 3개 실패·대조 통과(위 「되돌림 확인」). HEAD에서 3개 통과 | (워크트리 실행, 시험 추가 없음) |
| 12 | release 프레임 시험 실행 | 중앙값 1.55 ms, 최대 1.92 ms(보고서 1.53/1.85와 같은 수준). debug 실행은 19.13 ms로 단언 없이 통과 | (기존 시험 실행) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `paint`는 노트 수에 비례하는 할당·정렬을 하지 않는다(제안 2로 고친 문구) | 지킴 | `paint_scene`은 `paint_quad`와 `painted += 1`뿐(`element.rs:215-283`). `register_mouse`의 Box 2회는 상수. 수정 커밋이 paint 경로에 더한 것은 카운터 하나 |
| `RefCell` 이중 빌림 없음 | 없음 | `paint`의 `borrow()`는 `painted` 블록 끝에서 풀리고 그 뒤 `borrow_mut()`(`element.rs:185-192`). `paint_layer` 클로저가 `usize`를 돌려주는 것 외에 순서 변화 없음 |
| `pending_follow`는 한 번 적용되고 꺼진다 | 없음 | `ensure_cursor_visible`이 크기를 알면 첫 줄에서 `false`(`mod.rs:262`). 시도 3 |
| 줌은 상한·하한 사이 | 없음 | 크기를 모를 때 분기도 `clamp` 뒤 값만 쓴다(`mod.rs:449-455`). 시도 4 |
| 커서 틱 ≥ 0, 음높이 0–127 | 없음 | 수정 커밋이 커서 계산을 건드리지 않았다 |
| 장면 계산은 문서를 바꾸지 않는다 | 없음 | 변경 없음 |

## 5. REQ 대조

재리뷰 범위(1회차가 부분 충족으로 적은 항목과 수정이 닿는 항목)만 다시 대조했다. 나머지는 1회차 표 그대로다.

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.3 「클립 하나의 안쪽을 세로축 음높이, 가로축 틱으로」 | 충족 | 변경 없음. 진입 직후 보이는 범위가 한 스텝인 것은 제안 1(문구 위반은 아님) |
| 5.3 「배치 뷰는 `v`로 오간다」, 5.10 「전환은 `v`, 클립 위 `Enter`로 안으로」 | 충족 | `window.rs:256-261` 진입 감지, 시도 4·5·6·7 |
| UI-3 「키 입력 후 60 fps 안 반영, 10,000개 끊김 없이 스크롤」 | 충족(헤드리스 측정 범위에서) | release 프레임 중앙값 1.55 ms(시도 12), 키 뒤 휠 처리(1회차 시험 통과). GPU·실제 창은 단계 파일 읽기대로 SP-2와 S3 출구 수동 항목(`13-S3:15, 70`) |
| 계획 2 「커서가 뷰포트 밖으로 가면 뷰포트가 따라간다」 | 충족 | 들어갈 때 `mod.rs:256-262, 295-297`(시도 3·4·5), 키 이동은 기존 시험 7 |
| 계획 2 「피아노롤로 들어갈 때 … 첫 클립 시작으로」 | 충족 | `pianoroll_window.rs:115`, 1회차 시험 |
| 계획 3 「마우스 휠은 세로·가로 스크롤」 | 충족 | `element.rs:313-329`, 1회차 시험 4·5, 시도 1·2 |
| 단계 게이트 「criterion — 10,000 노트 화면의 paint 시간 기준값 커밋, 60 fps 상한 안」(읽기 줄 포함) | 충족 | `13-S3:15` 읽기 줄, `benches/pianoroll.rs`(장면), `pianoroll.rs:656-694`(프레임, release 단언). 운용은 제안 3 |
| 단계 게이트 「UI 테스트 — 커서 이동 키가 뷰포트를 따라 스크롤」 | 충족 | 기존 시험 7, 1회차 시도 3·17 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 수정 커밋은 스케줄러를 건드리지 않는다(`git show 9e6ffcf ba8f51a 3a54904 ebae157 --stat`에 `engine`·`midi` 없음).
- paint 경로: 카운터 `painted += 1`(`element.rs:259`)만 더해졌다. 할당 없음.
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 더한 것 없음. 기존 `mod.rs:325` `expect("C4")` 하나(1회차 판단 그대로 정당).
- 에러가 삼켜지는 곳: `pianoroll_window.rs:115` `let _ = place_cursor_in_clip(...)`는 bool 반환을 버리는 것이고 에러가 아니다. 의도(옮겼든 아니든 따라가기)가 주석에 있다.
- 시험 코드: `frame_with_10k_notes_fits_60fps_budget_in_release`는 debug에서 단언하지 않는다(의도된 것, 제안 3).

## 7. 지적 사항

1회차 지적·제안의 처리 확인:

| 1회차 항목 | 확인 | 근거 |
| --- | --- | --- |
| 지적 1 먼 클립 진입 시 뷰포트 | 수정됨 | `mod.rs:240-241, 256-262, 295-297`, `pianoroll_window.rs:113-116`. 되돌림 확인(시도 11), 회귀 없음(시도 3·4·5·6·8·9) |
| 지적 2 키 뒤 휠 무시 | 수정됨 | `element.rs:314` `should_handle_scroll`. 되돌림 확인(시도 11), 다른 요소 위 휠 안 가로챔(시도 1), 휠·키·휠(시도 2) |
| 지적 3 paint 측정 | 수정됨((a)+(b)) | `pianoroll.rs:656-694` release 단언, `13-S3:15` 읽기 줄, `13-S3:70` 수동 항목. `render_frame`은 `refresh`+`draw`라 실제 프레임을 잰다. 실행값 1.55 ms(시도 12) |
| 제안 1 `painted` 동어반복 | 수락·반영 | `element.rs:189, 211, 259, 283` |
| 제안 2 불변식 문구 | 수락·반영 | `plan.md:84` |
| 제안 3 `Ctrl+h` 읽기 | 수락(이월)·반영 | `13-S3:31` S3-T2 이월 줄 |
| 제안 4 가장자리 4 px | 수락(이월)·반영 | `13-S3:17` S3-T6 이월 줄 |
| 제안 5 `MaxLenCache` 계약 | 수락·반영 | `mod.rs:180-181` |
| 제안 6 크기 모를 때 `zoom` | 수락·반영 | `mod.rs:453-456`. `pending_follow`와의 맞물림은 시도 4에서 확인 |
| 제안 7 배치 뷰 줌 | 수락(이월)·반영 | `14-S4` 이월 줄 |

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| — | — | 차단·수정 없음 | — |

제안(구현자가 거절할 수 있음. 거절 이유를 보고서에 적는다):

| # | 내용 | 파일:줄 |
| --- | --- | --- |
| 제안 1 | 먼 클립으로 들어갈 때 `ensure_cursor_visible`의 「오른쪽 끝 맞춤」 규칙 때문에 커서 칸이 뷰포트 오른쪽 끝에 붙고 클립 안쪽은 한 스텝(960틱)만 보인다(시도 10: 커서 뒤 보이는 틱 960). 키로 오른쪽 끝을 넘을 때의 규칙으로는 맞지만, 들어갈 때는 `pending_follow` 적용 자리(`set_size`의 처음 분기)나 `entered_piano_roll`에서 `center_cursor` 또는 「커서를 왼쪽 여백 뒤에」로 두는 편이 5.3 「클립 하나의 안쪽을 보인다」에 가깝다. 1회차 지적 1도 「`ensure_cursor_visible`(또는 `center_cursor`)」로 둘 다 열어 두었다 | `mod.rs:268-272, 295-297` |
| 제안 2 | `entered_piano_roll`의 무조건 `ensure_cursor_visible`은 커서가 보이는 동안의 스크롤 자리는 지키지만(시도 6), 커서를 화면 밖으로 스크롤해 둔 채 `v v`하면 커서로 되돌아간다(시도 7). 키 경로(`l` 한 번에 되돌아옴, 1회차 시도 17)와 같은 규칙이라 결함은 아니다. 다만 S4-T1이 배치 뷰 커서를 따로 갖게 되면 「돌아올 때 어느 커서를 따르는가」가 이 줄에 걸리므로, 계획 「끝의 정의」 2에 「들어갈 때마다 커서 칸이 보이게 한다(스크롤 자리보다 커서 우선)」를 한 줄로 적어 두면 S4가 그대로 잇거나 바꿀 수 있다 | `pianoroll_window.rs:110-117`, `docs/reports/s3-t1/plan.md:18-19` |
| 제안 3 | release 프레임 시험은 자동 게이트 명령(`cargo test --workspace`, debug)에서는 단언하지 않는다(debug 19.13 ms, 출력만). 단계 파일 읽기 줄이 「release 시험이 16.7 ms 안을 단언한다」고 하므로, 게이트로 세려면 보고서·출구 게이트에 release 실행을 한 줄로 요구하거나(`cargo test -p keyscore-app --release --test pianoroll frame_with_10k`) `scripts/bench-check.py`처럼 스크립트에 넣는다. 또 20프레임 중앙값은 단언 기준으로 느슨하다(상한 16.7 ms에 대해 1.55 ms라 지금은 문제없음). 최대값도 함께 단언하면(예: 최대 < 16.7 ms) 지터 회귀가 걸린다. 배율 하한에서 노트가 전부 1 px 폭인 점은 CPU 비용에 영향이 없으나, 「10,000 노트 화면」의 대표성을 위해 기본 배율 근처에서 10,000개가 보이는 짧은 노트 배치(SP-2와 같은 배치)로 바꿔도 비용은 같다 | `crates/keyscore-app/tests/pianoroll.rs:656-694`, `plan/stages/13-S3-step-and-pianoroll.md:15` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

1회차 지적 3건은 모두 코드·문서로 고쳐졌고, 수정 전 커밋에서 실패하던 시험 3개가 수정 때문에 통과함을 워크트리로 확인했다. 수정이 닿는 경로(처음 크기 따라가기, 다른 요소 위 휠, 크기 모를 때 줌, 배치 뷰 왕복, 창 크기 변경)에서 회귀를 10개 시험으로 찾지 못했다. 제안 3건은 병합을 막지 않는다. `review_s3_t1_r2.rs`는 PR에 넣는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(10개 전부 통과, 워크트리 되돌림 1회, release 실행 1회)
- [x] REQ ID를 하나씩 대조했다(재리뷰 범위)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(수정 커밋은 `pianoroll/`, `pianoroll_window.rs`, `tests/pianoroll.rs`, `plan.md`, `13-S3`, `14-S4` 이월 줄뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`git diff main...HEAD -- crates/keyscore-app/tests`에 삭제 줄 없음, `ignore]` 검색 결과 없음)
- [x] 새 의존성이 있으면 라이선스를 확인했다(수정 커밋에 의존성 변경 없음)
- [x] 스타일 지적을 적지 않았다
