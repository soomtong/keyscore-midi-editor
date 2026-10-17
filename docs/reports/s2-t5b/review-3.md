# 적대적 리뷰: S2-T5b 트랜스포트 바와 트랙 목록 (리뷰 3회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않은 적대적 리뷰어). 1·2회차 리뷰어와도 다른 세션
- 대상 커밋: 75c7898(브랜치 `s2/t5b-transport-bar`, 범위 `git diff main...HEAD`, 리뷰 2 대응은 `git show 75c7898`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `edit_undo`·`edit_redo`(window.rs:287-325)는 트랜스포트 필드에 포커스가 있을 때만 `propagate`한다. 창 안의 다른 입력란(명령 팔레트 질의)은 그대로 문서 undo로 간다. main에는 이 처리기가 없었으므로(`git show main:crates/keyscore-app/src/window.rs`에 `Undo` 없음) 이 Task가 만든 변화인지 본다.
2. `propagate`는 같은 키의 다음 바인딩(gpui-base `Input`의 `Undo`)으로 떨어질 때만 뜻이 있다. 메뉴 막대의 「되돌리기」는 `edit::Undo`를 포커스로 디스패치할 뿐 다음 바인딩이 없다. 보고서의 「메뉴의 되돌리기도 같다」가 사실인지.
3. 조성 메뉴(transport_bar.rs:316-318)와 포트 메뉴(362-370)는 지금 값과 비교하지 않고 편집한다. 리뷰 2 지적 5가 「조성 메뉴에서 지금 조성을 고를 때도 같은 경로인지 함께 본다」고 적은 곳이다.
4. 「고친 필드」는 `shown`(마지막으로 앱이 넣은 글자)과 비교한다. 고친 동안에는 `shown`이 갱신되지 않으므로, 그 사이 문서가 바뀌고 사람이 옛 `shown`과 같은 글자를 치면 「고치지 않음」으로 판정돼 확정이 버려진다.
5. Blur에서 필드를 되돌린다. 시험 창은 활성이 아니면 gpui가 포커스 사건을 내지 않는다(gpui window.rs:3416-3426, 이전 포커스 경로를 비운다). 기존 테스트가 Blur 경로를 한 번도 돌리지 않았을 수 있다. 활성 창에서 기존 테스트가 그대로 통과하는지.
6. 버튼·토글 뒤 포커스 복귀: 입력란을 고치던 중 버튼·토글을 누르면 입력은 버려지고 포커스는 문서로 오는가. 꺼진 버튼(녹음·AI)은 포커스를 가져가는가.
7. `Esc` 가로채기(transport_bar.rs:156-168): 두 필드, 두 프리셋. 수식키 붙은 `Esc`는 가로채지 않는가.
8. 모드 변경 시 포커스 이동(window.rs:182-187): `ctrl-tab`, 상태줄 모드 메뉴.
9. 같은 값 확정(`120`, `4/4`)은 편집을 만들지 않는가. 입력란 `cmd-z`로 원래 글자로 돌아온 뒤 Enter는.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 팔레트 질의의 `cmd-z` | 없음 | |
| 2. 입력란 포커스에서 메뉴 되돌리기 | 없음 | r2 `enter_after_menu_undo_while_field_focused`는 전제를 바꿔 `SetTempo`로 본다 |
| 3. 조성·포트 메뉴에서 지금 값 | 없음 | |
| 4. 고친 필드 + 문서 변경 + 옛 값과 같은 글자 | 없음 | `typed_text_survives_document_change`는 다른 글자 |
| 5. 활성 창의 Blur | 없음 | 모든 시험 창이 비활성. `InputEvent::Blur`가 한 번도 나지 않는다 |
| 6. 고치던 입력란에서 버튼 | 없음 | `buttons_keep_document_focus`는 문서 포커스에서 시작 |
| 7. `Esc` 두 필드·두 프리셋 | 부분 | 템포·기본 프리셋·포커스만 |
| 8. 모드 메뉴 | 없음 | `ctrl-tab`은 모드만 확인 |
| 9. 같은 값 확정, 입력란 undo 뒤 Enter | 부분 | 고치지 않은 Enter만(`enter_without_change_makes_no_undo_unit`) |

## 3. 깨뜨리기 시도

모두 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t5b_r3`). 키는 `simulate_keystrokes`, 클릭은 `window.click`이다. 이번 회차의 `open`은 창을 활성으로 둔다(시도 5). 깨진 것은 부록에 코드로 싣는다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 트랙 지움 → `cmd-shift-p`로 팔레트, 질의에 `x` → `cmd-z` | 깨짐: 지운 트랙이 돌아온다. 질의 글자는 `x` 그대로. `edit_undo`가 트랜스포트 필드만 보고(window.rs:295) 팔레트 질의는 문서 undo로 간다. main에는 `edit::Undo` 처리기가 없어 질의의 `Input`이 받았다(이 Task가 만든 회귀) | 부록 C1 `cmd_z_in_palette_query_does_not_undo_document` |
| 2 | 트랙 지움 → 템포 입력란 `cmd-a 9` → `edit::Undo`를 포커스로 디스패치(메뉴 막대와 같은 경로) | 깨짐: 입력란은 `9` 그대로, 문서도 그대로. 메뉴 「되돌리기」가 아무 일도 하지 않는다. `propagate`는 키 디스패치에서만 다음 바인딩(`Input`의 `Undo`)으로 떨어진다. `edit::Redo`도 같다(문서 그대로 확인) | 부록 C2 `menu_undo_in_dirty_field_undoes_field_text` |
| 3 | 트랙 지움 → 조성 메뉴에서 지금 조성(`조성 없음`, 항목 0) → `edit::Undo` | 깨짐: undo 길이 1 → 2. 첫 undo가 같은 값 조성 편집을 되돌리고 트랙은 돌아오지 않는다 | 부록 C3 `key_menu_same_key_makes_no_undo_unit` |
| 4 | 트랙 지움 → 포트 메뉴에서 지금 포트(가상 포트, 항목 0) → `edit::Undo` | 깨짐: 시도 3과 같다 | 부록 C4 `port_menu_same_port_makes_no_undo_unit` |
| 5 | 기존 `review_s2_t5b`·`review_s2_t5b_r2`·`transport_bar`·`track_list`의 `open`에 `activate_window`만 더한 사본 실행(사본은 지웠다) | 못 깨뜨림(20·15·6·5 모두 통과). 다만 비활성 창에서는 고치던 필드에서 버튼을 눌러도 필드가 `9`를 보인 채 남는 것을 관찰했다. 테스트 환경 탓이고 활성 창에서는 문서 값으로 돌아온다(제안 3) | `leaving_dirty_field_by_click_reverts_it` |
| 6 | 템포 입력란 `cmd-a 9` → `cmd-z`(입력란 `120.0`, 문서 그대로) → `cmd-shift-z`(`9`) → `cmd-z enter` → `edit::Undo` | 못 깨뜨림(입력란 undo/redo, 되돌린 뒤 Enter는 편집 없음, 문서 undo는 지운 트랙을 되돌림) | `cmd_z_and_cmd_shift_z_edit_field_text_only` |
| 7 | `t 9 0 enter` → `cmd-z` → 입력란 `cmd-a 7 escape` → `cmd-shift-z` | 못 깨뜨림(입력란을 떠난 뒤 문서 undo/redo, 필드 동기화) | `cmd_z_after_leaving_field_undoes_document` |
| 8 | 두 프리셋에서 `t 9 9 escape`, 박자표 `cmd-a 3 / 4 escape`, 그 뒤 모드 키 | 못 깨뜨림 | `escape_cancels_both_fields_both_presets` |
| 9 | `shift-escape` | 못 깨뜨림(가로채지 않고 입력란에 머문다) | `shift_escape_is_not_cancel` |
| 10 | 트랙 지움 → 템포 `cmd-a 1 2 0 enter`, 박자표 `cmd-a 4 / 4 enter` → `edit::Undo` | 못 깨뜨림(같은 값 확정은 편집 없음) | `same_value_commit_makes_no_undo_unit` |
| 11 | 포커스만 있는 템포 입력란에서 `SetTempo(100)` → `cmd-a 1 2 0 enter` | 못 깨뜨림(필드가 100.0을 따라가고, 친 120이 확정) | `untouched_focused_field_follows_then_commits_typed` |
| 12 | 박자표 고치는 중 `SetTempo(100)` | 못 깨뜨림(템포 필드만 따라감, 박자표 확정) | `tempo_field_follows_while_meter_dirty` |
| 13 | 메트로놈 두 번, 고치던 입력란에서 메트로놈, 각 뒤 `m` | 못 깨뜨림 | `metronome_twice_and_from_field_keeps_mode_keys` |
| 14 | 입력란에서 재생·정지·패닉·탭 버튼, 꺼진 녹음·AI 버튼, 포트 메뉴 선택 뒤 `m` | 못 깨뜨림 | `buttons_from_field_return_to_document`, `disabled_buttons_keep_document_focus`, `port_menu_returns_focus` |
| 15 | 고치던 입력란에서 `ctrl-tab`, 상태줄 모드 메뉴 | 못 깨뜨림(입력 취소, 포커스 문서, Step의 `Esc`가 Normal로) | `ctrl_tab_from_dirty_field_cancels_and_leaves`, `mode_menu_from_field_leaves_field` |
| 16 | 템포 `cmd-a 9`(고침) → `SetTempo(100)` → `cmd-a 1 2 0 . 0 enter` | 깨짐: 친 `120.0`이 옛 `shown`과 같아 「고치지 않음」으로 판정되고 확정이 버려진다(템포 100 유지). 지금 UI에는 입력란 포커스를 둔 채 템포를 바꾸는 경로가 없어(메뉴 되돌리기는 시도 2로 막혔고 탭·팔레트는 포커스를 가져간다) 제안으로 둔다(제안 1) | 부록 C5 `typed_old_value_after_document_change_commits` |
| 17 | 템포 `cmd-a 9` 뒤 다른 창을 열어 활성화했다가 돌아옴 | 기록: 포커스는 입력란에 남지만 글자는 `120.0`으로 되돌려진다(창 비활성화도 Blur). 제안 2 | 없음(진단만) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 트랙·트랜스포트 편집은 각각 undo 한 단위, 빈 단위 없음 | 예 | 조성·포트 메뉴의 지금 값(transport_bar.rs:316-318, 362-370). 시도 3·4 |
| 텍스트 입력 중 `cmd-z`는 그 입력란의 undo(KM-14) | 예 | 팔레트 질의(window.rs:287-325). 시도 1 |
| 필드 표시 = 문서 값(입력 중이 아닐 때) | 아니오 | 시도 5·11·12 |
| 문서 포커스에서 모드 키가 산다(버튼·토글·메뉴 뒤) | 아니오 | 시도 13·14·15 |
| 현재 트랙은 문서에 있는 트랙이거나 없음 | 아니오 | 변경 없음 |
| 채널 1–16 | 아니오 | 변경 없음 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1(트랜스포트 버튼) | 충족 | transport_bar.rs:408-445. 시도 14 |
| TRN-2(메트로놈 토글) | 충족 | transport_bar.rs:482-493. 시도 13(리뷰 2 지적 1 해소) |
| TRN-4(템포·박자표·조성 필드) | 부분 충족 → 미충족 | 템포·박자표 필드는 충족(시도 6-12). 조성 메뉴의 지금 조성 선택이 빈 undo 단위를 만든다(시도 3) |
| KM-14(텍스트 입력 중 `Cmd+Z`/`Cmd+Shift+Z`) | 부분 충족 → 미충족 | 트랜스포트 입력란의 키는 충족(시도 6). 팔레트 질의는 문서를 되돌린다(시도 1, 이 Task의 회귀). 입력란에서 메뉴 「되돌리기·다시 하기」는 아무것도 하지 않는다(시도 2) |
| UI-11(모든 기능의 보이는 UI) | 부분 충족 → 미충족 | 입력란 undo의 보이는 UI(편집 메뉴 「되돌리기」)가 입력 중에 죽는다(시도 2). ⏹!·버튼은 충족 |
| SEQ-7·SEQ-8 | 충족 | 변경 없음. 1·2회차 판단 유지 |
| OUT-4 / 7.6 포트 드롭다운 | 부분 충족 → 미충족 | 고르기·포커스는 충족(시도 14). 지금 포트 선택이 빈 undo 단위(시도 4) |
| OUT-6 / 7.6 「패닉: ⏹! 버튼」 | 충족 | transport_bar.rs:438-442 |
| P1·P2 / A13 「Esc는 통상의 취소」 | 충족 | transport_bar.rs:156-179. 시도 8·9(리뷰 2 지적 4 해소) |
| 5.11 Tab | 충족 | 변경 없음 |
| 7.1 상단 트랜스포트 | 충족 | 변경 없음 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 이번 대응에 없음. 바뀐 곳은 모두 메인 스레드 처리기·관찰자다.
- 제품 코드의 `unwrap`/`expect`: 새로 들어온 것 없음.
- 에러가 삼켜지는 곳: 없음. 해석 실패는 `say`로 남는다(transport_bar.rs:227).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정(이 Task의 회귀, KM-14) | 명령 팔레트 질의에서 `cmd-z`·`cmd-shift-z`가 질의 글자가 아니라 문서를 되돌린다(C1). 이 Task가 루트에 `edit::Undo/Redo` 처리기를 더했고, 입력란 판정이 트랜스포트 필드만 본다. main에서는 처리기가 없어 질의 `Input`이 받았다. 「포커스가 텍스트 입력란에 있는가」를 필드 목록이 아니라 일반적으로 판정해야 S2-T5c 인스펙터 입력란에서도 같은 일이 되풀이되지 않는다 | window.rs:287-325, 295, 318; transport_bar.rs:138-145 |
| 2 | 수정(리뷰 2 지적 2 대응이 메뉴 경로에 닿지 않음) | 입력란에 포커스가 있을 때 편집 메뉴 「되돌리기·다시 하기」가 입력란도 문서도 되돌리지 않는다(C2). `propagate`는 키 디스패치의 다음 바인딩으로만 떨어지고 메뉴 디스패치에는 다음 바인딩이 없다. 보고서 「리뷰 2 대응」의 「메뉴의 되돌리기도 같다(macOS 관례)」와 다르다. 입력란에 포커스가 있으면 입력란의 `Undo`/`Redo`를 직접 디스패치하는 식으로 키·메뉴 두 경로가 같게 하고 메뉴 경로를 테스트로 지킨다 | window.rs:287-325 |
| 3 | 수정(리뷰 2 지적 5가 메뉴에 닿지 않음) | 조성 메뉴·포트 메뉴에서 지금 값을 고르면 같은 값 편집이 적용돼 빈 undo 단위와 「바뀜」 표시가 생긴다(C3, C4). 템포·박자표·탭과 같은 기준으로 값이 같으면 편집하지 않는다 | transport_bar.rs:316-318, 362-370 |
| 4 | 제안 | 고친 필드는 `shown`을 갱신하지 않아, 그 사이 문서가 바뀐 뒤 옛 `shown`과 같은 글자를 치면 확정이 버려진다(C5, 시도 16). 지금 UI에서는 닿는 경로가 없다. 지적 2를 고쳐 메뉴 되돌리기가 입력 중에 문서를 건드리지 않는 한 그대로다. 「고침」 판정을 문서 값 표기와의 비교로 바꾸거나 문서가 바뀔 때 고친 필드의 기준만 갱신하는 방법이 있다 | transport_bar.rs:41-44, 116-118, 129-135 |
| 5 | 제안 | 창이 비활성이 되면(다른 앱으로 전환) Blur로 고치던 글자가 문서 값으로 되돌려진다. 포커스는 입력란에 남는다(시도 17). macOS 입력란 관례는 글자를 지킨다. Blur 처리에서 창 비활성화를 구분할지 정하기를 권한다 | transport_bar.rs:197 |
| 6 | 제안 | 시험 창이 비활성이라 기존 테스트에서 `InputEvent::Focus/Blur`가 한 번도 나지 않는다(gpui window.rs:3416-3426). 이번 회차는 활성 창 사본으로 기존 4개 파일이 통과함을 확인했다. 기존 테스트 도우미 `open`도 창을 활성으로 두기를 권한다(`review_s2_t5b_r3.rs`의 `open`) | tests/transport_bar.rs:23-27 외 |
| 7 | 제안 | window.rs:83-85 주석 「gpui-component `Input`은 Esc에서 `window.blur()`를 부른다」는 리뷰 2에서 사실이 아님을 확인했다(gpui-base input/base/state.rs:2079-2116). 주석의 「왜」를 지금 사실로 고치기를 권한다 | window.rs:83-85 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2 지적 1(메트로놈 포커스), 3(고친 필드 판단), 4(Esc), 제안 1·2는 근본까지 닿았고 새 문제를 찾지 못했다(시도 5-15). 지적 2(KM-14)는 트랜스포트 입력란의 키 경로만 고쳐, 같은 처리기가 팔레트 질의를 문서 undo로 끌어오고(지적 1, 이 Task의 회귀) 메뉴 경로를 죽였다(지적 2). 지적 5(빈 undo 단위)는 필드·탭에만 닿고 조성·포트 메뉴에 남았다(지적 3). 셋 다 범위가 좁고 고칠 방향이 정해져 있다. 부록 C1-C4를 테스트 파일로 옮겨 통과시킨 뒤 재리뷰한다. 이 회차가 프로토콜 [8]의 마지막 재리뷰이므로, 다음 판정이 「병합 가능」이 아니면 결정 요청으로 전환한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/12-S2`·`13-S3`의 이월 줄은 계획 문서 갱신)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 C: 깨지는 테스트

