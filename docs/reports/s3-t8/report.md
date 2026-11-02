# Task 보고: S3-T8 일시정지 자리와 타이밍 시험 직렬화

- PR: 브랜치 `s3/t8-pause-position`
- 계획 파일: `docs/reports/s3-t8/plan.md`
- 계획 대비 달라진 점: 시험 1의 이름과 내용은 계획대로다. 앱 시험 둘(`tests/pause_position.rs`)도 같은 바이너리 안에서 서로 직렬로 돈다(박 시각을 재기 때문). `Scheduler::pause`는 기다리는 동안 옛 스냅샷을 받아 준다(교체가 큐 머리를 막으면 정지까지 닿지 않으므로).

## 원인

스케줄러는 lookahead(10 ms, 120 BPM에서 약 19틱) 안의 노트를 OS에 미리 넘기고, 넘긴 예약은 취소하지 않는다. 앱의 일시정지 자리는 앱이 든 기준점으로 구한 「지금」의 틱이었다. 그래서 박 직전 19틱 안에서 일시정지하면 박은 들리는데 자리는 박 앞이고, 이어서 재생하면 그 박이 한 번 더 울렸다. `review_s2_t5a_r2::pause_position_does_not_drift_with_many_live_edits`는 부하와 무관하게 단독 25회 중 1회 실패했다(약 19/480). 새 시험은 구현 전 코드에서 결정적으로 실패했다: 자리 959 대 들린 박 960, 이어 재생에서 음 41·44·49가 두 번 울림.

## 구현 요약

- `crates/keyscore-midi/src/scheduler.rs`: `Scheduler::pause(wait) -> Result<Option<Tick>, MidiError>`. 정지 명령을 넣고, 스레드가 그 정지를 처리하며 적은 「이어서 재생할 틱」을 `wait`(앱은 50 ms)까지 기다린다. 스레드는 `Command::Stop`을 처리할 때 정리 전에 `sent_through + 1`(보낸 것이 없으면 기준점 틱, 재생 중이 아니면 `NOT_PLAYING`)을 `Signals::paused_tick`에 쓰고 명령 번호를 `Signals::paused`에 Release로 쓴다. 원자 store 둘뿐이다. 규칙은 교체(`swap`)가 보낸 구간 뒤부터 이어 보내는 것과 같다.
- `crates/keyscore-app/src/transport.rs`: `transport_pause`가 이 창이 엔진 주인이면 `pause(PAUSE_WAIT)`의 틱을 일시정지 자리로 쓴다. `None`(재생 중이 아니었거나 시간 초과)이면 지금처럼 추정 자리.
- `crates/keyscore-midi/tests/scheduler.rs`: 모든 시험이 정적 `Mutex`로 서로 직렬로 돈다(독 무시). 20초 지터·드리프트 측정이 다른 루프백 시험과 겹치던 흔들림을 없앤다. 일시정지 시험 셋을 더했다.
- `crates/keyscore-app/tests/pause_position.rs`: 앱 경로 시험 둘.

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 2811 passed, 0 failed, 2 ignored(1시간판 측정용)
scheduler.rs 새 시험 셋: 3 passed(직렬, 약 9 s)
pause_position.rs: 2 passed(약 9 s)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `scheduler::pause_returns_first_unsent_tick_after_lookahead` | 통과 |
| 2 | `scheduler::pause_resume_neither_repeats_nor_drops_notes` | 통과 |
| 3 | `scheduler::pause_when_stopped_returns_none` | 통과 |
| 4 | `pause_position::pause_position_never_precedes_heard_beat` | 통과(구현 전 실패 확인) |
| 5 | `pause_position::resume_after_pause_does_not_repeat_heard_note` | 통과(구현 전 실패 확인) |
| 6 | `review_s2_t5a_r2` 13개 그대로 | 통과. `pause_position_does_not_drift_with_many_live_edits` 단독 25회 모두 통과(전에는 25회 중 1회 실패) |

일시정지와 관련된 다른 앱 시험 바이너리(`review_s2_t5a`, `review_s2_t5a_r3`, `engine`, `transport_bar`, `song_end`, `review_s3_t3d`, `review_s2_t5d`)도 단독으로 모두 통과.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings: 경고 0
- [x] test: 위
- [x] deny: advisories·bans·licenses·sources ok
- [x] `scripts/check-deps.sh` ok, `scripts/check-deps.test.sh` 16 passed, `scripts/bench-check.test.sh` 8 passed

## 알려진 한계

- 일시정지를 누른 뒤 lookahead(10 ms) 안의 노트는 지금처럼 울린다(넘긴 예약을 OS에서 지우지 않음, 계획 「범위 밖」).
- 일시정지 자리는 화면의 추정 재생 헤드보다 최대 lookahead만큼 뒤일 수 있다(약 19틱).
- 같은 틱의 이벤트는 넘기고 클릭만 남은 바퀴(항목 상한)에서 일시정지하면 이어서 재생할 때 그 클릭 하나를 건너뛴다.
- 루프 구간과의 관계는 S5(이어서 재생할 틱이 루프 끝과 같아지는 경우).
- `scheduler.rs` 바이너리는 직렬이 되어 약 20초 더 걸린다(지터·드리프트 측정 둘이 차례로 돈다).

## 리뷰어에게

- 실시간 스레드 쪽 변경은 `Command::Stop` 처리의 원자 store 둘뿐이다. 신호의 순서(틱 Relaxed → 번호 Release, 읽기는 번호 Acquire → 틱)와 「`&mut self`라 다른 정지가 끼지 않는다」는 전제를 봐 달라.
- `sent_through`가 항목 상한에 걸려 줄어든 경우, 교체·곡 끝과 겹친 경우에 이어서 재생할 틱이 맞는지.
- 시간 초과(`None`) 때 추정 자리로 돌아가는 길은 시험하지 않았다(스레드를 굶길 방법이 없다).
- 리뷰어는 `CARGO_TARGET_DIR=target/review`를 쓰고, MIDI·재생 시험을 주 세션의 전체 시험과 동시에 돌리지 않는다.
