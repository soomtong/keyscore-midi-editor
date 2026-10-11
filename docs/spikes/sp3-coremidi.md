# SP-3 coremidi 크레이트 범위

## 질문

`coremidi` 크레이트(crates.io 최신 0.9.2, MIT, rust-version 1.75, 의존 `coremidi-sys` 3.2.1 MIT)로 다음이 되는가.

- (a) 가상 소스 생성
- (b) 소스·목적지 열거
- (c) 핫플러그 알림 콜백
- (d) 미래 타임스탬프 지정 송신

MIDI 1.0 프로토콜 소스로 만들었을 때 수신 앱이 바로 보는가. 핵심 사실은 가상 소스(MIDIReceived 경로)에서 CoreMIDI가 미래 타임스탬프 예약을 지키는지, 아니면 즉시 전달하는지다.

## 방법

코드는 `spikes/sp3-coremidi/`(독립 cargo 프로젝트, 빈 `[workspace]`).

- `sender`: 가상 소스 "SP3 Virtual Source"를 만들고, 100 ms마다 한 라운드씩 300라운드, 라운드당 5종류(채널 0~4)의 Note On을 보낸다. 예약 오프셋은 채널별로 +0, +5, +20, +100 ms(`mach_absolute_time` 기준 미래 타임스탬프)와 타임스탬프 0(즉시)이다. 총 1500개. 송신 직전의 호스트 시각과 지정한 타임스탬프를 CSV로 남긴다.
  - `legacy` 모드: `coremidi` 크레이트(`Client::virtual_source` = `MIDISourceCreate`, `VirtualSource::received` = `MIDIReceived`, `PacketBuffer`).
  - `proto` 모드: `coremidi-sys` 직접 호출(`MIDISourceCreateWithProtocol(kMIDIProtocol_1_0)`, `MIDIEventListAdd`, `MIDIReceivedEventList`, UMP type 2 MIDI 1.0 채널 보이스).
- `receiver`: 별도 프로세스. `Client::new_with_notifications`로 알림을 받고, 소스가 나타나면(`Source::from_name`) 입력 포트(`input_port`, 레거시 `MIDIPacketList` 콜백)로 연결한다. 콜백 진입 직후 `mach_absolute_time`을 찍어 패킷 타임스탬프와 함께 CSV에 쓴다.
- 분석은 (채널, 노트, 순번)으로 송신 기록과 수신 기록을 맞춰 「도착 - 송신」과 「도착 - 지정 타임스탬프」를 구한다. mach 틱은 `mach_timebase_info`(이 기기 125/3)로 ns 변환했다.
- 가상 소스를 수신 프로세스가 열거하는지는 `MIDIGetNumberOfSources`(`Sources::count`)와 이름 조회로 확인했다.

측정 환경.

| 항목 | 값 |
| --- | --- |
| 기기 | Apple M1 Max |
| macOS | 27.0.1 (SDK 27.0) |
| rustc / 프로필 | 1.95.0 / `cargo build --release` (opt-level 3) |
| 부하 | 다른 부하를 주지 않은 유휴 상태, 송신·수신은 별도 프로세스 |

## 결과

기능 확인.

| 항목 | 결과 |
| --- | --- |
| (a) 가상 소스 생성 | 됨. `Client::virtual_source` |
| (b) 열거 | 됨. `Sources::count`, `Source::from_index/from_name/from_unique_id`, `Destinations`. 수신 프로세스에서 `sources: 1`, 이름·uid·protocol 조회 확인 |
| (c) 핫플러그 알림 | 됨. 수신 프로세스에서 소스 생성 때 `ObjectAdded(child=Source)` + `SetupChanged`, 송신 종료 때 `ObjectRemoved` + `SetupChanged`를 받았다. 단 알림은 클라이언트를 만든 스레드의 CFRunLoop이 돌아야 배달된다 |
| (d) 미래 타임스탬프 송신 | 됨. `PacketBuffer::new(timestamp, ..)` + `VirtualSource::received`. 타임스탬프 단위는 mach 틱 |
| MIDI 1.0 소스를 수신 앱이 보는가 | 봄. 두 모드 모두 레거시 입력 포트가 3바이트 Note On을 그대로 받았고, 소스의 `protocol` 속성은 1(MIDI 1.0)이었다 |

