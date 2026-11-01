# 적대적 리뷰: S3-T7 검색과 노트 정보 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `36be7f2` (`git diff main...HEAD`)
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t7_r1.rs`

## 0. 실행 환경의 한계(먼저 적는다)

리뷰 중 디스크가 가득 찼다(`df`: 가용 120–600MiB, `target/debug` 73G는 다른 세션의 것이라 건드리지 않았다). `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t7_r1`는 의존 crate 빌드 중 `No space left on device (os error 28)`로 멈췄다. 그래서 GPUI 창 시험은 실행하지 못했다.

대신 순수 계산은 실제로 돌렸다. `CARGO_TARGET_DIR=target/review cargo build -p keyscore-core`로 core만 빌드하고, 제품 파일 `crates/keyscore-app/src/search.rs`를 `#[path]`로 그대로 끌어온 시험 하네스(`target/review/s3t7-pure/main.rs`, 저장소 밖 빌드 디렉터리)를 `rustc --test`로 돌렸다. 그 하네스의 시험 셋은 리뷰 시험 파일 8절(`step_*`)과 6절(`parse_edge_forms_follow_decision_grammar`)과 같은 내용이다. 결과는 3절 표에 적었다.

리뷰 시험 파일의 창 시험(1–5, 7절)은 컴파일·실행을 확인하지 못했다. 구현자는 디스크를 비운 뒤 이 파일을 먼저 돌려 컴파일 오류가 있으면 리뷰어에게 알린다(시험 의도는 각 시험의 주석에 적었다).

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 노트 없는 트랙의 이름 결과: `find_hits`는 그 자리의 음높이를 0으로 두고, 이동은 커서 음높이를 그대로 둔다. 그 결과 위에서 「이전」을 하면 커서 자리 (틱, 트랙, 커서 음높이)가 결과 자리 (틱, 트랙, 0)보다 뒤라 같은 결과를 다시 고르지 않는가.
2. vim 오퍼레이터 대기 중 `n`·`cmd-g`: 결정 4절 4는 「대기를 끝내고 이동만」인데, vim 키 가로채기(`vim.rs` `allowed_while_pending`)가 `search::Next`를 허용 목록에 두지 않아 키를 삼키지 않는가.
3. 막대가 열린 채 모드가 바뀌는 경로: 전역 `ctrl-tab`(`mode::Cycle`)은 막대 입력란에서도 창 루트로 거품이 올라간다. 팔레트는 이를 막는데(`command_palette.rs:249`) 막대는 막지 않는다. 그 뒤 `Enter`·‹ › 버튼이 Step 커서를 옮기지 않는가(`search_confirm`은 모드를 다시 보지 않는다).
4. 검색어 문법 경계: `cb-1`, `B#9`, `b#-1`, 전각 공백 trim, `c##4`, `c#10`, 비ASCII(`가`, `é`, 아라비아 숫자), 따옴표 하나, 빈 따옴표, 따옴표 안 음높이. 바이트 슬라이스(`&rest[1..]`, `as_bytes()[0]`)가 비ASCII에서 공황하지 않는가.
5. 팝업 하나 규칙: `?` 오버레이·팝업 메뉴와 막대가 서로 닫는가, 닫은 뒤 포커스가 문서로 돌아오는가.
6. 포커스 이탈로 막대 닫기(결정 1절 7), 창 비활성 예외.
7. 배치 뷰에서 검색: 뷰는 그대로, 트랙·커서만 바뀌는가.
8. vim 비주얼 중 `n`: 비주얼 선택을 새 커서까지 다시 계산하는가. 다른 트랙으로 넘어갈 때 기준 칸이 이전 트랙의 것으로 남는가.
9. 정보 팝업이 Step 등 다른 모드에서 열리고(결정 6절 5) 닫은 뒤 모드·포커스가 그대로인가.
10. 정보 팝업 cent 반올림(음수, 절반), 길이 이름 충돌(점·셋잇단이 다른 코드의 기본 길이와 같은 틱).
11. 막대 `Enter` 가로채기의 수식키 조합: `cmd-enter`는 입력란을 지나 전역 `transport::Toggle`로 간다(의도인가), IME 조합 중에는 손대지 않는가.
12. 막대·팝업이 열린 채 문서가 바뀔 때: 정보 팝업은 닫히고 막대의 결과 수는 다시 세는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 노트 없는 트랙 결과에서 이전 | 없음 | `find_hits_text_matches_track_names_and_lyrics`는 자리만 보고, 그 자리에서의 `step`은 보지 않는다 |
| 2 오퍼레이터 대기 중 `n` | 없음 | 계획 표에도 없다. 결정 4절 4가 시험으로 고정되지 않았다 |
| 3 막대가 열린 채 모드 변경 | 없음 | `search_only_in_normal_mode`는 막대를 열기 전의 모드만 본다 |
| 4 문법 경계 | 부분 | `cb-1`·`b9`·`""`·`"a"`는 있다. 비ASCII·`b#-1`·`c#10`·전각 공백은 없다 |
| 5 팝업 하나(`?` 오버레이·팝업 메뉴) | 부분 | 팔레트·시트·정보 팝업만 있다 |
| 6 포커스 이탈로 닫기 | 없음 | 정보 팝업은 배경 클릭만, 막대는 없음 |
| 7 배치 뷰 | 없음 | 모든 창 시험이 피아노롤에서 시작한다 |
| 8 비주얼 중 `n` | 없음 | |
| 9 다른 모드의 정보 팝업 | 없음 | |
| 10 cent·길이 이름 | 있음 | `info_lines_show_pitch_start_length_channel`(+25 cent, 점4분, 셋잇단). 음수 cent는 없음 |
| 11 수식키 `Enter` | 없음 | |
| 12 문서 변경 중 팝업 | 부분 | 정보 팝업 `cmd-z`는 있음. 막대 결과 수 다시 세기는 없음 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 노트 없는 트랙 「Pad」의 이름 결과 위(커서 음높이 72)에서 `Backward` | 깨짐. Lead@0이 아니라 Pad 자신(`Found { index: 1, wrapped: false }`)을 다시 고른다. 창에서는 `cmd-shift-g`·vim `N`·`Shift+Enter`가 그 자리에 갇힌다 | `step_prev_from_empty_track_hit_position_picks_earlier_hit`(순수, 실행해 실패 확인), `search_prev_from_empty_track_hit_moves_to_earlier_hit`(창, 실행 못함) |
| 2 | 같은 자리가 유일한 결과일 때 앞·뒤의 돎 표시 | 깨짐. Forward는 `wrapped: true`, Backward는 `wrapped: false`라 「이전」만 돎 문구가 빠진다(결정 3절 5) | `step_on_sole_empty_track_hit_wraps_both_ways`(순수, 실행해 실패 확인), `search_prev_on_sole_empty_track_hit_says_wrapped`(창, 실행 못함) |
| 3 | 문법 경계 21가지(`cb-1`, `B#9`, `b#-1`=12, `G9`=127, 탭·전각 공백 trim, `bb`=클래스 10, `c##4`·`c#10`·`c4x`·`c가`·`가`·`é`·`c#٤`·`h`·`-1`·`"`·`"a"`는 글자, `""`는 빈칸, `"C#4"`는 글자) | 못 깨뜨림(공황 없음, 결정 2절과 같음) | `parse_edge_forms_follow_decision_grammar`(순수, 실행해 통과) |

