# Task 계획: S1-T2 OS 포트 열거·핫플러그·재연결

- 브랜치: `s1/t2-port-enumeration`
- 관련 REQ: 6.4 OUT-3, OUT-8, 9.2(포트 열거·핫플러그 감시는 백그라운드 스레드)
- 선행 Task: S1-T1
- 관련 스파이크: `docs/spikes/sp3-coremidi.md`(알림은 클라이언트를 만든 스레드의 CFRunLoop이 돌아야 배달된다)

## 끝의 정의 (내 말로)

- OS 출력 포트(CoreMIDI 목적지: USB-MIDI, IAC, 다른 앱의 가상 목적지)를 열거한다. 포트마다 이름과 고유 ID를 준다.
- OS 출력 포트에 송신하는 `OsOutput`이 `MidiPort`를 구현한다. 스케줄러는 가상 포트와 같은 방식으로 쓴다.
- 감시 스레드(`PortWatcher`)가 run loop을 소유하고 CoreMIDI 알림을 받는다. 알림이 올 때마다 목적지 목록을 다시 읽어 직전 목록과 비교하고, `Disappeared`/`Appeared` 이벤트를 정해진 순서로 채널에 보낸다.
- 이름 기반 자동 재연결은 CoreMIDI와 분리된 순수 상태 기계(`Reconnector`)로 둔다. 이벤트를 넣으면 「끊김」·「다시 연결」 동작이 나온다.
- OUT-8 대체 매핑 제안은 순수 함수다. 저장된 포트 이름과 지금 있는 포트를 받아 이름마다 제안을 낸다(데이터 구조만, UI는 S2).
- S1-T1 이월
  - 클라이언트 생성 재시도가 잠금을 쥔 채 잠들지 않게 한다.
  - 서버 재시작류 오류(`-304` 등)가 나면 캐시된 클라이언트를 버려 다음 호출이 다시 만들게 한다.
  - 반복 실행 스크립트로 드문 일괄 실패를 재현하고, 실패하면 출력이 남게 한다.

## 공개 인터페이스 초안

~~~rust
pub struct PortInfo { /* name: String, unique_id: i32 */ }   // getter
pub fn destinations() -> Vec<PortInfo>;                       // 열거 안정성은 source_names와 같은 재시도

pub struct OsOutput { /* 대상 고유 ID, 이름, 출력 포트 */ }
impl OsOutput { pub fn open(client: &MidiClient, port: &PortInfo) -> Result<Self, MidiError>; }
impl OsOutput { pub fn is_present(&self) -> bool; }          // 제어 스레드용 존재 확인
impl MidiPort for OsOutput { ... }

pub enum PortEvent { Disappeared(PortInfo), Appeared(PortInfo) }
pub struct PortWatcher { /* 스레드, run loop 핸들 */ }       // Drop: run loop 멈춤 + join
impl PortWatcher { pub fn start() -> Result<(Self, mpsc::Receiver<PortEvent>), MidiError>; }

pub struct Reconnector { /* 원하는 이름 → 상태 */ }
pub enum Link { Connected(PortInfo), Missing }
pub enum Action { Lost { name }, Reconnect { name, port: PortInfo } }
impl Reconnector {
    pub fn want(&mut self, name: &str, now: &[PortInfo]) -> &Link;
    pub fn apply(&mut self, event: &PortEvent) -> Vec<Action>;
}

pub enum Suggestion { Present(PortInfo), Virtual { name: String } }
pub fn suggest_mapping(saved: &[String], available: &[PortInfo]) -> Vec<(String, Suggestion)>;
~~~

## 세부 결정 (위임, 베스트 프랙티스)

