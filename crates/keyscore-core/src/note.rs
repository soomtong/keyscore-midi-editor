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
/// MIDI 2.0 노트별 피치(반음 7.25 고정소수)와 같은 해상도인 반음의 1/2^25 단위 정수로 담는다.
/// MIDI 2.0을 붙일 때 모델·파일 포맷을 바꾸지 않기 위해서다
/// (REQ M2-1, `docs/decisions/2026-10-12-clip-split-and-velocity.md` Q3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PitchOffset(i32);

impl PitchOffset {
    pub const ZERO: PitchOffset = PitchOffset(0);
    /// 반음 하나의 단위 수(2^25).
    pub const SEMITONE: i32 = 1 << 25;
    /// 1옥타브 = 12반음.
    pub const MAX: i32 = 12 * Self::SEMITONE;

    /// # Errors
    ///
    /// ±1옥타브를 넘으면 [`ModelError::PitchOffsetOutOfRange`].
    pub fn new(units: i32) -> Result<Self, ModelError> {
        if (-Self::MAX..=Self::MAX).contains(&units) {
            Ok(PitchOffset(units))
        } else {
            Err(ModelError::PitchOffsetOutOfRange(units))
        }
    }

    /// 반음의 1/2^25 단위 값.
    #[must_use]
    pub const fn units(self) -> i32 {
        self.0
    }
}

/// 노트 벨로시티. MIDI 2.0 해상도(16비트)로 담는다(REQ SEQ-3, M2-1).
///
/// 계이름 텍스트의 `v<0–127>`, UI, MIDI 1.0 송신은 [`Velocity::from_midi1`]·[`Velocity::to_midi1`]로
/// 변환한다. 변환은 MIDI 2.0 명세의 규칙(7→16 min-center-max 확장, 16→7 오른쪽 시프트)을 따르며
/// 7→16→7 왕복은 무손실이다(`docs/decisions/2026-10-12-clip-split-and-velocity.md` Q2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Velocity(u16);

impl Velocity {
    /// 새 트랙의 기본 벨로시티. MIDI 1.0 값 100을 16비트로 올린 것이다.
    /// 숫자 100은 REQ에 없고 흔한 시퀀서 기본값을 따른다.
    pub(crate) const TRACK_DEFAULT: Velocity = Velocity(0xC924);

    #[must_use]
    pub const fn new(value: u16) -> Self {
        Velocity(value)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// MIDI 1.0 7비트 값을 16비트로 올린다.
    ///
    /// # Errors
    ///
    /// 127을 넘으면 [`ModelError::Midi1VelocityOutOfRange`].
    pub fn from_midi1(value: u8) -> Result<Self, ModelError> {
        if value > 127 {
            return Err(ModelError::Midi1VelocityOutOfRange(value));
        }
        // MIDI 2.0 명세의 min-center-max 확장. 중앙(64) 이하는 시프트만 하고, 그 위는
        // 중앙 아래 6비트를 빈 하위 비트에 반복해 채워 127이 0xFFFF가 되게 한다.
        let src = u16::from(value);
        let mut out = src << 9;
        if src > 64 {
            let mut repeat = (src & 0x3F) << 3;
            while repeat != 0 {
                out |= repeat;
                repeat >>= 6;
            }
        }
        Ok(Velocity(out))
    }

    /// 16비트 값을 MIDI 1.0 7비트로 내린다(상위 7비트, MIDI 2.0 명세의 축소 규칙).
    ///
    /// 1–0x1FF는 0이 된다. MIDI 1.0 Note On의 벨로시티 0은 Note Off로 읽히므로,
    /// 송신 쪽(S1)이 노트를 보낼 때 이 경우를 따로 처리해야 한다(결정 Q2).
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "u16 >> 9는 0–127이라 u8에 들어간다"
    )]
    pub const fn to_midi1(self) -> u8 {
        (self.0 >> 9) as u8
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
    /// `.ksq` 노트 선택 속성 객체의 모르는 키(정규화한 JSON 객체 문자열). 다시 쓸 때 보존한다
    /// (REQ IO-1 「모르는 키는 보존」). 편집 API로는 바꾸지 않는다.
    extra: Option<Box<str>>,
}

/// 노트의 전체 순서: 시작, 음높이, 그다음 나머지 필드. 클립 안 노트는 이 순서로 정렬된다.
///
/// (시작, 음높이)만으로 정렬하면 같은 키의 노트를 지웠다 되살릴 때 자리가 바뀐다(S0-T4 리뷰 1 지적 4).
/// 전체 순서라서 어떤 삽입 순서로도 같은 클립이 되고, 같은 값의 노트끼리만 순서가 의미 없다.
impl Ord for Note {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            self.start,
            self.pitch,
            self.length,
            self.offset,
            self.velocity,
            &self.lyric,
            &self.extra,
        )
            .cmp(&(
                other.start,
                other.pitch,
                other.length,
                other.offset,
                other.velocity,
                &other.lyric,
                &other.extra,
            ))
    }
}

