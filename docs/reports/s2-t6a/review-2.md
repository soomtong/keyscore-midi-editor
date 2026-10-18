# 적대적 리뷰: S2-T6a 설정 창과 키맵 편집기 (리뷰 2회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 세션(구현자·1회차 리뷰어와 다른 세션)
- 대상 커밋: 123d895 (`s2/t6-settings-keymap`, 수정 커밋 c7b767d)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show c7b767d`, `keymap/user.rs`, `keymap/mod.rs`(`build`), `settings_window.rs`, GPUI `keymap.rs`(gpui-pre 0.3.8의 `resolve_bindings_for_input`, `bindings_for_action`, `binding_is_unbound`)만 읽고, 테스트와 보고서 「리뷰 1 대응」을 열기 전에 적었다.

1. `build`는 수식키 교환이 켜지면 표준 밖 레이어(사용자 포함)의 키를 바꿔 건다. 편집기가 표준 바인딩을 끄려고 사용자 레이어에 쓰는 `["zed::Unbind", 이름]`도 교환되어 표준 키와 어긋나는가(`cmd-z` → `alt-z`).
2. 캡처는 교환이 켜지면 누른 키를 교환해 파일 표기로 바꾼 뒤 충돌·표준 비교를 한다. 표준 레이어는 교환되지 않으므로 표준 키와의 비교가 뒤집히는가(누른 `cmd-z`는 놓치고, 누른 `alt-z`는 거짓 충돌).
3. 가져오기의 표준 덮기 검사(`overrides`)도 파일 표기로만 비교한다. 교환이 켜져 있으면 같은 문제인가.
4. `UserKeymap::push`는 자리(컨텍스트·키)마다 항목 하나다. 지정 해제가 걸린 자리에 다른 Action을 걸면 지정 해제가 사라진다. 그 Action을 다시 지우면 사용자가 지운 아래 레이어 바인딩이 되살아나는가.
5. `.bak` 정책의 `last_written`은 메모리에만 있다. 앱을 다시 띄운 뒤 첫 편집이 편집기가 쓴 파일을 「사람이 쓴 것」으로 보고 `.bak`의 원본을 덮는가.
6. `zed::Unbind`가 엔진에서 오류 없이 만들어지고(`Unbind(SharedString)` 역직렬화), 같은 키의 다른 Action은 살리는가. 메뉴·팔레트 키(`commands::effective_keys`)에서도 사라지는가.
7. `effective()`가 보이는 실효 바인딩이 그 바인딩 자신의 컨텍스트에서 엔진이 고르는 Action과 같은가(전역 지우기, 같은 자리 덮기, 접두 충돌 「유지」, vim 오버레이 섞어서).
8. 손으로 쓴 파일에 `null`과 `zed::Unbind`가 섞여 있을 때 읽기·복원(`r`)이 그 Action의 지정 해제만 빼는가.
9. 다른 창에서 부른 `keymap::Export`의 전달(`forward`)이 무한 전달 없이 설정 창 처리기에 닿는가. `SwapModifiers`가 설정 창 없이 되는가.
10. `window::Close`의 `defer`: 프로젝트 창 둘에서 `cmd-w`가 지금 창만 닫는가.
11. 시트가 열린 채 구역을 옮기면 시트가 닫히고 아무것도 쓰지 않는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 교환 + 표준 지우기 | 없음 | `brk_capture_with_swap_modifiers_fires_captured_key`는 표준과 무관한 키만 본다 |
| 2 교환 + 표준 키 캡처 | 없음 | |
| 3 교환 + 가져오기 표준 덮기 | 없음 | `brk_import_warns_standard_override`는 교환 꺼짐 |
| 4 지운 키의 부활 | 부분 | `clear_then_bind_same_key_elsewhere`는 덮은 직후만 본다(덮은 것을 다시 지우는 경우 없음) |
| 5 재시작 뒤 `.bak` | 없음 | `brk_bak_keeps_hand_written_original_after_two_edits`는 한 세션 안 |
| 6 `zed::Unbind` 빌드·메뉴 키 | 부분 | 오류 없음은 기존 테스트가 본다. 메뉴 키는 없음 |
| 7 모델 대 엔진 | 없음 | |
| 8 `null`·`zed::Unbind` 섞인 파일 | 없음 | |
| 9 전달·전역 처리기 | 부분 | 팔레트 프리셋만(`brk_palette_select_preset_reaches_handler`) |
| 10 `cmd-w` 프로젝트 창 | 없음 | 설정 창만(`brk_cmd_w_closes_settings_window`) |
| 11 시트 + 구역 전환 | 있음 | `brk_sheet_reachable_after_section_round_trip` |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s2_t6a_r2.rs`(통과한 것) 또는 부록 A(실패한 것, 파일에서 뺐다)에서 실제로 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 교환을 켜고 `edit::Undo`를 `backspace`로 지운다. 목록은 Undo 키 없음, 사용자 파일은 `"cmd-z": ["zed::Unbind", "edit::Undo"]`. 프로젝트 창에서 `cmd-z`는 여전히 `edit::Undo`이고 메뉴 키도 `cmd-z`다(엔진은 지정 해제를 `alt-z`로 건다). 편집기 알림은 「표준 단축키를 지웠다」 | 깨짐 | 부록 A `brk_swap_clear_standard_really_unbinds` |
| 2 | 교환을 켜고 `track::ToggleMute`에 `cmd-z`를 캡처한다. 시트도 표준 경고도 없이 「alt-z를 걸었다」(누른 키가 아닌 파일 표기)로 저장된다. 교환 없이 같은 일을 하면 전역 `cmd-z`와의 충돌 시트가 뜬다. 저장 뒤 프로젝트 창의 `cmd-z`는 여전히 `edit::Undo`라 캡처한 키가 뮤트하지 않는다 | 깨짐 | 부록 A `brk_swap_capture_standard_key_warns` |
| 3 | 교환을 켜고 `alt-z`를 캡처한다(엔진은 `alt-z`로 건다, 표준과 무관). 표준 `edit::Undo`와 충돌한다는 시트가 뜬다. 「유지」하면 「표준 단축키를 덮거나 지웠다」 경고와 함께 목록에서 Undo가 사라지지만 엔진과 메뉴의 `cmd-z`는 그대로 Undo다 | 깨짐 | 부록 A `brk_swap_capture_alt_key_no_false_conflict` |
| 4 | 교환을 켜고 `{"alt-s": "track::ToggleMute"}`(엔진은 `cmd-s`로 건다)를 가져온다. 시트·경고 없이 「키맵을 가져왔다」, 메뉴에서 저장의 `cmd-s`가 사라진다 | 깨짐 | 부록 A `brk_swap_import_warns_standard_override` |
| 5 | 뮤트의 `m`을 지우고, 솔로에 `m`을 걸고, 솔로의 키를 지운다. 사용자 파일에서 뮤트의 지정 해제가 사라져 뮤트에 `m`이 되살아난다 | 깨짐 | 부록 A `brk_cleared_key_not_revived_after_other_action_releases_it` |
| 6 | 손으로 쓴 원본으로 한 번 편집(`.bak` = 원본) → 엔진 재설치(재시작 흉내) → 한 번 더 편집. `.bak`이 편집기 출력으로 덮여 원본 주석이 사라진다 | 깨짐 | 부록 A `brk_bak_survives_restart` |
| 7 | 뮤트·`settings::Open`을 지운다. 파일은 `zed::Unbind`만 쓰고 `null`이 없으며, 엔진 오류는 표준 경고뿐이고, `settings::Open`의 메뉴 키가 없어지며 `cmd-,`는 아무 Action도 내지 않는다 | 못 깨뜨림 | `unbind_entries_build_cleanly_and_hide_only_target` |
| 8 | `"m": ["zed::Unbind", "track::ToggleMute"]`, `"s": null`이 섞인 손 편집 파일에서 뮤트에 `r` → 지정 해제만 빠지고 `null`은 남는다 | 못 깨뜨림 | `hand_written_null_and_unbind_mix_restores` |
| 9 | vim 프리셋에서 전역 `mode::Cycle` 지우기, 뮤트 지우기, 솔로에 `m`(vim `m <표지>`와 접두 충돌, 유지), `cursor::ClipStart`를 `ctrl-alt-g`로 옮기기 뒤, 목록의 모든 실효 바인딩을 그 바인딩 자신의 컨텍스트 경로에서 엔진에 물었다. 모두 같은 Action이 나온다 | 못 깨뜨림 | `editor_model_matches_engine_in_own_context` |
| 10 | 프로젝트 창에서 `keymap::Export`를 부르면 설정 창이 열리고 저장 대화상자 응답으로 파일이 써진다(무한 전달 없음) | 못 깨뜨림 | `export_from_project_window_opens_settings_and_prompts` |
| 11 | 설정 창 없이 프로젝트 창에서 `keymap::SwapModifiers` → 교환이 켜지고 설정 창은 열리지 않는다 | 못 깨뜨림 | `swap_modifiers_without_settings_window` |
| 12 | 프로젝트 창 둘에서 `cmd-w` → 지금 창만 닫히고 남은 창의 `m`은 뮤트다 | 못 깨뜨림 | `cmd_w_closes_only_active_project_window` |
| 13 | 충돌 시트가 열린 채 `ctrl-tab` → 시트가 닫히고, 돌아와 `enter`를 눌러도 파일은 비어 있다 | 못 깨뜨림 | `section_switch_with_sheet_writes_nothing` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 목록의 「지금 걸린 키」 = 엔진이 실제로 내는 키(KM-3) | 예(교환 켜짐) | 시도 1·3: 교환이 켜진 채 표준 바인딩을 지우거나 「유지」로 끄면 목록에서는 사라지고 엔진·메뉴에는 남는다 |
| 같은 경로의 같은 키는 저장 전에 충돌로 묻는다(KM-4) | 예(교환 켜짐) | 시도 2: 표준 키와의 충돌을 놓친다 |
| 사용자가 지운 바인딩은 지운 채로 남는다 | 예 | 시도 5 |
| 편집기가 고쳐 써도 사람이 쓴 원본은 `.bak`에 남는다(리뷰 1 제안 11 대응의 약속) | 예(재시작 뒤) | 시도 6 |
| 편집기 출력은 엔진이 오류 없이 읽는다 | 아니오 | 시도 7·8·9 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| KM-2 | 충족 | 프리셋 전환이 사용자 항목을 남긴다(1회차 테스트), 전역 처리기 `settings_window.rs:149-155` |
| KM-3 | 미충족(부분) | 교환이 켜지면 표준 바인딩 지우기의 표시가 엔진과 어긋난다(시도 1, `keymap/user.rs:262-276`, `keymap/mod.rs:185-189`). 교환 꺼짐에서는 시도 7·9로 충족 |
| KM-4 | 미충족(부분) | 교환이 켜지면 표준 키 충돌을 놓치고(시도 2) 거짓 충돌을 묻는다(시도 3). `settings_window.rs:418-451`. 교환 꺼짐에서는 충족 |
| KM-6 | 충족 | `keymap/mod.rs:498-518` |
| KM-8 | 충족 | 위 줄 키(`assets/keymaps/default.json:503-511`), 시트 닫기(시도 13), `cmd-w`(시도 12) |
| KM-9 | 충족 | 전달(시도 10), 적용 전 충돌·표준 검사(`settings_window.rs:607-633`). 교환 켜짐의 표준 검사 누락은 KM-12에 센다 |
| KM-11 | 미충족(부분) | 교환과 편집기의 표준 비교·지정 해제가 어긋난다(시도 1-4) |
| KM-12 | 미충족(부분) | 교환이 켜지면 캡처(시도 2)와 가져오기(시도 4)가 표준 덮기를 알리지 않고, 시도 1·3은 실제로 일어나지 않은 일을 경고한다 |
| 7.5 `Cmd+,` | 충족 | `settings_window.rs:134-142` |
| 7.6 설정 > 키맵 | 충족 | |
| P1 | 충족 | 시도 10·11, 1회차 `brk_preset_and_swap_by_keyboard_only` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 UI·키맵 편집만 건드린다).
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 생긴 것 없음(`unwrap_or_default`뿐).
- 에러가 삼켜지는 곳: `forward`의 `handle.update` 결과(`settings_window.rs:168-175`). 창이 막 닫힌 경우뿐이라 무해. `lib.rs:57-61`의 지연 닫기 결과도 같다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 수식키 교환(KM-11)과 표준 레이어의 키 공간이 섞인다. `build`는 사용자 레이어(편집기가 쓴 `zed::Unbind` 포함)를 교환해 걸지만 표준 레이어는 그대로 건다. 그런데 편집기는 (a) 표준 바인딩을 끌 때 표준 키를 그대로 지정 해제로 쓰고(엔진은 `alt-z`로 건다, 시도 1·3), (b) 캡처한 키를 교환한 파일 표기로 표준·충돌 비교를 한다(누른 `cmd-z`는 표준 충돌을 놓치고 캡처한 키가 동작하지도 않으며(시도 2), 누른 `alt-z`는 거짓 충돌과 거짓 경고(시도 3)), (c) 가져오기의 표준 덮기 검사도 파일 표기로 한다(시도 4, 저장의 `cmd-s`가 경고 없이 사라진다). 표준 레이어와 비교할 때와 표준 바인딩의 지정 해제를 쓸 때는 교환을 거친 키로 해야 한다(`hide`가 표준 대상이면 `swap_alt_cmd(keys)`를 쓰고, `binding_conflicts`·`overrides_standard`·가져오기 `overrides`는 엔진이 실제로 걸 키로 비교). 캡처 알림·시트의 키도 누른 키로 보이는 편이 맞다(지금은 파일 표기 `alt-z`를 보인다) | `crates/keyscore-app/src/keymap/user.rs:262-276`, `crates/keyscore-app/src/settings_window.rs:418-451`, `:479-482`, `:607-619`, `crates/keyscore-app/src/keymap/mod.rs:185-189` |
| 2 | 수정 | 사용자 레이어가 자리(컨텍스트·키)마다 항목 하나라서, 지정 해제가 있는 자리에 다른 Action을 걸면 `push`가 지정 해제를 지운다. 그 Action을 나중에 지우면 사용자가 지웠던 아래 레이어 바인딩이 알림 없이 되살아난다(시도 5: 뮤트 `m` 지우기 → 솔로에 `m` → 솔로 키 지우기 → 뮤트에 `m`). `zed::Unbind`는 Action마다 따로 둘 수 있으니 같은 컨텍스트의 절을 둘로 나눠 쓰거나, 지정 해제를 자리 비교에서 빼는 방안 | `crates/keyscore-app/src/keymap/user.rs:244-258`, `:302-309` |
| 3 | 제안 | 리뷰 1 제안 11 대응이 한 세션 안에서만 맞다. `last_written`이 메모리에만 있어 재시작 뒤 첫 편집은 편집기가 쓴 파일을 사람이 쓴 것으로 보고 `.bak`의 원본을 덮는다(시도 6). 파일이 `UserKeymap::parse(..).to_json()`과 같으면 편집기 출력으로 보는 식으로 디스크 내용만으로 판단하는 방안 | `crates/keyscore-app/src/keymap/mod.rs:498-506`, `:288` |
| 4 | 제안 | `effective()`의 `null_hides`는 컨텍스트 있는 `null`·지정 해제가 전역 바인딩도 가린다고 본다. GPUI에서 전역 바인딩은 가장 깊은 깊이라 그 컨텍스트가 가장 안쪽일 때만 가려진다. 편집기는 대상의 컨텍스트로만 지정 해제를 쓰므로 손 편집 파일에서만 나타난다(문서화된 근사의 한 갈래) | `crates/keyscore-app/src/keymap/user.rs:56-58` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1의 지적 1-10은 교환이 꺼진 상태에서 뿌리까지 고쳐졌다(시도 7-13). 지정 해제(`zed::Unbind`) 전환은 엔진이 오류 없이 만들고 모델과 엔진이 자기 컨텍스트에서 일치한다. 남은 것은 수식키 교환을 켰을 때의 표준 레이어 비교·지정 해제(지적 1)와 지운 바인딩의 부활(지적 2)이다. 둘 다 사용자 경로가 있는 실제 오동작이다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A: 실패한 깨뜨리기 테스트

