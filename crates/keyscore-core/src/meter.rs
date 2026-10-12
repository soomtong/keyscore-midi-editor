use crate::{ModelError, PPQ, Tick};

/// 박자표(REQ 2, SEQ-5). 박의 단위는 분모 음표다(6/8의 박은 8분음표).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeSignature {
    numerator: u8,
    denominator: u16,
}

impl TimeSignature {
    /// 4/4. 새 프로젝트 기본값(REQ 2).
    pub const COMMON: TimeSignature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };

    /// 분모는 1–256 사이의 2의 거듭제곱만 받는다. SMF는 x/128·x/256도 합법으로 담고(REQ 2
    /// 「SMF의 변화는 보존」), 박 길이 3840/분모가 정수인 것은 256(15틱)까지다.
    ///
    /// # Errors
    ///
    /// 분자가 0이거나 분모가 위 범위 밖이면 [`ModelError::TimeSignatureInvalid`].
    pub fn new(numerator: u8, denominator: u16) -> Result<Self, ModelError> {
        if numerator == 0 || !denominator.is_power_of_two() || denominator > 256 {
            return Err(ModelError::TimeSignatureInvalid(numerator, denominator));
        }
        Ok(TimeSignature {
            numerator,
            denominator,
        })
    }

    #[must_use]
    pub const fn numerator(self) -> u8 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> u16 {
        self.denominator
    }

    /// 박 하나의 틱 수.
    #[must_use]
    pub const fn beat_ticks(self) -> u64 {
        4 * PPQ as u64 / self.denominator as u64
    }

    /// 마디 하나의 틱 수.
    #[must_use]
    pub const fn bar_ticks(self) -> u64 {
        self.numerator as u64 * self.beat_ticks()
    }
}

/// 틱의 음악적 위치. 모두 0부터 센다. 사람에게 보일 때는 UI가 1을 더한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BarPosition {
    pub(crate) bar: u64,
    pub(crate) beat: u32,
    pub(crate) tick: u64,
}

impl BarPosition {
    #[must_use]
    pub const fn bar(self) -> u64 {
        self.bar
    }

    #[must_use]
    pub const fn beat(self) -> u32 {
        self.beat
    }

    /// 박 안의 틱.
    #[must_use]
    pub const fn tick(self) -> u64 {
        self.tick
    }
}

/// 박자표 변화의 열(SEQ-1, SEQ-5). 틱 0에 항상 항목이 있다.
///
/// 변화는 그 틱에서 새 마디를 시작한다. 변화가 마디 중간에 오면(SMF에서 가져온 경우) 직전 마디는
/// 짧은 마디로 센다. 각 항목은 그 변화가 시작하는 마디 번호를 캐시로 들고 있다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterMap {
    /// (변화 틱, 박자표, 시작 마디 번호). 틱 순, 첫 항목은 틱 0.
    changes: Vec<(Tick, TimeSignature, u64)>,
}

impl Default for MeterMap {
    fn default() -> Self {
        MeterMap::new(TimeSignature::COMMON)
    }
}

impl MeterMap {
    #[must_use]
    pub fn new(initial: TimeSignature) -> Self {
        MeterMap {
            changes: vec![(Tick::ZERO, initial, 0)],
        }
    }

    /// `at`에서 박자표를 바꾼다. 같은 틱에 변화가 있으면 교체한다.
    pub fn set(&mut self, at: Tick, sig: TimeSignature) {
        match self.changes.binary_search_by_key(&at, |c| c.0) {
            Ok(i) => self.changes[i].1 = sig,
            Err(i) => self.changes.insert(i, (at, sig, 0)),
        }
        self.rebuild();
    }

    /// `at`의 변화를 지운다.
    ///
    /// # Errors
    ///
    /// 틱 0은 지울 수 없다([`ModelError::RemoveInitialChange`]).
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

    /// 시작 마디 번호 캐시를 다시 계산한다. 마디 중간에서 끊긴 짧은 마디도 한 마디로 센다(올림).
    fn rebuild(&mut self) {
        for i in 1..self.changes.len() {
            let (prev_tick, prev_sig, prev_bar) = self.changes[i - 1];
            let span = self.changes[i].0.get() - prev_tick.get();
            self.changes[i].2 = prev_bar + span.div_ceil(prev_sig.bar_ticks());
        }
    }

    fn segment_of_tick(&self, at: Tick) -> usize {
        self.changes.partition_point(|c| c.0 <= at) - 1
    }

