# 적대적 리뷰: S3-T3c vim 오퍼레이터·텍스트 객체·비주얼 선택·카운트 (리뷰 4회차, 좁은 확인 리뷰)

- 리뷰어 세션: 구현자와 다른 새 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트)
- 대상 커밋: `a6cbdae`(브랜치 `s3/t3c-vim-operators`). 3회차 뒤 수정 `4f2d301`(지적 1, 붙든 경로를 놓는 때), `a6cbdae`(보고·계획 2절·3회차 시험 커밋과 기록 단언 하나 뒤집기)
- 범위: 결정 `docs/decisions/2026-10-27-s3-t3c-review-limit.md` A에 따라 리뷰 3 지적 1(가·나)의 수정만. 지적이 고쳐졌는가, 그리고 이 수정이 회귀·결정·계획 위반·결함을 만들었는가. 그 밖은 「제안」으로만 적는다
- 깨뜨리기 시험: `crates/keyscore-app/tests/review_s3_t3c_r4.rs`(14개, 모두 통과. 픽스처는 r3에서 복사)
- 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t3c_r4 --test review_s3_t3c_r3 --test review_s3_t3c_r2 --test review_s3_t3c --test vim_operators --test review_s2_t2_r2 --test review_s2_t2_r3 --test keymap_engine` → r4 14, r3 20, r2 19, 1회차 19, `vim_operators` 23, `review_s2_t2_r2` 23, `review_s2_t2_r3` 22, `keymap_engine` 11 모두 통과. `cargo clippy -p keyscore-app --test review_s3_t3c_r4 -- -D warnings`와 `rustfmt --check`는 깨끗하다. 워크스페이스 전체 시험과 MIDI·재생 시험은 돌리지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show 4f2d301`(`src/vim.rs`의 `vim_pending_input_changed`·`release_stale_prefix`, `src/window.rs`의 `observe_pending_input` 구독과 `render`의 호출), 주변의 `vim_keystroke`·`flush_held_then`·`key_context_path`, `assets/keymaps/vim.json`, GPUI 스냅샷(`gpui-pre 0.3.8` `window.rs`의 `dispatch_key_event`·`pending_input_changed`·시간 초과)을 읽고 시험을 쓰기 전에 적었다.

