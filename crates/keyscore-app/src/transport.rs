//! 창의 트랜스포트(S2-T5a, TRN-1·2·3, OUT-4·6). 엔진은 앱에 하나이고, 한 번에 한 창만 재생한다.

use std::time::Duration;

use gpui_kit::{BorrowAppContext as _, Context, SharedString, Window};
use keyscore_core::{MusicTime, TempoMap, Tick};
use keyscore_midi::{HostTime, MidiError};

use crate::actions::transport;
use crate::app_state::AppSettings;
use crate::engine::{Engine, PortProblem, Setup, TransportState, clock_commands};
use crate::settings::MetronomeSettings;
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
    /// 지금 스냅샷의 목적지 → 포트 번호(테스트 노트, PRE-3).
    pub(crate) routes: Vec<(String, u16)>,
    pub(crate) metronome: bool,
    /// 지금 스냅샷에 메트로놈 포트가 없다(MET-1). 그동안은 버튼이 켜져 있어도 클릭을 끈다.
    pub(crate) metronome_missing: bool,
    /// 지난 교체에서 메트로놈 설정이 들어가지 않았다. 다음 교체까지 클릭을 켜지 않는다.
    metronome_unset: bool,
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
        self.metronome_missing = false;
        self.metronome_unset = false;
    }

    /// 스케줄러에 넘길 메트로놈 켬: 버튼이 켜져 있고 이 스냅샷에 메트로놈 포트가 있을 때.
    fn clicks(&self) -> bool {
        self.metronome && !self.metronome_missing && !self.metronome_unset
    }
}

