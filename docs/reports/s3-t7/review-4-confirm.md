# 적대적 리뷰: S3-T7 검색과 노트 정보 (리뷰 4회차, 좁은 확인 리뷰)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트, 구현 컨텍스트를 물려받지 않음. 리뷰 1–3 리뷰어와도 다른 세션)
- 범위: 결정 `docs/decisions/2026-11-01-s3-t7-review-limit.md` A. 리뷰 3 기록(`6aa6a37`) 뒤의 수정만 본다(`git diff 6aa6a37 cabe4bf`): `search::walk`, `search_jump`의 그 사용, 노트 정보 cent 표시, 결정 4·5·6절의 바뀐 문장, 시험 `search_pure.rs`·`review_s3_t7_r3.rs`의 변경
- 대상 커밋: `cabe4bf`
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t7_r4.rs`
- 실행 환경: `CARGO_TARGET_DIR=target/review`, `-p keyscore-app --test <이름>`만 돌렸다. MIDI·재생 시험과 전체 시험은 돌리지 않았다

## 0. 리뷰 3 지적의 확인

| 리뷰 3 지적 | 확인 | 근거 |
| --- | --- | --- |
| 1 수정: 카운트 걷기 이차 비용 | 고쳐짐. 첫 걸음만 `step`, 나머지는 순번 산술이라 O(n). 노트 10,000개 debug에서 `n` 한 번 34.5 ms, `9999 n` 36.2 ms | `search.rs:309-341`, `review_s3_t7_r3::large_count_walk_is_not_quadratic` 통과 |
| 2 제안: 노트 없는 결과에서 끝나는 걷기의 음높이 | 고쳐짐. `pitch_from`이 걸은 길에서 마지막으로 지난 노트 결과다. 결정 5절 4에 적혔다 | `search.rs:329-335`, `search_bar.rs:234-236`, `review_s3_t7_r3::count_walk_equals_repeated_keys_with_empty_track_hits` 통과 |
| 3 제안: 0.05 cent 미만·작은 음수 | 고쳐짐. 부호는 반올림 전 값에서, 0으로 반올림되면 `±<0.1 cent`. 결정 6절 2에 적혔다 | `note_info.rs:72-93`, `tiny_*` 두 시험 통과 |
| 4 제안: 결정 5절 번호 | 고쳐짐(1, 2, 3, 4) | 결정 문서 5절 |
| 5 확인 항목: 보고서 게이트 칸 | 아직 `GATES_PLACEHOLDER` | `docs/reports/s3-t7/report.md:70` |
| 리뷰 시험 상수 변경 | 맞다. `48 * ((1 + 9999) % 10_000)` = `48 * 0` = 0이므로 `0`과 같은 뜻이다. 리뷰 3 시험 파일의 다른 줄은 바뀌지 않았다(`git diff 6aa6a37 -- tests/review_s3_t7_r3.rs`는 그 한 줄과 주석) | `review_s3_t7_r3.rs:346-347` |

이번 HEAD에서 다시 돌렸다: `review_s3_t7_r1` 13, `review_s3_t7_r2` 8, `review_s3_t7_r3` 9, `search` 18, `search_pure` 11, `note_info` 5 모두 통과.

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 순번 산술이 `step` 반복과 같은가: `step`이 결과 i의 자리에서 늘 i±1을 고르는가(노트 없는 결과는 음높이를 보지 않고, 커서 음높이는 지난 노트의 것을 들고 온다).
2. 돎: Forward `end >= n`, Backward `first.index < extra`, `count > n`이면 반드시 돎. 결과 1개, count가 결과 수의 배수일 때.
3. Backward의 순번 산술과 `pitch_from`의 거꾸로 찾기 방향.
4. 트랙 순서가 결과 정렬과 다를 때(트랙을 아무 순서로 둔 프로젝트, 같은 틱에 여러 트랙의 결과).
5. 노트 없는 결과만 있을 때 `pitch_from`이 `None`이고 커서 음높이를 두는가.
6. u32 끝의 카운트: `count - 1`, `usize` 변환, `n = 0`에서의 나머지(0으로 나눔).
7. cent 반올림 경계: 0.05 cent(16777.216 단위), 99.95 cent, 음수 쪽 같은 경계, 0이 아닌 오프셋이 0으로 보이는가.
8. `walk`의 전제(`hits`가 지금 프로젝트의 `find_hits` 결과)를 제품 경로가 지키는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1·3 `walk` = `step` 반복 | 부분 | `search_pure::walk_equals_repeated_steps`는 노트에 가사가 없어 노트 결과가 많아야 하나(트랙 「Pad lead」의 트랙 이름 결과)이고, 그 트랙이 늘 0번이다. 노트 결과 여럿·트랙 순서 섞임은 보지 않는다(지적 1) |
| 2 결과 1개·배수 | 부분 | 위 속성 시험의 count 1–29가 우연히 덮을 수 있을 뿐 |
| 4 트랙 순서 | 없음 | |
| 5 노트 없는 결과만 | 있음 | 위 속성 시험에서 `notes`가 빌 때 |
| 6 u32 끝 | 없음 | GUI 카운트는 9999로 막혀(`keymap/mod.rs:512`) 제품 경로로는 닿지 않는다 |
| 7 cent 경계 | 부분 | `info_lines_show_pitch_start_length_channel`이 ±1단위만 |
| 8 전제 | 해당 없음 | 코드 읽기. `search_jump`가 매번 지금 문서로 `find_hits`를 부른다(`search_bar.rs:211-222`) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 트랙 1–5개(이름 결과 있음/없음, 가사 노트 0–5개, 노트 없는 트랙의 클립 있음/없음)를 아무 순서로, 결과 위(노트 없는 결과 위에서는 다른 음높이)·임의 자리에서, 앞뒤로 count 1–39: `walk`와 `step` 반복의 (순번, 돎, 커서 음높이) 비교. 512 사례 | 못 깨뜨림 | `walk_equals_repeated_steps_on_mixed_track_orders` |
| 2 | 같은 생성기에서 count = `u32::MAX - 64`..=`u32::MAX`와 결과 수의 1–4배. 결과 수보다 크면 `(count - 1) % n + 1 + n`걸음의 실제 `step` 반복과 비교 | 못 깨뜨림 | `walk_with_huge_or_multiple_counts_matches_period` |
| 3 | 결과 1개(노트 없는 트랙): 앞에서 `1 n`은 돌지 않고, `2 n`·`u32::MAX n`은 돌고, Backward는 처음부터 돈다. 음높이는 둔다. `count = 0`은 1로. 빈 결과는 `None`(0으로 나누지 않음) | 못 깨뜨림 | `walk_single_hit_wrap_and_huge_count` |
| 4 | 오프셋 -MAX..=MAX 전 범위 속성: 부호가 오프셋의 부호, 보인 값이 참값에서 0.05 cent 안, 0이 아닌 오프셋이 0으로 보이지 않음, `<0.1`은 참값 0.05 cent 미만에서만 | 못 깨뜨림 | `cent_text_is_signed_and_within_half_tenth` |
| 5 | 경계 16777/16778 단위(±), 99.95 cent(33537654/33537655 단위, ±) | 못 깨뜨림 | `cent_rounding_boundaries` |

리뷰 시험 5개: 5 통과, 0 실패. 처음 실행에서 `cent_rounding_boundaries`가 실패했으나 99.95 cent의 단위 수를 내가 잘못 셈한 것이었다(33538457로 적음, 맞는 값 33537654.784). 고친 뒤 통과했다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 카운트 `k`의 걷기 = 키 k번(결과·돎·커서 음높이) | 못 찾음 | 시도 1–3 |
| 검색·정보 팝업은 문서·undo·`.`을 바꾸지 않는다 | 못 찾음 | 리뷰 3 뒤 변경은 읽기 전용 계산뿐 |
| `walk`의 전제(지금 프로젝트의 `find_hits` 결과) | 제품 경로로는 못 깨뜨림. 공개 함수에 낡은 `hits`를 주면 순번 산술이 `step`과 어긋날 수 있다(코드 읽기, 지적 2) | `search.rs:303-309` |
| core 모델 불변식 | 해당 없음 | 문서를 고치지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8.1 「반복은 숫자 카운트 접두」(`n`) | 충족 | `search.rs:309-341`, `search_bar.rs:222`. 시도 1–3 |
| 5.8 `i` 커서 노트 정보 팝업 | 충족 | `note_info.rs:72-93`. 시도 4·5 |
| NF-8 UI 프레임 60 fps(노트 10,000개) | 충족(이 경로에 한해) | `9999 n` 36.2 ms(debug), `n` 한 번과 같은 차수 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(변경에 `keyscore-midi`·엔진 없음)
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음. `walk`의 `usize::try_from(count).unwrap_or(usize::MAX)`는 64비트에서 늘 성공한다. `(count - 1) % n`은 `step`이 빈 결과에서 먼저 `None`을 내므로 `n > 0`이다. `hits[i]`는 `0..n` 안의 순번뿐이다
- 에러가 삼켜지는 곳: 없음

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 구현자의 속성 시험 `walk_equals_repeated_steps`는 노트에 가사가 없어 노트 결과가 많아야 하나이고 그 트랙이 늘 0번이라, 순번 산술이 노트 결과 사이를 걷는 경우와 트랙 순서가 섞인 경우를 거의 보지 않는다. 노트에 가사를 섞고 트랙 순서를 섞는 생성기로 넓히거나, 리뷰 시험 `walk_equals_repeated_steps_on_mixed_track_orders`를 그 몫으로 둔다 | `crates/keyscore-app/tests/search_pure.rs:474-484` |
| 2 | 제안 | `walk`는 `pub`이고 「`hits`가 지금 프로젝트의 `find_hits` 결과」라는 전제를 문서로만 둔다. 지금 유일한 호출자(`search_jump`)는 지키지만, 낡은 `hits`(트랙이 지워지거나 순서가 바뀐 뒤)를 주면 `step`은 그 결과를 거르는데 순번 산술은 세므로 어긋난다. `debug_assert!`로 전제를 확인하거나 가시성을 `pub(crate)`로 줄이기를 권한다(시험이 쓰므로 거절해도 된다) | `crates/keyscore-app/src/search.rs:303-309` |
| 3 | 확인 항목 | 보고서 「자동 게이트」가 아직 `GATES_PLACEHOLDER`다. 병합 직전 마지막 HEAD에서 네 게이트 결과로 채우고 「테스트 결과」에 `review_s3_t7_r3`·`review_s3_t7_r4` 줄을 더한다. 리뷰 시험 `review_s3_t7_r4.rs`는 `cargo fmt --check`와 `cargo clippy -p keyscore-app --test review_s3_t7_r4 -- -D warnings`를 통과한다 | `docs/reports/s3-t7/report.md:70` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 3 지적 1–4는 모두 동작으로 고쳐졌고, 경계(결과 1개, 결과 수의 배수, Backward, 트랙 순서 섞임, 노트 없는 결과만, u32 끝, cent 반올림 경계)에서 새로 깨진 것을 찾지 못했다. 차단·수정 지적은 없다. 제안 1·2는 받아들이거나 거절 이유를 보고서에 적고, 확인 항목 3(게이트 칸)을 채운 뒤 병합한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`review_s3_t7_r4` 5개: 5 통과)
- [x] REQ ID를 하나씩 대조했다(이번 범위의 5.8, 5.8.1, NF-8)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 3 뒤 변경은 `search.rs`, `search_bar.rs`, `note_info.rs`, 시험 둘, 결정 문서 둘)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 3 시험은 상수 한 줄만 같은 뜻으로 바뀜)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
