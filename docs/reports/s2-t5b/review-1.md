# 적대적 리뷰: S2-T5b 트랜스포트 바와 트랙 목록 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않은 적대적 리뷰어)
- 대상 커밋: e9e002e(브랜치 `s2/t5b-transport-bar`, 범위 `git diff main...HEAD`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 트랜스포트 입력란이 모드 키 컨텍스트(`Normal` 등) 안에 그려지는가. 그렇다면 입력란에 친 키가 텍스트가 되기 전에 모드 바인딩(`space`, vim 숫자 카운트 등)으로 먼저 빠지는가.
2. 템포·박자표 필드가 문서 값과 늘 같은가. 필드를 다시 채우는 곳(`refresh_transport_fields`)이 Enter·Blur·초기화뿐이라, undo/redo·탭 템포·`transport::SetTempo(값)` 뒤에는 옛 값이 남고 다음 Enter가 그 옛 값을 다시 확정하는가.
3. 현재 트랙이 undo/redo 뒤에도 문서에 있는가. 사라졌을 때 「이웃 트랙」(계획)으로 가는가, 첫 트랙으로 가는가.
4. 트랙 순서 이동(지우기+넣기)이 한 undo 단위이고 현재 트랙(같은 id)을 지키는가. 끝에서 이동하면 빈 undo 단위가 생기지 않는가.
5. 모든 트랙을 지운 뒤 트랙 Action이 패닉 없이 아무것도 하지 않는가.
6. M/S 버튼이 그 줄의 트랙만 바꾸는가. 모든 버튼이 카운트를 비우는가(결정 C).
7. SEQ-7 빈 채널 계산이 이름 포트 트랙을 섞지 않는가.
8. 템포 범위 끝(999.9, 1000, 9.9, NaN)과 탭 템포의 2초 리셋.
9. 실효 음색: 재생 중 순서 이동·채널 변경으로 실효 음색이 바뀌면 보내고, 무관한 편집(템포, 다른 채널 뮤트)에는 프로그램을 보내지 않는가.
10. 7.6 트랜스포트 행 전부(패닉 ⏹! 포함)가 바에 있는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 입력란 키가 모드 바인딩으로 빠짐 | 없음 | 구현 테스트는 `simulate_input`으로 키 바인딩을 우회해 친다(`tests/transport_bar.rs:67-70`) |
| 2. 필드가 undo·탭 템포 뒤 문서 값을 보임 | 없음 | undo 테스트는 문서 값만 본다(`tests/transport_bar.rs:137-143`) |
| 3. undo로 사라진 현재 트랙의 행선 | 부분 | 「문서에 있음」만 본다 |
| 4. 순서 이동 undo와 현재 트랙 | 부분 | 끝에서 이동하는 경우 없음 |
| 5. 트랙 0개에서 트랙 Action | 없음 | |
| 6. M/S 그 줄 대상, 모든 버튼 카운트 | 부분 | M/S만 카운트 확인 |
| 7. 이름 포트 채널 분리 | 없음 | |
| 8. 템포 범위 끝, 탭 리셋 | 없음 | |
| 9. 실효 음색 순서 이동·채널 변경·무관한 편집 | 없음 | 삭제·솔로·들리지 않는 트랙만 있다 |
| 10. 패닉 버튼 | 없음 | 구현에 없다 |

## 3. 깨뜨리기 시도

모두 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t5b`). 깨진 것은 부록에 코드로 싣는다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 템포 입력란에 포커스를 두고 `space` | 깨짐: `transport::Toggle`이 실행돼 메시지가 남는다(엔진이 있으면 재생이 시작된다) | 부록 A1 `space_in_tempo_field_does_not_run_transport` |
| 2 | vim 프리셋에서 템포 입력란에 `9 0 enter` | 깨짐: 숫자가 `operator::Count`로 빠져 입력란에 들어가지 않는다. 템포 120 그대로 | 부록 A2 `vim_digits_in_tempo_field` |
| 3 | 템포 90 확정 → `edit::Undo` → 입력란 값 | 깨짐: 문서는 120인데 입력란은 `90.0` | 부록 A3 `tempo_field_follows_undo` |
| 4 | `transport::SetTempo(90)` → 입력란 값 | 깨짐: 문서는 90인데 입력란은 `120.0`(탭 템포도 같은 경로) | 부록 A4 `tempo_field_follows_tap_and_set_tempo` |
| 5 | 마지막 트랙에서 `NewBelow` → undo | 깨짐: 현재 트랙이 이웃(지운 자리 앞, 원래 마지막 트랙)이 아니라 첫 트랙으로 간다 | 부록 A5 `undo_of_new_below_moves_to_neighbour` |
| 6 | 기본 프리셋에서 실제 키로 `9 0 . 5`, `6 / 8` | 못 깨뜨림(`.`·`/`는 처리기가 없어 텍스트로 떨어진다) | `typing_decimal_tempo_with_keys`, `typing_meter_with_keys` |
| 7 | 순서 이동 뒤 현재 트랙·undo·redo, 맨 위에서 위로 | 못 깨뜨림 | `move_keeps_current_track_and_undo_restores_order` |
| 8 | 트랙을 모두 지우고 Delete·Mute·MoveDown·Next, 그 뒤 undo | 못 깨뜨림 | `deleting_all_tracks_leaves_no_current_and_undo_restores` |
| 9 | 셋째 줄 S 버튼 | 못 깨뜨림(그 줄만 솔로, 현재 트랙이 됨) | `solo_button_targets_its_row` |
| 10 | 이름 포트 트랙이 채널 1을 쓸 때 새 트랙 | 못 깨뜨림(가상 포트 채널 1) | `named_port_channels_do_not_block_virtual_channels` |
| 11 | `SetTempo` 999.9 / 1000 / NaN / 9.9 | 못 깨뜨림 | `set_tempo_action_bounds` |
| 12 | 탭 → 2.1 s 쉼 → 탭 | 못 깨뜨림(템포 그대로) | `tap_tempo_resets_after_pause` |
| 13 | vim `3` 뒤 재생·정지·메트로놈·탭·M·S·`+` 클릭, 조성 메뉴 선택 | 못 깨뜨림(카운트가 비워짐) | `every_button_clears_count`, `key_menu_choice_clears_count` |
| 14 | 재생 중 공유 채널 트랙 순서 이동 | 못 깨뜨림(`C0 05`가 나가고 다른 채널은 안 나감) | `moving_shared_track_while_playing_sends_new_effective_program` |
| 15 | 재생 중 템포 변경·다른 채널 뮤트 | 못 깨뜨림(프로그램 송신 없음) | `unrelated_edits_while_playing_send_no_program` |
| 16 | 재생 중 공유 채널 뒤 트랙을 채널 3으로 | 못 깨뜨림(`C0 05`, `C2 09`) | `changing_shared_track_channel_while_playing` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 현재 트랙은 문서에 있는 트랙이거나 없음 | 아니오 | `keep_current_track`(window.rs:205)가 문서 알림마다 확인. 시도 7·8 |
| 트랙 편집은 각각 undo 한 단위 | 아니오 | 순서 이동은 `edit_document` 한 번(track_list.rs:176). 시도 7 |
| 채널 1–16 | 아니오 | `track::SetChannel`은 `Channel::new`로 거르고 메시지(track_list.rs:215-228) |
| 버튼은 실행 함수를 거친다 | 아니오 | 시도 13 |
| 필드 표시 = 문서 값 | 예 | 시도 3·4. 계획 「끝의 정의」 3의 필드 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1(트랜스포트 버튼) | 충족 | transport_bar.rs:310-323 |
| TRN-2(메트로놈 토글) | 충족 | transport_bar.rs:353-360 |
| TRN-4(템포·박자표·조성 필드) | 부분 충족 → 미충족 | 필드가 undo·탭 템포·`t` 값 뒤 옛 값을 보이고 Enter가 그 값을 다시 확정한다(transport_bar.rs:68-81은 Enter·Blur에서만 불린다, 99·128). vim 프리셋에서는 숫자를 칠 수 없다(window.rs:502-519, 676) |
| SEQ-7(빈 채널, 드럼 표시) | 충족 | tracks.rs:14-24, track_list.rs:285-294 |
| SEQ-8(공유 경고, 16채널 초과 권고) | 충족 | tracks.rs:32-46, track_list.rs:58-66 |
| OUT-4(프로그램·뱅크 송신, 공유 채널 실효 음색) | 충족 | engine.rs `effective_setups`. 재생 시작의 순서(들리지 않는 트랙 먼저, 들리는 트랙은 렌더 순서)와 같다. 시도 14–16 |
| UI-11 / 7.6 「패닉: 트랜스포트의 ⏹! 버튼」 | 부분 충족 → 미충족 | 메뉴 재생 > 패닉은 있으나(commands/table.rs:1647) 7.6이 정한 트랜스포트 버튼이 없다(transport_bar.rs:301-380). 단계 범위는 「7.6의 해당 행」 |
| 7.6 「클릭해 입력, 끌어서 조정」 | 부분(끌기는 S3-T6 이월, 단계 파일에 적힘) | plan/stages/13-S3-step-and-pianoroll.md |
| 5.11 Tab(Normal에서 다음/이전 트랙) | 충족 | track_list.rs:233-243, `root::Tab` 삼키기 window.rs:735-753 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 이번 변경에 없음. `effective_setups`는 메인 스레드의 `prepare`에서만 불린다.
- 제품 코드의 `unwrap`/`expect`: 새로 들어온 것 없음. 인덱싱 `tracks[index]`(track_list.rs:168, `position`에서 나온 값), `members[0]`(tracks.rs:42, `len() > 1` 뒤), `taps[0]`(transport_bar.rs:183, `len() >= 2` 뒤)는 정당하다.
- 에러가 삼켜지는 곳: `edit_undo`·`edit_redo`의 실패는 메시지로 남는다. 버튼 클릭의 `me.update` 실패(`let _ =`)는 창이 사라진 경우뿐이라 정당하다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 트랜스포트 입력란이 모드 키 컨텍스트 안에 그려져, 입력란에 친 키가 모드 바인딩으로 먼저 빠진다. `space`는 재생을 토글하고(A1), vim 프리셋의 숫자는 카운트가 되어 템포를 칠 수 없다(A2). 문자 키(`a`, `c`, `w`…)도 같은 경로다. 입력란 포커스의 디스패치 경로에서 모드 컨텍스트를 빼든(모드 메뉴·팔레트처럼 컨텍스트 겹 밖에 그리기) 다른 방법이든, 실제 키(`simulate_keystrokes`)로 치는 테스트로 지킨다 | window.rs:502-519, 676 |
| 2 | 수정 | 템포·박자표 필드가 문서 값을 따라가지 않는다. undo/redo, 탭 템포, `transport::SetTempo(값)` 뒤에도 옛 값을 보이고(A3, A4), 그 상태로 필드를 눌러 Enter만 치면 옛 값이 다시 확정된다. 박자표 undo도 같은 경로다. 포커스가 없을 때 문서 변경을 필드에 반영해야 한다 | transport_bar.rs:68-81, 96·99·125·128; window.rs:87-90 |
| 3 | 수정 | 7.6 「패닉: 트랜스포트의 ⏹! 버튼」이 바에 없다. 단계 범위가 「7.6의 해당 행」이고 Action·처리기는 S2-T5a에 있다. 버튼은 실행 함수를 거쳐야 한다(결정 C) | transport_bar.rs:301-380 |
| 4 | 수정 | 계획 「트랙이 지워지면 이웃 트랙으로 옮긴다」가 `track::Delete`에만 맞다. undo/redo로 현재 트랙이 사라지면 첫 트랙으로 간다(A5). 이웃(사라진 자리, 없으면 앞)으로 가거나 계획을 고친다 | window.rs:205-213 |
| 5 | 제안 | SEQ-8 경고가 프로그램만 비교한다. 뱅크가 다르고 프로그램이 같은 공유 트랙은 음색이 다른데 경고가 없다. 반대로 프로그램 없음과 있음을 「다름」으로 세어, 프로그램 없는 트랙이 다른 트랙 음색을 그대로 쓰는 흔한 구성에도 경고가 뜬다. 어느 쪽이든 의도를 보고서에 적기를 권한다 | tracks.rs:29-46 |
| 6 | 제안 | `root::Tab` 삼키기 자체는 기존 기능(팔레트 Tab 자동완성, 모드 메뉴, 다른 모드의 `tab`)을 깨지 않았다(기존 테스트 전부 통과). 다만 입력란 포커스에서 `Tab`이 Normal의 `track::Next`로 가 현재 트랙이 바뀐다. 지적 1을 고치면 입력란 안 `Tab`이 무엇을 할지(5.11 「다음 필드」 또는 무시)를 정해 테스트로 남기기를 권한다 | window.rs:735-753 |
| 7 | 제안 | `transport::SetTempo`(값 없음)로 필드에 들어갈 때 내용을 전체 선택하면 바로 새 값을 칠 수 있다 | transport_bar.rs:155-158 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1–4를 고친 뒤 부록 A1–A5를 `tests/review_s2_t5b.rs`에 옮겨 통과시키고 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/13-S3`의 이월 한 줄은 계획 문서 갱신)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A: 깨지는 테스트

