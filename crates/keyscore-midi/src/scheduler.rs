//! MIDI 스케줄러 스레드(REQ 9.2, 9.3, OUT-5, OUT-6, OUT-7, CTL-6, TRN-3).
//!
//! 제어 스레드가 만든 불변 스냅샷의 이벤트를 템포 맵으로 호스트 시각에 맞춰 몇 ms 앞서 예약 송신한다.
//! 정확한 송신 시각은 CoreMIDI 서버가 맞춘다(SP-3). 스케줄러 루프에는 할당·락·로그가 없다. 루프의
//! 시스템 콜은 MIDI 송신과 시각 대기(`mach_wait_until`)뿐이다.
//!
//! 제어 → 스케줄러 명령과, 스케줄러 → 제어로 돌려주는 스냅샷은 락 없는 SPSC 링버퍼(`rtrb`)로
//! 오간다. 교체된 스냅샷을 돌려받아 제어 스레드에서 해제하므로, 할당 해제도 스케줄러 스레드에서
//! 일어나지 않는다.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use keyscore_core::{MusicTime, TempoMap, Tick};

use crate::rt::{Timebase, make_realtime, wait_until};
use crate::{Group, HostTime, Midi1Message, MidiError, MidiPort};

/// 한 스냅샷이 가질 수 있는 포트 수. 스케줄러의 상태 표를 시작할 때 이 크기로 미리 잡는다.
pub const MAX_SCHEDULER_PORTS: usize = 32;

/// 24 ppqn MIDI Clock 한 칸의 틱(PPQ 960 / 24).
const CLOCK_TICKS: u64 = 40;
/// Song Position Pointer 한 칸(16분음표)의 틱.
const SPP_TICKS: u64 = 240;
/// 명령 큐와 돌려받기 큐의 크기.
const QUEUE: usize = 256;
/// 한 바퀴에 처리하는 명령 수 상한. 명령이 쏟아져도 송신이 밀리지 않게 한다.
const COMMANDS_PER_WAKE: usize = 64;
/// 돌려받기 큐가 가득할 때 잠시 들고 있을 수 있는 스냅샷 수.
const PENDING_GARBAGE: usize = 4;
/// 재생 시작·위치 이동 뒤 첫 이벤트까지의 여유. 첫 이벤트가 늦게 예약되지 않게 한다.
const START_MARGIN: Duration = Duration::from_millis(2);

/// 스냅샷의 이벤트 하나: 틱, 포트 번호(스냅샷 `ports`의 인덱스), 메시지.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledEvent {
    tick: Tick,
    port: u16,
    msg: Midi1Message,
}

impl ScheduledEvent {
    #[must_use]
    pub const fn new(tick: Tick, port: u16, msg: Midi1Message) -> Self {
        ScheduledEvent { tick, port, msg }
    }

    #[must_use]
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub const fn msg(&self) -> Midi1Message {
        self.msg
    }
}

/// 같은 틱 안의 순서: Note Off(벨로시티 0인 Note On 포함), 그 밖, Note On.
fn order_class(msg: Midi1Message) -> u8 {
    let b = msg.bytes();
    match (b[0] & 0xF0, b.get(2).copied().unwrap_or(0)) {
        (0x80, _) | (0x90, 0) => 0,
        (0x90, _) => 2,
        _ => 1,
    }
}

/// 스케줄러가 재생하는 불변 묶음. 제어 스레드에서 만들어 `Arc`로 넘긴다.
pub struct Snapshot {
    tempo: TempoMap,
    events: Vec<ScheduledEvent>,
    ports: Vec<Arc<dyn MidiPort>>,
}

