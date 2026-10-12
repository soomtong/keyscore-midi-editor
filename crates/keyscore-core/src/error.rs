use thiserror::Error;

/// 모델 불변식을 깨뜨리는 입력을 거부할 때의 오류.
///
/// 생성자와 편집 메서드는 불변식을 깨뜨릴 값을 받으면 상태를 바꾸지 않고 이 오류를 돌려준다.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModelError {
    #[error("채널은 1–16이어야 한다: {0}")]
    ChannelOutOfRange(u8),
    #[error("음높이는 0–127이어야 한다: {0}")]
    PitchOutOfRange(u8),
    #[error("피치 오프셋은 ±1옥타브(반음 1/2^25 단위 ±402653184)여야 한다: {0}")]
    PitchOffsetOutOfRange(i32),
    #[error("MIDI 1.0 벨로시티는 0–127이어야 한다: {0}")]
    Midi1VelocityOutOfRange(u8),
    #[error("프로그램은 0–127이어야 한다: {0}")]
    ProgramOutOfRange(u8),
    #[error("뱅크는 0–16383이어야 한다: {0}")]
    BankOutOfRange(u16),
    #[error("트랙 색은 팔레트 0–11이어야 한다: {0}")]
    ColorOutOfRange(u8),
    #[error("음이름(피치 클래스)은 0–11이어야 한다: {0}")]
    PitchClassOutOfRange(u8),
    #[error("노트 길이는 1틱 이상이어야 한다")]
    ZeroNoteLength,
    #[error("클립 길이는 1틱 이상이어야 한다")]
    ZeroClipLength,
    #[error("틱 계산이 u64 범위를 넘는다")]
    TickOverflow,
    #[error("노트(시작 {start}, 끝 {end})가 클립 길이 {clip_length} 밖이다")]
    NoteOutsideClip {
        start: u64,
        end: u64,
        clip_length: u64,
    },
    #[error("클립 구간 [{start}, {end})이 같은 트랙의 다른 클립과 겹친다")]
    ClipOverlap { start: u64, end: u64 },
    #[error("트랙에 없는 클립이다")]
    UnknownClip,
    #[error("템포는 4분음표당 1–16777215 µs여야 한다: {0}")]
    TempoOutOfRange(u32),
    #[error("박자표 {0}/{1}: 분자는 1 이상, 분모는 1·2·4·8·16·32·64여야 한다")]
    TimeSignatureInvalid(u8, u8),
    #[error("틱 0의 템포·박자표는 지울 수 없다")]
    RemoveInitialChange,
    #[error("분할 지점 {at}은 클립 [{start}, {end}) 안쪽이어야 한다")]
    SplitOutsideClip { at: u64, start: u64, end: u64 },
}
