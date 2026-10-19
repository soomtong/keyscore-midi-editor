//! S1-T4 적대적 리뷰 2회차 테스트(리뷰 1 수정 `c1794a2` 대상).
//!
//! 리뷰 1의 기록 포트(넘겨받은 타임스탬프와 바이트를 적음)에 더해, 스케줄러 스레드를 `send` 안에서
//! 붙잡아 두는 관문 포트를 쓴다. 관문이 닫힌 동안 명령을 쌓아 두면 그 명령들이 같은 바퀴에서 처리되므로
//! 「같은 바퀴의 재생과 교체」 같은 경계를 결정적으로 만들 수 있다. 관문은 원자 플래그로 돌며 기다리므로
//! 락·할당이 없다. 이 바이너리는 `assert_no_alloc` 전역 할당자를 두지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use keyscore_core::{Channel, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, Midi1Message, MidiError, MidiPort, ScheduledEvent, Scheduler, SchedulerConfig,
    Snapshot, control_change, note_off, note_on,
};

/// (스케줄러가 넘긴 호스트 타임스탬프, MIDI 1.0 바이트).
type Log = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

fn decode(ump: &[u32]) -> Vec<u8> {
    let w = ump[0];
    let byte = |shift: u32| u8::try_from((w >> shift) & 0xFF).unwrap();
    let status = byte(16);
    let len = match status & 0xF0 {
        0xC0 | 0xD0 => 2,
        0xF0 => match status {
            0xF1 | 0xF3 => 2,
            0xF2 => 3,
            _ => 1,
        },
        _ => 3,
    };
    [status, byte(8), byte(0)][..len].to_vec()
}

struct Rec {
    log: Log,
    dropped_on: Arc<Mutex<Option<String>>>,
}

impl MidiPort for Rec {
    fn name(&self) -> String {
        "rec".to_owned()
    }

    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        self.log.lock().unwrap().push((at.ticks(), decode(ump)));
        Ok(())
    }
}

impl Drop for Rec {
    fn drop(&mut self) {
        let name = std::thread::current().name().unwrap_or("").to_owned();
        *self.dropped_on.lock().unwrap() = Some(name);
    }
}

fn rec() -> (Arc<dyn MidiPort>, Log) {
    let log: Log = Arc::default();
    let port: Arc<dyn MidiPort> = Arc::new(Rec {
        log: Arc::clone(&log),
        dropped_on: Arc::default(),
    });
    (port, log)
}

/// `hold`가 켜져 있으면 `send` 안에서 돌며 기다린다. 들어오면 `entered`를 켠다.
struct Gate {
    hold: Arc<AtomicBool>,
    entered: Arc<AtomicBool>,
}

impl MidiPort for Gate {
    fn name(&self) -> String {
        "gate".to_owned()
    }

    fn send(&self, _at: HostTime, _ump: &[u32]) -> Result<(), MidiError> {
        self.entered.store(true, Ordering::SeqCst);
        while self.hold.load(Ordering::SeqCst) {
            std::hint::spin_loop();
        }
        Ok(())
    }
}

struct GateHandle {
    hold: Arc<AtomicBool>,
    entered: Arc<AtomicBool>,
}

impl GateHandle {
    fn close(&self) {
        self.entered.store(false, Ordering::SeqCst);
        self.hold.store(true, Ordering::SeqCst);
    }

