# 적대적 리뷰: S1-T4 스케줄러 스레드 (리뷰 3회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `3909133` (브랜치 `s1/t4-scheduler`). 리뷰 2 뒤 코드 변경은 `54be828` 하나(`git diff 2849f05 HEAD -- crates/`는 `scheduler.rs`뿐)
- 남긴 테스트: `crates/keyscore-midi/tests/review_s1_t4_r3.rs`(실패 4, 통과 9)
- 범위(좁힘): A. 리뷰 2 수정 지적 1–8의 해소와 그 주변 변형, B. 수정이 들인 회귀(기존 테스트, 실시간 루프의 할당·락·로그), C. 제안 9·10·12 미반영의 수용 여부. 이 밖의 것은 제안으로만 적는다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 지적 3 수정(`release_unless_continued`의 `ptr::addr_eq`)은 재생 중 교체 경로에만 있다. 정지 중 교체(`swap`의 `!self.playing` 분기)는 스냅샷만 바꾼다. 미리듣기로 옛 포트에 켠 노트가 새 포트로 꺼지거나(같은 번호, 다른 포트) 잊히지 않는가(포트 수 감소 + `active.fill(0)`). 리뷰 2가 이 경로를 명시했다.
2. 지적 4 수정(`panic_after`와 `popped` 비교)은 「패닉 앞의 명령을 먼저」만 보장한다. 명령 루프는 패닉 뒤에 넣은 명령까지 꺼낸 뒤 패닉을 적용하므로, 패닉 뒤 미리듣기 노트가 패닉의 Note Off로 꺼지지 않는가.
3. 지적 6 수정의 `MAX_ITEMS_PER_PASS`(4096)로 `schedule`이 일찍 끝나도 `sent_through`는 `tick_at(horizon)`으로 잡힌다. 상한에 걸린 바퀴 다음의 교체가 남은 이벤트를 「보냄」으로 건너뛰지 않는가(지적 1·2 수정과의 상호작용).
4. `sent_through = tick_at(horizon)`은 정확한 시각이 `horizon` 이하인 틱이다. 그런데 이벤트는 `host_of`(내림) ≤ `horizon`이면 보낸다. 정확한 시각이 (`horizon`, `horizon` + 1) 안의 틱은 보냈는데 `sent_through` 뒤라 교체 뒤 다시 보내지 않는가(중복).
5. 위치 이동과 교체가 같은 바퀴일 때(`set_base`가 `sent_through = None`) 새 위치의 이벤트·Clock이 한 번씩 가는가.
6. `u64::MAX` 근처에서 Clock + 루프, 호스트 틱보다 짧은 루프 + Clock에서 스레드가 명령을 받고 drop이 끝나는가.
7. 템포 맵이 다른 스냅샷을 번갈아 교체할 때 `move_base`(옛 템포)와 `base_time` 재계산(새 템포)이 정확한가.
8. RPN → NRPN → RPN 순서에서 chase가 마지막 선택을 데이터 엔트리 바로 앞에 두는가.
9. 새 코드에 할당·락·로그가 있는가(`AtomicU64`, `u128` 나눗셈, `for _ in 0..MAX_ITEMS_PER_PASS`).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 정지 중 교체 + 포트 바뀜 | 없음 | 리뷰 2 테스트는 재생 중 교체만 |
| 2 패닉 뒤 명령 | 없음 | 리뷰 2 테스트는 패닉 앞 명령만 |
| 3 상한에 걸린 바퀴 + 교체 | 없음 | |
| 4 내림 경계의 중복 | 없음 | 결정적으로 만들 방법을 찾지 못해 시도하지 않았다(제안 6) |
| 5 위치 이동 + 교체 같은 바퀴 | 없음 | 리뷰 2 테스트는 재생 + 교체 |
| 6 큰 틱 + 루프, 짧은 루프 + Clock | 부분 | 리뷰 2 테스트는 각각 루프 없음, Clock 없음 |
| 7 템포 맵이 다른 교체 반복 | 부분 | 리뷰 2 테스트 8은 같은 스냅샷 반복 |
| 8 RPN → NRPN → RPN | 부분 | 리뷰 2 테스트 7은 RPN → NRPN |
| 9 할당 | 있음 | `review_s1_t4_alloc.rs`, `scheduler_alloc.rs` |

