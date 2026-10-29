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

## 리뷰어에게

- 결정 문서의 추천 구현(창이 보간으로 보고 `stop`)과 달리 스케줄러가 멈춘다. 이유는 계획 2절.
- `peek_next`의 같은 틱 순서와 곡 끝 조기 반환, `sent_through_now`에서 `SongEnd`를 이벤트처럼 다룬 것이 교체·Clock·박 보고와 맞는지.
- `take_ended`의 번호 비교(`popped` 대 `pushed`)가 패닉·교체 막힘(`blocked`) 경로에서도 맞는지.
- 감시 작업은 `end_watch`에 들고, 창이 재생 중이 아니면 스스로 끝난다. 정지·다른 창 재생 뒤 새 재생이 앞 작업을 바꿔 놓는다.
