# Task 계획: S3-T3b 노트 편집

- 브랜치: `s3/t3b-note-edit`
- 관련 REQ: 5.8 선택·편집 표(`/`, `Cmd+J`, `1`–`7`·`Shift+1`–`7`·`0`, `e`, `q`, `~`, `.`), 5.7(「길이 직접 지정」, 되돌리기 묶음), 5.6(길이 코드 표, 생략한 자리는 커서의 현재 값), 5.8.1(vim `s` `r` `J` `K` `q` `~` `.`, `g` 접두), 6.2 EDT-2, 7.3 KM-7, P3(두 레이아웃)
- 선행 Task: S3-T3a(대상 고르기, 편집 뒤 선택, undo 뒤 선택)
- 관련 스파이크: 없음
- 결정: `docs/decisions/2026-10-26-s3-t3b-note-edit-details.md`(합치기·레가토 토글·퀀타이즈 설정·`e` 옥타브, 2026-10-26 사람 결정), `docs/decisions/2026-10-17-keymap-conflicts.md` A1·A9·B25
- 나눔: 단계 파일 S3-T3 「나눔(2026-10-26)」의 T3b. `Shift+q` 설정 시트는 S3-T3e로 뗀다(아래 「범위 밖」).

## 끝의 정의 (내 말로)

피아노롤(`Normal > PianoRoll`)에서 노트를 커서 자리에서 자르고, 같은 음높이끼리 합치고, 길이 코드로 길이를 바로 정하고, 음 키 하나로 음높이를 바꾸고, 앱 설정의 퀀타이즈를 적용하고, 레가토를 켜고 끄고, 마지막 편집을 `.`으로 되풀이할 수 있다. 키맵은 A1(vim `J` `K`)·A9(vim `g 1`…`g 7`, `g 0`)·B25(`shift-N` 바인딩 삭제)를 반영하고 REQ 5.8.1·KM-7 문구를 고친다.

용어는 S3-T3a 계획과 같다. 「대상」은 선택, 없으면 커서 위 노트 하나(`select::note_under`). 「현재 클립」은 현재 트랙에서 커서 틱을 품은 클립. 같은 값의 노트가 클립에 여럿이면 그 값의 노트 전부가 대상이다(`pianoroll::edit::targets`).

1. 공통 규칙(아래 모든 동작)
   - 배치 뷰면 아무것도 바꾸지 않고 알린다(T3a `edit_allowed`와 같다). 입력란에 포커스가 있으면 아무것도 하지 않는다(입력란이 받을 같은 동작이 없다).
   - 선택이 있고 그 (트랙, 클립)이 현재 클립이 아니면 거부하고 알린다(T3a·S3-T2와 같다).
   - 계산은 순수 함수(`pianoroll::edit`)이고 전부 아니면 전무다. 실패·변화 없음은 상태줄 한 줄이고 문서는 그대로다(undo 기록도 없다).
   - 적용은 undo 한 단위다. 되돌리기 묶음(5.7)은 쓰지 않는다. 같은 키를 거듭 눌러도 결과가 같거나(길이 코드), 누를 때마다 다른 대상을 고치기 때문이다(분할).
   - 편집 뒤 선택: 선택에서 시작했으면 선택은 「대상 가운데 그대로 남은 노트 + 이 편집으로 새로 나타난 노트」다(앞뒤 클립 노트의 다중집합 차, T3a `appeared`와 같은 계산). 기준 칸은 노트와 함께 옮기지 않고 비운다(다음 `Shift+이동`은 커서에서 새로 시작한다). 커서 노트에서 시작했으면 선택은 생기지 않고, 커서는 S3-T2 `follow_cursor_note` 규칙으로 바뀐 노트를 따라간다(시작이 옮겨지면 같이, 음높이가 바뀌면 같이, 노트 밖에 남으면 노트 안으로).
