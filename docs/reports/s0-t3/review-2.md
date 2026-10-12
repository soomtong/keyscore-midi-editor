# 적대적 리뷰: S0-T3 템포 맵과 틱↔시각 변환 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `2f29d8f` (브랜치 `s0/t3-tempo-map` HEAD). 1회차(`d1c409c`) 이후 커밋 `e125841`, `7096c3e`, `64b7ccc`, `ed21202`, `9823a64`, `2f29d8f`
- 기준: `docs/reports/s0-t3/review-1.md`, `docs/decisions/2026-10-12-tempo-unit-and-meter-changes.md`, `plan/stages/10-S0-foundation.md` S0-T3

## 1. 코드만 읽고 적은 「내가 검사할 것」

1회차 이후 바뀐 `meter.rs`, `error.rs`, `bench-check.py`, `ci.yml`의 diff만 읽고 적었다.

1. `TimeSignature::new(n, d)`가 `u16` 전 범위(0..=65535)에서 정확히 「2의 거듭제곱이고 ≤ 256」만 받는가. 0, 255, 257, 512, 32768. 박 길이 `3840 / d`가 모든 허용 분모에서 정수이고 `d`박이 온음표와 정확히 맞는가.
2. 분모 256(박 15틱)에서 `position`이 박·틱을 무차별 계산과 같게 나누는가. 마디 중간 변화(결정 Q2)와 겹쳐도 같은가. `beat_ticks`의 `u16`→`u64` 변환 누락.
3. `bar_start`의 `None` 경계: 마디 시작이 정확히 `u64::MAX`에 걸릴 때 `Some`, 하나 더 가면 `None`. `checked_mul` 통과 후 `checked_add`만 실패하는 경우. 변화 틱이 0이 아닐 때의 오프셋.
4. `position(Tick::MAX)`가 돌려준 마디는 `bar_start`가 항상 `Some`이어야 한다(역 관계).
5. 오류 타입이 `TimeSignatureInvalid(u8, u16)`로 바뀌어 메시지·`PartialEq`가 맞는가. `u8` 분자 255와 `u16` 분모 256 조합.
6. 결정 Q1(µs 단위 유지, 코드 변경 없음)과 Q2(변화 틱에서 새 마디, 현재 동작)가 코드·테스트와 어긋나지 않는가.
7. `bench-check.py` 인자: `--results`가 없는 디렉터리, `--update`와 `--results` 조합, `--baseline`이 없는 파일, 알 수 없는 인자. `--update`가 `--baseline` 외 경로에 쓰는가.
8. `bench-check.py` `run_benches`: `CRITERION_HOME`이 criterion에 실제로 적용되는가(그렇지 않으면 낡은 `target/criterion`을 읽지는 않아도 결과가 없다고 실패). 환경에 이미 있는 `CRITERION_HOME`을 덮어쓰는가. `cargo bench` 실패 시 종료 코드 전파. 결과 디렉터리 구조 가정(`**/new/benchmark.json`, `estimates.json`).
9. `bench-check.py` 비교: NaN, 음수, 기준값 파일 없음, `full_id` 없는 JSON.
10. `bench-check.test.sh`: 저장소 파일을 건드리는가(`benches/baseline.json`, `__pycache__`), 무작위 이름(`$RANDOM`) 충돌, `run_benches` 경로(1회차 지적 3·4의 실제 수정)를 시험하는가.
11. 지적 1~5와 제안 6·8을 되돌리면 각각 테스트가 실패하는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 분모 전 범위 | 부분 | `time_signature_rejects_zero_and_non_power_of_two`가 표본 7+9개. 0..=65535 전수 없음 |
| 2. 분모 256 `position` | 부분 | 단위 테스트는 `beat_ticks`·`bar_ticks`만. 리뷰 1의 속성 테스트는 분모 `u8` 범위(`u16::from(initial.1)`)라 128·256을 만들지 않을 수 있음 |
| 3. `bar_start`의 `None` 경계 | 부분 | 기본 4/4의 `u64::MAX/3840` ±1만. 비0 변화 틱·255/256 없음 |
| 4. `position`↔`bar_start` 역 | 있음 | `review_meter_map_u64_end_is_consistent` |
| 5. 오류 타입 | 있음 | 단위 테스트가 `TimeSignatureInvalid(4, d)` 비교 |
| 6. 결정 Q2 | 있음 | `meter_map_mid_bar_change_starts_new_bar` |
| 7. `bench-check.py` 인자 | 부분 | `--results`/`--baseline`/`--update`는 있음. 기준값 파일 없음, 알 수 없는 인자 없음 |
| 8. `run_benches` | 없음 | `bench-check.test.sh`는 전부 `--results`로 벤치 실행 경로를 우회한다. 지적 3·4의 핵심 수정(새 `CRITERION_HOME`)을 되돌려도 8/8 통과 |
| 9. NaN·기준값 파일 없음 | 없음 | |
| 10. 테스트의 부작용·`$RANDOM` | 부분 | 모든 사례가 `--baseline "$work/..."`를 지정해 저장소 파일을 건드리지 않음. `$RANDOM` 충돌은 미대응 |
| 11. 되돌리기 검증 | 해당 없음 | 3절 |

