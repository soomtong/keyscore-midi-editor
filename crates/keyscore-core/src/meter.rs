use crate::{ModelError, PPQ, Tick};

/// 박자표(REQ 2, SEQ-5). 박의 단위는 분모 음표다(6/8의 박은 8분음표).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeSignature {
    numerator: u8,
    denominator: u8,
}

impl TimeSignature {
    /// 4/4. 새 프로젝트 기본값(REQ 2).
    pub const COMMON: TimeSignature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };

    /// 분모는 1·2·4·8·16·32·64만 받는다. 그보다 짧은 박은 온음표(3840틱)를 나누어떨어지게
    /// 하지 못하거나(512 이상) 실용 범위를 넘는다.
    ///
    /// # Errors
    ///
    /// 분자가 0이거나 분모가 위 목록 밖이면 [`ModelError::TimeSignatureInvalid`].
    pub fn new(numerator: u8, denominator: u8) -> Result<Self, ModelError> {
        todo!("{numerator} {denominator}")
    }

    #[must_use]
    pub const fn numerator(self) -> u8 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> u8 {
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
        todo!("{at:?} {sig:?}")
    }

    /// `at`의 변화를 지운다.
    ///
    /// # Errors
    ///
    /// 틱 0은 지울 수 없다([`ModelError::RemoveInitialChange`]).
    pub fn remove(&mut self, at: Tick) -> Result<bool, ModelError> {
        todo!("{at:?}")
    }

    pub fn changes(&self) -> impl Iterator<Item = (Tick, TimeSignature)> + '_ {
        self.changes.iter().map(|&(tick, sig, _)| (tick, sig))
    }

    #[must_use]
    pub fn signature_at(&self, at: Tick) -> TimeSignature {
        todo!("{at:?}")
    }

    #[must_use]
    pub fn position(&self, at: Tick) -> BarPosition {
        todo!("{at:?}")
    }

    /// 마디 `bar`(0부터)가 시작하는 틱. u64를 넘으면 `u64::MAX`로 자른다.
    #[must_use]
    pub fn bar_start(&self, bar: u64) -> Tick {
        todo!("{bar}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(n: u8, d: u8) -> TimeSignature {
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
        for d in [0, 3, 5, 12, 128] {
            assert_eq!(
                TimeSignature::new(4, d),
                Err(ModelError::TimeSignatureInvalid(4, d))
            );
        }
        for d in [1, 2, 4, 8, 16, 32, 64] {
            assert!(TimeSignature::new(4, d).is_ok(), "{d}");
        }
        assert_eq!(sig(4, 4).bar_ticks(), 3840);
        assert_eq!(sig(6, 8).bar_ticks(), 2880);
        assert_eq!(sig(6, 8).beat_ticks(), 480);
        assert_eq!(sig(255, 64).bar_ticks(), 255 * 60);
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
        assert_eq!(map.bar_start(5), Tick::new(seven_eight));
        assert_eq!(map.bar_start(7), Tick::new(seven_eight + 2 * 3360));
        assert_eq!(map.signature_at(Tick::new(seven_eight - 1)), sig(6, 8));
    }

    #[test]
    fn meter_map_mid_bar_change_starts_new_bar() {
        let mut map = MeterMap::default();
        map.set(Tick::new(3840 + 1920), sig(3, 4)); // 마디 1의 절반에서 변화
        assert_eq!(pos(&map, 3840 + 1919), (1, 1, 959));
        assert_eq!(pos(&map, 3840 + 1920), (2, 0, 0));
        assert_eq!(map.bar_start(2), Tick::new(5760));
        assert_eq!(map.bar_start(3), Tick::new(5760 + 2880));
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
    fn meter_map_bar_start_saturates() {
        let map = MeterMap::default();
        assert_eq!(map.bar_start(u64::MAX), Tick::new(u64::MAX));
        let last = map.position(Tick::new(u64::MAX));
        assert_eq!(last.bar(), u64::MAX / 3840);
    }
}
