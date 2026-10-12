# 적대적 리뷰: S0-T2 핵심 모델 타입 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `a75e6b8` (`git diff main...HEAD`, 커밋 `4410b7b`..`a75e6b8` 5개)
- 기준: `plan/stages/10-S0-foundation.md` S0-T2, `docs/reports/s0-t2/plan.md`, `docs/decisions/2026-10-12-clip-split-and-velocity.md`, REQ 2·3·5.10·5.15·6.1·6.5

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트를 열기 전에 `src/*.rs`의 `#[cfg(test)]` 위쪽만 읽고 적었다.

1. `Clip`을 트랙 밖에서 만들어 `insert_note`로 채운 뒤 `insert_clip`하면 끝이 넘는 노트가 들어갈 수 있는가. `remove_clip`으로 꺼낸 숨김 꼬리 클립에 노트를 더 넣을 수 있는가.
2. 숨김 꼬리(노트 끝 > 클립 길이)를 가진 클립을 `move_clip`으로 u64 끝 근처에 옮기면 「클립 시작 + 노트 끝」이 u64를 넘는가. `move_clip`은 클립 길이만 검사한다(`track.rs:316`).
3. `move_clip`·`split_clip`·`insert_clip`이 u64 끝 경계(`start + length == u64::MAX`, `+1`)에서 오버플로를 `Err`로 돌려주고 상태를 그대로 두는가. 기존 속성 테스트의 틱 범위가 작으면 이 경계를 못 본다.
4. 숨김 꼬리 노트를 다시 분할하면(꼬리 노트의 시작이 분할점 뒤) 오른쪽 클립의 노트 시작이 `< 길이`를 지키는가.
5. 실패한 연산 뒤 상태 불변: `move_clip`이 `remove` 뒤 `set_start`가 실패하면 클립이 사라지는가(`track.rs:323-325`).
6. `ClipId`는 트랙마다 0부터 발급된다. 다른 트랙의 식별자로 이 트랙의 엉뚱한 클립을 고치거나 지울 수 있는가.
7. `next_clip_id`가 `PartialEq`에 들어가 내용이 같은 두 트랙이 다르다고 나오는가(S0-T4 「undo N번 = 초기 상태」와 충돌).
8. 트랙 안 클립의 이름·색을 바꿀 공개 경로가 있는가(REQ 5.10 「클립에는 이름과 색을 줄 수 있다」). `Track`은 `&mut Clip`을 내주지 않는다.
9. 범위 검증: 채널 0·17, 음높이 128, 벨로시티 128, 오프셋 ±1201, 길이 0 노트·클립, 팔레트 12, 피치 클래스 12.
10. 제품 코드의 패닉 지점: 검사 없는 덧셈(`Clip::end`, `Note::end`), 뺄셈(`split_at`), 인덱싱, `next_clip_id += 1`.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 트랙 밖 `Clip`·꺼낸 꼬리 클립에 끝 넘는 노트 | 부분 | `clip_insert_note_past_end_is_rejected`는 새 클립만. 꼬리 클립에 다시 넣는 경우 없음 |
| 2. 꼬리 클립 이동 후 절대 노트 끝 오버플로 | 없음 | 속성 테스트 틱 범위가 `0..20_000`이라 원리상 못 본다 |
| 3. u64 끝 경계의 이동·분할 | 부분 | `Clip::new`·`Note::new`·`Tick::checked_add`의 경계 단위 테스트만. `move_clip` 경계 없음 |
| 4. 꼬리 노트 재분할 | 간접 | 속성 테스트 15·17의 무작위 수열이 덮는다 |
| 5. 실패한 연산 상태 불변 | 있음 | `track_ops_failed_op_leaves_track_unchanged`. `next_clip_id`까지 비교해 엄격하다. 단 작은 틱 범위만 |
| 6. 다른 트랙의 `ClipId` | 없음 | `track_note_ops_on_unknown_clip_are_rejected`는 같은 트랙에서 지운 식별자만 본다 |
| 7. `next_clip_id`와 `PartialEq` | 없음 | |
| 8. 트랙 안 클립 이름·색 변경 | 없음 | 공개 API 자체가 없다 |
| 9. 범위 검증 | 있음 | 단위 테스트가 모두 덮는다 |
| 10. 패닉 지점 | 해당 없음 | 코드 읽기로 확인(6절) |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-core/tests/review_s0_t2.rs`에 있고 `cargo test -p keyscore-core --test review_s0_t2`로 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클립 `[0,100)`에 노트 `[0,100)` → 50에서 분할(왼쪽에 꼬리) → 왼쪽 클립을 `u64::MAX - 50`으로 이동. `move_clip`은 `Ok`. 클립 시작 + 노트 끝 = `MAX - 50 + 100`이 u64를 넘는다 | 깨짐 | `move_clip_with_hidden_tail_near_u64_max_keeps_absolute_note_end_in_u64` (실패) |
| 2 | 길이 10 클립을 `u64::MAX - 9`로 이동(1틱 넘침), `u64::MAX - 10`으로 이동(정확히 닿음) | 못 깨뜨림. `Err(TickOverflow)`, 상태 불변, 경계는 허용 | `move_clip_past_u64_max_is_rejected_and_state_unchanged` |
| 3 | 꼬리 노트를 같은 클립에 다시 `insert_note`, 꼬리 클립을 `remove_clip`으로 꺼내 `Clip::insert_note`, 다른 트랙에 넣고 `Track::insert_note` | 못 깨뜨림. 모두 `NoteOutsideClip`, 상태 불변 | `hidden_tail_cannot_be_created_by_insert_note_after_split` |
| 4 | 트랙 A의 `ClipId`로 트랙 B의 `remove_clip` | 불변식은 못 깨뜨림. 그러나 B의 무관한 클립이 지워진다(현재 동작 기록) | `clip_id_from_another_track_addresses_unrelated_clip` |
| 5 | u64 끝 근처(`MAX - 0..1000`)와 임의 u64에 몰린 삽입·이동·분할·노트 삽입 수열 1024사례. 매 연산 뒤 겹침 없음·노트 시작 < 길이·클립 끝 오버플로 없음·실패 시 상태 불변 | 못 깨뜨림 | `edge_tick_ops_preserve_invariants_and_failed_ops_leave_state` |

시도 1의 실패 출력:

~~~
move_clip 결과 Ok(()): 클립 시작 18446744073709551565 + 노트 끝 100가 u64를 넘는다
~~~

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | `insert_clip`·`move_clip`이 `overlaps`로 검사, `split_clip`은 원래 구간 안. 시도 5 |
| 노트 시작은 클립 안(`0 ≤ start < length`) | 아니오 | `insert_note`는 끝까지 검사, `split_at`은 `start ≥ offset`인 노트만 오른쪽으로 옮기고 `offset`을 뺀다. 시도 5 |
| `insert_note`로는 끝까지 클립 안 | 아니오 | `Clip::insert_note`(`clip.rs:92-108`) 하나로 모인다. 시도 3 |
| 채널 1–16, 음높이 0–127, 벨로시티 v1 0–127, 오프셋 ±1200 | 아니오 | 생성자 검증, 필드 비공개 |
| 노트·클립 길이 ≥ 1 | 아니오 | 생성자 검증, 분할은 `0 < offset < length`만 받는다 |
| 클립 끝(`start + length`) u64 안 | 아니오 | `Clip::new`, `set_start`, `move_clip`이 검사. 시도 2·5 |
| 노트의 절대 끝(클립 시작 + 노트 끝) u64 안 | 예 | 시도 1. 분할로 꼬리를 만든 뒤 `move_clip`(`track.rs:313-328`)이 클립 길이만 보고 꼬리를 보지 않는다. 계획 「끝의 정의」 3의 「틱 계산은 u64 오버플로 없이」가 이 값에는 보장되지 않는다 |
| 실패한 연산은 상태 불변 | 아니오 | `move_clip`은 검사 뒤 `remove`, `split_at`은 계산 뒤 변경. 시도 2·3·5 |
| `pub` 필드 없음 | 지켜짐 | `ClipId(pub(crate) u64)`만 crate 내부 공개 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| SEQ-1 | 충족(이 Task 범위) | `project.rs:7-10` 조성 + 트랙 목록, 수 제한 없음. 템포·박자표 맵은 계획 「범위 밖」(S0-T3) |
| SEQ-2 | 충족(이 Task 범위) | 클립 = (시작, 길이) `clip.rs:18-24`, 겹침 없음 `track.rs:269-301`, 이동 `track.rs:313`, 분할 `track.rs:337`. 복제·합치기·트림은 계획 「범위 밖」(S4-T2, 단계 파일 14 S4-T2 범위와 일치). 단 지적 1은 S4-T2의 「트림 후 복원」을 막는 경로다 |
| SEQ-3 | 충족 | 시작 `Tick`(u64) `note.rs:91`, 길이 u32 `note.rs:92`, 음높이 0–127 `note.rs:11`, 오프셋 ±1옥타브 `note.rs:39`, 벨로시티 u16에 v1 0–127(결정 Q2) `note.rs:71`, 채널은 트랙 `track.rs:124`. 오프셋 해상도(정수 센트)는 REQ에 없는 결정이다(지적 3) |
| SEQ-4 | 충족(이 Task 범위) | `CcPoint` 32비트 값 `note.rs:187-207`. 단계 파일 범위가 `CcPoint`만 정하고, 레인 구조·프로그램 체인지·피치 벤드는 계획 「범위 밖」(S6) |
| SEQ-7 | 충족(이 Task 범위) | 이름·색·포트·채널 1–16·뱅크·프로그램·기본 벨로시티·뮤트·솔로 `track.rs:120-134`, 10번 드럼 `track.rs:28`. 채널 자동 배정·팔레트 순서는 계획 「범위 밖」(S4-T3, 단계 파일 14 S4-T3 범위와 일치) |
| LYR-1 | 충족(모델 부분) | `Note::lyric` 선택 필드, 빈 문자열은 없음 `note.rs:131-137`, 멜리스마 `"-"`는 문자열 그대로 보관, 트랙 `lyric_lang` `track.rs:245-252`. `.ksq` 다섯 번째 원소는 S0-T6 |
| REQ 2 조성 | 충족 | `key.rs:27-32` 장·단조 12개씩 + 없음, 기본 없음 |
| REQ 2 트랙 색 | 충족(이 Task 범위) | `TrackColor` 0–11 `track.rs:35-55`. 자동 배정은 S4-T3 |
| REQ 5.10 클립 규칙 「이름과 색」 | 부분 충족 | `Clip::set_name`·`set_color`는 트랙 밖 클립에만 쓸 수 있다. 트랙 안 클립은 `remove_clip` → 변경 → `insert_clip`밖에 없고 그러면 `ClipId`가 바뀐다(지적 2) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 해당 없음. 이 Task에는 스케줄러 코드가 없다(`core`만 변경).
- 제품 코드의 `unwrap`/`expect`: 없음(`rg`로 확인, 모두 `#[cfg(test)]`와 `tests/` 안).
- 패닉 가능 지점(검사 없는 산술·인덱싱):
  - `clip.rs:58` `start + length`: `Clip::new`·`set_start`·`split_at`이 막는다. 안전.
  - `note.rs:152` `start + length`: `Note::new`·`with_start`가 막는다. 안전.
  - `clip.rs:143` `n.start - offset`: `partition_point`로 `start ≥ offset`인 노트만. 안전.
  - `clip.rs:147` `length - offset`: `offset < length` 검사 뒤. 안전.
  - `track.rs:348` `at - start`: `at > start` 검사 뒤. 안전.
  - `track.rs:261,315,339,348,360,370` 인덱싱: 모두 `index_of`의 결과. 안전.
  - `track.rs:286` `next_clip_id += 1`: 2^64번 삽입 뒤에만 넘친다. 현실적으로 도달 불가.
  - 공개 API 밖의 계산: 「클립 시작 + 노트 끝」은 core에 계산하는 곳이 없어 core 안에서는 패닉이 없다. 그러나 이 값을 계산할 하류(S1 송신, S0-T6 직렬화, S4-T2 트림 복원)가 시도 1의 상태를 만나면 디버그 빌드에서 패닉, 릴리스에서 랩어라운드한다(지적 1).
