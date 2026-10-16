# 적대적 리뷰: S1-T5 메트로놈과 미리듣기 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `09deb75` (브랜치 `s1/t5-metronome-preview`, `main..HEAD`)
- 리뷰어 테스트: `crates/keyscore-midi/tests/review_s1_t5.rs` (파일 안의 기록 포트 `Rec`만 쓴다. CoreMIDI 클라이언트를 만들지 않는다)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트 파일(`tests/metronome.rs`, `tests/metronome_alloc.rs`)을 열기 전에 `src/metronome.rs`, `src/scheduler.rs` diff만 읽고 적었다.

1. `play_note`의 다시 켜기: 아직 넘기지 않은 끄기를 `min(off.host, host)`에 보낸다. 그 끄기의 짝인 Note On이 미래 시각(lookahead 창 안에 미리 넘긴 클릭)이면, 지금(`now()`)에 들어온 미리듣기가 그 Note On보다 앞에 Note Off를 놓는다. 미래의 클릭 Note On 뒤에 끄기가 없어져 노트가 남을 수 있다. 울리는 노트 수(`active`)는 0으로 세어져 정지도 못 끈다.
2. 박 격자: 박자표 변화가 마디 중간(짧은 마디)일 때 `beat_at_or_after`가 `min(다음 박, 다음 변화)`를 고르는가. 변화 틱에서 강박인가.
3. 루프: 루프 끝 틱과 같은 박은 다음 바퀴 것(울리지 않음), 되감은 뒤 루프 시작 이상 첫 박부터. 박 위가 아닌 루프 시작.
4. 교체: 박자표를 바꾼 스냅샷으로 교체할 때 `last_click + 1`부터 새 박자표로 다시 찾는다. 박이 빠지거나 두 번 울리지 않는가. 격자가 어긋나지 않는가.
5. 같은 틱 순서: 이벤트, 클릭, Clock. `peek_next`에서 클릭은 `<=`, Clock은 `<`로 비교한다.
6. 틱 범위 끝(`u64::MAX` 근처): `beat_at_or_after`의 `checked_add`, `click`의 `saturating_add(1)`, 루프·교체와 겹칠 때 패닉 없음.
7. 끄기 대기 표 64칸: 넘침 때 `free_earliest_off`가 가장 이른 대기를 앞당겨 보낸다. 그 노트의 Note On보다 앞에 Off가 가지 않는가(`last_sent.min(off.host)`).
8. 정지·패닉·위치 이동·루프 되감기의 `release_notes`가 넘기지 않은 끄기를 버린다. 대신 `active`로 끄는가. 넘긴 Note On이 끄기 시각보다 뒤인 경우는 없는가(`release_time = max(now, last_sent)`).
9. 포트가 바뀐 교체: `forget_offs_on_changed_ports`와 `release_changed_ports`·`release_unless_continued`의 순서. 옛 포트에 노트가 남지 않는가.
10. 클릭 길이 > 박 간격(상한 10 s): 다시 켜기 경로가 클릭을 밀지 않는가.
11. 박 큐(256): 가득이면 새 박을 버린다. 스케줄러가 막히지 않는가. UI가 비운 뒤 다시 오는가. 버리는 쪽이 「새 박」이라 UI가 늦게 비우면 옛 박만 받는다.
12. 실시간 경로: `click`, `play_note`, `flush_offs`, `beat_at_or_after`(`MeterMap::position`, `changes()`)에 할당·락·로그가 없는가. `changes()` 선형 탐색의 비용.
13. 공개 API 검증: `set_metronome`은 포트 번호를 검사하는데 `preview`는 검사하지 않는다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 클릭 노트와 같은 노트 미리듣기 | 없음 | `preview_retrigger_same_note_restarts_length`는 미리듣기끼리만 본다(둘 다 `now`라 순서 문제가 없다) |
| 2 짧은 마디·박자표 변화 | 있음 | `metronome_follows_meter_change_and_short_bar` |
| 3 루프 끝·되감기 | 일부 | `metronome_restarts_grid_after_locate_and_loop_wrap`. 짧은 마디를 걸친 루프는 없음 |
| 4 박자표가 바뀐 교체 | 없음 | |
| 5 같은 틱 순서 | 없음 | |
| 6 `u64::MAX` 근처 | 없음 | |
| 7 64칸 넘침 | 있음 | `many_previews_never_leave_a_note_on`(정지 상태). 재생·클릭 중 넘침은 없음 |
| 8 정지·패닉·위치 이동 중 클릭 | 없음 | |
| 9 포트가 바뀐 교체 | 없음 | |
| 10 박보다 긴 클릭 | 없음 | 보고서 「리뷰어에게」 1에 질문으로만 있음 |
| 11 박 큐 넘침 | 없음 | |
| 12 할당 | 있음 | `metronome_and_preview_do_not_allocate` |
| 13 `preview` 포트 검사 | 없음 | |

