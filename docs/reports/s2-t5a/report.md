# Task 보고: S2-T5a MIDI 엔진 연결과 트랜스포트 Action

- PR: 브랜치 `s2/t5-transport-panels`(S2-T5를 나눈 첫 Task)
- 계획 파일: docs/reports/s2-t5a/plan.md
- 계획 대비 달라진 점: 없음. 트랜스포트 Action 처리기는 `src/transport.rs`의 `impl ProjectWindow`에 두었다(창 파일이 커지지 않게). 그래서 `ProjectWindow`의 `document`·`transport` 필드를 crate 안에서 보이게 했다.

## 구현 요약

- core `edit.rs`: 내부 `Op::SetKey { key, extra }`, `Op::SetPort { track, port, extra }`
  - 정방향(`extra: None`)은 값이 바뀔 때만 `key`·`port` 객체의 모르는 키를 지운다.
  - 역편집은 옛 값과 옛 모르는 키를 함께 담는다.
  - 공개 `Edit`는 그대로이고, 히스토리 크기 추정에 모르는 키를 더했다.
- app `engine.rs`: 앱 전역 `Engine`(GPUI Global)
  - `install`이 `MidiClient::new` → 기본 가상 포트 → 빈 스냅샷 스케줄러 순서로 만든다.
  - `prepare`가 프로젝트를 스냅샷으로 바꾼다.
    - 이름 포트는 OS 목적지를 열어 이름별로 재사용하고, 없으면 그 트랙을 빼고 이름을 돌려준다. 포트 상한도 같이 처리한다.
    - 재생 시작 때 보낼 뱅크·프로그램(OUT-4)도 함께 만든다.
  - 재생 중인 창(주인)을 하나만 둔다.
- app `transport.rs`: 창의 트랜스포트
  - Play, PlayFromCursor(S3까지 처음부터), Toggle, Stop, Pause, Panic, ToggleMetronome.
  - 일시정지 자리는 재생 시작 호스트 시각과 템포 맵(정수 시각 `MusicTime`)으로 계산한다.
  - 다른 창이 재생을 가져가면 앞 창을 정지 상태로 바꾼다. 창이 닫히면 그 창의 재생을 멈춘다.
  - 문서가 바뀌면(`cx.observe`) 재생 중일 때 스냅샷을 교체한다(TRN-3).
  - 메시지는 32개까지 남긴다.
- app `lib.rs`: `run`이 창보다 먼저 엔진을 설치한다. 실패해도 앱은 뜬다. `init`은 `CoreMIDI`를 건드리지 않는다.
- 테스트 dev-dependency로 `coremidi`(MIT, keyscore-midi와 같은 버전)를 더했다.

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 1092 passed, 0 failed (리뷰 1 대응 뒤 1114 passed, 0 failed)
keyscore-core unknown_keys_on_change: 5 passed(속성 테스트 포함)
keyscore-app engine: 8 passed(루프백, 5회 반복 모두 통과, 약 1.5 s)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `port_change_clears_unknown_keys_and_undo_restores_them`, `same_port_value_keeps_unknown_keys`, `unknown_keys_do_not_come_back_through_another_value` | 통과 |
| 2 | `key_change_clears_unknown_keys_and_undo_restores_them` | 통과 |
| 3 | `unknown_key_edits_keep_history_invariants` | 통과 |
| 4 | `play_action_sends_notes_to_virtual_port` | 통과(단계 게이트) |
| 5 | `stop_and_panic_silence_output` | 통과 |
| 6 | `pause_resumes_from_paused_position` | 통과 |
| 7 | `metronome_toggle_sends_clicks` | 통과 |
| 8 | `edit_while_playing_is_heard` | 통과 |
| 9 | `missing_named_port_is_reported_and_silent` | 통과 |
| 10 | `transport_without_engine_reports_message` | 통과 |
| 11 | `only_one_window_plays` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(1092 passed)
- [x] deny
- [x] check-deps

## 알려진 한계

