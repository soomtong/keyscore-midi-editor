# Task 보고: S3-T6b 드래그 이동·조절

- PR: 브랜치 `s3/t6b-mouse-drag`
- 계획 파일: docs/reports/s3-t6b/plan.md
- 결정: docs/decisions/2026-10-31-s3-t6-mouse-editing.md 6절(에이전트 해석, 이 Task에서 세부를 이어 적음)
- 계획 대비 달라진 점:
  - `ops::shifted`(노트마다의 결과 짝)를 공개로 더했다. 고스트가 편집 묶음 단계에서 거부된 계획(다음 클립과 겹침)의 자리를 보이려면 노트 계산만 따로 필요하다. `plan_shift`·`plan_shift_copy`도 이것을 부르므로 계산은 하나다.
  - `DragInput`의 음높이 인자는 `rows`가 아니라 `semitones`(위로 끌면 양수)다.
  - 시험 `dominant_axis_prefers_time_on_tie`, `snap_grid_follows_setting`, `settings_input_has_mouse_snap_choice`를 더했다(계획 표 밖).
  - 미리듣기를 PRE-1 공통 함수 `preview_note`로 뗐고(`preview_cursor_note`가 부름), `cursor_moved`에서 미리듣기만 뺀 `cursor_moved_quietly`를 두었다(놓은 뒤 커서가 따라갈 때 같은 음을 또 내지 않게).

## 구현 요약

- `pianoroll/ops.rs`: 일반 변환 `NoteShift { time: TimeShift, pitch, start, end }`, `NoteOp::shift(step)`. 노트 하나의 계산 `shift_note`(시간 → 시작 → 끝 → 음높이, 결과 길이 1틱 미만이면 그대로), `shifted`, `plan_shift`(`map_notes`), `plan_shift_copy`(`insert_notes`). `plan_note_op`은 `plan_shift(op.shift(step))` 한 줄이 됐다.
- `pianoroll/mouse.rs`: `Grip`·`grip_at`(MS-6), `Axis`·`dominant_axis`, `snap_nearest`(마디 시작 기준 격자, 다음 마디 시작도 격자선, 같으면 앞), `snap_grid`(MS-3), `DragInput`·`drag_shift`(스냅·자르기·한 축). `Gesture::Note`에 `Grab`(노트·자리·누른 내용 위치), 새 `Gesture::Move(Box<Drag>)`(대상 `Rc<[Note]>`, 지금 점, 수식키, 계산 열쇠, 고스트, 거부, 들려준 음높이), `Gesture::ghost_rects`.
- `pianoroll_mouse.rs`: 누름에서 `grab`, 문턱을 넘으면 잡은 노트가 선택에 있을 때 `Move`. `update_move`(움직임·수식키·뷰포트 변화가 모두 부름) → `plan_move`(변환이 같으면 다시 계산 안 함) → 고스트. 잡은 노트 고스트 음높이가 바뀌면 `preview_note`(설정 「이동 시 미리듣기」를 따름). 뗌에서 같은 계산을 한 번 더 하고 `finish_move`가 `apply_edit(.., None)`로 적용, 선택·「마지막 선택」·커서(본체 이동만)를 맞춘다. `piano_roll_modifiers`, 더블클릭 생성의 시작 격자도 `mouse_grid`.
- `pianoroll/element.rs`·`scene.rs`: 끌기·뗌 콜백이 수식키를 받는다. 장면 `ghosts`·`ghost_blocked`, 점선 테두리 고스트(거부면 재생 헤드 색).
- `window.rs`: 루트 요소의 `on_modifiers_changed`(`watch_modifiers`)가 `piano_roll_modifiers`를 부른다. 수식키 사건은 포커스 경로로만 온다.
- `actions.rs`·`commands/table.rs`·`settings.rs`·`settings_window.rs`·`settings_tabs.rs`: `MouseSnap`, `settings::SetMouseSnap`, `input.mouse_snap`(`"length"` 기본), 설정 > 입력 「마우스 스냅 단위」 선택 상자(Tab 순서 맨 뒤). `settings_tabs::sync`가 clippy 줄 수 상한을 넘어 줄 계산을 `selected_rows`로 뗐다.
- `selection.rs`: `preview_note`, `cursor_moved_quietly`. `docs/reports/s2-t2/action-catalog.md`에 새 Action 한 줄.

## 테스트 결과

