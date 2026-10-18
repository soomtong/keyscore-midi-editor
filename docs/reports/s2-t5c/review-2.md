# 적대적 리뷰: S2-T5c 트랙 인스펙터와 상태줄 (리뷰 2회차)

- 리뷰어 세션: 구현 세션·리뷰 1 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 113c2ea(브랜치 `s2/t5c-inspector-status`, 수정 커밋 cb456ce, `git diff main...HEAD`)
- 깨뜨리는 시도 파일: `crates/keyscore-app/tests/review_s2_t5c_r2.rs`(통과한 시도 13개). 깨진 시도 2개와 참고용 1개는 부록 A에 있다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

수정 커밋(`git show cb456ce`)과 `window.rs` Render, `inspector.rs` 테스트 노트, gpui-pre `elements/div.rs`의 `track_focus` 마우스다운(2774–2784)과 `on_mouse_down`(126–140)을 읽고 테스트를 열기 전에 적었다.

1. 지적 1의 뿌리: 새 `content` 처리기가 `main`의 겹(`track_focus`)이 덮던 범위를 모두 덮는가. `main`에서 겹 안이던 상태줄이 이번 Render에서는 `content` 밖(루트의 자식)이다.
2. 새 처리기는 `MouseButton::Left`만 본다. GPUI `track_focus`는 버튼을 가리지 않고 옮긴다. 오른쪽·가운데 버튼 클릭이 회귀하는가.
3. 처리기가 포커스를 빼앗지 말아야 할 곳: 입력란(다른 입력란이 고치던 중일 때), `Select` 트리거와 드롭다운 목록(마우스로 고르기), 팝업 메뉴 항목(색·채널·모드 메뉴), 팔레트, 뮤트·솔로 토글, 트랙 목록 줄(트랙 목록 자체 처리기와 겹침).
4. 처리기가 포커스를 옮겨야 할 곳에서 고치던 입력이 취소되는가(주 뷰, 인스펙터 빈 자리, Vim에서 뱅크 입력란 숫자 뒤).
5. 패널 경계 끌기가 고치던 입력을 확정하거나 버리는가(`main`과 같은가).
6. 테스트 노트: 다른 창 재생 중 가상 포트에서 프로그램뿐 아니라 뱅크 선택(CC0·CC32)도 빠지는가. 다른 창이 일시정지하면 주인이 풀려 다시 프로그램이 먼저 가는가. 정지 중 없는 이름 포트에서 이유 메시지가 그 포트만 알리는가.
7. `SetProgram` 넘침 수정이 범위 밖 값(`i64::MAX`, 129, -1) 모두에서 메시지만 남기는가. 둘 다 「없음」인 트랙에 「없음/없음」은 편집이 없는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 처리기 범위(상태줄) | 없음(아래 패널·인스펙터 빈 자리만, `review_s2_t5c.rs` 부록 A 이식분) | 깨짐(지적 1) |
| 2 오른쪽 버튼 | 없음 | 깨짐(지적 2) |
| 3 빼앗지 말아야 할 곳 | 일부(트랙 목록 줄, `Select` 키보드) | 입력란 간 이동, 드롭다운 마우스 고르기, 색·모드 메뉴, 토글을 보탬(통과) |
| 4 취소 | 일부(인스펙터 빈 자리) | 주 뷰, Vim 뱅크 숫자 뒤 빈 자리를 보탬(통과) |
| 5 경계 끌기 | 없음 | `main`과 같게 고치던 입력이 남는다. 회귀가 아니어서 지적하지 않는다(부록 A-3) |
| 6 테스트 노트 | 프로그램 체인지만(`test_note_while_other_window_plays_does_not_send_program`) | 뱅크, 일시정지 뒤, 이유 메시지를 보탬(통과) |
| 7 `SetProgram` 경계 | `i64::MIN`만 | 보탬(통과) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 템포 입력란 고치는 중 상태줄 클릭 → `m` | 깨짐(`95m`. `main`에서는 같은 테스트 통과: 회귀) | 부록 A-1 |
| 2 | 이름 입력란 고치는 중 인스펙터 빈 자리 오른쪽 클릭 → `m` | 깨짐(`Zzm`. `main`에서 템포 입력란·인스펙터 자리표시자로 같은 시도는 통과: 회귀) | 부록 A-2 |
| 3 | 이름 입력란 고치는 중 주 뷰 클릭: 취소, 모드 키, 편집 하나 | 못 깨뜨림 | `clicking_main_view_cancels_dirty_name` |
| 4 | 템포 고치는 중 인스펙터 이름 입력란 클릭 → `Xm` 입력: 바깥 처리기가 빼앗지 않음 | 못 깨뜨림 | `clicking_inspector_input_is_not_stolen_by_content_handler` |
| 5 | Vim: 뱅크 입력란 `12` → 인스펙터 빈 자리 → `)`: 카운트 없이 한 칸 | 못 깨뜨림 | `vim_click_inspector_background_then_motion` |
| 6 | 고친 이름 → 트랙 목록 줄 클릭(처리기 둘이 겹침) → `m`: 편집 없음, 뮤트 한 번 | 못 깨뜨림 | `track_row_click_double_focus_is_harmless` |
| 7 | 고친 이름 → 뮤트 토글 클릭: 이름 미확정, 한 단위, 뒤의 `m` 동작 | 못 깨뜨림 | `clicking_mute_toggle_with_dirty_name` |
| 8 | 프로그램 드롭다운을 마우스로 고르기(목록이 `content` 위에 뜬다): 한 단위, 문서 포커스 | 못 깨뜨림(드롭다운은 `occlude`라 바깥 처리기의 hitbox가 hover되지 않는다) | `program_list_mouse_pick_edits_once_and_returns` |
| 9 | 고친 이름 → 색 메뉴 항목 클릭 | 못 깨뜨림 | `color_menu_click_with_dirty_name` |
| 10 | 고친 이름 → 상태줄 모드 표시 → 모드 메뉴 STEP 클릭 | 못 깨뜨림 | `mode_menu_click_with_dirty_name` |
| 11 | `SetProgram{bank:-1, program:0}`을 뱅크·프로그램 없는 트랙에 | 못 깨뜨림(편집 없음) | `set_program_none_none_is_no_edit` |
| 12 | `SetProgram` program `i64::MAX`, 129, -1 | 못 깨뜨림(메시지만) | `set_program_out_of_range_reports` |
| 13 | 다른 창이 가상 포트 ch3로 재생 중, 뱅크 129·프로그램 40 트랙의 테스트 노트 | 못 깨뜨림(CC0·CC32·프로그램 없이 노트만) | `test_note_while_other_plays_sends_no_bank` |
| 14 | 다른 창이 재생 후 일시정지 → 이 창 테스트 노트 | 못 깨뜨림(주인이 풀려 프로그램이 다시 먼저) | `test_note_after_other_stops_sends_program_again` |
| 15 | 정지 중 없는 이름 포트 두 개 중 현재 트랙 것만 알리는가, 상태줄 마지막 메시지 | 못 깨뜨림 | `test_note_missing_named_port_reports_reason` |
| 16 | 이름 입력란 고치는 중 주 뷰·인스펙터 경계 끌기 → Enter | 고치던 이름이 확정된다. `main`도 같다(템포 입력란으로 확인). 회귀 아님, 지적 없음 | 부록 A-3(참고) |

