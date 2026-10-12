use crate::{ModelError, PPQ, Tick};

/// 템포. 4분음표 하나의 길이를 마이크로초로 담는다.
///
/// SMF의 Set Tempo 메타 이벤트와 같은 단위(24비트)라 가져오기·내보내기가 무손실이고(SEQ-9),
/// 정수라서 틱↔시각 변환을 정확히 계산할 수 있다(SEQ-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tempo(u32);

impl Tempo {
    /// 120 BPM. 새 프로젝트 기본값(REQ 2).
    pub const DEFAULT: Tempo = Tempo(500_000);
    /// SMF Set Tempo의 최댓값(24비트).
    pub const MAX_MICROS: u32 = 0x00FF_FFFF;

    /// # Errors
    ///
    /// 0이거나 24비트를 넘으면 [`ModelError::TempoOutOfRange`].
    pub fn from_micros_per_quarter(us: u32) -> Result<Self, ModelError> {
        if (1..=Self::MAX_MICROS).contains(&us) {
            Ok(Tempo(us))
        } else {
            Err(ModelError::TempoOutOfRange(us))
        }
    }

    /// 정수 BPM에서 만든다. 60,000,000을 나누어떨어지게 하지 않으면 가장 가까운 µs로 반올림한다.
    ///
    /// # Errors
    ///
    /// 0이거나 결과가 µs 범위(1..=24비트) 밖이면 [`ModelError::TempoOutOfRange`].
    pub fn from_bpm(bpm: u32) -> Result<Self, ModelError> {
        if bpm == 0 {
            return Err(ModelError::TempoOutOfRange(0));
        }
        let bpm = u64::from(bpm);
        let us = (60_000_000 + bpm / 2) / bpm;
        // us ≤ 60,000,000이라 u32에 들어간다.
        Self::from_micros_per_quarter(u32::try_from(us).unwrap_or(u32::MAX))
    }

    #[must_use]
    pub const fn micros_per_quarter(self) -> u32 {
        self.0
    }

    /// 표시용 BPM. 변환 계산에는 쓰지 않는다.
    #[must_use]
    pub fn bpm_f64(self) -> f64 {
        60_000_000.0 / f64::from(self.0)
    }

    /// 틱 하나의 길이를 [`MusicTime`] 단위(1/960 ns)로. `µs × 1000 / 960` ns이므로 정수다.
    const fn units_per_tick(self) -> u128 {
        self.0 as u128 * 1000
    }
}

/// 곡 시작 기준 시각. 단위는 1/960 나노초다.
///
/// 틱 하나의 길이(`µs/qn × 1000 / 960` ns)가 이 단위에서 정수가 되므로, 템포 구간을 이어
/// 붙여도 반올림 오차가 누적되지 않는다(NF-4). u128이라 u64 틱 전 범위를 담는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MusicTime(u128);

impl MusicTime {
    pub const ZERO: MusicTime = MusicTime(0);
    /// 1나노초의 단위 수.
    pub const UNITS_PER_NANO: u128 = PPQ as u128;

    #[must_use]
    pub const fn from_units(units: u128) -> Self {
        MusicTime(units)
    }

    #[must_use]
    pub const fn from_nanos(ns: u64) -> Self {
        MusicTime(ns as u128 * Self::UNITS_PER_NANO)
    }

    #[must_use]
    pub const fn units(self) -> u128 {
        self.0
    }

    /// 나노초(내림).
    #[must_use]
    pub const fn as_nanos(self) -> u128 {
        self.0 / Self::UNITS_PER_NANO
    }

    /// 표시용 초. 변환 계산에는 쓰지 않는다.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "표시용이라 f64 정밀도로 충분하다"
    )]
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / (Self::UNITS_PER_NANO as f64 * 1e9)
    }
}

/// 템포 변화의 열(SEQ-1, SEQ-5). 틱 0에 항상 항목이 있다.
///
/// 각 항목은 그 구간이 시작하는 시각을 함께 들고 있다. 변환이 이진 탐색 한 번과 곱셈·나눗셈
/// 한 번으로 끝나야 스케줄러(S1)가 부담 없이 부를 수 있다. 이 캐시는 `set`·`remove`가 다시
/// 계산하며, 공개 API로 따로 바꿀 수 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempoMap {
    /// (변화 틱, 템포, 구간 시작 시각). 틱 순, 첫 항목은 틱 0.
    changes: Vec<(Tick, Tempo, MusicTime)>,
}