창 시험 `vim_n_while_operator_pending_ends_wait_and_moves`, `vim_cmd_g_while_operator_pending_ends_wait_and_moves`, `bar_enter_after_mode_cycle_does_not_move_step_cursor`, `search_in_arrangement_keeps_view_and_switches_track`, `vim_visual_n_recomputes_selection_to_new_cursor`, `bar_and_keymap_overlay_close_each_other`, `bar_closes_when_focus_moves_to_document`, `note_info_opens_in_step_mode_and_returns_focus`는 파일에 썼지만 디스크 부족으로 실행하지 못했다(0절). 앞의 셋은 아래 7절의 코드 근거로 실패를 예상한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 검색·정보 팝업은 문서·undo·`.`을 바꾸지 않는다 | 못 찾음 | `search_jump`는 `seal_history`·`jump_cursor`·`set_current_track`만 부른다. `show_note_info`는 읽기만 한다 |
| 팝업은 하나 | 못 찾음(코드 읽기) | 여는 곳 여섯(`search_open`, `show_note_info`, `overlay.rs:233`, `quantize_sheet.rs:159`, `window.rs` 팔레트·팝업 메뉴)이 모두 나머지를 비운다 |
| `find_hits`는 정렬되고 자리가 겹치지 않는다 | 못 깨뜨림 | `BTreeMap` 열쇠 (틱, 트랙 순서, 음높이) |
| `step`은 「지금 자리」 다음·이전을 고른다 | 깨짐 | 노트 없는 트랙 결과의 자리와 이동 뒤 커서 자리가 다르다(3절 1·2) |
| 검색은 Normal 모드에서만 커서를 옮긴다(결정 1절 2) | 깨질 것으로 본다(실행 못함) | 막대를 연 뒤 `ctrl-tab` → `Enter`(7절 지적 3) |
| core 모델 불변식(겹침·노트는 클립 안·채널) | 해당 없음 | 이 Task는 문서를 고치지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8 `Cmd+F` 검색(`c#4`, 트랙 이름, 가사) | 충족 | `search.rs:47-90`, `search_bar.rs:75-130`, `standard.json` `cmd-f` |
| 5.8 `i` 커서 노트 정보 팝업(틱·길이·채널) | 충족 | `note_info.rs:18-69`, `note_info.rs:141-171`, `default.json:52` |
| 5.8.1 vim `/` 검색 | 충족 | `vim.json:166` |
| 5.8.1 vim `n`·`N` 다음·이전 결과 | 미충족(부분) | `n`·`N`은 걸려 있으나 노트 없는 트랙 결과에서 `N`이 갇힌다(`search.rs:194-207`, `search_bar.rs:221-225`). 오퍼레이터 대기 중 `n`이 삼켜진다(`vim.rs:108-138`, 결정 4절 4와 다름) |
| 5.8.1 vim `K` 노트 정보 | 충족 | `vim.json:229`, `vim.json:253` |
| 7.5 `Cmd+F` | 충족 | `standard.json` |
| P2 `Esc`는 통상의 취소(팝업 닫기) | 충족 | `search_bar.rs:287-289`, `note_info.rs:242-244` |
| 7절 모달 다이얼로그 없음 | 충족 | 막대·팝업 모두 창 안 `deferred` 패널 |
| UI-11 보이는 UI | 충족 | 메뉴 편집 > 검색(`commands/table.rs:1226`), 막대 ‹ › 버튼. 정보 팝업은 기존 표 항목(`table.rs:1042`) 그대로 |