1. 그리기의 비우기는 「붙든 경로 ≠ 지금 경로」일 때만 돈다. 정상 시퀀스 도중(`d i`, `v i`, `g`, `y`, `2 d i`) 그리기가 끼어도 대기 입력을 비우지 않는가.
2. 비우기가 붙든 경로에서 재생하는 접두가 단독 바인딩을 가지면(비주얼의 `y`는 `y y`의 접두이자 `operator::Yank`) 그 Action이 새는가. `release_stale_prefix`가 켜는 `ignore_replayed_operator`가 재생된 Yank만 삼키고 다음 오퍼레이터는 삼키지 않는가.
3. 창이 더러운 채 키가 오면 `dispatch_key_event`가 먼저 그린다. 그 그리기의 비우기 지연과 가로채기의 `flush_held_then`(비우기·재전송)이 한 키 안에서 겹친다. 재전송 키가 두 번 가거나, 무시 표시가 재전송 `d`를 삼키지 않는가.
4. `release_scheduled`가 그리기마다 지연을 다시 걸지 않게 막는가. 지연이 돈 뒤 플래그가 풀리는가(풀리지 않으면 다음 낡은 접두를 못 비운다).
5. `observe_pending_input`은 이 창의 대기 입력만 본다. 다른 창의 정상 대기 입력을 비우지 않는가.
6. (가) 시간 초과로 비워질 때 GPUI는 관찰자를 부른 뒤 재생한다. 재생이 그린(붙든) 경로에서 맞고 붙들기가 놓이는가. 대기 중 `shift` 단독은 접두만 비우고 오퍼레이터 대기는 남기는가.
7. 관찰자는 `dispatch_key_event` 안에서 동기로 창 엔티티를 갱신한다. 창 엔티티를 빌린 채 키를 디스패치하는 곳이 있으면 재진입한다.
8. 카운트 뒤 접두(`d 3 i`)가 낡을 때 카운트가 새지 않는가. `y` 접두 재전송(`y i n`)이 낡은 붙든 경로 뒤에도 맞는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 정상 시퀀스 사이의 그리기 | 부분 | r3 `normal_sequences_after_held_path`는 키 사이에 그리기를 끼우지 않는다(헬퍼 `press`의 그리기 하나). 리뷰에서 더함 |
| 2 재생되는 단독 바인딩 접두 | 없음 | 기존 재현은 모두 `null` 접두(`i`). 리뷰에서 비주얼 `y`로 더함 |
| 3 그리기·가로채기 비우기의 겹침 | 없음 | 기존 시험은 메뉴·클릭 뒤 늘 그린다. 리뷰에서 그리지 않는 길로 더함 |
| 4 지연 반복 | 간접 | 직접 관찰 수단 없음. 여러 번 그린 뒤 동작으로 확인 |
| 5 다른 창 | 없음 | 리뷰에서 더함 |
| 6 시간 초과·수식키 | 부분 | r3 `shift` 재현 넷(이제 통과). 시간 초과·대기 유지는 리뷰에서 더함 |
| 7 재진입 | 코드 읽기 | `dispatch_keystroke`·`flush_pending_keystrokes` 호출은 모두 `window.defer` 안, 엔티티 갱신 밖이다(`src/vim.rs:229`, `:345`, `:421`, `:446`, `src/keymap/mod.rs:362`) |
| 8 카운트·`y` 재전송 | 없음 | 리뷰에서 더함 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `d i` 뒤 두 번 그리고 `n` | 못 깨뜨림(대기 입력·`Operator` 경로 유지, D4 삭제, 끝난 뒤 경로 평범) | `delete_inner_note_survives_renders_between_keys` |
| 2 | `v i` 뒤 그리고 `b` | 못 깨뜨림(마디 선택, 비주얼 유지) | `visual_inner_bar_survives_renders_between_keys` |
| 3 | `g`·그리기·`g`, `y`·그리기·`y`·그리기·`p`, `2 d i`·그리기·`n` | 못 깨뜨림 | `normal_sequences_with_renders_between_keys` |
| 4 | `v i` 중 메뉴 `select::All`, Esc, `y i n`(`y` 접두 재전송) | 못 깨뜨림(그리기가 낡은 접두를 비움, 커서 노트 복사, 문서 그대로) | `yank_prefix_resend_after_stale_visual_prefix` |
| 5 | `v y`(대기) 중 메뉴 `select::All` | 못 깨뜨림(재생된 Yank가 삼켜져 전체 선택을 복사하지 않음, 이어 `d`는 대기에 듦) | `visual_yank_prefix_then_menu_select_all_does_not_copy` |
| 6 | `v y` 중 메뉴 `view::Toggle`, 이어 `enter` | 못 깨뜨림(알림·복사 없음, 배치 뷰 경로 평범, `clip::Open`) | `visual_yank_prefix_then_menu_view_toggle_then_enter` |
| 7 | `d g` 중 메뉴 `view::Toggle` 두 번, `l` | 못 깨뜨림(`g` 재생은 아무것도 안 함, 한 칸) | `delete_g_prefix_then_menu_view_toggle_flushes_quietly` |
| 8 | `d 3 i` 중 클릭, `l` | 못 깨뜨림(카운트 비움, 한 칸) | `delete_count_object_prefix_then_click_then_l_moves_once` |
| 9 | `d i` 중 그리지 않는 클릭, 곧바로 `a` | 못 깨뜨림(그리기 비우기와 `flush_held_then`이 겹쳐도 `a`는 한 번, Step) | `object_prefix_then_click_without_render_then_a_enters_step` |
| 10 | 9와 같고 `d w` | 못 깨뜨림(C4 삭제, 무시 표시가 재전송 `d`를 삼키지 않음) | `object_prefix_then_click_without_render_then_delete_word` |
| 11 | `v y` 중 그리지 않는 `view::Toggle` 두 번, 곧바로 `d w` | 못 깨뜨림(재생된 `y` 무시, 재전송 `d` 대기, 하나 삭제) | `visual_yank_prefix_then_toggles_without_render_then_delete` |
| 12 | `v y` 대기 시간 초과, `i` | 못 깨뜨림(관찰 → 붙들기 놓음, 재생 Yank가 선택 복사, 경로 평범, Step) | `visual_yank_prefix_timeout_releases_held_path` |
| 13 | `d i` 뒤 `shift` 단독, `w` | 못 깨뜨림(접두만 비워지고 대기·`Operator` 경로 유지, `d w`) | `object_prefix_shift_keeps_operator_pending` |
| 14 | 창 A·B. B `d i` 대기, A `d i` 뒤 `view::Toggle` | 못 깨뜨림(A만 비워짐, B의 대기 입력 유지, B `n`이 B에서만 지움) | `stale_prefix_release_does_not_touch_other_window` |

