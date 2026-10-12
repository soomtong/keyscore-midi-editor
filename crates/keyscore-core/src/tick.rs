use crate::ModelError;

/// 4분음표 하나의 틱 수. REQ SEQ-6에 따라 고정이다.
pub const PPQ: u32 = 960;

/// 시퀀서 내부 시간 단위(REQ 3). 음수가 없어 「틱 ≥ 0」을 타입으로 보장한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tick(u64);

impl Tick {
    pub const ZERO: Tick = Tick(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Tick(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// # Errors
    ///
    /// 합이 u64를 넘으면 [`ModelError::TickOverflow`].
    pub fn checked_add(self, delta: u64) -> Result<Tick, ModelError> {
        todo!("{delta}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_checked_add_overflow_is_rejected() {
        assert_eq!(Tick::new(u64::MAX).checked_add(1), Err(ModelError::TickOverflow));
        assert_eq!(Tick::new(1).checked_add(2), Ok(Tick::new(3)));
    }
}
