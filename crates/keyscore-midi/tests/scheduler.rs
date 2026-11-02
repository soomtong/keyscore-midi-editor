//! S1-T4 스케줄러 루프백 통합 테스트.
//!
//! 가상 출력 포트로 보내고, 같은 프로세스의 `coremidi` crate 수신기가 받은 순간의 호스트 시각을 잰다.
//! NF-1·NF-3·NF-4 수치는 표준 출력에 적는다(`--nocapture`로 본다).
//!
//! 긴 판: `KS_LONG_TESTS=1`이면 지터·드리프트를 10분, `KS_LONG_TESTS=hour`이면 1시간 잰다.

use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
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

/// 이 바이너리의 시험은 서로 직렬로 돈다. 20초 지터·드리프트 측정이 다른 루프백 시험과 겹치면 같은
/// 프로세스의 CoreMIDI 수신 스레드가 바빠져 p99가 0.5 ms를 넘곤 했다(HANDOFF 후속, S3-T8). 앞 시험이
/// 실패해도 다음 시험은 돌아야 하므로 독(poison)은 무시한다.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
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
                // CoreMIDI는 같은 타임스탬프의 메시지를 한 패킷으로 묶어 준다. 메시지마다 나눈다.
                for msg in split(p.data()) {
                    let _ = tx.send((arrived, p.timestamp(), msg));
                }
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

/// MIDI 1.0 바이트 열을 메시지로 나눈다(running status 없음).
fn split(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let len = match (data[i] & 0xF0, data[i]) {
            (0xC0 | 0xD0, _) | (0xF0, 0xF1 | 0xF3) => 2,
            (0xF0, 0xF2) => 3,
            (0xF0, _) => 1,
            _ => 3,
        };
        out.push(data[i..(i + len).min(data.len())].to_vec());
        i += len;
    }
    out
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

/// 두 호스트 시각 사이(ms). `HostTime::duration_since`로 정확히 바꾼다.
fn ms_between(earlier: u64, later: u64) -> f64 {
    HostTime::from_ticks(later)
        .duration_since(HostTime::from_ticks(earlier))
        .as_secs_f64()
        * 1000.0
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
    let _serial = serial();
    let lb = loopback("times");
    let mut s = Scheduler::start(snapshot(&lb, sixteenths(8)), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    let got = collect(&lb.rx, 16, Duration::from_secs(3));
    assert_eq!(got.len(), 16);
    // 패킷 타임스탬프 간격은 계산한 시각 그대로다(120 BPM: 60틱 = 31.25 ms, 180틱 = 93.75 ms).
    for pair in got.windows(2) {
        let gap_ms = ms_between(pair[0].1, pair[1].1);
        assert!(
            (gap_ms - 31.25).abs() < 0.001 || (gap_ms - 93.75).abs() < 0.001,
            "{gap_ms}"
        );
    }
    // 노트는 늦지 않게, 예약 시각 근처에 도착한다.
    for (arrived, stamp, _) in &got {
        let late_ms = ms_between(*stamp, *arrived);
        assert!(late_ms < 2.0, "{late_ms}");
    }
}

#[test]
fn send_now_latency_p99_within_3ms() {
    let _serial = serial();
    let lb = loopback("nf1");
    let mut s = Scheduler::start(snapshot(&lb, vec![]), SchedulerConfig::default()).unwrap();
    let mut lat = Vec::new();
    for i in 0..200u8 {
        let sent = HostTime::now().ticks();
        s.send_now(0, note_on(ch(1), pitch(i % 120), vel()))
            .unwrap();
        let (arrived, _, _) = lb.rx.recv_timeout(Duration::from_secs(1)).unwrap();
        lat.push(ms_between(sent, arrived));
        std::thread::sleep(Duration::from_millis(5));
    }
    let p = p99(lat);
    println!("NF-1 명령 투입 → 도착 p99: {p:.3} ms");
    assert!(p <= 3.0, "{p}");
}

/// (간격 편차 p99, 도착 기준 드리프트, 노트 수). 표준편차와 패킷 타임스탬프 기준 드리프트도 출력한다.
///
/// REQ NF-3은 「송신 간격 표준편차」, NF-4는 「모노토닉 클록 vs 누적 틱 시간」이다. 도착 시각에는 수신
/// 쪽 지터가 섞이므로, 스케줄러가 정한 패킷 타임스탬프 기준 값을 함께 적어 둘을 구분한다(리뷰 1 제안 16).
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
    let ons: Vec<(u64, u64)> = got
        .iter()
        .filter(|(_, _, b)| b[0] == 0x90)
        .map(|(a, stamp, _)| (*a, *stamp))
        .collect();
    let intervals: Vec<f64> = ons.windows(2).map(|w| ms_between(w[0].0, w[1].0)).collect();
    let deviations: Vec<f64> = intervals.iter().map(|i| (i - 125.0).abs()).collect();
    #[allow(clippy::cast_precision_loss)]
    let n = intervals.len() as f64;
    let mean = intervals.iter().sum::<f64>() / n;
    let std_dev = (intervals.iter().map(|i| (i - mean).powi(2)).sum::<f64>() / n).sqrt();
    #[allow(clippy::cast_precision_loss)]
    let expected = 125.0 * (ons.len() as f64 - 1.0);
    let last = ons.len() - 1;
    let drift = (ms_between(ons[0].0, ons[last].0) - expected).abs();
    let stamp_drift = (ms_between(ons[0].1, ons[last].1) - expected).abs();
    println!(
        "NF-3 간격 표준편차: {std_dev:.4} ms, NF-4 타임스탬프 기준 드리프트: {stamp_drift:.6} ms"
    );
    (p99(deviations), drift, ons.len())
}

