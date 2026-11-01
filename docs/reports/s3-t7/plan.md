# Task 계획: S3-T7 검색과 노트 정보

- 브랜치: `s3/t7-search-note-info`
- 관련 REQ: 5.8 표 `Cmd+F`·`i`, 5.8.1 vim `/`·`n`·`N`·`K`, 7.5 `Cmd+F`, P2, 7절, UI-11
- 선행 Task: S3-T2(커서·마크 이동), S3-T3e(창 안 시트), S2-T4(명령 표·팔레트)
- 관련 결정: `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md`(이 Task의 규칙 전부), `2026-10-29-in-window-sheets-vs-modal-dialogs.md`
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

Normal 모드의 프로젝트 창에서 `Cmd+F`(vim `/`)가 창 안 검색 막대를 연다. `c#4`·`eb`(음높이), 트랙 이름, 가사를 쳐서 `Enter`(다음)·`Shift+Enter`(이전)로 커서를 결과로 옮긴다. 막대를 닫은 뒤 `Cmd+G`·`Cmd+Shift+G`(vim `n`·`N`)가 지난 검색어로 다음·이전 결과로 간다. `i`(vim `K`)는 커서 노트의 정보(음높이·시작·길이·채널·벨로시티·가사·트랙)를 창 안 팝업으로 보이고 `Esc`·`Enter`로 닫는다. 세부 규칙은 결정 문서 1–6절이 전부이고 시험이 고정한다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/search.rs (순수 계산, GPUI 없음)
pub enum Query { Pitch(Pitch), PitchClass(u8), Text(String) }
pub enum QueryError { Empty, PitchOutOfRange }
impl Query { pub fn parse(text: &str) -> Result<Query, QueryError>; }
pub struct Hit { pub track: TrackId, pub tick: Tick, pub pitch: Pitch, pub has_note: bool }
pub fn find_hits(project: &Project, query: &Query) -> Vec<Hit>;          // 결정 3절 순서, 중복 없음
pub struct Position { pub track_index: usize, pub tick: Tick, pub pitch: Pitch }
pub enum Direction { Forward, Backward }
pub struct Found { pub index: usize, pub wrapped: bool }
pub fn step(project: &Project, hits: &[Hit], at: Position, direction: Direction) -> Option<Found>;

