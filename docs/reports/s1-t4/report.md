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

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

차단 1–12는 모두 같은 상태 기계(`Rt`)를 고치는 일이라 커밋 하나에 묶었다(`midi: Fix scheduler swap boundaries, clock resync, release and shutdown`). 지적마다 커밋 하나 규칙에서 벗어난 점을 여기 적는다.

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | drop 때 Note Off가 이미 넘긴 Note On보다 먼저, Clock Stop 없음 | 종료 때 마지막 예약 시각 뒤로 노트를 끄고, 재생 중이면 Clock Stop을 보낸다 |
| 2 | 패닉이 표를 비워 뒤에 도착하는 Note On이 남음 | 패닉 메시지를 지금 보낸 뒤, 울리는 노트를 마지막 예약 뒤로 한 번 더 끈다 |
| 3 | 교체 경계: 이어지는 노트가 길이 0, 이미 보낸 틱 재송신 | `scheduled_until`(마지막 창 끝)의 틱 T까지를 보낸 구간으로 보고 T 뒤부터 보낸다. 새 스냅샷에서 T 뒤 그 노트의 첫 사건이 Note Off면 노트를 끄지 않고 잇는다(스냅샷에 노트 칸별 색인을 미리 만든다). 새 이벤트는 이미 보낸 시각 앞으로 예약하지 않는다(`floor`) |
| 4 | 교체마다 기준점 오차 누적(200회에 49.7 ms) | 새 기준점을 옛 계산의 정확한 시각 (T, `host_of(T)`)으로 잡는다 |
| 5 | 교체 때 Clock 중복 | 다음 Clock은 T보다 큰 첫 칸 |
| 6 | 맞지 않는 위치의 SPP·첫 Clock 불일치 | SPP를 다음 16분음표로 올리고 첫 Clock을 거기에 둔다 |
| 7 | 재생 중 위치 이동·루프 되감기의 Clock 재동기화 | Stop → SPP → Continue(위치 0이면 Start)를 보낸다 |
| 8 | 큰 틱에서 넘침 패닉, 죽은 스레드에 `Ok` | Clock·시각 계산을 포화 산술로 바꾸고, 스레드가 끝났으면 명령이 `Os` 오류를 돌려준다 |
| 9 | 돌려받지 않은 교체가 큐 머리를 막아 패닉이 안 닿음 | 패닉은 명령 큐와 따로(원자 플래그) 전달한다. `swap` 문서에 `collect_garbage` 호출 의무와 막혔을 때의 동작을 적었다. `swap`이 직접 돌려받게 하는 안은 리뷰어의 할당 테스트(돌려받은 수를 셈)와 충돌해 쓰지 않았다 |
| 10 | chase가 데이터 엔트리를 RPN 선택보다 먼저 보냄 | 뱅크, 프로그램, 파라미터 선택(99·98·101·100), 데이터 엔트리(6·38), 나머지 CC 순서다. 상대값 CC(96·97)는 보내지 않는다 |
| 11 | 피치 벤드·채널 프레셔 chase 없음 | 둘 다 chase한다 |
| 12 | 종료 때 스냅샷·포트가 스케줄러 스레드에서 해제됨 | 종료 때 스냅샷(현재, 대기, 큐에 남은 교체)을 채널로 넘겨 `Scheduler::drop`이 제어 스레드에서 해제한다 |
| 13 | 교체마다 모든 지속음을 끊음 (제안) | 3과 함께 반영(이어지는 노트는 유지) |
| 14 | chase를 실시간 스레드에서 O(n)로 훑음 (제안) | 반영하지 않았다. 10만 이벤트 훑기는 1 ms 주기 안이다(재생 시작 때 한 번). 스냅샷에 미리 계산하는 일은 성능 문제가 보이면 한다 |
| 15 | `mach_wait_until` 예외의 결정 기록 (제안) | 결정 요청 `docs/decisions/2026-10-16-scheduler-wait-syscall.md`를 썼고, 사람이 A(시각 대기는 예외)를 골랐다 |
| 16 | NF-3·NF-4 측정 방식 (제안) | 표준편차와 타임스탬프 기준 드리프트도 출력한다(20초판: 표준편차 0.015–0.025 ms, 타임스탬프 드리프트 0) |
| 17 | 루프 끝 판정 규칙 (제안) | 규칙(기준점이 `end` 앞일 때만 되감음)을 `set_loop` 문서에 적었다 |
| 18 | 정지 상태 `stop`이 `send_now` 노트를 남김 (제안) | `stop`은 정지 중에도 울리는 노트를 끄고, `send_now`의 Note On도 센다 |