impl Snapshot {
    /// 이벤트를 정렬해 스냅샷을 만든다. 같은 틱에서는 Note Off, 프로그램·뱅크·CC 등, Note On 순서다.
    /// 같은 무리 안에서는 주어진 순서를 지킨다(뱅크 MSB 뒤 LSB 같은 순서가 의미 있다).
    ///
    /// # Errors
    ///
    /// 포트가 `MAX_SCHEDULER_PORTS`보다 많거나, 이벤트의 포트 번호가 `ports` 밖이면 `InvalidMessage`.
    pub fn new(
        tempo: TempoMap,
        ports: Vec<Arc<dyn MidiPort>>,
        mut events: Vec<ScheduledEvent>,
    ) -> Result<Self, MidiError> {
        if ports.len() > MAX_SCHEDULER_PORTS
            || events.iter().any(|e| usize::from(e.port) >= ports.len())
        {
            return Err(MidiError::InvalidMessage);
        }
        events.sort_by_key(|e| (e.tick, order_class(e.msg)));
        Ok(Snapshot {
            tempo,
            events,
            ports,
        })
    }

    #[must_use]
    pub fn events(&self) -> &[ScheduledEvent] {
        &self.events
    }

    /// `at` 이상인 첫 이벤트의 인덱스.
    fn first_at_or_after(&self, at: Tick) -> usize {
        self.events.partition_point(|e| e.tick < at)
    }

    /// `at`보다 큰 첫 이벤트의 인덱스.
    fn first_after(&self, at: Tick) -> usize {
        self.events.partition_point(|e| e.tick <= at)
    }
}

/// 스케줄러 설정.
#[derive(Debug, Clone, Copy)]
pub struct SchedulerConfig {
    /// 지금부터 이만큼 안에 오는 이벤트를 예약 송신한다. `wake`보다 길어야 한다.
    pub lookahead: Duration,
    /// 스케줄러가 깨는 주기.
    pub wake: Duration,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        SchedulerConfig {
            lookahead: Duration::from_millis(10),
            wake: Duration::from_millis(1),
        }
    }
}

enum Command {
    Play(Tick),
    Stop,
    Locate(Tick),
    Loop(Option<(Tick, Tick)>),
    Swap(Arc<Snapshot>),
    Panic,
    Clock(u16, bool),
    SendNow(u16, Midi1Message),
}

