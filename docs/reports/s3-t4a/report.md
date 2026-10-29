# Task 보고: S3-T4a Step 입력 핵심

- PR: 브랜치 `s3/t4a-step-input`
- 계획 파일: docs/reports/s3-t4a/plan.md
- 결정: `docs/decisions/2026-10-29-s3-t4-step-input-details.md`(1·2 사람 결정, 3–7 에이전트 해석)
- 계획 대비 달라진 점
  - 시험 9를 둘로 나눴다(`space_rests_and_does_not_create_clip`, `space_in_clip_pushes_later_notes`). 한 시험 안에서 창을 두 번 여는 대신이다.
  - 시험 23은 조합 밖의 `8`·`9`가 바인딩 없음(`None`)임을 따로 본다. 길이 코드는 0–7이라 맞는 동작이다.
  - `StepState`가 레이아웃을 들고, 창이 그릴 때 설정에서 읽어 둔다(`refresh_step_layout`). 키 컨텍스트 경로는 `&self`에서 고르기 때문이다.

## 구현 요약

- `crates/keyscore-core/src/transform.rs`: `Splice`와 `splice`. 클립 하나에서 `at` 이상 노트를 `by`만큼 밀거나 당기고, 지우기·바꾸기·넣기를 한 묶음(`RemoveNote`들, `ReplaceNotes` 또는 `ExtendClip`, `InsertNote`들)으로 만든다. 연장은 `insert_notes`와 같은 마디 단위 규칙(다음 클립 시작까지, 닿으면 거부), 꼬리 노트는 시작만 클립 안.
- `crates/keyscore-app/src/step.rs`(새 파일): `StepLength`, `step_ticks`(점·셋잇단·절반), `length_label`(`♩` `♪` `1/16` …), `pitch_of`(C4 = 60, 0–127), `StepState`(길이·옥타브·`n` 접두·조합·열린 화음·직전 스텝·세대 번호), `StepStatus`.
- `crates/keyscore-app/src/step_input.rs`(새 파일): `step::` 처리기 17개. 조합 → 확정(`step_commit`) → 쓰기(`step_write`: 클립 안이면 `splice`, 틈이면 `clip_for_notes`) → 전진. 대기 시간은 세대 번호로 맞춘 타이머. 쉼표·`Backspace`(지우고 당김, 조합 중이면 조합만 버림)·`Tab`/`Shift+Tab`·화음·타이·옥타브. 미리듣기는 기존 `sound_note`.
- `crates/keyscore-app/src/window.rs`: 필드 `step`, Step 처리기 등록, 그리기 전 레이아웃 읽기, Step을 나갈 때·undo·redo 전에 `step_settle`.
- `crates/keyscore-app/src/vim.rs`: Step 모드면 키 컨텍스트 경로 `Step > Piano|Solfege [> Pending] [> Ghost]`와 상태줄 `STEP ♩ oct4 [5c_]`.
- `crates/keyscore-app/src/settings.rs`: `input.step_default_length`(기본 `quarter`), `input.octave_wait_ms`(0–2000, 기본 300).
- `assets/keymaps/default.json`: 변화표 `#` `+` `-`를 `Step > Solfege > Pending`으로 옮겼다(조합이 없을 때 `#`은 점4분).

## 테스트 결과

