# 적대적 리뷰: S2-T3 모드 FSM과 Enter/Esc/Tab 규약 (리뷰 3회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트). 1·2회차 리뷰어와도 다른 세션이다.
- 대상 커밋: `4925281`(브랜치 `s2/t3-modes`, 범위 `git diff main...HEAD`. 리뷰 2 대응과 A13 반영은 `git show 5965ba4`(결정·REQ·단계 이월), `git show 4925281`(코드))
- 프로토콜상 마지막 재리뷰(3회차)다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show 4925281`의 `mode.rs`, `window.rs`, `actions.rs`, `default.json` 변경분, `vim.json`, 결정 문서 A13 해석 표와 `2026-10-17-mode-entry-view.md`, `git show 5965ba4 -- docs/REQ.md`만 읽고 적었다. 테스트는 이 목록을 적은 뒤에 열었다.

1. A13 해석 표의 S2-T3 행(`Step > Ghost`, 입력 모드 다섯, `AI`, `Normal > Selection`, 선택 없는 Normal)이 기본 프리셋에 그대로 있는가. 28개 상태 모두에서 실제 `escape` 한 번의 효과가 표와 같은가.
2. `Normal > Selection` 술어가 실제 경로 `Normal > Arrangement > Selection`·`Normal > PianoRoll > Selection`에 맞는가(GPUI `>`가 자식인지 자손인지).
3. 연타 수렴: 세 번 안에 Normal·선택 없음·고스트 없음, 그 뒤 No-op. 뷰와 레이아웃(패널 열림)이 바뀌지 않는가.
4. vim 오버레이가 `escape`를 `null`로 가리거나 다른 Action으로 바꾸는 곳이 있는가. vim 카운트(`3`) 뒤의 `Esc`.
5. 모드 메뉴(팝업)가 열려 있을 때 `Esc`는 메뉴만 닫는가(모든 상태에서). 그 다음 `Esc`는 표대로인가.
6. 창의 다른 영역(트랙 목록·트랜스포트·인스펙터)을 누른 뒤에도 `Esc`가 입력 모드를 나가는가(포커스가 문서 밖으로 새면 입력 모드에 갇힌다).
7. 모드 진입 뷰: Lyric·Controller는 배치 뷰에서 피아노롤로, 선택을 비운다. 그 밖은 뷰·선택 유지. 어떤 사건 조합으로도 `(Arrangement, Lyric|Controller)`가 생기지 않는가. vim `i`.
8. `mode::Back` 제거의 파급: `ALL_ACTION_NAMES`, 카탈로그, 키맵 파일, 옛 사용자 키맵.
9. REQ 수정 문구(P2, 5.2, 5.3, 5.5, 5.6, 5.8, 5.10, UI-2)가 결정 해석 표와 같은가. 다른 절(5.1, 5.4, 5.11, 5.12, 5.15, 7.3 예, 7.6)과 새로 어긋나는가.
10. 해석 표가 사람의 원문(「패널이 열리면 닫힘」「결정/선택 동작에서 작업을 취소」「그 외 No-OP」)을 넘어서거나 반대로 읽은 행이 있는가. 그 행이 REQ 의미로 들어갔는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 28개 상태 × `escape` 한 번 | 부분 | `mode_fsm::escape_cancels_innermost_thing_once`는 FSM 수준 표 10행(키 아님). `modes_ui::escape_steps_ghost_then_mode_then_selection`은 한 경로 |
| 2. `Normal > Selection` 술어 | 간접 | 위 UI 테스트의 세 번째 Esc가 피아노롤 경로에서 맞음을 보인다. 배치 뷰 경로는 없음 |
| 3. 연타 수렴·뷰 불변 | 있음 | `modes_ui::escape_mashing_converges_with_default_preset`·`_vim_preset`(시작 10개). 레이아웃 불변은 없음 |
| 4. vim 오버레이와 escape | 부분 | 위 vim 연타 테스트. 바인딩 집합 비교와 카운트는 없음 |
| 5. 메뉴가 열린 채 Esc | 부분 | `review_s2_t3_r2::mode_menu_open_keeps_mode_keys_out_in_every_mode`가 네 모드에서 「닫힌다」만 본다. 상태 불변·다음 Esc는 없음 |
| 6. 다른 영역 클릭 뒤 Esc | 없음 | |
| 7. 진입 뷰 | 부분 | `review_s2_t3_r2::lyric_and_controller_from_arrangement_open_in_piano_roll`(`w`, `shift-c`), `mode_fsm` 불변식. Step·Sequence·Play·AI 실제 키와 선택 유지는 없음 |
| 8. `mode::Back` 제거 | 부분 | 빌드가 증명. 옛 사용자 키맵은 없음 |
| 9·10. REQ 문구와 해석 | 해당 없음 | 문서 대조 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 기본·vim 프리셋 각각, 28개 상태(`ModeState::all`)에 사건으로 들어가 실제 `escape` 한 번 → A13 표와 비교 | 못 깨뜨림 | `escape_once_matches_a13_table_in_every_state_default`, `_vim` |
| 2 | 같은 28개 상태에서 `escape` 여섯 번: 뷰 그대로·Normal·선택 없음·고스트 없음, 창 포커스 유지, `LayoutSettings` 불변 | 못 깨뜨림 | 위 두 테스트 |
| 3 | 기본과 vim에서 키맵 전체의 `escape` 바인딩 (컨텍스트, Action) 집합 비교. 선택 없는 Normal 계열 컨텍스트에 `escape`가 있는가 | 못 깨뜨림. 두 집합이 같고 Normal·`Normal > Arrangement`·`Normal > PianoRoll`에는 없다 | `vim_layer_does_not_change_any_escape_binding` |
| 4 | 28개 상태 각각에서 모드 표시 클릭으로 메뉴를 열고 `escape` → 메뉴만 닫히고 상태 불변, 포커스 복귀, 다음 `escape`는 표대로 | 못 깨뜨림 | `escape_with_mode_menu_open_closes_only_the_menu` |
| 5 | vim, 선택 있음에서 `3` `escape`, 선택 없음에서 `3` `escape` | 못 깨뜨림. 카운트가 비고 한 가지 일만 한다 | `vim_count_then_escape_clears_count_and_does_one_thing` |
| 6 | `ctrl-tab`으로 Step → `track-list`·`transport`·`inspector`·`main-view` 클릭 → `escape` | 못 깨뜨림. Normal로 나간다 | `escape_leaves_input_mode_after_clicking_other_areas` |
| 7 | 배치 뷰·선택 있음에서 `ctrl-tab` `cmd-r` `cmd-k` `a` `w` `shift-c` → 상태, 이어서 `escape` → 뷰 유지. vim `i` | 못 깨뜨림. 결정 문서대로 | `entry_view_rule_by_real_keys` |
| 8 | 28개 상태 × 사건 두 개의 모든 조합에서 Lyric·Controller가 배치 뷰에 있는가 | 못 깨뜨림 | `lyric_and_controller_never_leave_piano_roll` |
| 9 | 옛 사용자 키맵 `{"escape": "mode::Back", "ctrl-x": "mode::Play"}` | 못 깨뜨림. 오류 1건, 같은 파일의 `ctrl-x`는 살아 있고 선택 없는 Normal의 Esc는 No-op | `mode_back_is_gone_and_old_user_keymap_reports_one_error` |
| 10 | 기본 레이아웃(인스펙터 열림), Normal·선택 없음에서 `escape` | 깨짐(사람 원문 「패널이 열리면 닫힘」 기준). 인스펙터가 그대로 열려 있다. 해석 표는 「닫지 않는다」로 읽었다 | 부록 A1(지적 2) |
| 11 | REQ 5.5 도식 아래 「가장 안쪽 것 하나만」 순서를 A13 해석 표의 컨텍스트 깊이와 대조 | 깨짐(문서). AI 요청 중 `Esc`에서 REQ는 패널을 먼저 닫고, 표는 요청만 취소하고 패널을 남긴다 | 없음(문서 결함, 지적 1) |
| 12 | REQ 5.6 새 문장 「모드를 나가면(`Esc`) 조합 중인 입력은 확정」을 사람 원문·해석 표의 다른 대기 입력 행과 대조 | 깨짐(문서). 같은 표가 vim 오퍼레이터 대기·카운트는 취소로 읽는다 | 없음(S3 동작, 지적 2) |

이 커밋에서 기존 테스트 `mode_fsm`(7), `modes_ui`(8), `review_s2_t3`(13), `review_s2_t3_r2`(12), `review_s2_t2_r2`(23), `keymap_engine`(11)을 다시 돌려 모두 통과했다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 고스트 ⇒ Step | 없음 | `with_mode`가 고스트를 지우고 `ShowGhost`는 Step 조건(`mode.rs:232`) |
| Lyric·Controller ⇒ 피아노롤(새 불변식) | 없음 | `with_mode`가 들어갈 때 뷰를 옮기고(`mode.rs:265`), `ToggleView`·`OpenClip`은 Normal에서만(`mode.rs:208-214`). 시도 8 |
| 창의 키 컨텍스트 = `context_path(mode_state())` | 없음 | `render`가 매번 경로로 감싼다 |
| `Esc` 한 번 = 가장 안쪽 것 하나 | 없음(이 Task 범위의 컨텍스트) | 시도 1·4·5 |
| `Esc`는 뷰를 바꾸지 않는다 | 없음 | 시도 2·7 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| P2 다섯 모드, 상태줄 표시 | 충족 | 이전 회차와 같다 |
| P2 `Esc` 통상의 취소(수정 문구) | 충족(이 Task의 컨텍스트). 「AI 패널만」 한정은 지적 2 | `assets/keymaps/default.json:196,357,406,469,479,524,531,538`. 시도 1–4 |
| 5.1 Step 진입 `Enter`/`Ctrl+Tab`, vim `i`, 종료 `Esc` | 충족 | 결정 `mode-entry-view`의 근거(5.1 「클립 밖이면 새 클립」, 5.10 `docs/REQ.md:393`, 5.11 `:436`·`:438`)를 REQ에서 확인했다. 시도 7 |
| 5.3 `Esc` 선택 해제, 뷰는 `v` | 충족 | 시도 1·2 |
| 5.4 Play 종료 `Esc` | 충족 | 시도 1 |
| 5.5 도식과 `Esc` 우선순위 줄 | 부분 충족 | 도식은 맞다. 우선순위 줄의 「팝업·패널 → 진행 중 조작」 순서가 결정 해석 표와 반대(`docs/REQ.md:179-180`). 지적 1 |
| 5.6 조합 중 입력의 확정 시점(새 문장) | 판정 보류(S3 동작) | `docs/REQ.md:195`. 사람 확인 없는 의미 추가. 지적 2 |
| 5.9 Controller 모드 「피아노롤 아래 레인」, 종료 `Esc` | 충족 | 리뷰 2 지적 1 해결. 시도 7·8 |
| 5.10 `v` 전환, `Enter`로 들어가기 | 충족 | |
| 5.11 순환 「모드만 바뀐다」 | 충족 | 문장 끝 「`Esc`는 여전히 어디서든 Normal로」(`docs/REQ.md:436`)는 제안 3 |
| 5.12 고스트만 지우고 Step | 충족 | 시도 1(`Step > Ghost`) |
| 5.15 가사 「커서 위 노트부터」「피아노롤이 강조」, `Esc`는 Normal로 | 충족(상태 수준) | 리뷰 2 지적 1 해결 |
| 7.3 예 `"escape": "mode::Normal"`(Step) | 충족 | `default.json:196` |
| 7.6 「모드 전환」 행 | 충족 | 시도 4 |
| UI-12 인스펙터 「패널」 | 지적 2 참조 | 사람 원문 「패널이 열리면 닫힘」과 REQ P2 새 문구의 관계 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 스케줄러를 건드리지 않는다)
- 제품 코드의 `unwrap`/`expect`: `window.rs` `wrap_in_contexts`의 `split_last().expect(..)`. 이전 회차와 같은 이유로 정당하다. 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: 이전 회차와 같다(모드 메뉴 항목 처리기의 `let _ = this.update(..)`). 옛 사용자 키맵의 `mode::Back`은 오류로 보고된다(시도 9).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 이 브랜치가 새로 쓴 REQ 5.5 우선순위 줄 「Esc는 가장 안쪽 것 하나만 닫는다: 팝업·패널 → 진행 중 조작(드래그·캡처·AI 요청) → AI 고스트 → 입력 모드 → 선택」이 A13 해석 표와 어긋난다. 표는 `AI > Requesting`(요청만 취소, 패널 유지)을 `AI`(패널 닫기)보다 깊게, `Keymap > Editing`(캡처 취소)·`Dragging`을 그보다 깊게 둔다. REQ 순서대로면 AI 요청 중 `Esc`는 패널을 닫고(AI-10 「취소 `Esc`」와도 다르다), 설정 화면의 키 캡처 중 `Esc`는 화면부터 닫는다. S7·S2-T6 구현자가 REQ와 결정 문서에서 서로 다른 답을 읽게 된다. 표의 깊이 순서대로 「팝업·팔레트·대화상자 → 진행 중 조작(드래그·캡처·AI 요청) → AI 패널 → AI 고스트 → 입력 모드 → 선택」처럼 고친다 | `docs/REQ.md:179-180`, `docs/decisions/2026-10-17-keymap-conflicts.md:133-143` |
| 2 | 수정 | A13 해석 표의 두 행이 사람 원문과 다르게 읽히는데, 사람 확인 없이 REQ 의미로 들어갔다. 개발 단계 문구 승인(`2026-10-17-req-wording-approval.md`)은 「의미를 새로 정하거나 바꾸는 판단」을 빼고 있다. (a) 조합 중 입력 `5c_`: 원문은 「결정/선택 동작에서 작업을 취소」인데, 해석은 `Esc`가 조합을 확정하고 모드까지 나간다(한 번에 두 일, 지적 1의 「하나만」과도 다르다). 같은 표가 vim 오퍼레이터 대기·카운트는 「취소」로 읽어(`:145`) 대기 입력끼리 반대다. 이 뜻이 REQ 5.6 새 문장과 S3 이월로 굳었다. (b) 열린 레이아웃 패널: 원문은 「패널이 열리면 닫힘」인데, 해석은 인스펙터·트랙 목록(REQ UI-12·7.1이 「패널」이라 부른다)을 닫지 않고, REQ P2는 닫는 대상을 「AI 패널」로 한정한 뒤 「그 밖에는 아무 일도 하지 않는다」로 맺는다. 지금 코드에서 기본 레이아웃(인스펙터 열림)의 Normal `Esc`는 No-op다(부록 A1, 원문 기준으로 실패). 두 행을 A13 보충 결정 요청으로 사람에게 묻고, 답이 올 때까지 REQ 5.6 새 문장과 P2의 「AI 패널」 한정을 결정 대기로 표시(또는 되돌림)한다. 코드는 (b)의 답이 「닫는다」일 때만 바뀐다 | `docs/REQ.md:100`, `docs/REQ.md:195`, `docs/decisions/2026-10-17-keymap-conflicts.md:146,148`, `plan/stages/12-S2-app-shell.md:56`, `plan/stages/13-S3-step-and-pianoroll.md:36` |

리뷰 2 지적 1(모드 진입 뷰)은 고쳐졌다. 결정 문서의 근거 문장(5.1, 5.9, 5.10 `docs/REQ.md:393`, 5.11 `:436`·`:438`, 5.15 `:631`)이 REQ에 그대로 있고, 실제 키로도 문서대로 움직인다(시도 7·8). A13의 S2-T3 행은 기본·vim 프리셋과 FSM에 표대로 반영됐고, 28개 상태·메뉴·카운트·포커스 이탈에서 깨뜨리지 못했다(시도 1–6). `mode::Back` 제거의 파급도 문제를 찾지 못했다(시도 9, 아래 제안 4).

## 제안 (선택)

1. 단계 파일 S2-T3 절의 범위·게이트 문구(「`Esc` 한 단계씩 나가기(고스트→선택→모드→피아노롤→배치 뷰)」, 「배치 뷰·선택 없음으로 수렴」)가 A13 이전 그대로다. `plan.md`가 재해석을 적었지만 단계 출구 검토는 단계 파일을 기준으로 하므로, 다른 단계 파일처럼 A13 반영 한 줄을 붙인다(`plan/stages/12-S2-app-shell.md:24-25`).
2. `plan.md`가 실제와 어긋난다. 전이 표 `Enter(m)` 행에 「Lyric·Controller는 피아노롤로, 선택을 비움」이 없고(`docs/reports/s2-t3/plan.md:45`), 불변식 목록에 「Lyric·Controller ⇒ 피아노롤」이 없으며(`:107-112`), 범위 밖·리스크에 「`escape` 프리셋 바인딩(A13 결정 뒤)」「A13 결정 대기」가 남아 있다(`:118`, `:123`).
3. REQ 5.11 끝 문장 「`Esc`는 여전히 어디서든 Normal로 돌아온다」(`docs/REQ.md:436`)는 P2를 「어느 입력 모드에서든」으로 고친 뒤에도 그대로다. 고스트(5.12)·팝업이 열린 경우와 맞지 않으니 P2와 같은 문구로 맞춘다.
4. 카탈로그 통계(`docs/reports/s2-t2/action-catalog.md:486` mode 14, `:507` 합계 285)는 S2-T2 시점 값이다. 「S2-T3에서 `mode::Back` 삭제」를 한 줄 덧붙이면 표와 통계가 따로 놀지 않는다. 결정 문서 머리의 「상태: 결정 대기」(`docs/decisions/2026-10-17-keymap-conflicts.md:4`)도 「A13 결정됨, 나머지 대기」로 적는다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

코드와 키 동작은 다시 볼 필요가 없다. 남은 지적 두 건은 모두 이 브랜치가 고친 REQ·결정 문서의 결함이다. 지적 1은 문장 한 줄의 순서를 고치면 되고, 지적 2는 그 자체가 사람에게 묻는 일(결정 요청)이다. 이번이 프로토콜 [8]의 세 번째 재리뷰이므로 판정이 나지 않은 이 Task는 결정 요청으로 넘긴다. 결정 요청에는 지적 2의 두 질문을 싣고, 지적 1의 문장 수정은 그와 함께 처리한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t3_r3`: 파일의 9개 통과. 부록 A1은 파일에 붙여 실행해 실패를 확인한 뒤 뺐다)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(단계 파일 이월 줄은 결정·리뷰 대응)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`mode_fsm`의 `back_repeated_…`, `escape_policies_…`와 `modes_ui`의 선택지 a·c 테스트는 결정된 정책의 테스트로 고쳐 썼고 이유가 보고서에 있다. `mode::Back`이 없어져 옛 기대값은 성립하지 않는다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 실패하는 테스트

