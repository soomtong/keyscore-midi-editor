# 적대적 리뷰: S2-T6a 설정 창과 키맵 편집기 (리뷰 1회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 세션(구현자와 다른 세션)
- 대상 커밋: b57700a (`s2/t6-settings-keymap`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트(`tests/settings_keymap.rs`, `tests/keymap_user.rs`)와 보고서를 열기 전에 `settings_window.rs`, `keymap/user.rs`, `keymap/mod.rs`, GPUI `keymap.rs`(gpui-pre 0.3.8)만 읽고 적었다.

1. 인자 있는 Action(`step::Note(a)`…`step::Note(w)`, `step::SetLength(..)`)의 줄에서 캡처하면 `UserKeymap::bind`가 그 컨텍스트의 같은 이름 바인딩을 모두 지우고 첫 인자 하나만 새 키에 건다. 음 입력 키 16개가 한 번의 캡처로 사라지는가.
2. 수식키 교환(KM-11)을 켠 채 캡처하면 누른 물리 키(`cmd-k`)를 그대로 사용자 레이어에 쓰고, `build`가 다시 `alt-k`로 바꿔 건다. 캡처한 키를 눌러도 그 Action이 나오지 않는가.
3. 전역(컨텍스트 없음) 바인딩을 지우거나 다른 키로 옮기면 사용자 레이어에 전역 `null`이 생긴다. GPUI의 NoAction은 전역이면 모든 컨텍스트의 같은 키를 가린다(`disabled_binding_matches_context`의 `(None, _) => true`, 그리고 컨텍스트 없는 바인딩은 가장 깊은 깊이로 정렬). `effective`는 그 자리만 지운다고 보므로 편집기 표시·충돌 계산과 엔진이 어긋난다. 예: `mode::Cycle`(전역 `ctrl-tab`)을 지우면 설정 창의 `settings::NextSection`(`Settings`의 `ctrl-tab`)도 죽는가.
4. 충돌 감지가 같은 컨텍스트 문자열만 본다. 전역 바인딩은 GPUI에서 가장 깊은 컨텍스트로 취급돼 컨텍스트 바인딩을 이긴다. 전역 Action(`settings::Open`)에 `r`을 걸면 시트 없이 걸리고, 목록의 `r`(복원)이 죽는가.
5. 충돌 시트가 열린 채 `ctrl-tab`으로 구역을 옮겼다 돌아오면 시트는 남고 포커스는 목록으로 간다. 목록에서는 `escape`·`enter`가 시트에 닿지 않고(`Dialog` 밖), 캡처도 시트 때문에 거부된다. 키보드만으로 시트를 닫을 수 있는가(KM-8, P1).
6. 프리셋 선택·수식키 교환·내보내기·가져오기를 키보드만으로 부를 수 있는가. `Settings`·`Keymap` 컨텍스트에 그 Action의 키가 없고, 팔레트는 프로젝트 창에서 디스패치되어 설정 창의 처리기에 닿지 않는다.
7. 가져오기에서 표준 단축키를 덮는 항목의 경고(계획 7항, KM-12)가 나오는가. `import_from`에는 표준 키 검사가 없다.
8. 캡처 중 `cmd-q`·`cmd-w`·`cmd-,`가 디스패치되지 않고 캡처되는가. 수식키 붙은 `escape`(`shift-escape`)는 캡처되는가.
9. 캡처 중 구역 전환·창 닫기 뒤 1 s가 지나도 파일이 바뀌지 않는가. 창을 닫은 뒤 다른 창의 키가 가로채지지 않는가.
10. `.bak`은 마지막 쓰기 전 내용만 남는다. 두 번 편집하면 사용자가 손으로 쓴 원본(주석)이 사라지는가.
11. 손으로 깨뜨린 `keymap.json`이 있을 때 가져오기는 그 파일을 덮어쓰는가(편집기의 「덮지 않는다」 규칙과의 관계).
12. 공백이 다른 컨텍스트(`Normal>PianoRoll`)가 사용자 파일에 있을 때 `bind`·`clear`·`restore`가 같은 자리로 보는가. 쓴 파일이 엔진에서 오류 없이 읽히는가(왕복).
13. 내보내기: 사용자 파일이 없으면 빈 레이어, 깨져 있으면 거부하는가.
14. 프리셋을 vim으로 바꿔도 사용자 항목이 남는가(KM-2). vim 오버레이 키를 지운 뒤 `r`이 되돌리는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 인자 있는 Action 캡처 | 없음 | `keymap_user.rs`의 `bind`는 인자 없는 Action만 본다. 깨짐(시도 1) |
| 2. 수식키 교환 + 캡처 | 없음 | `preset_switch_keeps_user_overrides`는 토글이 켜지는지만 본다. 깨짐(시도 2) |
| 3. 전역 `null`의 가림 범위 | 없음 | `effective_bindings_layer_and_unbind`는 같은 자리만 본다. 깨짐(시도 3) |
| 4. 전역 Action과 컨텍스트 키 충돌 | 없음 | 깨짐(시도 4) |
| 5. 시트가 열린 채 구역 전환 | 없음 | 깨짐(시도 5) |
| 6. 위 줄(프리셋·교환·내보내기·가져오기)의 키보드 경로 | 없음 | 기존 테스트는 `window.click`과 `dispatch_action`으로 부른다. 「키보드만」 게이트가 이 부분을 비켜 간다. 깨짐(시도 6·7) |
| 7. 가져오기의 표준 키 경고 | 없음 | 깨짐(시도 8) |
| 8. 캡처 중 `cmd-w` 등, 수식키 `escape` | `r`만 있음 | 못 깨뜨림(시도 11·12) |
| 9. 캡처 중 구역 전환·창 닫기 | 없음 | 쓰기 없음은 못 깨뜨림(시도 14). 다만 창을 닫은 뒤 다시 열 수 없다(시도 10) |
| 10. `.bak`의 원본 보존 | 없음(`.bak` 자체를 보는 테스트 없음) | 두 번째 편집에 원본이 사라진다(시도 9, 보고서의 알려진 한계) |
| 11. 깨진 사용자 파일 위로 가져오기 | 없음 | 시도하지 않음(가져오기는 명시적 교체라 `.bak`이 남는 것으로 충분하다고 봤다) |
| 12. 공백 다른 컨텍스트 왕복 | 단위 테스트에만 | 못 깨뜨림(시도 16) |
| 13. 내보내기의 없음·깨짐 | 없음 | 못 깨뜨림(시도 17) |
| 14. vim 오버레이 지우기·복원·프리셋 왕복 | 부분(`preset_switch_keeps_user_overrides`) | 못 깨뜨림(시도 15) |
| `cmd-w`로 설정 창 닫기(계획 1항) | 없음 | 깨짐(시도 11의 확장, 시도 13) |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s2_t6a.rs`에서 실행했다. 깨진 시도의 테스트는 파일에서 빼고 부록에 실패 출력과 함께 옮겼다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `step::Note` 줄에서 `enter ctrl-alt-n` 캡처 | 깨짐: `Step > Piano`의 음 입력 키 16개(`a s d f …`)가 모두 `null`이 되고 `ctrl-alt-n`→`step::Note(a)` 하나만 남는다(실효 39개 → 24개) | 부록 `brk_capture_on_args_action_keeps_other_variants` |
| 2 | 수식키 교환을 켜고 `cmd-ctrl-u` 캡처 뒤 프로젝트 창에서 `cmd-ctrl-u` | 깨짐: 파일에 `ctrl-cmd-u`가 쓰이고 엔진이 `ctrl-alt-u`로 바꿔 건다. 누른 키가 뮤트하지 않는다 | 부록 `brk_capture_with_swap_modifiers_fires_captured_key` |
| 3 | `mode::Cycle`(전역 `ctrl-tab`)을 `backspace`로 지운 뒤 설정 창에서 `ctrl-tab` | 깨짐: 전역 `null`이 `Settings`의 `settings::NextSection`까지 가린다. 편집기 목록은 NextSection이 `ctrl-tab (Settings)`에 걸려 있다고 보인다 | 부록 `brk_clearing_global_binding_keeps_context_bindings` |
| 4 | `settings::Open`(전역)에 `r` 캡처 | 깨짐: 충돌 시트 없이 걸린다. 그 뒤 목록의 `r`(복원)이 죽고, 검색란에 `r`을 칠 수 없다(전역 바인딩이 모든 컨텍스트의 `r`을 이긴다). 옛 키 `cmd-,`는 경고 없이 `null`이 된다 | 부록 `brk_global_binding_shadowing_context_key_is_a_conflict` |
| 5 | `o` 충돌 시트를 연 채 `ctrl-tab ctrl-shift-tab` 뒤 `escape` | 깨짐: 시트가 남고 포커스는 목록. `escape`는 `keymap::CancelCapture`, `enter`는 시트 때문에 거부된다. 키보드로 시트를 닫을 길이 없다 | 부록 `brk_sheet_reachable_after_section_round_trip` |
| 6 | 설정 창에서 `tab`+`space` 12번, 이어 `cmd-shift-p` | 깨짐: 프리셋·수식키 교환에 닿지 못한다(위 줄 Action에 키가 없고, 설정 창에는 팔레트가 없다) | 부록 `brk_preset_and_swap_by_keyboard_only` |
| 7 | 프로젝트 창 팔레트에서 「vim 프리셋」 | 깨짐: 팔레트에 항목은 보이지만 처리기가 설정 창 요소에만 있어 아무 일도 없다 | 부록 `brk_palette_select_preset_reaches_handler` |
| 8 | `cmd-s`를 덮는 파일 가져오기 | 깨짐: 경고 없이 「키맵을 가져왔다」 | 부록 `brk_import_warns_standard_override` |
| 9 | 주석 있는 손 편집 파일에서 편집 두 번 | 깨짐: `.bak`이 첫 편집 결과로 덮여 원본 주석이 어디에도 없다 | 부록 `brk_bak_keeps_hand_written_original_after_two_edits` |
| 10 | 설정 창을 닫은 뒤 프로젝트 창에서 `cmd-,` | 깨짐: 창이 다시 열리지 않는다. `current()`가 여전히 `Some`(뷰가 창 제거 뒤에도 살아 있음)이라 `open`이 닫힌 창의 `handle.update` 실패를 `let _`로 삼키고 끝난다 | 부록 `brk_reopen_settings_after_close` |
| 11 | 캡처 중 `cmd-w` | 못 깨뜨림: 창이 닫히지 않고 `cmd-w`가 캡처되며 표준 키 경고가 나온다 | `capture_swallows_standard_shortcuts_and_warns` |
| 12 | 캡처 중 `shift-escape` | 못 깨뜨림: 취소가 아니라 캡처된다 | `modified_escape_is_captured_not_cancel` |
| 13 | 캡처 아닐 때 설정 창에서 `cmd-w` | 깨짐: 창이 닫히지 않는다(프로젝트 창의 `cmd-w`도 마찬가지. `lib.rs`의 `window::Close` 처리기가 디스패치 중인 창에 `window.update`를 다시 걸어 실패한다) | 부록 `brk_cmd_w_closes_settings_window` |
| 14 | 캡처 중 구역 전환, 캡처 중 창 닫기, 그 뒤 프로젝트 창 `m` | 못 깨뜨림: 1 s 뒤에도 파일이 그대로이고 프로젝트 창 키는 가로채지지 않는다 | `capture_abandoned_by_section_switch_or_close_writes_nothing` |
| 15 | vim `cursor::ClipStart`(`g g`) 지우기 → 프리셋 왕복 → `r` | 못 깨뜨림 | `vim_overlay_clear_restore_and_preset_switch` |
| 16 | `"Normal  >  PianoRoll"` 손 편집 파일 위 편집, `.bak` 원문 | 못 깨뜨림 | `spaced_context_in_user_file_round_trips` |
| 17 | 사용자 파일 없음·깨짐에서 `export_to` | 못 깨뜨림 | `export_missing_and_broken_user_file` |
| 18 | 프리셋 키 지운 뒤 그 키를 다른 Action에 캡처 | 못 깨뜨림(시트 없이 걸리고 엔진이 그 Action을 낸다) | `clear_then_bind_same_key_elsewhere` |
| 19 | 두 번째 키를 0.9 s 뒤에 눌러 시퀀스 대기 재시작 | 못 깨뜨림 | `capture_sequence_restarts_settle_timer` |
| 20 | 검색 결과 0개에서 `enter enter x` | 못 깨뜨림(캡처 시작 안 함) | `capture_with_empty_filter_does_nothing` |
| 21 | `null` 항목이 있는 파일 가져오기 | 못 깨뜨림 | `import_with_unbind_entries_applies` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 편집기는 사용자 레이어만 고친다 | 아니오 | `save_user`는 `user_path`만 쓴다(`keymap/mod.rs:495`) |
| 편집기가 쓴 파일은 엔진이 오류 없이 읽는다 | 아니오(시도 15·16·18에서 `errors` 비어 있음). 표준 키를 가리면 경고가 `errors`에 들어가는 것은 의도 | — |
| 쓰기는 원자적이다 | 아니오. 다만 `.bak`은 매번 덮인다(시도 9) | `keymap/mod.rs:497-504` |
| 목록의 「지금 걸린 키」 = 엔진의 실효 바인딩(계획 2항) | 예 | 전역 `null`(시도 3), 전역 바인딩의 깊이 우선(시도 4), 수식키 교환(시도 2) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| KM-2 | 충족 | `keymap/mod.rs:390`, 시도 15, `preset_switch_keeps_user_overrides` |
| KM-3 | 미충족(부분) | 검색·캡처·지우기·복원은 동작. 인자 있는 Action 캡처가 다른 바인딩을 지운다(`keymap/user.rs:203-234`), 「현재 바인딩 표시」가 엔진과 어긋난다(`keymap/user.rs:49-74`) |
| KM-4 | 미충족(부분) | 같은 컨텍스트 문자열만 본다(`keymap/user.rs:79-100`). 전역 바인딩은 GPUI에서 가장 깊은 컨텍스트로 취급돼 모든 컨텍스트의 같은 키를 이긴다(시도 4) |
| KM-6 | 충족 | `keymap/mod.rs:473-510`, `editor_refuses_to_overwrite_broken_user_file` |
| KM-8 | 미충족 | 시트 고립(시도 5), 위 줄 네 Action의 키 경로 없음(시도 6·7), `cmd-w` 닫기 안 됨(시도 13). `Esc` 캡처 취소는 충족 |
| KM-9 | 미충족(부분) | 적용 전 충돌 검사는 충족(`settings_window.rs:551-560`). 가져오기·내보내기에 키 경로 없음(시도 6), 표준 키 경고 없음(시도 8) |
| KM-11 | 미충족(부분) | 교환 토글은 동작하나 press-to-bind와 함께 쓰면 캡처한 키가 걸리지 않는다(`settings_window.rs:346`, `keymap/mod.rs:185-189`) |
| KM-12 | 미충족(부분) | 캡처한 새 키가 표준 키면 경고(`settings_window.rs:423-435`). 가져오기(시도 8)와 표준 키를 `null`로 가리는 편집(시도 4의 `cmd-,`)은 편집기에서 알리지 않는다 |
| 7.5 `Cmd+,` | 미충족(부분) | 설정 창을 한 번 닫으면 다시 열리지 않는다(`settings_window.rs:101-111`, `133-136`) |
| 7.6 설정 > 키맵 | 충족 | 구역 목록과 키맵 구역 |
| P1 | 미충족 | KM-8과 같은 근거. 팔레트에 보이는 `keymap::SelectPreset`·`SwapModifiers`·`Export`·`Import`가 어디서도 키보드로 실행되지 않는다 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(설정 창은 UI 스레드만)
- 제품 코드의 `unwrap`/`expect`: 없음
- 에러가 삼켜지는 곳
  - `settings_window.rs:102` `let _ = handle.update(...)`: 창이 닫혔으면 실패를 삼키고 새 창도 열지 않는다(시도 10의 원인)
  - `settings_window.rs:349` `let _ = this.update_in(cx, Self::finish_capture)`: 뷰가 사라졌을 때만이라 정당
  - `lib.rs:55-57` `window::Close` 처리기의 `let _ = window.update(...)`: 디스패치 중인 창이면 늘 실패한다(시도 13). 이 Task 이전 코드이나 계획 1항이 기댄다

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 인자 있는 Action(`step::Note`, `step::SetLength`, `step::Chord` …) 줄에서 캡처하면 `bind`가 그 컨텍스트의 같은 이름 바인딩을 모두 `null`로 가리고 첫 인자 하나만 새 키에 건다. 한 번의 캡처로 음 입력 키 16개가 사라진다(조용한 바인딩 손실). 줄을 (이름, 인자) 단위로 나누거나, 인자 있는 Action은 그 인자의 바인딩만 바꿔야 한다 | `crates/keyscore-app/src/keymap/user.rs:203-234`, `crates/keyscore-app/src/settings_window.rs:378-392` |
| 2 | 차단 | 수식키 교환이 켜져 있으면 캡처한 물리 키를 그대로 사용자 레이어에 쓰고 `build`가 다시 교환해 건다. 캡처한 키를 눌러도 Action이 나오지 않는다(KM-11·KM-3). 충돌·표준 키 비교도 교환 전 키로 한다. 캡처한 키를 교환 역변환해 저장해야 한다 | `crates/keyscore-app/src/settings_window.rs:346`, `crates/keyscore-app/src/keymap/mod.rs:185-189` |
| 3 | 차단 | 실효 바인딩 계산이 GPUI의 `null` 규칙과 다르다. 전역 `null`(컨텍스트 없음)은 모든 컨텍스트의 같은 키를 가린다(`disabled_binding_matches_context`의 `(None, _) => true`, 컨텍스트 없는 바인딩은 가장 깊은 깊이). 전역 바인딩(`mode::Cycle`, 표준 레이어 전체)을 지우거나 다른 키로 옮기면 다른 Action의 컨텍스트 바인딩(`settings::NextSection`의 `ctrl-tab`, `lyric::Distribute`의 `cmd-v`)이 죽는데 목록은 걸려 있다고 보인다 | `crates/keyscore-app/src/keymap/user.rs:49-74`, `:216-224`, `:237-245` |
| 4 | 차단 | 설정 창을 닫은 뒤 `cmd-,`·`keymap::Edit`가 창을 다시 열지 않는다. `current()`는 뷰의 약한 참조만 보고(창 제거 뒤에도 업그레이드된다), `open`은 닫힌 창의 `handle.update` 실패를 삼키고 돌아간다. 창 핸들이 살아 있는지 확인하거나 창 닫힘에서 `OpenSettings`를 비워야 한다 | `crates/keyscore-app/src/settings_window.rs:101-111`, `:133-136` |
| 5 | 차단 | KM-8·P1: 위 줄의 프리셋 선택·수식키 교환·내보내기·가져오기에 키 경로가 없다. `Settings`·`Keymap` 컨텍스트에 바인딩이 없고, `tab`·`space`로 버튼을 누를 수 없으며, 설정 창에는 팔레트가 없다. 프로젝트 창 팔레트의 같은 항목은 처리기가 설정 창 요소에만 있어 아무 일도 하지 않는다. 단계 게이트 「키보드만으로 편집기 끝까지」의 테스트가 이 부분을 `click`·`dispatch_action`으로 비켜 간다 | `crates/keyscore-app/src/settings_window.rs:905-919`, `assets/keymaps/default.json:503-508`, `crates/keyscore-app/tests/settings_keymap.rs:290-313`, `:335` |
| 6 | 수정 | 충돌 시트가 열린 채 `ctrl-tab`으로 구역을 옮기면 시트는 남고 포커스는 시트 밖으로 간다. 돌아와도 `escape`·`enter`가 시트에 닿지 않고 캡처도 거부된다. 시트가 열려 있으면 구역 전환을 막거나, 돌아올 때 시트로 포커스를 돌려야 한다 | `crates/keyscore-app/src/settings_window.rs:232-246`, `:312-315` |
| 7 | 수정 | KM-4: 충돌 감지가 같은 컨텍스트 문자열만 본다. 전역 Action에 맨 키(`r`)를 걸면 시트 없이 모든 컨텍스트의 `r`을 가로챈다(목록의 복원, 검색란 입력까지). 전역 대상이면 모든 컨텍스트의 같은 키·접두를 충돌로 보여야 한다 | `crates/keyscore-app/src/keymap/user.rs:79-100` |
| 8 | 수정 | 계획 7항·KM-12: 가져오기가 표준 단축키를 덮는 항목을 알리지 않는다 | `crates/keyscore-app/src/settings_window.rs:536-562` |
| 9 | 수정 | 계획 1항 「`cmd-w`로 닫는다」가 동작하지 않는다(테스트도 없다). `window::Close` 처리기가 디스패치 중인 창에 `window.update`를 걸어 늘 실패한다. 프로젝트 창도 같다 | `crates/keyscore-app/src/lib.rs:54-58` |
| 10 | 수정 | KM-12: 다른 키로 옮기거나 지워 표준 단축키(`cmd-,`, `cmd-z` …)를 `null`로 가릴 때 편집기 알림이 없다(엔진 `errors`에만 남는다) | `crates/keyscore-app/src/settings_window.rs:423-435`, `:805-811` |
| 11 | 제안 | `.bak`이 쓸 때마다 덮여 두 번째 편집에서 손으로 쓴 원본(주석)이 사라진다. 편집기가 쓴 파일이 아닐 때만 `.bak`을 갱신하거나 원본을 따로 남기는 방안 | `crates/keyscore-app/src/keymap/mod.rs:497-500` |
| 12 | 제안 | 그릴 때마다 `rows()`가 세 번(렌더, `selected_action` 두 번) 디스크의 사용자 파일을 읽고 프리셋 JSON을 다시 파싱한다 | `crates/keyscore-app/src/settings_window.rs:202-208`, `:264-294`, `:673-674` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 1–5를 고치고, 부록의 깨뜨린 테스트를 그대로 저장소에 추가해 통과시킨 뒤 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/12-S2-app-shell.md`의 나눔 메모는 계획 문서 갱신)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 깨뜨린 테스트(실패해서 `review_s2_t6a.rs`에서 뺐다)

