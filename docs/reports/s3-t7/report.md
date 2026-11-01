# Task 보고: S3-T7 검색과 노트 정보

- PR: 브랜치 `s3/t7-search-note-info`
- 계획 파일: docs/reports/s3-t7/plan.md
- 결정: `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md`(에이전트 해석, 1–6절)
- 계획 대비 달라진 점
  - 막대의 `Enter`·`Shift+Enter`는 `InputEvent::PressEnter`가 아니라 막대의 키 가로채기에서 받는다. 입력란이 `Enter`를 전파해 「처리 안 된 키」가 되면 줄바꿈 글자가 들어가 다시 연 막대의 골라 둔 지난 검색어를 지웠다(`reopen_prefills_last_query`가 잡음). 계획 「리스크」를 고쳤다.
  - 순수 시험에 `step_uses_current_track_order`(결과의 트랙 순서는 지금 프로젝트 순서)를 더했다.
  - 창 생성 함수가 clippy 줄 수 상한(100)을 넘어, 문서 관찰자의 본문을 `ProjectWindow::document_changed`로 옮겼다(동작 같음).
  - 앱 crate dev-dependency에 `proptest`(워크스페이스 버전)를 더했다(속성 시험 8).

## 구현 요약

- `crates/keyscore-app/src/search.rs`(새 파일, 순수): `Query::parse`(음높이 `c#4`·`eb`·`b-1`, 클래스, 큰따옴표 글자, 범위 밖·빈칸 오류), `find_hits`(프로젝트 전체, (틱, 트랙 순서, 음높이) 순서, 같은 자리 하나, 트랙 이름 결과의 자리), `step`(다음·이전, 끝에서 돎).
- `crates/keyscore-app/src/search_bar.rs`(새 파일): 막대(`SearchBar`: 입력란·결과 수 글·구독), `search::Open`·`Next`·`Prev` 처리기, Normal 모드 검사, 이동(`search_jump`: 트랙 바꾸기 → `jump_cursor`, 오퍼레이터 대기 끝내기, 되돌리기 묶음 끊기, 상태줄 「검색 q: n/m」과 돎 문구), 키 가로채기(`Esc`·`Enter`·`Shift+Enter`), 그리기(‹ › × 버튼).
- `crates/keyscore-app/src/note_info.rs`(새 파일): `info_lines`(음높이·시작·길이·채널·벨로시티·가사·트랙, 오프셋 cent, 길이 코드 이름), 팝업(`NoteInfoPopup`, `Dialog > NoteInfo`, 배경 클릭·포커스 이탈·문서 변경으로 닫힘), `note::ShowInfo` 처리기.
- `crates/keyscore-app/src/window.rs`: 필드 `search_bar`·`last_search`·`note_info`, 루트 처리기 넷, 루트 자식으로 그리기, 문서 관찰자에서 정보 팝업 닫기·막대 결과 수 다시 세기, 팔레트·팝업 메뉴를 열면 막대·팝업을 닫음.
- `crates/keyscore-app/src/overlay.rs`, `quantize_sheet.rs`: 열 때 막대·정보 팝업을 닫음(팝업 하나 규칙).
- `crates/keyscore-app/src/selection.rs`: `jump_cursor`를 `pub(crate)`로.
- `assets/keymaps/default.json`: `Normal`에 `cmd-g` → `search::Next`, `cmd-shift-g` → `search::Prev`(결정 5절 3). 기존 `cmd-f`·vim `/`·`n`·`N`·`i`·`K`는 S2-T2부터 있었다.

## 테스트 결과

~~~
keyscore-app --test search_pure: 10 passed
keyscore-app --test search: 13 passed
keyscore-app --test note_info: 5 passed
keymap_engine·keymap_overlay·keymap_source·keymap_user·commands·carry_overs·quantize_sheet: 실패 없음
전체 cargo test --workspace --no-fail-fast: 아래 「자동 게이트」
~~~

