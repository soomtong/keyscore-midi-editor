# Task 보고: S2-T4 명령 팔레트와 메뉴바

- PR: 브랜치 `s2/t4-palette-menus`
- 계획 파일: docs/reports/s2-t4/plan.md
- 계획 대비 달라진 점
  - 메뉴 단축키 표시: 처음에는 모드 컨텍스트 키를 메뉴 제목 뒤에 붙이려 했다. GPUI macOS 메뉴는 Action의 바인딩 가운데 컨텍스트 없는 것을 먼저, 없으면 첫 바인딩을 키 등가물로 굽는다(`gpui-pre-macos` `create_menu_item`). 그래서 제목에 붙이면 두 번 보인다. 붙이지 않았고 계획을 고쳤다.
  - 맨 키 등가물이 텍스트 입력을 가로채는지는 헤드리스로 확인할 수 없어 S2 출구 수동 확인 항목으로 더했다(단계 파일 「출구 게이트」).
  - 새 Action `palette::Close`(A13 결정, `Palette` 컨텍스트의 `escape`). Action 287개.
  - 테스트를 하나 더했다: `palette_tab_completes_and_shift_tab_moves_back`.

## 구현 요약

- `src/commands/mod.rs`, `src/commands/table.rs`: 명령 표
  - Action 287개마다 한 줄: 표시 이름, 자리(`Menu{메뉴, 묶음, 순서}` 또는 `Elsewhere(눈에 보이는 자리)`), 유한한 인자 변형.
  - `palette_entries`: Action마다 한 줄, 변형이 있으면 변형마다 한 줄이다. 설명은 Action 문서 주석(`cx.action_documentation()`)이다. 값이 열린 인자 Action(이름, 템포 값, 음 키 등)은 보이되 실행할 수 없고, 설명에 「인자 필요」를 붙인다(A12).
  - `menus`: KeyScore·파일·편집·보기·트랙·재생·AI·도움말. 묶음이 바뀌면 구분선, 변형은 하위 메뉴, 열린 인자는 꺼진 빈 하위 메뉴(최근 파일, S2-T7)다.
  - 메뉴 72줄, 메뉴 밖 215줄. 7.6이 이름을 준 기능(재생, 녹음, 메트로놈, 뮤트, 솔로, 새 트랙, 패닉, 가사 입력·분배, 배치 뷰, 컨트롤러 레인, 퀀타이즈, AI 생성, 어시스트, 프롬프트 실험실, 키맵 편집·보기, 보정 마법사, 템포 설정, 테스트 노트 보내기, 포트 선택)은 그 이름을 쓴다.
- `src/dispatch.rs`: `run_action`. 카운트를 비우고 창 포커스로 디스패치한다. 팔레트와 상태줄 모드 메뉴가 쓴다(S2-T5 툴바도 쓸 것).
- `src/command_palette.rs`: gpui-component `Command` 위의 팔레트
  - 줄은 이름·설명·단축키를 보인다. 단축키는 문서 포커스 기준(`highest_precedence_binding_for_action_in`)이다.
  - 줄에 Action을 달지 않는다. 확정하면 `PaletteEvent::Run`을 내고, 창이 팔레트를 닫고 문서로 포커스를 돌린 뒤 `run_action`으로 실행한다.
  - `Esc`: 컴포넌트가 질의를 먼저 지우고 빈 질의에서 닫는다. `Tab`은 선택한 줄의 이름으로 질의를 채우고, `Shift+Tab`은 이전 줄이다.