아래 테스트는 `crates/keyscore-app/tests/review_s2_t6a.rs`의 도우미(`start`, `open_project`, `open_settings`, `select_exact`, `press`, `type_text`, `settle`, `effective_of`, `user_file`, `muted`)를 그대로 쓴다. 파일 끝에 붙이면 실행된다.

~~~rust
/// 인자 있는 Action(`step::Note`)의 줄에서 캡처해도 다른 인자의 바인딩(`s`→`step::Note(s)` 등)은 남아야 한다.
#[gpui_kit::test]
fn brk_capture_on_args_action_keeps_other_variants(cx: &mut TestAppContext) {
    let dir = start(cx, "args", None);
    let (handle, view) = open_settings(cx);
    let before = effective_of(cx, "step::Note").len();
    assert!(before > 5, "{before}");
    select_exact(cx, handle, &view, "step::Note");
    press(cx, handle, "enter ctrl-alt-n");
    settle(cx, handle);
    let after = effective_of(cx, "step::Note");
    assert!(
        after.len() >= before - 1,
        "캡처 한 번에 step::Note 바인딩이 {before}개에서 {}개로: {after:?}\n{}",
        after.len(),
        user_file(&dir)
    );
}

/// 수식키 교환을 켠 채 캡처한 키를 누르면 그 Action이 나와야 한다(KM-11, press-to-bind).
#[gpui_kit::test]
fn brk_capture_with_swap_modifiers_fires_captured_key(cx: &mut TestAppContext) {
    let dir = start(cx, "swap", None);
    let (project, pview) = open_project(cx);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter cmd-ctrl-u");
    settle(cx, handle);
    activate(cx, project);
    press(cx, project, "cmd-ctrl-u");
    assert!(
        muted(cx, &pview),
        "캡처한 cmd-ctrl-u가 뮤트하지 않는다: {}",
        user_file(&dir)
    );
}

