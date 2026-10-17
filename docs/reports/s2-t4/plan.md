# Task 계획: S2-T4 명령 팔레트와 메뉴바

- 브랜치: `s2/t4-palette-menus`
- 관련 REQ: UI-2, UI-11, KM-13, 7.6(대응표와 메뉴바 구성), KM-5(카운트)
- 선행 Task: S2-T2(Action·키맵), S2-T3(모드 FSM, 상태줄 모드 메뉴)
- 관련 결정: `2026-10-17-count-and-non-key-dispatch.md`(C), `2026-10-17-keymap-conflicts.md`(A12 `:` 명령줄은 결정 대기, A13 Esc)

## 끝의 정의 (내 말로)

1. 명령 표(`src/commands.rs`): 등록된 Action 286개마다 한 줄
   - 표시 이름(팔레트 이름 = 메뉴 항목 이름, 7.6 「팔레트 명령은 메뉴 항목 이름과 같다」)
   - 자리 `Place`
     - `Menu(메뉴, 묶음)`: 메뉴바에 둔다.
     - `Elsewhere(설명)`: 메뉴에 두지 않는다고 명시하고, 눈에 보이는 자리(UI-11)를 적는다. 예: 「피아노롤 커서(키)」, 「Step 모드 입력」, 「트랜스포트」, 「설정 > 키맵」.
   - 인자 변형: 인자 있는 Action 가운데 값이 유한한 것은 변형을 둔다(테마 3, 채널 1–16, 프리셋 2, 설정 섹션 등). 메뉴에서는 하위 메뉴, 팔레트에서는 변형마다 한 줄이다.
   - 값이 열린 인자(템포 값, 이름, 음 키 등)는 팔레트에 보이되 실행할 수 없게 둔다. 설명에 「인자 필요」를 붙인다. 팔레트에서 인자를 받는 것은 A12(`:` 명령줄) 결정 뒤다.
   - 설명은 Action 문서 주석(`cx.action_documentation()`)을 쓴다. 한 곳에서만 관리한다.
2. 실행 함수(`src/dispatch.rs`) `run_action(window, cx, action)`
   - 키가 아닌 경로(팔레트, 상태줄 모드 메뉴, 이후 툴바)의 유일한 실행 함수다.
   - 카운트를 먼저 비우고(결정 C), 창의 문서 포커스로 Action을 디스패치한다.
   - S2-T3의 모드 메뉴가 직접 부르던 `keymap::clear_count`도 이 함수로 옮긴다.
3. 메뉴바(KM-13, 7.6): 명령 표에서 자동으로 만든다.
   - 순서: KeyScore(앱) · 파일 · 편집 · 보기 · 트랙 · 재생 · AI · 도움말. 7.6의 일곱 메뉴에 macOS 앱 메뉴를 더한다.
   - 항목은 Action과 1:1이다. 단축키 표시는 둘로 나뉜다.
     - 컨텍스트 없는 바인딩(`cmd-` 표준 키, `ctrl-tab`): GPUI가 macOS 키 등가물로 그린다.
     - 모드 컨텍스트에만 있는 바인딩(`v`, `q`, `m` 등): 제목 뒤에 기호로 붙인다(예: 「배치 뷰  V」). 키 등가물로 걸면 NSMenu가 글자 입력을 가로채기 때문이다.
   - 키맵이 바뀔 때마다(`keymap::apply`의 `after_apply`) 메뉴를 다시 만든다. 그래서 표시는 늘 현재 키맵을 따른다.
4. 명령 팔레트(UI-2): gpui-component `Command`를 쓴다.
   - `palette::Open`(`cmd-shift-p`, `:`)으로 연다. 창 컨텍스트 겹 바깥(모드 메뉴와 같은 자리)에 띄운다.
   - 줄마다 이름, 설명, 현재 단축키를 보인다. 단축키는 팔레트가 아니라 문서 포커스 기준으로 찾는다. 팔레트를 연 자리에서 그 키가 무엇을 하는지 보여야 하기 때문이다.
   - 검색은 이름, Action 이름(`edit::Undo`), 설명에서 한다.
   - `Enter`로 실행한다. 팔레트를 닫고 문서로 포커스를 돌린 뒤 `run_action`을 부른다. 컴포넌트의 자체 디스패치는 쓰지 않는다(항목에 Action을 달지 않고 줄 내용을 직접 그린다).
   - `Esc`로 닫는다(A13). 질의가 있으면 컴포넌트가 질의를 먼저 지운다. 새 Action `palette::Close`는 `Palette` 컨텍스트의 `escape`에 건다.
   - `Tab`은 `palette::Complete`(선택 항목 이름으로 질의 채우기), `Shift+Tab`은 `palette::CompletePrev`(이전 항목)다.
