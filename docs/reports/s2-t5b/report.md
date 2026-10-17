# Task 보고: S2-T5b 트랜스포트 바와 트랙 목록

- PR: 브랜치 `s2/t5b-transport-bar`
- 계획 파일: docs/reports/s2-t5b/plan.md
- 계획 대비 달라진 점
  - 꺼진 버튼(녹음, `AI✦`, 트랙 없을 때의 포트)은 동작으로 확인한다. gpui-component 버튼의 꺼짐 상태가 접근성 스냅샷에 실리지 않기 때문이다(라이브러리 한계, 아래 「알려진 한계」).
  - 회귀 두 건을 함께 고쳤다.
    - `root::Tab` 삼키기: 트랜스포트 바에 탭 정지점(입력란·버튼)이 생기자, 우리 키맵에 `tab`이 없는 모드(Lyric 등)와 팝업에서 gpui-component `Root`의 Tab 포커스 이동이 창 포커스를 입력란으로 옮겼다. S2-T3 리뷰 1 제안 2가 경고한 일이다. 창 루트에서 `root::Tab`·`root::TabPrev`를 삼킨다. 다음 필드로의 `Tab`(5.11)은 대화상자·설정 화면의 일이고 그 화면은 Root 레이어에 뜬다.
    - 모드 메뉴 위치: 상태줄 높이 근삿값(28px) 대신 모드 표시의 실제 위치(지난 프레임 경계)를 쓴다. S2-T3 리뷰 2 제안 3이다.
  - 팝업 칸을 하나로 일반화했다(상태줄 모드 메뉴, 조성, 출력 포트). 팝업은 하나만 뜬다.

## 구현 요약

- `src/tracks.rs`(새 파일): GPUI 없는 계산
  - `lowest_free_channel`(SEQ-7), `program_conflicts`(SEQ-8), `format_position`(`마디.박.틱`)
  - `parse_tempo`/`format_tempo`(BPM 소수 한 자리, 10.0–999.9), `parse_meter`/`format_meter`, `key_choices`/`key_label`, `port_label`
- `src/track_list.rs`(새 파일): 트랙 목록과 트랙 Action
  - 트랙 Action: New, NewAbove, NewBelow, Delete, ToggleMute, ToggleSolo, MoveUp·MoveDown(지우기+넣기를 한 undo 단위로), SetChannel, Next·Prev(`Tab`/`Shift+Tab`), `cursor::FirstTrack/LastTrack`
  - 목록 줄: 색·이름·「포트 · ch N」(채널 10은 「드럼」), SEQ-8 경고, M/S 토글. 아래에 `+`
  - 버튼은 그 줄의 트랙을 현재 트랙으로 삼고 Action을 실행 함수로 낸다
- `src/transport_bar.rs`(새 파일): 트랜스포트 바
  - 버튼: 재생·정지·녹음(꺼짐)·탭 템포·`AI✦`(꺼짐). 실행 함수를 거친다
  - 입력·선택: 템포·박자표 입력란(Enter 확정, 잘못된 값은 메시지), 조성 메뉴, 출력 포트 메뉴(가상 + OS 목적지, 현재 트랙의 포트)
  - 표시: 위치(재생 중에는 프레임마다 갱신), 메트로놈 토글
  - `transport::SetTempo`(값 없으면 템포 필드 포커스), `TapTempo`(최근 8탭 평균, 2초 쉬면 새로)
- `src/window.rs`
  - 현재 트랙. 문서가 바뀌어 사라지면 첫 트랙이 현재 트랙이 된다.
  - `edit_document`(undo 한 단위 + 문서 알림), `edit::Undo/Redo`, 메시지(`say`)
  - 팝업 일반화, `root::Tab` 삼키기, 모드 표시 위치 측정
