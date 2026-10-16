# 적대적 리뷰: S1-T5 메트로놈과 미리듣기 (리뷰 2회차)

- 리뷰어 세션: 구현 세션·1회차 리뷰 세션과 다른 새 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `193c7a2` (브랜치 `s1/t5-metronome-preview`). 1회차 뒤 코드 변경은 `105270a..HEAD`의 `scheduler.rs`뿐이다.
- 범위: 확인 리뷰. A) 1회차 지적 1의 해결과 그 주변 변형, B) 회귀와 실시간 규율, C) 거절한 제안 3·4의 REQ 위반 여부. 그 밖은 제안까지만 적는다.
- 리뷰어 테스트: `crates/keyscore-midi/tests/review_s1_t5_r2.rs` (기록 포트 `Rec`만 쓴다)
- 검사한 불변식: (포트, 채널, 노트)마다 타임스탬프 순(같으면 넘긴 순)으로 Note On과 Note Off가 번갈아 오고, 끝 상태는 꺼짐이며, 패닉이 없다. 울리는 중에 다시 오는 Note On은 위반으로 센다(앞 노트의 Note Off가 새 노트를 끊는다). 이미 꺼진 노트의 Note Off는 무해하므로 세지 않는다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 105270a..HEAD -- crates/`와 `play_note`, `free_earliest_off`, `flush_offs`, `release_notes`, `forget_offs_on_changed_ports`, `click`만 읽고 적었다.

1. 다시 켜기: 넘기지 않은 끄기를 `min(off.host, host).max(off.on_host)`에 보내고 새 Note On을 `max(host, at)`에 둔다. 같은 시각이면 넘긴 순서(Off, On)로 맞는가.
2. 미래로 미룬 미리듣기 Note On(넘긴 끄기 뒤)의 대기를 짧은 미리듣기로 다시 켤 때 `on_host`가 미룬 시각이라 끄기가 그 Note On 뒤에 오는가.
3. 표 넘침(`free_earliest_off`): 가장 이른 칸을 고른다. 넘긴(sent) 칸은 시각이 가장 이르므로 먼저 뽑힌다. 그 칸을 버리면 「넘긴 끄기 뒤로 새 Note On을 미룬다」는 기억이 사라진다. 그 뒤 같은 노트를 다시 켜면 새 Note On이 넘긴 Note Off 앞에 간다.
4. 표 넘침 중 클릭 Note On이 미래에 있을 때 앞당긴 끄기가 `on_host` 뒤에 가는가.
5. 정지·패닉·위치 이동(`release_notes`): 넘기지 않은 칸을 버리고 울리는 노트를 `max(now, last_sent)`에 끈다. 그 Note Off는 대기 표에 남지 않는다. lookahead로 미래 클릭 Note On이 넘어가 있으면 정지의 Note Off도 미래다. 그 앞에서 같은 노트를 미리듣기하면 새 Note On이 지금에 나가고 정지의 Note Off가 그 노트를 끊는다.
6. 같은 노트로 박마다 다시 켜지는 클릭(강박=약박)이 박 간격보다 길 때 교대가 맞는가.
7. 실시간 경로: 바뀐 코드는 `u64` 필드 하나와 `max` 연산뿐인가. 할당·락·로그가 없는가.
8. `preview` 포트 검사(제안 2 반영)가 다른 명령과 같은 에러를 내는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 클릭과 같은 노트 미리듣기 | 있음 | 1회차 리뷰어 테스트 2개(`review_s1_t5.rs`)가 이제 통과한다 |
| 2 미룬 Note On의 다시 켜기 | 없음 | 이번에 추가 |
| 3 넘긴 칸이 넘침으로 버려진 뒤 다시 켜기 | 없음 | 이번에 추가 |
| 4 클릭이 미래일 때 표 넘침 | 일부 | 1회차 `preview_overflow_while_clicking_leaves_no_note_sounding`은 끝 상태만 본다 |
| 5 정지 뒤 미래 클릭 앞 미리듣기 | 없음 | 1회차 `stop_panic_locate_mid_click_leave_no_note_sounding`은 미리듣기 없이 끝 상태만 본다 |
| 6 같은 노트 긴 클릭 | 일부 | 1회차 `click_longer_than_beat_stays_on_grid`는 37·42를 번갈아 쓴다 |
| 7 할당 | 있음 | `metronome_alloc.rs` |
| 8 `preview` 포트 검사 | 없음 | 코드로 확인(`scheduler.rs:499`), 제안이라 테스트를 더하지 않았다 |

## 3. 깨뜨리기 시도

모두 `cargo test -p keyscore-midi --test review_s1_t5_r2`로 3회 이상 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | lookahead 500 ms, 정지 상태. 넘긴 끄기 뒤로 미룬 미리듣기 Note On(끄기 2 s, 대기)을 1 ms 미리듣기로 거듭 다시 켬. 8바퀴, 길이·간격을 바꿈 | 못 깨뜨림 | `retrigger_of_delayed_preview_keeps_on_off_alternating` |
| 2 | lookahead 500 ms, 120 BPM 클릭 중. 박 1 Note On이 미래일 때 서로 다른 노트 100개를 3 s로 미리듣기해 64칸을 넘침, 그 뒤 정지 | 못 깨뜨림 | `overflow_while_click_note_on_is_in_future_keeps_alternating` |
| 3 | 강박·약박 모두 42, 클릭 1 s, 박 250 ms(240 BPM). lookahead 기본값과 500 ms | 못 깨뜨림 | `same_note_click_longer_than_beat_keeps_alternating` |
| 4 | 클릭 42(30 ms)와 같은 노트를 7 ms 간격으로 120번, 길이 1·3·20·90·300 ms를 돌려 가며 미리듣기. 중간에 정지·재생·패닉·위치 이동. lookahead 기본값과 120 ms | 깨짐(간헐). 3회 중 1회, 단독 8회 중 1회 실패. 끝 상태는 꺼짐, 교대 위반 1건 | `preview_of_click_note_sweep_with_stop_and_panic_keeps_alternating` |
| 5 | 표 넘침이 넘긴 끄기 칸을 버린 뒤 같은 노트를 다시 켬: 노트 60을 100 ms로 미리듣기(끄기가 창 안이라 넘어감), 다른 노트 64개로 넘침, 노트 60을 300 ms로 다시 미리듣기 | 깨짐(3회 모두). 끝 상태는 꺼짐, 새 Note On이 넘긴 Note Off 앞 | `retrigger_after_overflow_evicts_sent_off_keeps_alternating` |
| 6 | lookahead 500 ms, 120 BPM 클릭. 재생 20 ms 뒤 정지(박 1의 Note On 42가 약 480 ms 뒤로 이미 넘어감), 10 ms 뒤 42를 1 s로 미리듣기 | 깨짐(3회 모두). 끝 상태는 꺼짐, 교대 위반 1건 | `preview_after_stop_with_future_click_keeps_alternating` |

깨짐의 원인(코드와 실패 기록으로 확인).

- 시도 6: 정지(`release_notes`, `scheduler.rs:1201`)가 노트 42를 `release_time()` = `max(now, last_sent)` = 미래 클릭 Note On 시각에 끄고, 그 클릭의 넘기지 않은 끄기 칸을 버린다(`scheduler.rs:1219`). 정지의 Note Off는 대기 표에 남지 않는다. 그래서 그 뒤 미리듣기는 같은 노트 칸을 찾지 못해 Note On을 지금에 보낸다. 타임스탬프 순서는 미리듣기 On(지금), 클릭 On(약 480 ms), 정지 Off(같은 시각), 미리듣기 Off(1 s 뒤)다. 미리듣기 노트가 설정한 길이(PRE-1)보다 일찍 끊기고, 끝의 미리듣기 Off는 짝이 없다.
- 시도 4: 같은 뿌리다. 실패 기록에서 미래로 미룬 Note On·넘긴 Note Off 묶음(약 90 ms 뒤) 뒤에 클릭이 같은 칸을 덮어쓰고(넘기지 않은 끄기), 곧 정지·위치 이동이 그 칸을 버린 뒤 같은 노트 미리듣기가 지금에 Note On을 보냈다. 칸이 노트마다 하나라 가장 마지막 사건만 기억하고, 그 칸을 `release_notes`가 버리면 그보다 앞서 넘긴 미래 사건의 기억도 사라진다.
- 시도 5: `free_earliest_off`(`scheduler.rs:1110`)는 `host`가 가장 이른 칸을 고른다. 넘긴 칸은 시각이 창 안이라 언제나 가장 이르므로 넘침 때 먼저 버려진다. 그 뒤 같은 노트의 `play_note`는 `host.max(off.host)`로 미룰 근거를 잃고 Note On을 지금에 보낸다. `play_note` 주석 「그래야 새 노트가 앞 노트의 Note Off로 끊기지 않는다」가 깨진다. 다만 넘긴 칸은 lookahead 창 안에서만 살아 있으므로, 기본 lookahead(10 ms)에서는 10 ms 안에 서로 다른 노트 64개가 대기해야 생긴다.
- 세 경우 모두 끝 상태는 꺼짐이다. 1회차 지적 1처럼 노트가 남지는 않는다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 미리듣기·클릭 노트는 Note Off 없이 남지 않는다(PRE-1, Task 테스트 게이트) | 아니오 | 1회차 시도 1·2 통과, 이번 시도 1~6 모두 끝 상태 꺼짐 |
| Note Off가 짝인 Note On보다 앞서지 않는다(지적 1) | 아니오 | 시도 1·2. `on_host`로 해결 |
| 같은 노트의 Note On·Off가 타임스탬프 순으로 번갈아 온다 | 예 | 시도 6(정지 뒤 미래 클릭 앞 미리듣기), 시도 4(간헐, 같은 뿌리), 시도 5(표 넘침이 넘긴 칸을 버림) |
| 패닉 없음 | 아니오 | 시도 1~6에서 스레드가 계속 돌았다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| MET-1 | 충족 | 1회차와 같다. 이번 변경은 클릭 송신 시각만 건드린다(`scheduler.rs:1077-1092`) |
| MET-2 | 충족(이 Task 범위) | 박 큐는 1회차와 같다. REQ 문구(상태줄 박자 표시, 테두리 깜빡임, 카운트인을 볼 수 있음)는 지연·손실 허용치를 정하지 않는다. 거절한 제안 3(가득 차면 새 박을 버림)은 UI가 비우는 주기에 달린 한계로, S2 UI 연결 때 정하겠다는 대응이 REQ를 어기지 않는다 |
| MET-3 | 충족 | 1회차와 같다 |
| PRE-1 | 부분 충족(미충족으로 셈) | 노트가 남지는 않는다. 그러나 정지 직후(lookahead로 미래 클릭이 넘어가 있을 때) 같은 노트 미리듣기는 정지의 Note Off에 끊겨 설정한 길이를 지키지 못한다(시도 6, `scheduler.rs:1219`) |
| REQ 9.2 스케줄러 스레드 할당·락 금지 | 충족 | 아래 6절. 거절한 제안 4(박자표 변화 선형 탐색)는 할당·락이 없고 박마다 한 번이라 이 ID를 어기지 않는다 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `105270a..HEAD`의 변경은 `PendingOff::on_host`(`u64`) 필드와 `max` 두 곳, `preview`(호출자 스레드)의 범위 검사뿐이다. `metronome_alloc.rs`가 3회 모두 통과했다.
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: 1회차와 같다(`click`의 박 큐 푸시, `send_to`의 포트 실패).
- 회귀: `cargo test -p keyscore-midi --no-fail-fast` 3회. 이번 파일(`review_s1_t5_r2`)의 깨짐 테스트를 뺀 모든 테스트가 3회 모두 통과했다(1회차 리뷰어 테스트 11개 포함). `#[ignore]` 2개는 이 Task 전부터 있던 것이고 `105270a..HEAD`에서 바뀌지 않았다. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` 통과.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 정지·패닉·위치 이동의 `release_notes`가 넘기지 않은 끄기 칸을 버리고, 자기가 미래 시각(`max(now, last_sent)`)에 보낸 Note Off를 기억하지 않는다. lookahead로 미래 클릭 Note On이 넘어가 있을 때 그 시각 앞에서 같은 노트를 미리듣기하면 새 Note On이 지금에 나가 정지의 Note Off에 끊긴다(교대 위반, PRE-1 길이 미준수). 끝 상태는 꺼짐. 깨뜨리는 테스트: `preview_after_stop_with_future_click_keeps_alternating`(결정적), `preview_of_click_note_sweep_with_stop_and_panic_keeps_alternating`(간헐, 같은 뿌리). 고칠 방향 예: 버리는 대신 그 칸을 `sent`, `host = at`으로 바꿔 두어 `play_note`가 `host.max(off.host)`로 미루게 한다 | `crates/keyscore-midi/src/scheduler.rs:1201-1223` |
| 2 | 제안 | 표 넘침 때 `free_earliest_off`가 넘긴 칸(시각이 가장 이름)을 먼저 버려, 그 노트를 다시 켜면 새 Note On이 넘긴 Note Off 앞에 간다. lookahead 창 안에 서로 다른 노트 64개가 대기해야 생겨 기본 설정에서는 현실적이지 않다. 넘기지 않은 칸을 먼저 고르고, 넘긴 칸만 남았을 때 가장 이른 것을 버리면 대부분 막힌다. 깨뜨리는 테스트: `retrigger_after_overflow_evicts_sent_off_keeps_alternating` | `crates/keyscore-midi/src/scheduler.rs:1110-1130` |

1회차 지적 1은 해결됐다. 1회차 제안 2는 반영됐다. 거절한 제안 3·4는 REQ ID를 어기지 않으므로 문서화한 한계로 받아들인다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(수정)이 이번 확인 범위의 불변식(교대)과 PRE-1의 길이를 깨므로 수정 뒤 재리뷰한다. 지적 2는 제안이라 거절할 수 있으나, 거절하면 `retrigger_after_overflow_evicts_sent_off_keeps_alternating`을 어떻게 다룰지(기대를 바꾸는 근거)를 보고서에 적는다. 리뷰어 테스트 6개 가운데 3개는 회귀 테스트로 남는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(3회 이상. 시도 5·6은 매번, 시도 4는 간헐)
- [x] REQ ID를 하나씩 대조했다(MET-1~3, PRE-1, REQ 9.2)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`105270a..HEAD`는 `scheduler.rs`와 보고서뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
