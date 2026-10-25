//! 노트 하나를 트랙의 포트·채널로 들려주는 공통 경로(PRE-1 미리듣기, PRE-3 테스트 노트). S3-T2가 S2-T5c의
//! 테스트 노트 코드를 여기로 옮겼다.
//!
//! 규칙: 이 창이 재생 중이면 그 스냅샷의 포트로(뱅크·프로그램은 재생이 이미 보냄), 다른 창이 재생 중이면
//! 기본 가상 포트로만(그 재생의 스냅샷·음색을 건드리지 않음), 정지 중이면 이 프로젝트의 스냅샷을 넣어 포트를
//! 열고 뱅크·프로그램을 먼저 보낸다.

use std::time::Duration;

use gpui_kit::{BorrowAppContext as _, Context};
use keyscore_core::{OutputPort, Pitch, Track, Velocity};
use keyscore_midi::{MidiError, bank_select, program_change};

use crate::engine::{Engine, PortProblem, TransportState};
use crate::window::ProjectWindow;

/// 노트를 보낸 결과.
#[derive(Debug)]
pub(crate) enum NoteSend {
    /// 보냈다.
    Sent,
    /// MIDI 엔진이 없다(CoreMIDI를 열지 못했다).
    NoEngine,
    /// 다른 창이 재생 중이라 이름 포트를 열 수 없다.
    OtherPlaying,
    /// 포트를 열지 못했다(이유 문구: 「출력 포트 「x」이 없어」 등).
    PortMissing(String),
    /// 스케줄러 오류.
    Error(MidiError),
}

impl NoteSend {
    /// 실패 이유를 「… 테스트 노트를 보내지 못했다」처럼 이어 쓸 수 있는 문구로. 성공이면 `None`.
    pub(crate) fn failure(&self, what: &str) -> Option<String> {
        Some(match self {
            NoteSend::Sent => return None,
            NoteSend::NoEngine => {
                format!("MIDI 엔진이 없어 {what}를 보낼 수 없다(CoreMIDI를 열지 못했다)")
            }
            NoteSend::OtherPlaying => {
                format!(
                    "다른 창이 재생 중이라 이름 포트로 {what}를 보낼 수 없다. 정지한 뒤 다시 한다"
                )
            }
            NoteSend::PortMissing(reason) => format!("{reason} {what}를 보내지 못했다"),
            NoteSend::Error(e) => format!("{what}를 보내지 못했다: {e}"),
        })
    }
}