## 3. 깨뜨리기 시도

모든 테스트는 이 파일 안의 `MidiPort` 구현(기록·관문·세는 포트)을 쓴다. CoreMIDI 클라이언트를 만들지 않는다. 실패 4개는 3회 연속 같은 결과였다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 정지 중 옛 포트 A에 `send_now(Note On)`, 같은 번호 0에 포트 B가 온 스냅샷으로 교체, `stop` | 깨짐. A는 Note On만 받고, Note Off는 B로 간다 | `swap_while_stopped_to_different_port_releases_preview_note_on_old_port` |
| 2 | 정지 중 포트 [A, B]의 B에 미리듣기 노트, [A]로 교체, `stop` | 깨짐. B는 Note Off를 받지 못한다(`active.fill(0)`이 잊음) | `swap_while_stopped_to_fewer_ports_releases_preview_note_on_removed_port` |
| 3 | 관문으로 붙잡은 바퀴에서 `panic()` 뒤 `send_now(Note On)` | 깨짐. Note On 뒤 패닉 CC 48개와 Note Off(61)가 간다. 패닉 뒤에 친 노트가 꺼진다 | `panic_does_not_silence_note_sent_after_it` |
| 4 | 틱 0에 CC 6000개(첫 항목은 관문 포트), `play(0)`의 첫 바퀴를 붙잡고 같은 내용으로 `swap` | 깨짐. 4095개만 간다. 상한(4096)에서 끊긴 나머지 1905개를 교체가 건너뛴다 | `swap_after_pass_capped_at_item_limit_loses_no_events` |
| 5 | 재생 중 포트 [A, B]에서 B의 노트가 울릴 때 [A]로 교체 | 못 깨뜨림. B로 Note Off | `swap_during_play_to_fewer_ports_releases_note_on_removed_port` |
| 6 | 같은 포트 객체를 둔 교체(지적 3 판정의 대조군) | 못 깨뜨림. 끊기지 않고 원래 끝(500 ms ± 1 µs)에서 꺼짐 | `swap_with_same_port_object_keeps_sounding_note` |
| 7 | 같은 바퀴에 `locate(960)`과 `swap` | 못 깨뜨림. 틱 960의 Note On 한 번 | `swap_in_same_wake_as_locate_keeps_event_at_new_position` |
| 8 | 7과 같은데 Clock 켬 | 못 깨뜨림. Stop, SPP(4), Continue, 첫 Clock이 같은 시각 | `swap_in_same_wake_as_locate_keeps_first_clock_with_continue` |
| 9 | 한 바퀴 처리량(64)을 넘는 명령 101개 뒤 패닉 | 못 깨뜨림. 패닉이 다음 바퀴까지 기다려 마지막 Note On도 끈다 | `panic_after_more_than_one_wake_of_commands_silences_last_note` |
| 10 | Clock 켜고 루프 [`u64::MAX` - 60, `u64::MAX` - 1)에서 재생 | 못 깨뜨림. `send_now` 처리, 200 ms에 Clock 50개 미만, drop 2 s 안 | `play_near_max_tick_with_clock_and_loop_stays_responsive` |
| 11 | 템포 1 µs/4분음표, 루프 [0, 1), Clock 켬 | 못 깨뜨림. `send_now` 처리, drop 2 s 안 | `loop_shorter_than_one_host_tick_with_clock_keeps_scheduler_responsive` |
| 12 | 틱 0에 RPN(0,0)=2, 10에 NRPN(1,8)=100, 20에 RPN(0,0)=12, 960에서 재생 | 못 깨뜨림. RPN(0,0)=12, 다른 파라미터에 잘못된 값 없음 | `chase_with_rpn_nrpn_rpn_writes_no_wrong_parameter` |
| 13 | 고정 120 BPM 맵과 먼 뒤(박 1000)에서 61 BPM으로 바뀌는 맵을 약 1.1 ms마다 번갈아 1000번 교체 | 못 깨뜨림. 박 격자 오차 1 µs 미만 | `many_swaps_alternating_tempo_maps_keep_beat_grid_exact` |
| 14 | 리뷰 2 테스트(`review_s1_t4_r2.rs`) | 못 깨뜨림. 14개 모두 통과(리뷰 2의 실패 9개 해소) | 없음 |
| 15 | `cargo test -p keyscore-midi --no-fail-fast` 3회 | 기존 테스트 3/3 통과. 세 번째 실행에는 이 파일이 들어가 위 실패 4개만 실패 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 스케줄러 루프에 할당·락·로그 없음 | 못 깨뜨림 | 새 코드는 `AtomicU64` load/store, `u128` 정수 산술(`__udivti3`는 할당·시스템 콜 없음), 고정 횟수 `for`뿐이다. `Vec` 증가·`Mutex`·`println!`·`format!` 없음. 할당 테스트 2개 통과 |
| 교체 뒤 이벤트가 빠지거나 겹치지 않는다(TRN-3) | 깨짐 | 시도 4(상한에 걸린 바퀴 뒤 교체). 재생·위치 이동·두 번 교체·템포 변경 경로는 못 깨뜨림(시도 7, 13, 리뷰 2 테스트) |
| 노트가 걸리지 않는다 | 깨짐 | 시도 1, 2(정지 중 교체). 재생 중 교체는 못 깨뜨림(시도 5, 6) |
| 시각 계산에 오차가 쌓이지 않는다(NF-4) | 못 깨뜨림 | 시도 13, 리뷰 2 테스트 8 |
| 스케줄러 스레드가 멈추지 않는다 | 못 깨뜨림 | 시도 10, 11, 리뷰 2 테스트 5·6 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 9.2 | 충족 | 루프 안 할당·락·로그 없음(`scheduler.rs:551-578, 779-843`) |
| TRN-3 | 부분 충족 | 상한에 걸린 바퀴 뒤 교체가 이벤트를 버림(`scheduler.rs:781, 839-842, 759-761`), 정지 중 교체가 옛 포트의 노트를 놓침(`scheduler.rs:736-741, 882-897`) |
| OUT-5 | 충족 | 리뷰 2 지적 1·5 해소, 시도 8, 10, 11 |
| OUT-6 | 충족 | 패닉 메시지와 순서(앞 명령 먼저)는 맞다. 뒤 명령까지 꺼지는 것(지적 2)은 OUT-6 문구 위반은 아니고 순서 버그다 |
| CTL-6 | 충족 | 재생 시작 chase의 선택 순서(시도 12, 리뷰 2 테스트 7) |
| NF-4 | 충족 | 교체 누적 0(시도 13, 리뷰 2 테스트 8) |