/// 스케줄러 스레드의 손잡이. drop하면 울리는 노트를 끄고 스레드를 끝낸다.
pub struct Scheduler {
    commands: rtrb::Producer<Command>,
    garbage: rtrb::Consumer<Arc<Snapshot>>,
    quit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Scheduler {
    /// 스케줄러 스레드를 시작한다. 처음에는 정지 상태다.
    ///
    /// # Errors
    ///
    /// `lookahead`가 `wake`보다 짧으면 `InvalidMessage`, 스레드를 만들지 못하면 `Os`.
    pub fn start(snapshot: Arc<Snapshot>, config: SchedulerConfig) -> Result<Self, MidiError> {
        if config.lookahead < config.wake || config.wake.is_zero() {
            return Err(MidiError::InvalidMessage);
        }
        let (commands, command_rx) = rtrb::RingBuffer::new(QUEUE);
        let (garbage_tx, garbage) = rtrb::RingBuffer::new(QUEUE);
        let quit = Arc::new(AtomicBool::new(false));
        let timebase = Timebase::get();
        // 상태 표는 여기(제어 스레드)에서 할당해 넘긴다. 스케줄러 스레드는 할당하지 않는다.
        let mut rt = Rt {
            snap: snapshot,
            commands: command_rx,
            garbage: garbage_tx,
            pending: Default::default(),
            quit: Arc::clone(&quit),
            timebase,
            lookahead: timebase.ticks(config.lookahead),
            wake: timebase.ticks(config.wake),
            start_margin: timebase.ticks(START_MARGIN),
            playing: false,
            stopped_at: Tick::ZERO,
            base_tick: Tick::ZERO,
            base_time: MusicTime::ZERO,
            base_host: 0,
            cursor: 0,
            looping: None,
            last_sent: 0,
            clock_mask: 0,
            next_clock: 0,
            active: vec![0; MAX_SCHEDULER_PORTS * 16 * 128].into_boxed_slice(),
            chase: vec![NONE; MAX_SCHEDULER_PORTS * 16 * CHASE_SLOTS].into_boxed_slice(),
        };
        let period = config.wake;
        let thread = std::thread::Builder::new()
            .name("keyscore-scheduler".into())
            .spawn(move || {
                // 우선순위를 못 올려도 동작은 같다. 지터만 커진다.
                let _ = make_realtime(rt.timebase, period);
                rt.run();
            })
            .map_err(|_| MidiError::Os {
                call: "thread spawn",
                status: -1,
            })?;
        Ok(Scheduler {
            commands,
            garbage,
            quit,
            thread: Some(thread),
        })
    }

    fn send(&mut self, command: Command) -> Result<(), MidiError> {
        push(&mut self.commands, command)
    }

    /// `from`부터 재생한다. 그 위치까지의 프로그램·뱅크·CC 값을 먼저 보낸다(chase).
    ///
    /// # Errors
    ///
    /// 명령 큐가 가득이면 `QueueFull`.
    pub fn play(&mut self, from: Tick) -> Result<(), MidiError> {
        self.send(Command::Play(from))
    }

    /// 정지한다. 울리는 노트를 끈다.
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn stop(&mut self) -> Result<(), MidiError> {
        self.send(Command::Stop)
    }

    /// 재생 위치를 옮긴다. 재생 중이면 그 위치에서 이어서 재생한다.
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn locate(&mut self, to: Tick) -> Result<(), MidiError> {
        self.send(Command::Locate(to))
    }

    /// 루프 범위 `[start, end)`. `None`이면 루프를 끈다.
    ///
    /// # Errors
    ///
    /// 범위가 비었으면 `InvalidMessage`, `QueueFull`.
    pub fn set_loop(&mut self, range: Option<(Tick, Tick)>) -> Result<(), MidiError> {
        if let Some((start, end)) = range
            && start >= end
        {
            return Err(MidiError::InvalidMessage);
        }
        self.send(Command::Loop(range))
    }

    /// 스냅샷을 원자적으로 바꾼다(TRN-3). 울리던 노트는 끈다.
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn swap(&mut self, snapshot: Arc<Snapshot>) -> Result<(), MidiError> {
        self.send(Command::Swap(snapshot))
    }

    /// 모든 포트·채널에 패닉 메시지를 보낸다(OUT-6).
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn panic(&mut self) -> Result<(), MidiError> {
        self.send(Command::Panic)
    }

    /// 포트의 MIDI Clock 송신을 켜거나 끈다(OUT-5, 기본 끔).
    ///
    /// # Errors
    ///
    /// 포트 번호가 `MAX_SCHEDULER_PORTS` 밖이면 `InvalidMessage`, `QueueFull`.
    pub fn set_clock(&mut self, port: u16, on: bool) -> Result<(), MidiError> {
        if usize::from(port) >= MAX_SCHEDULER_PORTS {
            return Err(MidiError::InvalidMessage);
        }
        self.send(Command::Clock(port, on))
    }

    /// 메시지를 지금 보낸다(미리듣기, NF-1 측정).
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn send_now(&mut self, port: u16, msg: Midi1Message) -> Result<(), MidiError> {
        self.send(Command::SendNow(port, msg))
    }

    /// 스케줄러가 돌려준, 더 쓰지 않는 스냅샷을 이 스레드에서 해제한다. 돌려받은 수를 돌려준다.
    pub fn collect_garbage(&mut self) -> usize {
        let mut count = 0;
        while let Ok(old) = self.garbage.pop() {
            drop(old);
            count += 1;
        }
        count
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.collect_garbage();
    }
}

/// chase 표에서 「값 없음」. 데이터 바이트는 0–127이라 겹치지 않는다.
const NONE: u8 = 0x80;
/// chase 표의 채널마다 칸: 0–119 CC(0과 32는 뱅크 MSB·LSB), 120번 칸은 프로그램.
const CHASE_SLOTS: usize = 121;
const PROGRAM_SLOT: usize = 120;

/// 스케줄러 스레드가 가진 상태. 표는 시작할 때 할당해 넘겨받는다.
struct Rt {
    snap: Arc<Snapshot>,
    commands: rtrb::Consumer<Command>,
    garbage: rtrb::Producer<Arc<Snapshot>>,
    pending: [Option<Arc<Snapshot>>; PENDING_GARBAGE],
    quit: Arc<AtomicBool>,
    timebase: Timebase,
    lookahead: u64,
    wake: u64,
    start_margin: u64,
    playing: bool,
    /// 정지 중의 위치(다음 재생의 기본값은 아니다. `play`는 위치를 받는다).
    stopped_at: Tick,
    /// 재생 기준점: 이 틱이 이 호스트 시각에 울린다.
    base_tick: Tick,
    base_time: MusicTime,
    base_host: u64,
    /// 다음에 보낼 이벤트 인덱스.
    cursor: usize,
    looping: Option<(Tick, Tick)>,
    /// 지금까지 예약한 가장 늦은 호스트 시각. 노트 끄기는 그 뒤로 예약해, 이미 넘긴 Note On보다
    /// 먼저 도착하지 않게 한다.
    last_sent: u64,
    clock_mask: u32,
    next_clock: u64,
    /// (포트, 채널, 음높이)마다 울리는 Note On 수.
    active: Box<[u8]>,
    /// (포트, 채널)마다 chase 값.
    chase: Box<[u8]>,
}

/// 다음에 처리할 것.
enum Next {
    Event(usize),
    Clock,
    LoopEnd(Tick),
}

impl Rt {
    fn run(&mut self) {
        loop {
            if self.quit.load(Ordering::Acquire) {
                let now = now();
                self.release_notes(now);
                return;
            }
            self.step();
            wait_until(now().saturating_add(self.wake));
        }
    }

