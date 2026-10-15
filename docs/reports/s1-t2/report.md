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

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

주 세션(구현자)이 직접 고쳤다.

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 같은 이름·고유 ID로 곧바로 다시 만든 목적지를 놓침 (차단) | `PortInfo`에 CoreMIDI 객체 참조를 붙이고, `diff_ports`는 참조가 바뀌면 사라짐+나타남으로 본다. 같음 비교는 이름과 ID 그대로다. 리뷰 테스트가 기대값 그대로 통과한다 | `midi: Report a destination recreated with the same name and ID` |
| 2 | `send` 오류 경로의 `String` 할당 (차단) | 송신 오류는 데이터 없는 `DestinationGone`으로 바꿨다. `PortGone(String)`은 스케줄러 밖의 `open`에만 쓴다(리뷰 테스트의 `PortGone(String)` 비교를 깨지 않으려고 타입은 그대로 뒀다) | `midi: Return an allocation-free error when a send target is gone` |
| 3 | 다른 코드가 첫 클라이언트를 만들면 감시가 소리 없이 죽음 (수정) | `start`가 잠깐 탐침 소스를 만들었다 지워 알림이 CoreMIDI 스레드로 오는지 확인하고, 아니면 `NotificationsUnavailable`이다. 그 오류 경로에서는 공유 상태를 놓지 않는다(다른 스레드 콜백 대비) | `midi: Fail PortWatcher::start when notifications miss the CoreMIDI thread` |
| 4 | 알림 묶음에 따라 재연결 대상이 달라짐 (수정) | 연결 대상은 언제나 지금 있는 같은 이름 포트 가운데 최소 ID다. 더 작은 ID가 나타나면 옮긴다. 이 규칙과 어긋나던 구현자 단위 테스트의 기대를 바꿨다 | `midi: Keep reconnection on the lowest unique ID among same-name ports` |
| 5 | 서버 소멸 분류가 과잉·누락 (수정) | `kMIDIInvalidClient`, `kMIDIServerStartErr`, `MIG_SERVER_DIED`만 캐시를 버린다. -304 등은 생성 재시도가 다룬다 | `midi: Narrow server-gone statuses and test that the cache is cleared` |
| 6 | 캐시 비우기를 실제로 시험하지 않음 (수정) | `forget_in`으로 떼어 단위 테스트가 슬롯을 직접 본다(새 클라이언트는 건드리지 않음 포함). 5와 한 커밋이다(테스트가 좁힌 분류를 함께 시험한다) | 5와 같음 |
| 7 | 가상 포트 재등록을 결정 없이 미룸, 캐시를 버린 뒤 이름이 남음 (수정) | `VirtualOutput::is_registered`·`reregister`를 더했다(같은 이름·자리로 새 엔드포인트, 성공 뒤 옛 것 해제). 이름 목록은 살아 있는 `VirtualOutput`이 들고 있어 재등록에 그대로 쓴다. 서버 재시작을 알아차려 모든 포트를 다시 등록하는 일은 포트를 소유한 앱의 몫이라 S1·S2 단계 파일 이월에 적었다 | `midi: Let apps re-register virtual ports after a MIDI server restart` |
| 8 | 스레드 이름으로 CoreMIDI 스레드를 판별 (제안) | 스레드 id로 판별한다 | `midi: Recognize the CoreMIDI thread by thread id instead of name` |
| 9 | 만들 수 없는 `Virtual` 제안 (제안) | 가상 포트 이름 규칙으로 정규화하고, 안 되면 `Unavailable`이다 | `midi: Suggest only creatable virtual port names in port mapping` |
| 10 | soak 스크립트가 이전 실패 로그를 지움 (제안) | 실행마다 새 디렉터리를 쓴다 | `midi: Keep soak logs from earlier runs in separate directories` |
| 11 | 20 ms 처리 지연 (제안) | 코드 변경 없음. 클라이언트·포트 생성과 해제에만 드는 지연이고 송신 경로와 무관하다 | — |

절차상 알릴 것:
- 지적 1 커밋을 test 대상 clippy 전에 커밋해 lint 하나가 남았고, 바로 다음 커밋(`midi: Fix a clippy lint in the recreated port diff test`)으로 고쳤다.
- 지적 7 커밋에 단계 파일 변경이 함께 들어갔다.

리뷰어 테스트는 바뀌지 않았다. 워크스페이스 테스트 558개 통과. `midi-soak.sh 10 0` 0회 실패, `midi-soak.sh 3 6` 0회 실패.

## 리뷰 대응 (review-2, 판정: 병합 가능)

지적은 모두 제안이다.

| # | 제안 | 대응 |
| --- | --- | --- |
| 1 | 탐침 중 CoreMIDI 실패도 `NotificationsUnavailable`로 보고 | 반영하지 않음. 드문 경로이고 시작 실패라는 결과는 같다 |
| 2 | 탐침 소스가 다른 앱 목록에 잠깐 보임 | 받아들인다. 이름(`KeyScore watcher probe`)으로 정체가 드러나고, 시작 때 한 번만 생긴다 |
| 3 | `start` 블로킹(약 0.25초, 최악 2초) | S2 이월 항목에 「메인 스레드가 아닌 곳에서 부른다」를 더했다 |
| 4 | 옛 엔드포인트가 살아 있을 때 `reregister`로 같은 이름이 잠깐 둘 | 받아들인다. 서버 재시작 뒤에만 부르는 함수다 |
| 5 | 서버 재시작 뒤 참조 재사용 시 `is_registered` 거짓 참 가능 | 수동 확인 항목으로 남긴다(시스템 프로세스를 종료하지 않는다) |
| 6 | `Virtual` 제안끼리 겹침, 16개 초과 | S2 이월 항목에 「UI에서 만들기 전에 확인」을 더했다 |
| 7 | 참조 0인 `PortInfo`끼리의 `diff_ports` | 받아들인다. 호출자는 감시자뿐이다 |
| 8 | 서버 재시작 뒤 감시자 재시작 | S2 이월 항목에 더했다 |
| 9 | soak 스크립트의 빈 디렉터리, 실행 위치 | 받아들인다. 개발 도구다 |

이월 기록 커밋: `plan: Record S1-T2 review 2 responses`.

## 맺음

리뷰 2회로 「병합 가능」 판정을 받았다.
