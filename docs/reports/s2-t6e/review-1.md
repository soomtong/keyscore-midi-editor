# 적대적 리뷰: S2-T6e 포트별 MIDI Clock, 테스트 노트, 포트 상태 표시 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `e74843e`(브랜치 `s2/t6e-clock-test-note`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t6e.rs`(10개, 감시를 켜는 3개는 자식 프로세스에서 혼자 돈다)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 정지 중 교체(설정 창 테스트 노트의 [기본, X] 스냅샷, 포트 삭제, 서버 복구, 인스펙터 정지 중 테스트 노트)는 클럭 마스크를 건드리지 않는다. 마스크의 번호 1이 남은 채 다음 재생에서 짝이 같아 명령이 없을 때 Start·Clock이 X로 새지 않는가.
2. 재생하는 창이 바뀔 때(A 재생 중 B 재생) 앞 재생의 Stop은 A 스냅샷의 클럭 포트로, 새 Start·Clock은 B 스냅샷의 클럭 포트로만 가는가. B의 트랙 포트가 같은 번호를 쓸 때 새지 않는가.
3. 재생 중 교체로 번호가 바뀌어도(트랙을 앞에 더함) 클럭과, 같은 포트를 쓰는 메트로놈 클릭이 함께 옮겨 가는가.
4. 클럭 전용 이름 포트(트랙·메트로놈이 쓰지 않는 목적지)가 재생 중 사라지거나 나타나면 포트 감시가 그 이름을 보는가. `wanted_names`·`windows_using`에 클럭 목록이 없다(OUT-3 자동 재연결).
5. 재생 시작 때 없던 클럭 포트는 `known_missing`에 들어간다. 그 뒤 편집(live 준비)으로는 다시 찾지 않는다. 무엇이 「없음」 기억을 지우는가.
6. 정지 중 테스트 노트를 X, Y로 빠르게 보내면 같은 번호 1에 다른 포트가 온다. X의 Note Off가 X로 가는가.
7. 클럭 체크 상자가 Tab 정지점이고 space로 켜지는가.
8. 설정 파일이 기본 포트를 이름(`{"named": 기본 포트 이름}`)으로 가리키면 목록에 같은 포트 줄이 둘 생기고 같은 요소 id를 쓴다. 보이는 상태와 실제 송신이 어긋나는가.
9. 포트 상태 줄의 안내가 고른 포트가 아니라 늘 기본 포트 이름을 말한다.
10. 가상 포트를 지워도 클럭 목록에 그 이름이 남는다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 정지 중 교체 뒤 다음 재생 | 없음 | 리뷰 테스트로 더함(통과) |
| 2 재생 창 바뀜 | 없음 | 리뷰 테스트로 더함(통과) |
| 3 번호 바뀜 | 있음(`clock_toggle_while_playing_and_survives_edits`) | 메트로놈과 같은 포트인 경우는 없음. 리뷰 테스트로 더함(통과) |
| 4 클럭 포트 소멸·복귀 | 없음 | 깨짐(지적 1) |
| 5 「없음」 기억 | 없음 | 대조군으로 확인(지적 1의 근거) |
| 6 테스트 노트 연속 | 없음 | 통과 |
| 7 체크 상자 키보드 | 없음 | 통과(실제 키 눌림·뗌을 보내야 한다. `simulate_keystrokes`는 뗌을 보내지 않는다) |
| 8 기본 포트 이름 항목 | 없음 | 통과(「켬」으로 보인다) |
| 9 안내 문구의 포트 | 없음 | 제안 1 |
| 10 지운 포트의 클럭 항목 | 없음 | 제안 2(메트로놈 포트와 같은 태도라 버그로 보지 않음) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 재생 시작 때 없던 클럭 전용 목적지가 재생 중 나타남(감시 켬) | 깨짐: 1.5초 감시를 돌려도 Clock 0개(`[]`) | `review_clock_port_appearing_during_play_gets_clock` |
| 2 | 재생 중 클럭 전용 목적지가 사라졌다 같은 이름으로 돌아옴(감시 켬) | 깨짐: 돌아온 목적지로 Clock 0개(`[]`) | `review_clock_port_return_reconnects` |
| 3 | 대조군: 1과 같은 상황에서 편집(트랙 더하기)은 잇지 못하고, 정지 뒤 다시 재생하면 Clock이 온다 | 못 깨뜨림(대조군 통과: 하네스·스케줄러는 늦게 나타난 목적지로 보낼 수 있다) | `review_control_clock_port_appearing_needs_replay` |
| 4 | 재생 중 클럭 전용 목적지 소멸의 알림 | 못 깨뜨림: 알림은 있다. 다만 「쓰지 않는 포트」의 핫플러그 문구(「…이 없어졌다」)로 나온다 | `review_clock_port_vanish_is_reported` |
| 5 | 재생·정지 뒤 정지 중 설정 창 테스트 노트([기본, X]), 다시 재생 | 못 깨뜨림: X로 Start·Clock 0, 클럭 포트로 Start 1 | `review_start_goes_to_clock_port_after_stopped_test_note` |
| 6 | A(가상 포트 트랙) 재생 중 B(이름 포트 트랙, 번호 1) 재생 | 못 깨뜨림: B 트랙 포트로 실시간 메시지 0, 클럭 포트로 Stop·Start·Clock | `review_window_switch_keeps_clock_on_its_port` |
| 7 | 메트로놈·클럭 같은 이름 포트, 재생 중 트랙을 앞에 더해 번호 바꿈 | 못 깨뜨림: 새 트랙 포트로 아무것도 안 감 | `review_metronome_and_clock_share_port_across_renumber` |
| 8 | 정지 중 테스트 노트 X 뒤 곧바로 Y | 못 깨뜨림: X Note Off가 X로, Y는 Note Off 하나, 마지막 기록은 Y | `review_test_note_off_reaches_first_port_after_quick_second` |
| 9 | 테스트 노트 버튼에서 Tab 한 번 뒤 space(키 눌림·뗌) | 못 깨뜨림: 기본 포트 클럭이 켜진다 | `review_clock_checkbox_reachable_by_keyboard` |
| 10 | 설정 파일 클럭 `[{"named": 기본 포트 이름}]` | 못 깨뜨림: 포트 0으로 Clock, 설정 줄 「켬」 | `review_clock_named_main_port_shows_on` |

