use keyscore_core::{Bank, Channel, Clip, Key, Program, Tempo, TimeSignature};

use crate::Diagnostic;

/// `@tempo`·`@time`·`@key` 헤더 값. 텍스트에 없던 헤더는 `None`이다.
///
/// `@key none`은 `Some(Key::None)`이라 「적지 않음」과 구분된다. 헤더는 가져올 때만 쓰고
/// 클립 단위 텍스트에서는 호출자가 무시한다(REQ 5.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Header {
    tempo: Option<Tempo>,
    time: Option<TimeSignature>,
    key: Option<Key>,
}

impl Header {
    #[must_use]
    pub const fn new(tempo: Option<Tempo>, time: Option<TimeSignature>, key: Option<Key>) -> Self {
        Header { tempo, time, key }
    }

    #[must_use]
    pub const fn tempo(&self) -> Option<Tempo> {
        self.tempo
    }

    #[must_use]
    pub const fn time(&self) -> Option<TimeSignature> {
        self.time
    }

    #[must_use]
    pub const fn key(&self) -> Option<Key> {
        self.key
    }

    pub(crate) fn set_tempo(&mut self, tempo: Tempo) {
        self.tempo = Some(tempo);
    }

    pub(crate) fn set_time(&mut self, time: TimeSignature) {
        self.time = Some(time);
    }

    pub(crate) fn set_key(&mut self, key: Key) {
        self.key = Some(key);
    }
}

/// `@track` 블록 하나(같은 이름의 성부 블록을 합친 것)의 결과.
///
/// 클립은 시작 0, 노트는 클립 기준이다. 트랙에 놓는 위치와 트랙 매칭은 호출자가 정한다(S3·S7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTrack {
    name: String,
    channel: Channel,
    program: Option<Program>,
    bank: Option<Bank>,
    clip: Clip,
}

impl ParsedTrack {
    pub(crate) fn new(
        name: String,
        channel: Channel,
        program: Option<Program>,
        bank: Option<Bank>,
        clip: Clip,
    ) -> Self {
        ParsedTrack {
            name,
            channel,
            program,
            bank,
            clip,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn channel(&self) -> Channel {
        self.channel
    }

    #[must_use]
    pub const fn program(&self) -> Option<Program> {
        self.program
    }

    #[must_use]
    pub const fn bank(&self) -> Option<Bank> {
        self.bank
    }

    #[must_use]
    pub fn clip(&self) -> &Clip {
        &self.clip
    }

    #[must_use]
    pub fn into_clip(self) -> Clip {
        self.clip
    }
}

/// [`crate::parse`]의 결과. 트랙은 텍스트에 처음 나온 순서다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    header: Header,
    tracks: Vec<ParsedTrack>,
    warnings: Vec<Diagnostic>,
}

impl Parsed {
    pub(crate) fn new(header: Header, tracks: Vec<ParsedTrack>, warnings: Vec<Diagnostic>) -> Self {
        Parsed {
            header,
            tracks,
            warnings,
        }
    }

    #[must_use]
    pub const fn header(&self) -> &Header {
        &self.header
    }

    #[must_use]
    pub fn tracks(&self) -> &[ParsedTrack] {
        &self.tracks
    }

    /// 느슨 모드에서 건너뛰거나 받아 준 것들. 엄격 모드에서는 이것들이 오류가 되므로 늘 비어 있다.
    #[must_use]
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }
}