타이밍(1500개 전부 수신, 손실 0). 단위 ms. 「도착 - 송신」은 예약 오프셋과 비교할 값, 「도착 - 타임스탬프」는 예약 시각 대비 도착 지연이다.

legacy 모드(`coremidi` 크레이트)

| 예약 | n | 도착-송신 p50 | p95 | max | 도착-타임스탬프 p50 | p95 | max |
| --- | --- | --- | --- | --- | --- | --- | --- |
| +0 | 300 | 0.088 | 0.138 | 0.446 | 0.088 | 0.138 | 0.446 |
| +5 | 300 | 5.054 | 5.106 | 5.161 | 0.054 | 0.106 | 0.161 |
| +20 | 300 | 20.100 | 20.145 | 20.184 | 0.100 | 0.145 | 0.184 |
| +100 | 300 | 100.122 | 100.175 | 100.203 | 0.122 | 0.175 | 0.203 |
| ts=0 | 300 | 0.043 | 0.075 | 0.138 | 해당 없음 | | |

proto 모드(`coremidi-sys` 직접 호출, MIDI 1.0 프로토콜 소스)

| 예약 | n | 도착-송신 p50 | p95 | max | 도착-타임스탬프 p50 | p95 | max |
| --- | --- | --- | --- | --- | --- | --- | --- |
| +0 | 300 | 0.056 | 0.072 | 0.435 | 0.056 | 0.072 | 0.435 |
| +5 | 300 | 5.050 | 5.077 | 5.100 | 0.050 | 0.077 | 0.100 |
| +20 | 300 | 20.059 | 20.082 | 20.102 | 0.059 | 0.083 | 0.102 |
| +100 | 300 | 100.060 | 100.084 | 100.104 | 0.060 | 0.084 | 0.104 |
| ts=0 | 300 | 0.036 | 0.047 | 0.068 | 해당 없음 | | |

지터(같은 예약 클래스 안 「도착-송신」의 표준편차)는 legacy 0.016~0.041 ms, proto 0.006~0.289 ms(+5 ms 클래스만 크고 나머지 클래스는 0.010 이하).

핵심 사실.

- 가상 소스 경로에서도 CoreMIDI는 미래 타임스탬프를 지킨다. +5/+20/+100 ms로 예약한 패킷은 정확히 그만큼 늦게(예약 시각 + 0.05~0.2 ms) 수신 클라이언트 콜백에 도착했고, 즉시 전달된 경우는 없었다. 수신 측 패킷 타임스탬프는 송신 측이 지정한 값과 1500/1500 일치했다.
- 즉 NF-2(가상 포트 → 수신 앱 ≤ 1 ms p99)는 이 기기에서 max 0.446 ms로 만족한다. 단 부하가 없는 유휴 상태 측정이고, 부하 상태·장시간·USB 장치 경로는 측정하지 않았다.
- 헤더(`MIDIServices.h`)는 `MIDIReceived`/`MIDIReceivedEventList`에서 「타임스탬프 0은 now와 같지 않으며, 가상 소스가 적절한 타임스탬프를 채워야 한다」고 적는다. 이번 측정에서 0은 즉시 전달됐지만 문서상 보장이 아니므로 스케줄러는 항상 0이 아닌 mach 시각을 채운다.
- 확인하지 못한 것. 예약 가능한 최대 선행 시간(수 초 이상 미래), 수신 클라이언트가 여럿일 때의 동작, 수신 앱이 입력 포트 콜백이 아니라 자체 큐로 타임스탬프를 해석하는 경우, 시스템 부하·슬립 복귀 중 동작. 수신 쪽 구현에 따라 타임스탬프를 무시하고 즉시 처리하는 앱도 있을 수 있다(이번 수신기는 CoreMIDI 입력 포트 기본 동작).

coremidi 0.9.2에 없는 API (직접 `coremidi-sys` 호출이 필요한 부분).

