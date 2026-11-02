# 적대적 리뷰: S3-T8 일시정지 자리와 타이밍 시험 직렬화 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션이고 1·2회차 리뷰어와도 다른 새 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `2211243` (브랜치 `s3/t8-pause-position`, `main` `8bf7553` 대비 전체, 2회차 뒤는 `7adce20^..2211243`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험·보고서를 열기 전에 `Scheduler::pause`, `PauseReply`, `Signals`(`paused`·`paused_tick`·`paused_playing`), `Rt::handle`의 `Command::Stop`, `sent_through_now`, `set_base`·`swap`·`start_playing`, `transport_pause`, `play_from`, `check_song_end`, `engine::recover`·`reregister_main`만 읽고 적었다.

1. 원자 순서: 스레드는 틱·재생 중 플래그를 Relaxed로 쓴 뒤 번호를 Release로 쓰고, 손잡이는 번호를 Acquire로 읽은 뒤 둘을 Relaxed로 읽는다. 번호가 `>= mine`인 순간 읽은 두 값이 같은 정지의 것인가. 다른 정지가 두 읽기 사이에 끼는 길(`&mut self` 밖의 송신자)이 있는가.
2. 재생 직후(같은 바퀴에서 `Play` 다음 `Stop`, 시작 여유 안) 일시정지: `sent_through`가 `None`이라 기준점 틱(= 시작 자리)을 돌려주는가. 이것을 수백 번 거듭해도 노트가 겹치거나 빠지지 않는가.
3. 재생 중 교체(`swap`)와 섞인 반복: 교체가 `sent_through`를 그대로 두고 기준점을 옮기므로 그 직후 정지의 자리가 맞는가.
4. 루프 되감기 뒤 일시정지: `set_base`가 `sent_through`를 지우고 바퀴 끝에서 새 기준점으로 다시 구하므로, 되감은 뒤 자리가 새 바퀴 안의 틱인가. 이어서 재생할 틱이 루프 끝과 같아지는 길이 실제로 닿는가(계획은 S5로 미룸).
5. 일시정지 직후 다른 창 재생: `transport_pause`가 주인을 놓으므로 다른 창의 `play_from`은 앞 주인(없음)을 `mark_stopped`하지 않는다. 일시정지한 창은 `Paused(at)`를 지키고, 그 창을 다시 재생하면 다른 창을 멈추고 `at`에서 잇는가.
6. 일시정지 상태에서 편집: `transport_document_changed`는 재생 중이 아니면 바로 돈다. 이어서 재생하면 새 스냅샷의 같은 틱부터 보내고 옛 스냅샷의 노트(옛 채널)를 보내지 않는가.
7. 일시정지 중 MIDI 서버 재시작 복구(`recover`): 주인이 없으므로 `transport_stopped_by_server`가 불리지 않아 `Paused(at)`가 남는가. `reregister_main`의 `stop`·빈 스냅샷 교체 뒤 이어서 재생이 소리 나는가.
8. 곡 끝 감시와의 경합: 감시와 일시정지는 같은 메인 스레드라 끼지 않는다. `NotPlaying` 뒤 남은 `ended`가 다음 재생에서 옛 곡 끝으로 읽히지 않는가(`last_start` 비교).
9. 실시간 경로: `7adce20`이 스레드 쪽에 더한 것은 원자 store 하나(`paused_playing`)뿐인가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 원자 순서 | 간접(2회차 `no_answer_leaves_stop_queued_and_later_pause_answers_its_own_stop`, `pause_while_playing_at_max_tick_is_not_reported_as_not_playing`) | 코드로 확인. 손잡이가 `&mut self`이고 정지 명령을 넣는 길은 `send`뿐이라 기다리는 동안 다른 정지가 끼지 않는다 |
| 2 재생 직후 일시정지 반복 | 1회차 `pause_right_after_play_or_locate_returns_start`(한 번씩) | 수백 번 섞은 반복은 없음(시도 1·3) |
| 3 교체와 섞인 반복 | 1회차 `pause_after_swaps_neither_repeats_nor_drops` | 짧은 간격 반복과 섞은 것은 없음(시도 1) |
| 4 루프 | 없음 | 시도 2·5 |
| 5 다른 창 재생 | 없음 | 시도 4 |
| 6 일시정지 중 편집 | 없음 | 시도 6 |
| 7 서버 재시작 복구 | 없음 | 시도 7 |
| 8 곡 끝 감시 | 앱 `pause_after_scheduler_reached_song_end_stops`, 2회차 `pause_racing_song_end_is_either_at_before_end_or_not_playing_with_ended` | 코드로 확인(`take_ended`는 `ended >= last_start`) |
| 9 실시간 경로 | `assert_no_alloc`(디버그 빌드) | `7adce20^..HEAD`의 `scheduler.rs` 스레드 쪽 변경은 `Command::Stop`의 store 하나뿐 |

