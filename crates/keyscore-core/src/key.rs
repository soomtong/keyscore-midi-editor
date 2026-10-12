use crate::ModelError;

/// 피치 클래스 0–11(C = 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PitchClass(u8);

impl PitchClass {
    /// # Errors
    ///
    /// 11을 넘으면 [`ModelError::PitchClassOutOfRange`].
    pub fn new(value: u8) -> Result<Self, ModelError> {
        if value < 12 {
            Ok(PitchClass(value))
        } else {
            Err(ModelError::PitchClassOutOfRange(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// 프로젝트 전역 조성(REQ 2 「조성」: 장조·단조 12개씩 + 없음).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Key {
    #[default]
    None,
    Major(PitchClass),
    Minor(PitchClass),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_class_new_above_11_is_rejected() {
        assert_eq!(
            PitchClass::new(12),
            Err(ModelError::PitchClassOutOfRange(12))
        );
        assert_eq!(PitchClass::new(11).map(PitchClass::get), Ok(11));
    }

    #[test]
    fn key_default_is_none() {
        assert_eq!(Key::default(), Key::None);
    }
}