2. 분할 `edit::Split`(`/`, vim `s`): 커서 틱 `c`(절대)가 대상 노트의 시작과 끝 사이에 있으면(`시작 < c < 끝`) 그 노트를 `[시작, c)`와 `[c, 끝)` 둘로 나눈다. 두 노트는 원래 노트의 다른 값(음높이·벨로시티·피치 오프셋·모르는 키)을 그대로 갖고, 가사는 앞 노트에만 남는다(음절은 한 번 불린다). 커서 틱이 시작이나 끝이면, 또는 커서 틱을 가로지르는 대상이 없으면 「분할: 커서 자리를 가로지르는 노트가 없다」. 선택이 있으면 선택 가운데 커서 틱을 가로지르는 노트를 모두(음높이 무관) 나눈다.
   - 숨김 꼬리 노트(끝이 클립 길이를 넘음)는 원래 노트를 뒤쪽 `[c, 끝)`으로 바꾸고(`ReplaceNotes`, 꼬리 그대로) 앞쪽을 새로 넣는다. 그래서 꼬리가 새로 생기지 않는다(core 불변식, 결정 `2026-10-12-clip-split-and-velocity.md` Q1). 일반 노트도 같은 방식으로 만든다.
3. 합치기 `edit::Join`(`cmd-j`, vim `shift-j`, A1): 결정 1-A. 대상을 음높이마다 묶어 둘 이상인 묶음마다 노트 하나로: 시작 = 묶음의 가장 이른 시작, 끝 = 묶음의 가장 늦은 끝, 나머지 값은 가장 이른 노트(같은 시작이면 `Note` 순서의 앞)의 것. 길이가 `u32`를 넘으면 거부. 묶음에 숨김 꼬리 노트가 있으면 그 꼬리 노트를 합친 노트로 바꾸고(꼬리 그대로) 나머지를 지운다. 없으면 가장 이른 노트를 바꾸고 나머지를 지운다. 클립은 늘리지 않는다(합친 끝은 이미 있던 끝이다). 둘 이상인 묶음이 없으면(선택 없이 커서 노트 하나 포함) 「합치기: 같은 음높이 노트를 둘 이상 고른다」.
4. 길이 코드 `note::SetLength(code)`(`1`–`7`, vim `g 1`…`g 7`, A9)·`note::SetDottedLength(code)`(`!` `@` `#` `$` `%` `^` `&`, B25)·`note::ResetLength`(`0`, vim `g 0`): 대상의 길이를 그 길이로 바꾼다(시작은 그대로, 5.7 「길이 직접 지정」, `Enter` 불필요). 길이는 5.6 표: 온 3840, 2분 1920, 4분 960, 8분 480, 16분 240, 32분 120, 64분 60(PPQ 960), 점음표는 그 3/2. `0`은 기본 길이 4분음표(960). 기본 길이 설정은 S3-T4가 Step과 함께 만든다(그때 이 값을 그 설정으로 바꾼다).
   - 클립 경계는 S3-T2 `Alt+Shift+l`과 같은 `map_notes` 규칙이다: 끝이 클립을 넘으면 클립을 마디 단위로 늘리고 다음 클립과 겹치면 거부, 숨김 꼬리 노트는 시작만 클립 안이면 된다.
   - 이미 그 길이인 노트는 뺀다. 바뀌는 노트가 없으면 「길이: 바뀌는 노트가 없다」.
   - Step 모드의 현재 길이(상태줄 `STEP ♩`)는 바꾸지 않는다(5.7은 선택 노트의 길이를 말한다. Step 길이는 S3-T4).
