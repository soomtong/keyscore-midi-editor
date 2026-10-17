# 적대적 리뷰: S2-T5b 트랜스포트 바와 트랙 목록 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않은 적대적 리뷰어). 1회차 리뷰어와도 다른 세션
- 대상 커밋: 156146e(브랜치 `s2/t5b-transport-bar`, 범위 `git diff main...HEAD`, 리뷰 1 대응은 `git show 156146e`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 트랜스포트 바를 모드 컨텍스트 겹 밖(window.rs:712-716, 792)으로 옮긴 뒤, 그 안에서 포커스를 잡는 요소(입력란, `Toggle`)에 포커스가 남으면 디스패치 경로에 `Normal` 등이 없다. 클릭으로 포커스를 가져가는 요소가 있으면 그 뒤 모드 키가 죽는다.
2. 입력란의 `cmd-z`·`cmd-shift-z`: 전역 `standard.json`의 `edit::Undo`와 gpui-base `Input` 컨텍스트의 `Undo`가 같은 깊이에서 다툰다. 루트에 `edit_undo` 처리기가 있으므로 문서 undo가 이기면 입력란 글자가 아니라 문서가 되돌려진다.
3. 입력란의 `Esc`: window.rs:83-85 주석은 「`Input`은 Esc에서 `window.blur()`를 부른다」를 전제로 한다. gpui-base 0.7.1에서 그런지, 아니면 포커스가 입력란에 남는지.
4. `sync_transport_fields`(transport_bar.rs:85-93)는 「입력 중」을 포커스로 판정한다. 포커스만 있고 고치지 않은 필드는 옛 값을 보인다. 그 상태에서 탭 템포 버튼(버튼은 마우스 다운에서 포커스를 막아 입력란 포커스가 그대로다)이나 메뉴 undo로 문서가 바뀐 뒤 Enter를 치면 옛 값이 다시 확정되는가.
5. 고치지 않은 필드에서 Enter: `set_song_tempo`가 같은 값으로 편집을 적용해 빈 undo 단위와 「바뀜」 표시를 만드는가.
6. vim 프리셋에서 입력란의 숫자·`/`·글자(`o`, `m`, `t`), `t` 키로 들어가 치기, Enter 뒤 포커스 복귀.
7. 현재 트랙 이웃: 지우기·undo·redo를 오갈 때, 새 트랙 둘을 undo 둘로 지울 때.
8. 패닉 버튼이 실행 함수를 거치는가. 재생·정지·패닉·탭 버튼, 조성 메뉴 뒤 문서 포커스가 사는가.
9. 입력란의 `ctrl-tab`(전역 `mode::Cycle`).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 바의 버튼·토글 클릭 뒤 모드 키 | 없음 | `every_button_clears_count`는 카운트만 본다 |
| 2. 입력란 `cmd-z` | 없음 | |
| 3. 입력란 `Esc` | 없음 | |
| 4. 포커스만 있는 필드와 다른 경로 편집 뒤 Enter | 없음 | `tempo_field_follows_*`는 포커스 없는 필드만 본다 |
| 5. 고치지 않은 Enter의 undo 단위 | 없음 | |
| 6. vim 키로 입력란 | 부분 | 숫자만(`vim_digits_in_tempo_field`). 글자·`t`·박자표 없음 |
| 7. 현재 트랙 이웃 | 부분 | NewBelow undo 한 번만 |
| 8. 패닉 버튼 | 있음 | `transport_panic_button_dispatches_panic` |
| 9. 입력란 `ctrl-tab` | 없음 | |

## 3. 깨뜨리기 시도

모두 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t5b_r2`). 키는 `simulate_keystrokes`(키 바인딩을 거침), 클릭은 실제 마우스 사건(`window.click`)이다. 깨진 것은 부록에 코드로 싣는다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 메트로놈 버튼 클릭 뒤 `m` | 깨짐: 메트로놈은 켜지지만 `m`이 아무 트랙도 뮤트하지 않는다. `Toggle`(gpui-base toggle.rs:127-191)은 자기 포커스 핸들을 잡고 마우스 다운에서 포커스를 막지 않는다. 바가 컨텍스트 겹 밖이라 그 포커스의 경로에 `Normal`이 없다 | 부록 B1 `metronome_button_keeps_mode_keys` |
| 2 | 트랙 하나 지움 → 템포 입력란에서 `9` → `cmd-z` | 깨짐: 지운 트랙이 돌아온다(문서 undo). 입력란 글자는 그대로 | 부록 B2 `cmd_z_in_tempo_field_does_not_undo_document` |
| 3 | 템포 입력란에서 `9 9` → `escape` | 깨짐: 포커스가 입력란에 남는다. gpui-base `escape`(input/base/state.rs:2079-2116)는 `cx.propagate()`만 하고 blur하지 않는다. 컨텍스트 겹 밖이라 위에 `escape` 바인딩도 없다 | 부록 B3 `escape_in_tempo_field_returns_to_document` |
| 4 | 템포 입력란 클릭(고치지 않음) → 탭 버튼 두 번(0.5 s) → Enter | 깨짐: 탭으로 정한 템포(약 112 BPM)가 옛 필드 값 120으로 다시 확정된다 | 부록 B4 `enter_after_tap_while_field_focused_keeps_tapped_tempo` |
| 5 | `t 9 0 enter` → 입력란 클릭 → `edit::Undo`(포커스로 디스패치, 메뉴 막대와 같은 경로) → Enter | 깨짐: undo한 90이 되살아난다 | 부록 B5 `enter_after_menu_undo_while_field_focused` |
| 6 | 트랙 지움 → 입력란 클릭 → 그대로 Enter → `edit::Undo` | 깨짐: 첫 undo가 같은 값 템포 편집을 되돌리고 트랙은 돌아오지 않는다(빈 undo 단위, 「바뀜」 표시) | 부록 B6 `enter_without_change_makes_no_undo_unit` |
| 7 | 기본·vim 프리셋에서 입력란에 `m o t x 1 2`, 그 뒤 `100 enter`, Enter 뒤 모드 키(`m`/vim `o`) | 못 깨뜨림 | `letters_and_digits_stay_in_field_both_presets` |
| 8 | 두 프리셋에서 `t` → `8 5 enter` | 못 깨뜨림(전체 선택으로 바로 덮어씀, 포커스 복귀) | `t_key_enters_tempo_field_and_commits` |
| 9 | vim 박자표 입력란 `6 / 8 enter` | 못 깨뜨림 | `vim_meter_field_with_keys` |
| 10 | 입력란 `cmd-a cmd-c right cmd-v` | 못 깨뜨림(전역 `select::All`·`edit::Copy`·`edit::Paste`는 처리기가 없어 입력란으로 떨어진다) | `cmd_a_c_v_in_tempo_field` |
| 11 | 문서 포커스에서 재생·정지·패닉·탭 버튼 클릭 뒤 `m`, 조성 메뉴 선택 뒤 `m` | 못 깨뜨림(`Button`은 마우스 다운에서 포커스를 막는다) | `buttons_keep_document_focus`, `key_menu_returns_focus` |
| 12 | 글자를 친 입력란이 있을 때 `SetTempo(100)` → 그 글자로 Enter | 못 깨뜨림(입력 중 글자를 덮지 않고 친 값이 확정) | `typed_text_survives_document_change` |
| 13 | 가운데 트랙 지움 → undo → redo, 마지막 트랙에서 새 트랙 둘 → undo 둘 | 못 깨뜨림(늘 문서에 있는 트랙, 마지막엔 원래 마지막 트랙) | `current_track_through_delete_undo_redo` |
| 14 | 입력란에서 `ctrl-tab` | 기록: 모드는 Step이 되지만 포커스는 입력란에 남는다(제안 1) | `ctrl_tab_in_tempo_field_cycles_mode` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 현재 트랙은 문서에 있는 트랙이거나 없음 | 아니오 | `keep_current_track`(window.rs:221-233). 시도 13 |
| 트랙 편집은 각각 undo 한 단위, 빈 단위 없음 | 예(트랜스포트 필드) | 고치지 않은 Enter가 같은 값 편집을 적용한다(transport_bar.rs:102-107, 123-137 → document.rs:39-46). 시도 6 |
| 필드 표시 = 문서 값(입력 중이 아닐 때) | 예 | 「입력 중」을 포커스로 판정한다(transport_bar.rs:85-93). 시도 4·5 |
| 문서 포커스에서 모드 키가 산다(버튼은 포커스를 가져가지 않는다) | 예 | 메트로놈 `Toggle`(transport_bar.rs:376). 시도 1 |
| 채널 1–16 | 아니오 | 변경 없음 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1(트랜스포트 버튼) | 충족 | transport_bar.rs:326-345. 시도 11 |
| TRN-2(메트로놈 토글) | 부분 충족 → 미충족 | 토글은 되지만 누른 뒤 모드 키가 죽는다(transport_bar.rs:376-382, 시도 1). P1 「키보드가 1급」 |
| TRN-4(템포·박자표·조성 필드) | 부분 충족 → 미충족 | 입력란의 `cmd-z`가 문서를 되돌리고(시도 2), 포커스만 있는 필드의 Enter가 다른 경로의 편집을 덮는다(시도 4·5). 키 입력 자체(지적 1 대응)는 두 프리셋 모두 충족(시도 7-10) |
| SEQ-7·SEQ-8 | 충족 | 변경 없음. 1회차 판단 유지 |
| OUT-4 | 충족 | 변경 없음 |
| OUT-6 / 7.6 「패닉: 트랜스포트의 ⏹! 버튼」 | 충족 | transport_bar.rs:336-341, 실행 함수 경유 |
| 7.6 「클릭해 입력」 | 부분 → 미충족 | 시도 2-6과 같은 근거. 끌기는 S3-T6 이월(문서화됨) |
| P2 / A13 결정 「Esc는 통상의 취소」 | 미충족 | 입력란의 Esc가 아무것도 취소하지 않고 포커스도 남는다(시도 3). `t`로 키보드만으로 들어온 사용자가 확정(Enter) 말고는 나갈 길이 없다(P1) |
| UI-11 | 충족 | ⏹! 버튼 추가 |
| 5.11 Tab | 충족 | 입력란 `Tab`은 아무 일도 하지 않음(`tab_inside_tempo_field_does_nothing`) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 이번 대응에 없음. `sync_transport_fields`·`keep_current_track`은 메인 스레드 관찰자다.
- 제품 코드의 `unwrap`/`expect`: 새로 들어온 것 없음. `keep_current_track`의 `tracks.get(index)`는 빈 목록에서 `None`(시도 13, 1회차 `deleting_all_tracks…`).
- 에러가 삼켜지는 곳: 없음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정(리뷰 1 대응이 만든 회귀) | 메트로놈 버튼(`Toggle`)을 누르면 포커스가 토글로 가고, 바가 모드 컨텍스트 밖이라 그 뒤 모든 모드 키가 죽는다(B1). 1회차 커밋(e9e002e)에서는 토글이 컨텍스트 겹 안이라 경로에 `Normal`이 있었다. 토글 클릭 뒤 문서로 포커스를 돌리거나, 포커스를 가져가지 않는 요소로 그리고, 바 안의 포커스 가능한 요소를 모두 실제 클릭+실제 키로 지킨다 | transport_bar.rs:376-382; window.rs:709-716, 792 |
| 2 | 수정 | 템포·박자표 입력란에서 `cmd-z`·`cmd-shift-z`가 입력란 글자가 아니라 문서를 되돌린다(B2). 전역 `standard.json`의 `edit::Undo`와 `Input` 컨텍스트의 `Undo`는 같은 깊이(gpui keymap.rs:274-280, 216-218)이고 뒤에 실린 전역이 이겨, 루트의 `edit_undo`가 처리한다. 그 사이 필드는 동기화를 건너뛰어(지적 3) 되돌린 값이 보이지 않는다 | assets/keymaps/standard.json:6-7; window.rs:519; transport_bar.rs:85-93 |
| 3 | 수정(리뷰 1 지적 2가 근본까지 닿지 않음) | 「입력 중」을 포커스로 판정해, 포커스만 있고 고치지 않은 필드가 옛 값을 보이고 Enter가 그 값을 다시 확정한다. 탭 템포 버튼(버튼은 입력란 포커스를 빼앗지 않는다)·메뉴 undo 뒤 Enter가 그 편집을 덮는다(B4, B5). 사용자가 고친 적이 있는지(`InputEvent::Change`)로 판정하거나, Enter에서 「고치지 않음」이면 확정하지 않는다 | transport_bar.rs:85-93, 102-107, 123-137 |
| 4 | 수정 | 입력란의 `Esc`가 아무 일도 하지 않고 포커스가 입력란에 남는다(B3). A13 결정 「결정/선택 동작에서 작업을 취소」·P1에 어긋난다(`t`로 들어온 사용자가 키보드로 취소할 길이 없다). window.rs:83-85 주석의 전제(「`Input`은 Esc에서 `window.blur()`를 부른다」)가 gpui-base 0.7.1에서 사실이 아니다(input/base/state.rs:2079-2116은 `propagate`만 한다) | window.rs:83-88; transport_bar.rs:96-140 |
| 5 | 수정 | 고치지 않은 필드의 Enter(템포·박자표)가 같은 값으로 편집을 적용해 빈 undo 단위와 「바뀜」 표시를 만든다(B6). 1회차가 순서 이동에서 확인한 「빈 undo 단위 없음」과 같은 기준이다. 값이 같으면 편집하지 않는다(조성 메뉴에서 지금 조성을 고를 때도 같은 경로인지 함께 본다) | transport_bar.rs:102-107, 123-137, 145-153; document.rs:39-46 |
| 6 | 제안 | 입력란의 `ctrl-tab`은 모드를 Step으로 바꾸지만 포커스는 입력란에 남아, 상태줄은 STEP인데 키가 글자로 들어간다(시도 14). 모드 전환 시 문서로 포커스를 돌릴지 정해 테스트로 남기기를 권한다 | window.rs:173-183, 488 |
| 7 | 제안 | `sync_transport_fields`는 한 필드에 포커스가 있으면 두 필드 모두 건너뛴다. 박자표 입력 중 템포가 바뀌면 템포 필드가 Blur 때까지 옛 값을 보인다(표시만, Blur에서 맞춰진다) | transport_bar.rs:89-91 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1 지적 1(모드 키 누수)·3(패닉 버튼)·4(이웃 트랙)는 근본까지 닿았다. 지적 2(필드 동기화)는 포커스 판정 때문에 절반만 닿았고(지적 3), 지적 1의 수정이 메트로놈 토글의 회귀(지적 1)를 만들었다. 지적 1–5를 고친 뒤 부록 B1–B6을 `tests/review_s2_t5b_r2.rs`로 옮겨 통과시키고 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/12-S2`의 이월 한 줄은 계획 문서 갱신)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 B: 깨지는 테스트

