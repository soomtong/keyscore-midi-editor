# 적대적 리뷰: S2-T4 명령 팔레트와 메뉴바 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `8ac5cbd`(브랜치 `s2/t4-palette-menus`, 범위 `git diff main...HEAD`). 리뷰 1 대응 커밋도 `8ac5cbd`

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show 8ac5cbd`, `command_palette.rs`, `commands/mod.rs`, `window.rs`, gpui-component `command/state.rs`(`on_query_changed`, `install_model`, `reset_selection`), gpui-pre-macos `create_menu_item`, gpui-pre `app_menu.rs`(메뉴 검증·디스패치)를 읽고 적었다.

1. `filterable(false)`에서 질의가 바뀌면 컴포넌트는 먼저 낡은(바뀌기 전) 줄 목록으로 `reset_selection`을 하고, 다음 렌더의 `install_model`이 그 `IndexPath`(행 번호)를 새 목록에서 보존한다. 앞 목록의 첫 줄이 꺼진 줄이면 강조가 새 목록의 1행에 남는가. Tab(`set_query`)도 같은 경로다.
2. `on_mouse_down_out` → `PaletteEvent::Close` → `close_palette`가 `window.focus(문서)`를 한다. 같은 클릭이 상태줄 모드 메뉴를 열어 포커스를 가져간 뒤에 이 포커스 복귀가 오면 모드 메뉴가 포커스 없이 남는가. 반대로 모드 메뉴가 열린 채 팔레트를 열면 모드 메뉴가 남는가.
3. 팔레트 안 클릭(질의 입력, 줄)이 「밖」으로 잡혀 줄 실행 전에 닫히는가.
4. `is_bare`가 GPUI 마스크와 같은가. GPUI는 cmd·ctrl·alt·shift만 마스크로 옮기고 fn은 버린다. `fn-k`는 GPUI에서 맨 `k`가 된다.
5. 감싼 항목(`menu::RunCommand`)이 창 루트 처리기에 닿는가: 포커스가 팔레트·모드 메뉴에 있을 때, 카운트가 쌓였을 때, 없는 Action 이름일 때. 프리셋 전환·핫리로드 뒤 다시 감싸는가.
6. 감싼 항목의 메뉴 사용 가능 여부. GPUI는 `is_action_available(메뉴 Action)`으로 항목을 켜고 끈다. 감싼 항목은 늘 `RunCommand`라 처리기가 없는 Action도 켜진 채 보이는가.
7. `Surface`는 타입이 됐지만 `detail`은 여전히 자유 문자열이다. 자리 설명이 실제 화면 조작과 맞는가(특히 vim 전용 기능: 마크, 카운트, 오퍼레이터).
8. 등록 Action 셈에 `menu::RunCommand`가 끼는가(`cx.all_action_names()`에는 있다).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 꺼진 첫 줄 뒤의 강조·Tab | 없음 | `every_palette_label_selects_its_own_row_first`는 순위 함수만 보고 컴포넌트의 강조는 보지 않는다. `palette_tab_then_enter_runs_the_completed_command`는 첫 줄이 켜진 질의만 쓴다 |
| 2. 팔레트 닫기와 모드 메뉴 포커스 | 없음 | A-4(리뷰 1)는 `status-bar`(빈 곳) 클릭만 본다 |
| 3. 팔레트 안 클릭 | 없음 | 이번에 추가(통과) |
| 4. fn 수식키 | 없음 | `no_menu_item_gets_a_bare_key_equivalent`는 같은 `is_bare`로 검사해 같은 착오를 공유한다 |
| 5. 감싼 항목 처리기 | 부분 | `menus_follow_keymap_changes`는 감싸는지만 본다. 실행 경로는 없음, 이번에 추가(통과) |
| 6. 감싼 항목의 사용 가능 여부 | 없음 | 제안 1 |
| 7. 자리 설명의 진실성 | 부분 | `elsewhere_places_name_a_visible_ui`는 `detail.ends_with("키")`만 거른다. 「vim 조합 키: …」처럼 뒤에 말을 붙이면 지나간다 |
| 8. RunCommand 셈 | 있음 | 게이트 테스트는 `ALL_ACTION_NAMES`로 걸러 센다. 끼어들지 않는다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 「sequence」(첫 줄 꺼진 `sequence::Note`, 강조는 2행 「녹음」) → Tab → Enter | 깨짐: 질의는 「녹음」인데 Enter가 2행 `sequence::SetLoopMode("겹쳐 쓰기")`로 간다. 모드는 Normal 그대로 | 부록 A-1 |
| 2 | 「카운트」 다음 「인」을 따로 입력 → Tab으로 강조 확인 | 깨짐: 강조가 「카운트인 0–4마디: 카운트인 1마디」(2행). 순위 함수의 첫 실행 가능 줄은 「…0마디」 | 부록 A-2 |
| 3 | 순위 함수와 컴포넌트 규칙을 흉내 내 모든 이름·Action 이름 접두 쌍을 훑음 | 깨짐: 한 글자 추가 5건(「se」→「sel」이 기본 대신 vim 프리셋 등), Tab 15건 이상(「CC」 8행 서스테인 Tab → Enter가 패닉, 「Se」 74행 설정 Tab → 퀀타이즈 설정, 「Se」 47행 전체 선택 → 클립 전체 선택 등) | 탐색용, 남기지 않음 |
| 4 | 팔레트가 열린 채 상태줄 모드 표시 클릭 → Esc | 깨짐: 팔레트는 닫히고 모드 메뉴가 열리지만 포커스는 문서. 아래·Enter·Esc가 메뉴에 닿지 않고 메뉴가 남는다 | 부록 A-3 |
| 5 | 모드 메뉴가 열린 채 `cmd-shift-p` → Esc ×2 | 깨짐: 팔레트는 닫히나 모드 메뉴가 남고 Esc로 닫히지 않는다 | 부록 A-3b |
| 6 | 사용자 키맵 `{"fn-k": "view::ToggleKeyboard"}`(컨텍스트 없음) | 깨짐: 「건반 뷰」가 감싸지지 않는다. GPUI는 마스크에서 fn을 버려 맨 `k`를 굽는다 | 부록 A-4 |
| 7 | vim 마크(`cursor::SetMark`·`JumpMark`)의 자리 | 깨짐: 「피아노롤 · 커서(클릭으로도 옮긴다)」. 클릭은 마크를 설정하지도 이름 붙은 마크로 가지도 않는다. 결정 요청 없음 | 부록 A-5 |
| 8 | 팔레트 포커스 중 감싼 항목(배치 뷰) 선택 | 못 깨뜨림 | `wrapped_menu_item_runs_while_palette_has_focus` |
| 9 | 모드 메뉴 포커스 중 감싼 항목(가사 입력) | 못 깨뜨림 | `wrapped_menu_item_runs_while_mode_menu_has_focus` |
| 10 | 카운트 `3` 뒤 감싼 항목 | 못 깨뜨림(카운트 비움) | `wrapped_menu_item_clears_pending_count` |
| 11 | 없는 Action 이름을 감싼 항목 | 못 깨뜨림(무시, 패닉 없음) | `wrapped_unknown_action_is_ignored` |
| 12 | vim ↔ default 프리셋 전환 뒤 맨 키 등가물 | 못 깨뜨림 | `preset_switch_rebuilds_menus_without_bare_keys` |
| 13 | 핫리로드로 컨텍스트 없는 맨 키 `k` 추가 | 못 깨뜨림 | `user_contextless_bare_key_is_wrapped_after_reload` |
| 14 | 팔레트 질의 입력 클릭 | 못 깨뜨림(열린 채, 질의 유지) | `clicking_inside_palette_keeps_it_open` |
| 15 | 팔레트 줄 클릭 | 못 깨뜨림(그 줄 실행 후 닫힘) | `clicking_a_palette_row_runs_it` |
| 16 | 「트랙 인스펙터」 한 글자씩 입력 → Enter | 못 깨뜨림 | `typing_label_char_by_char_runs_that_command` |
| 17 | 맞는 줄 없는 질의 → Enter | 못 깨뜨림(아무 일 없음, 열린 채) | `enter_with_no_match_does_nothing` |
| 18 | 대문자·앞뒤 공백 Action 이름 | 못 깨뜨림 | `ranking_ignores_case_and_surrounding_spaces` |

통과한 시도는 `crates/keyscore-app/tests/review_s2_t4_r2.rs`(11개)에 있다. 한글 IME 조합(marked text) 중 질의는 헤드리스로 재현할 수단이 없어 시도하지 않았다(`simulate_input`은 조합 없이 글자를 넣는다).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 등록 Action = 팔레트 줄 이름 = 메뉴 + 메뉴 밖 | 아니오 | `menu::RunCommand`는 `ALL_ACTION_NAMES`에 없고 게이트 셈은 그 목록으로 거른다. 메뉴 셈은 `menu_target`으로 푼다 |
| 팔레트 이름 겹침 없음(변형 포함) | 아니오 | `palette_labels_are_unique_including_variants` |
| 메뉴에 맨 키 등가물 없음 | 예 | fn 수식키 바인딩(지적 3) |
| 강조된 줄 = Enter가 실행하는 줄 = Tab으로 채운 줄 | 예 | 지적 1 |
| 모델 불변식(core) | 해당 없음 | 이 Task는 core를 건드리지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| UI-2 이름으로 검색·실행, 단축키 표시, Esc로 닫기 | 부분 충족 | 이름 검색·순위는 맞지만 강조된 줄이 순위와 어긋나 다른 명령을 실행한다(지적 1, `command_palette.rs:74-91`, gpui-component `state.rs:467`, `:182`) |
| UI-11 모든 기능은 눈에 보이는 UI | 부분 충족 | 리뷰 1 지적 1의 대부분은 해결(편집 메뉴 8줄, `Surface`). vim 마크 2건이 사실과 다른 자리를 적었다(지적 4, `commands/table.rs:281`, `:435`) |
| KM-13 메뉴에 같은 단축키, 항목 = Action 1:1 | 충족 | 감싼 항목은 제목 꼬리에 키(`commands/mod.rs:356-388`), `menu_target`으로 1:1. 메뉴는 키맵을 걸 때마다 다시 만든다(`lib.rs:67-77`) |
| 7.6 메뉴 구성, 팔레트에서 같은 이름으로 검색 | 부분 충족 | 이름을 그대로 쳐도 앞 질의의 첫 줄이 꺼져 있었다면 강조가 다른 줄이다(지적 1, A-1의 「녹음」) |
| 5.11 팔레트 Tab = 자동완성 확정, Shift+Tab = 이전 후보 | 부분 충족 | Tab이 채운 이름과 Enter가 실행하는 줄이 다르다(지적 1) |
| P2 Esc는 열린 팝업·팔레트를 닫는다 | 부분 충족 | 팔레트와 모드 메뉴가 엇갈리면 모드 메뉴가 Esc로 닫히지 않는다(지적 2, `window.rs:179-183`, `:205-228`, `command_palette.rs:172`) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 app 크레이트 UI만)
- 제품 코드의 `unwrap`/`expect`: 대응 커밋에서 새로 생긴 것 없음. `command_palette.rs` render의 `self.entries[*ix]`는 `visible`이 `entries`에서만 만들어지므로 범위 안
- 에러가 삼켜지는 곳: `window.rs:274-278` 감싼 항목의 `build_action` 실패는 조용히 무시(없는 이름은 사용자 키맵으로만 생기므로 허용). `command_palette.rs:191`의 `on_query.update` 결과 무시(팔레트가 이미 닫힌 경우라 허용)

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 1 지적 2·3이 근본까지 닿지 않았다. 강조가 순위 함수의 첫 실행 가능 줄이 아니라 「앞 질의 목록의 첫 실행 가능 줄 행 번호」에 남는다. 컴포넌트는 질의가 바뀌면 아직 바뀌기 전 모델(앞 `visible`)로 `reset_selection`을 하고(`state.rs:467-476`), 다음 렌더의 `install_model`이 그 `IndexPath`를 새 모델에서 보존한다(`state.rs:182-221`). `filterable(false)`에서 `IndexPath`는 위치일 뿐이라 다른 줄을 가리킨다. 앞 목록 첫 줄이 꺼진 줄이면(인자 열린 Action이 이름 순위 1·2에 오는 경우, 「sequence」「카운트」「se」「CC」) Tab 뒤 Enter가 다른 명령을 실행하고(A-1: 「녹음」 Tab → `sequence::SetLoopMode`), 한 글자 더 친 질의의 강조도 어긋난다(A-2). `set_query`에서 `visible`을 바꾼 뒤(또는 다음 렌더 뒤) 새 목록의 첫 실행 가능 줄로 `set_selected_index`를 다시 거는 등, 강조를 팔레트가 직접 정해야 한다 | `crates/keyscore-app/src/command_palette.rs:74-91`, `:187-191` |
| 2 | 수정 | 리뷰 1 지적 5의 수정(`on_mouse_down_out` 닫기)이 상태줄 모드 메뉴와 엇갈린다. 팔레트가 열린 채 모드 표시를 누르면 모드 메뉴는 열리지만 포커스가 문서로 돌아가(`close_palette`의 `window.focus`) 메뉴가 키를 받지 못하고 Esc로 닫히지 않는다(A-3). 모드 메뉴가 열린 채 `cmd-shift-p`로 팔레트를 열면 모드 메뉴가 닫히지 않고, 팔레트를 닫은 뒤에도 남아 Esc로 닫히지 않는다(A-3b). P2 「Esc는 열린 팝업·팔레트를 닫는다」. 두 겹을 하나만 열리게 하거나(서로 열 때 다른 쪽을 닫기), 포커스 복귀가 다른 겹의 포커스를 빼앗지 않게 해야 한다 | `crates/keyscore-app/src/window.rs:160-183`, `:205-228`, `crates/keyscore-app/src/command_palette.rs:172` |
| 3 | 수정 | `is_bare`가 GPUI `create_menu_item`과 다르다. GPUI는 키 등가물 마스크에 cmd·ctrl·alt·shift만 옮기고 fn은 버린다(`gpui-pre-macos-0.3.8/src/platform.rs:404-427`). `is_bare`는 `m.function`을 수식키로 쳐서 `fn-k`를 맨 키가 아니라고 본다. 사용자 키맵에 컨텍스트 없는 `fn-k`를 걸면 「건반 뷰」에 맨 `k` 키 등가물이 구워져, 리뷰 1 제안 3이 막으려던 「팔레트 입력란의 글자를 메뉴가 가로채기」가 그대로 생긴다(A-4). `no_menu_item_gets_a_bare_key_equivalent`도 같은 함수를 써서 잡지 못한다. `function`을 판정에서 빼고(기능 키 F1–F35 판정은 유지) 테스트를 GPUI 마스크 기준으로 고친다 | `crates/keyscore-app/src/commands/mod.rs:485-493`, `crates/keyscore-app/tests/commands.rs:553-595` |
| 4 | 수정 | UI-11. vim 마크 `cursor::SetMark`(`m`+글자)·`cursor::JumpMark`(`` ` ``+글자)의 자리가 「피아노롤 · 커서(클릭으로도 옮긴다)」인데, 클릭은 마크를 설정하지 않고 이름 붙은 마크로 가지도 않는다. 인자가 열려 있어 팔레트에서도 꺼진 줄이라 지금 이 기능은 키로만 된다. REQ 7.6은 마크의 자리를 주지 않으므로 CLAUDE.md대로 결정 요청(예: 「vim 전용 기능의 UI-11 적용 범위」)을 쓰고 표에 그 결정을 가리키게 해야 한다. 리뷰 1 지적 1(차단)의 남은 부분이다 | `crates/keyscore-app/src/commands/table.rs:281`, `:435` |