5. 음높이 바꾸기 `edit::SetPitch`(`e`, vim `r`): 대상은 커서 노트 하나다(REQ 표가 「커서 노트」라고 적는다). 선택은 보지도 바꾸지도 않는다. 커서 위에 노트가 없으면 알리고 끝.
   - 누르면 「음 키 대기」가 된다. 상태줄에 「음높이: 음 키를 누른다(Esc 취소)」. 대기 중의 다음 키 입력 하나는 키맵 디스패치 전에 창이 가로채 먹는다(`intercept_keystrokes`, 다른 창의 키는 보지 않는다).
   - 수식키 없는 음 키면: 레이아웃(설정 `input.key_layout`)에 따라 옥타브 안 반음 자리를 구하고, 커서 음높이의 옥타브 C(`커서 음높이 − 커서 음높이 % 12`)에 더한 음높이로 커서 노트를 바꾼다(결정 4-A). 건반형(P3): `a s d f g h j k l` = 0 2 4 5 7 9 11 12 14, `w e t y u o p` = 1 3 6 8 10 13 15. 계이름형: `c d e f g a b` = 0 2 4 5 7 9 11(KM-7 문자 기준: 키의 입력 글자 `key_char`, 없으면 `key`). 결과가 0–127 밖이면 거부. 같은 음높이면 「바뀌는 노트가 없다」. 바꾸기는 `map_notes`(다른 값 그대로)이고 커서 음높이가 따라간다.
   - 그 밖의 키(`Esc`, 수식키 조합, 음 키가 아닌 글자)는 대기만 풀고 「음높이 바꾸기를 취소했다」. 그 키는 다른 동작을 하지 않는다(먹는다).
   - 대기는 창마다 하나이고, 다른 동작으로 커서·문서가 바뀌어도 키를 받을 때 커서 노트를 다시 찾는다. 피아노롤을 떠나면(뷰 전환·모드 전환) 대기를 푼다.
6. 퀀타이즈 `edit::Quantize`(`q`, vim `q`): 결정 3-A. 앱 설정 `quantize`의 그리드·셋잇단·강도·스윙으로 대상에 core `transform::quantize`(S0-T7, 시작만 옮김, 마디 기준 그리드, 끝을 넘으면 마디 단위 연장)를 적용한다. 바뀌는 노트가 없으면 「퀀타이즈: 바뀌는 노트가 없다」.
   - 설정 `quantize {grid: "1/4"|"1/8"|"1/16"|"1/32"|"1/64", triplet: bool, strength: 0–100, swing: 0–75}`, 기본 `1/16`·끔·100·0. 필드마다 검사해 잘못되면 그 필드만 기본값과 경고, 모르는 키 보존(다른 설정 객체와 같다).
   - `edit::QuantizeSettings`(`shift-q`)의 시트는 S3-T3e. 이 Task에서 `shift-q`는 처리기가 없다(지금과 같다).
7. 레가토 `edit::Legato`(`~`, vim `~`): 결정 2-A. 대상마다 같은 클립에서 시작이 그 노트보다 늦은 노트(모든 노트) 가운데 가장 이른 시작 `s'`를 찾아 길이를 `s' − 시작`으로 바꾼다(늘리기·줄이기 모두). 없으면 그대로. 바뀌는 노트가 없으면 「레가토: 바뀌는 노트가 없다」(되돌릴 기억이 있으면 아래 토글).
   - 토글: 창이 마지막 레가토의 (문서 판 `revision`, 트랙, 클립, 원래 → 바뀐 노트 짝)을 기억한다. 다음 `~`에서 문서 판이 같고 대상의 값 집합이 기억한 「바뀐 노트」 집합과 같으면 바뀐 노트를 원래 노트로 되돌린다(새 편집, undo 한 단위, 기억은 지운다). 그 밖에는 붙이기를 하고 기억을 새로 쓴다. undo·다른 편집으로 판이 바뀌면 기억은 쓰지 않는다.
   - 클립 경계: 바뀐 끝은 다음 노트 시작이라 클립 안이다. 숨김 꼬리 노트는 레가토 대상에서 뺀다(리뷰 1 지적 1: 꼬리를 줄이면 공개 편집으로 다시 꼬리를 만들 수 없어 토글 되돌리기가 원래대로 돌리지 못한다. 꼬리는 분할로만 생긴다).
