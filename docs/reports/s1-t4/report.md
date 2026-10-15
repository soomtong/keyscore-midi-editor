# Task 보고: S1-T4 스케줄러 스레드

- PR: 브랜치 `s1/t4-scheduler`
- 계획 파일: `docs/reports/s1-t4/plan.md`
- 구현: 주 세션(실시간 경로)
- 계획 대비 달라진 점:
  - `assert_no_alloc`의 라이선스(BSD-1-Clause)가 `deny.toml` 허용 목록에 없었다. 실패 테스트 커밋에서 `cargo deny`가 실패했고, 다음 커밋에서 BSD-1-Clause를 허용 목록에 더했다. REQ 8.4가 허용하는 「BSD」 계열이고 BSD-2보다 조건이 적다.
  - 정렬은 (틱, 무리)로 하는 안정 정렬이다. 같은 무리 안에서는 주어진 순서를 지킨다(뱅크 MSB 뒤 LSB).
  - 크레이트 안에서만 쓰는 `Midi1Message::from_status`를 더했다. chase·노트 끄기·클록에 쓴다.
  - 공개 API `HostTime::duration_since`를 더했다. 테스트가 호스트 틱을 정확히 ms로 바꾸는 데 필요했다.
  - 테스트 수정(구현 중)
    - CoreMIDI는 같은 타임스탬프의 메시지를 한 패킷으로 묶어 준다. 그래서 수신기가 패킷을 메시지로 나누게 했다.
    - 처음 테스트의 노트 간격 기대(30 ms)가 틀렸다. 120 BPM에서 60틱은 31.25 ms다.
    - 50 ms로 잰 틱 비율은 20초에 수 ms 드리프트를 만들어 정확한 변환으로 바꿨다.

## 구현 요약

| 파일 | 내용 |
| --- | --- |
| `src/scheduler.rs` | `Snapshot`(정렬, 포트 ≤ 32), `Scheduler`(명령 `rtrb` 큐, 돌려받기 큐, drop 때 종료), 스케줄러 상태 `Rt`(미리 할당한 울리는 노트 표·chase 표), 기준점 기반 정수 시각 계산(`nanos_between`, `nanos_to_host_ticks`) |
| `src/rt.rs` | `mach_timebase_info`, `mach_wait_until`, `thread_policy_set`(time-constraint, 주기 1 ms, 계산 0.25 ms) |
| `src/time.rs` | `HostTime::duration_since` |
| `src/encode.rs` | `Midi1Message::from_status`(crate 안) |

스케줄러 루프 한 바퀴의 흐름은 이렇다.
1. 돌려받기 대기 자리 비우기
2. 명령 처리(최대 64개). 교체는 옛 스냅샷을 둘 자리가 있을 때만 꺼낸다.
3. 재생 중이면 지금 + 10 ms 안의 이벤트, 클록, 루프 끝을 시각 순으로 예약 송신한다.
4. 1 ms 뒤까지 `mach_wait_until`로 기다린다.

디버그 빌드에서는 한 바퀴를 `assert_no_alloc`으로 감싼다.

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 611 passed, 0 failed
NF-1 명령 투입 → 도착 p99: 약 1.09 ms (200회, 기준 ≤ 3 ms)
NF-3 간격 편차 p99(20초판): 0.035–0.139 ms (기준 ≤ 0.5 ms)
NF-4 드리프트(20초판): 0.004–0.065 ms (기준 1시간 ≤ 1 ms)
10분판(KS_LONG_TESTS=1, 노트 4800개): NF-3 간격 편차 p99 0.077 ms, NF-4 드리프트 0.011–0.046 ms
~~~

| 계획 # | 실제 이름 | 상태 |
| --- | --- | --- |
| 1 | `snapshot_orders_note_off_before_note_on_at_same_tick` | 통과 |
| 2 | `tick_to_host_is_integer_exact_across_tempo_changes` | 통과 |
| 3 | `play_sends_events_at_scheduled_host_times` | 통과 |
| 4 | `send_now_latency_p99_within_3ms` | 통과 |
| 5 | `playback_jitter_p99_within_half_ms_short` | 통과 |
| 6 | `playback_drift_within_1ms_short` | 통과 |
| 7 | `swap_during_play_stops_old_events_and_releases_notes` | 통과 |
| 8 | `stop_locate_and_loop_release_sounding_notes` | 통과 |
| 9 | `play_from_middle_chases_program_bank_and_cc` | 통과 |
| 10 | `panic_sends_all_sound_off_notes_off_sustain_to_every_port_channel` | 통과 |
| 11 | `clock_sends_24_ppqn_with_start_spp_continue_stop` | 통과 |
| 12 | `scheduler_loop_does_not_allocate` | 통과. 루프에 `vec!` 하나를 넣은 변이에서 SIGABRT로 실패함을 확인했다 |
| 13 | `full_command_queue_returns_queue_full` | 통과 |
| 14 | `garbage_snapshots_are_returned_not_dropped_on_scheduler_thread` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (611 passed)
- [x] deny(BSD-1-Clause 추가 뒤)
- [x] 단계별 추가 게이트
  - `check-deps` 셋 통과
  - 루프백 NF-1·NF-3·NF-4 수치 위
  - 스케줄러 할당 0(테스트 12)

## 알려진 한계

- 교체·위치 이동 때, 이미 CoreMIDI에 넘긴 lookahead 구간(최대 10 ms)의 옛 이벤트는 그대로 울린다. 가상 소스로 보낸 예약은 취소할 수 없다.
  - 그래서 노트 끄기는 예약한 마지막 시각 뒤로 둔다.
  - 새 스냅샷은 그 구간 뒤의 이벤트부터 보낸다. 그 구간 안에 새로 생긴 이벤트는 빠질 수 있다(최대 10 ms).
- 루프 되감기 때 chase는 하지 않는다. 같은 재생 흐름이라 값이 이어진다.
- 우선순위 설정이 실패해도 동작은 같고, 지터만 커질 수 있다. 실패를 따로 알리지 않는다.
- 1시간판(NF-3·NF-4)은 야간 CI 몫이다. remote가 없어 아직 돌리지 않았다.
- 모델(트랙·클립·뮤트·솔로)을 이벤트로 펼치는 변환은 S1-T7(`ksq-cli play`)과 S2·S4에서 한다.

## 리뷰어에게

스스로 불안한 곳:

1. `swap`의 위치 계산: `tick_at`(옛 템포)으로 `now_tick`·`sent_tick`을 구한 뒤, 새 템포로 `set_base(now_tick, now)`를 한다. 새 스냅샷의 템포가 다르면 기준점 계산이 맞는지 봐 달라.
2. `schedule`의 다음 항목 고르기
   - 같은 틱이면 이벤트가 클록·루프 끝보다 먼저다.
   - 루프 끝과 같은 틱의 이벤트는 다음 바퀴로 넘긴다(`tick < end`).
3. `release_notes`
   - 매번 32 × 16 × 128 칸을 훑는다.
   - 포트 수가 줄어든 스냅샷으로 바뀌면 남은 칸을 비우기만 한다(보낼 포트가 없다).
4. `wait_until` 사이에 명령이 들어오면 최대 1 ms 늦게 처리된다. NF-1 수치에 이 지연이 들어 있다.