`crates/keyscore-app/tests/review_s2_t5b.rs`의 도우미(`start`, `open`, `click`, `press`, `run`, `tempo`, `ids`, `current`, `three_tracks`)와 import를 그대로 쓴다. 2026-10-17 e9e002e에서 실행해 모두 실패했다.

실패 출력 요약

~~~
space_in_tempo_field_does_not_run_transport: left: 1, right: 0
vim_digits_in_tempo_field: left: Tempo(500000), right: Tempo(666667)
tempo_field_follows_undo: left: Some("90.0"), right: Some("120.0")
tempo_field_follows_tap_and_set_tempo: left: Some("120.0"), right: Some("90.0")
undo_of_new_below_moves_to_neighbour: left: Some(TrackId(20)), right: Some(TrackId(22))
~~~

~~~rust
/// 템포 필드에서 `space`를 쳐도 재생이 바뀌지 않는다.
#[gpui_kit::test]
fn space_in_tempo_field_does_not_run_transport(cx: &mut TestAppContext) {
    start(cx, "space");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    let before = cx.read(|cx| view.read(cx).messages().len());
    press(cx, handle, "space");
    assert_eq!(cx.read(|cx| view.read(cx).messages().len()), before);
}

/// vim 프리셋에서 템포 필드에 숫자를 친다.
#[gpui_kit::test]
fn vim_digits_in_tempo_field(cx: &mut TestAppContext) {
    start(cx, "vim-digits");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace 9 0 enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
}

