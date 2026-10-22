# Task 계획: S2-T7d 없는 출력 포트의 대체 매핑 제안(OUT-8)

- 브랜치: `s2/t7d-port-mapping`
- 관련 REQ: OUT-8(포트 이름 저장, 없는 포트는 가상 포트로 대체 매핑 제안), OUT-2(가상 포트 최대 16, 이름은 사용자가 정함), OUT-3(같은 이름 재연결), UI-11(모든 명령에 UI)
- 선행 Task: S1-T2(`suggest_mapping`), S2-T6d(앱 추가 가상 포트), S2-T7a(열기 흐름), S2-T7b(복구)
- 관련 결정: `docs/decisions/2026-10-22-out8-port-mapping-sheet.md`(1 A 열 때 시트, 2 A 같은 이름 가상 포트, 나누기: 이월 줄은 S2-T7e)

## 끝의 정의 (내 말로)

1. 순수 계산 `port_mapping::plan(project, own, destinations) -> Vec<Line>`: 트랙이 이름으로 가리키는데 앱 가상 포트·목적지 어디에도 정확히 같은 이름이 없는 포트마다 한 줄(트랙 순서, 같은 이름 한 줄, 그 이름을 쓰는 트랙 수). 줄의 제안은 결정 문서 「구현 세부」대로 `Use`(보이는 모양이 같은 기존 포트로 트랙 변경), `Create`(가상 포트 만들기, 정규화 이름이 다르면 트랙도 변경, 같은 시트의 같은 정규화 이름 둘째 줄은 `Use`), `Main`(이름 규칙 위반·16개 상한이면 기본 포트로 트랙 변경).
2. 엔진 `add_named_virtual_port(cx, name)`: 주어진 이름으로 추가 가상 포트를 만들고 설정에 저장한다(`add_virtual_port`와 같은 검사·후처리).
3. 파일을 연 새 창(`files::open_path`)과 복구한 창(`autosave::recover`)에 없는 포트가 있고 엔진이 있으면 시트를 띄운다. 「이 Mac에 없는 출력 포트가 n개 있다」, 줄마다 설명, [제안대로 매핑]/[그대로 두기]. 시트는 그 창의 파일 흐름(`file_flow`)으로 띄워 다른 시트와 겹치지 않는다.
4. [제안대로 매핑]: 그때 상태로 다시 계산 → 가상 포트를 만들고 → 트랙 변경을 undo 한 단위로 → 상태줄 요약(만든 포트 수, 바꾼 트랙 수, 실패).
5. 새 Action `midi::MapMissingPorts`(트랜스포트 메뉴, 팔레트): 같은 시트를 다시 연다. 없는 포트가 없거나 엔진이 없으면 상태줄로 알린다. Action 목록·명령 표·카탈로그 갱신(297개).

## 공개 인터페이스 초안

~~~rust
// keyscore_app::port_mapping
pub enum Mapping { Use(String), Create(String), Main(MainReason) }
pub enum MainReason { InvalidName, TooMany }
pub struct Line { pub saved: String, pub mapping: Mapping, pub tracks: usize }
pub fn plan(project: &Project, own: &[String], destinations: &[String]) -> Vec<Line>;
pub fn describe(line: &Line, main: &str) -> String;
pub fn plan_for(cx: &App, project: &Project) -> Option<Vec<Line>>; // 엔진이 없으면 None
pub fn offer(view: &Entity<ProjectWindow>, cx: &mut App);       // 없는 포트가 있으면 시트

// keyscore_app::engine
pub fn add_named_virtual_port(cx: &mut App, name: &str) -> Result<String, PortEditError>;

// keyscore_app::actions::midi
MapMissingPorts
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `plan_missing_name_suggests_virtual_port_with_same_name` | 단위 | 없는 이름 → `Create(같은 이름)`, 트랙 수 |
| 2 | `plan_skips_virtual_own_ports_and_destinations` | 단위 | `Virtual`, 기본·추가 앱 포트, 목적지 이름은 줄이 없다 |
| 3 | `plan_lists_each_name_once_in_track_order` | 단위 | 순서, 중복 제거, 트랙 수 |
| 4 | `plan_visually_same_port_retargets_tracks` | 단위 | 모양이 같은 앱 포트·목적지 → `Use(그 이름)` |
| 5 | `plan_unnormalized_name_creates_normalized_port` | 단위 | 공백·NFD 이름 → `Create(정규화)`, 둘째 같은 정규화 이름 → `Use` |
| 6 | `plan_invalid_name_falls_back_to_main` | 단위 | 제어 문자 이름 → `Main(InvalidName)` |
| 7 | `plan_over_limit_falls_back_to_main` | 단위 | 앱 포트 15개 + 없는 이름 둘 → `Create`, `Main(TooMany)` |
| 8 | `describe_names_action_and_track_count` | 단위 | 줄 설명 문구 |
| 9 | `open_file_with_missing_port_offers_sheet_and_creates_port` | UI(CoreMIDI) | 열면 시트, 매핑하면 가상 포트·설정, 트랙·바뀜 그대로 |
| 10 | `leave_as_is_changes_nothing` | UI | [그대로 두기]는 포트·문서를 바꾸지 않는다 |
| 11 | `open_file_without_missing_port_has_no_sheet` | UI | 시트 없음 |
| 12 | `retargeted_tracks_are_one_undo_unit` | UI | `Main`·`Use` 변경이 undo 한 번으로 돌아온다 |
| 13 | `map_missing_ports_action_reopens_sheet_or_says_none` | UI | 명령이 시트를 다시 띄우고, 없으면 상태줄 |
| 14 | `apply_recomputes_when_port_appeared_meanwhile` | UI | 시트가 떠 있는 동안 다른 경로로 포트가 생기면 중복 생성·오류 없음 |
| 15 | `no_engine_no_sheet` | UI | 엔진이 없으면 시트 없음, 명령은 알림 |
| 16 | `recovered_window_offers_sheet` | UI | 복구한 창에도 시트 |
| 17 | 기존 UI-11·명령 표·Action 목록 시험 | 통합 | 새 Action이 메뉴에 있고 목록 개수가 맞다 |

## 불변식

- 트랙 변경은 `ProjectWindow::edit_document`(문서의 undo 엔진)만 거친다.
- 가상 포트는 엔진의 기존 검사(`check_new_name`, keyscore-midi 중복·16개 상한)를 거친다. 시트가 떠 있는 동안 상태가 바뀌어도 적용 때 다시 계산하므로 같은 모양의 포트가 둘 생기지 않는다.
- 창마다 흐름 하나: 시트는 `run_file_flow`로 띄운다.

## 범위 밖

- S2-T7b 리뷰 4·S2-T7c 리뷰 2 이월 줄(S2-T7e).
- 메트로놈·클럭 포트(설정)의 대체 매핑. 설정 > MIDI 출력이 이미 고른다.
- 줄마다 다른 포트를 고르는 UI(결정 2 C를 고르지 않았다).
- 만든 가상 포트의 undo.

## 리스크·결정 요청

- 결정 `2026-10-22-out8-port-mapping-sheet.md`로 해결. 그 밖의 결정 요청 없음.
- 시험의 가상 포트는 프로세스 단위 16개 상한을 함께 쓴다. UI 시험은 포트를 하나씩만 만든다.