    fn wait_entered(&self) {
        let t0 = Instant::now();
        while !self.entered.load(Ordering::SeqCst) {
            assert!(t0.elapsed() < Duration::from_secs(2), "관문에 안 들어옴");
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    fn open(&self) {
        self.hold.store(false, Ordering::SeqCst);
    }
}

fn gate() -> (Arc<dyn MidiPort>, GateHandle) {
    let hold = Arc::new(AtomicBool::new(false));
    let entered = Arc::new(AtomicBool::new(false));
    let port: Arc<dyn MidiPort> = Arc::new(Gate {
        hold: Arc::clone(&hold),
        entered: Arc::clone(&entered),
    });
    (port, GateHandle { hold, entered })
}

/// 상태 바이트별 송신 수만 세는 포트(할당 없음). 스레드가 헛돌 때 기록이 무한히 커지지 않게 한다.
struct Counter {
    clocks: Arc<AtomicU64>,
    notes: Arc<AtomicU64>,
}

impl MidiPort for Counter {
    fn name(&self) -> String {
        "counter".to_owned()
    }

    fn send(&self, _at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        let status = (ump[0] >> 16) & 0xFF;
        if status == 0xF8 {
            self.clocks.fetch_add(1, Ordering::Relaxed);
        } else if status & 0xF0 == 0x90 {
            self.notes.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

fn ch(n: u8) -> Channel {
    Channel::new(n).unwrap()
}

fn on(p: u8) -> Midi1Message {
    note_on(
        ch(1),
        Pitch::new(p).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
}

fn off(p: u8) -> Midi1Message {
    note_off(ch(1), Pitch::new(p).unwrap(), Velocity::new(0))
}

fn cc(n: u8, v7: u32) -> Midi1Message {
    control_change(ch(1), n, v7 << 25).unwrap()
}

fn ev(tick: u64, msg: Midi1Message) -> ScheduledEvent {
    ScheduledEvent::new(Tick::new(tick), 0, msg)
}

fn bpm(n: u32) -> TempoMap {
    TempoMap::new(Tempo::from_bpm(n).unwrap())
}

fn micros(us: u32) -> TempoMap {
    TempoMap::new(Tempo::from_micros_per_quarter(us).unwrap())
}

fn snap_on(
    ports: &[Arc<dyn MidiPort>],
    tempo: TempoMap,
    events: Vec<ScheduledEvent>,
) -> Arc<Snapshot> {
    Arc::new(Snapshot::new(tempo, ports.to_vec(), events).unwrap())
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn ns_between(earlier: u64, later: u64) -> u128 {
    HostTime::from_ticks(later)
        .duration_since(HostTime::from_ticks(earlier))
        .as_nanos()
}

fn by_time(log: &Log) -> Vec<(u64, Vec<u8>)> {
    let mut v = log.lock().unwrap().clone();
    v.sort_by_key(|(t, _)| *t);
    v
}

fn sounding_at_end(log: &Log) -> BTreeSet<(u8, u8)> {
    let mut sounding = BTreeSet::new();
    for (_, b) in by_time(log) {
        let c = b[0] & 0x0F;
        match b[0] & 0xF0 {
            0x90 if b[2] > 0 => {
                sounding.insert((c, b[1]));
            }
            0x80 | 0x90 => {
                sounding.remove(&(c, b[1]));
            }
            0xB0 if b[1] == 120 || b[1] == 123 => sounding.retain(|(x, _)| *x != c),
            _ => {}
        }
    }
    sounding
}

fn note_ons(log: &Log, pitch: u8) -> Vec<u64> {
    by_time(log)
        .into_iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0x90 && b[1] == pitch && b[2] > 0)
        .map(|(t, _)| t)
        .collect()
}

/// 틱 k마다 서로 다른 CC 하나(CC 20, 채널 k/128+1, 값 k%128). 받은 메시지에서 k를 되찾을 수 있다.
fn dense(count: u64) -> Vec<ScheduledEvent> {
    (0..count)
        .map(|k| {
            let c = u8::try_from(k / 128 + 1).unwrap();
            let v = u32::try_from(k % 128).unwrap();
            ev(k, control_change(ch(c), 20, v << 25).unwrap())
        })
        .collect()
}

fn dense_ticks(log: &Log) -> BTreeSet<u64> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xB0 && b[1] == 20)
        .map(|(_, b)| u64::from(b[0] & 0x0F) * 128 + u64::from(b[2]))
        .collect()
}

fn first_gap(ticks: &BTreeSet<u64>) -> Option<(u64, u64)> {
    ticks
        .iter()
        .zip(ticks.iter().skip(1))
        .find(|(a, b)| **b != **a + 1)
        .map(|(a, b)| (*a, *b))
}

/// 받는 쪽이 RPN·NRPN 선택과 데이터 엔트리 MSB(CC 6)로 만드는 파라미터 값. 널 RPN(127, 127)은 버린다.
fn parameter_values(ccs: &[(u8, u8)]) -> BTreeMap<(bool, u8, u8), u8> {
    let (mut rpn, mut nrpn) = ((None, None), (None, None));
    let mut last_nrpn = None;
    let mut out = BTreeMap::new();
    for &(n, v) in ccs {
        match n {
            101 => (rpn.0, last_nrpn) = (Some(v), Some(false)),
            100 => (rpn.1, last_nrpn) = (Some(v), Some(false)),
            99 => (nrpn.0, last_nrpn) = (Some(v), Some(true)),
            98 => (nrpn.1, last_nrpn) = (Some(v), Some(true)),
            6 => {
                let sel = match last_nrpn {
                    Some(true) => nrpn,
                    Some(false) => rpn,
                    None => continue,
                };
                if let (Some(msb), Some(lsb)) = sel
                    && !(last_nrpn == Some(false) && msb == 127 && lsb == 127)
                {
                    out.insert((last_nrpn == Some(true), msb, lsb), v);
                }
            }
            _ => {}
        }
    }
    out
}

// ---- 깨뜨린 사례(현재 구현에서 실패한다) ----

/// 지적 1: 재생과 교체가 같은 바퀴에서 처리되면 `scheduled_until`이 재생 기준 시각보다 앞이라
/// `tick_at`이 `base_tick`으로 잘리고, 교체가 재생 위치의 이벤트를 「이미 보낸 것」으로 건너뛴다.
/// 재생 버튼 직후 편집(TRN-3)하면 첫 박의 노트가 사라진다.
#[test]
fn swap_in_same_wake_as_play_keeps_event_at_play_position() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let events = || vec![ev(0, on(60)), ev(480, off(60))];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    gh.close();
    s.send_now(1, cc(1, 0)).unwrap();
    gh.wait_entered();
    s.play(Tick::ZERO).unwrap();
    s.swap(snap_on(&ports, bpm(120), events())).unwrap();
    gh.open();
    sleep_ms(400);
    s.stop().unwrap();
    sleep_ms(20);
    assert_eq!(
        note_ons(&log, 60).len(),
        1,
        "틱 0의 Note On이 사라짐: {:?}",
        by_time(&log)
    );
}

/// 지적 1(Clock): 같은 경로로 틱 0의 Clock이 빠진다. Start 뒤 첫 Clock이 한 칸(40틱) 늦어 받는 장치가
/// 24 ppqn 한 칸 뒤처진다(OUT-5).
#[test]
fn swap_in_same_wake_as_play_keeps_first_clock_with_start() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    gh.close();
    s.send_now(1, cc(1, 0)).unwrap();
    gh.wait_entered();
    s.play(Tick::ZERO).unwrap();
    s.swap(snap_on(&ports, bpm(120), vec![])).unwrap();
    gh.open();
    sleep_ms(100);
    s.stop().unwrap();
    sleep_ms(20);
    let msgs = by_time(&log);
    let start_at = msgs.iter().find(|(_, b)| b[0] == 0xFA).unwrap().0;
    let first_clock = msgs.iter().find(|(_, b)| b[0] == 0xF8).unwrap().0;
    assert_eq!(
        first_clock,
        start_at,
        "Start 뒤 첫 Clock이 {} ns 늦음",
        ns_between(start_at, first_clock)
    );
}

/// 지적 2: 한 바퀴에서 교체가 두 번 처리되고 새 템포가 빠르면, 두 번째 교체가 첫 교체의 기준점과
/// `scheduled_until` 사이 틱(아무도 보내지 않음)을 「이미 보낸 구간」으로 건너뛴다. 이벤트가 사라진다.
/// 교체 한 번이면 같은 틱이 늦게라도 간다(아래 통과 테스트).
#[test]
fn two_swaps_in_one_wake_to_faster_tempo_lose_no_events() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    // 4 BPM(한 틱 15.6 ms)으로 틀다가 600 BPM(한 틱 0.1 ms)으로 바꾼다.
    let mut s = Scheduler::start(
        snap_on(&ports, micros(15_000_000), dense(2048)),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(60);
    gh.close();
    s.send_now(1, cc(1, 0)).unwrap();
    gh.wait_entered();
    s.swap(snap_on(&ports, micros(100_000), dense(2048)))
        .unwrap();
    s.swap(snap_on(&ports, micros(100_000), dense(2048)))
        .unwrap();
    gh.open();
    sleep_ms(400);
    s.stop().unwrap();
    let ticks = dense_ticks(&log);
    assert!(ticks.contains(&0) && ticks.contains(&2047), "{ticks:?}");
    assert_eq!(first_gap(&ticks), None, "보내지 않은 틱 구간");
}

/// 지적 3: 이어 울리는 노트(리뷰 1 지적 3·13의 수정)는 포트 번호만 본다. 같은 번호에 다른 포트가 온
/// 스냅샷(트랙 출력 변경)으로 바꾸면, 옛 포트의 노트는 끄지 않고 Note Off는 새 포트로 간다. 옛 포트에
/// 노트가 걸린다.
#[test]
fn swap_to_snapshot_with_different_port_releases_note_on_old_port() {
    let (old_port, old_log) = rec();
    let (new_port, _new_log) = rec();
    let events = || vec![ev(0, on(60)), ev(1920, off(60))];
    let mut s = Scheduler::start(
        snap_on(&[old_port], bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(100);
    s.swap(snap_on(&[new_port], bpm(120), events())).unwrap();
    sleep_ms(1200);
    s.stop().unwrap();
    sleep_ms(30);
    drop(s);
    let left = sounding_at_end(&old_log);
    assert!(
        left.is_empty(),
        "옛 포트에 걸린 노트 {left:?}: {:?}",
        by_time(&old_log)
    );
}

/// 지적 4: 패닉을 명령 큐 밖 원자 플래그로 옮기면서 순서가 뒤집혔다. 패닉 전에 넣은 `send_now` Note On이
/// 같은 바퀴에서 패닉 뒤에 처리되어, 패닉(OUT-6) 뒤에도 노트가 울린다.
#[test]
fn panic_silences_note_sent_before_it() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    // 관문 포트의 틱 0 이벤트로 스케줄러를 `schedule` 안에서 붙잡는다.
    let events = vec![ScheduledEvent::new(Tick::ZERO, 1, cc(1, 0))];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events),
        SchedulerConfig::default(),
    )
    .unwrap();
    gh.close();
    s.play(Tick::ZERO).unwrap();
    gh.wait_entered();
    s.send_now(0, on(61)).unwrap();
    s.panic().unwrap();
    gh.open();
    sleep_ms(50);
    let left = sounding_at_end(&log);
    assert!(
        left.is_empty(),
        "패닉 뒤 울리는 노트 {left:?}: {:?}",
        by_time(&log)
    );
}

/// 지적 5: 리뷰 1 지적 8의 포화 산술이 새 무한 루프를 만든다. `u64::MAX` 근처에서 Clock을 켜고 재생하면
/// `clock_after`가 `u64::MAX`로 포화하고 `next_clock += 40`도 그대로라, 같은 Clock을 끝없이 보낸다.
/// 스레드가 `quit`을 다시 보지 못해 drop(join)도 돌아오지 않는다. 그래서 이 테스트는 실패할 때 손잡이를
/// 잊는다(스레드는 프로세스 끝까지 돈다).
#[test]
fn play_near_max_tick_with_clock_does_not_flood_clock() {
    let clocks = Arc::new(AtomicU64::new(0));
    let port: Arc<dyn MidiPort> = Arc::new(Counter {
        clocks: Arc::clone(&clocks),
        notes: Arc::default(),
    });
    let mut s = Scheduler::start(
        snap_on(&[port], bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(u64::MAX - 10)).unwrap();
    sleep_ms(50);
    let n = clocks.load(Ordering::Relaxed);
    if n > 10 {
        std::mem::forget(s);
        panic!("50 ms에 Clock {n}개(스케줄러 스레드가 헛돈다)");
    }
}

/// 지적 6: 템포가 아주 빠르면(1 µs/4분음표, 공개 API가 허용) 짧은 루프 한 바퀴가 호스트 틱 0이 되어
/// 루프 끝을 같은 시각에 끝없이 되감는다. 스레드가 명령을 더 처리하지 못한다.
#[test]
fn loop_shorter_than_one_host_tick_keeps_scheduler_responsive() {
    let notes = Arc::new(AtomicU64::new(0));
    let port: Arc<dyn MidiPort> = Arc::new(Counter {
        clocks: Arc::default(),
        notes: Arc::clone(&notes),
    });
    let mut s = Scheduler::start(
        snap_on(&[port], micros(1), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_loop(Some((Tick::ZERO, Tick::new(1)))).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(30);
    s.send_now(0, on(61)).unwrap();
    sleep_ms(200);
    if notes.load(Ordering::Relaxed) == 0 {
        std::mem::forget(s);
        panic!("스케줄러가 루프 끝에서 헛돌아 send_now를 처리하지 않음");
    }
}

/// 지적 7: chase가 NRPN 선택(99·98)을 늘 RPN 선택(101·100)보다 먼저 보낸다. 곡에서 마지막으로 고른 것이
/// NRPN이면, 그 데이터 엔트리 값이 RPN(아래는 벤드 범위 RPN 0)에 들어간다. 리뷰 1 지적 10의 「값이 다른
/// 파라미터에 들어간다」가 남아 있다.
#[test]
fn chase_with_rpn_then_nrpn_writes_no_wrong_parameter() {
    let (port, log) = rec();
    let stream: Vec<(u64, u8, u8)> = vec![
        (0, 101, 0),
        (0, 100, 0),
        (0, 6, 2),
        (10, 99, 1),
        (10, 98, 8),
        (10, 6, 100),
    ];
    let mut events: Vec<ScheduledEvent> = stream
        .iter()
        .map(|&(t, n, v)| ev(t, cc(n, u32::from(v))))
        .collect();
    events.push(ev(1920, on(60)));
    let mut s = Scheduler::start(
        snap_on(&[port], bpm(120), events),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::new(960)).unwrap();
    sleep_ms(30);
    s.stop().unwrap();
    let chased: Vec<(u8, u8)> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xB0)
        .map(|(_, b)| (b[1], b[2]))
        .collect();
    let original: Vec<(u8, u8)> = stream.iter().map(|&(_, n, v)| (n, v)).collect();
    let truth = parameter_values(&original);
    for (param, v) in parameter_values(&chased) {
        assert_eq!(
            truth.get(&param),
            Some(&v),
            "chase가 파라미터 {param:?}에 {v}를 씀(곡: {truth:?}, chase CC: {chased:?})"
        );
    }
}

/// 지적 8: 교체마다 기준 호스트 시각을 `host_of(T)`(내림)로 다시 잡아, 교체 한 번에 호스트 틱 1 미만
/// (Apple Silicon 41.7 ns)씩 격자가 앞당겨진다. 리뷰 1 지적 4의 누적이 크기만 줄어 남아 있다. 바퀴마다
/// 교체 한 번씩 3000번이면 수십 µs다(정확한 산술이면 0).
#[test]
fn many_swaps_do_not_accumulate_rounding_drift() {
    let (port, log) = rec();
    let events = || {
        (0..30u64)
            .flat_map(|k| [ev(k * 960, on(60)), ev(k * 960 + 60, off(60))])
            .collect::<Vec<_>>()
    };
    let ports = [port];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(30);
    let h0 = note_ons(&log, 60)[0];
    let shared = snap_on(&ports, bpm(120), events());
    for _ in 0..3000 {
        s.swap(Arc::clone(&shared)).unwrap();
        std::thread::sleep(Duration::from_micros(1100));
        s.collect_garbage();
    }
    let swaps_done = HostTime::now().ticks();
    sleep_ms(1100);
    s.stop().unwrap();
    let beat: u128 = 500_000_000;
    let worst = note_ons(&log, 60)
        .into_iter()
        .filter(|t| *t > swaps_done)
        .map(|t| {
            let r = ns_between(h0, t) % beat;
            r.min(beat - r)
        })
        .max()
        .unwrap();
    assert!(
        worst < 1_000,
        "교체 3000번 뒤 박 격자에서 {worst} ns 벗어남"
    );
}

// ---- 못 깨뜨린 경계(통과한다) ----

/// 교체 한 번이면 빠른 템포로 바꿔도 이벤트가 빠지거나 겹치지 않는다(지적 2의 대조군).
#[test]
fn one_swap_to_faster_tempo_loses_no_events() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let mut s = Scheduler::start(
        snap_on(&ports, micros(15_000_000), dense(2048)),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(60);
    gh.close();
    s.send_now(1, cc(1, 0)).unwrap();
    gh.wait_entered();
    s.swap(snap_on(&ports, micros(100_000), dense(2048)))
        .unwrap();
    gh.open();
    sleep_ms(400);
    s.stop().unwrap();
    let ticks = dense_ticks(&log);
    let sent = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xB0 && b[1] == 20)
        .count();
    assert_eq!(sent, ticks.len(), "같은 틱을 두 번 보냄");
    assert!(ticks.contains(&0) && ticks.contains(&2047));
    assert_eq!(first_gap(&ticks), None);
}

/// 줄인 노트로 바꾸면 이어 울리다가 새 끝에서 꺼진다. 교체 시점에 끊기지 않는다.
#[test]
fn swap_shortening_sounding_note_ends_it_at_new_end() {
    let (port, log) = rec();
    let ports = [port];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), vec![ev(0, on(60)), ev(1920, off(60))]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(100);
    s.swap(snap_on(
        &ports,
        bpm(120),
        vec![ev(0, on(60)), ev(960, off(60))],
    ))
    .unwrap();
    sleep_ms(700);
    s.stop().unwrap();
    sleep_ms(20);
    let msgs = by_time(&log);
    let on_at = note_ons(&log, 60)[0];
    let offs: Vec<u64> = msgs
        .iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0x80 && b[1] == 60)
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(offs.len(), 1, "{msgs:?}");
    assert!(ns_between(on_at, offs[0]).abs_diff(500_000_000) < 1_000);
    assert!(sounding_at_end(&log).is_empty());
}

