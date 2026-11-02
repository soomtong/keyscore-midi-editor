# 적대적 리뷰: S3-T8 일시정지 자리와 타이밍 시험 직렬화 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 컨텍스트를 물려받지 않은 새 에이전트)
- 대상 커밋: `7446c22` (브랜치 `s3/t8-pause-position`, `main` `8bf7553` 대비)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험·보고서를 열기 전에 `Scheduler::pause`, `Signals::paused`·`paused_tick`, `Command::Stop` 처리, `sent_through_now`, `swap`, `transport_pause`만 읽고 적었다.

1. 실시간 스레드: `Command::Stop`에 더한 것이 원자 store 둘뿐이고 `assert_no_alloc` 범위 안인가.
2. 신호 순서: `paused_tick` Relaxed store → `paused` Release store, 읽기는 `paused` Acquire → `paused_tick` Relaxed. `pushed`와 `popped`가 같은 방식으로 세어지는가(교체로 막힌 명령은 꺼내지 않으므로 세지 않는다). 시간 초과로 남은 옛 정지가 뒤 `pause`의 답으로 잘못 읽히지 않는가.
3. 재생·위치 이동 직후 같은 바퀴의 정지: `set_base`가 `sent_through`를 `None`으로 두므로 시작 틱을 돌려야 한다.
4. 교체 직후 정지: `swap`은 `sent_through`를 그대로 두고 기준점을 `t`로 옮긴다. 이어서 재생할 틱이 `t + 1`이고 `cursor = first_after(t)`와 맞는가.
5. 항목 상한(`MAX_ITEMS_PER_PASS` 4096)에 걸려 같은 틱 이벤트 일부만 넘긴 바퀴: `sent_through_now`는 그 틱 앞까지로 줄이므로 이어서 재생하면 같은 틱의 넘긴 이벤트를 다시 넘길 것이다(교체 쪽은 「빠뜨리기보다 겹치기」로 받아들였다). 일시정지의 「겹침 없음」 약속과 어긋나는지 본다.
6. 곡 끝에서 스스로 멈춘 뒤의 `pause`: `playing == false`라 `NOT_PLAYING` → `None`. 앱은 시간 초과와 구별하지 못하고 추정 자리(곡 끝 뒤)로 일시정지할 것이다.
7. 교체가 큐 머리를 막고 있을 때 `pause`가 `collect_garbage`로 풀어 제때 답하는가.
8. 큐가 가득일 때: `pause`는 `QueueFull`, 정지는 들어가지 않는다. 앱은 이미 `take_owner(None)`을 했으므로 재생은 계속되는데 주인이 없다(기존 `stop`과 같은지).
9. 루프 끝 직전 정지: 이어서 재생할 틱이 루프 끝 `E`가 되면 `play(E)`는 루프 밖이다(계획이 S5로 미룸).
10. 앱: 정지 중·다른 창이 주인일 때 동작이 기존과 같은가(`mark_stopped`, 오류 보고).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 실시간 규율 | 간접(기존 `scheduler_alloc` 등, 디버그 빌드 `assert_no_alloc`) | 새 시험은 디버그 빌드라 정지 처리도 검사된다 |
| 2 신호 순서·번호 | 없음 | 리뷰 시험 1·2가 거듭 정지·재생으로 간접 확인 |
| 3 재생·위치 이동 직후 | 없음 | 리뷰 시험 2 추가 |
| 4 교체 직후 | 없음(앱 `review_s2_t5a_r2`가 간접) | 리뷰 시험 1 추가 |
| 5 항목 상한 | 없음 | 리뷰 시험 5 추가, 깨짐 |
| 6 곡 끝 뒤 | 없음 | 리뷰 시험 3(midi), 앱 시험 1 추가 |
| 7 교체로 막힌 큐 | 없음 | 리뷰 시험 4 추가 |
| 8 가득 찬 큐 | 없음 | 리뷰 시험 6 추가 |
| 9 루프 끝 | 없음 | 계획 「범위 밖」(S5). 시도하지 않았다 |
| 10 정지 중 | `pause_when_stopped_returns_none` | 앱 경로의 정지 중은 기존 조기 반환 그대로 |

## 3. 깨뜨리기 시도