~~~
keyscore-core --test transform_splice: 11 passed(속성 시험 1 포함)
keyscore-app --test step_input: 27 passed
keyscore-app --test step_settings: 3 passed
keyscore-app step::tests(단위): 4 passed
기존 시험 부분 실행·전체 실행: 아래 「자동 게이트」
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `req_5_6_rows_solfege_match_table` | 통과 |
| 2 | `req_5_6_rows_piano_match_table` | 통과 |
| 3 | `octave_wait_commits_after_timeout_with_fake_clock` | 통과 |
| 4 | `octave_wait_setting_changes_timeout` | 통과 |
| 5 | `enter_and_next_note_commit_immediately` | 통과(낡은 타이머가 지금 조합을 건드리지 않음도 봄) |
| 6 | `shift_note_is_half_once_not_sticky` | 통과 |
| 7 | `length_codes_and_dotted_are_sticky_zero_resets_to_setting` | 통과 |
| 8 | `triplet_prefix_applies_to_next_note_or_rest_only` | 통과 |
| 9 | `space_rests_and_does_not_create_clip`, `space_in_clip_pushes_later_notes` | 통과 |
| 10 | `backspace_deletes_step_and_pulls_later_notes` | 통과 |
| 11 | `backspace_while_composing_discards_composition` | 통과 |
| 12 | `tab_confirms_and_advances_shift_tab_steps_back` | 통과 |
| 13 | `ctrl_note_builds_chord_plain_note_or_tab_advances` | 통과 |
| 14 | `tie_extends_previous_step_and_advances` | 통과 |
| 15 | `octave_keys_change_sticky_octave_and_stop_at_bounds` | 통과 |
| 16 | `piano_octave_digit_sets_row_base` | 통과 |
| 17 | `solfege_accidentals_only_while_composing` | 통과 |
| 18 | `outside_clip_creates_bar_clip_and_extends_at_end` | 통과 |
| 19 | `insert_pushes_later_notes_and_rejects_next_clip_overlap` | 통과 |
| 20 | `each_commit_is_one_undo_unit` | 통과 |
| 21 | `leaving_step_commits_composition` | 통과 |
| 22 | `status_shows_length_octave_and_composition` | 통과 |
| 23 | `step_contexts_have_no_note_key_conflicts` | 통과 |
| 24 | `vim_preset_uses_same_step_grammar` | 통과 |
| 25 | `step_in_arrangement_view_writes_at_cursor` | 통과 |
| 26 | `first_step_edit_clears_note_selection` | 통과 |
| 27 | `splice_*`(10개) | 통과 |
| 28 | `splice_applies_and_keeps_invariants` | 통과 |
| 29 | `input_settings_*`(3개) | 통과 |
| 30 | `step::tests::*`(4개) | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음. 미리듣기(PRE-1)는 시험 환경에 MIDI 엔진이 없어 소리 자체는 보지 않는다(기존 `sound_note` 경로를 부른다).

## 자동 게이트

GATES_PLACEHOLDER

## 알려진 한계

- 5.6 표의 `4na 4nb 4nc`처럼 음 뒤에 숫자가 이어지는 입력은 대기 시간(300 ms) 안이면 옥타브로 읽힌다. 문법대로의 동작이고, 시험은 행 안의 토큰 사이에 대기 시간을 흘린다. 빨리 치면 `4na4`는 옥타브 4가 된다.
- 화음의 스텝 길이는 첫 화음 음의 길이다. 화음 안에서 길이가 다른 음(`Shift+음`)을 섞으면 노트는 제 길이지만 전진은 첫 음 길이만큼이다.
- 타이·직전 스텝은 커서가 그 스텝 끝에 있을 때만 유효하다. Normal 키로 커서를 옮겼다 돌아오면 다시 유효해진다(노트 값이 그대로면).
- `Backspace`는 클립 길이를 줄이지 않는다(줄이기는 S4-T2 비파괴 트림).
- 클립 끝(끝 = 커서)에서 친 쉼표도 클립을 마디 단위로 늘린다. 곡 끝에 쉼표를 치면 빈 마디가 남는다(리뷰 2 제안 5). 줄이기는 S4-T2.
- 범위 밖(S3-T4b·T4c): vim 진입 위치와 `R`, vim Step 표의 `Shift` 조합, 계이름 MML 호환(`o5` `l8` `r`), 설정 창 입력란, 미리듣기 가벼운 경로, 배치 뷰 표시.

## 리뷰어에게

