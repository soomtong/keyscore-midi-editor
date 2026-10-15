# 적대적 리뷰: S1-T1 CoreMIDI 가상 출력 포트 (리뷰 1회차)

- 리뷰어 세션: 구현 맥락을 물려받지 않은 별도 서브 에이전트 세션(구현자와 다른 세션)
- 대상 커밋: `fd883b2` (브랜치 `s1/t1-virtual-port`, `git diff main...HEAD`)
- 남긴 테스트: `crates/keyscore-midi/tests/review_s1_t1.rs` (실패 4, 통과 5, 자식 역할 1 ignored)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트 파일을 열기 전에 `src/*.rs`만 읽고 적었다.

1. `MaybeUninit<MIDIEventList>`의 크기·정렬과 `MIDIEventListAdd`에 넘기는 크기. 4워드 메시지가 들어가는가.
2. `MIDIObjectGetStringProperty`가 create rule인지(이중 해제·누수).
3. `unsafe impl Send/Sync for ClientInner`가 정말 필요한가. 필드만으로 자동 구현되면 unsafe 범위가 최소가 아니다.
4. `ClientInner::drop`의 `MIDIClientDispose`가 CoreMIDI 문서상 안전한 시점인가(마지막 클라이언트 해제).
5. drop 순서: 이름 목록에서 지운 뒤 엔드포인트를 해제하는 사이에 같은 이름 재생성이 끼어드는가. 잠금 범위.
6. 여러 스레드 동시 생성으로 16개 상한·이름 중복을 우회할 수 있는가. 클라이언트를 둘 만들면 상한이 앱 전체에 적용되는가.
7. 이름 중복 검사가 공백·대소문자 변형을 막는가(`valid_name`은 trim을 빈 이름 판정에만 쓴다).
8. `source_names`의 인덱스 열거가 열거 중 소스 소멸에 안전한가(인덱스가 당겨져 다른 소스를 건너뛰는가).
9. `send`의 할당·락·로그 여부, 워드 수 표, 과거·아주 먼 미래 타임스탬프, 1.0 소스에 보내는 2.0·예약 타입 메시지의 결과.
10. 시스템 전역 CoreMIDI 상태 때문에 테스트끼리 간섭하는가. 자식 프로세스 테스트가 고아를 남기는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 4워드 메시지·버퍼 크기 | 없음 | 2워드 이하만 보낸다. 리뷰 테스트 `send_four_word_message_fits_stack_event_list` 추가(통과) |
| 2 create rule | 간접(열거 테스트가 이름을 읽음) | 누수·이중 해제는 테스트로 드러나지 않는다. 헤더 67행 규칙과 대조해 맞음 |
| 3 unsafe impl 필요성 | 없음 | `virtual_output_and_client_are_send_and_sync`는 성질만 본다. 불필요함을 확인 |
| 4 마지막 클라이언트 해제 뒤 재생성 | 없음 | 깨짐 |
| 5 drop 중 같은 이름 재생성 | 없음 | 경계 테스트 추가(통과) |
| 6 동시 생성·두 번째 클라이언트 | 순차 17번째만 있음 | 동시 생성은 통과. 두 번째 클라이언트로 17번째가 생김(실험) |
| 7 공백 변형 이름 | 빈 이름·완전 중복만 | 깨짐 |
| 8 열거 중 소멸 | 없음 | 깨짐 |
| 9 송신 경계 | 0 타임스탬프·길이 불일치만 | 과거·먼 미래·2.0·예약 타입은 실험으로 확인 |
| 10 테스트 간섭 | 이름에 pid를 넣어 분리 | 반복 실행에서 간헐 실패 재현 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클라이언트·포트를 모두 drop하고 8–12초 기다린 뒤 `MidiClient::new` | 깨짐. `Os { MIDIClientCreate, -2 }`가 30초 넘게 이어지고 그 프로세스에서는 회복하지 않음. 다른 MIDI 클라이언트가 없는 시스템에서 MIDI 서버가 약 3초 뒤 종료하기 때문 | `client_new_after_last_client_dropped_and_server_idle_succeeds`(실패) |
| 2 | 앞뒤 공백만 다른 이름(`"X "`, `" X"`)으로 생성 | 깨짐. 둘 다 `Ok` | `create_virtual_output_name_differing_only_in_surrounding_whitespace_fails`(실패) |
| 3 | 사본에서 `unsafe impl Send/Sync for ClientInner`를 지우고 `virtual_output_and_client_are_send_and_sync` 실행 | 깨짐(불필요한 unsafe). 지워도 통과 | `client_source_send_sync_auto_traits_suffice_has_no_unsafe_impl`(실패, 소스 검사) |
| 4 | 살아 있는 소스 L보다 앞에 만든 소스를 열거 중에 지움 | 깨짐. 약 8.5만 회 열거 중 420–456회 L이 목록에서 빠짐. 앞쪽 소스를 지우지 않는 대조군은 0회 | `source_names_while_earlier_source_disposed_lists_live_source`(실패) |
| 5 | 32개 스레드가 한 클라이언트에 동시 생성 | 못 깨뜨림. 정확히 16 성공, 16 `TooManyPorts` | `create_virtual_output_concurrent_threads_never_exceeds_max` |
| 6 | 두 스레드가 같은 이름을 만들고 지우기 반복(「리뷰어에게」 2) | 못 깨뜨림. 같은 이름 소스가 둘 보인 적 없음. drop은 `MutexGuard`를 함수 끝까지 쥐므로 `MIDIEndpointDispose`가 잠금 안에서 끝난다. 구현자의 우려는 코드상 성립하지 않는다 | `create_and_drop_same_name_concurrently_never_shows_two_sources` |
| 7 | `MidiClient` 복제본을 모두 drop한 뒤 포트로 송신 | 못 깨뜨림. 소스 유지, `send` `Ok` | `send_after_all_client_handles_dropped_succeeds` |
| 8 | rename 후 drop하고 옛·새 이름으로 재생성 | 못 깨뜨림 | `drop_after_rename_frees_both_names` |
| 9 | 4워드(타입 0xD) 송신(「리뷰어에게」 1) | 못 깨뜨림. `Add`가 NULL을 돌려주지 않음. 바인딩상 `MIDIEventList`는 `repr(C)`(packed 아님), 안의 `MIDIEventPacket`은 `packed(4)`이고 `words: [u32; 64]`, 크기 276·정렬 4. 넘기는 크기 276은 버퍼 크기와 정확히 같다 | `send_four_word_message_fits_stack_event_list` |
| 10 | 1.0 소스에 2.0 Note On(타입 4, 벨로시티 0x8000) | 못 깨뜨림. CoreMIDI가 `90 3c 40`으로 바꿔 전달 | 실험만(아래 이유로 파일에서 뺌) |
| 11 | 과거 타임스탬프(1틱), `u64::MAX` | 1틱은 즉시 전달·타임스탬프 1 유지. `u64::MAX`는 `Ok`인데 1초 안에 오지 않음(서버 큐에 남는 것으로 보임) | 실험만 |
| 12 | 예약 타입 0x6, 유틸리티 NOOP(타입 0), 타입 0xD를 1.0 소스로 | 셋 다 `Ok`, 수신 없음. 에러 없이 버려진다 | 실험만 |
| 13 | 두 번째 `MidiClient`로 17번째 포트 생성, 두 클라이언트에 같은 이름 | 둘 다 `Ok`. 상한·중복 검사가 클라이언트 단위 | 실험만 |
| 14 | `cargo test -p keyscore-midi --lib --test virtual_port`를 연속 27회(12 + 15) | 깨짐(간헐). 3회 실패. 1회는 모든 테스트가 `MIDIClientCreate -304`(직전 실행이 끝나 MIDI 서버가 내려가는 중에 새 프로세스가 붙음), 2회는 `virtual_port.rs:100`의 `coremidi::Source::from_name(..).unwrap()`이 None(시도 4와 같은 인덱스 열거 경합) | 시도 4의 테스트가 원인을 고정 |
| 15 | 자식 프로세스 고아 확인 | 27회 반복과 리뷰 테스트 뒤 `pgrep`로 남은 자식 없음. 단 부모 테스트 러너가 외부에서 죽으면 자식은 60초 동안 남는다(코드 판독) | 리뷰 테스트는 `ChildGuard`(drop에서 kill·wait)로 패닉 경로까지 막음 |