impl Default for TempoMap {
    fn default() -> Self {
        TempoMap::new(Tempo::DEFAULT)
    }
}

impl TempoMap {
    #[must_use]
    pub fn new(initial: Tempo) -> Self {
        TempoMap {
            changes: vec![(Tick::ZERO, initial, MusicTime::ZERO)],
        }
    }

    /// `at`에서 템포를 바꾼다. 같은 틱에 변화가 있으면 교체한다.
    pub fn set(&mut self, at: Tick, tempo: Tempo) {
        match self.changes.binary_search_by_key(&at, |c| c.0) {
            Ok(i) => self.changes[i].1 = tempo,
            Err(i) => self.changes.insert(i, (at, tempo, MusicTime::ZERO)),
        }
        self.rebuild();
    }

    /// `at`의 변화를 지운다. 지웠으면 `Ok(true)`, 없으면 `Ok(false)`.
    ///
    /// # Errors
    ///
    /// 틱 0은 지울 수 없다([`ModelError::RemoveInitialChange`]). 곡 시작의 템포는 항상 있어야 한다.
    pub fn remove(&mut self, at: Tick) -> Result<bool, ModelError> {
        if at == Tick::ZERO {
            return Err(ModelError::RemoveInitialChange);
        }
        match self.changes.binary_search_by_key(&at, |c| c.0) {
            Ok(i) => {
                self.changes.remove(i);
                self.rebuild();
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// 구간 시작 시각 캐시를 다시 계산한다. 앞 구간의 끝 = 다음 구간의 시작이며, 모두 정수다.
    /// u64 틱 전 범위 × 최대 템포(16,777,215,000 단위/틱)도 u128 안이다.
    fn rebuild(&mut self) {
        for i in 1..self.changes.len() {
            let (prev_tick, prev_tempo, prev_start) = self.changes[i - 1];
            let span = u128::from(self.changes[i].0.get() - prev_tick.get());
            self.changes[i].2 = MusicTime(prev_start.0 + span * prev_tempo.units_per_tick());
        }
    }

    /// `at`이 속한 구간의 인덱스. 첫 항목이 틱 0이라 항상 있다.
    fn segment_of_tick(&self, at: Tick) -> usize {
        self.changes.partition_point(|c| c.0 <= at) - 1
    }

    /// 틱 순서의 변화.
    pub fn changes(&self) -> impl Iterator<Item = (Tick, Tempo)> + '_ {
        self.changes.iter().map(|&(tick, tempo, _)| (tick, tempo))
    }

    #[must_use]
    pub fn tempo_at(&self, at: Tick) -> Tempo {
        self.changes[self.segment_of_tick(at)].1
    }

    /// 틱의 시각. 정확하다(반올림 없음).
    #[must_use]
    pub fn tick_to_time(&self, at: Tick) -> MusicTime {
        let (start_tick, tempo, start_time) = self.changes[self.segment_of_tick(at)];
        let span = u128::from(at.get() - start_tick.get());
        MusicTime(start_time.0 + span * tempo.units_per_tick())
    }

    /// 시각이 속한 틱(내림). `tick_to_time(k) ≤ time < tick_to_time(k + 1)`인 k다.
    /// u64 틱 범위를 넘는 시각은 `u64::MAX` 틱으로 자른다.
    #[must_use]
    pub fn time_to_tick(&self, time: MusicTime) -> Tick {
        // 구간 시작 시각은 틱이 서로 다르고 템포가 0이 아니라서 엄격히 증가한다.
        let i = self.changes.partition_point(|c| c.2 <= time) - 1;
        let (start_tick, tempo, start_time) = self.changes[i];
        let span = (time.0 - start_time.0) / tempo.units_per_tick();
        let span = u64::try_from(span).unwrap_or(u64::MAX);
        Tick::new(start_tick.get().saturating_add(span))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_from_bpm_120_is_500000_micros() {
        assert_eq!(Tempo::from_bpm(120), Ok(Tempo::DEFAULT));
        assert_eq!(
            Tempo::from_bpm(90).map(Tempo::micros_per_quarter),
            Ok(666_667)
        );
        assert_eq!(Tempo::from_bpm(0), Err(ModelError::TempoOutOfRange(0)));
        // 3 BPM은 20,000,000 µs로 24비트를 넘는다.
        assert_eq!(
            Tempo::from_bpm(3),
            Err(ModelError::TempoOutOfRange(20_000_000))
        );
        assert_eq!(
            Tempo::from_micros_per_quarter(0),
            Err(ModelError::TempoOutOfRange(0))
        );
        assert_eq!(
            Tempo::from_micros_per_quarter(0x0100_0000),
            Err(ModelError::TempoOutOfRange(0x0100_0000))
        );
        assert!((Tempo::DEFAULT.bpm_f64() - 120.0).abs() < 1e-9);
    }

    #[test]
    fn tempo_map_one_quarter_at_120_is_half_second() {
        let map = TempoMap::default();
        assert_eq!(map.tick_to_time(Tick::new(960)).as_nanos(), 500_000_000);
        assert_eq!(map.tick_to_time(Tick::ZERO), MusicTime::ZERO);
        assert_eq!(
            map.time_to_tick(MusicTime::from_nanos(500_000_000)),
            Tick::new(960)
        );
        assert_eq!(
            map.time_to_tick(MusicTime::from_nanos(499_999_999)),
            Tick::new(959)
        );
    }

    #[test]
    fn tempo_map_one_hour_at_120_is_exact() {
        // 120 BPM에서 1시간 = 7,200 4분음표 = 6,912,000틱. 단계 파일의 「1시간 = 3,456,000틱」은
        // 60 BPM 기준이라 120 BPM에서는 30분이다(보고서 참고). 둘 다 정확히 떨어져야 한다.
        let map = TempoMap::default();
        let hour = map.tick_to_time(Tick::new(6_912_000));
        assert_eq!(hour, MusicTime::from_nanos(3_600_000_000_000));
        assert_eq!(map.time_to_tick(hour), Tick::new(6_912_000));
        let half = map.tick_to_time(Tick::new(3_456_000));
        assert_eq!(half, MusicTime::from_nanos(1_800_000_000_000));

        let sixty = TempoMap::new(Tempo::from_bpm(60).unwrap());
        assert_eq!(
            sixty.tick_to_time(Tick::new(3_456_000)),
            MusicTime::from_nanos(3_600_000_000_000)
        );
    }

    #[test]
    fn tempo_map_set_replaces_and_remove_keeps_tick_zero() {
        let mut map = TempoMap::default();
        let t = |bpm| Tempo::from_bpm(bpm).unwrap();
        map.set(Tick::new(1920), t(60));
        map.set(Tick::new(960), t(240));
        map.set(Tick::new(1920), t(100));
        map.set(Tick::ZERO, t(90));
        let changes: Vec<(u64, u32)> = map
            .changes()
            .map(|(k, v)| (k.get(), v.micros_per_quarter()))
            .collect();
        assert_eq!(changes, vec![(0, 666_667), (960, 250_000), (1920, 600_000)]);
        assert_eq!(map.tempo_at(Tick::new(959)), t(90));
        assert_eq!(map.tempo_at(Tick::new(960)), t(240));
        assert_eq!(map.tempo_at(Tick::new(u64::MAX)), t(100));

        assert_eq!(map.remove(Tick::ZERO), Err(ModelError::RemoveInitialChange));
        assert_eq!(map.remove(Tick::new(5)), Ok(false));
        assert_eq!(map.remove(Tick::new(960)), Ok(true));
        assert_eq!(map.changes().count(), 2);
        // 캐시가 다시 계산되었는지: 0..1920은 90 BPM.
        let at_1920 = 1920u128 * 666_667 * 1000;
        assert_eq!(map.tick_to_time(Tick::new(1920)).units(), at_1920);
    }

    #[test]
    fn tempo_map_two_segments_are_summed_exactly() {
        let mut map = TempoMap::default();
        map.set(Tick::new(960), Tempo::from_bpm(60).unwrap());
        // 960틱 @120 = 0.5 s, 다음 480틱 @60 = 0.5 s
        assert_eq!(map.tick_to_time(Tick::new(1440)).as_nanos(), 1_000_000_000);
        assert_eq!(
            map.time_to_tick(MusicTime::from_nanos(1_000_000_000)),
            Tick::new(1440)
        );
        assert_eq!(
            map.time_to_tick(MusicTime::from_nanos(500_000_000)),
            Tick::new(960)
        );
    }

    #[test]
    fn tempo_map_time_beyond_u64_ticks_saturates() {
        let map = TempoMap::new(Tempo::from_micros_per_quarter(1).unwrap());
        assert_eq!(
            map.time_to_tick(MusicTime::from_units(u128::MAX)),
            Tick::new(u64::MAX)
        );
    }
}
