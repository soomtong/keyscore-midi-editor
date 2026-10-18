# 적대적 리뷰: S2-T6b `?` 키맵 오버레이 (리뷰 1회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 에이전트 세션(구현자와 다른 세션)
- 대상 커밋: `67ab26a`(브랜치 `s2/t6b-keymap-overlay`, `git diff main...HEAD`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트와 보고서를 열기 전에 `overlay.rs`, `window.rs`, `default.json` 변경과 GPUI `keymap.rs`의 `bindings_for_input`·`window.rs`의 `dispatch_key_event`만 읽고 적었다.

1. 접두 시퀀스(vim `g g`)가 한 줄로 「g g」처럼 보이는가. `bindings_for_input`을 전체 시퀀스로 부르므로 맞을 것 같지만 vim 프리셋에서 확인한다. vim이 `null`로 지운 키(`z`, `m`)는 빠지는가.
2. vim 프리셋에서도 `?`가 오버레이를 여는가(5.8.1 충돌 메모). `escape`·`tab`의 Overlay 바인딩이 default.json에만 있으므로 vim이 default 위에 얹히는지 확인한다.
3. 가려진 바인딩: Selection의 `escape`(select::Clear), Step > Ghost의 `escape`·`tab`이 이기고, 그 아래의 `escape → mode::Normal`은 빠지는가.
4. 수식키 교환(KM-11)을 켜면 열린 오버레이의 `alt-k`가 `cmd-k`로 바뀌는가.
5. 인자 있는 Action(`step::SetLength(whole)` 등)을 이름으로만 모은다(`overlay.rs:78`). 제목은 `spec.label()`뿐이라 어느 키가 어느 인자인지 사라질 것이다. 명령 표에는 변형 라벨이 있다.
6. 오버레이 경로는 연 때 고정한다(`overlay.rs:135`). 전역 바인딩(`ctrl-tab` mode::Cycle, 컨텍스트 없음)은 Overlay 포커스에서도 맞고 처리기는 창 루트에 있으므로 열린 채 모드가 바뀔 수 있다. 그때 오버레이가 옛 모드를 보이는가.
7. `open_palette`(`window.rs:402`)는 오버레이를 닫지 않는다(`open_popup`만 닫는다). 오버레이 위에 `cmd-shift-p`로 팔레트를 열고 닫으면 포커스는 문서로 가고 오버레이는 남는가. 그 상태에서 Esc·모드 키는 어떻게 되는가.
8. 수식키 붙은 Esc(`shift-escape` 등)는 아무것도 하지 않아야 한다(A13). Step에서 오버레이를 닫는 Esc가 Step을 나가게 하면 안 된다.
9. 안쪽 클릭은 닫지 않고 바깥 클릭은 닫고 문서로 포커스를 돌리는가.
10. 열린 동안 `?`를 다시 누르면? Overlay 컨텍스트에는 `?`가 없어 닫히지 않을 것이다. 문서로 새지는 않는가.
11. 열린 동안 키맵이 줄어 쪽 수가 줄면 `render_overlay`가 쪽을 맞추는가(마지막 쪽에서).
12. 성능: 그릴 때마다 모든 바인딩마다 `bindings_for_input`(전체 키맵 순회)을 부른다. 바인딩 수의 제곱이다. 큰 사용자 키맵에서 잰다.
13. 두 창, 창 전환 뒤 Esc, 재생 중 열기, 모드 메뉴 팝업과의 교대.
14. 첫 승자 판정은 GPUI 디스패치의 「처리되지 않으면 다음 바인딩으로 넘어감」(`dispatch_key_event`)을 반영하지 않는다. 처리기가 없는 Action이 첫 승자면 오버레이는 그 줄을 보이고 실제로 동작하는 둘째 바인딩은 숨긴다(이론상 경로. 지금 키맵에서 실제 사례는 찾지 않았다).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. vim `g g`·null 키 | 없음 | 리뷰 테스트로 추가, 통과 |
| 2. vim에서 `?`·Esc·Tab | 없음 | 리뷰 테스트로 추가, 통과 |
| 3. Selection·Ghost 가림 | 없음 | 리뷰 테스트 둘 추가, 통과 |
| 4. 수식키 교환 반영 | 없음 | 리뷰 테스트로 추가, 통과 |
| 5. 인자 변형 구분 | 없음 | 깨짐(부록 A3) |
| 6. 열린 채 모드 변경 | 없음 | 깨짐(부록 A1) |
| 7. 오버레이 위 팔레트 | 없음 | 깨짐(부록 A2) |
| 8. 수식키 Esc, Step에서 Esc | 일부(`overlay_shows_current_mode`가 Step Esc만) | 수식키 Esc 추가, 통과 |
| 9. 클릭 안/밖 | 없음 | 추가, 통과 |
| 10. 열린 동안 `?` | 없음 | 추가, 통과(닫히지 않음 관찰) |
| 11. 쪽 수 줄어듦 | 없음 | 추가, 통과 |
| 12. 성능 | 없음 | 측정 테스트 추가 |
| 13. 두 창·재생·모드 메뉴·팔레트에서 열기(Step) | 없음 | 넷 추가, 통과 |
| 14. 처리기 없는 첫 승자 | 없음 | 실행하지 않음(3절에 적지 않음) |
| 사용자 키맵 반영(Unbind + 새 키) | 있음(`overlay_reflects_user_keymap`) | |
| Tab 쪽 넘김 끝에서 멈춤, 열린 동안 `m` 무시 | 있음(`overlay_pages_with_tab_and_closes_with_escape`) | |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s2_t6b.rs`에서 실행했다. 깨진 시도는 파일에서 빼고 부록에 남겼다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | vim 프리셋에서 `?`, `g g` 줄, null로 지운 `z` 빠짐, Tab·Esc | 못 깨뜨림 | `vim_preset_question_mark_opens_and_shows_sequences` |
| 2 | 수식키 교환을 열린 채 켜기 | 못 깨뜨림(`alt-k` → `cmd-k`) | `swap_alt_cmd_is_reflected` |
| 3 | Selection에서 `escape` 줄이 하나(select::Clear)이고 Esc가 선택을 지우지 않음 | 못 깨뜨림 | `selection_context_shows_winning_escape` |
| 4 | Step > Ghost에서 tab·escape 승자, `escape → mode::Normal` 숨김, Esc가 고스트를 지우지 않음 | 못 깨뜨림 | `ghost_context_shadows_step_escape` |
| 5 | `shift-`/`alt-`/`ctrl-escape` | 못 깨뜨림 | `modified_escape_does_nothing` |
| 6 | 열린 동안 `?` 다시, 그 뒤 `m` | 못 깨뜨림(새지 않음, 닫히지도 않음) | `question_mark_inside_overlay_does_not_leak` |
| 7 | 안쪽 클릭, 왼쪽 위 배경 클릭 | 못 깨뜨림 | `click_inside_keeps_click_outside_closes` |
| 8 | 열린 채 모드 메뉴(`mode::ShowMenu`) | 못 깨뜨림 | `mode_menu_replaces_overlay` |
| 9 | Step에서 팔레트로 「키맵 보기」 실행 | 못 깨뜨림 | `palette_opens_overlay_in_step` |
| 10 | 두 창, 창 전환 뒤 Esc | 못 깨뜨림 | `two_windows_are_independent` |
| 11 | 재생 중 열고 여러 번 그리고 닫기 | 못 깨뜨림 | `overlay_while_playing` |
| 12 | 마지막 쪽에서 사용자 키맵으로 한 글자 키를 모두 null, `keymap::apply` | 못 깨뜨림(쪽이 맞춰짐) | `page_clamps_when_keymap_shrinks` |
| 13 | 사용자 키맵 676개 추가(총 1245 바인딩)에서 `lines_for`·그리기 시간 | 기능은 못 깨뜨림. 비용은 제곱(아래) | `overlay_line_cost` |
| 14 | 열린 채 `ctrl-tab` | 깨짐: 모드는 Step, 오버레이는 「Normal > Arrangement」 키를 계속 보임 | 부록 A1 |
| 15 | 열린 채 `cmd-shift-p` | 깨짐: 팔레트와 오버레이가 함께 열림. 팔레트를 Esc로 닫으면 오버레이는 보이는데 포커스는 문서: `m`이 뮤트하고, Esc를 다시 눌러도 오버레이가 닫히지 않음 | 부록 A2 |
| 16 | Step 오버레이의 `step::SetLength` 줄 | 깨짐: `1, 2, 3, 4, 5, 6, 7 · Step 길이 (step::SetLength)` — 어느 키가 어느 길이인지 없음 | 부록 A3 |
| 17 | 열린 채 `cmd-z` | 관찰: 문서가 되돌려짐(뮤트 해제). 오버레이는 열린 채 | 부록 A4(탐침) |

성능 측정(같은 머신, `lines_for` 한 번 / 열린 상태 `render_frame` 한 번):

| 바인딩 수 | debug | release |
| --- | --- | --- |
| 568(기본) | - | 2.0 ms / 4.1 ms(닫힌 상태 그리기 1.6 ms) |
| 1245(사용자 676 추가) | 203 ms / 222 ms | 24.5 ms / 25.9 ms |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음, 노트는 클립 안, 틱 ≥ 0, 채널 1–16 | 아니오 | 이 Task는 모델을 건드리지 않는다(`overlay.rs`는 읽기만) |
| 팝업은 하나만 띄운다(`window.rs` 주석의 창 불변) | 예 | 오버레이 → `cmd-shift-p`(부록 A2). `open_palette`가 `self.overlay`를 비우지 않는다(`window.rs:402-419`) |
| 오버레이가 열린 동안 문서는 모드 키를 먹지 않는다(계획 끝의 정의 4) | 예 | 부록 A2 경로 뒤 `m`이 뮤트 |
| 오버레이는 현재 모드의 키를 보인다(계획 끝의 정의 2) | 예 | 부록 A1 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| UI-6 `?`로 현재 모드 키맵 겹쳐 표시 | 부분 충족 | 여는 경로·모드별 내용은 맞다. 그러나 열린 채 모드가 바뀌면 옛 모드를 보인다(`overlay.rs:135`, `overlay.rs:163`, 부록 A1). 인자 변형을 이름으로 뭉쳐 키의 뜻이 사라진다(`overlay.rs:78-87`, 부록 A3) |
| KM-10 오버레이는 항상 현재 키맵 | 충족 | 그릴 때마다 엔진에서 다시 구한다(`overlay.rs:163`). 사용자 Unbind, vim null, 수식키 교환, 핫리로드 모두 리뷰 테스트로 확인 |
| 5.8 `?` 행(기본) | 충족 | `default.json:64` |
| 5.8.1 vim `?`(356·371줄) | 충족 | vim은 default 위에 얹힌다(`keymap/mod.rs` `load_layers`). 리뷰 테스트 1 |
| 5.8 Tab 표(434줄) 다음/이전 쪽 | 충족 | `default.json:530-536`, `overlay.rs:147-153` |
| 7.6 도움말 > 키맵 보기 | 충족 | `commands/table.rs`의 `CommandSpec::menu("help::KeymapOverlay", "키맵 보기", MenuId::Help, ...)` |
| 11절 컴포넌트 표 `Sheet` 또는 `Popover` + `Kbd` | 미충족 | 맨 `div`와 GPUI 키 문자열(`overlay.rs:174-243`, 키는 `overlay.rs:192`). 보고서가 S8로 미뤘으나 결정 기록이 없다 |
| A13 Esc = 통상의 취소, 수식키 Esc 무동작 | 충족(오버레이가 포커스를 가질 때) | `default.json:533`. 부록 A2 경로에서는 Esc가 오버레이를 닫지 못한다 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 UI 스레드만 건드린다.
- 제품 코드의 `unwrap`/`expect`: 새 코드에 없음. `overlay.pages - 1`(`overlay.rs:167`)은 `page_count`가 `max(1)`이라 넘치지 않는다.
- 에러가 삼켜지는 곳: `KeyContext::parse(c).ok()`(`overlay.rs:48`) — 경로는 `&'static` 상수라 실패하지 않는다. 문제 없음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 오버레이가 열린 채 `palette::Open`(`cmd-shift-p`, 전역)이 맞아 팔레트가 함께 열린다. 팔레트를 닫으면 포커스는 문서로 가고 오버레이는 남는다. 이때 오버레이가 보이는데 모드 키(`m`)가 문서에 먹히고, Esc는 오버레이를 닫지 못한다(바깥 클릭이나 메뉴로만 닫힘). 「팝업은 하나만」과 계획 끝의 정의 4 위반. `open_popup`처럼 `open_palette`도 오버레이를 닫아야 한다. 회귀 테스트: 부록 A2 | `crates/keyscore-app/src/window.rs:402-419` |
| 2 | 수정 | 전역 `ctrl-tab`/`ctrl-shift-tab`(mode::Cycle/CycleBack)은 Overlay 포커스에서도 맞아 모드를 바꾼다. 오버레이는 연 때 고정한 경로를 계속 써서 Step인데 Normal 키를 보인다(UI-6 「현재 모드」). 보고서의 「열린 동안에는 모드가 바뀌지 않는다」는 사실이 아니다. 모드가 바뀌면 닫거나, 그릴 때 `mode_state().context_path()`를 다시 읽어야 한다. 회귀 테스트: 부록 A1 | `crates/keyscore-app/src/overlay.rs:28`, `:135`, `:163`; `assets/keymaps/default.json:8` |
| 3 | 수정 | 인자 있는 Action을 이름으로만 모아 `1, 2, 3, 4, 5, 6, 7 · Step 길이`처럼 키와 뜻의 대응이 사라진다(`step::SetLength`, `step::SetDottedLength`, `step::SetOctave`, vim `operator::Count` 등). 명령 표에 변형 라벨(`온음표` 등)이 있으므로 (Action, 인자)로 묶고 변형 라벨을 써야 한다. 키맵을 보여 주는 UI-6의 목적에 비춰 「대략」 충족. 회귀 테스트: 부록 A3 | `crates/keyscore-app/src/overlay.rs:78-87` |
| 4 | 수정 | REQ 11절 컴포넌트 표의 `Sheet`/`Popover` + `Kbd`를 쓰지 않고, 키도 GPUI 문자열(`ctrl-alt-u`)로 보인다. 보고서는 S8로 미뤘지만 REQ를 바꾸거나 미루는 결정 기록이 없다. `Kbd`로 그리거나 `docs/decisions/`에 미룸을 적어야 한다 | `crates/keyscore-app/src/overlay.rs:174-243`, `:192` |
| 5 | 제안 | `lines_for`가 바인딩마다 전체 키맵을 도는 `bindings_for_input`을 불러 바인딩 수의 제곱이다. 열 때 한 번, 그릴 때마다 한 번 더 돈다. 기본 키맵(568)은 release 2 ms로 괜찮지만 1245개에서 release 24.5 ms, debug 203 ms로 한 프레임을 넘는다. 키맵 세대(`keymap::apply`)마다 한 번만 구해 캐시하거나, 키 시퀀스별로 묶어 한 번만 풀면 된다 | `crates/keyscore-app/src/overlay.rs:53-69`, `:136`, `:163` |
| 6 | 제안 | 열린 동안 `cmd-z`·`cmd-t`·`cmd-enter` 같은 전역 표준 단축키는 오버레이 아래의 문서를 바꾼다(되돌리기로 뮤트가 풀리는 것을 확인). 계획은 「모드 키」만 막는다고 했으므로 위반은 아니나, 도움말 오버레이 아래에서 문서가 바뀌는 것이 의도인지 계획에 적어 두기를 권한다 | `crates/keyscore-app/src/window.rs:577-580`(루트 처리기), `assets/keymaps/standard.json` |
| 7 | 제안 | 열린 동안 `?`를 다시 누르면 아무 일도 없다(Overlay 컨텍스트에 `?` 없음). 메뉴·팔레트의 `help::KeymapOverlay`는 닫는데 키는 닫지 않아 어긋난다. Overlay 컨텍스트에 `?` → `help::CloseOverlay`를 두는 것을 고려 | `assets/keymaps/default.json:530-536` |
| 8 | 제안 | 「첫 승자」 판정은 GPUI가 처리되지 않은 Action에서 다음 바인딩으로 넘어가는 것(`gpui-pre-0.3.8/src/window.rs` `dispatch_key_event`의 `for binding in match_result.bindings`)을 반영하지 않는다. 처리기 없는 Action이 이기는 키에서는 표시와 실제가 어긋날 수 있다. 지금 키맵에서 사례는 찾지 못했다. 모듈 주석에 한계로 적기를 권한다 | `crates/keyscore-app/src/overlay.rs:65-70` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2·3·4를 고친 뒤 재리뷰. 부록 A1~A3의 테스트를 저장소에 넣고 통과시켜야 한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`popup`·`palette` 필드의 `pub(crate)` 승격은 오버레이 모듈이 쓰기 위한 것)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`cargo test --workspace` 전부 통과, diff에 `ignore` 없음)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 깨진 테스트와 실패 출력