## 제안 (구현자가 거절할 수 있음)

1. 감싼 항목은 메뉴 사용 가능 여부가 바뀐다. GPUI는 `cx.is_action_available(메뉴 Action)`으로 항목을 켜고 끄는데(`gpui-pre-0.3.8/src/platform/app_menu.rs:341-349`), 감싼 항목의 Action은 창 루트에 처리기가 있는 `RunCommand`라 창이 있으면 늘 켜진다. 처리기가 아직 없는 `edit::Delete`·`edit::Quantize` 등은 감싸지 않은 이웃(`edit::Undo` 등)과 달리 켜진 채 눌러도 아무 일이 없다. 처리기가 생기는 S3에서 문제가 줄지만, 메뉴 상태를 일관되게 하려면 `RunCommand` 처리기 등록을 조건부로 하거나 한계로 적는다.
2. `elsewhere_places_name_a_visible_ui`(`tests/review_s2_t4.rs:285`)는 `ends_with("키")`라 「vim 조합 키: 마우스 선택과 같은 기능」을 통과시킨다. 「키」를 자리 설명의 주어로 쓰지 않도록(오퍼레이터·텍스트 객체는 「선택 후 편집 메뉴 삭제·복사」처럼 화면 조작을 앞에) 문구와 검사를 맞추기를 권한다. `edit::CancelDrag`의 「마우스 드래그 중 Esc」도 키를 자리로 적은 것이다.
3. 순위에서 꺼진 줄이 더 낮은 순위의 실행 가능 줄보다 앞선다(「sequence」의 1행이 꺼진 `sequence::Note`). 지적 1을 고친 뒤에도 Enter가 1행이 아닌 2행을 실행하는 모습이 사용자에게 혼란스러울 수 있다. 같은 순위 안에서만이 아니라 「실행 가능 → 꺼짐」을 순위보다 앞에 둘지 정해 적는다.
4. alt 수식 바인딩도 GPUI가 키 등가물로 굽는다. 팔레트 입력란에서 Option+글자로 특수 문자를 칠 때 같은 가로채기가 생길 수 있다(지금 메뉴 항목 중 alt 바인딩이 골라지는 것은 없음). S2 출구 수동 확인 항목에 함께 적어 두기를 권한다.

