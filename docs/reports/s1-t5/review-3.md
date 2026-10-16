# 적대적 리뷰: S1-T5 메트로놈과 미리듣기 (리뷰 3회차)

- 리뷰어 세션: 구현 세션·1·2회차 리뷰 세션과 다른 새 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `3f88918` (브랜치 `s1/t5-metronome-preview`). 2회차 뒤 코드 변경은 `45e274d..HEAD`의 `scheduler.rs` 10줄(`5128dc5`)뿐이다.
- 범위: 확인 리뷰. A) 2회차 지적 1·제안 2의 해결과 그 주변 변형, B) 회귀와 실시간 규율, C) `off_until`이 미리듣기·클릭 Note On을 최대 lookahead만큼 미루는 절충의 REQ 위반 여부. 그 밖은 제안까지만 적는다.
- 리뷰어 테스트: `crates/keyscore-midi/tests/review_s1_t5_r3.rs` (기록 포트 `Rec`만 쓴다)
- 검사한 불변식: (포트, 채널, 노트)마다 타임스탬프 순(같으면 넘긴 순)으로 Note On과 Note Off가 번갈아 오고, 끝 상태는 꺼짐이다. 미리듣기(벨로시티 77로 구별)는 설정한 길이(1 ms 여유)보다 일찍 끊기지 않는다. 다만 끊은 Note Off와 같은 시각에 같은 노트의 새 Note On이 뒤따르는 경우(클릭·미리듣기의 다시 켜기)는 설계대로라 허용한다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 45e274d..HEAD -- crates/`와 `send_to`, `play_note`, `free_earliest_off`, `flush_offs`, `release_notes`만 읽고 적었다.

1. `send_to`가 Note Off(벨로시티 0 Note On 포함)를 넘길 때마다 `off_until[key]`를 `max`로 올리는가. 정지·패닉·위치 이동·루프 되감기(`release_notes`), 교체(`release_unless_continued`, `release_changed_ports`), 표 넘침(`free_earliest_off`), `flush_offs`, 시퀀스 이벤트의 Note Off가 모두 이 한 곳을 지나는가.
2. `play_note`가 Note On 시각을 `off_until` 뒤로 미룬 뒤 그 시각을 `on_host`와 대기 끄기의 기준으로 쓰는가. 즉 미룬 만큼 끄기도 밀려 미리듣기 길이가 줄지 않는가.
3. 같은 시각 Off·On: 미룬 Note On이 넘긴 Note Off와 같은 타임스탬프면 넘긴 순서(Off 먼저)가 지켜지는가.
4. 위치 이동 직후(새 위치 박 0 클릭 Note On이 미래), 루프 되감기 직전·직후 미리듣기에서 교대와 길이가 지켜지는가.
5. 재생 중 교체가 울리는 시퀀스 노트를 `max(now, last_sent)`(미래)에 끈 직후 같은 노트 미리듣기가 그 Note Off에 끊기지 않는가.
6. lookahead로 미리 넘긴 시퀀스 노트의 Note Off(스냅샷 이벤트) 앞에서 같은 노트를 미리듣기하면 미리듣기가 그 뒤로 미뤄지는가(정지 뒤 포함).
7. 실시간 경로: `off_until`은 `Scheduler::start`에서 한 번 할당하는가. 바퀴 안의 추가는 인덱스 읽기·쓰기와 `max`뿐인가.
8. `off_until`이 줄어들지 않는 값이라, 포트가 바뀐 번호나 오래된 값 때문에 Note On이 lookahead보다 더 미뤄질 수 있는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 모든 Note Off 경로의 기록 | 일부 | 2회차 테스트가 정지·표 넘침 경로를 덮는다. 교체·시퀀스 Note Off 경로는 없음, 이번에 추가 |
| 2 미룬 만큼 끄기도 밀림 | 없음 | 2회차 테스트는 교대·끝 상태만 본다. 이번에 길이 검사를 더함 |
| 3 같은 시각 Off·On 순서 | 있음 | `check`가 안정 정렬로 넘긴 순서를 본다 |
| 4 위치 이동·루프 되감기 직후 | 일부 | 2회차 스윕에 위치 이동 한 번. 루프 되감기 없음, 이번에 추가 |
| 5 교체가 끈 노트 | 없음 | 이번에 추가 |
| 6 시퀀스 노트 Note Off 뒤 미리듣기 | 없음 | 이번에 추가 |
| 7 할당 | 있음 | `metronome_alloc.rs`, 코드로 확인(`scheduler.rs:305`) |
| 8 지나친 지연 | 없음 | 코드로 확인. 아래 4절 |

## 3. 깨뜨리기 시도

2회차 리뷰어 테스트: `cargo test -p keyscore-midi --test review_s1_t5_r2` 6개 모두 통과. 2회차에 간헐 실패하던 `preview_of_click_note_sweep_with_stop_and_panic_keeps_alternating`는 단독으로 8회 실행해 8회 모두 통과했다.

이번 테스트는 `cargo test -p keyscore-midi --test review_s1_t5_r3`로 3회 실행해 3회 모두 통과했다. 테스트가 수정을 실제로 가리는지 보려고 임시 worktree에서 수정 전 커밋(`45e274d`)에 같은 파일을 놓고 돌렸다(그 worktree는 지웠다). 수정 전에는 시도 3·4가 교대 위반으로 실패했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | lookahead 300 ms, 120 BPM, 클릭 42(강박=약박, 50 ms). 재생 중 박 위·박 사이로 8번 위치 이동하고 직후(0~2 ms) 42를 120 ms 미리듣기 | 못 깨뜨림(수정 전에도 통과) | `preview_right_after_locate_with_future_click_keeps_length` |
| 2 | 1 s 루프(2박), 같은 클릭. 42를 60 ms로 71 ms마다 50번 미리듣기해 되감기 직전·직후 위상을 모두 지남. lookahead 기본값과 300 ms | 못 깨뜨림(수정 전에도 통과) | `preview_around_loop_wrap_with_future_click_keeps_length` |
| 3 | lookahead 500 ms, 채널 2 CC를 25 ms마다 보내 `last_sent`를 창 끝에 둠. 노트 60이 울리는 중 교체로 그 노트를 미래 시각에 끄게 하고, 직후 60을 800 ms 미리듣기. 3바퀴, 교체 시점을 바꿈 | 못 깨뜨림(수정 전에는 깨짐) | `preview_right_after_swap_releasing_note_keeps_length` |
| 4 | lookahead 500 ms, 시퀀스 노트 60(250 ms)의 On·Off가 모두 넘어간 뒤 60을 300 ms 미리듣기. 재생 중과 정지 직후 두 경우 | 못 깨뜨림(수정 전에는 깨짐) | `preview_after_scheduled_sequence_note_off_keeps_length` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 노트의 Note On·Off가 타임스탬프 순으로 번갈아 온다 | 아니오(이번 범위의 경로) | 2회차 시도 4·5·6과 이번 시도 1~4 모두 통과. 모든 Note Off가 `send_to`(`scheduler.rs:1172-1194`)를 지나 `off_until`에 남고, 미리듣기·클릭 Note On은 그 뒤에 간다(`scheduler.rs:1101-1103`) |
| 미리듣기는 설정한 길이보다 일찍 끊기지 않는다(다시 켜기·정지 등 트랜스포트 제외) | 아니오 | 시도 1~4. 끄기 시각이 미룬 `host`에서 계산된다(`scheduler.rs:1106`) |
| 끝 상태는 꺼짐 | 아니오 | 시도 1~4 |
| Note On 지연은 lookahead 이하(절충 C) | 아니오 | `off_until`에 들어가는 시각은 넘긴 Note Off 시각이고, 넘기는 시각은 lookahead 창 안(`flush_offs`, `schedule`)이거나 `release_time()` = `max(now, last_sent)`로 역시 창 안이다. 줄지 않는 값이지만 지난 시각은 `max(now, …)`에 묻혀 영향이 없다. 포트가 바뀐 번호의 옛 값도 같은 상한을 넘지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| MET-1 | 충족 | 클릭 송신은 `click` → `play_note`(`scheduler.rs:1044-1110`). 이번 변경으로 클릭 Note On이 같은 노트의 넘긴 Note Off 뒤로 최대 lookahead(기본 10 ms) 밀릴 수 있다. MET-1은 클릭의 포트·채널·노트·길이와 켜짐 조건을 정하고 박 대비 시각 허용치는 정하지 않으므로 어기지 않는다 |
| MET-2 | 충족(이 Task 범위) | 박 큐(`click`의 `beats.push`)는 미루기 전 박 시각 `host`를 그대로 보고한다. 2회차와 같다 |
| MET-3 | 충족 | 2회차와 같다 |
| PRE-1 | 충족 | 2회차에 부분 충족으로 셌던 「정지 뒤 미래 클릭 앞 미리듣기」가 통과하고(`preview_after_stop_with_future_click_keeps_alternating`), 교체·시퀀스 Note Off 뒤에도 설정한 길이를 지킨다(시도 3·4). 시작 지연 최대 lookahead는 PRE-1이 시작 지연을 정하지 않으므로 절충으로 받아들인다 |
| REQ 9.2 스케줄러 스레드 할당·락 금지 | 충족 | 아래 6절 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `off_until`은 `Scheduler::start`에서 `MAX_SCHEDULER_PORTS × 16 × 128`칸으로 한 번 할당한다(`scheduler.rs:305`). 바퀴 안의 추가는 `get`/`get_mut`과 `max`뿐이다(`scheduler.rs:1102-1103`, `1188-1190`). `step`은 디버그 빌드에서 `assert_no_alloc`으로 감싸여 있고(`scheduler.rs:669`) `metronome_alloc.rs`를 포함한 전체가 통과했다.
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음. 새 코드는 `get(..).copied().unwrap_or(0)`로 범위 밖을 0으로 둔다.
- 에러가 삼켜지는 곳: 2회차와 같다(`click`의 박 큐 푸시, `send_to`의 포트 실패).
- 회귀: `cargo test -p keyscore-midi --no-fail-fast` 3회 이상 실행, 모두 통과(합계 227 통과, 0 실패, `#[ignore]` 2개는 이 Task 전부터 있던 것). `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` 통과.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `play_note` 머리 주석은 아직 「넘긴 끄기는 … 그 시각이 뒤면 새 Note On을 그 뒤로 미룬다」로 대기 표 칸만 말한다. 이제 근거가 `off_until`이고 대기 표의 `off.sent` 분기(`host.max(off.host)`)는 `off_until`과 겹친다. 주석에 「최대 lookahead만큼 미뤄질 수 있다」를 적어 두면 C의 절충이 코드에서 보인다 | `crates/keyscore-midi/src/scheduler.rs:1066-1071` |

2회차 지적 1(수정)과 제안 2는 해결됐다. 이번 범위에서 수정·차단 지적은 없다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

2회차 리뷰어 테스트 6개가 모두 통과하고, 간헐 실패하던 스윕도 단독 8회 통과했다. 위치 이동·루프 되감기·교체·시퀀스 Note Off 주변 변형(이번 테스트 4개)에서 교대·끝 상태·미리듣기 길이가 모두 지켜졌다. 실시간 규율 위반과 회귀가 없다. 제안 1은 거절할 수 있다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(3회. 수정 전 커밋에서 시도 3·4가 깨지는 것도 확인)
- [x] REQ ID를 하나씩 대조했다(MET-1~3, PRE-1, REQ 9.2)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`45e274d..HEAD`는 `scheduler.rs`와 보고서뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
