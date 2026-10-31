# Task 보고: S3-T6a 마우스 선택

- PR: 브랜치 `s3/t6a-mouse-select`
- 계획 파일: docs/reports/s3-t6a/plan.md
- 결정: docs/decisions/2026-10-31-s3-t6-mouse-editing.md(에이전트 해석)
- 계획 대비 달라진 점:
  - `piano_roll_release`는 창(`Window`)을 받지 않는다(쓰지 않음).
  - 시험 28(`document_change_during_band_cancels_gesture`): undo가 되살린 노트를 선택으로 두는 기존 규칙(S3-T3a `appeared`)이 먼저라, 「undo 뒤 선택을 이어진 움직임이 바꾸지 않는다」로 단언한다.
  - 시험 목록 30개 중 23개를 UI 파일에, 7개를 순수 파일에 두었다(번호 대응은 아래 표).

## 구현 요약

- `crates/keyscore-app/src/pianoroll/mouse.rs`(새): 순수 계산. 러버밴드 범위(`band`, 틱 내림·올림, 행 양끝), 안의 노트(`band_notes`, S3-T2 `notes_in`과 같은 걸침 기준), 클립 고르기(`band_clip`), `Shift+클릭` 시간 범위(`time_range_notes`), 마디 기준 격자 내림(`snap_floor`), 3 px 문턱, 선택의 첫 칸. 진행 중 조작 `Gesture`(노트 누름, 빈 곳 누름, 러버밴드, 취소됨).
- `crates/keyscore-app/src/pianoroll_mouse.rs`(새): 창 처리기. `piano_roll_press`·`piano_roll_drag`·`piano_roll_release`, `Esc` 가로채기(`intercept_mouse_keys`), 취소(`cancel_piano_roll_gesture`), 더블클릭 삭제(`edit::Delete`를 `window.defer` + `run_action`)와 생성(`plan_paste` + `apply_placed`).
- `pianoroll/element.rs`: `on_click`을 `on_press`(수식키·누른 횟수 포함)·`on_drag`·`on_release`로 바꿨다. 끌기·뗌은 피아노롤 밖에서도 부른다. `Cmd+휠`·핀치 → `zoom_at`. 러버밴드 직사각형 그리기.
- `pianoroll/mod.rs`: `Inner.gesture`·`last_clicked`, `zoom_at`, `gesture_active`, `band_rect`. `scene.rs`: `Scene.band`.
- `pianoroll_window.rs`: 피아노롤을 4 px 여백 안에 둔다(`main-view`는 안쪽, S3-T1 이월).
- `selection.rs`: 문서 변경 때 진행 중 조작을 취소. `note_edit.rs`: `apply_placed`를 `pub(crate)`로. `window.rs`: 가로채기 구독. `vim.rs`: `vim_pending_input_changed`가 `&mut Window`를 받아 함수로 바로 넘긴다(`ProjectWindow::new`의 clippy 줄 수 상한).

## 테스트 결과