`crates/keyscore-app/tests/review_s2_t5b_r2.rs`의 도우미(`start`, `open`, `run`, `click`, `press`, `ids`, `tempo`, `muted`, `tempo_focused`, `three_tracks`)와 import를 그대로 쓴다. 2026-10-17 156146e에서 실행해 모두 실패했다.

실패 출력 요약

~~~
metronome_button_keeps_mode_keys: 메트로놈 버튼을 누른 뒤 모드 키가 닿지 않는다
cmd_z_in_tempo_field_does_not_undo_document: left: [TrackId(6), TrackId(7), TrackId(8)], right: [TrackId(7), TrackId(8)]
escape_in_tempo_field_returns_to_document: Esc 뒤에도 템포 입력란에 포커스가 남는다
enter_after_tap_while_field_focused_keeps_tapped_tempo: left: Tempo(500000), right: Tempo(530035)
enter_after_menu_undo_while_field_focused: left: Tempo(666667), right: Tempo(500000)
enter_without_change_makes_no_undo_unit: left: 2, right: 3
~~~

~~~rust
#[gpui_kit::test]
fn escape_in_tempo_field_returns_to_document(cx: &mut TestAppContext) {
    start(cx, "escape");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace 9 9");
    press(cx, handle, "escape");
    assert!(
        !tempo_focused(cx, handle, &view),
        "Esc 뒤에도 템포 입력란에 포커스가 남는다"
    );
}