시도 10–12의 수신 테스트를 파일에서 뺀 이유: dev 의존 `coremidi` 0.9.2의 `Client`는 drop해도 해제되지 않는다(`client.rs:340`에서 `MIDIClientDispose`를 주석 처리). 같은 바이너리에 두면 MIDI 서버를 붙잡아 시도 1의 재현을 무력화한다. 이 사실 자체가 시도 1의 근거를 뒷받침한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 핸들 이중 해제·해제 뒤 사용 없음 | 아니오 | 엔드포인트는 `VirtualOutput`만 소유, 클라이언트는 `Arc`. `send`와 `drop`은 빌림 규칙상 겹치지 않는다 |
| 포트보다 클라이언트가 먼저 해제되지 않음 | 아니오 | 시도 7 |
| 클라이언트를 다시 만들 수 있음(RAII 해제가 이후 사용을 막지 않음) | 예 | 시도 1. `ClientInner::drop`의 `MIDIClientDispose` |
| 한 클라이언트에 16개 이하 | 아니오 | 시도 5 |
| 앱 전체 16개 이하(OUT-2) | 예 | 시도 13. `MidiClient::new`를 두 번 |
| 신디 목록에서 구분되지 않는 이름 금지 | 예 | 시도 2, 13 |
| `source_names`가 살아 있는 소스를 모두 돌려줌 | 예 | 시도 4 |
| 송신 경로 할당·락·로그 없음 | 아니오 | 코드 판독(6절) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-1 | 부분 충족 | 1.0 프로토콜 가상 소스 생성은 됨(`client.rs:116-145`, `PROTOCOL_1_0`). 그러나 클라이언트를 모두 drop한 뒤에는 그 프로세스에서 다시 등록할 수 없다(`client.rs:55-62`, 시도 1). 기본 이름 "KeyScore Out"은 어디에도 없다(앱 시작 시 등록은 이 Task 범위 문구에 있으나 미구현) |
| OUT-2 | 부분 충족 | 상한·중복이 클라이언트 단위라 두 번째 클라이언트로 우회(`client.rs:120-125`, 시도 13). 공백 변형 이름 허용(`client.rs:31-37`, 시도 2). 이름 변경은 됨(`client.rs:162-187`) |
| 8.4 비정상 종료 시 포트 미잔존 | 충족 | `virtual_port.rs:228-252`, 27회 중 서버 경합 1회를 빼고 통과 |
| 9.2 스케줄러 스레드 할당·락 금지 | 충족 | `client.rs:209-244`. 스택 버퍼, 락·로그 없음, 시스템 콜은 허용된 MIDI 송신 API(`MIDIReceivedEventList`)뿐 |