아래 테스트는 `tests/review_s2_t6a_r2.rs`의 도우미(`start`, `open_project`, `open_settings`, `select_exact`, `press`, `settle`, `activate`, `effective_of`, `dispatched`, `menu_keys`, `user_file`)를 쓴다. 실패해서 파일에서 뺐다. 고친 뒤 그 파일로 옮겨 통과시킨다.

~~~rust
/// 수식키 교환을 켠 채 표준 단축키(`edit::Undo`의 `cmd-z`)를 지우면 엔진에서도 꺼져야 한다(목록은 꺼졌다고 보인다).
#[gpui_kit::test]
fn brk_swap_clear_standard_really_unbinds(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-clear-std", None);
    let (project, _) = open_project(cx);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "edit::Undo");
    press(cx, handle, "backspace");
    assert!(effective_of(cx, "edit::Undo").is_empty());
    activate(cx, project);
    let fired = dispatched(cx, project, "cmd-z");
    let menu = menu_keys(cx, "edit::Undo");
    assert!(
        fired.as_deref() != Some("edit::Undo") && menu.is_none(),
        "편집기는 Undo 키가 없다고 보이지만 cmd-z가 {fired:?}, 메뉴 키 {menu:?}\n{}",
        user_file(&dir)
    );
}

/// 수식키 교환을 켠 채 `cmd-z`를 캡처하면 표준 단축키(`edit::Undo`)와의 충돌 시트나 표준 경고가 나와야 한다
/// (KM-4, KM-12). 교환 없이는 시트가 뜬다.
#[gpui_kit::test]
fn brk_swap_capture_standard_key_warns(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-cap-std", None);
    let (project, _) = open_project(cx);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter cmd-z");
    settle(cx, handle);
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    activate(cx, project);
    let fired = dispatched(cx, project, "cmd-z");
    assert!(
        sheet || msg.contains("표준"),
        "시트·경고 없이 걸렸다: {msg:?}, cmd-z는 이제 {fired:?}\n{}",
        user_file(&dir)
    );
}