/// Clock 켜고 루프: 되감을 때마다 Stop, SPP(루프 시작), Continue가 같은 시각에 가고, 한 바퀴의 Clock 수는
/// 바퀴 길이 / 40틱이다(Clock 중복·누락 없음).
#[test]
fn loop_wrap_with_clock_resyncs_and_keeps_clock_count() {
    let (port, log) = rec();
    let mut s = Scheduler::start(
        snap_on(&[port], bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    s.set_loop(Some((Tick::new(480), Tick::new(1440)))).unwrap();
    s.play(Tick::new(480)).unwrap();
    sleep_ms(1700);
    s.stop().unwrap();
    sleep_ms(20);
    let msgs = by_time(&log);
    let continues: Vec<usize> = msgs
        .iter()
        .enumerate()
        .filter(|(_, (_, b))| b[0] == 0xFB)
        .map(|(i, _)| i)
        .collect();
    assert!(continues.len() >= 3, "{msgs:?}");
    for w in continues.windows(2) {
        let lap = &msgs[w[0] + 1..w[1]];
        let clocks = lap.iter().filter(|(_, b)| b[0] == 0xF8).count();
        assert_eq!(clocks, 24, "한 바퀴 Clock 수");
        let n = lap.len();
        assert_eq!(lap[n - 2].1, vec![0xFC]);
        assert_eq!(lap[n - 1].1, vec![0xF2, 2, 0]);
        assert_eq!(lap[n - 2].0, msgs[w[1]].0);
        assert_eq!(lap[n - 1].0, msgs[w[1]].0);
    }
}

/// 큐에 교체가 남은 채로 drop해도 모든 스냅샷의 포트가 제어 스레드에서 해제된다(리뷰 1 지적 12).
#[test]
fn drop_with_queued_swaps_releases_all_ports_on_control_thread() {
    let mut marks = Vec::new();
    let mut make = || {
        let dropped_on: Arc<Mutex<Option<String>>> = Arc::default();
        marks.push(Arc::clone(&dropped_on));
        let port: Arc<dyn MidiPort> = Arc::new(Rec {
            log: Arc::default(),
            dropped_on,
        });
        snap_on(&[port], bpm(120), vec![ev(0, on(60))])
    };
    let mut s = Scheduler::start(make(), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    let mut queued = 0;
    for _ in 0..600 {
        if s.swap(make()).is_err() {
            break;
        }
        queued += 1;
    }
    assert!(queued > 260, "{queued}");
    sleep_ms(30);
    drop(s);
    let me = std::thread::current().name().unwrap_or("").to_owned();
    for (i, m) in marks.iter().enumerate() {
        let thread = m.lock().unwrap().clone();
        assert_eq!(thread.as_deref(), Some(me.as_str()), "스냅샷 {i}");
    }
}

/// 노트 색인은 제어 스레드에서 만든다. 포트 32개(색인 칸 65,536개)와 노트 사건 20만 개도 디버그 빌드에서
/// 몇 백 ms 안이다.
#[test]
fn snapshot_note_index_with_32_ports_and_200k_events_is_bounded() {
    let ports: Vec<Arc<dyn MidiPort>> = (0..32).map(|_| rec().0).collect();
    let events: Vec<ScheduledEvent> = (0..100_000u64)
        .flat_map(|k| {
            let port = u16::try_from(k % 32).unwrap();
            let p = u8::try_from(k % 128).unwrap();
            [
                ScheduledEvent::new(Tick::new(k * 10), port, on(p)),
                ScheduledEvent::new(Tick::new(k * 10 + 5), port, off(p)),
            ]
        })
        .collect();
    let t0 = Instant::now();
    let s = Snapshot::new(bpm(120), ports, events).unwrap();
    let took = t0.elapsed();
    assert_eq!(s.events().len(), 200_000);
    assert!(took < Duration::from_secs(2), "{took:?}");
}
