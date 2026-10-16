//! SMF 청크·이벤트 계층.

use thiserror::Error;

use crate::Midi1Message;

/// SMF 형식. Type 2(독립 시퀀스)는 읽지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmfFormat {
    /// Type 0: 트랙 하나에 모든 채널.
    Single,
    /// Type 1: 동시에 울리는 여러 트랙.
    Parallel,
}

/// 읽기 상한. 악의적이거나 망가진 파일이 메모리를 과하게 잡지 못하게 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmfLimits {
    max_tracks: usize,
    max_events: usize,
}

impl SmfLimits {
    /// 트랙 1024개, 이벤트 200만 개. 실용 상한 64트랙(SEQ-1)과 10만 노트 곡보다 넉넉하다.
    pub const DEFAULT: SmfLimits = SmfLimits {
        max_tracks: 1024,
        max_events: 2_000_000,
    };

    #[must_use]
    pub const fn new(max_tracks: usize, max_events: usize) -> Self {
        SmfLimits {
            max_tracks,
            max_events,
        }
    }

    #[must_use]
    pub const fn max_tracks(&self) -> usize {
        self.max_tracks
    }

    #[must_use]
    pub const fn max_events(&self) -> usize {
        self.max_events
    }
}

impl Default for SmfLimits {
    fn default() -> Self {
        SmfLimits::DEFAULT
    }
}

/// SMF 읽기 오류.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SmfError {
    /// `MThd` 헤더로 시작하지 않는다.
    #[error("SMF가 아니다(MThd 없음)")]
    NotSmf,
    /// 헤더 길이가 6보다 작다.
    #[error("SMF 헤더 길이가 잘못됐다: {0}")]
    BadHeader(u32),
    /// 파일이 `offset`에서 끝났다(청크·이벤트 길이가 남은 바이트보다 길다).
    #[error("SMF가 {offset}바이트에서 잘렸다")]
    Truncated { offset: usize },
    /// Type 0·1이 아니다.
    #[error("지원하지 않는 SMF 형식: {0}")]
    UnsupportedFormat(u16),
    /// Type 0인데 트랙이 하나가 아니다.
    #[error("Type 0 SMF의 트랙 수가 1이 아니다: {0}")]
    SingleTrackCount(usize),
    /// 시간 단위가 SMPTE 프레임이다(4분음표 단위만 읽는다).
    #[error("SMPTE 시간 단위는 지원하지 않는다")]
    SmpteDivision,
    /// 4분음표당 틱이 0이다.
    #[error("4분음표당 틱이 0이다")]
    ZeroDivision,
    /// 트랙이 상한보다 많다.
    #[error("트랙이 {max}개를 넘는다")]
    TooManyTracks { max: usize },
    /// 이벤트가 상한보다 많다.
    #[error("이벤트가 {max}개를 넘는다")]
    TooManyEvents { max: usize },
    /// 가변 길이 수가 4바이트를 넘는다.
    #[error("{offset}바이트의 가변 길이 수가 4바이트를 넘는다")]
    BadVarLen { offset: usize },
    /// running status가 없는 자리에 데이터 바이트가 왔다.
    #[error("{offset}바이트에 상태 바이트가 없다")]
    MissingStatus { offset: usize },
    /// 이벤트에 쓸 수 없는 상태 바이트(0xF1–0xF6, 0xF8–0xFE)가 왔다.
    #[error("{offset}바이트에 SMF에 올 수 없는 상태 바이트 {status:#04x}")]
    BadStatus { offset: usize, status: u8 },
    /// 채널 메시지의 데이터 바이트가 7비트가 아니다.
    #[error("{offset}바이트의 데이터 바이트가 7비트가 아니다")]
    BadData { offset: usize },
}

/// 트랙 하나. 이벤트는 원본 PPQ의 절대 틱 순이다. End of Track은 따로 든다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SmfTrack {
    events: Vec<SmfEvent>,
    end: u64,
}

impl SmfTrack {
    #[must_use]
    pub fn events(&self) -> &[SmfEvent] {
        &self.events
    }

    /// End of Track 틱(없으면 마지막 이벤트 틱).
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.end
    }
}

/// 절대 틱의 이벤트 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmfEvent {
    tick: u64,
    kind: SmfEventKind,
}

impl SmfEvent {
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub const fn kind(&self) -> &SmfEventKind {
        &self.kind
    }
}

/// 이벤트 내용.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmfEventKind {
    /// 채널 메시지(running status를 풀어 상태 바이트를 채운 것).
    Channel(Midi1Message),
    /// 메타 이벤트(End of Track 0x2F 제외).
    Meta { kind: u8, data: Vec<u8> },
    /// SysEx(0xF0 또는 0xF7 이스케이프). 앞 상태 바이트를 뺀 본문.
    SysEx { escape: bool, data: Vec<u8> },
}

/// SMF 파일 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmfFile {
    format: SmfFormat,
    ppq: u16,
    tracks: Vec<SmfTrack>,
}

impl SmfFile {
    /// 기본 상한으로 읽는다.
    ///
    /// # Errors
    ///
    /// [`SmfError`]. 어떤 입력에도 패닉하지 않는다.
    pub fn read(bytes: &[u8]) -> Result<SmfFile, SmfError> {
        SmfFile::read_with(bytes, SmfLimits::DEFAULT)
    }

    /// # Errors
    ///
    /// [`SmfError`].
    pub fn read_with(bytes: &[u8], limits: SmfLimits) -> Result<SmfFile, SmfError> {
        let _ = (bytes, limits);
        todo!()
    }

    /// 바이트로 쓴다. 결정적이다(같은 내용이면 같은 바이트). running status는 쓰지 않는다.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        todo!()
    }

    #[must_use]
    pub const fn format(&self) -> SmfFormat {
        self.format
    }

    /// 4분음표당 틱.
    #[must_use]
    pub const fn ppq(&self) -> u16 {
        self.ppq
    }

    #[must_use]
    pub fn tracks(&self) -> &[SmfTrack] {
        &self.tracks
    }
}
