# Task 계획: S0-T3 템포 맵과 틱↔시각 변환

- 브랜치: `s0/t3-tempo-map`
- 관련 REQ: 2(템포·박자표 변화 행, 새 프로젝트 기본값 120·4/4), 6.1 SEQ-1·SEQ-5·SEQ-6, 8.1 NF-4, 9.2(스케줄러가 템포 맵으로 틱→시각 계산)
- 선행 Task: S0-T2
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

- `Project`가 템포 맵과 박자표 맵을 가진다(SEQ-1). 둘 다 틱 0에 항상 값이 있고, 새 프로젝트는 120 BPM·4/4다.
- 틱 → 시각, 시각 → 틱 변환이 정수 산술만으로 된다. 틱 → 시각은 정확하고(반올림 없음), 시각 → 틱은 내림이다. 그래서 「틱 → 시각 → 틱」 왕복 오차는 0이다.
- 시각의 단위는 1/960 나노초(u128)다. 틱 하나의 길이 `µs/qn × 1000 / 960` ns가 이 단위에서 정수(`µs/qn × 1000`)가 되므로, 템포 구간을 아무리 이어 붙여도 누적 오차가 생기지 않는다. 1시간(120 BPM, 6,912,000틱)은 정확히 3,600,000,000,000 ns다. 단계 파일의 「3,456,000틱」은 60 BPM 기준이라 정정한다(NF-4 준비).
- 박자표 맵으로 틱 ↔ (마디, 박, 박 안 틱) 변환과 마디 시작 틱 계산이 된다. 박의 단위는 박자표 분모다(6/8의 박은 8분음표).
- criterion 벤치(틱→시각, 시각→틱)와 기준값 `benches/baseline.json`, 10% 회귀 검사 스크립트가 있다.
- 원문과 다른 점: 원문의 `tick_to_seconds`/`seconds_to_tick`은 `tick_to_time`/`time_to_tick`으로 하고, 초는 `MusicTime::as_secs_f64`(표시용)로 얻는다. 부동소수를 변환 경로에서 빼기 위해서다.

## 공개 인터페이스 초안

~~~rust
// tempo.rs
pub struct Tempo(u32);                 // 4분음표당 µs, 1..=0xFF_FFFF (SMF 24비트)
impl Tempo {
    pub const DEFAULT: Tempo;          // 500_000 (120 BPM)
    pub fn from_micros_per_quarter(us: u32) -> Result<Self, ModelError>;
    pub fn from_bpm(bpm: u32) -> Result<Self, ModelError>;      // 60_000_000 / bpm, 나누어떨어지지 않으면 반올림
    pub fn micros_per_quarter(self) -> u32;
    pub fn bpm_f64(self) -> f64;       // 표시용
}
pub struct MusicTime(u128);            // 1/960 ns 단위
impl MusicTime {
    pub const ZERO: MusicTime;
    pub fn from_nanos(ns: u64) -> Self;
    pub fn as_nanos(self) -> u128;     // 내림
    pub fn as_secs_f64(self) -> f64;   // 표시용
    pub fn units(self) -> u128;
}
pub struct TempoMap { changes: Vec<(Tick, Tempo, MusicTime)> }   // 틱 순, 첫 항목은 틱 0, 세 번째는 구간 시작 시각(캐시)
impl TempoMap {
    pub fn new(initial: Tempo) -> Self;
    pub fn set(&mut self, at: Tick, tempo: Tempo);                // 같은 틱이면 교체
    pub fn remove(&mut self, at: Tick) -> Result<bool, ModelError>; // 틱 0은 지울 수 없음
    pub fn changes(&self) -> impl Iterator<Item = (Tick, Tempo)>;
    pub fn tempo_at(&self, at: Tick) -> Tempo;
    pub fn tick_to_time(&self, at: Tick) -> MusicTime;
    pub fn time_to_tick(&self, time: MusicTime) -> Tick;           // 내림
}
// meter.rs
pub struct TimeSignature { numerator: u8, denominator: u8 }  // 분자 1..=255, 분모 1·2·4·8·16·32·64
impl TimeSignature {
    pub const COMMON: TimeSignature;   // 4/4
    pub fn new(numerator: u8, denominator: u8) -> Result<Self, ModelError>;
    pub fn beat_ticks(self) -> u64;    // 3840 / 분모
    pub fn bar_ticks(self) -> u64;
}
pub struct BarPosition { bar: u64, beat: u32, tick: u64 }        // 0부터. 표시는 UI가 +1
pub struct MeterMap { changes: Vec<(Tick, TimeSignature, u64)> } // 세 번째는 그 변화가 시작하는 마디 번호(캐시)
impl MeterMap {
    pub fn new(initial: TimeSignature) -> Self;
    pub fn set(&mut self, at: Tick, sig: TimeSignature);
    pub fn remove(&mut self, at: Tick) -> Result<bool, ModelError>;
    pub fn changes(&self) -> impl Iterator<Item = (Tick, TimeSignature)>;
    pub fn signature_at(&self, at: Tick) -> TimeSignature;
    pub fn position(&self, at: Tick) -> BarPosition;
    pub fn bar_start(&self, bar: u64) -> Tick;
}
// Project에 tempo_map()/tempo_map_mut(), meter_map()/meter_map_mut() 추가
~~~