리뷰어 테스트는 바뀌지 않았다. 16개와 할당 테스트 모두 통과한다. 워크스페이스 테스트 628개 통과.

## 리뷰 대응 (review-2, 판정: 수정 후 재리뷰)

수정 1–8도 같은 상태(`Rt`의 기준점·보낸 구간·패닉 순서)를 고치는 일이라 커밋 하나에 묶었다(`midi: Keep swap boundaries exact and fix panic order, clock end and loops`).

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | 재생·위치 이동과 같은 바퀴의 교체가 재생 위치의 이벤트·Clock을 건너뜀 | `sent_through: Option<Tick>`을 둔다. 기준점을 새로 잡으면 `None`이고, 그때 교체는 `first_at_or_after(base_tick)`와 기존 `next_clock`을 쓴다 |
| 2 | 한 바퀴 두 번 교체가 빠른 템포에서 틱을 건너뜀 | `schedule`이 실제로 보낸 마지막 틱을 `sent_through`에 들고, 교체는 다시 계산하지 않고 그 값을 쓴다. 교체에 `now()`를 섞지 않는다 |
| 3 | 같은 번호의 다른 포트로 바꾸면 옛 포트에 노트가 걸림 | 번호마다 옛 포트와 새 포트가 같은 객체인지(`ptr::addr_eq`) 본다. 다르면 옛 포트로 끈다. 포트 수가 줄 때도 옛 포트로 끈 뒤 칸을 비운다 |
| 4 | 패닉이 그 전에 넣은 명령보다 먼저 처리됨 | `Scheduler`가 넣은 명령 수를 세고, 패닉 때 그 값(`panic_after`)을 함께 넘긴다. 스케줄러는 꺼낸 명령 수가 그 값에 닿거나 큐 머리가 교체로 막혔을 때 패닉을 적용한다 |
| 5 | 큰 틱에서 Clock을 같은 시각에 끝없이 보냄 | `clock_after`가 포화하면 `u64::MAX`를 「Clock 없음」으로 보고 더 보내지 않는다 |
| 6 | 호스트 틱 0 길이의 루프가 끝없이 되감음 | 되감은 시각을 기준 시각 + 1 호스트 틱 이상으로 두고, 한 `schedule`이 보내는 항목을 4096개로 묶는다 |
| 7 | chase가 마지막 선택이 NRPN이어도 RPN을 나중에 선택 | 마지막에 고른 쪽(RPN/NRPN)을 chase 표에 기억하고 그쪽을 데이터 엔트리 바로 앞에 보낸다 |
| 8 | 교체마다 기준 시각이 내림되어 누적 | 기준 시각을 `host × numer × 960`(`base_q: u128`)으로 정확히 들고, 교체는 그 값에서 정확히 옮긴다. 내림은 송신 시각을 낼 때만 한다 |
| 9 | 루프 되감기 때 chase 없음 (제안) | 반영하지 않았다. CTL-6은 재생 시작 때를 요구한다. 되감기 chase는 S4 루프 편집과 함께 본다 |
| 10 | RPN 널 패턴 (제안) | 반영하지 않았다. 파라미터별 값 표는 chase 표 크기를 바꾸는 일이라 S6(컨트롤러 레인)에서 본다 |
| 11 | 교체에 `now()`를 섞어 늦은 바퀴에서 버림 (제안) | 2와 함께 해소. 교체는 `sent_through`만 쓴다 |
| 12 | 노트 색인이 포트당 2048칸 (제안) | 반영하지 않았다. 32포트 512 KB는 한계 안이고, 돌려받지 않은 스냅샷이 쌓이는 경우는 `collect_garbage` 의무로 막는다. 메모리 문제가 보이면 정렬 표로 바꾼다 |

