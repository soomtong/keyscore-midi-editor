//! CoreMIDI 가상 포트·포트 열거, 스케줄러 스레드, MIDI 1.0 인코딩, SMF 입출력.
//!
//! CoreMIDI FFI 때문에 unsafe가 필요하다. crate 전체가 아니라 함수 단위로
//! `#[allow(unsafe_code)]`와 이유 주석을 단다(plan/00-conventions.md).