/// 수식키 교환을 켠 채 `alt-z`를 캡처하면 실제로 걸리는 키는 `alt-z`라 표준 `cmd-z`와 부딪히지 않는다. 충돌
/// 시트가 뜨면 안 된다.
#[gpui_kit::test]
fn brk_swap_capture_alt_key_no_false_conflict(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-cap-alt", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "enter alt-z");
    settle(cx, handle);
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    if sheet {
        // 「새 바인딩 유지」를 고르면 Undo가 목록에서 사라지지만 엔진은 여전히 cmd-z로 Undo한다
        press(cx, handle, "enter");
    }
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    assert!(
        !sheet,
        "alt-z(실제 키)가 표준 cmd-z와 충돌한다고 묻는다. 유지 뒤 알림: {msg:?}, Undo 목록 {:?}, 메뉴 {:?}\n{}",
        effective_of(cx, "edit::Undo"),
        menu_keys(cx, "edit::Undo"),
        user_file(&dir)
    );
}

/// 앱을 다시 띄운 뒤(엔진 재설치) 첫 편집이 `.bak`의 손으로 쓴 원본을 편집기 출력으로 덮으면 안 된다.
#[gpui_kit::test]
fn brk_bak_survives_restart(cx: &mut TestAppContext) {
    let original = "// 내 메모\n[{\"context\": \"Normal\", \"bindings\": {\"ctrl-alt-u\": \"track::ToggleMute\"}}]\n";
    let dir = start(cx, "bak-restart", Some(original));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "backspace");
    // 다시 띄우기: 엔진만 새로 설치한다(같은 사용자 파일)
    cx.update(|cx| {
        let path = keymap::user_path(cx);
        let options = keymap::options(cx);
        keymap::install(cx, path, options, |_| {});
    });
    activate(cx, handle);
    select_exact(cx, handle, &view, "track::New");
    press(cx, handle, "backspace");
    let bak = std::fs::read_to_string(dir.join("keymap.json.bak")).unwrap_or_default();
    assert!(bak.contains("내 메모"), "재시작 뒤 원본이 사라졌다: {bak}");
}

