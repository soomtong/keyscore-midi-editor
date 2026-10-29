# Task 계획: S3-T4a Step 입력 핵심

- 브랜치: `s3/t4a-step-input`
- 관련 REQ: 5.1, 5.6(문법·길이 코드 표·입력 예 표), 5.10(클립 밖 입력·연장), 5.11(Step 줄의 `Tab`/`Shift+Tab`, 화음 줄), P3(두 레이아웃), KM-7, PRE-1(Step 음 키 미리듣기), A13(모드를 나가면 조합 확정)
- 선행 Task: S3-T3b(길이 코드 `length_ticks`, 음 키 표 `pianoroll::keys`), S3-T3e
- 관련 결정: `docs/decisions/2026-10-29-s3-t4-step-input-details.md`(이 Task의 규칙. 1·2는 사람 결정), `2026-10-17-keymap-conflicts.md`(A13, B25, B27), `2026-10-17-mode-entry-view.md`
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

Step 모드에서 키보드만으로 노트를 찍는다. 건반형·계이름형 레이아웃 모두 5.6 입력 예 표의 모든 행이 표대로 동작한다. 규칙은 결정 문서 1–6이 전부이고 아래 시험이 고정한다. 요약:

1. 키 컨텍스트: Step 모드의 경로는 `Step > Piano` 또는 `Step > Solfege`(설정 `input.key_layout`), 조합 중이면 안쪽에 `Pending`, 고스트는 맨 안쪽 `Ghost`. `#` `+` `-`(변화표)는 `Step > Solfege > Pending`으로 옮긴다(조합이 없을 때 `#`은 점4분음표).
2. 음 키(`step::Note`)는 조합을 연다. 옥타브 숫자(`step::SetOctave`), `Enter`(`step::Commit`), 대기 시간(설정 `input.octave_wait_ms`, 기본 300 ms) 만료, 다음 음 키, 다른 Step 동작, 모드 나가기, undo·redo가 조합을 확정한다. 확정 = 노트 쓰기 + 커서 전진(화음 제외).
3. 쓰기는 삽입이다: 커서가 든 클립에서 시작 ≥ 커서인 노트를 입력 길이만큼 밀고 노트를 넣는다. 클립이 없으면 마디 단위 새 클립. 끝을 넘으면 마디 단위 연장. 확정 하나가 undo 한 단위.
4. 길이: `1`–`7`(스티키), 기호 키 `!`…`&`(점음표, 스티키), `0`(설정 `input.step_default_length`, 기본 4분), `n`(셋잇단 2/3, 다음 하나), `Shift+음`(절반, 그 음 하나).
5. `Space` 쉼표(밀고 전진, 클립을 만들지 않음), `Backspace` 한 스텝 뒤로 지우고 당김(조합 중이면 조합만 버림), `Tab` 확정·전진(없으면 커서만 전진), `Shift+Tab` 한 스텝 뒤로(편집 없음), `Ctrl+음` 화음, `~` 타이, `[` `]`·`Z` `X`(건반형) 옥타브.
6. 상태줄 모드 표시 `STEP ♩ oct4`, 조합 중 `STEP 1/16 oct4 5c_`.
7. 미리듣기: 음 키 즉시 지금 옥타브로, 확정 때 음높이가 바뀌었으면 바뀐 음으로 한 번 더(PRE-1, 기존 `sound_note` 경로).

단계 파일과 다른 점: S3-T4를 T4a·T4b·T4c로 나눴다(결정 7절). 미리듣기 가벼운 경로와 배치 뷰 표시는 T4c, vim Step 조합·MML 호환·설정 창 입력란은 T4b다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-core/src/transform.rs
/// 클립 하나의 「자리 만들고 넣기」. `at`(클립 기준) 이상에서 시작하는 노트를 `by`만큼 옮기고(음수면 당김),
/// `remove`를 지우고, `replace` 짝을 바꾸고, `insert`를 넣는다. 늘려야 하면 마디 단위(`insert_notes` 규칙).
pub struct Splice { pub at: u64, pub by: i64, pub remove: Vec<Note>, pub replace: Vec<(Note, Note)>, pub insert: Vec<Note> }
pub fn splice(project: &Project, track: TrackId, clip: ClipId, splice: &Splice) -> Result<Vec<Edit>, ModelError>;