| 필요 | coremidi 0.9.2 | 대응 |
| --- | --- | --- |
| 프로토콜 지정 가상 소스 `MIDISourceCreateWithProtocol` | 없음. `virtual_source`는 `MIDISourceCreate`(레거시)뿐. 가상 목적지는 `virtual_destination_with_protocol` 있음 | sys 직접 호출. 이번에 동작 확인(proto 모드) |
| 원시 `MIDIClientRef` 접근 | 없음(`Client`의 핸들이 비공개) | sys로 클라이언트도 직접 생성해야 `MIDISourceCreateWithProtocol`에 넘길 수 있다 |
| `MIDIObjectGetDataProperty` / `SetDataProperty`, `MIDIObjectGetProperties`(딕셔너리) | 없음(문자열·정수·불리언 속성만) | sys 직접 호출 |
| 알 수 없는 알림 ID | `Notification::try_from`이 `Err(id)`로 실패. `kMIDIMsgInternalStart` 이상이나 향후 추가 ID는 열거형으로 못 받음 | 콜백에서 오류를 무시하도록 처리하거나 sys 콜백 사용 |
| 알림 콜백 전달 | CFRunLoop이 필요(크레이트가 run loop을 돌려주지 않음) | 포트 열거·핫플러그 전용 스레드에서 `CFRunLoop::run_in_mode`를 직접 돌린다(9.2절 백그라운드 스레드) |
| MIDI Thru 연결, MIDI Setup(`MIDIDeviceCreate` 등), UMP 엔드포인트(`MIDIUMPEndpoint*`) | 없음 | v1 범위 밖. 필요하면 sys |
| 할당 없는 송신 | `PacketBuffer`는 `Vec` 기반(할당), `send`·`received`는 패킷 리스트 포인터를 받는 얇은 래퍼 | 실시간 스레드용 버퍼는 미리 할당한 `PacketBuffer`를 `clear` 후 재사용하거나, `MIDIPacketListInit/Add`를 sys로 직접 호출 |

`coremidi-sys` 3.2.1은 미리 생성된 바인딩이라 빌드에 bindgen이 필요 없다. `MIDISourceCreateWithProtocol`, `MIDIEventListAdd`, `MIDIReceivedEventList`, `MIDISendEventList`, `MIDIObjectGetDataProperty`, `MIDIObjectGetProperties`, `MIDIClientCreateWithBlock`가 모두 있다.

Surge XT 등 실제 외부 신디는 이 기기에 없어 확인하지 못했다.

사람이 수동으로 확인할 항목.

- Surge XT(또는 다른 스탠드얼론 신디)가 입력 장치 목록에서 「SP3 Virtual Source」(레거시 소스와 프로토콜 1.0 소스 둘 다)를 보고 소리를 내는가.
- 신디에서 약 100 ms 앞서 예약한 노트가 의도한 시각에 소리 나는가(수신 앱이 타임스탬프를 존중하는지). 재현: `cargo run --release --bin sender -- legacy 300 /tmp/tx.csv` 후 신디에서 입력 선택.

## 계획에 미치는 영향

- S1-T1 결정 분기는 B(`core-midi-sys` 직접 호출)를 권고한다. 근거는 `coremidi` 크레이트가 (a)~(d)를 모두 해내므로 분기 A로도 OUT-1·OUT-3·OUT-7이 구현되지만, REQ M2-2(MIDI 2.0 프로토콜 가상 소스)와 이후 UMP 확장에 필요한 `MIDISourceCreateWithProtocol`과 원시 클라이언트 핸들이 크레이트에 없고, 실시간 스레드의 할당 금지 규칙(9.2절) 때문에 송신 경로는 어차피 `MIDIPacketList`를 직접 다뤄야 하기 때문이다. 결론: 열거·핫플러그·속성 조회처럼 비실시간 부분은 `coremidi` 크레이트를 써도 되나(두 방식 혼용 가능), 가상 소스 생성과 송신 경로는 `coremidi-sys`를 직접 쓰는 분기 B 중심으로 `midi` crate를 설계한다. 라이선스는 둘 다 MIT로 `cargo deny` 허용 목록에 맞는다.
- OUT-7(`MIDIPacketList` 타임스탬프 예약)의 전제는 이 기기에서 성립했다. CoreMIDI가 예약 시각을 지키므로 스케줄러는 수 ms 앞서 밀어 넣으면 되고, 스케줄러가 직접 시각을 맞추는 대체 설계는 불필요하다. 다만 수신 앱이 타임스탬프를 무시하는 경우를 대비해 「예약 선행 시간」을 설정 가능한 값으로 두고(기본 수 ms), 위 수동 확인 결과를 S1 출구 게이트 전에 확인한다. 항상 0이 아닌 타임스탬프를 채운다.
- 알림은 CFRunLoop이 필요하므로 S1-T2의 핫플러그 감시 스레드가 run loop을 소유해야 한다.
