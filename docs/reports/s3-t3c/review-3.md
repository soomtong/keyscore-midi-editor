# 적대적 리뷰: S3-T3c vim 오퍼레이터·텍스트 객체·비주얼 선택·카운트 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 새 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트)
- 대상 커밋: `6b93794`(브랜치 `s3/t3c-vim-operators`). 2회차 뒤 수정 `f4b0b32`(지적 1, 대기 입력 경로 붙들기), `a9445a1`(지적 3, 대기를 끝내는 모든 길이 카운트를 비움), `cbb04d6`(지적 2, 기준 칸만 있는 선택), `6b93794`(보고·계획·2회차 시험 단언)
- 범위: 2회차 지적의 수정만. 수정이 지적을 고쳤는가, 그리고 회귀·결정·계획 위반·결함을 새로 만들었는가. 특히 `held`가 정상 시퀀스를 깨뜨리지 않는지, 붙든 경로가 남아 엉뚱한 컨텍스트로 그려지는 경우가 없는지
- 깨뜨리기 시험: `crates/keyscore-app/tests/review_s3_t3c_r3.rs`(20개, 4개 실패 = 지적 재현)
- 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t3c_r3 --test review_s3_t3c_r2 --test review_s3_t3c --test vim_operators --test review_s2_t2_r2 --test review_s2_t2_r3 --test keymap_engine` → r3 16 통과·4 실패, 나머지 파일 모두 통과(r2 19, 1회차 19, vim_operators, `review_s2_t2_r2` 23, `review_s2_t2_r3` 22, `keymap_engine` 11). `cargo clippy -p keyscore-app --test review_s3_t3c_r3 -- -D warnings`와 `rustfmt --check`는 깨끗하다. 워크스페이스 전체 시험과 MIDI·재생 시험은 돌리지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

세 수정 커밋의 차이와 `src/vim.rs`(`held`, `key_context_path`, `vim_keystroke`, `flush_held_then`, `let_through_vim`), `src/window.rs`(`wrap_in_contexts`, 모드·트랙 전환의 `clear_operator`), `src/selection.rs`(`sync_selection_mode`), `src/keymap/mod.rs`(A13 Esc 가로채기, 카운트 관찰), GPUI 스냅샷(`gpui-pre 0.3.8` `window.rs`의 `dispatch_key_event`·대기 시간 초과·`replay_pending_input`)을 읽고 시험을 쓰기 전에 적었다.

1. `key_context_path`는 `held`와 `has_pending_keystrokes()`만 본다. 대기 입력이 가로채기를 거치지 않는 길(수식키 단독 입력, 시간 초과, 포커스 이동)로 비워지면 알림이 없어 다시 그리지 않는다. 그린 경로가 `Operator`·`Visual`로 남아 다음 키가 그 경로로 맞춰지지 않는가.
2. 붙들기는 모드·뷰를 보지 않는다. `d i` 중 메뉴로 배치 뷰·Step 모드로 가면 `Normal > PianoRoll > Operator`로 그려 계획 「불변식」 「`Operator`·`Visual`은 Normal + 피아노롤일 때만」을 어기지 않는가.
3. `flush_held_then`은 비우기의 알림 뒤 다시 보낸다. 다시 보낸 키가 다시 그린 경로로 맞춰지는가(`dispatch_key_event`가 더러운 창을 먼저 그리는가). 특히 `Operator`와 Normal의 뜻이 다른 `i` `a`.
4. `held`는 가로채기를 통과한 키마다 그 순간의 경로로 다시 정해진다. `v i n` 뒤 Esc처럼 붙든 경로가 남은 다음 Normal 시퀀스(`g g`, `y y p`, `z z`, `' x`, `2 g g`, `m x`)가 깨지지 않는가. 같은 경로 안의 시퀀스(`v g g`, `d g g`)도.
5. 다시 보낸 키가 카운트(`3 l`)·오퍼레이터(`d w`)일 때. `flush_held_then`이 켜는 `ignore_replayed_operator`가 다시 보낸 `d`를 삼키지 않는가.
6. 지적 2 수정: 기준 칸만 있는 선택에서 `3 d`의 카운트·상태줄·경로, `y`(대기 시간 뒤) `y`.
7. 지적 3 수정: `clear_operator`를 거치는 다른 길(뷰 전환, 선택이 생김)도 카운트를 비우는가.
8. 기본 프리셋(멀티키 시퀀스 없음)에서 `held`가 늘 `None`인가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 가로채기 밖에서 비워진 대기 입력 뒤의 그린 경로 | 없음 | 2회차 시험은 다음 키가 일반 키인 경우만 |
| 2 붙든 경로와 모드·뷰 전환 | 없음 | |
| 3 다시 보낸 키의 경로 | 부분 | r2의 `n`은 두 경로에서 뜻이 같아 구별하지 못한다. 리뷰에서 `i` `a` `enter`를 더함 |
| 4 붙든 경로 뒤 정상 시퀀스 | 부분 | 기존 vim_operators·키맵 시험이 시퀀스를 보지만 붙든 경로가 남은 뒤는 아니다. 리뷰에서 더함 |
| 5 다시 보낸 카운트·오퍼레이터 | 없음 | 리뷰에서 더함 |
| 6 기준 칸만 있는 선택의 `3 d`, `y y` | 부분 | r2 시험은 `d`, `d i b`만 |
| 7 다른 길의 카운트 | 부분 | r2 시험은 클릭만 |
| 8 기본 프리셋 | 있음 | `keymap_engine`·`review_s2_t2_r*`(통과). 리뷰에서 그린 경로 하나를 더함 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `d i` 중 클릭 뒤 `i` | 못 깨뜨림(붙든 경로에서 비운 뒤 다시 그린 경로로 `i`가 가서 Step) | `object_prefix_then_click_then_i_enters_step` |
| 2 | `d i` 중 `select::All`, Esc, `a` | 못 깨뜨림(Step) | `object_prefix_then_menu_select_all_then_a_enters_step` |
| 3 | `d i` 중 클릭 뒤 `3 l` | 못 깨뜨림(세 칸, 문서 그대로) | `object_prefix_then_click_then_count_motion` |
| 4 | `d i` 중 클릭 뒤 `d w` | 못 깨뜨림(다시 보낸 `d`가 무시 표시에 삼켜지지 않고 C4 삭제) | `object_prefix_then_click_then_delete_word` |
| 5 | `d i` 중 클릭, `shift` 단독, `i` | 깨짐: `shift`에서 GPUI가 `i`를 붙든 `Operator` 경로로 비우고(알림 없음) 그린 경로가 `[Root, Normal, PianoRoll, Operator]`로 남는다. 대기 입력은 없다. 다음 `i`는 그 경로의 `null`·접두로 맞아 Step으로 가지 않는다 | `object_prefix_then_click_then_shift_then_i_enters_step` |
| 6 | 5에 이어 `i n` | 깨짐: 남은 `Operator` 경로에서 `i n`이 `object::InnerNote`로 맞아 「텍스트 객체(i n, i b, a c)는 오퍼레이터(d y c)나 비주얼(v) 뒤에 쓴다」를 알린다. Normal이면 Step 뒤 `n`(셋잇단) | `object_prefix_then_click_then_shift_then_i_n_enters_step` |
| 7 | `d i` 중 메뉴 `view::Toggle` | 깨짐: 배치 뷰가 `[Root, Normal, PianoRoll, Operator]`로 그려진다(계획 「불변식」 위반) | `object_prefix_then_menu_view_toggle_draws_arrangement_context` |
| 8 | 7에 이어 `enter` | 못 깨뜨림(일반 키는 붙든 경로에서 비운 뒤 다시 그린 배치 경로로 가 `clip::Open`) | `object_prefix_then_menu_view_toggle_then_enter_opens_clip` |
| 9 | 7에 이어 `shift` 단독, `enter` | 깨짐: 5와 같은 길로 피아노롤 경로가 남아 `enter`가 `note::Audition`으로 맞고 피아노롤로 들어가지 않는다 | `object_prefix_then_menu_view_toggle_then_shift_then_enter_opens_clip` |
| 10 | `d i` 중 메뉴 `mode::Step` | 기록: Step 모드가 `Operator` 경로로 그려진다(7과 같은 위반). Esc 뒤에는 풀린다. Step 입력 처리기가 아직 없어 첫 음 키의 차이는 관찰하지 못했다 | `object_prefix_then_menu_step_draws_operator_context_records` |
| 11 | `v i` 중 `select::All`, `shift` 단독, Esc | 못 깨뜨림(남은 `Visual` 경로에서도 Esc는 선택을 지운다) | `visual_object_prefix_then_menu_select_all_then_shift_then_escape_clears` |
| 12 | `v i n` Esc 뒤 `m x` `' x` `z z` `2 g g` `y y p` `i` | 못 깨뜨림(모두 정상, 끝에 Step) | `normal_sequences_after_held_path` |
| 13 | `v g g`, `d g g` | 못 깨뜨림 | `visual_then_g_g_still_moves`, `delete_g_g_still_deletes` |
| 14 | `v i` Esc Esc `i` | 못 깨뜨림(첫 Esc는 시퀀스만, 둘째가 비주얼 끝, `i`는 Step) | `visual_object_prefix_escape_escape_then_i_enters_step` |
| 15 | 기본 프리셋 `cmd-left` `x` | 못 깨뜨림(그린 경로에 `Operator`·`Visual` 없음) | `default_preset_keys_unaffected` |
| 16 | 기준 칸만 있는 선택에서 `3 d` | 못 깨뜨림(`Some((Delete, Some(3)))`, 기준 칸 지움, 경로 `Operator`, 상태줄 `NORMAL 3d`) | `anchor_only_selection_then_count_delete_word` |
| 17 | 기준 칸만 있는 선택(빈 칸)에서 `y`(대기 시간 뒤) `y` | 못 깨뜨림(대기 뒤 끝남, 빈 칸이라 복사 없음, 경로 `Normal > PianoRoll`) | `anchor_only_selection_then_yank_line` |
| 18 | `d 3` 뒤 `view::Toggle` 두 번, `l` | 못 깨뜨림(한 칸) | `delete_count_then_view_toggle_then_l_moves_once` |
| 19 | `3 d` 뒤 `select::All`, Esc, `l` | 못 깨뜨림(한 칸) | `count_delete_then_menu_select_all_then_l_moves_once` |