팔레트는 코드로 확인했다. gpui-component `CommandState` 루트가 `track_focus`라 행을 누르면 그쪽이 `prevent_default`하고, 바깥 처리기는 건너뛴다(`main`의 겹과 같다). 팝업 메뉴(`popup_menu.rs:1502`)와 `Select` 드롭다운(`select.rs:614`)은 `occlude`다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 인스펙터 편집 하나 = undo 한 단위 | 아니오 | 시도 6·7·8·9 |
| 같은 값은 편집 없음 | 아니오 | 시도 8(리뷰 1 시도 9와 함께), 11 |
| 고친 글자가 다른 트랙·엉뚱한 때에 확정되지 않음 | 아니오(경계 끌기는 `main`과 같은 기존 동작) | 시도 3·6·7·9·10, 부록 A-3 |
| 다른 창의 재생 소리 상태를 테스트 노트가 바꾸지 않음 | 아니오 | 시도 13 |
| 값 범위(뱅크 0–16383, 프로그램 0–127) | 아니오 | 시도 11·12, 리뷰 1 A-3 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| UI-12 | 충족 | `crates/keyscore-app/src/inspector.rs:484-628`, 시도 7–12 |
| UI-11 | 충족 | Action 처리기 `inspector.rs:191-297` |
| UI-5 | 충족(이 Task 범위. 길이 코드·옥타브는 S3-T4) | `window.rs:635-687` |
| 7.1 화면 구성(상태줄) | 충족 | `window.rs:635-687` |
| 7.1 화면 구성(겹 바깥을 눌러 문서로 돌아오기) | 부분 충족 | 상태줄·오른쪽 클릭에서 돌아오지 않는다(지적 1·2). `crates/keyscore-app/src/window.rs:829-840`, `:892` |
| SEQ-7 | 충족 | `inspector.rs` `channel_label` |
| PRE-3 | 충족 | `inspector.rs:300-391`, 시도 13–15, 리뷰 1 A-4 |
| KM-6 | 충족 | 리뷰 1 시도 12 |
| KM-14 | 충족 | 리뷰 1 시도 1 |
| 7.6 인스펙터 행 | 충족 | `inspector.rs:484-628` |
| 11절 `Select` GM 이름 검색 | 충족 | 시도 8 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 수정은 UI 스레드의 `send_now`·`preview` 인자만 바꿨다).
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 없다. `checked_sub`로 리뷰 1 지적 2가 뿌리에서 고쳐졌다(`inspector.rs:258-264`).
- 에러가 삼켜지는 곳: 없음. 정지 중 이름 포트 경로가 `prepare`의 `missing`을 이제 알린다(`inspector.rs:370-375`).
- 범위 밖 관찰: `crates/keyscore-app/tests/transport_bar.rs`의 `tap_tempo_sets_tempo_from_intervals`가 이 기계에서 혼자 돌리면 늘 실패한다(103.9 BPM). `main`의 임시 worktree에서도 같은 값으로 실패하므로 이 Task의 회귀가 아니다(클릭마다 그리기 시간이 탭 간격에 더해진다). `cargo test --workspace` 전체 실행에서는 한 번 통과, 한 번 실패했다. S2-T5b 쪽의 시간 의존 테스트라 이 리뷰의 지적으로 세지 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 1 지적 1이 뿌리에서 다 고쳐지지 않았다. 새 마우스다운 처리기는 `content`(트랙 목록·주 뷰·인스펙터·아래 패널)에만 달렸고, 상태줄은 루트의 다른 자식이다. `main`(S2-T5b)에서는 상태줄이 겹 안이어서 빈 자리를 누르면 문서로 돌아왔다. 지금은 템포·인스펙터 입력란을 고치다 상태줄(트랙·메시지 글자, 빈 자리)을 누르면 포커스가 입력란에 남아 `m`이 `95m`이 된다. 리뷰 1 지적 1 본문이 「상태줄 빈 자리도 같은 구조다」라고 짚었던 곳이다. 처리기를 `content`와 상태줄을 함께 감싸는 요소로 옮기고 부록 A-1을 저장소 테스트로 더한다 | `crates/keyscore-app/src/window.rs:829-840`, `crates/keyscore-app/src/window.rs:892` |
| 2 | 수정 | 처리기가 `MouseButton::Left`만 본다. 주석은 GPUI `track_focus`가 하던 일과 같다고 하지만 `track_focus`의 마우스다운은 버튼을 가리지 않는다(gpui-pre `elements/div.rs:2776`). 인스펙터 빈 자리·아래 패널을 오른쪽 버튼으로 누르면 포커스가 입력란에 남는다(`Zzm`). `main`에서는 같은 시도가 문서로 돌아온다: 회귀. `on_any_mouse_down`으로 바꾸고 부록 A-2를 더한다 | `crates/keyscore-app/src/window.rs:832` |
| 3 | 제안 | 정지 중 없는 이름 포트의 테스트 노트는 「출력 포트 「X」이 없어 그 트랙은 소리 내지 않는다」 다음에 「출력 포트 「X」을 열지 못해 테스트 노트를 보내지 못했다」를 남긴다. 이유가 「없음」인데 마지막 메시지(상태줄에 보이는 것)는 「열지 못해」라 이유와 어긋난다. 「보내지 못했다」만 남기거나 이유를 따르는 말로 한다 | `crates/keyscore-app/src/inspector.rs:386-388` |