/// 전역 바인딩(`mode::Cycle`의 `ctrl-tab`)을 지워도 설정 창의 `ctrl-tab`(`settings::NextSection`, 목록에
/// 그대로 보인다)은 살아 있어야 한다.
#[gpui_kit::test]
fn brk_clearing_global_binding_keeps_context_bindings(cx: &mut TestAppContext) {
    let dir = start(cx, "global-null", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "mode::Cycle");
    press(cx, handle, "backspace");
    let shown = effective_of(cx, "settings::NextSection");
    assert!(
        !shown.is_empty(),
        "편집기는 NextSection이 걸려 있다고 보인다"
    );
    press(cx, handle, "ctrl-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::Ai,
        "ctrl-tab이 죽었다: {shown:?}\n{}",
        user_file(&dir)
    );
}

/// 전역 Action(`settings::Open`)에 목록의 키 `r`을 걸면 충돌로 묻거나, 적어도 목록의 `r`(복원)은 살아야 한다.
#[gpui_kit::test]
fn brk_global_binding_shadowing_context_key_is_a_conflict(cx: &mut TestAppContext) {
    let dir = start(cx, "global-shadow", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    assert!(user_file(&dir).contains("track::ToggleMute") || user_file(&dir).contains("null"));
    select_exact(cx, handle, &view, "settings::Open");
    press(cx, handle, "enter r");
    settle(cx, handle);
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    if sheet {
        return;
    }
    // 시트 없이 걸렸다. 검색란에 `r`이 들어가는지, 목록의 `r`이 아직 복원인지 본다
    press(cx, handle, "/");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, "track::togglemute");
    let query_ok = selected(cx, &view).as_deref() == Some("track::ToggleMute");
    press(cx, handle, "escape");
    press(cx, handle, "/");
    type_text(cx, handle, "togglemute");
    press(cx, handle, "enter");
    assert_eq!(selected(cx, &view).as_deref(), Some("track::ToggleMute"));
    press(cx, handle, "r");

    assert!(
        !effective_of(cx, "track::ToggleMute").is_empty(),
        "시트 없이 r이 settings::Open에 먹혀 복원이 안 된다\n{}",
        user_file(&dir)
    );
    assert!(
        query_ok,
        "검색란에 r을 칠 수 없다(전역 r이 먹는다)\n{}",
        user_file(&dir)
    );
}

