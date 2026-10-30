# Task 보고: S3-T4b vim Step과 MML 호환

- PR: 브랜치 `s3/t4b-vim-step-mml`
- 계획 파일: docs/reports/s3-t4b/plan.md
- 결정: `docs/decisions/2026-10-30-s3-t4b-vim-step-mml.md`(에이전트 해석)
- 계획 대비 달라진 점
  - 설정 창의 두 선택 상자는 레이턴시 보정 버튼 뒤에 둔다. 기존 Tab 순서 시험(`settings_sections_keyboard_reachable`: 미리듣기 길이 → 보정)을 바꾸지 않기 위해서다.
  - `StepStatus`에 `overwrite`를 더했고, `composing`은 MML 접두 대기(`l16_`)도 보인다.
  - 시험 23(`mml_length_*`)은 `step::tests::mml_length_reads_denominators_and_dot` 하나다.

## 구현 요약

- `crates/keyscore-app/src/step.rs`: `mml_length`(분모 `1` `2` `4` `8` `16` `32` `64`와 점), `MmlKind`·`MmlInput`, `StepState`의 `mml`(MML 접두 대기)·`overwrite`(덮어쓰기 Step), `drop_after`(마디 처음까지 지울 때 커서 뒤 자취 버림), 상태줄 `STEP R …`과 `l16_`.
- `crates/keyscore-app/src/step_input.rs`
  - 키 컨텍스트: `Step > Piano|Solfege [> Pending | > Mml] [> Ghost]`.
  - 진입 위치 처리기 5개(`mode::Step`·`StepAfterNote`·`StepAtBarStart`·`StepAtBarEnd`·`StepOverwrite`). FSM이 Step으로 가지 못하면 아무것도 하지 않는다. 이전에는 `window.rs`의 `on_mode` 다섯 줄이었다.
  - 덮어쓰기: 음(`step_write`)·쉼표(`step_rest_room`)·타이·지우기(`step_delete_span`)가 `by = 0`과 `remove`(스텝 범위에서 시작하는 노트, `starting_in`)를 쓴다.
  - `step_back`의 지우기를 `step_delete_span`으로 떼어 `Shift+Backspace`(`step_back_to_bar_start`)와 함께 쓴다. 조합·MML 버리기는 `step_discard`.
  - `HalfRest`(`step_rest_of(half)`), `OctaveOnce`(조합 음만, `'` `,` 표시, 대기 다시 잼), `TieLegato`(확정 뒤 `plan_legato`로 직전 스텝), `PreviewFromBar`(확정 → Normal → `bar_back` 마디부터 `play_from`).
  - MML: `step_mml_open`(조합·화음 먼저 확정), `step_mml_digit`(`o`는 한 자리로 끝, `l`은 쌓음), `step_mml_dot`, `step_settle_mml`(`step_commit` 맨 앞에서 부름. 그래서 다른 모든 Step 동작·모드 나가기·undo가 먼저 확정한다).
- `crates/keyscore-app/src/window.rs`: Step 진입 `on_mode` 다섯 줄을 지우고, Step을 나갈 때 `overwrite = false`.
- `crates/keyscore-app/src/transport.rs`: `play_from`을 `pub(crate)`로.
- `crates/keyscore-app/src/actions.rs`·`commands/table.rs`: 새 Action `step::MmlOctave`·`MmlLength`·`MmlDot`·`MmlDigit(n)`, `settings::SetStepDefaultLength(code)`·`SetOctaveWait(ms)`(모두 팔레트 자리 있음). 카탈로그 `docs/reports/s2-t2/action-catalog.md`에 줄을 더했다.
- `crates/keyscore-app/src/settings_window.rs`·`settings_tabs.rs`: 두 설정 Action의 앱 전역 처리기(범위 밖 값 무시), 설정 > 입력의 선택 상자 둘(목록 밖 값은 덧붙여 보임).
- `assets/keymaps/default.json`: `Step > Solfege`에 `o` `l` `r`, 새 컨텍스트 `Step > Solfege > Mml`에 `0`–`9`·`.`. vim 프리셋의 `{` `}` `shift-space` `shift-backspace` `shift-enter`와 `i` `a` `I` `A` `R`은 S2-T2에 이미 있었다.

## 테스트 결과