## 리뷰 1 지적·제안의 처리 확인

| 항목 | 판정 | 근거 |
| --- | --- | --- |
| 지적 1(UI-11) | 대부분 해결, 남은 것 지적 4 | `Surface` 타입, 편집 메뉴 8줄(메뉴 80줄) |
| 지적 2(Tab 뒤 Enter) | 미해결 | 지적 1 |
| 지적 3(메뉴 이름 = 첫 줄) | 순위 함수는 해결, 컴포넌트 강조는 미해결 | 지적 1 |
| 지적 4(변형 이름 겹침) | 해결 | `palette_labels_are_unique_including_variants` 통과 |
| 지적 5(문서 클릭 뒤 팔레트 잔류) | 해결, 새 문제 지적 2 | |
| 제안 1(ctrl-tab) | 해결 | |
| 제안 2(메뉴 재생성 단언) | 해결 | `menus_follow_keymap_changes`, 이번 12·13번 시도 |
| 제안 3(맨 키 등가물) | 해결, 규칙 착오 지적 3 | |
| 제안 4(설정 항목) | 해결 | 앱 메뉴 「설정」 직접 항목 |
| 제안 5(Shift+Tab) | 거절 사유 수용 | |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 6개 실패, `review_s2_t4_r2.rs` 11개 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(미뤄 둔 `tests/deferred/count_non_key_paths.rs`는 결정 C(메뉴 막대는 한계)에 맞게 고쳐 `tests/count_non_key_paths.rs`로 옮겼다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 테스트

`crates/keyscore-app/tests/` 아래 새 파일로 두면 그대로 돈다. 리뷰 시점(`8ac5cbd`)에 6개 모두 실패했다.

~~~rust
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, OwnedMenuItem, TestAppContext};
use keyscore_app::commands;
use keyscore_app::keymap;
use keyscore_app::mode::Mode;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("ks-review-s2t4-r2f-{}-{name}", std::process::id()));
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

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    render(cx, handle);
}

