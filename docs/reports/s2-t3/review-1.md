# 적대적 리뷰: S2-T3 모드 FSM과 Enter/Esc/Tab 규약 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트)
- 대상 커밋: `c4ece0b`(브랜치 `s2/t3-modes`, 범위 `git diff main...HEAD`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`mode.rs`, `window.rs`, `keymap/mod.rs` 변경분과 프리셋만 읽고 적었다. 다만 `tests/modes_ui.rs`는 헬퍼(창 열기, 클릭)를 빌리려고 이 목록을 다 적기 전에 훑어보았다. `tests/mode_fsm.rs`는 이 목록을 적은 뒤에 열었다.

1. `Ctrl+Tab` 한 바퀴(4번)가 시작 상태로 돌아오는가. 5.11은 「모드만 바뀐다」인데 `with_mode`가 뷰를 바꾸고 선택을 비운다.
2. 모드 메뉴가 열린 채 문서 키(`v`, `a`, `x`, `space`, 숫자)가 메뉴 뒤 컨텍스트로 새는가. 메뉴는 가장 안쪽 포커스 요소의 자식으로 그려진다(`deferred`는 디스패치 트리의 부모를 유지한다).
3. 메뉴 두 번 열기, 바깥 클릭으로 닫기, 표시 두 번 클릭에서 포커스가 창으로 돌아오는가.
4. 메뉴가 열린 채 컨텍스트 깊이가 바뀌어도 메뉴가 계속 동작하는가.
5. 대기 중인 멀티키 시퀀스가 컨텍스트 깊이 변화(2 → 3)를 지나서도 맞는가.
6. `Normal > Selection` 술어(프리셋에 A13 대기로 비어 있음)가 경로 `Normal > Arrangement > Selection`에서 맞는가.
7. 바탕 레이어가 프리셋 전환·핫리로드 뒤에도 남는가. `install`이 두 번 불리면 이전 사용자 레이어가 바탕 레이어로 얼어붙는가.
8. 바탕 레이어의 `Root` 컨텍스트 바인딩(`tab`, `cmd-c`)이 우리 레이어의 컨텍스트 없는 바인딩보다 깊어서 이기는가. 처리기가 없는 우리 Action(`track::Next`)이 바탕 레이어로 떨어지는가.
9. `ai::Accept`·`ai::Dismiss`가 AI가 아닌 모드에서 실행되면 무엇이 되는가.
10. 처리기 누락: `mode::*` 14개, `view::Toggle`, `clip::Open`, `select::Clear`, `ai::OpenPanel`·`Accept`·`Dismiss`·`DismissGhost`.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. `Ctrl+Tab` 한 바퀴의 복귀 | 부분 | `mode_fsm::cycle_visits_normal_step_sequence_play`가 모드 이름의 순서만 본다. 상태(뷰·선택)가 돌아오는지는 보지 않는다 |
| 2. 메뉴가 열린 채 문서 키 | 없음 | |
| 3. 메뉴 닫힘과 포커스 복귀 | 부분 | `status_bar_shows_mode_and_menu_switches_mode`가 `escape`·항목 선택으로 닫히는 것만 본다. 포커스 복귀, 두 번 열기, 바깥 클릭은 없다 |
| 4. 메뉴가 열린 채 깊이 변화 | 없음 | |
| 5. 대기 시퀀스와 깊이 변화 | 없음 | |
| 6. `Normal > Selection` 술어 | 없음 | 정책 a·c UI 테스트는 `Normal`·`Step` 컨텍스트에만 건다 |
| 7. 바탕 레이어 유지·`install` 두 번 | 부분 | `component_bindings_survive_keymap_apply`가 `apply` 한 번 뒤만 본다 |
| 8. `Root` 바인딩과 우선순위 | 부분 | `unbound_tab_keeps_window_focus`가 Lyric·Sequence·AI에서 포커스 유지만 본다 |
| 9. AI 밖의 `ai::Accept`·`Dismiss` | 없음 | 전이 표가 「어느 모드에서든 Normal로」를 의도로 적었다 |
| 10. 처리기 누락 | 있음(간접) | 코드로 대조했다. 누락 없음 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 모든 Normal 상태에서 `Cycle` 4번, `CycleBack` 4번 | 깨짐. 배치 뷰(선택 있음) → 피아노롤(선택 없음) | 부록 A1(`cycle_round_trip_restores_state`) |
| 2 | 배치 뷰·선택 있음에서 `ctrl-tab` 4번(기본 프리셋, UI) | 깨짐. 같은 결과 | 부록 A2(`ctrl_tab_round_trip_restores_state`) |
| 3 | 상태줄 모드 메뉴를 열고 `v`, `a`를 누른다 | 깨짐. 메뉴 뒤에서 뷰가 바뀌고 AI 모드로 간다. 메뉴는 열린 채 남는다 | 부록 A3(`mode_menu_open_keeps_document_keys_out`) |
| 4 | Play에서 `ai::Dismiss`, `ai::Accept` 디스패치 | 깨짐. Play를 떠나 Normal | 부록 A4(`ai_accept_dismiss_outside_ai_keep_mode`) |
| 5 | 사용자 파일이 있는 상태로 `keymap::install`을 한 번 더 부르고, 파일을 지운 뒤 `apply` | 깨짐. 지운 사용자 바인딩(`f9`)이 바탕 레이어로 남는다 | 부록 A5(제안 근거, `install_twice_does_not_freeze_user_layer`) |
| 6 | 메뉴 열기 → 바깥(`main-view`) 클릭 | 못 깨뜨림. 닫히고 포커스가 창으로 돌아온다 | `review_s2_t3::mode_menu_outside_click_closes_and_returns_focus` |
| 7 | `mode::ShowMenu` 두 번 → `escape` | 못 깨뜨림 | `mode_menu_opened_twice_closes_with_one_escape` |
| 8 | 모드 표시 두 번 클릭 → `escape` | 못 깨뜨림 | `mode_indicator_clicked_twice_leaves_window_usable` |
| 9 | 메뉴가 열린 채 `ctrl-tab`(깊이 2 → 1) → `down` `enter` | 못 깨뜨림. 메뉴가 동작하고 포커스가 돌아온다 | `mode_menu_keeps_working_after_context_depth_change` |
| 10 | `g` 대기 중에 `OpenClip`·`Select`(깊이 2 → 3) → `p` | 못 깨뜨림. `g p`가 맞는다 | `pending_sequence_survives_context_depth_change` |
| 11 | `Normal > Selection`의 `escape`를 사용자 키맵으로 걸고 배치 뷰·피아노롤 선택에서 누른다 | 못 깨뜨림. GPUI의 `>`는 자손 술어라 중간에 뷰 컨텍스트가 있어도 맞는다 | `normal_selection_predicate_matches_through_view_context` |
| 12 | vim 프리셋 전환 + 사용자 파일 핫리로드 뒤 `PopupMenu` 바인딩 | 못 깨뜨림 | `base_layer_survives_preset_switch_and_hot_reload` |
| 13 | 컨텍스트 없는 사용자 `tab`이 `Root`의 `root::Tab`에 지는가(Lyric) | 못 깨뜨림. 사용자 바인딩이 이긴다 | `user_global_tab_beats_base_root_tab` |
| 14 | 기본 프리셋 Normal에서 `tab`의 실제 디스패치(키 관찰자) | 관찰. `track::Next`에 처리기가 없어 `root::Tab`이 처리한다. 지금은 다른 탭 정지점이 없어 포커스가 안 움직인다 | 없음(제안 2. 지금 동작을 굳히는 테스트라 남기지 않았다) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 고스트 ⇒ Step | 없음 | 필드 비공개, `apply`의 모든 갈래가 `with_mode`(고스트 지움) 또는 `ShowGhost`의 Step 조건을 거친다 |
| 노트 입력 모드 ⇒ 피아노롤 | 없음 | `with_mode`가 강제, `ToggleView`는 Normal에서만 |
| 창의 키 컨텍스트 = `context_path(mode_state())` | 없음 | `render`가 매번 경로로 감싼다 |
| 모델 불변식(`core`) | 해당 없음 | 이 Task는 `core`를 건드리지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| P2 다섯 모드, 상태줄 상시 표시 | 충족 | `crates/keyscore-app/src/window.rs:240`, `mode.rs` `Mode::MENU` |
| P2·5.5 `Esc` | 판정 보류(A13) | 프리셋 비움은 의도. FSM과 정책 a·c UI 테스트로 수렴 확인 |
| 5.1 Step 진입 `Enter`/`Ctrl+Tab` | 충족 | `default.json` `Normal > PianoRoll` `enter`, 전역 `ctrl-tab` |
| 5.2 Sequence 진입 `Cmd+R`, 종료 후 Normal | 부분 충족 | 배치 뷰에서 들어가면 피아노롤의 Normal로 돌아온다(지적 1) |
| 5.3 피아노롤 `Enter` → Step, `v` | 충족 | `modes_ui::keys_drive_modes_through_key_contexts` |
| 5.4 Play 진입 `Cmd+K` | 충족 | 같은 테스트 |
| 5.5 전환 도식 | 부분 충족 | 도식의 화살표는 맞다. `Normal ──Cmd+R──▶ Sequence ──Esc──▶ Normal`이 출발한 뷰로 돌아오지 않는다(지적 1) |
| 5.10 `v` 전환, 배치 뷰 `Enter` | 충족 | `mode.rs` `OpenClip`·`ToggleView` |
| 5.11 모드 순환 「현재 커서 위치를 유지한 채 모드만 바뀌므로」 | 미충족 | `crates/keyscore-app/src/mode.rs:256-268` `with_mode`가 뷰를 바꾸고 `with_view`(`mode.rs:243`)가 선택을 비운다. 부록 A1·A2 |
| 5.12 고스트만 지우고 Step에 머문다 | 충족 | `modes_ui::ghost_escape_under_policy_c_stays_in_step` |
| 7.6 「모드 전환」 행: 모드 표시 클릭 → 모드 메뉴 | 부분 충족 | 메뉴는 열리고 고를 수 있다. 열린 동안 문서 키가 메뉴 뒤로 샌다(지적 2) |
| UI-5 상태줄 모드 표시 | 충족(이 Task 범위) | 길이·옥타브는 S3 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 스케줄러를 건드리지 않는다)
- 제품 코드의 `unwrap`/`expect`: `crates/keyscore-app/src/window.rs:222` `split_last().expect("컨텍스트 경로는 비지 않는다")`. `context_path`가 `&'static` 리터럴만 돌려주고 모든 갈래가 길이 1 이상이라 정당하다.
- 에러가 삼켜지는 곳: `window.rs` 모드 메뉴 항목 처리기의 `let _ = this.update(...)`. 창이 닫힌 뒤 메뉴 항목이 눌리는 경우뿐이라 정당하다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | `Ctrl+Tab` 순환이 모드만 바꾸지 않는다. 배치 뷰(선택 있음)에서 `ctrl-tab` 4번이면 피아노롤·선택 없음으로 끝난다. 5.11 「현재 커서 위치를 유지한 채 모드만 바뀌므로」와 어긋난다. 같은 규칙 때문에 배치 뷰에서 `Cmd+R`(Sequence) → 나가기가 출발한 배치 뷰가 아니라 피아노롤로 돌아온다(5.5 `Normal ──Cmd+R──▶ Sequence ──Esc──▶ Normal`). 원인은 계획의 「노트 입력 모드는 피아노롤로」 규칙이고, 계획도 이 규칙을 REQ가 직접 적지 않은 세부라고 적었다. REQ에 없는 결정이라 `docs/decisions/`에 결정 요청을 먼저 써야 했다(CLAUDE.md 「반드시 지킬 것」). 고칠 방향(뷰를 건드리지 않기, 진입 전 뷰를 기억했다가 돌려주기 등)은 결정 요청에서 정한다. 재현: 부록 A1, A2 | `crates/keyscore-app/src/mode.rs:256`, `crates/keyscore-app/src/mode.rs:199`, `docs/reports/s2-t3/plan.md` 「전이 표 > 규칙」 |
| 2 | 수정 | 상태줄 모드 메뉴가 열린 채 누른 키가 메뉴 뒤 문서 컨텍스트로 간다. 메뉴(`deferred`)가 가장 안쪽 포커스 요소 안에 그려져 디스패치 경로가 `… Normal > Arrangement > PopupMenu`가 되고, `PopupMenu`가 걸지 않은 키는 모두 `Normal` 바인딩으로 떨어진다. 메뉴를 연 채 `v`는 뷰를 바꾸고 `a`는 AI 모드로 가며 메뉴는 열린 채(체크는 낡은 채) 남는다. 이후 단계에서 처리기가 붙으면 `x`(`edit::Delete`), `space`(`transport::Toggle`)도 메뉴 뒤에서 실행된다. 7.6 「모드 전환」 메뉴의 동작으로 맞지 않다. 재현: 부록 A3 | `crates/keyscore-app/src/window.rs:240-256`, `crates/keyscore-app/src/window.rs:167`, `crates/keyscore-app/src/window.rs:437` |
| 3 | 수정 | `ai::Accept`·`ai::Dismiss`가 AI가 아닌 모드에서도 `ToNormal`이다. Play에서 팔레트(S2-T4)로 「AI 닫기」를 실행하면 Play를 떠난다. 5.5의 이 두 전이는 「AI 패널 → Normal」뿐이다. 보고서 「알려진 한계」는 S7이 수락·버림을 붙인다고만 적었고 AI 밖에서의 동작은 다루지 않았다. AI 모드에서만 전이하도록 하거나(사건을 나누거나 처리기에서 모드를 본다) 전이 표에 「AI 밖에서는 그대로」를 넣는다. 재현: 부록 A4 | `crates/keyscore-app/src/window.rs:211-212` |