「부분 충족」은 미충족으로 센다. 미충족 2건.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: `send`에는 없음. 단 `VirtualOutput::drop`은 `Mutex`를 잠그고 `String`을 해제하며 IPC를 한다(`client.rs:190-201`). 스케줄러가 포트의 마지막 소유자가 되면 실시간 스레드에서 drop이 일어난다. S1-T4 설계에서 포트 drop을 비실시간 스레드로 넘겨야 한다(제안 7).
- 제품 코드의 `unwrap`/`expect`: 없음. `names()`는 `unwrap_or_else(PoisonError::into_inner)`로 독을 무시하며, 잠금 안에서 사용자 코드가 돌지 않아 목록이 어긋날 경로를 찾지 못했다.
- 에러가 삼켜지는 곳:
  - `ClientInner::drop`, `VirtualOutput::drop`의 dispose 결과(drop이라 불가피).
  - `source_names`가 이름 조회 실패 소스를 조용히 건너뜀(`client.rs:264`).
  - `send`가 1.0 소스에서 전달되지 않는 타입(0x0 NOOP, 0x6·0x7 등 예약, 0xD)을 `Ok`로 받는다(시도 12). `u64::MAX` 같은 터무니없는 미래 시각도 `Ok`(시도 11).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | `ClientInner::drop`이 `MIDIClientDispose`를 부른다. `MIDIServices.h` 1470–1476행은 앱의 마지막 클라이언트를 해제하면 MIDI 서버가 종료할 수 있고 그 뒤 그 앱의 `MIDIClientCreate`가 모두 실패한다며 「strongly discouraged」라고 적는다. 실측으로 재현했다(-2가 30초 이상, 회복 없음). 공개 API만으로 OUT-1 등록이 영구 불가능해지는 경로다. 클라이언트를 프로세스 전역 하나로 두고(예: `OnceLock`) 해제하지 않는 쪽을 권한다. 그러면 지적 4의 앱 전체 상한·중복 문제도 함께 풀린다. 엔드포인트 RAII는 그대로 둔다 | `crates/keyscore-midi/src/client.rs:55-62` |
