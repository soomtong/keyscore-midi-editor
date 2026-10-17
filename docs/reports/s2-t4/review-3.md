# 적대적 리뷰: S2-T4 명령 팔레트와 메뉴바 (리뷰 3회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음). 프로토콜상 마지막 재리뷰
- 대상 커밋: `f5642a5`(브랜치 `s2/t4-palette-menus`, 범위 `git diff main...HEAD`). 리뷰 2 대응은 `a28aeb2`(결정·이월 기록), `f5642a5`(코드)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show a28aeb2 f5642a5`, `command_palette.rs`, `commands/mod.rs`, `window.rs`, gpui-component `command/state.rs`(`install_model`, `on_query_changed`, `reset_selection`, `select_by`), gpui-pre-macos `create_menu_item`을 읽고 적었다.

1. 「실행 가능한 줄 먼저」로 첫 줄이 늘 실행 가능해졌다. 그런데 앞 목록에 실행 가능한 줄이 없으면(맞는 줄 없음, 꺼진 줄만 맞음) 컴포넌트의 강조는 `None`이다. 그 뒤 질의를 넓히면 강조가 살아나는가(`install_model`의 `preserve_no_selection` 분기).
2. 위/아래로 강조를 옮긴 뒤 글자를 치면 강조가 첫 줄로 돌아오는가. Shift+Tab(`SelectUp`)이 첫 줄에서 돌아 꺼진 줄(맨 뒤)에 서지 않는가.
3. 실제 컴포넌트에서 실행 가능한 모든 이름이 자기 줄을 강조하는가(리뷰 2의 검사는 순위 함수만 봤다).
4. 팝업 하나만: `open_mode_menu`의 `self.palette = None`은 포커스를 돌리지 않는다. 메뉴 막대·팔레트 줄로 `mode::ShowMenu`를 낼 때 모드 메뉴가 키를 받는가. 팔레트 → 모드 메뉴 → 팔레트 왕복 뒤 남는 팝업이 있는가.
5. `close_palette`의 「포커스가 팔레트 안이거나 없을 때만 문서로」 조건이 문서 클릭·Esc·실행 경로에서 포커스를 잃게 하지 않는가.
6. `is_bare`: Alt만 있는 키는 감싸고, Ctrl·기능 키는 감싸지 않는가. 감싼 제목 꼬리의 표시(⌥).
7. KM-13 「메뉴에 같은 단축키」: 제목 꼬리 키는 `native_key_equivalent`(GPUI `find_or_first`)로 고른다. 이 규칙은 컨텍스트 가려짐을 보지 않는다. vim 덧층이 같은 키를 다른 Action에 걸면 메뉴가 엉뚱한 키를 보이는가. 시퀀스(`z z`)만 걸린 항목은 표시가 있는가.
8. UI-11 자리 설명: 리뷰 2 제안 2의 검사(「키맵」을 뺀 「키」 금지)가 실제로 있는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 강조 없음 → 질의 넓힘 | 없음 | 이번에 추가(통과) |
| 2. 위/아래 뒤 입력, Shift+Tab 돌기 | 부분 | `palette_tab_completes_and_shift_tab_moves_back`는 꺼진 줄이 없는 질의. 이번에 추가(통과) |
| 3. 컴포넌트 강조의 전수 | 없음 | `every_palette_label_selects_its_own_row_first`는 순위 함수만. 이번에 추가(통과) |
| 4. 메뉴 막대·팔레트 줄의 `mode::ShowMenu`, 왕복 | 없음 | 리뷰 2 A-3·A-3b는 클릭과 `cmd-shift-p`만. 이번에 추가(통과) |
| 5. 닫힐 때 포커스 | 있음 | 리뷰 1 A-4, 리뷰 2 A-3 |
| 6. Alt·Ctrl·기능 키 감싸기 | 부분 | `fn`만(리뷰 2 A-4). 이번에 추가(통과) |
| 7. 메뉴 키 표시의 가려짐·시퀀스 | 없음 | `no_menu_item_gets_a_bare_key_equivalent`·`menus_follow_keymap_changes`는 default 프리셋의 감쌈 여부만 본다. 부록 B-1, B-2(실패) |
| 8. 자리 설명의 「키」 | 있음 | `commands.rs:129-134`(`every_action_has_exactly_one_command_spec` 안) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | vim 프리셋에서 시퀀스만 걸린 메뉴 항목의 단축키 표시 | 깨짐: 「세로 확대」(`z k`), 「세로 축소」(`z j`), 「커서를 가운데로」(`z z`)에 단축키가 보이지 않는다 | 부록 B-1 |
| 2 | vim 프리셋에서 감싼 메뉴 항목의 제목 꼬리 키를 모든 모드 컨텍스트에서 풀어 봄 | 깨짐: 8건이 어느 컨텍스트에서도 다른 Action이다. 「배치 뷰    V」(`v` = `select::VisualNote`), 「반복    R」(`edit::SetPitch`), 「분할    /」(`search::Open`), 「가사 입력    W」(`cursor::NextNote`), 「음높이 바꾸기    E」(`cursor::NoteEnd`), 「컨트롤러 레인    C」(`operator::Change`), 「솔로    O」(`track::NewBelow`), 「AI 생성    A」(`mode::StepAfterNote`) | 부록 B-2 |
| 3 | 같은 vim 프리셋에서 팔레트의 「배치 뷰」 단축키 표시 | 못 깨뜨림(가려진 `v`를 보이지 않는다) | `palette_does_not_show_shadowed_key_in_vim` |
| 4 | 맞는 줄 없는 질의 → Backspace → Enter | 못 깨뜨림 | `backspace_from_no_match_highlights_first_runnable_row` |
| 5 | 꺼진 줄만 맞는 질의(`cursor::setmark`) → Enter(무시) → Esc(질의 지움) → Tab | 못 깨뜨림 | `disabled_only_query_then_clear_highlights_first_runnable_row` |
| 6 | 아래 두 번 → 글자 더 입력 → Tab | 못 깨뜨림 | `typing_after_moving_down_resets_highlight_to_top` |
| 7 | 꺼진 줄이 맞는 「카운트」에서 Shift+Tab → Tab | 못 깨뜨림(마지막 실행 가능 줄) | `shift_tab_from_top_wraps_to_last_runnable_row` |
| 8 | 리뷰 2에서 어긋났던 질의 15개를 한 글자씩, 한 팔레트에서 Esc로 지우며 이어 침 | 못 깨뜨림 | `incremental_queries_always_highlight_first_runnable_row` |
| 9 | 실제 컴포넌트에 실행 가능한 이름 전수 입력 → Tab | 못 깨뜨림(전수 85초 1회 실행. 남긴 테스트는 8줄마다 표본) | `every_runnable_label_is_highlighted_in_component` |
| 10 | 빈 질의의 순서 | 못 깨뜨림(실행 가능 → 꺼짐, 그 안은 표 순서) | `empty_query_lists_runnable_rows_first_in_table_order` |
| 11 | 팔레트가 열린 채 메뉴 막대로 `mode::ShowMenu` → Esc → `ctrl-tab` | 못 깨뜨림 | `show_menu_from_menu_bar_while_palette_open` |
| 12 | 팔레트 줄 `mode::ShowMenu` Enter → Esc → `ctrl-tab` | 못 깨뜨림 | `show_menu_from_palette_row_takes_keys` |
| 13 | 모드 메뉴가 열린 채 문서 클릭 → `ctrl-tab` | 못 깨뜨림 | `clicking_document_closes_mode_menu` |
| 14 | 팔레트(질의 있음) → 모드 표시 클릭 → `cmd-shift-p` → 실행 | 못 깨뜨림(새 팔레트는 빈 질의, 팝업 잔류 없음) | `palette_mode_menu_palette_roundtrip` |
| 15 | 사용자 키맵 `alt-k` | 못 깨뜨림(감쌈, 제목 꼬리 ⌥K) | `alt_binding_is_wrapped_and_shown` |
| 16 | 사용자 키맵 `ctrl-k`, `f5` | 못 깨뜨림(감싸지 않음) | `ctrl_and_function_keys_are_not_wrapped` |

통과한 시도는 `crates/keyscore-app/tests/review_s2_t4_r3.rs`(14개)에 있다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 등록 Action = 팔레트 줄 = 메뉴 + 메뉴 밖 | 아니오 | 게이트 테스트 통과 |
| 강조된 줄 = Enter가 실행하는 줄 = Tab으로 채운 줄 | 아니오 | 시도 4–10 |
| 팝업은 하나, 닫히면 포커스는 문서 또는 다른 팝업 | 아니오 | 시도 11–14 |
| 메뉴에 맨 키 등가물 없음 | 아니오 | 시도 15–16, 리뷰 2 시도 12–13 |
| 메뉴에 보인 단축키 = 그 명령을 실행하는 키 | 예 | vim 프리셋(지적 1, 2) |
| 모델 불변식(core) | 해당 없음 | 이 Task는 core를 건드리지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| UI-2 이름으로 검색·실행, 단축키 표시, Esc로 닫기 | 충족 | `commands/mod.rs:399-428`(실행 가능 줄 먼저), 시도 4–10. 단축키는 가려짐을 보는 `highest_precedence_binding_for_action_in`(`command_palette.rs:95`), 시도 3 |
| UI-11 모든 기능은 눈에 보이는 UI | 충족 | 마크는 결정 `2026-10-17-vim-marks-visible-ui.md` a, S3 이월(`13-S3-step-and-pianoroll.md`). 자리 설명의 「키」 금지 검사 `commands.rs:129-134` |
| KM-13 메뉴에 같은 단축키, 항목 = Action 1:1 | 부분 충족 | 1:1은 충족. vim 프리셋에서 단축키 표시가 틀리거나 빠진다(지적 1·2, `commands/mod.rs:370`, `:459-482`). 단계 파일 S2-T4 테스트 게이트 「단축키 표시가 현재 키맵을 반영」도 메뉴에서는 지켜지지 않는다 |
| 7.6 메뉴 구성, 팔레트에서 같은 이름으로 검색 | 충족 | 시도 9(컴포넌트 전수) |
| 5.11 팔레트 Tab·Shift+Tab | 충족 | 시도 6–7 |
| P2 Esc는 열린 팝업·팔레트를 닫는다 | 충족 | `window.rs:165-166`, `:216-217`, `:185-191`, 시도 11–14, 리뷰 2 A-3·A-3b |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(app 크레이트 UI만)
- 제품 코드의 `unwrap`/`expect`: 대응 커밋에서 새로 생긴 것 없음
- 에러가 삼켜지는 곳: 리뷰 2와 같음(감싼 항목의 `build_action` 실패, 닫힌 팔레트에 대한 `update` 결과). 새로 생긴 것 없음

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | KM-13. 키 시퀀스만 걸린 Action의 메뉴 항목에 단축키가 보이지 않는다. vim 프리셋의 「세로 확대」(`z k`), 「세로 축소」(`z j`), 「커서를 가운데로」(`z z`). GPUI는 한 키가 아닌 바인딩을 키 등가물로 굽지 않고, `native_key_equivalent`도 한 키가 아니면 `None`이라 제목 꼬리 표시도 붙지 않는다. 이미 맨 키에 쓰는 「제목 꼬리 표시」를 시퀀스에도 쓰면 된다(B-1). 1·2회차가 놓쳤다 | `crates/keyscore-app/src/commands/mod.rs:370`, `:478-481`, `assets/keymaps/vim.json:32-34` |
| 2 | 수정 | KM-13, S2-T4 테스트 게이트 「단축키 표시가 현재 키맵을 반영」. vim 프리셋에서 감싼 메뉴 항목 8개의 제목 꼬리 키가 그 명령이 아니라 vim 덧층의 다른 명령을 실행한다(「배치 뷰    V」를 보고 `v`를 누르면 노트 비주얼 선택, 「AI 생성    A」의 `a`는 노트 뒤 Step 진입). 제목 꼬리는 GPUI `find_or_first`(가려짐을 보지 않음)를 흉내 낸 `native_key_equivalent`로 고르기 때문이다. 감싼 항목의 제목은 앱이 만드므로, 표시할 키를 「모드 컨텍스트 가운데 하나에서 실제로 그 Action으로 풀리는 바인딩」(팔레트와 같이 가려짐을 보는 규칙)으로 고르고, 없으면 키를 보이지 않아야 한다(B-2). 알려진 한계 「사용자가 `null`로 지운 프리셋 키」와 뿌리는 같지만, 이것은 사용자 조작 없이 기본 제공 프리셋에서 생긴다. 1·2회차가 놓쳤다 | `crates/keyscore-app/src/commands/mod.rs:370-376`, `:459-482`, `assets/keymaps/vim.json:28,36,40,50,132,143,150,155` |

## 제안 (구현자가 거절할 수 있음)

1. `filter_entries` 문서 주석 첫 줄 「빈 질의는 모든 줄을 표 순서대로 준다」는 이제 틀리다(빈 질의도 실행 가능한 줄이 먼저). 둘째 단락과 맞춘다(`commands/mod.rs:392`).
2. 리뷰 2 제안 1(감싼 항목은 처리기가 없어도 켜져 보인다)을 「한계로 남긴다」고 했지만 보고서 「알려진 한계」 목록에는 없다. 목록에 옮겨 S2 출구에서 보이게 한다.
3. `operator::Count`(「카운트」)의 자리 설명 「마우스 선택 후 편집 메뉴(삭제·복사·변경)와 같은 기능」은 오퍼레이터 셋과 같은 문구다. 카운트는 반복 횟수라 「같은 기능」의 근거가 약하다. 「마우스로 범위를 잡아 한 번에 편집」처럼 카운트에 맞게 적기를 권한다(`commands/table.rs:1017`).
4. 꺼진 줄의 이름을 그대로 치면(「카운트」, 「마크 설정」) 강조는 다른 실행 가능 줄에 가고 Tab이 질의를 그 줄 이름으로 바꾼다. 리뷰 2 제안 3에서 정한 대로이므로 결함은 아니다. 꺼진 줄에 「인자 입력은 A12 뒤」 같은 표시를 붙이면 혼란이 준다.
5. 지적 2를 고치면 같은 규칙으로 「감싸지 않은 항목(Cmd·Ctrl)이 GPUI가 굽는 키도 가려졌는가」를 테스트로 지킨다(지금 기본·vim 프리셋에서 해당 0건, 사용자 키맵으로는 생길 수 있음).

## 리뷰 2 지적·제안의 처리 확인

| 항목 | 판정 | 근거 |
| --- | --- | --- |
| 지적 1(강조와 순위 어긋남) | 해결(근본) | 실행 가능 줄이 늘 첫 줄이라 컴포넌트가 어느 목록의 첫 줄로 되돌려도 맞다. 강조 없음에서 넓히는 경로(`install_model`의 `reset_selection`)도 맞다. 시도 4–10 |
| 지적 2(팔레트 ↔ 모드 메뉴) | 해결, 새 문제 없음 | 시도 11–14 |
| 지적 3(`is_bare`와 GPUI 마스크) | 해결 | fn 무시, Alt 감쌈, Ctrl·기능 키 유지. 시도 15–16 |
| 지적 4(마크 자리) | 해결 | 결정 a, 자리 설명이 결정 파일을 가리킴, S3 이월 기록 |
| 제안 1(감싼 항목의 사용 가능 여부) | 한계로 수용 | 알려진 한계 목록 반영은 제안 2 |
| 제안 2(「키」 설명 검사) | 해결 | `commands.rs:129-134` |
| 제안 3(꺼진 줄 순위) | 해결 | 지적 1과 함께 |
| 제안 4(Alt) | 해결 | 출구 수동 확인 문구 갱신(`12-S2-app-shell.md:75`) |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2의 지적 넷은 모두 근본까지 해결됐고 새 회귀는 없다. 남은 것은 1·2회차가 놓친 KM-13 결함 둘(vim 프리셋의 메뉴 단축키 표시)이다. 이번이 프로토콜상 세 번째 재리뷰이므로 `plan/01-protocol.md` [8]에 따라 주 세션은 결정 요청으로 넘긴다. 리뷰어가 보는 선택지는 둘이다.

- a. 지적 1·2만 고치고(`action_item`의 표시 키 선택, 범위가 좁다) 부록 B-1·B-2와 시도 3을 회귀 테스트로 옮긴 뒤, 그 두 항목만 보는 확인 리뷰로 병합한다.
- b. 범위 재정의: 「메뉴 단축키 표시의 가려짐·시퀀스」를 알려진 한계(`null`로 지운 프리셋 키)와 함께 S2-T6(키맵 편집기, KM-10)으로 이월하고 S2-T4를 병합한다. 이 경우 S2-T4 테스트 게이트 「단축키 표시가 현재 키맵을 반영」의 메뉴 부분이 S2-T6으로 옮겨 감을 단계 파일에 적어야 한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 2개 실패, `review_s2_t4_r3.rs` 14개 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`a28aeb2`의 S3 이월 한 줄은 결정 a의 기록)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 B. 실패하는 테스트

`crates/keyscore-app/tests/` 아래 새 파일로 두면 그대로 돈다. 리뷰 시점(`f5642a5`)에 2개 모두 실패했다.

~~~
B-1: 단축키가 있는데 메뉴에 보이지 않는 항목 3건: ["세로 확대", "세로 축소", "커서를 가운데로"]
B-2: 메뉴에 보인 키가 그 명령을 실행하지 않는 항목 8건: ["반복    R", "분할    /", "가사 입력    W", "음높이 바꾸기    E", "배치 뷰    V", "컨트롤러 레인    C", "솔로    O", "AI 생성    A"]
~~~

~~~rust
use gpui_kit::{App, OwnedMenuItem, TestAppContext};
use keyscore_app::commands;
use keyscore_app::keymap::{self, Preset};

// B-1 (지적 1): KM-13. vim 프리셋에서 키 시퀀스(`z k`, `z j`, `z z`)만 걸린 메뉴 항목은 메뉴에 단축키가
// 보이지 않는다. GPUI는 한 키가 아닌 바인딩을 키 등가물로 굽지 않고(`create_menu_item`), 감싼 항목의 제목
// 꼬리 표시도 한 키(`native_key_equivalent`)일 때만 붙는다.
#[gpui_kit::test]
fn sequence_shortcuts_are_shown_in_menu_titles(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t4-r3f-{}-b1", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| keyscore_app::init(cx, dir.join("settings.json")));
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    fn walk(items: &[OwnedMenuItem], cx: &App, out: &mut Vec<String>) {
        for item in items {
            match item {
                OwnedMenuItem::Action { name, action, .. } => {
                    let target = commands::menu_target(action.as_ref(), cx).unwrap();
                    let has_binding = cx
                        .key_bindings()
                        .borrow()
                        .bindings_for_action(target.as_ref())
                        .next()
                        .is_some();
                    let shown = commands::native_key_equivalent(action.as_ref(), cx).is_some()
                        || name.contains(commands::MENU_KEY_SEPARATOR);
                    if has_binding && !shown {
                        out.push(name.clone());
                    }
                }
                OwnedMenuItem::Submenu(menu) => walk(&menu.items, cx, out),
                _ => {}
            }
        }
    }
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let mut missing = Vec::new();
    cx.update(|cx| {
        for menu in &menus {
            walk(&menu.items, cx, &mut missing);
        }
    });
    assert!(
        missing.is_empty(),
        "단축키가 있는데 메뉴에 보이지 않는 항목 {}건: {:?}",
        missing.len(),
        missing
    );
}

// B-2 (지적 2): KM-13. vim 프리셋에서 감싼 메뉴 항목의 제목 꼬리 키가 어느 모드 컨텍스트에서도 그 Action이
// 아니라 vim 덧층의 다른 Action을 실행한다(「배치 뷰    V」인데 `v`는 select::VisualNote). 제목 꼬리는
// GPUI의 `find_or_first`(가려짐을 보지 않는다)로 고른 첫 바인딩이다. 팔레트는 가려짐을 보는
// `highest_precedence_binding_for_action_in`을 써서 맞다.
#[gpui_kit::test]
fn wrapped_menu_keys_are_not_shadowed_in_vim(cx: &mut TestAppContext) {
    use gpui_kit::KeyContext;
    const PATHS: &[&[&str]] = &[
        &["Normal", "Arrangement"],
        &["Normal", "Arrangement", "Selection"],
        &["Normal", "PianoRoll"],
        &["Normal", "PianoRoll", "Selection"],
        &["Step"],
        &["Step", "Ghost"],
        &["Sequence"],
        &["Play"],
        &["AI"],
        &["Lyric"],
        &["Controller"],
    ];
    fn walk(items: &[OwnedMenuItem], cx: &App, out: &mut Vec<String>) {
        for item in items {
            match item {
                OwnedMenuItem::Action { name, action, .. }
                    if action.as_any().is::<commands::RunCommand>() =>
                {
                    let target = commands::menu_target(action.as_ref(), cx).unwrap();
                    let key = commands::native_key_equivalent(target.as_ref(), cx).unwrap();
                    let keymap = cx.key_bindings();
                    let keymap = keymap.borrow();
                    let runs_target = PATHS.iter().any(|path| {
                        let stack: Vec<KeyContext> =
                            path.iter().map(|c| KeyContext::parse(c).unwrap()).collect();
                        let (bindings, _) = keymap.bindings_for_input(&[key.clone()], &stack);
                        bindings
                            .first()
                            .is_some_and(|b| b.action().partial_eq(target.as_ref()))
                    });
                    if !runs_target {
                        out.push(name.clone());
                    }
                }
                OwnedMenuItem::Submenu(menu) => walk(&menu.items, cx, out),
                _ => {}
            }
        }
    }
    let dir = std::env::temp_dir().join(format!("ks-review-s2t4-r3f-{}-b2", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| keyscore_app::init(cx, dir.join("settings.json")));
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let menus = cx.update(|cx| cx.get_menus()).unwrap();
    let mut wrong = Vec::new();
    cx.update(|cx| {
        for menu in &menus {
            walk(&menu.items, cx, &mut wrong);
        }
    });
    assert!(
        wrong.is_empty(),
        "메뉴에 보인 키가 그 명령을 실행하지 않는 항목 {}건: {:?}",
        wrong.len(),
        wrong
    );
}
~~~
