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
        MetronomeConfig {
            port,
            channel,
            accent,
            beat,
            // 7비트 127과 100. 강박이 더 크게 들리게 한다(값은 REQ에 없어 흔한 값을 쓴다).
            accent_velocity: Velocity::new(0xFFFF),
            beat_velocity: Velocity::from_midi1(100).unwrap_or(Velocity::new(100 << 9)),
            length: DEFAULT_CLICK_LENGTH,
        }
    }

    #[must_use]
    pub fn with_velocities(self, accent: Velocity, beat: Velocity) -> Self {
        MetronomeConfig {
            accent_velocity: accent,
            beat_velocity: beat,
            ..self
        }
    }

    /// 클릭 길이. 0이면 1 ms, [`MAX_NOTE_LENGTH`]를 넘으면 상한으로 줄인다.
    #[must_use]
    pub fn with_length(self, length: Duration) -> Self {
        MetronomeConfig {
            length: clamp_length(length),
            ..self
        }
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
        // 채널 10은 GM 드럼 채널이고, 37은 사이드 스틱, 42는 닫힌 하이햇이다.
        let channel = Channel::DRUMS;
        let accent = Pitch::new(37).unwrap_or(Pitch::MIN);
        let beat = Pitch::new(42).unwrap_or(Pitch::MIN);
        MetronomeConfig::new(0, channel, accent, beat)
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