    /// 한 바퀴. 디버그 빌드에서는 할당이 없음을 `assert_no_alloc`으로 검사한다(REQ 9.2).
    fn step(&mut self) {
        #[cfg(debug_assertions)]
        assert_no_alloc::assert_no_alloc(|| self.step_inner());
        #[cfg(not(debug_assertions))]
        self.step_inner();
    }

    fn step_inner(&mut self) {
        self.flush_garbage();
        for _ in 0..COMMANDS_PER_WAKE {
            // 교체 명령은 옛 스냅샷을 들고 있을 자리가 있을 때만 꺼낸다(스레드에서 해제하지 않으려고).
            if let Ok(Command::Swap(_)) = self.commands.peek()
                && !self.pending.iter().any(Option::is_none)
            {
                break;
            }
            let Ok(command) = self.commands.pop() else {
                break;
            };
            self.handle(command);
        }
        if self.playing {
            self.schedule();
        }
    }

    /// 기준점에서 `tick`까지의 호스트 시각.
    fn host_of(&self, tick: Tick) -> u64 {
        let ns = nanos_between(&self.snap.tempo, self.base_tick, tick);
        let ticks = nanos_to_host_ticks(ns, self.timebase.numer, self.timebase.denom);
        self.base_host.saturating_add(ticks)
    }

    /// 호스트 시각 `host`에 닿은 틱(내림).
    fn tick_at(&self, host: u64) -> Tick {
        let elapsed = host.saturating_sub(self.base_host);
        let ns =
            u128::from(elapsed) * u128::from(self.timebase.numer) / u128::from(self.timebase.denom);
        let time = MusicTime::from_units(self.base_time.units() + ns * MusicTime::UNITS_PER_NANO);
        self.snap.tempo.time_to_tick(time).max(self.base_tick)
    }