## 3. 깨뜨리기 시도

모두 실제로 실행했다. 제품 파일은 고치지 않았고, 수정을 되돌리는 변이는 `git archive HEAD`로 만든 `/tmp` 사본에서만 했다(끝나고 삭제).

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `TimeSignature::new(1, d)`를 `d` = 0..=65535 전수로 호출: 허용은 정확히 {1,2,4,…,256}, `denominator()` 일치, `beat_ticks × d == 3840`, 거부는 `TimeSignatureInvalid(1, d)`, 분자 0은 거부, 255/256 허용 | 못 깨뜨림 | `time_signature_accepts_exactly_powers_of_two_up_to_256` |
| 2 | 분모 256·분자 7(마디 105틱)을 틱 100(마디 중간)에 설정하고 4마디(420틱)를 틱 단위로 `position`·`bar_start`를 무차별 계산과 비교 | 못 깨뜨림 | `meter_map_denominator_256_positions_match_brute_force` |
| 3 | `bar_start` 경계: 기본 4/4에서 `u64::MAX/3840`은 `Some`, +1과 `u64::MAX`는 `None`. 틱 19217(비0)에서 255/256(3825틱/마디)로 바꾼 맵에서 마지막으로 들어가는 마디는 `Some`, 다음은 `None`. `position(Tick::MAX)`의 마디가 그 마지막 마디 | 못 깨뜨림 | `bar_start_none_boundary_is_exact_for_default_and_denominator_256` |
| 4 | 마지막 변화(255/256)가 `u64::MAX-1`에 있는 맵에서 마디 번호 `u64::MAX`까지 10,000개와 0..10,000에 `bar_start` 호출(패닉 여부), `position(Tick::MAX)`로부터 `start + beat×15 + tick == u64::MAX` 재구성 | 못 깨뜨림 | `bar_start_never_panics_for_any_bar_near_u64_max` |
| 5 | 변이: `denominator > 256`을 `> 64`로(지적 2 되돌림) | 단위 테스트 1개와 이번 리뷰 테스트 4개 실패. 리뷰 1의 통합 테스트는 통과(분모 128·256을 만들지 않음) | 위 테스트들 |
| 6 | 변이: `bar_start`가 범위 밖에서 `Some(Tick::MAX)`로 자르게(제안 6 되돌림) | `meter_map_bar_start_beyond_u64_is_none`, `review_meter_map_u64_end_is_consistent`, 이번 리뷰의 경계 테스트 실패 | 위 테스트들 |
| 7 | 변이: `bench-check.py`의 중복 검사 제거 / 기준값 0 검사 제거 / `--update`의 최소값 제거 / 결과 없음 검사 제거 / 임계값 10%→100%(지적 3·5와 비교 로직 되돌림) | 5개 변이 모두 `bench-check.test.sh`가 정확히 해당 사례 1개를 실패시킴. 단 「기준값 0 검사 제거」와 「결과 없음 검사 제거」 변이는 종료 코드는 여전히 실패이고 출력 문자열이 달라서 잡힌다(문자열까지 요구하는 테스트라 올바름) | 없음 |
| 8 | 변이: `CRITERION_HOME` 지정 제거(지적 4 되돌림) | 깨뜨림(테스트가 못 잡음): `bench-check.test.sh` 8/8 통과. `run_benches`는 시험되지 않는다(제안 9) | 없음 |
| 9 | 실제 `python3 -I scripts/bench-check.py` 전체(벤치 실행 포함, 17초) | 못 깨뜨림. `tempo_map/tick_to_time_1000` -0.2%, `time_to_tick_1000` +0.9%, 종료 코드 0. `target/criterion`의 수정 시각이 실행 전후 같음(낡은 평면 디렉터리를 읽지 않음, `CRITERION_HOME`이 criterion에 실제로 적용됨). `benches/baseline.json` 해시 불변 | 없음 |
| 10 | 가짜 `cargo`를 PATH 앞에 두고: (a) 종료 코드 101 (b) 아무것도 안 쓰고 0 종료 (c) 환경 `CRITERION_HOME=/nonexistent`가 이미 설정됨 | (a) `CalledProcessError` 트레이스백, 종료 코드 1 전파(통과하지는 않음. 메시지만 거칢). (b) 「결과가 없다」, 1. (c) 스크립트가 만든 임시 디렉터리로 덮어씀, 낡은 환경값 영향 없음 | 없음 |
| 11 | 인자 조합: `--results /nonexistent`, `--baseline` 없는 파일, 알 수 없는 인자 | `--results` 없는 경로는 「결과가 없다」 1. 기준값 파일이 없으면 `FileNotFoundError` 트레이스백(종료 코드 1, 실패는 맞음). 알 수 없는 인자는 argparse 오류. 저장소 파일 변화 없음 | 없음 |
| 12 | 결과의 평균이 `NaN`인 가짜 결과와 기준값 100 | 깨뜨림(경미): `ok a: nan ns (+nan%)`, 종료 코드 0. `nan > 1.1`이 거짓이라 비교가 열린 채로 통과한다. criterion이 실제로 NaN을 쓰는 경우는 보지 못했다(제안 10) | 없음 |
| 13 | `--update`가 쓰는 경로: 인자 없이 쓰면 `benches/baseline.json`, 아니면 `--baseline`뿐. `bench-check.test.sh` 실행 전후 `git status` | 못 깨뜨림. 저장소 밖 임의 경로에 쓰는 코드 없음(`mkdir parents`는 `--baseline` 부모만). 테스트 후 저장소 변화 없음 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 박자표 분자 ≥ 1, 분모 2의 거듭제곱 ≤ 256 | 아니오 | 필드 비공개, 생성자만. 시도 1 |
| 박 길이가 정수(3840/분모) | 아니오 | 분모 ≤ 256이면 항상 정수. 시도 1 |
| `bar_start(b)`가 `Some`이면 `position(그 틱)`의 마디가 `b` | 아니오 | 시도 3·4와 리뷰 1의 속성 테스트 |
| `bar_start`는 존재하지 않는 마디를 값으로 위장하지 않음 | 아니오 | `None`. 시도 3·6 |
| 1회차에서 점검한 템포·박자표 맵 불변식 | 아니오 | 1회차 이후 `tempo.rs` 변경 없음(`git diff 2a13356..HEAD --stat`) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| REQ 2 템포·박자표 변화(SMF의 변화는 보존) | 충족 | 분모 1–256 허용(`meter.rs:24`), 마디 중간 변화 보존(결정 Q2, `meter.rs:134-140`). 512 이상은 SMF에서 정수 틱으로 표현 불가이므로 가져오기 단계(S1)의 처리 문제로 남는다(이 Task 밖) |
| REQ 2 새 프로젝트 기본값 | 충족 | 1회차와 같음 |
| SEQ-1, SEQ-5, SEQ-6 | 충족 | 1회차 이후 `tempo.rs` 변경 없음 |
| SEQ-9 SMF 가져오기·내보내기(전제 타입) | 충족(이 Task 범위) | 템포 µs 무손실(결정 Q1), 박자표 분모 128·256 수용 |
| NF-4 1시간 드리프트 | 충족(이 Task 범위) | 1회차와 같음 |
| TRN-4 템포 필드 | 충족(결정 Q1) | UI는 BPM을 소수 셋째 자리로 반올림해 보인다는 결정이 S2에 이월됨 |
| 9.2 스케줄러가 템포 맵으로 틱→시각 | 충족(이 Task 범위) | 변환 경로 변경 없음 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 1회차 이후 `tempo_at`·`tick_to_time`·`time_to_tick` 변경 없음. `bar_start`의 `checked_mul`/`checked_add`/`Option`은 할당 없음.
- 제품 코드의 `unwrap`/`expect`: 없음(`meter.rs`의 `unwrap`은 `#[cfg(test)]` 안). 1회차의 `unwrap_or` 세 곳 중 `meter.rs:164`는 분자 ≤ 255, `bar_ticks`가 u64로 항상 들어가 도달 불가이며 그대로. `bar_start`의 `map_or(u64::MAX)`는 사라지고 `Option`이 됨.
- 에러가 삼켜지는 곳: `time_to_tick`의 포화(문서화됨). `bar_start`의 포화는 제거됐다. 제품 외 스크립트에서는 `bench-check.py`가 NaN을 통과시킨다(시도 12).

