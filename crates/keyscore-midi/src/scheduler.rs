//! MIDI 스케줄러 스레드(REQ 9.2, 9.3, OUT-5, OUT-6, OUT-7, CTL-6, TRN-3).
//!
//! 제어 스레드가 만든 불변 스냅샷의 이벤트를 템포 맵으로 호스트 시각에 맞춰 몇 ms 앞서 예약 송신한다.
//! 정확한 송신 시각은 CoreMIDI 서버가 맞춘다(SP-3). 스케줄러 루프에는 할당·락·로그가 없다. 루프의
//! 시스템 콜은 MIDI 송신과 시각 대기(`mach_wait_until`)뿐이다.
//!
//! 제어 → 스케줄러 명령과, 스케줄러 → 제어로 돌려주는 스냅샷은 락 없는 SPSC 링버퍼(`rtrb`)로
//! 오간다. 교체된 스냅샷을 돌려받아 제어 스레드에서 해제하므로, 할당 해제도 스케줄러 스레드에서
//! 일어나지 않는다.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

use keyscore_core::{Channel, MeterMap, MusicTime, Pitch, TempoMap, Tick, Velocity};

use crate::metronome::{Beat, MetronomeConfig, clamp_length};
use crate::rt::{Timebase, make_realtime, wait_until};
use crate::{Group, HostTime, Midi1Message, MidiError, MidiPort, note_on};

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
/// 한 바퀴에 예약하는 항목 수 상한. 무엇이 잘못돼도 스레드가 명령을 계속 받게 한다.
const MAX_ITEMS_PER_PASS: usize = 4096;
/// 재생 시작·위치 이동 뒤 첫 이벤트까지의 여유. 첫 이벤트가 늦게 예약되지 않게 한다.
const START_MARGIN: Duration = Duration::from_millis(2);
/// 끄기 대기 표의 칸 수(미리듣기·클릭 Note Off). 가득 차면 가장 이른 대기를 앞당겨 보낸다.
const PENDING_OFFS: usize = 64;
/// 박 보고 큐(MET-2). UI가 비우지 않으면 새 박을 버린다.
const BEAT_QUEUE: usize = 256;

/// 노트 사건을 노트 칸별로 모은다. 정렬된 `events`에서 만들므로 칸 안도 틱 순이다.
/// 노트 칸별 노트 사건과 칸마다의 (시작, 개수).
type NoteIndex = (Vec<(Tick, bool)>, Vec<(u32, u32)>);

fn index_notes(events: &[ScheduledEvent], ports: usize) -> NoteIndex {
    let keys = ports * 16 * 128;
    let mut counts = vec![0u32; keys];
    let key_of = |e: &ScheduledEvent| {
        let b = e.msg.bytes();
        let kind = b[0] & 0xF0;
        (b.len() == 3 && (kind == 0x80 || kind == 0x90)).then(|| {
            (
                active_index(e.port, b[0] & 0x0F, b[1]),
                kind == 0x90 && b[2] > 0,
            )
        })
    };
    for e in events {
        if let Some((k, _)) = key_of(e) {
            counts[k] += 1;
        }
    }
    let mut ranges = Vec::with_capacity(keys);
    let mut next = 0u32;
    for &c in &counts {
        ranges.push((next, 0));
        next += c;
    }
    let mut out = vec![(Tick::ZERO, false); next as usize];
    for e in events {
        if let Some((k, on)) = key_of(e) {
            let (start, len) = &mut ranges[k];
            out[(*start + *len) as usize] = (e.tick, on);
            *len += 1;
        }
    }
    (out, ranges)
}

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
    meter: MeterMap,
    events: Vec<ScheduledEvent>,
    ports: Vec<Arc<dyn MidiPort>>,
    /// 노트(포트, 채널, 음높이)마다 노트 사건(틱, Note On인가)을 틱 순으로 모은 것과, 노트마다의
    /// (시작, 개수). 교체 때 「이 노트가 이어서 울려야 하나」를 할당 없이 찾는 데 쓴다.
    note_events: Vec<(Tick, bool)>,
    note_ranges: Vec<(u32, u32)>,
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
        let (note_events, note_ranges) = index_notes(&events, ports.len());
        Ok(Snapshot {
            tempo,
            meter: MeterMap::default(),
            events,
            ports,
            note_events,
            note_ranges,
        })
    }

    /// 노트 칸 `key`(`active_index`)에서 `t` 뒤 첫 노트 사건이 Note Off인가.
    fn next_note_is_off(&self, key: usize, t: Tick) -> bool {
        let Some(&(start, len)) = self.note_ranges.get(key) else {
            return false;
        };
        let (start, len) = (start as usize, len as usize);
        let Some(list) = self.note_events.get(start..start + len) else {
            return false;
        };
        let i = list.partition_point(|&(tick, _)| tick <= t);
        list.get(i).is_some_and(|&(_, on)| !on)
    }

    #[must_use]
    pub fn events(&self) -> &[ScheduledEvent] {
        &self.events
    }

    /// 박자표 맵을 둔다(메트로놈 박 격자, MET-1). 기본은 4/4다.
    #[must_use]
    pub fn with_meter(self, meter: MeterMap) -> Self {
        Snapshot { meter, ..self }
    }

    #[must_use]
    pub fn meter(&self) -> &MeterMap {
        &self.meter
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
    Clock(u16, bool),
    SendNow(u16, Midi1Message),
    Metronome(MetronomeConfig),
    MetronomeEnabled(bool),
    MetronomeForced(bool),
    Preview(u16, Midi1Message, Duration),
}

/// 제어 스레드와 스케줄러 스레드가 함께 보는 신호.
struct Signals {
    quit: AtomicBool,
    /// 패닉 요청. 명령 큐와 따로 두어, 큐가 막혀도 패닉은 닿게 한다(S1-T4 리뷰 1 지적 9).
    panic: AtomicBool,
    /// 패닉 요청 때까지 큐에 넣은 명령 수. 그 명령들을 먼저 처리한 뒤 패닉한다(리뷰 2 지적 4).
    panic_after: AtomicU64,
}

