# Task 보고: S2-T3 모드 FSM과 Enter/Esc/Tab 규약

- PR: 브랜치 `s2/t3-modes`
- 계획 파일: docs/reports/s2-t3/plan.md
- 계획 대비 달라진 점
  - 불변식 하나를 더했다: 노트 입력 모드(Step, Sequence, Lyric)는 피아노롤에만 있다. 계획의 「규칙」(배치 뷰에서 들어가면 피아노롤로 바꾼다)에서 바로 나오는 것이라 상태 수(24)와 전수 테스트에 넣었다.
  - 테스트를 하나 더했다: `unbound_tab_keeps_window_focus`(바탕 레이어의 `root::Tab`이 창 포커스를 빼앗지 않는다).
  - 바탕 레이어를 키맵 편집기(S2-T6)가 볼 수 있게 `keymap::base_bindings`를 공개했다.
  - 기존 리뷰 테스트 하나(`review_s2_t2_r2.rs`의 `check_single_keys`)의 기대 모델에 바탕 레이어를 더했다. 아래 「리뷰어에게」 3.

## 구현 요약

- `crates/keyscore-app/src/mode.rs`(새 파일): GPUI 없는 모드 FSM
  - `View`, `Mode`, `ModeEvent`, `ModeState`(필드 비공개)
  - `apply`가 전이 표를 구현하고, `context_path`가 상태별 키 컨텍스트 경로를 준다. `all()`은 불변식을 지키는 상태 24개다.
- `crates/keyscore-app/src/window.rs`
  - `ProjectWindow`가 `ModeState`와 열린 모드 메뉴를 갖는다.
  - 렌더는 컨텍스트 경로 길이만큼 `div`를 겹치고, 가장 안쪽이 창 포커스를 잡는다. 루트의 `Normal` 고정 컨텍스트를 대신한다.
  - 처리기: `mode::*` 전부, `view::Toggle`, `clip::Open`, `select::Clear`, `ai::OpenPanel`·`Accept`·`Dismiss`·`DismissGhost`. 모두 FSM 사건 하나를 넣는다.
  - 상태줄 왼쪽은 모드 이름(접근성 레이블 포함)이다. 누르거나 `mode::ShowMenu`를 실행하면 gpui-component `PopupMenu`가 열린다. 항목은 P2의 다섯 모드이고, 고르면 카운트를 먼저 비운다(결정 C).
  - 메인 영역 자리표시자는 현재 뷰 이름을 보인다.
- `crates/keyscore-app/src/keymap/mod.rs`
  - `install`이 그때 걸려 있던 바인딩(gpui-component가 `init`에서 건 것)을 바탕 레이어로 떠 둔다.
  - `apply`는 `clear_key_bindings` 뒤에 바탕 레이어를 가장 먼저 다시 건다.
  - 이것이 없으면 S2-T2 이후 `PopupMenu`·`Input`·`Dialog`의 키가 모두 사라져 있었다(모드 메뉴를 키보드로 조작할 수 없었다).

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 980 passed, 0 failed
mode_fsm: 7 passed
modes_ui: 8 passed
review_s2_t2_r2: 23 passed (기대 모델 수정 후)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `mode_fsm::transition_table_matches_req_5_5` | 통과 |
| 2 | `mode_fsm::every_state_upholds_invariants_after_any_event` | 통과(24개 상태 × 17개 사건, 시작 상태에서 전부 닿음) |
| 3 | `mode_fsm::back_repeated_converges_to_arrangement_without_selection` | 통과 |
| 4 | `mode_fsm::escape_policies_converge_from_every_state` | 통과(A13 a·b·c) |
| 5 | `mode_fsm::cycle_visits_normal_step_sequence_play` | 통과 |
| 6 | `mode_fsm::context_path_per_state` | 통과 |
| 7 | `modes_ui::keys_drive_modes_through_key_contexts` | 통과 |
| 8 | `modes_ui::escape_mashing_converges_under_policy_a`, `_policy_c`, `ghost_escape_under_policy_c_stays_in_step` | 통과 |
| 9 | `modes_ui::status_bar_shows_mode_and_menu_switches_mode` | 통과 |
| 10 | `modes_ui::component_bindings_survive_keymap_apply` | 통과 |
| 11 | `modes_ui::main_view_placeholder_names_current_view` | 통과 |
| (추가) | `modes_ui::unbound_tab_keeps_window_focus` | 통과 |
| (추가) | `mode_fsm::labels_name_every_mode` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(980 passed)
- [x] deny
- [x] check-deps

## 알려진 한계

- `escape` 프리셋 바인딩은 비어 있다(A13 결정 대기). FSM은 세 선택지가 쓰는 Action을 모두 제공하고, 선택지 a·c를 사용자 키맵으로 건 UI 테스트가 수렴을 확인한다. 결정이 나면 `default.json`·`vim.json`에 `escape` 몇 줄을 넣고, 선택지 UI 테스트를 프리셋 기준으로 바꾼다.
- 선택(`Select`)과 고스트(`ShowGhost`)를 일으키는 Action은 아직 없다(S3, S7). 테스트는 `apply_mode_event`로 넣는다.
- `ai::Accept`·`ai::Dismiss` 처리기는 지금 Normal로 돌아가기만 한다. 결과 수락·버림은 S7이 같은 처리기에 붙인다.
- 상태줄은 모드 이름만 보인다. `STEP ♩ oct4`의 길이·옥타브는 S3.
- 모드 메뉴가 열린 채로 키로 모드가 바뀌면 메뉴의 체크 표시는 열 때의 모드를 가리킨다. 메뉴는 다시 열 때 새로 만든다.

## 리뷰어에게

1. 컨텍스트 겹치기(`wrap_in_contexts`). 경로 길이가 상태에 따라 1–3으로 바뀌고, 포커스를 잡는 요소의 깊이도 바뀐다. 같은 `FocusHandle`을 다른 깊이의 요소가 잡을 때 포커스·대기 중인 키 시퀀스(`g g` 등)가 어떻게 되는지 의심해 볼 곳이다.
2. 전이 표의 세부 규칙 네 가지(계획의 「규칙」): 뷰가 바뀌면 선택을 비움, 고스트는 Step에만, 노트 입력 모드는 피아노롤로, 모드 진입 Action은 어느 모드에서든 그 모드로. REQ 5.5·5.10·5.11에서 읽어 낸 것이다.
3. 바탕 레이어 보존은 의도한 동작 변화다. 그래서 `review_s2_t2_r2.rs::check_single_keys`가 「우리 레이어에 바인딩이 없는 키는 아무것도 디스패치하지 않는다」고 기대하던 곳에서, gpui-component 바인딩(`root::Tab`, `Dialog`의 `ui::Confirm`·`ui::Cancel` 등)이 디스패치된다.
   - 테스트를 지우지 않고, 기대 모델에 「그 키·그 Action의 바탕 레이어 바인딩이면 맞다」를 더했다.
   - 컴포넌트 `Dialog` 컨텍스트 이름이 우리 키맵의 `Dialog`와 같다. 우리 대화상자도 gpui-component `Dialog`를 쓸 것이라 같은 컨텍스트가 맞다고 본다.
4. 모드 메뉴 항목 처리기는 `keymap::clear_count`를 직접 부른다. S2-T4의 실행 함수로 옮길 자리다.