시도 5·6·9의 `shift` 단독 입력은 macOS에서 Shift를 눌렀다 다른 키 없이 떼면 GPUI가 만드는 키 입력이다(`gpui-pre` `window.rs` `dispatch_key_event`의 수식키 처리). 시험은 `simulate_keystrokes("shift")`로 같은 입력을 보낸다. 가로채기는 `is_modifier_only`로 이 입력을 거르고, GPUI는 `i shift`에 맞는 바인딩이 없어 `i`를 붙든 경로에서 재생(`null`)한 뒤 대기 입력을 비운다. 이때 창 엔티티에 알림이 없어 `key_context_path`가 다시 평가되지 않는다. `dispatch_key_event`는 창이 더러울 때만 먼저 그리므로 다음 키는 낡은 그린 경로로 맞춰진다. `Operator`·`Visual`의 `i` `a`는 단독 바인딩이 없어(`null`) GPUI의 시간 초과는 걸리지 않으므로 이 경우는 수식키·포커스 이동처럼 가로채기 밖의 길에서 생긴다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 키 컨텍스트 경로의 `Operator`·`Visual`은 Normal + 피아노롤일 때만 붙는다(계획 「불변식」) | 있다 | `d i` 중 메뉴 `view::Toggle`·`mode::Step`. `key_context_path`(`src/vim.rs:202`)가 모드·뷰를 보지 않고 대기 입력만 본다(시도 7·10) |
| 그린 키 컨텍스트 경로는 `context_path()`와 같거나, 대기 입력이 남은 동안만 붙든 경로다(계획 2절 「대기 입력이 남아 있는 동안 그 경로로 그린다」) | 있다 | 가로채기 밖에서 대기 입력이 비워지면(수식키 단독) 붙든 경로가 대기 입력 없이 남는다(시도 5·6·9) |
| 대기 중 `i` `a` 단독은 아무 일도 하지 않는다 | 없다 | 2회차 재현 넷 통과. 클릭·메뉴 뒤 다음 키는 붙든 경로에서 비워진다(시도 1–4) |
| 오퍼레이터 대기와 선택·비주얼은 동시에 없다 | 없다 | 기준 칸만 있는 선택은 `start_operator`가 지운다(`src/vim.rs:469`, 시도 16·17) |
| 대기가 끝나면 카운트도 비운다 | 없다 | `clear_operator`(`src/vim.rs:210`)를 클릭·선택·모드·뷰·트랙 전환이 모두 거친다(시도 18·19, r2 시도 7) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8.1 `d` `y` `c` 오퍼레이터(`dw`) | 충족 | 시도 4·13, 기존 시험 |
| 5.8.1 텍스트 객체 `dac` `yib` `vin` | 충족 | 기준 칸만 있는 선택에서도 대기·`Operator` 경로(r2 `anchor_only_selection_then_d_i_b` 통과, 시도 16) |
| 5.8.1 `v` `V` `Ctrl+v` 비주얼 | 충족 | 시도 11·13·14 |
| 5.8.1 모션 | 충족 | 시도 3·12·13 |
| 5.8.1·5.7 숫자 카운트 접두 | 충족 | 시도 3·16·18·19 |
| 5.8.1 `.` 마지막 편집 반복 | 충족 | 변경 없음(r2 시도 17 통과) |
| 5.8.1 Normal 키(`i` Step 진입 등)와 배치 뷰 `enter` | 부분 충족 | 붙든 경로가 남으면 Normal의 `i`, 배치 뷰의 `enter`가 다른 컨텍스트로 맞는다(지적 1) |
| KM-5, P8 | 충족 | `keymap_engine`, `review_s2_t2_r2`·`r3` 통과 |

