/// CoreMIDI 타임스탬프 단위의 호스트 시각(`mach_absolute_time` 틱).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostTime(u64);

// libc 의존을 더하지 않으려고 이 함수 하나만 직접 선언한다. 시그니처는 <mach/mach_time.h>와 같다.
#[allow(unsafe_code)]
unsafe extern "C" {
    // 인자가 없고 전역 상태를 바꾸지 않으며 언제 불러도 정의된 값을 돌려준다.
    safe fn mach_absolute_time() -> u64;
}

impl HostTime {
    /// 지금의 호스트 시각.
    #[must_use]
    pub fn now() -> Self {
        HostTime(mach_absolute_time())
    }

    #[must_use]
    pub const fn from_ticks(ticks: u64) -> Self {
        HostTime(ticks)
    }

    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.0
    }

    /// `earlier`부터 이 시각까지의 기간. `earlier`가 더 늦으면 0이다. 호스트 틱 비율
    /// (`mach_timebase_info`)로 정확히 바꾼다.
    #[must_use]
    pub fn duration_since(self, earlier: HostTime) -> std::time::Duration {
        let tb = crate::rt::Timebase::get();
        let ticks = u128::from(self.0.saturating_sub(earlier.0));
        let nanos = ticks * u128::from(tb.numer) / u128::from(tb.denom);
        std::time::Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::HostTime;

    #[test]
    fn now_is_nonzero_and_monotonic() {
        let a = HostTime::now();
        let b = HostTime::now();
        assert!(a.ticks() > 0);
        assert!(b >= a);
    }
}
