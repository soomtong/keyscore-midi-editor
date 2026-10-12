# Task 보고: S0-T3 템포 맵과 틱↔시각 변환

- PR: 로컬 브랜치 `s0/t3-tempo-map` (remote 없음)
- 계획 파일: docs/reports/s0-t3/plan.md
- 계획 대비 달라진 점
  - 단계 파일 게이트의 「1시간(120 BPM, 3,456,000틱)」은 산수 오류다. 120 BPM에서 1시간은 6,912,000틱이고, 3,456,000틱은 60 BPM의 1시간(120 BPM에서는 30분)이다. 처음에는 단계 파일 수치를 그대로 옮긴 테스트가 실패했다. 테스트는 세 경우(120 BPM 6,912,000틱 = 1시간, 120 BPM 3,456,000틱 = 30분, 60 BPM 3,456,000틱 = 1시간)를 모두 정확히 검사하도록 고쳤고, 단계 파일 수치도 정정했다(`plan:` 커밋).
  - `MusicTime::from_units`를 공개했다. 속성 테스트와 이후 스케줄러가 단위 값을 직접 다루기 위해서다.
  - 벤치 기준값 검사(`scripts/bench-check.py`)는 CI에 넣지 않았다(아래 「알려진 한계」).

## 구현 요약

- `src/tempo.rs`
  - `Tempo`: 4분음표당 µs, 1..=24비트, SMF 단위. `from_bpm`은 반올림한다.
  - `MusicTime`: 1/960 ns 단위의 u128.
  - `TempoMap`: 틱 0 항목이 항상 있고, 구간 시작 시각을 캐시한다. 변환은 이진 탐색 한 번에 곱셈 또는 나눗셈 한 번이다.
  - `time_to_tick`은 내림이고, u64 틱 범위를 넘는 시각은 `u64::MAX`로 자른다.
- `src/meter.rs`
  - `TimeSignature`: 분모는 1–64 사이의 2의 거듭제곱이다.
  - `BarPosition`: 마디, 박, 박 안 틱. 모두 0부터 센다.
  - `MeterMap`: 변화마다 시작 마디 번호를 캐시한다. 마디 중간 변화는 새 마디를 시작하고, 끊긴 마디도 한 마디로 센다.
- `src/project.rs`: `Project`가 `TempoMap`·`MeterMap`을 가진다. 기본값은 120 BPM, 4/4다.
- `src/error.rs`: `TempoOutOfRange`, `TimeSignatureInvalid`, `RemoveInitialChange`를 추가했다.
- `tests/tempo_map.rs`: 속성 테스트 5개.
- `benches/tempo.rs`, `benches/baseline.json`, `scripts/bench-check.py`: 벤치, 기준값, 10% 회귀 검사.
- 의존 추가: `criterion`(dev, 0.7, `default-features = false`, `cargo_bench_support`). `check-deps.sh` 허용 목록에 이미 있었다.

## 테스트 결과

~~~
keyscore-core 전체: 62 passed (lib 40, model_invariants 4, 리뷰 테스트 13, tempo_map 5)
tempo_map 속성 테스트: 각 1024 사례(meter는 기본 256)
check-deps.test: 11 passed
bench-check: tick_to_time_1000 7.1 µs, time_to_tick_1000 11.9 µs (변환 1회당 7–12 ns)
~~~

테스트를 먼저 썼다(`2709320`). `todo!()` 뼈대에서 단위 11개와 속성 5개가 실패하는 것을 확인했다. 처음부터 통과한 것은 `project_new_has_default_tempo_and_meter` 하나다. `Default` 구현이 뼈대에 이미 있었다.

`bench-check.py`는 기준값을 15% 낮춘 사본에서 `FAIL ... +18.3%`, 기준값에만 있는 벤치는 `FAIL ... 결과 없음`으로 종료 코드 1을 내는 것을 확인했다.

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `tempo_from_bpm_120_is_500000_micros` | 통과 |
| 2 | `tempo_map_one_quarter_at_120_is_half_second` | 통과 |
| 3 | `tempo_map_one_hour_at_120_is_exact` | 통과(수치 정정) |
| 4 | `tempo_map_three_changes_round_trip_is_exact` | 통과 |
| 5 | `tempo_map_time_to_tick_is_floor` | 통과 |
| 6 | `tempo_map_tick_to_time_is_strictly_increasing` | 통과 |
| 7 | `tempo_map_segment_sum_equals_direct_conversion` | 통과 |
| 8 | `tempo_map_set_replaces_and_remove_keeps_tick_zero`, `meter_map_set_replaces_and_remove_keeps_tick_zero` | 통과 |
| 9 | `time_signature_rejects_zero_and_non_power_of_two` | 통과 |
| 10 | `meter_map_position_crosses_changes` | 통과 |
| 11 | `meter_map_mid_bar_change_starts_new_bar` | 통과 |
| 12 | `meter_map_bar_start_inverts_position` | 통과 |
| 13 | `project_new_has_default_tempo_and_meter` | 통과 |
| 14 | 벤치 2개 + `benches/baseline.json` | 기준값 커밋 |
| 추가 | `tempo_map_two_segments_are_summed_exactly`, `tempo_map_time_beyond_u64_ticks_saturates`, `meter_map_bar_start_saturates` | 통과 |

## 자동 게이트

- [x] fmt (한 번 놓쳐 커밋 뒤 고쳤다. 아래 「리뷰어에게」 참고)
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] `scripts/check-deps.sh`, `scripts/check-deps.test.sh` 11/11
- [x] 단계 추가 게이트: criterion 기준값 커밋, `scripts/bench-check.py` 통과(로컬)

## 알려진 한계