「부분 충족」은 미충족으로 센다: 5.8.1 `n`·`N` 하나.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 스케줄러를 건드리지 않는다. `git diff main...HEAD`에 `keyscore-midi`·엔진 변경 없음)
- 제품 코드의 `unwrap`/`expect`: `search.rs:126` `u8::try_from(semitone.rem_euclid(12)).expect("0–11")` 하나. `rem_euclid(12)`는 0–11이라 정당하다. `search_bar.rs:216` `hits[found.index]`는 `step`이 `hits`의 순번만 돌려주므로 범위 안이다
- 에러가 삼켜지는 곳: 없음. `Query::parse` 오류는 막대·상태줄 글로 나간다(`search_bar.rs:40-41`, `190`). 다만 오류 경로(`search_bar.rs:189-192`)는 `clear_operator`보다 먼저 돌아가 오퍼레이터 대기를 남긴다(결정 4절 4, 지적 4)

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 노트 없는 트랙의 이름 결과에서 「이전」이 같은 결과를 다시 골라 커서가 갇힌다. 결정 3절 3(자리 음높이 0)과 4절 1(커서 음높이를 둔다)이 맞물리지 않는다. 그 결과로 간 뒤 커서 자리는 (틱, 트랙, 커서 음높이)라 결과 자리 (틱, 트랙, 0)보다 「뒤」가 된다. `cmd-shift-g`·vim `N`·막대 `Shift+Enter`·‹ 버튼이 모두 갇히고, 그 결과가 유일하면 「이전」만 돎 문구가 빠진다. 5.8.1 `N` 미충족이라 차단으로 둔다. 고치는 길은 구현자가 정한다(예: `step`이 자리 비교에서 `has_note`가 거짓인 결과의 음높이를 무시하고 (틱, 트랙)만 보거나, 이동 뒤 커서 음높이를 결과 자리와 같게 다루기). 결정 문서도 함께 맞춘다 | `search.rs:194-207`, `search.rs:236-260`, `search_bar.rs:221-225` |
| 2 | 수정 | vim 오퍼레이터 대기 중 `n`·`N`·`cmd-g`가 결정 4절 4(「대기를 끝내고 이동만 한다」)와 달리 키를 삼키고 「오퍼레이터를 취소했다」만 남긴다. 가로채기의 허용 목록 `allowed_while_pending`에 `search::Next`·`Prev`가 없어서 `let_through_vim`이 `cancel_operator` 뒤 `false`(삼킴)를 돌려준다. 결정을 고칠지 동작을 고칠지 정하고 시험으로 고정한다(계획 표에 이 시험이 없다) | `vim.rs:108-138`, `vim.rs:404-416`, 결정 4절 4 |
| 3 | 수정 | 막대가 열린 채 전역 `ctrl-tab`(`mode::Cycle`)이 창 루트에 닿아 모드가 Step으로 바뀌고 막대는 남는다. 그 뒤 `Enter`·‹ › 버튼은 `search_confirm` → `search_jump`로 모드를 다시 보지 않고 Step 커서를 옮긴다(결정 1절 2 위반). 팔레트는 같은 문제를 `command_palette.rs:247-250`에서 막았다(리뷰 선례). 막대에도 같은 막기를 두거나, `search_confirm`에서 `search_allowed`를 다시 본다. 정보 팝업은 읽기 전용이라 해가 없다 | `search_bar.rs:164-183`, `search_bar.rs:305-325`, `window.rs:882-883` |
| 4 | 수정 | 지난 검색어가 잘못된 것(예: `b9`를 `Enter`로 확정)일 때 `search::Next`는 `clear_operator` 전에 돌아가 오퍼레이터 대기를 남긴다. 결과 없음 경로는 대기를 끝내는데 오류 경로만 다르다(결정 4절 4). 지적 2를 고치면 이 경로도 함께 시험한다 | `search_bar.rs:187-193`, `search_bar.rs:211` |
| 5 | 수정 | 누락 시험: 막대의 포커스 이탈 닫기(결정 1절 7), `?` 오버레이·팝업 메뉴와 막대의 팝업 하나, 배치 뷰 검색, vim 비주얼 중 `n`, 다른 모드에서의 정보 팝업(결정 6절 5), 막대가 열린 채 문서가 바뀔 때 결과 수 다시 세기. 리뷰 시험 파일에 앞의 다섯을 썼다(실행 못함). 막대 결과 수 다시 세기는 시험이 없다 | `tests/search.rs`, `tests/note_info.rs` |
| 6 | 제안 | vim 비주얼 중 다른 트랙의 결과로 가면 기준 칸(이전 트랙의 틱·음높이)이 새 트랙에 그대로 적용되어 `recompute_visual`이 새 트랙에서 선택을 만든다. 마크 이동(`` ` ``)과 같은 동작이라 이 Task만의 문제는 아니지만, 결정 4절 2 「선택은 바꾸지 않는다」와 「비주얼은 다시 계산한다」가 트랙을 넘을 때 무엇인지 결정 문서에 한 줄 적기를 권한다 | `search_bar.rs:218-232`, `vim.rs:954-980` |
| 7 | 제안 | 결정 2절 2 「감싼 안이 비면 결과가 없다」: `"  "`(공백만)은 빈칸이 아니라 공백 두 칸 글자로 찾는다. 의도면 결정 문서에 적는다 | `search.rs:50-60` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 1건(지적 1)과 수정 4건. 재리뷰 전에 구현자는 디스크를 비우고 `review_s3_t7_r1.rs` 전부를 돌려 결과를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(순수 계산 셋. 창 시험은 디스크 부족으로 실행 못함, 0절)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`window.rs` 문서 관찰자 본문을 `document_changed`로 옮긴 것은 동작 같은 정리로 보고서에 적혀 있다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`proptest` dev-dependency, 워크스페이스에 이미 있는 MIT/Apache-2.0)
- [x] 스타일 지적을 적지 않았다
