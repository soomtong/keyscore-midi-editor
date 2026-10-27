# Task 보고: S3-T3c vim 오퍼레이터와 비주얼 선택

- PR: 브랜치 `s3/t3c-vim-operators`
- 계획 파일: docs/reports/s3-t3c/plan.md
- 결정: `docs/decisions/2026-10-26-s3-t3c-vim-operators.md`(사람, 2026-10-26, 네 항목 모두 A)
- 계획 대비 달라진 점
  - `i` `a` 단독(`null`)은 GPUI가 시간 초과를 걸지 않아 다음 키를 기다린다. 텍스트 객체 시퀀스 중의 `Esc`는 A13대로 시퀀스만 취소한다. 계획 2절 문장을 고쳤다.
  - 오른쪽 클릭(마크 메뉴)도 오퍼레이터 대기를 끝낸다(계획 2절에 더함).
  - undo 뒤 선택과 「선택이 있으면 바로」의 겹침을 계획 「리스크」에 더했다.
  - 테스트 이름은 계획 표 그대로다. 순수 시험은 계획의 2·3·4를 여덟 개로 나눴다.

## 구현 요약

- `src/pianoroll/vim.rs`(순수): `Operator`, `Motion`(`reach`·`counts`·`as_cursor_move`), `Reach`, `TextObject`, `VisualKind`, `motion_target`(S3-T1·T2의 이동 + 새 `W` `B` `e` `E`), `reading_key`(칸 → 읽기 순서 자리), `motion_notes`(배타·포함·클립 처음부터·클립 끝까지), `object_notes`, `visual_notes`.
- `src/pianoroll/mod.rs`: `moved`(`move_cursor`의 순수 판), `PianoRollState::visual`/`set_visual`, `set_anchor(None)`이 비주얼도 끝낸다.
- `src/vim.rs`(창): `VimState{pending, ignore_replayed_operator}`, `context_path`(모드 경로 + `Operator`/`Visual`, 정적 배열), `mode_label`(`NORMAL 3d`, `VISUAL` …), 키 가로채기 `intercept_vim_keys`(대기 중 허용되지 않는 키는 동작 없이 취소, `y` 접두 뒤의 키는 삼켜 `y`를 먼저 확정한 뒤 다시 보냄), `run_motion`(모든 모션 처리기의 공통 길: 카운트, 오퍼레이터 대기면 범위에 작용), `apply_operator`(d·y·c, 커서·선택·Step·`.` 기억), 객체·비주얼 처리기, `C`(`edit::ChangeToBarEnd`), 처리기가 없던 모션 넷(`NextBeat` `PrevBeat` `NoteEnd` `LastNoteEndInBar`).
- `src/selection.rs`·`src/pianoroll_window.rs`: 이동 처리기가 `run_motion`/`run_find`를 거친다. `cursor_moved`는 비주얼 중이면 기준 칸을 지우지 않고 선택을 다시 계산한다. `Shift` 넓히기는 비주얼만 끝낸다. 클릭·우클릭은 오퍼레이터 대기를 끝낸다(`click_piano_roll_cell`).
- `src/note_edit.rs`: `delete_notes`·`clear_selection_after_edit`·`copy_targets`를 `pub(crate)`로, `D`의 대상 계산을 `bar_end_targets`로 떼어 `C`와 함께 쓴다.
- `src/note_ops.rs`: `LastEdit::Operator{op, target}`, `.`이 `apply_operator`로 되풀이.
- `src/window.rs`: 컨텍스트 겹과 상태줄이 `context_path`·`mode_label`을 쓴다. 모드·뷰 전환과 트랙 바뀜이 대기·비주얼을 끝낸다. 키 관찰이 대기 중 실행된 `y y`·`y y p`로 대기를 끝낸다(`vim_after_keystroke`).
- `src/keymap/mod.rs`: 대기 입력 비우기를 `flush_pending_keystrokes`로 뗐다(Esc 가로채기와 함께 씀). `src/overlay.rs`·`src/commands/mod.rs`: 오버레이·메뉴 키 표시가 `Operator`·`Visual` 경로도 본다.
- `assets/keymaps/vim.json`: `Normal > Operator`에 `i`·`a` `null`, 새 `Normal > Visual`(텍스트 객체 셋, `i`·`a` `null`).

