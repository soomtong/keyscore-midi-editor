# 적대적 리뷰: S3-T8 일시정지 자리와 타이밍 시험 직렬화 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션이고 1회차 리뷰어와도 다른 새 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `ac37801` (브랜치 `s3/t8-pause-position`, `main` `8bf7553` 대비 전체, 특히 `39dbd69..ac37801`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험·보고서를 열기 전에 `Scheduler::pause`, `PauseReply`, `Signals::paused`·`paused_tick`, `Command::Stop` 처리, `sent_through_now`, `transport_pause`, `Engine::take_owner`·`is_owner`, `check_song_end`만 읽고 적었다.

1. 상한(`MAX_ITEMS_PER_PASS` 4096)에 걸려 한 틱을 나눠 보내는 중의 일시정지: 자리는 그 틱이고, 이어서 재생하면 겹침이 그 틱 안의 이미 넘긴 이벤트로만 한정되는가. 다른 틱이 겹치거나 빠지지 않는가.
2. `NotPlaying`이 곡 끝이 아닌 이유로 오는 길: 스케줄러가 스스로 `playing = false`가 되는 곳은 곡 끝(`Next::SongEnd`, 지난 곡 끝으로 교체 포함)뿐인가. 그 밖에는 앱이 정지를 보내면서 창 상태도 바꾸는가. 표식 `NOT_PLAYING = u64::MAX`와 실제 틱이 부딪히는 길(기준점 틱이 `u64::MAX`, `sent_through + 1` 포화)은 없는가.
3. `QueueFull` 뒤: `pushed`가 늘지 않아 번호가 어긋나지 않는가. 창은 `Playing`·주인·기준점(재생 헤드)·곡 끝 감시를 그대로 들고 있는가. 큐가 빈 뒤의 일시정지가 정상으로 답하고 소리를 멈추는가.
4. `NoAnswer` 뒤 남은 정지: 곧 처리되어 소리가 멈추는가. 그 뒤의 `pause`가 남은 정지의 답을 자기 답으로 읽지 않는가(번호 비교 `>= mine`). 남은 정지 뒤 다시 재생·일시정지가 섞이지 않는가.
5. 곡 끝과 겨루는 일시정지: 곡 끝 정리는 lookahead만큼 일찍 한다. 답이 `At(t)`면 `t ≤ 곡 끝`이고 `take_ended`가 없어야 하며, `NotPlaying`이면 `take_ended`가 남아야 한다.
6. 다른 창이 엔진 주인일 때: `is_owner`가 거짓이면 스케줄러를 건드리지 않고 추정 자리로 일시정지한다. 재생 중이면서 주인이 아닌 창이 생기는 길이 있는가.
7. 신호 순서: 틱 Relaxed store → 번호 Release store, 읽기는 번호 Acquire → 틱. 다른 정지(`stop`, `play_from` 안의 `stop`)가 같은 신호를 덮어도 `&mut self`라 기다리는 동안은 끼지 않는가.
8. 실시간 경로: 1회차 뒤 바뀐 것이 스레드 쪽에 없는가(`Command::Stop`은 원자 store 둘 그대로).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 상한에 걸린 틱 | 1회차 리뷰 시험(실패 상태) | 2회차에서 기대를 규칙에 맞추고 앞뒤 틱 노트를 더했다(시도 1) |
| 2 곡 끝 아닌 `NotPlaying` | 없음 | 코드 경로로 확인(곡 끝뿐), 표식 충돌은 시도 5에서 깨짐 |
| 3 `QueueFull` 뒤 | 스케줄러 쪽 1회차 `pause_with_full_queue_errors_and_playback_continues`. 앱 쪽 없음(보고서: 공개 길 없음) | 시도 3·4. 앱 쪽은 `engine::send_port_test_note`로 큐를 채울 수 있었다 |
| 4 `NoAnswer` 뒤 | 없음(보고서: 스레드를 굶길 방법 없음) | `pause(Duration::ZERO)`로 만들 수 있다(시도 2) |
| 5 곡 끝과 겨룸 | 앱 `pause_after_scheduler_reached_song_end_stops`(곡 끝을 넉넉히 지난 뒤만) | 시도 6 |
| 6 다른 창이 주인 | 없음 | `play_from`이 앞 주인을 `mark_stopped`하므로 「재생 중인데 주인 아님」 길을 찾지 못했다. 시험하지 않았다 |
| 7 신호 순서 | 간접(1회차 시험 1·2) | 시도 2가 남은 정지 뒤 번호 비교를 본다 |
| 8 실시간 경로 | `assert_no_alloc`(디버그 빌드) | `39dbd69..HEAD`에 스레드 쪽 변경 없음(코드로 확인) |

