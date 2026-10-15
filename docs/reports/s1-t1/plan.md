# Task 계획: S1-T1 CoreMIDI 가상 출력 포트

- 브랜치: `s1/t1-virtual-port`
- 관련 REQ: 6.3 OUT-1, OUT-2, 8.4(종료 시 포트 해제), 9.2
- 선행 Task: S0
- 관련 스파이크: `docs/spikes/sp3-coremidi.md`(결정 분기 B: `coremidi-sys` 직접 호출)

## 끝의 정의 (내 말로)

- `keyscore-midi`가 CoreMIDI 클라이언트와 가상 출력 포트(가상 소스)를 RAII 타입으로 제공한다.
  - 가상 소스는 MIDI 1.0 프로토콜(`MIDISourceCreateWithProtocol(kMIDIProtocol_1_0)`)로 만든다.
  - 프로세스 전체에서 최대 16개를 만들 수 있고 이름은 프로세스 안에서 겹치지 않는다(결정 `2026-10-15-virtual-port-limit-scope.md`). 이름을 바꿀 수 있고, 값이 drop되면 해제된다.
  - CoreMIDI 클라이언트는 프로세스에 하나만 만들고 해제하지 않는다. 마지막 클라이언트를 해제하면 MIDI 서버가 내려가 이후 생성이 모두 실패하기 때문이다(리뷰 1 지적 1). `MidiClient`는 그 클라이언트와 프로세스 포트 목록을 가리키는 핸들이다.
- 가상·OS 포트 공통 트레이트 `MidiPort`를 둔다. 송신은 미래 호스트 시각(mach 틱)을 받는 UMP 메시지 송신이고, 힙 할당이 없다. OS 포트 구현은 S1-T2에서 한다.
- 같은 프로세스의 수신 클라이언트가 가상 소스를 열거로 보고, 보낸 메시지를 받는다(통합 테스트).
- 포트를 가진 프로세스가 `kill -9`로 죽으면 소스가 사라진다. 단계 파일은 수동 확인 항목으로 두었지만, 자식 프로세스를 띄우는 통합 테스트로 자동화한다. S1 출구의 수동 확인은 그대로 둔다.

## 공개 인터페이스 초안

~~~rust
pub struct HostTime(u64);                    // mach_absolute_time 틱
impl HostTime { pub fn now() -> Self; pub const fn from_ticks(u64) -> Self; pub const fn ticks(self) -> u64 }

pub struct MidiClient { /* 프로세스 클라이언트 핸들 */ }
impl MidiClient {
    pub fn new(name: &str) -> Result<Self, MidiError>;
    pub fn create_virtual_output(&self, name: &str) -> Result<VirtualOutput, MidiError>;
}

pub struct VirtualOutput { /* MIDIEndpointRef, 이름 */ }   // Drop: MIDIEndpointDispose
pub const DEFAULT_PORT_NAME: &str = "KeyScore Out";
impl VirtualOutput { pub fn rename(&mut self, name: &str) -> Result<(), MidiError> }

pub trait MidiPort: Send + Sync {
    fn name(&self) -> &str;
    /// UMP 메시지 하나(1–4 워드)를 `at`에 도착하도록 예약 송신한다. 할당 없음.
    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError>;
}

pub const MAX_VIRTUAL_OUTPUTS: usize = 16;
pub fn source_names() -> Vec<String>;        // 시스템 소스 이름 열거(테스트·S1-T2 출발점)

#[derive(thiserror::Error)] pub enum MidiError {
    Os { call: &'static str, status: i32 }, InvalidName, DuplicateName(String),
    TooManyPorts, InvalidMessage, ZeroTimestamp,
}
~~~

## 세부 결정 (위임, 베스트 프랙티스)