3회차 실패 재현 넷(`object_prefix_then_click_then_shift_then_i_enters_step`, `..._i_n_enters_step`, `object_prefix_then_menu_view_toggle_then_shift_then_enter_opens_clip`, `object_prefix_then_menu_view_toggle_draws_arrangement_context`)은 수정 뒤 통과한다.

시험 전제 하나를 바로잡았다: 비주얼 중 피아노롤 클릭은 비주얼을 끝내지 않고 커서만 옮긴다(`click_piano_roll_cell`은 `clear_operator`만 부른다). 처음 쓴 두 시험이 이 전제로 실패해 비주얼을 끝내는 길을 메뉴(`select::All`, `view::Toggle`)로 바꿨다. 수정과 무관한 기존 동작이다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 키 컨텍스트 경로의 `Operator`·`Visual`은 Normal + 피아노롤일 때만 붙는다 | 없다(한 프레임 예외는 제안 1) | 시도 6·7·14, r3 재현 통과. `release_stale_prefix`(`src/vim.rs:216`)가 그리기에서 비우기를 걸고 지연 뒤 알림으로 다시 그린다 |
| 그린 경로는 `context_path()`와 같거나, 대기 입력이 남은 동안만 붙든 경로다 | 없다 | `vim_pending_input_changed`(`src/vim.rs:205`)가 GPUI의 모든 비우기 길(일치·불일치·수식키·시간 초과)에서 불린다(시도 12·13, r3 `shift` 재현) |
| 비우기가 재생하는 접두는 아무 일도 하지 않는다 | 없다 | `null` 접두(`i` `a`), 바인딩 없는 접두(`g`), 오퍼레이터 접두(`y`, 무시 표시) 모두(시도 5·6·7·11) |
| 정상 시퀀스의 대기 입력은 그리기에 비워지지 않는다 | 없다 | 붙든 경로 = 지금 경로면 `release_stale_prefix`가 바로 돌아온다(시도 1–3) |
| 대기가 끝나면 카운트도 비운다 | 없다 | 시도 8 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8.1 `d` `y` `c` 오퍼레이터, 텍스트 객체 `dac` `yib` `vin` | 충족 | 시도 1·2·3·4·10·11, `vim_operators` 통과 |
| 5.8.1 `v` `V` `Ctrl+v` 비주얼 | 충족 | 시도 2·5·6·12 |
| 5.8.1·5.7 숫자 카운트 접두 | 충족 | 시도 3·8 |
| 5.8.1 Normal 키(`i` `a` Step 진입)와 배치 뷰 `enter` | 충족 | 3회차 「부분 충족」이 r3 재현 통과와 시도 6·9·12로 풀렸다 |
| KM-5, P8 | 충족 | `keymap_engine`, `review_s2_t2_r2`·`r3` 통과 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(수정은 UI 스레드의 관찰자·그리기뿐).
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 없음.
- 에러가 삼켜지는 곳: `release_stale_prefix` 지연의 `let _ = me.update(...)`는 창이 닫힌 경우라 정당하다(`flush_held_then`과 같다).

## 7. 지적 사항