실행: `cargo test -p keyscore-app --test review_s2_t6e` → 8 통과, 2 실패(시도 1, 2).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 클럭 번호는 늘 지금 스냅샷의 번호 | 재생 중에는 못 깨뜨림 | 정지 중 교체는 마스크를 남기지만 스케줄러는 정지 중 클럭을 보내지 않고(`Command::Stop`은 `playing`일 때만 0xFC), 재생 시작이 늘 교체 뒤 명령을 맞춘다. 시도 5·6·7 통과 |
| 설정의 클럭 목록은 메트로놈 포트와 같은 모양, 중복 없음 | 못 깨뜨림 | `clock_settings_round_trip`, 이름 변경 때 `retain`+`push` |
| 클럭 포트는 트랙·메트로놈 포트와 같은 규칙으로 연다(계획 끝의 정의 1) | 깨짐 | 여는 규칙은 같지만 포트 감시(`ports.rs:131` `wanted_names`, `ports.rs:153` `windows_using`)가 클럭 이름을 보지 않아 재연결 규칙이 다르다(지적 1) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-5 | 충족 | 설정 > MIDI 출력 체크 상자(`settings_ports.rs:340`), 기본 끔, 24 ppqn·Start/Stop/Continue/SPP는 스케줄러. 시도 5·6 |
| OUT-3(클럭 포트에 대해) | 미충족 | 클럭 전용 포트의 소멸은 「쓰지 않는 포트」 문구로만 알리고, 같은 이름이 돌아와도 자동 재연결하지 않는다(`ports.rs:131`, `ports.rs:153`). 시도 1·2 |
| PRE-2 | 충족(SP-4 범위) | `settings_ports.rs:318` 가상 포트 수·마지막 테스트 노트·안내. 리스너 표시는 SP-4로 불가 |
| PRE-3 | 충족 | 설정 > MIDI 출력 버튼과 `midi::SendTestNote`(`settings_ports.rs` `ports_test_note`), 인스펙터 「테스트」. 설정 창은 가상 포트만 고를 수 있다(목적지는 인스펙터). 제안 3 |
| 7.6 연결 테스트 노트 | 충족 | 설정 > MIDI 출력, 트랙 인스펙터, 팔레트 「테스트 노트 보내기」 |
| UI-11 | 충족 | 클럭·테스트 노트 모두 설정 화면에 보인다. 키보드로도 닿는다(시도 9) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `keyscore-midi`를 바꾸지 않았다(`git diff main...HEAD --stat`).
- 제품 코드의 `unwrap`/`expect`: 새로 더한 것 없음(`unwrap_or`, `unwrap_or_default`만).
- 에러가 삼켜지는 곳
  - 재생 중 교체에서 `scheduler.swap`이 실패하면 이미 큐에 넣은 클럭 끄기는 되돌리지 않고 `clock_state`는 `None`으로 남는다. 다음 편집이 맞추지만 그때까지 클럭이 끊기고 알림은 「재생 중 편집을 반영하지 못했다」뿐이다(클럭 언급 없음). 동작은 계획대로라 지적하지 않는다.
  - 재생 시작의 `clock_missing`은 `say_once`로 알리고, 재생 중 교체마다 같은 문구는 다시 남기지 않는다. 의도대로다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 클럭 전용 이름 포트가 포트 감시 대상이 아니다. `wanted_names`·`windows_using`이 메트로놈 포트와 트랙 포트만 본다. 그래서 (a) 재생 시작 때 없던 클럭 포트가 재생 중 나타나도 Clock이 가지 않는다. 재생 시작이 그 이름을 `known_missing`에 넣어 편집(live 준비)으로도 다시 찾지 않고, 정지 뒤 다시 재생해야 이어진다. (b) 재생 중 사라졌다 같은 이름으로 돌아와도 `port_lost`·`port_returned`가 불리지 않아 편집 전까지 Clock이 끊긴다. (c) 소멸·출현 알림이 「쓰지 않는 포트」 문구로 나온다. 계획의 「트랙·메트로놈 포트와 같은 규칙」과 OUT-3 자동 재연결에 어긋난다. 재현: `review_clock_port_appearing_during_play_gets_clock`(실제: Clock `[]`), `review_clock_port_return_reconnects`(실제: Clock `[]`). 대조: `review_control_clock_port_appearing_needs_replay` 통과 | `crates/keyscore-app/src/ports.rs:131`, `crates/keyscore-app/src/ports.rs:153` |