8. 마지막 편집 반복 `edit::RepeatLast`(`.`): 창이 마지막으로 성공한 편집 동작을 기억하고 `.`이 같은 동작을 지금 대상에 다시 한다(vim `.`).
   - 기억하는 동작: 삭제(`Delete` `DeleteBefore` `DeleteToBarEnd`), `Cut`, 붙여넣기 셋, `Duplicate`, `Repeat`, `Split`, `Join`, 길이 코드 셋, `Quantize`, `Legato`, 5.7 `Alt` 조작 10개, `SetPitch`(받은 음 키의 반음 자리까지. `.`은 대기 없이 그 자리를 지금 커서 노트의 옥타브에 적용).
   - 기억하지 않는 것: 복사(`Copy` `CopyBar`), undo·redo, 커서·선택·뷰, `.` 자신, 실패·변화 없음으로 끝난 동작.
   - 반복은 원래 동작과 같은 처리기를 거친다(같은 검사·같은 선택 규칙). 다시 보낸 Action이 적용된 뒤에도 묶음을 끊는다(리뷰 1 지적 2: 복제·반복은 묶음 키로 적용되어 다음 같은 키와 묶였다). 메뉴·팔레트에서 실행한 편집도 기억한다. 기억할 것이 없으면 「반복할 편집이 없다」. 반복된 동작은 undo 한 단위다(`.` 키는 묶음을 끊는다).
   - 기억은 창마다다(다른 창의 편집은 반복하지 않는다).
9. 키맵(결정 A1·A9·B25)
   - `default.json`: `Normal`과 `Step`의 `shift-1`…`shift-7` 바인딩을 지운다(B25 a′: 입력되는 글자 그대로. 점음표는 `!`…`&`). 파일 머리 주석을 고친다.
   - vim의 맨 `0`은 카운트 자리다(`Normal`·`Normal > PianoRoll`의 `0` = `operator::Count(0)`, 맨 앞 `0`은 카운트를 시작하지 않는다, 리뷰 1 지적 3). 기본 프리셋의 `0`(길이 기본값)은 vim에서 `g 0`이다.
   - `vim.json`: `Normal`에 `shift-j` = `edit::Join`, `shift-k` = `note::ShowInfo`(A1, 정보 팝업은 S3-T7), `g 1`…`g 7` = `note::SetLength(…)`, `g 0` = `note::ResetLength`(A9). 기본 프리셋의 깊은 컨텍스트 `shift-j`/`shift-k`(선택 넓히기)가 가리지 않도록 `Normal > PianoRoll`·`Normal > Arrangement`에도 같은 두 키를 건다(파일 머리의 규칙). `Step`의 `// 결정 대기: B25` 주석은 「vim 키 없음(`~`와 같은 글자)」으로 바꾼다. 남은 반영 목록 주석에서 A1·A9를 지운다.
   - REQ: 5.8.1 `j` `k` 행의 Shift 칸(옥타브는 `Alt+Shift+j/k`), `J`·`K` 행, `g` 행에 `g1`…`g7`·`g0`, 7.3 KM-7에 키맵 표기 규칙(B25 a′). 결정 문서 「후속」의 반영 줄을 더한다.

