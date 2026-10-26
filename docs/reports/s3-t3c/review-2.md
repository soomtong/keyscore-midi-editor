# 적대적 리뷰: S3-T3c vim 오퍼레이터·텍스트 객체·비주얼 선택·카운트 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 새 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트)
- 대상 커밋: `12f75f7`(브랜치 `s3/t3c-vim-operators`). 1회차 뒤 수정 `aeb202d`(지적 3), `df2f893`(지적 2), `9ba9ab1`(지적 1), `68d2127`(1회차 리뷰 시험 파일, `y escape` 단언 하나를 A13에 맞춤), `12f75f7`(보고·계획)
- 범위: 1회차 지적의 수정이 맞는지, 수정이 새 결함을 만들지 않았는지, 같은 갈래(대기·비주얼·선택 상태가 키가 아닌 경로로 어긋남, 대기 입력 비우기의 재생)의 결함
- 깨뜨리기 시험: `crates/keyscore-app/tests/review_s3_t3c_r2.rs`(19개, 7개 실패 = 지적 재현)
- 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t3c_r2 --test review_s3_t3c` → 1회차 파일 19개 모두 통과, 2회차 파일 12 통과·7 실패. `cargo clippy -p keyscore-app --test review_s3_t3c_r2 -- -D warnings`와 `rustfmt --check`는 깨끗하다. 워크스페이스 전체 시험과 MIDI·재생 시험은 돌리지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

세 수정 커밋의 차이와 `src/vim.rs`(가로채기·오퍼레이터·비주얼), `src/selection.rs`(`sync_selection_mode`, `extend`), `src/keymap/mod.rs`(A13 Esc 가로채기, 카운트), `src/dispatch.rs`, `src/pianoroll_window.rs`(클릭)를 읽고 시험을 쓰기 전에 적었다.

1. 지적 1 수정은 `sync_selection_mode`에서 오퍼레이터 대기만 비운다. GPUI의 대기 입력(`d i`의 `i`)은 그대로라 컨텍스트가 `Operator`에서 `Selection`으로 바뀐 뒤 다음 키·Esc에서 `i`가 Normal의 `i`(Step)로 재생되지 않는가. 클릭(`clear_operator`)과 비주얼을 끝내는 키 아닌 경로(`v i` 뒤 팔레트 `select::All`)도 같은가.
2. `sync_selection_mode`의 `has`는 기준 칸만 있어도 참인데 `start_operator`는 노트 선택·비주얼만 본다. 기준 칸만 있는 선택(빈 칸 직사각형) 중 `d`는 대기에 들고 `context_path`는 `Normal > PianoRoll`일 때만 `Operator`를 붙인다. 대기와 선택 컨텍스트가 공존하지 않는가.
3. `clear_operator`(클릭·모드·뷰)는 키맵 카운트를 비우지 않는다(`cancel_operator`·`operator_cancel`은 비운다). `d 3` 뒤 클릭이면 3이 다음 이동으로 새지 않는가.
4. 지적 1 수정이 오퍼레이터 처리 도중 대기를 먼저 지우지 않는가: `run_motion`·`apply_operator`가 `cursor_moved` → `sync_selection_mode`를 부르는 순서(`3 d w`, `v w c`, 메뉴 모션).
5. 지적 2 수정: `d` 대기의 `y escape`는 `d` 대기를 남기는가. 무시 표시(`ignore_replayed_operator`)가 다음 정상 `y`를 삼키지 않는가. 대기 입력이 둘(`y y`, `y y p`의 접두)일 때 Esc는 어떤가.
6. 지적 3 수정: 거부 뒤 선택·비주얼·상태줄이 남고, 커서를 선택의 클립으로 되돌린 `d`가 지우는가. 거부된 `c`는 Step으로 가지 않고 `.` 기억을 바꾸지 않는가. `y`는 클립 밖이어도 복사하는가. 틈에서 시작한 비주얼의 「클립 밖」 거부.
7. 대기 중 선택을 만들지 않는 키 아닌 편집(팔레트 `edit::Paste`)의 대기.
8. 대기 없는 빠른 `y`(대기 입력) 중 팔레트 `select::All`, 이어 `w`.
9. `68d2127`이 1회차 시험의 단언을 약하게 바꾸지 않았는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 키 아닌 경로로 끝난 대기의 대기 입력 재생 | 없음 | 1회차 시험은 대기 입력 없는 `d`만 본다 |
| 2 기준 칸만 있는 선택과 `d` | 없음 | |
| 3 클릭으로 끝난 대기의 카운트 | 없음 | 시험 24(`click_cancels_operator`)는 카운트 없는 `d` |
| 4 수정 뒤 오퍼레이터 처리 순서 | 있음 | 기존 시험 1·5·6, 1회차 시도 3. 리뷰에서 더함(통과) |
| 5 `d y escape`, 무시 표시 수명, `y y escape` | 부분 | `y`(대기 시간 뒤) `y escape`만(1회차 시험). 나머지는 리뷰에서 더함 |
| 6 거부 뒤 이어 쓰기, `c` 거부, `y` | 부분 | 1회차 시험 둘은 「선택이 남는다」만. 나머지는 리뷰에서 더함(통과) |
| 7 대기 중 팔레트 편집 | 없음 | 리뷰에서 더함(기록) |
| 8 `y` 대기 입력 중 팔레트 선택 | 없음 | 리뷰에서 더함(기록) |
| 9 `68d2127` 단언 변경 | 해당 없음 | 바뀐 단언은 `yank_pending_then_y_escape_copies`의 「대기도 끝난다」 하나. A13·계획 2절 27줄(`d i escape`와 같이 시퀀스만 취소)과 맞고, 「복사 없음」 단언은 그대로다. 수긍한다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `d i` 중 팔레트 `select::All`, 이어 `n` | 깨짐: 대기는 끝났지만 남은 대기 입력 `i`가 `Selection` 컨텍스트에서 Normal `i`로 재생되어 Step 모드(경로 `["Step"]`) | `object_prefix_then_menu_select_all_then_n_stays_normal` |
| 2 | 같은 상태에서 `escape` | 깨짐: A13 가로채기의 비우기가 `i`를 재생해 Step 모드 | `object_prefix_then_menu_select_all_then_escape_stays_normal` |
| 3 | `d i` 중 피아노롤 클릭, 이어 `n` | 깨짐: 1과 같다(클릭 경로 `clear_operator`) | `object_prefix_then_click_then_n_stays_normal` |
| 4 | `v i` 중 팔레트 `select::All`(비주얼 끝), 이어 `n` | 깨짐: 1과 같다 | `visual_object_prefix_then_menu_select_all_then_n_stays_normal` |
| 5 | 빈 칸 (0, 50)에서 팔레트 `select::ExtendRight`(기준 칸만, 노트 0) 뒤 `d` | 깨짐: `pending=Some(Delete)`, 경로 `Normal > PianoRoll > Selection`, 상태줄 `NORMAL d` | `anchor_only_selection_then_d_pending_without_operator_context` |
| 6 | 5의 상태에서 `d i b` | 깨짐: `i`가 `Operator`의 `null`이 아니라 Normal의 Step 진입과 맞아 허용 밖 키로 취소. 커서 마디의 노트가 지워지지 않는다 | `anchor_only_selection_then_d_i_b` |
| 7 | `d 3` 뒤 클릭, 이어 `l` | 깨짐: 커서가 (0, 60)에서 (2880, 60)으로 세 칸 간다(대기의 카운트가 샘) | `delete_count_then_click_then_l_moves_once` |
| 8 | `d` 뒤 팔레트 `cursor::NextNote` | 못 깨뜨림(D4 삭제, 대기 끝. 1회차 시도 3 동작 유지) | `menu_motion_still_applies_after_fix` |
| 9 | `3 d w` → `u` → `v w c` | 못 깨뜨림(D4 E4 G4 삭제, 되돌림, D4 E4 삭제 후 Step) | `change_word_still_works_after_fix` |
| 10 | 대기 없는 빠른 `y` 중 팔레트 `select::All`, 이어 `w` | 못 깨뜨림(비운 `y`가 선택에 바로 작용해 6개 복사, `w`는 이동, 대기 없음). 기록 | `yank_prefix_then_menu_select_all_then_w_records` |
| 11 | 대기 중 팔레트 `edit::Paste`(빈 칸) | 기록: 붙여넣기 실행, 대기 남음(`Operator` 경로, 선택 없음 → 불변식 위반 아님). 제안 5 | `pending_then_menu_paste_records` |
| 12 | `d y escape` 뒤 `w` | 못 깨뜨림(`d` 대기 남음, 복사 없음, `w`가 D4 삭제) | `delete_pending_y_escape_keeps_delete` |
| 13 | `y`(대기 시간 뒤) `y escape` 뒤 `y` | 못 깨뜨림(대기 유지 뒤 `y`가 D4 복사, 대기 끝. 무시 표시가 새지 않는다) | `yank_pending_y_escape_then_y_still_works` |
| 14 | Normal `y escape` 뒤 `y y` | 못 깨뜨림(복사) | `normal_y_escape_then_y_y_copies` |
| 15 | `y`(대기 시간 뒤) `y y escape` | 기록: D4 복사, 대기 끝. 제안 4 | `yank_pending_y_y_escape_records` |
| 16 | `v }` `d`(거부) 뒤 `{` `d` | 못 깨뜨림(거부 때 선택·비주얼·`VISUAL` 유지, 대기 없음. 되돌린 뒤 선택을 지움) | `visual_refused_then_back_into_clip_deletes` |
| 17 | `d w` 뒤 `v }` `c`(거부) → `escape` → `.` | 못 깨뜨림(Step으로 가지 않음, `.`은 `d w`) | `visual_refused_change_keeps_state` |
| 18 | `v }` `y` | 못 깨뜨림(클립 밖이어도 선택 복사, 비주얼 끝) | `visual_outside_then_yank_copies` |
| 19 | 틈에서 `v l` `d` → `escape` | 못 깨뜨림(「커서가 클립 밖이다」, 비주얼 유지, Esc로 끝, 경로 `Normal > PianoRoll`) | `visual_in_gap_then_delete_keeps_visual` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 오퍼레이터 대기와 선택·비주얼은 동시에 없다 | 있다 | 기준 칸만 있는 선택(팔레트 `select::Extend*`) 뒤 `d`(시도 5). `start_operator`(`src/vim.rs:386`)는 노트 선택·비주얼만 보고 `sync_selection_mode`의 `has`(기준 칸 포함)와 다르다. 메뉴 `select::All`·`edit::Undo` 경로는 수정으로 막혔다(1회차 시험 통과) |
| 키 컨텍스트의 `Operator`는 대기와 함께 | 있다 | 위 상태에서 경로가 `Selection`(`src/vim.rs:171`). `i b`·`a c`가 취소된다(시도 6) |
| 대기 중 `i` `a` 단독은 아무 일도 하지 않는다(계획 2절) | 있다 | 대기·비주얼이 키 아닌 경로로 끝나면 남은 대기 입력 `i`가 새 컨텍스트에서 Step 진입으로 재생된다(시도 1–4) |
| 비주얼이면 기준 칸이 있다 | 없다 | 거부 뒤에도 기준 칸·비주얼이 함께 남는다(시도 16·19) |
| 문서는 `apply`로만 | 없다 | 거부 경로는 문서를 바꾸지 않는다(시도 16·17) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8.1 `d` `y` `c` 오퍼레이터(`dw`) | 충족 | `src/vim.rs:363`(`start_operator`), 시도 9·12 |
| 5.8.1 텍스트 객체 `dac` `yib` `vin` | 부분 충족 | 키 경로는 충족. 기준 칸만 있는 선택 중 `d i b`가 취소된다(지적 2) |
| 5.8.1 `v` `V` `Ctrl+v` 비주얼 | 충족 | 거부 때 선택 유지(`src/vim.rs:515`, 사람 결정 2026-10-27), 시도 16–19 |
| 5.8.1 `D` `C` | 충족 | 1회차와 같다(변경 없음) |
| 5.8.1 모션 | 충족 | 1회차와 같다. 메뉴 모션 유지(시도 8) |
| 5.8.1·5.7 숫자 카운트 접두 | 부분 충족 | 클릭으로 끝난 대기의 카운트가 다음 이동으로 샌다(지적 3) |
| 5.8.1 `.` 마지막 편집 반복 | 충족 | 거부된 `c`가 기억을 바꾸지 않는다(시도 17) |
| KM-5, P8 | 충족 | 변경 없음(1회차 시험 25) |

「부분 충족」 2건은 지적 2·3으로 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(수정은 UI 스레드 처리기뿐).
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 없음.
- 에러가 삼켜지는 곳: 새로 없음. `ignore_replayed_operator_once`는 지연으로 표시를 내리므로 재생이 오지 않아도 표시가 남지 않는다(시도 13·14).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 오퍼레이터 대기·비주얼이 키가 아닌 경로(팔레트·메뉴의 선택 → `sync_selection_mode`, 피아노롤 클릭 → `clear_operator`, 비주얼을 끝내는 `select::All`)로 끝날 때 GPUI의 대기 입력은 남는다. `d i`·`v i`의 `i`가 남은 채 컨텍스트가 `Operator`·`Visual`에서 빠지면, 다음 키(`n`)나 Esc(A13 가로채기의 비우기)가 `i`를 Normal의 `i`로 재생해 Step 모드로 들어간다. 계획 2절 「대기 중 `i` `a` 단독은 아무 일도 하지 않는다」와 「그 밖의 키는 동작 없이 대기만 취소」 위반이다. 1회차 지적 1 수정은 대기만 비우고 대기 입력을 다루지 않았다(클릭 경로는 1회차부터). 고치는 길 예: 키 아닌 경로가 대기·비주얼을 끝낼 때 대기 입력이 있으면 옛 컨텍스트에서 재생을 무시하며 비운다(`flush_then_cancel`과 같은 순서). `sync_selection_mode`는 `Window`가 없으므로 비우기를 창 지연으로 넘기거나, 대기 입력이 있는 동안에는 대기를 끝내기 전에 비운다. 재현: `object_prefix_then_menu_select_all_then_n_stays_normal`, `object_prefix_then_menu_select_all_then_escape_stays_normal`, `object_prefix_then_click_then_n_stays_normal`, `visual_object_prefix_then_menu_select_all_then_n_stays_normal` | `crates/keyscore-app/src/selection.rs:131`; `crates/keyscore-app/src/vim.rs:198`; `crates/keyscore-app/src/pianoroll_window.rs:118` |
| 2 | 수정 | 기준 칸만 있는 선택(빈 칸 직사각형. vim 프리셋에서는 팔레트 `select::Extend*`로 생긴다) 중 `d`가 대기에 든다. `start_operator`는 노트 선택·비주얼만 「선택」으로 보고, `context_path`는 기본 경로가 `Normal > PianoRoll`일 때만 `Operator`를 붙여, 대기(`NORMAL d`)와 `Selection` 컨텍스트가 공존한다. `d i b`·`d a c`는 `i`·`a`가 Normal 바인딩과 맞아 취소된다. 계획 「불변식」의 「오퍼레이터 대기와 비주얼은 동시에 없다(선택·비주얼이 있으면 대기에 들지 않는다)」 위반이다. 지적 1 수정의 `sync_selection_mode`는 `start_operator` 뒤에 불리지 않아 이 경로를 잡지 못한다. 고치는 길 예: `start_operator`의 판단을 `mode_state().has_selection()`(기준 칸 포함)에 맞추고 그때의 대상(빈 선택이면 커서 노트)을 계획에 적거나, `d`가 대기에 들 때 기준 칸을 지운다 | `crates/keyscore-app/src/vim.rs:386`, `:171`; `crates/keyscore-app/src/selection.rs:128` |
| 3 | 수정 | 클릭으로 끝난 대기의 키맵 카운트가 남는다. `d 3` 뒤 클릭, 이어 `l`이면 세 칸 간다. `clear_operator`(클릭·모드·뷰 전환)는 카운트를 비우지 않고(`cancel_operator`·`operator_cancel`은 비운다), 키맵의 카운트 비우기는 키 관찰에서만 돈다. 계획 2절 「대기는 … 클릭하면 끝난다」에서 오퍼레이터의 카운트(`d 3 w`의 3)는 대기에 딸린 것이라 함께 버려야 한다. 재현: `delete_count_then_click_then_l_moves_once` | `crates/keyscore-app/src/vim.rs:198` |
| 4 | 제안 | `y` 대기(대기 시간 뒤) 중 `y y escape`는 키맵의 A13 비우기가 `y y`(`edit::Copy`)를 실행해 커서 노트를 복사하고 대기를 끝낸다. 같은 대기의 `y escape`(복사 없음, 대기 유지)와 다르다. vim의 `yy`로 보면 맞고 Normal의 `y y escape`도 S3-T2부터 같은 결과라 결함으로 세지 않는다. 계획 2절 29줄에 「대기 입력이 `y y`일 때의 Esc는 `y y`를 실행한다」를 한 줄 적기를 권한다. 기록: `yank_pending_y_y_escape_records` | `crates/keyscore-app/src/vim.rs:268`–`273`; `crates/keyscore-app/src/keymap/mod.rs:355`–`364` |
| 5 | 제안 | 대기 중 팔레트로 선택을 만들지 않는 편집(`edit::Paste` 등)을 실행하면 편집은 되고 대기는 남는다. 이어지는 모션은 편집 뒤 커서에서 작용한다. 불변식 위반은 아니고 계획 2절의 「그 밖의 키」는 키만 말하므로 결함으로 세지 않는다. 대기를 끝내는 때(계획 2절 28줄)에 「키 아닌 경로의 Action」을 더할지 정하기를 권한다. 기록: `pending_then_menu_paste_records` | `crates/keyscore-app/src/vim.rs:339`–`358` |

1회차 지적의 수정 확인:

- 지적 1(`9ba9ab1`): 1회차 재현 둘 통과. 메뉴 모션·`3 d w`·`v w c` 회귀 없음(시도 8·9). 다만 대기 입력과 기준 칸만 있는 선택을 다루지 않아 지적 1·2가 남는다.
- 지적 2(`df2f893`): 맞다. `d y escape`는 `d` 대기를 남기고, 무시 표시는 다음 정상 `y`를 삼키지 않는다(시도 12–14).
- 지적 3(`aeb202d`): 사람 결정 「거부, 선택 유지」대로다. 거부 뒤 이어 쓰기, `c` 거부, `y`, 틈 비주얼 모두 일관된다(시도 16–19).
- `68d2127`: 단언 하나의 변경은 A13·계획 2절과 맞는다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2·3(수정)을 고치고 `review_s3_t3c_r2.rs`의 실패 시험 7개가 통과하면 재리뷰한다. 시험의 기대가 계획과 다르다고 보면(예: 지적 2를 「기준 칸만 있는 선택은 선택이 아니다」로 정리) 계획에 먼저 적고 그 시험의 단언을 그 규칙에 맞춘다. 제안 4·5는 거절할 수 있으며 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(1회차 뒤 변경은 `src/vim.rs`, `src/selection.rs`, `tests/review_s3_t3c.rs`, `docs/reports/s3-t3c/`의 계획·보고뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