## 테스트 결과

~~~
vim_pure: 8 passed
vim_operators: 23 passed
keyscore-app 전체: (아래 게이트)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `operator_times_target_table` | 통과 |
| 2 | `reading_key_uses_note_start_under_cell_or_cell`, `motion_notes_exclusive_inclusive_and_chords`, `motion_reach_table` | 통과 |
| 3 | `beat_targets_follow_meter`, `note_end_targets`, `step_and_note_targets_reuse_s3_rules` | 통과 |
| 4 | `object_notes_inner_note_bar_and_clip`, `visual_notes_by_kind` | 통과 |
| 5 | `count_moves_cursor` | 통과 |
| 6 | `operator_count_multiplies` | 통과 |
| 7 | `pitch_motion_cancels_operator` | 통과 |
| 8 | `other_key_cancels_operator_without_running` | 통과 |
| 9 | `escape_cancels_operator` | 통과 |
| 10 | `yank_prefix_then_object_or_motion` | 통과 |
| 11 | `yank_prefix_escape_cancels` | 통과 |
| 12 | `operator_with_selection_acts_now` | 통과 |
| 13 | `operator_in_arrangement_says_piano_roll` | 통과 |
| 14 | `status_shows_pending_operator_and_visual` | 통과 |
| 15 | `visual_modes_select_by_kind` | 통과 |
| 16 | `visual_toggle_switch_and_escape` | 통과 |
| 17 | `visual_objects_replace_selection` | 통과 |
| 18 | `visual_operators_act_and_end` | 통과 |
| 19 | `visual_ends_on_extend_select_and_view` | 통과 |
| 20 | `visual_click_extends` | 통과 |
| 21 | `dot_repeats_operator` | 통과 |
| 22 | `change_to_bar_end` | 통과 |
| 23 | `operator_is_one_undo_unit` | 통과 |
| 24 | `click_cancels_operator` | 통과 |
| 25 | `default_preset_unchanged` | 통과 |
| 26 | `palette_object_without_operator_says` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(워크스페이스 `--no-fail-fast`, 0ee70f0 기준 2151 통과, 실패 0)
- [x] deny
- [x] `scripts/check-deps.sh`, `check-deps.test.sh`, `bench-check.test.sh`

## 알려진 한계

- 배치 뷰의 오퍼레이터·비주얼·이동 카운트는 S4. 검색 모션(`n`)은 S3-T7. 마크 점프는 모션이 아니다(대기 취소).
- `c`가 들어간 Step 모드의 음 입력은 S3-T4다. `.`은 `c`의 삭제와 Step 진입까지만 되풀이한다.
- 모션 카운트 반복은 진행이 없으면 멈추지만, 노트 단위 모션(`w`)은 한 번에 트랙의 노트를 훑으므로(S3-T2 `next_note`) `9999 w`는 노트 수 × 카운트에 비례한다.
- 읽기 순서 규칙(결정 1 A)의 결과로 `d h`·`d B`가 같은 틱의 낮은 음을 남긴다(시험 1 기록).
- undo가 만든 선택 뒤의 `d` `y` `c`는 모션을 기다리지 않는다(계획 「리스크」).

## 리뷰어에게

- `src/vim.rs` `vim_keystroke`: GPUI의 대기 입력·재생 동작에 기대는 부분이다. `y` 접두 뒤 키를 삼켜 지연 두 번(비우기 → 다시 그림 → 다시 보냄)으로 처리하고, 대기 취소 때는 지금 컨텍스트에서 비운 뒤 취소한다(먼저 취소하면 `i`가 Step으로 재생됨). `ignore_replayed_operator`로 비우기가 재생하는 `y`를 무시한다.
- 대기 중 허용 목록(`allowed_while_pending`)이 계획 2절과 맞는지, 빠진 모션이나 새는 키가 없는지.
- 모든 이동 처리기를 `run_motion`으로 바꿨다. 기본 프리셋의 이동은 카운트가 없어 한 번이다(기본 프리셋 숫자는 길이 코드).
- 비주얼과 S3-T2 기준 칸의 상호작용(`cursor_moved`, `extend`, `set_anchor`).