// crates/keyscore-app/src/note_info.rs
pub struct InfoLine { pub label: &'static str, pub value: String }
pub fn info_lines(project: &Project, track: TrackId, clip: ClipId, note: &Note) -> Vec<InfoLine>;

// ProjectWindow (시험·접근성)
pub fn search_bar_open(&self) -> bool;
pub fn search_status(&self) -> Option<String>;   // 막대의 결과 수 글
pub fn last_search(&self) -> Option<String>;
pub fn note_info(&self) -> Option<Vec<InfoLine>>;
~~~

키맵: 기본 프리셋 `Normal`에 `cmd-g` → `search::Next`, `cmd-shift-g` → `search::Prev`. 막대 컨텍스트 `SearchBar`, 정보 팝업 `Dialog > NoteInfo`. 새 Action은 없다.

## 테스트 목록

순수 계산 `crates/keyscore-app/tests/search_pure.rs`, 창 시험 `crates/keyscore-app/tests/search.rs`·`note_info.rs`.

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `parse_pitch_forms_cover_accidentals_and_octaves` | 단위 | `c#4`=61, `Eb3`=51, `b-1`=11, `cb4`=59, 대소문자, 공백 |
| 2 | `parse_pitch_without_octave_is_pitch_class` | 단위 | `a`→클래스 9, `c#`→1, `cb`→11 |
| 3 | `parse_out_of_range_and_empty_and_quoted` | 단위 | `b9`·`cb-1` 범위 밖, 빈칸, `"a"`는 글자, `""`는 빈 검색어, `bass`는 글자 |
| 4 | `find_hits_pitch_matches_exact_and_class` | 단위 | 여러 트랙·클립, 클립 시작 더하기, 오프셋 무시, 클래스 |
| 5 | `find_hits_text_matches_track_names_and_lyrics` | 단위 | 대소문자 무시 부분 문자열, 트랙 결과 자리(첫 노트·첫 클립·0), 같은 자리 중복 하나 |
| 6 | `find_hits_are_ordered_by_tick_track_pitch` | 단위 | 결정 3절 4 |
| 7 | `step_forward_backward_and_wrap` | 단위 | 다음·이전, 끝에서 돎(wrapped), 유일한 결과 위, 빈 결과 |
| 8 | `step_order_property_visits_every_hit_once` | 속성 | 임의 결과에서 Forward를 결과 수만큼 반복하면 모든 결과를 한 번씩 돈다 |
| 9 | `info_lines_show_pitch_start_length_channel` | 단위 | 음이름·번호, 마디.박.틱, 길이 코드 이름, 채널, 벨로시티, 가사, 오프셋 센트 |
| 10 | `cmd_f_opens_bar_and_enter_jumps_to_next_pitch` | UI | `cmd-f`, `c#4` 입력, 결과 수, `enter` → 커서·트랙 이동, 막대 닫힘, 포커스 문서, 상태줄 「3/12」꼴 |
| 11 | `shift_enter_jumps_backward_and_wraps` | UI | `shift-enter` 이전, 돎 문구 |
| 12 | `escape_closes_without_changing_last_search` | UI | `escape` 닫힘, 지난 검색어 그대로, 커서 그대로 |
| 13 | `cmd_g_repeats_last_search_in_default_preset` | UI | 닫은 뒤 `cmd-g`·`cmd-shift-g`, `n`은 여전히 다음 노트 |
| 14 | `vim_slash_and_n_navigate` | UI | vim `/`, `enter`, `n`, `shift-n` |
| 15 | `search_jumps_to_other_track_by_name_and_lyric` | UI | 트랙 이름 결과로 현재 트랙이 바뀜, 가사 결과 |
| 16 | `reopen_prefills_last_query` | UI | 다시 열면 지난 검색어가 차 있고 `enter`가 같은 검색 |
| 17 | `next_without_query_says_so` | UI | 지난 검색어 없이 `cmd-g` → 「검색어가 없다」, 커서 그대로 |
| 18 | `search_only_in_normal_mode` | UI | Step 모드에서 `search::Open`·`search::Next`는 메시지만 |
| 19 | `document_keys_do_not_reach_document_while_bar_open` | UI | 막대에서 `x`·`l`가 문서·커서를 바꾸지 않음 |
| 20 | `bar_buttons_navigate_and_close` | UI | ‹ › 버튼은 이동하고 막대를 둔다, × 닫기 |
| 21 | `bar_and_other_popups_close_each_other` | UI | 팔레트·시트·정보 팝업과 하나만 |
| 22 | `search_does_not_touch_undo_or_selection` | UI | undo 길이·선택 그대로 |
| 23 | `i_opens_note_info_for_cursor_note` | UI | `i` → 정보 줄, 포커스 팝업, `escape`·`enter` 닫힘 |
| 24 | `i_without_note_says_so` | UI | 빈 칸에서 `i` → 메시지, 팝업 없음 |
| 25 | `vim_shift_k_opens_note_info` | UI | vim `shift-k` |
| 26 | `note_info_closes_on_document_change_and_outside_click` | UI | 열린 채 `cmd-z` → 닫힘, 배경 클릭 → 닫힘 |
| 27 | `note_info_keys_do_not_reach_document` | UI | 팝업에서 `x`·`l`가 문서·커서를 바꾸지 않음 |

## 불변식

- 검색·정보 팝업은 문서·undo 기록·`.` 반복의 마지막 편집을 바꾸지 않는다. 검색은 커서·현재 트랙만 바꾼다.
- 팝업은 하나: 팔레트·오버레이·팝업 메뉴·퀀타이즈 시트·검색 막대·노트 정보 가운데 하나만 열려 있다.
- `find_hits`의 결과는 (틱, 트랙 순서, 음높이)로 정렬되어 있고 자리가 겹치지 않는다. `step`은 그 순서에서만 고른다.

## 범위 밖

- 점진 검색(치는 동안 이동), 결과 강조 표시, 결과 목록 창.
- 검색 모션(`d n`, `d /…`).
- 정규식·여러 조건 검색.
- 배치 뷰에서 결과 클립을 따로 표시하는 것(커서·트랙만 옮긴다).
- 정보 팝업에서 값을 고치는 것(인스펙터·키 조작이 한다).

## 리스크·결정 요청

- 세부는 모두 결정 `2026-11-01-s3-t7-search-and-note-info.md`(에이전트 해석)에 적었다. REQ 뜻을 바꾸지 않는 읽기다. 사람이 다르게 보면 그 문서를 고친다.
- 기본 프리셋 `Cmd+G`·`Cmd+Shift+G`는 REQ 표에 없는 키다. 7.5 전역 표는 건드리지 않고 기본 프리셋 `Normal`에 둔다(사용자 키맵으로 바꿀 수 있다).
- gpui-component 한 줄 입력란은 `Enter`·`Shift+Enter`를 `InputEvent::PressEnter { shift }`로 알리지만, 사건을 낸 뒤 키를 전파해 「처리 안 된 키」로 남기므로 줄바꿈 글자가 입력으로 들어가 골라 둔 글자를 지운다(구현 중 발견). 그래서 막대의 키 가로채기(`capture_key_down`)가 `Enter`·`Shift+Enter`·`Esc`를 입력란보다 먼저 받는다. IME 조합 중에는 건드리지 않는다.