5. 미뤄 둔 카운트 테스트를 결정 C에 맞게 고쳐 켠다(`tests/deferred/count_non_key_paths.rs` → `tests/count_non_key_paths.rs`). 메뉴 경로를 실행 함수 경로로 바꾼다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `every_action_has_exactly_one_command_spec` | 단위 | 명령 표의 이름 집합 = `ALL_ACTION_NAMES`, 중복 없음. 이름·자리 설명이 비지 않음 |
| 2 | `action_counts_match_palette_and_menus` | UI(검사 테스트, 단계 게이트) | 등록 Action 수 = 팔레트의 Action 이름 수 = 메뉴에 있는 Action 이름 수 + `Elsewhere` 수. 메뉴 쪽은 `cx.get_menus()`를 하위 메뉴까지 훑어 센다 |
| 3 | `menus_follow_req_7_6` | UI | 메뉴 이름과 순서, 7.6이 메뉴에 적은 기능이 해당 메뉴에 있음 |
| 4 | `menu_titles_equal_palette_names` | UI | 메뉴 항목 제목(단축키 꼬리 제외) = 팔레트 이름 |
| 5 | `shortcut_display_follows_current_keymap` | UI | 팔레트 줄과 메뉴 제목의 단축키가 기본 키맵을 보이고, 사용자 키맵을 바꾸면 따라 바뀜 |
| 6 | `every_variant_builds` | UI | 인자 변형마다 `cx.build_action`이 성공 |
| 7 | `palette_opens_filters_runs_and_closes` | UI | `cmd-shift-p`·`:`로 열림, 입력으로 거름, `Enter`가 Action을 실행(인스펙터 토글), 닫힌 뒤 포커스가 문서로 돌아옴, `Esc`로 닫힘 |
| 8 | `palette_rows_show_name_description_shortcut` | UI | 줄에 이름·설명·단축키(문서 컨텍스트 기준, 예: Normal에서 `view::Toggle`은 `V`) |
| 9 | `open_argument_actions_are_listed_but_not_runnable` | UI | 값이 열린 인자 Action은 보이되 실행되지 않음 |
| 10 | `count_non_key_paths`(미뤄 둔 테스트 2개를 결정 C에 맞게) | UI | 실행 함수로 실행한 Action은 카운트를 받지 않고, 그 앞에 친 카운트는 다음 키 Action에 붙지 않음 |
| 11 | `mode_menu_uses_run_action` | UI | 기존 S2-T3 테스트가 그대로 통과(모드 메뉴 클릭이 카운트를 비움) |

## 불변식

- 팔레트·모드 메뉴·(이후) 툴바의 실행은 모두 `run_action` 하나를 거친다.
- 명령 표가 메뉴·팔레트의 유일한 원천이다. Action을 더하면 표에 한 줄을 더해야 테스트 1이 통과한다.

## 범위 밖

- 팔레트에서 인자 입력, `:` 명령줄 문법(A12 결정 대기)
- 각 Action의 실제 동작(각 단계). 메뉴·팔레트에서 실행해도 처리기가 없으면 아무 일도 없다.
- 최근 파일 목록(S2-T7). 메뉴에는 빈 「최근 파일」 하위 메뉴만 둔다.
- 트랙 > 포트 하위 메뉴(포트 목록이 동적, S2-T5). 그때까지 `Elsewhere("트랙 인스펙터의 포트")`.
- 메뉴 막대 경로의 카운트(결정 C의 한계)

## 리스크·결정 요청

- 모드 컨텍스트 단축키를 메뉴 제목 꼬리로 보이는 방식은 KM-13 「같은 단축키를 표시한다」를 지키는 세부다. NSMenu 키 등가물로 걸 수 없는 이유는 위 3에 적었다.
- 팔레트 이름을 286개 새로 짓는다. 7.6이 이름을 준 것(재생, 녹음, 템포 설정, 메트로놈, 뮤트, 솔로, 새 트랙, 패닉, 가사 입력, 가사 분배, 배치 뷰, 컨트롤러 레인, 퀀타이즈, AI 생성, 어시스트 켜기/끄기, 프롬프트 실험실, 키맵 편집, 키맵 보기, 보정 마법사, 테스트 노트 보내기, 포트 선택)은 그대로 쓴다.
