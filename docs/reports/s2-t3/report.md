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
cargo test --workspace --no-fail-fast: 980 passed, 0 failed (리뷰 1 대응 뒤 993, A13·리뷰 2 대응 뒤 1005 passed, 0 failed)
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

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1(차단): `Ctrl+Tab`·`Cmd+R`이 뷰·선택을 바꾼다 | 「노트 입력 모드는 피아노롤로」 규칙을 뺐다. REQ에 없는 결정을 더하는 대신 5.11 문장 그대로 「모드만 바뀐다」로 줄였으므로 결정 요청은 쓰지 않았다. 상태는 32개(뷰 2 × 모드 7 × 선택 2 + Step 고스트 4). `mode.rs` `with_mode` |
| 지적 2: 메뉴가 열린 채 키가 문서로 간다 | 메뉴를 컨텍스트 겹 바깥(루트의 자식)에 그린다. 메뉴 포커스의 디스패치 경로에 문서 컨텍스트가 없다. 위치는 상태줄 왼쪽 위(`mode_menu_overlay`) |
| 지적 3: `ai::Accept`·`Dismiss`가 AI 밖에서 Normal로 보낸다 | 사건 `LeaveAi`를 더했다. AI 모드에서만 Normal로 간다 |
| 제안 1: `install` 두 번이면 사용자 레이어가 바탕에 얼어붙는다 | 이미 설치돼 있으면 처음 뜬 바탕 레이어를 그대로 쓴다 |
| 제안 2: 처리기 없는 `tab`이 `root::Tab`으로 떨어진다 | S2-T5 이월로 단계 파일에 적었다 |
| 제안 3: 바탕 레이어의 `alt-cmd-i` | S2-T6 이월로 단계 파일에 적었다 |
| 제안 4: 한 바퀴 검사를 상태 전체로 | `cycle_visits_normal_step_sequence_play`가 상태 전체를 비교한다 |

부록 A1–A5는 `tests/review_s2_t3.rs`로 옮겨 모두 통과한다. `modes_ui::ghost_escape_under_policy_c_stays_in_step`은 빠진 규칙에 기대던 시작 상태(배치 뷰에서 Step → 피아노롤)를 `OpenClip`으로 명시했다.

## A13 결정과 리뷰 2 대응

| 항목 | 대응 |
| --- | --- |
| A13 결정(사용자, 「통상의 취소, 그 외 No-op」) | Fable 5.1 검토로 컨텍스트별 표를 만들어 결정 문서에 적었다. 프리셋 `escape`: `Step > Ghost` → `ai::DismissGhost`, 입력 모드 → `mode::Normal`, `Normal > Selection` → `select::Clear`, 선택 없는 Normal은 없음. `mode::Back`과 사건 `Back`을 뺐다(Action 286개). REQ P2·5.2·5.3·5.5·5.6·5.8·5.10·UI-2 문구를 고쳤다(개발 단계 문구 수정 승인). 새 Action이 필요한 컨텍스트(팔레트·대화상자·오버레이·AI 요청·vim 오퍼레이터)는 S2-T4·S2-T6·S3·S7 이월 |
| 리뷰 2 지적 1: 모드 진입 때의 뷰가 REQ 결정 없이 정해졌다 | 결정 `docs/decisions/2026-10-17-mode-entry-view.md`를 썼다. Step·Sequence·Play·AI는 모드만(5.11, 5.10), Lyric·Controller는 피아노롤로(5.15, 5.9). 부록 A1을 `review_s2_t3_r2.rs`로 옮겨 통과(나가기는 `Esc`로 바꿈) |
| 제안 1: 계획 인터페이스에 `LeaveAi` 누락 | 고쳤다 |
| 제안 2: 모드 전환 뒤 남는 선택과 Esc | A13 결정으로 입력 모드의 `Esc`는 먼저 Normal로 가고(선택 유지), 다음 `Esc`가 선택을 지운다. 상태줄이 `STEP` 그대로 남는 경우는 없다 |
| 제안 3: 모드 메뉴 위치 근삿값 | S2-T6 이월(창 최소 크기와 함께) |
| 제안 4: 메뉴가 열린 동안의 전역 바인딩 | 의도로 계획 불변식에 적었다 |

테스트 변경: `mode_fsm`의 `back_repeated_…`, `escape_policies_…`는 결정된 정책의 `escape_cancels_innermost_thing_once`, `escape_mashing_converges_to_normal_without_changing_view`로 고쳐 썼다. `modes_ui`의 선택지 a·c 사용자 키맵 테스트는 프리셋 기준 테스트(기본·vim)와 단계 순서 테스트로 고쳐 썼다.

## 리뷰 3 대응

리뷰 3은 세 번째 재리뷰다. 코드 결함은 없었고 지적 두 건은 문서였다.

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: REQ 5.5 Esc 순서 문장이 해석 표와 반대 | 「진행 중 조작(드래그·키 캡처·AI 요청) → 팝업·패널 → AI 고스트 → 입력 모드 → 선택」으로 고쳤다 |
| 지적 2: 해석 두 행(조합 중 입력, 레이아웃 패널)을 사람 확인 없이 REQ에 넣었다 | 사람에게 물어 확인받았다(2026-10-17, YoungTaek): 조합 중 Esc는 확정 후 Normal, 레이아웃 패널은 닫지 않는다. A13 절에 적었다. 리뷰 3 부록 A1(레이아웃 패널을 닫는다고 기대)은 이 결정과 반대라 옮기지 않는다 |
| 제안 1: 단계 파일 S2-T3 게이트 문구 | A13 반영 한 줄을 붙였다 |
| 제안 2: plan.md 어긋남 | `Enter(m)` 행, 불변식, 결정 대기 문구를 고쳤다 |
| 제안 3: REQ 5.11 「어디서든」 | 「어느 입력 모드에서든」으로 고쳤다 |
| 제안 4: 카탈로그 통계·결정 문서 상태 | 고쳤다 |

지적 2가 사람 결정으로 닫혔으므로 프로토콜 [8]의 결정 요청은 이 확인으로 갈음하고, 지적 1·제안 반영만 좁게 확인받는다(S1-T6 선례와 같은 방식).
