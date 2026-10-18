# 적대적 리뷰: S2-T5c 트랙 인스펙터와 상태줄 (리뷰 3회차)

- 리뷰어 세션: 구현 세션·리뷰 1·리뷰 2 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 65d27e2(브랜치 `s2/t5c-inspector-status`, 수정 커밋 551e7f4, `git diff main...HEAD`)
- 깨뜨리는 시도 파일: `crates/keyscore-app/tests/review_s2_t5c_r3.rs`(통과한 시도 7개). 깨진 시도 3개는 부록 A에 있다. `main` 대조는 임시 worktree(`/tmp/ks-main-r3`, 지움)에서 했다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

수정 커밋(`git show 551e7f4`), `window.rs`의 `focus_document_on_press`·Render, `track_list.rs`의 목록 처리기, gpui-pre `elements/div.rs`(`track_focus` 마우스다운 2774–2786, `on_any_mouse_down` 162–172), gpui-component `Button`(`button.rs:827-845`)·`Input`(`input.rs:744-753`)·`Toggle`을 읽고 테스트를 열기 전에 적었다.

1. 리뷰 2 지적 1·2가 뿌리에서 고쳐졌는가: 상태줄(트랙 글자, 메시지, 빈 자리)과 `content`가 모든 버튼에서 문서로 포커스를 돌리는가. 가운데 버튼도.
2. 처리기는 `default_prevented`면 건너뛴다. `Button`은 왼쪽 버튼에서만 `prevent_default`로 포커스를 피하고, 오른쪽 버튼에서는 자기 `track_focus`가 포커스를 가져간다. 겹 바깥(트랙 목록·인스펙터)의 버튼·토글을 오른쪽 클릭하면 포커스가 겹 바깥 버튼에 남아 모드 키가 죽는가. `main`에서는 트랙 목록이 겹 안이라 버튼이 포커스를 가져가도 모드 키가 살았다.
3. 트랙 목록 자체 처리기는 여전히 `MouseButton::Left`만 본다. 줄의 빈 자리 오른쪽 클릭은 `content` 처리기가 받는가.
4. 상태줄 모드 표시: 왼쪽 클릭은 처리기의 포커스 이동 뒤에도 메뉴가 열리고 고를 수 있는가(고치던 템포는 확정되지 않는가). 오른쪽 클릭은 메뉴를 열지 않고 문서로 돌아오는가.
5. 입력란 오른쪽 클릭: `Input`의 `track_focus`가 먼저 받아 바깥 처리기가 빼앗지 않는가.
6. 테스트 노트 메시지 합치기: 없는 이름 포트에서 이유와 결과가 한 메시지로, 마지막에 남는가. 다른 경로(재생 중, 다른 창 재생 중, 가상 포트)는 그대로인가.
7. 다시 쓴 `tap_tempo_sets_tempo_from_intervals`가 공허하지 않은가(탭이 무시돼 기본 120 BPM이 남아도 통과하는가).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 상태줄·모든 버튼 | 있음(`review_s2_t5c_r2.rs`의 상태줄 왼쪽 클릭, 인스펙터 오른쪽 클릭) | 가운데 버튼, 상태줄 트랙 글자를 보탬(통과) |
| 2 겹 바깥 버튼 오른쪽 클릭 | 없음 | 깨짐(지적 1, 부록 A) |
| 3 트랙 목록 줄 오른쪽 클릭 | 없음 | 보탬(통과) |
| 4 모드 표시 | 왼쪽 클릭+고친 이름(`mode_menu_click_with_dirty_name`) | 고친 템포, 오른쪽 클릭을 보탬(통과) |
| 5 입력란 오른쪽 클릭 | 없음 | 보탬(통과) |
| 6 테스트 노트 메시지 | 있음(`test_note_missing_named_port_reports_reason`) | 코드 대조로 확인. 다른 경로는 바뀌지 않았다 |
| 7 탭 템포 | 있음 | 아래 시도 9 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 트랙 목록 뮤트 토글 `M` 오른쪽 클릭 → `m` | 깨짐(키 컨텍스트 `[Root]`, 뮤트 안 됨. `main`에서 같은 시도 통과: 회귀) | 부록 A-1 |
| 2 | 트랙 목록 `+` 버튼 오른쪽 클릭 → `m` | 깨짐(`[Root]`. `main` 통과: 회귀) | 부록 A-2 |
| 3 | 인스펙터 테스트 노트 버튼 오른쪽 클릭 → `m` | 깨짐(`[Root]`. 새 UI라 `main` 대응 없음, 원인은 1·2와 같다) | 부록 A-3 |
| 4 | 트랙 목록 줄 오른쪽 클릭(고친 이름) → `m` | 못 깨뜨림(`content` 처리기가 받는다, 이름 미확정) | `right_click_track_row_returns_focus` |
| 5 | 상태줄 트랙 글자를 가운데 버튼으로 누름(고친 템포) → `m` | 못 깨뜨림 | `middle_click_status_bar_returns_focus` |
| 6 | 템포 입력란에 `3` 고치는 중 상태줄 트랙 글자 클릭 → `m`: 카운트로 새지 않고 편집 하나 | 못 깨뜨림 | `status_click_then_mute_once` |
| 7 | 고친 템포 → 모드 표시 왼쪽 클릭 → STEP 고르기 / 고친 이름 → 모드 표시 오른쪽 클릭 | 못 깨뜨림(메뉴가 열리고 고른다, 템포 미확정 / 메뉴 안 열리고 문서로 돌아옴) | `status_mode_click_with_dirty_tempo_opens_menu`, `right_click_status_mode_returns_focus_without_menu` |
| 8 | 고친 템포 → 인스펙터 이름 입력란 오른쪽 클릭 → 글자 | 못 깨뜨림(입력란이 포커스를 받고 `Qm`이 들어감, 뮤트 없음) | `right_click_inspector_input_takes_focus` |
| 9 | 오른쪽 클릭으로 포커스를 받은 뮤트 토글에서 `space` | 못 깨뜨림(토글이 눌리지 않는다. 다만 재생도 안 된다: 지적 1과 같은 증상) | `right_click_mute_toggle_then_space` |
| 10 | `tap_tempo_sets_tempo_from_intervals`를 혼자 세 번 실행 | 세 번 모두 통과. 공허성: 탭이 무시되면 기본 120 BPM이 남는데, 상한은 `60/shortest + 0.1`이고 `shortest`는 500 ms 잠 세 번과 가운데 클릭 두 번의 처리 시간을 합친 것의 1/3이라 120.0이 통과하려면 클릭 두 번이 합쳐 약 1.25 ms 안에 끝나야 한다. 리뷰 2에서 잰 클릭 처리 시간(수십 ms)으로는 일어나지 않는다. 간격 수를 잘못 세면(÷2, ÷4) 범위 밖이다. 의미 있는 검사다 | 없음(기존 테스트 실행) |