## 3. 깨뜨리기 시도

모두 `CARGO_TARGET_DIR=target/review`로 해당 시험 바이너리만, 루프백 시험은 하나씩 돌렸다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 지적 1 기대 수정. 틱 480에 Note On 12288개, 앞뒤 틱(0–940, 20틱 간격)에 다른 워드의 노트 47개. 틱 480의 첫 워드가 넘어가는 순간 일시정지 → 이어서 재생. 자리 = 480, 다른 틱은 정확히 한 번, 틱 480은 빠짐 없음, 두 번 넘긴 것은 일시정지 전에 넘긴 것뿐 | 못 깨뜨림(4회 모두 자리 `Tick(480)`, 4096/12288개 넘긴 뒤, 겹침 4096개 모두 그 틱 안) | `keyscore-midi/tests/review_s3_t8.rs::pause_inside_capped_tick_does_not_resend_its_events`(기대 수정) |
| 2 | `pause(Duration::ZERO)`로 `NoAnswer`를 만든다(20회 중 여러 번). 남은 정지가 곧 소리를 멈추는지, 이어서 `pause`가 `NotPlaying`인지, 다시 재생 → `pause(50 ms)`가 새 자리를 답하고 그 뒤 노트를 넘기지 않는지 | 못 깨뜨림 | `keyscore-midi/tests/review_s3_t8_r2.rs::no_answer_leaves_stop_queued_and_later_pause_answers_its_own_stop` |
| 3 | 큐를 채워 `pause` → `QueueFull`. 큐가 빈 뒤 `pause` → `At(t)`, `t` 앞의 노트만 넘겼고 이어서 재생하면 겹침·빠짐 없음 | 못 깨뜨림(번호 어긋남 없음) | `review_s3_t8_r2.rs::pause_after_queue_full_answers_once_queue_drains` |
| 4 | 앱: 재생 중 한 번의 창 갱신 안에서 `engine::send_port_test_note`를 큐가 찰 때까지 부르고(256개) 곧바로 `transport::Pause`. 창이 `Playing`으로 남고 오류를 한 번 알리는지, 소리가 계속되는지, 다음 `Pause`가 소리를 멈추고 들은 박 뒤에 자리를 잡는지, 이어서 재생하면 겹침·빠짐이 없는지 | 못 깨뜨림(3회 반복 통과) | `keyscore-app/tests/review_s3_t8_r2.rs::pause_with_full_queue_keeps_playing_and_a_later_pause_works` |
| 5 | `play(Tick::new(u64::MAX))`로 재생 중(넘길 것 없음) `pause` | 깨짐. 재생 중인데 `NotPlaying`. 이어서 재생할 틱 `base_tick = u64::MAX`가 표식 `NOT_PLAYING`과 같다. `sent_through + 1`의 `saturating_add`도 같은 값에 닿을 수 있다. 앱은 이 답을 곡 끝 정지로 처리해 정지 상태(틱 0)로 간다 | `review_s3_t8_r2.rs::pause_while_playing_at_max_tick_is_not_reported_as_not_playing`(실패) |
| 6 | 곡 끝(틱 480) 24 ms 전부터 0.5 ms씩 늦춰 일시정지 24회. `At(t)`면 `t ≤ 480`·넘긴 노트 < `t`·`take_ended` 없음·이어서 재생하면 곡 끝에서 멈추고 8음이 한 번씩, `NotPlaying`이면 `take_ended`가 있고 8음이 한 번씩 | 못 깨뜨림(`At` 18회, `NotPlaying` 6회) | `review_s3_t8_r2.rs::pause_racing_song_end_is_either_at_before_end_or_not_playing_with_ended` |

