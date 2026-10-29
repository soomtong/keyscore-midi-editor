//! S3-T3d 적대적 리뷰 2회차의 깨뜨리기 시험(`docs/reports/s3-t3d/review-2.md`).
//!
//! 대상은 커밋 `88d90c7`: 곡 끝 이후에서 시작하면 클럭 맞추기(Start·SPP·Continue, 재생 중이었으면 Stop)를
//! 건너뛴다. 재생 중 `play`·`locate`, 곡 끝 = 시작 위치, 시작 직후 같은 바퀴에 들어온 루프·교체를 본다.

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
    MidiClient::new("ks-review-t3d-r2").unwrap()
}

fn loopback(name: &str) -> Loopback {
    let client = midi();
    let name = format!("KS review t3d r2 {} {name}", std::process::id());
    let port: Arc<dyn MidiPort> = Arc::new(client.create_virtual_output(&name).unwrap());
    let (tx, rx) = mpsc::channel();
    let receiver = coremidi::Client::new("ks-review-t3d-r2-rx").unwrap();
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

/// 재생 중(0부터, 곡 끝 없음 = 길게) Clock이 흐르게 한 뒤 받은 것을 비운다.
fn playing_with_clock(lb: &Loopback, end: Option<u64>) -> Scheduler {
    let mut s = start(snapshot(lb, Vec::new(), end));
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let before = drain(lb, Duration::from_millis(20));
    assert!(
        clock_msgs(&before).iter().any(|m| m == &vec![0xF8]),
        "앞 재생에서 Clock이 흐르지 않았다: {before:?}"
    );
    s
}

#[test]
fn play_past_end_while_playing_sends_exactly_one_stop() {
    let lb = loopback("replay");
    let mut s = playing_with_clock(&lb, Some(960));
    s.play(Tick::new(1920)).unwrap();
    assert!(wait_ended(&mut s, Duration::from_millis(500)));
    s.stop().unwrap();
    drop(s);
    let got = drain(&lb, Duration::from_millis(300));
    let clock = clock_msgs(&got);
    // play 직전 바퀴에서 넘긴 Clock이 늦게 닿을 수 있다. Stop 뒤로는 아무것도 없어야 한다.
    let stops = clock.iter().filter(|m| m == &&vec![0xFC]).count();
    assert_eq!(stops, 1, "Stop은 정확히 한 번: {clock:?}");
    assert_eq!(clock.last(), Some(&vec![0xFC]), "Stop 뒤 Clock: {clock:?}");
    assert!(
        !clock
            .iter()
            .any(|m| m[0] == 0xF2 || m[0] == 0xFA || m[0] == 0xFB),
        "곡 끝 뒤 재시작에 SPP·Start·Continue가 나갔다: {clock:?}"
    );
}

#[test]
fn locate_past_end_while_playing_sends_exactly_one_stop() {
    let lb = loopback("locate");
    let mut s = playing_with_clock(&lb, Some(960));
    s.locate(Tick::new(1000)).unwrap();
    assert!(wait_ended(&mut s, Duration::from_millis(500)));
    drop(s);
    let got = drain(&lb, Duration::from_millis(300));
    let clock = clock_msgs(&got);
    let stops = clock.iter().filter(|m| m == &&vec![0xFC]).count();
    assert_eq!(stops, 1, "Stop은 정확히 한 번: {clock:?}");
    assert_eq!(clock.last(), Some(&vec![0xFC]), "Stop 뒤 Clock: {clock:?}");
    assert!(
        !clock
            .iter()
            .any(|m| m[0] == 0xF2 || m[0] == 0xFA || m[0] == 0xFB),
        "{clock:?}"
    );
}

#[test]
fn play_at_exact_end_sends_only_stop() {
    let lb = loopback("exact");
    let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(960)).unwrap();
    assert!(wait_ended(&mut s, Duration::from_millis(300)));
    let got = drain(&lb, Duration::from_millis(300));
    assert_eq!(clock_msgs(&got), vec![vec![0xFC]], "{got:?}");
}