fn palette_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).palette_open())
}

fn mode_menu_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_menu_open())
}

fn mode_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

// A-1 (지적 1): 첫 줄이 꺼진 질의에서 Tab → Enter가 Tab으로 채운 명령이 아닌 둘째 줄을 실행한다.
// 「sequence」의 첫 줄은 꺼진 `sequence::Note`, 강조는 둘째 줄 「녹음」(mode::Sequence)이다.
#[gpui_kit::test]
fn tab_then_enter_after_disabled_top_row_runs_completed_command(cx: &mut TestAppContext) {
    start(cx, "a1");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "sequence");
    press(cx, handle, "tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some("녹음")
    );
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert_eq!(
        mode_of(cx, &view),
        Mode::Sequence,
        "Tab으로 채운 「녹음」의 Enter가 mode::Sequence를 실행하지 않았다"
    );
}

// A-2 (지적 1): 한 글자 더 치면 강조가 앞 질의의 「첫 실행 가능 줄 위치」에 남는다(한글 질의).
#[gpui_kit::test]
fn typing_one_more_char_highlights_first_runnable_row(cx: &mut TestAppContext) {
    start(cx, "a2");
    let (handle, view) = open(cx);
    let expected = cx.update(|cx| {
        let entries = commands::palette_entries(cx);
        let order = commands::filter_entries(&entries, "카운트인");
        let first = order
            .iter()
            .find(|ix| entries[**ix].action().is_some())
            .unwrap();
        entries[*first].label().to_string()
    });
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "카운트");
    type_text(cx, handle, "인");
    // Tab은 강조된 줄의 이름으로 질의를 채우므로 강조 위치가 드러난다
    press(cx, handle, "tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)),
        Some(expected),
        "「카운트」의 첫 줄(꺼진 operator::Count) 때문에 강조가 둘째 줄에 남았다"
    );
}