impl ProjectWindow {
    /// `track`의 포트·채널로 `pitch`를 `length` 동안 들려준다.
    pub(crate) fn sound_note(
        &mut self,
        track: &Track,
        pitch: Pitch,
        velocity: Velocity,
        length: Duration,
        cx: &mut Context<Self>,
    ) -> NoteSend {
        if !cx.has_global::<Engine>() {
            return NoteSend::NoEngine;
        }
        let destination = match track.port() {
            OutputPort::Virtual => String::new(),
            OutputPort::Named(name) => name.clone(),
        };
        let me = cx.entity().downgrade();
        let playing_here =
            self.transport.state == TransportState::Playing && cx.global::<Engine>().is_owner(&me);
        let result: Result<bool, MidiError> = if playing_here {
            // 지금 스냅샷의 포트로 보낸다. 뱅크·프로그램은 재생이 이미 보냈다.
            let port = self
                .transport
                .routes
                .iter()
                .find(|(d, _)| *d == destination)
                .map(|(_, p)| *p);
            match port {
                Some(port) => send_note(
                    cx.global_mut::<Engine>(),
                    port,
                    track,
                    pitch,
                    velocity,
                    length,
                    false,
                ),
                None => Ok(false),
            }
        } else if cx.global::<Engine>().has_owner() && !destination.is_empty() {
            // 다른 창의 재생 스냅샷은 바꾸지 않는다. 기본 가상 포트(포트 0)는 어느 스냅샷에도 있다.
            return NoteSend::OtherPlaying;
        } else if destination.is_empty() {
            // 다른 창이 재생 중이면 뱅크·프로그램을 보내지 않는다. 같은 채널을 쓰는 그 재생의 음색이 바뀐다
            // (S2-T5c 리뷰 1 지적 3). 노트만 보낸다.
            let other_playing = cx.global::<Engine>().has_owner();
            send_note(
                cx.global_mut::<Engine>(),
                0,
                track,
                pitch,
                velocity,
                length,
                !other_playing,
            )
        } else {
            // 정지 중: 이 프로젝트의 스냅샷을 넣어 포트를 연다(재생은 하지 않는다).
            let document = self.document.clone();
            let mut missing = Vec::new();
            // 메트로놈 포트도 넘긴다. 빼면 준비가 그 포트를 「쓰지 않는 포트」로 보고 놓는다.
            let metronome_port = crate::transport::metronome_settings(cx).port;
            let clock_ports = crate::transport::clock_settings(cx);
            let result = cx.update_global::<Engine, _>(|engine, cx| {
                let prepared = engine.prepare(
                    document.read(cx).project(),
                    false,
                    &metronome_port,
                    &clock_ports,
                )?;
                let port = prepared
                    .routes
                    .iter()
                    .find(|(d, _)| *d == destination)
                    .map(|(_, p)| *p);
                let Some(port) = port else {
                    missing = prepared.missing;
                    return Ok(false);
                };
                let scheduler = engine.scheduler();
                scheduler.collect_garbage();
                scheduler.swap(prepared.snapshot)?;
                send_note(engine, port, track, pitch, velocity, length, true)
            });
            // 포트를 열지 못한 이유(없음·열기 실패·상한)를 재생 시작과 같은 말로 알린다(S2-T5c 리뷰 1 제안 5).
            // 이유와 결과를 한 메시지로 알린다. 상태줄은 마지막 메시지만 보이므로 이유가 따로 앞에 있으면
            // 보이는 말과 실제 이유가 어긋난다(리뷰 2 제안 3).
            if let Some((name, problem)) = missing.iter().find(|(name, _)| *name == destination) {
                let reason = match problem {
                    PortProblem::NotFound => format!("출력 포트 「{name}」이 없어"),
                    PortProblem::OpenFailed(e) => format!("출력 포트 「{name}」을 열지 못해({e})"),
                    PortProblem::TooMany => format!(
                        "출력 포트가 너무 많아(최대 {}) 「{name}」을 열지 못해",
                        keyscore_midi::MAX_SCHEDULER_PORTS
                    ),
                };
                return NoteSend::PortMissing(reason);
            }
            result
        };
        match result {
            Ok(true) => NoteSend::Sent,
            Ok(false) => {
                let virtual_name = crate::engine::virtual_port_name(cx);
                let port = crate::tracks::port_label(track.port(), &virtual_name);
                NoteSend::PortMissing(format!("출력 포트 「{port}」을 열지 못해"))
            }
            Err(e) => NoteSend::Error(e),
        }
    }
}

/// 노트를 보낸다. `setup`이면 먼저 그 트랙의 뱅크·프로그램을 보낸다. 보냈으면 `true`.
fn send_note(
    engine: &mut Engine,
    port: u16,
    track: &Track,
    pitch: Pitch,
    velocity: Velocity,
    length: Duration,
    setup: bool,
) -> Result<bool, MidiError> {
    let scheduler = engine.scheduler();
    let channel = track.channel();
    if setup {
        if let Some(bank) = track.bank() {
            for msg in bank_select(channel, bank) {
                scheduler.send_now(port, msg)?;
            }
        }
        if let Some(program) = track.program() {
            scheduler.send_now(port, program_change(channel, program))?;
        }
    }
    scheduler.preview(port, channel, pitch, velocity, length)?;
    Ok(true)
}