박자표 변화가 마디 중간에 오면(SMF에서 가져온 경우) 그 틱에서 새 마디가 시작한다. 직전 마디는 짧은 마디로 센다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `tempo_from_bpm_120_is_500000_micros` | 단위 | BPM 변환, 0·범위 밖 거부 |
| 2 | `tempo_map_one_quarter_at_120_is_half_second` | 단위 | 기본 변환 |
| 3 | `tempo_map_one_hour_at_120_is_exact` | 단위 | 120 BPM 6,912,000틱 = 60 BPM 3,456,000틱 = 3,600,000,000,000 ns 정확(NF-4) |
| 4 | `tempo_map_three_changes_round_trip_is_exact` | 속성 | 템포 변화 3개, 임의 틱 t: `time_to_tick(tick_to_time(t)) == t` (게이트) |
| 5 | `tempo_map_time_to_tick_is_floor` | 속성 | `tick_to_time(k) ≤ x < tick_to_time(k+1)`, k = `time_to_tick(x)` |
| 6 | `tempo_map_tick_to_time_is_strictly_increasing` | 속성 | 단조 증가 |
| 7 | `tempo_map_segment_sum_equals_direct_conversion` | 속성 | 구간별 길이 합 = 직접 변환(누적 오차 없음) |
| 8 | `tempo_map_set_replaces_and_remove_keeps_tick_zero` | 단위 | 같은 틱 교체, 틱 0 삭제 거부, 정렬 |
| 9 | `time_signature_rejects_zero_and_non_power_of_two` | 단위 | 분자 0, 분모 3·128 거부 |
| 10 | `meter_map_position_crosses_changes` | 단위 | 4/4 → 6/8 → 7/8 경계의 마디·박 |
| 11 | `meter_map_mid_bar_change_starts_new_bar` | 단위 | 마디 중간 변화 |
| 12 | `meter_map_bar_start_inverts_position` | 속성 | `position(bar_start(b)) == (b, 0, 0)`, `bar_start(position(t).bar) ≤ t` |
| 13 | `project_new_has_default_tempo_and_meter` | 단위 | 120 BPM, 4/4 |
| 14 | 벤치 `tick_to_time`, `time_to_tick` (변화 100개) | 벤치 | 기준값 커밋, 10% 회귀 검사 |

## 불변식

- 두 맵 모두 틱 0에 항목이 있고, 틱 순으로 정렬되어 있으며, 같은 틱 항목이 둘 이상이 아니다.
- 캐시(구간 시작 시각, 시작 마디 번호)는 `set`·`remove` 뒤 항상 다시 계산되어 원본과 맞다. 공개 API로 캐시만 바꿀 수 없다.
- 변환에 부동소수를 쓰지 않는다. `f64`는 표시용 함수에만 있다.

## 범위 밖

- 템포·박자표 편집 UI(TRN-4, S2), SMF 가져오기의 PPQ 재배율(SEQ-6 후반, S1), 스케줄러(S1), 퀀타이즈 그리드(S0-T7).
- 템포 램프(점진 변화). REQ에 없다.

## 리스크·결정 요청

- 템포를 4분음표당 µs(u32)로 담는다. 60,000,000을 나누어떨어지게 하지 않는 BPM은 µs 단위로 반올림된다. 예를 들어 90 BPM은 666,667 µs(89.99996 BPM)가 되어, 의도한 템포 대비 1시간에 약 1.8 ms 늦어진다. 이것은 「사용자가 친 BPM」과의 차이이며, 스케줄러와 모델 사이의 드리프트(NF-4)와는 무관하다. 소수 BPM(TRN-4 입력)도 같다. SMF가 이 단위라 SEQ-9 가져오기·내보내기는 무손실이다. REQ에 템포 저장 단위가 없어서 내린 해석이다. 리뷰에서 다른 해석이 나오면 결정 요청으로 돌린다.
- 벤치 10% 회귀 검사는 같은 기기에서만 의미가 있다. CI 러너는 기기가 바뀌므로, 기준값은 CI 러너에서 다시 잡아야 한다(remote가 생긴 뒤).
