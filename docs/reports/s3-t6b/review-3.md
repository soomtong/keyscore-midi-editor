# 적대적 리뷰: S3-T6b 드래그 이동·조절 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트, 구현 컨텍스트를 물려받지 않음. 1·2회차 리뷰어와도 다른 세션)
- 대상 커밋: `c47e767`(브랜치 `s3/t6b-mouse-drag`). 2회차 이후 수정 커밋 `4c63b6f`(`pitch_room`), `f7c9d7d`(`cancels_drag`, `edit::CancelDrag` 처리기)
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t6b_r3.rs`(`CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t6b_r3`). 11개 모두 통과. 결함을 보이는 시험은 없다

## 0. 2회차 지적의 수정 확인

| 2회차 지적 | 확인 | 근거 |
| --- | --- | --- |
| 1 수정: 오프셋 노트를 가장자리 너머로 끌면 전체 거부, 고스트 사라짐 | 고쳐짐 | `mouse.rs:345` `pitch_room`이 `Note::with_offset`과 같은 `[0, 128·S)` 범위로 올림(`div_euclid` + 나머지 있으면 1)·내림을 계산하고 0–127로도 자른다. 2회차 시험 둘 통과. 음높이 0–127 전부 × 오프셋 13가지(±12반음, ±1반음, ±반음±1단위, ±1단위, 0)에서 위·아래 자르기가 받아들여지고 한 반음 더 가면 거부되는 최대값임을 확인(`review3_pitch_room_clamp_is_valid_and_maximal_for_all_offsets`). 여러 노트는 교집합이고 `plan_shift`가 끝값을 받는다. 아래 가장자리·`Alt` 복제도 멈춤 |
| 2 수정: `edit::CancelDrag` 처리기 없음, 취소 키가 하드코딩 | 고쳐짐 | `pianoroll_mouse.rs:27` `cancels_drag`가 키맵의 `edit::CancelDrag` 바인딩(그대로, 그리고 `Cmd`·`Alt`·`Shift`를 뺀 모양)과 맞춘다. `window.rs:947`에 처리기. 사용자 키맵 파일로 `f9`에 옮기고 `escape`를 `null`로 지우면 `escape`는 취소하지 않고 `shift-f9`가 취소한다(`review3_user_keymap_file_moves_cancel_key`). 2회차 시험 `review2_cancel_drag_action_cancels_drag` 통과 |
| 3 제안: 가장 가까운 격자선 스냅의 바깥 이동 | 결정 문구만 좁힘(수락 범위) | 결정 6절 3. 사람 확인 항목으로 남긴 것은 타당 |

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일을 열기 전에 `4c63b6f`·`f7c9d7d` diff와 `Note::with_offset`, GPUI `Keymap::bindings_for_action`·`KeyBinding::match_keystrokes`만 읽고 적었다.

1. `pitch_room`의 올림·내림: 음수 오프셋의 하한(절대 0 아래), 양수 오프셋의 상한, 오프셋 0에서 `127 − pitch`·`−pitch`와 같은가, 경계 음높이 0·127, 단위 하나 차이(±1, S±1).
2. 여러 노트의 교집합과 `up.max(down)`: Δ 0 요청이 0으로 남는가, 축 잠금(`Axis::Time`)에서 음높이 0인가, 자른 값을 `plan_shift`가 받는가.
3. 아래 방향·`Alt` 복제 화면 경로에서도 가장자리 멈춤·오프셋 유지·undo 하나.
4. `cancels_drag`의 수식키 빼기가 조작이 없을 때 다른 바인딩(`cmd-escape`, `alt-f9`)을 가로채지 않는가.
5. 사용자 키맵에서 `null`로 지운 `escape`가 더는 취소하지 않는가(`disabled_binding_indices` 경로), 옮긴 키를 수식키와 함께 눌러도 취소하는가.
6. 러버밴드에서 바꿔 묶은 키·`edit::CancelDrag` 디스패치가 누르기 전 선택으로 되돌리는가.
7. 조작이 없을 때 `edit::CancelDrag`가 선택·커서·문서·undo·상태줄에 해가 없는가, 취소 뒤 `Gesture::Cancelled`가 다음 드래그를 막지 않는가.
8. `bindings_for_action`은 컨텍스트를 보지 않는다. 다른 컨텍스트의 `edit::CancelDrag` 키도 취소하는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 `pitch_room` 경계 | 부분(2회차 시험 위쪽 한 경우) | 리뷰 시험으로 전수 확인, 못 깨뜨림 |
| 2 교집합·축 잠금·`plan_shift` | 없음 | 리뷰 시험, 못 깨뜨림 |
| 3 아래 방향·`Alt` 복제 | 없음 | 리뷰 시험, 못 깨뜨림 |
| 4 조작 없을 때 통과 | 부분(`review2_escape_after_drag_reaches_normal_binding`) | 수식키 붙은 다른 바인딩은 리뷰 시험, 못 깨뜨림 |
| 5 사용자 키맵 `null` | 없음(보고서는 코드로 확인) | 리뷰 시험으로 실제 파일 경로 확인 |
| 6 러버밴드 | 없음(바꾼 키·Action 쪽) | 리뷰 시험, 못 깨뜨림 |
| 7 조작 없을 때 Action | 없음 | 리뷰 시험, 못 깨뜨림 |
| 8 컨텍스트 무시 | 없음 | 리뷰 시험으로 기록(제안 1) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 음높이 0–127 × 오프셋 13가지(유효 조합 1500+)에서 Δ ±300, 0, 범위 안 세 값, `Axis::Time` | 못 깨뜨림: 자른 값이 유효하고 최대, Δ 0은 0, 범위 안은 그대로, 시간 축은 0 | `review3_pitch_room_clamp_is_valid_and_maximal_for_all_offsets` |
| 2 | 오프셋이 다른 세 노트(절대 112, 98, 63.5)를 ±300·끝값으로 계획 | 못 깨뜨림: 위 15·아래 −63, `plan_shift` 모두 `Ok(Some)` | `review3_pitch_room_intersects_notes_and_plan_shift_accepts_edges` |
| 3 | 음높이 12 − 12반음(절대 0) 노트를 한 박 오른쪽·3반음 아래로 끎 | 못 깨뜨림: 고스트 `(1920, 12)`, 거부 아님, 적용 뒤 오프셋 유지, undo +1 | `review3_body_drag_negative_offset_note_stops_at_bottom_edge` |
| 4 | 115 + 12반음 노트를 `Alt` 복제로 2반음 위·한 박 뒤 | 못 깨뜨림: 사본 `(1920, 960, 115)` 오프셋 유지, undo +1 | `review3_alt_copy_offset_note_stops_at_top_edge` |
| 5 | 사용자 키맵 파일 `{"context": "Dragging", "bindings": {"f9": "edit::CancelDrag", "escape": null}}` 뒤 끄는 중 `escape`, `shift-f9` | 못 깨뜨림: `escape`는 취소하지 않고 `shift-f9`가 취소, 문서·undo 그대로 | `review3_user_keymap_file_moves_cancel_key` |
| 6 | 조작이 없을 때 `cmd-escape`·`alt-f9`(각각 `select::Clear`에 묶음, `f9`는 `edit::CancelDrag`) | 못 깨뜨림: 둘 다 선택 해제에 닿는다 | `review3_bare_match_does_not_steal_keys_without_gesture` |
| 7 | `Cmd+러버밴드` 중 바꿔 묶은 `cmd-f9` | 못 깨뜨림: 누르기 전 선택 `[N1]`, 뗌 무효 | `review3_rebound_key_cancels_band_and_restores_selection` |
| 8 | `Cmd+러버밴드` 중 `edit::CancelDrag` 디스패치 | 못 깨뜨림: 같은 복원 | `review3_cancel_drag_action_during_band_restores_selection` |
| 9 | 조작 없이 `edit::CancelDrag` 디스패치 뒤 새 드래그 | 못 깨뜨림: 선택·커서·undo·상태줄 그대로, 이어서 세 노트 이동이 undo 하나 | `review3_cancel_drag_action_without_gesture_is_harmless` |
| 10 | Action 취소 → 뗌 → 같은 드래그 다시 | 못 깨뜨림: 첫 번째 무효, 두 번째 적용 | `review3_drag_after_action_cancel_works` |
| 11 | 「Step」 컨텍스트에 `f8` → `edit::CancelDrag`를 묶고 Normal 피아노롤에서 끄는 중 `f8` | 컨텍스트를 보지 않아 취소된다. 해는 없어 결함이 아니라 기록(제안 1) | `review3_cancel_key_in_other_context_still_cancels_drag`(통과) |

회귀 확인: `review_s3_t6b_r2` 9, `review_s3_t6b` 8, `mouse_drag` 19, `mouse_drag_pure` 15, `mouse_select` 26, `pianoroll_select` 14, `keymap_engine` 11, `commands` 12 모두 `target/review`에서 통과. 지시대로 MIDI 시험과 전체 시험은 돌리지 않았다. 리뷰 시험 파일은 `cargo fmt --check`·`clippy -D warnings`를 통과한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 절대 피치 범위(결정 2026-10-12 B) | 못 깨뜨림 | `pitch_room`이 `with_offset`과 같은 식, `with_pitch`가 다시 검사 |
| 노트 필드 보존(LYR-1) | 못 깨뜨림 | 시도 3·4에서 오프셋 유지 |
| 노트는 클립 안, 클립 겹침 없음 | 못 깨뜨림 | 수정 커밋이 시간 쪽을 건드리지 않음 |
| 드래그 한 번 = undo 하나, 취소면 0 | 못 깨뜨림 | 시도 3·4·5·9·10 |
| 키·마우스 같은 계산 | 유지 | 수정은 `drag_shift`의 자르기만. 노트 계산은 여전히 `shift_note` 하나, 계획은 `plan_shift`·`plan_shift_copy` |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.4 머리말 「같은 Action을 호출」 | 충족 | `ops.rs` `plan_note_op` → `plan_shift`, 마우스도 같은 함수 |
| 7.4 표 「드래그 중 `Esc` 취소, 원위치」 | 충족 | `pianoroll_mouse.rs:27`·`:46`, 시도 5·7–10 |
| KM-1·KM-3 | 충족 | 취소 키를 키맵 `edit::CancelDrag`에서 읽음, 사용자 파일로 바꿔 묶기·지우기 동작(시도 5). 컨텍스트는 보지 않음(제안 1) |
| MS-1 고스트·undo 한 단위 | 충족 | 시도 3·4, 2회차 시험 |
| MS-2 상대 위치, 끝 같은 양 | 충족 | 기존 시험, 시도 9 |
| MS-3 스냅·`Cmd` 자유·설정 | 충족 | 기존 시험 |
| MS-4 미리듣기 | 코드상 충족(MIDI 시험 미실행) | 자른 음높이가 유효해져 범위 밖 음을 들려주던 2회차 문제도 사라짐 |
| MS-6 끝 영역 | 충족 | 기존 시험 |
| 결정 6절 5·9·11 | 충족 | 시도 1–10 |
| 5.7 `Alt` 조작 회귀 | 충족 | `review_s3_t6b` 통과 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `cancels_drag`는 UI 스레드의 키 가로채기에서 키맵 `RefCell`을 빌린다(키 하나마다 바인딩 몇 개를 훑는 정도).
- 제품 코드의 `unwrap`/`expect`: 새로 없음. `pitch_room`의 `i32::try_from(..).unwrap_or(0)`은 값이 ±127 안이라 닿지 않는다.
- 에러가 삼켜지는 곳: `plan_move`의 거부 고스트 `shifted(..).unwrap_or_default()`는 이제 음높이로는 닿지 않는다(시도 1).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `cancels_drag`는 `bindings_for_action`의 결과를 컨텍스트와 상관없이 쓴다. 그래서 「Dragging」이 아닌 컨텍스트(예: 「Step」)에 묶은 `edit::CancelDrag` 키도 Normal 피아노롤의 드래그를 취소한다. 「Dragging」은 엔진이 세우지 않는 이름뿐인 컨텍스트라 해는 없지만, 결정 6절 11의 「컨텍스트 「Dragging」」과 글이 어긋난다. 바인딩 술어가 없거나 「Dragging」인 것만 보거나, 결정 문구에 「컨텍스트는 보지 않는다」를 적는 정도. 기록: `review3_cancel_key_in_other_context_still_cancels_drag`(통과) | `crates/keyscore-app/src/pianoroll_mouse.rs:27-38` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

2회차 지적 1·2는 고쳐졌고 수정이 새 결함을 만들지 않았다. `pitch_room`은 모든 음높이와 경계 오프셋에서 유효한 최대값을 주고, 취소 키는 키맵에서 읽히며 조작이 없을 때 다른 바인딩을 가로채지 않는다. 단계 게이트 셋(다중 선택 상대 위치, 드래그 한 번 = undo 하나, `Esc` 취소)과 리뷰 초점(마우스 전용 편집 경로 없음)도 유지된다. 남은 것은 제안 하나다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(2회차 이후 커밋은 `mouse.rs`·`pianoroll_mouse.rs`·`window.rs`·`mouse_drag.rs` 시험 추가·결정·보고서만)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