/// 스케줄러 스레드의 손잡이. drop하면 울리는 노트를 끄고 스레드를 끝낸다.
pub struct Scheduler {
    commands: rtrb::Producer<Command>,
    /// 큐에 넣은 명령 수.
    pushed: u64,
    garbage: rtrb::Consumer<Arc<Snapshot>>,
    beats: rtrb::Consumer<Beat>,
    /// 스레드가 끝날 때 들고 있던 스냅샷을 넘겨받는 길. 끝난 뒤에만 쓰므로 할당이 있어도 된다.
    leftovers: mpsc::Receiver<Arc<Snapshot>>,
    signals: Arc<Signals>,
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
        let (beat_tx, beats) = rtrb::RingBuffer::new(BEAT_QUEUE);
        let (leftover_tx, leftovers) = mpsc::channel();
        let signals = Arc::new(Signals {
            quit: AtomicBool::new(false),
            panic: AtomicBool::new(false),
            panic_after: AtomicU64::new(0),
        });
        let timebase = Timebase::get();
        // 상태 표는 여기(제어 스레드)에서 할당해 넘긴다. 스케줄러 스레드는 할당하지 않는다.
        let rt = Rt {
            snap: snapshot,
            commands: command_rx,
            garbage: garbage_tx,
            pending: Default::default(),
            leftovers: leftover_tx,
            signals: Arc::clone(&signals),
            timebase,
            lookahead: timebase.ticks(config.lookahead),
            wake: timebase.ticks(config.wake),
            start_margin: timebase.ticks(START_MARGIN),
            playing: false,
            base_tick: Tick::ZERO,
            base_time: MusicTime::ZERO,
            base_q: 0,
            sent_through: None,
            popped: 0,
            cursor: 0,
            looping: None,
            last_sent: 0,
            scheduled_until: 0,
            floor: 0,
            clock_mask: 0,
            next_clock: 0,
            active: vec![0; MAX_SCHEDULER_PORTS * 16 * 128].into_boxed_slice(),
            chase: vec![NONE; MAX_SCHEDULER_PORTS * 16 * CHASE_SLOTS].into_boxed_slice(),
            off_until: vec![0; MAX_SCHEDULER_PORTS * 16 * 128].into_boxed_slice(),
            metronome: MetronomeConfig::default(),
            click_length: timebase.ticks(MetronomeConfig::default().length()),
            metronome_enabled: false,
            metronome_forced: false,
            next_beat: u64::MAX,
            last_click: None,
            beats: beat_tx,
            offs: [None; PENDING_OFFS],
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
            pushed: 0,
            garbage,
            beats,
            leftovers,
            signals,
            thread: Some(thread),
        })
    }

    fn send(&mut self, command: Command) -> Result<(), MidiError> {
        // 스레드가 패닉 등으로 끝났으면 명령을 받아도 처리되지 않는다. 조용히 `Ok`를 주지 않는다.
        if self.thread.as_ref().is_none_or(JoinHandle::is_finished) {
            return Err(MidiError::Os {
                call: "scheduler thread",
                status: -1,
            });
        }
        push(&mut self.commands, command)?;
        self.pushed += 1;
        Ok(())
    }

    /// `from`부터 재생한다. 그 위치까지의 프로그램·뱅크·CC·프레셔·벤드 값을 먼저 보낸다(chase).
    /// 재생 중이면 울리는 노트를 끄고 그 위치에서 다시 시작한다.
    ///
    /// # Errors
    ///
    /// 명령 큐가 가득이면 `QueueFull`, 스레드가 끝났으면 `Os`.
    pub fn play(&mut self, from: Tick) -> Result<(), MidiError> {
        self.send(Command::Play(from))
    }

    /// 정지한다. 울리는 노트(미리듣기로 켠 노트 포함)를 끈다.
    ///
    /// # Errors
    ///
    /// `QueueFull`, `Os`.
    pub fn stop(&mut self) -> Result<(), MidiError> {
        self.send(Command::Stop)
    }

    /// 재생 위치를 옮긴다. 재생 중이면 그 위치에서 이어서 재생한다.
    ///
    /// # Errors
    ///
    /// `QueueFull`, `Os`.
    pub fn locate(&mut self, to: Tick) -> Result<(), MidiError> {
        self.send(Command::Locate(to))
    }

    /// 루프 범위 `[start, end)`. `None`이면 루프를 끈다. 재생 위치가 이미 `end` 뒤면 다음 재생·위치
    /// 이동까지 되감지 않는다(기준점이 `end` 앞일 때만 되감는다).
    ///
    /// # Errors
    ///
    /// 범위가 비었으면 `InvalidMessage`, `QueueFull`, `Os`.
    pub fn set_loop(&mut self, range: Option<(Tick, Tick)>) -> Result<(), MidiError> {
        if let Some((start, end)) = range
            && start >= end
        {
            return Err(MidiError::InvalidMessage);
        }
        self.send(Command::Loop(range))
    }

    /// 스냅샷을 원자적으로 바꾼다(TRN-3). 새 스냅샷에서도 이어서 울려야 하는 노트는 그대로 두고, 나머지
    /// 울리는 노트는 끈다.
    ///
    /// 옛 스냅샷은 돌려받기 큐로 돌아온다. 호출자는 `collect_garbage`를 주기적으로(교체 뒤 한 번 이상)
    /// 불러야 한다. 부르지 않아 돌려받기 큐(256)와 대기 자리(4)가 차면, 그 뒤의 교체와 그 뒤에 넣은
    /// 명령(정지·위치 이동 등)은 큐에서 기다린다. 패닉은 큐와 따로 전달되어 이때도 닿는다(리뷰 1 지적 9).
    ///
    /// # Errors
    ///
    /// `QueueFull`, `Os`.
    pub fn swap(&mut self, snapshot: Arc<Snapshot>) -> Result<(), MidiError> {
        self.send(Command::Swap(snapshot))
    }

    /// 모든 포트·채널에 패닉 메시지를 보낸다(OUT-6). 명령 큐와 따로 전달되어, 큐가 가득해도 닿는다.
    ///
    /// # Errors
    ///
    /// 스레드가 끝났으면 `Os`.
    pub fn panic(&mut self) -> Result<(), MidiError> {
        if self.thread.as_ref().is_none_or(JoinHandle::is_finished) {
            return Err(MidiError::Os {
                call: "scheduler thread",
                status: -1,
            });
        }
        self.signals
            .panic_after
            .store(self.pushed, Ordering::Release);
        self.signals.panic.store(true, Ordering::Release);
        Ok(())
    }

    /// 포트의 MIDI Clock 송신을 켜거나 끈다(OUT-5, 기본 끔).
    ///
    /// # Errors
    ///
    /// 포트 번호가 `MAX_SCHEDULER_PORTS` 밖이면 `InvalidMessage`, `QueueFull`, `Os`.
    pub fn set_clock(&mut self, port: u16, on: bool) -> Result<(), MidiError> {
        if usize::from(port) >= MAX_SCHEDULER_PORTS {
            return Err(MidiError::InvalidMessage);
        }
        self.send(Command::Clock(port, on))
    }

    /// 메시지를 지금 보낸다(미리듣기, NF-1 측정). Note On은 울리는 노트로 세어 `stop`이 끈다.
    ///
    /// # Errors
    ///
    /// `QueueFull`, `Os`.
    pub fn send_now(&mut self, port: u16, msg: Midi1Message) -> Result<(), MidiError> {
        self.send(Command::SendNow(port, msg))
    }

    /// 메트로놈 설정을 바꾼다(MET-1, MET-3).
    ///
    /// # Errors
    ///
    /// 포트 번호가 `MAX_SCHEDULER_PORTS` 밖이면 `InvalidMessage`, `QueueFull`, `Os`.
    pub fn set_metronome(&mut self, config: MetronomeConfig) -> Result<(), MidiError> {
        if usize::from(config.port()) >= MAX_SCHEDULER_PORTS {
            return Err(MidiError::InvalidMessage);
        }
        self.send(Command::Metronome(config))
    }

    /// 트랜스포트의 메트로놈 버튼(기본 끔).
    ///
    /// # Errors
    ///
    /// `QueueFull`, `Os`.
    pub fn set_metronome_enabled(&mut self, on: bool) -> Result<(), MidiError> {
        self.send(Command::MetronomeEnabled(on))
    }

    /// 버튼과 무관하게 클릭을 켠다(Sequence 녹음, 카운트인, MET-1).
    ///
    /// # Errors
    ///
    /// `QueueFull`, `Os`.
    pub fn set_metronome_forced(&mut self, on: bool) -> Result<(), MidiError> {
        self.send(Command::MetronomeForced(on))
    }

    /// 재생 중 지난 박 하나를 꺼낸다(MET-2). UI가 주기적으로 불러 비운다. 비우지 않아 큐가 차면
    /// 스케줄러는 새 박을 버린다.
    pub fn poll_beat(&mut self) -> Option<Beat> {
        self.beats.pop().ok()
    }

    /// 노트를 지금 켜고 `length` 뒤에 끈다(PRE-1, 기본 [`DEFAULT_PREVIEW_LENGTH`]). 같은 노트가
    /// 아직 울리면 먼저 끄고 다시 켠다. 길이는 [1 ms, `MAX_NOTE_LENGTH`]로 맞춘다.
    ///
    /// [`DEFAULT_PREVIEW_LENGTH`]: crate::DEFAULT_PREVIEW_LENGTH
    ///
    /// # Errors
    ///
    /// 포트 번호가 `MAX_SCHEDULER_PORTS` 밖이면 `InvalidMessage`, `QueueFull`, `Os`.
    pub fn preview(
        &mut self,
        port: u16,
        channel: Channel,
        pitch: Pitch,
        velocity: Velocity,
        length: Duration,
    ) -> Result<(), MidiError> {
        if usize::from(port) >= MAX_SCHEDULER_PORTS {
            return Err(MidiError::InvalidMessage);
        }
        let on = note_on(channel, pitch, velocity);
        self.send(Command::Preview(port, on, clamp_length(length)))
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
        self.signals.quit.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.collect_garbage();
        // 스레드가 끝날 때 넘긴 스냅샷(포트 포함)을 여기, 제어 스레드에서 해제한다(리뷰 1 지적 12).
        while let Ok(snap) = self.leftovers.try_recv() {
            drop(snap);
        }
    }
}