## 7. 지적 사항

1회차 지적 1~5와 제안 6·8은 모두 해결을 확인했다(3절 5·6·7·9·10).

| 1회차 # | 해결 여부 | 근거 |
| --- | --- | --- |
| 1 템포 단위 결정 요청 | 해결 | 결정 파일 Q1(A, 정수 µs). 코드 변경 없음이 결정과 일치 |
| 2 분모 128·256 | 해결 | 시도 1·2, 되돌리면 실패(시도 5) |
| 3 중복 `full_id` | 해결 | 중복 검사와 새 `CRITERION_HOME`. 검사만 제거하면 테스트 실패(시도 7), 실제 실행에서 낡은 디렉터리 무시(시도 9) |
| 4 낡은 결과 | 해결(테스트 없음) | 시도 9·10에서 동작 확인. 테스트로는 보호되지 않음(시도 8, 제안 9) |
| 5 기준값 0·0 나눗셈 | 해결 | `--update` 최소값 0.001과 `<= 0` 검사, 각각 변이로 확인(시도 7) |
| 6 `bar_start` | 해결 | `Option<Tick>`, 시도 3·6 |
| 8 벤치 CI | 해결(일부, 수용) | `bench-check.test.sh`가 CI에 있고(`ci.yml`) 벤치 게이트는 S8-T3 이월(`18-S8-release.md`)로 추적됨. 기준값이 러너에서 의미가 없다는 이유가 타당하다 |

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 9 | 제안 | 지적 3·4의 핵심 수정인 `run_benches`(새 `CRITERION_HOME`)가 테스트로 보호되지 않는다. `CRITERION_HOME` 지정 한 줄을 지워도 `bench-check.test.sh`가 8/8 통과한다(시도 8). 가짜 `cargo`(PATH 앞에 둔 셸 스크립트)로 「`CRITERION_HOME`이 매 실행 새 빈 디렉터리로 넘어가고 `cargo`가 0 아닌 값으로 끝나면 스크립트도 실패한다」를 시험할 수 있다. 수정으로 올리지 않은 이유: 동작은 리뷰어가 실제 실행으로 확인했고(시도 9·10), 대상이 제품 불변식이 아닌 보조 스크립트이며, 게이트 자체는 S8에서 CI에 들어갈 때 다시 보게 된다. 거절하면 이유를 보고서에 적는다 | `scripts/bench-check.py:36-38`, `scripts/bench-check.test.sh` |
| 10 | 제안 | 결과 평균이 `NaN`이면 `nan > 1.1`이 거짓이라 `ok`로 통과한다(시도 12). `ratio != ratio` 또는 `math.isfinite` 검사로 실패시키면 열린 채로 통과하는 경로가 닫힌다. 실제 criterion 출력에서 생길 가능성은 낮다 | `scripts/bench-check.py:75-77` |
| 11 | 제안 | `cargo bench` 실패나 기준값 파일 없음이 파이썬 트레이스백으로 끝난다(시도 10·11). 종료 코드 1은 맞다. 「cargo bench가 N으로 실패」「기준값 파일이 없다. --update로 만든다」 같은 한 줄 메시지가 사람에게 낫다 | `scripts/bench-check.py:37`, `63` |
| 12 | 제안 | `bench-check.test.sh`의 `fake_result`가 디렉터리 이름 접미사로 `$RANDOM`(0..32767)을 쓴다. 사례 r4(같은 `full_id` 두 개)에서 두 번 연속 같은 값이 나오면(약 1/32768) 디렉터리가 하나로 합쳐져 「중복」 사례가 거짓으로 실패한다. 증분 카운터로 바꾸면 결정적이다 | `scripts/bench-check.test.sh:12` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