impl PartialOrd for Note {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
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
            extra: None,
        })
    }

    /// 피치 오프셋을 붙인다.
    ///
    /// # Errors
    ///
    /// 음높이 + 오프셋이 0 이상 128반음 미만이 아니면 [`ModelError::AbsolutePitchOutOfRange`].
    /// MIDI 2.0 노트별 피치(7.25 고정소수)가 담을 수 있는 값만 받는다
    /// (`docs/decisions/2026-10-12-absolute-pitch-range.md` B).
    pub fn with_offset(self, offset: PitchOffset) -> Result<Self, ModelError> {
        todo!("{offset:?}")
    }

    pub(crate) fn extra(&self) -> Option<&str> {
        self.extra.as_deref()
    }

    pub(crate) fn with_extra(self, extra: Option<Box<str>>) -> Self {
        Note { extra, ..self }
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

    fn v(x: u8) -> Velocity {
        Velocity::from_midi1(x).unwrap()
    }

    #[test]
    fn pitch_new_above_127_is_rejected() {
        assert_eq!(Pitch::new(128), Err(ModelError::PitchOutOfRange(128)));
        assert_eq!(Pitch::new(127).map(Pitch::get), Ok(127));
        assert_eq!(Pitch::new(0).map(Pitch::get), Ok(0));
    }

    #[test]
    fn pitch_offset_beyond_one_octave_is_rejected() {
        let max = 12 << 25;
        assert_eq!(PitchOffset::MAX, max);
        assert_eq!(
            PitchOffset::new(max + 1),
            Err(ModelError::PitchOffsetOutOfRange(max + 1))
        );
        assert_eq!(
            PitchOffset::new(-max - 1),
            Err(ModelError::PitchOffsetOutOfRange(-max - 1))
        );
        assert_eq!(
            PitchOffset::new(i32::MIN),
            Err(ModelError::PitchOffsetOutOfRange(i32::MIN))
        );
        assert_eq!(PitchOffset::new(max).map(PitchOffset::units), Ok(max));
        assert_eq!(PitchOffset::new(-max).map(PitchOffset::units), Ok(-max));
    }

    #[test]
    fn velocity_from_midi1_above_127_is_rejected() {
        assert_eq!(
            Velocity::from_midi1(128),
            Err(ModelError::Midi1VelocityOutOfRange(128))
        );
    }

    #[test]
    fn velocity_from_midi1_follows_midi2_min_center_max_scaling() {
        // MIDI 2.0 명세의 7→16 변환: 0과 중앙 64는 시프트만, 그 위는 하위 비트를 반복해 127 → 0xFFFF.
        assert_eq!(v(0).get(), 0);
        assert_eq!(v(1).get(), 1 << 9);
        assert_eq!(v(64).get(), 0x8000);
        assert_eq!(v(65).get(), 0x8208);
        assert_eq!(v(100).get(), 0xC924);
        assert_eq!(v(127).get(), 0xFFFF);
    }

    #[test]
    fn velocity_midi1_round_trip_is_lossless() {
        for x in 0..=127u8 {
            assert_eq!(v(x).to_midi1(), x, "{x}");
        }
        // 16→7은 상위 7비트(오른쪽 시프트)이며 하위 비트를 버린다. 상위 비트 절삭이 아니다(M2-1).
        assert_eq!(Velocity::new(0xFFFF).to_midi1(), 127);
        assert_eq!(Velocity::new(0x01FF).to_midi1(), 0);
        assert_eq!(Velocity::new(0x0200).to_midi1(), 1);
    }

    #[test]
    fn note_offset_outside_midi2_range_is_rejected() {
        let semis = |n: i32| PitchOffset::new(n * PitchOffset::SEMITONE).unwrap();
        let at =
            |p: u8| Note::new(Tick::ZERO, 1, Pitch::new(p).unwrap(), Velocity::new(1)).unwrap();
        // 127 + 1반음 = 128반음: 범위 밖. 127 + (1반음 − 1단위)는 범위 안.
        assert_eq!(
            at(127).with_offset(semis(1)).unwrap_err(),
            ModelError::AbsolutePitchOutOfRange
        );
        let just_below = PitchOffset::new(PitchOffset::SEMITONE - 1).unwrap();
        assert!(at(127).with_offset(just_below).is_ok());
        // 0 − 1단위: 음수라서 범위 밖. 0 + 0은 범위 안.
        assert_eq!(
            at(0)
                .with_offset(PitchOffset::new(-1).unwrap())
                .unwrap_err(),
            ModelError::AbsolutePitchOutOfRange
        );
        assert!(at(0).with_offset(PitchOffset::ZERO).is_ok());
        assert_eq!(at(60).with_offset(semis(12)).unwrap().offset(), semis(12));
        assert!(at(60).with_offset(semis(-12)).is_ok());
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