~~~
mouse_drag_pure: 15 passed
mouse_drag: 18 passed
mouse_drag_preview: 1 passed (CoreMIDI 루프백)
관련 기존 시험(mouse_select 26, mouse_select_pure 7, review_s3_t6a 10·r2 8·r3 9, pianoroll_select 14,
settings_tabs 8, commands 7, keymap_engine 12, keymap_source 11, keymap_overlay 4, command_line 4,
step_settings 3, step_vim_mml 22): 모두 통과
전체 cargo test --workspace: 리뷰 뒤에 돌린다(아래 자동 게이트)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `note_ops_equal_plan_shift_of_their_shift` | 통과 |
| 2 | `plan_shift_keeps_notes_that_would_vanish` | 통과 |
| 3 | `plan_shift_rejects_before_clip_start_pitch_range_and_next_clip` | 통과 |
| 4 | `plan_shift_copy_inserts_copies_and_keeps_originals` | 통과 |
| 5 | `grip_edge_zone_is_six_px_or_third_of_narrow_note` | 통과 |
| 6 | `snap_nearest_rounds_to_bar_relative_grid` | 통과 |
| 7 | `drag_shift_body_snaps_grabbed_start` | 통과 |
| 8 | `drag_shift_free_rounds_raw_ticks` | 통과 |
| 9 | `drag_shift_clamps_clip_start_and_pitch_range` | 통과 |
| 10 | `drag_shift_end_snaps_and_keeps_grabbed_length` | 통과 |
| 11 | `drag_shift_start_keeps_end_and_clamps` | 통과 |
| 12 | `drag_shift_axis_lock_zeroes_other_axis` | 통과 |
| 13 | `mouse_snap_setting_round_trips_and_rejects_bad_values` | 통과 |
| 14 | `drag_selection_keeps_relative_positions_with_one_undo` | 통과 |
| 15 | `ghost_shows_result_while_document_stays` | 통과 |
| 16 | `escape_during_drag_cancels_and_release_does_nothing` | 통과 |
| 17 | `cmd_during_drag_disables_snap` | 통과 |
| 18 | `shift_drag_moves_on_one_axis` | 통과 |
| 19 | `alt_drag_duplicates_with_one_undo` | 통과 |
| 20 | `right_edge_drag_moves_all_ends_by_same_amount` | 통과 |
| 21 | `left_edge_drag_moves_start_and_keeps_end` | 통과 |
| 22 | `narrow_note_edge_zone_is_a_third` | 통과 |
| 23 | `drag_past_clip_end_extends_clip_by_bars` | 통과 |
| 24 | `blocked_drag_shows_blocked_ghost_and_does_not_edit` | 통과 |
| 25 | `cursor_follows_body_move` | 통과 |
| 26 | `drag_of_cmd_toggled_out_note_does_nothing` | 통과 |
| 27 | `document_change_during_drag_cancels` | 통과 |
| 28 | `wheel_during_drag_recomputes_ghost` | 통과 |
| 29 | `mouse_snap_setting_changes_drag_and_double_click_grid` | 통과 |
| 30 | `drag_in_step_mode_does_nothing` | 통과 |
| 31 | `body_drag_previews_each_new_pitch_once` | 통과 |
| 추가 | `dominant_axis_prefers_time_on_tie`, `snap_grid_follows_setting`, `settings_input_has_mouse_snap_choice` | 통과 |

시험 순서: 순수 시험 15개와 UI 시험 17개, 미리듣기 시험은 `todo!()` 뼈대로 실패를 확인한 뒤(커밋 e48d028) 구현했다. `settings_input_has_mouse_snap_choice`는 설정 창 선택 상자를 만든 뒤에 썼다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(전체): 리뷰 3 대응 뒤 `cargo test --workspace --no-fail-fast` 286 바이너리, 2641 통과, 0 실패
- [x] deny
- [x] `scripts/check-deps.sh`
- [x] `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`(둘 다 종료 코드 0)

## 알려진 한계

- 끝 영역 위의 포인터 모양(좌우 화살표)은 바꾸지 않는다(REQ에 없다).
- 드래그 중 자동 스크롤은 없다. 지금 점은 피아노롤 가장자리에서 멈춘다.
- 대상은 한 클립(선택이 한 클립이다). 배치 뷰 클립 드래그는 S4-T4.
- 마우스 편집은 `.` 반복이 기억하지 않는다(결정 6절 10).
- 실제 창에서의 감각(끝 영역 6 px, 고스트 점선 색)은 사람 확인 항목이다.

## 리뷰어에게

- 리뷰 초점(단계 파일): 마우스 조작이 키 조작과 같은 계산을 쓰는가. 키 `Alt` 조작의 `plan_note_op`은 `plan_shift(op.shift(step))`이고 마우스는 같은 `plan_shift`·`plan_shift_copy`를 부른다. 노트 하나의 계산은 `ops::shift_note` 하나뿐이다. 마우스 전용 편집 경로가 있으면 차단 대상이다.
- 고스트와 적용: 끄는 동안 `plan_move`가 만든 계획의 `moved`가 고스트이고, 뗌은 같은 `update_move`로 마지막 계산을 한 뒤 같은 변환으로 계획을 다시 만들어 적용한다. 거부된 계획의 고스트만 `shifted`(노트 계산)에서 온다.
- 불안한 곳: `plan_shift`가 `map_notes` 콜백을 「고른 순서대로 부른다」에 기대어 결과를 차례로 꺼낸다(지금 `transform::plan`은 그렇다). 수식키 변화는 창 루트의 `on_modifiers_changed`로 받는데, 포커스가 피아노롤 밖(입력란 등)이면 오지 않는다(끄는 중에는 누름이 포커스를 문서로 가져온다). 왼쪽 끝 조절의 스냅이 「끝 앞 마지막 격자선」으로 물러날 때 원래 시작보다 앞일 수 있다(의도: 결정 6절 3).
- 스냅·자르기·한 축의 기하는 결정 6절 3–6의 글이 기준이다. 글과 코드가 어긋나면 지적해 달라.