`crates/keyscore-app/tests/review_s2_t5b_r3.rs`의 도우미(`start`, `open`(창 활성), `run`, `click`, `click_menu_item`, `press`, `ids`, `tempo`, `shown`, `set_tempo`, `three_tracks`)와 import를 그대로 쓴다. 아래 `undo_len` 도우미를 더한다. 2026-10-17 75c7898에서 실행해 모두 실패했다. C5는 제안 4의 근거다.

실패 출력 요약

~~~
cmd_z_in_palette_query_does_not_undo_document: left: 3, right: 2 (질의는 "x" 그대로)
menu_undo_in_dirty_field_undoes_field_text: left: Some("9"), right: Some("120.0") (트랙 수 2 그대로)
key_menu_same_key_makes_no_undo_unit: left: 2, right: 1 (undo 길이. 바로 undo하면 트랙 대신 조성 편집이 되돌려진다)
port_menu_same_port_makes_no_undo_unit: left: 2, right: 1 (undo 길이)
typed_old_value_after_document_change_commits: left: Tempo(600000), right: Tempo(500000)
~~~

~~~rust
fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

#[gpui_kit::test]
fn cmd_z_in_palette_query_does_not_undo_document(cx: &mut TestAppContext) {
    start(cx, "palette-cmd-z");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    press(cx, handle, "cmd-shift-p x");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some("x")
    );
    press(cx, handle, "cmd-z");
    assert_eq!(
        ids(cx, &view).len(),
        2,
        "팔레트 질의의 cmd-z가 문서를 되돌렸다"
    );
}