## 리뷰 1 대응

| # | 구분 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | 선택이 생기면(메뉴 `select::All`, undo가 되살린 노트 등 키가 아닌 경로 포함) `sync_selection_mode`가 오퍼레이터 대기를 끝낸다. 불변식 「대기와 선택은 함께 없다」를 한곳에서 지킨다 | 9ba9ab1 |
| 2 | 수정 | 대기 중 `y` 시퀀스의 Esc도 비우기가 재생하는 `y`를 무시한다(`ignore_replayed_operator_once`). 리뷰 시험의 「대기도 끝난다」 단언 하나는 A13·계획 2절(시퀀스 중 Esc는 시퀀스만 취소, `d i escape`와 같음)에 맞춰 「`y` 대기는 남고 다음 Esc가 끝낸다」로 바꿨다. 복사가 일어나지 않는다는 단언은 그대로다 | df2f893, 68d2127 |
| 3 | 수정 | 사람 결정(2026-10-27, 「거부, 선택 유지」): 비주얼 중 커서가 선택의 클립을 벗어난 뒤 `d` `c`는 S3-T3a대로 거부하고, 선택·비주얼은 남긴다(거부 때 `end_visual`을 하지 않는다) | aeb202d |
| 4 | 제안 | 받지 않음: 결정 1 A와 계획 4절 「목적지가 없으면 대상이 없다」 글자대로다 | - |
| 5 | 제안 | 받지 않음: 계획 8절의 「선택(편집 대상)」 되풀이다. vim의 같은 길이 반복은 범위 밖 | - |
| 6 | 제안 | 받음(문서): 계획 8절에 「노트가 없는 `c`도 Step 진입은 했으므로 마지막 편집이 된다」를 적었다 | - |
| 7 | 제안 | 받지 않음: 계획 2절 「그 밖의 키는 동작 없이 대기만 취소」(vim과 같다). 사람이 바꾸려면 결정 요청 | - |

## 리뷰 2 대응

| # | 구분 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 1 지적 1과 같은 갈래라 원인을 고쳤다: GPUI 대기 입력이 `Operator`·`Visual` 경로에서 남으면 창이 그 경로를 붙들어 그리고(`VimState::held`, `key_context_path`), 경로가 달라진 뒤 다음 키가 오면 붙든 경로에서 대기 입력을 비운 다음 그 키를 다시 보낸다(`flush_held_then`). 클릭·메뉴 선택·비주얼을 끝내는 `select::All` 모두 같은 길이다 | f4b0b32 |
| 2 | 수정 | 기준 칸만 있는 선택에서 오퍼레이터를 시작하면 기준 칸을 지우고 대기에 든다(계획 2절에 규칙) | cbb04d6 |
| 3 | 수정 | 대기를 끝내는 모든 길(`clear_operator`: 클릭, 선택이 생김, 모드·뷰·트랙 바뀜)이 카운트도 비운다 | a9445a1 |
| 4 | 제안 | 받음(문서): 계획 2절에 `y` 대기의 `y y escape` 동작을 적었다 | - |
| 5 | 제안 | 받음(문서): 계획 2절에 「메뉴·팔레트의 다른 편집은 실행되고 대기는 남는다」를 적었다. 불변식(대기와 선택)은 선택이 생기면 대기를 끝내는 규칙이 지킨다 | - |

## 리뷰 3 대응(결정 `docs/decisions/2026-10-27-s3-t3c-review-limit.md` A)

| # | 구분 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | (가) 창이 `observe_pending_input`로 대기 입력을 보고, 없어지면 붙들기를 놓고 다시 그린다(수식키 단독 입력 등 가로채기를 거치지 않는 비우기). (나) 그리기(`render`) 때 붙든 경로의 대기 입력이 남았는데 지금 경로와 다르면, 곧바로 그 경로에서 대기 입력을 비우고 붙들기를 놓는다. 다음 키를 기다리지 않으니 다른 모드·뷰가 `Operator`로 그려지지 않는다. 리뷰어의 기록 시험 `object_prefix_then_menu_step_draws_operator_context_records`의 단언은 수정 뒤 동작(Operator로 그려지지 않음, 대기 입력 없음)으로 뒤집었다 | 4f2d301 |