- `src/window.rs`: `palette::Open` 처리기, 팔레트는 모드 메뉴처럼 컨텍스트 겹 바깥에 띄운다. 모드 메뉴 항목은 `mode::*`·`ai::OpenPanel` Action을 `run_action`으로 실행한다(처리기를 키와 공유).
- `src/lib.rs`: 메뉴를 명령 표로 만든다. 키맵을 다시 걸 때마다(`after_apply`) 다시 만든다.
- `tests/count_non_key_paths.rs`: S2-T2가 미뤄 둔 테스트 2개를 결정 C에 맞게 실행 함수 경로로 고쳐 켰다(`tests/deferred/` 폴더는 비어 지웠다).

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 1026 passed, 0 failed
commands: 10 passed
count_non_key_paths: 2 passed
modes_ui: 8 passed(모드 메뉴가 run_action을 거친 뒤에도 카운트 비움 테스트 통과)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `commands::every_action_has_exactly_one_command_spec` | 통과(이름 겹침 없음도 검사) |
| 2 | `commands::action_counts_match_palette_and_menus` | 통과: 287 = 팔레트 287 = 메뉴 72 + 메뉴 밖 215 |
| 3 | `commands::menus_follow_req_7_6` | 통과 |
| 4 | `commands::menu_titles_equal_palette_names` | 통과 |
| 5 | `commands::shortcut_display_follows_current_keymap` | 통과 |
| 6 | `commands::every_variant_builds` | 통과 |
| 7 | `commands::palette_opens_filters_runs_and_closes` | 통과 |
| 8 | `commands::palette_rows_show_name_description_shortcut` | 통과 |
| 9 | `commands::open_argument_actions_are_listed_but_not_runnable` | 통과 |
| 10 | `count_non_key_paths::count_typed_before_non_key_run_does_not_reach_next_key_action`, `non_key_run_during_pending_sequence_gets_no_count` | 통과 |
| 11 | `modes_ui::status_bar_shows_mode_and_menu_switches_mode` | 통과 |
| (추가) | `commands::palette_tab_completes_and_shift_tab_moves_back` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(1026 passed)
- [x] deny
- [x] check-deps

## 알려진 한계

- 메뉴 막대로 실행한 Action은 카운트를 비우지 않는다(결정 C의 한계).
- 맨 키 메뉴 등가물의 입력 가로채기 여부는 S2 출구 수동 확인(단계 파일).
- 사용자가 `null`로 지운 프리셋 키도 GPUI가 첫 바인딩으로 골라 메뉴에 보일 수 있다(키맵 편집기 S2-T6에서 다룬다).
- 팔레트에서 인자 입력은 A12 결정 뒤. 그때까지 값이 열린 인자 Action은 꺼진 줄이다.
- 트랙 > 포트 하위 메뉴(동적 포트 목록)는 S2-T5, 최근 파일 목록은 S2-T7.
- 팔레트 이름 286개 가운데 7.6이 이름을 주지 않은 것은 Action 설명의 첫 구절에서 지었다. 실사용자 QA에서 다듬는다(개발 단계 문구는 승인 범위).
- `os_action`(macOS 표준 잘라내기·복사 셀렉터)은 쓰지 않았다. 표준 `cmd-` 키는 GPUI 키맵이 처리한다.

## 리뷰어에게

1. 명령 표의 자리 배정(`commands/table.rs`)이 UI-11 「모든 기능은 눈에 보이는 UI를 가진다」를 지키는지. `Elsewhere` 문자열은 이후 단계가 만들 자리(트랜스포트, 인스펙터, 설정 화면)를 가리키는 것이 많다. 지금 실제로 있는 것은 메뉴·팔레트·상태줄 모드 메뉴뿐이다.
2. 팔레트 확정 → 닫기 → 포커스 복귀 → `run_action`의 순서. `window.dispatch_action`은 미뤄 실행되므로 그때의 포커스가 문서인지.
3. 팔레트의 단축키 표시가 문서 포커스의 「지난 프레임」 디스패치 경로를 쓴다(GPUI 제약). 모드가 바뀐 직후 첫 프레임에서 어떻게 보이는지.
4. `Tab` 자동완성이 `Input` 컨텍스트의 키에 가려지지 않는지(테스트는 통과).
