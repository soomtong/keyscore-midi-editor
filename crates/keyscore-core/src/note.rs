use crate::{ModelError, Tick};

/// MIDI 음높이 0–127.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pitch(u8);

impl Pitch {
    /// # Errors
    ///
    /// 127을 넘으면 [`ModelError::PitchOutOfRange`].
    pub fn new(value: u8) -> Result<Self, ModelError> {
        if value <= 127 {
            Ok(Pitch(value))
        } else {
            Err(ModelError::PitchOutOfRange(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// 음높이에 더하는 미세 조정(REQ SEQ-3 「피치 오프셋 ±1옥타브 소수」).
///
/// 모델은 정수만 다룬다(REQ 9.5)는 원칙에 맞춰 센트(반음의 1/100) 단위 정수로 담는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PitchOffset(i16);

impl PitchOffset {
    pub const ZERO: PitchOffset = PitchOffset(0);
    /// 1옥타브 = 1200센트.
    pub const MAX_CENTS: i16 = 1200;

    /// # Errors
    ///
    /// ±1200센트를 넘으면 [`ModelError::PitchOffsetOutOfRange`].
    pub fn new(cents: i16) -> Result<Self, ModelError> {
        if (-Self::MAX_CENTS..=Self::MAX_CENTS).contains(&cents) {
            Ok(PitchOffset(cents))
        } else {
            Err(ModelError::PitchOffsetOutOfRange(cents))
        }
    }

    #[must_use]
    pub const fn cents(self) -> i16 {
        self.0
    }
}

/// 노트 벨로시티.
///
/// 타입은 MIDI 2.0 해상도(u16)를 담을 수 있지만, v1은 0–127 값만 쓴다
/// (`docs/decisions/2026-10-12-clip-split-and-velocity.md` Q2).
/// `.ksq`·계이름 텍스트의 숫자와 같은 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Velocity(u16);

impl Velocity {
    /// v1이 담는 최댓값.
    pub const V1_MAX: u16 = 127;

    /// 새 트랙의 기본 벨로시티. 숫자 자체는 REQ에 없고 흔한 시퀀서 기본값을 따른다.
    pub(crate) const TRACK_DEFAULT: Velocity = Velocity(100);

    /// # Errors
    ///
    /// 127을 넘으면 [`ModelError::VelocityOutOfRange`].
    pub fn new(value: u16) -> Result<Self, ModelError> {
        if value <= Self::V1_MAX {
            Ok(Velocity(value))
        } else {
            Err(ModelError::VelocityOutOfRange(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// 클립 안의 노트 이벤트(REQ 3, SEQ-3, LYR-1).
///
/// 시작 틱은 클립 시작 기준이다. 그래서 클립을 옮기면 노트가 함께 간다.
/// 채널은 노트가 아니라 트랙이 가진다.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Note {
    start: Tick,
    length: u32,
    pitch: Pitch,
    offset: PitchOffset,
    velocity: Velocity,
    lyric: Option<String>,
}

impl Note {
    /// # Errors
    ///
    /// 길이가 0이면 [`ModelError::ZeroNoteLength`], 끝 틱이 u64를 넘으면 [`ModelError::TickOverflow`].
    pub fn new(
        start: Tick,
        length: u32,
        pitch: Pitch,
        velocity: Velocity,
    ) -> Result<Self, ModelError> {
        if length == 0 {
            return Err(ModelError::ZeroNoteLength);
        }
        // end()가 덧셈을 검사 없이 하므로 여기서 오버플로를 막는다.
        start.checked_add(u64::from(length))?;
        Ok(Note {
            start,
            length,
            pitch,
            offset: PitchOffset::ZERO,
            velocity,
            lyric: None,
        })
    }

    #[must_use]
    pub fn with_offset(self, offset: PitchOffset) -> Self {
        Note { offset, ..self }
    }

    /// 가사 음절을 붙인다. 빈 문자열은 「가사 없음」이다(LYR-1).
    #[must_use]
    pub fn with_lyric(self, lyric: impl Into<String>) -> Self {
        let lyric = lyric.into();
        Note {
            lyric: if lyric.is_empty() { None } else { Some(lyric) },
            ..self
        }
    }

    #[must_use]
    pub const fn start(&self) -> Tick {
        self.start
    }

    #[must_use]
    pub const fn length(&self) -> u32 {
        self.length
    }

    /// 클립 기준 끝 틱(배타). 생성자가 오버플로를 막으므로 항상 계산된다.
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.start.get() + self.length as u64
    }

    #[must_use]
    pub const fn pitch(&self) -> Pitch {
        self.pitch
    }

    #[must_use]
    pub const fn offset(&self) -> PitchOffset {
        self.offset
    }

    #[must_use]
    pub const fn velocity(&self) -> Velocity {
        self.velocity
    }

    #[must_use]
    pub fn lyric(&self) -> Option<&str> {
        self.lyric.as_deref()
    }

    /// 같은 내용을 다른 시작 틱에 둔 노트. 클립 분할·이동에서 기준점을 바꿀 때 쓴다.
    pub(crate) fn with_start(&self, start: Tick) -> Result<Self, ModelError> {
        start.checked_add(u64::from(self.length))?;
        Ok(Note {
            start,
            ..self.clone()
        })
    }
}

/// 컨트롤러 레인의 점(REQ SEQ-4). 값은 32비트로 보관하고 송신 해상도는 출력 쪽이 정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CcPoint {
    tick: Tick,
    value: u32,
}

impl CcPoint {
    #[must_use]
    pub const fn new(tick: Tick, value: u32) -> Self {
        CcPoint { tick, value }
    }

    #[must_use]
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    #[must_use]
    pub const fn value(&self) -> u32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: u8) -> Pitch {
        Pitch::new(v).unwrap()
    }

    fn v(x: u16) -> Velocity {
        Velocity::new(x).unwrap()
    }

    #[test]
    fn pitch_new_above_127_is_rejected() {
        assert_eq!(Pitch::new(128), Err(ModelError::PitchOutOfRange(128)));
        assert_eq!(Pitch::new(127).map(Pitch::get), Ok(127));
        assert_eq!(Pitch::new(0).map(Pitch::get), Ok(0));
    }

    #[test]
    fn pitch_offset_beyond_one_octave_is_rejected() {
        assert_eq!(
            PitchOffset::new(1201),
            Err(ModelError::PitchOffsetOutOfRange(1201))
        );
        assert_eq!(
            PitchOffset::new(-1201),
            Err(ModelError::PitchOffsetOutOfRange(-1201))
        );
        assert_eq!(
            PitchOffset::new(i16::MIN),
            Err(ModelError::PitchOffsetOutOfRange(i16::MIN))
        );
        assert_eq!(PitchOffset::new(1200).map(PitchOffset::cents), Ok(1200));
        assert_eq!(PitchOffset::new(-1200).map(PitchOffset::cents), Ok(-1200));
    }

    #[test]
    fn velocity_new_above_127_is_rejected_in_v1() {
        assert_eq!(Velocity::new(128), Err(ModelError::VelocityOutOfRange(128)));
        assert_eq!(Velocity::new(0).map(Velocity::get), Ok(0));
        assert_eq!(Velocity::new(127).map(Velocity::get), Ok(127));
    }

    #[test]
    fn note_new_zero_length_is_rejected() {
        assert_eq!(
            Note::new(Tick::ZERO, 0, p(60), v(100)),
            Err(ModelError::ZeroNoteLength)
        );
        let note = Note::new(Tick::new(10), 5, p(60), v(100)).unwrap();
        assert_eq!(note.end(), 15);
    }

    #[test]
    fn note_new_end_overflowing_u64_is_rejected() {
        assert_eq!(
            Note::new(Tick::new(u64::MAX), 1, p(60), v(100)),
            Err(ModelError::TickOverflow)
        );
    }

    #[test]
    fn note_with_empty_lyric_has_no_lyric() {
        let note = Note::new(Tick::ZERO, 1, p(60), v(100)).unwrap();
        assert_eq!(note.clone().with_lyric("").lyric(), None);
        assert_eq!(note.with_lyric("사").lyric(), Some("사"));
    }
}
