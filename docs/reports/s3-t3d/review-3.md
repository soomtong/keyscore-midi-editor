# 적대적 리뷰: S3-T3d 곡 끝 정지 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 컨텍스트를 물려받지 않은 새 리뷰 에이전트)
- 대상 커밋: `6dfcc08`(`crates/keyscore-midi/src/scheduler.rs`: `Rt::clock_resync`, `past_song_end`, `start_playing`·`schedule`·`Next::SongEnd`). 배경은 리뷰 2 지적 1과 보고서 「리뷰 2」 대응
- 범위: 이 커밋이 만든 회귀·결함과 리뷰 2 지적 1의 해결 여부만 「수정」「차단」으로 친다. 그 밖은 「제안」

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 리뷰 2 시도 6·7(곡 끝 뒤 `play` 직후 같은 바퀴의 곡 끝 늘리는 `swap`, `set_loop`)이 이제 맞추기(SPP·Continue, 재생 중이었으면 앞 Stop)를 받는가.
2. 미룬 맞추기의 시각: `schedule`은 `start_playing`이 잡은 `host`를 그대로 쓴다. 대기가 다음 바퀴로 넘어간 뒤(lookahead가 시작 여유 2 ms보다 짧을 때) 교체가 들어오면 `host`가 `floor`·`scheduled_until`·`last_sent`보다 앞인가. SPP·Continue가 첫 Clock·이벤트보다 늦게 나가지 않는가.
3. 대기 중 `next_clock = u64::MAX`인 동안 Clock(0xF8)이 나가지 않는가. 대기 중 `set_clock(on)`은 `next_clock`을 다시 잡는데, 그래도 곡 끝 정리가 먼저인가.
4. 대기 중 `set_clock(on)`(시작 때 마스크 0)과 곡 끝 늘리는 교체: 맞추기가 새 마스크로 나가는가.
5. 대기 중 `Stop`: Stop 하나만, 남은 `clock_resync`가 다음 재생에 새지 않는가.
6. 대기 중 `Play`·`Locate`: 새 시작이 대기를 덮어쓰는가(Start 또는 SPP·Continue, 옛 대기의 맞추기가 겹쳐 나가지 않는가).
7. 교체가 곡 끝을 늘렸다가 같은 바퀴에서 다시 지난 곳으로 줄이는 경우: Stop만.
8. 실시간 경로 할당·락·로그.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 | 있음(`review_s3_t3d_r2.rs` 6·7) | 이번 커밋으로 통과. 반복 시험 추가 |
| 2 | 없음 | 리뷰어 시험 추가(lookahead 1 ms) |
| 3 | 부분(`review_s3_t3d_r2.rs` 1·2) | 바퀴를 넘기는 대기, `set_clock` 경우 추가 |
| 4–7 | 없음 | 리뷰어 시험 추가 |
| 8 | 디버그 빌드 `assert_no_alloc` | 루프백 시험에서 걸렸다 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-midi/tests/review_s3_t3d_r3.rs`. 실행은 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-midi --test review_s3_t3d_r3 --test review_s3_t3d_r2 --test song_end --test review_s3_t3d`를 20번(그 밖에 `review_s3_t3d_r2` 단독 55번). `review_s3_t3d_r3` 9개는 20번 모두 통과했다. `review_s3_t3d_r2`는 첫 번째 묶음 실행에서 한 번 `play_past_end_then_swap_extending_end_in_same_wake_resyncs_clock`가 실패하고 나머지 74번은 통과했다(아래 시도 9와 지적 1).

| # | 시도 | 결과 | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 정지 상태, 곡 끝 960, `play(1920)` 직후 `stop`, 이어서 `play(0)` | 못 깨뜨림. 첫 단계는 Stop 하나. 다음 재생은 Start로 시작하고 SPP·Continue 없음, Stop 하나 | `stop_while_resync_deferred_sends_single_stop_and_next_play_starts_clean` |
| 2 | 마스크 0으로 `play(1920)` 직후 `set_clock(0, true)`(교체 없음) | 못 깨뜨림. 다시 잡힌 `next_clock`에도 곡 끝 정리가 먼저라 Clock 없음(Stop 하나 또는 없음) | `set_clock_on_while_deferred_without_swap_sends_only_stop` |
| 3 | 2에 이어 같은 바퀴에 곡 끝 96000 교체 | 못 깨뜨림. 새 마스크로 SPP·Continue 뒤 Clock | `set_clock_on_while_deferred_then_swap_extending_resyncs` |
| 4 | `play(1920)` 직후 곡 끝 96000 교체, 다시 곡 끝 960 교체 | 못 깨뜨림. 곡 끝 정리가 오고 Stop만(맞추기 없음) | `swap_extending_then_shortening_in_same_wake_sends_only_stop` |
| 5 | 곡 끝 960으로 교체 직후 `play(1920)`, `play(0)` | 못 깨뜨림. Start 뒤 Clock, SPP·Continue 없음 | `play_past_end_then_play_zero_in_same_wake_starts_without_spp` |
| 6 | `play(1920)` 직후 `locate(480)`(곡 끝 960 앞) | 못 깨뜨림. 같은 바퀴면 Stop·SPP·Continue 뒤 Clock, 곡 끝에서 Stop | `locate_before_end_while_deferred_resyncs` |
| 7 | lookahead 1 ms·wake 1 ms(시작 여유 2 ms보다 짧아 대기가 바퀴를 넘긴다). `play(1920)` 뒤 0–2 ms 여러 간격으로 곡 끝 96000 교체 | 못 깨뜨림. 회차마다 8번 중 4–8번이 「곡 끝 정리 전에 교체가 닿은」 경로였고, 모두 SPP·Continue가 첫 Clock 앞. 미룬 `host`는 `floor`와 같고 교체는 `floor`를 `scheduled_until`(< `host`, 아니면 곡 끝 정리가 먼저 나갔다) 이상으로만 올리므로 순서가 맞다 | `deferred_across_wakes_then_swap_extending_resyncs` |
| 8 | lookahead 1 ms, 재생 중 곡 끝 960으로 교체 뒤 `play(1920)` | 못 깨뜨림. 대기가 바퀴를 넘겨도 Clock이 끼지 않고 Stop 하나가 마지막 | `deferred_across_wakes_while_playing_sends_exactly_one_stop` |
| 9 | 리뷰 2 시도 6을 재생 위상을 바꿔 12번 반복(`play` 호출 시각 뒤 도착분만) | 못 깨뜨림(12/12 같은 바퀴 경로). 다만 회차의 약 1/3에서 `play` 앞 바퀴에 이미 넘긴 옛 Clock 하나가 `play` 뒤, Stop 앞에 도착했다(t0 뒤 최대 약 8 ms, lookahead 안). 넘긴 예약은 취소할 수 없으므로 정상이다. 이 옛 Clock을 떼면 Stop·SPP·Continue 순서가 늘 맞다 | `play_past_end_then_swap_while_playing_resyncs_repeatedly` |

