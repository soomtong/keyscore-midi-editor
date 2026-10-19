//! S1-T4 적대적 리뷰 3회차 테스트(리뷰 2 수정 `54be828` 대상).
//!
//! 리뷰 2의 기록 포트·관문 포트·세는 포트를 그대로 쓴다. 모두 이 파일 안의 `MidiPort` 구현이라 CoreMIDI
//! 클라이언트를 만들지 않는다. 관문 포트는 스케줄러 스레드를 `send` 안에서 붙잡아, 그동안 넣은 명령이
//! 같은 바퀴에서 처리되게 한다. 이 바이너리는 `assert_no_alloc` 전역 할당자를 두지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    fn name(&self) -> String {
        "rec".to_owned()
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

/// 상태 바이트별 송신 수만 세는 포트(할당 없음).
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

fn counter() -> (Arc<dyn MidiPort>, Arc<AtomicU64>, Arc<AtomicU64>) {
    let clocks = Arc::new(AtomicU64::new(0));
    let notes = Arc::new(AtomicU64::new(0));
    let port: Arc<dyn MidiPort> = Arc::new(Counter {
        clocks: Arc::clone(&clocks),
        notes: Arc::clone(&notes),
    });
    (port, clocks, notes)
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

fn note_offs(log: &Log, pitch: u8) -> usize {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| {
            b[1] == pitch && (b[0] & 0xF0 == 0x80 || (b[0] & 0xF0 == 0x90 && b[2] == 0))
        })
        .count()
}

/// 스케줄러를 다른 스레드에서 drop하고 `limit` 안에 끝나는지 본다. 끝나지 않으면 스레드를 남기고 실패한다.
fn drop_within(s: Scheduler, limit: Duration) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        drop(s);
        let _ = tx.send(());
    });
    assert!(
        rx.recv_timeout(limit).is_ok(),
        "drop이 {limit:?} 안에 끝나지 않음(스케줄러 스레드가 quit을 못 본다)"
    );
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

/// 틱 0에 몰린 CC `count`개. k번째는 CC 20 + k/2048, 채널 (k/128)%16 + 1, 값 k%128이라 받은 메시지에서
/// k를 되찾을 수 있다.
fn burst_at_zero(count: u64) -> Vec<ScheduledEvent> {
    (0..count)
        .map(|k| {
            let c = u8::try_from((k / 128) % 16 + 1).unwrap();
            let n = u8::try_from(20 + k / 2048).unwrap();
            let v = u32::try_from(k % 128).unwrap();
            ev(0, control_change(ch(c), n, v << 25).unwrap())
        })
        .collect()
}

fn burst_ids(log: &Log) -> BTreeSet<u64> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xB0 && (20..30).contains(&b[1]))
        .map(|(_, b)| u64::from(b[1] - 20) * 2048 + u64::from(b[0] & 0x0F) * 128 + u64::from(b[2]))
        .collect()
}

// ---- 깨뜨린 사례(현재 구현에서 실패한다) ----