C. 제안 9·10·12 미반영: 셋 다 REQ를 어기지 않는다. CTL-6은 「재생 시작 시」 chase를 요구하고 루프 되감기는 문구 밖이다(9). RPN 널 패턴과 파라미터별 값(10)은 CTL-6 chase의 정밀도 문제로, 보고서대로 S6 컨트롤러 레인에서 다룰 수 있다. 색인 메모리(12)는 REQ ID에 걸리지 않는다. 문서화된 한계로 받아들인다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 1번 표 9 참고.
- 제품 코드의 `unwrap`/`expect`: 없음. `unwrap_or`만 새로 들어왔다(`base_host`, `host_of`의 `u64::try_from`).
- 에러가 삼켜지는 곳: 리뷰 2와 같다(`send_to` 결과, `make_realtime`, `join` 결과).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 2 지적 3 일부 미해소. 정지 중 교체는 스냅샷만 바꾸므로, 미리듣기로 켠 노트가 같은 번호의 다른 포트로 꺼지거나(옛 포트에 걸림), 포트 수가 줄면 `release_notes`의 `active.fill(0)`이 잊는다. 리뷰 2가 「정지 중 교체도 같은 문제」라고 적은 경로인데 보고서는 재생 중 경로만 고쳤다. 정지 중 교체에도 울리는 노트가 있으면 `release_unless_continued`(또는 옛 포트로 끄기)를 적용하라. 테스트: `swap_while_stopped_to_different_port_releases_preview_note_on_old_port`, `swap_while_stopped_to_fewer_ports_releases_preview_note_on_removed_port` | `scheduler.rs:736-741, 882-897` |
| 2 | 수정 | 리뷰 2 지적 4 수정의 반대 방향 회귀. 명령 루프가 `panic_after`에 닿아도 멈추지 않고 패닉 뒤에 넣은 명령까지 꺼낸 뒤 패닉을 적용한다. 패닉 뒤에 친 미리듣기 노트가 패닉의 Note Off로 꺼진다. 패닉이 걸려 있으면 `popped == panic_after`에서 꺼내기를 멈추고 패닉을 적용한 뒤 이어서 꺼내라. 테스트: `panic_does_not_silence_note_sent_after_it` | `scheduler.rs:554-574` |
| 3 | 수정 | 리뷰 2 지적 6 수정(`MAX_ITEMS_PER_PASS`)과 지적 1·2 수정(`sent_through`)의 상호작용 회귀. 상한에 걸려 `schedule`이 일찍 끝나도 `sent_through = tick_at(horizon)`이라, 다음 교체가 남은 이벤트를 건너뛴다(틱 0의 6000개 중 1905개 손실). Clock도 코드상 같은 식으로 `clock_after(t + 1)`에서 건너뛴다(테스트하지 않음). 상한에서 끝났으면 `sent_through`를 마지막으로 보낸 항목의 틱 바로 앞(같은 틱의 남은 이벤트가 있으므로)으로 두거나, 교체가 커서 위치를 그대로 쓰게 하라. `scheduled_until`도 창 끝으로 올리지 않는 편이 맞다. 테스트: `swap_after_pass_capped_at_item_limit_loses_no_events` | `scheduler.rs:781, 839-842, 759-761` |
| 4 | 제안 | 패닉 플래그가 하나라, 스케줄러가 `panic`을 읽은 뒤 `false`로 쓰기 전에 두 번째 `panic()`이 오면 두 번째 패닉이 첫 패닉에 합쳐진다. 그 사이 넣은 명령은 패닉보다 늦게 처리될 수 있다. `compare_exchange`나 패닉 순번(카운터)으로 바꾸면 없어진다. 테스트로 재현하지 않았다(코드 읽기) | `scheduler.rs:569-573, 386-389` |
| 5 | 제안 | 데이터 엔트리 값이 파라미터별이 아니라서, 곡이 NRPN에 값을 쓴 뒤 데이터 없이 RPN만 고르면 chase가 그 NRPN 값을 RPN에 쓴다. 리뷰 2 제안 10과 같은 뿌리이므로 S6에서 함께 보면 된다. 테스트로 남기지 않았다 | `scheduler.rs:963-970` |
| 6 | 제안 | `sent_through = tick_at(horizon)`(정확한 시각 ≤ `horizon`)과 송신 조건 `host_of(tick)`(내림) ≤ `horizon`이 한 호스트 틱 안에서 어긋난다. 정확한 시각이 (`horizon`, `horizon` + 1 호스트 틱) 안인 틱의 이벤트는 보냈는데 `sent_through` 뒤로 남아, 그 직후 교체에서 한 번 더 갈 수 있다. 확률은 교체당 약 41 ns / 틱 길이(120 BPM에 약 10^-4)다. 결정적으로 재현하지 못했다(코드 읽기). 3을 고칠 때 「마지막으로 보낸 항목의 틱」을 쓰면 함께 사라진다 | `scheduler.rs:813-816, 839-842` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2의 수정 8건 중 2, 5, 6, 7, 8은 리뷰 2 테스트와 주변 변형(위치 이동 + 교체, 큰 틱 + 루프, 짧은 루프 + Clock, RPN → NRPN → RPN, 템포 맵이 다른 교체 1000번)에서 깨지지 않았다. 지적 1도 재생·위치 이동 경로에서 해소됐다. 실시간 루프에 할당·락·로그는 들어오지 않았고 기존 테스트는 3회 모두 통과했다. 제안 9·10·12의 미반영은 REQ를 어기지 않아 문서화된 한계로 받아들인다.

남는 수정은 셋이다. 지적 3의 정지 중 경로가 빠졌고(1), 지적 4 수정이 반대 방향 순서를 뒤집었고(2), 지적 6의 상한이 지적 1·2의 `sent_through`와 맞물려 교체 때 이벤트를 버린다(3). 모두 범위가 좁고 해당 함수 안에서 고칠 수 있다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(리뷰 2 테스트 파일은 보조 포트 양식을 가져오려고 1번 뒤에 읽었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(3회, 같은 결과)
- [x] REQ ID를 하나씩 대조했다(9.2, TRN-3, OUT-5, OUT-6, CTL-6, NF-4)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 2 뒤 변경은 `scheduler.rs`, 보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 1·2 테스트 파일 변경 없음)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