`crates/keyscore-app/tests/review_s2_t3_r3.rs`의 헬퍼(`start`, `open`, `press`)를 그대로 쓴다. 이 파일에 붙여 넣으면 실행된다. 기대값은 사람 원문 「패널이 열리면 닫힘」을 문자 그대로 읽은 것이다. 지적 2의 결정이 해석 표대로(레이아웃 패널은 닫지 않음) 나면 이 테스트는 버리고, 반대 기대값(인스펙터가 열린 채 남음)을 회귀 테스트로 둔다.

A1. 지적 2 (b)

~~~rust
#[gpui_kit::test]
fn escape_closes_open_inspector_panel_in_normal(cx: &mut TestAppContext) {
    start(cx, "panel-escape", None);
    let (handle, view) = open(cx);
    if !cx.read(|cx| view.read(cx).layout().inspector_open) {
        press(cx, handle, "cmd-i");
    }
    assert!(cx.read(|cx| view.read(cx).layout().inspector_open));
    press(cx, handle, "escape");
    assert!(
        !cx.read(|cx| view.read(cx).layout().inspector_open),
        "열린 인스펙터 패널이 Esc로 닫히지 않았다"
    );
}
// 실패: 열린 인스펙터 패널이 Esc로 닫히지 않았다
~~~

지적 1과 지적 2 (a)는 문서 결함이다. 대상 동작(AI 요청 취소 S7, 키 캡처 S2-T6, 조합 중 입력 S3)이 아직 없어 실행할 수 있는 테스트로 쓸 수 없다.
