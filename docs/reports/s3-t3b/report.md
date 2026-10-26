# Task 보고: S3-T3b 노트 편집

- PR: 브랜치 `s3/t3b-note-edit`
- 계획 파일: docs/reports/s3-t3b/plan.md
- 결정: `docs/decisions/2026-10-26-s3-t3b-note-edit-details.md`(사람, 2026-10-26), `docs/decisions/2026-10-17-keymap-conflicts.md` A1·A9·B25
- 계획 대비 달라진 점
  - `Shift+q` 설정 시트는 계획대로 S3-T3e로 뗐다(단계 파일 「나눔」에 줄을 더했다).
  - 시험 이름: 계획의 10·11·12·14를 기본·vim으로 나눴다(`vim_s_splits_cursor_note`, `vim_shift_j_joins`, `vim_g_digits_set_length`, `vim_r_sets_cursor_note_pitch`). 순수 시험을 더했다(`split_keeps_lyric_on_first_half_only`, `join_takes_values_from_earliest_and_latest_end`, `set_pitch_changes_only_pitch`, `quantize_uses_given_settings_and_none_when_on_grid`, `selection_after_keeps_untouched_targets_and_adds_new_notes`, `pitch_in_octave_of_uses_cursor_octave`, `legato_none_when_already_connected_or_last`).
  - 기존 시험 하나의 단언을 결정에 맞춰 뒤집었다: `tests/review_s2_t2.rs` `shift_digit_dotted_length_matches_macos_keystroke`의 「대조군」은 `shift-3` 바인딩이 닿는다고 단언했는데, 결정 B25 a′가 `shift-N` 바인딩을 지우라고 한다. 시험은 지우지 않고 대조군을 「`shift-3`은 이제 닿지 않는다」로 바꿨다. 본 시험(입력 글자 `#`가 점음표에 닿음)은 그대로다.
  - T3a `edit_allowed`의 배치 뷰 문구에 「분할·길이 등」을 더했다(같은 함수를 쓴다).
  - `Settings::load`가 clippy `too_many_lines`(107줄)에 걸려 이유를 단 `allow`를 붙였다. 기존 읽기 코드는 바꾸지 않았다.

## 구현 요약

- `src/pianoroll/edit.rs`(순수): `length_ticks`·`DEFAULT_LENGTH`, `Changed`(편집 + 커서가 따라갈 짝), `plan_split`(원래 노트를 뒤쪽으로 바꾸고 앞쪽을 넣음 → 꼬리를 이어받음, 가사는 앞쪽만), `plan_join`(음높이 묶음, 꼬리 노트가 있으면 그것을 바꿈), `plan_set_length`, `plan_set_pitch`, `plan_legato`(클립의 다음 시작, `partition_point`), `plan_restore`, `plan_quantize`(core `quantize`, 같은 값 짝을 걸러 변화 판정), `selection_after`. 분할 외는 모두 core `map_notes`(S0-T7 경계 규칙)를 거친다.
- `src/pianoroll/keys.rs`(순수): `semitone_of_key`(P3 두 레이아웃), `pitch_in_octave_of`. S3-T4가 다시 쓸 수 있다.
- `src/note_ops.rs`(창 처리기): `note_targets`(T3a `edit_allowed`·`edit_targets` 재사용) → 순수 계산 → `finish`(적용, 선택에서 시작했으면 `selection_after`, 커서 노트면 `follow_cursor_note`, `.` 기억). 처리기 9개(`Split` `Join` `SetLength` `SetDottedLength` `ResetLength` `Quantize` `Legato` `SetPitch` `RepeatLast`). `e` 대기는 창의 `intercept_keystrokes`(`intercept_pitch_keys`)가 다음 키 하나를 먹는다. 레가토 기억은 (판, 트랙, 클립, 짝, 적용 뒤 대상 값 집합). `.`은 `LastEdit::{Action, Op, Pitch}`로 기억하고 Action은 `window.dispatch_action`(다음 효과 주기)으로, `Alt` 조작·음높이는 바로 적용한다. 처음에 `seal_history`로 묶음을 끊는다.
- `src/note_edit.rs`·`src/selection.rs`: 성공한 삭제·잘라내기·붙여넣기·복제·반복·`Alt` 조작이 `.`에 기억된다(`remember_edit`). 도우미 몇 개를 `pub(crate)`로.
- `src/settings.rs`: `QuantizeSettings{grid, triplet, strength, swing}`(기본 1/16·끔·100·0), `quantize` 객체 읽기·쓰기(필드별 경고, 모르는 키 보존), `to_core`.
- `src/window.rs`: `note_ops` 상태, `pitch_pending()`, 가로채기 구독.
- 키맵: `default.json`의 `shift-1`…`shift-7` 14개(`Normal`·`Step`) 삭제와 머리 주석(B25 a′). `vim.json`: `Normal`·`Normal > PianoRoll`·`Normal > Arrangement`에 `shift-j`=`edit::Join`·`shift-k`=`note::ShowInfo`(A1), `Normal`에 `g 1`…`g 7`·`g 0`(A9), `Step`의 B25 주석.
- REQ: 5.8.1 `j` `k` 행·`g` 행, 7.3 KM-7(표기 규칙). 결정 문서 `2026-10-17-keymap-conflicts.md`에 반영 줄.

