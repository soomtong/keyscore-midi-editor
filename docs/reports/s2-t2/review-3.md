# 적대적 리뷰: S2-T2 키맵 엔진 (리뷰 3회차, 확인 리뷰)

- 리뷰어 세션: 구현 세션·리뷰 1·2 세션과 다른 새 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `ed6e391` (브랜치 `s2/t2-keymap`, 리뷰 2 대응 `b9e0231`)
- 범위: 리뷰 2 지적 1–5가 근본까지 고쳐졌는가(A), `review_s2_t2.rs`의 `take_count` → `drain_count` 변경이 의도와 기대값을 지키는가(B), 회귀가 없는가(C). 그 밖은 제안으로만 적는다.
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t2_r3.rs` (24개, 실패 2개는 아래 지적 1·2다. 제안 3·4는 지금 동작을 고정하는 통과 테스트다)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 240d850..HEAD`, `keymap/mod.rs`의 `build`·`install`·`take_count`, GPUI `window.rs`의 `dispatch_key_event`·`replay_pending_input`·`finish_dispatch_key_event`·`dispatch_action`, `keymap.rs`의 `resolve_bindings_for_input`, `app_menu.rs`를 먼저 읽고 적었다.

1. `in_key_dispatch`는 인터셉터가 켜고 관찰자가 끈다. 대기에 들어간 키(`g`)는 관찰자를 부르지 않으므로 대기 중에는 참으로 남는다. 그동안 메뉴로 실행된 처리기가 카운트를 받는가.
2. 지적 2의 시나리오 자체(`3` → 메뉴 → 키 Action): 새 의미에서 메뉴 처리기는 카운트를 받지 않지만 카운트는 남는다. 다음 키 Action이 받는가. 리뷰 2 테스트는 처리기 밖 `take_count`가 늘 `None`인 의미 변경만으로 통과하지 않는가.
3. 대기 시간 만료로 다시 재생되는 짧은 바인딩(`3 y` 1 s)과, 대기가 깨져 재생되는 접두(`3 y w`)에 카운트가 바르게 가는가. 재생 경로는 인터셉터를 다시 부르지 않는다.
4. `build`의 정렬 키 `(layer, 키 개수)`가 레이어를 넘는 접두·시퀀스(사용자 `g` 대 프리셋 `g g`, 사용자 `w w` 대 프리셋 `w`, 사용자 `g g x`), `null`(`y` null, `y y` null)에서 레이어 규칙대로인가.
5. 두 프리셋의 모든 시퀀스에 대해 자기 바인딩이 있는 모든 접두가 대기 뒤 실행되고, 키 사이를 0.9 s로 느리게 쳐도 시퀀스가 닿는가.
6. 충돌 정규화가 시퀀스의 각 키에도 적용되는가(`y Y`/`y shift-y`, `super-k cmd-j`, 공백 두 개).
7. 교환 경고가 별칭(`Alt-s`, `alt-shift-s`, `shift-alt-z`)에도 걸리고, 교환을 끄면 사라지는가. 교환 아래 `super-s`는 실제로 `alt-s`가 되는데 경고가 남는가(거짓 경고).
8. 사용자·프리셋 시퀀스가 표준 키로 시작하면 표준 키가 1 s 기다린다. 이것이 경고되는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 대기 중 메뉴 | 없음 | |
| 2. 메뉴 뒤 다음 키 Action | 없음 | `count_is_cleared_by_menu_dispatched_action`(리뷰 2)은 처리기 밖 `take_count`라 의미 변경 뒤 늘 통과한다 |
| 3. 재생 경로의 카운트 | 부분 | 리뷰 2 `count_survives_pending_sequence_and_reaches_its_handler`는 완결 시퀀스만 |
| 4. 레이어를 넘는 접두 | 없음 | |
| 5. 모든 접두·느린 입력 | 없음 | 리뷰 2 체계 테스트는 시퀀스 끝까지만 |
| 6. 시퀀스 표기 변형 | 부분 | 리뷰 2는 단일 키와 `cmd-shift-k g` 하나 |
| 7. 교환 경고 변형 | 부분 | 리뷰 2는 `alt-s` 하나 |
| 8. 표준 키로 시작하는 시퀀스 | 없음 | |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | vim `3` → `window.dispatch_action(edit::Undo)`(메뉴 경로) → 5 s → `g g` | 깨짐: 메뉴 처리기는 `None`을 받지만 카운트가 남아 `ClipStart` 처리기가 3을 받는다. 리뷰 2 지적 2의 시나리오 그대로다 | `count_typed_before_menu_action_does_not_reach_next_key_action` |
| 2 | vim `3 g`(대기) → 메뉴 `edit::Undo` | 깨짐: 메뉴 처리기가 `Some(3)`을 받는다. 대기 키는 관찰자를 부르지 않아 `in_key_dispatch`가 참으로 남는다. macOS는 메뉴를 열 때 대기 입력만 지우고(`init_app_menus`의 `clear_pending_keystrokes`) 이 플래그와 카운트는 남는다. 보고서의 「메뉴·팔레트 처리기는 받지 않는다」와 다르다 | `menu_action_during_pending_sequence_gets_no_count` |
| 3 | vim `4` → 메뉴 `edit::Undo` | 못 깨뜨림(메뉴 처리기 `None`) | `menu_action_after_count_gets_no_count` |
| 4 | `3 g g`, `3 y y p`, `1 2 z z` | 못 깨뜨림(처리기가 카운트를 받고 뒤에 남지 않음) | `count_reaches_sequences_of_every_length` |
| 5 | `3 y` 1 s, `5 y y` 1 s(대기 만료 재생) | 못 깨뜨림(`Yank` 3, `Copy` 5) | `count_reaches_prefix_action_fired_by_timeout` |
| 6 | `3 y w`(대기가 깨져 재생) | 못 깨뜨림(`Yank` 3, `NextNote` `None`) | `count_goes_to_replayed_prefix_not_to_breaking_key` |
| 7 | `7` 뒤 바인딩 없는 `escape`, `f13`, `ctrl-alt-shift-f9`, `shift-f12`, `q`, `tab`, `ctrl-q` | 못 깨뜨림(모두 비워짐, 다음 `g g`는 `None`). 리뷰 2 지적 3은 근본까지 고쳐졌다 | `count_cleared_by_any_unbound_key` |
| 8 | `3 g 4 g g`(접두 대기가 숫자로 깨짐) | 못 깨뜨림 | `count_after_broken_prefix_does_not_leak` |
| 9 | 사용자 `w w` 위, vim `w` 아래 | 못 깨뜨림(`w w` 닿음, `w`는 1 s 뒤) | `user_sequence_extends_preset_single_key` |
| 10 | 사용자 `g` 위, vim `g g` 아래 | 못 깨뜨림(위 레이어가 이겨 바로 실행) | `user_single_key_overrides_preset_sequence_prefix` |
| 11 | 사용자 `g g x` 위, vim `g g` 아래 | 못 깨뜨림 | `user_three_key_sequence_extends_preset_two_key` |
| 12 | 사용자 `"y": null`, `"y y": null` + `"y y p": null` | 못 깨뜨림(`y y`는 살고 `y`는 사라짐, 시퀀스를 지우면 `y` 바로 실행) | `user_null_on_prefix_or_sequence`, `user_null_on_sequence_lets_prefix_fire_now` |
| 13 | 기본 프리셋 위 사용자 `f6`·`f6 f7`(같은 절, 긴 것이 파일 앞) | 못 깨뜨림 | `default_preset_user_prefix_and_sequence` |
| 14 | 두 프리셋: 모든 시퀀스의 자기 바인딩 있는 접두(길이 1·2)를 치고 1.1 s, 그리고 시퀀스를 키 사이 0.9 s로 침 | 못 깨뜨림(vim `y`·`y y` 포함). 리뷰 2 지적 1은 근본까지 고쳐졌다 | `every_bound_prefix_fires_after_timeout_and_slow_sequences_reach_default`, `..._vim` |
| 15 | 두 프리셋(교환 전후)의 시퀀스 첫 키가 표준 키인가 | 못 깨뜨림(없음) | `preset_sequences_never_start_with_standard_key` |
| 16 | 시퀀스 표기 변형 같은 키: `y shift-y`/`y Y`, `cmd-k super-j`/`super-k cmd-j`, `ctrl-alt-x  g`/`alt-ctrl-x g`, `Alt-k Shift-G`/`alt-k shift-g` | 못 깨뜨림 | `sequence_conflicts_detected_across_spellings` |
| 17 | 시퀀스 표기 변형 접두: `y Y`/`y shift-y p`, `super-k`/`cmd-k g`, `g G`/`g shift-g x` | 못 깨뜨림. 리뷰 2 지적 4는 근본까지 고쳐졌다 | `sequence_prefix_conflicts_detected_across_spellings` |
| 18 | 사용자 `Y`가 `shift-y` 입력에 닿는가(정규화의 전제) | 못 깨뜨림 | `uppercase_letter_binding_matches_shift_letter_keystroke` |
| 19 | 교환 아래 사용자 `Alt-s`, `alt-shift-s`, `shift-alt-z`, `alt-,` | 못 깨뜨림(모두 경고). `option-s`는 GPUI가 키로 받지 않아 잘못된 키 오류가 난다(경고 대상 아님) | `swap_warning_for_alias_and_case_variants` |
| 20 | 교환 켬 → 끔 | 못 깨뜨림(경고가 따라 생기고 사라짐). 리뷰 2 지적 5는 근본까지 고쳐졌다 | `swap_warning_follows_toggle` |
| 21 | 교환 아래 사용자 `super-s`(실제로는 `alt-s`에 걸림) | 거짓 경고가 남는다(제안 3, 지금 동작 고정) | `swap_warns_for_cmd_key_moved_away_by_swap` |
| 22 | 사용자 `cmd-s x`(교환 없음), `alt-s x`(교환) | `cmd-s`가 경고 없이 1 s 기다린 뒤 저장한다(제안 4, 지금 동작 고정) | `user_sequence_starting_with_standard_key_delays_it_without_warning` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 프리셋 파일의 모든 바인딩(접두 포함)은 그 컨텍스트에서 닿는다 | 못 깨뜨림 | 시도 14 |
| 레이어 규칙(위 레이어 이김, `null`은 아래만 지움)이 시퀀스·접두에도 선다 | 못 깨뜨림 | 시도 9–13 |
| 메뉴·팔레트 처리기는 카운트를 받지 않는다(보고서가 정한 의미) | 시퀀스 대기 중이면 받는다 | `crates/keyscore-app/src/keymap/mod.rs:296-311`, `take_count` 404-410 |
| 쓰이지 않은 카운트는 다음 Action 뒤에 남지 않는다 | 메뉴 경로 뒤에 남아 다음 키 Action이 받는다 | 같은 곳 |
| 표준 키는 프리셋이 덮지 않는다(KM-12) | 못 깨뜨림(시퀀스 첫 키 포함) | 시도 15 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| KM-4 (엔진 부분) | 충족 | 시도 16–18, `conflict.rs:62-67` |
| KM-5 | 부분 충족 | 시퀀스·접두·대기 만료·재생 경로의 카운트는 충족(시도 4–8, 14). 메뉴 경로와 카운트의 관계가 지적 1·2로 남는다. REQ는 키 이외 경로를 정하지 않는다 |
| KM-11 | 충족 | 시도 19–20 |
| KM-12 | 충족 | 시도 15, 19–20. 거짓 경고와 시퀀스 대기는 제안 3·4 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(UI 스레드만).
- 제품 코드의 `unwrap`/`expect`: 이번 변경에 없음.
- 에러가 삼켜지는 곳: 새로 생긴 것 없음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 2 지적 2가 근본까지 고쳐지지 않았다. 지적의 시나리오(`3` → 메뉴로 실행 취소 → 한참 뒤 키 Action)에서 키 Action이 여전히 3을 받는다. 바뀐 것은 메뉴 처리기가 카운트를 받지 않는다는 것뿐이고, 리뷰 2 테스트 `count_is_cleared_by_menu_dispatched_action`은 처리기 밖 `take_count`가 늘 `None`이 된 의미 변경 때문에 통과한다(실제 `pending_count`는 3). 보고서는 「마우스로 메뉴 막대」만 한계로 적었지만 `window.dispatch_action`(툴바·팔레트 등 키가 아닌 모든 경로)도 같다. 메뉴 경로에서 처리기 밖 `take_count`가 카운트를 비우게 하는 것(카운트를 쓰는 처리기는 모두 `take_count`를 부른다), 루트 뷰의 `capture_any_mouse_down`으로 비우는 것 등이 가능하다. REQ가 정하지 않는 의미이므로 결정 요청이 맞다. 테스트: `count_typed_before_menu_action_does_not_reach_next_key_action` | `crates/keyscore-app/src/keymap/mod.rs:290-311,401-410` |
| 2 | 수정 | 고친 의미가 시퀀스 대기 중에 깨진다. `3 g`(대기) 뒤 메뉴로 실행된 `edit::Undo` 처리기가 `Some(3)`을 받는다. 인터셉터가 켠 `in_key_dispatch`를 대기 키에서는 관찰자가 끄지 않기 때문이다. macOS는 메뉴를 열 때 GPUI가 대기 입력만 지우므로(`init_app_menus`) 실제 사용에서 `3 g`, 메뉴 「실행 취소」가 세 번 실행 취소한다. 테스트: `menu_action_during_pending_sequence_gets_no_count` | `crates/keyscore-app/src/keymap/mod.rs:296-311` |
| 3 | 제안 | 교환 아래 사용자 `super-s`는 실제로 `alt-s`에 걸려 저장을 덮지 않는데 `load_layers`가 교환 전 키로 「표준 단축키를 덮는다」고 경고한다(거짓 경고). `build`처럼 교환 뒤 키로 판단하면 된다. 테스트: `swap_warns_for_cmd_key_moved_away_by_swap`(지금 동작 고정, 통과) | `crates/keyscore-app/src/keymap/mod.rs:107-118` |
| 4 | 제안 | 사용자 시퀀스가 표준 키로 시작하면(`cmd-s x`, 교환 아래 `alt-s x`) `cmd-s`가 1 s 기다린 뒤 저장한다. 덮어쓰기는 아니지만 표준 키 동작이 바뀌므로 KM-12 경고 대상에 넣을 수 있다. 테스트: `user_sequence_starting_with_standard_key_delays_it_without_warning`(지금 동작 고정, 통과) | `crates/keyscore-app/src/keymap/mod.rs:107-118,189-200` |

