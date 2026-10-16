//! 메트로놈 설정과 박 보고(MET-1~3), 미리듣기 길이(PRE-1).

use std::time::Duration;

use keyscore_core::{Channel, Pitch, Velocity};

use crate::HostTime;

/// 미리듣기 노트 길이의 기본값(PRE-1).
pub const DEFAULT_PREVIEW_LENGTH: Duration = Duration::from_millis(200);

/// 미리듣기·클릭 노트 길이의 상한. 넘으면 줄인다. 끄기 대기 표의 칸이 오래 묶이지 않게 한다.
pub const MAX_NOTE_LENGTH: Duration = Duration::from_secs(10);

/// 클릭 길이의 기본값. REQ에 값이 없어 드럼 음원의 원샷 샘플을 끊지 않을 만큼 짧은 흔한 값을 쓴다.
const DEFAULT_CLICK_LENGTH: Duration = Duration::from_millis(50);

/// MIDI 메트로놈 설정(MET-1, MET-3). 포트는 스냅샷의 포트 번호다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetronomeConfig {
    port: u16,
    channel: Channel,
    accent: Pitch,
    beat: Pitch,
    accent_velocity: Velocity,
    beat_velocity: Velocity,
    length: Duration,
}

impl MetronomeConfig {
    /// 벨로시티와 길이는 기본값(강박 127, 약박 100, 50 ms)이다.
    #[must_use]
    pub fn new(port: u16, channel: Channel, accent: Pitch, beat: Pitch) -> Self {
        let _ = (port, channel, accent, beat, DEFAULT_CLICK_LENGTH);
        todo!()
    }

    #[must_use]
    pub fn with_velocities(self, accent: Velocity, beat: Velocity) -> Self {
        let _ = (accent, beat);
        todo!()
    }

    /// 클릭 길이. 0이면 1 ms, [`MAX_NOTE_LENGTH`]를 넘으면 상한으로 줄인다.
    #[must_use]
    pub fn with_length(self, length: Duration) -> Self {
        let _ = clamp_length(length);
        todo!()
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub const fn channel(&self) -> Channel {
        self.channel
    }

    /// 마디 첫 박의 노트.
    #[must_use]
    pub const fn accent(&self) -> Pitch {
        self.accent
    }

    /// 나머지 박의 노트.
    #[must_use]
    pub const fn beat(&self) -> Pitch {
        self.beat
    }

    #[must_use]
    pub const fn accent_velocity(&self) -> Velocity {
        self.accent_velocity
    }

    #[must_use]
    pub const fn beat_velocity(&self) -> Velocity {
        self.beat_velocity
    }

    #[must_use]
    pub const fn length(&self) -> Duration {
        self.length
    }
}

impl Default for MetronomeConfig {
    /// 포트 0, 채널 10, 강박 37, 약박 42(MET-1).
    fn default() -> Self {
        todo!()
    }
}

/// 재생 중 지난(또는 곧 올) 박 하나(MET-2 시각 메트로놈). 0부터 센다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beat {
    pub(crate) bar: u64,
    pub(crate) index: u32,
    pub(crate) at: HostTime,
}

impl Beat {
    #[must_use]
    pub const fn bar(&self) -> u64 {
        self.bar
    }

    #[must_use]
    pub const fn beat(&self) -> u32 {
        self.index
    }

    /// 박이 울리는 호스트 시각. 스케줄러는 lookahead만큼 미리 알리므로 지금보다 뒤일 수 있다.
    #[must_use]
    pub const fn at(&self) -> HostTime {
        self.at
    }
}

/// 길이를 [1 ms, `MAX_NOTE_LENGTH`]로 맞춘다.
pub(crate) fn clamp_length(length: Duration) -> Duration {
    length.clamp(Duration::from_millis(1), MAX_NOTE_LENGTH)
}
