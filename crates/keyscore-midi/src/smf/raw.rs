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
        if !bytes.starts_with(b"MThd") {
            return Err(SmfError::NotSmf);
        }
        let header_len = be_u32(bytes, 4)?;
        if header_len < 6 {
            return Err(SmfError::BadHeader(header_len));
        }
        let format = be_u16(bytes, 8)?;
        let declared = usize::from(be_u16(bytes, 10)?);
        let division = be_u16(bytes, 12)?;
        let format = match format {
            0 => SmfFormat::Single,
            1 => SmfFormat::Parallel,
            other => return Err(SmfError::UnsupportedFormat(other)),
        };
        if division & 0x8000 != 0 {
            return Err(SmfError::SmpteDivision);
        }
        if division == 0 {
            return Err(SmfError::ZeroDivision);
        }
        if declared > limits.max_tracks {
            return Err(SmfError::TooManyTracks {
                max: limits.max_tracks,
            });
        }
        if format == SmfFormat::Single && declared != 1 {
            return Err(SmfError::SingleTrackCount(declared));
        }
        // 헤더의 트랙 수만 믿고 미리 잡지 않는다. 실제로 읽은 청크만큼만 늘린다.
        let mut pos = 8usize.saturating_add(header_len as usize);
        let mut tracks = Vec::new();
        let mut events = 0usize;
        while tracks.len() < declared {
            let id = bytes
                .get(pos..pos.saturating_add(4))
                .ok_or(SmfError::Truncated { offset: pos })?;
            let len = be_u32(bytes, pos.saturating_add(4))? as usize;
            let start = pos.saturating_add(8);
            let end = start
                .checked_add(len)
                .filter(|&e| e <= bytes.len())
                .ok_or(SmfError::Truncated { offset: start })?;
            if id == b"MTrk" {
                tracks.push(read_track(&bytes[start..end], start, &mut events, limits)?);
            }
            // 모르는 청크는 명세대로 건너뛴다.
            pos = end;
        }
        Ok(SmfFile {
            format,
            ppq: division,
            tracks,
        })
    }

    /// 바이트로 쓴다. 결정적이다(같은 내용이면 같은 바이트). running status는 쓰지 않는다.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"MThd");
        out.extend_from_slice(&6u32.to_be_bytes());
        let format: u16 = match self.format {
            SmfFormat::Single => 0,
            SmfFormat::Parallel => 1,
        };
        out.extend_from_slice(&format.to_be_bytes());
        let count = u16::try_from(self.tracks.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&count.to_be_bytes());
        out.extend_from_slice(&self.ppq.to_be_bytes());
        for track in self.tracks.iter().take(usize::from(count)) {
            let body = write_track(track);
            out.extend_from_slice(b"MTrk");
            out.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_be_bytes());
            out.extend_from_slice(&body);
        }
        out
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

impl SmfTrack {
    pub(crate) fn from_parts(events: Vec<SmfEvent>, end: u64) -> SmfTrack {
        SmfTrack { events, end }
    }
}

impl SmfEvent {
    pub(crate) fn new(tick: u64, kind: SmfEventKind) -> SmfEvent {
        SmfEvent { tick, kind }
    }
}

impl SmfFile {
    pub(crate) fn from_parts(format: SmfFormat, ppq: u16, tracks: Vec<SmfTrack>) -> SmfFile {
        SmfFile {
            format,
            ppq,
            tracks,
        }
    }
}

/// 가변 길이 수의 최댓값(4바이트, 28비트).
const MAX_VAR_LEN: u32 = 0x0FFF_FFFF;

fn be_u16(bytes: &[u8], at: usize) -> Result<u16, SmfError> {
    match bytes.get(at..at.saturating_add(2)) {
        Some(&[a, b]) => Ok(u16::from_be_bytes([a, b])),
        _ => Err(SmfError::Truncated { offset: at }),
    }
}

fn be_u32(bytes: &[u8], at: usize) -> Result<u32, SmfError> {
    match bytes.get(at..at.saturating_add(4)) {
        Some(&[a, b, c, d]) => Ok(u32::from_be_bytes([a, b, c, d])),
        _ => Err(SmfError::Truncated { offset: at }),
    }
}

/// 트랙 본문을 읽는 커서. 오류의 위치는 파일 기준(`base` + 위치)이다.
struct Cursor<'a> {
    body: &'a [u8],
    pos: usize,
    base: usize,
}

impl Cursor<'_> {
    fn offset(&self) -> usize {
        self.base.saturating_add(self.pos)
    }

    fn byte(&mut self) -> Result<u8, SmfError> {
        let b = *self.body.get(self.pos).ok_or(SmfError::Truncated {
            offset: self.offset(),
        })?;
        self.pos += 1;
        Ok(b)
    }

    fn var_len(&mut self) -> Result<u32, SmfError> {
        let start = self.offset();
        let mut value = 0u32;
        for _ in 0..4 {
            let b = self.byte()?;
            value = (value << 7) | u32::from(b & 0x7F);
            if b & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(SmfError::BadVarLen { offset: start })
    }

    fn data(&mut self, len: u32) -> Result<&[u8], SmfError> {
        let start = self.pos;
        let end = start
            .checked_add(len as usize)
            .filter(|&e| e <= self.body.len())
            .ok_or(SmfError::Truncated {
                offset: self.offset(),
            })?;
        self.pos = end;
        Ok(&self.body[start..end])
    }
}