B(리뷰어 테스트 변경): `review_s2_t2.rs`의 다섯 곳(`vim_piano_roll_digits_are_count_prefix` 2, `vim_arrangement_digits_are_count_prefix`, `count_is_cleared_when_another_action_runs`, `count_saturates_and_clamps_odd_digits` 2)은 처리기 밖에서 쌓인 값을 읽는다. 새 `take_count`는 키 디스패치 밖에서 늘 `None`이므로 바꾸지 않으면 실패한다(필요한 변경). `drain_count`는 `pending_count` 뒤 `clear_count`로, 옛 `take_count`의 처리기 밖 동작(꺼내고 비움)과 같다. 기대값 8, 3, 42, `None`, 9999, 9는 그대로이고 각 테스트의 의도(숫자가 카운트로 쌓임, 다른 Action 뒤 비워짐, 포화)를 지킨다. 문제 없음. 다만 같은 이유로 리뷰 2 파일의 카운트 테스트 셋(`count_is_cleared_by_menu_dispatched_action`, `count_is_cleared_by_unbound_escape`, `count_survives_pending_sequence_and_reaches_its_handler`의 끝 단언)은 처리기 밖 `take_count`라 의미 변경 뒤 무조건 통과한다. 이 중 Esc는 시도 7이 `pending_count`로 다시 확인했고, 메뉴는 지적 1이다.