#[gpui_kit::test]
fn cmd_z_in_tempo_field_does_not_undo_document(cx: &mut TestAppContext) {
    start(cx, "cmd-z");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    let after_delete = ids(cx, &view);
    assert_eq!(after_delete.len(), 2);
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace 9");
    press(cx, handle, "cmd-z");
    assert_eq!(ids(cx, &view), after_delete, "입력란의 cmd-z가 문서 undo가 됐다");
}

#[gpui_kit::test]
fn enter_after_tap_while_field_focused_keeps_tapped_tempo(cx: &mut TestAppContext) {
    start(cx, "tap-then-enter");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    click(cx, handle, "transport-tap");
    std::thread::sleep(std::time::Duration::from_millis(500));
    click(cx, handle, "transport-tap");
    let tapped = tempo(cx, &view);
    assert_ne!(tapped, Tempo::DEFAULT);
    assert!(tempo_focused(cx, handle, &view), "버튼이 입력란 포커스를 가져갔다");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), tapped, "Enter가 옛 필드 값을 다시 확정했다");
}

#[gpui_kit::test]
fn metronome_button_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "metronome-focus");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "transport-metronome");
    assert!(cx.read(|cx| view.read(cx).metronome_on()));
    press(cx, handle, "m");
    assert!(muted(cx, &view, 0), "메트로놈 버튼을 누른 뒤 모드 키가 닿지 않는다");
}

#[gpui_kit::test]
fn enter_without_change_makes_no_undo_unit(cx: &mut TestAppContext) {
    start(cx, "noop-enter");
    let (handle, view) = open(cx, three_tracks());
    run(cx, handle, track::Delete);
    click(cx, handle, "transport-tempo");
    press(cx, handle, "enter");
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 3);
}

#[gpui_kit::test]
fn enter_after_menu_undo_while_field_focused(cx: &mut TestAppContext) {
    start(cx, "menu-undo-then-enter");
    let (handle, view) = open(cx, Project::new());
    press(cx, handle, "t 9 0 enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
    click(cx, handle, "transport-tempo");
    run(cx, handle, edit::Undo);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT, "Enter가 undo한 템포를 되살렸다");
}
~~~
