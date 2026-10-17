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
cargo test --workspace --no-fail-fast: 1026 passed, 0 failed (리뷰 1 대응 뒤 1043, 리뷰 2 대응 뒤 1060, 리뷰 3 대응 뒤 1077 passed, 0 failed)
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
- 감싼 메뉴 항목(`menu::RunCommand`)은 처리기가 아직 없는 Action이어도 켜져 보인다. GPUI는 항목을 그 Action의 처리 가능 여부로 켜고 끈다(리뷰 2 제안 1).
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

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1(차단): 「메뉴 밖」 17건이 키만 가리킨다(UI-11) | 자리를 자유 문자열에서 화면 면 `Surface`(피아노롤, 배치 뷰, 트랙 목록, 인스펙터, 트랜스포트, 상태줄, 건반 뷰, Sequence 레인 뷰, 컨트롤러 레인, AI 패널, 설정, 명령 팔레트, 대화상자, 키맵 오버레이, 검색 막대)와 그 안의 자리로 좁혔다. 「키」는 화면 면이 아니라 고를 수 없다. 키로만 되던 편집 8개(뒤·앞에 붙여넣기, 앞 노트 삭제, 마디 복사, 마디 끝까지 삭제·변경, 마지막 편집 반복, 음높이 바꾸기)는 편집 메뉴로 옮겼다(메뉴 80줄). 휴머나이즈는 퀀타이즈 설정 시트, 드래그 취소는 피아노롤의 드래그, vim 오퍼레이터·텍스트 객체는 「마우스 선택 후 편집 메뉴와 같은 기능」으로 적었다 |
| 지적 2: Tab 뒤 Enter가 다른 명령을 실행 | 거르기와 순위를 팔레트가 직접 한다(`commands::filter_entries`, 컴포넌트는 `filterable(false)`). 순위는 이름 같음 → 이름 시작 → 이름 포함 → Action 이름 같음 → 포함 → 설명 포함, 같은 순위에서는 실행할 수 있는 줄 먼저. Tab이 이름을 채우면 그 줄이 맨 위이고 컴포넌트가 강조를 첫 줄로 되돌린다 |
| 지적 3: 메뉴 이름을 치면 다른 명령이 첫 줄 | 위 순위로 해결. 부록 A-2를 순위 함수 기준으로 옮겼다(모든 실행 가능한 줄이 자기 이름의 첫 줄) |
| 지적 4: 변형 줄 이름 15건 겹침 | 메뉴 밖 변형은 「명령 이름: 변형」(예: 「Step 길이: 4분음표」, 「생성 종류: 새로 만들기」). 메뉴 하위 항목 이름은 그대로 |
| 지적 5: 문서를 누르면 팔레트가 남고 Esc도 안 됨 | 팔레트 밖을 누르면 닫는다(`on_mouse_down_out`). 포커스 변경 콜백(`on_focus_in`/`on_focus_out`)은 이 경로에서 불리지 않아(헤드리스에서 확인) 쓰지 않았다 |
| 제안 1: 팔레트가 열린 동안 ctrl-tab이 모드를 바꿈 | 팔레트가 `mode::Cycle`·`CycleBack`을 받아 멈춘다 |
| 제안 2: 메뉴 재생성 단언이 없음 | `menus_follow_keymap_changes`: 컨텍스트 없는 수식키를 걸면 감싼 항목이 실제 Action 항목으로 바뀐다 |
| 제안 3: 맨 키 메뉴 등가물이 입력을 가로챌 위험 | GPUI가 맨 키(수식키 없음 또는 Shift뿐, 기능 키 아님)를 키 등가물로 구울 항목은 `menu::RunCommand`로 감싸 키 등가물을 없애고 제목 뒤에 키를 보인다(「배치 뷰    V」). 감싼 항목은 실행 함수로 실행되므로 그 경로는 카운트도 비운다. `no_menu_item_gets_a_bare_key_equivalent`가 GPUI와 같은 규칙으로 검사한다. 출구 수동 확인 항목은 그대로 둔다(실기기 확인) |
| 제안 4: 「설정」이 하위 메뉴 | 인자 없음 변형이 있으면 그것만 바로 항목으로 둔다(앱 메뉴 「설정」). 섹션 변형은 팔레트에만 |
| 제안 5: Shift+Tab 의미 | 「이전 후보」= 강조를 한 줄 위로(5.11 표). 그대로 둔다 |

