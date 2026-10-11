//! KeyScore 모델 계층: 프로젝트·트랙·클립·노트, 템포 맵, undo, 퀀타이즈, `.ksq` 직렬화.
//!
//! UI·MIDI·네트워크 없이 CLI와 속성 기반 테스트로 돌 수 있어야 하므로
//! GPUI·CoreMIDI·네트워크 crate에 의존하지 않는다(REQ 9.6).
#![deny(unsafe_code)]