모두 `CARGO_TARGET_DIR=target/review`로 해당 시험 바이너리만 돌렸다. midi 쪽은 CoreMIDI 대신 할당 없이 `MidiPort::send` 호출을 기록하는 가짜 포트를 써서 「OS에 넘긴 것」을 정확히 본다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 재생 중 교체를 1–3번 거듭한 직후(같은 바퀴일 수 있게) 일시정지 10회, 매번 자리 이상의 노트를 넘기지 않았는지, 끝까지 이어서 재생해 64음이 한 번씩 순서대로인지 | 못 깨뜨림 | `keyscore-midi/tests/review_s3_t8.rs::pause_after_swaps_neither_repeats_nor_drops` |
| 2 | `play(from)` 또는 `play` + `locate(from)` 바로 뒤 일시정지 20회. 자리 ≥ 시작, 넘긴 노트가 [시작, 자리) 안 | 못 깨뜨림 | `pause_right_after_play_or_locate_returns_start` |
| 3 | 곡 끝(틱 480)에서 스스로 멈춘 뒤 `pause` | 설계대로 `None`, `take_ended`는 남는다 | `pause_after_song_end_returns_none` |
| 4 | 옛 스냅샷을 받지 않고 교체 40개를 넣어 큐 머리를 막은 뒤 `pause(50 ms)` | 못 깨뜨림(제때 `Some`) | `pause_with_swaps_blocking_queue_head_still_answers` |
| 5 | 틱 480에 서로 다른 Note On 12288개(상한 4096의 세 바퀴). 첫 바퀴가 넘어가는 순간 일시정지 | 깨짐. 4096/12288개를 넘긴 뒤 자리 `Tick(480)`. 이어서 재생하면 넘긴 4096개를 다시 넘긴다. 4회 실행 모두 같은 결과 | `pause_inside_capped_tick_does_not_resend_its_events`(실패) |
| 6 | 명령 큐를 채운 뒤 `pause` | `Err(QueueFull)`, 재생은 계속된다(설계대로지만 앱 쪽 제안 4의 근거) | `pause_with_full_queue_errors_and_playback_continues` |
| 7 | 앱 Normal 모드: 스케줄러가 곡 끝(0.5 s)에서 멈춘 뒤 창의 곡 끝 감시가 깨기 전에 `transport::Pause` | 깨짐(기존 동작과 같음). `Paused(Tick(1559))`로 곡 끝(960) 뒤에 일시정지. 이어서 재생하면 곧바로 정지로 돌아온다. 시험은 어느 쪽이든 통과하도록(고치면 정지, 아니면 이어서 재생이 정지로 회복) 남겼다 | `keyscore-app/tests/review_s3_t8.rs::pause_after_scheduler_song_end_pauses_past_end_then_play_stops` |

