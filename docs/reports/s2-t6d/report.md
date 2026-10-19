# Task 보고: S2-T6d 설정 > MIDI 출력의 가상 포트

- PR: 브랜치 `s2/t6d-midi-output`
- 계획 파일: docs/reports/s2-t6d/plan.md
- 계획 대비 달라진 점
  - keyscore-midi는 `rename(&mut self)`를 그대로 두고 공유 중에 부르는 `set_name(&self)`를 더했다. `rename`은 `set_name`을 부른다. 기존 테스트의 `let mut port`가 그대로 컴파일된다.
  - keyscore-midi에 이름 검사 공개 함수 둘(`normalize_port_name`, `same_port_name`)을 더했다. 앱이 설정 파일·새 이름을 엔진에 넘기기 전에 같은 규칙으로 본다.
  - `MidiPort::name`이 `String`을 돌려주게 바뀌어, 그것을 구현·비교하던 기존 테스트 파일 11곳의 시그니처·비교식을 맞췄다(동작은 그대로).
  - 엔진이 설치 때 만들지 못한 추가 포트는 설정에서 빼지 않고 알린다(다음 시작 때 다시 해 본다). 그래서 「설정 = 엔진」 불변식은 「추가·이름 변경·삭제는 둘을 함께 바꾼다」로 좁혔다.
  - `engine::rename_virtual_port`는 고친 새 이름을 돌려준다.
  - 「16개 상한」 테스트는 프로세스 전역 상한이라 자식 프로세스에서 돈다(`add_virtual_port_stops_at_sixteen`).

## 구현 요약

- keyscore-midi `client.rs`
  - `VirtualOutput`의 이름을 `Mutex<String>`에 둔다.
  - `set_name(&self)`: 이름 목록 잠금, 포트 이름 잠금 순서로 잡고(만들기·drop과 같은 순서) 중복을 보고, `kMIDIPropertyName`을 바꾼다. 엔드포인트는 그대로라 고유 ID와 외부 앱의 연결이 유지된다.
  - 실시간 스레드의 `send`는 이름을 읽지 않는다.
- `src/settings.rs`: `MidiOutputSettings { main_name, virtual_ports }`.
  - 항목마다 검사한다: 이름 규칙, 보이는 모양 중복(기본 포트 포함), 16개 상한. 맞지 않는 항목은 빼고 경고한다.
  - `main_or_default()`.
- `src/engine.rs`
  - `extras`(추가 가상 포트)와 `retired`(지웠지만 스냅샷이 아직 쥔 포트).
  - 설치 때 설정의 추가 포트를 만든다. `run`은 `main_name`으로 기본 포트를 만든다.
  - `prepare`: 이름 포트를 먼저 기본 포트 이름(포트 0), 그다음 추가 포트, 그다음 CoreMIDI 목적지에서 찾는다(`open_port`). 메트로놈 포트도 같다. `routes`에 기본 포트 이름도 넣는다.
  - `virtual_port_names`, `is_own_port`, `port_choices`(기본, 추가 가상, 앱 포트와 이름이 다른 목적지).
  - `add_virtual_port`: 「기본 이름 N」 가운데 앱 포트·목적지와 보이는 모양이 다른 첫 이름.
  - `rename_virtual_port`
    - 새 이름 검사: 규칙, 지금 목적지 이름과 같으면 거부.
    - `set_name` 뒤 설정(`main_name` 또는 목록 항목)과 메트로놈 포트를 고친다.
    - 열린 창마다 옛 이름 트랙을 `SetTrackProp`으로 고친다(창마다 undo 한 단위).
  - `remove_virtual_port`
    - 기본 포트는 거부한다.
    - 재생 중이면 주인 창이 다시 준비하고, 정지 중이면 기본 포트만 든 스냅샷으로 바꾼다.
    - 그 뒤 200 ms까지 스냅샷이 놓기를 기다려 버린다. 남은 것은 포트 감시 주기가 버린다.
  - 서버 재시작 재등록이 추가 포트도 다시 등록한다.
- `src/ports.rs`
  - 앱 포트 이름은 감시 대상(`wanted_names`)에서 뺀다. `port_present`는 앱 포트를 있다고 본다.
  - 감시 주기마다 지운 포트를 정리한다(`housekeeping`).
- `src/settings_ports.rs`(새 파일): MIDI 출력 구역.
  - 포트 목록 표시와 편집할 포트 선택 상자.
  - 이름 입력란: Enter는 `midi::RenameVirtualPort`, Esc는 되돌리기.
  - 추가·삭제 버튼: Tab 정지점, `enter`/`space`. 기본 포트가 골라져 있으면 삭제는 꺼진다.
  - 세 Action의 앱 전역 처리기. 대상은 설정 창에서 고른 포트이고, 설정 창이 없으면 이름 바꾸기는 기본 포트, 삭제는 설정 창을 열고 알린다. 결과·오류는 설정 창 메시지로 알린다.
- `src/settings_tabs.rs`: `row`·선택 상자 도우미를 공용으로 열고, Tab 정지점 버튼 도우미 `action_button`을 보정 마법사와 함께 쓴다.
- 트랜스포트·인스펙터 포트 메뉴와 메트로놈 포트 선택이 `port_choices`를 쓴다.
- 새 Action `midi::RemoveVirtualPort`(Action 296개, 명령 표 「설정 > MIDI 출력」).

## 테스트 결과

~~~
cargo test --workspace: 1476 passed, 0 failed, 2 ignored(무시 둘은 이 Task 전부터 있던 수동 측정 테스트)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `midi_output_settings_round_trip` | 통과 |
| 2 | `virtual_output_renames_while_shared`(keyscore-midi) | 통과 |
| 3 | `add_virtual_port_routes_named_tracks`, `add_virtual_port_stops_at_sixteen` | 통과 |
| 4 | `rename_virtual_port_updates_open_tracks` | 통과 |
| 5 | `rename_rejects_bad_names` | 통과 |
| 6 | `remove_virtual_port_leaves_tracks_missing` | 통과 |
| 7 | `settings_midi_output_keyboard_flow` | 통과 |
| 8 | `virtual_ports_created_from_settings_at_install` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`

## 알려진 한계

- 닫힌 프로젝트 파일의 옛 이름 트랙은 고치지 않는다(결정 A). 열면 「포트 없음」이다. 대체 매핑 제안(OUT-8 뒤 절)은 파일 열기 흐름(S2-T7)에서 본다.
- 앱 포트와 같은 이름의 목적지가 나중에 생기면 앱 포트가 이기고, 그 목적지는 포트 메뉴에 보이지 않는다.
- 설치 때 만들지 못한 추가 포트의 알림은 창이 열리기 전이라 표준 오류에만 남는다.
- 지운 포트는 재생 중 스냅샷이 놓을 때까지(보통 한 주기) CoreMIDI 목록에 남는다. 그동안 같은 이름의 새 포트는 만들 수 없다.
- 팔레트에서 `midi::RenameVirtualPort`는 인자를 받을 수 없어 설명 꼬리만 단다(기존 규칙).

## 리뷰어에게

- `set_name`의 잠금 순서(이름 목록 → 포트 이름)와 drop·만들기·`reregister`의 순서가 엇갈려 교착할 길이 있는지.
- 재생 중 이름 바꾸기·지우기에서 스냅샷 교체와 `routes`·트랙 편집 순서.
- 앱 포트 이름과 목적지 이름이 겹치는 경우(설정 파일 손 편집, 나중에 생긴 목적지)의 라우팅·감시·상태줄.