`main` 대조(임시 worktree, 같은 도우미): `track-mute-0`·`track-add`·`track-row-0` 오른쪽 클릭 뒤 `m`은 통과, `transport-play` 오른쪽 클릭 뒤 `m`은 `main`에서도 실패(`[Root]`)했다. 트랜스포트 바는 `main`에서도 겹 바깥이었으므로 그쪽은 기존 동작이다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 인스펙터 편집 하나 = undo 한 단위 | 아니오 | 시도 6, 리뷰 2 시도 6–9 |
| 고친 글자가 엉뚱한 때에 확정되지 않음 | 아니오 | 시도 4·7·8 |
| 겹 바깥을 누른 뒤 모드 키가 산다 | 예(오른쪽 버튼으로 버튼·토글을 누를 때) | 시도 1–3 |
| 다른 창의 재생 소리 상태를 테스트 노트가 바꾸지 않음 | 아니오(이번 수정은 정지 중 경로의 메시지만 바꿨다) | `inspector.rs:345-388` |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| UI-12 | 충족 | `crates/keyscore-app/src/inspector.rs:484-628` |
| UI-11 | 충족 | `inspector.rs:191-297` |
| UI-5 | 충족(이 Task 범위. 길이 코드·옥타브는 S5) | `window.rs:647-706` |
| 7.1 화면 구성(상태줄) | 충족 | `window.rs:647-706` |
| 7.1 화면 구성(겹 바깥을 눌러 문서로 돌아오기) | 부분 충족 | 상태줄·빈 자리·모든 버튼은 고쳐졌으나 겹 바깥 버튼·토글 오른쪽 클릭에서 모드 키가 죽는다(지적 1). `crates/keyscore-app/src/track_list.rs:369-372`, `crates/keyscore-app/src/window.rs:639-644` |
| SEQ-7 | 충족 | `inspector.rs` `channel_label` |
| PRE-3 | 충족 | `inspector.rs:300-406`, `review_s2_t5c_r2.rs` 테스트 노트 시도 |
| KM-6 | 충족 | `status_bar.rs` 테스트 |
| KM-14 | 충족 | 리뷰 1 시도 1 |
| 7.6 인스펙터 행 | 충족 | `inspector.rs:484-628` |
| 11절 `Select` GM 이름 검색 | 충족 | 리뷰 2 시도 8 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이번 수정은 UI 스레드의 처리기와 메시지만 바꿨다).
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 없다.
- 에러가 삼켜지는 곳: 없음. 정지 중 없는 이름 포트는 이유(없음·열기 실패·상한)와 결과를 한 메시지로 알리고 끝난다(`inspector.rs:370-388`). `prepare`가 실패하면 `?`로 `Err`가 되어 「테스트 노트를 보내지 못했다: …」로 알린다.
- 자동 게이트: `cargo fmt --all -- --check` 통과, `cargo clippy --workspace --all-targets -- -D warnings` 통과, `cargo test --workspace` 실패 0(이 리뷰 파일 포함).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 겹 바깥의 버튼·토글을 오른쪽 클릭하면 포커스가 그 버튼에 남아 모드 키가 모두 죽는다(키 컨텍스트 `[Root]`). gpui-component `Button`·`Toggle`은 포커스 요소이고, 왼쪽 버튼에서만 `prevent_default`로 포커스를 피한다. 오른쪽 버튼에서는 자기 `track_focus`가 포커스를 가져가고 `prevent_default`하므로, `focus_document_on_press`(`default_prevented`면 건너뜀)도 트랙 목록 처리기(`MouseButton::Left`만)도 문서로 돌리지 않는다. `main`에서는 트랙 목록이 겹 안이라 `M`·`S`·`+`를 오른쪽 클릭해도 모드 키가 살았다(부록 A-1·A-2가 `main`에서 통과): 이 Task가 트랙 목록을 겹 밖으로 옮기며 생긴 회귀다. 인스펙터 버튼(테스트 노트·색·포트·채널·뮤트·솔로)도 같은 원인이다(부록 A-3). 사용자는 트랙 줄의 `M`을 실수로 오른쪽 클릭한 뒤 `space`·`m`·모드 키가 아무 일도 하지 않는 것을 겪고, 주 뷰를 눌러야 돌아온다. 고치는 길 예: 트랙 목록 처리기를 `on_any_mouse_down`으로 바꾸고(이미 `default_prevented`를 보지 않으므로 자식 버튼이 포커스를 가져간 뒤에도 문서로 돌린다), 인스펙터 버튼 묶음에도 같은 처리기를 단다. 또는 겹 바깥 버튼을 포커스 받지 않게 한다. 부록 A-1–A-3을 저장소 테스트로 더한다 | `crates/keyscore-app/src/track_list.rs:369-372`, `crates/keyscore-app/src/window.rs:639-644`, `crates/keyscore-app/src/inspector.rs:546`, `crates/keyscore-app/src/inspector.rs:582` |
| 2 | 제안 | 트랜스포트 바 버튼(`transport-play` 등)을 오른쪽 클릭해도 같은 증상이다. `main`에서도 같으므로(트랜스포트 바는 S2-T5b부터 겹 밖) 이 Task의 회귀는 아니다. 지적 1을 고치는 김에 같은 처리기로 덮을 수 있다 | `crates/keyscore-app/src/window.rs:838-843` |

