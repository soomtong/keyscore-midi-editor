# 적대적 리뷰: S1-T4 스케줄러 스레드 (리뷰 4회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `d21d9e8` (브랜치 `s1/t4-scheduler`). 리뷰 3 뒤 코드 변경은 `6ec5ce4` 하나(`git diff f069999..HEAD -- crates/`는 `scheduler.rs`뿐)
- 남긴 테스트: `crates/keyscore-midi/tests/review_s1_t4_r4.rs`(통과 10, 실패 0)
- 범위(좁힘): A. 리뷰 3 수정 지적 1–3의 해소와 그 주변 변형, B. 수정이 들인 회귀(기존 테스트, 실시간 루프의 할당·락·로그), C. 제안 4·5 미반영의 수용 여부. 이 밖의 것은 제안으로만 적는다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 지적 1 수정(`release_changed_ports`)은 번호마다 `same_port_at`으로 같은 객체인지 본다. 같은 포트 객체가 다른 번호로 옮겨 가면([A, B] → [B, A]) 두 노트가 각자의 옛 포트로 꺼지고, 다른 포트로 Note Off가 새지 않는가.
2. 정지 중 같은 번호에 같은 포트가 남는 교체(포트 추가)는 미리듣기 노트를 끊지 않는가(대조군).
3. 지적 2 수정(명령을 꺼내기 전마다 `panic_if_due(false)`)은 앞 명령 수가 한 바퀴 처리량(64)과 정확히 같을 때, 그리고 두 바퀴에 걸칠 때도 앞뒤를 가르는가.
4. 큐 머리가 교체로 막혔을 때(`blocked`) 패닉은 바로 가는가. 막힘이 풀린 뒤 처리되는 패닉 뒤 명령의 노트를 끄지 않는가. 패닉 앞에 넣었지만 막힌 교체 뒤에 있던 명령은 어떻게 되는가.
5. 지적 3 수정(`sent_through_now`)에서 다음 항목이 Clock이면 그 틱을 「보냄」으로 친다. 상한이 Clock 바로 앞에서 끊긴 바퀴 뒤 교체에서 Clock이 빠지거나 겹치지 않는가. 같은 틱 중간에서 끊긴 경우(`None`)와 기준점 뒤의 틱에서 끊긴 경우도.
6. 교체는 `next_clock`을 다시 계산하지 않는다. 그런데 `Command::Clock`(재생 중 다른 포트의 Clock 켜기)은 여전히 `clock_after(sent_through + 1)`로 다시 계산한다. 다음 항목이 Clock이라 `sent_through`가 그 Clock 틱이면 그 Clock을 건너뛰지 않는가.
7. 상한에 걸린 바퀴도 `scheduled_until`을 창 끝으로 올린다(리뷰 3 지적 3이 고치라고 적은 부분). 교체가 `floor`를 거기로 올리면 남은 항목이 늦게 가지 않는가.
8. 다음 이벤트가 먼 뒤일 때 보낸 구간 바로 뒤에 넣은 이벤트가 `sent_through_now`의 내림(`tick_at(horizon + 1)` 뒤 한 틱 빼기)에 먹히지 않고 한 번, 정확한 시각에 가는가.
9. 교체를 짧은 간격으로 100번 해도 Clock 격자가 그대로인가(다음 항목이 Clock인 순간의 교체 포함).
10. 새 코드에 할당·락·로그가 있는가(`peek_next`의 클로저, `release_changed_ports`의 고정 크기 반복).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 정지 중 포트 순서 바뀜 | 없음 | 리뷰 3 테스트는 다른 포트로 바뀜, 포트 수 줄어듦 |
| 2 정지 중 같은 포트 + 포트 추가 | 없음 | |
| 3 패닉 앞 명령 64개, 70개 | 부분 | 리뷰 3 시도 9는 101개(Note On 하나 확인), 앞뒤 순서는 리뷰 3 지적 2 테스트(앞 명령 1개) |
| 4 막힌 교체 + 패닉 | 부분 | `review_s1_t4_alloc.rs`가 막힘 뒤 패닉을 할당 관점으로만 본다 |
| 5 상한 + 교체 + Clock | 없음 | 리뷰 3 지적 3 테스트는 Clock 끔 |
| 6 상한 + `set_clock` | 없음 | |
| 7 상한 + 교체의 시각 | 없음 | |
| 8 보낸 구간 바로 뒤 삽입 | 없음 | |
| 9 교체 반복 + Clock | 부분 | 리뷰 3 시도 13은 Clock 끔 |
| 10 할당 | 있음 | `review_s1_t4_alloc.rs`, `scheduler_alloc.rs` |

