# SP-4 가상 포트 리스너 감지

## 질문

가상 소스를 만든 앱이, 다른 앱이 그 소스에 연결했는지(리스너 유무)를 공개 CoreMIDI API로 알 수 있는가. PRE-2의 「포트에 리스너가 붙었는지 상태줄에 표시」를 구현할 수 있는가.

## 방법

코드는 `spikes/sp4-listener-detect/`(독립 cargo 프로젝트, 빈 `[workspace]`, `coremidi-sys` 직접 호출).

- `owner`: `MIDIClientCreate`(raw C 콜백)로 클라이언트를 만들고 가상 소스 "SP4 Virtual Source"를 `MIDISourceCreate`로 만든다. 메인 스레드 run loop을 50 ms 단위로 돌리며 다음을 기록한다.
  - 도착하는 모든 알림의 raw `messageID`(crate 변환을 거치지 않음), 객체·속성 이름
  - 소스의 `MIDIObjectGetProperties` 전체 속성 딕셔너리 변화
  - `kMIDIPropertyConnectionUniqueID` 정수 속성 값 변화
- `peer`: 별도 프로세스. 같은 프로세스 안에 클라이언트 2개와 입력 포트 2개를 만들어 소스에 차례로 `MIDIPortConnectSource` → `MIDIPortDisconnectSource` 하고, 다시 연결한 뒤 클라이언트를 `MIDIClientDispose`(해제 없이)하고, 마지막에 연결한 채 프로세스를 종료한다. 각 단계 사이 1.5초 대기.
- 헤더(`MIDIServices.h`, SDK 27.0)의 알림 종류와 속성 문서를 확인했다.

측정 환경은 SP-3과 같다(Apple M1 Max, macOS 27.0.1, rustc 1.95.0, release).

## 결과

헤더 조사.

- `MIDINotificationMessageID`는 7종뿐이다: SetupChanged, ObjectAdded, ObjectRemoved, PropertyChanged, ThruConnectionsChanged, SerialPortOwnerChanged, IOError. 입력 포트 연결·해제를 뜻하는 알림은 없다. ObjectAdded/Removed는 장치·엔티티·엔드포인트 단위다.
- `kMIDIPropertyConnectionUniqueID`는 「드라이버 객체가 외부 장치에 연결됨」을 나타내는 드라이버용 속성이다(없거나 0이면 연결 없음). 클라이언트 입력 포트의 연결과는 무관하다.
- `MIDIReceived`/`MIDIReceivedEventList`의 문서는 「소스에 연결된 클라이언트 입력 포트들에 배포한다」고만 하고, 연결 수를 돌려주거나 조회하는 API는 없다. 클라이언트 목록을 열거하는 공개 API도 없다.

실험(owner 로그와 peer 로그 대조).

| peer 동작 | owner에서 관찰된 알림·속성 변화 |
| --- | --- |
| 클라이언트 0 연결 | 없음 |
| 클라이언트 1 연결 | 없음 |
| 클라이언트 0 해제 / 클라이언트 1 해제 | 없음 |
| 클라이언트 0 재연결 후 연결 상태에서 클라이언트 `Dispose` | 없음 |
| 클라이언트 1 연결 상태에서 프로세스 종료 | 없음 |

- 소스 속성 딕셔너리는 실험 내내 `name`, `uniqueID` 두 키로 변하지 않았다(`PROPERTIES CHANGED` 출력 0회).
- `kMIDIPropertyConnectionUniqueID` 조회는 내내 `-10835`(`kMIDIUnknownProperty`, 속성 없음)였다.
- owner가 받은 알림은 자기 소스 생성 때의 `ObjectAdded`(id 2)와 `SetupChanged`(id 1) 한 쌍뿐이다(peer의 모든 동작 동안 추가 알림 0건).
- `MIDIReceived`는 리스너가 없어도, 있어도 `noErr`(0)를 돌려준다. 반환값으로 구분할 수 없다.
- 확인하지 못한 것: Audio MIDI Setup 등 시스템 도구가 쓰는 비공개 경로, MIDI Thru 연결(`MIDIThruConnectionFind`는 영속 Thru 연결만 다루므로 일반 앱의 입력 연결과 무관하다고 판단하고 실험하지 않음).

결론: 공개 API로는 가상 소스의 리스너 유무를 알 수 없다.

## 계획에 미치는 영향

- S8-T1은 「리스너 표시」가 아니라 「안내만」 분기를 택한다. 온보딩에서 가상 포트 이름과 외부 신디의 입력 선택 절차를 보여 주고, 「테스트 노트 보내기」와 체크리스트로 사용자가 직접 확인하게 한다.
- PRE-2의 「(확인 가능하면)」 조건은 확인 불가로 확정되므로 상태줄·포트 설정 화면에 「리스너 연결됨」 표시는 구현하지 않는다. 대신 「포트 N개 활성, 마지막 테스트 노트 송신 시각」 같은, 앱이 스스로 아는 정보만 표시한다. 비공개 경로는 쓰지 않는다.
- REQ 리스크 표의 「신디 연결 전엔 무음」 완화책은 온보딩 안내, 추천 신디 목록, 시각 메트로놈, `:test`에 의존한다(리스너 감지는 완화책에서 빠진다). REQ 문구를 바꿀 필요가 있는지는 사람이 판단하고, 필요하면 `docs/decisions/`에 결정 요청을 쓴다.
