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

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 차단 1: 인자 있는 Action(`step::Note`)에서 캡처하면 다른 인자의 키가 모두 사라진다 | `bind`는 같은 인자(라벨)의 키만 바꾼다. 인자마다 바인딩이 따로 있는 Action은 목록에서 어느 것을 바꿀지 고를 수 없어 캡처를 거절하고 keymap.json 손 편집을 안내한다(`argument_variants`) |
| 차단 2: 수식키 교환을 켠 채 캡처한 키가 걸리지 않는다(KM-11) | 교환이 켜져 있으면 누른 키를 교환해 파일에 쓴다(엔진이 다시 교환해 누른 키로 건다). 목록도 실제로 누르는 키로 보인다 |
| 차단 3: 전역 `null`이 모든 컨텍스트의 같은 키를 죽이는데 목록은 살아 있다고 보인다 | 원인은 GPUI `NoAction`이 우선순위가 낮은 같은 키를 Action과 무관하게 모두 끄는 것이다. 편집기는 `null` 대신 GPUI의 Action 지정 해제 `["zed::Unbind", "ns::Name"]`를 쓴다. 그 Action만 끄고 같은 키의 다른 Action은 산다. 실효 바인딩 계산도 `null`(같거나 얕은 컨텍스트, 전역이면 모두)과 지정 해제를 GPUI처럼 다룬다. 충돌 검사는 지정 해제를 세지 않는다 |
| 차단 4: 설정 창을 닫으면 `cmd-,`로 다시 열리지 않는다 | `current`가 창이 아직 있는지 본다 |
| 차단 5: 프리셋·수식키 교환·내보내기·가져오기에 키 경로가 없다(KM-8, P1) | 키맵 구역에 키를 둔다: `alt-1`/`alt-2` 프리셋, `alt-m` 수식키 교환, `cmd-shift-e` 내보내기, `cmd-shift-i` 가져오기. `SelectPreset`·`SwapModifiers`는 앱 전역 처리기로 옮겨 프로젝트 창 팔레트에서도 된다. 내보내기·가져오기는 다른 창에서 부르면 설정 창을 열어 넘긴다 |
| 수정 6: 시트가 열린 채 구역을 옮겼다 오면 키보드로 닫을 수 없다 | 구역을 옮기면 시트를 닫는다(알림을 남긴다) |
| 수정 7: 충돌 검사가 같은 컨텍스트 문자열만 본다(KM-4) | 한 경로에서 함께 맞을 수 있는 컨텍스트(같음, 바깥·안쪽, 전역)의 같은 키·접두를 충돌로 본다. 「새 바인딩 유지」는 그 상대를 지정 해제로 끈다 |
| 수정 8: 가져오기가 표준 단축키 덮기를 경고하지 않는다 | 표준 단축키를 덮는 항목이 있으면 충돌과 함께 적용 전 시트에 보인다 |
| 수정 9: `cmd-w`로 창이 닫히지 않는다 | `window::Close` 처리기가 디스패치가 끝난 뒤(`defer`) 창을 닫는다 |
| 수정 10: 키를 옮기거나 지워 표준 단축키가 꺼져도 알림이 없다 | `bind`·`clear`가 표준 단축키를 껐으면 편집기 알림에 경고를 붙인다 |
| 제안 11: `.bak`이 저장마다 덮여 손으로 쓴 원본이 사라진다 | 편집기가 마지막으로 쓴 내용과 다를 때(사람이 고쳤을 때)만 `.bak`을 쓴다 |
| 제안 12: 그릴 때마다 디스크·프리셋을 다시 읽는다 | 알려진 한계로 둔다(약 300줄, 체감 지연 없음). 느리면 캐시한다 |

부록 A 테스트는 `tests/review_s2_t6a.rs`로 옮겨 통과한다. 전제를 바꾼 것:
- `brk_preset_and_swap_by_keyboard_only`: 탭 이동 + space 대신 이번에 둔 키(`alt-2`, `alt-m`)를 쓴다. 수식키 교환이 켜지면 프리셋 키의 `alt`가 `cmd`가 되므로(KM-11) 끄는 키는 `cmd-m`이다.
- `brk_cmd_w_closes_settings_window`: 창이 닫혀 그 창을 그릴 수 없으므로 그리지 않는 키 보내기를 쓴다.
- `capture_swallows_standard_shortcuts_and_warns`(깨지지 않은 시도): 지적 7 대응으로 `cmd-w`가 전역 `window::Close`와 부딪혀 시트가 먼저 뜬다. `enter`로 유지한 뒤 같은 것을 본다.

