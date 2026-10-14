/// CoreMIDI 타임스탬프 단위의 호스트 시각(`mach_absolute_time` 틱).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostTime(u64);

impl HostTime {
    /// 지금의 호스트 시각.
    #[must_use]
    pub fn now() -> Self {
        todo!()
    }

    #[must_use]
    pub const fn from_ticks(ticks: u64) -> Self {
        HostTime(ticks)
    }

    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.0
    }
}