#[test]
fn playback_jitter_p99_within_half_ms_short() {
    let _serial = serial();
    let (jitter, drift, n) = jitter_and_drift("nf3");
    println!("NF-3 간격 편차 p99: {jitter:.4} ms (노트 {n}개), NF-4 드리프트: {drift:.4} ms");
    assert!(jitter <= 0.5, "{jitter}");
}

#[test]
fn playback_drift_within_1ms_short() {
    let _serial = serial();
    let (_, drift, n) = jitter_and_drift("nf4");
    println!("NF-4 드리프트: {drift:.4} ms (노트 {n}개)");
    assert!(drift <= 1.0, "{drift}");
}

#[test]
fn swap_during_play_stops_old_events_and_releases_notes() {
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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

/// 음 번호가 노트마다 다른 열: `i`번째 노트는 틱 `i × gap`, 음 `20 + i`, 길이 `gap / 2`.
fn numbered(count: u8, gap: u64) -> Vec<ScheduledEvent> {
    (0..count)
        .flat_map(|i| {
            let t = u64::from(i) * gap;
            [
                ev(t, note_on(ch(1), pitch(20 + i), vel())),
                ev(
                    t + gap / 2,
                    note_off(ch(1), pitch(20 + i), Velocity::new(0)),
                ),
            ]
        })
        .collect()
}

/// 받은 Note On의 음 번호.
fn note_ons(got: &[Received]) -> Vec<u8> {
    got.iter()
        .filter(|(_, _, b)| b[0] & 0xF0 == 0x90 && b[2] > 0)
        .map(|(_, _, b)| b[1])
        .collect()
}

/// 호스트 시각 `start`에서 `after`가 지날 때까지 잔다(지났으면 곧바로 돌아온다).
fn sleep_until(start: u64, after: Duration) {
    let since = HostTime::now().duration_since(HostTime::from_ticks(start));
    std::thread::sleep(after.saturating_sub(since));
}

/// 일시정지가 돌려준 틱은 「OS에 아직 넘기지 않은 첫 틱」이다. lookahead(10 ms)로 이미 넘긴 노트 직전에
/// 눌러도 그 노트는 틱 앞쪽에 있고, 틱 이상의 노트는 하나도 오지 않는다(S3-T8).
#[test]
fn pause_returns_first_unsent_tick_after_lookahead() {
    let _serial = serial();
    let lb = loopback("pause-tick");
    // 120 BPM에서 480틱 = 250 ms. 넷째 노트(틱 1440)의 5 ms 앞에서 누른다.
    let mut s =
        Scheduler::start(snapshot(&lb, numbered(8, 480)), SchedulerConfig::default()).unwrap();
    for round in 0..5 {
        s.play(Tick::ZERO).unwrap();
        let first = collect(&lb.rx, 1, Duration::from_secs(1));
        assert_eq!(note_ons(&first), vec![20], "{round}회차 첫 노트");
        sleep_until(first[0].1, Duration::from_millis(3 * 250 - 5));
        let at = s
            .pause(Duration::from_millis(50))
            .unwrap()
            .expect("재생 중이었다");
        let got = collect(&lb.rx, 64, Duration::from_millis(60));
        let heard = note_ons(&got);
        // 넷째 노트(틱 1440, 음 23)는 lookahead로 이미 넘겼다.
        assert_eq!(
            heard.last(),
            Some(&23),
            "{round}회차 들은 노트 {heard:?}, 자리 {at:?}"
        );
        assert!(
            at.get() > 1440,
            "{round}회차 자리 {at:?}가 이미 넘긴 노트(틱 1440) 앞이다"
        );
        assert!(
            at.get() < 1920,
            "{round}회차 자리 {at:?}가 넘기지 않은 노트(틱 1920) 뒤다"
        );
    }
}

/// 일시정지와 이어서 재생을 거듭해도 노트가 겹치거나 빠지지 않는다.
#[test]
fn pause_resume_neither_repeats_nor_drops_notes() {
    let _serial = serial();
    let lb = loopback("pause-resume");
    // 60틱(31.25 ms)마다 음 번호가 다른 노트 64개, 2초.
    let mut s =
        Scheduler::start(snapshot(&lb, numbered(64, 60)), SchedulerConfig::default()).unwrap();
    let mut from = Tick::ZERO;
    let mut heard = Vec::new();
    for i in 0..12u64 {
        s.play(from).unwrap();
        // 자리마다 노트 경계와 다른 곳에 떨어지도록 재생 길이를 바꾼다.
        std::thread::sleep(Duration::from_millis(97 + i * 7));
        from = s
            .pause(Duration::from_millis(50))
            .unwrap()
            .expect("재생 중이었다");
        heard.extend(note_ons(&collect(&lb.rx, 256, Duration::from_millis(40))));
    }
    s.play(from).unwrap();
    heard.extend(note_ons(&collect(&lb.rx, 256, Duration::from_secs(3))));
    let expected: Vec<u8> = (20..84).collect();
    assert_eq!(heard, expected, "겹치거나 빠졌다");
}

/// 정지 중의 일시정지는 정지와 같고 이어서 재생할 자리가 없다.
#[test]
fn pause_when_stopped_returns_none() {
    let _serial = serial();
    let lb = loopback("pause-stopped");
    let mut s =
        Scheduler::start(snapshot(&lb, numbered(4, 480)), SchedulerConfig::default()).unwrap();
    assert_eq!(s.pause(Duration::from_millis(50)).unwrap(), None);
    assert!(collect(&lb.rx, 1, Duration::from_millis(50)).is_empty());
}
