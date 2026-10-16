//! S1-T4 적대적 리뷰 4회차 테스트(리뷰 3 수정 `6ec5ce4` 대상).
//!
//! 리뷰 3의 기록 포트·관문 포트를 그대로 쓴다. 모두 이 파일 안의 `MidiPort` 구현이라 CoreMIDI 클라이언트를
//! 만들지 않는다. 관문 포트는 스케줄러 스레드를 `send` 안에서 붙잡아, 그동안 넣은 명령이 다음 바퀴에서
//! 처리되게 한다. 이 바이너리는 `assert_no_alloc` 전역 할당자를 두지 않는다.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
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
}

impl MidiPort for Rec {
    fn name(&self) -> &'static str {
        "rec"
    }

    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        self.log.lock().unwrap().push((at.ticks(), decode(ump)));
        Ok(())
    }
}

fn rec() -> (Arc<dyn MidiPort>, Log) {
    let log: Log = Arc::default();
    let port: Arc<dyn MidiPort> = Arc::new(Rec {
        log: Arc::clone(&log),
    });
    (port, log)
}

/// `hold`가 켜져 있으면 `send` 안에서 돌며 기다린다. 들어오면 `entered`를 켠다.
struct Gate {
    hold: Arc<AtomicBool>,
    entered: Arc<AtomicBool>,
}

impl MidiPort for Gate {
    fn name(&self) -> &'static str {
        "gate"
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

fn note_offs(log: &Log, pitch: u8) -> usize {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| {
            b[1] == pitch && (b[0] & 0xF0 == 0x80 || (b[0] & 0xF0 == 0x90 && b[2] == 0))
        })
        .count()
}

fn notes_with_pitch(log: &Log, pitch: u8) -> usize {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| matches!(b[0] & 0xF0, 0x80 | 0x90) && b[1] == pitch)
        .count()
}

fn status_times(log: &Log, status: u8) -> Vec<u64> {
    by_time(log)
        .into_iter()
        .filter(|(_, b)| b[0] == status)
        .map(|(t, _)| t)
        .collect()
}

/// 스케줄러를 다른 스레드에서 drop하고 `limit` 안에 끝나는지 본다.
fn drop_within(s: Scheduler, limit: Duration) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        drop(s);
        let _ = tx.send(());
    });
    assert!(
        rx.recv_timeout(limit).is_ok(),
        "drop이 {limit:?} 안에 끝나지 않음"
    );
}

/// 틱 `tick`에 몰린 CC `count`개. k번째는 CC 20 + k/2048, 채널 (k/128)%16 + 1, 값 k%128이다.
fn burst_at(tick: u64, count: u64) -> Vec<ScheduledEvent> {
    (0..count)
        .map(|k| {
            let c = u8::try_from((k / 128) % 16 + 1).unwrap();
            let n = u8::try_from(20 + k / 2048).unwrap();
            let v = u32::try_from(k % 128).unwrap();
            ev(tick, control_change(ch(c), n, v << 25).unwrap())
        })
        .collect()
}

fn burst_ids(log: &Log) -> Vec<u64> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xB0 && (20..30).contains(&b[1]))
        .map(|(_, b)| u64::from(b[1] - 20) * 2048 + u64::from(b[0] & 0x0F) * 128 + u64::from(b[2]))
        .collect()
}

/// 120 BPM의 Clock 한 칸(40틱 = 500 ms / 24).
const CLOCK_NS: u128 = 500_000_000 / 24;

/// k번째 Clock이 Start(또는 Continue) + k칸에 가는지 본다. 앞의 `late_ok`개는 늦을 수 있으나(리뷰 4 제안
/// 2 참고) 앞당겨지거나 한 칸 넘게 밀리지 않는다. 빠지면 뒤 Clock이 모두 한 칸씩 밀리고, 겹치면 앞당겨진다.
fn assert_clock_grid(log: &Log, start_status: u8, late_ok: usize) {
    let start = status_times(log, start_status);
    assert_eq!(start.len(), 1, "{start_status:#x} 수: {:?}", by_time(log));
    let clocks = status_times(log, 0xF8);
    assert!(clocks.len() >= 5, "Clock {}개", clocks.len());
    for (k, &c) in clocks.iter().enumerate() {
        assert!(c >= start[0], "Clock {k}이 시작 메시지보다 앞");
        let got = ns_between(start[0], c);
        let expect = CLOCK_NS * k as u128;
        if k < late_ok {
            // 늦어도 미리보기 창(10 ms) 안이다.
            assert!(
                got + 1_000 >= expect && got <= expect + 11_000_000,
                "Clock {k}: {got} ns, 기대 {expect} ns"
            );
        } else {
            assert!(
                got.abs_diff(expect) < 1_000,
                "Clock {k}: {got} ns, 기대 {expect} ns(빠짐이면 한 칸 늦고, 겹침이면 한 칸 이름)"
            );
        }
    }
}