리뷰 1 지적 2(넘침)와 지적 3(다른 창 재생 중 프로그램 체인지)은 뿌리에서 고쳐졌다. 지적 3은 뱅크 선택까지 함께 빠지는 것(시도 13)과 주인이 풀린 뒤 원래대로 돌아오는 것(시도 14)을 확인했다. 제안 4·5도 반영됐다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2를 고치고 부록 A-1·A-2를 저장소 테스트로 더한 뒤 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t5c_r2`: 통과 13, 깨진 2개와 참고 1개는 부록. 회귀 판단은 `main` 임시 worktree에서 같은 시도를 돌려 확인)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음, `cargo deny check` 통과)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 깨진 테스트

아래는 `review_s2_t5c_r2.rs`의 도우미(`start`, `open`, `render`, `click`, `press`, `type_text`, `shown`, `project_of`, `two_tracks`, `dirty_name`, `dirty_tempo`, `bounds`, `undo_len`)를 그대로 쓴다.

### A-1. 상태줄 클릭(지적 1)

~~~rust
/// 상태줄 빈 자리를 눌러도 문서로 포커스가 돌아와야 한다. `main`(S2-T5b)에서는 상태줄이 겹 안이라 돌아왔다.
#[gpui_kit::test]
fn clicking_status_bar_returns_focus_to_document(cx: &mut TestAppContext) {
    start(cx, "status-click");
    let (handle, view) = open(cx, two_tracks());
    dirty_tempo(cx, handle, "95");
    click(cx, handle, "status-bar");
    press(cx, handle, "m");
    assert!(
        project_of(cx, &view).tracks()[0].mute(),
        "상태줄을 누른 뒤에도 포커스가 템포 입력란에 남는다: tempo={:?}",
        shown(cx, handle, "transport-tempo")
    );
}
~~~

~~~
---- clicking_status_bar_returns_focus_to_document stdout ----
thread 'clicking_status_bar_returns_focus_to_document' panicked at crates/keyscore-app/tests/review_s2_t5c_r2.rs:180:5:
상태줄을 누른 뒤에도 포커스가 템포 입력란에 남는다: tempo=Some("95m")
~~~

같은 테스트를 `main`의 임시 worktree에서 돌리면 통과한다(`test main_status_bar ... ok`).

### A-2. 오른쪽 버튼 클릭(지적 2)

~~~rust
/// 인스펙터 빈 자리를 오른쪽 버튼으로 눌러도 문서로 돌아와야 한다(GPUI `track_focus`는 모든 버튼에서 옮긴다).
#[gpui_kit::test]
fn right_clicking_inspector_background_returns_focus(cx: &mut TestAppContext) {
    start(cx, "inspector-right");
    let (handle, view) = open(cx, two_tracks());
    dirty_name(cx, handle, "Zz");
    cx.update_window(handle, |_, window, cx| window.right_click("inspector", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "m");
    assert!(
        project_of(cx, &view).tracks()[0].mute(),
        "오른쪽 클릭 뒤에도 포커스가 이름 입력란에 남는다: name={:?}",
        shown(cx, handle, "inspector-name")
    );
}
~~~

~~~
---- right_clicking_inspector_background_returns_focus stdout ----
thread 'right_clicking_inspector_background_returns_focus' panicked at crates/keyscore-app/tests/review_s2_t5c_r2.rs:198:5:
오른쪽 클릭 뒤에도 포커스가 이름 입력란에 남는다: name=Some("Zzm")
~~~

`main`에서는 템포 입력란을 고치다 인스펙터 자리표시자(`inspector`)를 오른쪽 클릭하는 같은 시도가 통과한다(`test main_right_click_inspector ... ok`).

### A-3. 경계 끌기(참고, 지적 아님)

~~~rust
/// 인스펙터와 주 뷰 사이 경계를 끌어 크기를 바꿔도 고치던 이름이 확정되지 않는다.
#[gpui_kit::test]
fn dragging_panel_handle_with_dirty_name(cx: &mut TestAppContext) {
    start(cx, "drag-handle");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Zz");
    let b = bounds(cx, handle, "inspector");
    let from = gpui_kit::point(b.origin.x - gpui_kit::px(1.), b.origin.y + gpui_kit::px(200.));
    let to = gpui_kit::point(from.x - gpui_kit::px(40.), from.y);
    cx.update_window(handle, |_, window, cx| window.drag(from, to, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "Lead", "끌기 뒤 Enter가 이름을 확정했다");
    assert_eq!(undo_len(cx, &view), len);
}
~~~

~~~
---- dragging_panel_handle_with_dirty_name stdout ----
thread 'dragging_panel_handle_with_dirty_name' panicked at crates/keyscore-app/tests/review_s2_t5c_r2.rs:358:5:
assertion `left == right` failed: 끌기 뒤 Enter가 이름을 확정했다
  left: "Zz"
~~~

`main`에서 템포 입력란으로 같은 끌기를 하면(인스펙터 너비 260 → 301로 실제로 바뀜) 역시 고치던 템포가 남아 Enter로 확정된다. 경계 끌기가 입력을 버리지 않는 것은 기존 동작이고 잘못이라 보기도 어렵다. 지적하지 않는다.