## 리뷰 2 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 수식키 교환을 켜면 편집기와 엔진이 표준 단축키를 다른 키로 본다 | 편집기는 모든 키를 실제로 누르는 키로 다룬다. 실효 바인딩(`effective_with`)은 엔진처럼 표준 밖 레이어의 키를 교환해 계산하고, 표준 레이어와 같은 자리에서 비교한다. `UserKeymap`은 실제 키를 받아 파일에 쓸 때만 교환한다(`set_swap`). 그래서 표준 단축키의 지정 해제도 엔진이 다시 교환해 그 키에 걸린다. 가져오기의 표준 덮기 검사도 엔진이 거는 키로 본다. `r`(복원)은 표준 레이어의 지정 해제도 되돌린다 |
| 지적 2: 지운 바인딩이 되살아난다(뮤트 `m` 지움 → 솔로에 `m` → 솔로 지움 → 뮤트 `m` 복귀) | 새 바인딩은 그 자리의 지정 해제를 지우지 않는다. 같은 자리에 지정 해제와 바인딩이 함께 있을 수 있고, 파일에는 같은 자리의 k번째 항목을 그 컨텍스트의 k번째 절로 써서 앞뒤를 지킨다. 「새 바인딩 유지」는 같은 자리의 다른 Action도 지정 해제로 끈다 |
| 제안 3: `.bak` 정책이 앱을 다시 띄우면 지켜지지 않는다 | 메모리 대신 내용으로 판단한다. 다시 읽어 쓰면 글자 그대로 같은 파일은 편집기가 쓴 것이고, 아니면 사람이 고친 것이라 `.bak`으로 남긴다 |
| 제안 4: 컨텍스트 있는 `null`·지정 해제가 전역 바인딩도 가린다고 계산한다 | GPUI처럼 전역 바인딩은 컨텍스트 있는 `null`보다 앞서므로 가리지 않게 고쳤다 |

부록 A 테스트는 `tests/review_s2_t6a_r2.rs`로 옮겨 통과한다. 리뷰 테스트의 도우미 `effective_of`는 편집기와 같은 기준(수식키 교환 반영)으로 바꿨다. 내 단위 테스트 `user_keymap_round_trips_and_edits`의 「ToggleMute 복원 때 ToggleSolo의 `s`가 돌아온다」는 지적 2와 반대 전제라, 「ToggleSolo를 복원해야 돌아온다」로 고쳤다.

## 리뷰 3 대응

재리뷰 한도에 닿아 결정 요청을 냈다(`docs/decisions/2026-10-18-s2-t6a-review-limit.md`). 결정 A에 따라 지적 1을 고치고 좁은 확인 리뷰를 받는다.

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 표준 단축키를 지운 뒤 수식키 교환을 바꾸면 지운 단축키가 되살아난다 | `keymap::set_swap_alt_cmd`가 교환을 바꿀 때 사용자 파일에서 표준 단축키를 겨눈 지정 해제(`zed::Unbind`)의 파일 키를 새 교환 상태에 맞게 고쳐 쓴다(`UserKeymap::retarget_standard_unbinds`). 프리셋을 겨눈 지정 해제는 엔진이 프리셋과 함께 교환하므로 그대로 둔다. 파일이 깨졌으면 손대지 않는다 |
| 제안 2: 손으로 고친 파일의 절 순서·컨텍스트 있는 `null` 근사 | 알려진 한계로 둔다. 편집기만으로는 만들 수 없다 |
| 참고: `keyscore-text`의 시간 측정 테스트가 전체 실행 중 한 번 실패 | 이 Task 밖이다. 다시 보이면 별도로 다룬다 |

부록 A 테스트는 `tests/review_s2_t6a_r3.rs`로 옮겨 통과한다.