// crates/keyscore-app/src/step.rs (GPUI 없는 상태)
pub struct StepLength { pub code: LengthCode, pub dotted: bool }
pub struct StepStatus { pub length: StepLength, pub octave: u8, pub composing: Option<String>, pub chord_open: bool }
pub fn length_label(length: StepLength) -> String;            // "♩", "1/16.", …
pub fn step_ticks(length: StepLength, triplet: bool, half: bool) -> u32;

// crates/keyscore-app/src/step_input.rs (창 처리기)
impl ProjectWindow {
    pub fn step_status(&self) -> StepStatus;
}

// crates/keyscore-app/src/settings.rs
pub struct InputSettings { pub key_layout: KeyLayout, pub step_default_length: LengthCode, pub octave_wait_ms: u32 }
~~~

## 테스트 목록

UI 시험 파일 `crates/keyscore-app/tests/step_input.rs`(헤드리스 창, 키 입력, `advance_clock`).

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `req_5_6_rows_solfege_match_table` | UI | 5.6 입력 예 표 12행을 차례로(앞 행의 상태를 이어 받음) 계이름형에서. 행마다 새 노트(시작·길이·음높이)와 길이·옥타브 상태 |
| 2 | `req_5_6_rows_piano_match_table` | UI | 같은 표를 건반형 키(`c`→`a`, `f#`→`t` 등)로 |
| 3 | `octave_wait_commits_after_timeout_with_fake_clock` | UI | 음 키 뒤 299 ms는 노트 없음·`Pending`, 300 ms에 현재 옥타브로 확정 |
| 4 | `octave_wait_setting_changes_timeout` | UI | 설정 1000 ms면 999 ms까지 대기 |
| 5 | `enter_and_next_note_commit_immediately` | UI | `Enter`, 다음 음 키가 곧바로 확정 |
| 6 | `shift_note_is_half_once_not_sticky` | UI | `Shift+음`은 절반 하나, 다음 음은 원래 길이 |
| 7 | `length_codes_and_dotted_are_sticky_zero_resets_to_setting` | UI | `1`–`7`, `!`…`&`, `0`이 설정 기본 길이(8분으로 바꿔 확인) |
| 8 | `triplet_prefix_applies_to_next_note_or_rest_only` | UI | `n`은 다음 하나만 2/3 |
| 9 | `space_rests_and_does_not_create_clip` | UI | 틈에서 `Space`는 커서만 전진, `3 space`는 4분쉼표 |
| 10 | `backspace_deletes_step_and_pulls_later_notes` | UI | 지우고 당김, 클립 길이 그대로 |
| 11 | `backspace_while_composing_discards_composition` | UI | 조합만 버림 |
| 12 | `tab_confirms_and_advances_shift_tab_steps_back` | UI | 조합 있으면 확정, 없으면 커서만 한 스텝, `Shift+Tab` 편집 없이 뒤로 |
| 13 | `ctrl_note_builds_chord_plain_note_or_tab_advances` | UI | 같은 자리 겹침, 첫 음만 밂, 닫으면 스텝 길이만큼 전진 |
| 14 | `tie_extends_previous_step_and_advances` | UI | 직전 음·화음 연장, 뒤 노트 밂, 직전 스텝이 아니면 거부 |
| 15 | `octave_keys_change_sticky_octave_and_stop_at_bounds` | UI | `[` `]`, 건반형 `z` `x`, 0·9에서 멈춤, 127 넘는 음은 거부 |
| 16 | `piano_octave_digit_sets_row_base` | UI | oct4 `k 3` → C4, 이후 `a`는 C3(사람 결정 2) |
| 17 | `solfege_accidentals_only_while_composing` | UI | `c#` `c+` `c-` `c##`, 조합 없을 때 `#`은 점4분 |
| 18 | `outside_clip_creates_bar_clip_and_extends_at_end` | UI | 틈 → 마디 클립, 끝 = 커서면 그 클립을 연장 |
| 19 | `insert_pushes_later_notes_and_rejects_next_clip_overlap` | UI | 밀기, 늘리기, 다음 클립에 닿으면 거부·문서 그대로 |
| 20 | `each_commit_is_one_undo_unit` | UI | 음 셋 → undo 셋. 조합 중 `cmd-z`는 그 음을 없앤다 |
| 21 | `leaving_step_commits_composition` | UI | `escape`, `ctrl-tab`이 현재 옥타브로 확정(A13) |
| 22 | `status_shows_length_octave_and_composition` | UI | `STEP ♩ oct4`, `STEP 1/16 oct4 5c_`, `STEP ♩. oct4` |
| 23 | `step_contexts_have_no_note_key_conflicts` | UI | 두 레이아웃·두 프리셋의 Step 경로에서 `n` `z` `x` `0`–`9` `[` `]` `space` `tab` `enter` `backspace` `~`가 음 Action이 아니고, 음 키마다 실효 Action이 하나(리뷰 초점) |
| 24 | `vim_preset_uses_same_step_grammar` | UI | vim 프리셋에서 표의 몇 행 |
| 25 | `step_in_arrangement_view_writes_at_cursor` | UI | 배치 뷰 그대로, 커서 자리에 클립·노트 |
| 26 | `first_step_edit_clears_note_selection` | UI | 선택이 있으면 첫 확정에서 비움 |
| 27 | `splice_*` | 단위 | core `splice`: 밀기·당기기·지우기·바꾸기·넣기, 마디 연장, 다음 클립 거부, 꼬리 노트 |
| 28 | `splice_applies_and_keeps_invariants` | 속성 | 임의 클립·`at`·`by`에서 결과가 적용되고 `at` 앞 노트는 그대로, 뒤 노트는 `by`만큼 |
| 29 | `input_settings_*` | 단위 | `step_default_length`·`octave_wait_ms` 읽기·쓰기·범위 밖 기본값 |
| 30 | `step_ticks_*`, `length_label_*` | 단위 | 길이 계산(점·셋잇단·절반 조합 정수), 상태줄 표기 |

