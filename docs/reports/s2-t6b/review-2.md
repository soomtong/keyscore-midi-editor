# 적대적 리뷰: S2-T6b `?` 키맵 오버레이 (리뷰 2회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 에이전트 세션(구현자·리뷰 1 리뷰어와 다른 세션)
- 대상 커밋: `859e99c`(브랜치 `s2/t6b-keymap-overlay`, 수정 커밋 `899625f`, `git diff main...HEAD`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

리뷰 1 문서와 보고서의 「리뷰 1 대응」, `overlay.rs`·`window.rs`·`default.json` 변경만 읽고, 테스트 파일을 열기 전에 적었다.

1. 줄 캐시는 (KeymapVersion, 모드 경로 포인터)로 무효화한다(`overlay.rs:178`). `context_path()`는 match 갈래마다 승격된 `&'static` 슬라이스라 같은 경로면 같은 포인터, 다른 경로면 다른 (주소, 길이)다. 다만 테스트 API `overlay_lines`(`overlay.rs:149`)는 캐시를 거치지 않으므로 기존 테스트는 캐시가 낡는지 보지 못한다. 그린 줄(`overlay-row` 접근성 이름)과 머리말로 확인한다.
2. 같은 모드 안에서 경로만 바뀌는 경우(Selection 켬/끔, Step > Ghost 켬/끔), 키맵 다시 읽기(판만 오르고 내용 같음), 수식키 교환, 프리셋 교체가 그린 줄에 반영되는가.
3. 열린 채 `ctrl-tab`/`ctrl-shift-tab`과 메뉴·팔레트 경로의 모드 Action으로 모드가 바뀔 때 그린 줄·머리말이 따라가는가. 그 뒤 Esc는 오버레이만 닫는가.
4. 변형 묶기: 팔레트 줄을 찾지 못한 열린 인자 Action은 `format!("{action:?}")`를 묶음 키와 제목에 쓴다(`overlay.rs:112-113`). GPUI의 `impl Debug for dyn Action`이 인자를 찍는가. 찍지 않으면 인자가 다른 바인딩이 다시 한 줄로 뭉치고 디버그 표기가 화면에 샌다.
5. 배경(`keymap-overlay-backdrop`, `overlay.rs:281`)에 `occlude()`가 없다. GPUI `HitboxBehavior::Normal`이면 아래 요소의 hitbox도 hovered로 남아, 바깥 클릭이 닫기와 함께 아래 버튼(트랙 뮤트, 트랙 추가)을 누를 것이다.
6. Overlay 컨텍스트의 `?`로 열고 닫기를 되풀이, Step처럼 `?`가 걸리지 않은 모드에서의 `?`, 팝업↔오버레이↔팔레트 배타(양방향).
7. `Kbd` 렌더링: 실제로 `kbd:<키>` 요소가 그려지는가. `Kbd::format`이 `delete`와 `backspace`를 같은 기호로 보이는가.
8. 성능: 캐시가 실제로 듣는가(열린 채 다시 그리기 비용이 닫힌 상태와 비슷한가). 열기·모드 변경 때의 비용.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 그린 줄이 캐시로 낡는가 | 없음(기존 테스트는 모두 캐시를 거치지 않는 `overlay_lines`로 본다) | 리뷰 테스트 넷 추가, 통과 |
| 2. 선택·고스트·키맵·교환·프리셋 변화 | 일부(`page_clamps_when_keymap_shrinks`, `swap_alt_cmd_is_reflected`가 `overlay_lines`로만) | 그린 줄 기준으로 추가, 통과 |
| 3. 열린 채 모드 변경 | 있음(`ctrl_tab_while_open_does_not_leave_stale_overlay`, `overlay_lines`로) | 그린 줄·머리말·여러 번 순환·모드 Action으로 추가, 통과 |
| 4. 열린 인자 Action | 없음(`argument_variants_are_distinguished`는 변형 있는 `step::SetLength`만) | 깨짐(부록 B2) |
| 5. 바깥 클릭이 아래로 샘 | 없음(`click_inside_keeps_click_outside_closes`는 빈 왼쪽 위만 클릭) | 깨짐(부록 B1) |
| 6. `?` 토글·배타 | 일부(`question_mark_inside_overlay_does_not_leak`, `mode_menu_replaces_overlay`, `palette_over_overlay_does_not_orphan_overlay`) | 반복 토글, Step의 `?`, 팝업 → 메뉴 열기, 팔레트 → 메뉴 열기 추가, 통과 |
| 7. `Kbd` | 없음 | 그려짐 확인 추가, 통과. `delete`/`backspace` 기호는 제안 |
| 8. 성능 | 리뷰 1의 `overlay_line_cost`(캐시 없는 `lines_for` 시간) | 캐시 그리기 시간 측정 추가 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s2_t6b_r2.rs`에서 실행했다. 깨진 시도는 파일에서 빼고 부록에 남겼다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 열린 채 `ctrl-tab` 8번, `ctrl-shift-tab` 1번. 매번 그린 줄·머리말이 지금 경로·쪽으로 새로 구한 줄과 같은가 | 못 깨뜨림 | `rendered_rows_follow_mode_cycle` |
| 2 | 오버레이 포커스에서 `mode::Step`·`Lyric`·`Normal` Action, 그 뒤 Step에서 Esc | 못 깨뜨림(Esc는 오버레이만 닫고 Step 유지) | `rendered_rows_follow_mode_actions` |
| 3 | 마지막 쪽 근처에서 Selection 켬/끔, Step > Ghost 켬/끔 | 못 깨뜨림 | `rendered_rows_follow_selection_and_ghost` |
| 4 | 뮤트 줄이 있는 쪽에서 사용자 키맵 다시 읽기(`m` 지우고 새 키), 같은 파일 다시 읽기, 수식키 교환, vim 프리셋 | 못 깨뜨림 | `rendered_rows_follow_keymap_changes` |
| 5 | `?` 열기·Tab·`?` 닫기 4회, 다시 열면 첫 쪽, 닫은 뒤 `m` | 못 깨뜨림 | `question_mark_toggles_repeatedly` |
| 6 | Step에서 `?`(걸리지 않은 키), 메뉴로 연 뒤 `?` | 못 깨뜨림(열지 않음, 닫음, Step 유지) | `question_mark_in_step_does_not_open_but_closes` |
| 7 | 모드 메뉴 열린 채 메뉴 「키맵 보기」, 오버레이 위 `cmd-shift-p`, 팔레트 열린 채 메뉴 「키맵 보기」, Esc 뒤 `m` | 못 깨뜨림(늘 하나만, 포커스 복귀) | `popup_palette_overlay_are_exclusive` |
| 8 | Step 오버레이 첫 쪽의 `kbd:space` 등 디버그 셀렉터 | 못 깨뜨림(`Kbd`로 그려짐) | `step_keys_are_painted_as_kbd` |
| 9 | 바인딩 1245개(사용자 676 추가)에서 닫힌 그리기·열기·열린 채 다시 그리기·모드 변경 시간 | 못 깨뜨림(아래 표) | `cached_render_is_cheap_with_big_keymap` |
| 10 | 오버레이가 열린 채 배경 너머 트랙 줄의 뮤트 토글(`track-mute-0`)을 클릭 | 깨짐: 오버레이가 닫히고 트랙도 뮤트된다 | 부록 B1 |
| 11 | 오버레이가 열린 채 배경 너머 「+」(`track-add`)를 클릭 | 깨짐: 오버레이가 닫히고 트랙이 하나 생긴다 | 부록 B1 |
| 12 | 기본 프리셋 Play 모드 오버레이의 `play::SelectTrack(n)` 줄 | 깨짐: `f1, f2, …, f8 · 트랙 1–8 직접 선택 dyn Action { name: "play::SelectTrack" }` 한 줄 | 부록 B2 |
| 13 | 탐침: vim 프리셋 Normal·Step, 기본 프리셋 전 모드의 모든 줄 출력(테스트로 남기지 않음) | 관찰: vim `operator::Count(1..9)`가 `1, …, 9 · 카운트 dyn Action { … }` 한 줄, vim `{`/`}`(`step::OctaveOnce(-1/1)`)가 `{, } · 입력 중인 음만 옥타브 ±1 dyn Action { … }` 한 줄로 방향이 사라짐. 기본 Normal의 `edit::Delete`는 `x, delete, backspace`인데 `Kbd`는 `delete`·`backspace`를 모두 `⌫`로 그린다 | 부록 B3 |

성능(같은 머신, 바인딩 1245개, `cached_render_is_cheap_with_big_keymap` 출력):

| 빌드 | 닫힌 그리기 | `?`로 열기(키 처리+그리기) | 열린 채 다시 그리기(5회 평균) | 열린 채 `ctrl-tab` |
| --- | --- | --- | --- | --- |
| release | 1.6 ms | 38.8 ms | 2.0 ms | 4.5 ms |
| debug | 10.4 ms | 268.5 ms | 24.9 ms | 51.7 ms |

캐시는 듣는다(리뷰 1의 열린 채 그리기 release 25.9 ms → 2.0 ms). 열 때와 모드·키맵이 바뀔 때만 한 번 다시 구한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음, 노트는 클립 안, 틱 ≥ 0, 채널 1–16 | 아니오 | 이 Task는 모델을 건드리지 않는다 |
| 팝업은 하나만 띄운다(`window.rs` 창 불변) | 아니오 | 팔레트·팝업·오버레이 어느 쪽에서 열어도 나머지를 닫는다(`window.rs:409-410`, `:470-471`, `overlay.rs` `help_keymap_overlay`). 시도 7 |
| 오버레이가 열린 동안 문서는 모드 키를 먹지 않는다(계획 끝의 정의 4) | 키로는 아니오, 마우스로는 예 | 키는 오버레이 포커스가 막는다. 그러나 배경 너머 클릭이 아래 버튼을 눌러 문서를 바꾼다(부록 B1) |
| 오버레이는 지금 모드의 키를 보인다(정의 2) | 아니오 | 그린 줄 기준 시도 1-4 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| UI-6 `?`로 현재 모드 키맵 겹쳐 표시 | 부분 충족 | 모드 추종·변형 있는 Action은 고쳐졌다(시도 1-4, 리뷰 1 부록 A1·A3 통과). 그러나 변형 없는 열린 인자 Action은 여전히 키마다 다른 뜻을 한 줄로 뭉치고 GPUI 디버그 표기를 제목에 보인다(`overlay.rs:107-114`, 부록 B2, B3). 기본 프리셋 Play에서 보인다 |
| KM-10 오버레이는 항상 현재 키맵 | 충족 | 판이 바뀌면 다시 구한다(`overlay.rs:174-185`). 그린 줄 기준 시도 4 |
| 5.8 `?` 행, Tab 표의 다음/이전 쪽 | 충족 | `default.json:64`, `:530-538` |
| 5.8.1 vim `?` | 충족 | 리뷰 1 시도 1 그대로 통과 |
| 7.6 도움말 > 키맵 보기 | 충족 | `commands/table.rs` `help::KeymapOverlay` 메뉴 항목. 시도 7 |
| 11절 컴포넌트 표 `Sheet` 또는 `Popover` + `Kbd` | 충족 | 키는 `Kbd`(`overlay.rs:266`, 시도 8). 그릇은 popover 테마의 가운데 패널. 보고서의 해석(`Popover`는 기준 요소에 붙어 뜨고 `Sheet`는 자체 포커스·Esc를 가져 `Overlay` 컨텍스트와 겹침)은 11절을 모양 지침으로 읽은 것이고 REQ 뜻(겹쳐 보이는 패널 + 키 표기)을 바꾸지 않는다고 본다 |
| A13 Esc = 통상의 취소, 수식키 Esc 무동작 | 충족 | `default.json:533`. 리뷰 1 테스트와 시도 2 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. UI 스레드만 건드린다.
- 제품 코드의 `unwrap`/`expect`: 새 코드에 없음. `pages - 1`은 `page_count`가 `max(1)`이라 넘치지 않는다.
- 에러가 삼켜지는 곳: `KeyContext::parse(c).ok()`(`overlay.rs` `lines_for`) — 경로가 `&'static` 상수라 실패하지 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 배경에 `occlude()`가 없어(GPUI `HitboxBehavior::Normal`) 오버레이 바깥 클릭이 닫기와 함께 그 아래 요소에도 전달된다. 어두운 배경 너머의 트랙 뮤트 토글을 누르면 오버레이가 닫히고 트랙이 뮤트되며, 「+」를 누르면 트랙이 생긴다. 배경은 모달처럼 보이는데 문서가 바뀐다(계획 끝의 정의 4 「열린 동안 문서는 … 먹지 않는다」의 마우스 경로). 같은 이유로 휠 스크롤·hover·툴팁도 아래로 간다. 배경에 `.occlude()`를 걸면 된다(안쪽 패널의 `stop_propagation`은 그대로). 회귀 테스트: 부록 B1 둘 | `crates/keyscore-app/src/overlay.rs:280-291` |
| 2 | 수정 | 리뷰 1 지적 3이 변형 없는 열린 인자 Action에서 뿌리까지 고쳐지지 않았다. 팔레트 줄을 찾지 못하면 `format!("{action:?}")`를 묶음 키와 제목에 쓰는데, GPUI의 `impl Debug for dyn Action`(`gpui-pre-0.3.8/src/action.rs:176-182`)은 `dyn Action { name: "…" }`만 찍고 인자를 찍지 않는다. 그래서 인자가 다른 바인딩이 한 줄로 뭉치고 디버그 표기가 사용자에게 보인다. 기본 프리셋 Play: `f1, …, f8 · 트랙 1–8 직접 선택 dyn Action { name: "play::SelectTrack" }`. vim: `{, } · 입력 중인 음만 옥타브 ±1 dyn Action { … }`(어느 키가 내림인지 사라짐), `1, …, 9 · 카운트 dyn Action { … }`. 보고서의 「변형이 없는 열린 인자는 Action 표기를 붙인다」는 실제로 이뤄지지 않는다. 인자 값을 실제로 드러내는 표기(예: 키맵 원문 `play::SelectTrack(1)`, 또는 바인딩마다 한 줄)로 묶고 제목을 만들어야 한다. 회귀 테스트: 부록 B2 | `crates/keyscore-app/src/overlay.rs:104-114` |
| 3 | 제안 | 테스트 API `overlay_lines`는 캐시를 거치지 않고 매번 새로 구한다. 그래서 구현자 테스트와 리뷰 1 테스트는 모두 캐시가 낡는 경우를 잡지 못한다(이번 리뷰의 `assert_fresh`처럼 그린 줄로 보는 테스트가 필요했다). `overlay_lines`가 캐시된 줄을 돌려주거나, 그린 줄을 보는 테스트를 구현자 테스트에 두기를 권한다 | `crates/keyscore-app/src/overlay.rs:147-156` |
| 4 | 제안 | `Kbd::format`(gpui-kit)은 macOS에서 `delete`(앞으로 지우기)와 `backspace`를 모두 `⌫`로 그린다. 기본 Normal의 `edit::Delete` 줄이 `X, ⌫, ⌫`로 보인다. `delete`는 `⌦`가 맞다. gpui-kit 쪽 수정이거나 오버레이에서 같은 그림의 키를 합치면 된다 | `crates/keyscore-app/src/overlay.rs:266`, gpui-kit `crates/component/src/kbd.rs` |
| 5 | 제안 | 큰 키맵(1245 바인딩)에서 여는 순간은 release 38.8 ms로 한두 프레임을 넘는다(다시 그리기는 2 ms). `lines_for`가 여전히 바인딩마다 전체 키맵을 도는 제곱 비용이다. 키 시퀀스별로 묶어 `bindings_for_input`을 한 번만 부르면 줄어든다. 기본 키맵에서는 문제없다 | `crates/keyscore-app/src/overlay.rs:69-140` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1 지적 1(팔레트), 2(모드 추종), 4(`Kbd`)와 제안 5(캐시), 7(`?` 닫기)은 뿌리에서 고쳐졌고 그린 줄 기준으로도 깨뜨리지 못했다. 캐시 무효화(판 + 경로 포인터)도 선택·고스트·키맵 다시 읽기·교환·프리셋에서 낡지 않았다. 그러나 지적 1(배경 너머 클릭이 문서를 바꿈)과 지적 2(리뷰 1 지적 3의 잔여: 열린 인자 Action이 뭉치고 디버그 표기가 보임)를 고친 뒤 재리뷰한다. 부록 B1·B2 테스트를 저장소에 넣고 통과시켜야 한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(diff에 `ignore` 없음. 자동 게이트 넷 모두 통과: fmt, clippy, `cargo test --workspace`, `cargo deny check`. 무시된 테스트 1개는 `main`의 keyscore-midi 기존 것)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 깨진 테스트와 실패 출력

`review_s2_t6b_r2.rs`의 도우미(`start`, `open`, `press`, `run`, `lines`, `muted`)를 그대로 쓴다. B1은 아래 도우미가 더 필요하다.

~~~rust
fn track_count(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).project().tracks().len())
}
~~~