「수정」 이상의 지적은 없다.

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 비우기를 거는 그리기 자체는 아직 붙든 경로로 그린다(재생을 그 경로에서 맞추려는 의도). 그래서 키 밖에서 뷰·모드가 바뀐 직후 한 프레임은 배치 뷰·Step이 `Operator`·`Visual` 경로로 그려지고, 같은 효과 주기의 지연과 알림 뒤 다음 프레임에서 풀린다. 그 사이 온 키는 `flush_held_then`이 받으므로 동작은 맞다(시도 9–11). 계획 2절 「다음 그리기에서 곧바로 붙든 경로로 비운 뒤 놓는다」에 「그 한 프레임은 붙든 경로로 그린다」를 덧붙이면 불변식과의 관계가 분명해진다 | `crates/keyscore-app/src/vim.rs:212`–`238`; `docs/reports/s3-t3c/plan.md:30` |
| 2 | 제안 | `release_stale_prefix`는 재생될 접두가 오퍼레이터인지 보지 않고 `ignore_replayed_operator`를 켠다. 지금은 지연 안에서 내리고, 그 사이 온 키는 `flush_held_then`이 가로채므로 새는 길이 없다(시도 11). 다만 이 안전은 「붙든 경로가 낡은 동안 오는 키는 모두 가로채기에서 비우기 길로 간다」에 기대고 있으니 주석에 그 전제를 적어 두면 좋다 | `crates/keyscore-app/src/vim.rs:226`–`227` |
| 3 | 제안 | 관찰자는 `dispatch_key_event` 안에서 동기로 창 엔티티를 갱신한다(`gpui-pre` `window.rs`의 `pending_input_changed`). 지금 키 디스패치는 모두 엔티티 갱신 밖의 `window.defer`에서 하므로 재진입이 없다. 앞으로 창 엔티티를 빌린 채 `dispatch_keystroke`·`flush_pending_keystrokes`를 부르면 관찰자의 `update`가 실패한다는 점을 `flush_pending_keystrokes` 주석에 적어 두면 좋다 | `crates/keyscore-app/src/window.rs:168`–`170`; `crates/keyscore-app/src/keymap/mod.rs:255` |

3회차 지적의 수정 확인:

- 지적 1 (가)(`4f2d301`): 맞다. `observe_pending_input`로 대기 입력이 없어지면 붙들기를 놓고 알린다. 수식키 단독·시간 초과·정상 완료 모두 이 길을 탄다(r3 `shift` 재현 셋 통과, 시도 12·13).
- 지적 1 (나)(`4f2d301`): 맞다. 키 밖에서 대기·비주얼이 끝나면 그리기에서 붙든 경로로 비우고 놓는다. 배치 뷰·Step이 다음 프레임부터 평범한 경로로 그려지고(r3 `..._draws_arrangement_context` 통과, 시도 6·7), 재생은 붙든 경로에서 맞아 Action이 새지 않는다(시도 5·6·7·11). `release_scheduled`로 지연은 한 번만 걸리고 지연 안에서 풀린다(시도 1–3에서 이어지는 대기·비우기가 맞다).
- `a6cbdae`의 단언 뒤집기: `object_prefix_then_menu_step_draws_operator_context_records`는 3회차에 「기록」으로 지금 동작(Step이 `Operator`로 그려짐)을 단언한 시험이고, 그 동작은 3회차 지적 1 (나)의 위반 그 자체였다. 수정 뒤 동작(경로에 `Operator` 없음, 대기 입력 없음, Esc 뒤도 없음)으로 뒤집은 것은 계획 불변식과 맞고, 단언을 약하게 하지 않았다. 수긍한다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 3 지적 1(가·나)이 고쳐졌고, 3회차 재현 넷이 통과한다. 수정이 만든 회귀·결정 위반·계획 위반·결함을 찾지 못했다: 정상 시퀀스(`g g`, `y y p`, `d i n`, `v i b`, `2 d i n`, `y i n` 재전송)는 그리기를 사이에 끼워도 맞고, 그리기에서 거는 지연은 한 번만 걸리며 다른 창에 영향을 주지 않고, 비우기가 재생하는 Action(`null`, 바인딩 없는 `g`, 오퍼레이터 `y`)은 새지 않는다. 제안 1–3은 거절할 수 있다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(3회차 뒤 변경은 `src/vim.rs`, `src/window.rs`, `tests/review_s3_t3c_r3.rs`, `docs/reports/s3-t3c/`의 계획·보고뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(r3 단언 하나는 수정 뒤 동작으로 뒤집혔고 위에서 수긍)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