fn read_track(
    body: &[u8],
    base: usize,
    events: &mut usize,
    limits: SmfLimits,
) -> Result<SmfTrack, SmfError> {
    let mut c = Cursor { body, pos: 0, base };
    let mut tick = 0u64;
    let mut running: Option<u8> = None;
    let mut list = Vec::new();
    let mut end = None;
    while c.pos < body.len() {
        tick = tick.saturating_add(u64::from(c.var_len()?));
        let at = c.offset();
        let first = c.byte()?;
        let status = if first & 0x80 != 0 {
            first
        } else {
            // 데이터 바이트: 앞 채널 메시지의 상태를 잇는다(running status). 다시 읽게 한 칸 물린다.
            c.pos -= 1;
            running.ok_or(SmfError::MissingStatus { offset: at })?
        };
        let kind = match status {
            0x80..=0xEF => {
                // 메타·SysEx 뒤에도 running status를 잇는다. 명세는 끊는다고 하지만 그렇게 쓴 파일이
                // 흔하다(너그럽게 읽기).
                running = Some(status);
                let d1 = data_byte(&mut c)?;
                let d2 = if matches!(status & 0xF0, 0xC0 | 0xD0) {
                    0
                } else {
                    data_byte(&mut c)?
                };
                SmfEventKind::Channel(Midi1Message::from_status(status, d1, d2))
            }
            0xFF => {
                let kind = c.byte()?;
                let len = c.var_len()?;
                let data = c.data(len)?.to_vec();
                if kind == 0x2F {
                    end = Some(tick);
                    break;
                }
                SmfEventKind::Meta { kind, data }
            }
            0xF0 | 0xF7 => {
                let len = c.var_len()?;
                SmfEventKind::SysEx {
                    escape: status == 0xF7,
                    data: c.data(len)?.to_vec(),
                }
            }
            _ => return Err(SmfError::BadStatus { offset: at, status }),
        };
        *events += 1;
        if *events > limits.max_events {
            return Err(SmfError::TooManyEvents {
                max: limits.max_events,
            });
        }
        list.push(SmfEvent { tick, kind });
    }
    let end = end.unwrap_or(tick);
    Ok(SmfTrack { events: list, end })
}

fn data_byte(c: &mut Cursor<'_>) -> Result<u8, SmfError> {
    let at = c.offset();
    let b = c.byte()?;
    if b & 0x80 != 0 {
        return Err(SmfError::BadData { offset: at });
    }
    Ok(b)
}

fn put_var_len(out: &mut Vec<u8>, value: u32) {
    let value = value.min(MAX_VAR_LEN);
    let mut shift = 21;
    let mut started = false;
    while shift > 0 {
        let group = u8::try_from((value >> shift) & 0x7F).unwrap_or(0);
        if group != 0 || started {
            out.push(group | 0x80);
            started = true;
        }
        shift -= 7;
    }
    out.push(u8::try_from(value & 0x7F).unwrap_or(0));
}

/// 델타를 쓴다. 가변 길이 수(28비트)를 넘으면 빈 텍스트 메타로 시간을 채운다.
fn put_delta(out: &mut Vec<u8>, mut delta: u64) {
    while delta > u64::from(MAX_VAR_LEN) {
        put_var_len(out, MAX_VAR_LEN);
        out.extend_from_slice(&[0xFF, 0x01, 0x00]);
        delta -= u64::from(MAX_VAR_LEN);
    }
    put_var_len(out, u32::try_from(delta).unwrap_or(MAX_VAR_LEN));
}

fn put_block(out: &mut Vec<u8>, data: &[u8]) {
    // 가변 길이 수로 담을 수 없는 길이는 자른다(256 MB를 넘는 메타는 쓰지 않는다).
    let len = data.len().min(MAX_VAR_LEN as usize);
    put_var_len(out, u32::try_from(len).unwrap_or(MAX_VAR_LEN));
    out.extend_from_slice(&data[..len]);
}

fn write_track(track: &SmfTrack) -> Vec<u8> {
    let mut out = Vec::new();
    let mut last = 0u64;
    for e in &track.events {
        put_delta(&mut out, e.tick.saturating_sub(last));
        last = last.max(e.tick);
        match &e.kind {
            SmfEventKind::Channel(m) => out.extend_from_slice(m.bytes()),
            SmfEventKind::Meta { kind, data } => {
                out.push(0xFF);
                out.push(*kind);
                put_block(&mut out, data);
            }
            SmfEventKind::SysEx { escape, data } => {
                out.push(if *escape { 0xF7 } else { 0xF0 });
                put_block(&mut out, data);
            }
        }
    }
    put_delta(&mut out, track.end.saturating_sub(last));
    out.extend_from_slice(&[0xFF, 0x2F, 0x00]);
    out
}