/// 충돌 시트가 열린 채 구역을 옮겼다 돌아와도 키보드만으로 시트를 닫을 수 있어야 한다(KM-8, P1).
#[gpui_kit::test]
fn brk_sheet_reachable_after_section_round_trip(cx: &mut TestAppContext) {
    let dir = start(cx, "sheet-stuck", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter o");
    settle(cx, handle);
    assert!(cx.read(|cx| view.read(cx).sheet_open()));
    press(cx, handle, "ctrl-tab ctrl-shift-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::Keymap
    );
    press(cx, handle, "escape");
    assert!(
        !cx.read(|cx| view.read(cx).sheet_open()),
        "시트가 남았고 escape가 닿지 않는다"
    );
    let _ = dir;
}

/// 프리셋 선택·수식키 교환을 키보드만으로(탭 이동 + space/enter) 할 수 있어야 한다(KM-8, P1).
#[gpui_kit::test]
fn brk_preset_and_swap_by_keyboard_only(cx: &mut TestAppContext) {
    start(cx, "kbd-preset", None);
    let (handle, _view) = open_settings(cx);
    let mut reached = false;
    for _ in 0..12 {
        press(cx, handle, "tab");
        press(cx, handle, "space");
        let o = cx.read(keymap::options);
        if o.preset == Preset::Vim || o.swap_alt_cmd {
            reached = true;
            break;
        }
    }
    // 팔레트(cmd-shift-p)도 설정 창에서 시도
    if !reached {
        press(cx, handle, "cmd-shift-p");
        type_text(cx, handle, "vim");
        press(cx, handle, "enter");
        reached = cx.read(keymap::options).preset == Preset::Vim;
    }
    assert!(reached, "키보드만으로 프리셋·수식키 교환에 닿지 못한다");
}

/// 표준 단축키를 덮는 키맵을 가져오면 경고해야 한다(계획 7항, KM-12).
#[gpui_kit::test]
fn brk_import_warns_standard_override(cx: &mut TestAppContext) {
    let dir = start(cx, "import-std", None);
    let (handle, view) = open_settings(cx);
    let file = dir.join("in.json");
    std::fs::write(&file, r#"[{"bindings": {"cmd-s": "track::ToggleMute"}}]"#).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(km::Import), cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    assert!(
        sheet || msg.contains("표준"),
        "표준 단축키 덮기 경고 없음: {msg:?}"
    );
}

/// 손으로 쓴 주석은 편집기가 두 번 써도 어딘가(.bak)에 남아야 한다.
#[gpui_kit::test]
fn brk_bak_keeps_hand_written_original_after_two_edits(cx: &mut TestAppContext) {
    let original = "// 내 메모\n[{\"context\": \"Normal\", \"bindings\": {\"ctrl-alt-u\": \"track::ToggleMute\"}}]\n";
    let dir = start(cx, "bak", Some(original));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::New");
    press(cx, handle, "backspace");
    let bak = std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap_or_default();
    assert!(bak.contains("내 메모"), "원본 주석이 사라졌다: {bak}");
}

#[gpui_kit::test]
fn brk_reopen_settings_after_close(cx: &mut TestAppContext) {
    start(cx, "reopen", None);
    let (project, _) = open_project(cx);
    let (handle, _view) = open_settings(cx);
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    for _ in 0..3 {
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
    }
    assert_eq!(cx.update(|cx| cx.windows().len()), 1, "창이 닫히지 않았다");
    activate(cx, project);
    press(cx, project, "cmd-,");
    assert_eq!(
        cx.update(|cx| cx.windows().len()),
        2,
        "설정 창을 닫은 뒤 cmd-,가 다시 열지 못한다(current: {})",
        cx.update(|cx| settings_window::current(cx)).is_some()
    );
}

/// 창 닫기(`window::Close`)를 키로 부르면 지금 창이 닫혀야 한다(계획 1항 「`cmd-w`로 닫는다」).
#[gpui_kit::test]
fn brk_cmd_w_closes_settings_window(cx: &mut TestAppContext) {
    start(cx, "cmdw-close", None);
    let (_project, _) = open_project(cx);
    let (handle, _view) = open_settings(cx);
    assert_eq!(cx.update(|cx| cx.active_window()), Some(handle));
    press(cx, handle, "cmd-w");
    assert_eq!(
        cx.update(|cx| cx.windows().len()),
        1,
        "cmd-w가 설정 창을 닫지 않았다"
    );
}

/// 팔레트에 보이는 「vim 프리셋」(`keymap::SelectPreset`)을 프로젝트 창의 팔레트에서 고르면 프리셋이 바뀌어야
/// 한다(P1: 키보드만으로 모든 기능, 설정 창 위 줄의 프리셋·수식키 교환에는 키가 없다).
#[gpui_kit::test]
fn brk_palette_select_preset_reaches_handler(cx: &mut TestAppContext) {
    start(cx, "pal-preset", None);
    let (project, _) = open_project(cx);
    let (_h, _v) = open_settings(cx);
    activate(cx, project);
    press(cx, project, "cmd-shift-p");
    type_text(cx, project, "vim 프리셋");
    press(cx, project, "enter");
    assert_eq!(
        cx.read(keymap::options).preset,
        Preset::Vim,
        "팔레트의 vim 프리셋이 아무 일도 하지 않는다"
    );
}
~~~

## 부록 B. 실패 출력

`cargo test -p keyscore-app --test review_s2_t6a`(대상 커밋 b57700a, 깨뜨린 테스트를 붙인 상태): 10 passed, 11 failed.

~~~
failures:

---- brk_reopen_settings_after_close stdout ----

thread 'brk_reopen_settings_after_close' (1296376) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:510:5:
assertion `left == right` failed: 설정 창을 닫은 뒤 cmd-,가 다시 열지 못한다(current: true)
  left: 1
 right: 2

---- brk_import_warns_standard_override stdout ----

thread 'brk_import_warns_standard_override' (1296373) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:476:5:
표준 단축키 덮기 경고 없음: "키맵을 가져왔다"

---- brk_cmd_w_closes_settings_window stdout ----

thread 'brk_cmd_w_closes_settings_window' (1296371) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:526:5:
assertion `left == right` failed: cmd-w가 설정 창을 닫지 않았다
  left: 2
 right: 1

---- brk_palette_select_preset_reaches_handler stdout ----

thread 'brk_palette_select_preset_reaches_handler' (1296374) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:540:5:
assertion `left == right` failed: 팔레트의 vim 프리셋이 아무 일도 하지 않는다
  left: Default
 right: Vim

---- brk_capture_on_args_action_keeps_other_variants stdout ----

thread 'brk_capture_on_args_action_keeps_other_variants' (1296368) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:340:5:
캡처 한 번에 step::Note 바인딩이 39개에서 24개로: [(Some("Step > Solfege"), "c", "step::Note(c)"), (Some("Step > Solfege"), "d", "step::Note(d)"), (Some("Step > Solfege"), "e", "step::Note(e)"), (Some("Step > Solfege"), "f", "step::Note(f)"), (Some("Step > Solfege"), "g", "step::Note(g)"), (Some("Step > Solfege") …(생략)
[
  {
    "context": "Step > Piano",
    "bindings": {
      "a": null,
      "s": null,
      "d": null,
      "f": null,
      "g": null,
      "h": null,
      "j": null,
      "k": null,
      "l": null,
      "w": null,
      "e": null,
      "t": null,
      "y": null,
      "u": null,
      "o": null,
      "p": null,
      "ctrl-alt-n": ["step::Note", "a"]
    }
  }
]


---- brk_clearing_global_binding_keeps_context_bindings stdout ----

thread 'brk_clearing_global_binding_keeps_context_bindings' (1296370) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:374:5:
assertion `left == right` failed: ctrl-tab이 죽었다: [(Some("Settings"), "ctrl-tab", "settings::NextSection")]
[
  {
    "bindings": {
      "ctrl-tab": null
    }
  }
]

  left: Keymap
 right: Ai

---- brk_capture_with_swap_modifiers_fires_captured_key stdout ----

thread 'brk_capture_with_swap_modifiers_fires_captured_key' (1296369) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:360:5:
캡처한 cmd-ctrl-u가 뮤트하지 않는다: [
  {
    "context": "Normal",
    "bindings": {
      "m": null,
      "ctrl-cmd-u": "track::ToggleMute"
    }
  }
]


---- brk_sheet_reachable_after_section_round_trip stdout ----

thread 'brk_sheet_reachable_after_section_round_trip' (1296438) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:429:5:
시트가 남았고 escape가 닿지 않는다

---- brk_bak_keeps_hand_written_original_after_two_edits stdout ----

thread 'brk_bak_keeps_hand_written_original_after_two_edits' (1296367) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:493:5:
원본 주석이 사라졌다: [
  {
    "context": "Normal",
    "bindings": {
      "ctrl-alt-u": "track::ToggleMute",
      "o": null
    }
  }
]


---- brk_preset_and_swap_by_keyboard_only stdout ----

thread 'brk_preset_and_swap_by_keyboard_only' (1296375) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:458:5:
키보드만으로 프리셋·수식키 교환에 닿지 못한다

---- brk_global_binding_shadowing_context_key_is_a_conflict stdout ----

thread 'brk_global_binding_shadowing_context_key_is_a_conflict' (1296372) panicked at crates/keyscore-app/tests/review_s2_t6a.rs:409:5:
시트 없이 r이 settings::Open에 먹혀 복원이 안 된다
[
  {
    "context": "Normal",
    "bindings": {
      "m": null
    }
  },
  {
    "bindings": {
      "cmd-,": null,
      "r": "settings::Open"
    }
  }
]
~~~