/// undo 뒤 필드가 문서 값을 보인다.
#[gpui_kit::test]
fn tempo_field_follows_undo(cx: &mut TestAppContext) {
    start(cx, "field-undo");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace");
    cx.simulate_input(handle, "90");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
    run(cx, handle, edit::Undo);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    let shown = cx
        .update_window(handle, |_, window, _| {
            window.find("transport-tempo").value().map(str::to_owned)
        })
        .unwrap();
    assert_eq!(shown.as_deref(), Some("120.0"));
}

/// 탭 템포 뒤 필드가 문서 값을 보인다.
#[gpui_kit::test]
fn tempo_field_follows_tap_and_set_tempo(cx: &mut TestAppContext) {
    start(cx, "field-tap");
    let (handle, view) = open(cx, Project::new());
    run(cx, handle, transport::SetTempo(Opt(Some(Real(90.0)))));
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
    let shown = cx
        .update_window(handle, |_, window, _| {
            window.find("transport-tempo").value().map(str::to_owned)
        })
        .unwrap();
    assert_eq!(shown.as_deref(), Some("90.0"));
}

/// 새 트랙을 undo하면 현재 트랙은 이웃 트랙으로 간다(계획 「트랙이 지워지면 이웃 트랙으로」).
#[gpui_kit::test]
fn undo_of_new_below_moves_to_neighbour(cx: &mut TestAppContext) {
    start(cx, "undo-neighbour");
    let (handle, view) = open(cx, three_tracks());
    let before = ids(cx, &view);
    run(cx, handle, keyscore_app::actions::cursor::LastTrack);
    run(cx, handle, track::NewBelow);
    assert_eq!(ids(cx, &view).len(), 4);
    run(cx, handle, edit::Undo);
    assert_eq!(current(cx, &view), Some(before[2]));
}
~~~