## 3. 깨뜨리기 시도

모두 `cargo test -p keyscore-midi --test review_s1_t5`로 3회 이상 실행했다. 결과는 매번 같았다. `cargo test --workspace --no-fail-fast`에서 실패는 아래 깨짐 2개뿐이고, S1-T4까지의 기존 테스트는 모두 통과한다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | lookahead 500 ms, 120 BPM, 메트로놈 켬. 재생 20 ms 뒤 채널 10·노트 42를 기본 길이(200 ms)로 미리듣기, 박 2 전에 정지 | 깨짐. 끝에 (채널 10, 42)가 울린 채 남는다 | `preview_of_click_note_during_lookahead_leaves_no_note_sounding` |
| 2 | 기본 설정(lookahead 10 ms), 240 BPM. 박 3 클릭 4 ms 앞에 노트 42를 1 ms로 미리듣기, 박 5 전에 정지 | 깨짐. 같은 노트가 남는다 | `short_preview_just_before_click_leaves_no_note_sounding_with_default_lookahead` |
| 3 | 클릭 120 ms로 재생 중 여러 시점에 정지·패닉·위치 이동 12회 | 못 깨뜨림 | `stop_panic_locate_mid_click_leave_no_note_sounding` |
| 4 | 4/4로 재생 중 3/4 스냅샷으로 교체. 박 순서·강박·격자 2 µs | 못 깨뜨림 | `swap_to_new_meter_keeps_click_grid_and_follows_new_accents` |
| 5 | 클릭 포트 번호 0의 포트를 A↔B로 6번 바꾸는 교체와 미리듣기 | 못 깨뜨림 | `swap_changing_metronome_port_leaves_no_note_sounding` |
| 6 | 틱 960에 노트 이벤트, 클릭, Clock이 겹칠 때 같은 시각의 넘긴 순서 | 못 깨뜨림(이벤트, 클릭, Clock) | `same_tick_event_then_click_then_clock` |
| 7 | 클릭 길이 1분(상한 10 s로 줄음), 박 250 ms | 못 깨뜨림(격자 2 µs 안, 남는 노트 없음) | `click_longer_than_beat_stays_on_grid` |
| 8 | 틱 1440(마디 중간)에 3/4로 바뀌고 루프 [480, 2400) | 못 깨뜨림(42, 37 되풀이, 간격 정확) | `loop_over_short_bar_repeats_same_clicks` |
| 9 | 박자표 변화를 `u64::MAX - 7`·`u64::MAX`에 두고 그 근처에서 재생·루프·교체 | 못 깨뜨림(스레드 살아 있음) | `play_near_tick_max_keeps_scheduler_alive` |
| 10 | 4/256 박자로 1.5 s 동안 박 큐를 비우지 않음 | 못 깨뜨림(256개 뒤 새 박이 다시 옴) | `beat_queue_overflow_recovers_after_drain` |
| 11 | 클릭 중 서로 다른 미리듣기 150개(5 s)로 64칸을 넘침 | 못 깨뜨림 | `preview_overflow_while_clicking_leaves_no_note_sounding` |