    pub fn changes(&self) -> impl Iterator<Item = (Tick, TimeSignature)> + '_ {
        self.changes.iter().map(|&(tick, sig, _)| (tick, sig))
    }

    #[must_use]
    pub fn signature_at(&self, at: Tick) -> TimeSignature {
        self.changes[self.segment_of_tick(at)].1
    }

    #[must_use]
    pub fn position(&self, at: Tick) -> BarPosition {
        let (start, sig, start_bar) = self.changes[self.segment_of_tick(at)];
        let offset = at.get() - start.get();
        let within = offset % sig.bar_ticks();
        BarPosition {
            bar: start_bar + offset / sig.bar_ticks(),
            // 분자 ≤ 255라 박 번호는 u32에 들어간다.
            beat: u32::try_from(within / sig.beat_ticks()).unwrap_or(u32::MAX),
            tick: within % sig.beat_ticks(),
        }
    }

    /// 마디 `bar`(0부터)가 시작하는 틱. 그 마디가 u64 틱 범위 안에서 시작하지 않으면 `None`.
    #[must_use]
    pub fn bar_start(&self, bar: u64) -> Option<Tick> {
        // 시작 마디 번호는 변화마다 1 이상 늘어나므로 엄격히 증가한다.
        let (start, sig, start_bar) =
            self.changes[self.changes.partition_point(|c| c.2 <= bar) - 1];
        (bar - start_bar)
            .checked_mul(sig.bar_ticks())
            .and_then(|span| start.get().checked_add(span))
            .map(Tick::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(n: u8, d: u16) -> TimeSignature {
        TimeSignature::new(n, d).unwrap()
    }

    fn pos(map: &MeterMap, t: u64) -> (u64, u32, u64) {
        let p = map.position(Tick::new(t));
        (p.bar(), p.beat(), p.tick())
    }

    #[test]
    fn time_signature_rejects_zero_and_non_power_of_two() {
        assert_eq!(
            TimeSignature::new(0, 4),
            Err(ModelError::TimeSignatureInvalid(0, 4))
        );
        for d in [0, 3, 5, 12, 255, 512, 1024] {
            assert_eq!(
                TimeSignature::new(4, d),
                Err(ModelError::TimeSignatureInvalid(4, d))
            );
        }
        for d in [1, 2, 4, 8, 16, 32, 64, 128, 256] {
            assert!(TimeSignature::new(4, d).is_ok(), "{d}");
        }
        assert_eq!(sig(4, 4).bar_ticks(), 3840);
        assert_eq!(sig(6, 8).bar_ticks(), 2880);
        assert_eq!(sig(6, 8).beat_ticks(), 480);
        assert_eq!(sig(255, 64).bar_ticks(), 255 * 60);
        // SMF는 x/128·x/256을 합법으로 담는다(리뷰 1 지적 2). 3840 / 256 = 15틱.
        assert_eq!(sig(3, 256).beat_ticks(), 15);
        assert_eq!(sig(7, 128).bar_ticks(), 7 * 30);
    }

    #[test]
    fn meter_map_position_crosses_changes() {
        let mut map = MeterMap::default();
        map.set(Tick::new(2 * 3840), sig(6, 8)); // 마디 2부터 6/8
        map.set(Tick::new(2 * 3840 + 3 * 2880), sig(7, 8)); // 마디 5부터 7/8
        assert_eq!(pos(&map, 0), (0, 0, 0));
        assert_eq!(pos(&map, 3840 + 960 + 5), (1, 1, 5));
        assert_eq!(pos(&map, 7680), (2, 0, 0));
        assert_eq!(pos(&map, 7680 + 2880 + 480 * 5 + 7), (3, 5, 7));
        let seven_eight = 7680 + 3 * 2880;
        assert_eq!(pos(&map, seven_eight), (5, 0, 0));
        assert_eq!(pos(&map, seven_eight + 3360 + 480 * 6), (6, 6, 0));
        assert_eq!(map.bar_start(5), Some(Tick::new(seven_eight)));
        assert_eq!(map.bar_start(7), Some(Tick::new(seven_eight + 2 * 3360)));
        assert_eq!(map.signature_at(Tick::new(seven_eight - 1)), sig(6, 8));
    }

    #[test]
    fn meter_map_mid_bar_change_starts_new_bar() {
        let mut map = MeterMap::default();
        map.set(Tick::new(3840 + 1920), sig(3, 4)); // 마디 1의 절반에서 변화
        assert_eq!(pos(&map, 3840 + 1919), (1, 1, 959));
        assert_eq!(pos(&map, 3840 + 1920), (2, 0, 0));
        assert_eq!(map.bar_start(2), Some(Tick::new(5760)));
        assert_eq!(map.bar_start(3), Some(Tick::new(5760 + 2880)));
    }

    #[test]
    fn meter_map_set_replaces_and_remove_keeps_tick_zero() {
        let mut map = MeterMap::default();
        map.set(Tick::new(3840), sig(3, 4));
        map.set(Tick::new(3840), sig(5, 4));
        map.set(Tick::ZERO, sig(2, 4));
        let changes: Vec<(u64, u8)> = map
            .changes()
            .map(|(t, s)| (t.get(), s.numerator()))
            .collect();
        assert_eq!(changes, vec![(0, 2), (3840, 5)]);
        // 2/4 마디 둘(1920 × 2) 뒤 5/4가 마디 2에서 시작
        assert_eq!(pos(&map, 3840), (2, 0, 0));
        assert_eq!(map.remove(Tick::ZERO), Err(ModelError::RemoveInitialChange));
        assert_eq!(map.remove(Tick::new(3840)), Ok(true));
        assert_eq!(pos(&map, 3840), (2, 0, 0));
        assert_eq!(map.remove(Tick::new(3840)), Ok(false));
    }

    #[test]
    fn meter_map_bar_start_beyond_u64_is_none() {
        let map = MeterMap::default();
        assert_eq!(map.bar_start(u64::MAX), None);
        assert_eq!(
            map.bar_start(u64::MAX / 3840),
            Some(Tick::new(u64::MAX / 3840 * 3840))
        );
        assert_eq!(map.bar_start(u64::MAX / 3840 + 1), None);
        let last = map.position(Tick::new(u64::MAX));
        assert_eq!(last.bar(), u64::MAX / 3840);
    }
}