- 에러가 삼켜지는 곳: 없음. `move_clip`의 `clip.set_start(new_start)?`(`track.rs:325`)는 `remove` 뒤에 있어 실패하면 클립이 사라질 수 있는 모양이지만, 같은 계산을 `track.rs:316`에서 먼저 검사하므로 도달하지 않는다.

`scripts/check-deps.test.sh` 변경(`d11e520`): 정당하다. `core`에 `[dev-dependencies]`가 생기면서 끝에 덧붙인 `reqwest = ...` 줄이 dev 의존으로 해석되던 결함을 고쳤다. `cfg(all())` 타깃 테이블은 호스트에서 항상 참이라 각 사례가 의도한 종류(normal·dev·build)를 그대로 주입한다. `check-deps.sh`가 `--target all -e normal,build,dev`를 쓰므로 타깃 테이블과 일반 테이블은 같은 그래프 간선이 된다. `-e dev`나 `--target all`을 빼는 퇴화는 사례 D·E와 A가 여전히 잡는다. 이유 문자열 검사도 남아 있어 TOML 파싱 오류로 「통과」하는 일이 없다. 11/11 통과를 직접 확인했다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 숨김 꼬리를 가진 클립을 `move_clip`으로 옮기면 「클립 시작 + 노트 끝」이 u64를 넘을 수 있다(시도 1, 실패 테스트 남김). core의 명시된 불변식(클립 끝 u64 안)은 지켜지고 core 안에서 이 값을 계산하는 곳이 없어 차단으로 올리지는 않았다. 그러나 계획 「끝의 정의」 3이 약속한 「틱 계산은 u64 오버플로 없이」가 숨김 꼬리에는 성립하지 않고, 결정 Q1이 요구하는 「다시 합치면/늘리면 원래 노트로 돌아온다」가 이 클립에서는 구조적으로 불가능해진다(되살릴 길이가 u64를 넘음). `move_clip`(과 이후 클립 시작을 바꾸는 모든 연산)이 「시작 + max(길이, 가장 늦은 노트 끝)」을 검사하거나, `Clip`의 불변식으로 「시작 + 노트 끝 ≤ u64::MAX」를 두고 `Clip` 문서에 적을 것. 테스트 `move_clip_with_hidden_tail_near_u64_max_keeps_absolute_note_end_in_u64`가 통과해야 한다 | `crates/keyscore-core/src/track.rs:313-328`, `crates/keyscore-core/src/clip.rs:9-13` |
| 2 | 수정 | 트랙 안 클립의 이름·색을 바꿀 공개 API가 없다. `Clip::set_name`·`set_color`는 `&mut Clip`이 필요한데 `Track`은 이를 내주지 않는다. 우회(`remove_clip` → 변경 → `insert_clip`)는 `ClipId`를 바꾸므로 식별자를 쥔 쪽(이후 undo·UI 선택)이 깨진다. REQ 5.10 「클립에는 이름과 색을 줄 수 있다」의 부분 충족. 겹침과 무관한 속성이므로 `Track::set_clip_name(id, ..)`·`set_clip_color(id, ..)` 같은 경로가 필요하고 단위 테스트를 더할 것 | `crates/keyscore-core/src/track.rs:254-372`, `crates/keyscore-core/src/clip.rs:72-85` |
| 3 | 수정 | 피치 오프셋을 정수 센트(i16, 1센트 해상도)로 둔 것은 REQ에 없는 결정인데 `docs/decisions/`에 결정 요청 없이 계획 「리스크」에만 적고 확정했다(CLAUDE.md 「REQ에 없는 결정은 임의로 정하지 않는다」). REQ 6.5는 「나중에 2.0을 붙일 때 모델과 파일 포맷을 바꾸지 않도록」을 요구하고, MIDI 2.0 노트별 피치(M2-3)는 7.25 고정소수라 1센트 정수로는 손실 없이 담지 못한다. 결정 요청(선택지 예: 정수 센트 / 1/100보다 고운 정수 단위 / MIDI 2.0 7.25 고정소수 그대로)을 쓰고 사람의 결정을 받을 것. S0-T6 `.ksq` 포맷이 이 값에 묶이므로 지금 정해야 한다 | `crates/keyscore-core/src/note.rs:25-51`, `docs/reports/s0-t2/plan.md` 「리스크」 |
| 4 | 제안 | `ClipId`가 트랙마다 0부터 발급되어 다른 트랙의 식별자가 무관한 클립을 가리킨다(시도 4). 지금은 문서로만 막는다. 트랙 간 이동(S4-T2)·undo(S0-T4)가 식별자를 들고 다니기 시작하면 잘못된 클립 삭제로 이어질 수 있다. 프로젝트 전역 발급이나 트랙 식별자를 함께 담는 방식을 S0-T4 전에 검토 | `crates/keyscore-core/src/clip.rs:3-5`, `crates/keyscore-core/src/track.rs:284-288` |
| 5 | 제안 | `Track`의 `PartialEq`가 `next_clip_id`를 비교한다. 클립을 넣고 지운 트랙은 내용이 같아도 초기 트랙과 다르다. S0-T4 게이트 「undo N번 = 초기 상태」를 역연산으로 구현하면 이 비교로 실패하거나, 테스트가 비교 대상을 바꾸게 된다. S0-T4 계획에서 「같음」의 정의를 정해 둘 것 | `crates/keyscore-core/src/track.rs:119-134` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

