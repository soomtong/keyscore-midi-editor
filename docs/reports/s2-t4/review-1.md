# 적대적 리뷰: S2-T4 명령 팔레트와 메뉴바 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `bea6d05`(브랜치 `s2/t4-palette-menus`, 범위 `git diff main...HEAD`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트 파일을 열기 전에 `commands/mod.rs`, `commands/table.rs`, `dispatch.rs`, `command_palette.rs`, `window.rs`, gpui-component `command/state.rs`를 읽고 적었다.

1. 단계 게이트의 셈은 팔레트·메뉴가 같은 명령 표에서 만들어지므로 구성상 늘 맞는다. 실제로 걸러야 하는 것은 `Elsewhere` 문자열이 UI-11이 말하는 「눈에 보이는 UI」(메뉴바·트랜스포트·인스펙터·패널·설정 화면)인가다. 아무 문자열이나 통과하는가.
2. 팔레트 `Tab`(`set_query(label)`) 뒤에 컴포넌트가 선택을 「첫 일치 줄」로 되돌린다(`reset_selection`). 이름이 다른 줄의 이름·Action 이름·설명에 부분 문자열로 들어 있으면 Tab 뒤 Enter가 다른 명령을 실행하는가.
3. 변형 줄의 이름(`Variant::label`)까지 합치면 팔레트 이름이 겹치는가. 테스트 1의 겹침 검사는 명령 표 줄 이름만 보는가.
4. 팔레트가 열린 채 문서가 포커스를 가져가면(문서 클릭) 팔레트가 남는가. 그때 Esc는 문서 경로로 가서 팔레트를 닫지 못하는가(P2).
5. 빈 질의의 Esc: 컴포넌트 `Cancel`이 `on_cancel` 뒤 `cx.propagate()`한다. 같은 Esc가 문서(Step의 `escape` → `mode::Normal`)로 새는가.
6. Enter 확정: `on_confirm`이 미뤄 실행되고 그 안에서 포커스를 옮긴 뒤 `run_action`을 부른다. 같은 Enter가 문서(`enter` → `clip::Open`)로 새거나 Action이 두 번 실행되는가.
7. 실제 팔레트 경로(키로 연 팔레트의 Enter)가 카운트를 비우는가. 기존 테스트는 `run_action`을 직접 부른다.
8. 팔레트가 열린 동안 컨텍스트 없는 전역 키(`ctrl-tab` → `mode::Cycle`)가 뒤의 모드를 바꾸는가.
9. 메뉴 단축키 표시가 키맵 재적용을 따르는가를 헤드리스로 확인하는 테스트가 실제로 무엇을 단언하는가.
10. 맨 키 메뉴 키 등가물(`v`, `x`): `gpui-pre-macos` `handle_key_event`가 키 등가물 단계에서 처리되지 않은 맨 키에 `NO`를 돌려주는가.
11. `:`를 열린 팔레트 입력란에서 치면 글자가 들어가는가, 팔레트를 다시 여는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. `Elsewhere`가 눈에 보이는 UI인가 | 없음 | `every_action_has_exactly_one_command_spec`은 비지 않음만 본다 |
| 2. Tab 뒤 Enter가 Tab으로 고른 명령을 실행 | 부분 | `palette_tab_completes_and_shift_tab_moves_back`은 「트랙 인스펙터」 한 줄만 본다. 이 줄은 우연히 첫 일치다 |
| 3. 변형까지 포함한 이름 겹침 | 없음 | 명령 표 줄 이름만 검사(`tests/commands.rs:126-135`) |
| 4. 문서가 포커스를 가져간 뒤의 팔레트 | 없음 | |
| 5. 빈 질의 Esc가 문서로 새지 않음 | 부분 | Normal에서만 본다(새도 No-op이라 드러나지 않는다) |
| 6. Enter가 문서로 새지 않음, 한 번만 실행 | 부분 | 인스펙터 토글만 본다. 뷰·모드 불변은 보지 않는다 |
| 7. 실제 팔레트 경로의 카운트 비움 | 없음 | `count_non_key_paths`는 `run_action` 직접 호출 |
| 8. 팔레트 중 전역 키 | 없음 | |
| 9. 메뉴 단축키가 키맵을 따름 | 형식만 | `tests/commands.rs:370`은 `get_menus().is_some()`만 단언. 재적용 전에도 참이다 |
| 10. 맨 키 메뉴 키 등가물 | 없음 | 헤드리스 불가. 단계 출구 수동 항목으로 넘겼다 |
| 11. 팔레트 안의 `:` | 없음 | |

