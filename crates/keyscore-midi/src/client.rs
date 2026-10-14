use crate::{HostTime, MidiError, MidiPort};

/// 한 클라이언트가 만들 수 있는 가상 출력 포트 수(OUT-2).
pub const MAX_VIRTUAL_OUTPUTS: usize = 16;

/// CoreMIDI 클라이언트. 복제하면 같은 클라이언트를 공유한다.
#[derive(Clone)]
pub struct MidiClient {}

impl MidiClient {
    /// 클라이언트를 만든다.
    ///
    /// # Errors
    ///
    /// 이름이 비었으면 `InvalidName`, CoreMIDI 호출이 실패하면 `Os`.
    pub fn new(name: &str) -> Result<Self, MidiError> {
        let _ = name;
        todo!()
    }

    /// MIDI 1.0 프로토콜 가상 소스(앱의 출력 포트)를 만든다(OUT-1, OUT-2).
    ///
    /// # Errors
    ///
    /// 이름이 비었으면 `InvalidName`, 같은 클라이언트에 같은 이름이 있으면 `DuplicateName`,
    /// 이미 16개면 `TooManyPorts`, CoreMIDI 호출이 실패하면 `Os`.
    pub fn create_virtual_output(&self, name: &str) -> Result<VirtualOutput, MidiError> {
        let _ = name;
        todo!()
    }
}

/// 가상 출력 포트. drop되면 CoreMIDI 쪽에서도 해제된다.
pub struct VirtualOutput {}

impl VirtualOutput {
    /// 포트 이름을 바꾼다. 같은 클라이언트의 다른 포트 이름과 겹치면 안 된다.
    ///
    /// # Errors
    ///
    /// `InvalidName`, `DuplicateName`, `Os`.
    pub fn rename(&mut self, name: &str) -> Result<(), MidiError> {
        let _ = name;
        todo!()
    }
}

impl Drop for VirtualOutput {
    fn drop(&mut self) {}
}

impl MidiPort for VirtualOutput {
    fn name(&self) -> &str {
        todo!()
    }

    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        let _ = (at, ump);
        todo!()
    }
}

/// 시스템의 MIDI 소스 이름을 열거 순서대로 돌려준다.
#[must_use]
pub fn source_names() -> Vec<String> {
    todo!()
}