창 시험 18개는 처리기 등록 넷과 `cmd-g` 키맵을 빼고 돌려 모두 실패하는 것을 확인했다(처음 구현과 같은 브랜치, 커밋 전). 순수 시험 10개는 `todo!()` 뼈대에서 실패를 확인했다(커밋 b712774).

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `parse_pitch_forms_cover_accidentals_and_octaves` | 통과 |
| 2 | `parse_pitch_without_octave_is_pitch_class` | 통과 |
| 3 | `parse_out_of_range_and_empty_and_quoted` | 통과 |
| 4 | `find_hits_pitch_matches_exact_and_class` | 통과 |
| 5 | `find_hits_text_matches_track_names_and_lyrics` | 통과 |
| 6 | `find_hits_are_ordered_by_tick_track_pitch` | 통과 |
| 7 | `step_forward_backward_and_wrap` | 통과 |
| — | `step_uses_current_track_order` | 통과(추가) |
| 8 | `step_order_property_visits_every_hit_once` | 통과(proptest) |
| 9 | `info_lines_show_pitch_start_length_channel` | 통과 |
| 10 | `cmd_f_opens_bar_and_enter_jumps_to_next_pitch` | 통과 |
| 11 | `shift_enter_jumps_backward_and_wraps` | 통과 |
| 12 | `escape_closes_without_changing_last_search` | 통과 |
| 13 | `cmd_g_repeats_last_search_in_default_preset` | 통과 |
| 14 | `vim_slash_and_n_navigate` | 통과 |
| 15 | `search_jumps_to_other_track_by_name_and_lyric` | 통과 |
| 16 | `reopen_prefills_last_query` | 통과 |
| 17 | `next_without_query_says_so` | 통과 |
| 18 | `search_only_in_normal_mode` | 통과 |
| 19 | `document_keys_do_not_reach_document_while_bar_open` | 통과 |
| 20 | `bar_buttons_navigate_and_close` | 통과 |
| 21 | `bar_and_other_popups_close_each_other` | 통과 |
| 22 | `search_does_not_touch_undo_or_selection` | 통과 |
| 23 | `i_opens_note_info_for_cursor_note` | 통과 |
| 24 | `i_without_note_says_so` | 통과 |
| 25 | `vim_shift_k_opens_note_info` | 통과 |
| 26 | `note_info_closes_on_document_change_and_outside_click` | 통과 |
| 27 | `note_info_keys_do_not_reach_document` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

GATES_PLACEHOLDER

## 알려진 한계

- 점진 검색, 결과 강조, 검색 모션(`d n`)은 범위 밖(결정 「후속」).
- IME 조합 중의 `Enter`는 입력기에 맡긴다(조합 확정). 시험 하네스에 IME가 없어 사람 확인 항목이다.
- 막대 바깥을 누르면 그 누름이 아래에 닿고(배경을 막지 않음), 포커스가 문서로 가며 막대가 닫힌다. 문서 아닌 곳(트랜스포트 입력란)을 누르면 그 입력란으로 포커스가 가고 막대가 닫힌다.
- 막대 안의 `Cmd+G`는 동작하지 않는다(`Normal` 컨텍스트 바인딩이라). 막대 안에서는 `Enter`·`Shift+Enter`·‹ › 버튼을 쓴다.

## 리뷰어에게

- `search_bar.rs` `search_key_down`: `Enter`를 입력란보다 먼저 가로챈다. 수식키 조건(`Shift`만 허용)과 IME 조합 검사가 맞는지.
- `search.rs` `step`: 결과의 트랙이 프로젝트에 없으면 고르지 않는다. 결과는 이동할 때마다 다시 찾으므로 실제로는 생기지 않는다.
- 트랙 이름 결과의 자리(결정 3절 3)와 노트 없는 트랙의 음높이 처리(커서 음높이 유지).
- 정보 팝업은 문서가 바뀌면 닫힌다(`document_changed`). 재생 중 문서 교체 경로가 없는지.
- 팝업 하나 규칙: 팔레트·`?` 오버레이·팝업 메뉴·퀀타이즈 시트·검색 막대·정보 팝업의 여닫기 조합.

## 리뷰 1 대응

리뷰 1 판정 「수정 후 재리뷰」(차단 1, 수정 4, 제안 2). 리뷰 시험 `review_s3_t7_r1.rs` 13개는 고친 뒤 모두 통과한다.

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 차단: 노트 없는 트랙 결과에서 「이전」이 갇힘 | `step`이 노트 없는 결과는 (틱, 트랙)만 보고 지금 자리와 비교한다. 결정 3절 5에 적었다 | 7206b61 |
| 2 수정: 오퍼레이터 대기 중 `n`·`N`·`cmd-g`가 삼켜짐 | `allowed_while_pending`에 `search::Next`·`Prev`. 처리기가 대기를 끝내고 이동만 한다 | b485192 |
| 3 수정: 막대가 열린 채 모드가 바뀐 뒤 `Enter`가 Step 커서를 옮김 | 막대가 `mode::Cycle`·`CycleBack`을 삼키고(팔레트와 같다), `search_confirm`이 Normal이 아니면 막대를 닫고 메시지만 남긴다. 시험 `bar_does_not_search_after_mode_change` | d07e60e |
| 4 수정: 잘못된 지난 검색어가 오퍼레이터 대기를 남김 | `search_jump` 첫머리에서 대기를 끝낸다. 시험 `invalid_last_query_still_ends_operator_wait` | d5865f1 |
| 5 수정: 빠진 시험 | 리뷰 시험이 포커스 이탈·오버레이·배치 뷰·비주얼 `n`·Step 정보 팝업을 덮는다. 팝업 메뉴(`bar_and_popup_menu_close_each_other`)와 결과 수 다시 세기(`bar_recounts_when_document_changes`)를 더했다 | 0455693 |
| 6 제안: 트랙을 넘는 비주얼 | 받아들임. 결정 4절 2에 「기준 칸은 그대로」를 적었다 | 이 문서 커밋 |
| 7 제안: `"  "` | 받아들임. 결정 2절 2에 「감싼 안은 다듬지 않는다」를 적었다 | 이 문서 커밋 |