공개 API로 핵심 불변식(겹침 없음, 노트 시작은 클립 안, `insert_note`는 끝까지 클립 안, 값 범위, 실패 시 상태 불변)을 깨뜨리지 못했다. 지적 1~3을 고친 뒤 재리뷰한다. 지적 1의 실패 테스트(`tests/review_s0_t2.rs`)는 지우거나 `#[ignore]`하지 말고 통과시킬 것.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(5개, 1개 실패)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`scripts/check-deps.test.sh`는 S0-T1 파일이지만 이 Task의 dev 의존 추가로 드러난 결함 수정이라 정당하다고 판단, 6절)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(check-deps.test.sh의 11개 사례 유지)
- [x] 새 의존성이 있으면 라이선스를 확인했다(`thiserror`, dev `proptest`. `cargo deny check` 통과, `check-deps.sh` 허용 목록 안)
- [x] 스타일 지적을 적지 않았다

## 부록: 자동 게이트 실행 결과(대상 커밋 기준, 리뷰 테스트 추가 전)

~~~
cargo fmt --all -- --check                          통과
cargo clippy --workspace --all-targets -- -D warnings  통과
cargo test --workspace                              통과(core lib 24, model_invariants 4)
cargo deny check                                    advisories ok, bans ok, licenses ok, sources ok
scripts/check-deps.sh                               check-deps: ok
scripts/check-deps.test.sh                          11 passed, 0 failed
~~~

리뷰 테스트 추가 후: `cargo fmt --check`·`cargo clippy -p keyscore-core --all-targets -D warnings` 통과, `cargo test -p keyscore-core --test review_s0_t2`는 4 통과 1 실패(시도 1, 의도된 실패).
