//! CoreMIDI 가상 포트·포트 열거, 스케줄러 스레드, MIDI 1.0 인코딩, SMF 입출력.
//!
//! CoreMIDI FFI 때문에 unsafe가 필요하다. crate 전체가 아니라 함수 단위로
//! `#[allow(unsafe_code)]`와 이유 주석을 단다(plan/00-conventions.md).
#![deny(unsafe_code)]

mod client;
mod encode;
mod error;
mod metronome;
mod midi_thread;
mod os;
mod port;
mod ports;
mod render;
mod rt;
mod scheduler;
mod smf;
mod time;
mod ump;
mod watcher;

pub use client::{
    DEFAULT_PORT_NAME, MAX_VIRTUAL_OUTPUTS, MidiClient, VirtualOutput, is_rejected_name_char,
    normalize_port_name, same_port_name, source_names,
};
pub use encode::{
    Group, Midi1Message, bank_select, bend_to_14bit, cc_to_7bit, continue_, control_change,
    note_off, note_on, panic_messages, pitch_bend, program_change, song_position, start, stop,
    timing_clock, velocity_to_7bit,
};
pub use error::MidiError;
pub use metronome::{Beat, DEFAULT_PREVIEW_LENGTH, MAX_NOTE_LENGTH, MetronomeConfig};
pub use os::{OsOutput, destinations};
pub use port::MidiPort;
pub use ports::{
    Action, Link, PortEvent, PortInfo, Reconnector, Suggestion, diff_ports, suggest_mapping,
};
pub use render::{Rendered, render};
pub use scheduler::{MAX_SCHEDULER_PORTS, ScheduledEvent, Scheduler, SchedulerConfig, Snapshot};
pub use smf::{
    ExportReport, ImportError, ImportReport, MAX_SONG_TICKS, SmfError, SmfEvent, SmfEventKind,
    SmfFile, SmfFormat, SmfLimits, SmfTrack, Song, SongNote, SongTrack, smf_export, smf_import,
};
pub use time::HostTime;
pub use ump::ump_word_count;
pub use watcher::PortWatcher;