## 테스트 결과

~~~
note_ops_pure: 16 passed
note_ops: 15 passed
note_edit(T3a): 10 passed, review_s2_t2: 26 passed
전체: (게이트 결과 아래)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `length_ticks_match_req_table` | 통과 |
| 2 | `split_cuts_notes_crossing_cursor_only`, `split_keeps_lyric_on_first_half_only` | 통과 |
| 3 | `split_keeps_hidden_tail_as_tail` | 통과 |
| 4 | `join_merges_same_pitch_groups`, `join_takes_values_from_earliest_and_latest_end` | 통과 |
| 5 | `join_with_tail_keeps_tail` | 통과 |
| 6 | `set_length_extends_clip_by_bar_and_skips_equal` | 통과 |
| 7 | `legato_sets_end_to_next_start_any_pitch`, `legato_none_when_already_connected_or_last` | 통과 |
| 8 | `semitone_of_key_follows_p3_layouts`, `pitch_in_octave_of_uses_cursor_octave` | 통과 |
| 9 | `quantize_settings_read_write_and_defaults` | 통과 |
| 10 | `split_slash_and_vim_s_split_at_cursor`, `vim_s_splits_cursor_note` | 통과 |
| 11 | `join_cmd_j_and_vim_shift_j`, `vim_shift_j_joins` | 통과 |
| 12 | `length_codes_set_selection_length`, `vim_g_digits_set_length` | 통과 |
| 13 | `shift_digit_keys_are_not_bound` | 통과 |
| 14 | `e_then_note_key_sets_cursor_note_pitch`, `vim_r_sets_cursor_note_pitch` | 통과 |
| 15 | `quantize_q_uses_settings`, `quantize_uses_given_settings_and_none_when_on_grid` | 통과 |
| 16 | `legato_tilde_toggles_back` | 통과 |
| 17 | `dot_repeats_last_edit` | 통과 |
| 18 | `vim_shift_k_is_show_info_and_g_digits_are_lengths` | 통과 |
| 19 | `outside_clip_selection_is_refused_for_new_edits` | 통과 |
| 20 | `arrangement_view_note_edit_keys_do_nothing` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [ ] test(전체, `--no-fail-fast`): 돌리는 중
- [ ] deny
- [ ] `scripts/check-deps.sh`, `check-deps.test.sh`, `bench-check.test.sh`

## 알려진 한계

- `Shift+q` 설정 시트 없음(S3-T3e). `settings.json`을 손으로 고치면 `q`가 따른다.
- vim `K`(`note::ShowInfo`)는 바인딩만 있다(S3-T7).
- 계이름형 `e`는 음 키 하나만 받는다(반음은 뒤이어 `+`/`-`). 결정 4-A.
- `e` 대기 중 마우스·메뉴로 피아노롤을 떠나면 대기는 다음 키에서 풀린다(그 키는 평소대로 동작). 그때까지 `pitch_pending()`은 참이다.
- `.`은 vim 오퍼레이터(`d w` 등)를 아직 모른다(S3-T3c).
- `0`의 기본 길이는 상수 960이다(기본 길이 설정은 S3-T4).
- 합치기·레가토·분할·길이·퀀타이즈는 선택의 클립 하나에만 적용된다(선택이 한 클립이라서).

## 리뷰어에게

- `plan_split`·`plan_join`의 숨김 꼬리 처리: core는 `ReplaceNotes`의 `from`이 꼬리일 때만 꼬리를 허용한다. 분할은 원래 노트를 뒤쪽으로 바꾸고, 합치기는 묶음의 꼬리 노트를 바꾼다. 같은 값 노트가 여럿일 때(대상 고르기는 같은 값 전부)도 `map_notes`의 개수 검사와 `RemoveNote` 순서가 맞는지.
- `selection_after`의 다중집합 처리(같은 값이 사라지고 다시 나타날 때).
- `.`이 `Action`을 `dispatch_action`(다음 효과 주기)으로 보내는 경로: 그 사이에 포커스가 바뀌거나 대기 키가 끼면 무엇이 되는가. 묶음 끊기 순서.
- `e` 가로채기가 키맵의 전역 가로채기(카운트·`Esc` 시퀀스)와 함께 돌 때. 먹은 키 뒤에 `in_key_dispatch`·카운트가 남는가.
- 레가토 토글의 기억 조건(판·대상 집합)이 너무 넓거나 좁지 않은가.
- 퀀타이즈 변화 판정은 core가 내는 `ReplaceNotes` 짝의 `from != to`에 기댄다. core가 다른 편집을 낼 때(클립 연장만) 빠지는 경우가 있는가.
