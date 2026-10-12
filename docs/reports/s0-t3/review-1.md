# 적대적 리뷰: S0-T3 템포 맵과 틱↔시각 변환 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `2a13356` (`git log main..HEAD`의 6개 커밋, `git diff main...HEAD`)
- 기준: `plan/stages/10-S0-foundation.md` S0-T3, `docs/REQ.md` 2절(템포·박자표 변화, 새 프로젝트 기본값), 6.1절 SEQ-1·SEQ-5·SEQ-6·SEQ-9, 8.1절 NF-4, 9.2절, `plan/00-conventions.md`

## 1. 코드만 읽고 적은 「내가 검사할 것」

`src/tempo.rs`, `src/meter.rs`, `src/project.rs`의 `#[cfg(test)]` 위쪽만 읽고 적었다.

1. `tick_to_time`/`time_to_tick`을 캐시·이진 탐색을 쓰지 않는 기준 모델(구간을 처음부터 걸으며 u128로 더함)과 비교한다. 임의 템포 맵, 삽입 순서 섞기, 중간 변화 삭제 뒤에도 같은가.
2. 경계: 변화 틱 정확히, 변화 틱 −1·+1, 틱 0, `u64::MAX`, 시각이 구간 시작 시각과 정확히 같을 때와 그 −1·+1 단위.
3. `time_to_tick`의 `partition_point(..) - 1`은 첫 항목의 캐시 시각이 0이고 구간 시작 시각이 엄격히 증가한다는 가정에 기댄다. `set(Tick::ZERO, ..)`·1틱 간격 연속 변화·최소 템포(1 µs)에서 가정이 깨지는가.
4. 오버플로: `rebuild`의 `span * units_per_tick` 누적(최대 템포 16,777,215 µs × u64 틱 전 범위), `MusicTime::from_units(u128::MAX)`의 `time_to_tick`, `from_bpm(u32::MAX)`, `MeterMap::rebuild`의 마디 번호 누적, `bar_start(u64::MAX)`.
5. 공개 API로 맵 불변식(틱 0 항목, 정렬, 중복 없음, 캐시 일치)을 깨는 경로: `remove(0)`, 같은 틱 `set`, `tempo_map_mut`, `MusicTime::from_units` 공개, 파생 `PartialEq`가 캐시까지 비교하는 점.
6. `MeterMap`: 마디 중간 변화(짧은 마디), 1틱 간격 연속 변화, 분자 255·분모 64(박 60틱), `u64` 끝 근처 변화에서 `position`과 `bar_start`가 서로 역인가. 틱을 하나씩 세는 무차별 모델과 같은가.
7. `TimeSignature::new`가 받는 분모 목록이 SMF 박자표 메타(분모 = 2^dd)를 보존하는 데 충분한가(REQ 2 「SMF의 변화는 보존」, SEQ-9). 주석의 「512 이상은 나누어떨어지지 않는다」는 128·256을 막는 근거가 되는가.
8. 템포 저장 단위(4분음표당 µs, 정수)가 REQ의 「템포 120」(BPM), TRN-4(입력·끌기·탭 템포)와 맞는가. 60,000,000을 나누지 않는 정수 BPM(90, 140 등)이 정확히 표현되지 않는 것을 누가 결정했는가.
9. 1시간 게이트 정정: 120 BPM × 60분 = 7,200 4분음표 × 960 = 6,912,000틱이 맞는가.
10. 실시간 경로: `tick_to_time`/`time_to_tick`/`tempo_at`에 할당·락이 없는가.
11. `bench-check.py`: 비교 방향, 0 나눗셈, 기준값에만/결과에만 있는 벤치, 같은 `full_id`가 두 디렉터리에 있을 때, 일부만 다시 돈 벤치(낡은 결과), `CARGO_TARGET_DIR`.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 독립 기준 모델과 비교 | 부분 | `tempo_map_segment_sum_equals_direct_conversion`이 구간 합과 비교하지만 틱 0..3,000,000, 변화 틱 1..2,000,000에 한정. 삽입 순서 섞기·삭제 뒤 비교 없음 |
| 2. 경계(변화 틱 ±1, 구간 시작 시각 ±1) | 부분 | 임의 틱 왕복과 floor 성질은 있으나 변화 틱·구간 시작 시각을 겨냥한 표본 없음 |
| 3. 구간 시작 시각 엄격 증가 | 간접 | 엄격 증가 속성은 틱 쪽만. 1틱 간격·최소 템포 조합 없음 |
| 4. 오버플로 | 부분 | `u128::MAX` 시각 1건, 틱 끝 근처 왕복은 있으나 변화가 u64 끝 근처에 있는 맵 없음 |
| 5. 공개 API 불변식 | 부분 | 단위 테스트로 교체·삭제만. 임의 조작 수열 뒤 캐시 일치 검사 없음 |
| 6. MeterMap 무차별 비교 | 부분 | `meter_map_bar_start_inverts_position`은 역 관계만 보고 분자 ≤ 16. 255/64, 1틱 간격, u64 끝 변화 없음 |
| 7. 분모 128·256 | 없음 | 128을 거부하는 테스트만 있다(`time_signature_rejects_zero_and_non_power_of_two`) |
| 8. 템포 단위 결정 | 해당 없음 | 계획 「리스크」에 적었으나 `docs/decisions/`에 결정 요청이 없다 |
| 9. 1시간 게이트 | 있음 | `tempo_map_one_hour_at_120_is_exact` |
| 10. 실시간 경로 | 해당 없음 | 코드로 확인(6절) |
| 11. bench-check 결함 | 부분 | 구현자가 회귀 검출·결과 없음 2건만 수동 확인 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-core/tests/review_s0_t3.rs`(통합 테스트, 공개 API만) 또는 임시 디렉터리에서 실행했다. 테스트가 실제로 무언가를 검사하는지 보려고 제품 코드에 변이 두 개(`MeterMap::rebuild`의 `div_ceil` → `/`, `time_to_tick`의 분자에 +1)를 임시로 넣어 4개 테스트가 실패하는 것을 확인하고 되돌렸다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 임의 조작 수열(설정·삭제 섞기, 틱 0·작은 값·임의 u64·u64 끝 1,000 안쪽, 템포 1·최대·임의) 2,048 사례 뒤 맵 내용·정렬·틱 0 항목, 같은 내용을 틱 순으로 새로 만든 맵과의 `PartialEq`(캐시 포함) | 못 깨뜨림 | `review_tempo_map_matches_reference_after_random_ops` |
| 2 | 같은 사례에서 0·1·u64 끝·각 변화 틱 ±1의 `tick_to_time`을 기준 모델(BTreeMap을 걸으며 u128 합)과 비교, 왕복 0틱, `tempo_at` | 못 깨뜨림 | 위와 같음 |
| 3 | 같은 사례에서 각 구간 시작 시각과 그 ±1 단위, 0·1·임의 u128·`u128::MAX`의 `time_to_tick`을 기준 모델(내림, u64 끝에서 자름)과 비교 | 못 깨뜨림 | 위와 같음 |
| 4 | 최소·최대 템포가 번갈아 `u64::MAX/2`, `u64::MAX-2`, `u64::MAX-1`, `u64::MAX`에 있는 맵: 오버플로·패닉, 마지막 틱 이후 시각의 자름 | 못 깨뜨림 | `review_tempo_map_extreme_tempos_at_u64_end_match_reference` |
| 5 | 1틱 간격 변화 64개(1 µs ↔ 최대 µs 번갈아, 역순 삽입): 매 틱 시각과 그 시각 −1 단위의 `time_to_tick` | 못 깨뜨림 | `review_tempo_map_one_tick_apart_changes_match_reference` |
| 6 | `from_bpm`: 1·2·3·4·7·90·120·140·999·7,746·120,000,000·120,000,001·`u32::MAX`를 독립 반올림 계산과 비교, 범위 밖은 오류 | 못 깨뜨림(패닉·오버플로 없음). 40–240 BPM 중 90·140 등이 정확히 표현되지 않음을 확인(지적 #1 근거) | `review_tempo_from_bpm_rounding_and_bounds` |
| 7 | `MeterMap`을 틱을 하나씩 세는 무차별 모델과 비교: 255/64·1/64·1/1 포함 임의 박자표, 마디 중간 변화, 1틱 간격 연속 변화 최대 5개, 0..8,000틱 전부의 `position`과 각 마디의 `bar_start`, 512 사례 | 못 깨뜨림 | `review_meter_map_matches_brute_force` |
| 8 | `MeterMap` u64 끝: `u64::MAX-20,000`에 255/64, `u64::MAX-2`에 1/64, `u64::MAX-1`에 255/64, `u64::MAX`에 7/8. `position` → `bar_start` 재구성, 마지막 변화가 새 마디를 시작하는지, 그 다음 마디의 자름 | 못 깨뜨림. 단 `bar_start(last)`와 `bar_start(last + 1)`이 둘 다 `u64::MAX`라 구분이 안 된다(제안 #6) | `review_meter_map_u64_end_is_consistent` |
| 9 | 1시간 게이트 정정 검산: 120 BPM은 초당 2 × 960 = 1,920틱, × 3,600 = 6,912,000. 60 BPM은 3,456,000. 90 BPM 반올림 드리프트 0.333 µs × 5,400박 ≈ 1.8 ms(보고서 수치) | 정정이 맞다 | 없음(산수) |
| 10 | `bench-check.py`: 결과 0.04 ns 벤치로 `--update` 후 비교 | 깨짐. 기준값이 `0.0`으로 반올림돼 기록되고 다음 비교에서 `ZeroDivisionError`(종료 코드 1이지만 판정이 아니라 예외) | 임시 디렉터리(스크립트 사본) |
| 11 | `bench-check.py`: 같은 `full_id`의 결과가 두 디렉터리에 있을 때(예: 낡은 `zz_old`는 100 ns, 현재 `aa_cur`는 500 ns, 기준 100 ns) | 깨짐. glob 순서상 나중 것이 이겨 `ok +0.0%`, 종료 코드 0. 400% 회귀를 놓친다. 디렉터리 이름을 바꾸면 FAIL이 되어 순서 의존이다 | 임시 디렉터리 |
| 12 | 실제 저장소에서 `cargo bench ... -- tick_to_time`만 돌린 뒤 `bench-check.py` | 깨짐. `time_to_tick_1000`은 측정하지 않았는데 이전 실행의 결과로 `ok`가 나온다(파일 수정 시각으로 확인). 낡은 결과를 거르지 않는다 | 없음(실행 기록) |
| 13 | `CARGO_TARGET_DIR`을 다른 곳으로 두고 `bench-check.py` | 깨짐(경미). 환경 변수를 무시하고 `target/criterion`만 본다. 결과가 없다고 실패하거나, 그 자리에 낡은 결과가 있으면 그것을 비교한다 | 임시 디렉터리 |
| 14 | 비교 방향: 실제 벤치 실행 후 비교(+0.1%, +1.5% → ok), 400% 느린 결과 → FAIL | 못 깨뜨림. `ratio > 1.1`이면 실패로 방향이 맞다. 기준값에만 있는 벤치는 FAIL, 결과에만 있는 벤치는 `new`로 표시만 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 템포·박자표 맵에 틱 0 항목이 있다 | 아니오 | `remove(Tick::ZERO)`는 `RemoveInitialChange`. `set(0)`은 교체. 시도 1 |
| 틱 순 정렬, 같은 틱 중복 없음 | 아니오 | `binary_search_by_key` 후 교체 또는 삽입. 시도 1·5 |
| 캐시(구간 시작 시각, 시작 마디 번호)가 내용과 일치 | 아니오 | 모든 변경 경로(`set`, `remove`)가 `rebuild`를 부른다. 필드는 비공개, `Project::tempo_map_mut`도 같은 메서드만 노출. 시도 1·7 |
| 구간 시작 시각 엄격 증가(`time_to_tick`의 전제) | 아니오 | 틱이 서로 다르고 템포 ≥ 1 µs라 길이 ≥ 1000 단위. 시도 3·5 |
| 시작 마디 번호 엄격 증가(`bar_start`의 전제) | 아니오 | 구간 길이 ≥ 1틱이라 `div_ceil` ≥ 1. 시도 7·8 |
| 템포 범위 1..=24비트, 박자표 분자 ≥ 1·분모 2의 거듭제곱 | 아니오 | 생성자만 있고 필드 비공개 |

공개 API만으로 깨지는 불변식은 찾지 못했다.

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| REQ 2 템포·박자표 변화(모델은 맵, SMF 변화 보존) | 미충족(부분) | 맵은 있다(`tempo.rs:109`, `meter.rs:87`). 그러나 SMF가 담을 수 있는 분모 128·256 박자표를 거부한다(`meter.rs:17-24`, 지적 #2). 3,840틱을 나누어떨어지게 하므로(30·15틱) 표현 가능한데 막혀 있어 「보존」이 안 된다 |
| REQ 2 새 프로젝트 기본값(120, 4/4, 조성 없음) | 충족 | `tempo.rs:12`, `meter.rs:12`, `project.rs` `Default`. 트랙 2개는 S4-T5 범위(`plan/stages/14-S4-clips-and-arrangement.md:35`) |
| SEQ-1 프로젝트 = 템포 맵 + 박자표 맵 + 트랙 목록 | 충족 | `project.rs` 필드 `tempo_map`, `meter_map`, `tracks` |
| SEQ-5 전역 이벤트, 템포 맵을 누적해 틱↔초 | 충족 | `tempo.rs:158-199`. 시도 1–5로 기준 모델과 일치 확인 |
| SEQ-6 PPQ 960 고정 | 충족(이 Task 범위) | `tick.rs` `PPQ`, `tempo.rs:53-56`. SMF 재배율은 S1 |
| SEQ-9 SMF 가져오기·내보내기 | 이 Task 범위 밖이나 전제 타입에 결함 | 템포는 µs 24비트로 무손실. 박자표 분모 128·256은 거부(지적 #2) |
| NF-4 1시간 드리프트 ≤ 1 ms (준비) | 충족(이 Task 범위) | 정수 산술, `tempo_map_one_hour_at_120_is_exact`. 게이트 수치 정정(6,912,000틱)도 맞다(시도 9) |
| 9.2 스케줄러가 템포 맵으로 틱→시각 | 충족(이 Task 범위) | 변환 경로에 할당·락 없음(6절) |
| TRN-4 템포 필드(입력·끌기·탭) | 판단 보류 → 결정 요청 필요 | 정수 µs 저장은 REQ에 없는 해석이다(지적 #1) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `tick_to_time`, `time_to_tick`, `tempo_at`은 `Vec` 읽기, `partition_point`, 정수 곱·나눗셈뿐이다(`tempo.rs:167-199`). `set`/`remove`는 `Vec::insert`로 할당하지만 편집 경로다. 스케줄러가 맵을 어떻게 공유받을지(스냅샷 교체)는 S1 몫이다.
- 제품 코드의 `unwrap`/`expect`: 없음. `unwrap_or`가 세 곳 있다.
  - `tempo.rs:39`: `us ≤ 60,000,000`이라 도달 불가. 정당.
  - `tempo.rs:197`: u64 틱 범위를 넘는 시각을 `u64::MAX`로 자름. 문서화됨(`tempo.rs:190`). 정당.
  - `meter.rs:164`: 분자 ≤ 255라 도달 불가. 정당.
- 에러가 삼켜지는 곳: `time_to_tick`과 `bar_start`의 포화(자름). 둘 다 문서화되어 있고 u64 틱 끝(120 BPM에서 약 3억 년)에서만 생긴다. `bar_start`는 존재하지 않는 마디와 `u64::MAX`에서 시작하는 실제 마디를 구분하지 못한다(제안 #6).
- 패닉 후보 점검: `partition_point(..) - 1`(3곳)은 첫 항목이 틱 0·시각 0·마디 0이라 항상 ≥ 1. `rebuild`의 틱 뺄셈은 정렬로 안전. `time.0 - start_time.0`은 `start_time ≤ time`인 구간을 골랐으므로 안전. 0 나눗셈은 템포 ≥ 1, `bar_ticks` ≥ 60이라 없음. `rebuild` 곱셈 상한은 `u64::MAX × 16,777,215,000 ≈ 3.1e29 < u128::MAX`. 시도 1–8에서 패닉 없음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 템포 저장 단위를 「4분음표당 정수 µs」로 정한 것은 REQ에 없는 결정이다. REQ는 템포를 BPM으로만 말하고(2절 「템포 120」, TRN-4 입력·끌기·탭 템포), 이 단위는 곧 `.ksq` 포맷(S0-T6, IO-1)에 그대로 저장된다. 결과로 60,000,000을 나누지 않는 정수 BPM(90, 140, 70, 110, 130 등)과 탭 템포의 소수 BPM이 정확히 표현되지 않는다(90 BPM은 89.99996, 1시간에 약 1.8 ms. 시도 6). 구현자도 계획 「리스크」에서 「다른 해석이 나오면 결정 요청」이라 적었다. 리뷰어 판단: CLAUDE.md 규칙(REQ에 없는 결정은 `docs/decisions/`에 결정 요청)에 따라 결정 요청을 써야 한다. 선택지 예: A 정수 µs(SMF 무손실, 현재 구현), B BPM 유리수(예: 1/1000 BPM 단위) 저장 후 SMF 입출력에서 µs로 변환, C 둘 다 보관. 포맷에 박히기 전에 결정되어야 하므로 이 Task에서 요청을 내는 것이 맞다. 코드 변경이 필요한지는 결정에 따른다 | `crates/keyscore-core/src/tempo.rs:3-8`, `27-40`; `docs/reports/s0-t3/plan.md` 「리스크」 |
| 2 | 수정 | `TimeSignature::new`가 분모 128·256을 거부한다. 주석은 「512 이상은 온음표(3840틱)를 나누어떨어지게 하지 못한다」를 근거로 들지만 128(30틱)·256(15틱)은 나누어떨어진다. SMF 박자표 메타는 분모를 2^dd로 담으므로 x/128 등은 합법이고, REQ 2절은 「SMF의 변화는 보존」을 요구한다. 「실용 범위를 넘는다」는 것은 REQ에 없는 제한이다. 128·256을 받거나, 거부한다면 SMF 가져오기에서 어떻게 처리할지(결정 요청 #1에 함께) 정해야 한다. 거부 테스트도 그에 맞춰야 한다 | `crates/keyscore-core/src/meter.rs:17-24`, 단위 테스트 `time_signature_rejects_zero_and_non_power_of_two`의 `128` |
| 3 | 수정 | `bench-check.py`가 같은 `full_id`의 결과 디렉터리가 둘 이상이면 glob 순서상 나중 것을 조용히 쓴다. 벤치를 `benchmark_group("tempo_map")`으로 옮기는 흔한 리팩터링만으로 낡은 평면 디렉터리(`tempo_map_tick_to_time_1000/`)와 새 디렉터리(`tempo_map/tick_to_time_1000/`)가 같은 `full_id`로 공존한다. 시도 11에서 400% 회귀를 `ok`로 통과시켰다. 중복 `full_id`는 실패로 처리해야 한다 | `scripts/bench-check.py:21-27` |
| 4 | 수정 | `bench-check.py`가 낡은 결과를 거르지 않는다. 필터를 건 `cargo bench`나 일부 벤치의 빌드 실패 뒤에도 이전 결과로 `ok`가 나온다(시도 12). 회귀 검사의 목적(10% 이상 느려지면 실패)을 우회한다. 예: 스크립트가 직접 `cargo bench`를 돌리거나, 실행 전에 `target/criterion`을 비우거나, 결과 파일 시각을 실행 시작 시각과 비교 | `scripts/bench-check.py:21-27` |
| 5 | 수정 | 기준값이 0.1 ns 단위로 반올림되어 0.05 ns 미만 벤치는 `0.0`으로 기록되고, 이후 비교가 `ZeroDivisionError`로 죽는다(시도 10). 현재 벤치(7–12 µs)에서는 생기지 않지만 회귀 검사 도구가 예외로 끝나는 경로다. 0 이하 기준값을 명시적 오류로 처리 | `scripts/bench-check.py:37`, `50` |
| 6 | 제안 | `bar_start`가 존재하지 않는 마디와 `u64::MAX`에서 시작하는 실제 마디를 같은 값(`u64::MAX`)으로 돌려준다(시도 8). `Option<Tick>`이나 `Result`가 호출자에게 더 정직하다. 실용 범위에서는 생기지 않는다 | `crates/keyscore-core/src/meter.rs:169-179` |
| 7 | 제안 | 마디 중간 박자표 변화를 「그 틱에서 새 마디, 직전은 짧은 마디」로 세는 규칙은 REQ에 없다. SMF 가져오기(S1)의 마디 번호에 직접 영향을 주므로 결정 요청 #1에 함께 적어 두기를 권한다 | `crates/keyscore-core/src/meter.rs:84-85`, `134-140` |
| 8 | 제안 | `plan/00-conventions.md:57`은 「10% 이상 느려지면 CI가 실패한다」인데 CI(`.github/workflows/ci.yml`)에 벤치 단계가 없다. 보고서 「알려진 한계」에 이유가 있고 S0 출구로 넘긴다고 했으나, 추적이 보고서 문장에만 있다. S0 출구 게이트 항목이나 결정 요청으로 남겨 두기를 권한다. `CARGO_TARGET_DIR`을 무시하는 점(시도 13)도 CI 캐시 설정에 따라 문제가 될 수 있다 | `scripts/bench-check.py:15-16`, `.github/workflows/ci.yml` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

변환 로직과 맵 불변식은 깨뜨리지 못했다. 기준 모델·무차별 모델과 경계·u64 끝·최소·최대 템포에서 모두 일치했고 패닉 경로도 없었다. 재리뷰가 필요한 이유는 지적 #1(REQ에 없는 템포 단위 결정의 결정 요청 누락), #2(SMF 박자표 보존을 막는 분모 제한), #3–#5(단계 게이트의 일부인 10% 회귀 검사 도구가 회귀를 놓치거나 예외로 끝남)다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`review_s0_t3.rs` 6개 통과, 변이 2개로 실패 확인 후 원복. bench-check 시도 10–13은 깨짐)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/10-S0-foundation.md`의 1시간 틱 수 정정은 산수 오류 정정이며 검산 결과 맞다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`criterion` 0.7, dev 전용, `default-features = false`. `cargo deny check` 통과, `check-deps.sh` 통과)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트 (리뷰어 실행)

- `cargo fmt --all -- --check`: 통과(리뷰 테스트 포함)
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과
- `cargo test --workspace`: 통과(`review_s0_t3` 6개 포함)
- `cargo deny check`: 통과(`advisories ok, bans ok, licenses ok, sources ok`. 기존 허용 항목 미사용 경고만)
- `scripts/check-deps.sh`: ok, `scripts/check-deps.test.sh`: 11 passed
- `cargo bench -p keyscore-core --bench tempo` 후 `python3 -I scripts/bench-check.py`: `tick_to_time_1000` +0.1%, `time_to_tick_1000` +1.5%, 종료 코드 0. `benches/baseline.json`은 바꾸지 않았다