구현자의 시험도 돌렸다: `-p keyscore-midi --test scheduler pause` 3 통과, `--test scheduler playback_`(NF-3·NF-4 단축판) 2 통과, `-p keyscore-app --test pause_position` 3 통과, `-p keyscore-app --test review_s3_t8` 1 통과, `-p keyscore-midi --test review_s3_t8` 6 통과(기대 수정 뒤).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 이어서 재생하면 겹침은 상한에 걸린 그 틱 안에서만, 빠짐은 없다(리뷰 1 지적 1 대응의 규칙) | 아니오 | 시도 1 |
| `PauseReply`는 정지를 처리할 때의 상태를 맞게 답한다(`At` ⇔ 재생 중) | 예(기준점 틱 또는 이어서 재생할 틱이 `u64::MAX`) | 시도 5. `crates/keyscore-midi/src/scheduler.rs:887`–`892`가 `u64::MAX`를 그대로 담고 `scheduler.rs:446`이 표식으로 읽는다 |
| `pause`의 답은 내 정지의 것이다(`NoAnswer`·`QueueFull` 뒤 포함) | 아니오 | 시도 2·3 |
| `QueueFull`이면 창은 재생 상태·주인을 지킨다 | 아니오 | 시도 4 |
| 모델 불변식(`core`) | 해당 없음 | 변경 없음 |