#[test]
fn play_at_zero_with_zero_end_sends_only_stop() {
    // 빈 프로젝트(곡 끝 0)에서 Space: Start(0xFA) 없이 Stop만.
    let lb = loopback("zero");
    let mut s = start(snapshot(&lb, Vec::new(), Some(0)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    assert!(wait_ended(&mut s, Duration::from_millis(300)));
    let got = drain(&lb, Duration::from_millis(300));
    assert_eq!(clock_msgs(&got), vec![vec![0xFC]], "{got:?}");
}

#[test]
fn play_just_before_end_still_resyncs_clock() {
    // 곡 끝 바로 앞(959 < 960)은 건너뛰기 대상이 아니다: SPP·Continue 뒤 Stop.
    let lb = loopback("before");
    let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(959)).unwrap();
    assert!(wait_ended(&mut s, Duration::from_millis(300)));
    let got = drain(&lb, Duration::from_millis(300));
    let clock = clock_msgs(&got);
    assert_eq!(clock.first().map(|m| m[0]), Some(0xF2), "{clock:?}");
    assert!(clock.contains(&vec![0xFB]), "{clock:?}");
    assert_eq!(clock.last(), Some(&vec![0xFC]), "{clock:?}");
}

/// 재생이 이어졌다면(곡 끝 정리가 오지 않았다면) 클럭 장치는 `from`으로 맞춰져야 한다: `stopped_first`면 Stop
/// 뒤에, SPP와 Continue가 첫 Clock보다 앞에 온다.
fn assert_resynced(ended: bool, clock: &[Vec<u8>], stopped_first: bool) {
    if ended {
        // 두 명령이 다른 바퀴에 꺼내졌다(경합이 일어나지 않음). 이때는 곡 끝 정리의 Stop만 본다.
        assert!(clock.contains(&vec![0xFC]), "{clock:?}");
        return;
    }
    let first_clock = clock.iter().position(|m| m == &vec![0xF8]);
    let before = &clock[..first_clock.unwrap_or(clock.len())];
    assert!(
        before.iter().any(|m| m[0] == 0xF2) && before.contains(&vec![0xFB]),
        "재생이 이어지는데 클럭 장치에 SPP·Continue가 없다(Clock만 받는다): {:?}",
        &clock[..clock.len().min(8)]
    );
    if stopped_first {
        assert_eq!(before.first(), Some(&vec![0xFC]), "{clock:?}");
    }
}

#[test]
fn play_past_end_then_swap_extending_end_in_same_wake_resyncs_clock() {
    // 곡 끝 뒤 시작(맞추기 건너뜀) 직후 같은 바퀴에 곡 끝을 늘리는 교체가 들어오면 곡 끝 정리가 오지 않고
    // 재생이 이어진다. 클럭 장치는 0부터 돌던 그대로 Stop·SPP·Continue 없이 Clock만 받아 위치가 어긋난다.
    let lb = loopback("swap-race");
    let mut s = playing_with_clock(&lb, Some(960));
    let longer = snapshot(&lb, Vec::new(), Some(96_000));
    // 두 명령이 한 바퀴(1 ms)에 꺼내지도록 붙여 넣는다.
    s.play(Tick::new(1920)).unwrap();
    s.swap(longer).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let ended = s.take_ended();
    s.stop().unwrap();
    s.collect_garbage();
    let got = drain(&lb, Duration::from_millis(100));
    // 앞 재생에서 이미 넘긴 옛 Clock이 `playing_with_clock`의 drain 뒤에 도착할 수 있다(부하에서 실패, 리뷰 3
    // 제안 1). 맨 앞의 옛 Clock은 떼고 본다.
    let clock: Vec<Vec<u8>> = clock_msgs(&got)
        .into_iter()
        .skip_while(|m| m == &vec![0xF8])
        .collect();
    assert_resynced(ended, &clock, true);
}

#[test]
fn play_past_end_then_loop_in_same_wake_resyncs_clock() {
    // 곡 끝 뒤 시작 직후 같은 바퀴에 루프가 켜지면 곡 끝을 무시하고 재생이 이어진다. 클럭 장치는 SPP·Continue
    // 없이 Clock만 받는다(정지 상태의 장치는 따라오지 않는다).
    let lb = loopback("loop-race");
    let mut s = start(snapshot(&lb, Vec::new(), Some(960)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(1920)).unwrap();
    s.set_loop(Some((Tick::ZERO, Tick::new(480)))).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let ended = s.take_ended();
    s.stop().unwrap();
    let got = drain(&lb, Duration::from_millis(100));
    assert_resynced(ended, &clock_msgs(&got), false);
}
