//! MIDI 스케줄러 스레드(REQ 9.2, 9.3, OUT-5, OUT-6, OUT-7, CTL-6, TRN-3).
//!
//! 제어 스레드가 만든 불변 스냅샷의 이벤트를 템포 맵으로 호스트 시각에 맞춰 몇 ms 앞서 예약 송신한다.
//! 정확한 송신 시각은 CoreMIDI 서버가 맞춘다(SP-3). 스케줄러 루프에는 할당·락·로그가 없다.

use std::sync::Arc;
use std::time::Duration;

use keyscore_core::{TempoMap, Tick};

use crate::{Midi1Message, MidiError, MidiPort};

/// 한 스냅샷이 가질 수 있는 포트 수. 스케줄러의 상태 표를 시작할 때 이 크기로 미리 잡는다.
pub const MAX_SCHEDULER_PORTS: usize = 32;

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

/// 스케줄러가 재생하는 불변 묶음. 제어 스레드에서 만들어 `Arc`로 넘긴다.
pub struct Snapshot {}

impl Snapshot {
    /// 이벤트를 정렬해 스냅샷을 만든다. 같은 틱에서는 Note Off, 프로그램·뱅크·CC 등, Note On 순서다.
    ///
    /// # Errors
    ///
    /// 포트가 `MAX_SCHEDULER_PORTS`보다 많거나, 이벤트의 포트 번호가 `ports` 밖이면 `InvalidMessage`.
    pub fn new(
        tempo: TempoMap,
        ports: Vec<Arc<dyn MidiPort>>,
        events: Vec<ScheduledEvent>,
    ) -> Result<Self, MidiError> {
        let _ = (tempo, ports, events);
        todo!()
    }

    #[must_use]
    pub fn events(&self) -> &[ScheduledEvent] {
        todo!()
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

/// 스케줄러 스레드의 손잡이. drop하면 울리는 노트를 끄고 스레드를 끝낸다.
pub struct Scheduler {}

impl Scheduler {
    /// 스케줄러 스레드를 시작한다. 처음에는 정지 상태다.
    ///
    /// # Errors
    ///
    /// 스레드를 만들지 못하거나 `lookahead`가 `wake`보다 짧으면 오류.
    pub fn start(snapshot: Arc<Snapshot>, config: SchedulerConfig) -> Result<Self, MidiError> {
        let _ = (snapshot, config);
        todo!()
    }

    /// `from`부터 재생한다. 그 위치까지의 프로그램·뱅크·CC 값을 먼저 보낸다(chase).
    ///
    /// # Errors
    ///
    /// 명령 큐가 가득이면 `QueueFull`.
    pub fn play(&mut self, from: Tick) -> Result<(), MidiError> {
        let _ = from;
        todo!()
    }

    /// 정지한다. 울리는 노트를 끈다.
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn stop(&mut self) -> Result<(), MidiError> {
        todo!()
    }

    /// 재생 위치를 옮긴다. 재생 중이면 그 위치에서 이어서 재생한다.
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn locate(&mut self, to: Tick) -> Result<(), MidiError> {
        let _ = to;
        todo!()
    }

    /// 루프 범위 `[start, end)`. `None`이면 루프를 끈다.
    ///
    /// # Errors
    ///
    /// 범위가 비었으면 `InvalidMessage`, `QueueFull`.
    pub fn set_loop(&mut self, range: Option<(Tick, Tick)>) -> Result<(), MidiError> {
        let _ = range;
        todo!()
    }

    /// 스냅샷을 원자적으로 바꾼다(TRN-3).
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    #[allow(clippy::needless_pass_by_value)] // 실패 테스트 커밋용 뼈대.
    pub fn swap(&mut self, snapshot: Arc<Snapshot>) -> Result<(), MidiError> {
        let _ = snapshot;
        todo!()
    }

    /// 모든 포트·채널에 패닉 메시지를 보낸다(OUT-6).
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn panic(&mut self) -> Result<(), MidiError> {
        todo!()
    }

    /// 포트의 MIDI Clock 송신을 켜거나 끈다(OUT-5, 기본 끔).
    ///
    /// # Errors
    ///
    /// 포트 번호가 `MAX_SCHEDULER_PORTS` 밖이면 `InvalidMessage`, `QueueFull`.
    pub fn set_clock(&mut self, port: u16, on: bool) -> Result<(), MidiError> {
        let _ = (port, on);
        todo!()
    }

    /// 메시지를 지금 보낸다(미리듣기, NF-1 측정).
    ///
    /// # Errors
    ///
    /// `QueueFull`.
    pub fn send_now(&mut self, port: u16, msg: Midi1Message) -> Result<(), MidiError> {
        let _ = (port, msg);
        todo!()
    }

    /// 스케줄러가 돌려준, 더 쓰지 않는 스냅샷을 이 스레드에서 해제한다. 돌려받은 수를 돌려준다.
    pub fn collect_garbage(&mut self) -> usize {
        todo!()
    }
}

/// `from`에서 `to`까지의 시각(ns, 내림). 정수 산술이라 템포 변화가 있어도 오차가 쌓이지 않는다.
#[allow(dead_code)] // 실패 테스트 커밋용 뼈대. 구현 커밋에서 쓴다.
pub(crate) fn nanos_between(tempo: &TempoMap, from: Tick, to: Tick) -> u128 {
    let _ = (tempo, from, to);
    todo!()
}

/// ns를 `mach_absolute_time` 틱으로 바꾼다. `mach_timebase_info`는 `틱 × numer / denom = ns`다.
#[allow(dead_code)] // 실패 테스트 커밋용 뼈대. 구현 커밋에서 쓴다.
pub(crate) fn nanos_to_host_ticks(nanos: u128, numer: u32, denom: u32) -> u64 {
    let _ = (nanos, numer, denom);
    todo!()
}

/// 명령을 큐에 넣는다. 가득이면 `QueueFull`.
#[allow(dead_code)] // 실패 테스트 커밋용 뼈대. 구현 커밋에서 쓴다.
fn push<T>(queue: &mut rtrb::Producer<T>, item: T) -> Result<(), MidiError> {
    let _ = (queue, item);
    todo!()
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