/// `m`을 지운 Action(뮤트)은 그 자리를 다른 Action(솔로)이 가졌다가 놓아도 `m`을 되찾지 않아야 한다(사용자가 지운
/// 것은 지운 채로).
#[gpui_kit::test]
fn brk_cleared_key_not_revived_after_other_action_releases_it(cx: &mut TestAppContext) {
    let dir = start(cx, "revive", None);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "track::ToggleMute");
    press(cx, handle, "backspace");
    select_exact(cx, handle, &view, "track::ToggleSolo");
    press(cx, handle, "enter m");
    settle(cx, handle);
    press(cx, handle, "backspace");
    let mute = effective_of(cx, "track::ToggleMute");
    assert!(
        mute.is_empty(),
        "지운 뮤트의 m이 되살아났다: {mute:?}\n{}",
        user_file(&dir)
    );
}

/// 수식키 교환을 켠 채 `alt-s`(엔진은 `cmd-s`로 건다)를 가져오면 표준 단축키(저장) 덮기를 적용 전에 알려야 한다.
#[gpui_kit::test]
fn brk_swap_import_warns_standard_override(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-import", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    let file = dir.join("in.json");
    std::fs::write(&file, r#"[{"bindings": {"alt-s": "track::ToggleMute"}}]"#).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(km::Import), cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    let sheet = cx.read(|cx| view.read(cx).sheet_open());
    let msg = cx.read(|cx| view.read(cx).message()).unwrap_or_default();
    assert!(
        sheet || msg.contains("표준"),
        "표준 저장 키를 덮는데 경고 없이 적용: {msg:?}, cmd-s 메뉴 {:?}",
        menu_keys(cx, "file::Save")
    );
}
~~~

실행 결과(`cargo test -p keyscore-app --test review_s2_t6a_r2`, 123d895):

~~~
---- brk_swap_clear_standard_really_unbinds stdout ----
편집기는 Undo 키가 없다고 보이지만 cmd-z가 Some("edit::Undo"), 메뉴 키 Some("cmd-z")
[
  {
    "bindings": {
      "cmd-z": ["zed::Unbind", "edit::Undo"]
    }
  }
]

---- brk_swap_capture_standard_key_warns stdout ----
시트·경고 없이 걸렸다: "track::ToggleMute에 alt-z를 걸었다", cmd-z는 이제 Some("edit::Undo")
[
  {
    "context": "Normal",
    "bindings": {
      "m": ["zed::Unbind", "track::ToggleMute"],
      "alt-z": "track::ToggleMute"
    }
  }
]

---- brk_swap_capture_alt_key_no_false_conflict stdout ----
alt-z(실제 키)가 표준 cmd-z와 충돌한다고 묻는다. 유지 뒤 알림: "track::ToggleMute에 cmd-z를 걸었다. 경고: 표준 단축키를 덮거나 지웠다(메뉴 단축키가 바뀐다)", Undo 목록 [], 메뉴 Some("cmd-z")
[
  {
    "bindings": {
      "cmd-z": ["zed::Unbind", "edit::Undo"]
    }
  },
  {
    "context": "Normal",
    "bindings": {
      "m": ["zed::Unbind", "track::ToggleMute"],
      "cmd-z": "track::ToggleMute"
    }
  }
]

---- brk_swap_import_warns_standard_override stdout ----
표준 저장 키를 덮는데 경고 없이 적용: "키맵을 가져왔다", cmd-s 메뉴 None

---- brk_cleared_key_not_revived_after_other_action_releases_it stdout ----
지운 뮤트의 m이 되살아났다: [(Some("Normal"), "m", "track::ToggleMute")]
[
  {
    "context": "Normal",
    "bindings": {
      "o": ["zed::Unbind", "track::ToggleSolo"]
    }
  }
]

---- brk_bak_survives_restart stdout ----
재시작 뒤 원본이 사라졌다: [
  {
    "context": "Normal",
    "bindings": {
      "ctrl-alt-u": "track::ToggleMute",
      "o": ["zed::Unbind", "track::ToggleSolo"]
    }
  }
]
~~~