## 3. 깨뜨리기 시도

모두 실제로 실행했다. 깨진 시도의 테스트는 부록 A에 있다(파일에 넣지 않았다). 못 깨뜨린 시도는 `crates/keyscore-app/tests/review_s2_t4.rs`에 남겼다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `view::toggletracklist`로 거르고 Tab(질의 「트랙 목록」) → Enter | 깨짐: `track::FocusList`(「트랙 목록 패널로 포커스」)가 첫 줄이 되어 트랙 목록이 토글되지 않음 | 부록 A-1 `palette_tab_then_enter_runs_the_completed_command` |
| 2 | 실행할 수 있는 모든 줄에 대해 그 이름으로 거른 첫 줄이 자기 자신인가(컴포넌트 `CommandItem::matches`의 부분 문자열 규칙) | 깨짐: 58줄. 예: 「저장」→ `ai::SaveFixture`, 「열기」→ `ai::OpenPanel`, 「재생」→ `ai::EditPrompt`, 「새로 만들기」→ `ai::SetKind`, 「배치 뷰」→ `cursor::NextClipBoundary`, 「정지」→ `transport::Pause` | 부록 A-2 `every_palette_label_selects_its_own_row_first` |
| 3 | 변형 줄까지 포함한 팔레트 이름 겹침 | 깨짐: 15건. 「새로 만들기」(`file::New`, `ai::SetKind`), 「온음표」…「64분음표」(`note::SetLength`, `step::SetLength`), 「점온음표」…(`note::SetDottedLength`, `step::SetDottedLength`) | 부록 A-3 `palette_labels_are_unique_including_variants` |
| 4 | 팔레트를 연 채 문서 영역(`status-bar`)을 클릭 → Esc | 깨짐: 클릭으로 문서가 포커스를 가져가고 팔레트는 열린 채 남는다. Esc는 문서 경로(Normal, No-op)로 가서 팔레트가 닫히지 않는다 | 부록 A-4 `palette_closes_when_document_takes_focus_or_on_escape` |
| 5 | `Elsewhere` 자리가 키만 가리키는가(`…키`로 끝남) | 깨짐: 17건(「피아노롤 편집 키」 10, 「vim 프리셋 키」 7) | 부록 A-5 `elsewhere_places_name_a_visible_ui` |
| 6 | 팔레트 중 `ctrl-tab` | 깨짐(제안 수준): 팔레트가 열린 채 뒤의 모드가 Step으로 바뀐다 | 부록 A-6 `ctrl_tab_does_not_cycle_mode_behind_open_palette` |
| 7 | Step에서 팔레트를 열고 빈 질의 Esc | 못 깨뜨림: 팔레트만 닫히고 Step 유지 | `palette_escape_on_empty_query_closes_palette_without_leaving_step` |
| 8 | 배치 뷰에서 팔레트 Enter 실행 뒤 같은 Enter가 `clip::Open`으로 새는가 | 못 깨뜨림 | `palette_enter_runs_once_and_does_not_reach_document` |
| 9 | 열린 팔레트에 `a:b` 입력 | 못 깨뜨림: 질의 `a:b` | `colon_typed_in_open_palette_goes_to_query` |
| 10 | 두 번 열기(질의 유지) 뒤 Enter가 한 번만 실행 | 못 깨뜨림 | `opening_twice_keeps_one_palette_and_runs_once` |
| 11 | vim `3` → 팔레트 Enter 실행 뒤 카운트 | 못 깨뜨림: 비워짐 | `palette_enter_clears_pending_count` |
| 12 | vim `3` → 팔레트 열고 Esc만 | 못 깨뜨림: 비워짐 | `count_does_not_survive_palette_opened_and_escaped` |
| 13 | 팔레트로 `mode::Play` 실행 → Esc | 못 깨뜨림: Play가 되고, 포커스가 문서로 돌아와 Esc가 Normal로 | `palette_runs_mode_action_into_document` |
| 14 | 팔레트 중 Normal 컨텍스트 키 `cmd-i` | 못 깨뜨림: 문서로 가지 않음 | `normal_context_keys_do_not_reach_document_while_palette_open` |
| 15 | 사용자 키맵으로 `palette::Close`를 `f7`에 걸고 질의가 있는 팔레트에서 누름 | 못 깨뜨림: 닫힘 | `user_bound_palette_close_closes_palette_with_query` |