## 8. 판정

- [ ] 병합 가능
- [ ] 수정 후 재리뷰
- [x] 범위 재정의 (이유: 아래)

리뷰 2 지적 1·3·4·5는 근본까지 고쳐졌다(시도 4–20). 남은 것은 지적 1·2, 곧 「키가 아닌 경로(메뉴 막대, 툴바, 팔레트의 `dispatch_action`)와 카운트의 관계」 한 곳이다. 이것은 REQ KM-5가 정하지 않은 의미이고, GPUI에 모든 디스패치를 보는 훅이 없어 구현자가 임의로 의미를 정했다(보고서 대응 2). 리뷰 3회가 끝났으므로 프로토콜(`plan/01-protocol.md` [8])대로 결정 요청으로 넘긴다.

- S2-T2 범위에서 「키 이외 경로의 카운트 의미」를 뺀다. 구현자는 `docs/decisions/<날짜>-count-non-key-dispatch.md`에 선택지(메뉴 경로의 처리기 밖 `take_count`가 비움, 마우스 입력으로 비움, 한계로 REQ에 적음)와 지적 2(대기 중 플래그)를 적는다.
- 실패하는 두 테스트(`count_typed_before_menu_action_does_not_reach_next_key_action`, `menu_action_during_pending_sequence_gets_no_count`)는 그 결정을 따르는 후속 Task의 수용 테스트로 옮긴다. 지우거나 `#[ignore]`하지 않는다.
- 위 두 가지가 커밋되면 S2-T2의 나머지는 이 리뷰 범위(A·B·C)에서 병합해도 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다 (`cargo test -p keyscore-app --test review_s2_t2_r3`: 22 통과, 2 실패. `cargo test -p keyscore-app --no-fail-fast` 3회 모두 다른 테스트 파일은 전부 통과(리뷰 2 23개 포함), 결과 같음)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다 (`240d850..HEAD`: `keymap/mod.rs`, `keymap/conflict.rs`, 테스트 두 파일, 보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다 (`review_s2_t2.rs`, `keymap_engine.rs`는 같은 기대값으로 읽는 도우미만 바뀜)
- [x] 새 의존성이 있으면 라이선스를 확인했다 (새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

자동 게이트: `cargo fmt --all -- --check` 통과, `cargo clippy --workspace --all-targets -- -D warnings` 통과.