부록 A-1–A-6은 `tests/review_s2_t4.rs`로 옮겨 모두 통과한다(A-2는 순위 함수, A-5는 자리 설명 기준으로 고쳤다).

## 리뷰 2 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 질의가 바뀐 뒤 강조가 첫 실행 가능 줄로 가지 않는다 | 순위에서 실행할 수 있는 줄을 늘 먼저 둔다(꺼진 줄은 맨 뒤, 빈 질의도). 그래서 컴포넌트가 강조를 어느 목록의 첫 줄로 되돌려도 첫 줄은 실행할 수 있는 최선의 줄이다(제안 3도 이것으로 정함) |
| 지적 2: 팔레트 바깥 클릭과 모드 메뉴가 엇갈림 | 팝업은 하나만 띄운다: 모드 메뉴를 열면 팔레트를, 팔레트를 열면 모드 메뉴를 닫는다. 팔레트를 닫을 때 포커스가 아직 팔레트 안에 있을 때만 문서로 돌린다 |
| 지적 3: `is_bare`가 GPUI 마스크와 다름(fn) | fn은 키 등가물에서 버려지므로 없는 것으로 본다. Alt(Option+글자 = 특수 문자 입력)도 글자 입력과 겹친다고 보아 감싼다(제안 4). Cmd·Ctrl이 없고 기능 키가 아니면 감싼다 |
| 지적 4: vim 마크의 자리가 사실이 아님(UI-11) | 사람에게 물어 결정했다(`docs/decisions/2026-10-17-vim-marks-visible-ui.md` a): 피아노롤·배치 뷰 우클릭 메뉴, 구현은 S3 이월 |
| 제안 1: 감싼 항목은 처리기 없이도 켜져 보인다 | 한계로 남긴다. GPUI는 메뉴 항목을 그 Action의 처리 가능 여부로 켜고 끈다. 처리기가 각 단계에서 붙으면 실제 차이는 사라진다 |
| 제안 2: 「vim 조합 키」 같은 설명이 테스트를 통과 | 자리 설명을 고쳤고(오퍼레이터·텍스트 객체·드래그 취소), 테스트는 「키맵」을 뺀 「키」가 설명에 있으면 실패한다 |
| 제안 3: 꺼진 줄의 순위 | 지적 1과 같이 정했다 |
| 제안 4: Alt 키 등가물 | 지적 3과 같이 감싼다. 출구 수동 확인 문구에도 넣었다 |

부록 A-1–A-5는 `tests/review_s2_t4_r2.rs`로 옮겨 모두 통과한다.

## 리뷰 3 대응(결정 `docs/decisions/2026-10-17-s2-t4-review-limit.md` A)

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 키 시퀀스만 걸린 메뉴 항목에 단축키가 안 보인다 | 제목 꼬리의 키를 `commands::effective_keys`로 고른다. 시퀀스(`z z`)도 된다 |
| 지적 2: 제목 꼬리 키가 가려진 키다 | `effective_keys`는 우선순위가 높은 바인딩부터, Normal 모드 경로(배치 뷰·피아노롤)에서 그 키를 누르면 이 Action이 나오는 첫 바인딩을 고른다(팔레트와 같은 뜻). GPUI가 굽는 키 등가물과 실제로 누를 키가 같고 감싸지 않을 때만 꼬리를 생략한다 |
| 제안 1: 순위 함수 문서 주석 | 고쳤다 |
| 제안 2: 감싼 항목이 켜져 보이는 한계 | 「알려진 한계」에 적었다 |
| 제안 3: `operator::Count` 자리 설명 | 「같은 명령을 여러 번 실행하는 것과 같은 기능」으로 고쳤다 |
| 제안 4: 꺼진 줄 이름을 치면 강조가 다른 줄 | 리뷰 2에서 정한 설계(실행 가능한 줄 먼저)라 그대로 둔다 |
| 제안 5: 감싸지 않은 항목도 같은 검사 | 부록 B를 옮긴 테스트가 기본·vim 프리셋의 모든 메뉴 항목에서, 보인 키가 실제로 그 명령을 실행하는지와 키가 있는데 안 보이는 항목이 없는지를 본다 |
