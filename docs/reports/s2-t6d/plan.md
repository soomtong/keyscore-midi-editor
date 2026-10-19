# Task 계획: S2-T6d 설정 > MIDI 출력의 가상 포트

- 브랜치: `s2/t6d-midi-output`
- 관련 REQ: OUT-1, OUT-2, OUT-3(서버 재시작 재등록), OUT-8(포트 이름 저장), 7.6(가상 포트 추가·이름 변경 = 설정 > MIDI 출력), UI-11
- 선행 Task: S2-T5a(엔진), S2-T5d(포트 감시·재등록), S2-T6c(설정 창 구역)
- 관련 스파이크: 없음
- 결정: `docs/decisions/2026-10-19-virtual-port-rename-tracks.md` A(이름을 바꾸면 열린 프로젝트의 트랙도 고친다)
- 나눔: 단계 파일의 S2-T6d를 2026-10-19에 다시 나눴다. 포트별 클럭·테스트 노트·포트 상태 표시는 S2-T6e, 이월 줄은 S2-T6f다.

## 끝의 정의 (내 말로)

1. 포트 모델
   - 앱의 가상 포트는 기본 포트 하나(스냅샷 포트 0, 트랙의 `OutputPort::Virtual`) + 추가 포트 0–15개다. 합쳐 16개를 넘지 않는다(OUT-2, 프로세스 상한과 같다).
   - 트랙은 추가 가상 포트를 이름으로 가리킨다(`OutputPort::Named(이름)`, OUT-8). 엔진은 이름 포트를 먼저 앱의 가상 포트에서 찾고(기본 포트 이름 포함), 없으면 CoreMIDI 목적지에서 찾는다. 앱의 가상 포트는 소스라 목적지 목록에 없다. 같은 이름의 목적지가 따로 있어도 앱 포트가 이긴다.
   - `.ksq` 포맷과 core 타입은 바꾸지 않는다.
2. 설정 파일 `midi_output`
   - `main_name`(있으면 기본 포트 이름. 없으면 OUT-1의 "KeyScore Out"), `virtual_ports`(추가 포트 이름 목록). 모르는 키 보존, 잘못된 값은 그 필드만 기본값 + 경고(이름 규칙 위반·중복·16 초과 항목은 빼고 경고).
   - 앱 시작(`run`)은 `main_name`으로 기본 포트를 만들고, 엔진 설치는 추가 포트를 만든다. 만들지 못한 포트는 상태줄에 알린다.
3. 설정 > MIDI 출력 구역
   - 가상 포트 목록(기본 포트가 맨 위). 목록은 위아래 화살표로 고르고, 고른 줄에서 이름 입력란으로 이름을 바꾼다(Enter 확정, Esc 취소).
   - 「가상 포트 추가」 버튼(`midi::AddVirtualPort`): 「KeyScore Out 2」처럼 비어 있는 이름으로 하나를 만들고 그 줄을 고른다. 16개면 알리고 만들지 않는다.
   - 「삭제」 버튼(새 `midi::RemoveVirtualPort`): 고른 추가 포트를 지운다. 기본 포트는 지울 수 없다. 그 포트를 쓰던 트랙은 이름을 그대로 두어 「포트 없음」이 된다(알림).
   - `midi::RenameVirtualPort(새 이름)`: 고른 포트(설정 창이 없으면 기본 포트)의 이름을 바꾼다. 키맵·JSON에서 인자를 주어 부를 수 있다. 팔레트에서는 인자를 받을 수 없어 기존 규칙대로 설명 꼬리를 단다.
   - 이름 규칙: keyscore-midi의 이름 규칙(빈 이름, 제어 문자 등)과 보이는 모양 기준 중복(결정 `2026-10-15-port-name-visual-duplicates.md`). 지금 보이는 CoreMIDI 목적지 이름과 같은 이름도 거부한다(트랙의 이름 포트가 헷갈린다).
4. 이름 변경(결정 A)
   - CoreMIDI 엔드포인트 이름만 바꾼다(고유 ID 유지, 외부 신디 연결 유지). 재생 중에도 끊기지 않는다.
   - 열린 창마다 옛 이름을 가리키던 트랙을 새 이름으로 고친다(창마다 undo 한 단위). 기본 포트를 바꾸면 `Virtual` 트랙은 그대로이고, 옛 기본 포트 이름을 이름으로 가리키던 트랙만 고친다.
   - 메트로놈 포트 설정이 옛 이름이면 새 이름으로 고친다.
5. 라우팅·감시
   - 트랙·메트로놈의 이름 포트가 앱의 가상 포트면 그 포트로 보낸다. 포트 감시는 앱 포트 이름을 「사라진 목적지」로 다루지 않는다. 상태줄 「포트 없음」도 앱 포트 이름은 있음으로 본다.
   - 트랜스포트·인스펙터의 포트 메뉴와 메트로놈 포트 선택에 추가 가상 포트가 보인다.
   - MIDI 서버가 다시 뜨면 기본 포트와 추가 포트를 모두 다시 등록한다.
