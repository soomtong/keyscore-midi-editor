# 적대적 리뷰: S1-T4 스케줄러 스레드 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `00f9320` (브랜치 `s1/t4-scheduler`, `git diff main...HEAD`)
- 남긴 테스트: `crates/keyscore-midi/tests/review_s1_t4.rs`(실패 12, 통과 4), `crates/keyscore-midi/tests/review_s1_t4_alloc.rs`(통과 1)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `Scheduler` drop과 패닉이 `now`에 노트를 끈다. lookahead 안에 이미 넘긴 Note On이 그 뒤에 도착하면 노트가 걸리지 않는가.
2. 교체(`swap`)가 기준점을 `(tick_at(now) 내림, now)`로 다시 잡는다. 교체마다 최대 한 틱만큼 격자가 밀려 누적되지 않는가(NF-4).
3. 교체 때 `sent_tick = tick_at(last_sent)`가 내림이라, 이미 보낸 틱의 이벤트·Clock을 새 스냅샷에서 다시 보내지 않는가. 반대로 이미 예약한 Note On에 같은 시각 Note Off를 붙여 길이 0으로 만들지 않는가.
4. `div_ceil(CLOCK_TICKS) * CLOCK_TICKS`, `next_clock += CLOCK_TICKS`가 공개 API의 큰 틱에서 넘쳐 스케줄러 스레드가 패닉하지 않는가.
5. 돌려받기 큐가 가득 차면 교체 명령이 큐 머리에서 막힌다. 뒤에 줄 선 패닉·정지도 막히지 않는가.
6. SPP는 16분음표 내림 위치인데 첫 Clock은 40틱 올림 위치다. 16분음표에 맞지 않는 위치에서 받는 쪽 위치가 어긋나지 않는가. 재생 중 위치 이동 때 Stop 없이 SPP를 보내지 않는가.
7. chase가 CC를 번호 순으로 보낸다. RPN/NRPN(101·100·99·98 뒤 6·38) 순서가 뒤집히지 않는가. 피치 벤드는 chase하지 않는가.
8. 스레드 종료 때 `Rt`가 스케줄러 스레드에서 drop된다. 스냅샷·포트의 마지막 소유자가 스케줄러면 포트 해제(락·CoreMIDI 호출)가 그 스레드에서 일어나지 않는가.
9. 루프 안의 모든 호출(`rtrb` push/pop/peek, `TempoMap` 변환, `MidiPort::send`, `Arc` 이동)에 할당·락·로그가 있는가. 돌려받기 큐가 가득한 갈래도 할당이 없는가.
10. 템포가 다른 스냅샷으로 바꾼 뒤 새 격자가 정확한가. 루프 되감기를 여러 번 해도 격자가 정확한가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 drop·패닉 뒤 걸린 노트 | 없음 | 패닉 테스트는 정지 상태에서 메시지 48개만 본다. drop 테스트 없음 |
| 2 교체 반복 뒤 격자 | 없음 | 교체 테스트는 한 번만 하고 메시지 종류만 본다 |
| 3 교체 때 중복·길이 0 | 없음 | 교체 테스트는 옛 노트 48이 꺼지고 62가 안 오는 것만 본다 |
| 4 큰 틱 넘침 | 없음 | |
| 5 돌려받기 큐 포화 | 없음 | 교체 1회만 |
| 6 SPP 정렬, 재생 중 위치 이동 | 부분 | 960틱(정렬된 위치)에서 정지 뒤 재생만 본다 |
| 7 RPN 순서, 벤드 chase | 없음 | 뱅크·프로그램·CC7만 |
| 8 종료 때 해제 스레드 | 없음 | |
| 9 할당 0 | 부분 | `scheduler_alloc.rs`는 교체 1회, 포트 1개. 큐 포화 갈래와 포트 수가 다른 교체는 안 지난다 |
| 10 템포 다른 교체, 루프 반복 격자 | 부분 | 루프는 메시지 종류 5개만 본다 |

## 3. 깨뜨리기 시도

