# Task 보고: S2-T5d 포트 감시와 재연결

- PR: 브랜치 `s2/t5d-port-watch`
- 계획 파일: docs/reports/s2-t5d/plan.md
- 계획 대비 달라진 점
  - `engine::recover_virtual_port`의 오류 타입은 `RecoverError`(`Midi`, `PortBusy`)다. 스케줄러가 가상 포트를 2 s 안에 놓지 않는 경우를 `MidiError`로 나타낼 수 없어서다.
  - 테스트·진단용 `engine::opened_ports`와 `ports::port_present`가 공개 인터페이스에 더해졌다.

## 구현 요약

- `src/ports.rs`(새 파일): 앱 전역 `PortMonitor`.
  - `start_monitor`: 엔진이 있으면 백그라운드에서 `PortWatcher::start`를 부른다(블로킹). 메인 스레드 작업이 100 ms마다 `tick`을 돈다. 앱 `run()`이 엔진 설치 뒤 부른다.
  - `tick`
    - 이벤트를 비워 지금 목록(`present`)과 `Reconnector`에 넣는다.
    - 그 뒤 열린 창들의 트랙이 쓰는 이름 포트 가운데 새 이름을 지금 목록으로 등록한다. 이벤트를 먼저 반영하는 이유: 쌓여 있던 나타남 이벤트가 「다시 연결」을 잘못 알리지 않게 한다.
  - `Lost`: 엔진 핸들을 놓고(`port_lost`), 그 포트를 쓰는 창마다 「사라졌다」 메시지를 남긴다.
  - `Reconnect`: 엔진의 옛 핸들과 「없음」 기억을 지운다(`port_returned`). 그 포트를 쓰는 창마다 「다시 연결했다」 메시지를 남기고, 재생 중이면 스냅샷을 다시 만든다. 그 목적지의 뱅크·프로그램도 다시 보낸다(`transport_port_returned`).
  - 10번에 한 번 기본 가상 포트의 `is_registered`를 보고, 거짓이면 `recover_virtual_port`를 부른다.
  - 감시자를 시작하지 못하면 열린 창에 알리고 감시 없이 계속 뜬다.
- `src/engine.rs`
  - `main`은 `Arc<VirtualOutput>`, `opened`는 `Arc<OsOutput>`로 구체 타입을 갖는다.
  - 재생 시작 준비(`prepare`, `live` 아님)는 이 프로젝트가 쓰지 않는 연 포트를 놓는다. 다시 쓰기 전에는 `is_present`를 보고, 사라졌으면 놓고 다시 찾는다(S2-T5a 리뷰 1 제안 1·6 이월).
  - `recover_virtual_port`
    - 재생 중인 창을 멈추고 알린다.
    - 스케줄러에 빈 스냅샷을 넣은 뒤, 옛 스냅샷이 돌아와 가상 포트의 참조가 하나가 될 때까지 기다린다(최대 2 s). 그다음 `reregister`한다. 이렇게 해서 실시간 스레드가 보내는 동안에는 엔드포인트를 바꾸지 않는다.
    - 포트 0이 기본 가상 포트인 스냅샷으로 되돌리고, 열린 창에 알리고, 감시자를 다시 시작한다.
- `src/window.rs`
  - 열린 창 목록(`OpenWindows`, 약한 참조), `project_windows`, `say_everywhere`를 더했다.
  - 상태줄에 「포트 없음」(`status-port-missing`)을 더했다. 현재 트랙의 이름 포트가 감시에 없을 때만 보인다.
- `src/transport.rs`: `transport_stopped_by_server`, `transport_port_returned`.

## 테스트 결과

~~~
cargo test --workspace: 1313 passed, 0 failed, 0 ignored
port_watch.rs 6개: 병렬 실행 3번 연속 통과(약 1.4 s)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `missing_port_shows_in_status_bar` | 통과 |
| 2 | `port_vanish_and_return_updates_status_and_messages` | 통과 |
| 3 | `playback_reconnects_to_returned_port` | 통과 |
| 4 | `unused_opened_ports_are_released` | 통과(엔진 단위 대신 UI에서 `engine::opened_ports`로 본다) |
| 5 | `recover_virtual_port_stops_playback_and_reregisters` | 통과 |
| 6 | `monitor_without_engine_is_inert` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`

## 알려진 한계

- MIDI 서버 재시작의 감지(`is_registered`가 거짓)는 자동 테스트로 일으킬 수 없다. MIDI 서버를 끄는 것은 금지다. 복구 절차는 등록된 포트에도 같게 동작하므로 그 절차만 테스트했다. 감지는 S2 출구 수동 확인 항목이다.
- `Reconnector`에서 이름을 지우는 API가 없다. 더 쓰지 않는 이름도 등록이 남지만, 메시지는 그 포트를 쓰는 창에만 간다.
- 사라진 포트의 대체 매핑 제안(OUT-8)은 S2-T7(파일 열기)이다.
- 같은 이름의 다른 포트가 남아 있을 때 사라지면 「사라졌다」와 「다시 연결했다」가 연달아 남는다.

## 리뷰어에게

- `tick`의 순서(이벤트 반영 → 새 이름 등록)와, 알림이 목록보다 늦게 오는 경우. 테스트는 감시가 목적지를 볼 때까지 기다린 뒤 창을 연다(`wait_seen`). 그 경합이 사용자에게 어떻게 보이는지(늦게 꽂힌 장치에 「다시 연결했다」) 판단 바란다.
- `reregister_main`이 스케줄러의 참조를 기다리는 방식(빈 스냅샷 + `collect_garbage` + `Arc::get_mut`).
- 재생 중 포트 복귀 때 `transport_document_changed`와 뱅크·프로그램 재송신이 겹쳐 같은 메시지를 두 번 보낼 수 있다.