## 3. 깨뜨리기 시도

모두 `CARGO_TARGET_DIR=target/review`로 해당 시험 바이너리만 돌렸다. 루프백 시험은 `--test-threads=1`로 하나씩.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 스케줄러: 노트 100개(30틱 간격) 위에서 `play(from)` → 0–13 ms(0 ms 포함) 기다림 → 1/4 확률로 같은 스냅샷 교체 → `pause(50 ms)`를 끝까지 거듭(수백 회). 넘긴 Note On이 0..100 순서대로 정확히 한 번 | 못 깨뜨림(3회 실행 모두 통과) | `crates/keyscore-midi/tests/review_s3_t8_r3.rs::rapid_play_pause_cycles_with_swaps_neither_repeat_nor_drop` |
| 2 | 스케줄러: 루프 (0, 960) 안 노트 16개, 5–64 ms 간격으로 일시정지·이어서 재생 60회(되감기 여러 번). 넘긴 노트가 루프 순서를 겹침·빠짐 없이 잇는다 | 못 깨뜨림(82개, 루프 끝 경계 0회) | `review_s3_t8_r3.rs::pause_across_loop_wraps_keeps_cyclic_order` |
| 3 | 앱: 노트 40개(62.5 ms 간격)에서 `transport::Play` → 0–39 ms → `transport::Pause`를 들은 노트가 다 찰 때까지. 들은 노트가 순서대로 한 번씩, 매번 `Playing`·`Paused` | 못 깨뜨림(3회 통과) | `crates/keyscore-app/tests/review_s3_t8_r3.rs::rapid_play_pause_actions_hear_each_note_once` |
| 4 | 앱: 창 A 재생 → 일시정지 → 창 B 재생(짝수 회차는 정지, 홀수 회차는 재생 중으로 둠)을 4회, 마지막에 A 재생. A는 매번 `Paused(at)`를 지키고, B 사이에 A 노트가 없고, A 재생이 B를 멈추며, A의 노트는 겹침·빠짐 없음 | 못 깨뜨림(3회 통과) | `review_s3_t8_r3.rs::other_window_plays_between_pause_and_resume` |
| 5 | 스케줄러: 루프 끝(960) 직전 lookahead 안에서 일시정지 24회. 계획이 S5로 미룬 「이어서 재생할 틱 = 루프 끝」이 공개 API로 닿는지 | 닿음(3/24회 `At(960)`). 앱은 `set_loop`을 부르지 않아 앱에서는 닿지 않으므로 계획 「범위 밖」의 근거는 맞다. 지적하지 않는다 | `review_s3_t8_r3.rs::pause_just_before_loop_end_can_return_loop_end`(관찰 시험, 통과) |
| 6 | 앱: 일시정지 → 트랙 채널을 1 → 2로 편집 → 이어서 재생. 상태는 `Paused`로 남고, 이어서 들은 노트는 모두 채널 2(`0x91`), 앞뒤를 합쳐 겹침·빠짐 없음 | 못 깨뜨림 | `review_s3_t8_r3.rs::edit_while_paused_then_resume_continues_on_new_snapshot` |
| 7 | 앱: 일시정지 → `engine::recover_virtual_port`(서버 재시작 복구) → 이어서 재생. `Paused(at)` 그대로, 이어서 재생이 소리 나고 겹침·빠짐 없음 | 못 깨뜨림 | `review_s3_t8_r3.rs::recover_while_paused_keeps_pause_position` |