단계 파일 원문과 다른 점: `Shift+q` 설정 시트를 S3-T3e로 뗐다(결정 문서 「후속」). 단계 파일의 「`+`/`-`/`>`/`<`」는 S3-T2가 끝냈다(별칭 바인딩과 `NoteOp`).

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/pianoroll/edit.rs (순수)
/// 길이 코드의 틱(5.6). 점음표는 3/2.
pub fn length_ticks(code: LengthCode, dotted: bool) -> u32;
/// 기본 길이(4분음표). S3-T4가 설정으로 바꾼다.
pub const DEFAULT_LENGTH: u32 = 960;
/// 결과: 편집과 (원래, 바뀐) 짝. 바뀌는 노트가 없으면 `Ok(None)`.
pub struct Changed { pub edits: Vec<Edit>, pub moved: Vec<(Note, Note)> }
pub fn plan_split(project: &Project, track: TrackId, clip: ClipId, notes: &[Note], at: Tick) -> Result<Option<Changed>, ModelError>;
pub fn plan_join(project: &Project, track: TrackId, clip: ClipId, notes: &[Note]) -> Result<Option<Changed>, ModelError>;
pub fn plan_set_length(project: &Project, track: TrackId, clip: ClipId, notes: &[Note], length: u32) -> Result<Option<Changed>, ModelError>;
pub fn plan_set_pitch(project: &Project, track: TrackId, clip: ClipId, note: &Note, pitch: Pitch) -> Result<Option<Changed>, ModelError>;
pub fn plan_legato(project: &Project, track: TrackId, clip: ClipId, notes: &[Note]) -> Result<Option<Changed>, ModelError>;
pub fn plan_restore(project: &Project, track: TrackId, clip: ClipId, pairs: &[(Note, Note)]) -> Result<Option<Changed>, ModelError>;
pub fn plan_quantize(project: &Project, track: TrackId, clip: ClipId, notes: &[Note], settings: &QuantizeSettings) -> Result<Option<Changed>, ModelError>;
/// 편집 뒤 선택: 대상 가운데 남은 노트 + 새로 나타난 노트.
pub fn selection_after(before: &Clip, after: &Clip, targets: &[Note]) -> BTreeSet<Note>;

// crates/keyscore-app/src/pianoroll/keys.rs (순수)
/// 음 키의 옥타브 안 반음 자리(P3). 음 키가 아니면 `None`.
pub fn semitone_of_key(layout: KeyLayout, key: &str) -> Option<u8>;
/// 커서 음높이의 옥타브 C에 반음 자리를 더한 음높이. 0–127 밖이면 `None`.
pub fn pitch_in_octave_of(cursor: Pitch, semitone: u8) -> Option<Pitch>;

// crates/keyscore-app/src/settings.rs
pub struct QuantizeSettings { pub grid: QuantizeGrid, pub triplet: bool, pub strength: u8, pub swing: u8 }
impl QuantizeSettings { pub fn core(&self) -> keyscore_core::transform::QuantizeSettings; }
pub struct Settings { …, pub quantize: QuantizeSettings }