/// 지적 1(리뷰 2 지적 3의 정지 중 경로): 정지 중 교체는 스냅샷만 바꾼다. 미리듣기로 옛 포트 A에 켠 노트가
/// 울리는 채로 같은 번호에 다른 포트 B가 온 스냅샷으로 바꾸면, 뒤의 `stop`은 B로 Note Off를 보내고 A의
/// 노트는 걸린다. 리뷰 2가 「정지 중 교체도 같은 문제」라고 적은 경로다.
#[test]
fn swap_while_stopped_to_different_port_releases_preview_note_on_old_port() {
    let (old_port, old_log) = rec();
    let (new_port, new_log) = rec();
    let mut s = Scheduler::start(
        snap_on(&[old_port], bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.send_now(0, on(60)).unwrap();
    sleep_ms(20);
    s.swap(snap_on(&[new_port], bpm(120), vec![])).unwrap();
    sleep_ms(20);
    s.collect_garbage();
    s.stop().unwrap();
    sleep_ms(20);
    drop_within(s, Duration::from_secs(2));
    let left = sounding_at_end(&old_log);
    assert!(
        left.is_empty(),
        "옛 포트에 걸린 노트 {left:?}(옛 포트 {:?}, 새 포트 {:?})",
        by_time(&old_log),
        by_time(&new_log)
    );
}

/// 지적 1(포트 수가 줄어든 경우): 정지 중 포트 [A, B]에서 B로 미리듣기 노트를 켜고 [A]로 바꾸면,
/// `release_notes`는 새 스냅샷의 포트(A)만 돌고 `active.fill(0)`으로 B의 노트를 잊는다.
#[test]
fn swap_while_stopped_to_fewer_ports_releases_preview_note_on_removed_port() {
    let (a, _a_log) = rec();
    let (b, b_log) = rec();
    let mut s = Scheduler::start(
        snap_on(&[Arc::clone(&a), b], bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.send_now(1, on(60)).unwrap();
    sleep_ms(20);
    s.swap(snap_on(&[a], bpm(120), vec![])).unwrap();
    sleep_ms(20);
    s.collect_garbage();
    s.stop().unwrap();
    sleep_ms(20);
    drop_within(s, Duration::from_secs(2));
    let left = sounding_at_end(&b_log);
    assert!(
        left.is_empty(),
        "없어진 포트에 걸린 노트 {left:?}: {:?}",
        by_time(&b_log)
    );
}

/// 지적 2(리뷰 2 지적 4 수정의 반대 방향): 패닉 플래그는 「패닉 때까지 넣은 명령 수」를 넘으면 적용되는데,
/// 같은 바퀴에서 패닉 뒤에 넣은 명령까지 먼저 처리한 뒤 적용된다. 패닉 뒤에 미리듣기로 친 노트가 패닉의
/// Note Off로 바로 꺼진다. 사용자가 패닉 직후 친 건반이 소리 나지 않는다.
#[test]
fn panic_does_not_silence_note_sent_after_it() {
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
    s.panic().unwrap();
    s.send_now(0, on(61)).unwrap();
    gh.open();
    sleep_ms(50);
    assert_eq!(note_ons(&log, 61).len(), 1, "{:?}", by_time(&log));
    assert_eq!(
        note_offs(&log, 61),
        0,
        "패닉 뒤에 친 노트를 패닉이 끔: {:?}",
        by_time(&log)
    );
}

/// 지적 3(리뷰 2 지적 1·2 수정과 지적 6 수정의 상호작용): 한 바퀴의 예약 항목을 4096개로 묶은 뒤에도
/// `schedule`은 `sent_through`를 창 끝(`tick_at(horizon)`)으로 둔다. 상한에 걸려 남은 이벤트가 있는 바퀴
/// 다음에 교체가 오면 그 이벤트들을 「보낸 것」으로 건너뛴다. 틱 하나에 이벤트가 4096개를 넘는 곡(포트
/// 32개 × 16채널의 초기화 CC 등)에서 재생 직후 편집하면 이벤트가 사라진다.
#[test]
fn swap_after_pass_capped_at_item_limit_loses_no_events() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let events = || {
        let mut v = vec![ScheduledEvent::new(Tick::ZERO, 1, cc(1, 0))];
        v.extend(burst_at_zero(6000));
        v
    };
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    gh.close();
    s.play(Tick::ZERO).unwrap();
    // 첫 바퀴의 첫 항목(관문 포트 CC)에서 붙잡혔다. 이 바퀴는 4096개에서 끊긴다.
    gh.wait_entered();
    s.swap(snap_on(&ports, bpm(120), events())).unwrap();
    gh.open();
    sleep_ms(100);
    s.stop().unwrap();
    sleep_ms(20);
    let ids = burst_ids(&log);
    assert_eq!(
        ids.len(),
        6000,
        "틱 0의 CC 6000개 중 {}개만 보냄",
        ids.len()
    );
}

// ---- 못 깨뜨린 경계(통과한다) ----

/// 리뷰 2 지적 3: 재생 중 포트 수가 줄어든 스냅샷으로 바꾸면 없어진 포트의 울리는 노트를 그 포트로 끈다.
#[test]
fn swap_during_play_to_fewer_ports_releases_note_on_removed_port() {
    let (a, _a_log) = rec();
    let (b, b_log) = rec();
    let events = vec![
        ScheduledEvent::new(Tick::ZERO, 1, on(60)),
        ScheduledEvent::new(Tick::new(1920), 1, off(60)),
    ];
    let mut s = Scheduler::start(
        snap_on(&[Arc::clone(&a), b], bpm(120), events),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(100);
    assert_eq!(note_ons(&b_log, 60).len(), 1);
    s.swap(snap_on(&[a], bpm(120), vec![])).unwrap();
    sleep_ms(50);
    s.stop().unwrap();
    sleep_ms(20);
    drop_within(s, Duration::from_secs(2));
    assert!(sounding_at_end(&b_log).is_empty(), "{:?}", by_time(&b_log));
}

/// 리뷰 2 지적 3: 같은 포트 객체를 그대로 둔 교체는 이어 울리는 노트를 끊지 않는다(같은 포트 판정의 대조군).
#[test]
fn swap_with_same_port_object_keeps_sounding_note() {
    let (port, log) = rec();
    let events = || vec![ev(0, on(60)), ev(960, off(60))];
    let ports = [port];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(100);
    s.swap(snap_on(&ports, bpm(120), events())).unwrap();
    sleep_ms(600);
    s.stop().unwrap();
    sleep_ms(20);
    let on_at = note_ons(&log, 60)[0];
    let offs: Vec<u64> = by_time(&log)
        .into_iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0x80 && b[1] == 60)
        .map(|(t, _)| t)
        .collect();
    assert_eq!(offs.len(), 1, "{:?}", by_time(&log));
    assert!(ns_between(on_at, offs[0]).abs_diff(500_000_000) < 1_000);
}

/// 리뷰 2 지적 1: 위치 이동과 교체가 같은 바퀴에서 처리돼도 새 위치의 이벤트를 한 번 보낸다.
#[test]
fn swap_in_same_wake_as_locate_keeps_event_at_new_position() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let events = || vec![ev(960, on(60)), ev(1440, off(60))];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), events()),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(30);
    gh.close();
    s.send_now(1, cc(1, 0)).unwrap();
    gh.wait_entered();
    s.locate(Tick::new(960)).unwrap();
    s.swap(snap_on(&ports, bpm(120), events())).unwrap();
    gh.open();
    sleep_ms(400);
    s.stop().unwrap();
    sleep_ms(20);
    assert_eq!(note_ons(&log, 60).len(), 1, "{:?}", by_time(&log));
    assert!(sounding_at_end(&log).is_empty());
}