~~~
mouse_select_pure: 7 passed
mouse_select: 23 passed
관련 기존 시험(pianoroll, pianoroll_select, review_s3_t1·r2, review_s2_t5c_r2·r3·r4, review_s2_t6b,
vim_operators, review_s3_t3c·r2·r3·r4, keyboard_view_ui, window, modes_ui): 모두 통과
전체 cargo test --workspace: 리뷰 뒤에 돌린다(아래 자동 게이트)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `band_covers_ticks_floor_ceil_and_rows_inclusive` | 통과 |
| 2 | `band_notes_use_overlap_rule` | 통과 |
| 3 | `band_clip_prefers_press_then_now_then_first_overlap` | 통과 |
| 4 | `time_range_notes_ignore_pitch` | 통과 |
| 5 | `snap_floor_uses_bar_start_grid` | 통과 |
| 6 | `drag_threshold_is_three_px` | 통과 |
| 7 | `first_cell_is_earliest_then_highest` | 통과 |
| 8 | `click_note_selects_only_that_note` | 통과 |
| 9 | `click_empty_clears_selection_and_moves_cursor` | 통과 |
| 10 | `click_selected_note_keeps_selection_until_release` | 통과 |
| 11 | `cmd_click_toggles_in_same_clip_and_replaces_in_other_clip` | 통과 |
| 12 | `shift_click_extends_by_time_range_from_last_clicked` | 통과 |
| 13 | `rubber_band_selects_notes_inside_and_previews_live` | 통과 |
| 14 | `cmd_rubber_band_adds_to_selection` | 통과 |
| 15 | `escape_during_rubber_band_restores_selection_and_keeps_mode` | 통과 |
| 16 | `rubber_band_across_clips_picks_press_clip` | 통과 |
| 17 | `rubber_band_from_gap_moves_cursor_into_selected_clip_and_alt_k_works` | 통과 |
| 18 | `mouse_selection_then_alt_k_transposes` | 통과 |
| 19 | `double_click_note_deletes_it_with_one_undo` | 통과 |
| 20 | `double_click_empty_creates_note_of_length_code_snapped` | 통과 |
| 21 | `cmd_double_click_empty_does_not_snap` | 통과 |
| 22 | `double_click_in_gap_creates_bar_clip` | 통과 |
| 23 | `drag_below_threshold_is_click` | 통과 |
| 24 | `step_mode_click_only_moves_cursor` | 통과 |
| 25 | `cmd_wheel_zooms_around_pointer` | 통과 |
| 26 | `pinch_zooms_around_pointer` | 통과 |
| 27 | `edge_pixel_of_piano_roll_is_clickable` | 통과 |
| 28 | `document_change_during_band_cancels_gesture` | 통과 |
| 29 | `visual_mode_ends_on_mouse_press` | 통과 |
| 30 | `release_outside_piano_roll_finishes_band` | 통과 |

시험 순서: 순수 시험 7개는 `todo!()` 뼈대로 실패를 확인한 뒤 구현했다. UI 시험은 창 처리기를 쓴 뒤에 썼다(첫 실행에서 시험 28의 기대가 틀려 고쳤다). 숨기지 않고 적는다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(전체): 리뷰 3 대응 뒤 `cargo test --workspace --no-fail-fast` 280 바이너리, 2578 통과, 0 실패
- [x] deny
- [x] `scripts/check-deps.sh`
- [x] `scripts/check-deps.test.sh`(16 통과), `scripts/bench-check.test.sh`(8 통과)

## 알려진 한계

- 노트 위에서 시작한 끌기는 아무것도 하지 않는다(이동·조절은 S3-T6b).
- 러버밴드는 자동 스크롤하지 않는다. 포인터가 밖으로 나가면 직사각형이 가장자리에서 멈춘다.
- 마우스 줌은 커서를 따라가지 않는다(다음 커서 키가 뷰포트를 데려온다).
- 4 px 여백은 누를 수 없는 테두리다.
- 실제 창의 트랙패드(핀치·탭-드래그) 감각은 사람 확인 항목이다.

## 리뷰어에게

- 리뷰 초점(단계 파일): 마우스가 키와 같은 Action·상태를 쓰는가. 선택은 `replace_selection`·`set_selection`, 삭제는 `edit::Delete` 처리기, 생성은 붙여넣기의 `plan_paste`·`apply_placed`다. 마우스 전용 편집 경로가 있으면 차단 대상이다.
- 불안한 곳: `Esc` 가로채기의 순서(마우스 → `e` 음높이 대기 → vim). `piano_roll_press`가 커서를 옮기는 `click_piano_roll_cell`(미리듣기·비주얼 재계산 포함)을 그대로 부른 뒤 선택을 바꾼다. 더블클릭 삭제는 `window.defer`로 미룬 디스패치라 그 사이 상태가 바뀌는 길이 있는지.
- `Shift+클릭`의 「마지막 선택」과 러버밴드의 클립 순서는 결정 3절 4·5의 글이 기준이다.

## 리뷰 1 대응