| 2 | 수정 | `source_names`가 개수→인덱스 순회라, 열거 중 앞쪽 소스가 사라지면 살아 있는 소스를 건너뛴다(시도 4). SAFETY 주석의 「사라져도 0이 온다」는 사라진 소스 자신만 다루고 다른 소스가 건너뛰어지는 것은 다루지 않는다. S1-T2의 소멸 판정 출발점이므로 지금 고친다. 예: 열거 앞뒤 개수와 결과를 비교해 바뀌면 다시 열거하고, 문서에 「스냅샷이 아니다」를 적는다 | `crates/keyscore-midi/src/client.rs:247-272` |
| 3 | 수정 | 기존 통합 테스트가 연속 실행에서 간헐 실패한다(27회 중 3회). `Source::from_name(..).unwrap()`은 지적 2와 같은 경합으로 None이 된다(`virtual_port.rs:75`, `:100`). 찾기를 `wait_until`로 재시도해야 한다. 보고서의 「10회 반복 모두 통과」는 재현되지 않았다. 또 직전 프로세스가 끝나 MIDI 서버가 내려가는 순간 새 프로세스가 붙으면 `MIDIClientCreate -304`로 전 테스트가 실패한다. 이 환경 요인을 보고서의 알려진 한계에 적고, 가능하면 테스트 바이너리 시작 시 서버 준비를 확인하는 방법을 찾는다 | `crates/keyscore-midi/tests/virtual_port.rs:75`, `:100` |
| 4 | 수정 | 이름 중복 검사가 앞뒤 공백 변형을 막지 않는다. plan.md가 든 근거(「신디 입력 목록에서 구분되지 않는다」)에 스스로 어긋난다. 이름을 trim해 저장·비교하거나 앞뒤 공백을 `InvalidName`으로 거부한다 | `crates/keyscore-midi/src/client.rs:31-37`, `:120`, `:168` |
| 5 | 수정 | `unsafe impl Send/Sync for ClientInner`가 불필요하다. 필드(`u32`, `Mutex<Vec<String>>`)로 자동 구현된다(사본에서 지우고 확인). 리뷰 초점 「unsafe 범위 최소」에 어긋나고, 나중에 `!Send` 필드가 들어와도 컴파일러가 잡지 못한다. 지우고 `virtual_output_and_client_are_send_and_sync`가 그대로 지키게 한다 | `crates/keyscore-midi/src/client.rs:64-69` |
| 6 | 제안 | `send`가 1.0 소스로 전달되지 않는 UMP 타입(0x0, 0x6, 0x7, 0x8–0xC 예약, 0xD)을 `Ok`로 받고 CoreMIDI가 조용히 버린다. 1.0 포트가 받는 타입(1, 2, 3, 4)만 허용하고 나머지는 `InvalidMessage`로 돌려주면 S1-T3 인코더 버그가 드러난다 | `crates/keyscore-midi/src/client.rs:216`, `crates/keyscore-midi/src/ump.rs:6-14` |
| 7 | 제안 | `VirtualOutput::drop`은 락·해제·IPC를 한다. S1-T4에서 스케줄러가 포트의 마지막 `Arc`를 쥐지 않게 하는 규칙을 문서(트레이트 주석)에 남긴다 | `crates/keyscore-midi/src/client.rs:190-201`, `crates/keyscore-midi/src/port.rs:5` |
| 8 | 제안 | OUT-1의 기본 이름 "KeyScore Out"을 상수로 두지 않았다. S1-T7 게이트와 S8의 이름 일원화가 이 이름을 쓰므로 `keyscore-midi`에 상수 하나를 둔다 | `crates/keyscore-midi/src/lib.rs` |
| 9 | 제안 | 자식 프로세스는 60초 sleep에 기댄다. 부모 러너가 외부에서 죽으면 최대 60초 고아가 남는다. 자식이 stdin EOF(부모 종료)를 감지해 끝내거나, 부모 쪽에 drop 시 kill하는 가드를 둔다 | `crates/keyscore-midi/tests/virtual_port.rs:219-251` |

