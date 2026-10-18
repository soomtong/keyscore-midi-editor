# Task 보고: S2-T6a 설정 창과 키맵 편집기

- PR: 브랜치 `s2/t6-settings-keymap`
- 계획 파일: docs/reports/s2-t6a/plan.md
- 계획 대비 달라진 점
  - `settings_window::import_from`은 공개 함수가 아니다. 가져오기는 `keymap::Import` Action(파일 고르기 대화상자)으로만 하고, 테스트는 GPUI 시험 플랫폼의 대화상자 응답으로 본다.
  - `Effective`에 `name`·`args`가 더해졌다(인자 있는 Action의 바인딩을 같은 인자로 다시 걸기 위해).
  - 공개 API: `keymap::user_path`, `keymap::current_layers`, `keymap::save_user`, `UserKeymap::bindings`, `UserKeymap::keep_later_of_conflicts`.
  - 새 Action 7개: `keymap::SelectNext`·`SelectPrev`·`FocusSearch`, `settings::NextSection`·`PrevSection`, `dialog::Confirm`·`Cancel`(A13). 명령 표(`Surface::Settings`·`Dialog`)와 기본 프리셋 바인딩을 더했다.

## 구현 요약

- `src/keymap/user.rs`(새 파일, GPUI 무관)
  - `effective`: 표준·프리셋·사용자 레이어를 겹친 실효 바인딩. 같은 자리(공백 지운 컨텍스트, 정규화한 키)는 뒤 항목이 이기고 `null`은 지운다.
  - `binding_conflicts`: 같은 컨텍스트에서 같은 키이거나 접두 관계(양방향)인 다른 Action.
  - `UserKeymap`: 사용자 레이어. `parse`는 오류가 하나라도 있으면 거부한다(편집기가 고쳐 쓰면 읽히지 않은 항목이 사라지므로). `to_json`은 컨텍스트마다 한 절로 쓴다.
  - 편집 동작
    - `bind`: 그 컨텍스트의 그 Action 키를 새 키 하나로 바꾼다. 아래 레이어 키는 `null`로 가린다.
    - `clear`: 그 Action의 키를 모두 지운다.
    - `restore`: 그 Action으로 가는 사용자 항목과, 그 Action의 프리셋 키를 가리던 `null`을 뺀다.
    - `keep_later_of_conflicts`: 충돌마다 파일에서 앞 항목을 뺀다.
- `src/keymap/mod.rs`
  - `user_path`
  - `current_layers`: 지금 레이어와, 사용자 파일을 고쳐 써도 되는지.
  - `save_user`: 쓰기 전 내용을 `.bak`으로 남기고, 임시 파일 + rename으로 쓴 뒤 곧바로 다시 건다.
- `src/settings_window.rs`(새 파일): 앱에 하나인 설정 창.
  - 창 틀
    - 왼쪽 구역 목록(입력, MIDI 출력, 메트로놈, 키맵, AI). 키맵 밖 구역은 S2-T6c 자리표시자다.
    - `settings::Open`·`keymap::Edit`는 앱 전역 처리기라 프로젝트 창 어디서든 연다.
    - 컨텍스트는 `Settings` > `Keymap` > `Editing`, 시트는 `Dialog`다.
  - 키맵 구역
    - 위 줄: 프리셋 토글(기본/vim), 수식키 교환, 내보내기, 가져오기.
    - 검색란: 이름·제목·설명으로 거른다. `enter`·`down`으로 목록, `escape`로 검색어를 비우고 목록.
    - 목록: 명령 표의 모든 Action과 지금 걸린 키(컨텍스트 함께).
  - press-to-bind
    - `enter`로 캡처를 시작한다. 캡처 중의 키는 앱 키 가로채기(`intercept_keystrokes`)로 모든 디스패치보다 먼저 받는다. 그래서 `r`·`backspace`·`Cmd` 단축키도 Action이 되지 않는다.
    - 마지막 키 뒤 1 s(KM-5와 같다)에 확정하고, 수식키 없는 `escape`는 취소한다(KM-8).
    - 대상 컨텍스트는 그 Action의 지금(없으면 프리셋의) 첫 바인딩의 컨텍스트, 없으면 전역이다.
    - 충돌이 있으면 시트를 연다. 「새 바인딩 유지」(`enter`)는 상대 키를 지운다. 접두 충돌의 상대는 그 키를 `null`로 가린다. 「취소」는 `escape`다.
    - 표준 단축키를 덮으면 경고 문구를 남긴다(KM-12).
  - 가져오기: 읽고, 같은 레이어 충돌을 적용 전에 시트로 묻는다(취소 / 뒤 항목 남기고 적용). 문법·항목 오류 파일은 거부한다.
  - 내보내기: 지금 사용자 레이어를 고른 경로에 쓴다.
  - 사용자 파일에 오류가 있으면 편집기는 고쳐 쓰지 않고 이유를 알린다.

## 테스트 결과

~~~
cargo test --workspace: 1354 passed, 0 failed, 0 ignored
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `user_keymap_round_trips_and_edits`, `user_keymap_parse_rejects_syntax_errors_and_keeps_unbind` | 통과 |
| 2 | `effective_bindings_layer_and_unbind`, `binding_conflicts_same_keys_and_prefix` | 통과 |
| 3 | `settings_open_and_sections` | 통과 |
| 4 | `keymap_editor_keyboard_only_flow`(단계 게이트) | 통과 |
| 5 | `capture_escape_cancels_and_keys_do_not_dispatch` | 통과 |
| 6 | `binding_conflict_sheet_asks_which_to_keep`, `prefix_conflict_opens_sheet` | 통과 |
| 7 | `preset_switch_keeps_user_overrides` | 통과 |
| 8 | `import_conflicts_are_caught_before_apply`(단계 게이트) | 통과 |
| 9 | `export_writes_user_layer` | 통과 |
| 10 | `editor_refuses_to_overwrite_broken_user_file` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`

## 알려진 한계

- 편집기가 `keymap.json`을 다시 쓰면 주석과 순서가 사라진다. 쓰기 전 내용은 `keymap.json.bak`에 남는다(쓸 때마다 덮는다).
- 같은 Action을 여러 컨텍스트에 따로 거는 일, 인자가 필요한데 지금 바인딩이 없는 Action(`step::Note(인자)` 등)을 거는 일은 JSON 손 편집으로 한다. 인자 없이 걸면 엔진이 그 항목을 오류로 알린다.
- 바탕 레이어(gpui-component) 바인딩은 목록에 보이지 않는다(S2-T6c 이월).
- 결정 대기 키(A1–A12, B24, B25)는 특별히 다루지 않는다.
- 목록은 그릴 때마다 레이어를 다시 읽는다(약 300개 Action). 느리면 캐시한다.

## 리뷰어에게

- 캡처를 앱 키 가로채기로 한 것. 다른 창의 키에는 영향이 없어야 하고(창 핸들 비교), 캡처가 아닐 때는 아무것도 가로채지 않아야 한다.
- `bind`가 접두 충돌의 상대를 지우는 방식(`clear_place`로 그 자리만 `null`)과 같은 키 충돌(새 항목이 그 자리를 덮음).
- 실효 바인딩 계산은 GPUI 우선순위(깊이)를 흉내 내지 않고 같은 자리만 본다. 사용자의 얕은 컨텍스트가 프리셋의 깊은 컨텍스트를 덮지 못하는 경우(엔진 문서의 예외)는 목록 표시와 실제가 다를 수 있다.