- 벤치 10% 회귀 검사는 CI에 없다. 기준값을 이 개발 Mac(M1 Max)에서 잡았기 때문이다. 기기가 다른 CI 러너에서 비교하면 거짓 실패가 난다. remote가 생기면 러너에서 `--update`로 기준값을 잡고 CI 단계를 추가해야 한다. S0 출구 보고서의 수동 항목으로 넘긴다.
- 템포 단위가 µs라서, 60,000,000을 나누어떨어지게 하지 않는 BPM(예: 90)은 반올림된다(계획의 「리스크」). 모델과 스케줄러 사이 드리프트와는 무관하다.
- SMF 가져오기의 PPQ 재배율(SEQ-6 후반)은 S1이다.

## 리뷰어에게

- 핵심은 `TempoMap::rebuild`, `tick_to_time`, `time_to_tick`, `MeterMap::rebuild`, `bar_start`다. 경계(변화 틱 정확히, u64 끝, 시각이 구간 시작과 같을 때)를 봐 달라.
- `time_to_tick`의 구간 탐색은 「구간 시작 시각이 엄격히 증가한다」에 기대고 있다. 이 가정이 깨질 수 있는 공개 API 경로가 있는지 봐 달라.
- 템포 저장 단위(µs) 해석이 REQ와 맞는지(계획의 「리스크」) 봐 달라. 다른 해석이 맞다면 결정 요청 사안이다.
- 과정상 실수가 두 번 있었다. (1) 계획 파일이 구현 커밋에 섞여서 커밋을 나눴다. (2) `fmt --check` 실패 뒤에도 셸이 멈추지 않아 포맷 안 된 벤치 파일이 커밋됐고, 병합 전에 같은 커밋으로 고쳤다.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | 템포 저장 단위를 결정 요청 없이 정함 | 받음. 사람에게 물었고 「4분음표당 µs」로 결정됐다(`docs/decisions/2026-10-12-tempo-unit-and-meter-changes.md` Q1). 코드 변경 없음 |
| 2 | 분모 128·256 거부 | 받음. `TimeSignature`의 분모를 u16으로 바꾸고 1–256의 2의 거듭제곱을 받는다. 512부터 거부 |
| 3 | `bench-check.py`가 중복 `full_id` 중 하나를 조용히 고름 | 받음. 매 실행을 새 `CRITERION_HOME`에 돌려 이전 실행 결과가 섞이지 않게 했다. 한 실행 안의 중복은 실패로 처리한다 |
| 4 | 낡은 결과를 거르지 않음 | 3과 같은 수정으로 해결. 측정하지 않은 벤치는 「결과 없음」으로 실패한다 |
| 5 | 기준값 0과 0 나눗셈 | 받음. `--update`는 0을 쓰지 않고(최소 0.001), 비교는 기준값 0 이하를 명시적으로 실패시킨다 |
| 6 | `bar_start`가 존재하지 않는 마디와 u64 끝 마디를 구분 못 함 (제안) | 받음. `Option<Tick>`을 돌려준다. 리뷰어 테스트의 포화 단언을 `None`으로 바꿨다(`7096c3e` 본문에 이유) |
| 7 | 마디 중간 박자표 변화 규칙이 REQ에 없음 (제안) | 받음. 결정 Q2로 확정했다(그 틱에서 새 마디). 코드 변경 없음 |
| 8 | 벤치 게이트가 CI에 없음, `CARGO_TARGET_DIR` 무시 (제안) | 일부 받음. `CARGO_TARGET_DIR` 문제는 3의 수정(결과를 직접 지정한 임시 디렉터리에 씀)으로 사라졌다. 벤치 게이트는 `plan/stages/18-S8-release.md` S8-T3에 이월로 적었다. 벤치를 돌리지 않는 스크립트 회귀 테스트 `scripts/bench-check.test.sh`(8개 사례)는 CI에 넣었다 |

테스트를 먼저 바꿨다(`7096c3e`). `todo!()` 뼈대에서 단위 5개, 리뷰 테스트 2개, 속성 1개가 실패하는 것을 확인했고, `bench-check.test.sh`는 기존 스크립트에서 6/8이 실패했다.

사고 기록: 기존 `bench-check.py`는 인자를 무시했다. 그래서 새 테스트의 `--update` 사례를 기존 스크립트로 돌렸을 때 저장소의 `benches/baseline.json`이 덮어써졌다. `git checkout`으로 바로 되돌렸고, 커밋된 기준값은 바뀌지 않았다. 새 스크립트는 `--baseline`으로 경로를 받고, 테스트는 임시 디렉터리에만 쓴다. 그 테스트의 「0을 쓰지 않음」 검사는 파일이 없어도 통과하는 결함이 있었다. 파일이 반드시 있어야 하도록 고쳤다.

~~~
keyscore-core 전체: 68 passed (lib 40, model_invariants 4, 리뷰 테스트 19, tempo_map 5)
check-deps.test: 11 passed
bench-check.test: 8 passed
bench-check(실측): tick_to_time_1000 +0.5%, time_to_tick_1000 +0.8%
~~~

## 리뷰 대응 (review-2, 판정: 병합 가능)

제안 9–12(`bench-check.py`의 `CRITERION_HOME` 전달 테스트, NaN 처리, 실패 메시지, 테스트의 `$RANDOM` 접미사)는 받되, 이 PR에서는 고치지 않는다. 벤치 게이트를 CI에서 실제로 켜는 S8-T3으로 이월했다(`plan/stages/18-S8-release.md`). 「병합 가능」 판정 뒤에 코드를 바꾸면 다시 리뷰해야 하고, 지금은 이 스크립트가 CI 게이트가 아니라 로컬 도구이기 때문이다.
