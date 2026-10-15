# Task 보고: S1-T2 OS 포트 열거·핫플러그·재연결

- PR: 브랜치 `s1/t2-port-enumeration`
- 계획 파일: `docs/reports/s1-t2/plan.md`
- 계획 대비 달라진 점:
  1. 감시 스레드를 따로 두지 않는다. `MIDIServices.h`에 따르면 알림은 프로세스에서 처음 `MIDIClientCreate`가 불린 스레드의 run loop으로 온다(구현 중 실패로 확인). 그래서 crate의 모든 클라이언트 생성·해제를 run loop을 계속 돌리는 전용 스레드(`midi_thread`, 이름 `keyscore-coremidi`)에서 하고, `PortWatcher`는 그 스레드에서 알림 클라이언트를 만든다.
  2. 사라진 목적지로의 송신: CoreMIDI는 오류 없이 버린다(실측). 그래서 `send`에서 `PortGone`을 기대하지 않고, 제어 스레드용 `OsOutput::is_present`를 더했다. 다시 열기는 `PortGone`이다. 테스트 3의 이름과 기대를 바꿨다(계획 표에 적음).
  3. 테스트 5는 「스레드 종료」 대신 「drop이 1초 안에 끝나고 채널이 닫힘」을 본다(감시 스레드가 없어서).
  4. `MidiClient`는 클라이언트 핸들을 저장하지 않고 호출마다 캐시에서 읽는다. 서버 소멸 오류로 캐시가 비면 다음 호출이 다시 만든다.

## 구현 요약

| 파일 | 내용 |
| --- | --- |
| `src/ports.rs` | `PortInfo`, `PortEvent`, `diff_ports`(사라짐 먼저, 이름·ID순), `Reconnector`(이름 기반 재연결, 같은 이름은 최소 ID), `suggest_mapping`(OUT-8: `Present`/`Virtual`) |
| `src/os.rs` | `destinations()`(안정 열거), `OsOutput`(고유 ID로 열기, `MIDIOutputPortCreate`, `MIDISendEventList`, drop 때 `MIDIPortDispose`, `is_present`) |
| `src/watcher.rs` | `PortWatcher`: 알림 클라이언트와 C 콜백, `SetupChanged`마다 목록 비교, `catch_unwind`로 FFI 경계 보호, drop 때 해제 뒤 공유 상태를 놓아 채널을 닫음 |
| `src/midi_thread.rs` | CoreMIDI 전용 스레드: 작업 큐 + run loop(20 ms 단위), 작업 패닉 격리 |
| `src/client.rs` | `create_with_retry`(재시도 사이에 잠금을 놓음), `is_server_gone`(`kMIDIServerStartErr`, MIG -300..=-308), `forget_client_if_server_gone`, 송신 공용 `with_event_list` |
| `scripts/midi-soak.sh` | N회 반복 실행, 실패 회차 로그 보존, 회차 사이 쉬기(서버 유휴 종료 경로) |

## 테스트 결과

~~~
cargo test --workspace: 546 passed, 0 failed
scripts/midi-soak.sh 15 0: 15회 가운데 0회 실패
scripts/midi-soak.sh 4 6: 4회 가운데 0회 실패(회차 사이 6초, 서버 유휴 종료 뒤 재기동 포함)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `destinations_lists_virtual_destination_created_by_other_client` | 통과 |
| 2 | `os_output_send_reaches_destination` | 통과 |
| 3 | `os_output_after_destination_removed_is_not_present_and_cannot_reopen` | 통과(계획 변경, 위 2) |
| 4 | `watcher_reports_disappear_then_appear_in_order` | 통과 |
| 5 | `watcher_drop_closes_event_channel_promptly` | 통과(계획 변경, 위 3) |
| 6 | `reconnector_reconnects_when_same_name_returns` | 통과 |
| 7 | `reconnector_picks_lowest_id_among_same_names` | 통과 |
| 8 | `diff_orders_disappeared_before_appeared_by_name` | 통과 |
| 9 | `suggest_mapping_present_or_virtual` | 통과 |
| 10 | `simulated_port_vanish_and_return_reconnects_end_to_end` | 통과(단계 게이트) |
| 11 | `process_client_retry_does_not_hold_lock_while_sleeping` | 통과 |
| 12 | `server_gone_status_clears_cached_client` | 통과(상태 분류. 실제 서버 재시작은 시스템 프로세스를 죽여야 해서 수동 항목) |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (546 passed)
- [x] deny
- [x] 단계별 추가 게이트
  - `check-deps.sh` ok
  - `check-deps.test.sh` 15 passed
  - `bench-check.test.sh` 8 passed
  - `midi-soak.sh` 위 수치

## 알려진 한계

- 알림이 crate의 CoreMIDI 스레드로 오려면, 프로세스의 첫 CoreMIDI 클라이언트를 이 crate가 만들어야 한다. 앱(S2)은 시작할 때 `MidiClient::new`를 가장 먼저 부른다. 테스트도 그렇게 한다(`os_ports.rs`의 `init`).
- 서버 재시작 뒤 가상 포트를 다시 등록하는 일은 앱(S2)이 맡는다. 이 crate는 캐시된 클라이언트를 버려 다시 만들 수 있게만 한다.
- 실제 MIDI 서버 재시작 대응은 자동 테스트하지 않았다(시스템 프로세스를 종료하지 않는다).
- 드문 일괄 실패(S1-T1 이월)는 이번 반복 실행에서 재현되지 않았다. 재시도 한도 10초, 잠금 해제, 캐시 무효화로 원인 후보를 다뤘고, 다시 나면 `midi-soak.sh` 로그로 원인을 본다.

## 리뷰어에게

스스로 불안한 곳:

1. `midi_thread`
   - 「처음 `MIDIClientCreate`를 부른 스레드의 run loop」이라는 문서 문장에 기댄다. 다른 crate(`coremidi`)가 먼저 클라이언트를 만들면 알림이 오지 않는다.
   - 작업이 run loop 20 ms 단위로 처리되므로 클라이언트 생성이 최대 20 ms 늦을 수 있다.
2. `watcher.rs`
   - refcon을 `usize`로 넘기고 `Arc::from_raw`로 되돌린다. 해제 순서(클라이언트 해제 뒤 참조 해제)가 「해제 뒤 콜백 없음」 가정에 기댄다.
3. `Reconnector::apply`
   - `Disappeared`가 온 뒤 같은 이름의 다른 포트가 이미 있으면 Lost와 Reconnect를 함께 낸다.
   - 이벤트 순서(사라짐 먼저)에 기대므로, 같은 비교에서 사라짐과 나타남이 같이 오면 결과가 순서에 따라 달라지는지 봐 달라.
