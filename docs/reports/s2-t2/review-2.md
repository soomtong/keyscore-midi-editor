# 적대적 리뷰: S2-T2 키맵 엔진 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `6fab10e` (브랜치 `s2/t2-keymap`, 리뷰 1 대응 `44fd00a`)
- 범위: 리뷰 1 지적 1–9가 근본까지 고쳐졌는가, 받아들인 제안 10·11·13이 문서대로 동작하는가, 회귀가 없는가. 그 밖은 제안으로만 적는다.
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t2_r2.rs` (23개, 실패 6개는 아래 지적 사항이다)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff d9b8c35..HEAD`, `keymap/mod.rs`, `keymap/conflict.rs`, `keymap/source.rs`, 두 프리셋 파일, GPUI `keymap.rs`(`resolve_bindings_for_input`)·`window.rs`(대기 입력, 키 관찰자)·macOS `events.rs`를 먼저 읽고 적었다.

1. 남은 `null`(vim `Normal`의 `m`, `z`)과 깊은 컨텍스트 바인딩이 같은 프리셋의 얕은 바인딩을 가리지 않는가. 단일 키 전부를 컨텍스트 스택마다 실제로 쳐서, 레이어 규칙(위 레이어가 이김, 레이어 안에서는 깊은 컨텍스트, `null`은 아래 레이어만 지움)으로 계산한 기대값과 비교한다.
2. GPUI는 「뒤 인덱스의 완결 바인딩」이 「앞 인덱스의 대기 시퀀스」를 버린다(`binding_ix > ix`면 대기 취소). vim의 `y`(뒤)와 `y y`(앞)처럼 파일 순서가 시퀀스를 죽이는 곳이 있는지.
3. Shift 숫자 외의 `shift-<기호>` 바인딩이 있는지, 있으면 macOS US 입력과 맞는지.
4. 카운트 비우기가 `observe_keystrokes`라서 키가 아닌 경로(메뉴, `dispatch_action`)와 Action 없는 키(`escape`, A13 대기라 비어 있음)는 비우지 못한다. 코드 주석은 「`3`, `Esc`, `x`」를 막는다고 적었다.
5. 보고서 대응 5는 「키도 GPUI 키 입력으로 정규화해 비교한다」인데 `conflict.rs`는 `b.keys` 원문으로 묶는다.
6. 사용자 키 표준 보호 경고가 교환 전 키로 비교한다. 교환을 켜면 사용자 `alt-s`가 `cmd-s`가 된다.
7. 설정 저장이 디바운스 뒤 디스크에 닿고, 다음 `init`이 그것을 읽어 엔진에 넘기는가.
8. 잘못된 context 절(빈 문자열, 배열, 객체, 술어 문법 오류, 중복 필드와 섞임)이 전역으로 새지 않는가.
9. 마지막 좋은 사용자 레이어: 부트부터 문법 오류, 좋은 파일로 교체, 빈 배열, UTF-8 아님, 디렉터리.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 모든 단일 키 × 컨텍스트 | 없음 | 구현 쪽은 리뷰 1 테스트(`b`, 숫자)만 통과시켰다 |
| 2. 시퀀스 도달 | 없음 | `presets_have_no_conflicts_and_only_known_actions`는 접두 충돌을 허용 목록으로만 본다 |
| 3. Shift 기호 | 부분 | 리뷰 1의 `shift-3` 하나 |
| 4. 메뉴·Esc 뒤 카운트 | 없음 | `count_is_cleared_when_another_action_runs`는 키로 실행된 `f5`만 |
| 5. 키 표기 정규화 충돌 | 없음 | 리뷰 1의 컨텍스트 공백만 |
| 6. 교환 아래 표준 키 경고 | 없음 | |
| 7. 재시작 뒤 프리셋 | 부분 | `set_preset_persists_to_settings`는 메모리의 설정 값만 본다 |
| 8. 잘못된 context 변형 | 부분 | `"context": 5`만 |
| 9. 마지막 좋은 레이어 | 부분 | `syntax_error_in_user_keymap_keeps_last_good_layer` 한 경로 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 기본 프리셋: 걸린 모든 단일 키를 모든 컨텍스트 스택(`Normal`, `Normal > PianoRoll`, `Normal > Arrangement`, `Step`과 파일에 나오는 모든 컨텍스트, 32개)에서 쳐서 레이어 규칙 기대값과 비교 | 못 깨뜨림 | `every_single_key_dispatches_layer_rule_action_default` |
| 2 | vim 프리셋, 같은 방법 | 못 깨뜨림(남은 `null` `m`·`z`는 아래 레이어만 지운다. 리뷰 1 지적 1은 단일 키에서 근본까지 고쳐졌다) | `every_single_key_dispatches_layer_rule_action_vim` |
| 3 | 기본 프리셋: 모든 여러 키 시퀀스를 그 컨텍스트에서 끝까지 침 | 못 깨뜨림 | `every_sequence_reaches_its_action_default` |
| 4 | vim 프리셋, 같은 방법 | 깨짐: `y y`는 `Yank` 두 번, `y y p`는 `Yank Yank PasteAfter`. `edit::Copy`, `edit::Repeat`에 닿지 않는다 | `every_sequence_reaches_its_action_vim` |
| 5 | 두 프리셋의 모든 `shift-<숫자·기호>` 바인딩을 macOS US 입력(Shift 없는 기호 글자)으로 침 | 못 깨뜨림(숫자 14개, 기호 바인딩은 프리셋에 없음) | `shifted_symbol_bindings_match_us_keystrokes_default`, `..._vim` |
| 6 | vim `3` 뒤 메뉴 경로(`window.dispatch_action(edit::Undo)`) | 깨짐: 카운트 3이 남는다 | `count_is_cleared_by_menu_dispatched_action` |
| 7 | vim `3 escape` | 깨짐: 카운트 3이 남는다(`escape`는 A13 대기로 비어 Action이 없다) | `count_is_cleared_by_unbound_escape` |
| 8 | vim `1 2 g g`, `4 g`·1 s·`g g` | 못 깨뜨림(처리기가 12, 4를 받고 뒤에 비워짐) | `count_survives_pending_sequence_and_reaches_its_handler` |
| 9 | 같은 레이어·컨텍스트의 `cmd-shift-k`/`shift-cmd-k`, `alt-k`/`Alt-k`, `cmd-k`/`super-k`, `ctrl-alt-h`/`alt-ctrl-h` | 깨짐: 충돌 없음 | `conflicts_detected_across_key_spellings` |
| 10 | `shift-cmd-k`와 `cmd-shift-k g` | 깨짐: 접두 충돌 없음 | `prefix_conflicts_detected_across_key_spellings` |
| 11 | 컨텍스트 `Normal  >PianoRoll`과 `Normal> PianoRoll` | 못 깨뜨림 | `conflicts_detected_across_context_whitespace` |
| 12 | 앱 A에서 vim·교환 켜기 → 1.1 s → 같은 경로로 앱 B `init` | 못 깨뜨림(B가 vim·교환으로 뜨고 `b`가 `PrevNote`) | `preset_and_swap_survive_restart` |
| 13 | context `""`, `"Normal >"`, `["Normal"]`, `"&& Normal"`, 중복 context(좋은 것 뒤 숫자, 숫자 뒤 좋은 것), `{}` | 못 깨뜨림(어느 것도 새지 않고 각 절이 오류를 냄) | `bad_context_sections_never_leak_bindings` |
| 14 | 사용자 `super-s`, `Cmd-s`, `shift-cmd-z`, `win-z`, `cmd-shift-S`, `"cmd-s": null` | 못 깨뜨림(모두 경고) | `user_override_of_standard_shortcut_warns_for_aliases`, `user_unbinding_standard_shortcut_warns` |
| 15 | 교환 켠 뒤 사용자 `alt-s` | 깨짐: 실제로 `cmd-s`(저장)를 덮는데 경고가 없다 | `user_override_of_standard_shortcut_warns_under_swap` |
| 16 | 두 프리셋의 모든 키를 교환한 뒤 표준 키와 비교 | 못 깨뜨림 | `swapped_preset_keys_never_hit_standard_keys` |
| 17 | 부트부터 문법 오류 → 고침 | 못 깨뜨림 | `syntax_error_at_boot_then_fixed` |
| 18 | 좋은 파일 → 문법 오류 → 다른 좋은 파일 → `[]` | 못 깨뜨림(유지, 교체, 비움) | `last_good_user_layer_is_replaced_and_cleared` |
| 19 | UTF-8 아닌 파일, 경로가 디렉터리 | 못 깨뜨림(읽기 오류 1개, 마지막 좋은 레이어 유지) | `unreadable_user_keymap_is_reported` |
| 20 | 중복 `bindings` | 못 깨뜨림(보고, 앞 것 유지) | `duplicate_bindings_field_is_reported_and_first_kept` |
| 21 | vim `m` 뒤 5 s 기다리고 `n` | 대기 시간이 없다: `SetMark(n)`이 나온다(KM-5 1 s와 다름, 지금 동작 고정) | `vim_prefix_without_own_binding_waits_past_timeout` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 프리셋 파일의 모든 바인딩은 그 컨텍스트에서 닿는다 | 단일 키는 못 깨뜨림. vim `y y`, `y y p`는 닿지 않는다 | `assets/keymaps/vim.json:19,26,154` |
| 표준 단축키는 프리셋이 덮지 않는다(KM-12) | 못 깨뜨림(교환 포함) | 시도 16 |
| 사용자가 표준 키를 덮으면 경고(KM-12) | 교환 옵션 아래에서 경고 없이 덮인다 | `crates/keyscore-app/src/keymap/mod.rs:109-118` |
| 카운트는 쓰이지 않으면 다음 Action 뒤 비워진다 | 메뉴 경로, Action 없는 키에서 남는다 | `crates/keyscore-app/src/keymap/mod.rs:265-276` |
| 잘못된 context는 전역으로 새지 않는다 | 못 깨뜨림 | 시도 13 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| KM-1 | 충족 | 시도 1–2, 13 |
| KM-2 | 부분 충족 | 프리셋 저장·재시작 충족(시도 12). vim의 `y y`·`y y p`가 닿지 않는다(지적 1) |
| KM-4 (엔진 부분) | 부분 충족 | 키 표기 차이의 충돌을 놓친다(지적 4). 접두 충돌의 「짧은 쪽은 대기 뒤 실행」 설명이 파일 순서에 따라 틀린다(지적 1) |
| KM-5 | 부분 충족 | 시퀀스 중 카운트 유지는 충족(시도 8). 메뉴·Esc 뒤 카운트가 남는다(지적 2, 3). 자기 바인딩 없는 접두에 대기 시간이 없다(제안 6) |
| KM-6 | 충족 | 시도 17–20 |
| KM-7 | 충족(US·ABC 자판 한정, 다른 자판은 B25) | 시도 5 |
| KM-11 | 충족 | 시도 12, 16, 리뷰 1 `swap_alt_cmd_handles_modifier_aliases` |
| KM-12 | 부분 충족 | 교환 아래 사용자 덮기 경고 없음(지적 5) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(UI 스레드만).
- 제품 코드의 `unwrap`/`expect`: 키맵·설정 코드에 없음.
- 에러가 삼켜지는 곳: 새로 생긴 것 없음. `"context": null`은 오류 없이 전역 절이 된다(JSON `null`을 「컨텍스트 없음」으로 읽는 것이라 지적하지 않음).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | vim `Normal`에서 `"y": "operator::Yank"`(154행)가 `"y y": "edit::Copy"`(19행)·`"y y p": "edit::Repeat"`(26행)보다 뒤에 있다. GPUI는 뒤 인덱스의 완결 바인딩이 있으면 앞 인덱스의 대기 시퀀스를 버리므로(`resolve_bindings_for_input`의 `binding_ix > ix`) `y`가 바로 실행되고 `y y`·`y y p`에는 절대 닿지 않는다. 리뷰 1 지적 1과 같은 뿌리(GPUI 해석 규칙을 프리셋이 따르지 않음)다. 짧은 바인딩을 긴 시퀀스보다 앞에 두거나(파일 순서), 빌드에서 시퀀스를 뒤로 보낸다. `conflict.rs`의 `Prefix` 설명(「짧은 쪽은 대기 시간이 지나야 실행된다」)도 순서 조건을 적거나, 순서가 반대인 접두를 별도 충돌로 보고한다. 테스트: `every_sequence_reaches_its_action_vim` | `assets/keymaps/vim.json:19,26,154`, `crates/keyscore-app/src/keymap/conflict.rs:13-16` |
| 2 | 수정 | 카운트를 `observe_keystrokes`로만 비워서, 메뉴(키가 아닌 경로)로 실행된 Action 뒤에도 남는다. `3` → 메뉴 실행 취소 → 한참 뒤 `x`가 3번 지운다. 리뷰 1 지적 4(「숫자 외 Action이 지나가도 남는다」)가 경로 하나만 고쳐졌다. 키 경로와 무관하게 Action 디스패치 뒤에 비우는 곳(예: 메뉴·`dispatch_action` 경로도 지나는 지점)을 쓴다. 테스트: `count_is_cleared_by_menu_dispatched_action` | `crates/keyscore-app/src/keymap/mod.rs:265-276` |
| 3 | 수정 | Action 없는 키(`escape`는 A13 대기로 vim `Normal`에서 비어 있음)는 관찰자에 `action: None`으로 와서 카운트를 비우지 않는다. 코드 주석은 바로 이 예(「`3`, `Esc`, 한참 뒤 `x`」)를 막는다고 적었다. 대기 중이 아닌 키가 Action 없이 끝나도 비우거나(대기 키는 관찰자를 부르지 않는다), 주석과 보고서를 고친다. 테스트: `count_is_cleared_by_unbound_escape` | `crates/keyscore-app/src/keymap/mod.rs:265-276` |
| 4 | 수정 | 보고서 대응 5는 「키도 GPUI 키 입력으로 정규화해 비교한다」인데 `conflicts`는 `b.keys` 원문으로 묶는다. 같은 키의 다른 표기(`cmd-shift-k`/`shift-cmd-k`, `Alt-k`, `super-k`)의 같은 키 충돌과 접두 충돌을 놓친다(KM-4). `canonical_keys`로 묶는다. 테스트: `conflicts_detected_across_key_spellings`, `prefix_conflicts_detected_across_key_spellings` | `crates/keyscore-app/src/keymap/conflict.rs:64` |
| 5 | 수정 | 사용자 레이어의 표준 키 경고(KM-12)를 교환 전 키로 비교한다. 교환을 켜면 사용자 `alt-s`가 `cmd-s`로 걸려 저장을 덮는데 경고가 없다. 경고 비교에 교환 뒤 키를 쓴다(교환 상태가 바뀌면 `apply`가 다시 돈다). 테스트: `user_override_of_standard_shortcut_warns_under_swap` | `crates/keyscore-app/src/keymap/mod.rs:109-118` |
| 6 | 제안 | 자기 바인딩이 없는 접두(vim `m`, `g`, `z`, `'`, `` ` ``)는 GPUI가 타이머를 걸지 않아(`pending_has_binding`이 거짓) 다음 키를 무한히 기다린다. `m` 뒤 5 s 뒤의 `n`이 `search::Next`가 아니라 `SetMark(n)`이다. KM-5 「시퀀스 대기 시간 1 s」와 다르다. 받아들이면 엔진이 대기 시작을 보고 1 s 뒤 대기 입력을 버리게 하고, 고정 테스트의 기대값을 바꾼다. vim 원본 동작(마크 글자를 기다림)과 같다는 이유로 거절할 수도 있다. 테스트: `vim_prefix_without_own_binding_waits_past_timeout`(지금 동작 고정, 통과) | GPUI `window.rs` 대기 입력, `assets/keymaps/vim.json` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1 지적 1(단일 키)·2·6·7·8·9와 제안 10·11·13은 근본까지 고쳐졌다(시도 1–2, 5, 12–20). 수정 1–5는 리뷰 1 지적 1·4·5·3(KM-12) 대응이 한 경로만 막은 곳이다. 고친 뒤 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다 (`cargo test -p keyscore-app --test review_s2_t2_r2`: 17 통과, 6 실패. `cargo test -p keyscore-app --no-fail-fast` 3회 모두 다른 테스트 파일은 전부 통과, 결과 같음)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다 (`d9b8c35..HEAD`: 키맵 코드·프리셋·`window.rs`의 `key_context` 한 줄·테스트·문서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다 (`keymap_engine.rs`는 두 테스트 추가뿐)
- [x] 새 의존성이 있으면 라이선스를 확인했다 (새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

자동 게이트: `cargo fmt --all -- --check` 통과, `cargo clippy --workspace --all-targets -- -D warnings` 통과.