~~~
keyscore-app --test step_vim_mml: 22 passed
keyscore-app step::tests(단위): 5 passed
관련 기존 시험(부분 실행): step_input 28, review_s3_t4a 11, review_s3_t4a_r2 11, review_s3_t4a_r3 7,
settings_tabs 8, commands 12, keymap_engine 11, modes_ui 8, command_line 7, keymap_overlay 4, mode_fsm 7,
count_non_key_paths 2 — 모두 통과
전체: 아래 「자동 게이트」
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `vim_i_enters_step_at_cursor` | 통과(구현 전에도 통과: 기존 `mode::Step` 동작) |
| 2 | `vim_a_enters_after_cursor_note_or_at_cursor` | 통과 |
| 3 | `vim_shift_i_and_shift_a_enter_at_bar_start_and_end` | 통과 |
| 4 | `vim_shift_r_overwrites_without_pushing` | 통과 |
| 5 | `overwrite_rest_tie_and_backspace_do_not_shift` | 통과 |
| 6 | `overwrite_chord_removes_range_once` | 통과 |
| 7 | `overwrite_ends_when_leaving_step_and_plain_entry_inserts` | 통과 |
| 8 | `overwrite_extends_clip_at_end` | 통과(구현 전에도 통과: 클립 끝 뒤에는 밀 노트가 없어 삽입과 같다) |
| 9 | `shift_space_rests_half_length_once` | 통과 |
| 10 | `shift_backspace_deletes_to_bar_start_and_pulls` | 통과 |
| 11 | `braces_shift_composing_note_octave_once` | 통과 |
| 12 | `tie_legato_from_palette_connects_last_step` | 통과 |
| 13 | `preview_from_bar_leaves_step_and_plays_from_bar_start` | 통과(CoreMIDI 엔진) |
| 14 | `mml_octave_sets_sticky_octave` | 통과 |
| 15 | `mml_length_reads_denominators_and_dot` | 통과 |
| 16 | `mml_rest_is_space` | 통과 |
| 17 | `mml_wait_shows_status_and_backspace_discards` | 통과 |
| 18 | `req_5_1_twinkle_in_mml_commands` | 통과 |
| 19 | `mml_keys_only_in_solfege_layout` | 통과(구현 전에도 통과: 건반형 `o` `l`은 원래 음 키) |
| 20 | `settings_input_has_step_length_and_wait_choices` | 통과 |
| 21 | `settings_actions_set_step_default_length_and_wait` | 통과 |
| 22 | `step_shift_keys_have_no_conflicts_in_vim_preset` | 통과 |
| 23 | `step::tests::mml_length_reads_denominators_and_dot` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [ ] test(전체, `--no-fail-fast`): 실행 중. 결과를 이 줄에 적는다
- [ ] deny, `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`: 전체 시험과 함께 실행 중
- 단계별 추가 게이트: 이 Task는 피아노롤 벤치·프레임 경로를 건드리지 않는다

## 알려진 한계

- 덮어쓰기에서 스텝 범위 앞에서 시작해 범위로 걸쳐 들어오는 노트는 그대로 둔다(삽입 규칙과 같다). 같은 음높이면 새 노트와 겹친다.
- 덮어쓰기의 `Backspace`는 vim `R`처럼 원래 노트를 되살리지 않는다(되살리기는 undo).
- `Shift+Backspace`는 `Backspace`처럼 커서 바로 앞 틱을 품은 클립에서만 지운다. 마디 안에 클립 경계가 있으면 앞 클립의 노트는 남는다.
- MML `<` `>` `&`와 쉼표 뒤 길이 숫자(`r4`)는 받지 않는다(결정 4절).
- REQ 5.1 예 `g2`는 텍스트 문법(뒤 숫자 = 길이)이다. 키 입력에서는 뒤 숫자가 옥타브라(REQ 2절) `l2 g`로 친다. REQ 예 문구는 그대로 두었다.
- `TieLegato`는 B25 결정대로 vim 키가 없고 팔레트로만 부른다.

## 리뷰어에게

- 결정 문서는 모두 에이전트 해석이다. 특히 `Shift+Enter`의 「입력을 마치고」를 Step을 나가는 것으로 읽은 것, 덮어쓰기 범위를 「시작이 [자리, 자리 + 스텝)인 노트」로 정한 것을 봐 주면 좋겠다.
- `step_settle_mml`을 `step_commit` 맨 앞에서 부르는 구조가 모든 경로(대기 타이머, 다른 음 키, `Tab`, 모드 나가기, undo)에서 MML 대기를 먼저 확정하는지.
- 진입 처리기가 `mode::Step` 등을 `step_handlers`로 옮겼다. 키맵·메뉴·팔레트에서 오는 경로가 그대로 Step으로 들어가는지(다른 모드, 배치 뷰, AI 패널 중).
- 덮어쓰기와 자취(`trail`)·직전 스텝(`last`)의 상호작용: 덮어쓰기에서 `Backspace`·타이 뒤의 자취가 맞는지.
