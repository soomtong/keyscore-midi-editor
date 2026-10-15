//! OS 출력 포트(CoreMIDI 목적지)의 열거와 송신(OUT-3).

use crate::{HostTime, MidiClient, MidiError, MidiPort, PortInfo};

/// 시스템의 MIDI 목적지(OS 출력 포트)를 열거 순서대로 돌려준다.
#[must_use]
pub fn destinations() -> Vec<PortInfo> {
    todo!()
}

/// OS 출력 포트로 보내는 연결.
pub struct OsOutput {}

impl OsOutput {
    /// `port`를 연다.
    ///
    /// # Errors
    ///
    /// 그 고유 ID의 목적지가 없으면 `PortGone`, CoreMIDI 호출이 실패하면 `Os`.
    pub fn open(client: &MidiClient, port: &PortInfo) -> Result<Self, MidiError> {
        let _ = (client, port);
        todo!()
    }
}

impl MidiPort for OsOutput {
    fn name(&self) -> &str {
        todo!()
    }

    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        let _ = (at, ump);
        todo!()
    }
}