/// chase 표에서 「값 없음」. 데이터 바이트는 0–127이라 겹치지 않는다.
const NONE: u8 = 0x80;
/// chase 표의 채널마다 칸: 0–119 CC(0과 32는 뱅크 MSB·LSB), 프로그램, 채널 프레셔, 벤드 LSB·MSB.
const PROGRAM_SLOT: usize = 120;
const PRESSURE_SLOT: usize = 121;
const BEND_LSB_SLOT: usize = 122;
const BEND_MSB_SLOT: usize = 123;
/// 마지막으로 고른 파라미터 종류: `RPN` 또는 `NRPN`(값 없음이면 `NONE`).
const SELECT_KIND_SLOT: usize = 124;
const CHASE_SLOTS: usize = 125;
const RPN: u8 = 0;
const NRPN: u8 = 1;
/// RPN·NRPN 선택 CC. 데이터 엔트리보다 먼저 보내야 값이 맞는 파라미터에 들어간다(리뷰 1 지적 10).
/// 데이터 엔트리는 마지막으로 고른 파라미터에 들어가므로, 마지막에 고른 종류를 나중에 보낸다
/// (리뷰 2 지적 7).
const RPN_SELECT: [usize; 2] = [101, 100];
const NRPN_SELECT: [usize; 2] = [99, 98];
const PARAMETER_SELECT: [usize; 4] = [99, 98, 101, 100];
/// 데이터 엔트리 MSB·LSB.
const DATA_ENTRY: [usize; 2] = [6, 38];
/// 상대값이라 chase로 다시 보내면 두 번 적용되는 CC(데이터 증가·감소).
const RELATIVE: [usize; 2] = [96, 97];

/// 스케줄러 스레드가 가진 상태. 표는 시작할 때 할당해 넘겨받는다.
struct Rt {
    snap: Arc<Snapshot>,
    commands: rtrb::Consumer<Command>,
    garbage: rtrb::Producer<Arc<Snapshot>>,
    pending: [Option<Arc<Snapshot>>; PENDING_GARBAGE],
    leftovers: mpsc::Sender<Arc<Snapshot>>,
    signals: Arc<Signals>,
    timebase: Timebase,
    lookahead: u64,
    wake: u64,
    start_margin: u64,
    playing: bool,
    /// 재생 기준점: 이 틱이 이 호스트 시각에 울린다.
    base_tick: Tick,
    base_time: MusicTime,
    /// 기준 호스트 시각 × `scale`(= numer × 960). 교체마다 기준점을 옮겨도 내림 오차가 쌓이지 않게
    /// 정확한 값으로 든다(리뷰 2 지적 8).
    base_q: u128,
    /// 지금 기준점에서 보낸 마지막 틱. `None`이면 기준점 이후로 아직 아무것도 보내지 않았다
    /// (재생 시작·위치 이동·루프 되감기 직후, 리뷰 2 지적 1·2).
    sent_through: Option<Tick>,
    /// 처리한(꺼낸) 명령 수. 패닉 순서를 맞추는 데 쓴다(리뷰 2 지적 4).
    popped: u64,
    /// 다음에 보낼 이벤트 인덱스.
    cursor: usize,
    looping: Option<(Tick, Tick)>,
    /// 지금까지 예약한 가장 늦은 호스트 시각.
    last_sent: u64,
    /// 이 호스트 시각까지의 항목은 모두 예약했다(마지막 `schedule`의 창 끝).
    scheduled_until: u64,
    /// 교체·위치 이동 뒤의 이벤트는 이 시각 앞으로 예약하지 않는다. 이미 넘긴 옛 이벤트보다 먼저
    /// 도착하지 않게 하려는 것이다.
    floor: u64,
    clock_mask: u32,
    next_clock: u64,
    /// (포트, 채널, 음높이)마다 울리는 Note On 수.
    active: Box<[u8]>,
    /// (포트, 채널)마다 chase 값.
    chase: Box<[u8]>,
    /// (포트, 채널, 음높이)마다 지금까지 넘긴 가장 늦은 Note Off 시각. 넘긴 예약은 취소할 수 없으므로
    /// 미리듣기·클릭의 Note On을 이 뒤로 미뤄, 앞서 넘긴 Note Off에 끊기지 않게 한다(S1-T5 리뷰 2).
    off_until: Box<[u64]>,
    metronome: MetronomeConfig,
    /// 클릭 길이(호스트 틱). 설정이 바뀔 때 한 번 바꾼다.
    click_length: u64,
    metronome_enabled: bool,
    metronome_forced: bool,
    /// 다음 박 틱. `u64::MAX`는 「더 없음」이다. 박은 클릭이 꺼져 있어도 센다(박 보고, MET-2).
    next_beat: u64,
    /// 지금 기준점에서 마지막으로 처리한 박 틱. 교체 때 새 박자표로 다음 박을 다시 찾는 기준이다.
    last_click: Option<u64>,
    beats: rtrb::Producer<Beat>,
    /// 시각이 오면 보낼 Note Off(미리듣기·클릭). 스택 배열이라 할당이 없다.
    offs: [Option<PendingOff>; PENDING_OFFS],
}