`review_s2_t6b.rs`의 도우미(`start`, `open`, `press`, `run`, `lines`, `mode_of`, `keys_of`, `header`, `muted`)를 그대로 쓴다.

### A1. 열린 채 `ctrl-tab` → 옛 모드 표시 (지적 2)

~~~rust
#[gpui_kit::test]
fn ctrl_tab_while_open_does_not_leave_stale_overlay(cx: &mut TestAppContext) {
    start(cx, "cycle", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    press(cx, handle, "ctrl-tab");
    let m = mode_of(cx, &view);
    if m != Mode::Normal {
        if let Some(l) = lines(cx, &view) {
            assert!(
                keys_of(&l, "step::Rest").contains(&"space"),
                "모드는 {m:?}인데 오버레이는 Normal 키를 보인다: {:?}",
                header(cx, handle)
            );
        }
    }
}
~~~

~~~
thread 'ctrl_tab_while_open_does_not_leave_stale_overlay' panicked at crates/keyscore-app/tests/review_s2_t6b.rs:393:13:
모드는 Step인데 오버레이는 Normal 키를 보인다: Some("키맵 · Normal > Arrangement · 1/4 쪽 (Tab 다음, Shift+Tab 이전, Esc 닫기)")
~~~

### A2. 오버레이 위 팔레트 → 고아 오버레이 (지적 1)

~~~rust
#[gpui_kit::test]
fn palette_over_overlay_does_not_orphan_overlay(cx: &mut TestAppContext) {
    start(cx, "palover", None);
    let (handle, view) = open(cx);
    press(cx, handle, "?");
    press(cx, handle, "cmd-shift-p");
    let palette_open = cx.read(|cx| view.read(cx).palette_query(cx).is_some());
    assert!(
        !(palette_open && lines(cx, &view).is_some()),
        "팔레트와 오버레이가 함께 열려 있다"
    );
    press(cx, handle, "escape");
    if lines(cx, &view).is_some() {
        press(cx, handle, "m");
        assert!(!muted(cx, &view), "오버레이가 보이는데 m이 문서를 뮤트했다");
    }
}
~~~

~~~
thread 'palette_over_overlay_does_not_orphan_overlay' panicked at crates/keyscore-app/tests/review_s2_t6b.rs:411:5:
팔레트와 오버레이가 함께 열려 있다
~~~

첫 단언을 지나친 탐침 결과(A4와 같은 실행): 팔레트를 Esc로 닫은 뒤 `overlay_after_esc=true muted=true`, Esc를 한 번 더 눌러도 `overlay_after_2nd_esc=true`.

### A3. 인자 변형이 뭉침 (지적 3)

~~~rust
#[gpui_kit::test]
fn argument_variants_are_distinguished(cx: &mut TestAppContext) {
    start(cx, "variants", None);
    let (handle, view) = open(cx);
    run(cx, handle, mode::Step);
    run(cx, handle, help::KeymapOverlay);
    let l = lines(cx, &view).unwrap();
    let set_len: Vec<_> = l.iter().filter(|x| x.contains("step::SetLength")).collect();
    assert!(
        set_len.iter().any(|x| x.contains("온음표")),
        "변형 라벨이 없다: {set_len:?}"
    );
}
~~~

~~~
thread 'argument_variants_are_distinguished' panicked at crates/keyscore-app/tests/review_s2_t6b.rs:431:5:
변형 라벨이 없다: ["1, 2, 3, 4, 5, 6, 7 · Step 길이 (step::SetLength)"]
~~~

### A4. 탐침 출력(테스트로 남기지 않음)

~~~
PALETTE: overlay_after_esc=true muted=true
PALETTE: overlay_after_2nd_esc=true
CYCLE: mode=Step overlay=true
CYCLE esc: mode=Step overlay=false
UNDO: muted=false overlay=true
~~~

`CYCLE esc`: `ctrl-tab` 뒤에도 포커스는 오버레이에 있어 Esc는 오버레이만 닫는다(Step 유지). `UNDO`: `m`으로 뮤트 → `?` → `cmd-z`가 오버레이 아래 문서를 되돌렸다(지적 6).