「부분 충족」 1건은 지적 1로 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(수정은 UI 스레드 가로채기·렌더뿐).
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 없음(`wrap_in_contexts`의 `expect`는 기존, 경로는 비지 않는다).
- 에러가 삼켜지는 곳: `flush_held_then`의 `let _ = me.update(...)`는 창이 닫힌 경우라 정당하다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 붙든 경로가 대기 입력보다 오래, 그리고 Normal 피아노롤 밖에서 그려진다. (가) `key_context_path`는 `held`와 `has_pending_keystrokes()`를 그릴 때만 본다. 대기 입력이 가로채기를 거치지 않는 길(수식키 단독 입력. 가로채기는 `is_modifier_only`로 거른다)로 비워지면 창 엔티티에 알림이 없어 다시 그리지 않으므로 그린 경로가 `Operator`·`Visual`로 남는다. 다음 키는 낡은 경로로 맞아 `d i`·클릭·`shift` 뒤 `i`가 Step으로 가지 않고(`null`·접두), `i n`은 「텍스트 객체는 … 뒤에 쓴다」가 되며, 배치 뷰 `enter`는 `note::Audition`이 된다. 2회차 지적 1이 고친 「키가 아닌 경로로 끝난 대기 뒤 다음 키의 뜻이 바뀐다」와 같은 갈래가 이 수정으로 새로 생겼다. (나) 붙들기가 모드·뷰를 보지 않아 `d i` 중 메뉴 `view::Toggle`·`mode::Step` 뒤 배치 뷰·Step 모드가 `Normal > PianoRoll > Operator`로 그려진다. 계획 「불변식」 「키 컨텍스트 경로의 `Operator`·`Visual`은 Normal + 피아노롤일 때만 붙는다」 위반이다(일반 키는 다음 키에서 비워져 동작은 맞고, (가)와 겹치면 동작도 틀린다). 고치는 길 예: 창이 `cx.observe_pending_input(window, …)`(`gpui-pre` `app/context.rs:521`)로 대기 입력 변화를 보고 알림을 내 `key_context_path`를 다시 평가하게 하고, `held`가 남은 채 대기 입력이 없어지면 `held`를 놓는다. 모드·뷰·트랙이 바뀌는 `clear_operator` 길에서는 대기 입력이 있으면 그 자리에서 붙든 경로로 비우고(`flush_held_then(None, …)`과 같은 순서) 붙들기를 끝내거나, 계획 불변식에 「대기 입력이 남은 동안의 붙든 경로」 예외와 그 끝을 적는다. 재현: `object_prefix_then_click_then_shift_then_i_enters_step`, `object_prefix_then_click_then_shift_then_i_n_enters_step`, `object_prefix_then_menu_view_toggle_then_shift_then_enter_opens_clip`, `object_prefix_then_menu_view_toggle_draws_arrangement_context` | `crates/keyscore-app/src/vim.rs:202`–`207`, `:253`–`256`, `:264`–`275`; `crates/keyscore-app/src/window.rs:881`–`882` |

