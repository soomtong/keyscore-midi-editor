use crate::{HostTime, MidiError};

/// 가상 포트와 OS 포트에 공통인 출력 포트.
///
/// `send`는 스케줄러 스레드에서 불리므로 할당·락·로그를 하지 않는다(REQ 9.2).
pub trait MidiPort: Send + Sync {
    /// 사용자에게 보이는 포트 이름.
    fn name(&self) -> &str;

    /// UMP 메시지 하나(1–4 워드)를 `at`에 도착하도록 예약 송신한다.
    ///
    /// # Errors
    ///
    /// 타임스탬프가 0이면 `ZeroTimestamp`, 워드 수가 메시지 타입과 다르면 `InvalidMessage`,
    /// CoreMIDI 호출이 실패하면 `Os`.
    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError>;
}
