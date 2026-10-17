# 적대적 리뷰: S2-T2 키맵 엔진 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `725b428` (브랜치 `s2/t2-keymap`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t2.rs` (26개, 실패 10개는 아래 지적 사항이다)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`keymap/source.rs`, `keymap/mod.rs`, `keymap/conflict.rs`, `settings.rs`, `lib.rs`와 GPUI(`gpui-pre-0.3.8/src/keymap.rs`, `gpui-pre-macos-0.3.8/src/events.rs`)를 먼저 읽고 적었다.

1. `build`는 `null`을 메타 없는 `NoAction`으로 건다. GPUI의 `resolve_bindings_for_input`은 `NoAction`보다 순위가 낮은 같은 키의 바인딩을 컨텍스트와 무관하게 「전부」 가린다(메타가 없으면 모두 0). 주석의 「같은 (컨텍스트, 키)의 앞 바인딩만」은 사실과 다르다. vim.json의 `Normal > PianoRoll` `null`이 vim 자신의 `Normal` 바인딩까지 가리는지 본다.
2. GPUI는 컨텍스트 없는 바인딩의 깊이를 스택 길이로 친다. 전역 바인딩이 가장 깊은 컨텍스트 바인딩과 동률이 되는데, 모듈 문서의 우선순위 설명에 이 경우가 없다.
3. 표준 키 보호와 `swap_alt_cmd`가 키 문자열을 그대로 비교·치환한다. GPUI는 `super`/`win`/`secondary`, 대소문자 무시(`CMD`, `Alt`), 수식키 순서(`shift-cmd-z`)를 같은 키로 읽는다.
4. KM-12 「사용자 오버라이드로는 바꿀 수 있되 경고」: `load_layers`가 사용자 레이어에서 표준 키를 덮을 때 아무것도 남기지 않는다.
5. `parse`에서 `context`가 문자열이 아니면 오류만 넣고 `context = None`으로 진행한다. 그 절의 바인딩이 전역이 된다.
6. JSONC 읽기: 깊이 상한 64, 문자 단위 열, `\r`은 공백 취급, `\u` 대리 쌍, `number`/`word`의 끝 처리. 패닉 경로와 위치가 파일 밖을 가리키는 경로가 있는지.
7. 충돌 감지의 컨텍스트 비교가 공백만 정리한다. `Normal>PianoRoll`과 `Normal > PianoRoll`은 같은 술어인데 다른 그룹이 된다.
8. 카운트: 숫자 외 Action이 지나가도 `count`가 남는다. `0`은 카운트 숫자가 아니라 10 이상을 만들 수 없다. 계획의 「창마다」와 달리 전역이다.
9. `set_preset`/`set_swap_alt_cmd`는 `KeymapState.options`만 바꾸고 `AppSettings`에 쓰지 않는다. 계획 3은 「프리셋 선택은 설정에 저장」이다.
10. 핫리로드: 파일 삭제·재생성, 문법 오류 뒤 정상, 같은 주기 안 연속 저장. 문법 오류면 사용자 레이어 전체가 빠진다.
11. S2-T1의 `cmd-i`는 전역이었는데 default.json에서 `Normal` 컨텍스트로 옮겨졌다. 앱 어디에도 `key_context`가 없다.
12. KM-7: GPUI macOS는 Shift가 소문자가 아닌 글자를 내면 Shift를 떼고 그 글자를 키로 쓴다(`events.rs` 470–479행). 프리셋의 `shift-1`…`shift-7`이 실제 입력과 맞는지.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. `null`이 같은 프리셋의 얕은 바인딩까지 가림 | 없음 | `presets_have_no_conflicts_and_only_known_actions`는 파일 수준만 본다. 실제 디스패치는 확인하지 않는다 |
| 2. 전역 바인딩과 깊은 컨텍스트의 동률 | 없음 | `more_specific_context_wins`는 두 컨텍스트 바인딩만 본다 |
| 3. 수식키 별칭·순서 | 없음 | `swap_alt_cmd_option_rewrites_preset_bindings`는 소문자 `alt`/`cmd`만 본다 |
| 4. KM-12 경고 | 없음 | |
| 5. 잘못된 타입의 context | 없음 | `parse_errors_report_line_and_column`은 bindings 쪽 오류만 본다 |
| 6. JSONC 견고성 | 부분 | 깊이 100, 닫히지 않은 주석, BOM만. 멀티바이트·CRLF·무작위 입력 없음 |
| 7. 컨텍스트 표기 차이의 충돌 | 없음 | |
| 8. 카운트 잔류·`0`·포화 | 부분 | 누적(`1 2` → 12)만 |
| 9. 프리셋 저장 | 없음 | |
| 10. 핫리로드 삭제·재생성·문법 오류 | 부분 | 생성과 항목 오류만 |
| 11. S2-T1 `cmd-i` | 없음 | S2-T1 테스트는 Action을 직접 디스패치해 키 경로를 보지 않는다 |
| 12. Shift 숫자 | 없음 | `same_physical_key_fires_same_action_in_korean_input`은 `key`로 맞는다는 GPUI 자체 동작만 확인한다(입력 모양을 테스트가 정하므로 KM-7의 근거는 되지 못한다) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | vim, `Normal > PianoRoll`에서 `b` | 깨짐: 아무 Action도 안 나감(기대 `cursor::PrevNote`) | `vim_piano_roll_b_moves_to_previous_note` |
| 2 | vim, `Normal > PianoRoll`에서 `3`(대조 `8`) | 깨짐: `8`은 카운트, `3`은 없음 | `vim_piano_roll_digits_are_count_prefix` |
| 3 | vim, 배치 뷰에서 `4 2` | 못 깨뜨림(42) | `vim_arrangement_digits_are_count_prefix` |
| 4 | default, 피아노롤 `b` | 못 깨뜨림(`select::Bar`) | `default_piano_roll_b_selects_bar` |
| 5 | 사용자 파일에서 `Normal > Arrangement` 뒤의 전역 같은 키 | 전역이 이김(문서에 없는 동작, 현재 동작 고정) | `later_global_binding_ties_with_deepest_context` |
| 6 | 사용자 `Normal`의 `b`로 프리셋 피아노롤 `b` 덮기 | 덮지 못함(GPUI 규칙대로, 문서화 필요) | `user_shallow_context_does_not_override_deeper_preset` |
| 7 | 사용자 `cmd-s` 덮기 | 깨짐: 경고 없음(KM-12) | `user_override_of_standard_shortcut_warns` |
| 8 | 프리셋 키를 GPUI 키 입력으로 정규화해 표준과 비교, 수식키 교환 전후 | 못 깨뜨림 | `preset_keys_never_equal_standard_keys_after_normalization` |
| 9 | `swap_alt_cmd("super-j")`, `Alt-j`, `CMD-shift-k` | 깨짐: 그대로 남음 | `swap_alt_cmd_handles_modifier_aliases` |
| 10 | `"context": 5` | 깨짐: 바인딩이 전역으로 걸림 | `non_string_context_does_not_make_bindings_global` |
| 11 | 멀티바이트 키·주석 뒤 오류 위치 | 못 깨뜨림(문자 단위 열) | `error_column_counts_multibyte_characters_as_one` |
| 12 | CRLF 파일의 오류·바인딩 위치 | 못 깨뜨림 | `error_position_is_correct_with_crlf` |
| 13 | 배열·객체·인자 안 20만 단계 중첩 | 못 깨뜨림 | `very_deep_nesting_is_rejected_without_overflow` |
| 14 | 바인딩 5만 개 | 못 깨뜨림 | `huge_flat_file_parses` |
| 15 | 무작위 글자열 2만 개 + REQ 예시 변형 2만 개(NUL, BOM, 이모지, 한글, CR 포함) | 못 깨뜨림(패닉 없음, 위치가 파일 안) | `malformed_input_never_panics_and_positions_stay_in_bounds` |
| 16 | 대리 쌍 반쪽, 짧은 `\u`, `1e999999999`, `-`, `01`, `nul`, 빈 파일 | 못 깨뜨림 | `odd_escapes_and_numbers_do_not_panic` |
| 17 | `Normal > PianoRoll`과 `Normal>PianoRoll`에 같은 키 | 깨짐: 충돌 없음 | `conflicts_detected_across_context_spellings` |
| 18 | `3` 뒤 다른 Action(`f5`), 그 뒤 `take_count` | 깨짐: `Some(3)`이 남음 | `count_is_cleared_when_another_action_runs` |
| 19 | `9` 열두 번, `Count(-7)`, `Count(i64::MAX)` | 못 깨뜨림(9999, 0·9로 자름) | `count_saturates_and_clamps_odd_digits` |
| 20 | 키맵 파일 삭제 후 재생성 | 못 깨뜨림 | `hot_reload_survives_delete_and_recreate` |
| 21 | 문법 오류 → 같은 주기 안 두 번 저장 → 정상 | 못 깨뜨림(오류 2행 보고, 표준 레이어 유지, 마지막 내용 적용) | `hot_reload_invalid_then_valid` |
| 22 | `settings.json`의 keymap으로 부트 | 못 깨뜨림 | `keymap_settings_drive_engine_at_boot` |
| 23 | keymap 설정 왕복과 모르는 키 보존 | 못 깨뜨림 | `keymap_settings_round_trip_keeps_unknown_keys` |
| 24 | `set_preset(Vim)` 뒤 설정 값 | 깨짐: 설정은 `Default` 그대로 | `set_preset_persists_to_settings` |
| 25 | 프로젝트 창에서 `cmd-i`(S2-T1 동작) | 깨짐: 두 프리셋 모두 토글 안 됨 | `s2_t1_inspector_shortcut_survives_in_both_presets` |
| 26 | Step에서 macOS가 만드는 Shift+3 입력(`key: "#"`, Shift 없음), 대조군 `shift-3` | 깨짐: 대조군만 닿음 | `shift_digit_dotted_length_matches_macos_keystroke` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 표준 단축키는 프리셋이 덮지 않는다(KM-12) | 내장 프리셋으로는 못 깨뜨림. 보호가 문자열 비교라 별칭 표기(`super-s`, `shift-cmd-z`)를 쓰는 프리셋이 들어오면 뚫린다 | `keymap/mod.rs` `load_layers`의 `standard_keys` |
| 뒤 레이어가 이긴다(같은 깊이) | 레이어 안 `null`이 같은 레이어의 얕은 바인딩까지 지운다 | `assets/keymaps/vim.json` 194–210행, `keymap/mod.rs:170` |
| 오류 항목만 빠지고 나머지 적용 | 잘못된 타입의 `context`는 「빠지지」 않고 전역으로 넓어진다 | `keymap/source.rs` `parse`의 `"context"` 분기 |
| 파서는 패닉하지 않는다 | 못 깨뜨림 | 시도 13–16 |
| 카운트는 넘치지 않는다 | 못 깨뜨림 | 시도 19 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| KM-1 (컨텍스트·키 시퀀스 → Action, 7.3 형식) | 충족 | `keymap/source.rs`, 스냅샷 `req_example_keymap_snapshot` |
| KM-2 (프리셋 2종, 오버라이드 유지) | 부분 충족 | 오버라이드는 유지(`preset_switch_keeps_user_overrides`). vim 프리셋이 피아노롤에서 `b` `e` `n` `p` `B` `H` `L` `1`–`7`을 잃는다(지적 1). 프리셋 선택이 설정에 남지 않는다(지적 6) |
| KM-4 (충돌·접두 충돌 감지, 엔진 부분) | 부분 충족 | 같은 술어의 다른 표기를 놓친다(지적 5) |
| KM-5 (시퀀스, 카운트, 1 s) | 부분 충족 | 시퀀스·1 s는 충족. 카운트가 다음 Action 뒤에도 남는다(지적 4) |
| KM-6 (핫리로드, 줄 번호 보고) | 충족 | `keymap/mod.rs` `install`/`apply`, 시도 20–21. 상태줄 표시는 S2-T5 |
| KM-7 (물리 키) | 부분 충족 | 한/영 전환은 GPUI가 ASCII 자판으로 키를 만들어 맞는다(코드 읽기 근거, 수동 확인 대기). Shift 숫자 바인딩 14개가 macOS 입력과 맞지 않는다(지적 2) |
| KM-11 (수식키 교환) | 부분 충족 | 별칭 표기를 바꾸지 않는다(지적 8) |
| KM-12 (표준 `Cmd` 프리셋 불변, 사용자 덮기 시 경고) | 부분 충족 | 프리셋 불변은 충족. 경고 없음(지적 3) |
| 7.5 (표준 단축키) | 충족 | `assets/keymaps/standard.json` |
| 5.8 / 5.8.1 (프리셋 내용) | 부분 충족 | 표본 대조(7.3 예시 키, A1·A8·A11·A13 키 비움)는 맞다. 지적 1·2의 키는 파일에 있으나 실제로 닿지 않는다 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 UI 스레드만).
- 제품 코드의 `unwrap`/`expect`: 키맵·설정 코드에 없음.
- 에러가 삼켜지는 곳:
  - `keymap/source.rs` `parse`: `context` 타입 오류 뒤 바인딩을 전역으로 계속 만든다(지적 7).
  - `keymap/source.rs` `parse`: 한 절에 `"bindings"`가 둘이면 앞 것이 소리 없이 버려진다(지적 11).
  - `settings.rs` keymap 읽기: `preset`이 모르는 값(`"emacs"`)이면 경고 후 기본값을 쓰고, 저장할 때 원래 값을 덮어쓴다(같은 파일의 `theme`와 같은 방식이라 지적하지 않음).
  - `keymap/mod.rs` `apply`: 사용자 파일을 읽지 못하면(권한 등) 파일이 없는 것과 같이 다루고 오류를 남기지 않는다(지적 11에 함께 적음).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | vim.json의 `Normal > PianoRoll`·`Normal > Arrangement` `null`은 GPUI `NoAction` 규칙상 그보다 순위가 낮은 같은 키를 「전부」 가린다. 기본 프리셋 피아노롤의 `b` 등을 지우려다 vim 자신의 `Normal` `b`(`PrevNote`), `e`, `n`, `p`, `shift-b`, `shift-h`/`shift-l`, 카운트 `1`–`7`까지 죽는다. 피아노롤은 vim 사용자의 주 화면이다. `null` 대신 같은 컨텍스트(`Normal > PianoRoll`)에 vim 바인딩을 다시 적거나, 레이어에 `KeyBindingMetaIndex`를 달아 `null`이 아래 레이어만 가리게 한다. `build`의 주석(「같은 (컨텍스트, 키)의 앞 바인딩만」)도 GPUI 동작과 다르니 고친다. 테스트: `vim_piano_roll_b_moves_to_previous_note`, `vim_piano_roll_digits_are_count_prefix` | `assets/keymaps/vim.json:186-211`, `crates/keyscore-app/src/keymap/mod.rs:170` |