2회차 지적의 수정 확인:

- 지적 1(`f4b0b32`): 2회차 재현 넷(클릭·`select::All`·비주얼 `select::All` 뒤 `n`, Esc) 통과. 다시 보낸 키는 `dispatch_key_event`가 더러운 창을 먼저 그리므로 새 경로로 맞는다(시도 1–4·8). 정상 시퀀스·같은 경로 안 시퀀스·기본 프리셋 회귀 없음(시도 12–15, 키맵 시험 통과). 다만 붙든 경로를 놓는 때가 「가로채기를 거친 다음 키」뿐이라 위 지적 1이 남는다.
- 지적 2(`cbb04d6`): 맞다. 기준 칸을 지우고 대기에 들며 카운트·상태줄·경로가 맞다(시도 16·17, r2 재현 둘 통과).
- 지적 3(`a9445a1`): 맞다. 클릭·선택·뷰 전환 모두 카운트를 비운다(시도 18·19, r2 재현 통과). `cancel_operator`와 달리 알림(「취소했다」)은 없으며 계획 2절과 맞다.
- 제안 4·5: 계획 2절에 적혔다. 수긍한다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(수정)은 이번 수정이 만든 새 결함이고 계획 불변식 위반이라 병합 조건을 채우지 못한다. 고친 뒤 `review_s3_t3c_r3.rs`의 실패 시험 4개가 통과하면 된다. 재리뷰는 이 지적 하나의 수정(붙든 경로를 놓는 때)으로 좁힌다. 시험의 기대가 계획과 다르다고 보면(예: 배치 뷰·Step의 붙든 경로를 허용하는 예외) 계획 불변식에 먼저 적고 그 시험의 단언을 맞춘다. 그래도 (가)의 `i`·`enter`가 다른 컨텍스트로 맞는 동작은 남겨서는 안 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(2회차 뒤 변경은 `src/vim.rs`, `src/window.rs`, `src/selection.rs`, `tests/review_s3_t3c_r2.rs`, `docs/reports/s3-t3c/`의 계획·보고뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