/// 시각 `host`에 보낼 Note Off. `sent`면 이미 `CoreMIDI`에 넘겼고(취소할 수 없다) 시각이 지나면 칸을 비운다.
#[derive(Clone, Copy)]
struct PendingOff {
    host: u64,
    /// 짝인 Note On의 시각. 끄기를 앞당겨도 이보다 앞에 두지 않는다(S1-T5 리뷰 1 지적 1).
    on_host: u64,
    port: u16,
    /// 상태 바이트(0x80 | 채널)와 노트.
    status: u8,
    note: u8,
    sent: bool,
}

/// 다음에 처리할 것.
#[derive(Clone, Copy)]
enum Next {
    Event(usize),
    Click,
    Clock,
    LoopEnd(Tick),
}

impl Rt {
    fn run(mut self) {
        while !self.signals.quit.load(Ordering::Acquire) {
            self.step();
            wait_until(now().saturating_add(self.wake));
        }
        // 끝: 걸린 노트를 끄고(이미 넘긴 Note On 뒤로) 클록 장치를 세운다(리뷰 1 지적 1).
        let at = self.release_time();
        self.release_notes(at);
        if self.playing {
            self.send_clock_message(at, 0xFC);
        }
        // 스냅샷과 포트는 제어 스레드에서 해제되도록 넘긴다(리뷰 1 지적 12). 루프는 끝났으므로 여기서의
        // 할당은 실시간 규율과 상관없다.
        let Rt {
            snap,
            mut commands,
            pending,
            leftovers,
            ..
        } = self;
        let _ = leftovers.send(snap);
        for old in pending.into_iter().flatten() {
            let _ = leftovers.send(old);
        }
        while let Ok(command) = commands.pop() {
            if let Command::Swap(new) = command {
                let _ = leftovers.send(new);
            }
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
        let mut blocked = false;
        for _ in 0..COMMANDS_PER_WAKE {
            // 패닉은 그 앞에 넣은 명령까지 처리한 바로 그 자리에서 한다. 뒤에 넣은 명령(패닉 직후의
            // 미리듣기 등)은 패닉 뒤에 처리한다(리뷰 2 지적 4, 리뷰 3 지적 2).
            self.panic_if_due(false);
            // 교체 명령은 옛 스냅샷을 들고 있을 자리가 있을 때만 꺼낸다(스레드에서 해제하지 않으려고).
            if let Ok(Command::Swap(_)) = self.commands.peek()
                && !self.pending.iter().any(Option::is_none)
            {
                blocked = true;
                break;
            }
            let Ok(command) = self.commands.pop() else {
                break;
            };
            self.popped += 1;
            self.handle(command);
        }
        // 큐 머리가 교체로 막혔으면 앞 명령을 기다리지 않는다.
        self.panic_if_due(blocked);
        if self.playing {
            self.schedule();
        }
        self.flush_offs();
    }

    fn panic_if_due(&mut self, blocked: bool) {
        if self.signals.panic.load(Ordering::Acquire)
            && (blocked || self.popped >= self.signals.panic_after.load(Ordering::Acquire))
        {
            self.signals.panic.store(false, Ordering::Release);
            self.panic();
        }
    }

    /// 기준 호스트 시각을 정확히 나타내는 배율(`base_q` 단위).
    fn scale(&self) -> u128 {
        u128::from(self.timebase.numer.max(1)) * MusicTime::UNITS_PER_NANO
    }

    /// 기준점(내림한 호스트 시각).
    fn base_host(&self) -> u64 {
        u64::try_from(self.base_q / self.scale()).unwrap_or(u64::MAX)
    }

    /// 기준점에서 `tick`까지의 호스트 시각.
    fn host_of(&self, tick: Tick) -> u64 {
        let units = self
            .snap
            .tempo
            .tick_to_time(tick)
            .units()
            .saturating_sub(self.base_time.units());
        let q = self
            .base_q
            .saturating_add(units.saturating_mul(u128::from(self.timebase.denom)));
        u64::try_from(q / self.scale()).unwrap_or(u64::MAX)
    }

    /// 호스트 시각 `host`에 닿은 틱(내림).
    fn tick_at(&self, host: u64) -> Tick {
        let q = u128::from(host).saturating_mul(self.scale());
        let units = q.saturating_sub(self.base_q) / u128::from(self.timebase.denom.max(1));
        let time = MusicTime::from_units(self.base_time.units().saturating_add(units));
        self.snap.tempo.time_to_tick(time).max(self.base_tick)
    }

    /// 기준점을 정수 호스트 시각에 새로 잡는다(재생 시작, 위치 이동, 루프 되감기).
    fn set_base(&mut self, tick: Tick, host: u64) {
        self.base_tick = tick;
        self.base_time = self.snap.tempo.tick_to_time(tick);
        self.base_q = u128::from(host).saturating_mul(self.scale());
        self.sent_through = None;
        self.last_click = None;
    }

    /// 기준점을 같은 계산 위의 틱 `t`로 옮긴다. 내림 없이 정확하다(교체).
    fn move_base(&mut self, t: Tick) {
        let units = self
            .snap
            .tempo
            .tick_to_time(t)
            .units()
            .saturating_sub(self.base_time.units());
        self.base_q = self
            .base_q
            .saturating_add(units.saturating_mul(u128::from(self.timebase.denom)));
        self.base_tick = t;
        self.base_time = self.snap.tempo.tick_to_time(t);
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Play(from) => {
                let was_playing = self.playing;
                if was_playing {
                    let at = self.release_time();
                    self.release_notes(at);
                }
                self.start_playing(from, was_playing);
            }
            Command::Stop => {
                let at = self.release_time();
                // 정지 중이어도 미리듣기로 켠 노트를 끈다(리뷰 1 제안 18).
                self.release_notes(at);
                if self.playing {
                    self.send_clock_message(at, 0xFC);
                    self.playing = false;
                }
            }
            Command::Locate(to) => {
                if self.playing {
                    let at = self.release_time();
                    self.release_notes(at);
                    self.start_playing(to, true);
                }
            }
            Command::Loop(range) => self.looping = range,
            Command::Swap(new) => self.swap(new),
            Command::Clock(port, on) => {
                let bit = 1u32 << port;
                if on {
                    self.clock_mask |= bit;
                    if self.playing {
                        self.next_clock = match self.sent_through {
                            Some(t) => clock_after(t.get().saturating_add(1)),
                            None => clock_after(self.base_tick.get()),
                        };
                    }
                } else {
                    self.clock_mask &= !bit;
                }
            }
            Command::SendNow(port, msg) => {
                let now = now();
                self.send_to(port, now, msg);
            }
            Command::Metronome(config) => {
                self.metronome = config;
                self.click_length = self.timebase.ticks(config.length());
            }
            Command::MetronomeEnabled(on) => self.metronome_enabled = on,
            Command::MetronomeForced(on) => self.metronome_forced = on,
            Command::Preview(port, on, length) => {
                let length = self.timebase.ticks(length);
                self.play_note(port, on, now(), length);
            }
        }
    }