시도 1·2의 원인(코드로 확인).

- 클릭 Note On은 박 시각 `Hc`(미래, lookahead 창 안)로 넘어간다. Note Off(`Hc + 길이`)는 창 밖이면 대기 표에 넘기지 않은 채 남는다.
- 같은 (포트, 상태, 노트)의 미리듣기가 `now < Hc`에 오면 `play_note`(`scheduler.rs:1076`)가 `at = off.host.min(host)` = `now`에 Note Off를 보낸다. 미리듣기 Note On도 `now`, 미리듣기 Note Off는 `now + 길이`다.
- `now + 길이 < Hc`이면 타임스탬프 순서는 Off, On, Off, 클릭 On이 된다. 클릭 Note On 뒤의 끄기가 없다.
- `active`는 +1(클릭 On) −1(앞당긴 Off) +1(미리듣기 On) −1(미리듣기 Off) = 0이다. 그래서 정지(`release_notes`)도 이 노트를 끄지 않는다. 재생이 이어지면 다음 같은 노트 클릭이 와야 꺼지고, 정지하면 패닉 전까지 남는다.
- 기본 설정에서는 미리듣기 길이가 lookahead(10 ms)보다 짧을 때 생긴다. lookahead를 미리듣기 기본 길이(200 ms)보다 길게 두면 기본 길이에서도 생긴다. `SchedulerConfig::lookahead`는 공개 설정이고 상한이 없다.
- 같은 뿌리의 다른 경로가 있다(코드로 확인, 테스트는 위 둘로 대표). 넘긴 끄기 때문에 미래로 미룬 미리듣기 Note On(`host = host.max(off.host)`) 뒤에 짧은 미리듣기가 다시 오면, 같은 식으로 앞당긴 Off가 미룬 On보다 앞선다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 미리듣기·클릭 노트는 Note Off 없이 남지 않는다(PRE-1, S1-T5 테스트 게이트) | 예 | 시도 1·2. 클릭 노트와 같은 노트의 짧은 미리듣기 |
| Note Off가 짝인 Note On보다 앞서지 않는다 | 예 | 같은 경로. 앞당긴 Off가 미래 클릭 On보다 앞이다 |
| 클릭은 박 격자 위(교체·루프·짧은 마디) | 아니오 | 시도 4·7·8 |
| 박자표 변화 틱은 새 마디 첫 박(강박) | 아니오 | 시도 4·8 |
| 틱 범위 끝에서 패닉 없음 | 아니오 | 시도 9 |
| 끄기 대기 표가 넘쳐도 Note Off를 잃지 않음 | 아니오 | 시도 11 |
| 채널 1–16, 노트 0–127 | 아니오 | `Channel`·`Pitch` 타입으로만 받는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| MET-1 포트·채널·노트(기본 10·37·42)로 클릭, 버튼·강제 켬, 길이 설정 | 충족 | `metronome.rs:102-110`, `scheduler.rs:1044`(enabled ‖ forced), `metronome.rs:57-62`(길이). 녹음·카운트인 상태 연결은 S2·S4 범위 |
| MET-2 시각 메트로놈(박 이벤트 채널) | 충족(이 Task 범위: UI로 보내는 채널) | `scheduler.rs:1038`(`Beat` 푸시, 클릭이 꺼져도), `poll_beat`. 상태줄·테두리 표시는 UI Task |
| MET-3 메트로놈 전용 포트 | 충족 | `MetronomeConfig::port`, `set_metronome` 포트 검사. 시도 5 |
| PRE-1 미리듣기 Note On/Off, 기본 200 ms | 미충족 | `DEFAULT_PREVIEW_LENGTH`와 송신은 있으나, 클릭과 같은 노트일 때 Note Off 없이 노트가 남는다(시도 1·2, `scheduler.rs:1076`). Task 테스트 게이트 「미리듣기 노트가 Note Off 없이 남지 않음」도 같은 경로로 깨진다 |
| REQ 9.2 스케줄러 스레드 할당·락 금지 | 충족 | 아래 6절 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `click`, `play_note`, `free_earliest_off`, `flush_offs`, `forget_offs_on_changed_ports`는 스택 배열과 `rtrb` 푸시만 쓴다. `beat_at_or_after`의 `MeterMap::position`·`signature_at`은 이분 탐색, `changes()`는 슬라이스 반복자(`map`)라 할당이 없다. 다만 `changes()`의 `find`는 박마다 박자표 변화 수에 비례하는 선형 탐색이다(제안 3).
- 제품 코드의 `unwrap`/`expect`: 새 코드에 없음. `unwrap_or` 셋(`metronome.rs` 기본값 구성)은 상수 입력이라 실패하지 않는다.
- 에러가 삼켜지는 곳
  - `click`의 `let _ = self.beats.push(..)`: 큐가 가득이면 박을 버린다. 의도이고 주석이 있다(제안 2).
  - `send_to`의 포트 실패 무시: S1-T4부터의 방침.
  - `preview`에 범위 밖 포트 번호를 주면 에러 없이 아무것도 보내지 않는다(제안 1).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `play_note`가 넘기지 않은 끄기를 `min(off.host, host)`에 앞당겨 보낸다. 그 끄기의 짝 Note On이 미래 시각(lookahead로 미리 넘긴 클릭, 또는 미룬 미리듣기)이면 Off가 On보다 앞서고, 미리듣기 길이가 그 간격보다 짧으면 노트가 Note Off 없이 남는다. `active`가 0이 되어 정지로도 꺼지지 않는다. PRE-1·Task 테스트 게이트 위반. 깨뜨리는 테스트: `preview_of_click_note_during_lookahead_leaves_no_note_sounding`, `short_preview_just_before_click_leaves_no_note_sounding_with_default_lookahead`. 고칠 방향 예: 대기 칸에 짝 Note On 시각을 함께 두고 앞당긴 Off와 새 On을 `max(그 On 시각, host)` 뒤에 둔다 | `crates/keyscore-midi/src/scheduler.rs:1063-1095` |