| 2 | 제안 | 포트 상태 줄의 안내가 고른 포트와 상관없이 늘 기본 포트 이름(「…에서 「기본」을 고른다」)을 말한다. 추가 포트로 테스트 노트를 보낸 뒤에는 그 포트 이름을 말해야 계획의 「이 포트를 고른다」와 맞다 | `crates/keyscore-app/src/settings_ports.rs:326` |
| 3 | 제안 | 가상 포트를 지워도 `midi_output.clock`에 그 이름이 남는다. 메트로놈 포트와 같은 태도라 버그는 아니지만, 같은 이름의 목적지가 생기면 거기로 Clock이 간다. 보고서의 「알려진 한계」에 적거나 지울 때 함께 빼는 것을 검토한다 | `crates/keyscore-app/src/engine.rs:350` |
| 4 | 제안 | 설정 창의 테스트 노트는 가상 포트로만 보낸다. PRE-3 「지정 포트」를 목적지까지 넓히려면 클럭 목록처럼 포트 선택지를 쓸 수 있다(인스펙터가 목적지를 맡으므로 비차단) | `crates/keyscore-app/src/engine.rs:448` |
| 5 | 제안 | `port_key`의 주석은 「이름으로 가리킨 기본 포트도 빈 문자열이다」라고 하지만 `Named(기본 포트 이름)`은 그 이름을 열쇠로 쓴다. 번호는 0으로 같아 동작은 맞다. 주석을 고친다 | `crates/keyscore-app/src/engine.rs` `port_key` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1을 고치고 `review_clock_port_appearing_during_play_gets_clock`, `review_clock_port_return_reconnects`가 통과해야 한다. 고칠 때 「재생 시작이 클럭 이름을 `known_missing`에 넣은 뒤 나타남」 경로(감시가 그 이름을 「없음」으로 등록해 두어야 나타남이 `Reconnect`가 된다)를 함께 본다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 리뷰 중 알게 된 하네스 주의

- 클럭 체크 상자는 키를 뗄 때 켠다. `simulate_keystrokes`는 키 뗌을 보내지 않아 `PlatformInput::KeyDown`·`KeyUp`을 직접 보냈다.
- 포트 감시를 켜는 테스트는 같은 프로세스의 다른 테스트와 겹치면 감시 시작이 늦거나 실패해 자식 프로세스(`run_isolated`)로 돌렸다. 감시 시작 전에 다른 `coremidi` 클라이언트(테스트 목적지)를 만들면 자식 프로세스에서 감시가 시작되지 않아, 목적지는 `start` 뒤에 만든다.
- 감시는 원하는 이름을 이벤트 처리 뒤에 등록한다. 재생과 같은 주기에 목적지를 만들면 「나타남」이 아니라 처음부터 「연결됨」으로 등록되어 재연결 동작이 나오지 않는다. 시도 1은 한 주기 돌린 뒤 목적지를 만든다.
