# Task 보고: S3-T8 일시정지 자리와 타이밍 시험 직렬화

- PR: 브랜치 `s3/t8-pause-position`
- 계획 파일: `docs/reports/s3-t8/plan.md`
- 계획 대비 달라진 점: 리뷰 1 뒤 `pause`는 `Option<Tick>` 대신 `PauseReply`(`At`, `NotPlaying`, `NoAnswer`)를 돌려준다. 시험 1의 이름과 내용은 계획대로다. 앱 시험 둘(`tests/pause_position.rs`)도 같은 바이너리 안에서 서로 직렬로 돈다(박 시각을 재기 때문). `Scheduler::pause`는 기다리는 동안 옛 스냅샷을 받아 준다(교체가 큐 머리를 막으면 정지까지 닿지 않으므로).

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
- 같은 틱의 이벤트는 넘기고 클릭·Clock만 남은 바퀴(항목 상한)에서 일시정지하면 이어서 재생할 때 그 틱의 클릭·Clock을 건너뛴다(이벤트의 겹치기와 반대, `pause` 문서에 적음).
- 한 틱의 이벤트가 바퀴당 항목 상한(4096)을 넘어 그 틱을 나눠 보내는 중에 일시정지하면 자리는 그 틱이고, 이어서 재생하면 그 틱에서 이미 넘긴 이벤트를 다시 넘긴다(리뷰 1 지적 1, 아래 대응).
- 기존 `stop`(정지 Action)도 큐가 가득이면 주인을 놓아 소리를 멈출 길이 없다. 이번 Task가 만든 길이 아니라 그대로 두었다(리뷰 1 제안 4).
- 루프 구간과의 관계는 S5(이어서 재생할 틱이 루프 끝과 같아지는 경우).
- `scheduler.rs` 바이너리는 직렬이 되어 약 20초 더 걸린다(지터·드리프트 측정 둘이 차례로 돈다).

## 리뷰어에게

- 실시간 스레드 쪽 변경은 `Command::Stop` 처리의 원자 store 둘뿐이다. 신호의 순서(틱 Relaxed → 번호 Release, 읽기는 번호 Acquire → 틱)와 「`&mut self`라 다른 정지가 끼지 않는다」는 전제를 봐 달라.
- `sent_through`가 항목 상한에 걸려 줄어든 경우, 교체·곡 끝과 겹친 경우에 이어서 재생할 틱이 맞는지.
- 시간 초과(`None`) 때 추정 자리로 돌아가는 길은 시험하지 않았다(스레드를 굶길 방법이 없다).
- 리뷰어는 `CARGO_TARGET_DIR=target/review`를 쓰고, MIDI·재생 시험을 주 세션의 전체 시험과 동시에 돌리지 않는다.

## 리뷰 대응

### 리뷰 1 (`review-1.md`, 수정 후 재리뷰)

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | 받아들임(동작 유지, 근거 기록). 상한에 걸린 틱을 나눠 보내는 중의 일시정지는 교체(`swap`)와 같은 「빠뜨리기보다 겹치기」를 따른다. 빠뜨리면 그 틱의 남은 Note On을 잃지만(정지가 울리는 노트를 모두 끄므로 Note Off는 문제가 아니다, 리뷰 2 제안 2로 고침), 겹치면 같은 틱의 Note On이 한 번 더 갈 뿐이다. 겹침 없이 하려면 이어서 재생할 자리를 틱이 아니라 「틱 + 그 틱 안의 이벤트 순번」으로 들고 `play`가 그것을 받아야 해서 공개 API와 앱의 `Paused(Tick)` 뜻이 바뀐다. 한 틱 4096개는 MIDI 1.0이 한 순간에 낼 수 없는 양이라 실제 곡에서 생기지 않는다고 본다. `pause` 문서와 「알려진 한계」에 적었다. 리뷰 시험 `pause_inside_capped_tick_does_not_resend_its_events`의 기대는 2회차 리뷰어가 이 규칙에 맞춰 달라 | 76ae330(문서) |
| 2 | 제안 | 받아들임. `pause` 문서에 적음 | 76ae330 |
| 3 | 제안 | 받아들임. `PauseReply`로 「재생 중이 아니었다」와 「답 없음」을 나눴다. 스케줄러가 이미 멈췄으면 앱은 정지 상태로 간다(곡 끝 정지 결정 A). 시험 `pause_position::pause_after_scheduler_reached_song_end_stops`(구현 전 `Paused(1623)`로 실패 확인) | 76ae330 |
| 4 | 제안 | 일시정지 쪽만 받아들임. 큐가 가득이면 주인을 놓지 않고 재생 상태로 남겨 알린다. 기존 `stop`은 범위 밖(「알려진 한계」). 스케줄러 쪽은 리뷰 시험 `pause_with_full_queue_errors_and_playback_continues`, 앱 쪽은 2회차 리뷰 시험 `review_s3_t8_r2::pause_with_full_queue_keeps_playing_and_a_later_pause_works`(`engine::send_port_test_note`로 큐를 채움)가 본다. 처음 보고서의 「앱 시험에서 큐를 채울 공개 길이 없다」는 틀렸다(리뷰 2 제안 4) | d7a6423 |

대응 뒤 시험(바이너리별): `keyscore-midi` `scheduler` 13 passed, `review_s3_t8` 5 passed·1 failed(지적 1 기대, 2회차에서 맞춤), `keyscore-app` `pause_position` 3 passed, `review_s3_t8` 1, `review_s2_t5a_r2` 13, `song_end` 6, `review_s3_t3d` 3, `engine` 8, `transport_bar` 6 모두 통과. fmt·clippy 통과.

### 리뷰 2 (`review-2.md`, 수정 후 재리뷰)

1회차 지적 1–4의 대응은 맞다고 판단했고, 1회차 지적 1 시험의 기대를 겹침 규칙(겹침은 상한에 걸린 그 틱 안에서만, 다른 틱은 정확히 한 번)에 맞췄다.

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | 받아들임. 재생 중이었나를 `Signals::paused_playing`(원자 bool)로 따로 적어 틱 `u64::MAX`와 표식이 겹치지 않게 했다. 리뷰 시험 `review_s3_t8_r2::pause_while_playing_at_max_tick_is_not_reported_as_not_playing` 통과 | 7adce20 |
| 2 | 제안 | 받아들임. 리뷰 1 대응 1행의 근거 문장을 고침 | 이 커밋 |
| 3 | 제안 | 받아들임. `pause` 문서와 「알려진 한계」에 클릭·Clock 예외를 적음 | 7adce20 |
| 4 | 제안 | 받아들임. 리뷰 1 대응 4행을 고침 | 이 커밋 |

### 리뷰 3 (`review-3.md`, 병합 가능)

지적 없음. 리뷰 시험 7개(빠른 재생·일시정지 반복, 루프 되감기, 다른 창 재생, 일시정지 중 편집, 가상 포트 복구) 모두 통과. 루프 끝과 같은 일시정지 자리는 S5 이월 줄로 넘겼다(`plan/stages/15-S5-sequence-and-play.md`).

병합 전 게이트(`2211243` + 리뷰 3 시험): 전체 시험 2824 passed·0 failed·2 ignored(리뷰 3 시험 전), 리뷰 3 시험 바이너리 둘은 기본 병렬로 각각 통과(앱 3회 반복). fmt·clippy·deny·check-deps 통과.
