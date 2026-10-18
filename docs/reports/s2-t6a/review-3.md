# 적대적 리뷰: S2-T6a 설정 창과 키맵 편집기 (리뷰 3회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 세션(구현자·1·2회차 리뷰어와 다른 세션)
- 대상 커밋: 20f8914 (`s2/t6-settings-keymap`, 수정 커밋 18fad47)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show 18fad47`, `keymap/user.rs` 전체, `keymap/mod.rs`(`build`, `current_layers`, `save_user`), `settings_window.rs`의 캡처·가져오기·복원, GPUI `keymap.rs`(gpui-pre 0.3.8의 `resolve_bindings_for_input`, `bindings_for_action`, `binding_is_unbound`)만 읽고, 테스트와 보고서 「리뷰 2 대응」을 열기 전에 적었다.

1. 사용자 레이어의 지정 해제는 쓸 때의 교환 상태로 파일 표기가 정해진다(`file_keys`). 표준 레이어는 교환되지 않으므로, 표준 단축키의 지정 해제를 쓴 뒤 교환을 켜거나 끄면 엔진이 그 지정 해제를 다른 키에 걸어 지운 표준 단축키가 되살아나는가. 프리셋 지정 해제는 프리셋과 함께 교환되니 괜찮은가.
2. 같은 자리에 지정 해제와 같은 Action의 바인딩이 함께 있을 때(뮤트 `m` 지우기 → 뮤트에 다시 `m`) GPUI가 앞의 지정 해제로 뒤의 바인딩을 끄는가(`binding_is_unbound`는 순서·위치를 보지 않는다). 메뉴 키(`bindings_for_action`)는 `disabled_ix > ix`만 본다.
3. k번째 절로 나눠 쓴 파일이 엔진·편집기·가져오기 충돌 검사(`conflict::conflicts`)를 오류·거짓 충돌 없이 지나는가. 내보내기 → 가져오기 왕복이 같은 파일이 되는가.
4. `.bak`의 내용 기준 판단(`parse(..).to_json() == text`)이 k번째 절이 있는 편집기 출력에서도 멱등인가. 사람이 고친 내용은 `.bak`이 되는가.
5. 교환을 켠 채 여러 편집 뒤 목록의 실효 바인딩이 그 자신의 컨텍스트에서 엔진이 고르는 Action과 같은가. 메뉴 키도 같은가.
6. 교환을 켠 채 접두 충돌을 실제 키로 묻는가(프리셋 `cmd-k`는 교환 뒤 `alt-k`).
7. `r`(복원)이 교환 켜짐에서 표준 지정 해제를 되돌리는가.
8. `to_json`의 절 정렬(k, 처음 나온 순서)이 서로 다른 컨텍스트의 같은 키 사이 순서(GPUI는 같은 깊이에서 뒤 항목 우선)를 바꾸는가.
9. `null_hides` 변경(컨텍스트 있는 `null`은 전역을 가리지 않는다)이 GPUI와 맞는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 교환 토글 뒤 표준 지정 해제 | 없음 | 리뷰 2 부록 테스트는 교환 상태를 바꾸지 않는다 |
| 2 지운 키를 같은 Action에 다시 걸기 | 없음 | |
| 3 k번째 절 왕복 | 부분 | `brk_cleared_key_not_revived_after_other_action_releases_it`는 편집기 안에서만. 내보내기·가져오기 왕복 없음 |
| 4 `.bak` 내용 기준 | 있음 | `brk_bak_survives_restart`(k번째 절 없는 파일) |
| 5 교환 켜짐 모델 대 엔진 | 없음 | `editor_model_matches_engine_in_own_context`는 교환 꺼짐 |
| 6 교환 켜짐 접두 충돌 | 없음 | |
| 7 교환 켜짐 표준 복원 | 없음 | |
| 8 절 정렬 | 없음 | |
| 9 `null_hides` | 없음 | |

## 3. 깨뜨리기 시도

통과한 것은 `crates/keyscore-app/tests/review_s2_t6a_r3.rs`에 남겼고, 실패한 것은 부록 A로 옮겼다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 교환 꺼짐에서 `edit::Undo`를 지운다(파일 `"cmd-z": ["zed::Unbind", "edit::Undo"]`). 교환을 켠다. 엔진이 지정 해제를 `alt-z`로 옮겨 걸어 목록·메뉴·`cmd-z` 모두 Undo가 되살아난다. 알림·경고 없음 | 깨짐 | 부록 A `brk_swap_toggle_after_clearing_standard_keeps_it_cleared` |
| 2 | 교환 켜짐에서 `settings::Open`을 지운다(파일 `"alt-,": [..]`, 엔진은 `cmd-,`). 교환을 끈다. `cmd-,`가 다시 설정 열기다 | 깨짐 | 부록 A `brk_swap_off_after_clearing_standard_keeps_it_cleared` |
| 3 | 교환 켜짐에서 프리셋 `mode::Play`(실제 `alt-k`)를 지우고 교환을 끈다. 프리셋 지정 해제는 프리셋과 함께 교환되어 계속 꺼져 있다 | 못 깨뜨림 | `swap_toggle_keeps_preset_unbind` |
| 4 | 뮤트 `m` 지우기 → 뮤트에 다시 `m`(시트 없음) → 엔진 `m` = 뮤트, 메뉴 `m`, 모델 = 엔진 → 다시 지우기 → 꺼짐 → `r` → 프리셋 `m` 복귀, 파일에 뮤트 항목 없음 | 못 깨뜨림 | `clear_then_rebind_same_key_same_action_works` |
| 5 | 뮤트 지우기, 솔로에 `m`(같은 자리 둘), Undo 지우기 → 내보내기 → 사용자 파일 비우고 재설치 → 가져오기. 표준 Undo 지정 해제 때문에 KM-12 시트가 뜨고(맞는 동작), 적용하면 파일이 내보내기 전과 글자 그대로 같고, 엔진 오류는 경고뿐, 모델 = 엔진 | 못 깨뜨림 | `export_import_round_trip_with_stacked_places` |
| 6 | 교환 켜짐에서 표준 `settings::Open` 지우기, 뮤트에 `cmd-z`(Undo 충돌 시트 → 유지), `cursor::ClipStart`를 `cmd-g`로, 솔로 지우고 `o` 다시 걸기. 목록의 모든 실효 바인딩이 엔진과 같고 메뉴 키가 누른 키다. `r`로 `cmd-,`가 돌아온다 | 못 깨뜨림 | `swap_on_model_matches_engine_after_edits` |
| 7 | 손으로 쓴 원본 → 뮤트 지우기·솔로에 `m`(같은 컨텍스트 절 둘) → 재설치 → 편집. `.bak`은 원본 그대로. 사람이 주석을 더한 뒤 재설치·편집하면 그 내용이 `.bak`이 되고, 뮤트 `m`은 끝까지 꺼져 있다 | 못 깨뜨림 | `bak_policy_with_stacked_places_and_hand_edit` |
| 8 | 교환 켜짐에서 뮤트에 `alt-k x` 캡처 → 실제 키 `alt-k`(교환된 `mode::Play`)와의 접두 충돌 시트가 뜬다 | 못 깨뜨림 | `swap_on_prefix_conflict_uses_actual_keys` |

시도 8(절 정렬)과 9(`null_hides`)는 편집기가 만들 수 없는 손 편집 파일에서만 나타나므로 실행 테스트를 두지 않고 지적 2의 제안으로만 적는다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 목록의 「지금 걸린 키」 = 엔진이 실제로 내는 키(KM-3) | 아니오 | 시도 4·5·6(교환 켜짐 포함). 토글 뒤에도 목록과 엔진은 같다(둘 다 되살아난다) |
| 같은 경로의 같은 키·접두는 저장 전에 충돌로 묻는다(KM-4) | 아니오 | 시도 6·8 |
| 사용자가 지운 바인딩은 지운 채로 남는다 | 예(교환 토글) | 시도 1·2: 표준 단축키의 지정 해제만 교환 상태에 묶여 있다. 다른 Action의 해제(시도 4·7)와 프리셋 해제(시도 3)는 남는다 |
| 편집기가 고쳐 써도 사람이 쓴 원본은 `.bak`에 남는다 | 아니오 | 시도 7 |
| 편집기 출력은 엔진이 오류 없이 읽고 왕복한다 | 아니오 | 시도 5 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| KM-2 | 충족 | 1회차 테스트, 전역 처리기 `settings_window.rs:149-155` |
| KM-3 | 미충족(부분) | 목록·`Enter`·`Backspace`·`r`는 교환 켜짐·꺼짐 모두 맞다(시도 4·6). 다만 표준 단축키의 `Backspace` 해제가 교환 토글로 풀린다(시도 1·2, `keymap/user.rs:333-358`, `keymap/mod.rs:185-189`) |
| KM-4 | 충족 | 시도 6·8, `settings_window.rs:397-450` |
| KM-6 | 충족 | `keymap/mod.rs:497-522`, 시도 7 |
| KM-8 | 충족 | 2회차 시도 12·13 |
| KM-9 | 충족 | 시도 5 |
| KM-11 | 미충족(부분) | 켜고 끄기 자체와 켜진 상태의 편집은 맞다(시도 6). 켜고 끌 때 표준 지정 해제가 따라 교환되어 효과를 잃는다(시도 1·2) |
| KM-12 | 충족 | 캡처(시도 6), 가져오기(시도 5)에서 표준 덮기·지우기를 알린다. 토글로 되살아날 때는 알림이 없지만 그것은 KM-3·KM-11에 센다 |
| 7.5 `Cmd+,` | 충족 | `settings_window.rs:134-142` |
| 7.6 설정 > 키맵 | 충족 | |
| P1 | 충족 | 2회차 시도 10·11 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 UI·키맵 편집만 건드린다).
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: 2회차와 같다(`forward`의 `handle.update`, `lib.rs`의 지연 닫기). 새로 생긴 곳 없음.
- 참고: `cargo test --workspace`를 한 번에 돌릴 때 `keyscore-text`의 `export_time_stays_near_linear_when_unrestable_gaps_repeat`(S0 리뷰 테스트, 이 Task 범위 밖)가 한 번 실패했고 단독 실행에서는 통과했다. 부하에 따른 시간 측정 흔들림으로 보고 지적하지 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 표준 단축키의 지정 해제가 쓴 순간의 수식키 교환 상태에 묶인다. `hide`는 표준 대상의 지정 해제를 `file_keys`(지금 교환 상태로 교환한 키)로 쓰고, `build`는 사용자 레이어 전체(지정 해제 포함)를 그때그때의 교환 상태로 다시 교환한다. 표준 레이어는 교환되지 않으므로 지운 뒤 교환을 켜거나 끄면 지정 해제가 다른 키(`cmd-z` ↔ `alt-z`)로 가서 지운 표준 단축키가 알림 없이 되살아난다(시도 1·2). 같은 조작에서 프리셋 지정 해제(시도 3)와 다른 Action 지정 해제는 살아남으니 표준 대상만의 비대칭이다. 사용자가 실수 방지로 `cmd-q`·`cmd-w`를 지운 뒤 KM-11을 켜면 그 키가 다시 앱을 끝내거나 창을 닫는다. 표준 레이어를 겨눈 지정 해제는 교환과 무관해야 한다: 예를 들어 `build`·`effective_with`가 대상 Action이 그 (교환 전) 키로 표준 레이어에 걸린 `zed::Unbind`는 교환하지 않고, `hide`는 표준 대상이면 교환하지 않은 키를 쓰는 방안. 또는 교환을 바꿀 때 사용자 파일의 표준 지정 해제를 고쳐 쓰는 방안 | `crates/keyscore-app/src/keymap/user.rs:333-358`, `:85-95`, `:402-415`, `crates/keyscore-app/src/keymap/mod.rs:185-189` |
| 2 | 제안 | 손 편집 파일에서만 나타나는 근사 두 가지. (a) `to_json`이 같은 컨텍스트의 떨어진 절을 합치고 k번째 절을 뒤로 모으므로, 손으로 쓴 파일에서 「`Normal` 절의 `m` → 전역 절의 `m`」처럼 서로 다른 컨텍스트의 같은 키 순서가 바뀌면 컨텍스트가 가장 안쪽일 때(GPUI는 전역을 가장 깊은 깊이로 본다) 이기는 쪽이 바뀔 수 있다. 편집기의 충돌 처리(「유지」가 관련 컨텍스트의 같은 키를 지운다) 때문에 편집기만으로는 만들 수 없다. (b) `null_hides`는 이제 컨텍스트 있는 `null`이 전역을 가리지 않는다고 보지만, GPUI `resolve_bindings_for_input`에서는 그 컨텍스트가 가장 안쪽이면 같은 깊이가 되어 뒤에 건 `null`이 전역도 가린다(메뉴 키를 고르는 `bindings_for_action`은 가리지 않는다). 편집기는 대상 자신의 컨텍스트로만 쓰므로 실제 경로가 없다. 문서화된 근사로 두어도 된다 | `crates/keyscore-app/src/keymap/user.rs:236-262`, `:54-58` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2의 지적 1(교환 켜짐의 캡처·지우기·가져오기·표준 비교)과 지적 2(지운 바인딩 부활), 제안 3(`.bak`)은 교환 상태를 바꾸지 않는 한 뿌리까지 고쳐졌다(시도 3-8). 같은 자리의 k번째 절 쓰기도 엔진·충돌 검사·내보내기 왕복·`.bak` 판단을 모두 지난다. 남은 것은 지적 1 하나다: 표준 단축키를 지운 뒤 KM-11 교환을 바꾸면 지운 키가 알림 없이 돌아온다. 두 조작 모두 같은 설정 화면의 일반 기능이고 결과(`cmd-q` 부활 등)가 구체적이라 「수정」으로 센다. 프로토콜 [8]에 따라 이번이 세 번째 재리뷰이므로, 이 지적을 고치는 방식(표준 지정 해제를 교환에서 빼기, 또는 토글 때 고쳐 쓰기)을 결정 요청으로 올리는 것이 다음 단계다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A: 실패한 깨뜨리기 테스트

아래 테스트는 `tests/review_s2_t6a_r3.rs`의 도우미(`start`, `open_project`, `open_settings`, `select_exact`, `press`, `activate`, `effective_of`, `dispatched`, `menu_keys`, `user_file`)를 쓴다. 실패해서 파일에서 뺐다. 고친 뒤 그 파일로 옮겨 통과시킨다.

~~~rust

/// 교환을 끈 채 표준 단축키(`edit::Undo`의 `cmd-z`)를 지운 뒤 교환을 켜도 Undo는 지운 채여야 한다.
#[gpui_kit::test]
fn brk_swap_toggle_after_clearing_standard_keeps_it_cleared(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-toggle-std", None);
    let (project, _) = open_project(cx);
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "edit::Undo");
    press(cx, handle, "backspace");
    assert!(effective_of(cx, "edit::Undo").is_empty());
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let list = effective_of(cx, "edit::Undo");
    activate(cx, project);
    let fired = dispatched(cx, project, "cmd-z");
    assert!(
        list.is_empty() && fired.as_deref() != Some("edit::Undo"),
        "교환을 켜니 지운 Undo가 돌아왔다: 목록 {list:?}, cmd-z {fired:?}, 메뉴 {:?}\n{}",
        menu_keys(cx, "edit::Undo"),
        user_file(&dir)
    );
}

/// 교환을 켠 채 표준 단축키를 지운 뒤 교환을 꺼도 지운 채여야 한다.
#[gpui_kit::test]
fn brk_swap_off_after_clearing_standard_keeps_it_cleared(cx: &mut TestAppContext) {
    let dir = start(cx, "swap-off-std", None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, true));
    let (handle, view) = open_settings(cx);
    select_exact(cx, handle, &view, "settings::Open");
    press(cx, handle, "backspace");
    assert_eq!(menu_keys(cx, "settings::Open"), None);
    cx.update(|cx| keymap::set_swap_alt_cmd(cx, false));
    let list = effective_of(cx, "settings::Open");
    assert!(
        list.is_empty(),
        "교환을 끄니 지운 설정 열기가 돌아왔다: 목록 {list:?}, 메뉴 {:?}\n{}",
        menu_keys(cx, "settings::Open"),
        user_file(&dir)
    );
}
~~~

실행 결과(`cargo test -p keyscore-app --test review_s2_t6a_r3`, 20f8914):

~~~
---- brk_swap_toggle_after_clearing_standard_keeps_it_cleared stdout ----
교환을 켜니 지운 Undo가 돌아왔다: 목록 [(None, "cmd-z", "edit::Undo")], cmd-z Some("edit::Undo"), 메뉴 Some("cmd-z")
[
  {
    "bindings": {
      "cmd-z": ["zed::Unbind", "edit::Undo"]
    }
  }
]

---- brk_swap_off_after_clearing_standard_keeps_it_cleared stdout ----
교환을 끄니 지운 설정 열기가 돌아왔다: 목록 [(None, "cmd-,", "settings::Open")], 메뉴 Some("cmd-,")
[
  {
    "bindings": {
      "alt-,": ["zed::Unbind", "settings::Open"]
    }
  }
]
~~~