| 2 | 수정 | GPUI macOS는 Shift+숫자를 `key: "!"`…, Shift 없음으로 만든다(`gpui-pre-macos-0.3.8/src/events.rs:470-479`). `shift-1`…`shift-7`(`note::`·`step::SetDottedLength`, 14개)은 실제 입력에서 닿지 않는다. 결정 요청 B25의 권고 a(「숫자는 `shift-1` 접두」)가 이 사실과 어긋나므로 결정 요청에 근거를 더하고, 프리셋 표기를 고치거나(입력 글자 `!` `@` `#` …는 자판마다 다름) 앱이 Shift 숫자를 물리 키로 되돌리는 방법을 정한다. 테스트: `shift_digit_dotted_length_matches_macos_keystroke` | `assets/keymaps/default.json:122-134,190-202`, `docs/decisions/2026-10-17-keymap-conflicts.md` B25 |
| 3 | 수정 | KM-12 「사용자 오버라이드로는 바꿀 수 있되 경고를 띄운다」가 없다. 계획은 KM-12를 범위로 적었고 보고서는 이 누락을 적지 않았다. 사용자 레이어가 표준 키를 덮으면 `KeymapError`(또는 별도 경고)를 남긴다. 테스트: `user_override_of_standard_shortcut_warns` | `crates/keyscore-app/src/keymap/mod.rs:100-107` |
| 4 | 수정 | 카운트가 카운트를 쓰지 않는 다음 Action 뒤에도 남는다. `3`, `Esc`, 한참 뒤 `x`가 3번 실행된다. 숫자 외 Action이 디스패치되면(예: `observe_keystrokes`나 디스패치 뒤 훅) 카운트를 비운다. 테스트: `count_is_cleared_when_another_action_runs` | `crates/keyscore-app/src/keymap/mod.rs:305-320` |
| 5 | 수정 | 충돌 감지가 컨텍스트를 공백 정리만 해서 `Normal>PianoRoll`과 `Normal > PianoRoll`(GPUI에서 같은 술어)의 같은 키 충돌을 놓친다(KM-4). `KeyBindingContextPredicate::parse` 결과를 비교 키로 쓴다. 테스트: `conflicts_detected_across_context_spellings` | `crates/keyscore-app/src/keymap/conflict.rs:30-34` |
| 6 | 수정 | `set_preset`/`set_swap_alt_cmd`가 `AppSettings`에 쓰지 않아 다음 실행에서 되돌아간다. 계획 3(「프리셋 선택은 설정에 저장」)과 다르고 엔진 상태와 설정 두 곳에 진실이 생긴다. 테스트: `set_preset_persists_to_settings` | `crates/keyscore-app/src/keymap/mod.rs:277-286` |
| 7 | 수정 | `"context"`가 문자열이 아니면 오류를 내고도 그 절의 바인딩을 전역으로 건다. 전역은 모든 컨텍스트에서 가장 깊은 깊이로 맞으므로 잘못 쓴 파일이 앱 전체 키를 가로챈다. 그 절을 통째로 뺀다. 테스트: `non_string_context_does_not_make_bindings_global` | `crates/keyscore-app/src/keymap/source.rs:106-110` |
| 8 | 수정 | `swap_alt_cmd`가 GPUI가 받는 별칭(`super`, `win`, `secondary`, 대소문자 `Alt`/`CMD`)을 바꾸지 않는다. 사용자 파일에서 같은 키가 교환 여부에 따라 다르게 동작한다. `Keystroke::parse`한 수식키를 바꾸고 다시 `unparse`한다. 같은 이유로 표준 키 보호(`standard_keys`)도 정규화된 키로 비교하는 편이 안전하다(지금 내장 프리셋은 겹치지 않음, `preset_keys_never_equal_standard_keys_after_normalization`). 테스트: `swap_alt_cmd_handles_modifier_aliases` | `crates/keyscore-app/src/keymap/mod.rs:75-91,120-136` |
| 9 | 수정 | S2-T1에서 전역이던 `cmd-i`(인스펙터)가 default.json의 `Normal` 컨텍스트로 옮겨졌는데 앱 어디에도 `key_context("Normal")`이 없어 프로젝트 창에서 키로 동작하지 않는다(메뉴 단축키 경로는 헤드리스에서 확인 불가). 같은 이유로 프리셋의 컨텍스트 바인딩 전부가 지금 앱에서 닿지 않는다. 창 루트에 `Normal`(과 배치 뷰 `Arrangement`) 컨텍스트를 두거나, S2-T3까지 `cmd-i`를 전역으로 둔다. 테스트: `s2_t1_inspector_shortcut_survives_in_both_presets` | `assets/keymaps/default.json:26`, `crates/keyscore-app/src/window.rs` |
| 10 | 제안 | 모듈 문서의 우선순위 설명에 두 예외를 적는다. (a) 전역 바인딩은 가장 깊은 컨텍스트와 동률이라 뒤에 있으면 이긴다(`later_global_binding_ties_with_deepest_context`). (b) 사용자 `Normal` 바인딩은 프리셋 `Normal > PianoRoll`의 같은 키를 덮지 못한다(`user_shallow_context_does_not_override_deeper_preset`). KM-3 편집 화면(S2-T6)이 사용자에게 이걸 알려야 한다 | `crates/keyscore-app/src/keymap/mod.rs:1-10` |
| 11 | 제안 | `parse`가 한 절의 중복 `"bindings"`/`"context"`를 소리 없이 덮는다. 사용자 파일을 읽지 못하는 오류(권한, UTF-8 아님)를 「파일 없음」으로 다룬다. 둘 다 오류로 보고하면 KM-6의 「오류 표시」에 맞다 | `crates/keyscore-app/src/keymap/source.rs:104-118`, `crates/keyscore-app/src/keymap/mod.rs:259` |
| 12 | 제안 | 계획 6의 「카운트가 시작된 뒤의 `0`」이 구현되지 않아 10 이상의 카운트를 만들 수 없다. 결정 요청 A9와 함께 다룬다. 카운트가 계획의 「창마다」가 아니라 전역이다 | `assets/keymaps/vim.json:150-158`, `crates/keyscore-app/src/keymap/mod.rs:206` |
| 13 | 제안 | 문법 오류 하나로 사용자 레이어 전체가 빠진다. 편집기가 쓰는 도중 저장하면 오버라이드가 잠깐 모두 사라진다. 마지막으로 성공한 사용자 레이어를 유지하고 오류만 보고하는 쪽이 낫다 | `crates/keyscore-app/src/keymap/mod.rs:253-275` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

수정 1–9를 고친 뒤 재리뷰한다. 1·2는 vim·기본 프리셋의 핵심 키가 실제 입력에서 닿지 않는 문제라 우선이다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다 (`cargo test -p keyscore-app --test review_s2_t2`: 16 통과, 10 실패)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다 (REQ.md KM-5 문구 변경은 결정 `2026-10-17-key-sequence-timeout.md`가 에이전트에게 맡긴 것)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다 (리뷰어 테스트 `review_s2_t1*.rs`, `window.rs`의 변경은 Action 이름 바꾸기뿐)
- [x] 새 의존성이 있으면 라이선스를 확인했다 (`schemars` 1: MIT, GPUI가 이미 쓰는 판. `insta`: 개발 의존, Apache-2.0)
- [x] 스타일 지적을 적지 않았다