기존 `cargo test -p keyscore-app` 전체(모드 FSM, S2-T3 모드 메뉴, 키맵 엔진, 창 레이아웃, 이전 리뷰 테스트)는 이 브랜치에서 모두 통과했다. 회귀는 찾지 못했다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 명령 표 = 등록 Action(한 줄씩, 겹침 없음) | 아니오 | 테스트 1이 지킨다 |
| 팔레트 이름 = 메뉴 이름이고 사람이 고를 수 있다(이름 겹침 없음, 계획 테스트 1의 의도) | 예 | 변형 이름은 검사 밖. 시도 3 |
| 팔레트의 「이름으로 고른 명령」이 실행된다 | 예 | Tab 뒤 Enter, 또는 메뉴 이름을 그대로 쳐서 Enter. 시도 1·2 |
| 키가 아닌 실행은 `run_action` 하나를 거친다(결정 C) | 아니오 | 팔레트(`window.rs:166-170`)와 모드 메뉴(`window.rs:212`)만 실행한다. 메뉴 막대는 결정 C의 한계 |
| 모든 Action에 눈에 보이는 자리(UI-11) | 예 | 「…키」만 적은 `Elsewhere` 17건. 시도 5 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| UI-2 이름 검색·실행 | 부분 충족(미충족으로 셈) | 이름으로 검색한 첫 줄이 다른 명령인 경우 58건, 이름 겹침 15건. `command_palette.rs:67-77`, `commands/mod.rs:225` |
| UI-2 단축키 함께 표시 | 충족 | `command_palette.rs`의 `shortcut`, 테스트 5·8 |
| UI-2 `Esc`로 닫는다(P2) | 부분 충족(미충족으로 셈) | 문서가 포커스를 가져간 뒤에는 Esc로 닫히지 않는다. `window.rs:158-183`(포커스를 잃어도 팔레트를 닫지 않음), `command_palette.rs:153`(`palette::Close`는 `Palette` 컨텍스트에만) |
| UI-2 `:`도 팔레트 | 충족(Normal 한정) | `assets/keymaps/default.json:63`. 다른 모드의 `:`는 A12 결정 대기 |
| UI-11 모든 기능에 눈에 보이는 UI | 미충족 | `commands/table.rs:350` 등 「피아노롤 편집 키」 10건(`edit::Humanize`, `edit::CopyBar`, `edit::SetPitch`, `edit::PasteBefore/After`, `edit::RepeatLast`, `edit::DeleteBefore`, `edit::DeleteToBarEnd`, `edit::ChangeToBarEnd`, `edit::CancelDrag`), 「vim 프리셋 키」 7건. 단계 게이트 테스트가 아무 문자열이나 받는다(`tests/commands.rs:141-196`) |
| KM-13 메뉴 7개(+앱 메뉴), 항목 = Action 1:1 | 충족 | `commands/mod.rs`의 `menus`, 테스트 3·4 |
| KM-13 메뉴에 같은 단축키 표시 | 확인 불가(헤드리스) | GPUI가 굽는다. 재적용 뒤 다시 거는지는 `tests/commands.rs:370`이 단언하지 못한다(제안 2) |
| 7.6 메뉴바 구성 | 충족(트랙 > 포트는 S2-T5, 최근 파일 목록은 S2-T7로 명시) | 테스트 3 |
| 7.6 「모든 메뉴 항목은 팔레트에서 같은 이름으로 검색된다」 | 부분 충족(미충족으로 셈) | 「새로 만들기」·「저장」·「열기」·「재생」·「정지」·「루프」·「메트로놈」·「배치 뷰」·「건반 뷰」·「트랙 목록」·「삭제」·「반복」·「검색」을 그대로 치면 첫 줄이 다른 명령이다(시도 2) |
| 5.11 팔레트 `Tab` 자동완성 확정 / `Shift+Tab` 이전 후보 | 부분 충족(미충족으로 셈) | 「확정」한 후보가 Tab 뒤에 다른 후보로 바뀐다. 시도 1 |
| KM-5·결정 C(키가 아닌 경로는 카운트를 비움) | 충족 | 시도 11·12, `dispatch.rs` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 앱 UI만 건드린다).
- 제품 코드의 `unwrap`/`expect`: 이 diff에서 새로 생긴 것 없음. `window.rs`의 `expect("컨텍스트 경로는 비지 않는다")`는 S2-T3부터 있던 것.
- 에러가 삼켜지는 곳
  - `command_palette.rs:166`, `:169`의 `let _ = weak.update(...)`: 팔레트 엔티티가 이미 사라졌을 때뿐이라 정당하다.
  - `commands/mod.rs`의 `build_variant`·`menu_item`: `build_action` 실패를 `None`/빈 하위 메뉴로 바꾼다. 변형 JSON 오타가 메뉴에서 조용히 사라질 수 있으나 `every_variant_builds`가 잡는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | UI-11 미충족이고 단계 게이트가 그것을 잡지 못한다. `Elsewhere` 17건이 키만 가리킨다(「피아노롤 편집 키」, 「vim 프리셋 키」). 휴머나이즈·마디 복사·음높이 바꾸기·앞/뒤 붙여넣기·마지막 편집 반복 같은 기능은 눈에 보이는 자리가 없다. 게이트 테스트는 비지 않은 문자열이면 무엇이든 「메뉴에 없다고 명시」로 센다. 메뉴(편집 등)나 패널 자리를 주거나, REQ 7.6에 없는 배정이면 `docs/decisions/`에 결정 요청을 쓰고 그 줄을 보류로 표시한다. 그리고 `Place::Elsewhere`를 UI-11의 자리 목록(메뉴바·트랜스포트·인스펙터·패널·설정·상태줄 등) 가운데 하나를 가리키는 타입으로 바꾸거나 검사 테스트를 더해 키만 적은 자리를 거부한다 | `crates/keyscore-app/src/commands/table.rs:350` 이하, `crates/keyscore-app/src/commands/mod.rs:52-58`, `crates/keyscore-app/tests/commands.rs:141-196` |