판정 「수정 후 재리뷰」. 리뷰 초점(마우스 전용 편집 경로)은 지적 없음.

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 차단: 피아노롤·Normal을 떠나도 조작이 남음 | 수락. `apply_mode_event`가 모드·뷰가 바뀔 때 조작을 버리고(선택만 바뀐 `Select` 사건은 제외), 끌기·뗌·`Esc`는 `live_gesture`로 Normal 피아노롤인지 확인해 아니면 버린다. 결정 2절 1에 글로 적음 | d766e23 |
| 2 수정: Step에서 끌기가 선택을 만듦 | 수락. 지적 1과 같은 원인(검사가 누를 때만)이라 같은 수정 | d766e23 |
| 3 수정: 누른 점이 화면 px | 수락. `mouse::Anchor`(틱·행)로 기억하고 직사각형은 그릴 때 뷰포트로 바꾼다. 결정 3절 5에 적음 | d766e23 |
| 4 수정: 빠진 시험 | 수락. `escape_during_plain_press_cancels_release_action`, `shift_click_empty_without_base_is_plain_click`, `cmd_wheel_zoom_stops_at_bounds` | 242798c |
| 5 제안: 폭 0 러버밴드 | 수락. 폭 0이면 한 틱(`band_between`). 결정 3절 5에 적음 | d766e23 |
| 6 제안: 시험 먼저 | 수락. 이번 수정은 리뷰 시험 6개가 실패하는 상태에서 했다 | — |

리뷰 시험 `drag_from_outside_after_return_does_not_resume_stale_band`의 전제 단언(「남은 조작」)은 수정으로 거짓이 되어 「뷰 전환이 조작을 버린다」로 바꿨다. 결함을 단언하던 뒤 단언은 그대로다. 수정 뒤 `mouse_select` 26, `mouse_select_pure` 7, `review_s3_t6a` 10 통과, clippy·fmt 통과.

## 리뷰 2 대응

판정 「수정 후 재리뷰」. 리뷰 1 지적 1·2·4와 제안 5는 고쳐졌다고 확인됨.

| 지적 | 대응 |
| --- | --- |
| 1 수정: 누른 채 스크롤·줌 뒤 움직이지 않고 떼면 선택이 직사각형과 다름 | 수락. 원인은 선택을 「끌 때만」 다시 계산한 것이다. 선택은 누른 점·지금 점·뷰포트로 정해지므로 뷰포트가 바뀌는 길(Element의 새 콜백 `on_view_change`: 휠·`Cmd+휠`·핀치)과 뗌에서도 같은 `update_band`를 부른다. 결정 3절 5에 적음 |
| 2 제안: `lo + 1` 넘침 | 수락. `saturating_add(1)` |
| 3 제안: 빈 뷰포트 검사 | 수락. `update_band`가 크기 0 뷰포트면 아무것도 하지 않는다 |

`register_mouse`가 clippy 줄 수 상한을 넘어 휠·핀치를 `register_view_input`으로 뗐다. 수정 뒤 `review_s3_t6a_r2` 8, `review_s3_t6a` 10, `mouse_select` 26, `mouse_select_pure` 7, `pianoroll` 13, `review_s3_t1`·`r2` 통과.

## 리뷰 3 대응

판정 「병합 가능」. 제안 둘 모두 수락(ac882a2).

| 지적 | 대응 |
| --- | --- |
| 1 제안: 키 줌(`cmd-=` 등)·가운데가 `on_view_change`를 거치지 않음 | 수락. 리뷰 2와 같은 갈래라 뷰포트가 바뀌는 길을 모두 묶었다: 줌·가운데 처리기와 `cursor_moved`(커서를 따라 스크롤)가 `piano_roll_view_changed`를 부른다. 결정 3절 5 문구도 고침 |
| 2 제안: 틱 상한의 폭 0 범위가 빔 | 수락. 상한에서는 아래로 한 틱 `[MAX−1, MAX)` |

수정 뒤 `review_s3_t6a_r3` 9, `review_s3_t6a_r2` 8, `review_s3_t6a` 10, `mouse_select` 26, `mouse_select_pure` 7, `pianoroll` 13, `pianoroll_select` 14, `review_s3_t1` 19, `vim_operators` 23 통과.
