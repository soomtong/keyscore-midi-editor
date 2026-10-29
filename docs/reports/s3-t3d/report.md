# Task 보고: S3-T3d 곡 끝 정지

- PR: 브랜치 `s3/t3d-end-of-song`
- 계획 파일: docs/reports/s3-t3d/plan.md
- 결정: `docs/decisions/2026-10-25-playback-end-of-song.md` A(곡 끝에서 정지·틱 0·메트로놈 멈춤, Sequence 모드 예외)
- 계획 대비 달라진 점
  - 앱 시험은 계획의 10–14를 여섯 개로 묶었다. 「곡 끝 뒤 커서부터 재생」은 시험 전용 API 없이 닿는 길(Sequence에서 곡 끝을 지나 일시정지 → Normal → 재개)로 시험한다(`resume_after_song_end_stops_at_once`). 「Sequence에서 나오면 멈춤」(12)은 `sequence_mode_keeps_playing_past_song_end` 뒤쪽에 넣었다.
  - 공개 접근자 `ProjectWindow::transport_position`을 더했다(시험이 「위치 틱 0」을 본다).

## 구현 요약

- `crates/keyscore-midi/src/scheduler.rs`
  - `Snapshot::with_end(Option<Tick>)`·`end()`: 곡 끝 틱.
  - 실시간 루프의 다음 항목에 `Next::SongEnd`. 같은 틱 순서는 이벤트 > 곡 끝 > 클릭·Clock(곡 끝 틱의 클릭·Clock은 보내지 않는다). 곡 끝이 기준점 틱 이하(곡 끝 뒤에서 시작, 지난 곡 끝으로 교체)면 무엇보다 먼저. 처리는 정지 명령과 같은 정리(`release_notes`, Clock Stop)를 곡 끝 호스트 시각에 하고 `playing = false`.
  - `Signals::ended`(원자값, 그때까지 처리한 명령 수)와 `Scheduler::take_ended`. `play`·`locate`가 `last_start`를 적어, 그 앞의 곡 끝 정지는 보고하지 않는다.
  - 루프 중에는 곡 끝을 무시한다(S5).
- `crates/keyscore-app/src/engine.rs`: `song_end(&Project)`(가장 늦은 `Clip::end`, 없으면 0), `END_POLL`(20 ms), `prepare`에 `stop_at_end`.
- `crates/keyscore-app/src/transport.rs`: 재생 시작 때 곡 끝 감시 작업(`watch_song_end`), 주기마다 `check_song_end`가 주인이고 `take_ended`면 주인을 놓고 `mark_stopped`. Sequence 모드면 곡 끝을 넣지 않는다(`stops_at_song_end`).
- `crates/keyscore-app/src/window.rs`: `apply_mode_event`가 Sequence 진입·이탈 때 `transport_refresh`.
- `crates/keyscore-app/src/preview.rs`: 정지 중 미리듣기 준비는 곡 끝을 넣지 않는다.

## 테스트 결과

~~~
keyscore-midi --test song_end: 8 passed
keyscore-app --test song_end: 6 passed
재생을 쓰는 keyscore-app 시험 바이너리 37개(--no-fail-fast): 실패 없음
전체 cargo test --workspace: 리뷰 뒤 병합 전에 돌린다(아래 「자동 게이트」)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `song_end_stops_playback_and_sends_offs` | 통과 |
| 2 | `song_end_does_not_click_on_end_tick` | 통과 |
| 3 | `play_from_at_or_after_end_stops_at_once` | 통과 |
| 4 | `end_none_keeps_playing` | 통과 |
| 5 | `swap_extending_end_keeps_playing` | 통과 |
| 6 | `swap_to_end_before_position_stops_at_once` | 통과 |
| 7 | `take_ended_is_false_after_new_play` | 통과 |
| 8 | `song_end_in_loop_is_ignored` | 통과 |
| 9 | `song_end_is_latest_clip_end` | 통과 |
| 10 | `playback_stops_at_song_end_and_rewinds` | 통과 |
| 11, 12 | `sequence_mode_keeps_playing_past_song_end` | 통과 |
| 13 | `edit_extending_song_while_playing_plays_to_new_end` | 통과 |
| 14 | `space_after_song_end_plays_from_start` | 통과 |
| (더함) | `resume_after_song_end_stops_at_once` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [ ] test(전체): 리뷰어 작업이 끝난 뒤 `--no-fail-fast`로 돌린다(타이밍 시험이 겹치면 흔들린다)
- [x] deny
- [x] 단계별 추가 게이트: 루프백으로 곡 끝 뒤 정지·틱 0·메트로놈 멈춤, Sequence 모드 예외(위 시험). `scripts/check-deps.sh`·`check-deps.test.sh`·`bench-check.test.sh` 통과

## 알려진 한계

