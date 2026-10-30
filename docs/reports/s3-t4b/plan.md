# Task 계획: S3-T4b vim Step과 MML 호환

- 브랜치: `s3/t4b-vim-step-mml`
- 관련 REQ: 5.1(vim `i`, MML 호환 줄), 5.6, 5.8.1(글자 키 `i` `a` `r`의 대문자, Step 모드 표), 5.11, PRE-1
- 선행 Task: S3-T4a(Step 상태 `step.rs`, 처리기 `step_input.rs`, core `transform::splice`)
- 관련 결정: `docs/decisions/2026-10-30-s3-t4b-vim-step-mml.md`(이 Task의 규칙, 에이전트 해석), `2026-10-29-s3-t4-step-input-details.md`(Step 규칙 1–6), `2026-10-17-keymap-conflicts.md`(B25: `TieLegato` 키 비움, B27: `b`는 음만)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

결정 문서 1–5가 전부이고 아래 시험이 고정한다. 요약:

1. vim `i` `a` `I` `A` `R`이 결정 1절의 자리에서 Step으로 들어간다. `R`은 덮어쓰기 Step이다.
2. 덮어쓰기 Step은 밀거나 당기지 않고 스텝 범위에서 시작하는 노트를 지운다(음·쉼표·타이·`Backspace`·`Shift+Backspace`).
3. vim Step 표의 `Shift` 조합 `HalfRest`·`BackToBarStart`·`PreviewFromBar`·`OctaveOnce`가 키로, `TieLegato`가 팔레트로 동작한다.
4. 계이름 레이아웃에서 `o<숫자>`·`l<숫자>[.]`·`r`을 받는다(컨텍스트 `Step > Solfege > Mml`).
5. 설정 > 입력에 Step 기본 길이·옥타브 대기 시간 선택 상자가 있고, 팔레트 Action 둘이 같은 값을 바꾼다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/actions.rs
pub mod step { /* 새 Action */ MmlOctave, MmlLength, MmlDot, struct MmlDigit(pub Int) }
pub mod settings { struct SetStepDefaultLength(pub LengthCode), struct SetOctaveWait(pub Int) }

// crates/keyscore-app/src/step.rs
pub struct StepStatus { /* + */ pub overwrite: bool, /* composing은 MML 대기도 보인다 */ }
pub(crate) enum MmlKind { Octave, Length }
pub(crate) struct MmlInput { kind: MmlKind, digits: String, dotted: bool }
pub fn mml_length(digits: &str, dotted: bool) -> Option<StepLength>;  // "16" → 16분
~~~

## 테스트 목록