// ---- A1. 리뷰 3 지적 1 주변(정지 중 교체와 포트) ----

/// 정지 중 같은 포트 객체가 다른 번호로 옮겨 가도([A, B] → [B, A]) 미리듣기 노트를 각자의 옛 포트로 끄고,
/// 다른 포트로 Note Off를 보내지 않는다.
#[test]
fn swap_while_stopped_with_ports_reordered_releases_each_note_on_its_own_port() {
    let (a, a_log) = rec();
    let (b, b_log) = rec();
    let mut s = Scheduler::start(
        snap_on(&[Arc::clone(&a), Arc::clone(&b)], bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.send_now(0, on(60)).unwrap();
    s.send_now(1, on(62)).unwrap();
    sleep_ms(20);
    s.swap(snap_on(&[b, a], bpm(120), vec![])).unwrap();
    sleep_ms(20);
    s.collect_garbage();
    s.stop().unwrap();
    sleep_ms(20);
    drop_within(s, Duration::from_secs(2));
    assert!(sounding_at_end(&a_log).is_empty(), "{:?}", by_time(&a_log));
    assert!(sounding_at_end(&b_log).is_empty(), "{:?}", by_time(&b_log));
    assert_eq!(
        notes_with_pitch(&a_log, 62),
        0,
        "A로 B의 노트: {:?}",
        by_time(&a_log)
    );
    assert_eq!(
        notes_with_pitch(&b_log, 60),
        0,
        "B로 A의 노트: {:?}",
        by_time(&b_log)
    );
    assert_eq!(note_offs(&a_log, 60), 1);
    assert_eq!(note_offs(&b_log, 62), 1);
}

/// 정지 중 같은 번호에 같은 포트 객체를 둔 교체(포트가 하나 늘어남)는 미리듣기 노트를 끊지 않는다. 뒤따른
/// Note Off 하나로 꺼진다.
#[test]
fn swap_while_stopped_with_same_port_and_added_port_keeps_preview_note() {
    let (a, a_log) = rec();
    let (b, b_log) = rec();
    let mut s = Scheduler::start(
        snap_on(&[Arc::clone(&a)], bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.send_now(0, on(60)).unwrap();
    sleep_ms(20);
    s.swap(snap_on(&[a, b], bpm(120), vec![])).unwrap();
    sleep_ms(20);
    assert_eq!(
        note_offs(&a_log, 60),
        0,
        "교체가 노트를 끊음: {:?}",
        by_time(&a_log)
    );
    s.send_now(0, off(60)).unwrap();
    sleep_ms(20);
    s.collect_garbage();
    s.stop().unwrap();
    sleep_ms(20);
    drop_within(s, Duration::from_secs(2));
    assert_eq!(note_offs(&a_log, 60), 1, "{:?}", by_time(&a_log));
    assert!(sounding_at_end(&a_log).is_empty());
    assert!(by_time(&b_log).is_empty(), "{:?}", by_time(&b_log));
}

// ---- A2. 리뷰 3 지적 2 주변(패닉 순서) ----

/// 관문으로 붙잡은 첫 명령 + 앞 명령 `before`개, 패닉, 뒤 명령 하나. 앞의 마지막 노트(60)는 패닉이 끄고
/// 뒤의 노트(61)는 울린다.
fn panic_between(before: usize) {
    let (port, log) = rec();
    let (g, gh) = gate();
    let mut s = Scheduler::start(
        snap_on(&[port, g], bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    gh.close();
    s.send_now(1, cc(1, 0)).unwrap();
    gh.wait_entered();
    for _ in 0..before - 2 {
        s.send_now(0, cc(1, 0)).unwrap();
    }
    s.send_now(0, on(60)).unwrap();
    s.panic().unwrap();
    s.send_now(0, on(61)).unwrap();
    gh.open();
    sleep_ms(50);
    // 보낸 순서(기록 순서)로 본다.
    let sent = log.lock().unwrap().clone();
    let pos = |f: &dyn Fn(&[u8]) -> bool| sent.iter().position(|(_, b)| f(b));
    let panic_at = pos(&|b| b[0] & 0xF0 == 0xB0 && b[1] == 123).expect("패닉 CC 123 없음");
    let on60 = pos(&|b| b[0] & 0xF0 == 0x90 && b[1] == 60).expect("노트 60 없음");
    let on61 = pos(&|b| b[0] & 0xF0 == 0x90 && b[1] == 61).expect("노트 61 없음");
    assert!(on60 < panic_at, "앞 노트가 패닉 뒤에 감");
    assert!(on61 > panic_at, "뒤 노트가 패닉 앞에 감");
    assert_eq!(note_ons(&log, 60).len(), 1);
    assert_eq!(note_ons(&log, 61).len(), 1);
    assert_eq!(note_offs(&log, 60), 1, "앞 노트를 패닉이 안 끔");
    assert_eq!(
        note_offs(&log, 61),
        0,
        "뒤 노트를 패닉이 끔: {:?}",
        by_time(&log)
    );
}

/// 앞 명령이 정확히 한 바퀴 처리량(64)이면 패닉은 그 바퀴 끝에, 뒤 명령은 다음 바퀴에 간다.
#[test]
fn panic_after_exactly_one_wake_of_commands_orders_both_sides() {
    panic_between(64);
}

/// 앞 명령이 두 바퀴에 걸치면(70) 패닉은 둘째 바퀴 가운데서 앞뒤를 가른다.
#[test]
fn panic_in_middle_of_second_wake_orders_both_sides() {
    panic_between(70);
}

/// 큐 머리가 교체로 막힌 동안의 패닉은 기다리지 않고 바로 간다(리뷰 1 지적 9). 막힘이 풀린 뒤 처리되는
/// 패닉 뒤 명령의 노트는 끄지 않는다.
#[test]
fn panic_while_swap_blocks_queue_head_is_sent_and_spares_later_note() {
    let (port, log) = rec();
    let mk = || snap_on(&[Arc::clone(&port)], bpm(120), vec![]);
    let mut s = Scheduler::start(mk(), SchedulerConfig::default()).unwrap();
    // 돌려받기 큐(256) + 대기 자리(4)를 채우고, 261번째 교체가 큐 머리를 막는다.
    for _ in 0..261 {
        while s.swap(mk()).is_err() {
            sleep_ms(1);
        }
    }
    sleep_ms(30);
    s.send_now(0, on(60)).unwrap();
    s.panic().unwrap();
    s.send_now(0, on(61)).unwrap();
    sleep_ms(30);
    let panic_ccs = by_time(&log)
        .into_iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xB0 && b[1] == 123)
        .count();
    assert_eq!(panic_ccs, 16, "막힌 동안 패닉이 안 감: {:?}", by_time(&log));
    assert!(
        note_ons(&log, 61).is_empty(),
        "막힌 교체 뒤 명령이 먼저 처리됨"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut returned = 0;
    while returned < 261 && Instant::now() < deadline {
        returned += s.collect_garbage();
        sleep_ms(5);
    }
    sleep_ms(30);
    assert_eq!(note_ons(&log, 61).len(), 1, "{:?}", by_time(&log));
    assert_eq!(note_offs(&log, 61), 0, "패닉 뒤 노트를 끔");
    s.collect_garbage();
    drop_within(s, Duration::from_secs(2));
}

// ---- A3. 리뷰 3 지적 3 주변(상한에 걸린 바퀴, Clock, 보낸 구간) ----

/// 틱 0의 이벤트가 정확히 상한(4096)을 채워 다음 항목이 틱 0의 Clock인 바퀴 뒤에 교체한다. 이벤트는 한
/// 번씩, Clock은 빠짐·겹침 없이 간다. 남은 틱 0의 Clock은 창 끝까지 늦는다(제안 2, 실측 약 9 ms).
#[test]
fn swap_after_pass_capped_right_before_clock_sends_events_and_clocks_once() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let events = || {
        let mut v = vec![ScheduledEvent::new(Tick::ZERO, 1, cc(1, 0))];
        v.extend(burst_at(0, 4095));
        v
    };
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    gh.close();
    s.play(Tick::ZERO).unwrap();
    gh.wait_entered();
    s.swap(snap_on(&ports, bpm(120), events())).unwrap();
    gh.open();
    sleep_ms(200);
    s.stop().unwrap();
    sleep_ms(20);
    let ids = burst_ids(&log);
    let unique: BTreeSet<u64> = ids.iter().copied().collect();
    assert_eq!(unique.len(), 4095);
    assert_eq!(ids.len(), 4095, "틱 0 이벤트가 겹쳐 감");
    assert_clock_grid(&log, 0xFA, 1);
}

/// 같은 틱 중간(6000개 중 4096)에서 끊긴 바퀴 뒤 교체, Clock 켬. 이벤트는 빠짐없이(겹침은 허용), Clock은
/// 빠짐·겹침 없이 간다(첫 Clock은 늦을 수 있다, 제안 2).
#[test]
fn swap_after_pass_capped_mid_tick_with_clock_keeps_clock_grid() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let events = || {
        let mut v = vec![ScheduledEvent::new(Tick::ZERO, 1, cc(1, 0))];
        v.extend(burst_at(0, 6000));
        v
    };
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    gh.close();
    s.play(Tick::ZERO).unwrap();
    gh.wait_entered();
    s.swap(snap_on(&ports, bpm(120), events())).unwrap();
    gh.open();
    sleep_ms(200);
    s.stop().unwrap();
    sleep_ms(20);
    let unique: BTreeSet<u64> = burst_ids(&log).into_iter().collect();
    assert_eq!(unique.len(), 6000);
    assert_clock_grid(&log, 0xFA, 1);
}

/// 같은 틱 뒤쪽(틱 40)에서 끊긴 바퀴 뒤 교체. 그 틱의 Clock(40)은 이벤트 뒤에 한 번 간다(늦을 수
/// 있다, 제안 2).
#[test]
fn swap_after_pass_capped_at_later_tick_keeps_clock_on_that_tick() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let events = || {
        let mut v = vec![ScheduledEvent::new(Tick::new(40), 1, cc(1, 0))];
        v.extend(burst_at(40, 6000));
        v
    };
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    gh.close();
    s.play(Tick::ZERO).unwrap();
    gh.wait_entered();
    s.swap(snap_on(&ports, bpm(120), events())).unwrap();
    gh.open();
    sleep_ms(200);
    s.stop().unwrap();
    sleep_ms(20);
    let unique: BTreeSet<u64> = burst_ids(&log).into_iter().collect();
    assert_eq!(unique.len(), 6000);
    assert_clock_grid(&log, 0xFA, 2);
}

/// Clock을 켜고 재생하는 동안 0.7–2.7 ms 간격으로 교체를 100번 한다. 다음 항목이 Clock인 순간에 교체가
/// 걸려도 Clock은 빠짐·겹침 없이 간다.
#[test]
fn many_swaps_with_clock_on_keep_clock_grid() {
    let (port, log) = rec();
    let ports = [port];
    let events = || vec![ev(0, cc(1, 0)), ev(9600, on(60)), ev(9601, off(60))];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    for i in 0..100u64 {
        s.swap(snap_on(&ports, bpm(120), events())).unwrap();
        std::thread::sleep(Duration::from_micros(700 + (i * 313) % 2000));
        s.collect_garbage();
    }
    s.stop().unwrap();
    sleep_ms(20);
    assert_clock_grid(&log, 0xFA, 0);
}

/// 다음 이벤트가 먼 뒤(5 s)일 때, 보낸 구간 바로 뒤(교체 시각 + 25 ms)에 이벤트를 넣는 교체. 새 이벤트는
/// 한 번, 정확한 시각에 간다.
#[test]
fn swap_inserting_event_just_after_sent_window_sends_it_once_on_time() {
    let (port, log) = rec();
    let ports = [port];
    let base = || vec![ev(0, cc(1, 0)), ev(9600, on(70)), ev(9700, off(70))];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), base()),
        SchedulerConfig::default(),
    )
    .unwrap();
    let t0 = Instant::now();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(100);
    // 120 BPM에서 1 ms = 1.92틱. 교체를 넣는 지금 + 25 ms(재생 시작 여유 2 ms는 덤).
    let elapsed = u64::try_from(t0.elapsed().as_micros()).unwrap();
    let tick = (elapsed + 25_000) * 192 / 100_000 + 1;
    let mut v = base();
    v.push(ev(tick, on(62)));
    v.push(ev(tick + 10, off(62)));
    s.swap(snap_on(&ports, bpm(120), v)).unwrap();
    sleep_ms(100);
    s.stop().unwrap();
    sleep_ms(20);
    let marker = status_times(&log, 0xB0)[0];
    let ons = note_ons(&log, 62);
    assert_eq!(
        ons.len(),
        1,
        "새 이벤트 {}번: {:?}",
        ons.len(),
        by_time(&log)
    );
    let expect = u128::from(tick) * 500_000_000 / 960;
    let got = ns_between(marker, ons[0]);
    assert!(
        got.abs_diff(expect) < 1_000,
        "틱 {tick}: {got} ns, 기대 {expect} ns"
    );
    assert_eq!(note_offs(&log, 62), 1);
}
