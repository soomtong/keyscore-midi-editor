//! KeyScore 모델 계층: 프로젝트·트랙·클립·노트, 템포 맵, undo, 퀀타이즈, `.ksq` 직렬화.
//!
//! UI·MIDI·네트워크 없이 CLI와 속성 기반 테스트로 돌 수 있어야 하므로
//! GPUI·CoreMIDI·네트워크 crate에 의존하지 않는다(REQ 9.6).
//!
//! 모델 불변식(같은 트랙의 클립은 겹치지 않음, 노트는 클립 안, 값의 범위)은 이 crate의
//! 타입이 지킨다. 공개 필드는 없고, 값을 바꾸는 메서드는 불변식을 깨뜨릴 입력에 `Err`를
//! 돌려주며 상태를 바꾸지 않는다.
#![deny(unsafe_code)]

mod clip;
mod edit;
mod error;
mod history;
mod key;
mod meter;
mod note;
mod project;
mod tempo;
mod tick;
mod track;

pub use clip::{Clip, ClipId};
pub use edit::{Edit, TrackProp};
pub use error::ModelError;
pub use history::{CoalesceKey, History};
pub use key::{Key, PitchClass};
pub use meter::{BarPosition, MeterMap, TimeSignature};
pub use note::{CcPoint, Note, Pitch, PitchOffset, Velocity};
pub use project::Project;
pub use tempo::{MusicTime, Tempo, TempoMap};
pub use tick::{PPQ, Tick};
pub use track::{Bank, Channel, OutputPort, Program, Track, TrackColor};