모두 `cargo test -p keyscore-midi --test review_s1_t4`로 실제 실행했다. 실패 12개는 3회 연속 같은 결과였다. 기록 포트(`MidiPort`를 구현해 스케줄러가 넘긴 타임스탬프를 그대로 적음)를 써서 결정적으로 만들었다. 여러 테스트가 lookahead를 300 ms로 넓혔다. 기본 10 ms에서도 같은 일이 그 창 안에서 확률적으로 일어난다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | Note On을 미래로 예약한 뒤 `Scheduler` drop | 깨짐. Note Off가 Note On보다 먼저 예약돼 노트가 걸림 | `drop_with_note_on_already_scheduled_leaves_no_stuck_note` |
| 2 | Note On을 미래로 예약한 뒤 패닉, 정지 | 깨짐. 패닉이 표를 비워 정지가 Note Off를 안 보냄 | `panic_then_stop_with_note_on_already_scheduled_leaves_no_stuck_note` |
| 3 | 이미 예약한 Note On이 있는 채로 같은 내용의 스냅샷으로 교체 | 깨짐. Note On과 같은 타임스탬프에 Note Off가 붙어 길이 0(노트 사라짐) | `swap_with_identical_snapshot_sends_scheduled_note_on_once_and_keeps_length` |
| 4 | 재생 중 같은 내용 교체 200회(3 ms 간격) | 깨짐. 박 격자가 49.7 ms 밀림 | `repeated_swaps_keep_beat_grid_within_1ms` |
| 5 | Clock 켜고 재생 중 교체 100회 | 깨짐. Clock 간격 최소 0.07 ms(같은 Clock 중복) | `swaps_during_play_do_not_duplicate_midi_clock` |
| 6 | Clock 켜고 틱 100에서 재생 | 깨짐. SPP 0(틱 0)인데 첫 Clock은 틱 120 | `play_from_unaligned_tick_sends_spp_matching_first_clock` |
| 7 | Clock 켜고 재생 중 위치 이동 | 깨짐. FA, F2, FB(Stop 없이 SPP) | `locate_while_playing_with_clock_sends_stop_before_spp` |
| 8 | `play(Tick::new(u64::MAX))` 뒤 `send_now` | 깨짐. `scheduler.rs:504` 곱셈 넘침 패닉으로 스레드가 죽고 이후 명령이 무시됨 | `play_at_max_tick_keeps_scheduler_thread_alive` |
| 9 | `collect_garbage` 없이 교체 300회 뒤 패닉 | 깨짐. `panic()`은 `Ok`인데 포트에 아무것도 안 감 | `panic_after_many_uncollected_swaps_still_reaches_ports` |
| 10 | CC101·100·6·38(벤드 범위 RPN) 뒤 중간에서 재생 | 깨짐. chase 순서 6, 38, 100, 101 | `chase_sends_rpn_select_before_data_entry` |
| 11 | 피치 벤드 뒤 중간에서 재생 | 깨짐. 벤드 chase 없음 | `play_from_middle_chases_pitch_bend` |
| 12 | 스냅샷의 마지막 소유자를 스케줄러로 두고 drop | 깨짐. 포트가 `keyscore-scheduler` 스레드에서 해제됨 | `drop_does_not_release_ports_on_scheduler_thread` |
| 13 | Note On 예약 뒤 정지 | 못 깨뜨림(`release_time`이 `last_sent` 뒤) | `stop_with_note_on_already_scheduled_releases_after_it` |
| 14 | 같은 음 겹침 뒤 정지 | 못 깨뜨림 | `stop_with_same_pitch_overlap_releases_note` |
| 15 | 120 → 60 BPM 스냅샷 교체 뒤 박 간격 | 못 깨뜨림(1 s ± 1 µs) | `swap_to_slower_tempo_keeps_new_grid_exact` |
| 16 | 0..480 루프 10바퀴 이상 | 못 깨뜨림(격자 오차 < 1 µs, 정지 뒤 걸린 노트 없음) | `loop_wrap_keeps_grid_exact_over_many_iterations` |
| 17 | `AllocDisabler`로 포트 1–3개·템포 60–179 BPM 교체 262회(돌려받기 큐 포화 → 대기 자리 → 회수), Clock 3포트, 루프, 벤드·CC chase, 패닉, 위치 이동 | 못 깨뜨림(중단 없음) | `review_s1_t4_alloc.rs`의 `scheduler_loop_does_not_allocate_when_garbage_queue_is_full` |
| 18 | 구현 테스트 `--test scheduler` 5회 반복 | 못 깨뜨림. 5/5 통과. NF-1 p99 1.079–1.120 ms, NF-3 p99 0.033–0.147 ms, NF-4 0.011–0.246 ms | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 스케줄러 루프에 할당·락·로그 없음 | 못 깨뜨림 | 시도 17. `rtrb` 0.4.0 push/pop/peek는 원자 연산과 슬롯 읽기·쓰기뿐(할당·락 없음) |
| 스냅샷은 스케줄러 스레드에서 해제되지 않는다 | 깨짐(종료 때) | 시도 12. 루프 안에서는 지켜진다 |
| 교체 뒤 옛 스냅샷 이벤트를 보내지 않는다(원자성) | 지켜짐 | 명령 하나로 `snap`이 바뀐다. 다만 교체 경계에서 중복·누락이 생긴다(시도 3, 5) |
| 노트가 걸리지 않는다 | 깨짐 | 시도 1, 2 |
| 시각 계산에 오차가 쌓이지 않는다(NF-4) | 깨짐(교체 때) | 시도 4. 재생·루프만으로는 지켜진다(시도 16) |
| 스케줄러 스레드가 패닉하지 않는다 | 깨짐 | 시도 8 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 9.2 | 부분 충족 | 루프 안 할당·락·로그 없음(시도 17). `mach_wait_until`은 「시스템 콜 금지」의 예외로 계획에만 적혀 있고 결정 기록이 없다(제안 3). 종료 때 포트 해제가 스케줄러 스레드에서 일어난다(`scheduler.rs:371-376`, 시도 12) |
| 9.3 | 충족 | 호스트 시각이 mach 클록 그대로다. 키 타임스탬프 변환 자체는 이 Task 범위 밖 |
| OUT-5 | 부분 충족 | SPP·첫 Clock 어긋남(`scheduler.rs:493-504`), 재생 중 SPP(`scheduler.rs:444-452`), 교체 때 Clock 중복(`scheduler.rs:524`) |
| OUT-6 | 부분 충족 | 메시지 자체는 맞다(`scheduler.rs:697-708`). 미리 예약된 Note On 뒤 걸림(시도 2), 교체가 막히면 패닉이 안 감(시도 9) |
| OUT-7 | 충족 | `send_to`가 미래 호스트 타임스탬프로 예약, 0은 `max(1)` |
| CTL-6 | 부분 충족 | 프로그램·뱅크·CC는 chase. 피치 벤드 없음, RPN 순서 뒤집힘(`scheduler.rs:653-694`) |
| TRN-3 | 부분 충족 | 교체는 원자적이지만 교체마다 격자가 밀리고(`scheduler.rs:522`), 예약된 노트가 사라지거나 Clock·이벤트가 중복된다(`scheduler.rs:514-524`) |
| NF-1 | 충족 | p99 약 1.1 ms(5회) |
| NF-3 | 충족(측정 방식 주의) | p99 0.033–0.147 ms. REQ는 「간격 표준편차」, 테스트는 「|간격 - 125 ms|의 p99」(제안 5) |
| NF-4 | 부분 충족 | 교체 없는 재생은 충족. 재생 중 편집(교체) 200회에 49.7 ms(시도 4) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 루프(`step_inner`) 안에는 없음. `rtrb`의 push/pop/peek, `TempoMap::tick_to_time`/`time_to_tick`(이진 탐색과 정수 연산), `with_event_list`(스택 `MIDIEventList`), `MidiError` 생성(힙 없는 변형)을 따라갔다. 시스템 콜은 `MIDIReceivedEventList`/`MIDISendEventList`, `mach_wait_until`. `mach_absolute_time`은 commpage 읽기다. 루프 밖 종료 경로(`run`의 quit 갈래 뒤 `Rt` drop)에서 스냅샷·포트·남은 명령의 `Arc`가 스케줄러 스레드에서 해제된다(지적 12).
- 제품 코드의 `unwrap`/`expect`: 없음(`unwrap_or`만). 대신 산술 넘침 패닉 경로가 있다: `scheduler.rs:462, 504, 524, 587, 593`(`div_ceil(40) * 40`, `+= 40`), `scheduler.rs:487`(`now() + start_margin`, 실제로는 안 넘침).
- unsafe: `rt.rs`의 SAFETY 주장(스택 출력 포인터, 정책 구조체 크기 = COUNT 4)은 헤더와 맞다. `mach_wait_until`을 `safe fn`으로 선언한 것도 인자가 값뿐이라 타당하다.
- 에러가 삼켜지는 곳: `send_to`의 송신 결과(계획대로), `make_realtime` 실패(보고서에 적힘), `Scheduler::drop`의 `join` 결과(스레드 패닉을 숨김, 지적 8과 함께 봐야 함), `swap`의 `else { return; }`(peek 검사로 닿지 않음).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | drop 때 `release_notes(now())`. 이미 미래로 넘긴 Note On이 Note Off 뒤에 도착해 걸린다. `release_time()`(= `last_sent` 뒤)을 써야 한다. drop 때 Clock Stop(FC)도 안 보낸다 | `scheduler.rs:373-375` |
| 2 | 수정 | 패닉이 `now`에 보내고 `active`를 비운다. 그 뒤 도착하는 예약 Note On은 이후 정지·위치 이동에서 꺼지지 않는다. 패닉 메시지를 `last_sent` 뒤에도 한 번 더 두거나, 표를 비우지 말고 Note Off를 `last_sent` 뒤로 보내야 한다 | `scheduler.rs:697-708` |
| 3 | 차단 | 교체(TRN-3) 경계가 틀린다. `release_notes(last_sent)`가 이미 예약한 Note On에 같은 시각 Note Off를 붙여 새·옛 스냅샷에 모두 있는 노트를 길이 0으로 지운다. `sent_tick = tick_at(last_sent)`는 내림이라 경우에 따라 이미 보낸 틱을 다시 보낸다(Clock 중복은 지적 5에서 재현). 새 템포가 빠르면 새 이벤트가 이미 보낸 옛 이벤트·Note Off보다 앞 시각에 예약될 수 있다 | `scheduler.rs:512-524` |
| 4 | 차단 | 교체 때 `set_base(now_tick, now)`. `now_tick`은 내림이라 기준점이 최대 한 틱(120 BPM에서 0.52 ms) 뒤로 밀리고 교체마다 쌓인다. 교체 200회에 49.7 ms. 재생 중 편집이 잦은 앱에서 NF-4를 넘는다. 옛 템포로 `host_of(now_tick)`을 구해 기준점으로 써야 한다 | `scheduler.rs:516, 522` |
| 5 | 수정 | 교체 때 `next_clock = ceil(sent_tick / 40) * 40`. `sent_tick`이 내림이라 이미 보낸 Clock을 다시 보낸다(최소 간격 0.07 ms). 받는 장치의 위치가 교체마다 한 Clock 앞선다 | `scheduler.rs:524` |
| 6 | 수정 | 16분음표에 맞지 않는 위치에서 재생하면 SPP는 내림(틱 0), 첫 Clock은 40틱 올림(틱 120). 받는 쪽은 Continue 뒤 첫 Clock을 SPP 위치로 본다. SPP를 올림하고 첫 Clock을 그 16분음표에 맞추거나, 첫 Clock 위치를 SPP에 맞춰야 한다 | `scheduler.rs:490-504` |
| 7 | 수정 | 재생 중 위치 이동이 Stop 없이 SPP·Continue를 보낸다. SPP는 정지 상태에서 받는 메시지라 장치가 무시할 수 있다. FC → F2 → FB 순서가 맞다. 루프 되감기 때도 Clock 장치의 위치를 다시 맞추지 않는다 | `scheduler.rs:444-452, 589-594` |
| 8 | 수정 | 공개 API `play`/`locate`에 큰 틱(`u64::MAX - 39` 이상)을 주면 `div_ceil(40) * 40`이 넘쳐 디버그에서 스케줄러 스레드가 패닉으로 죽는다. 이후 명령은 `Ok`로 받아지지만 처리되지 않고, drop은 `join` 에러를 버린다. 릴리스에서는 감싸 돌아 Clock이 틱 0부터 다시 센다. `saturating_*`/`checked_*`로 바꾸거나 위치 상한을 API에서 거부해야 한다 | `scheduler.rs:462, 504, 524, 587, 593` |
| 9 | 수정 | `collect_garbage`를 안 부르면 260번째 뒤 교체가 큐 머리에서 막히고, 뒤의 패닉(OUT-6)·정지도 처리되지 않는다. `panic()`은 `Ok`를 돌려준다. 문서에 호출 의무가 없다. 막힌 교체 뒤의 명령을 처리하거나(패닉은 별도 우선 신호), 큐 포화를 `swap`의 `Err`로 알려야 한다 | `scheduler.rs:393-399, 304-312` |
| 10 | 수정 | chase가 CC를 번호 순으로 보내 데이터 엔트리(6·38)가 RPN/NRPN 선택(101·100·99·98)보다 먼저 간다. 피치 벤드 범위 같은 값이 다른 파라미터에 들어간다. 선택 CC를 데이터 엔트리 앞에 보내야 한다 | `scheduler.rs:683-693` |
| 11 | 수정 | chase가 피치 벤드(CTL-4 레인)와 채널 프레셔를 보내지 않는다. CTL-6의 「현재 위치의 값」은 레인 전체로 읽힌다. 범위 밖이라고 보면 `docs/decisions/`에 결정 요청을 남겨야 한다 | `scheduler.rs:653-667` |
| 12 | 수정 | 스레드 종료 때 `Rt`(현재 스냅샷, 대기 자리, 큐에 남은 `Swap`)가 스케줄러 스레드에서 drop된다. 스케줄러가 마지막 소유자면 포트 해제(락, `MIDIEndpointDispose`)가 그 스레드에서 일어난다. `MidiPort` 문서(`port.rs:7-8`)의 계약 위반이다. 종료 때 스냅샷을 돌려받기 큐로 넘기거나 `Scheduler`가 현재 스냅샷의 복제를 들고 있어야 한다 | `scheduler.rs:371-376` |
| 13 | 제안 | 교체마다 울리는 모든 노트를 끈다. 재생 중 다른 트랙의 노트 하나만 고쳐도 모든 지속음이 끊긴다. 바뀌지 않은 노트는 이어지게 하는 방식(새 스냅샷에서 아직 울려야 할 노트는 끄지 않음)을 검토하고, 지금 방식을 유지하면 TRN-3 해석을 결정 문서로 남긴다 | `scheduler.rs:519` |
| 14 | 제안 | chase가 재생 시작 때마다 `from` 앞의 모든 이벤트를 스케줄러 스레드에서 훑는다. 긴 곡(수십만 이벤트)에서 한 바퀴가 1 ms 주기를 넘어 첫 이벤트가 늦을 수 있다. chase 값을 스냅샷에 미리 계산해 두는 편이 낫다 | `scheduler.rs:655-667` |
| 15 | 제안 | `mach_wait_until`을 9.2 「시스템 콜 금지」의 예외로 쓰는 것은 계획에만 적혀 있다. REQ와 어긋나는 해석이므로 `docs/decisions/`에 남긴다 | `rt.rs:63-65` |
| 16 | 제안 | NF-3을 REQ 정의(송신 간격 표준편차)로도 계산해 함께 적는다. NF-4는 도착 시각 첫·끝 차라 수신 지터(최대 0.25 ms 관찰)가 섞인다. 패킷 타임스탬프 기준 값을 같이 적으면 진짜 드리프트와 구분된다 | `tests/scheduler.rs:206-226` |
| 17 | 제안 | 루프 끝 판정이 `end > base_tick`이라, 재생 위치가 이미 루프 끝 뒤일 때 루프를 켜면 즉시 되감길지가 마지막 기준점(재생 시작인지 교체인지)에 따라 달라진다. 규칙을 하나로 정한다 | `scheduler.rs:552` |
| 18 | 제안 | 정지 상태의 `stop`은 아무것도 하지 않아 `send_now`로 켠 노트가 남는다(S1-T5 미리듣기와 엮임). `stopped_at`은 쓰이지 않는다 | `scheduler.rs:435-452` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 2건(지적 3, 4)은 이 Task의 리뷰 초점인 「재생 중 모델이 바뀔 때(TRN-3) 스냅샷 교체」에서 나왔다. 교체 자체는 원자적이지만, 경계 계산이 노트를 지우고 격자를 누적해서 민다. 수정 10건은 걸린 노트(1, 2), Clock(5, 6, 7), 스레드 생존(8, 9), chase(10, 11), 종료 경로(12)다. 실시간 루프의 할당·락·로그는 찾지 못했다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`encode.rs`의 crate 내부 생성자, `time.rs`의 `duration_since`, `deny.toml` BSD-1-Clause는 보고서에 적힌 Task 관련 변경)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`rtrb` MIT/Apache-2.0, `assert_no_alloc` BSD-1-Clause: REQ 8.4 BSD 계열로 허용 목록에 추가됨, `cargo deny` 통과는 보고서 기준)
- [x] 스타일 지적을 적지 않았다
