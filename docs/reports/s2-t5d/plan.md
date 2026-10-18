# Task 계획: S2-T5d 포트 감시와 재연결

- 브랜치: `s2/t5d-port-watch`(S2-T5를 나눈 넷째 Task)
- 관련 REQ: OUT-1, OUT-3, OUT-4, UI-5, 7.1(상태줄)
- 선행 Task: S2-T5c(상태줄·메시지), S2-T5a(엔진), S1-T2(`PortWatcher`, `Reconnector`, `is_present`, `reregister`)
- 관련 스파이크: 없음
- 이월 받음
  - S1-T1·S1-T2: `PortWatcher` 이벤트로 상태줄 표시와 `Reconnector` 재연결. 열린 `OsOutput`은 `is_present`로 확인한다. `PortWatcher::start`는 메인 스레드가 아닌 곳에서 부른다(약 0.25 s, 최악 2 s 블로킹). MIDI 서버 재시작(`is_registered`가 거짓)이면 가상 포트를 `reregister`하고 상태줄에 알리며, 감시자도 다시 시작한다.
  - S2-T5a 리뷰 1 제안 1·6: 한 번 연 OS 포트(`Engine::opened`)가 쓰이지 않아도 닫히지 않고, 사라졌는지 확인하지 않는다.

## 끝의 정의 (내 말로)

1. 포트 감시(`src/ports.rs`, 앱 전역 `PortMonitor`)
   - `ports::start_monitor(cx)`: 엔진이 있으면 백그라운드에서 `PortWatcher::start`를 부르고, 성공하면 전역에 둔다. 앱 `run()`은 엔진 설치 뒤 부른다. 실패하면(`NotificationsUnavailable` 등) 감시 없이 계속 뜨고 열린 창에 알린다.
   - 감시자는 100 ms마다 이벤트를 비운다. 지금 있는 목적지 목록과 `Reconnector`를 갖고, 원하는 이름은 열린 창들의 트랙이 쓰는 이름 포트 전체다.
   - 사라짐(`Action::Lost`): 엔진의 그 이름 핸들을 놓는다. 그 포트를 쓰는 창마다 「출력 포트 「X」이 사라졌다. 같은 이름의 포트가 돌아오면 다시 연결한다」를 남긴다.
   - 돌아옴(`Action::Reconnect`): 엔진의 옛 핸들을 놓고 「없음」 기억을 지운다. 그 포트를 쓰는 창마다 「출력 포트 「X」에 다시 연결했다」를 남긴다. 그 포트를 쓰는 창이 재생 중이면 스냅샷을 다시 만들어 교체하고, 그 목적지의 뱅크·프로그램을 다시 보낸다(장치가 새로 켜졌을 수 있다).
2. 상태줄(UI-5): 현재 트랙의 이름 포트가 지금 없으면 트랙 글 옆에 「포트 없음」을 경고색으로 보인다(`status-port-missing`). 감시가 없으면(엔진 없음) 보이지 않는다.
3. 연 포트 정리(이월)
   - 엔진은 연 OS 포트를 구체 타입(`OsOutput`)으로 갖고, 재생 시작의 스냅샷 준비 때 이 프로젝트가 쓰지 않는 핸들은 놓는다. 스냅샷이 들고 있는 동안은 살아 있으므로 소리는 끊기지 않는다.
   - 재사용 전에 `is_present`를 보고, 사라진 핸들은 놓고 다시 연다(없으면 「없음」).
4. MIDI 서버 재시작(OUT-1)
   - 감시자는 1 s마다 기본 가상 포트의 `is_registered`를 본다. 거짓이면 `engine::recover_virtual_port`를 부른다.
   - `recover_virtual_port`: 재생 중인 창을 멈추고(「MIDI 서버가 다시 시작되어 재생을 멈췄다」), 스케줄러에 빈 스냅샷을 넣어 가상 포트의 다른 참조를 돌려받은 뒤 `reregister`한다. 열린 창에 「가상 포트 「KeyScore Out」을 다시 등록했다」를 남기고 감시자를 다시 시작한다.
   - 서버 재시작은 자동 테스트로 일으킬 수 없다(MIDI 서버를 끌 수 없다). `recover_virtual_port`는 등록된 포트에도 같게 동작하므로(같은 이름으로 새 엔드포인트) 그 절차를 테스트한다. 감지(`is_registered` 거짓)는 S2 출구 수동 확인 항목으로 둔다.

## 공개 인터페이스 초안

~~~rust
// keyscore_app::ports
pub fn start_monitor(cx: &mut App);
pub fn is_monitoring(cx: &App) -> bool;
pub const POLL_INTERVAL: Duration; // 100 ms
// keyscore_app::engine
pub fn recover_virtual_port(cx: &mut App) -> Result<(), MidiError>;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `missing_port_shows_in_status_bar` | UI + CoreMIDI | 없는 이름 포트 트랙이 현재 트랙이면 상태줄에 「포트 없음」, 가상 포트 트랙이면 없음 |
| 2 | `port_vanish_and_return_updates_status_and_messages` | UI + CoreMIDI | 쓰는 목적지를 지우면 「사라졌다」 메시지와 「포트 없음」, 같은 이름으로 다시 만들면 「다시 연결했다」와 표시가 사라짐. 쓰지 않는 포트의 변화는 메시지를 남기지 않는다 |
| 3 | `playback_reconnects_to_returned_port` | UI + 루프백 | 재생 중 목적지가 사라졌다 돌아오면 새 목적지로 프로그램과 노트가 닿는다 |
| 4 | `unused_opened_ports_are_released` | 단위(엔진) | 재생 시작 준비 때 이 프로젝트가 쓰지 않는 연 포트를 놓는다. 사라진 핸들은 다시 열지 않고 「없음」 |
| 5 | `recover_virtual_port_stops_playback_and_reregisters` | UI + 루프백 | 재생 중 복구: 재생 멈춤, 메시지, 같은 이름 소스가 남고 다시 재생하면 노트가 닿는다 |
| 6 | `monitor_without_engine_is_inert` | UI | 엔진 없이 `start_monitor`는 아무 일도 하지 않는다 |

## 불변식

- 스케줄러 스레드는 바뀌지 않는다. 포트 확인·열기·해제는 메인 스레드에서만 한다.
- 스냅샷이 쓰는 포트는 스냅샷이 놓을 때까지 산다(`Arc`). 엔진이 놓아도 재생 중 소리가 끊기지 않는다.

## 범위 밖

- 사라진 포트의 대체 매핑 제안(`suggest_mapping`, OUT-8): 파일 열기(S2-T7)
- 포트 리스너 표시(PRE-2), 가상 포트 추가·이름 바꾸기(OUT-2 UI)
- 창 닫기 훅(S2-T7)

## 리스크·결정 요청

- 서버 재시작 감지는 수동 확인으로만 볼 수 있다. 출구 보고서의 수동 항목에 넣는다.
