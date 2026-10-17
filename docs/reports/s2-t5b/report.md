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
cargo test --workspace --no-fail-fast: 1166 passed, 0 failed
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
