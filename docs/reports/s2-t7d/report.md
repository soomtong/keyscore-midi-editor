# Task 보고: S2-T7d 없는 출력 포트의 대체 매핑 제안(OUT-8)

- PR: 로컬 브랜치 `s2/t7d-port-mapping`
- 계획 파일: docs/reports/s2-t7d/plan.md
- 계획 대비 달라진 점: `ask_port_mapping`이 줄 목록을 빌린다(clippy). 그 밖에는 없음.

## 구현 요약

- `crates/keyscore-app/src/port_mapping.rs`(새 모듈)
  - `plan`: 트랙이 이름으로 가리키는 포트 가운데, 앱 가상 포트 이름과도 지금 보이는 목적지 이름과도 정확히 같지 않은 것마다 한 줄을 낸다(트랙 순서, 같은 이름은 한 줄, 트랙 수 포함). 제안은 `Use`(보이는 모양이 같은 기존 포트, 앱 포트가 목적지보다 먼저), `Create`(정규화 이름, 같은 시트의 같은 정규화 이름 둘째 줄은 `Use`), `Main`(이름 규칙 위반, 16개 상한) 셋 중 하나다.
  - `describe`: 시트의 줄 설명. 제어 문자는 `\u{..}`로 보인다.
  - `plan_for`: 엔진의 앱 포트 이름과 `destinations()`로 계산한다. 엔진이 없으면 `None`.
  - `offer`: 없는 포트가 있으면 그 창의 파일 흐름(`run_file_flow`)으로 시트를 띄운다. [제안대로 매핑]/[그대로 두기].
  - `apply`: 적용할 때 다시 계산한다. 가상 포트를 만들고, 트랙 변경(`Use`, 이름이 바뀐 `Create`, `Main` → `Virtual`)을 `edit_document` 한 번(undo 한 단위)으로 넣는다. 만들지 못한 포트를 `Use`로 가리키는 줄은 바꾸지 않는다. 결과는 상태줄에 요약한다.
  - `ProjectWindow::midi_map_missing_ports`: `midi::MapMissingPorts` 처리기. 엔진이 없으면 「MIDI 엔진이 없다…」, 없는 포트가 없으면 「없는 출력 포트가 없다」.
- `engine.rs` `add_named_virtual_port`: 주어진 이름으로 추가 가상 포트를 만들고 설정에 저장한다(`check_new_name` + keyscore-midi의 중복·상한 검사, `ports_edited`).
- `files.rs` `open_path`(새 창을 연 경우만)와 `autosave.rs` `recover`(복구한 창마다)가 `port_mapping::offer`를 부른다.
- 새 Action `midi::MapMissingPorts`(트랜스포트 메뉴 4묶음 「없는 포트 대체 매핑…」, 팔레트). `ALL_ACTION_NAMES`, 명령 표, 카탈로그(`docs/reports/s2-t2/action-catalog.md`)를 갱신했다.
- `plan/stages/12-S2-app-shell.md`: 나누기 줄(T7d = OUT-8, T7e = 이월 줄)과 이월 줄의 대상 Task.

## 테스트 결과

~~~
cargo test --workspace: 테스트 바이너리 207개 모두 ok, 1811 passed, 0 failed, 2 ignored(기존)
cargo test -p keyscore-app --test port_mapping: 16 passed; 0 failed
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `plan_missing_name_suggests_virtual_port_with_same_name` | 통과 |
| 2 | `plan_skips_virtual_own_ports_and_destinations` | 통과 |
| 3 | `plan_lists_each_name_once_in_track_order` | 통과 |
| 4 | `plan_visually_same_port_retargets_tracks` | 통과 |
| 5 | `plan_unnormalized_name_creates_normalized_port` | 통과 |
| 6 | `plan_invalid_name_falls_back_to_main` | 통과 |
| 7 | `plan_over_limit_falls_back_to_main` | 통과 |
| 8 | `describe_names_action_and_track_count` | 통과 |
| 9 | `open_file_with_missing_port_offers_sheet_and_creates_port` | 통과 |
| 10 | `leave_as_is_changes_nothing` | 통과 |
| 11 | `open_file_without_missing_port_has_no_sheet` | 통과 |
| 12 | `retargeted_tracks_are_one_undo_unit` | 통과 |
| 13 | `map_missing_ports_action_reopens_sheet_or_says_none` | 통과 |
| 14 | `apply_recomputes_when_port_appeared_meanwhile` | 통과 |
| 15 | `no_engine_no_sheet` | 통과 |
| 16 | `recovered_window_offers_sheet` | 통과 |
| 17 | 기존 명령 표·UI-11 시험 | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] `scripts/check-deps.sh`, `check-deps.test.sh`(16), `bench-check.test.sh`(8)

## 알려진 한계

- 만든 가상 포트는 undo로 되돌리지 않는다(설정 > MIDI 출력에서 지운다). 트랙 변경만 undo 한 단위다.
- 메트로놈·클럭 포트(설정)는 대상이 아니다.
- 시트는 줄마다 고르지 않는다(결정 2 A). 일부만 원하면 [그대로 두기] 뒤 인스펙터에서 트랙 포트를 고른다.
- 적용할 때 다시 계산하므로, 시트가 떠 있는 동안 상태가 바뀌면 시트에 보인 것과 다르게 적용될 수 있다(결과는 상태줄 요약에 나온다).
- 같은 파일을 이미 연 창으로 돌아가는 열기는 다시 묻지 않는다(명령으로 연다).
- S2-T7b·S2-T7c 이월 줄은 S2-T7e.

## 리뷰어에게

- `plan`의 「없음」 판정은 정확히 같은 이름이고 제안은 보이는 모양으로 비교한다. 엔진(`open_port`/`best_match`)과 결과가 맞는지, 앱 포트와 목적지가 겹칠 때의 차례를 봐 주기 바란다.
- `apply`가 다시 계산할 때 이미 다른 창이 같은 정규화 이름의 포트를 만든 경우와, 만들기가 실패한 경우의 트랙 변경.
- `open_path` 안에서 시트를 띄우므로 `open_paths`의 실패 시트 호스트 고르기·손대지 않은 원래 창 닫기와 겹치는지.
- 복구 흐름(`recover`)은 호스트 창의 흐름 안에서 돈다. 복구한 창마다 시트가 따로 뜬다.
