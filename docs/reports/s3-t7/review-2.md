# 적대적 리뷰: S3-T7 검색과 노트 정보 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트, 구현 컨텍스트를 물려받지 않음. 리뷰 1 리뷰어와도 다른 세션)
- 대상 커밋: `bc2fa3c` (`git diff main...HEAD`)
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t7_r2.rs`
- 실행 환경: `CARGO_TARGET_DIR=target/review`, `-p keyscore-app --test <이름>`만 돌렸다. MIDI·재생 시험과 전체 시험은 돌리지 않았다

## 0. 리뷰 1 지적의 확인

| 리뷰 1 지적 | 확인 | 근거 |
| --- | --- | --- |
| 1 차단: 노트 없는 트랙 결과에서 「이전」이 갇힘 | 고쳐짐 | `search.rs:250-256` `relation`이 노트 없는 결과를 (틱, 트랙)으로만 비교한다. 그 트랙에는 노트 결과가 없으므로 순서가 섞이지 않는다. `review_s3_t7_r1.rs` 13개 통과, 이번 회차의 같은 틱 빈 트랙 여럿·클립 없는 트랙(`step_cycles_through_several_empty_track_hits`)과 커서 음높이를 그대로 두는 속성 시험(`step_with_kept_cursor_pitch_visits_every_hit_once`)도 통과 |
| 2 수정: 오퍼레이터 대기 중 `n`·`cmd-g`가 삼켜짐 | 고쳐짐 | `vim.rs:138-141`, `search_bar.rs:194`. 리뷰 1 창 시험 통과 |
| 3 수정: 막대가 열린 채 모드 변경 뒤 `Enter` | 고쳐짐 | `search_bar.rs:174-179`, `search_bar.rs:334-335`. `bar_does_not_search_after_mode_change` 통과 |
| 4 수정: 잘못된 지난 검색어가 대기를 남김 | 고쳐짐 | `search_bar.rs:192-200` 첫머리 `clear_operator`. `invalid_last_query_still_ends_operator_wait` 통과 |
| 5 수정: 빠진 시험 | 고쳐짐 | `tests/search.rs` 17개(4개 추가), 리뷰 1 시험 13개 모두 통과 |
| 6·7 제안 | 반영 | 결정 2절 2, 4절 2 |

리뷰 1 시험 파일은 이번에 실제로 돌렸다(리뷰 1은 디스크 부족으로 창 시험을 못 돌렸다): `review_s3_t7_r1` 13 통과, `search` 17 통과, `note_info` 5 통과, `search_pure` 10 통과.

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 팔레트에서 「검색」·「노트 정보」를 실행하는 길: `close_palette`가 문서로 포커스를 돌린 뒤 Action을 디스패치한다. 그 순서에서 막대 입력란·정보 팝업의 포커스 이탈 구독이 바로 막대·팝업을 닫지 않는가.
2. 리뷰 1 지적 1 수정(`step`의 `relation`): 노트 없는 트랙 결과를 (틱, 트랙)으로만 비교할 때, 그 트랙 위에서 앞뒤 결과와의 순서가 섞이지 않는가. 노트 없는 결과가 여럿(이름이 같은 빈 트랙 둘, 같은 틱)일 때 앞·뒤로 모두 도는가.
3. 리뷰 1 지적 3 수정: 막대가 `mode::Cycle`만 삼킨다. 막대가 열린 채 다른 길(메뉴 `mode::*`, 팔레트 아님)로 모드가 바뀐 뒤 ‹ › 버튼·`Enter`가 막대를 닫고 커서를 두는가.
4. vim 카운트 `3 n`: 카운트를 쓰는가(3번째 결과), 버리는가. 결정에 적혀 있는가.
5. 노트 정보 cent 표시: 반음의 1/200보다 작은 오프셋이 「+0 cent」로 보이지 않는가(`SEMITONE = 1 << 25`).
6. 배치 뷰에서 `i`: 배치 뷰에는 음높이 커서가 보이지 않는데 숨은 피아노롤 커서 음높이의 노트 정보를 보이지 않는가.
7. 같은 클립에 시작·음높이가 같은 노트 둘(겹친 노트)이 있으면 결과 수가 하나로 줄지 않는가.
8. 막대가 열린 채 `Tab`: 입력란에서 버튼으로 포커스가 가면 포커스 이탈로 막대가 닫히는가.
9. 막대 `Enter`가 막대를 닫은 뒤 같은 `Enter` 키가 문서의 `enter` 바인딩(vim 피아노롤 `note::Audition`, 기본 `clip::Open`)으로 새지 않는가.
10. 지난 검색어가 따옴표 검색어(`"c#4"`)일 때 막대를 다시 열면 그대로 채워지고 같은 결과를 내는가.
11. 결정과 계획의 범위: 단계 파일 S3-T7 범위 「`n`/`Shift+n` 이동」과 S3-T3c 넘김 「검색 모션은 S3-T7」을 결정이 어떻게 다루는가.
12. 보고서가 완성되었는가(자동 게이트 결과 칸).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 팔레트로 「검색」·「노트 정보」 | 없음 | `search.rs`·`note_info.rs`는 `palette::Open`으로 닫히는 것만 보고, 팔레트에서 실행하는 길은 없다. 리뷰 1 `note_info_opens_in_step_mode_and_returns_focus`는 Action을 직접 디스패치한다 |
| 2 노트 없는 결과 여럿 | 부분 | 리뷰 1 시험은 빈 트랙 하나. `search_pure.rs`의 속성 시험은 노트 없는 결과를 만들지 않고 커서를 결과 음높이로 옮긴다(창의 동작과 다름) |
| 3 다른 길의 모드 변경 | 있음 | `bar_does_not_search_after_mode_change`(`mode::Step` 디스패치 뒤 `Enter`). 버튼 경로는 같은 함수라 따로 보지 않았다 |
| 4 vim 카운트 `3 n` | 없음 | 결정에도 없다 |
| 5 아주 작은 피치 오프셋 cent | 없음 | `info_lines_show_pitch_start_length_channel`은 +25 cent만 |
| 6 배치 뷰 `i` | 없음 | |
| 7 같은 자리의 노트 둘 | 없음 | 결정 3절은 트랙 결과와 노트 결과의 합침만 적는다 |
| 8 막대 안 `Tab` | 없음 | |
| 9 막대 `Enter`가 문서 `enter`로 새는가 | 부분 | `cmd_f_opens_bar_and_enter_jumps_to_next_pitch`는 포커스만 보고 모드를 보지 않는다 |
| 10 따옴표 지난 검색어 | 없음 | 코드 읽기로 확인(`search_confirm`이 바깥 공백만 다듬어 저장) |
| 11 계획 범위 | 해당 없음 | 문서 대조(7절 지적 3) |
| 12 보고서 완성 | 해당 없음 | `report.md:69`가 `GATES_PLACEHOLDER` 그대로다 |
| (KM-14) 막대 안 `cmd-z` | 없음 | `bar_recounts_when_document_changes`는 `cmd-z` 대신 직접 `undo()`를 부른다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 팔레트(`cmd-shift-p`)에서 「검색」 실행: 팔레트가 문서로 포커스를 돌린 뒤 `search::Open`이 와도 막대가 열리고 입력란에 포커스 | 못 깨뜨림 | `palette_search_command_opens_bar_with_focus` |
| 2 | Step 모드에서 팔레트로 「노트 정보」: 팝업이 열리고 포커스를 가지며, `Esc` 뒤 모드는 Step | 못 깨뜨림 | `palette_note_info_in_step_mode_keeps_popup_focused` |
| 3 | 문서 노트 하나를 지운 뒤 막대 입력란에서 `cmd-z`(KM-14) | 못 깨뜨림(문서·undo 기록 그대로) | `bar_cmd_z_does_not_undo_document` |
| 4 | 막대 `Enter`·빈 검색어 `Enter`·`Shift+Enter` 뒤 기본 피아노롤 `enter`(`mode::Step`)로 새는가 | 못 깨뜨림(모드 Normal) | `bar_enter_does_not_leak_to_document_enter` |
| 5 | 막대 안 `Tab` | 못 깨뜨림(막대 열린 채) | `bar_tab_does_not_close_bar` |
| 6 | vim `3 n`(Lead@0 c#에서 세 번째 다음 결과 (BAR+960, 49)) | 깨짐. 카운트를 버리고 한 칸만 간다: (480, 37) | `vim_count_n_moves_count_results` |
| 7 | 같은 틱 노트 없는 트랙 둘 + 클립 없는 트랙을 커서 음높이 72 그대로 앞뒤로 돌기 | 못 깨뜨림 | `step_cycles_through_several_empty_track_hits` |
| 8 | 속성 시험: 트랙 1–4개(클립 없음/빈 클립/노트 0–3개), 임의 시작 자리, 노트 없는 결과는 커서 음높이 유지. 결과 수만큼 걸으면 모두 한 번, 돎 1회 이하 | 못 깨뜨림(256 사례) | `step_with_kept_cursor_pitch_visits_every_hit_once` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 검색·정보 팝업은 문서·undo·`.`을 바꾸지 않는다 | 못 찾음 | 막대 안 `cmd-z`도 입력란의 것(시도 3). `search_jump`는 `seal_history`·`set_current_track`·`jump_cursor`만 |
| 팝업은 하나 | 못 찾음 | 여는 곳 여섯이 모두 나머지를 비운다. 팔레트 실행 경로도 열린 것이 하나(시도 1·2) |
| `step`은 지금 자리의 다음·이전을 고르고 결과 수만큼 걸으면 모두 지난다 | 못 깨뜨림 | 시도 7·8 |
| 검색은 Normal 모드에서만 커서를 옮긴다 | 못 찾음 | `search_allowed`, `search_confirm`의 재확인 |
| core 모델 불변식 | 해당 없음 | 문서를 고치지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8 `Cmd+F` 검색(`c#4`, 트랙 이름, 가사) | 충족 | `search.rs:47-90`, `search_bar.rs:75-130`, `standard.json:22` |
| 5.8 `i` 커서 노트 정보 팝업(틱·길이·채널) | 충족 | `note_info.rs:18-69`, `note_info.rs:141-171`, `default.json:52` |
| 5.8.1 vim `/` | 충족 | `vim.json:166` |
| 5.8.1 vim `n`·`N` | 충족 | `vim.json:167-168`, `vim.json:247`, `search.rs:236-291` |
| 5.8.1 머리말 「반복은 숫자 카운트 접두로 한다」(`n`에 대해) | 미충족 | `search_bar.rs:250-268`이 `keymap::take_count`를 부르지 않는다. 시도 6 |
| 5.8.1 vim `K` | 충족 | `vim.json:253`, `vim.json:229` |
| 7.5 `Cmd+F` | 충족 | `standard.json:22` |
| KM-14 텍스트 입력 중 `Cmd+Z` | 충족 | 시도 3 |
| P2 `Esc`는 통상의 취소 | 충족 | `search_bar.rs:293-295`, `note_info.rs:242-244` |
| 7절(786행) 모달 다이얼로그 없음 | 충족 | 막대·팝업 모두 창 안 `deferred` 패널 |
| UI-11 | 충족 | 메뉴 편집 > 검색(`commands/table.rs:1226`), 막대 ‹ › 버튼, 팔레트 |

