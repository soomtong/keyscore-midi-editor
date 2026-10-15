//! S1-T4 스케줄러 루프백 통합 테스트.
//!
//! 가상 출력 포트로 보내고, 같은 프로세스의 `coremidi` crate 수신기가 받은 순간의 호스트 시각을 잰다.
//! NF-1·NF-3·NF-4 수치는 표준 출력에 적는다(`--nocapture`로 본다).
//!
//! 긴 판: `KS_LONG_TESTS=1`이면 지터·드리프트를 10분, `KS_LONG_TESTS=hour`이면 1시간 잰다.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::{Bank, Channel, Pitch, Program, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, Midi1Message, MidiClient, MidiPort, ScheduledEvent, Scheduler, SchedulerConfig,
    Snapshot, bank_select, control_change, note_off, note_on, program_change,
};

/// 받은 메시지: (도착 호스트 시각, 패킷 타임스탬프, 바이트).
type Received = (u64, u64, Vec<u8>);

struct Loopback {
    port: Arc<dyn MidiPort>,
    rx: mpsc::Receiver<Received>,
    // 수신 포트와 클라이언트는 테스트가 끝날 때까지 살아 있어야 한다.
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

/// 이 바이너리의 첫 CoreMIDI 클라이언트를 `keyscore-midi`가 만든다(알림 조건, S1-T2).
fn midi() -> MidiClient {
    MidiClient::new("ks-sched").unwrap()
}

fn unique(name: &str) -> String {
    format!("KS sched {} {name}", std::process::id())
}

fn loopback(name: &str) -> Loopback {
    let client = midi();
    let name = unique(name);
    let port: Arc<dyn MidiPort> = Arc::new(client.create_virtual_output(&name).unwrap());
    let (tx, rx) = mpsc::channel();
    let receiver = coremidi::Client::new("ks-sched-rx").unwrap();
    let input = receiver
        .input_port("in", move |list: &coremidi::PacketList| {
            let arrived = HostTime::now().ticks();
            for p in list {
                let _ = tx.send((arrived, p.timestamp(), p.data().to_vec()));
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let source = loop {
        if let Some(s) = coremidi::Source::from_name(&name) {
            break s;
        }
        assert!(Instant::now() < deadline, "소스가 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    };
    input.connect_source(&source).unwrap();
    Loopback {
        port,
        rx,
        _input: input,
        _client: receiver,
    }
}

fn ch(n: u8) -> Channel {
    Channel::new(n).unwrap()
}

fn pitch(n: u8) -> Pitch {
    Pitch::new(n).unwrap()
}

fn vel() -> Velocity {
    Velocity::from_midi1(100).unwrap()
}

fn tempo_120() -> TempoMap {
    TempoMap::new(Tempo::from_bpm(120).unwrap())
}

fn ev(tick: u64, msg: Midi1Message) -> ScheduledEvent {
    ScheduledEvent::new(Tick::new(tick), 0, msg)
}

fn snapshot(lb: &Loopback, events: Vec<ScheduledEvent>) -> Arc<Snapshot> {
    Arc::new(Snapshot::new(tempo_120(), vec![Arc::clone(&lb.port)], events).unwrap())
}

/// mach 틱 → ns 비율을 경과 시간으로 잰다(기기마다 다르다).
fn ticks_per_ms() -> f64 {
    let (t0, i0) = (HostTime::now(), Instant::now());
    std::thread::sleep(Duration::from_millis(50));
    let (t1, i1) = (HostTime::now(), Instant::now());
    #[allow(clippy::cast_precision_loss)]
    let r = (t1.ticks() - t0.ticks()) as f64 / i1.duration_since(i0).as_secs_f64() / 1000.0;
    r
}

/// 받은 메시지를 `limit` 동안 또는 `count`개까지 모은다.
fn collect(rx: &mpsc::Receiver<Received>, count: usize, limit: Duration) -> Vec<Received> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    while got.len() < count {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(r) => got.push(r),
            Err(_) => break,
        }
    }
    got
}

fn p99(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    // 올림한 99% 위치(1부터). 정수로 센다.
    let i = (values.len() * 99).div_ceil(100);
    values[i.clamp(1, values.len()) - 1]
}

fn long_duration() -> Duration {
    match std::env::var("KS_LONG_TESTS").as_deref() {
        Ok("hour") => Duration::from_hours(1),
        Ok(_) => Duration::from_mins(10),
        Err(_) => Duration::from_secs(20),
    }
}

/// 120 BPM에서 16분음표(240틱 = 125 ms)마다 짧은 노트를 둔다.
fn sixteenths(count: u64) -> Vec<ScheduledEvent> {
    (0..count)
        .flat_map(|i| {
            let t = i * 240;
            [
                ev(t, note_on(ch(1), pitch(60), vel())),
                ev(t + 60, note_off(ch(1), pitch(60), Velocity::new(0))),
            ]
        })
        .collect()
}

#[test]
fn play_sends_events_at_scheduled_host_times() {
    let lb = loopback("times");
    let mut s = Scheduler::start(snapshot(&lb, sixteenths(8)), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    let got = collect(&lb.rx, 16, Duration::from_secs(3));
    assert_eq!(got.len(), 16);
    let per_ms = ticks_per_ms();
    // 패킷 타임스탬프 간격은 계산한 시각 그대로다(정수 계산, 125 ms·30 ms).
    for pair in got.windows(2) {
        #[allow(clippy::cast_precision_loss)]
        let gap_ms = (pair[1].1 - pair[0].1) as f64 / per_ms;
        assert!(
            (gap_ms - 30.0).abs() < 0.05 || (gap_ms - 95.0).abs() < 0.05,
            "{gap_ms}"
        );
    }
    // 노트는 늦지 않게, 예약 시각 근처에 도착한다.
    for (arrived, stamp, _) in &got {
        #[allow(clippy::cast_precision_loss)]
        let late_ms = (*arrived as f64 - *stamp as f64) / per_ms;
        assert!(late_ms < 2.0, "{late_ms}");
    }
}

#[test]
fn send_now_latency_p99_within_3ms() {
    let lb = loopback("nf1");
    let mut s = Scheduler::start(snapshot(&lb, vec![]), SchedulerConfig::default()).unwrap();
    let per_ms = ticks_per_ms();
    let mut lat = Vec::new();
    for i in 0..200u8 {
        let sent = HostTime::now().ticks();
        s.send_now(0, note_on(ch(1), pitch(i % 120), vel()))
            .unwrap();
        let (arrived, _, _) = lb.rx.recv_timeout(Duration::from_secs(1)).unwrap();
        #[allow(clippy::cast_precision_loss)]
        lat.push((arrived - sent) as f64 / per_ms);
        std::thread::sleep(Duration::from_millis(5));
    }
    let p = p99(lat);
    println!("NF-1 명령 투입 → 도착 p99: {p:.3} ms");
    assert!(p <= 3.0, "{p}");
}

fn jitter_and_drift(name: &str) -> (f64, f64, usize) {
    let lb = loopback(name);
    let length = long_duration();
    let notes = length.as_millis() / 125;
    let notes = u64::try_from(notes).unwrap();
    let mut s =
        Scheduler::start(snapshot(&lb, sixteenths(notes)), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    let got = collect(
        &lb.rx,
        usize::try_from(notes * 2).unwrap(),
        length + Duration::from_secs(5),
    );
    let ons: Vec<u64> = got
        .iter()
        .filter(|(_, _, b)| b[0] == 0x90)
        .map(|(a, _, _)| *a)
        .collect();
    let per_ms = ticks_per_ms();
    #[allow(clippy::cast_precision_loss)]
    let deviations: Vec<f64> = ons
        .windows(2)
        .map(|w| ((w[1] - w[0]) as f64 / per_ms - 125.0).abs())
        .collect();
    #[allow(clippy::cast_precision_loss)]
    let expected = 125.0 * (ons.len() as f64 - 1.0);
    #[allow(clippy::cast_precision_loss)]
    let drift = ((ons[ons.len() - 1] - ons[0]) as f64 / per_ms - expected).abs();
    (p99(deviations), drift, ons.len())
}

#[test]
fn playback_jitter_p99_within_half_ms_short() {
    let (jitter, drift, n) = jitter_and_drift("nf3");
    println!("NF-3 간격 편차 p99: {jitter:.4} ms (노트 {n}개), NF-4 드리프트: {drift:.4} ms");
    assert!(jitter <= 0.5, "{jitter}");
}

#[test]
fn playback_drift_within_1ms_short() {
    let (_, drift, n) = jitter_and_drift("nf4");
    println!("NF-4 드리프트: {drift:.4} ms (노트 {n}개)");
    assert!(drift <= 1.0, "{drift}");
}

#[test]
fn swap_during_play_stops_old_events_and_releases_notes() {
    let lb = loopback("swap");
    // 옛 스냅샷: 0틱에 긴 노트(울리는 채로 둔다), 그 뒤 960마다 음 62.
    let mut old = vec![ev(0, note_on(ch(1), pitch(48), vel()))];
    old.extend((1..20).map(|i| ev(i * 960, note_on(ch(1), pitch(62), vel()))));
    let mut s = Scheduler::start(snapshot(&lb, old), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    let first = collect(&lb.rx, 1, Duration::from_secs(1));
    assert_eq!(first[0].2, vec![0x90, 48, 100]);
    std::thread::sleep(Duration::from_millis(100));
    // 새 스냅샷: 음 64만.
    let new = (0..40)
        .map(|i| ev(i * 960, note_on(ch(1), pitch(64), vel())))
        .collect();
    s.swap(snapshot(&lb, new)).unwrap();
    let after = collect(&lb.rx, 4, Duration::from_secs(3));
    // 교체 직후 울리던 48을 끈다. 그 뒤에는 옛 이벤트(62)가 오지 않는다.
    assert_eq!(after[0].2, vec![0x80, 48, 0], "{after:?}");
    assert!(after[1..].iter().all(|(_, _, b)| b[1] == 64), "{after:?}");
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(s.collect_garbage(), 1);
}

#[test]
fn stop_locate_and_loop_release_sounding_notes() {
    let lb = loopback("release");
    let events = vec![
        ev(0, note_on(ch(2), pitch(50), vel())),
        ev(9600, note_off(ch(2), pitch(50), Velocity::new(0))),
    ];
    let mut s = Scheduler::start(snapshot(&lb, events), SchedulerConfig::default()).unwrap();
    // 정지
    s.play(Tick::ZERO).unwrap();
    assert_eq!(
        collect(&lb.rx, 1, Duration::from_secs(1))[0].2,
        vec![0x91, 50, 100]
    );
    s.stop().unwrap();
    assert_eq!(
        collect(&lb.rx, 1, Duration::from_secs(1))[0].2,
        vec![0x81, 50, 0]
    );
    // 위치 이동
    s.play(Tick::ZERO).unwrap();
    assert_eq!(
        collect(&lb.rx, 1, Duration::from_secs(1))[0].2,
        vec![0x91, 50, 100]
    );
    s.locate(Tick::new(4800)).unwrap();
    assert_eq!(
        collect(&lb.rx, 1, Duration::from_secs(1))[0].2,
        vec![0x81, 50, 0]
    );
    s.stop().unwrap();
    // 루프 되감기: 0..480 루프면 노트가 끝나기 전에 되감으므로 매번 끄고 다시 친다.
    s.set_loop(Some((Tick::ZERO, Tick::new(480)))).unwrap();
    s.play(Tick::ZERO).unwrap();
    let got = collect(&lb.rx, 5, Duration::from_secs(2));
    let kinds: Vec<u8> = got.iter().map(|(_, _, b)| b[0]).collect();
    assert_eq!(kinds, vec![0x91, 0x81, 0x91, 0x81, 0x91], "{got:?}");
}

#[test]
fn play_from_middle_chases_program_bank_and_cc() {
    let lb = loopback("chase");
    let [msb, lsb] = bank_select(ch(3), Bank::new(130).unwrap());
    let events = vec![
        ev(0, msb),
        ev(0, lsb),
        ev(0, program_change(ch(3), Program::new(5).unwrap())),
        ev(0, control_change(ch(3), 7, 0x4000_0000).unwrap()),
        ev(480, control_change(ch(3), 7, 0x7FFF_FFFF).unwrap()),
        ev(480, program_change(ch(3), Program::new(9).unwrap())),
        ev(1920, note_on(ch(3), pitch(70), vel())),
    ];
    let mut s = Scheduler::start(snapshot(&lb, events), SchedulerConfig::default()).unwrap();
    s.play(Tick::new(960)).unwrap();
    let got: Vec<Vec<u8>> = collect(&lb.rx, 5, Duration::from_secs(2))
        .into_iter()
        .map(|(_, _, b)| b)
        .collect();
    // 마지막 값으로: 뱅크 MSB·LSB, 프로그램, CC, 그다음 이후 이벤트.
    assert_eq!(
        got,
        vec![
            vec![0xB2, 0, 1],
            vec![0xB2, 32, 2],
            vec![0xC2, 9],
            vec![0xB2, 7, 63],
            vec![0x92, 70, 100],
        ]
    );
}

#[test]
fn panic_sends_all_sound_off_notes_off_sustain_to_every_port_channel() {
    let lb = loopback("panic");
    let mut s = Scheduler::start(snapshot(&lb, vec![]), SchedulerConfig::default()).unwrap();
    s.panic().unwrap();
    let got = collect(&lb.rx, 48, Duration::from_secs(2));
    assert_eq!(got.len(), 48);
    for (i, (_, _, b)) in got.iter().enumerate() {
        let chan = u8::try_from(i / 3).unwrap();
        let cc = [120, 123, 64][i % 3];
        assert_eq!(b, &vec![0xB0 | chan, cc, 0]);
    }
}

#[test]
fn clock_sends_24_ppqn_with_start_spp_continue_stop() {
    let lb = loopback("clock");
    let mut s = Scheduler::start(snapshot(&lb, vec![]), SchedulerConfig::default()).unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    // 120 BPM 한 박(500 ms) 동안 Start 하나와 Clock 24개(틱 0, 40, … 920).
    std::thread::sleep(Duration::from_millis(505));
    s.stop().unwrap();
    let got: Vec<u8> = collect(&lb.rx, 100, Duration::from_millis(300))
        .into_iter()
        .map(|(_, _, b)| b[0])
        .collect();
    assert_eq!(got[0], 0xFA);
    #[allow(clippy::naive_bytecount)] // 테스트에서 몇십 바이트를 세는 데 의존을 더하지 않는다.
    let clocks = got.iter().filter(|b| **b == 0xF8).count();
    assert!((24..=26).contains(&clocks), "{clocks}");
    assert_eq!(*got.last().unwrap(), 0xFC);
    // 중간에서 시작하면 SPP 뒤 Continue. 960틱 = 16분음표 4개.
    s.play(Tick::new(960)).unwrap();
    let got = collect(&lb.rx, 2, Duration::from_secs(1));
    assert_eq!(got[0].2, vec![0xF2, 4, 0]);
    assert_eq!(got[1].2, vec![0xFB]);
}

#[test]
fn garbage_snapshots_are_returned_not_dropped_on_scheduler_thread() {
    let lb = loopback("garbage");
    let first = snapshot(&lb, vec![]);
    let weak = Arc::downgrade(&first);
    let mut s = Scheduler::start(first, SchedulerConfig::default()).unwrap();
    s.swap(snapshot(&lb, vec![])).unwrap();
    // 스케줄러가 돌려줄 때까지 옛 스냅샷은 살아 있다(스케줄러 스레드에서 해제되지 않는다).
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut returned = 0;
    while returned == 0 && Instant::now() < deadline {
        assert!(weak.upgrade().is_some(), "돌려받기 전에 해제됐다");
        std::thread::sleep(Duration::from_millis(5));
        returned = s.collect_garbage();
    }
    assert_eq!(returned, 1);
    assert!(weak.upgrade().is_none());
}
