//! Standard MIDI File 읽기·쓰기(REQ IO-2, SEQ-6, SEQ-9, CTL-10, LYR-6).
//!
//! 세 계층으로 나눈다.
//! - [`SmfFile`]: 청크와 이벤트. 원본 PPQ의 절대 틱. 모르는 메타·SysEx도 그대로 담는다.
//! - [`Song`]: PPQ 960으로 재배율한 중간 표현. 노트, 프로그램, CC·벤드·프레셔 포인트 열(32비트), 가사.
//! - 모델 변환(`Song::to_project`, `Song::from_project`): 마디 단위 클립 묶기와 펼치기.
//!
//! 모델에는 아직 컨트롤러 레인 저장소가 없어(S6, CTL-7) CC 보존은 [`Song`]까지다.

mod raw;
mod report;
mod song;

pub use raw::{SmfError, SmfEvent, SmfEventKind, SmfFile, SmfFormat, SmfLimits, SmfTrack};
pub use report::{ExportReport, ImportError, ImportReport, smf_export, smf_import};
pub use song::{MAX_SONG_TICKS, Song, SongNote, SongTrack};
