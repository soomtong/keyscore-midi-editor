# 적대적 리뷰: S3-T3d 곡 끝 정지 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 컨텍스트를 물려받지 않은 새 리뷰 에이전트)
- 대상 커밋: `88d90c7`(`crates/keyscore-midi/src/scheduler.rs` `start_playing`, 시험 `song_end.rs::play_past_end_sends_only_stop_to_clock_devices`). 배경은 리뷰 1 제안 1과 보고서 「리뷰 대응」
- 범위: 이 커밋이 만든 회귀·결함만 「수정」「차단」으로 친다. 그 밖은 「제안」

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 재생 중 곡 끝 뒤로 `play`: 예전에는 `resync_clock`의 Stop(재생 중이었으므로) + SPP + Continue + 곡 끝 정리의 Stop으로 Stop이 두 번이었다. 이제 Stop이 정확히 한 번이고 그 뒤로 Clock이 없는가.
2. 재생 중 `locate`도 같은 길(`start_playing(to, true)`)이라 같은 결과인가.
3. 곡 끝 = 시작 위치(`end <= from`의 경계), 곡 끝 0에서 틱 0 시작(빈 프로젝트의 `Space`, 예전엔 Start 0xFA가 나감). 곡 끝 바로 앞(959 < 960)은 건너뛰지 않고 맞추는가.
4. `start_playing`의 판정(`looping.is_none() && end <= from`)과 `peek_next`의 즉시 곡 끝 판정(`looping.is_none() && end <= base_tick`)이 같은 조건인가. 둘이 어긋나면 맞추기도 곡 끝 정리도 없는 재생이 생긴다.
5. 판정 시점: `start_playing`은 명령을 꺼낼 때 판정하지만 곡 끝 정리는 같은 바퀴의 `schedule`에서 한다. 그 사이(같은 바퀴, 최대 64개 명령)에 `Loop(Some)`이나 곡 끝을 늘리는 `Swap`이 꺼내지면 4의 두 판정이 갈라진다.
6. `Clock(on)` 명령이 건너뛰기 뒤에 와도 `next_clock`만 바꾸므로 곡 끝 정리를 막지 않는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 재생 중 곡 끝 뒤 `play` | 없음 | 구현자 시험은 정지 상태에서 시작만 본다. 리뷰어 시험 추가 |
| 2 재생 중 `locate` | 없음 | 리뷰어 시험 추가 |
| 3 경계 | 없음 | 곡 끝 = 시작, 곡 끝 0, 곡 끝 바로 앞. 리뷰어 시험 추가 |
| 4 판정 일치 | 코드로 확인 | 두 식이 같다(`base_tick = from`, `set_base`) |
| 5 같은 바퀴의 루프·교체 | 없음 | 리뷰어 시험 추가(깨짐) |
| 6 `Clock(on)` | 코드로 확인 | `handle`의 `Command::Clock`은 `next_clock`만 바꾼다. 곡 끝 판정에 닿지 않는다 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-midi/tests/review_s3_t3d_r2.rs`. 실행은 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-midi --test review_s3_t3d_r2 --test song_end --test review_s3_t3d`를 세 번(`song_end` 9, `review_s3_t3d` 7 통과, `review_s3_t3d_r2` 5 통과 2 실패, 세 번 모두 같음). 비교를 위해 변경 전 커밋 `9015461`의 임시 worktree(시험 뒤 제거)에서 같은 파일을 돌렸다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클럭 켜고 0부터 재생(곡 끝 960) 중 `play(1920)`, 곡 끝 정지 뒤 `stop`, drop | 못 깨뜨림. Stop 정확히 한 번, 마지막이 Stop, SPP·Start·Continue 없음. 변경 전 커밋에서는 실패(Stop 두 번)라 이 커밋이 고친 것이다 | `play_past_end_while_playing_sends_exactly_one_stop` |
| 2 | 같은 상황에서 `locate(1000)` | 못 깨뜨림(1과 같음). 변경 전 실패 | `locate_past_end_while_playing_sends_exactly_one_stop` |
| 3 | 정지 상태, 곡 끝 960에서 `play(960)` | 못 깨뜨림(Stop만). 변경 전 실패(SPP·Continue·Stop) | `play_at_exact_end_sends_only_stop` |
| 4 | 곡 끝 0, `play(0)` | 못 깨뜨림(Stop만, Start 없음). 변경 전 실패(Start·Stop) | `play_at_zero_with_zero_end_sends_only_stop` |
| 5 | 곡 끝 960, `play(959)` | 못 깨뜨림(SPP·Continue 뒤 Stop, 건너뛰기 대상 아님) | `play_just_before_end_still_resyncs_clock` |
| 6 | 클럭 켜고 0부터 재생 중, `play(1920)`(곡 끝 960 뒤) 바로 다음에 곡 끝 96000인 스냅샷 `swap`. 두 명령이 한 바퀴(1 ms)에 꺼내진다 | 깨짐. 곡 끝 정리가 오지 않아 재생이 1920부터 이어지는데, 클럭 장치는 Stop·SPP·Continue를 하나도 받지 못하고 Clock(0xF8)만 받는다. 0부터 돌던 장치는 옛 위치에서 계속 세어 시퀀서(1920)와 어긋나며, 다음 정지·재시작까지 그대로다. 변경 전 커밋에서는 통과(Stop·SPP·Continue) | `play_past_end_then_swap_extending_end_in_same_wake_resyncs_clock` |
| 7 | 정지 상태, `play(1920)`(곡 끝 960 뒤) 바로 다음에 `set_loop(Some((0, 480)))` | 깨짐. 루프 중이라 곡 끝을 무시하고 재생이 이어지는데, 클럭 장치는 SPP·Continue 없이 Clock만 받는다(정지 상태의 장치는 따라오지 않는다). 루프 끝 480이 기준점 뒤가 아니라 되감기의 다시 맞추기도 없다. 변경 전 통과 | `play_past_end_then_loop_in_same_wake_resyncs_clock` |