/// 리뷰 2 지적 1(Clock): 위치 이동과 교체가 같은 바퀴면 Continue와 첫 Clock이 같은 시각이다.
#[test]
fn swap_in_same_wake_as_locate_keeps_first_clock_with_continue() {
    let (port, log) = rec();
    let (g, gh) = gate();
    let ports = [port, g];
    let mut s = Scheduler::start(
        snap_on(&ports, bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(30);
    gh.close();
    s.send_now(1, cc(1, 0)).unwrap();
    gh.wait_entered();
    s.locate(Tick::new(960)).unwrap();
    s.swap(snap_on(&ports, bpm(120), vec![])).unwrap();
    gh.open();
    sleep_ms(100);
    s.stop().unwrap();
    sleep_ms(20);
    let msgs = by_time(&log);
    let cont = msgs.iter().position(|(_, b)| b[0] == 0xFB).unwrap();
    assert_eq!(msgs[cont - 1].1, vec![0xF2, 4, 0], "{msgs:?}");
    let first_clock = msgs[cont..].iter().find(|(_, b)| b[0] == 0xF8).unwrap().0;
    assert_eq!(first_clock, msgs[cont].0, "Continue 뒤 첫 Clock이 늦음");
}

/// 리뷰 2 지적 4: 한 바퀴 처리량(64개)보다 많은 명령 뒤의 패닉도 그 명령들을 다 처리한 뒤 적용된다.
#[test]
fn panic_after_more_than_one_wake_of_commands_silences_last_note() {
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
    for _ in 0..100 {
        s.send_now(0, cc(7, 100)).unwrap();
    }
    s.send_now(0, on(61)).unwrap();
    s.panic().unwrap();
    gh.open();
    sleep_ms(50);
    assert_eq!(note_ons(&log, 61).len(), 1);
    let left = sounding_at_end(&log);
    assert!(left.is_empty(), "패닉 뒤 울리는 노트 {left:?}");
}

/// 리뷰 2 지적 5·6: `u64::MAX` 근처에서 Clock을 켜고 루프를 돌아도 스레드가 명령을 받고 drop이 끝난다.
/// Clock은 40의 배수 틱(`u64::MAX - 15`)에서만 간다.
#[test]
fn play_near_max_tick_with_clock_and_loop_stays_responsive() {
    let (port, clocks, notes) = counter();
    let mut s = Scheduler::start(
        snap_on(&[port], bpm(120), vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    s.set_loop(Some((Tick::new(u64::MAX - 60), Tick::new(u64::MAX - 1))))
        .unwrap();
    s.play(Tick::new(u64::MAX - 60)).unwrap();
    sleep_ms(100);
    s.send_now(0, on(61)).unwrap();
    sleep_ms(100);
    assert_eq!(notes.load(Ordering::Relaxed), 1, "send_now가 처리되지 않음");
    // 한 바퀴(59틱, 약 31 ms)에 Clock 하나. 200 ms면 열 개 남짓이다.
    let n = clocks.load(Ordering::Relaxed);
    assert!(n < 50, "200 ms에 Clock {n}개");
    drop_within(s, Duration::from_secs(2));
}

/// 리뷰 2 지적 6: 호스트 틱보다 짧은 루프에서 Clock까지 켜도 스레드가 명령을 받고 drop이 끝난다.
#[test]
fn loop_shorter_than_one_host_tick_with_clock_keeps_scheduler_responsive() {
    let (port, _clocks, notes) = counter();
    let mut s = Scheduler::start(
        snap_on(&[port], micros(1), vec![ev(0, cc(7, 1))]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_clock(0, true).unwrap();
    s.set_loop(Some((Tick::ZERO, Tick::new(1)))).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(30);
    s.send_now(0, on(61)).unwrap();
    sleep_ms(100);
    assert_eq!(notes.load(Ordering::Relaxed), 1, "send_now가 처리되지 않음");
    drop_within(s, Duration::from_secs(2));
}

/// 리뷰 2 지적 7: RPN → NRPN → RPN 순서로 고른 곡에서 chase는 마지막(RPN)을 데이터 엔트리 바로 앞에 둔다.
#[test]
fn chase_with_rpn_nrpn_rpn_writes_no_wrong_parameter() {
    let (port, log) = rec();
    let stream: Vec<(u64, u8, u8)> = vec![
        (0, 101, 0),
        (0, 100, 0),
        (0, 6, 2),
        (10, 99, 1),
        (10, 98, 8),
        (10, 6, 100),
        (20, 101, 0),
        (20, 100, 0),
        (20, 6, 12),
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
    let got = parameter_values(&chased);
    assert_eq!(got.get(&(false, 0, 0)), Some(&12), "{chased:?}");
    for (param, v) in got {
        assert_eq!(truth.get(&param), Some(&v), "chase CC: {chased:?}");
    }
}

/// 리뷰 2 지적 8: 템포 변화가 있는 맵(먼 뒤에서 바뀜)과 고정 템포 맵을 번갈아 1000번 바꿔도 박 격자가
/// 1 µs 안에 남는다. 두 맵은 재생 구간에서 같은 시각을 주므로 오차는 0이어야 한다.
#[test]
fn many_swaps_alternating_tempo_maps_keep_beat_grid_exact() {
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
    let plain = snap_on(&ports, bpm(120), events());
    let mut changing = bpm(120);
    changing.set(Tick::new(960 * 1000), Tempo::from_bpm(61).unwrap());
    let changing = snap_on(&ports, changing, events());
    for i in 0..1000 {
        let next = if i % 2 == 0 { &changing } else { &plain };
        s.swap(Arc::clone(next)).unwrap();
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
        "교체 1000번 뒤 박 격자에서 {worst} ns 벗어남"
    );
}