- 일시정지 자리는 앱이 계산한 근삿값이다(스케줄러의 실제 송신 위치가 아니다). 루프 구간이 생기면(S3·S4) 계산이 틀어진다. 그때 스케줄러에 위치 조회를 더할지 정한다.
- 없는 이름 포트의 트랙은 소리 내지 않는다. 포트가 돌아올 때의 재연결과, 열어 둔 OS 포트가 사라졌는지 확인(`is_present`)은 S2-T5c다.
- 메트로놈은 기본 설정(기본 가상 포트, 채널 10, 37·42)이다. 설정 화면은 S2-T6이다.
- 메시지는 창 안에만 쌓이고 아직 화면에 보이지 않는다(상태줄은 S2-T5c).
- 편집 경로가 문서를 바꾼 뒤 `cx.notify()`를 불러야 재생 중 반영(TRN-3)이 된다. 지금 문서를 바꾸는 UI 경로는 없고, 이후 편집 처리기는 이 약속을 지켜야 한다.

## 리뷰어에게

1. 실시간 경로는 건드리지 않았다. 확인할 곳은 제어 쪽 호출 순서다(`play_from`: stop → swap → 프로그램 send_now → 메트로놈 → play). 이 순서에서 프로그램 체인지가 첫 노트보다 늦게 나가거나, 앞 재생의 노트가 걸린 채 남는 경우가 있는지.
2. `transport::release`(창이 닫힐 때)는 닫히는 창의 약한 참조가 이미 끊겼다는 가정에 기댄다.
3. core `Op::SetPort`·`Op::SetKey`의 undo/redo가 모르는 키를 정확히 되돌리는지. 같은 값 편집, 가상 ↔ 이름 왕복, 조성 없음 왕복을 보라.

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 재생 중 템포를 바꾸면 일시정지 자리가 튄다 | 위치 기준점(`Anchor`: 호스트 시각, 틱, 그때의 템포 맵)을 둔다. 스냅샷을 교체할 때마다 옛 템포로 지금 자리를 구해 기준점을 다시 잡는다(스케줄러가 보낸 구간을 옛 템포로 두고 이어 가는 것과 같다) |
| 지적 2: 재생 중 프로그램·뱅크 변경이 송신되지 않는다 | 교체 때 새 스냅샷의 뱅크·프로그램 가운데 지난 것과 다른 것만 그 자리에서 보낸다 |
| 지적 3(차단): 재생 시작 `send_now`가 명령 큐를 넘친다 | 재생 시작 때는 보내지 않는다. 스케줄러가 시작 위치까지의 프로그램·뱅크를 따라잡아 보낸다(chase, S1-T4). 실패하면 스케줄러를 멈추고 창 상태를 정지로 맞춘다. 앞 주인 창은 결과와 무관하게 정지 상태로 바꾼다 |
| 제안 1: 창 닫힘이 엔티티 해제에 의존 | 한계로 둔다(실제 앱에서는 창이 닫히면 뷰가 해제된다. 테스트처럼 뷰를 붙잡는 코드는 없다). 저장 안 된 변경의 닫기 시트(S2-T7)를 붙일 때 창 닫기 훅으로 옮긴다 |
| 제안 2: 포트 문제 원인별 문구 | `PortProblem`(없음, 열기 실패, 상한)으로 나눴다 |
| 제안 3: 재생 중 편집마다 목적지 재열거 | 재생 시작 때 못 찾은 이름을 다음 재생 시작까지 기억하고, 교체 때는 다시 찾지 않는다 |
| 제안 4: 곡 끝에서 멈추지 않음 | 한계로 둔다. 곡 끝 처리는 루프·커서와 함께 S3·S4에서 정한다(대부분의 DAW는 곡 끝 뒤로 계속 간다) |
| 제안 5: 프로그램 순서 테스트가 `setup`을 검증하지 못함 | `setup` 송신을 없앴다. 일시정지 뒤 재개의 프로그램 순서는 리뷰 테스트가 chase로 확인한다 |
| 제안 6: 연 OS 포트 캐시가 닫히지 않음 | S2-T5c(포트 감시·재연결)에서 다룬다 |
| 제안 7: `Project::set_key`·`Track::set_port`가 `pub` | 이 Task 이전부터의 공개 API다. 히스토리를 거치지 않는 직접 수정은 이 Task의 범위 밖이다 |

부록 A의 테스트 4개는 `tests/review_s2_t5a.rs`로 옮겨 모두 통과한다.