    /// 노트 끄기를 예약할 시각: 이미 예약한 이벤트 뒤.
    fn release_time(&self) -> u64 {
        now().max(self.last_sent)
    }

    /// `from`에서 재생을 시작한다. `was_playing`이면 클록 장치에 Stop을 먼저 보낸다(재생 중 SPP는
    /// 무시될 수 있다, 리뷰 1 지적 7).
    fn start_playing(&mut self, from: Tick, was_playing: bool) {
        let host = self
            .release_time()
            .max(self.scheduled_until)
            .max(now().saturating_add(self.start_margin));
        self.set_base(from, host);
        self.floor = host;
        self.cursor = self.snap.first_at_or_after(from);
        self.chase(from, host);
        self.next_clock = self.resync_clock(from, host, was_playing);
        self.next_beat = beat_at_or_after(&self.snap.meter, from);
        self.playing = true;
    }

    /// 클록 장치를 `from`에 맞춘다: 0이면 Start, 아니면 (재생 중이었으면 Stop 뒤) SPP와 Continue.
    /// 다음 Clock 틱을 돌려준다. SPP는 16분음표 단위라, 맞지 않는 위치는 다음 16분음표로 올리고 첫
    /// Clock을 거기에 둔다(리뷰 1 지적 6).
    fn resync_clock(&mut self, from: Tick, host: u64, was_playing: bool) -> u64 {
        if self.clock_mask == 0 {
            return clock_after(from.get());
        }
        if was_playing {
            self.send_clock_message(host, 0xFC);
        }
        if from == Tick::ZERO {
            self.send_clock_message(host, 0xFA);
            return 0;
        }
        let sixteenths = from.get().div_ceil(SPP_TICKS).min(0x3FFF);
        let spp_value = u16::try_from(sixteenths).unwrap_or(0x3FFF);
        let spp = Midi1Message::from_status(
            0xF2,
            u8::try_from(spp_value & 0x7F).unwrap_or(0),
            u8::try_from(spp_value >> 7).unwrap_or(0),
        );
        self.send_clock_raw(host, spp);
        self.send_clock_message(host, 0xFB);
        // SPP 상한(16383)에 걸린 아주 늦은 위치는 첫 Clock을 `from` 뒤 첫 칸에 둔다.
        clock_after(sixteenths.saturating_mul(SPP_TICKS).max(from.get()))
    }