## 불변식

- 모델 불변식(겹침 없음, 노트는 클립 안)은 core `splice`·`clip_for_notes`가 지킨다. 실패하면 문서는 그대로이고 상태줄에 알린다. 커서·Step 상태도 실패한 확정에서는 바뀌지 않는다.
- 조합이 있으면 키 컨텍스트에 `Pending`이 있다. 조합과 대기 시간 작업은 같은 세대 번호로 맞춘다(낡은 타이머는 아무 일도 하지 않는다).
- 확정 하나 = `Document::apply` 하나 = undo 한 단위.
- Step 상태는 창 하나의 것이다. 다른 창과 공유하지 않는다.

## 범위 밖

- vim Step 진입 위치(`i` `a` `I` `A`)와 `R`, vim Step 표의 `Shift` 조합, 계이름 MML 호환(`o5` `l8` `r`), 설정 창 입력란(S3-T4b).
- 미리듣기 가벼운 경로(스냅샷 재사용·교체 세대), 배치 뷰의 Step 커서·클립 생성 표시(S3-T4c).
- 드럼 트랙 라벨 입력과 킷 페이지(S6-T4). AI 고스트(S7).
- 건반 뷰(S3-T5).

## 리스크·결정 요청

- REQ에 없는 세부는 결정 `2026-10-29-s3-t4-step-input-details.md`에 적었다. 삽입(밀기)과 건반 옥타브 기준은 사람에게 물어 정했다.
- 5.6 표의 공백은 키가 아니라 구분으로 읽는다(`d e f`는 키 셋). `2 Space`·`5 Enter`의 `Space`·`Enter`만 키다. `4na 4nb 4nc`처럼 음 뒤에 숫자가 이어지는 행은 300 ms 안이면 옥타브로 읽히므로, 시험은 행 안의 토큰 사이에 대기 시간을 흘려 표의 뜻(세 음 모두 8분 셋잇단)을 맞춘다. 이 점은 보고서에 적는다.
- GPUI 대기 입력(다중 키)과 조합 대기는 다른 것이다. Step 바인딩은 모두 한 키라 GPUI 대기는 생기지 않는다.
