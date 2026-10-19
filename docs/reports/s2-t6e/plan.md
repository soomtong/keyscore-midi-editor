# Task 계획: S2-T6e 포트별 MIDI Clock, 테스트 노트, 포트 상태 표시

- 브랜치: `s2/t6e-clock-test-note`
- 관련 REQ: OUT-5, PRE-2, PRE-3, 7.6(연결 테스트 노트 = 설정 > MIDI 출력, 트랙 인스펙터), UI-11
- 선행 Task: S2-T6d(가상 포트, 설정 > MIDI 출력 구역), S2-T6c(메트로놈 교체 묶음), S1-T4·T5(스케줄러 `set_clock`, Start/Stop/Continue/SPP)
- 관련 스파이크: SP-4(`docs/spikes/sp4-listener-detect.md`: 공개 API로 리스너를 알 수 없다)
- 이월: 단계 파일 S2-T6 절의 「S2-T6c 리뷰 2 제안 1: 교체 묶음의 실패 처리」

## 끝의 정의 (내 말로)

1. 포트별 MIDI Clock(OUT-5)
   - 설정 `midi_output.clock`: 클럭을 보낼 포트 목록(`"virtual"` 또는 `{"named": 이름}`, 메트로놈 포트와 같은 모양). 기본 빈 목록(끔).
   - 설정 > MIDI 출력에 「MIDI Clock」 목록: 지금 포트 선택지(기본 가상, 추가 가상, 목적지, 목록에 없는 켜 둔 포트)마다 체크 상자 하나. 체크하면 그 포트에 24 ppqn Clock과 Start/Stop/Continue/SPP를 보낸다(스케줄러가 이미 한다).
   - 엔진 `prepare`가 클럭 포트를 트랙·메트로놈 포트와 같은 규칙으로 스냅샷에 넣는다(트랙이 쓰지 않아도). 찾지 못한 포트는 건너뛰고 상태줄에 한 번 알린다.
   - 재생 시작: 교체 뒤, 재생 전에 클럭 포트 번호를 켠다(지난 번호는 먼저 끈다). 재생 중 교체: 클럭 (이름, 번호) 짝이 바뀌었을 때만, 교체 전에 옛 번호를 끄고 교체 뒤에 새 번호를 켠다(메트로놈과 같은 이유: 번호는 스냅샷마다 다시 매겨진다).
   - 재생 중 클럭 설정을 바꾸면 곧바로 반영한다. 재생 중에 새로 켠 포트는 Clock만 받고 Start/SPP는 다음 재생 시작부터다(스케줄러 동작).
   - 가상 포트 이름을 바꾸면 클럭 목록의 그 이름도 따라간다(메트로놈 포트와 같다).
2. 교체 묶음의 실패 처리(이월)
   - 재생 중 교체의 메트로놈·클럭 명령이 큐에서 실패하면, 다음 교체(편집·토글·설정 변경)가 메트로놈 설정과 클럭 번호를 처음부터 다시 보낸다(「지금 스케줄러에 들어간 상태」를 기억하지 않고 실패 표시를 둔다). 메트로놈 버튼 토글은 `set_metronome`이 실패한 상태에서 켜지 않는다.
3. 테스트 노트(PRE-3)
   - 설정 > MIDI 출력에 「테스트 노트 보내기」 버튼. 편집할 포트(가상 포트)로 채널 1, 가운데 C, 200 ms를 보낸다. 설정 창에서 `midi::SendTestNote`를 내도(팔레트·키) 같은 일을 한다. 프로젝트 창의 같은 Action은 지금처럼 현재 트랙으로 보낸다.
   - 정지 중이면 기본 포트와 그 포트를 든 스냅샷을 넣고 스케줄러 미리듣기로 보낸다(인스펙터의 정지 중 경로와 같다). 재생 중이면 그 재생의 스냅샷에 그 포트가 있을 때만 보내고, 없으면 「재생 중에는 …」으로 알린다(재생 스냅샷을 바꾸지 않는다). 기본 포트는 늘 보낼 수 있다.