## 3. 깨뜨리기 시도

모든 테스트는 이 파일 안의 `MidiPort` 구현(기록·관문 포트)을 쓴다. CoreMIDI 클라이언트를 만들지 않는다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 정지 중 [A, B]의 A에 60, B에 62를 켜고 [B, A]로 교체, `stop` | 못 깨뜨림. 60은 A로, 62는 B로 한 번씩 꺼지고 다른 포트로 새지 않는다 | `swap_while_stopped_with_ports_reordered_releases_each_note_on_its_own_port` |
| 2 | 정지 중 [A]의 A에 60을 켜고 [A, B]로 교체, 뒤에 `send_now(Note Off)` | 못 깨뜨림. 교체가 끊지 않고 Note Off 하나로 꺼진다. B는 아무것도 받지 않는다 | `swap_while_stopped_with_same_port_and_added_port_keeps_preview_note` |
| 3 | 관문으로 붙잡은 명령 포함 앞 명령 64개(마지막 Note On 60), 패닉, Note On 61 | 못 깨뜨림. 보낸 순서가 60 < 패닉 < 61, 60만 꺼진다 | `panic_after_exactly_one_wake_of_commands_orders_both_sides` |
| 4 | 3과 같은데 앞 명령 70개(두 바퀴에 걸침) | 못 깨뜨림 | `panic_in_middle_of_second_wake_orders_both_sides` |
| 5 | 교체 261번(돌려받기 큐 256 + 대기 4 + 막힌 머리 1) 뒤 Note On 60, 패닉, Note On 61, 그 뒤 돌려받기 | 패닉은 막힌 동안 바로 가고(CC 123 16개), 61은 꺼지지 않는다. 60은 패닉 뒤에 처리되어 울린 채 남는다(제안 3, 임시 단언으로 확인한 뒤 뺐다) | `panic_while_swap_blocks_queue_head_is_sent_and_spares_later_note` |
| 6 | 틱 0에 관문 CC + CC 4095개(상한 4096을 딱 채워 다음 항목이 틱 0 Clock), Clock 켬, 첫 바퀴를 붙잡고 같은 내용으로 교체 | 빠짐·겹침은 못 깨뜨림. CC 4095개가 한 번씩, Clock은 한 칸씩 간다. 다만 틱 0 Clock이 Start보다 약 9 ms 늦다(제안 2) | `swap_after_pass_capped_right_before_clock_sends_events_and_clocks_once` |
| 7 | 6과 같은데 CC 6000개(같은 틱 중간에서 끊김, `sent_through = None`) | 빠짐은 못 깨뜨림. 6000개 모두, Clock 격자 유지. 첫 Clock은 6처럼 늦다(제안 2) | `swap_after_pass_capped_mid_tick_with_clock_keeps_clock_grid` |
| 8 | 관문 CC + CC 6000개를 틱 40에 두고 재생, 그 바퀴를 붙잡고 교체 | 빠짐은 못 깨뜨림. 틱 40 Clock은 한 번. 약 0.47 ms 늦다(제안 2) | `swap_after_pass_capped_at_later_tick_keeps_clock_on_that_tick` |
| 9 | Clock 켜고 재생 중 0.7–2.7 ms 간격으로 같은 스냅샷 교체 100번 | 못 깨뜨림. 모든 Clock이 Start + k칸 ± 1 µs | `many_swaps_with_clock_on_keep_clock_grid` |
| 10 | 다음 이벤트가 5 s 뒤일 때, 교체 시각 + 25 ms의 틱에 Note On 62를 넣는 교체 | 못 깨뜨림. 한 번, 틱 시각 ± 1 µs에 가고 Note Off도 한 번 | `swap_inserting_event_just_after_sent_window_sends_it_once_on_time` |
| 11 | 6의 상한 바퀴를 붙잡은 동안 교체 대신 `set_clock(2, true)` | 깨짐. 포트 0의 틱 0 Clock이 빠져 첫 Clock이 20.83 ms(틱 40)에 간다. 리뷰 3 수정 전에도 같았다(`tick_at(horizon)` ≥ 0). 범위 밖이라 제안 1로 적고, 실패 테스트(`set_clock_after_pass_capped_before_clock_keeps_pending_clock`)는 실행 뒤 파일에서 뺐다 | 없음 |
| 12 | 리뷰 3 테스트(`review_s1_t4_r3.rs`) | 못 깨뜨림. 13개 모두 통과(리뷰 3의 실패 4개 해소) | 없음 |
| 13 | `cargo test -p keyscore-midi --no-fail-fast` 3회(이 파일 포함) | 3/3 모두 통과. 실패 0 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 스케줄러 루프에 할당·락·로그 없음 | 못 깨뜨림 | 새 코드는 `AtomicBool`/`AtomicU64` load/store(`panic_if_due`), 스택 클로저(`peek_next`), 정수 산술(`sent_through_now`), 고정 크기 반복과 `send_to`(`release_changed_ports`, `release_notes`와 같은 모양)뿐이다. `Vec` 증가·`Mutex`·`println!`·`format!` 없음. 할당 테스트 2개 통과 |
| 교체 뒤 이벤트가 빠지지 않는다(TRN-3) | 못 깨뜨림 | 시도 6–10, 리뷰 3 지적 3 테스트. 같은 틱에서 상한에 걸리면 그 틱을 다시 보낸다(겹침, 보고서에 적힌 선택) |
| Clock이 빠지거나 겹치지 않는다(OUT-5) | 교체 경로는 못 깨뜨림 | 시도 6–9. 상한 + `set_clock`만 빠짐(시도 11, 제안 1) |
| 노트가 걸리지 않는다 | 못 깨뜨림 | 시도 1, 2, 리뷰 3 지적 1 테스트 |
| 패닉의 앞뒤 순서 | 못 깨뜨림 | 시도 3, 4, 리뷰 3 지적 2 테스트. 큐가 막힌 때만 앞 명령이 패닉 뒤로 간다(시도 5, 제안 3) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 9.2 | 충족 | 루프 안 할당·락·로그 없음(`scheduler.rs:553-585, 739-778, 791-887, 944-960`) |
| TRN-3 | 충족 | 교체 뒤 빠짐 없음(시도 6–10). 상한 바퀴 뒤 남은 항목이 창 끝까지 늦는 것(제안 2)은 4096개 넘는 항목이 한 창에 몰린 극단에서만 생긴다 |
| OUT-5 | 충족 | 교체 경로 Clock 격자 유지(시도 6–9). 시도 11은 상한 + 재생 중 Clock 켜기의 극단(제안 1) |
| OUT-6 | 충족 | 모든 포트·채널에 패닉 메시지(시도 5에서 CC 123 16개). 순서(시도 3, 4) |
| CTL-6 | 충족 | 재생 시작 chase. 이번 수정이 건드리지 않았다 |