「부분 충족」은 미충족으로 센다: 카운트 하나.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(`git diff main...HEAD`에 `keyscore-midi`·엔진 변경 없음)
- 제품 코드의 `unwrap`/`expect`: `search.rs:126` `expect("0–11")` 하나. `rem_euclid(12)`라 정당하다. `search_bar.rs:222` `hits[found.index]`는 `step`이 `hits`의 순번만 돌려준다
- 에러가 삼켜지는 곳: 없음. `Query::parse` 오류는 막대 글과 상태줄로 나간다

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 보고서가 완성되지 않았다. 「자동 게이트」 절이 `GATES_PLACEHOLDER` 그대로이고, 「테스트 결과」의 수가 리뷰 1 대응 전 것이다(`search: 13 passed`인데 지금 17개, 리뷰 시험 결과 줄 없음). 병합 조건(자동 게이트 통과)의 증빙이 없다. 네 게이트를 돌려 결과를 적는다 | `docs/reports/s3-t7/report.md:28-31`, `report.md:69` |
| 2 | 수정 | vim `3 n`이 카운트를 버리고 한 칸만 간다. REQ 5.8.1은 vim 프리셋의 반복을 숫자 카운트 접두로 하고, 이동 카운트(`3 l`)는 S3-T3c에서 이미 동작한다. 결정 4·5절은 카운트를 적지 않는다. `search_repeat`에서 `take_count`로 그만큼 걷거나(vim과 같음), 카운트를 받지 않는다면 결정에 그 까닭을 적고 시험으로 고정한다 | `search_bar.rs:250-268`, 결정 4·5절 |
| 3 | 제안 | 검색 모션의 넘김이 끊겼다. 단계 파일의 S3-T3c 기록은 「검색 모션은 S3-T7」인데, S3-T7 계획·결정은 범위 밖(「필요하면 후속 Task」)으로 두고 단계 파일에 넘길 곳을 적지 않았다. 단계 파일 S3-T7 범위의 「`n`/`Shift+n` 이동」도 기본 프리셋에서는 `Cmd+G`로 바뀌었다(결정 5절 3). 단계 파일에 두 가지를 한 줄씩 적기를 권한다 | `plan/stages/13-S3-step-and-pianoroll.md:48`, `:99`, `docs/reports/s3-t7/plan.md:83` |
| 4 | 제안 | 반음의 1/200보다 작은 피치 오프셋(`units` < 2^25/200)은 「+0 cent」로 보인다. 오프셋이 있다는 것은 맞지만 값이 0처럼 읽힌다. 소수 cent를 보이거나 「≈0 cent」 같은 표시를 결정에 적기를 권한다(코드 읽기, 시험 없음) | `note_info.rs:81-82` |
| 5 | 제안 | 배치 뷰에서도 `i`가 숨은 피아노롤 커서 음높이의 노트 정보를 연다(`note_under_cursor`는 뷰를 보지 않는다). 배치 뷰에는 음높이 커서가 보이지 않아 「커서 노트」가 무엇인지 사용자가 알 수 없다. 결정 6절 1에 배치 뷰의 뜻을 한 줄 적기를 권한다(코드 읽기, 시험 없음) | `note_info.rs:147`, `selection.rs:121-127` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1의 차단 1건과 수정 4건은 모두 고쳐졌고 리뷰 1 시험 13개가 통과한다. 이번 회차의 차단은 없다. 수정 2건(보고서의 자동 게이트 미기재, vim `n` 카운트)이 남는다. 지적 1은 문서만, 지적 2는 동작 또는 결정 한 줄이라 재리뷰는 그 둘의 좁은 확인이면 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`review_s3_t7_r2` 8개: 7 통과, 1 실패 `vim_count_n_moves_count_results`)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`window.rs`의 `document_changed` 추출은 동작 같은 정리로 보고서에 적혀 있다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`proptest` dev-dependency, 워크스페이스에 이미 있는 MIT/Apache-2.0)
- [x] 스타일 지적을 적지 않았다
