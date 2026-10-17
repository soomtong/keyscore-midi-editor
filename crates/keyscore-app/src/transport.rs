//! 창의 트랜스포트(S2-T5a, TRN-1·2·3, OUT-4·6). 엔진은 앱에 하나이고, 한 번에 한 창만 재생한다.

use std::time::Duration;

use gpui_kit::{BorrowAppContext as _, Context, SharedString, Window};
use keyscore_core::{MusicTime, Project, Tick};
use keyscore_midi::{HostTime, MetronomeConfig, MidiError};

use crate::actions::transport;
use crate::engine::{Engine, TransportState};
use crate::window::ProjectWindow;

/// 메시지를 몇 개까지 남기는가. 오래된 것부터 버린다.
const MAX_MESSAGES: usize = 32;

/// 창 하나의 트랜스포트 상태.
#[derive(Default)]
pub(crate) struct Transport {
    pub(crate) state: TransportState,
    /// 재생을 시작한 (호스트 시각, 틱). 일시정지 자리를 계산한다(스케줄러에 위치 조회가 없다).
    started: Option<(HostTime, Tick)>,
    pub(crate) metronome: bool,
    pub(crate) messages: Vec<SharedString>,
}

impl Transport {
    fn say(&mut self, message: impl Into<SharedString>) {
        if self.messages.len() == MAX_MESSAGES {
            self.messages.remove(0);
        }
        self.messages.push(message.into());
    }

    /// 다른 창이 재생을 가져갔거나 창이 정지했다.
    pub(crate) fn mark_stopped(&mut self) {
        self.state = TransportState::Stopped;
        self.started = None;
    }
}

/// 재생을 시작한 뒤 흐른 시간만큼 간 틱.
fn position(project: &Project, started: (HostTime, Tick)) -> Tick {
    let (at, from) = started;
    let elapsed: Duration = HostTime::now().duration_since(at);
    let tempo = project.tempo_map();
    let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
    let start_time = tempo.tick_to_time(from).units();
    let now =
        MusicTime::from_units(start_time.saturating_add(MusicTime::from_nanos(nanos).units()));
    tempo.time_to_tick(now)
}

impl ProjectWindow {
    pub(crate) fn transport_play(
        &mut self,
        _: &transport::Play,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let from = match self.transport.state {
            TransportState::Paused(at) => at,
            TransportState::Stopped | TransportState::Playing => Tick::ZERO,
        };
        self.play_from(from, cx);
    }

    /// 커서가 생기기 전(S3)까지는 처음부터다.
    pub(crate) fn transport_play_from_cursor(
        &mut self,
        _: &transport::PlayFromCursor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.play_from(Tick::ZERO, cx);
    }

    pub(crate) fn transport_toggle(
        &mut self,
        _: &transport::Toggle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.transport.state == TransportState::Playing {
            self.stop(cx);
        } else {
            self.transport_play(&transport::Play, window, cx);
        }
    }

    pub(crate) fn transport_stop(
        &mut self,
        _: &transport::Stop,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop(cx);
    }

    pub(crate) fn transport_pause(
        &mut self,
        _: &transport::Pause,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.transport.state != TransportState::Playing {
            return;
        }
        let at = self.transport.started.map_or(Tick::ZERO, |s| {
            position(self.document.read(cx).project(), s)
        });
        self.stop(cx);
        self.transport.state = TransportState::Paused(at);
        cx.notify();
    }

    pub(crate) fn transport_panic(
        &mut self,
        _: &transport::Panic,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !cx.has_global::<Engine>() {
            self.transport.say("MIDI 엔진이 없어 패닉을 보낼 수 없다");
            cx.notify();
            return;
        }
        if let Err(e) = cx.global_mut::<Engine>().scheduler().panic() {
            self.transport.say(format!("패닉을 보내지 못했다: {e}"));
            cx.notify();
        }
    }

    pub(crate) fn transport_toggle_metronome(
        &mut self,
        _: &transport::ToggleMetronome,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.transport.metronome = !self.transport.metronome;
        let me = cx.entity().downgrade();
        let on = self.transport.metronome;
        if cx.has_global::<Engine>() && cx.global::<Engine>().is_owner(&me) {
            let result = cx
                .global_mut::<Engine>()
                .scheduler()
                .set_metronome_enabled(on);
            self.report(result, cx);
        }
        cx.notify();
    }