- `src/engine.rs`: 재생 중 프로그램 비교를 (목적지, 채널)별 「실효 음색」으로 바꿨다. chase와 같은 규칙으로 정한다: 들리지 않는 트랙 먼저, 들리는 트랙은 트랙 순서, 뱅크·프로그램은 각각 마지막 값. 실효 음색이 바뀐 채널만 다시 보낸다. 이것으로 S2-T5a 이월(공유 채널 트랙 삭제, 솔로 이동, 들리지 않는 트랙만의 변경, 리뷰 5의 뱅크 참고)이 닫힌다.

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 1166 passed, 0 failed (리뷰 1 대응 뒤 1186, 리뷰 2 대응 뒤 1201 passed, 0 failed)
tracks_logic: 7, track_list: 5, transport_bar: 6, shared_channel: 3 passed
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `track_list::new_track_takes_lowest_free_channel` | 통과 |
| 2 | `track_list::track_list_rows_and_current_track` | 통과 |
| 3 | `track_list::track_actions_are_undo_units` | 통과 |
| 4 | `track_list::mute_solo_buttons_run_through_run_action` | 통과 |
| 5 | `transport_bar::transport_buttons_dispatch_actions` | 통과(꺼짐은 동작으로) |
| 6 | `transport_bar::tempo_meter_key_fields_edit_document` | 통과 |
| 7 | `transport_bar::tap_tempo_sets_tempo_from_intervals` | 통과 |
| 8 | `transport_bar::metronome_button_reflects_state` | 통과 |
| 9 | `transport_bar::port_dropdown_sets_current_track_port` | 통과(테스트가 만든 CoreMIDI 가상 목적지) |
| 10 | `track_list::seq8_warns_on_shared_channel_with_different_programs` | 통과 |
| 11 | `shared_channel::removing_later_shared_track_while_playing_restores_remaining_program` 외 2 | 통과. 이월 수용 테스트는 편집 직후 50 ms를 버리던 것을 모으게 고쳤다(실효 음색은 편집 즉시 나간다) |
| 12 | `transport_bar::position_display_follows_transport` | 통과 |
| (단위) | `tracks_logic` 7개 | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(1166 passed)
- [x] deny
- [x] check-deps

## 알려진 한계

- gpui-component 버튼(`Button`)의 꺼짐 상태가 접근성 트리에 실리지 않는다. 화면 읽기 프로그램에는 녹음·`AI✦` 버튼이 켜진 것처럼 들린다. 라이브러리 한계라 S8 접근성 점검에 넘긴다.
- 템포·박자표 필드의 끌어서 조정(7.6)은 S3-T6 이월이다(단계 파일).
- 트랙 이름·색 편집은 인스펙터(S2-T5c)에서 한다.
- 포트 메뉴는 열 때 OS 목적지를 열거한다(메인 스레드). 포트 감시·소멸 표시는 S2-T5c다.
- 새 트랙은 기본 가상 포트만 본다. 16채널이 다 찼으면 채널 1을 함께 쓰고 다른 포트를 권하는 메시지를 남긴다.

## 리뷰어에게

1. `root::Tab` 삼키기: 창 안 어디서도 `Tab`이 포커스를 옮기지 않는다. 트랜스포트 입력란 사이를 `Tab`으로 옮겨 다니는 것은 이제 안 된다(마우스·`transport::SetTempo`로 들어간다). 의도한 동작이지만 의견을 듣고 싶다.
2. 실효 음색 비교의 규칙이 chase와 정확히 같은지(재생 시작의 이벤트 순서와 `effective_setups`의 순서).
3. 현재 트랙 보존(undo로 트랙이 사라지거나 돌아올 때)과 트랙 순서 이동(지우기+넣기)의 undo.
4. 템포 필드의 확정·되돌림(포커스를 잃으면 문서 값으로 되돌린다)과 IME 입력.

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 트랜스포트 입력란에 친 키가 모드 바인딩으로 먼저 빠진다(`space` 재생, vim 숫자 카운트) | 트랜스포트 바를 모드 키 컨텍스트 겹 바깥(창 루트의 첫 줄)에 그린다. 입력란의 디스패치 경로에 `Normal` 등이 없다. 인스펙터 입력란도 같게 하라고 S2-T5c 이월에 적었다 |
| 지적 2: 필드가 undo·탭 템포·`SetTempo` 뒤 옛 값을 보인다 | 문서가 바뀌면(`observe_in`) 입력 중이 아닌 필드를 문서 값으로 맞춘다 |
| 지적 3: 7.6 「패닉: 트랜스포트의 ⏹! 버튼」이 없다 | 정지 옆에 `⏹!` 버튼을 더했다(실행 함수) |
| 지적 4: undo로 현재 트랙이 사라지면 첫 트랙으로 간다 | 현재 트랙의 마지막 자리를 기억하고, 사라지면 그 자리의 이웃(없으면 앞 트랙)으로 간다 |
| 제안: SEQ-8 비교 대상 | REQ SEQ-8은 「프로그램이 다르면 경고」라 프로그램만 비교한다. 프로그램 없음과 있음은 다른 것으로 센다(없음은 신디의 지금 음색이 그대로라 다른 트랙의 음색이 남는다). 뱅크만 다른 경우는 경고하지 않는다 |
| 제안: 입력란 안의 `Tab` | 아무 일도 하지 않는다(트랙 이동도, 포커스 이동도 없음). 테스트 `tab_inside_tempo_field_does_nothing` |
| 제안: `SetTempo`(값 없음)로 필드에 들어갈 때 전체 선택 | 그렇게 했다 |

부록 A 테스트 5개와 새 테스트 2개(패닉 버튼, 입력란 `Tab`)는 `tests/review_s2_t5b.rs`에서 통과한다.