시도 9의 옛 Clock이 리뷰 2 시험 `assert_resynced`의 드문 실패 원인으로 보인다. 그 시험은 첫 0xF8 앞에서 SPP·Continue를 찾는데, `playing_with_clock`의 drain이 끝난 뒤 `play` 전에 이미 넘긴 옛 Clock이 도착하면 첫 0xF8이 옛 것이 되어 앞 구간이 비고 실패한다. 실패 메시지를 다시 잡지 못해(55번 단독 재실행, 19번 묶음 재실행 모두 통과) 단정은 못 하지만, 같은 모양의 실패를 시도 9에서 결정적으로 재현했고(옛 Clock을 떼기 전 판은 네 번 모두 `[F8, FC, F2, FB, F8 ...]`로 실패) 스케줄러 쪽 순서 결함은 찾지 못했다.

## 4. 코드 검토

- `past_song_end`로 `start_playing`의 판정과 `peek_next`의 즉시 곡 끝 판정을 한 함수로 묶었다. 두 판정이 갈라질 수 없다.
- 대기 중 `sent_through_now`는 `None`이다(`horizon < base_host`이거나, `peek_next`가 기준점 틱의 `SongEnd`라 `pending?`가 `None`). 그래서 대기 중 교체는 `move_base`를 하지 않고, 맞추기는 `base_tick = from`에서 한다.
- `Clock(on)`은 대기 중 `next_clock`을 유한 값으로 다시 잡지만, 곡 끝 정리가 늘 먼저이고 대기가 풀리면 `resync_clock`이 다시 덮어쓰므로 Clock이 나가지 않는다(시도 2·8).
- `Stop` 뒤 `clock_resync`가 `Some`으로 남지만 `schedule`은 `playing`일 때만 돌고 `playing`을 켜는 길은 `start_playing` 하나이며 거기서 늘 다시 정한다. 새지 않는다(시도 1).
- 대기를 버리는 곳은 `Next::SongEnd` 처리뿐이다. 곡 끝 정리가 시작 여유 때문에 다음 바퀴로 밀려도 대기가 남는다(시도 7·8).

## 5. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 리뷰 2 시험 `assert_resynced`(재생 중 시작인 시도 6)는 `play` 앞 바퀴에 넘긴 옛 Clock이 drain 뒤에 도착하면 드물게 실패한다(20번 묶음 실행 중 1번). 스케줄러 결함이 아니라 시험 하네스의 경합이다. 기존 시험은 고치지 않는 규칙이므로, 보고서에 이 시험이 드물게 실패할 수 있음과 원인을 적고, 같은 시나리오는 옛 Clock을 떼고 보는 `review_s3_t3d_r3.rs::play_past_end_then_swap_while_playing_resyncs_repeatedly`가 고정한다고 남긴다 | `crates/keyscore-midi/tests/review_s3_t3d_r2.rs:221`–`236` |
| 2 | 제안 | `Command::Stop` 처리에서 `clock_resync`를 `None`으로 비우면 「대기는 재생 중에만 있다」가 상태로도 맞는다. 지금은 해가 없다(4절) | `crates/keyscore-midi/src/scheduler.rs:819`–`827` |
| 3 | 제안 | 대기가 바퀴를 넘기는 경로(lookahead < 시작 여유)는 기본 설정(10 ms)에서는 드물어 구현자 시험이 닿지 않는다. 리뷰어 시험 7·8을 그대로 커밋한다 | `crates/keyscore-midi/tests/review_s3_t3d_r3.rs` |

「수정」「차단」 없음. 리뷰 2 지적 1은 고쳐졌다(`review_s3_t3d_r2.rs` 6·7 통과, 시도 3·7·9).

## 6. 실시간 경로

추가된 것은 `Option<(u64, bool)>` 필드 하나와 분기뿐이다. 할당·락·로그 없음. 디버그 빌드 `assert_no_alloc`이 모든 루프백 시험에서 걸렸고 할당 패닉은 없었다.

## 7. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰어 시험 파일 `review_s3_t3d_r3.rs`는 저장소에 남긴다. 리뷰어는 지시에 따라 전체 시험과 앱 시험을 돌리지 않았다.

## 체크리스트

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(묶음 20번, 경합 시험 반복)
- [x] `git show 6dfcc08`에 범위 밖 변경이 없다(`scheduler.rs` 한 곳)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성 없음
- [x] 스타일 지적을 적지 않았다