    fn set_base(&mut self, tick: Tick, host: u64) {
        self.base_tick = tick;
        self.base_time = self.snap.tempo.tick_to_time(tick);
        self.base_host = host;
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Play(from) => self.start_playing(from, true),
            Command::Stop => {
                if self.playing {
                    let at = self.release_time();
                    self.stopped_at = self.tick_at(at);
                    self.release_notes(at);
                    self.send_clock_message(at, 0xFC);
                    self.playing = false;
                }
            }
            Command::Locate(to) => {
                if self.playing {
                    let at = self.release_time();
                    self.release_notes(at);
                    self.start_playing(to, false);
                } else {
                    self.stopped_at = to;
                }
            }
            Command::Loop(range) => self.looping = range,
            Command::Swap(new) => self.swap(new),
            Command::Panic => self.panic(),
            Command::Clock(port, on) => {
                let bit = 1u32 << port;
                if on {
                    self.clock_mask |= bit;
                    if self.playing {
                        let now_tick = self.tick_at(now());
                        self.next_clock = now_tick.get().div_ceil(CLOCK_TICKS) * CLOCK_TICKS;
                    }
                } else {
                    self.clock_mask &= !bit;
                }
            }
            Command::SendNow(port, msg) => {
                let now = now();
                self.send_to(port, now, msg);
            }
        }
    }

    /// 노트 끄기를 예약할 시각: 이미 예약한 이벤트 뒤.
    fn release_time(&self) -> u64 {
        now().max(self.last_sent)
    }

    fn start_playing(&mut self, from: Tick, from_play: bool) {
        if self.playing && from_play {
            let at = self.release_time();
            self.release_notes(at);
        }
        let host = self.release_time().max(now() + self.start_margin);
        self.set_base(from, host);
        self.cursor = self.snap.first_at_or_after(from);
        self.chase(from, host);
        if self.clock_mask != 0 {
            if from == Tick::ZERO {
                self.send_clock_message(host, 0xFA);
            } else {
                let sixteenths =
                    u16::try_from((from.get() / SPP_TICKS).min(0x3FFF)).unwrap_or(0x3FFF);
                let spp = Midi1Message::from_status(
                    0xF2,
                    u8::try_from(sixteenths & 0x7F).unwrap_or(0),
                    u8::try_from(sixteenths >> 7).unwrap_or(0),
                );
                self.send_clock_raw(host, spp);
                self.send_clock_message(host, 0xFB);
            }
        }
        self.next_clock = from.get().div_ceil(CLOCK_TICKS) * CLOCK_TICKS;
        self.playing = true;
    }

    fn swap(&mut self, new: Arc<Snapshot>) {
        // `step_inner`는 빈 자리가 있을 때만 교체 명령을 꺼내므로 자리는 늘 있다.
        let Some(slot) = self.pending.iter().position(Option::is_none) else {
            return;
        };
        if self.playing {
            let now = now();
            let at = self.release_time();
            let now_tick = self.tick_at(now);
            // 이미 CoreMIDI에 넘긴 구간(last_sent까지)은 옛 스냅샷의 것이다. 새 스냅샷은 그 뒤부터 보낸다.
            let sent_tick = self.tick_at(self.last_sent.max(now));
            self.release_notes(at);
            let old = std::mem::replace(&mut self.snap, new);
            self.pending[slot] = Some(old);
            self.set_base(now_tick, now);
            self.cursor = self.snap.first_after(sent_tick);
            self.next_clock = sent_tick.get().div_ceil(CLOCK_TICKS) * CLOCK_TICKS;
        } else {
            let old = std::mem::replace(&mut self.snap, new);
            self.pending[slot] = Some(old);
        }
        self.flush_garbage();
    }

    fn flush_garbage(&mut self) {
        for slot in &mut self.pending {
            if let Some(old) = slot.take() {
                match self.garbage.push(old) {
                    Ok(()) => {}
                    Err(rtrb::PushError::Full(old)) => *slot = Some(old),
                }
            }
        }
    }