### B1. 배경 너머 클릭이 아래 버튼을 누름 (지적 1)

~~~rust
/// 오버레이 바깥(어두운 배경) 클릭이 닫기만 하고 그 아래 트랙 뮤트 버튼을 누르지 않는다.
#[gpui_kit::test]
fn backdrop_click_does_not_hit_track_mute(cx: &mut TestAppContext) {
    start(cx, "clickthru", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    cx.update_window(handle, |_, window, cx| window.click("track-mute-0", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(lines(cx, &view).is_none(), "바깥 클릭이 닫지 않았다");
    assert!(!muted(cx, &view), "오버레이 바깥 클릭이 아래의 뮤트 버튼을 눌렀다");
}

/// 오버레이 바깥 클릭이 아래 「+」(트랙 추가) 버튼을 누르지 않는다.
#[gpui_kit::test]
fn backdrop_click_does_not_add_track(cx: &mut TestAppContext) {
    start(cx, "clickadd", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    let n = track_count(cx, &view);
    cx.update_window(handle, |_, window, cx| window.click("track-add", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(lines(cx, &view).is_none(), "바깥 클릭이 닫지 않았다");
    assert_eq!(track_count(cx, &view), n, "오버레이 바깥 클릭이 아래의 트랙 추가 버튼을 눌렀다");
}
~~~

~~~
thread 'backdrop_click_does_not_hit_track_mute' panicked at crates/keyscore-app/tests/review_s2_t6b_r2.rs:388:5:
오버레이 바깥 클릭이 아래의 뮤트 버튼을 눌렀다

thread 'backdrop_click_does_not_add_track' panicked at crates/keyscore-app/tests/review_s2_t6b_r2.rs:403:5:
assertion `left == right` failed: 오버레이 바깥 클릭이 아래의 트랙 추가 버튼을 눌렀다
  left: 2
 right: 1
~~~

두 테스트 모두 앞의 단언(「바깥 클릭이 닫지 않았다」)은 통과했다. 즉 닫기와 아래 버튼 동작이 함께 일어난다.

### B2. 열린 인자 Action이 뭉치고 디버그 표기가 보임 (지적 2)

~~~rust
/// Play의 `f1`~`f8`(`play::SelectTrack(n)`)은 키마다 다른 트랙이다. 한 줄로 뭉치거나 GPUI 디버그 표기
/// (`dyn Action { .. }`)가 제목에 새면 안 된다.
#[gpui_kit::test]
fn open_argument_actions_keep_their_arguments(cx: &mut TestAppContext) {
    start(cx, "openarg", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Play);
    run(cx, handle, help::KeymapOverlay);
    let l = lines(cx, &view).unwrap();
    let sel: Vec<_> = l.iter().filter(|x| x.ends_with("(play::SelectTrack)")).collect();
    assert!(
        !l.iter().any(|x| x.contains("dyn Action")),
        "디버그 표기가 제목에 샜다: {sel:?}"
    );
    assert!(sel.len() >= 8, "f1~f8이 한 줄로 뭉쳤다: {sel:?}");
}
~~~

~~~
thread 'open_argument_actions_keep_their_arguments' panicked at crates/keyscore-app/tests/review_s2_t6b_r2.rs:418:5:
디버그 표기가 제목에 샜다: ["f1, f2, f3, f4, f5, f6, f7, f8 · 트랙 1–8 직접 선택 dyn Action { name: \"play::SelectTrack\" } (play::SelectTrack)"]
~~~

### B3. 탐침 출력(테스트로 남기지 않음)

vim 프리셋 Normal·Step, 기본 프리셋 전 모드의 `overlay_lines`를 모두 찍은 것 가운데 관련 줄.

~~~
1, 2, 3, 4, 5, 6, 7, 8, 9 · 카운트 dyn Action { name: "operator::Count" } (operator::Count)
{, } · 입력 중인 음만 옥타브 ±1 dyn Action { name: "step::OctaveOnce" } (step::OctaveOnce)
f1, f2, f3, f4, f5, f6, f7, f8 · 트랙 1–8 직접 선택 dyn Action { name: "play::SelectTrack" } (play::SelectTrack)
x, delete, backspace · 삭제 (edit::Delete)
~~~

마지막 줄은 화면에서 `Kbd`로 `X`, `⌫`, `⌫`가 된다(지적 4). 기본 프리셋에서 `dyn Action` 표기가 나오는 줄은 Play의 `play::SelectTrack` 하나였다(`Step > Pending`의 `step::SetOctave(n)` 등은 지금 모드 경로에 그 컨텍스트가 없어 보이지 않는다).