- 창은 곡 끝 정지를 최대 `END_POLL`(20 ms) 늦게 안다. 그 사이 창 상태는 「재생 중」이고 재생 헤드는 곡 끝을 조금 지나 그려질 수 있다. 소리는 스케줄러가 정확히 멈춘다.
- 곡 끝 직전 편집으로 곡이 길어졌는데 교체가 스케줄러에 닿기 전에 옛 곡 끝을 지나면 멈춘다. 교체는 정지를 되돌리지 않는다.
- 그 틈에 일시정지하면 일시정지 자리가 곡 끝 뒤일 수 있다. 재개하면 곧바로 멈춘다(정지 위치 틱 0).
- 곡 끝 처리에서 곡 끝 틱의 이벤트를 보낸 뒤 교체가 오면 그 틱의 Note Off가 한 번 더 나갈 수 있다(`sent_through_now`, 빠뜨리기보다 겹치기).
- 루프 구간과 곡 끝의 관계는 S5.
- 곡 끝 직전(스케줄러 lookahead 10 ms와 창 지연 안)에 Sequence로 들어가면 교체가 닿기 전에 멈출 수 있다(리뷰 1 제안 2).
- Sequence 진입·이탈은 스냅샷 전체 교체라, 재생 중 편집과 같이 그 틈에 박 하나·Clock 한 칸이 빠질 수 있다(리뷰 1 제안 2).
- `Scheduler::locate`는 정지 중이면 무시되는데 `last_start`는 올린다. 곡 끝 정지 뒤 창이 아직 「재생 중」인 틈에 위치 이동을 부르면 창이 재생 중으로 남는다. 지금 앱은 `locate`를 부르지 않는다. 위치 이동 Task(S5)로 넘긴다(리뷰 1 제안 3).

## 리뷰어에게

- 결정 문서의 추천 구현(창이 보간으로 보고 `stop`)과 달리 스케줄러가 멈춘다. 이유는 계획 2절.
- `peek_next`의 같은 틱 순서와 곡 끝 조기 반환, `sent_through_now`에서 `SongEnd`를 이벤트처럼 다룬 것이 교체·Clock·박 보고와 맞는지.
- `take_ended`의 번호 비교(`popped` 대 `pushed`)가 패닉·교체 막힘(`blocked`) 경로에서도 맞는지.
- 감시 작업은 `end_watch`에 들고, 창이 재생 중이 아니면 스스로 끝난다. 정지·다른 창 재생 뒤 새 재생이 앞 작업을 바꿔 놓는다.

## 리뷰 대응

### 리뷰 1(판정 「병합 가능」, 제안 4)

| # | 대응 |
| --- | --- |
| 1 | 받음. 곡 끝 이후에서 시작하면 클럭 맞추기(Start·SPP·Continue)를 건너뛰고 곡 끝 정리의 Stop 하나만 보낸다(`start_playing`). 시험 `play_past_end_sends_only_stop_to_clock_devices` |
| 2 | 받음. 「알려진 한계」에 두 줄 |
| 3 | 받음. 「알려진 한계」와 단계 파일 S5 이월 |
| 4 | 받음. 리뷰어 시험 `crates/keyscore-midi/tests/review_s3_t3d.rs`, `crates/keyscore-app/tests/review_s3_t3d.rs`를 커밋 |

### 리뷰 2(커밋 88d90c7만 좁게, 판정 「수정 후 재리뷰」)

| # | 대응 |
| --- | --- |
| 1(수정) | 받음. 곡 끝 이후 시작의 클럭 맞추기를 건너뛰지 않고 미룬다(`Rt::clock_resync`). `schedule`이 그 바퀴의 루프·곡 끝으로 다시 보아 곧바로 곡 끝이 아니면 그때 `resync_clock`을 하고, 곡 끝 정리를 실제로 처리할 때만 버린다. 처음 고친 판은 곡 끝 정리가 시작 여유(`START_MARGIN`) 때문에 다음 바퀴로 밀리면 대기를 먼저 버려 리뷰어 시험이 네 번 중 한 번 실패했다. 고친 뒤 `review_s3_t3d_r2` 15회 연속 통과 |
| 제안 1 | 받음. 리뷰어 시험 `crates/keyscore-midi/tests/review_s3_t3d_r2.rs` 커밋 |
| 제안 2 | 받음. 리뷰 1 대응 1의 보충: 재생 중이었으면 맞추기가 보내던 앞 Stop도 함께 미루므로 곡 끝 뒤로 `play`·`locate`하면 Stop은 곡 끝 정리의 한 번이다 |

### 리뷰 3(커밋 6dfcc08만 좁게, 판정 「병합 가능」, 제안 3)

| # | 대응 |
| --- | --- |
| 1 | 받음(기록). `review_s3_t3d_r2::play_past_end_then_swap_extending_end_in_same_wake_resyncs_clock`는 드물게 실패할 수 있다(리뷰어 실행 75회 중 1회). 원인은 시험 하네스로 본다: `play` 앞 바퀴에 이미 넘긴 옛 Clock이 drain 뒤 Stop보다 먼저 도착하면 「첫 0xF8 앞 구간」이 빈다. 같은 시나리오는 옛 Clock을 떼고 보는 `review_s3_t3d_r3::play_past_end_then_swap_while_playing_resyncs_repeatedly`가 고정한다 |
| 2 | 거절. `Command::Stop`에서 `clock_resync`를 비우지 않아도 다음 `play`·`locate`가 덮어쓰고 정지 중에는 `schedule`이 돌지 않는다(리뷰 3도 해가 없다고 확인). 마지막 리뷰 뒤 코드를 바꾸지 않는다 |
| 3 | 받음. `crates/keyscore-midi/tests/review_s3_t3d_r3.rs` 커밋 |
