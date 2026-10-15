//! 포트 목록 감시 스레드(OUT-3, REQ 9.2 백그라운드).

use std::sync::mpsc::Receiver;

use crate::{MidiError, PortEvent};

/// CoreMIDI 알림을 받아 목적지 목록의 변화를 이벤트로 보내는 스레드. drop하면 멈추고 끝난다.
pub struct PortWatcher {}

impl Drop for PortWatcher {
    fn drop(&mut self) {}
}

impl PortWatcher {
    /// 감시 스레드를 시작한다. 받는 쪽은 `PortEvent`를 순서대로 받는다.
    ///
    /// # Errors
    ///
    /// 알림 클라이언트를 만들지 못하면 `Os`.
    pub fn start() -> Result<(Self, Receiver<PortEvent>), MidiError> {
        todo!()
    }
}