4. 포트 상태 표시(PRE-2, SP-4)
   - 리스너 연결 여부는 알 수 없다(SP-4). 설정 > MIDI 출력에 앱이 아는 정보만 보인다: 「가상 포트 N개 활성」, 「마지막 테스트 노트: 시:분:초, 포트 이름」(앱 실행 중, 설정 창·인스펙터 어느 쪽에서 보냈든), 「들리지 않으면 신디의 MIDI 입력에서 이 포트를 고른다」 안내.
   - 상태줄은 지금처럼 테스트 노트를 보낼 때 메시지를 남긴다(새 표시를 더하지 않는다).

## 공개 인터페이스 초안

~~~rust
// keyscore_app::settings
pub struct MidiOutputSettings { pub main_name: Option<String>, pub virtual_ports: Vec<String>, pub clock: Vec<OutputPort> }
// keyscore_app::engine
pub(crate) struct Prepared { /* … */ pub(crate) clock: Vec<(String, u16)>, pub(crate) clock_missing: Vec<(String, PortProblem)> }
pub fn last_test_note(cx: &App) -> Option<(std::time::SystemTime, String)>;
pub fn clock_commands(old: Option<&[(String, u16)]>, new: &[(String, u16)]) -> (Vec<u16>, Vec<u16>); // (끌 번호, 켤 번호)
// keyscore_app::window::ProjectWindow
pub fn clock_ports(&self) -> Vec<String>; // 지금 재생 스냅샷에서 클럭을 켠 포트 이름(시험용)
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `clock_settings_round_trip` | 단위 | `midi_output.clock` 읽기·쓰기, 잘못된 항목은 빼고 경고, 중복 제거 |
| 2 | `clock_port_receives_start_clock_stop` | 루프백 | 이름 포트(테스트 목적지, 트랙이 쓰지 않음)에 클럭을 켜고 재생하면 Start(0xFA) 뒤 Clock(0xF8)이 박당 24개 가까이 오고, 정지하면 Stop(0xFC). 클럭을 켜지 않은 기본 포트에는 0xF8이 오지 않는다 |
| 3 | `clock_toggle_while_playing_and_survives_edits` | 루프백 | 재생 중 체크하면 Clock이 오기 시작하고, 끄면 멈춘다. 재생 중 트랙을 앞에 더해 포트 번호가 바뀌어도 클럭이 엉뚱한 포트로 가지 않는다 |
| 4 | `settings_test_note_sends_to_selected_port` | 루프백·UI | 설정 창의 버튼과 설정 창에서 낸 `midi::SendTestNote`가 고른 추가 가상 포트로 Note On/Off(ch1, 60)를 보낸다. 마지막 테스트 노트 표시가 갱신된다. 다른 창이 재생 중이고 그 포트가 재생에 없으면 보내지 않고 알린다 |
| 5 | `port_status_shows_known_facts` | UI | 상태 줄: 가상 포트 수, 마지막 테스트 노트(인스펙터에서 보낸 것 포함), 리스너 안내 |
| 6 | `clock_commands_resend_everything_after_failure` | 단위 | 클럭 명령 계산(`engine::clock_commands`): 짝이 같으면 명령 없음, 바뀌면 옛 번호 끄기·새 번호 켜기, 지난 묶음이 실패해 상태를 모르면 모든 번호를 끄고 새 번호를 켠다. 큐를 앱 공개 API로 채울 수 없어 실패 자체는 계산 함수의 입력으로 준다 |
| 7 | `clock_follows_virtual_port_rename` | UI | 클럭을 켠 추가 가상 포트의 이름을 바꾸면 클럭 목록도 새 이름 |

## 불변식

- 클럭 번호는 늘 지금 스냅샷의 번호다(교체마다 맞춘다).
- 설정의 클럭 목록은 포트 모양이 메트로놈 포트와 같고 중복이 없다.

## 범위 밖

- 리스너 연결 표시(SP-4로 불가).
- 클럭 송신 지터 측정(S1-T4 NF 게이트가 이미 본다).
- 상태줄의 새 표시.

## 리스크·결정 요청

- 테스트 노트의 채널 1·가운데 C·벨로시티 100·200 ms는 UI 세부다(PRE-1 미리듣기 기본 길이와 같다).
- 교체 묶음 실패는 앱 공개 API로 일으킬 수 없다. 실패 뒤의 동작은 계산 함수로 시험한다.
