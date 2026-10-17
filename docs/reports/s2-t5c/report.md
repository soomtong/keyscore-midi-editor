# Task 보고: S2-T5c 트랙 인스펙터와 상태줄

- PR: 브랜치 `s2/t5c-inspector-status`
- 계획 파일: docs/reports/s2-t5c/plan.md
- 계획 대비 달라진 점
  - 단계 파일의 S2-T5c에서 포트 감시(OUT-3)를 S2-T5d로 나눴다(계획 「나눔」, 단계 파일 갱신).
  - 공개 인터페이스에 `tracks::color_label`, `tracks::track_display_name`이 더해졌다. 상태줄 확인은 `status_text` 대신 접근성 레이블(`status-track`, `status-message`, `status-keymap-error`)로 한다.

## 구현 요약

- `src/fields.rs`(새 파일): 창의 한 줄 입력란 다섯 개(템포·박자표·이름·뱅크·기본 벨로시티)의 공통 규칙. S2-T5b가 트랜스포트 바에 두었던 규칙을 옮겨 일반화했다.
  - 고친 필드 판단(`shown`), Enter 확정(같은 값은 편집 없음), Esc 취소, 포커스 잃음 취소, 문서 변경 동기화.
  - 인스펙터 필드는 현재 트랙이 바뀌면 새 트랙 값으로 맞춘다(`follow_current_track`).
  - 이월(S2-T5b 리뷰 3 제안 5): 창이 비활성일 때의 Blur는 취소하지 않는다(macOS 관례). 트랜스포트 입력란에도 같다.
- `src/inspector.rs`(새 파일): 트랙 인스펙터.
  - 줄 구성: 이름(입력란), 색(12색 메뉴), 포트(트랜스포트와 같은 메뉴), 채널(1–16 메뉴, 10은 드럼), 뱅크(입력란, 비우면 없음), 프로그램(gpui-component `Select`, 검색 가능, 「없음」 + GM 128개), 기본 벨로시티(입력란 1–127), 가사 언어(없음·ko·en·ja + 지금 값), 뮤트·솔로 토글, 테스트 버튼.
  - Action 처리기: `track::Rename`, `SetColor`(팔레트 이름 또는 0–11), `SetPort`(빈 문자열·`virtual`은 가상 포트), `SetProgram{bank, program}`(GM 번호 1–128, 0은 없음, 뱅크 음수는 없음), `SetDefaultVelocity`, `SetLyricLang`(빈 문자열은 없음), `midi::SendTestNote`.
  - 편집은 `set_track_props` 한 번이 undo 한 단위다. 이미 그 값인 속성은 빼고, 남은 것이 없으면 편집하지 않는다.
- `src/gm.rs`(새 파일): GM 1 음색 128개와 `program_label`(`1 Acoustic Grand Piano`).
- 테스트 노트(PRE-3): 현재 트랙의 포트·채널로 가운데 C를 트랙 기본 벨로시티, 미리듣기 길이(200 ms)로 보낸다.
  - 정지 중이면 먼저 그 트랙의 뱅크·프로그램을 보낸다. 이름 포트는 이 프로젝트의 스냅샷을 넣어(재생하지 않음) 연다.
  - 이 창이 재생 중이면 지금 스냅샷의 포트로 보낸다(`Prepared::routes`, `Transport::routes`).
  - 다른 창이 재생 중이고 이름 포트면 보내지 않고 알린다.
- `src/window.rs`
  - 모드 키 컨텍스트 겹을 가운데 주 뷰만 감싸게 옮겼다. 트랙 목록·인스펙터·아래 패널·상태줄은 겹 바깥이다(이월 S2-T5b 리뷰 1 지적 1).
  - 상태줄(`render_status_bar`): 왼쪽 모드 표시, 그 옆 「트랙 · 포트 chN」, 오른쪽 키맵 오류(KM-6, 경고색, 있으면 메시지보다 먼저) 또는 마지막 메시지.
- `src/lib.rs`: 키맵을 다시 걸 때마다(핫리로드) `refresh_windows`로 상태줄을 다시 그린다.
- `src/tracks.rs`: `parse_bank`, `parse_velocity`, `parse_color`, `color_label`, `track_display_name`, `status_track_text`.