    fn schedule(&mut self) {
        let horizon = now().saturating_add(self.lookahead);
        loop {
            let event = self
                .snap
                .events
                .get(self.cursor)
                .map(|e| (e.tick, self.cursor));
            let clock = (self.clock_mask != 0).then_some(Tick::new(self.next_clock));
            let loop_end = self.looping.filter(|&(_, end)| end > self.base_tick);
            let mut next: Option<(Tick, Next)> = None;
            let mut consider = |tick: Tick, what: Next| {
                if next.as_ref().is_none_or(|(t, _)| tick < *t) {
                    next = Some((tick, what));
                }
            };
            if let Some((start, end)) = loop_end {
                consider(end, Next::LoopEnd(start));
            }
            if let Some(tick) = clock {
                consider(tick, Next::Clock);
            }
            // 같은 틱이면 이벤트가 먼저(클록·루프 끝보다).
            if let Some((tick, i)) = event
                && loop_end.is_none_or(|(_, end)| tick < end)
                && next.as_ref().is_none_or(|(t, _)| tick <= *t)
            {
                next = Some((tick, Next::Event(i)));
            }
            let Some((tick, what)) = next else {
                return;
            };
            let host = self.host_of(tick);
            if host > horizon {
                return;
            }
            match what {
                Next::Event(i) => {
                    let e = self.snap.events[i];
                    self.cursor = i + 1;
                    self.send_to(e.port, host, e.msg);
                }
                Next::Clock => {
                    self.send_clock_message(host, 0xF8);
                    self.next_clock += CLOCK_TICKS;
                }
                Next::LoopEnd(start) => {
                    self.release_notes(host);
                    self.set_base(start, host);
                    self.cursor = self.snap.first_at_or_after(start);
                    self.next_clock = start.get().div_ceil(CLOCK_TICKS) * CLOCK_TICKS;
                }
            }
        }
    }

    /// 포트로 보낸다. 실패는 무시한다(포트가 사라져도 스케줄러는 계속 돈다). 울리는 노트를 센다.
    fn send_to(&mut self, port: u16, host: u64, msg: Midi1Message) {
        let Some(target) = self.snap.ports.get(usize::from(port)) else {
            return;
        };
        let host = host.max(1);
        let _ = target.send(HostTime::from_ticks(host), &[msg.ump(Group::ZERO)]);
        self.last_sent = self.last_sent.max(host);
        let b = msg.bytes();
        if b.len() == 3 && b[0] & 0xF0 == 0x90 || b[0] & 0xF0 == 0x80 {
            let i = active_index(port, b[0] & 0x0F, b[1]);
            if let Some(count) = self.active.get_mut(i) {
                if b[0] & 0xF0 == 0x90 && b[2] > 0 {
                    *count = count.saturating_add(1);
                } else {
                    *count = count.saturating_sub(1);
                }
            }
        }
    }

    fn send_clock_message(&mut self, host: u64, status: u8) {
        self.send_clock_raw(host, Midi1Message::from_status(status, 0, 0));
    }

    fn send_clock_raw(&mut self, host: u64, msg: Midi1Message) {
        let mask = self.clock_mask;
        for port in 0..self.snap.ports.len() {
            if mask & (1u32 << port) != 0 {
                let port = u16::try_from(port).unwrap_or(u16::MAX);
                self.send_to(port, host, msg);
            }
        }
    }

    /// 울리는 노트를 모두 끈다. `host`에 예약한다.
    fn release_notes(&mut self, host: u64) {
        for port in 0..self.snap.ports.len() {
            for ch in 0..16u8 {
                for note in 0..128u8 {
                    let p = u16::try_from(port).unwrap_or(u16::MAX);
                    let i = active_index(p, ch, note);
                    if self.active.get(i).copied().unwrap_or(0) > 0 {
                        self.active[i] = 0;
                        self.send_to(p, host, Midi1Message::from_status(0x80 | ch, note, 0));
                    }
                }
            }
        }
        // 포트 수가 바뀐 스냅샷에서 남은 칸도 비운다(보낼 포트가 없다).
        self.active.fill(0);
    }