## 리뷰 1 대응

판정 「수정 후 재리뷰」. 리뷰 초점(마우스 전용 편집 경로)은 지적 없음.

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 차단: `shift_note`가 노트를 새로 만들어 가사·피치 오프셋·모르는 키가 사라짐(키 `Alt` 조작 포함 회귀) | 수락. `with_length` → `with_start` → `with_pitch`로 원래 노트를 고친다. `with_pitch`가 오프셋을 더한 절대 피치 범위도 다시 검사한다 | 623e9fc |
| 2 수정: 수식키를 쥔 채 `Esc`가 드래그를 취소하지 않음 | 수락. `Esc` 가로채기가 수식키와 무관하게 진행 중 조작을 취소한다(조작이 없으면 키를 그대로 흘려보낸다). 결정 6절 11에 적음 | 62d9d9b |
| 3 제안: 격자 밖 짧은 노트의 끝을 안쪽으로 끌면 길어짐 | 수락. 대체 격자선은 원래 끝을 넘지 않고(끝 조절), 원래 시작 앞으로 가지 않는다(시작 조절). 결정 6절 3 문구 고침 | 56b0af1 |
| 4 제안: `watch_modifiers`가 `swallow_root_tab` 문서 주석 가운데에 들어감 | 수락. 주석을 제자리로 | 2a0cee3 |

리뷰 시험 정정 둘(숨기지 않고 적는다):
- `review_key_alt_ops_keep_lyric_and_pitch_offset`: 틱 960의 노트를 한 마디 앞당기는 경우는 클립 시작 앞이라 main에서도 거부된다. 그 경우만 거부를 단언하게 바꿨다. 나머지 아홉 조작의 가사·오프셋 단언은 그대로다.
- `review_end_grip_left_drag_on_short_offgrid_note_lengthens`: 제안 3을 받아들여 기대를 끝 Δ 0으로 바꿨다(이름은 그대로).

수정 뒤 `review_s3_t6b` 8, `mouse_drag` 18, `mouse_drag_pure` 15, `mouse_select` 26, `review_s3_t6a` 10, `pianoroll_select` 14 통과, fmt·clippy 통과.

## 리뷰 2 대응

판정 「수정 후 재리뷰」. 리뷰 1 지적 1–4는 고쳐졌다고 확인됨. 단계 게이트 셋과 리뷰 초점도 확인됨.

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 수정: 피치 오프셋이 있는 노트를 위로 끌면 가장자리에서 멈추지 않고 전체 거부(고스트도 사라짐) | 수락. `drag_shift`의 음높이 자르기가 노트마다 `pitch_room`(음높이 0–127과 오프셋을 더한 절대 피치 `[0, 128)` 반음)을 본다. 결정 6절 5 문구 고침 | 4c63b6f |
| 2 수정: 키맵의 `edit::CancelDrag`(「Dragging」)에 처리기가 없고 실제 취소는 하드코딩된 `escape` | 수락. 가로채기는 그대로 앞에서 받되(vim 시퀀스 대기보다 먼저), 키는 키맵의 `edit::CancelDrag` 바인딩과 맞춰 본다(드래그 수식키를 뺀 모양 포함). `edit::CancelDrag` 처리기(팔레트)도 단다. 새 시험 `rebound_cancel_drag_key_cancels_drag`. 결정 6절 11 문구 고침 | f7c9d7d |
| 3 제안: 가장 가까운 격자선 스냅이 안쪽으로 끈 끝을 바깥으로 보냄 | 결정 문구만 좁혔다(대체 격자선 규칙의 목적, 스냅 자체의 성질은 그대로). 스냅 방향은 사람 확인 항목으로 남긴다 | f7c9d7d |

사용자 키맵이 「Dragging」의 `escape`를 `null`이나 `zed::Unbind`로 지우면, GPUI `Keymap::bindings_for_action`이 그 바인딩을 걸러 낸다(`disabled_binding_indices` 검사). 그래서 `escape`는 더 취소하지 않는다(코드로 확인).

수정 뒤 `review_s3_t6b_r2` 9, `review_s3_t6b` 8, `mouse_drag` 19, `mouse_drag_pure` 15, `mouse_select` 26, `review_s3_t6a` 10·r2 8·r3 9, `commands` 12, `keymap_engine` 11 통과, fmt·clippy 통과.

## 리뷰 3 대응

판정 「병합 가능」. 리뷰 2 지적 1·2의 수정이 확인됐다(오프셋 경계 1500여 조합, 사용자 키맵 파일로 취소 키 옮기기, 조작 없을 때 가로채지 않음).

| 지적 | 대응 |
| --- | --- |
| 1 제안: `cancels_drag`가 바인딩 컨텍스트를 보지 않음(다른 컨텍스트의 `edit::CancelDrag` 키도 취소) | 동작은 그대로 두고(드래그 중에는 해가 없다) 결정 6절 11 문구를 동작에 맞췄다 |