## 테스트 결과

~~~
cargo test --workspace: 1250 passed, 0 failed, 0 ignored
send_test_note_reaches_track_port: 이름 포트 목적지에서 프로그램 → Note On → Note Off 순서로 받음(정지 중), 재생 중에도 Note On 받음
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `inspector_logic.rs` 5개(`gm_program_names_have_128_entries_and_one_based_labels`, `parse_bank_accepts_empty_and_range`, `parse_velocity_accepts_1_to_127`, `parse_color_accepts_names_and_indices`, `status_track_text_names_track_port_and_channel`) | 통과 |
| 2 | `inspector_shows_current_track` | 통과 |
| 3 | `inspector_fields_edit_track_as_undo_units`, `inspector_field_escape_cancels` | 통과 |
| 4 | `inspector_typing_does_not_reach_mode_keys` | 통과 |
| 5 | `inspector_menus_edit_track` | 통과 |
| 6 | `inspector_program_select_searches_gm_names` | 통과 |
| 7 | `inspector_actions_validate_arguments` | 통과 |
| 8 | `inspector_mute_solo_test_buttons_run_actions` | 통과 |
| 9 | `send_test_note_reaches_track_port` | 통과 |
| 10 | `status_bar_shows_track_and_messages` | 통과 |
| 11 | `status_bar_shows_keymap_errors` | 통과 |
| 12 | `inactive_window_keeps_field_text` | 통과 |
| (추가) | `outside_panels_return_focus_to_document` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

추가 테스트: 키 컨텍스트 겹을 주 뷰로 좁히자 트랙 목록의 `+`·M/S 버튼을 누른 뒤 포커스가 버튼에 남아 모드 키가 먹지 않았다(S2-T5b 리뷰 테스트 두 개가 잡았다). 트랙 목록을 누르면 문서로 포커스를 돌리고, 프로그램 드롭다운이 닫히면(고름·`Esc`) 문서로 돌아가게 했다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`

## 알려진 한계

- 포트 감시·재연결·재등록, 상태줄의 포트 소멸·복귀 표시, 연 OS 포트 정리는 S2-T5d다. 정지 중 테스트 노트로 연 이름 포트도 `Engine::opened`에 남는다.
- 상태줄의 길이 코드·옥타브·박 표시(UI-5 일부, MET-2)는 S5다.
- 테스트 노트가 정지 중에 스냅샷을 넣는 것은 재생과 무관하다(다음 재생이 다시 넣는다). 넣은 스냅샷의 이벤트는 재생하지 않으면 나가지 않는다.
- 상태줄 오른쪽은 메시지 하나(마지막)만 보인다. 메시지 기록 보기는 없다.
- 키맵 오류가 여럿이면 첫 오류만 보인다.
- 트랙 이름은 비울 수 없다(목록·상태줄에서 알아볼 수 없다). 파일에서 읽은 빈 이름은 그대로 두고 「트랙 N」으로 부른다.

## 리뷰어에게

- 키 컨텍스트 겹을 주 뷰로 좁힌 변경(`window.rs` Render). 모드 키는 포커스 자리에서 디스패치되므로 같다고 보았다. 트랙 목록 클릭·버튼 뒤 포커스가 문서로 돌아오는지 확인 바란다.
- `render_inspector`가 그릴 때 `follow_current_track`과 `sync_program_select`를 부른다(현재 트랙은 `Window` 없는 곳에서도 바뀐다). 그리기 중 상태 변경이 다른 사건 순서에서 어긋나는지.
- 고치던 이름 입력란이 있는 채로 현재 트랙이 바뀌면(트랙 목록 클릭은 Blur로 먼저 취소된다. 키는 입력란이 받는다) 글자를 버린다. 다른 경로가 있는지.
- 프로그램 `Select`의 검색 입력란과 `Esc`, 선택 뒤 포커스.
- 테스트 노트의 세 경로(이 창 재생 중, 다른 창 재생 중, 정지 중)와 메시지 순서.