// A-3 (지적 2): 팔레트가 열린 채 상태줄 모드 표시를 누르면 모드 메뉴가 열리지만 포커스는 문서로 간다.
// 모드 메뉴는 키(아래·Enter·Esc)를 받지 못하고 Esc로 닫히지 않는다(P2).
#[gpui_kit::test]
fn mode_menu_opened_by_click_while_palette_open_takes_keys(cx: &mut TestAppContext) {
    start(cx, "a3");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    click(cx, handle, "status-mode");
    assert!(!palette_open(cx, &view));
    assert!(mode_menu_open(cx, &view));
    press(cx, handle, "escape");
    assert!(
        !mode_menu_open(cx, &view),
        "모드 메뉴가 열린 채 남고 Esc로 닫히지 않는다"
    );
}

// A-3b (지적 2): 모드 메뉴가 열린 채 cmd-shift-p로 팔레트를 열면 모드 메뉴가 남는다. 팔레트를 닫은 뒤에도
// 모드 메뉴는 Esc로 닫히지 않는다.
#[gpui_kit::test]
fn opening_palette_over_mode_menu_does_not_orphan_it(cx: &mut TestAppContext) {
    start(cx, "a3b");
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    assert!(mode_menu_open(cx, &view));
    press(cx, handle, "cmd-shift-p");
    press(cx, handle, "escape");
    press(cx, handle, "escape");
    assert!(!palette_open(cx, &view));
    assert!(
        !mode_menu_open(cx, &view),
        "팔레트를 닫은 뒤에도 모드 메뉴가 남고 Esc로 닫히지 않는다"
    );
}