UI 시험 파일 `crates/keyscore-app/tests/step_vim_mml.rs`(헤드리스 창). 재생 시험(`PreviewFromBar`)만 CoreMIDI 엔진을 띄운다.

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `vim_i_enters_step_at_cursor` | UI | `i`: 커서 그대로, 삽입 |
| 2 | `vim_a_enters_after_cursor_note_or_at_cursor` | UI | 노트 위 `a` → 노트 끝, 빈 칸 `a` → 그대로 |
| 3 | `vim_shift_i_and_shift_a_enter_at_bar_start_and_end` | UI | 마디 중간에서 `I`·`A` |
| 4 | `vim_shift_r_overwrites_without_pushing` | UI | `R` 뒤 음: 범위 노트 교체, 뒤 노트 그대로, 상태줄 `STEP R` |
| 5 | `overwrite_rest_tie_and_backspace_do_not_shift` | UI | 쉼표가 지움, 타이가 지움·늘림, `Backspace` 지우기만 |
| 6 | `overwrite_chord_removes_range_once` | UI | `Ctrl+음` 둘 + 음 |
| 7 | `overwrite_ends_when_leaving_step_and_plain_entry_inserts` | UI | `Esc` 뒤 `i`는 삽입 |
| 8 | `overwrite_extends_clip_at_end` | UI | 클립 끝에서 덮어쓰기 음이 클립을 늘림 |
| 9 | `shift_space_rests_half_length_once` | UI | 절반 쉼표, 스티키 아님, `n` 셋잇단과 겹침 |
| 10 | `shift_backspace_deletes_to_bar_start_and_pulls` | UI | 마디 중간 → 마디 시작, 마디 시작 → 앞 마디 시작, 뒤 노트 당김, 조합 버림 |
| 11 | `braces_shift_composing_note_octave_once` | UI | `c }` → C5 하나, 다음 음은 oct4, 조합 표시 `c'_`, 조합 없으면 알림 |
| 12 | `tie_legato_from_palette_connects_last_step` | UI | 뒤에 틈이 있는 직전 음을 다음 노트에 붙임, 직전 스텝 아니면 알림 |
| 13 | `preview_from_bar_leaves_step_and_plays_from_bar_start` | UI·MIDI | Normal로 나가고 재생 헤드가 마디 시작 이후·커서 앞 |
| 14 | `mml_octave_sets_sticky_octave` | UI | `o5 c` → C5, 이후 유지, `c o3`의 `c`는 앞 옥타브 |
| 15 | `mml_length_reads_denominators_and_dot` | UI | `l8 c`, `l16 c`, `l4. c`, `l1` 뒤 `c`, 잘못된 `l3`은 알림 |
| 16 | `mml_rest_is_space` | UI | `r` = 쉼표 |
| 17 | `mml_wait_shows_status_and_backspace_discards` | UI | 상태줄 `l16_`, 컨텍스트 `Mml`, `Backspace` 버림, `Enter` 확정 |
| 18 | `req_5_1_twinkle_in_mml_commands` | UI | REQ 5.1 예의 뜻을 MML 명령으로: `o4 l4 c c g g a a l2 g l4 f f e e d d l2 c` → 두 마디 14음. 예의 `g2`는 텍스트 문법(뒤 숫자 = 길이)이라 키 입력에서는 `l2 g`로 친다 |
| 19 | `mml_keys_only_in_solfege_layout` | UI | 건반형 `o` `l`은 음 키 |
| 20 | `settings_input_has_step_length_and_wait_choices` | UI | 설정 창 입력 구역에서 Tab으로 두 상자에 가서 고르면 설정이 바뀜 |
| 21 | `settings_actions_set_step_default_length_and_wait` | UI | 팔레트 Action 둘, 범위 밖 값 무시, `0`이 새 기본 길이 |
| 22 | `step_shift_keys_have_no_conflicts_in_vim_preset` | UI | vim 프리셋 두 레이아웃의 Step 경로에서 `{` `}` `shift-space` `shift-backspace` `shift-enter`가 표의 Action, `o` `l` `r`과 MML 숫자는 계이름형만 |
| 23 | `mml_length_*` | 단위 | 분모·점 표, 범위 밖 |

## 불변식

- 모델 불변식은 core `splice`·`clip_for_notes`가 지킨다. 덮어쓰기도 같은 `Splice`(`by = 0`, `remove`)로 쓴다.
- 조합(`composing`)과 MML 대기(`mml`)는 동시에 있지 않다. 키 컨텍스트는 `Pending` 또는 `Mml` 하나다.
- 확정 하나 = `Document::apply` 하나 = undo 한 단위(덮어쓰기 포함).
- 덮어쓰기 여부는 Step 안에서만 참이다.

## 범위 밖

- 미리듣기 가벼운 경로, 배치 뷰 Step 커서·클립 생성 표시(S3-T4c).
- MML `<` `>` `&`, 쉼표 뒤 길이 숫자(결정 4절).
- 드럼 트랙(S6-T4), AI 고스트(S7), 건반 뷰(S3-T5).

## 리스크·결정 요청

- REQ에 없는 세부는 결정 `2026-10-30-s3-t4b-vim-step-mml.md`에 에이전트 해석으로 적었다. 특히 `Shift+Enter`의 「입력을 마치고」를 Step을 나가는 것으로 읽었다. 사람이 다르게 보면 고친다.
- REQ 5.1 예 `g2`는 MML 문법(뒤 숫자 = 길이)이고 키 입력에서는 뒤 숫자가 옥타브다(REQ 2절). 키 입력 문법을 바꾸지 않으므로 시험 18은 예의 뜻(2분음표 G)을 키 문법(`2 g`)으로 친다. REQ 예 문구는 바꾸지 않고 보고서에 적는다.