    /// `from` 직전까지의 프로그램·뱅크·CC 마지막 값을 `host`에 보낸다(CTL-6).
    fn chase(&mut self, from: Tick, host: u64) {
        self.chase.fill(NONE);
        for e in &self.snap.events[..self.snap.first_at_or_after(from)] {
            let b = e.msg.bytes();
            let ch = usize::from(b[0] & 0x0F);
            let base = (usize::from(e.port) * 16 + ch) * CHASE_SLOTS;
            match b[0] & 0xF0 {
                0xB0 if b.len() == 3 && usize::from(b[1]) < PROGRAM_SLOT => {
                    self.chase[base + usize::from(b[1])] = b[2];
                }
                0xC0 if b.len() == 2 => self.chase[base + PROGRAM_SLOT] = b[1],
                _ => {}
            }
        }
        for port in 0..self.snap.ports.len() {
            let p = u16::try_from(port).unwrap_or(u16::MAX);
            for ch in 0..16u8 {
                let base = (port * 16 + usize::from(ch)) * CHASE_SLOTS;
                // 뱅크 MSB·LSB, 프로그램, 나머지 CC 순서다(뱅크는 프로그램 체인지 앞에 와야 한다).
                for slot in [0usize, 32] {
                    let v = self.chase[base + slot];
                    if v != NONE {
                        let cc = u8::try_from(slot).unwrap_or(0);
                        self.send_to(p, host, Midi1Message::from_status(0xB0 | ch, cc, v));
                    }
                }
                let v = self.chase[base + PROGRAM_SLOT];
                if v != NONE {
                    self.send_to(p, host, Midi1Message::from_status(0xC0 | ch, v, 0));
                }
                for slot in 1..PROGRAM_SLOT {
                    if slot == 32 {
                        continue;
                    }
                    let v = self.chase[base + slot];
                    if v != NONE {
                        let cc = u8::try_from(slot).unwrap_or(0);
                        self.send_to(p, host, Midi1Message::from_status(0xB0 | ch, cc, v));
                    }
                }
            }
        }
    }

    fn panic(&mut self) {
        let now = now();
        for port in 0..self.snap.ports.len() {
            let p = u16::try_from(port).unwrap_or(u16::MAX);
            for ch in 0..16u8 {
                for cc in [120u8, 123, 64] {
                    self.send_to(p, now, Midi1Message::from_status(0xB0 | ch, cc, 0));
                }
            }
        }
        self.active.fill(0);
    }
}

fn now() -> u64 {
    HostTime::now().ticks()
}

fn active_index(port: u16, ch: u8, note: u8) -> usize {
    (usize::from(port) * 16 + usize::from(ch & 0x0F)) * 128 + usize::from(note & 0x7F)
}

/// `from`에서 `to`까지의 시각(ns, 내림). 정수 산술이라 템포 변화가 있어도 오차가 쌓이지 않는다.
pub(crate) fn nanos_between(tempo: &TempoMap, from: Tick, to: Tick) -> u128 {
    (tempo.tick_to_time(to).units() - tempo.tick_to_time(from).units()) / MusicTime::UNITS_PER_NANO
}

/// ns를 `mach_absolute_time` 틱으로 바꾼다(내림). `mach_timebase_info`는 `틱 × numer / denom = ns`다.
pub(crate) fn nanos_to_host_ticks(nanos: u128, numer: u32, denom: u32) -> u64 {
    let ticks = nanos * u128::from(denom) / u128::from(numer.max(1));
    u64::try_from(ticks).unwrap_or(u64::MAX)
}