| 2 | 수정 | 팔레트 `Tab` 뒤 `Enter`가 Tab으로 고른 명령이 아닌 다른 명령을 실행한다. `complete`가 `set_query(label)`만 하고, 컴포넌트는 질의가 바뀌면 선택을 첫 일치 줄로 되돌린다(gpui-component `command/state.rs` `on_query_changed` → `reset_selection`). 일치는 이름·Action 이름·설명의 부분 문자열이라 58줄이 남에게 선택을 뺏긴다. Tab 뒤에 원래 줄의 `IndexPath`로 `set_selected_index`를 다시 걸어야 한다 | `crates/keyscore-app/src/command_palette.rs:67-77` |
| 3 | 수정 | 메뉴 이름을 그대로 팔레트에 치면 첫 줄이 다른 명령이다(7.6 「팔레트에서 같은 이름으로 검색된다」, UI-2 「이름으로 검색·실행」). 「저장」+Enter가 `ai::SaveFixture`, 「새로 만들기」+Enter가 `ai::SetKind("new")`, 「재생」+Enter가 `ai::EditPrompt` 쪽으로 간다. 이름이 정확히 같은 줄을 앞에 두는 순위(정확 일치 > 접두 > 부분)가 필요하다. 지적 2와 같은 원인이지만 Tab 없이도 생긴다 | `crates/keyscore-app/src/command_palette.rs:103-147`(줄 순서·검색 키워드), 부록 A-2 |
| 4 | 수정 | 변형 줄까지 합치면 팔레트 이름이 15건 겹친다(「새로 만들기」, 음표 길이 14건). 테스트 1의 겹침 검사는 명령 표 줄 이름만 본다. 「새로 만들기」는 메뉴 파일 > 새로 만들기와 같은 이름이라 지적 3을 더 키운다. 변형 이름에 맥락을 넣고(예: 「AI 종류: 새로 만들기」, 「Step 길이: 온음표」) 겹침 검사를 팔레트 줄 전체로 넓힌다 | `crates/keyscore-app/src/commands/mod.rs:225`, `crates/keyscore-app/src/commands/table.rs:62-75`, `:679`, `:1093`, `crates/keyscore-app/tests/commands.rs:126-135` |
| 5 | 수정 | 팔레트가 열린 채 문서가 포커스를 가져가면(문서 클릭) 팔레트가 화면에 남고 Esc로 닫히지 않는다(P2 「열린 팔레트는 Esc로 닫는다」). `palette::Close`는 `Palette` 컨텍스트에만 있고, 팔레트는 포커스를 잃어도 닫히지 않는다. 모드 메뉴(PopupMenu)는 포커스를 잃으면 닫힌다. 팔레트 포커스 아웃에서 닫거나(`on_focus_out`/`on_blur`), 열린 동안 문서 쪽 Esc가 팔레트를 먼저 닫게 한다 | `crates/keyscore-app/src/window.rs:158-183` |

