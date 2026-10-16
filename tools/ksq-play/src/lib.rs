//! `.ksq`·`.ksqz`를 `CoreMIDI`로 재생하는 도구(S1-T7, 스케줄러의 첫 실사용).
//!
//! 종료 코드: 0 곡 끝까지 재생했거나 `q`로 멈춤, 1 실패(파일·MIDI 오류), 2 잘못된 사용법,
//! 130 Ctrl-C.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

/// Ctrl-C를 받았을 때 바이너리가 명령 채널로 보내는 값.
pub const INTERRUPT: &str = "\u{3}";

/// 사용법 문자열.
pub const USAGE: &str = "사용법: ksq-play <파일.ksq|.ksqz> [--port <가상 포트 이름>] [--from <마디>] \
[--metronome] [--metronome-port <OS 목적지 이름>] [--wait <초>]\n재생 중: p + Enter 패닉, q + Enter 정지";

/// 해석한 인자.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    path: PathBuf,
    port_name: String,
    from_bar: u64,
    metronome: bool,
    metronome_port: Option<String>,
    wait: Duration,
}

impl Options {
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// 가상 포트 이름(기본 「KeyScore Out」).
    #[must_use]
    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    /// 시작 마디(1부터).
    #[must_use]
    pub const fn from_bar(&self) -> u64 {
        self.from_bar
    }

    #[must_use]
    pub const fn metronome(&self) -> bool {
        self.metronome
    }

    #[must_use]
    pub fn metronome_port(&self) -> Option<&str> {
        self.metronome_port.as_deref()
    }

    #[must_use]
    pub const fn wait(&self) -> Duration {
        self.wait
    }
}

/// 잘못된 사용법.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct UsageError(pub String);

/// 인자를 해석한다.
///
/// # Errors
///
/// 파일이 없거나 둘 이상, 모르는 옵션, 값이 없거나 잘못된 옵션이면 [`UsageError`].
pub fn parse_args(args: &[OsString]) -> Result<Options, UsageError> {
    let _ = args;
    todo!()
}

/// 재생한다. `commands`의 `p`는 패닉, `q`는 정지 후 끝, [`INTERRUPT`]는 패닉·정지 후 130.
pub fn run(
    args: &[OsString],
    commands: &Receiver<String>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = (args, commands, out, err);
    todo!()
}