    fn swap(&mut self, new: Arc<Snapshot>) {
        // `step_inner`는 빈 자리가 있을 때만 교체 명령을 꺼내므로 자리는 늘 있다.
        let Some(slot) = self.pending.iter().position(Option::is_none) else {
            return;
        };
        if !self.playing {
            // 정지 중에도 미리듣기 노트가 울릴 수 있다. 포트가 바뀌거나 없어진 번호의 노트는 옛 포트로
            // 끈다. 같은 포트의 노트는 뒤따를 Note Off가 그대로 닿으므로 둔다(리뷰 3 지적 1).
            let at = self.release_time();
            self.release_changed_ports(&new, at);
            self.forget_offs_on_changed_ports(&new);
            let old = std::mem::replace(&mut self.snap, new);
            self.pending[slot] = Some(old);
            self.flush_garbage();
            return;
        }
        let at = self.release_time();
        // 보낸 구간: 지금 기준점에서 보낸 마지막 틱 T까지. 아무것도 보내지 않았으면 기준점부터 새로 보낸다.
        // 같은 바퀴에 교체가 거듭돼도 T는 그대로라 보내지 않은 틱을 보낸 것으로 치지 않는다(리뷰 2
        // 지적 1·2). 기준점은 옛 계산 위에서 정확히 옮겨 오차가 쌓이지 않는다(리뷰 1 지적 4, 리뷰 2
        // 지적 8).
        let sent = self.sent_through;
        let t = sent.unwrap_or(self.base_tick);
        // 박은 마지막으로 처리한 박 뒤의 첫 박부터 새 박자표로 다시 찾는다. 이미 지난 시각의 박이면
        // `floor`에 늦게 나간다(빠뜨리지 않는다).
        let click_from = self
            .last_click
            .map_or(self.base_tick, |c| Tick::new(c.saturating_add(1)));
        if sent.is_some() {
            self.move_base(t);
        }
        // 새 스냅샷에서 T 뒤 첫 노트 사건이 Note Off이고 같은 포트면 이어서 울린다. 나머지는 끈다.
        self.release_unless_continued(&new, t, at);
        self.forget_offs_on_changed_ports(&new);
        let old = std::mem::replace(&mut self.snap, new);
        self.pending[slot] = Some(old);
        self.base_time = self.snap.tempo.tick_to_time(self.base_tick);
        // 새 이벤트를 이미 예약한 시각 앞에 두지 않는다. 늦은 바퀴면 버리지 않고 지금 보낸다.
        self.floor = self.floor.max(self.scheduled_until);
        // `next_clock`은 스냅샷과 무관한 절대 틱이고 보낸 구간 바로 뒤를 가리키므로 그대로 둔다.
        self.cursor = if sent.is_some() {
            self.snap.first_after(t)
        } else {
            self.snap.first_at_or_after(t)
        };
        self.next_beat = beat_at_or_after(&self.snap.meter, click_from);
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

    /// 다음에 보낼 항목(이벤트, Clock, 루프 끝 가운데 가장 이른 것).
    fn peek_next(&self) -> Option<(Tick, Next)> {
        let event = self
            .snap
            .events
            .get(self.cursor)
            .map(|e| (e.tick, self.cursor));
        // `u64::MAX`는 「더 보낼 Clock 없음」이다(넘침 대신, 리뷰 2 지적 5).
        let clock = (self.clock_mask != 0 && self.next_clock != u64::MAX)
            .then_some(Tick::new(self.next_clock));
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
        // 같은 틱이면 클릭이 Clock보다 먼저, 이벤트보다 뒤다. 루프 끝과 같은 틱의 박은 다음 바퀴의 것이다.
        if self.next_beat != u64::MAX && loop_end.is_none_or(|(_, end)| self.next_beat < end.get())
        {
            let tick = Tick::new(self.next_beat);
            if next.as_ref().is_none_or(|(t, _)| tick <= *t) {
                next = Some((tick, Next::Click));
            }
        }
        // 같은 틱이면 이벤트가 먼저(클록·루프 끝보다). 루프 끝과 같은 틱의 이벤트는 다음 바퀴의 것이다.
        if let Some((tick, i)) = event
            && loop_end.is_none_or(|(_, end)| tick < end)
            && next.as_ref().is_none_or(|(t, _)| tick <= *t)
        {
            next = Some((tick, Next::Event(i)));
        }
        next
    }

    fn schedule(&mut self) {
        let horizon = now().saturating_add(self.lookahead);
        for _ in 0..MAX_ITEMS_PER_PASS {
            let Some((tick, what)) = self.peek_next() else {
                break;
            };
            let host = self.host_of(tick).max(self.floor);
            if host > horizon {
                break;
            }
            match what {
                Next::Event(i) => {
                    let e = self.snap.events[i];
                    self.cursor = i + 1;
                    self.send_to(e.port, host, e.msg);
                }
                Next::Click => self.click(tick, host),
                Next::Clock => {
                    self.send_clock_message(host, 0xF8);
                    self.next_clock = self.next_clock.saturating_add(CLOCK_TICKS);
                }
                Next::LoopEnd(start) => {
                    self.release_notes(host);
                    // 루프 한 바퀴가 호스트 틱 하나보다 짧으면 같은 시각을 끝없이 되감는다. 적어도 한 틱은
                    // 나아가게 한다(리뷰 2 지적 6).
                    let host = host.max(self.base_host().saturating_add(1));
                    self.set_base(start, host);
                    self.cursor = self.snap.first_at_or_after(start);
                    // 클록 장치도 루프 시작으로 다시 맞춘다(리뷰 1 지적 7).
                    self.next_clock = self.resync_clock(start, host, true);
                    self.next_beat = beat_at_or_after(&self.snap.meter, start);
                }
            }
        }
        self.scheduled_until = self.scheduled_until.max(horizon);
        self.sent_through = self.sent_through_now(horizon);
    }

    /// 보낸 구간의 끝 틱. 기준점 뒤로 아무것도 보내지 않았으면 `None`.
    ///
    /// 송신 판정과 같은 내림(`host_of(tick) <= horizon`)으로 구해야 교체 때 같은 틱을 두 번 보내지
    /// 않는다. 항목 상한에 걸려 일찍 끝난 바퀴면 아직 보내지 않은 다음 항목 앞까지로 줄인다(리뷰 3
    /// 지적 3).
    fn sent_through_now(&self, horizon: u64) -> Option<Tick> {
        if horizon < self.base_host() {
            return None;
        }
        let mut through = self.tick_at(horizon.saturating_add(1));
        if through > self.base_tick && self.host_of(through) > horizon {
            through = Tick::new(through.get() - 1);
        }
        if let Some((tick, what)) = self.peek_next() {
            let pending = match what {
                // 같은 틱의 이벤트는 다 보냈다(이벤트가 먼저). 그 틱의 클릭·Clock은 `next_beat`·
                // `next_clock`이 들고 있다.
                Next::Click | Next::Clock => Some(tick),
                // 그 틱의 이벤트는 아직이다. 상한에 걸려 같은 틱 일부를 보냈다면 교체 때 그 틱을 다시
                // 보낸다(빠뜨리기보다 겹치기). 루프 끝 틱의 이벤트는 다음 바퀴의 것이다.
                Next::Event(_) | Next::LoopEnd(_) => {
                    (tick > self.base_tick).then(|| Tick::new(tick.get() - 1))
                }
            };
            through = through.min(pending?);
        }
        // 기준점 틱의 이벤트를 아직 보내지 않았을 수 있다. 보냈는지는 위에서 판단했다.
        (through >= self.base_tick).then_some(through)
    }

    /// 박 `tick`(호스트 시각 `host`)을 처리한다. 박을 보고하고, 클릭이 켜져 있으면 노트를 보낸다.
    fn click(&mut self, tick: Tick, host: u64) {
        let pos = self.snap.meter.position(tick);
        // UI가 비우지 않아 큐가 가득이면 버린다. 시각 표시는 하나 빠져도 다음 박에서 맞는다.
        let _ = self.beats.push(Beat {
            bar: pos.bar(),
            index: pos.beat(),
            at: HostTime::from_ticks(host),
        });
        if self.metronome_enabled || self.metronome_forced {
            let m = self.metronome;
            let (pitch, velocity) = if pos.beat() == 0 {
                (m.accent(), m.accent_velocity())
            } else {
                (m.beat(), m.beat_velocity())
            };
            let on = note_on(m.channel(), pitch, velocity);
            self.play_note(m.port(), on, host, self.click_length);
        }
        self.last_click = Some(tick.get());
        self.next_beat =
            beat_at_or_after(&self.snap.meter, Tick::new(tick.get().saturating_add(1)));
    }

    /// Note On `on`을 `host`에 보내고 `length` 뒤의 Note Off를 끄기 대기 표에 둔다(미리듣기, 클릭).
    ///
    /// 같은 노트의 끄기가 기다리고 있으면 그 노트를 먼저 끈다(짝 Note On 시각 뒤에). 이미 `CoreMIDI`에
    /// 넘긴 Note Off는 취소할 수 없으므로, 어느 경로로 넘겼든(`off_until`) 그 시각이 뒤면 새 Note On을
    /// 그 뒤로 미룬다. 그래야 새 노트가 앞 노트의 Note Off로 끊기지 않는다. 넘긴 시각은 모두 lookahead
    /// 창 안이라 미루는 폭은 lookahead(기본 10 ms)를 넘지 않는다.
    fn play_note(&mut self, port: u16, on: Midi1Message, host: u64, length: u64) {
        let [status, note, _] = on.bytes() else {
            return;
        };
        let off_status = 0x80 | (status & 0x0F);
        let mut host = host;
        let mut slot = None;
        for (i, entry) in self.offs.iter_mut().enumerate() {
            match entry {
                Some(off) if off.port == port && off.status == off_status && off.note == *note => {
                    if off.sent {
                        host = host.max(off.host);
                    } else {
                        // 짝인 Note On이 아직 미래면(lookahead로 미리 넘긴 클릭) 그 뒤에 끄고, 새 노트도
                        // 그 뒤로 미룬다. 끄기가 짝보다 먼저 가면 그 노트가 남는다.
                        let at = off.host.min(host).max(off.on_host);
                        host = host.max(at);
                        let msg = Midi1Message::from_status(off_status, *note, 0);
                        *entry = None;
                        self.send_to(port, at, msg);
                    }
                    slot = Some(i);
                    break;
                }
                None if slot.is_none() => slot = Some(i),
                _ => {}
            }
        }
        let slot = slot.unwrap_or_else(|| self.free_earliest_off());
        // 정지·패닉·표 넘침 등 어느 경로로든 미래로 넘긴 Note Off보다 앞에 켜지 않는다.
        let key = active_index(port, status & 0x0F, *note);
        let host = host.max(self.off_until.get(key).copied().unwrap_or(0));
        self.send_to(port, host, on);
        self.offs[slot] = Some(PendingOff {
            host: host.saturating_add(length.max(1)),
            on_host: host,
            port,
            status: off_status,
            note: *note,
            sent: false,
        });
    }

    /// 끄기 대기 표가 가득일 때: 가장 이른 대기를 지금 보내고(넘기지 않았으면) 그 칸을 돌려준다.
    /// Note Off를 잃는 일은 없다. 노트가 예정보다 일찍 끊길 뿐이다.
    fn free_earliest_off(&mut self) -> usize {
        let mut earliest = 0;
        for (i, entry) in self.offs.iter().enumerate() {
            if let (Some(a), Some(b)) = (entry, &self.offs[earliest])
                && a.host < b.host
            {
                earliest = i;
            }
        }
        if let Some(off) = self.offs[earliest].take()
            && !off.sent
        {
            let at = now().max(self.last_sent.min(off.host)).max(off.on_host);
            self.send_to(
                off.port,
                at,
                Midi1Message::from_status(off.status, off.note, 0),
            );
        }
        earliest
    }

    /// 시각이 lookahead 창에 들어온 Note Off를 예약 송신하고, 시각이 지난 칸을 비운다.
    fn flush_offs(&mut self) {
        let now = now();
        let horizon = now.saturating_add(self.lookahead);
        for i in 0..PENDING_OFFS {
            let Some(mut off) = self.offs[i] else {
                continue;
            };
            if !off.sent && off.host <= horizon {
                self.send_to(
                    off.port,
                    off.host,
                    Midi1Message::from_status(off.status, off.note, 0),
                );
                off.sent = true;
            }
            self.offs[i] = (!off.sent || off.host > now).then_some(off);
        }
    }

    /// 교체 때: 같은 번호에 같은 포트가 없으면 그 번호의 끄기 대기를 버린다. 그 노트는 옛 포트로 이미
    /// 껐고, 남겨 두면 새 포트로 엉뚱한 Note Off가 간다.
    fn forget_offs_on_changed_ports(&mut self, new: &Snapshot) {
        for entry in &mut self.offs {
            if let Some(off) = entry
                && !same_port_at(&self.snap, new, usize::from(off.port))
            {
                *entry = None;
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
        let kind = b[0] & 0xF0;
        if b.len() == 3 && (kind == 0x90 || kind == 0x80) {
            let i = active_index(port, b[0] & 0x0F, b[1]);
            if let Some(count) = self.active.get_mut(i) {
                if kind == 0x90 && b[2] > 0 {
                    *count = count.saturating_add(1);
                } else {
                    *count = count.saturating_sub(1);
                    if let Some(until) = self.off_until.get_mut(i) {
                        *until = (*until).max(host);
                    }
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
            let p = u16::try_from(port).unwrap_or(u16::MAX);
            for ch in 0..16u8 {
                for note in 0..128u8 {
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
        // 방금 모두 껐으므로 아직 넘기지 않은 끄기 대기는 버린다. 넘긴 것은 시각이 지날 때까지 두어,
        // 같은 노트를 다시 켤 때 그 뒤로 미루는 데 쓴다(`play_note`).
        for entry in &mut self.offs {
            if entry.is_some_and(|off| !off.sent) {
                *entry = None;
            }
        }
    }

    /// 새 스냅샷에서 같은 번호에 같은 포트가 없으면 그 번호의 울리는 노트를 옛 포트로 끈다.
    fn release_changed_ports(&mut self, new: &Snapshot, host: u64) {
        for port in 0..self.snap.ports.len() {
            if same_port_at(&self.snap, new, port) {
                continue;
            }
            let p = u16::try_from(port).unwrap_or(u16::MAX);
            for ch in 0..16u8 {
                for note in 0..128u8 {
                    let i = active_index(p, ch, note);
                    if self.active.get(i).copied().unwrap_or(0) > 0 {
                        self.active[i] = 0;
                        self.send_to(p, host, Midi1Message::from_status(0x80 | ch, note, 0));
                    }
                }
            }
        }
    }

    /// 교체 때: 새 스냅샷에서 `t` 뒤 같은 노트의 첫 사건이 Note Off면 이어서 울리게 두고, 아니면 끈다.
    fn release_unless_continued(&mut self, new: &Snapshot, t: Tick, host: u64) {
        for port in 0..self.snap.ports.len() {
            let p = u16::try_from(port).unwrap_or(u16::MAX);
            for ch in 0..16u8 {
                for note in 0..128u8 {
                    let i = active_index(p, ch, note);
                    if self.active.get(i).copied().unwrap_or(0) == 0 {
                        continue;
                    }
                    // 같은 번호에 다른 포트가 왔으면 옛 포트의 노트는 끈다(리뷰 2 지적 3).
                    if same_port_at(&self.snap, new, port) && new.next_note_is_off(i, t) {
                        continue;
                    }
                    self.active[i] = 0;
                    self.send_to(p, host, Midi1Message::from_status(0x80 | ch, note, 0));
                }
            }
        }
    }

    /// `from` 직전까지의 프로그램·뱅크·CC·프레셔·벤드 마지막 값을 `host`에 보낸다(CTL-6).
    fn chase(&mut self, from: Tick, host: u64) {
        self.chase.fill(NONE);
        for e in &self.snap.events[..self.snap.first_at_or_after(from)] {
            let b = e.msg.bytes();
            let ch = usize::from(b[0] & 0x0F);
            let base = (usize::from(e.port) * 16 + ch) * CHASE_SLOTS;
            match (b[0] & 0xF0, b.len()) {
                (0xB0, 3) if usize::from(b[1]) < PROGRAM_SLOT => {
                    let cc = usize::from(b[1]);
                    self.chase[base + cc] = b[2];
                    if RPN_SELECT.contains(&cc) {
                        self.chase[base + SELECT_KIND_SLOT] = RPN;
                    } else if NRPN_SELECT.contains(&cc) {
                        self.chase[base + SELECT_KIND_SLOT] = NRPN;
                    }
                }
                (0xC0, 2) => self.chase[base + PROGRAM_SLOT] = b[1],
                (0xD0, 2) => self.chase[base + PRESSURE_SLOT] = b[1],
                (0xE0, 3) => {
                    self.chase[base + BEND_LSB_SLOT] = b[1];
                    self.chase[base + BEND_MSB_SLOT] = b[2];
                }
                _ => {}
            }
        }
        for port in 0..self.snap.ports.len() {
            let p = u16::try_from(port).unwrap_or(u16::MAX);
            for ch in 0..16u8 {
                let base = (port * 16 + usize::from(ch)) * CHASE_SLOTS;
                let cc = |slot: usize| u8::try_from(slot).unwrap_or(0);
                // 순서: 뱅크 MSB·LSB(프로그램보다 먼저), 프로그램, 파라미터 선택(데이터 엔트리보다 먼저),
                // 데이터 엔트리, 나머지 CC, 프레셔, 벤드.
                for slot in [0usize, 32] {
                    self.chase_cc(p, ch, host, base, slot, cc(slot));
                }
                let v = self.chase[base + PROGRAM_SLOT];
                if v != NONE {
                    self.send_to(p, host, Midi1Message::from_status(0xC0 | ch, v, 0));
                }
                let (first, last) = if self.chase[base + SELECT_KIND_SLOT] == NRPN {
                    (RPN_SELECT, NRPN_SELECT)
                } else {
                    (NRPN_SELECT, RPN_SELECT)
                };
                for slot in first.into_iter().chain(last).chain(DATA_ENTRY) {
                    self.chase_cc(p, ch, host, base, slot, cc(slot));
                }
                for slot in 1..PROGRAM_SLOT {
                    if slot == 32
                        || PARAMETER_SELECT.contains(&slot)
                        || DATA_ENTRY.contains(&slot)
                        || RELATIVE.contains(&slot)
                    {
                        continue;
                    }
                    self.chase_cc(p, ch, host, base, slot, cc(slot));
                }
                let v = self.chase[base + PRESSURE_SLOT];
                if v != NONE {
                    self.send_to(p, host, Midi1Message::from_status(0xD0 | ch, v, 0));
                }
                let (lsb, msb) = (
                    self.chase[base + BEND_LSB_SLOT],
                    self.chase[base + BEND_MSB_SLOT],
                );
                if lsb != NONE && msb != NONE {
                    self.send_to(p, host, Midi1Message::from_status(0xE0 | ch, lsb, msb));
                }
            }
        }
    }

    fn chase_cc(&mut self, port: u16, ch: u8, host: u64, base: usize, slot: usize, cc: u8) {
        let v = self.chase[base + slot];
        if v != NONE {
            self.send_to(port, host, Midi1Message::from_status(0xB0 | ch, cc, v));
        }
    }

    /// 패닉(OUT-6): 지금 모든 포트·채널에 All Sound Off, All Notes Off, Sustain 0을 보낸다. 이미 미래로
    /// 넘긴 Note On이 그 뒤에 도착할 수 있으므로, 울리는 노트를 그 뒤로 한 번 더 끈다(리뷰 1 지적 2).
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
        let at = self.release_time();
        self.release_notes(at);
    }
}

fn now() -> u64 {
    HostTime::now().ticks()
}

/// `t` 이상인 첫 Clock 틱(40의 배수). 넘치면 가장 큰 값(리뷰 1 지적 8).
fn clock_after(t: u64) -> u64 {
    // 포화하면 `u64::MAX`(40의 배수가 아니다)가 되어 「더 보낼 Clock 없음」을 뜻한다.
    t.div_ceil(CLOCK_TICKS).saturating_mul(CLOCK_TICKS)
}

fn active_index(port: u16, ch: u8, note: u8) -> usize {
    (usize::from(port) * 16 + usize::from(ch & 0x0F)) * 128 + usize::from(note & 0x7F)
}

/// `at` 이상인 첫 박 틱. 박자표 변화는 새 마디(첫 박)를 시작한다. 없으면(틱 범위 끝) `u64::MAX`.
fn beat_at_or_after(meter: &MeterMap, at: Tick) -> u64 {
    let pos = meter.position(at);
    if pos.tick() == 0 {
        return at.get();
    }
    let beat = meter.signature_at(at).beat_ticks();
    let next = (at.get() - pos.tick()).checked_add(beat);
    let change = meter
        .changes()
        .map(|(tick, _)| tick.get())
        .find(|&tick| tick > at.get());
    match (next, change) {
        (Some(n), Some(c)) => n.min(c),
        (Some(n), None) => n,
        (None, Some(c)) => c,
        (None, None) => u64::MAX,
    }
}

/// `port` 번호에 옛 스냅샷과 같은 포트 객체가 있는가. 번호가 같아도 포트가 바뀔 수 있다(출력 변경,
/// 재연결).
fn same_port_at(old: &Snapshot, new: &Snapshot, port: usize) -> bool {
    match (old.ports.get(port), new.ports.get(port)) {
        (Some(a), Some(b)) => std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b)),
        _ => false,
    }
}

/// `from`에서 `to`까지의 시각(ns, 내림). `to`가 앞이면 0이다. 스케줄러는 내림 없이 `MusicTime` 단위로
/// 계산하고(`Rt::host_of`), 이 함수는 그 산술을 단위 테스트로 확인하는 데 쓴다.
#[cfg(test)]
pub(crate) fn nanos_between(tempo: &TempoMap, from: Tick, to: Tick) -> u128 {
    tempo
        .tick_to_time(to)
        .units()
        .saturating_sub(tempo.tick_to_time(from).units())
        / MusicTime::UNITS_PER_NANO
}

/// ns를 `mach_absolute_time` 틱으로 바꾼다(내림). `mach_timebase_info`는 `틱 × numer / denom = ns`다.
pub(crate) fn nanos_to_host_ticks(nanos: u128, numer: u32, denom: u32) -> u64 {
    let ticks = nanos.saturating_mul(u128::from(denom)) / u128::from(numer.max(1));
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