- `step_input.rs`의 `step_commit` 화음 분기(열기·잇기·닫기)와 `step_back`의 지울 범위(커서 바로 앞 틱을 품은 클립, 클립 시작에서 자름)를 먼저 봐 주면 좋겠다.
- 조합 중에 Step 밖의 경로(마우스로 트랙 바꾸기, 메뉴 실행, 다른 창 활성)는 조합을 곧바로 확정하지 않는다. 대기 시간이 지나면 조합 때의 트랙에 확정된다. 모드 나가기·undo·redo만 즉시 확정한다.
- 키 컨텍스트 경로는 그릴 때의 레이아웃을 쓴다. 설정 창에서 레이아웃을 바꾸면 다음 그리기부터 바뀐다.

## 리뷰 1 대응

| # | 분류 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 지적 | 받음. 스텝 자취(`StepState::trail`)를 두고 `Backspace`·`Shift+Tab`은 커서가 지나온 스텝의 끝이면 그 스텝의 시작으로 간다. 자취 밖이면 현재 길이 한 스텝 | e1f1021 |
| 2 | 지적 | 받음. core `Splice::reach`(노트 없이 덮어야 할 끝)를 더하고, 커서가 든 클립(끝 = 커서 포함)에서 친 쉼표가 끝을 넘으면 마디 단위로 늘린다. 결정 4절에 한 줄 | a555511 |
| 3 | 지적 | 받음. 코드를 결정에 맞췄다: 옥타브 키와 `n`도 화음을 닫는다. 화음 안의 다른 옥타브는 음 뒤 숫자(`Ctrl+c` `5`)로 친다는 문장을 결정 5절에 더했다 | c2b0899 |
| 4 | 제안 | 거절. undo는 문서만 되돌리고 커서는 그대로 두는 것이 Normal 모드와 같은 규칙이다(S3-T3a). Step만 다르게 하면 「undo 뒤 커서」 규칙이 둘이 된다. 빈 자리는 `Shift+Tab`으로 돌아가 다시 친다 | - |
| 5 | 제안 | 거절(후속). 빈 클립을 지우는 것은 클립 편집(S4)의 일이다. undo로는 클립까지 사라진다(시험 20) | - |
| 6 | 제안 | 받음. 조합 표시는 친 그대로(`c-_`, 건반형은 음이름), `n` 뒤 길이 코드도 `n5c_`. 시험 `status_composition_shows_keys_as_typed` | ccadacd |
| 7 | 제안 | 받음. 리뷰의 core 속성 시험(`crates/keyscore-core/tests/review_s3_t4a.rs`)을 커밋해 남긴다 | 39777f1 |

## 리뷰 2 대응

| # | 분류 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 지적 | 받음. 원인(자취가 어느 문서·트랙의 것인지 모름)을 고쳤다: 자취에 (트랙, 문서 revision) 열쇠를 달고, Step 자신의 편집(`apply` 한 번)만 열쇠를 잇는다. undo·redo·Step 밖 편집·트랙 바꿈 뒤에는 버리고, Step을 나가도 버린다 | 2e71606 |
| 2 | 지적 | 받음. 쉼표의 연장 끝(`reach`)을 다음 클립 시작에서 자른다. 커서는 쉼표 길이만큼 간다. 밀린 노트가 다음 클립에 닿는 것은 여전히 거부 | 2329f83 |
| 3 | 지적 | 받음. 결정 1절에 「한 스텝」 규칙과 자취를 버리는 때, 4절에 연장 끝, 5절에 `Shift+Tab`과 `Tab`의 차이를 적었다 | 1519fb8 |
| 4 | 제안 | 거절. `Tab`은 앞으로 갈 스텝이라 지나온 기록이 없다. 현재 길이로 가는 것이 REQ 5.11 「커서만 한 스텝 전진」과 맞다. 결정 5절에 이 차이를 적었다 | 1519fb8 |
| 5 | 제안 | 받음. 「알려진 한계」에 적었다 | 이 커밋 |