## 리뷰 2 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1(회귀): 메트로놈 버튼을 누르면 포커스가 버튼에 남아 모드 키가 먹지 않는다 | 트랜스포트 바의 버튼·토글은 누른 뒤 문서로 포커스를 돌린다 |
| 지적 2: 입력란의 `cmd-z`가 문서를 되돌린다 | 입력란에 포커스가 있으면 `edit::Undo/Redo` 처리기가 처리하지 않고 넘겨(`propagate`) 입력란의 되돌리기가 받는다(KM-14). 메뉴의 되돌리기도 같다(macOS 관례) |
| 지적 3: 포커스만 있고 고치지 않은 필드가 옛 값을 보이고, Enter가 다른 경로의 편집을 덮는다 | 필드마다 마지막으로 앱이 넣은 글자를 기억하고, 지금 글자가 그와 다를 때만 「사람이 고친 필드」로 본다(입력란의 변경 사건은 값을 넣은 뒤에도 늦게 올 수 있어 사건으로 세지 않는다). 고치지 않은 필드는 문서가 바뀌면 바로 따라간다 |
| 지적 4: 입력란의 `Esc`가 아무것도 취소하지 않는다 | 입력란을 감싼 요소가 수식키 없는 `Esc`를 먼저 받아 입력을 취소하고 문서로 돌아간다(A13) |
| 지적 5: 고치지 않은 필드의 Enter가 같은 값으로 편집을 만든다 | 고치지 않은 필드의 Enter와, 고쳤어도 값이 문서와 같은 확정은 편집을 만들지 않는다. 탭 템포·`SetTempo`도 같은 값이면 편집하지 않는다 |
| 제안 1: 입력란에서 `ctrl-tab`으로 모드가 바뀌어도 포커스가 입력란에 남는다 | 모드가 바뀔 때 입력란에 포커스가 있으면 고치던 입력을 취소하고 문서로 포커스를 옮긴다 |
| 제안 2: 한 필드에 포커스가 있으면 두 필드 모두 동기화를 건너뛴다 | 필드마다 따로 본다(지적 3과 같은 수정) |

부록 B 테스트 6개는 `tests/review_s2_t5b_r2.rs`로 옮겨 통과한다. 두 개는 전제를 고쳤다.
- `enter_after_tap_while_field_focused_keeps_tapped_tempo`: 「버튼이 입력란 포커스를 가져가지 않는다」는 전제가 지적 1의 수정과 반대다. 탭 뒤 필드를 다시 누르고 고치지 않은 채 Enter를 치는 경로로 같은 결함을 본다.
- `enter_after_menu_undo_while_field_focused`: 「입력란에 포커스가 있을 때 메뉴 되돌리기가 문서를 되돌린다」는 전제가 KM-14와 반대다. `transport::SetTempo`로 같은 결함을 본다.

## 리뷰 3 대응

재리뷰 한도에 닿아 결정 요청을 냈다(`docs/decisions/2026-10-17-s2-t5b-review-limit.md`). 결정 A에 따라 지적 세 건을 고치고 좁은 확인 리뷰를 받는다.

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1(회귀): 명령 팔레트 질의의 `cmd-z`가 문서를 되돌린다 | 「포커스가 텍스트 입력란에 있는가」를 필드 목록이 아니라 포커스 경로의 키 컨텍스트에 `Input`이 있는지로 본다. 트랜스포트 필드, 팔레트 질의, 이후 인스펙터 입력란이 모두 같다 |
| 지적 2: 입력란 포커스에서 메뉴의 되돌리기가 아무것도 하지 않는다 | `propagate` 대신 입력란의 `input::Undo`/`input::Redo`를 직접 디스패치한다. 키와 메뉴 두 경로가 같게 입력란을 되돌린다 |
| 지적 3: 조성·포트 메뉴에서 지금 값을 다시 고르면 빈 undo 단위가 생긴다 | 고를 때의 문서 값과 같으면 편집하지 않는다 |
| 제안 4: 문서가 바뀐 뒤 옛 기준과 같은 글자를 친 확정이 버려진다 | 문서가 바뀌면 고친 필드의 기준을 새 문서 값으로 옮긴다(글자는 그대로 둔다) |
| 제안 5: 창이 비활성이 되면 고치던 글자가 되돌려진다 | 알려진 한계로 둔다. 입력란 포커스를 잃는 다른 경로와 같은 규칙(취소)이고, 창 비활성화를 구분하는 일은 S2-T5c 인스펙터 입력란과 함께 정한다 |
| 제안 6: 기존 테스트 창이 비활성이다 | `tests/transport_bar.rs`의 `open`이 창을 활성으로 둔다 |
| 제안 7: window.rs 주석이 사실과 다르다 | 「팝업·패널이 닫히며 포커스한 요소가 사라질 때」로 고쳤다 |

부록 C 테스트 5개는 `tests/review_s2_t5b_r3.rs`로 옮겨 통과한다. 메뉴 경로 테스트에 다시 하기(`edit::Redo`)를 더했다.
