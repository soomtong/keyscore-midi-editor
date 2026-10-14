# Task 보고: S1-T1 CoreMIDI 가상 출력 포트

- PR: 브랜치 `s1/t1-virtual-port`
- 계획 파일: `docs/reports/s1-t1/plan.md`
- 계획 대비 달라진 점:
  - `HostTime::now`는 `mach_absolute_time`을 직접 선언해 쓴다(`unsafe extern` + `safe fn`). libc 의존을 더하지 않으려는 것이다.
  - 이름이 같은 포트로 `rename`하면 아무 일도 하지 않고 성공한다.
  - 테스트 3은 mach 틱 비율을 가정하지 않고, 경과 시간으로 재서 예약 시각을 만든다(Intel 1 ns, Apple Silicon 125/3 ns).

## 구현 요약

| 파일 | 내용 |
| --- | --- |
| `crates/keyscore-midi/src/client.rs` | `MidiClient`(Arc로 공유하는 CoreMIDI 클라이언트, 이름 목록 `Mutex`), `VirtualOutput`(`MIDISourceCreateWithProtocol(1.0)`, `rename`, drop 때 `MIDIEndpointDispose`), `MidiPort::send`(스택 `MIDIEventList`, `MIDIReceivedEventList`), `source_names` |
| `crates/keyscore-midi/src/port.rs` | `MidiPort` 트레이트(가상·OS 포트 공통, `Send + Sync`) |
| `crates/keyscore-midi/src/time.rs` | `HostTime`(mach 틱) |
| `crates/keyscore-midi/src/ump.rs` | UMP 메시지 타입별 워드 수 표 |
| `crates/keyscore-midi/src/error.rs` | `MidiError` |
| 의존 | `coremidi-sys` 3.2.1, `core-foundation` 0.10, `thiserror`. dev 의존은 `coremidi` 0.9.2로, 테스트에서 구현과 다른 경로의 독립 수신기로 쓴다 |

unsafe는 FFI 호출을 감싼 함수에만 있다. 함수마다 `#[allow(unsafe_code)]`와 SAFETY 주석을 달았고, crate는 `#![deny(unsafe_code)]`다. 송신 경로에는 힙 할당·락·로그가 없다. 단언(`assert_no_alloc`)은 S1-T4 게이트에서 한다.

## 테스트 결과

~~~
cargo test --workspace: 482 passed, 0 failed
keyscore-midi: 단위 2, 통합 9(+ 자식 프로세스 역할 1, ignored). 10회 반복 실행 모두 통과
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `virtual_output_create_is_visible_to_source_enumeration` | 통과 |
| 2 | `virtual_output_send_is_received_by_in_process_client` | 통과(바이트 `92 3c 64`, 타임스탬프 일치) |
| 3 | `virtual_output_future_timestamp_arrives_no_earlier_than_scheduled` | 통과 |
| 4 | `virtual_output_rename_updates_enumerated_name` | 통과 |
| 5 | `create_virtual_output_seventeenth_fails_with_too_many_ports` | 통과 |
| 6 | `create_virtual_output_duplicate_or_blank_name_fails` | 통과 |
| 7 | `send_rejects_zero_timestamp_and_wrong_word_count` | 통과 |
| 8 | `killed_process_leaves_no_virtual_source` | 통과(자식 SIGKILL 뒤 2초 안에 소스 사라짐) |
| 9 | `ump_word_count_matches_message_type` | 통과 |
| 10 | `virtual_output_and_client_are_send_and_sync` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음. 더한 테스트는 `now_is_nonzero_and_monotonic`이다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (482 passed)
- [x] deny
- [x] 단계별 추가 게이트
  - `check-deps.sh` ok(midi 내부 의존은 core만)
  - `check-deps.test.sh` 15 passed
  - `bench-check.test.sh` 8 passed

## 알려진 한계

- 영속 고유 ID(`kMIDIPropertyUniqueID`)를 두지 않는다. 신디가 앱 재시작 뒤 포트를 기억하는지는 S2에서 본다.
- 포트 이름 중복 검사는 같은 클라이언트 안에서만 한다. 다른 앱과의 이름 충돌은 막지 않는다.
- 헤드리스 CI 러너에서 CoreMIDI 통합 테스트가 도는지는 확인하지 못했다(remote 없음).
- 실제 신디(Surge XT 등) 수신은 S1 출구의 수동 항목이다.

## 리뷰어에게

스스로 불안한 곳:

1. `send`의 `MaybeUninit<MIDIEventList>` 사용
   - `MIDIEventListInit`/`Add`가 헤더와 패킷만 채운다는 가정에 기대고, 넘기는 크기는 `size_of::<MIDIEventList>()`다.
   - 구조체가 packed인지, 패킷 워드 배열이 64인지에 따라 4워드 메시지가 들어가는지 확인해 달라.
2. `VirtualOutput::drop`의 순서: 이름 목록에서 먼저 지운 뒤 엔드포인트를 해제한다. 두 단계 사이에 같은 이름으로 새 포트를 만들면, 해제 전 CoreMIDI에 같은 이름의 소스가 잠깐 둘 존재할 수 있다.
3. `unsafe impl Send/Sync for ClientInner`의 근거(CoreMIDI 스레드 안전성)
