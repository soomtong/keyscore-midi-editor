//! 창의 트랜스포트(S2-T5a, TRN-1·2·3, OUT-4·6). 엔진은 앱에 하나이고, 한 번에 한 창만 재생한다.

use std::time::Duration;

use gpui_kit::{BorrowAppContext as _, Context, SharedString, Window};
use keyscore_core::{MusicTime, TempoMap, Tick};
use keyscore_midi::{HostTime, MetronomeConfig, MidiError};

use crate::actions::transport;
use crate::engine::{Engine, PortProblem, Setup, TransportState};
use crate::window::ProjectWindow;

/// 메시지를 몇 개까지 남기는가. 오래된 것부터 버린다.
const MAX_MESSAGES: usize = 32;

/// 창 하나의 트랜스포트 상태.
#[derive(Default)]
pub(crate) struct Transport {
    pub(crate) state: TransportState,
    /// 재생 위치의 기준점. 일시정지 자리를 계산한다(스케줄러에 위치 조회가 없다).
    anchor: Option<Anchor>,
    /// 지금 스냅샷의 뱅크·프로그램. 재생 중 편집으로 바뀐 것만 보낸다(OUT-4, TRN-3).
    setup: Vec<Setup>,
    pub(crate) metronome: bool,
    pub(crate) messages: Vec<SharedString>,
}

impl Transport {
    pub(crate) fn say(&mut self, message: impl Into<SharedString>) {
        if self.messages.len() == MAX_MESSAGES {
            self.messages.remove(0);
        }
        self.messages.push(message.into());
    }

    /// 표시할 재생 위치: 재생 중이면 지금 자리, 일시정지면 그 자리, 정지면 처음.
    pub(crate) fn position(&self) -> Tick {
        match self.state {
            TransportState::Playing => self.anchor.as_ref().map_or(Tick::ZERO, Anchor::position),
            TransportState::Paused(at) => at,
            TransportState::Stopped => Tick::ZERO,
        }
    }

    /// 다른 창이 재생을 가져갔거나 창이 정지했다.
    pub(crate) fn mark_stopped(&mut self) {
        self.state = TransportState::Stopped;
        self.anchor = None;
        self.setup.clear();
    }
}

/// 재생 위치의 기준점: 이 호스트 시각에 이 틱이었고, 그 뒤로는 이 템포 맵으로 흐른다.
///
/// 재생 중 편집으로 템포가 바뀌면 스케줄러는 이미 보낸 구간을 옛 템포로 두고 그 자리부터 새 템포로
/// 이어 간다. 그래서 스냅샷을 교체할 때마다 옛 템포로 지금 자리를 구해 기준점을 다시 잡는다(리뷰 1 지적 1).
struct Anchor {
    at: HostTime,
    tick: Tick,
    tempo: TempoMap,
}

impl Anchor {
    fn new(tick: Tick, tempo: TempoMap) -> Self {
        Anchor {
            at: HostTime::now(),
            tick,
            tempo,
        }
    }

    /// 기준점 뒤 흐른 시간만큼 간 틱.
    fn position(&self) -> Tick {
        self.position_at(HostTime::now())
    }

    /// 호스트 시각 `now`의 틱.
    fn position_at(&self, now: HostTime) -> Tick {
        let elapsed: Duration = now.duration_since(self.at);
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        let start = self.tempo.tick_to_time(self.tick).units();
        let now = MusicTime::from_units(start.saturating_add(MusicTime::from_nanos(nanos).units()));
        self.tempo.time_to_tick(now)
    }
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
        let at = self
            .transport
            .anchor
            .as_ref()
            .map_or(Tick::ZERO, Anchor::position);
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

