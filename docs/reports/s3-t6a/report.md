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
- [ ] test(전체): 리뷰 뒤 `--no-fail-fast`로 돌린다(리뷰어 시험과 겹치면 지터 시험이 흔들린다)
- [x] deny
- [x] `scripts/check-deps.sh`
- [ ] `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`: 리뷰 뒤 전체 시험과 함께 기록

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
