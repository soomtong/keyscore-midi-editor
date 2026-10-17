# 적대적 리뷰: S2-T3 모드 FSM과 Enter/Esc/Tab 규약 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트). 1회차 리뷰어와도 다른 세션이다.
- 대상 커밋: `aee4fbf`(브랜치 `s2/t3-modes`, 범위 `git diff main...HEAD`. 리뷰 1 대응은 `git show aee4fbf`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show aee4fbf`의 `mode.rs`, `window.rs`, `keymap/mod.rs` 변경분과 리뷰 1, 보고서 「리뷰 1 대응」만 읽고 적었다. 테스트는 이 목록을 적은 뒤에 열었다.

1. 지적 1 대응(「노트 입력 모드는 피아노롤로」 규칙 삭제)이 만든 새 상태(Step·Sequence·Lyric·Controller × 배치 뷰)가 REQ 5.5 Esc 사슬 「모드 → 피아노롤 → 배치 뷰」, 5.1 `i`, 5.9 Controller, 5.15 가사와 맞는가. 기본 키로 실제로 닿는가.
2. 이 규칙 삭제가 「REQ에 없는 결정」이 아닌가. 리뷰 1은 결정 요청을 먼저 쓰라고 했다.
3. 모드를 바꿔도 선택이 남으므로, 선택을 든 채 들어간 모드에서 `Esc`(A13 선택지 a)가 보이지 않는 선택부터 지우지 않는가.
4. 루트 자식으로 옮긴 모드 메뉴: 열린 채 각 모드(Normal·Step·Play·Lyric)의 컨텍스트 키가 새는가. `Normal` 컨텍스트의 `cmd-` 키(`cmd-i`)는 처리기가 메뉴의 조상(루트)에 있으니 디스패치되는가.
5. 메뉴가 열린 채 `tab`(바탕 레이어 `root::Tab`)이 포커스를 빼앗아 메뉴를 고아로 만드는가.
6. 메뉴 위치: `STATUS_BAR_HEIGHT` 근삿값(28px)으로 모드 표시를 가리거나 떠 있지 않은가. 창 크기를 바꾸면 따라오는가. 작은 창에서 잘리는가.
7. 바깥 클릭(트랙 목록·트랜스포트·인스펙터)과 포커스 복귀.
8. 대기 중인 멀티키(`g`)가 메뉴 포커스로 넘어가 메뉴 안에서 완성되는가.
9. 메뉴로 모드를 고를 때도 「모드만 바뀐다」인가.
10. `LeaveAi`: AI 밖에서 무변화, AI에서 뷰·선택 유지하며 Normal.
11. 제안 1 대응: `install` 두 번이면 바탕 레이어는 그대로지만, 카운트 처리기(`register_count_actions`)와 키 관찰자가 두 번 달린다. 숫자가 두 번 세어지는가. 우리 레이어가 두 벌 걸리는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 모드 × 배치 뷰 상태와 REQ | 부분 | `mode_fsm::transition_table_matches_req_5_5`가 `(Arr, Normal) Enter(Lyric) → (Arr, Lyric)`, `Enter(Controller) → (Arr, Controller)`를 기대값으로 굳혔다. REQ 5.15·5.9와의 대조는 없다 |
| 2. 결정 요청 | 없음 | 보고서가 「결정 요청은 쓰지 않았다」고 적었다 |
| 3. 선택을 든 채 모드에서 Esc | 부분 | 전이 표에 `(Pr, Step, sel) Back → (Pr, Step, no sel)` 행은 있다. 선택이 모드로 넘어가는 경로(배치 뷰 선택 → `ctrl-tab`)는 없다 |
| 4. 메뉴가 열린 채 각 모드의 키 | 부분 | `review_s2_t3::mode_menu_open_keeps_document_keys_out`이 Normal의 `v`·`a`만 본다 |
| 5. 메뉴와 `tab` | 없음 | |
| 6. 메뉴 위치·창 크기 | 없음 | |
| 7. 바깥 클릭 | 부분 | `main-view` 클릭만 |
| 8. 대기 멀티키와 메뉴 | 없음 | |
| 9. 메뉴 선택과 뷰·선택 | 없음 | `modes_ui`의 메뉴 테스트는 모드만 본다 |
| 10. `LeaveAi` | 있음 | 전이 표 3행과 `review_s2_t3::ai_accept_dismiss_outside_ai_keep_mode` |
| 11. `install` 두 번 | 부분 | `install_twice_does_not_freeze_user_layer`가 지운 사용자 바인딩만 본다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 기본 프리셋, 배치 뷰에서 `w`, `shift-c`, `ctrl-tab`, `cmd-r` | 깨짐(REQ 대조). 각각 `(Arrangement, Lyric)`, `(Arrangement, Controller)`, `(Arrangement, Step)`, `(Arrangement, Sequence)`. 사용자 키맵으로 `escape → mode::Back`(A13 선택지 a)을 걸면 `Esc` 한 번에 배치 뷰 Normal로 간다(피아노롤을 거치지 않는다) | 부록 A1(`lyric_and_controller_from_arrangement_open_in_piano_roll`) |
| 2 | vim 프리셋, 배치 뷰에서 `i` | 관찰. `(Arrangement, Step)`. 같은 자리의 `enter`는 `clip::Open`(피아노롤 Normal)이다. 5.1 「vim 프리셋은 `i`도 받는다」(`Enter`와 같은 진입)와 두 키의 결과가 갈린다 | 없음(결정 대상이라 동작을 굳히지 않았다) |
| 3 | 배치 뷰·선택 있음 → `ctrl-tab` → `escape`(선택지 a를 사용자 키맵으로) | 관찰. `(Arrangement, Step, 선택 있음)` → `(Arrangement, Step, 선택 없음)`. 상태줄은 `STEP` 그대로이고 컨텍스트 경로(`Step`)에 선택이 없어 화면상 아무 일도 없다. 두 번째 `escape`에서 Normal | 없음(제안 2) |
| 4 | Normal·Step·Play·Lyric 각각에서 메뉴를 열고 `v` `a` `w` `shift-c` `cmd-r` `cmd-k` `space` `1` | 못 깨뜨림. 상태 그대로, 메뉴 열린 채, `escape`로 닫히고 포커스 복귀 | `mode_menu_open_keeps_mode_keys_out_in_every_mode` |
| 5 | 메뉴를 열고 `cmd-i`(`Normal` 컨텍스트의 `view::ToggleInspector`, 처리기는 루트) | 못 깨뜨림. 인스펙터 그대로 | `mode_menu_open_normal_context_cmd_keys_stay_out` |
| 6 | 메뉴를 열고 `tab` → `escape` → `v` | 못 깨뜨림 | `mode_menu_open_tab_then_escape_returns_focus` |
| 7 | 메뉴 경계를 모드 표시와 비교, 창을 900×420으로 줄인 뒤 다시 비교 | 못 깨뜨림. 메뉴 아래 끝 572px, 모드 표시 위 끝 576.5px(창 높이 600). 크기 변경 뒤에도 같은 간격으로 따라오고 메뉴는 열린 채 | `mode_menu_sits_above_indicator_and_follows_resize` |
| 8 | 창 높이 200·160·120에서 메뉴 경계(일회성 탐침) | 관찰. 높이 160 이하에서 메뉴(높이 146px)가 위 끝 0에 붙어 모드 표시를 덮고, 120에서는 아래가 창 밖으로 잘린다. 키보드 조작은 된다 | 없음(제안 3) |
| 9 | 메뉴를 열고 `track-list`, `transport`, `inspector`를 차례로 클릭 | 못 깨뜨림. 닫히고 포커스 복귀 | `mode_menu_click_on_track_list_closes_and_returns_focus` |
| 10 | `g p` 사용자 바인딩, `g` → 모드 표시 클릭 → `p`, 닫고 다시 `p` | 못 깨뜨림. 대기 중인 `g`가 메뉴로 넘어가지 않는다 | `pending_sequence_does_not_cross_into_mode_menu` |
| 11 | 배치 뷰·선택 있음에서 메뉴로 STEP 선택 | 못 깨뜨림. `(Arrangement, Step, 선택 있음)`, 포커스 복귀 | `mode_menu_pick_keeps_view_and_selection` |
| 12 | 모든 상태 × `LeaveAi`, `a` → `ai::Accept`/`ai::Dismiss` | 못 깨뜨림 | `leave_ai_only_leaves_ai_and_keeps_view_and_selection`, `ai_accept_and_dismiss_leave_ai_by_action` |
| 13 | `install` 두 번 뒤 `ctrl-tab`(`mode::Cycle`) 바인딩 수, 바탕 레이어의 `PopupMenu` 바인딩 수, 바탕 레이어에 우리 Action이 섞였는가 | 못 깨뜨림 | `install_twice_keeps_one_copy_of_each_layer` |
| 14 | vim 프리셋에서 `install` 두 번 뒤 `3` | 못 깨뜨림. `pending_count == Some(3)`(처리기가 두 번 달려도 한 번만 센다) | `install_twice_count_prefix_counts_once` |

기존 테스트 `mode_fsm`(7), `modes_ui`(8), `review_s2_t3`(13), `review_s2_t2_r2`(23)도 이 커밋에서 다시 돌려 모두 통과했다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 고스트 ⇒ Step | 없음 | `with_mode`가 모드를 바꿀 때 고스트를 지우고, `ShowGhost`는 Step 조건(`mode.rs:228`) |
| 창의 키 컨텍스트 = `context_path(mode_state())` | 없음 | `render`가 매번 경로로 감싼다 |
| 모드 메뉴 포커스의 디스패치 경로에 문서 컨텍스트가 없다(새 불변식) | 없음 | 시도 4·5·10 |
| 「노트 입력 모드 ⇒ 피아노롤」 | 해당 없음(지적 1 대응으로 삭제) | 이 삭제가 지적 1의 대상 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| P2 다섯 모드, 상태줄 상시 표시 | 충족 | `window.rs` `mode_indicator` |
| P2·5.5 `Esc` | 판정 보류(A13) | 프리셋 비움은 의도 |
| 5.1 Step 진입 `Enter`/`Ctrl+Tab`, vim `i` | 부분 충족 | 배치 뷰에서 `enter`는 클립 열기, vim `i`는 배치 뷰 그대로 Step(시도 2). 같은 진입 키의 결과가 다르다. 지적 1 |
| 5.2 Sequence 진입·종료 | 충족 | 리뷰 1 지적 1의 「출발한 뷰로 돌아오지 않음」은 고쳐졌다 |
| 5.5 전환 도식과 Esc 사슬 「모드 → 피아노롤 → 배치 뷰」 | 부분 충족 | 배치 뷰에서 들어간 모드는 `Esc`가 피아노롤을 거치지 않는다(시도 1). 사슬을 「모드는 피아노롤 안에 있다」로 읽을지는 REQ가 정하지 않았다. 지적 1 |
| 5.9 Controller 모드 「피아노롤 아래 레인을 화면 전체로」 | 미충족(상태 수준) | `mode.rs:257` `with_mode`가 배치 뷰를 유지해 `(Arrangement, Controller)`가 된다. 지적 1 |
| 5.10 `v` 전환, 배치 뷰 `Enter` | 충족 | |
| 5.11 모드 순환 「모드만 바뀐다」 | 충족 | `review_s2_t3::cycle_round_trip_restores_state`, `ctrl_tab_round_trip_restores_state` |
| 5.12 고스트만 지우고 Step에 머문다 | 충족 | `modes_ui::ghost_escape_under_policy_c_stays_in_step` |
| 5.15 가사 모드 「커서 위 노트부터」「피아노롤은 가사 입력 중인 노트를 강조」 | 미충족(상태 수준) | 배치 뷰에서 `w`(`default.json:30`)가 `(Arrangement, Lyric)`. 배치 뷰 커서는 노트 위에 있지 않다. 지적 1 |
| 7.6 「모드 전환」 행: 모드 표시 클릭 → 모드 메뉴 | 충족 | 리뷰 1 지적 2 고쳐짐(시도 4–11) |
| UI-5 상태줄 모드 표시 | 충족(이 Task 범위) | |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 스케줄러를 건드리지 않는다)
- 제품 코드의 `unwrap`/`expect`: `window.rs` `wrap_in_contexts`의 `split_last().expect(..)`. 1회차와 같은 이유로 정당하다. 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: 모드 메뉴 항목 처리기의 `let _ = this.update(..)`(창이 닫힌 뒤). 1회차와 같다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 1 지적 1을 「노트 입력 모드는 피아노롤로」 규칙 삭제로 고쳤는데, 삭제도 REQ가 정하지 않은 결정이다. 이제 모든 모드가 배치 뷰와 짝지어지고 기본 키로 닿는다: 배치 뷰에서 `w` → `(Arrangement, Lyric)`, `shift-c` → `(Arrangement, Controller)`, `ctrl-tab`·`cmd-r` → `(Arrangement, Step/Sequence)`, vim `i` → `(Arrangement, Step)`. (a) 5.15는 가사 모드를 「커서 위 노트부터」 시작하고 「피아노롤은 가사 입력 중인 노트를 강조」한다고 적고, 5.9는 Controller 모드를 「피아노롤 아래」 레인을 키운 상태로 적는다. 둘은 순환(5.11) 밖 모드라 「모드만 바뀐다」 근거도 닿지 않는다. 문장대로라면 이 두 모드는 피아노롤에 있어야 한다(부록 A1). (b) Step·Sequence는 5.11이 배치 뷰 순환을 허용한다고 읽을 수 있지만, 5.5 Esc 사슬 「모드 → 피아노롤 → 배치 뷰」와 5.1 「vim은 `i`도」(배치 뷰 `enter`는 클립 열기)와는 어긋난다(시도 1·2). 1회차가 요구한 대로 `docs/decisions/`에 결정 요청(「배치 뷰에서 모드에 들어가면 뷰는 어떻게 되는가: 모드별로」)을 쓰고, (a)는 REQ 문장대로 고치거나 결정 대기로 명시한다. 결정 전 임시 동작은 보고서와 단계 파일(S3·S6 이월)에 적는다 | `crates/keyscore-app/src/mode.rs:257`, `assets/keymaps/default.json:30-31`, `assets/keymaps/vim.json:35`, `crates/keyscore-app/tests/mode_fsm.rs` 전이 표 `Enter(Controller)`·`Enter(Lyric)` 행, `docs/reports/s2-t3/report.md` 「리뷰 1 대응」 |

리뷰 1 지적 2(모드 메뉴 격리)와 지적 3(`LeaveAi`), 제안 1(`install` 두 번)은 고쳐졌고 새 문제를 찾지 못했다(시도 4–14).

## 제안 (선택)

1. 계획(`docs/reports/s2-t3/plan.md`) 「공개 인터페이스 초안」의 `ModeEvent`에 `ModeEvent::LeaveAi`가 빠져 있다. 계획 문서를 실제 API와 맞춘다.
2. 모드를 바꿔도 선택이 남게 되어(지적 1 대응), 배치 뷰에서 클립을 고른 채 `ctrl-tab`으로 Step에 들어가면 A13 선택지 a(모든 `escape` → `mode::Back`)에서 첫 `Esc`가 보이지 않는 선택만 지운다(시도 3. 상태줄은 `STEP` 그대로). 5.5 사슬 순서(선택 → 모드)대로이긴 하지만 5.1·5.4의 「종료 `Esc`」 체감과 다르다. A13 결정 요청(`docs/decisions/2026-10-17-keymap-conflicts.md`)에 이 경우를 한 줄 덧붙여 결정자가 보게 하기를 권한다.
3. 모드 메뉴 위치가 창 높이에서 `STATUS_BAR_HEIGHT`(28px 근삿값)를 뺀 고정점이다(`window.rs:259-268`). 지금 테마에서는 맞지만(간격 4.5px), 창 높이 160px 이하에서는 메뉴가 모드 표시를 덮고 120px에서는 아래가 잘린다(시도 8). 창 최소 크기를 두거나, 모드 표시의 실제 경계(`on_prepaint` 등)로 위치를 잡으면 글꼴 크기 변경에도 안전하다.
4. 메뉴가 열린 채 `ctrl-tab`(컨텍스트 없는 전역 바인딩)으로 모드가 바뀌면 체크가 낡는다(보고서 「알려진 한계」에 있음). 메뉴 격리 뒤에도 전역 바인딩(`ctrl-tab`, `cmd-` 표준 키)은 메뉴 뒤에서 그대로 실행된다. 의도라면 계획의 「불변식」에 「메뉴가 열린 동안 전역 바인딩만 동작한다」로 적어 두면 S2-T4 팔레트 같은 다른 오버레이도 같은 규칙을 따르기 쉽다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1은 REQ에 없는 결정을 결정 요청 없이 내린 것(CLAUDE.md 「반드시 지킬 것」)이고, 그 결과 5.9·5.15 문장과 어긋나는 상태가 기본 키로 닿는다. 결정 요청을 쓰고 Lyric·Controller를 REQ 문장대로 맞추거나(또는 결정 대기로 명시하고 이월) 재리뷰한다. 메뉴 격리, `LeaveAi`, `install` 두 번은 다시 볼 필요가 없다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t3_r2`: 파일의 11개 통과, 부록 A1은 파일에 붙여 실행해 실패 확인 후 뺐다)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(단계 파일의 이월 두 줄은 리뷰 1 제안 2·3 대응)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`mode_fsm`·`modes_ui`의 테스트 이름이 `c4ece0b`와 같다. 전이 표의 기대값은 규칙 변경에 맞춰 바뀌었다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 실패하는 테스트