/// 명령을 큐에 넣는다. 가득이면 `QueueFull`.
fn push<T>(queue: &mut rtrb::Producer<T>, item: T) -> Result<(), MidiError> {
    queue.push(item).map_err(|_| MidiError::QueueFull)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use keyscore_core::{Channel, Pitch, Tempo, TempoMap, Tick, Velocity};

    use super::{ScheduledEvent, Snapshot};
    use crate::{HostTime, MidiError, MidiPort, control_change, note_off, note_on};

    struct Null;

    impl MidiPort for Null {
        fn name(&self) -> &'static str {
            "null"
        }

        fn send(&self, _at: HostTime, _ump: &[u32]) -> Result<(), MidiError> {
            Ok(())
        }
    }

    fn ports() -> Vec<Arc<dyn MidiPort>> {
        vec![Arc::new(Null)]
    }

    #[test]
    fn snapshot_orders_note_off_before_note_on_at_same_tick() {
        let ch = Channel::new(1).unwrap();
        let p = Pitch::new(60).unwrap();
        let on = note_on(ch, p, Velocity::new(0x8000));
        let off = note_off(ch, p, Velocity::new(0));
        let cc = control_change(ch, 7, 1).unwrap();
        let events = vec![
            ScheduledEvent::new(Tick::new(480), 0, on),
            ScheduledEvent::new(Tick::new(480), 0, cc),
            ScheduledEvent::new(Tick::new(480), 0, off),
            ScheduledEvent::new(Tick::new(0), 0, on),
        ];
        let s = Snapshot::new(TempoMap::default(), ports(), events).unwrap();
        let order: Vec<(u64, u8)> = s
            .events()
            .iter()
            .map(|e| (e.tick().get(), e.msg().bytes()[0]))
            .collect();
        assert_eq!(
            order,
            vec![(0, 0x90), (480, 0x80), (480, 0xB0), (480, 0x90)]
        );
        // 포트 번호가 범위 밖이면 거부한다.
        let bad = vec![ScheduledEvent::new(Tick::ZERO, 1, on)];
        assert_eq!(
            Snapshot::new(TempoMap::default(), ports(), bad).err(),
            Some(MidiError::InvalidMessage)
        );
        let many: Vec<Arc<dyn MidiPort>> = (0..=super::MAX_SCHEDULER_PORTS)
            .map(|_| Arc::new(Null) as Arc<dyn MidiPort>)
            .collect();
        assert_eq!(
            Snapshot::new(TempoMap::default(), many, vec![]).err(),
            Some(MidiError::InvalidMessage)
        );
    }

    #[test]
    fn tick_to_host_is_integer_exact_across_tempo_changes() {
        // 120 BPM(500 ms/박)에서 960틱 뒤 60 BPM(1000 ms/박). 기준 960에서 2880까지는 정확히 2초다.
        let mut tempo = TempoMap::new(Tempo::from_bpm(120).unwrap());
        tempo.set(Tick::new(960), Tempo::from_bpm(60).unwrap());
        let ns = super::nanos_between(&tempo, Tick::new(960), Tick::new(2880));
        assert_eq!(ns, 2_000_000_000);
        assert_eq!(
            super::nanos_between(&tempo, Tick::ZERO, Tick::new(960)),
            500_000_000
        );
        // 호스트 틱 변환은 정수 비율이다(125/3이면 3 ns = 125/125... 왕복 오차 없음).
        let ticks = super::nanos_to_host_ticks(1_000_000_000, 125, 3);
        assert_eq!(ticks, 24_000_000);
        assert_eq!(
            super::nanos_to_host_ticks(1_000_000_000, 1, 1),
            1_000_000_000
        );
    }

    #[test]
    fn full_command_queue_returns_queue_full() {
        let (mut tx, _rx) = rtrb::RingBuffer::<u8>::new(1);
        assert_eq!(super::push(&mut tx, 1), Ok(()));
        assert_eq!(super::push(&mut tx, 2), Err(MidiError::QueueFull));
    }
}