- 이름: 앞뒤 공백을 뗀 이름을 쓴다. 비었거나 제어 문자·보이지 않는 서식 문자가 있으면 `InvalidName`이다. 프로세스 안에서 이미 쓰는 이름은 `DuplicateName`이다. 신디의 입력 목록에서 구분되지 않기 때문이다. 다른 앱과의 이름 충돌은 막지 않는다.
- 타임스탬프 0은 거부한다(`ZeroTimestamp`). SP-3 결론과 헤더 문서대로 항상 0이 아닌 mach 시각을 채운다.
- UMP 길이는 메시지 타입(상위 4비트)이 정한 워드 수와 같아야 한다. 다르면 `InvalidMessage`다.
- 송신 버퍼는 스택의 고정 크기 배열이다(실시간 스레드 규칙, 9.2).
- 클라이언트는 프로세스에 하나이고 해제하지 않는다. 생성은 MIDI 서버 재시작 중의 일시 실패를 3초까지 다시 시도한다.
- 포트 수와 이름 관리에는 `Mutex`를 쓴다. 생성·이름 변경 경로에서만 쓰고, 송신 경로에는 락이 없다.
- 영속 고유 ID(`kMIDIPropertyUniqueID`)는 이 Task 범위 밖이다. 신디가 재시작 뒤 포트를 기억하는 문제는 S2(프로젝트 저장)에서 본다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `virtual_output_create_is_visible_to_source_enumeration` | 통합 | 만든 소스가 `source_names()`에 보이고, drop 후 사라짐 |
| 2 | `virtual_output_send_is_received_by_in_process_client` | 통합 | `coremidi`(dev 의존, 독립 구현) 입력 포트가 Note On 3바이트를 받음, 타임스탬프 일치 |
| 3 | `virtual_output_future_timestamp_arrives_no_earlier_than_scheduled` | 통합 | +20 ms 예약이 그 전에 도착하지 않음 |
| 4 | `virtual_output_rename_updates_enumerated_name` | 통합 | 이름 변경이 열거에 반영 |
| 5 | `create_virtual_output_seventeenth_fails_with_too_many_ports` | 통합 | 16개까지 되고 17번째는 실패, 하나 drop하면 다시 됨 |
| 6 | `create_virtual_output_duplicate_or_blank_name_fails` | 통합 | 중복·빈 이름 거부, rename도 같은 규칙 |
| 7 | `send_rejects_zero_timestamp_and_wrong_word_count` | 통합 | 0 타임스탬프, 타입과 길이가 다른 UMP 거부 |
| 8 | `killed_process_leaves_no_virtual_source` | 통합(자식 프로세스) | 자식이 소스를 만든 뒤 SIGKILL, 2초 안에 소스가 사라짐 |
| 9 | `ump_word_count_matches_message_type` | 단위 | 메시지 타입별 워드 수 표 |
| 10 | `virtual_output_and_client_are_send_and_sync` | 단위(컴파일) | 스레드 간 이동 가능 |

## 불변식

- 모든 CoreMIDI 핸들은 RAII 값이 소유한다. 공개 API로 이중 해제나 해제 뒤 사용을 만들 수 없다.
- unsafe는 FFI 호출을 감싼 함수 안에만 있고, 함수마다 `#[allow(unsafe_code)]`와 이유 주석을 단다.
- 송신 경로(`MidiPort::send`)에는 할당·락·로그가 없다. 단언은 S1-T4의 `assert_no_alloc` 게이트에서 한다. 여기서는 코드로 보장한다.

## 범위 밖

- OS 포트 열거·핫플러그·재연결(S1-T2), MIDI 1.0 인코딩(S1-T3), 스케줄러(S1-T4), 영속 고유 ID
- MIDI 2.0 프로토콜 소스(M2-2)는 v1 범위 밖이다. 생성 함수는 1.0으로 고정한다.

## 리스크·결정 요청

- 헤드리스 CI 러너에서 CoreMIDI 서버가 동작하는지 모른다. remote가 없으므로 지금은 로컬에서만 확인한다.
- 의존 추가: `coremidi-sys` 3.2.1(MIT), `core-foundation` 0.10(MIT/Apache-2.0), `thiserror`, dev: `coremidi` 0.9.2(MIT). 모두 허용 목록 라이선스다.
- 결정 요청 없음.

## 개정

- 2026-10-15: 리뷰 1 지적 1(클라이언트 해제)과 리뷰 2 지적 1(OUT-2 범위 결정)에 따라 `MidiClient`의 의미, 포트 상한·이름 규칙을 고쳤다(리뷰 2 지적 3).