`crates/keyscore-app/tests/review_s2_t3_r2.rs`의 헬퍼(`start`, `open`, `press`, `event`, `state`)를 그대로 쓴다. 이 파일에 붙여 넣으면 실행된다. 기대값은 REQ 5.15·5.9 문장대로 읽은 것이고, 결정 요청의 결과가 다르면 그에 맞춰 바꾼다.

A1. 지적 1

~~~rust
#[gpui_kit::test]
fn lyric_and_controller_from_arrangement_open_in_piano_roll(cx: &mut TestAppContext) {
    // REQ 5.15 「커서 위 노트부터 시작」「피아노롤은 가사 입력 중인 노트를 강조」,
    // 5.9 Controller 모드 = 「피아노롤 아래」 레인을 화면 전체로 키운 상태.
    start(cx, "arr-lyric", None);
    let (handle, view) = open(cx);
    for (key, mode) in [("w", Mode::Lyric), ("shift-c", Mode::Controller)] {
        assert_eq!(state(cx, &view), ModeState::INITIAL);
        press(cx, handle, key);
        let s = state(cx, &view);
        assert_eq!(s.mode(), mode);
        assert_eq!(s.view(), View::PianoRoll, "{key}: 배치 뷰에서 {mode:?}");
        event(cx, handle, &view, ModeEvent::Back);
        event(cx, handle, &view, ModeEvent::Back);
    }
}
// 실패: w: 배치 뷰에서 Lyric
//       left: Arrangement, right: PianoRoll
~~~