앞 회차 시험도 돌렸다: `-p keyscore-midi --test review_s3_t8_r2` 4 통과, `--test scheduler pause` 3 통과. 새 시험 두 파일은 `cargo fmt --check`, `cargo clippy -p keyscore-midi -p keyscore-app --all-targets -- -D warnings` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `pause`의 답(`At`·`NotPlaying`)은 내 정지를 처리할 때의 상태다 | 아니오 | 틱·플래그 Relaxed → 번호 Release(`crates/keyscore-midi/src/scheduler.rs:894`–`898`), 번호 Acquire → 둘 읽기(`scheduler.rs:447`–`454`). 정지를 넣는 길은 `&mut self`의 `send`뿐 |
| 이어서 재생하면 겹침·빠짐 없음(상한에 걸린 틱 제외) | 아니오 | 시도 1·2·3·4·6·7 |
| 일시정지한 창은 다른 창의 재생·편집·복구에도 자리를 지킨다 | 아니오 | 시도 4·6·7 |
| 모델 불변식(`core`) | 해당 없음 | `core` 변경 없음 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1 (일시정지) | 충족 | `crates/keyscore-app/src/transport.rs:174`–`218`, `crates/keyscore-midi/src/scheduler.rs:439`–`461`, `888`–`905`. 시도 1·3·4·6·7. 루프와의 경계(시도 5)는 앱이 루프를 만들지 않아 닿지 않고 계획대로 S5 |
| TRN-3 (재생 중 편집) | 충족 | 시도 1(교체와 섞은 반복), 시도 6(일시정지 중 편집 뒤 재개) |
| NF-3 (지터) | 충족 | 실시간 경로 변경은 원자 store 셋뿐. 2회차의 `playback_jitter_p99_within_half_ms_short` 통과 뒤 `7adce20`이 바꾼 것은 store 하나 |
| NF-4 (드리프트) | 충족 | 같은 근거. 시간 계산 코드 변경 없음 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `Command::Stop`에 원자 store 셋(`scheduler.rs:894`–`898`)이고 `step`의 `assert_no_alloc` 안이다.
- 제품 코드의 `unwrap`/`expect`: 이번 변경에 없음.
- 에러가 삼켜지는 곳: 없음. `QueueFull`은 알리고 재생 상태로 남는다(`transport.rs:205`–`210`). 그 밖의 오류는 알리고 추정 자리로 일시정지(`transport.rs:211`). `NoAnswer`는 알리지 않고 추정 자리(2회차가 받아들인 근거 그대로).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| - | - | 없음. 2회차 지적 1 대응(`paused_playing` 분리)은 맞고, 새로 시도한 일곱 길에서 깨뜨리지 못했다. 시도 5의 루프 끝 경계는 계획 「범위 밖」(S5)이고 앱에서 닿지 않음을 확인했다 | - |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

2회차 지적 1의 대응은 맞다. 재생 중 플래그를 따로 두어 틱 값과 표식이 겹치지 않고, 쓰기(틱·플래그 Relaxed → 번호 Release)와 읽기(번호 Acquire → 둘)의 순서로 번호가 내 정지 이상일 때 읽은 두 값은 그 정지의 것이다. 기다리는 동안 다른 정지는 `&mut self` 때문에 끼지 않는다. 재생 직후·짧은 간격의 반복, 교체·루프 되감기와 섞인 일시정지, 다른 창 재생, 일시정지 중 편집과 서버 재시작 복구에서 겹침·빠짐·상태 어긋남을 만들지 못했다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`7adce20^..HEAD`: `scheduler.rs`, `report.md`)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