리뷰 2 지적 1(상태줄)·지적 2(모든 버튼)는 뿌리에서 고쳐졌다(시도 4–7). 제안 3(테스트 노트 메시지)도 반영됐고, 다시 쓴 탭 템포 테스트는 공허하지 않다(시도 10).

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1을 고치고 부록 A-1–A-3을 저장소 테스트로 더한다. 이 리뷰가 프로토콜 [8]의 세 번째 재리뷰이므로, 판정이 나지 않은 이 상태는 「3회 재리뷰에도 판정이 안 나면 결정 요청으로 전환」에 따라 처리한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t5c_r3`: 통과 7, 깨진 3개는 부록. 회귀 판단은 `main` 임시 worktree에서 같은 시도를 돌려 확인)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`transport_bar.rs` 테스트 수정은 리뷰 2가 짚은 시간 의존 실패에 대한 것이고 검사 내용은 같다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 깨진 테스트

아래는 `review_s2_t5c_r3.rs`의 도우미(`start`, `open`, `right_click`, `project_of`, `two_tracks`, `mute_after`)와 다음 도우미를 쓴다.

~~~rust
fn ctx(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    cx.update_window(handle, |_, window, _| format!("{:?}", window.context_stack()))
        .unwrap()
}
~~~

### A-1. 트랙 목록 뮤트 토글 오른쪽 클릭(지적 1, 회귀)

~~~rust
/// 트랙 목록 뮤트 토글을 오른쪽 클릭한 뒤에도 모드 키가 먹어야 한다. `main`에서는 통과한다.
#[gpui_kit::test]
fn right_click_track_mute_toggle_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "r-mute");
    let (handle, view) = open(cx, two_tracks());
    right_click(cx, handle, "track-mute-0");
    assert!(!project_of(cx, &view).tracks()[0].mute(), "오른쪽 클릭이 토글했다");
    let c = ctx(cx, handle);
    assert!(mute_after(cx, handle, &view), "오른쪽 클릭 뒤 모드 키가 죽었다: context={c}");
}
~~~

~~~
---- right_click_track_mute_toggle_keeps_mode_keys stdout ----
thread 'right_click_track_mute_toggle_keeps_mode_keys' panicked at crates/keyscore-app/tests/review_s2_t5c_r3.rs:217:5:
오른쪽 클릭 뒤 모드 키가 죽었다: context=[Root]
~~~

`main` worktree에서 같은 시도(`main_right_mute_toggle`): `ok`.

### A-2. 트랙 목록 `+` 버튼 오른쪽 클릭(지적 1, 회귀)

~~~rust
/// 트랙 목록 `+` 버튼을 오른쪽 클릭한 뒤에도 모드 키가 먹어야 한다. `main`에서는 통과한다.
#[gpui_kit::test]
fn right_click_track_add_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "r-add");
    let (handle, view) = open(cx, two_tracks());
    right_click(cx, handle, "track-add");
    let c = ctx(cx, handle);
    assert!(mute_after(cx, handle, &view), "오른쪽 클릭 뒤 모드 키가 죽었다: context={c}");
}
~~~

~~~
---- right_click_track_add_keeps_mode_keys stdout ----
thread 'right_click_track_add_keeps_mode_keys' panicked at crates/keyscore-app/tests/review_s2_t5c_r3.rs:227:5:
오른쪽 클릭 뒤 모드 키가 죽었다: context=[Root]
~~~

`main` worktree에서 같은 시도(`main_right_track_add`): `ok`.

### A-3. 인스펙터 버튼 오른쪽 클릭(지적 1, 같은 원인)

~~~rust
/// 인스펙터 테스트 노트 버튼 오른쪽 클릭 뒤 모드 키.
#[gpui_kit::test]
fn right_click_inspector_button_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "r-test");
    let (handle, view) = open(cx, two_tracks());
    right_click(cx, handle, "inspector-test");
    let c = ctx(cx, handle);
    assert!(mute_after(cx, handle, &view), "오른쪽 클릭 뒤 모드 키가 죽었다: context={c}");
}
~~~

~~~
---- right_click_inspector_button_keeps_mode_keys stdout ----
thread 'right_click_inspector_button_keeps_mode_keys' panicked at crates/keyscore-app/tests/review_s2_t5c_r3.rs:237:5:
오른쪽 클릭 뒤 모드 키가 죽었다: context=[Root]
~~~

참고: `main`에서 `transport-play` 오른쪽 클릭 뒤 `m`도 `[Root]`로 실패한다(지적 2, 기존 동작).