| 2 | 제안 | `preview`가 포트 번호를 검사하지 않는다. `set_metronome`·`set_clock`은 `MAX_SCHEDULER_PORTS` 밖이면 `InvalidMessage`를 돌려준다. 일관성을 위해 같은 검사를 둔다 | `crates/keyscore-midi/src/scheduler.rs:491` |
| 3 | 제안 | 박 큐가 가득이면 새 박을 버린다. UI가 늦게 비우면 오래된 박 256개만 받고 지금 박은 잃는다. 시각 메트로놈(MET-2)에는 최신 박이 중요하다. UI 쪽에서 `at`이 지난 박을 건너뛰는 규칙을 문서화하거나, 큐 대신 최신 박 하나를 원자 값으로 두는 방법을 검토한다 | `crates/keyscore-midi/src/scheduler.rs:1036-1043` |
| 4 | 제안 | `beat_at_or_after`가 박마다 `changes().find(..)`로 박자표 변화 전체를 선형 탐색한다. SMF에서 변화가 수천 개면 실시간 경로의 박당 비용이 커진다. `MeterMap`에 이분 탐색 API(다음 변화 틱)를 두는 편이 낫다 | `crates/keyscore-midi/src/scheduler.rs:1375-1378` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(수정)이 PRE-1과 Task 테스트 게이트를 깨므로 수정 뒤 재리뷰한다. 리뷰어 테스트 11개 가운데 2개가 실패하고 9개는 회귀 테스트로 남는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(3회 이상, 결과 같음)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`keyscore-midi`의 src 셋, 테스트 둘, 보고서 둘)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