C. 리뷰 3 제안 4·5 미반영: 제안 4(패닉 둘의 경합)는 두 번째 패닉이 늦게 갈 수 있을 뿐 패닉 메시지는 반드시 모든 포트·채널로 가므로 OUT-6 문구를 어기지 않는다. 제안 5(데이터 엔트리 값이 파라미터별이 아님)는 곡이 NRPN에 값을 쓴 뒤 데이터 없이 RPN만 고르는 패턴에서 chase 값이 어긋나는 정밀도 문제다. CTL-6은 「재생 시작 시 현재 위치의 값을 먼저 보낸다」만 요구하고, RPN·NRPN 파라미터별 상태는 컨트롤러 레인(S6)에서 다룰 수 있다. TRN-3과는 무관하다. 둘 다 문서화된 한계로 받아들인다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 4번 표 첫 줄 참고.
- 제품 코드의 `unwrap`/`expect`: 새로 들어온 것 없음. `release_changed_ports`의 `u16::try_from(...).unwrap_or(u16::MAX)`, `self.active.get(i).copied().unwrap_or(0)`뿐이다.
- 에러가 삼켜지는 곳: 리뷰 3과 같다(`send_to` 결과, `make_realtime`, `join` 결과).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 재생 중 `set_clock(on)`은 `next_clock = clock_after(sent_through + 1)`로 다시 계산한다. 리뷰 3 수정으로 다음 항목이 Clock이면 `sent_through`가 그 Clock 틱이 되므로(`sent_through_now`의 `Next::Clock => Some(tick)`), 상한에 걸려 Clock이 남은 바퀴 직후의 `set_clock`은 그 Clock을 이미 켜져 있던 포트에서도 건너뛴다(시도 11, 첫 Clock 20.83 ms 늦음). 수정 전에도 같은 결과였고 한 창에 4096개 넘는 항목이 몰려야 생긴다. `clock_mask`가 이미 0이 아니면 `next_clock`을 그대로 두면 된다 | `scheduler.rs:671-680, 875-878` |
| 2 | 제안 | 상한에 걸린 바퀴도 `scheduled_until`을 창 끝(`horizon`)으로 올리고, 교체는 `floor`를 거기로 올린다. 그 바퀴에 남은 항목(다시 보내는 같은 틱 이벤트, 그 틱의 Clock)이 최대 미리보기 창(10 ms)만큼 늦게 간다(시도 6·7에서 첫 Clock 약 9 ms, 시도 8에서 0.47 ms). 빠지거나 겹치지는 않는다. 리뷰 3 지적 3이 「`scheduled_until`도 창 끝으로 올리지 않는 편이 맞다」고 적은 부분이 반영되지 않았다. 상한에서 끝났으면 `scheduled_until`을 마지막으로 보낸 항목의 시각까지만 올리면 된다. 테스트는 앞 Clock의 늦음을 창 안으로 허용해 통과한다(`assert_clock_grid`의 `late_ok`) | `scheduler.rs:770, 856` |
| 3 | 제안 | 큐 머리가 교체로 막힌 동안의 패닉은 기다리지 않고 바로 가므로(리뷰 1 지적 9의 선택), 패닉 앞에 넣었지만 막힌 교체 뒤에 있던 명령(시도 5의 Note On 60)이 패닉 뒤에 처리되어 울린 채 남는다. 전제가 `collect_garbage`를 부르지 않는 호출자이고 `swap` 문서에 패닉이 큐와 따로 닿는다고 적혀 있다. 막힘이 풀릴 때 `popped < panic_after`였던 패닉의 노트 끄기를 한 번 더 하거나, 문서에 「막힌 동안의 패닉은 그 앞 명령을 기다리지 않는다」를 적으면 된다 | `scheduler.rs:553-585` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 3의 수정 3건은 모두 해소됐다. 리뷰 3 테스트 13개가 통과하고, 주변 변형(정지 중 포트 순서 바뀜, 같은 포트 + 포트 추가, 패닉 앞 명령 64개와 70개, 막힌 교체 + 패닉, Clock 바로 앞·같은 틱 중간·뒤 틱에서 끊긴 바퀴 + 교체, 교체 100번 + Clock, 보낸 구간 바로 뒤 삽입)에서 이벤트·Clock의 빠짐이나 겹침, 걸린 노트, 패닉 순서 뒤집힘을 만들지 못했다. 실시간 루프에 할당·락·로그는 들어오지 않았고 `cargo test -p keyscore-midi --no-fail-fast` 3회 모두 통과했다. 리뷰 3 제안 4·5의 미반영은 OUT-6, TRN-3, CTL-6 문구를 어기지 않아 문서화된 한계로 받아들인다.

남은 셋은 모두 제안이다. 1과 2는 한 창에 4096개 넘는 항목이 몰린 극단에서만 생기는 시각 문제(Clock 하나 빠짐, 최대 10 ms 늦음)이고, 3은 `collect_garbage`를 부르지 않는 호출자에서만 생기며 이미 문서화된 선택의 결과다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(리뷰 3 테스트 파일은 보조 포트 양식을 가져오려고 1번 뒤에 읽었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(남긴 10개는 3회 통과, 시도 11은 실행해 실패를 확인한 뒤 뺐다)
- [x] REQ ID를 하나씩 대조했다(9.2, TRN-3, OUT-5, OUT-6, CTL-6)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 3 뒤 변경은 `scheduler.rs`, 보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 1–3 테스트 파일 변경 없음)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