// crates/keyscore-app/src/window.rs (창 상태)
pub fn pitch_pending(&self) -> bool; // 시험용
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `length_ticks_match_req_table` | 단위 | 5.6 표의 일곱 길이와 점음표 |
| 2 | `split_cuts_notes_crossing_cursor_only` | 단위 | 커서를 가로지르는 노트만, 시작·끝 자리는 그대로, 가사는 앞에만 |
| 3 | `split_keeps_hidden_tail_as_tail` | 단위 | 꼬리 노트 분할이 적용에 성공하고 뒤쪽이 꼬리 |
| 4 | `join_merges_same_pitch_groups` | 단위 | 음높이 묶음별 합침, 하나뿐인 음높이는 그대로, 묶음 없으면 `None` |
| 5 | `join_with_tail_keeps_tail` | 단위 | 꼬리를 포함한 묶음 적용 성공 |
| 6 | `set_length_extends_clip_by_bar_and_skips_equal` | 단위 | 마디 단위 연장, 같은 길이 제외, 겹치면 거부 |
| 7 | `legato_sets_end_to_next_start_any_pitch` | 단위 | 다음 시작(음높이 무관), 줄이기, 마지막 노트 그대로, 화음 |
| 8 | `semitone_of_key_follows_p3_layouts` | 단위 | 건반형·계이름형 표, 음 키 아닌 키 `None` |
| 9 | `quantize_settings_read_write_and_defaults` | 단위(settings) | 기본값, 필드별 경고, 모르는 키 보존 |
| 10 | `split_slash_and_vim_s_split_at_cursor` | UI | `/`(선택·커서 노트), vim `s`, undo 한 단위, 뒤 선택 |
| 11 | `join_cmd_j_and_vim_shift_j` | UI | `cmd-j`, vim `shift-j`, 뒤 선택, 묶음 없으면 알림 |
| 12 | `length_codes_set_selection_length` | UI | `1`–`7`, `#`(점4분), `0`, vim `g 5`·`g 0`, 커서 노트 |
| 13 | `shift_digit_keys_are_not_bound` | UI·키맵 | `shift-3`이 아니라 `#`로 점음표, 프리셋에 `shift-N` 없음 |
| 14 | `e_then_note_key_sets_cursor_note_pitch` | UI | 건반형 `e a`, `e k`(옥타브 위), 계이름형 `e g`, `Esc`·다른 키 취소(먹음), vim `r` |
| 15 | `quantize_q_uses_settings` | UI | 기본 1/16, 설정 바꾸면 1/8, 변화 없음 알림 |
| 16 | `legato_tilde_toggles_back` | UI | `~` 붙이기, 다시 `~` 되돌림, 사이에 다른 편집이면 다시 붙이기 |
| 17 | `dot_repeats_last_edit` | UI | `5` 뒤 다른 노트에서 `.`, `Alt+k` 뒤 `.`, `e a` 뒤 `.`, 복사는 기억 안 함, 기억 없음 알림 |
| 18 | `vim_shift_k_is_show_info_and_g_digits_are_lengths` | 키맵 | vim 바인딩이 깊은 컨텍스트에서도 이김(`bindings_for_input`) |
| 19 | `outside_clip_selection_is_refused_for_new_edits` | UI | 선택이 다른 클립이면 `/` `cmd-j` `5` `q` `~` 거부 |
| 20 | `arrangement_view_note_edit_keys_do_nothing` | UI | 배치 뷰에서 `/` `5` `q` `~`는 문서 그대로 |

## 불변식

- 모든 편집은 `ProjectDocument::apply` 한 번(undo 한 단위)이고, 계산 실패면 문서·undo 기록이 그대로다.
- 숨김 꼬리는 새로 생기지 않는다(분할·합치기가 꼬리 노트를 바꾸기로만 다룬다, core가 거부하므로 시험으로 고정).
- 노트는 클립 안(시작), 클립 겹침 없음: core가 보장하고 계산은 `map_notes`·`ReplaceNotes`만 쓴다.
- 음 키 대기는 그 창의 키만 먹는다.

## 범위 밖

- `Shift+q` 퀀타이즈 설정 시트(S3-T3e로 뗌). 설정은 `settings.json`을 손으로 고쳐도 읽힌다.
- 노트 정보 팝업(`note::ShowInfo`, S3-T7). vim `K`는 바인딩만.
- 배치 뷰의 분할·합치기(클립 단위, S4).
- Step 모드의 현재 길이·기본 길이 설정, Step의 음 키 입력(S3-T4). `semitone_of_key`는 S3-T4가 쓸 수 있게 순수 함수로 둔다.
- 복제 간격의 격자 정렬(결정 대기 `2026-10-26-duplicate-offset-grid.md`).
- vim 오퍼레이터와 `.`의 결합(`d w` 반복 등, S3-T3c).

## 리스크·결정 요청

- 합치기·레가토 토글·퀀타이즈 설정·`e` 옥타브는 사람이 정했다(`docs/decisions/2026-10-26-s3-t3b-note-edit-details.md`).
- 분할의 가사를 앞 노트에만 남기는 것은 「음절은 한 번」이라는 5.15의 노트별 음절 모델에서 나온 읽기다. 리뷰가 다르게 보면 결정 요청으로 넘긴다.
- `.`이 `Window::dispatch_action`으로 기억한 Action을 다시 보내는 방식은 같은 처리기를 거쳐 규칙이 갈라지지 않는다. 대신 처리기가 실행 시점의 포커스를 따르므로 피아노롤 포커스에서만 의미가 있다(`.` 자체가 피아노롤 키다).