구현자의 시험도 돌렸다: `-p keyscore-midi --test scheduler` 13 통과(55 s), `-p keyscore-app --test pause_position` 2 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 이어서 재생할 틱 앞의 이벤트는 모두 넘겼고, 그 틱 이상은 하나도 넘기지 않았다 | 예(한 틱의 이벤트가 상한에 걸려 여러 바퀴에 나뉠 때) | 시도 5. `sent_through_now`의 `Next::Event` 갈래(`scheduler.rs:1168`)가 같은 틱 일부를 넘겼어도 그 틱 앞까지로 줄이고, `Command::Stop`(`scheduler.rs:859`)이 그 값 + 1을 돌려준다 |
| 교체·재생·위치 이동과 겹친 정지에서 겹침·빠짐 없음 | 아니오 | 시도 1·2 |
| `paused` 번호가 내 정지의 것일 때만 틱을 읽는다 | 아니오 | `&mut self`로 다른 명령이 끼지 않고, 시간 초과로 남은 옛 정지는 번호가 작다. 교체로 막힌 명령은 꺼내지 않아 `popped`가 `pushed`와 같은 순서로 센다 |
| 모델 불변식(`core`) | 해당 없음 | 변경 없음 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1 (일시정지) | 충족 | `crates/keyscore-app/src/transport.rs:174`–`204`, `crates/keyscore-midi/src/scheduler.rs:412`–`429`. 루프 구간과의 관계는 S5로 미뤄 이번 범위에서 빼고 셌다 |
| TRN-3 (재생 중 편집) | 충족 | 교체 뒤 일시정지가 맞다(리뷰 시도 1), `review_s2_t5a_r2` 그대로 통과(보고서) |
| NF-3 (지터) | 충족 | `crates/keyscore-midi/tests/scheduler.rs` `playback_jitter_p99_within_half_ms_short` 직렬 실행 통과 |
| NF-4 (드리프트) | 충족 | 같은 파일 `playback_drift_within_1ms_short` 통과 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `Command::Stop`에 원자 store 둘(`scheduler.rs:865`–`866`)만 더했고 `step`의 `assert_no_alloc` 안이다.
- 제품 코드의 `unwrap`/`expect`: 이번 변경에 없음.
- 에러가 삼켜지는 곳: 없음. 다만 `pause`의 `Ok(None)`이 「재생 중이 아니었다」와 「시간 초과」를 한데 묶어(`scheduler.rs:422`, `425`) 앱이 두 경우를 구별하지 못한다(제안 3).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 한 틱의 이벤트가 항목 상한(4096)에 걸려 여러 바퀴에 나뉠 때 그 사이에 일시정지하면 자리는 그 틱이고, 이어서 재생하면 같은 틱의 이미 넘긴 이벤트를 다시 넘긴다(시도 5에서 4096개 겹침). 계획의 끝의 정의 1(「이미 넘긴 노트를 다시 보내지 않고」)과 어긋나며, 보고서 「알려진 한계」에는 클릭 하나 건너뛰기만 있고 이 겹침은 없다. 고치거나(예: 정지가 그 틱의 남은 이벤트를 넘긴 뒤 다음 틱을 돌려주기, 또는 겹침을 받아들이는 근거를 결정 문서로), 받아들이면 그 이유와 범위를 보고서에 적고 2회차에서 리뷰 시험의 기대를 그 범위로 맞춘다 | `crates/keyscore-midi/src/scheduler.rs:1168`, `859`; 시험 `crates/keyscore-midi/tests/review_s3_t8.rs:272` |
| 2 | 제안 | 같은 원인을 보고서·`pause` 문서에 남긴다. 교체 쪽(`sent_through_now` 주석)은 이미 「빠뜨리기보다 겹치기」를 적었으나 `pause` 문서는 「넘기지 않은 첫 틱」이라고만 한다 | `crates/keyscore-midi/src/scheduler.rs:403`–`411` |
| 3 | 제안 | `pause`의 `None`이 「재생 중이 아니었다」와 「시간 초과」를 구별하지 않는다. 스케줄러가 곡 끝에서 멈춘 뒤 창의 감시(20 ms 주기)가 깨기 전에 일시정지하면 곡 끝 뒤 추정 자리로 일시정지한다(시도 7, `Paused(1559)`, 곡 끝 960). 결정 A(곡 끝에서 정지·틱 0)와 맞추려면 「재생 중이 아니었다」를 따로 돌려 정지 상태로 가면 된다. 기존 동작과 같고 이어서 재생하면 곧 정지로 회복하므로 제안 | `crates/keyscore-midi/src/scheduler.rs:422`; `crates/keyscore-app/src/transport.rs:197` |
| 4 | 제안 | `pause`가 `QueueFull`이면 정지가 들어가지 않아 재생이 계속되는데, 창은 이미 주인을 놓고(`take_owner(None)`) 일시정지 상태가 된다. 정지 Action은 주인이 아니라 정지를 보내지 않으므로 다음 재생(`play_from`의 `stop`)이나 패닉 전까지 소리가 멈추지 않는다(시도 6이 스케줄러 쪽을 확인). 기존 `stop`도 같아 이번 Task가 만든 문제는 아니다 | `crates/keyscore-app/src/transport.rs:194`, `198`(기존 `478`–`480`) |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1의 리뷰 시험이 실패한 채로 작업 트리에 있어 자동 게이트(`cargo test --workspace`)를 막는다. 고치거나, 받아들이는 근거를 적은 뒤 2회차에서 시험의 기대를 맞춘다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(제품 코드 2파일, 시험 2파일, 계획·보고 2파일)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
