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

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 같은 이름 포트가 여럿일 때 엔진(열거 순서 첫 포트)과 `Reconnector`(고유 ID 최소)가 다른 포트를 고른다 | 엔진도 같은 이름 가운데 고유 ID가 가장 작은 포트를 연다(`best_match`). 둘이 늘 같은 포트를 본다 |
| 지적 2: `is_present`가 이름을 보지 않아, 감시 없이 이름이 바뀐 포트로 소리가 간다 | 연 핸들과 함께 그때의 `PortInfo`를 기억한다. 재생 시작 준비는 목록에서 그 이름의 대상이 아직 같은 포트(고유 ID·엔드포인트)인지 본다. 이름이 바뀌었거나 더 작은 ID의 같은 이름 포트가 생겼으면 다시 연다. 재생 중 편집은 목록을 다시 읽지 않고(`is_present`만), 변화는 감시가 알린다 |
| 지적 3: 서버 확인의 복구 실패가 두 번씩, 1초마다 쌓인다 | 실패는 잇단 실패의 처음 한 번만 알리고, 실패하는 동안은 10초마다 시도한다. 감시자 재시작은 등록에 성공했을 때만 한다 |
| 제안 4: 마지막 `swap` 실패가 등록 결과를 지운다 | 등록 결과를 먼저 내고, 되돌리기 실패는 등록이 성공했을 때만 오류로 낸다 |
| 제안 5: 돌아온 포트에 프로그램을 두 번 보낸다 | 그 목적지의 지난 음색을 잊고 스냅샷을 교체한다. 교체가 「바뀐 음색」으로 한 번만 보낸다 |
| 제안 6: 쓰지 않는 포트의 핫플러그가 상태줄에 보이지 않는다 | OUT-3 「핫플러그와 포트 소멸을 감지해 상태줄에 표시」를 따라, 그 포트를 쓰지 않는 창에 「새로 보인다」·「없어졌다」 메시지를 남긴다. 쓰는 창은 재연결 문구(「사라졌다」·「다시 연결했다」)만 받는다. REQ 문구 그대로의 해석이라 결정 요청은 내지 않았다 |

부록 A·B·C 테스트는 `tests/review_s2_t5d.rs`로 옮겨 통과한다. 두 테스트는 전제를 고쳤다.
- `break_duplicate_names_engine_port_vanishes_while_lower_id_remains`: 지적 1의 수정으로 엔진이 ID가 작은 포트를 쓴다. 그 포트를 지우고 남은 포트로 옮겨 가는지 본다.
- `vanish_and_return_messages_only_user_window_and_other_keeps_playing`: 제안 6 대응으로 쓰지 않는 창도 핫플러그 알림을 받는다. 그 창에 재연결 문구가 가지 않는지 본다.

## 리뷰 2 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 감시 중 재생하며 쓰는 포트의 이름이 바뀌면 「사라졌다」 뒤에도 그 포트로 소리가 간다 | `Lost`를 받은 창이 재생 중이면 스냅샷을 다시 만든다(`transport_port_lost`). 엔진은 그 이름을 「없음」으로 알고 있어 그 트랙을 빼고, 재생 시작과 같은 「없어 소리 내지 않는다」를 알린다 |
| 제안 2: 끊긴 적 없이 더 작은 ID의 같은 이름 포트로 옮길 때 「다시 연결했다」 | 그 경우는 「같은 이름의 다른 포트로 바꿨다」로 알린다. 이 이벤트로 끊긴 뒤 남은 같은 이름 포트로 옮기는 것은 그대로 「다시 연결했다」다 |
| 제안 3: 포트를 많이 내는 장치가 메시지 목록을 밀어낸다 | 한 번의 감시에서 쓰지 않는 포트의 나타남·사라짐은 종류마다 한 줄로 묶는다(「출력 포트 N개가 새로 보인다: …」). 이름은 모두 적는다 |
| 부록 B(판정 보류) | 같은 프로세스에서 만든 목적지의 알림이 다른 테스트 뒤에 오지 않는 테스트 환경 문제로 보고 그대로 둔다 |

부록 A 테스트는 `tests/review_s2_t5d_r2.rs`로 옮겨 통과한다. 리뷰 테스트 셋은 위 대응에 맞게 전제를 고쳤다.
- `many_port_hotplug_is_bounded_and_status_shows_last`, `hotplug_wording_per_window_and_rename_order`: 묶은 줄에도 이름이 모두 들어 있어 그대로 통과한다. 고친 것은 없다.
- `closed_window_port_changes_are_harmless`: 새 창은 다른 테스트 포트의 핫플러그 알림을 받으므로 「메시지 없음」 대신 「재연결 문구 없음」을 본다.
- `duplicate_names_consistent_choice_reconnects`: 수정 없이 통과한다(끊긴 뒤 옮김은 「다시 연결했다」).