1회차의 수정 5건과 제안 2건이 모두 해결됐고, 되돌리면 테스트가 실패한다(`run_benches`의 새 `CRITERION_HOME`만 예외로 제안 9에 적음). 분모 1–256 경계와 `bar_start`의 `None` 경계를 공개 API로 깨뜨리지 못했다. 차단·수정 지적은 없다. 제안 9–12는 구현자가 거절할 수 있다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`review_s0_t3_r2.rs` 4개 통과, 변이 5·6·7·8, 실제 벤치 실행, 가짜 `cargo`)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`git diff 2a13356..HEAD --stat`의 11개 파일 모두 Task, 리뷰 대응, 결정, S8 이월 항목)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`tempo_map.rs`는 `unwrap()` 추가만, `meter.rs` 단위 테스트는 128을 허용 목록으로 옮김. 이름 변경 `meter_map_bar_start_saturates` → `..._beyond_u64_is_none`은 의미 변경에 따른 것으로 결정 파일과 일치)
- [x] 새 의존성이 있으면 라이선스를 확인했다(1회차 이후 새 의존성 없음. `cargo deny check` 통과)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트 (리뷰어 실행)

- `cargo fmt --all -- --check`: 통과(리뷰어 테스트 파일을 `rustfmt`로 정리한 뒤)
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과
- `cargo test --workspace`: 통과(`review_s0_t3_r2` 4개 포함)
- `cargo deny check`: 통과(`advisories ok, bans ok, licenses ok, sources ok`)
- `scripts/check-deps.sh`: 통과, `scripts/check-deps.test.sh`: 11 passed
- `scripts/bench-check.test.sh`: 8 passed
- `python3 -I scripts/bench-check.py`(벤치 포함): 종료 코드 0, `benches/baseline.json` 불변