## 제안 (선택)

1. `keymap::install`이 그때의 모든 바인딩을 바탕 레이어로 뜬다(`crates/keyscore-app/src/keymap/mod.rs:269`). 두 번 불리면 첫 설치의 표준·프리셋·사용자 레이어가 바탕 레이어로 얼어붙어, 사용자가 지운 바인딩이 영영 남는다(부록 A5). 지금 앱은 `init`에서 한 번만 부르지만 `install`은 `pub`이다. 두 번째 호출이면 기존 `base`를 유지하거나, 바탕 레이어를 「gpui-component `init`이 건 것」으로 명시해 뜨는 편이 안전하다.
2. 처리기가 없는 우리 Action은 바탕 레이어로 떨어진다(GPUI는 처리되지 않은 바인딩 다음 후보를 디스패치한다). 기본 프리셋 Normal의 `tab`(`track::Next`, `default.json:53`)이 지금 `root::Tab`(포커스 이동)으로 처리된다(시도 14). 지금은 다른 탭 정지점이 없어 무해하지만 S2-T5가 인스펙터 입력칸 같은 탭 정지점을 들이면, `track::Next` 처리기가 붙기 전까지 Normal의 `Tab`이 창 포커스를 입력칸으로 옮겨 모든 모드 키가 끊긴다. S2-T5 이월 항목에 적어 두기를 권한다.
3. 바탕 레이어에 gpui-component의 디버그 전역 바인딩 `alt-cmd-i`(`inspector::ToggleInspector`, `debug_assertions`에서만)가 들어간다. 키맵 편집기(S2-T6)가 `base_bindings`를 보여 줄 때 이 키를 빼거나 「개발용」으로 표시할지 정해 두면 좋다.
4. `mode_fsm::cycle_visits_normal_step_sequence_play`의 「한 바퀴」 검사는 모드 이름만 본다. 지적 1을 고칠 때 상태 전체 비교로 바꾸면 회귀를 막는다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1은 REQ 5.11 미충족이고 REQ에 없는 결정을 결정 요청 없이 내린 것이라 「차단」이다. 결정 요청을 쓰고(이 Task를 보류할지는 구현자 판단), 결정에 따라 FSM을 고친 뒤 지적 2·3과 함께 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(`modes_ui.rs`는 헬퍼를 빌리려고 먼저 훑었다. 1절 머리에 적었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t3`, 부록의 5개가 실패, 파일의 8개가 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`review_s2_t2_r2.rs` 변경은 바탕 레이어 도입에 따른 기대 모델 보완으로 보고서에 이유가 있다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 실패하는 테스트

`crates/keyscore-app/tests/review_s2_t3.rs`의 헬퍼(`start`, `open`, `press`, `event`, `click`, `menu_open`, `state`)를 그대로 쓴다. 이 파일에 붙여 넣으면 실행된다. A1–A4는 지적, A5는 제안 1의 근거다. 고친 뒤에는 이 테스트를 파일로 옮겨 남긴다(01-protocol [7]).

A1. 지적 1(FSM)

~~~rust
#[test]
fn cycle_round_trip_restores_state() {
    for start in ModeState::all().filter(|s| s.mode() == Mode::Normal) {
        let mut s = start;
        for _ in 0..4 {
            s = s.apply(ModeEvent::Cycle);
        }
        assert_eq!(s, start, "Ctrl+Tab 한 바퀴");
        for _ in 0..4 {
            s = s.apply(ModeEvent::CycleBack);
        }
        assert_eq!(s, start, "Ctrl+Shift+Tab 한 바퀴");
    }
}
// 실패: left: ModeState { view: PianoRoll, mode: Normal, selection: false, .. }
//       right: ModeState { view: Arrangement, mode: Normal, selection: false, .. }
~~~

A2. 지적 1(기본 프리셋 키)

~~~rust
#[gpui_kit::test]
fn ctrl_tab_round_trip_restores_state(cx: &mut TestAppContext) {
    start(cx, "cycle-ui", None);
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::Select);
    let before = state(cx, &view);
    for _ in 0..4 {
        press(cx, handle, "ctrl-tab");
    }
    assert_eq!(state(cx, &view), before);
}
// 실패: left: { view: PianoRoll, mode: Normal, selection: false }
//       right: { view: Arrangement, mode: Normal, selection: true }
~~~

A3. 지적 2

~~~rust
#[gpui_kit::test]
fn mode_menu_open_keeps_document_keys_out(cx: &mut TestAppContext) {
    start(cx, "menu-keys", None);
    let (handle, view) = open(cx);
    click(cx, handle, "status-mode");
    assert!(menu_open(cx, &view));
    press(cx, handle, "v");
    assert_eq!(state(cx, &view), ModeState::INITIAL, "메뉴가 열린 채 v가 뷰를 바꿨다");
    press(cx, handle, "a");
    assert_eq!(state(cx, &view).mode(), Mode::Normal, "메뉴가 열린 채 a가 AI로 갔다");
    assert!(menu_open(cx, &view));
}
// 실패: 메뉴가 열린 채 v가 뷰를 바꿨다
//       left: { view: PianoRoll, .. }  right: { view: Arrangement, .. }
// (v 줄을 빼면 a 줄에서 left: Ai, right: Normal)
~~~

A4. 지적 3

~~~rust
#[gpui_kit::test]
fn ai_accept_dismiss_outside_ai_keep_mode(cx: &mut TestAppContext) {
    use keyscore_app::actions::ai;
    start(cx, "ai-outside", None);
    let (handle, view) = open(cx);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Play));
    for action in [
        Box::new(ai::Dismiss) as Box<dyn gpui_kit::Action>,
        Box::new(ai::Accept),
    ] {
        cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
            .unwrap();
        render(cx, handle);
        assert_eq!(state(cx, &view).mode(), Mode::Play);
    }
}
// 실패: left: Normal, right: Play
~~~

A5. 제안 1

~~~rust
#[gpui_kit::test]
fn install_twice_does_not_freeze_user_layer(cx: &mut TestAppContext) {
    let dir = start(cx, "install-twice", Some(r#"[{"bindings": {"f9": "mode::Play"}}]"#));
    let path = dir.join("keymap.json");
    cx.update(|cx| keymap::install(cx, path.clone(), keymap::options(cx), |_| {}));
    std::fs::remove_file(&path).unwrap();
    cx.update(keymap::apply);
    cx.update(|cx| {
        let km = cx.key_bindings();
        let km = km.borrow();
        assert!(
            !km.bindings().any(|b| b.action().name() == "mode::Play"
                && b.keystrokes()[0].unparse() == "f9"),
            "지운 사용자 바인딩이 남았다"
        );
    });
}
// 실패: 지운 사용자 바인딩이 남았다
~~~