`NotPlaying`이 곡 끝이 아닌 이유로 오는 길은 시도 5 말고는 찾지 못했다. 스케줄러가 스스로 `playing = false`가 되는 곳은 `Next::SongEnd`(`scheduler.rs` `schedule`)뿐이고, 그 밖의 정지(`stop`, `play_from` 안과 실패 뒤의 `stop`, `release`, `reregister_main`)는 모두 앱이 보내면서 창을 정지 상태로 바꾸거나 주인을 바꾼다. 패닉은 재생을 멈추지 않는다. 시도 5는 앱의 틱이 `MAX_SONG_TICKS` 안이라 앱에서는 닿지 않는다.

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1 (일시정지) | 충족 | `crates/keyscore-app/src/transport.rs:174`–`218`, `crates/keyscore-midi/src/scheduler.rs:425`–`457`, `884`–`901`. 앱에서 닿는 틱 범위에서는 시도 1–4·6 모두 맞다. 시도 5는 앱에서 닿지 않는 `keyscore-midi` 공개 API의 경계값이라 아래 지적 1로 따로 셌다. 루프 구간과의 관계는 계획대로 S5 |
| TRN-3 (재생 중 편집) | 충족 | 1회차 시도 1(교체 뒤 일시정지) 그대로 통과, `39dbd69..HEAD`에 교체 쪽 변경 없음 |
| NF-3 (지터) | 충족 | `crates/keyscore-midi/tests/scheduler.rs` `playback_jitter_p99_within_half_ms_short` 통과(직렬) |
| NF-4 (드리프트) | 충족 | 같은 파일 `playback_drift_within_1ms_short` 통과 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `39dbd69..HEAD`에서 스레드 쪽 코드(`Rt`)는 바뀌지 않았다. `Command::Stop`의 원자 store 둘(`scheduler.rs:893`–`894`)은 `step`의 `assert_no_alloc` 안이다.
- 제품 코드의 `unwrap`/`expect`: 이번 변경에 없음.
- 에러가 삼켜지는 곳: 없음. `QueueFull`은 알리고 재생 상태로 남는다(`transport.rs:205`–`210`). 그 밖의 오류는 알리고 추정 자리로 일시정지한다(`transport.rs:211`). `NoAnswer`는 알리지 않고 추정 자리를 쓴다(`transport.rs:196`). 소리는 남은 정지로 곧 멈추므로(시도 2) 알릴 일은 아니라고 본다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 표식 충돌. 재생 중인데 이어서 재생할 틱이 `u64::MAX`면(`play(Tick::new(u64::MAX))`, 또는 `sent_through = u64::MAX - 1`의 `saturating_add(1)`) `paused_tick`이 `NOT_PLAYING`과 같아 `pause`가 `NotPlaying`을 돌린다. `PauseReply` 문서의 뜻(「정지를 처리할 때 이미 재생 중이 아니었다」)과 어긋나고, 앱은 이 답을 곡 끝 정지로 처리한다. 앱의 틱은 `MAX_SONG_TICKS` 안이라 앱에서는 닿지 않지만, `keyscore-midi`의 공개 API로 깨진다. 예: 재생 중 여부를 따로 신호로 두거나, 틱을 `u64::MAX - 1`로 막거나, `play`·`locate`가 범위 밖 틱을 받지 않게 한다. 리뷰 시험이 실패한 채로 작업 트리에 있다 | `crates/keyscore-midi/src/scheduler.rs:289`, `887`–`892`, `446`; 시험 `crates/keyscore-midi/tests/review_s3_t8_r2.rs:308` |
| 2 | 제안 | 지적 1 대응의 근거 문장 「빠뜨리면 그 틱의 Note Off를 잃어 노트가 울린 채 남을 수 있지만」은 일시정지에는 맞지 않는다. 정지가 울리는 노트를 모두 끄고, 이어서 재생은 그 앞의 노트를 다시 켜지 않으므로 Note Off를 잃어도 걸린 노트는 남지 않는다. 빠뜨리면 그 틱의 Note On(들려야 할 노트)을 잃는다는 것이 맞는 근거다. 결론(겹치기)은 타당하다고 보아 시험 기대를 맞췄다. 보고서의 문장을 고친다 | `docs/reports/s3-t8/report.md` 리뷰 대응 표 1행 |
| 3 | 제안 | 상한에 걸린 바퀴에서 클릭·Clock만 남은 틱은 이어서 재생할 때 건너뛴다(계획 「리스크」·보고서 「알려진 한계」에 있음). 이벤트는 「빠뜨리기보다 겹치기」인데 클릭은 빠뜨리기라 같은 문서 안의 규칙이 둘이다. `pause` 문서에 클릭 쪽 예외도 한 줄 적는다 | `crates/keyscore-midi/src/scheduler.rs:425`–`433`(문서), `1187`–`1191`(`Next::Click`·`Next::Clock` 갈래) |
| 4 | 제안 | 보고서 리뷰 대응 4의 「앱 시험에서 스케줄러 큐를 채울 공개 길이 없다」는 맞지 않다. `engine::send_port_test_note`는 재생 중에도 명령 하나를 넣어 한 번의 갱신 안에서 큐를 채운다. 앱 쪽 `QueueFull` 길은 리뷰 시험 `keyscore-app/tests/review_s3_t8_r2.rs::pause_with_full_queue_keeps_playing_and_a_later_pause_works`가 본다(통과). 보고서의 문장을 고친다 | `docs/reports/s3-t8/report.md` 리뷰 대응 표 4행 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

1회차 지적 1–4의 대응은 맞다. 지적 1의 「빠뜨리기보다 겹치기」는 교체와 같은 규칙이고, 겹침이 그 틱 안에만 머무는 것을 시험으로 확인해 1회차 시험의 기대를 그 규칙으로 고쳤다(통과). 다만 새로 더한 `PauseReply`에서 표식 충돌(지적 1)을 찾았고, 그 리뷰 시험이 실패한 채로 있어 자동 게이트(`cargo test --workspace`)를 막는다. 고친 뒤 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`39dbd69..HEAD`: 제품 코드 `scheduler.rs`·`lib.rs`·`transport.rs`, 시험 3파일, 보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(1회차 시험은 이름을 두고 기대만 고쳤다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
