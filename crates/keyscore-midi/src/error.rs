use thiserror::Error;

/// MIDI 계층의 오류.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MidiError {
    /// CoreMIDI 호출이 0이 아닌 `OSStatus`를 돌려줬다.
    #[error("CoreMIDI {call} 실패: OSStatus {status}")]
    Os { call: &'static str, status: i32 },
    /// 포트 이름에 보이는 글자가 없거나, 제어 문자·줄 구분자·표시되지 않는 문자가 있다.
    #[error("포트 이름이 비었거나 쓸 수 없는 문자가 있다")]
    InvalidName,
    /// 프로세스에 같은 이름(NFC, 앞뒤 공백 제외)의 가상 포트가 이미 있다.
    #[error("가상 포트 이름이 이미 쓰인다: {0}")]
    DuplicateName(String),
    /// 가상 포트를 더 만들 수 없다(OUT-2 최대 16).
    #[error("가상 포트는 최대 {max}개다", max = crate::MAX_VIRTUAL_OUTPUTS)]
    TooManyPorts,
    /// UMP 워드 수가 메시지 타입과 맞지 않는다.
    #[error("UMP 메시지 길이가 메시지 타입과 맞지 않는다")]
    InvalidMessage,
    /// 타임스탬프 0은 「지금」이 아니다(CoreMIDI 헤더). 항상 실제 호스트 시각을 채운다.
    #[error("타임스탬프 0은 쓸 수 없다")]
    ZeroTimestamp,
}
