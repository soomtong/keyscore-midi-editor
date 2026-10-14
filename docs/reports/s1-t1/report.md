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

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 마지막 클라이언트 해제로 MIDI 서버 종료, 이후 클라이언트 생성 영구 실패 (차단) | CoreMIDI 클라이언트를 프로세스에 하나만 만들고 해제하지 않는다. `MidiClient`는 자기 포트 목록만 갖는 핸들이다. 16개 상한과 이름 중복은 핸들 단위이고, 앱은 핸들을 하나 만들어 복제해 쓴다(문서에 적음) | `midi: Share one never-disposed CoreMIDI client per process` |
| 2 | 열거 중 앞쪽 소스가 사라지면 살아 있는 소스를 건너뜀 (수정) | 엔드포인트 목록을 읽고 이름을 모은 뒤 목록이 그대로인지 확인하고, 바뀌었으면 다시 읽는다(최대 8회) | `midi: Re-read source enumeration until the endpoint list is stable` |
| 3 | 테스트 간헐 실패 (수정) | `Source::from_name`은 재시도한다(2와 같은 커밋). 서버 재시작 직후의 `-304`는 클라이언트 생성을 3초까지 재시도해 흡수한다(1과 같은 커밋). 고친 뒤 약 80회 반복에서 1회 실패가 있었고, 원인 출력을 잡으려 다시 58회 돌렸지만 재현되지 않았다 | 위 두 커밋 |
| 4 | 앞뒤 공백만 다른 이름 허용 (수정) | 이름의 앞뒤 공백을 떼어 그 이름을 포트 이름으로 쓰고 중복도 그것으로 판정한다 | `midi: Trim port names before use and duplicate checks` |
| 5 | 불필요한 `unsafe impl Send/Sync` (수정) | 지웠다. 필드로 자동 구현된다 | 1과 같은 커밋 |
| 6 | 1.0 소스로 전달되지 않는 UMP 타입도 `Ok` (제안) | 반영하지 않았다. 리뷰어의 경계 테스트 `send_four_word_message_fits_stack_event_list`가 4워드 타입 0xD 송신의 `Ok`를 고정하고 있어, 타입 제한은 그 테스트와 충돌한다. 타입 검사는 S1-T3(인코딩)에서 인코더가 만드는 타입만 내보내는 방식으로 다룬다 | — |
| 7 | 포트 drop을 스케줄러 스레드에서 하지 말 것 (제안) | `MidiPort` 문서에 적었다. S1-T4에서 지킨다 | `midi: Name the default port, document drop off the scheduler thread` |
| 8 | 기본 이름 상수 없음 (제안) | `DEFAULT_PORT_NAME = "KeyScore Out"` | 위와 같음 |
| 9 | kill 테스트 자식이 고아로 60초 남을 수 있음 (제안) | 자식이 부모가 바뀌면(재부모화) 곧 끝난다 | 위와 같음 |

OUT-2(최대 16)는 핸들 단위라, 핸들을 둘 만들면 넘을 수 있다. 앱이 핸들을 하나만 두는 것은 S2(앱 셸)에서 지킨다. 리뷰 테스트 파일은 바뀌지 않았다. 워크스페이스 테스트 491개 통과.