## 제안 (구현자가 거절할 수 있음)

1. 팔레트가 열린 동안 컨텍스트 없는 전역 키(`ctrl-tab` `mode::Cycle`, `ctrl-shift-tab`)가 팔레트 뒤의 모드를 바꾼다(부록 A-6). 팔레트를 「컨텍스트 겹 바깥」에 둔 취지(문서 키가 팔레트에 섞이지 않게)와 어긋난다. 막을지(팔레트 컨텍스트에서 `ctrl-tab`을 `null`) 그대로 둘지 정해 적는다. `assets/keymaps/default.json:6-10`.
2. 메뉴 단축키가 키맵 재적용을 따르는지에 대한 단언이 없다(`tests/commands.rs:370`의 `is_some()`은 재적용 전에도 참). `set_menus` 호출 횟수를 세거나, GPUI `create_menu_item`의 고르기 규칙(컨텍스트 없는 바인딩 먼저, 없으면 첫 바인딩)을 함수로 떼어 `bindings_for_action`으로 「메뉴에 보일 키」를 계산해 키맵을 바꾼 뒤 비교하는 테스트를 권한다. 사용자가 `null`로 지운 프리셋 키가 남는 한계도 그 함수에서 드러난다.
3. 맨 키 메뉴 키 등가물(출구 게이트 수동 항목)의 근거 보강. `gpui-pre-macos-0.3.8/src/window.rs`의 `handle_key_event`는 키 등가물 단계에서 GPUI가 처리하지 않은 키에 대해, 수식키가 `function`만이 아니면 `NO`를 돌려준다(2757-2759행 부근, 「Don't send key equivalents to the input handler if there are key modifiers other than Function key」). 수식키가 없는 맨 키 `v`도 `Modifiers::default() != Modifiers::function()`이라 `NO`다. 즉 팔레트 입력란처럼 바인딩이 없는 곳에서 친 `v`는 AppKit이 메인 메뉴 `performKeyEquivalent:`에 넘기면 「배치 뷰」 메뉴가 먼저 실행될 수 있다. 수동 확인 전에 컨텍스트가 있는 바인딩은 메뉴 키 등가물로 쓰지 않는 쪽(메뉴 제목에 키를 글자로만 붙이기 등)을 미리 검토해 두기를 권한다.
4. 설정(`cmd-,`)이 「KeyScore > 설정 > 설정 열기」 하위 메뉴 안에 있다. macOS 관례는 앱 메뉴 바로 아래 「설정…」 항목이다. 「설정 열기」를 앱 메뉴의 일반 항목으로 두고 섹션 변형만 하위 메뉴로 두는 것을 권한다(`commands/table.rs:952-965`).
5. 팔레트 `Shift+Tab`은 선택만 위로 옮긴다(`SelectUp`). 5.11의 「이전 후보」를 Tab과 짝으로 읽으면 이전 줄의 이름으로 질의를 채우는 쪽이 맞을 수 있다. 지적 2를 고칠 때 함께 정해 적는다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(단계 파일 한 줄은 출구 게이트 수동 항목 추가로 보고서에 적혀 있다. `tests/deferred/`는 옮겨 켠 것)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 테스트

아래는 `crates/keyscore-app/tests/` 아래 새 파일로 두면 그대로 돈다(도움 함수는 `review_s2_t4.rs`와 같다). 리뷰 시점에 A-1~A-6 모두 실패했다.

~~~rust
use std::collections::BTreeMap;
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::commands::{self, Place};
use keyscore_app::mode::Mode;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-r1-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx
        .update(|cx| open_project_window(Project::new(), cx))
        .unwrap();
    render(cx, handle);
    (handle, view)
}
fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}
fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}
fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    render(cx, handle);
}

