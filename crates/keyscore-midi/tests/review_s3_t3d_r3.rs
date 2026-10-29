//! S3-T3d 적대적 리뷰 3회차의 깨뜨리기 시험(`docs/reports/s3-t3d/review-3.md`).
//!
//! 대상은 커밋 `6dfcc08`: 곡 끝 이후 시작의 클럭 맞추기를 `Rt::clock_resync`로 미루고 `schedule` 첫머리에서
//! 곡 끝이 아니면 맞춘다. 대기 중 `stop`·`play`·`locate`·`set_clock`, 교체가 곡 끝을 다시 줄이는 경우, 대기가
//! 여러 바퀴에 걸치는 경우(lookahead가 시작 여유 2 ms보다 짧을 때)를 본다.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::{Tempo, TempoMap, Tick};
use keyscore_midi::{
    HostTime, MidiClient, MidiPort, ScheduledEvent, Scheduler, SchedulerConfig, Snapshot,
};

/// 받은 메시지: (도착 호스트 시각, 바이트).
type Received = (u64, Vec<u8>);

struct Loopback {
    port: Arc<dyn MidiPort>,
    rx: mpsc::Receiver<Received>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

/// 이 바이너리의 첫 CoreMIDI 클라이언트를 `keyscore-midi`가 만든다(알림 조건, S1-T2).
fn midi() -> MidiClient {
    MidiClient::new("ks-review-t3d-r3").unwrap()
}

fn loopback(name: &str) -> Loopback {
    let client = midi();
    let name = format!("KS review t3d r3 {} {name}", std::process::id());
    let port: Arc<dyn MidiPort> = Arc::new(client.create_virtual_output(&name).unwrap());
    let (tx, rx) = mpsc::channel();
    let receiver = coremidi::Client::new("ks-review-t3d-r3-rx").unwrap();
    let input = receiver
        .input_port("in", move |list: &coremidi::PacketList| {
            let arrived = HostTime::now().ticks();
            for p in list {
                for msg in split(p.data()) {
                    let _ = tx.send((arrived, msg));
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

/// 120 BPM: 960틱 = 0.5 s.
fn snapshot(lb: &Loopback, events: Vec<ScheduledEvent>, end: Option<u64>) -> Arc<Snapshot> {
    let tempo = TempoMap::new(Tempo::from_bpm(120).unwrap());
    Arc::new(
        Snapshot::new(tempo, vec![Arc::clone(&lb.port)], events)
            .unwrap()
            .with_end(end.map(Tick::new)),
    )
}

fn start(snap: Arc<Snapshot>) -> Scheduler {
    Scheduler::start(snap, SchedulerConfig::default()).unwrap()
}

fn drain(lb: &Loopback, limit: Duration) -> Vec<Received> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match lb.rx.recv_timeout(left) {
            Ok(r) => got.push(r),
            Err(_) => return got,
        }
    }
}

/// `take_ended`가 `true`가 될 때까지 기다린다.
fn wait_ended(s: &mut Scheduler, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if s.take_ended() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// 시스템 리얼타임·공통 메시지(Clock·Start·Continue·Stop·SPP)만.
fn clock_msgs(got: &[Received]) -> Vec<Vec<u8>> {
    got.iter()
        .map(|(_, m)| m.clone())
        .filter(|m| m[0] >= 0xF0)
        .collect()
}

/// lookahead가 시작 여유(2 ms)보다 짧으면 곡 끝 정리가 시작 바퀴에 나가지 않아 대기가 여러 바퀴에 걸친다.
fn start_short(snap: Arc<Snapshot>) -> Scheduler {
    Scheduler::start(
        snap,
        SchedulerConfig {
            lookahead: Duration::from_millis(1),
            wake: Duration::from_millis(1),
        },
    )
    .unwrap()
}

fn is_resync(m: &[u8]) -> bool {
    m[0] == 0xF2 || m[0] == 0xFA || m[0] == 0xFB
}

/// 재생이 이어졌다면 첫 Clock 앞에 SPP·Continue(또는 Start)가 있어야 한다.
fn assert_resynced(ended: bool, clock: &[Vec<u8>]) {
    if ended {
        assert!(clock.contains(&vec![0xFC]), "{clock:?}");
        assert!(
            !clock.contains(&vec![0xF8]),
            "곡 끝 정리 앞뒤로 Clock: {clock:?}"
        );
        return;
    }
    let first_clock = clock.iter().position(|m| m == &vec![0xF8]);
    assert!(
        first_clock.is_some(),
        "재생이 이어지는데 Clock이 없다: {clock:?}"
    );
    let before = &clock[..first_clock.unwrap()];
    assert!(
        before.iter().any(|m| m[0] == 0xF2) && before.contains(&vec![0xFB]),
        "SPP·Continue 없이 Clock: {:?}",
        &clock[..clock.len().min(8)]
    );
}

#[test]
fn stop_while_resync_deferred_sends_single_stop_and_next_play_starts_clean() {
    // 대기 중 Stop: 클럭 장치는 Stop 하나만 받고, 남은 대기가 다음 재생에 새지 않는다.
    let lb = loopback("stop");
    let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(1920)).unwrap();
    s.stop().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let got = drain(&lb, Duration::from_millis(50));
    assert_eq!(clock_msgs(&got), vec![vec![0xFC]], "{got:?}");
    let _ = s.take_ended();
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    s.stop().unwrap();
    let got = drain(&lb, Duration::from_millis(100));
    let clock = clock_msgs(&got);
    assert_eq!(clock.first(), Some(&vec![0xFA]), "{clock:?}");
    assert!(
        !clock.iter().any(|m| m[0] == 0xF2 || m[0] == 0xFB),
        "{clock:?}"
    );
    assert_eq!(
        clock.iter().filter(|m| m == &&vec![0xFC]).count(),
        1,
        "{clock:?}"
    );
}

#[test]
fn set_clock_on_while_deferred_without_swap_sends_only_stop() {
    // 대기 중 `set_clock(on)`이 `next_clock`을 다시 잡아도 곡 끝 정리가 먼저라 Clock은 나가지 않는다.
    let lb = loopback("clock-on");
    let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
    s.play(Tick::new(1920)).unwrap();
    s.set_clock(0, true).unwrap();
    assert!(wait_ended(&mut s, Duration::from_millis(300)));
    std::thread::sleep(Duration::from_millis(50));
    let got = drain(&lb, Duration::from_millis(100));
    let clock = clock_msgs(&got);
    // 두 명령이 다른 바퀴면 곡 끝 정리 때 마스크가 0이라 아무것도 없다. 같은 바퀴면 Stop 하나.
    assert!(clock.is_empty() || clock == vec![vec![0xFC]], "{clock:?}");
}

#[test]
fn set_clock_on_while_deferred_then_swap_extending_resyncs() {
    // 시작 때 마스크 0으로 미뤘다가 같은 바퀴에 Clock을 켜고 곡 끝을 늘리면 SPP·Continue로 맞춘다.
    let lb = loopback("clock-on-swap");
    let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
    let longer = snapshot(&lb, Vec::new(), Some(96_000));
    s.play(Tick::new(1920)).unwrap();
    s.set_clock(0, true).unwrap();
    s.swap(longer).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let ended = s.take_ended();
    s.stop().unwrap();
    s.collect_garbage();
    let got = drain(&lb, Duration::from_millis(100));
    let clock = clock_msgs(&got);
    if ended {
        // 다른 바퀴: 마스크가 켜지기 전에 끝났을 수 있다.
        assert!(!clock.contains(&vec![0xF8]), "{clock:?}");
    } else {
        assert_resynced(false, &clock);
    }
}

#[test]
fn swap_extending_then_shortening_in_same_wake_sends_only_stop() {
    // 대기 중 교체가 곡 끝을 늘렸다가 다시 지난 곳으로 줄이면 맞추기 없이 Stop만.
    let lb = loopback("swap-back");
    let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(1920)).unwrap();
    s.swap(snapshot(&lb, Vec::new(), Some(96_000))).unwrap();
    s.swap(snapshot(&lb, Vec::new(), Some(960))).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let ended = s.take_ended();
    s.stop().unwrap();
    s.collect_garbage();
    let got = drain(&lb, Duration::from_millis(100));
    let clock = clock_msgs(&got);
    // 세 명령이 한 바퀴면 Stop만. 첫 교체가 다른 바퀴에서 먼저 맞췄다면 맞추기 뒤 Stop이 마지막.
    assert!(ended, "곡 끝 정리가 오지 않았다: {clock:?}");
    assert_eq!(clock.last(), Some(&vec![0xFC]), "{clock:?}");
    if !clock.iter().any(|m| is_resync(m)) {
        assert_eq!(clock, vec![vec![0xFC]], "{clock:?}");
    }
}

#[test]
fn play_past_end_then_play_zero_in_same_wake_starts_without_spp() {
    // 대기 중 `play(0)`: 새 시작이 대기를 덮어쓴다. Start만, SPP·Continue 없음.
    let lb = loopback("replay-zero");
    let mut s = start(snapshot(&lb, Vec::new(), Some(96_000)));
    s.set_clock(0, true).unwrap();
    s.swap(snapshot(&lb, Vec::new(), Some(960))).unwrap();
    s.play(Tick::new(1920)).unwrap();
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    s.stop().unwrap();
    s.collect_garbage();
    let got = drain(&lb, Duration::from_millis(100));
    let clock = clock_msgs(&got);
    let first_clock = clock
        .iter()
        .position(|m| m == &vec![0xF8])
        .unwrap_or(clock.len());
    let before = &clock[..first_clock];
    assert!(before.contains(&vec![0xFA]), "Start 없이 Clock: {clock:?}");
    assert!(
        !clock.iter().any(|m| m[0] == 0xF2 || m[0] == 0xFB),
        "{clock:?}"
    );
    // 첫 Clock 앞 Stop은 많아야 하나(곡 끝 정리 또는 재생 중 재시작의 Stop).
    assert!(
        before.iter().filter(|m| m == &&vec![0xFC]).count() <= 1,
        "{clock:?}"
    );
}

#[test]
fn locate_before_end_while_deferred_resyncs() {
    // 대기 중 `locate(480)`(곡 끝 960 앞): 같은 바퀴면 즉시 맞춘다(Stop·SPP·Continue).
    let lb = loopback("locate-back");
    let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(1920)).unwrap();
    s.locate(Tick::new(480)).unwrap();
    assert!(wait_ended(&mut s, Duration::from_millis(600)));
    let got = drain(&lb, Duration::from_millis(100));
    let clock = clock_msgs(&got);
    assert_eq!(clock.last(), Some(&vec![0xFC]), "{clock:?}");
    if clock.contains(&vec![0xF8]) {
        assert_resynced(false, &clock);
    } else {
        assert_eq!(clock, vec![vec![0xFC]], "{clock:?}");
    }
}

#[test]
fn deferred_across_wakes_then_swap_extending_resyncs() {
    // lookahead 1 ms < 시작 여유 2 ms: 대기가 다음 바퀴로 넘어간다. 그 사이에 곡 끝을 늘리는 교체.
    // 미룬 호스트 시각은 floor와 같고, 이벤트·Clock은 floor 앞에 가지 않으므로 SPP·Continue가 먼저여야 한다.
    let mut hit = 0;
    for (i, delay_us) in [0u64, 200, 400, 700, 1000, 1300, 1600, 2000]
        .into_iter()
        .enumerate()
    {
        let lb = loopback(&format!("cross-{i}"));
        let mut s = start_short(snapshot(&lb, Vec::new(), Some(960)));
        s.set_clock(0, true).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        let longer = snapshot(&lb, Vec::new(), Some(96_000));
        s.play(Tick::new(1920)).unwrap();
        std::thread::sleep(Duration::from_micros(delay_us));
        s.swap(longer).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        let ended = s.take_ended();
        s.stop().unwrap();
        s.collect_garbage();
        let got = drain(&lb, Duration::from_millis(100));
        let clock = clock_msgs(&got);
        if !ended {
            hit += 1;
        }
        assert_resynced(ended, &clock);
    }
    eprintln!("cross-wake: 곡 끝 정리 전에 교체가 닿은 경우 {hit}/8");
}

#[test]
fn deferred_across_wakes_while_playing_sends_exactly_one_stop() {
    // 재생 중 곡 끝 뒤로 `play`, 대기가 바퀴를 넘긴다: 그 사이 Clock이 나가지 않고 Stop은 한 번.
    let lb = loopback("cross-playing");
    let mut s = start_short(snapshot(&lb, Vec::new(), Some(96_000)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    s.swap(snapshot(&lb, Vec::new(), Some(960))).unwrap();
    // 0.1 s ≈ 192틱 < 960이라 아직 재생 중. 곡 끝 뒤로 이동.
    s.play(Tick::new(1920)).unwrap();
    assert!(wait_ended(&mut s, Duration::from_millis(500)));
    s.collect_garbage();
    let got = drain(&lb, Duration::from_millis(200));
    let clock = clock_msgs(&got);
    let last_stop = clock.iter().rposition(|m| m == &vec![0xFC]).unwrap();
    assert_eq!(last_stop, clock.len() - 1, "Stop 뒤 Clock: {clock:?}");
    assert_eq!(
        clock.iter().filter(|m| m == &&vec![0xFC]).count(),
        1,
        "{clock:?}"
    );
    assert!(!clock.iter().skip(1).any(|m| is_resync(m)), "{clock:?}");
}

#[test]
fn play_past_end_then_swap_while_playing_resyncs_repeatedly() {
    // 리뷰 2 시도 6을 여러 번. `play` 직전에 이미 넘긴 옛 Clock은 `play` 호출 시각 앞에 도착하므로 그 뒤에 도착한
    // 것만 본다. 리뷰 2 시험은 이를 거르지 않아, 드물게 옛 Clock이 drain 뒤·play 앞에 끼면 실패한다.
    let mut raced = 0;
    for i in 0..12 {
        let lb = loopback(&format!("swap-rep-{i}"));
        let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
        s.set_clock(0, true).unwrap();
        s.play(Tick::ZERO).unwrap();
        std::thread::sleep(Duration::from_millis(60 + 7 * i));
        let longer = snapshot(&lb, Vec::new(), Some(96_000));
        let t0 = HostTime::now().ticks();
        s.play(Tick::new(1920)).unwrap();
        s.swap(longer).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let ended = s.take_ended();
        s.stop().unwrap();
        s.collect_garbage();
        let got = drain(&lb, Duration::from_millis(100));
        let after: Vec<Received> = got.iter().filter(|(t, _)| *t >= t0).cloned().collect();
        if !ended {
            raced += 1;
        }
        let clock = clock_msgs(&after);
        // `play` 앞 바퀴에서 이미 넘긴 옛 Clock은 취소할 수 없어 Stop 앞에 도착할 수 있다. 첫 Stop 앞의 Clock은
        // 옛 재생의 것이므로 떼고 본다(리뷰 2 시험 `assert_resynced`가 드물게 실패하는 까닭).
        let lead = clock.iter().take_while(|m| m == &&vec![0xF8]).count();
        if lead > 0 {
            let first = after
                .iter()
                .find(|(_, m)| m[0] >= 0xF0)
                .map(|(t, _)| *t - t0);
            eprintln!("swap-rep {i}: Stop 앞 옛 Clock {lead}개, 첫 도착 t0+{first:?} 호스트 틱");
        }
        let clock = clock[lead..].to_vec();
        assert_resynced(ended, &clock);
        if !ended {
            assert_eq!(
                clock.first(),
                Some(&vec![0xFC]),
                "재생 중이었으면 Stop 먼저: {clock:?}"
            );
        }
    }
    eprintln!("swap-rep: 같은 바퀴 {raced}/12");
}