/// 스냅샷을 넣은 결과. 창 상태에 옮기고 알릴 것.
struct Swapped {
    missing: Vec<(String, PortProblem)>,
    setup: Vec<Setup>,
    routes: Vec<(String, u16)>,
    tempo: TempoMap,
    metronome_problem: Option<(String, PortProblem)>,
    clock_missing: Vec<(String, PortProblem)>,
    /// 교체 뒤 명령의 실패(재생 중 교체에서만). 교체는 들어갔으므로 알리기만 한다.
    metronome_error: Option<MidiError>,
    clock_error: Option<MidiError>,
    send_error: Option<MidiError>,
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
        let on = self.transport.clicks();
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
        let metronome = metronome_settings(cx);
        let clock_ports = clock_settings(cx);
        let button = self.transport.metronome;
        let result = cx.update_global::<Engine, _>(|engine, cx| {
            let project = document.read(cx).project();
            let prepared = engine.prepare(project, true, &metronome.port, &clock_ports)?;
            let tempo = project.tempo_map().clone();
            // 지난 클럭 상태를 꺼낸다. 이 묶음이 끝까지 성공해야 새 상태를 적는다. 중간에 실패하면 모르는 상태로
            // 남아 다음 교체가 모든 번호를 다시 맞춘다(S2-T6c 리뷰 2 제안 1 이월).
            let old_clock = engine.take_clock_state();
            let (clock_off, clock_on) = clock_commands(old_clock.as_deref(), &prepared.clock);
            let scheduler = engine.scheduler();
            // 스케줄러가 돌려준 옛 스냅샷을 해제한다. 쌓이면 교체 명령이 큐에서 막힌다(리뷰 2 지적 1).
            scheduler.collect_garbage();
            // 포트 번호는 스냅샷마다 다시 매겨진다. 명령은 따로 큐에 들어가므로, 스케줄러가 교체와 새 설정 사이에
            // 깨면 새 스냅샷에 옛 포트 번호로 클릭·클럭이 나갈 수 있다. 교체 전에 클릭과 바뀐 클럭 번호를 끄고,
            // 교체 뒤에 새 번호로 켠다(S2-T6c 리뷰 1 지적 2). 그 틈의 박 하나(클럭 한 칸)는 빠질 수 있다.
            scheduler.set_metronome_enabled(false)?;
            let mut clock_error = None;
            for port in clock_off {
                if let Err(e) = scheduler.set_clock(port, false) {
                    clock_error.get_or_insert(e);
                }
            }
            scheduler.swap(prepared.snapshot)?;
            // 교체는 들어갔다. 이 뒤의 실패는 상태를 되돌리지 않고 알리기만 한다(리뷰 2 제안 1).
            let mut send_error = None;
            let mut metronome_error = None;
            if let Some(port) = prepared.metronome
                && let Err(e) = scheduler.set_metronome(metronome.config(port))
            {
                metronome_error = Some(e);
            } else if let Err(e) =
                scheduler.set_metronome_enabled(button && prepared.metronome.is_some())
            {
                metronome_error = Some(e);
            }
            for port in clock_on {
                if let Err(e) = scheduler.set_clock(port, true) {
                    clock_error.get_or_insert(e);
                }
            }
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
            if clock_error.is_none() {
                engine.set_clock_state(prepared.clock);
            }
            Ok::<_, MidiError>(Swapped {
                missing: prepared.missing,
                setup: prepared.setup,
                routes: prepared.routes,
                tempo,
                metronome_problem: prepared.metronome_problem,
                clock_missing: prepared.clock_missing,
                metronome_error,
                clock_error,
                send_error,
            })
        });
        match result {
            Ok(swapped) => {
                self.transport.anchor = Some(Anchor {
                    at: now,
                    tick: here,
                    tempo: swapped.tempo.clone(),
                });
                self.apply_swapped(swapped);
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
        let button = self.transport.metronome;
        let metronome = metronome_settings(cx);
        let clock_ports = clock_settings(cx);
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
            let prepared = engine.prepare(project, false, &metronome.port, &clock_ports)?;
            let tempo = project.tempo_map().clone();
            // 실패하면 클럭 상태는 모르는 채로 남는다(다음 재생이 모든 번호를 다시 맞춘다)
            let old_clock = engine.take_clock_state();
            let (clock_off, clock_on) = clock_commands(old_clock.as_deref(), &prepared.clock);
            let scheduler = engine.scheduler();
            scheduler.collect_garbage();
            scheduler.stop()?;
            scheduler.swap(prepared.snapshot)?;
            if let Some(port) = prepared.metronome {
                scheduler.set_metronome(metronome.config(port))?;
            }
            scheduler.set_metronome_enabled(button && prepared.metronome.is_some())?;
            // 클럭 번호는 재생 전에 맞춘다. 재생 시작의 Start/SPP가 그 포트들로 나간다(OUT-5).
            for port in clock_off {
                scheduler.set_clock(port, false)?;
            }
            for port in clock_on {
                scheduler.set_clock(port, true)?;
            }
            scheduler.play(from)?;
            engine.set_clock_state(prepared.clock);
            engine.take_owner(Some(me.clone()));
            Ok::<_, MidiError>(Swapped {
                missing: prepared.missing,
                setup: prepared.setup,
                routes: prepared.routes,
                tempo,
                metronome_problem: prepared.metronome_problem,
                clock_missing: prepared.clock_missing,
                metronome_error: None,
                clock_error: None,
                send_error: None,
            })
        });
        match result {
            Ok(swapped) => {
                self.transport.state = TransportState::Playing;
                self.transport.anchor = Some(Anchor::new(from, swapped.tempo.clone()));
                self.apply_swapped(swapped);
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

    /// MIDI 서버가 다시 떠서 엔진이 재생을 멈췄다.
    pub(crate) fn transport_stopped_by_server(&mut self, cx: &mut Context<Self>) {
        if self.transport.state == TransportState::Playing {
            self.transport
                .say("MIDI 서버가 다시 시작되어 재생을 멈췄다");
        }
        self.transport.mark_stopped();
        cx.notify();
    }

    /// 이름 포트가 사라졌다(포트 감시). 이 창이 재생 중이면 그 포트 없이 스냅샷을 다시 만든다.
    pub(crate) fn transport_port_lost(&mut self, cx: &mut Context<Self>) {
        self.transport_document_changed(cx);
    }

    /// 이름 포트가 돌아왔다(포트 감시). 이 창이 재생 중이면 스냅샷을 다시 만들어 새 핸들로 보내고, 그
    /// 목적지의 뱅크·프로그램을 다시 보낸다. 돌아온 장치는 새로 켜졌을 수 있다.
    pub(crate) fn transport_port_returned(&mut self, name: &str, cx: &mut Context<Self>) {
        let me = cx.entity().downgrade();
        if self.transport.state != TransportState::Playing
            || !cx.has_global::<Engine>()
            || !cx.global::<Engine>().is_owner(&me)
        {
            return;
        }
        // 그 목적지의 지난 음색을 잊는다. 그러면 스냅샷 교체가 그 목적지의 뱅크·프로그램을 「바뀐 것」으로 보고
        // 한 번 보낸다(따로 보내면 두 번 나간다, 리뷰 1 제안 5).
        self.transport.setup.retain(|s| s.destination != name);
        self.transport_document_changed(cx);
    }

    /// 교체 결과를 창 상태에 옮기고 문제를 알린다.
    fn apply_swapped(&mut self, s: Swapped) {
        self.transport.setup = s.setup;
        self.transport.routes = s.routes;
        self.report_missing(&s.missing);
        self.report_metronome(s.metronome_problem.as_ref());
        // 메트로놈 설정이 들어가지 않았으면 다음 교체까지 버튼 토글로 켜지 않는다(옛 포트 번호로 울리지 않게,
        // S2-T6c 리뷰 2 제안 1 이월)
        self.transport.metronome_unset = s.metronome_error.is_some();
        for (name, problem) in &s.clock_missing {
            let message = match problem {
                PortProblem::NotFound => {
                    format!("MIDI Clock 포트 「{name}」이 없어 클럭을 보내지 않는다")
                }
                PortProblem::OpenFailed(e) => {
                    format!("MIDI Clock 포트 「{name}」을 열지 못해 클럭을 보내지 않는다: {e}")
                }
                PortProblem::TooMany => format!(
                    "출력 포트가 너무 많아(최대 {}) MIDI Clock 포트 「{name}」로 클럭을 보내지 않는다",
                    keyscore_midi::MAX_SCHEDULER_PORTS
                ),
            };
            self.say_once(message);
        }
        if let Some(e) = s.metronome_error {
            self.transport
                .say(format!("바뀐 메트로놈 설정을 보내지 못했다: {e}"));
        }
        if let Some(e) = s.clock_error {
            self.transport.say(format!(
                "MIDI Clock 설정을 보내지 못했다(다음 편집 때 다시 맞춘다): {e}"
            ));
        }
        if let Some(e) = s.send_error {
            self.transport
                .say(format!("바뀐 뱅크·프로그램을 보내지 못했다: {e}"));
        }
    }

    /// 같은 메시지가 이미 있으면 다시 남기지 않는다.
    fn say_once(&mut self, message: String) {
        if !self
            .transport
            .messages
            .iter()
            .any(|m| m.as_ref() == message)
        {
            self.transport.say(message);
        }
    }

    /// 메트로놈 포트를 찾지 못했으면 클릭을 끈 채로 두고 알린다(MET-1). 찾았으면 표시를 지운다.
    fn report_metronome(&mut self, problem: Option<&(String, PortProblem)>) {
        self.transport.metronome_missing = problem.is_some();
        let Some((name, problem)) = problem else {
            return;
        };
        let message = match problem {
            PortProblem::NotFound => format!("메트로놈 포트 「{name}」이 없어 클릭이 나지 않는다"),
            PortProblem::OpenFailed(e) => {
                format!("메트로놈 포트 「{name}」을 열지 못해 클릭이 나지 않는다: {e}")
            }
            PortProblem::TooMany => format!(
                "출력 포트가 너무 많아(최대 {}) 메트로놈 포트 「{name}」로 클릭이 나지 않는다",
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

    /// 설정(메트로놈 포트·값, 가상 포트 목록)이 바뀌었다. 재생 중이면 스냅샷을 다시 만들어 바로 반영한다.
    pub(crate) fn transport_refresh(&mut self, cx: &mut Context<Self>) {
        self.transport_document_changed(cx);
    }

    /// 앱 가상 포트의 이름이 바뀌었다(같은 엔드포인트). 지금 재생의 음색 기록을 새 이름으로 옮긴다.
    pub(crate) fn transport_port_renamed(&mut self, old: &str, new: &str) {
        for setup in &mut self.transport.setup {
            if setup.destination == old {
                new.clone_into(&mut setup.destination);
            }
        }
    }

    pub(crate) fn report_missing(&mut self, missing: &[(String, PortProblem)]) {
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

/// 지금 메트로놈 설정(설정 창이 바꾼다). 설정 전역이 없으면(시험) 기본값.
pub(crate) fn metronome_settings(cx: &gpui_kit::App) -> MetronomeSettings {
    if cx.has_global::<AppSettings>() {
        AppSettings::get(cx).metronome.clone()
    } else {
        MetronomeSettings::default()
    }
}

/// 지금 클럭 포트 설정(OUT-5). 설정 전역이 없으면(시험) 없음.
pub(crate) fn clock_settings(cx: &gpui_kit::App) -> Vec<keyscore_core::OutputPort> {
    if cx.has_global::<AppSettings>() {
        AppSettings::get(cx).midi_output.clock.clone()
    } else {
        Vec::new()
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