6. keyscore-midi 변경: `VirtualOutput::rename`이 `&self`로 이름을 바꿀 수 있게 이름을 안쪽 잠금에 둔다(스냅샷이 포트를 `Arc`로 쥔 채 재생 중에도 이름을 바꾼다). 실시간 스레드의 `send`는 이름을 읽지 않으므로 잠금이 실시간 경로에 들어가지 않는다. 그래서 `MidiPort::name`은 `String`을 돌려준다.

## 공개 인터페이스 초안

~~~rust
// keyscore_midi
impl VirtualOutput { pub fn rename(&self, name: &str) -> Result<(), MidiError>; }
pub trait MidiPort { fn name(&self) -> String; /* … */ }

// keyscore_app::settings
pub struct MidiOutputSettings { pub main_name: Option<String>, pub virtual_ports: Vec<String> }

// keyscore_app::engine
pub fn virtual_port_names(cx: &App) -> Vec<String>;   // 기본 포트 먼저
pub fn port_choices(cx: &App) -> Vec<OutputPort>;      // 기본, 추가 가상, 목적지
pub fn add_virtual_port(cx: &mut App) -> Result<String, PortEditError>;
pub fn rename_virtual_port(cx: &mut App, old: &str, new: &str) -> Result<(), PortEditError>;
pub fn remove_virtual_port(cx: &mut App, name: &str) -> Result<(), PortEditError>;

// keyscore_app::actions::midi
pub struct RemoveVirtualPort; // 새 Action
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `midi_output_settings_round_trip` | 단위 | `midi_output` 읽기·쓰기, 모르는 키, 잘못된 이름·중복·17개째는 빼고 경고 |
| 2 | `virtual_output_renames_while_shared` | 통합(midi) | `Arc`로 쥔 가상 포트의 이름을 바꾸면 CoreMIDI 소스 이름이 바뀌고 같은 엔드포인트로 계속 받는다. 중복 이름은 거부 |
| 3 | `add_virtual_port_routes_named_tracks` | 루프백 | 추가한 포트 「… 2」를 트랙이 이름으로 고르면 재생 노트가 그 소스로만 간다. 포트 메뉴 선택지에 보인다. 16개 상한 |
| 4 | `rename_virtual_port_updates_open_tracks` | 루프백·UI | 설정 창에서 이름을 바꾸면 소스 이름이 바뀌고, 열린 두 창의 그 포트 트랙이 새 이름(창마다 undo 한 단위, undo로 옛 이름), 재생 중 바꿔도 소리가 계속 그 포트로 간다. 메트로놈 포트 설정도 따라간다 |
| 5 | `rename_rejects_bad_names` | UI | 빈 이름, 다른 가상 포트와 보이는 모양이 같은 이름, 지금 목적지 이름은 거부하고 알린다. Esc는 취소 |
| 6 | `remove_virtual_port_leaves_tracks_missing` | UI | 삭제하면 소스가 사라지고 설정에서 빠진다. 그 포트 트랙은 「포트 없음」. 기본 포트는 삭제 버튼이 꺼져 있다 |
| 7 | `settings_midi_output_keyboard_flow` | UI | 키보드만: 구역 열기 → 추가 → 이름 입력란에서 이름 치고 Enter → 위아래로 고르기 → 삭제 |
| 8 | `virtual_ports_created_from_settings_at_install` | 루프백 | 설정 파일의 추가 포트가 엔진 설치 때 생긴다. 상태줄 「포트 없음」·포트 감시가 앱 포트를 없다고 하지 않는다 |

## 불변식

- 앱 가상 포트는 합쳐 16개 이하, 보이는 모양 기준으로 이름이 겹치지 않는다(keyscore-midi가 지킨다).
- 기본 포트는 늘 스냅샷 포트 0이고 지울 수 없다.
- 설정의 `virtual_ports`는 엔진에 실제로 있는 추가 포트와 같다(추가·이름 변경·삭제가 둘을 함께 바꾼다. 엔진 실패면 설정도 바꾸지 않는다).
- 실시간 스레드의 송신 경로에 잠금이 없다.

## 범위 밖

- 포트별 MIDI Clock, 테스트 노트, 포트 상태 표시: S2-T6e.
- 닫힌 프로젝트 파일의 옛 이름 트랙(결정 A), 다른 Mac에서 연 파일의 대체 매핑 제안(OUT-8 뒤 절): 파일 열기 흐름(S2-T7)에서 본다.
- 가상 포트 순서 바꾸기.

## 리스크·결정 요청

- `MidiPort::name`의 반환형을 `String`으로 바꾸는 것은 keyscore-midi 공개 API 변경이다. 이 trait을 구현한 기존 테스트의 가짜 포트 몇 개도 시그니처를 맞춘다(동작은 그대로).
- 새 Action `midi::RemoveVirtualPort`는 REQ 7.6 대응표의 「가상 포트 추가·이름 변경」 행을 보완한다(16개 상한이 있어 지우는 길이 필요하다). Action 수 295 → 296.
- 추가 포트의 기본 이름 「KeyScore Out N」은 UI 세부다.