- 비교 기준: 이벤트는 고유 ID로 목록을 비교해 만든다. 같은 ID인데 이름이 바뀌면 `Disappeared`(옛 이름) 뒤 `Appeared`(새 이름)로 낸다.
- 이벤트 순서: 한 번의 비교에서 `Disappeared`를 모두 먼저, `Appeared`를 그 뒤에 낸다. 각 무리 안에서는 이름순이고, 이름이 같으면 ID순이다.
- 재연결: 이름이 정확히 같은 포트만 다시 연결한다. 같은 이름의 포트가 여럿이면 고유 ID가 가장 작은 것을 고른다(결정적).
- 매핑 제안: 저장된 이름과 정확히 같은 포트가 있으면 `Present`이고, 없으면 같은 이름의 가상 포트를 만들자는 `Virtual`이다. 비슷한 이름 추천은 S2 UI에서 다룬다.
- 앱 자신의 가상 포트는 소스이므로 목적지 목록에 나오지 않는다. 그래서 자기 자신에게 연결하는 일은 생기지 않는다.
- `OsOutput::send`는 가상 포트와 같은 검사(0 타임스탬프, 워드 수)를 하고, 스택 `MIDIEventList`와 `MIDISendEventList`를 쓴다. 할당과 락이 없다.
- 감시 스레드
  - 알림 함수는 C 함수 포인터와 refCon(`Box<Sender>`)으로 받는다. block 의존을 더하지 않으려는 것이다.
  - 알림 클라이언트는 감시 스레드에서 만든다. 마지막 클라이언트 해제 문제(S1-T1 리뷰 1)를 피하려고, 프로세스 클라이언트가 이미 있는 상태에서만 해제한다. 프로세스 클라이언트는 해제되지 않는다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `destinations_lists_virtual_destination_created_by_other_client` | 통합 | `coremidi` crate로 만든 가상 목적지가 열거됨, 이름·ID |
| 2 | `os_output_send_reaches_destination` | 통합 | `OsOutput`으로 보낸 Note On이 그 목적지에 도착 |
| 3 | `os_output_after_destination_removed_is_not_present_and_cannot_reopen` | 통합 | 대상 소멸 뒤 `is_present`가 거짓, 다시 열기는 `PortGone`. CoreMIDI는 사라진 목적지로의 송신을 오류 없이 버린다(구현 중 실측, 처음 계획은 송신 오류였다) |
| 4 | `watcher_reports_disappear_then_appear_in_order` | 통합 | 목적지 생성·제거·재생성 시 이벤트 순서 |
| 5 | `watcher_drop_closes_event_channel_promptly` | 통합 | drop이 1초 안에 끝나고 이벤트 채널이 닫힘 |
| 6 | `reconnector_reconnects_when_same_name_returns` | 단위 | Lost → Reconnect, 다른 이름은 무시 |
| 7 | `reconnector_picks_lowest_id_among_same_names` | 단위 | 결정적 선택 |
| 8 | `diff_orders_disappeared_before_appeared_by_name` | 단위 | 비교 순서 규칙, 이름 바뀜 |
| 9 | `suggest_mapping_present_or_virtual` | 단위 | OUT-8 제안 |
| 10 | `simulated_port_vanish_and_return_reconnects_end_to_end` | 통합 | 게이트: 감시 → `Reconnector` → `OsOutput` 재연결 뒤 송신 도착 |
| 11 | `process_client_retry_does_not_hold_lock_while_sleeping` | 단위 | 재시도 중 다른 스레드가 잠금을 오래 기다리지 않음 |
| 12 | `server_gone_status_clears_cached_client` | 단위 | 서버 오류 분류와 캐시 무효화 |

`scripts/midi-soak.sh`: `cargo test -p keyscore-midi`를 N회 돌리고, 실패한 회차의 전체 출력을 `target/midi-soak/`에 남긴다.

## 불변식

- unsafe는 FFI 호출 함수에만 있다. 알림 콜백은 패닉이 FFI 경계를 넘지 않게 `catch_unwind`로 감싼다.
- 송신 경로(`OsOutput::send`)에 할당·락·로그가 없다.
- 감시 스레드는 drop 때 run loop을 멈추고 join한다. 스레드가 남지 않는다.

## 범위 밖

- 상태줄 표시, 포트 선택 UI, 비슷한 이름 추천: S2
- 트랙 라우팅(OUT-4): S1-T4·S4
- MIDI 서버 자체를 죽여 재시작을 실험하는 일: 시스템 프로세스를 종료하지 않는다(안전 규칙). 서버 재시작 대응은 오류 분류와 캐시 무효화를 단위 테스트로 보고, 실제 재시작은 수동 확인으로 남긴다.

## 리스크·결정 요청

- 결정 요청 없음. REQ OUT-3의 「같은 이름」은 정확히 같은 이름으로 해석한다(S1-T1의 보이는 모양 규칙은 우리 포트 이름에 대한 것이다. 다른 장치 이름은 장치가 정한다).