    /// 재생 중 문서가 바뀌었다. 스냅샷을 다시 만들어 교체하고, 바뀐 뱅크·프로그램을 보낸다(TRN-3, OUT-4).
    pub(crate) fn transport_document_changed(&mut self, cx: &mut Context<Self>) {
        let me = cx.entity().downgrade();
        if self.transport.state != TransportState::Playing
            || !cx.has_global::<Engine>()
            || !cx.global::<Engine>().is_owner(&me)
        {
            return;
        }
        // 교체 직전의 자리를 옛 템포로 구한다. 새 기준점의 시각도 같은 순간으로 잡아야, 스냅샷을 만드는
        // 동안 흐른 시간이 빠지지 않는다(리뷰 2 지적 2).
        let now = HostTime::now();
        let here = self
            .transport
            .anchor
            .as_ref()
            .map_or(Tick::ZERO, |a| a.position_at(now));
        let document = self.document.clone();
        let old_setup = std::mem::take(&mut self.transport.setup);
        let result = cx.update_global::<Engine, _>(|engine, cx| {
            let project = document.read(cx).project();
            let prepared = engine.prepare(project, true)?;
            let tempo = project.tempo_map().clone();
            let scheduler = engine.scheduler();
            // 스케줄러가 돌려준 옛 스냅샷을 해제한다. 쌓이면 교체 명령이 큐에서 막힌다(리뷰 2 지적 1).
            scheduler.collect_garbage();
            scheduler.swap(prepared.snapshot)?;
            // 교체는 들어갔다. 이 뒤의 실패는 상태를 되돌리지 않고 알리기만 한다(리뷰 2 제안 1).
            let mut send_error = None;
            for entry in prepared
                .setup
                .iter()
                .filter(|new| !old_setup.iter().any(|old| old.same_as(new)))
            {
                for msg in &entry.messages {
                    if let Err(e) = scheduler.send_now(entry.port, *msg) {
                        send_error.get_or_insert(e);
                    }
                }
            }
            Ok::<_, MidiError>((prepared.missing, prepared.setup, tempo, send_error))
        });
        match result {
            Ok((missing, setup, tempo, send_error)) => {
                self.transport.anchor = Some(Anchor {
                    at: now,
                    tick: here,
                    tempo,
                });
                self.transport.setup = setup;
                self.report_missing(&missing);
                if let Some(e) = send_error {
                    self.transport
                        .say(format!("바뀐 뱅크·프로그램을 보내지 못했다: {e}"));
                }
            }
            Err(e) => {
                self.transport.setup = old_setup;
                self.transport
                    .say(format!("재생 중 편집을 반영하지 못했다: {e}"));
            }
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
        // 앞 주인은 결과와 무관하게 정지 상태다. 아래 `stop`이 그 재생을 멈춘다.
        let previous = cx
            .global_mut::<Engine>()
            .take_owner(None)
            .filter(|p| p != &me);
        if let Some(previous) = previous {
            let _ = previous.update(cx, |other, cx| {
                other.transport.mark_stopped();
                cx.notify();
            });
        }
        // 재생 시작 위치의 뱅크·프로그램은 스케줄러가 따라잡아 보낸다(chase). 따로 보내면 명령 큐가
        // 트랙 수만큼 차서 재생이 실패할 수 있다(리뷰 1 지적 3).
        let result = cx.update_global::<Engine, _>(|engine, cx| {
            let project = document.read(cx).project();
            let prepared = engine.prepare(project, false)?;
            let tempo = project.tempo_map().clone();
            let scheduler = engine.scheduler();
            scheduler.collect_garbage();
            scheduler.stop()?;
            scheduler.swap(prepared.snapshot)?;
            scheduler.set_metronome(MetronomeConfig::default())?;
            scheduler.set_metronome_enabled(metronome)?;
            scheduler.play(from)?;
            engine.take_owner(Some(me.clone()));
            Ok::<_, MidiError>((prepared.missing, prepared.setup, tempo))
        });
        match result {
            Ok((missing, setup, tempo)) => {
                self.transport.state = TransportState::Playing;
                self.transport.anchor = Some(Anchor::new(from, tempo));
                self.transport.setup = setup;
                self.report_missing(&missing);
            }
            Err(e) => {
                // 실패 전에 들어간 명령(정지 등)이 있을 수 있다. 상태를 소리와 맞춘다.
                let _ = cx.global_mut::<Engine>().scheduler().stop();
                self.transport.mark_stopped();
                self.transport.say(format!("재생하지 못했다: {e}"));
            }
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

    fn report_missing(&mut self, missing: &[(String, PortProblem)]) {
        for (name, problem) in missing {
            let message = match problem {
                PortProblem::NotFound => {
                    format!("출력 포트 「{name}」이 없어 그 트랙은 소리 내지 않는다")
                }
                PortProblem::OpenFailed(e) => {
                    format!("출력 포트 「{name}」을 열지 못해 그 트랙은 소리 내지 않는다: {e}")
                }
                PortProblem::TooMany => format!(
                    "출력 포트가 너무 많아(최대 {}) 「{name}」 트랙은 소리 내지 않는다",
                    keyscore_midi::MAX_SCHEDULER_PORTS
                ),
            };
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
