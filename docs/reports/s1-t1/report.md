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

## 리뷰 대응 (review-2, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 16개 상한·이름 중복이 핸들 단위라 OUT-2 부분 충족 (차단) | 사람이 결정 `2026-10-15-virtual-port-limit-scope.md`에서 프로세스 단위를 골랐다. 모든 `MidiClient` 핸들이 프로세스 포트 목록을 나눠 쓴다. 상한을 채우거나 정확히 세는 테스트(구현자 1, 리뷰 1의 1, 리뷰 2의 2)는 자식 프로세스에서 돌게 했고, 입력과 기대는 그대로다. 리뷰 2의 거친 흔들기 테스트는 240개를 만들 수 없게 되어, 흔들 소스를 상한이 없는 `coremidi` crate 클라이언트로 만들도록 바꿨다(고정 소스는 그대로 `MidiClient`). `MidiClient`를 drop하는 리뷰 1 테스트 둘은 핸들이 해제할 것이 없어져 `clippy::drop_non_drop`만 껐다. 다른 핸들의 같은 이름을 허용하던 구현자 테스트는 거부를 기대하도록 바꿨다 | `plan: Decide the virtual port limit applies per process`, `midi: Enforce the 16 port limit and unique names per process` |
| 2 | NUL이 든 이름으로 같은 이름 소스 둘 (수정) | 제어 문자와 보이지 않는 서식 문자(ZWSP 등, BOM, soft hyphen)가 든 이름은 `InvalidName`이다 | `midi: Reject control and invisible characters in port names` |
| 3 | `plan.md` 미갱신 (수정) | `MidiClient` 의미, 상한·이름 규칙을 고치고 「개정」 절을 더했다 | `plan: Revise S1-T1 plan and carry review 2 suggestions to S1-T2 and S1-T3` |
| 4 | `source_names` 포기를 알리지 않음 (제안) | 최선의 근사라는 점과, 소멸 판정은 S1-T2 알림과 함께 쓴다는 점을 문서에 적었다 | `midi: Document that source_names is best effort under constant churn` |
| 5 | 서버 재시작 시 캐시된 클라이언트 회복 수단 없음 (제안) | S1-T2 이월로 단계 파일에 적었다 | 3과 같음 |
| 6 | 리뷰 1 제안 6의 S1-T3 의무 기록 위치 (제안) | S1-T3 이월로 단계 파일에 적었다 | 3과 같음 |
| 7 | 보이지 않는 문자로 구분 안 되는 이름 (제안) | 2와 함께 반영. 내부 연속 공백은 허용한다(보이는 차이가 있다) | 2와 같음 |

리뷰어 테스트를 바꾼 범위는 위 1의 설명이 전부다(실행 위치, 흔들 소스를 만드는 클라이언트, lint 허용). 워크스페이스 테스트 504개 통과.

## 리뷰 대응 (review-3, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | NFC·NFD 한글 이름이 둘 다 만들어짐 (수정) | 이름을 NFC로 맞춰 저장하고 그것으로 중복을 판정한다(`unicode-normalization`, MIT/Apache-2.0. unsafe FFI 대신 순수 Rust crate를 골랐다) | `midi: Normalize port names to NFC and reject default-ignorable characters` |
| 2 | 보이지 않는 문자 거부 목록이 근거와 어긋남 (수정) | 기준을 유니코드 `Default_Ignorable_Code_Point`와 줄·문단 구분자로 바꿨다. 양방향 제어(U+202A–202E, U+2066–2069, U+061C)도 거부한다. 보이는 글자가 없는 이름(점자 빈칸 등만)도 거부한다 | 1과 같음 |
| 3 | 과잉 거부(ZWJ 이모지, 페르시아어 ZWNJ)와 남은 빈틈(U+2028/2029, U+3164, U+2800) (제안) | 결합자·이체자 선택자·태그 문자는 받고, 나머지 빈틈은 막았다. 단위 테스트 둘을 더했다 | 1과 같음 |
| 4 | `run_isolated`가 테스트 이름 오타를 놓침 (제안) | 계획자가 넣은 헬퍼 셋 모두 「정확히 1개 통과」도 확인한다 | `midi: Make isolated test runs fail when the child runs no test` |
| 5 | 오류 문서가 옛 규칙 (제안) | `InvalidName`·`DuplicateName`과 `# Errors`를 고쳤다 | `midi: Bring error and Errors docs in line with the name rules` |
| 6 | 반복 실행 중 원인 모를 일괄 실패 (제안) | S1-T2 이월로 단계 파일에 적었다(서버 재시작 감지와 함께 재현·원인 확인) | `plan: Carry the rare whole-binary MIDI test failure to S1-T2` |

리뷰어 테스트는 바뀌지 않았다. 워크스페이스 테스트 509개 통과.