// A-4 (지적 3): GPUI `create_menu_item`의 수식키 마스크는 cmd·ctrl·alt·shift만 본다. `fn-k`를 컨텍스트 없이
// 걸면 GPUI는 맨 `k`를 키 등가물로 굽는데, `commands::is_bare`는 fn을 수식키로 쳐서 감싸지 않는다.
#[gpui_kit::test]
fn fn_modifier_binding_is_not_baked_as_bare_menu_key(cx: &mut TestAppContext) {
    let dir = start(cx, "a4");
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"bindings": {"fn-k": "view::ToggleKeyboard"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let view_menu = menus.iter().find(|m| m.name.as_ref() == "보기").unwrap();
    let (title, key) = cx.update(|cx| {
        view_menu
            .items
            .iter()
            .find_map(|i| match i {
                OwnedMenuItem::Action { name, action, .. } if name.starts_with("건반 뷰") => {
                    Some((
                        name.clone(),
                        commands::native_key_equivalent(action.as_ref(), cx),
                    ))
                }
                _ => None,
            })
            .unwrap()
    });
    // GPUI가 실제로 굽는 마스크(cmd·ctrl·alt·shift)로 본 맨 키
    let baked_bare = key.is_some_and(|k| {
        let m = k.modifiers;
        !(m.platform || m.control || m.alt) && k.key.len() == 1
    });
    assert!(!baked_bare, "「{title}」에 맨 키 k가 키 등가물로 구워진다");
}

// A-5 (지적 4): UI-11. REQ가 눈에 보이는 자리를 주지 않은 vim 마크(5.8.1 `m`, `` ` ``)는 지어낸 자리 대신 결정
// 요청이 있어야 한다. 「커서(클릭으로도 옮긴다)」는 마크를 설정하지도, 이름 붙은 마크로 가지도 않는다.
#[test]
fn marks_have_a_visible_place_or_a_decision_request() {
    let decisions = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/decisions");
    let text: String = std::fs::read_dir(decisions)
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap_or_default())
        .collect();
    let missing: Vec<String> = ["cursor::SetMark", "cursor::JumpMark"]
        .iter()
        .filter_map(|name| match commands::spec(name).unwrap().place() {
            commands::Place::Elsewhere { detail, .. } if !text.contains(name) => {
                Some(format!("{name} ({detail})"))
            }
            _ => None,
        })
        .collect();
    assert!(missing.is_empty(), "{}건: {:?}", missing.len(), missing);
}
~~~