리뷰어 테스트(1·2회차)는 바뀌지 않았다. `keyscore-midi` 171개, 워크스페이스 642개 통과. `keyscore-midi` 테스트를 세 번 더 돌려 모두 통과했다.

## 리뷰 대응 (review-3, 판정: 수정 후 재리뷰)

수정 1–3을 커밋 하나에 묶었다(`midi: Release changed ports when stopped, order panic exactly, track sent ticks`). 셋 다 교체 상태를 함께 고친다.

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | 정지 중 교체에서 포트가 바뀌거나 줄면 미리듣기 노트가 옛 포트에 걸림 | 정지 중 교체도 번호마다 같은 포트 객체인지 보고(`same_port_at`), 아니면 옛 포트로 끈다. 같은 포트의 노트는 뒤따를 Note Off가 닿으므로 둔다 |
| 2 | 패닉 뒤에 넣은 명령까지 처리한 뒤 패닉해 그 노트가 꺼짐 | 명령을 하나 꺼내기 전마다 패닉 조건(`popped >= panic_after`)을 본다. 패닉은 앞 명령 바로 뒤, 뒤 명령 앞에서 처리된다. 큐 머리가 교체로 막힌 때는 그대로 바로 처리한다 |
| 3 | 항목 상한에 걸린 바퀴가 남은 틱을 보낸 것으로 침 | `sent_through`를 아직 보내지 않은 다음 항목(`peek_next`)의 틱과 송신 판정과 같은 내림(`host_of(tick) <= horizon`)으로 구한다. 상한에 걸린 틱은 교체 때 다시 보낸다(빠뜨리기보다 겹치기). 교체는 `next_clock`을 다시 계산하지 않는다(보낸 구간 바로 뒤의 절대 틱이다) |
| 4 | 패닉 둘이 경합하면 합쳐짐 (제안) | 반영하지 않았다. 두 번째 패닉이 첫 패닉 뒤 명령까지 기다릴 수 있으나 패닉은 한 번 반드시 간다. 패닉 메시지는 멱등이라 결과는 같다 |
| 5 | 데이터 엔트리 값이 파라미터별이 아님 (제안) | 리뷰 2 제안 10과 함께 S6에서 본다 |
| 6 | `sent_through`와 송신 판정의 내림 어긋남 (제안) | 3과 함께 해소 |

리뷰어 테스트(1–3회차)는 바뀌지 않았다. 리뷰 3의 실패 4개가 통과한다. `keyscore-midi` 테스트를 세 번 더 돌려 모두 통과했고, 워크스페이스 655개 통과.

## 리뷰 대응 (review-4, 판정: 병합 가능)

지적은 제안 셋뿐이다. 셋 다 반영하지 않고 한계로 남긴다. 앱이 스케줄러를 붙이는 S2-T5와 S4 재생 편집에서 다시 본다.

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | 상한에 걸려 Clock이 남은 바퀴 직후 재생 중 `set_clock(on)`이면 Clock 하나가 빠짐 | 한계. 한 창(10 ms)에 항목이 4096개를 넘을 때만 생긴다 |
| 2 | 상한에 걸린 바퀴 뒤 교체하면 남은 항목이 최대 10 ms 늦음(`scheduled_until`을 창 끝까지 올림) | 한계. 빠지거나 겹치지는 않는다. 상한에 걸린 바퀴만의 일이다 |
| 3 | 교체로 큐가 막힌 동안의 패닉은 막힌 교체 뒤의 Note On보다 먼저 나감 | 한계. 리뷰 1 지적 9에서 정한 동작(패닉은 막힘을 기다리지 않는다)의 결과다. `collect_garbage`를 부르는 호출자에서는 생기지 않는다 |