#[gpui_kit::test]
fn menu_undo_in_dirty_field_undoes_field_text(cx: &mut TestAppContext) {
    start(cx, "menu-undo-field");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    // 메뉴 막대는 포커스로 `edit::Undo`를 디스패치한다
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 2);
    assert_eq!(
        shown(cx, handle, "transport-tempo").as_deref(),
        Some("120.0"),
        "메뉴 되돌리기가 입력란 글자를 되돌리지 않는다"
    );
}

#[gpui_kit::test]
fn key_menu_same_key_makes_no_undo_unit(cx: &mut TestAppContext) {
    start(cx, "key-same");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    let before = undo_len(cx, &view);
    click(cx, handle, "transport-key");
    click_menu_item(cx, handle, 0); // 지금 조성(조성 없음)
    assert_eq!(undo_len(cx, &view), before);
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 3);
}

#[gpui_kit::test]
fn port_menu_same_port_makes_no_undo_unit(cx: &mut TestAppContext) {
    start(cx, "port-same");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    let before = undo_len(cx, &view);
    click(cx, handle, "transport-port");
    click_menu_item(cx, handle, 0); // 지금 포트(가상 포트)
    assert_eq!(undo_len(cx, &view), before);
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 3);
}

/// 제안 4의 근거. 지금 UI에서는 입력란 포커스를 둔 채 템포를 바꾸는 경로가 없다.
#[gpui_kit::test]
fn typed_old_value_after_document_change_commits(cx: &mut TestAppContext) {
    start(cx, "stale-shown");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a 9");
    run(cx, handle, set_tempo(100.0));
    press(cx, handle, "cmd-a 1 2 0 . 0 enter");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT, "친 120.0이 확정되지 않았다");
}
~~~