// A-1 (지적 2): Tab(자동완성) 뒤 Enter가 Tab으로 고른 명령을 실행한다.
#[gpui_kit::test]
fn palette_tab_then_enter_runs_the_completed_command(cx: &mut TestAppContext) {
    start(cx, "tab-enter");
    let (handle, view) = open(cx);
    let before = cx.read(|cx| view.read(cx).layout().track_list_open);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggletracklist");
    press(cx, handle, "tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some("트랙 목록")
    );
    press(cx, handle, "enter");
    assert!(!cx.read(|cx| view.read(cx).palette_open()));
    // 실패: 「트랙 목록 패널로 포커스」(track::FocusList)가 실행되어 트랙 목록이 그대로다
    assert_ne!(
        cx.read(|cx| view.read(cx).layout().track_list_open),
        before,
        "Tab으로 채운 「트랙 목록」의 Enter가 view::ToggleTrackList를 실행하지 않았다"
    );
}

// A-2 (지적 2·3): 실행할 수 있는 모든 줄에서, 그 이름으로 거른 첫 줄이 자기 자신이다.
// 실패: 58건(「저장」→ ai::SaveFixture, 「열기」→ ai::OpenPanel, 「재생」→ ai::EditPrompt ...)
#[gpui_kit::test]
fn every_palette_label_selects_its_own_row_first(cx: &mut TestAppContext) {
    start(cx, "tab-data");
    cx.update(|cx| {
        let entries = commands::palette_entries(cx);
        let mut bad = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            if e.action().is_none() {
                continue;
            }
            let q = e.label().to_lowercase();
            // gpui-component `CommandItem::matches`: 이름 또는 키워드(Action 이름, 설명)에 부분 문자열
            let first = entries
                .iter()
                .position(|o| {
                    o.action().is_some()
                        && (o.label().to_lowercase().contains(&q)
                            || o.name().to_lowercase().contains(&q)
                            || o.description().to_lowercase().contains(&q))
                })
                .unwrap();
            if first != i {
                bad.push(format!("{} [{}] -> {}", e.label(), e.name(), entries[first].name()));
            }
        }
        assert!(bad.is_empty(), "{}건: {:?}", bad.len(), bad);
    });
}

// A-3 (지적 4): 변형 줄까지 포함한 팔레트 이름은 겹치지 않는다. 실패: 15건
#[gpui_kit::test]
fn palette_labels_are_unique_including_variants(cx: &mut TestAppContext) {
    start(cx, "labels");
    cx.update(|cx| {
        let mut labels: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        let entries = commands::palette_entries(cx);
        for e in &entries {
            labels.entry(e.label().to_string()).or_default().push(e.name());
        }
        let dups: Vec<_> = labels.iter().filter(|(_, v)| v.len() > 1).collect();
        assert!(dups.is_empty(), "{}건: {:?}", dups.len(), dups);
    });
}

// A-4 (지적 5): 팔레트가 열린 채 문서가 포커스를 가져가도(클릭) Esc로 닫힌다.
#[gpui_kit::test]
fn palette_closes_when_document_takes_focus_or_on_escape(cx: &mut TestAppContext) {
    start(cx, "click");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    cx.update_window(handle, |_, window, cx| window.click("status-bar", cx))
        .unwrap();
    render(cx, handle);
    press(cx, handle, "escape");
    assert!(
        !cx.read(|cx| view.read(cx).palette_open()),
        "문서를 누른 뒤 팔레트가 열린 채 남고 Esc로도 닫히지 않는다"
    );
}

// A-5 (지적 1): UI-11. 「다른 곳」이 키만 가리키지 않는다. 실패: 17건
#[test]
fn elsewhere_places_name_a_visible_ui() {
    let bad: Vec<String> = commands::specs()
        .iter()
        .filter_map(|s| match s.place() {
            Place::Elsewhere(p) if p.ends_with("키") => Some(format!("{} ({p})", s.name())),
            _ => None,
        })
        .collect();
    assert!(bad.is_empty(), "{}건: {:?}", bad.len(), bad);
}

// A-6 (제안 1): 팔레트가 열린 동안 ctrl-tab이 뒤의 모드를 바꾸지 않는다. 실패: Step이 된다
#[gpui_kit::test]
fn ctrl_tab_does_not_cycle_mode_behind_open_palette(cx: &mut TestAppContext) {
    start(cx, "ctrltab");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    press(cx, handle, "ctrl-tab");
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Normal);
}
~~~