6·7은 두 명령이 다른 바퀴에 꺼내지면(경합이 없으면) 곡 끝 정지의 Stop만 확인하고 통과하도록 썼다. 실제로는 세 번 모두 같은 바퀴에 꺼내져 실패했다.

## 4. 원인

`start_playing`은 명령을 꺼내는 순간의 `looping`·`snap.end`로 「곧 곡 끝 정리가 온다」고 미리 판정해 맞추기를 건너뛴다. 곡 끝 정리는 같은 바퀴 뒤쪽의 `schedule` → `peek_next`가 그때의 `looping`·`snap.end`로 다시 판정한다. 그 사이에 꺼낸 `Loop(Some)`(`handle`의 `Command::Loop`)이나 곡 끝을 늘리는 `Swap`(`swap`, `next_clock`을 그대로 둔다)이 두 번째 판정을 바꾸면, 맞추기를 건너뛰었는데 곡 끝 정리도 없는 재생이 된다. 변경 전에는 맞추기를 늘 했으므로 이 상태가 없었다.

앱에서의 도달: `transport.rs`는 교체를 `play` 앞에 넣고(`:369` → `:381`), `set_loop` 호출은 아직 없다(S5). 그래서 지금 앱에서는 `play` 직후 1 ms 안에 편집 교체가 들어오는 드문 경합에서만 닿는다. 그러나 `Scheduler`의 공개 API(`play`·`locate` 뒤 `swap`·`set_loop`)로는 결정적으로 재현되고, S5의 루프·위치 이동이 바로 이 순서를 쓸 수 있다.

## 5. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 곡 끝 뒤 시작에서 맞추기를 건너뛴 뒤 같은 바퀴에 `Loop(Some)`이나 곡 끝을 늘리는 `Swap`이 꺼내지면, 재생은 이어지는데 클럭 장치는 Stop·SPP·Continue 없이 Clock만 받는다(시도 6·7, 변경 전에는 없던 회귀). 판정을 명령 처리 때가 아니라 곡 끝 정리를 판정하는 곳과 같은 시점으로 옮긴다. 예: `start_playing`은 맞추기를 「대기」(시작 틱·시각·`was_playing`)로만 남기고, `schedule` 첫머리에서 `peek_next`가 즉시 곡 끝이 아니면 그때 `resync_clock`을 한다(곡 끝이면 대기를 버린다). 또는 `Loop`·`Swap` 처리에서 대기 중인 맞추기가 있고 더는 즉시 곡 끝이 아니면 맞춘다. 어느 쪽이든 `next_clock`도 그 결과로 다시 잡아야 한다 | `crates/keyscore-midi/src/scheduler.rs:880`–`887`, `:831`–`832`, `:994`–`997` |
| 2 | 제안 | 구현자 시험 `play_past_end_sends_only_stop_to_clock_devices`는 정지 상태 시작만 본다. 이 커밋이 실제로 고친 것(재생 중 `play`·`locate`의 Stop 두 번 → 한 번, 곡 끝 0의 Start 제거)은 리뷰어 시험 1–4가 고정한다. 리뷰어 시험 파일을 그대로 커밋한다 | `crates/keyscore-midi/tests/review_s3_t3d_r2.rs` |
| 3 | 제안 | 보고서 「리뷰 대응」 1의 문구 「Start·SPP·Continue를 건너뛴다」에 「재생 중이었으면 보내던 앞 Stop도 건너뛰어 Stop이 한 번이 된다」를 덧붙이면 동작이 다 적힌다 | `docs/reports/s3-t3d/report.md:80` |

## 6. 실시간 경로

변경은 `bool` 계산과 분기뿐이라 할당·락·로그가 없다. 디버그 빌드 `assert_no_alloc`이 위 루프백 시험에서 걸렸고 통과했다(시험 실패는 내용 단언이며 할당 패닉이 아니다). 지적 1의 수정도 고정 크기 필드 하나면 되어 실시간 규율에 닿지 않는다.

## 7. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(수정)을 고치고 `review_s3_t3d_r2.rs`의 7개 시험이 모두 통과하면 다시 본다. 리뷰어 시험 파일은 저장소에 남긴다. 리뷰어는 지시에 따라 전체 시험을 돌리지 않았다.

## 체크리스트

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(세 번, 그리고 변경 전 커밋과 비교)
- [x] `git show 88d90c7`에 범위 밖 변경이 없다(`scheduler.rs` 한 곳, 시험 하나)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성 없음
- [x] 스타일 지적을 적지 않았다