    /// 재생 중 문서가 바뀌었다. 스냅샷을 다시 만들어 교체한다(TRN-3).
    pub(crate) fn transport_document_changed(&mut self, cx: &mut Context<Self>) {
        let me = cx.entity().downgrade();
        if self.transport.state != TransportState::Playing
            || !cx.has_global::<Engine>()
            || !cx.global::<Engine>().is_owner(&me)
        {
            return;
        }
        let document = self.document.clone();
        let result = cx.update_global::<Engine, _>(|engine, cx| {
            let prepared = engine.prepare(document.read(cx).project())?;
            engine.scheduler().swap(prepared.snapshot)?;
            Ok::<_, MidiError>(prepared.missing)
        });
        match result {
            Ok(missing) => self.report_missing(&missing),
            Err(e) => self
                .transport
                .say(format!("재생 중 편집을 반영하지 못했다: {e}")),
        }
        cx.notify();
    }

    fn play_from(&mut self, from: Tick, cx: &mut Context<Self>) {
        if !cx.has_global::<Engine>() {
            self.transport
                .say("MIDI 엔진이 없어 재생할 수 없다(CoreMIDI를 열지 못했다)");
            cx.notify();
            return;
        }
        let me = cx.entity().downgrade();
        let document = self.document.clone();
        let metronome = self.transport.metronome;
        let result = cx.update_global::<Engine, _>(|engine, cx| {
            let prepared = engine.prepare(document.read(cx).project())?;
            let scheduler = engine.scheduler();
            scheduler.stop()?;
            scheduler.swap(prepared.snapshot)?;
            for (port, msg) in &prepared.setup {
                scheduler.send_now(*port, *msg)?;
            }
            scheduler.set_metronome(MetronomeConfig::default())?;
            scheduler.set_metronome_enabled(metronome)?;
            scheduler.play(from)?;
            Ok::<_, MidiError>((prepared.missing, engine.take_owner(Some(me.clone()))))
        });
        match result {
            Ok((missing, previous)) => {
                self.transport.state = TransportState::Playing;
                self.transport.started = Some((HostTime::now(), from));
                self.report_missing(&missing);
                if let Some(previous) = previous.filter(|p| p != &me) {
                    let _ = previous.update(cx, |other, cx| {
                        other.transport.mark_stopped();
                        cx.notify();
                    });
                }
            }
            Err(e) => self.transport.say(format!("재생하지 못했다: {e}")),
        }
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        let me = cx.entity().downgrade();
        if cx.has_global::<Engine>() && cx.global::<Engine>().is_owner(&me) {
            let engine = cx.global_mut::<Engine>();
            engine.take_owner(None);
            let result = engine.scheduler().stop();
            self.report(result, cx);
        }
        self.transport.mark_stopped();
        cx.notify();
    }

    fn report(&mut self, result: Result<(), MidiError>, cx: &mut Context<Self>) {
        if let Err(e) = result {
            self.transport.say(format!("MIDI 오류: {e}"));
            cx.notify();
        }
    }

    fn report_missing(&mut self, missing: &[String]) {
        for name in missing {
            let message = format!("출력 포트 「{name}」이 없어 그 트랙은 소리 내지 않는다");
            if !self
                .transport
                .messages
                .iter()
                .any(|m| m.as_ref() == message)
            {
                self.transport.say(message);
            }
        }
    }
}

/// 창이 닫힌다. 그 창이 재생 중이었으면 멈춘다. 닫히는 창의 약한 참조는 이미 끊겼으므로, 주인이 살아
/// 있지 않으면 그 창이었다.
pub(crate) fn release(cx: &mut gpui_kit::App) {
    if !cx.has_global::<Engine>() {
        return;
    }
    let engine = cx.global_mut::<Engine>();
    match engine.take_owner(None).filter(|o| o.upgrade().is_some()) {
        Some(other) => {
            engine.take_owner(Some(other));
        }
        None => {
            let _ = engine.scheduler().stop();
        }
    }
}