「리뷰어에게」 답:

1. `MaybeUninit<MIDIEventList>`: 안전하다. `MIDIEventList`는 `repr(C)`(packed 아님) 크기 276·정렬 4, 패킷은 `packed(4)`에 `words: [u32; 64]`다. 넘기는 크기가 버퍼 크기와 같고, `MIDIReceivedEventList`는 Init·Add가 채운 영역만 읽는다. 4워드 송신 확인(시도 9).
2. drop 순서: 우려는 성립하지 않는다. `names` 가드가 함수 끝까지 살아 dispose가 잠금 안에서 끝난다(시도 6). 다만 지적 1의 클라이언트 dispose가 진짜 문제다.
3. `unsafe impl Send/Sync`: 근거 이전에 필요가 없다(지적 5).

나머지 SAFETY 주장 점검: `MIDIObjectGetStringProperty`의 create rule은 헤더 67행 규칙과 맞다. `unsafe extern` + `safe fn mach_absolute_time()`은 인자 없고 항상 정의된 값을 돌려주므로 타당하다. `MIDIClientCreate`·`MIDISourceCreateWithProtocol`·`MIDIObjectSetStringProperty`의 포인터 수명 주장은 맞다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 1건(지적 1), 수정 4건(지적 2–5). 재리뷰 때 `review_s1_t1.rs`의 실패 4건이 통과해야 하고, 통과 5건은 계속 통과해야 한다. `client_new_after_last_client_dropped_and_server_idle_succeeds`는 시스템에 다른 MIDI 클라이언트가 떠 있으면 재현되지 않고 통과하므로, 고친 뒤 판정은 코드 판독(클라이언트를 해제하지 않음)과 함께 본다. 소스 문자열 검사 테스트(`client_source_..._has_no_unsafe_impl`)는 지적 5를 고친 뒤 구현자가 컴파일 검사로 바꿔도 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`child_holds_virtual_output_until_killed`의 `#[ignore]`는 자식 역할용 새 테스트)
- [x] 새 의존성이 있으면 라이선스를 확인했다(`coremidi-sys` MIT, `core-foundation` MIT/Apache-2.0, dev `coremidi` MIT. `cargo deny check` licenses ok)
- [x] 스타일 지적을 적지 않았다
