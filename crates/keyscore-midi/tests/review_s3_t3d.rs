//! S3-T3d 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s3-t3d/review-1.md`).
//!
//! 곡 끝 틱에 Note Off·박·MIDI Clock이 겹칠 때의 순서, 곡 끝 뒤 시작의 클럭 메시지, 곡 끝 정지와 미리듣기,
//! 루프를 끈 순간 이미 곡 끝을 지난 경우를 루프백으로 본다.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::{Channel, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, MetronomeConfig, Midi1Message, MidiClient, MidiPort, ScheduledEvent, Scheduler,
    SchedulerConfig, Snapshot, note_off, note_on,
};

type Received = (u64, Vec<u8>);

struct Loopback {
    port: Arc<dyn MidiPort>,
    rx: mpsc::Receiver<Received>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn midi() -> MidiClient {
    MidiClient::new("ks-review-t3d").unwrap()
}

fn loopback(name: &str) -> Loopback {
    let client = midi();
    let name = format!("KS review t3d {} {name}", std::process::id());
    let port: Arc<dyn MidiPort> = Arc::new(client.create_virtual_output(&name).unwrap());
    let (tx, rx) = mpsc::channel();
    let receiver = coremidi::Client::new("ks-review-t3d-rx").unwrap();
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

fn ch1() -> Channel {
    Channel::new(1).unwrap()
}

fn on(tick: u64, n: u8) -> ScheduledEvent {
    let msg = note_on(
        ch1(),
        Pitch::new(n).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    );
    ScheduledEvent::new(Tick::new(tick), 0, msg)
}

fn off(tick: u64, n: u8) -> ScheduledEvent {
    let msg: Midi1Message = note_off(
        ch1(),
        Pitch::new(n).unwrap(),
        Velocity::from_midi1(0).unwrap(),
    );
    ScheduledEvent::new(Tick::new(tick), 0, msg)
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

fn is_on(m: &[u8], n: u8) -> bool {
    m.len() == 3 && m[0] & 0xF0 == 0x90 && m[1] == n && m[2] > 0
}

fn is_off(m: &[u8], n: u8) -> bool {
    m.len() == 3 && m[1] == n && (m[0] & 0xF0 == 0x80 || (m[0] & 0xF0 == 0x90 && m[2] == 0))
}

fn is_click(m: &[u8]) -> bool {
    m.len() == 3 && m[0] == 0x99 && m[2] > 0
}

fn position(got: &[Received], pred: impl Fn(&[u8]) -> bool) -> Option<usize> {
    got.iter().position(|(_, m)| pred(m))
}

/// 곡 끝 960틱에 Note Off·박(두 번째 마디 아님, 4/4의 둘째 박)·Clock이 모두 겹친다. Note Off는 나가고, 그
/// 틱의 클릭과 Clock(0xF8)은 나가지 않으며, Stop(0xFC)은 정확히 한 번이다. 곡 끝 뒤 `stop`과 스케줄러
/// drop도 Stop을 다시 보내지 않는다.
#[test]
fn song_end_tick_sends_off_but_no_clock_or_click_and_one_stop() {
    let lb = loopback("tie");
    let mut s = start(snapshot(&lb, vec![on(0, 60), off(960, 60)], Some(960)));
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_secs(2)),
        "곡 끝에서 멈추지 않았다"
    );
    std::thread::sleep(Duration::from_millis(100));
    s.stop().unwrap();
    drop(s);
    let got = drain(&lb, Duration::from_millis(500));
    let clocks = got.iter().filter(|(_, m)| m[..] == [0xF8]).count();
    // 0, 40, …, 920 = 24개. 960의 Clock은 다음 박의 것이다.
    assert_eq!(clocks, 24, "{got:?}");
    let stops = got.iter().filter(|(_, m)| m[..] == [0xFC]).count();
    assert_eq!(stops, 1, "Stop은 한 번: {got:?}");
    let metronome_hits = got.iter().filter(|(_, m)| is_click(m)).count();
    assert_eq!(metronome_hits, 1, "곡 끝 박의 클릭이 났다: {got:?}");
    let off60 = position(&got, |m| is_off(m, 60)).expect("곡 끝 틱의 Note Off가 없다");
    let stop = position(&got, |m| m[..] == [0xFC]).unwrap();
    assert!(
        off60 < stop,
        "곡 끝 틱의 Note Off가 Stop 뒤에 왔다: {got:?}"
    );
    let last_clock = got.iter().rposition(|(_, m)| m[..] == [0xF8]).unwrap();
    assert!(last_clock < stop, "Stop 뒤에 Clock이 나갔다: {got:?}");
}

/// 곡 끝이 Clock 칸(40틱)에 맞지 않을 때(1001): 1000의 Clock까지 나가고 1040은 나가지 않는다.
#[test]
fn song_end_off_clock_grid_sends_clocks_before_end_only() {
    let lb = loopback("grid");
    let mut s = start(snapshot(&lb, vec![on(0, 60), off(1001, 60)], Some(1001)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_secs(2)),
        "곡 끝에서 멈추지 않았다"
    );
    let got = drain(&lb, Duration::from_millis(400));
    let clocks = got.iter().filter(|(_, m)| m[..] == [0xF8]).count();
    assert_eq!(clocks, 26, "{got:?}");
}

/// 곡 끝 뒤에서 시작: 클럭 장치에 Clock(0xF8)은 하나도 가지 않고 Stop으로 끝난다. Start/Continue가 함께
/// 나가는지는 기록만 한다(계획 「chase만 하고 곧바로 멈춘다」의 해석, 리뷰 1 제안).
#[test]
fn play_past_end_with_clock_sends_no_clock_ticks() {
    let lb = loopback("pastclock");
    let mut s = start(snapshot(&lb, vec![on(2400, 67), off(2880, 67)], Some(1920)));
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(2000)).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_millis(300)),
        "곧바로 멈춰야 한다"
    );
    let got = drain(&lb, Duration::from_millis(600));
    assert!(
        !got.iter().any(|(_, m)| m[..] == [0xF8]),
        "곡 끝 뒤 시작에 Clock이 나갔다: {got:?}"
    );
    let last = got.last().map(|(_, m)| m.clone());
    assert_eq!(last, Some(vec![0xFC]), "마지막은 Stop이어야 한다: {got:?}");
    let started = got.iter().any(|(_, m)| m[..] == [0xFB] || m[..] == [0xFA]);
    eprintln!("곡 끝 뒤 시작의 Start/Continue 송신: {started}, 받은 것: {got:?}");
}

/// 곡 끝 정지 뒤 미리듣기는 그대로 켜지고 길이 뒤에 꺼진다(곡 끝 정리가 미리듣기를 막지 않는다).
#[test]
fn preview_after_song_end_sounds_and_releases() {
    let lb = loopback("preview");
    let mut s = start(snapshot(&lb, vec![on(0, 70), off(240, 70)], Some(240)));
    s.play(Tick::ZERO).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_secs(1)),
        "곡 끝에서 멈추지 않았다"
    );
    drop(drain(&lb, Duration::from_millis(100)));
    s.preview(
        0,
        ch1(),
        Pitch::new(70).unwrap(),
        Velocity::from_midi1(90).unwrap(),
        Duration::from_millis(200),
    )
    .unwrap();
    let got = drain(&lb, Duration::from_millis(500));
    let on_at = got
        .iter()
        .find(|(_, m)| is_on(m, 70))
        .map(|(t, _)| *t)
        .expect("미리듣기 Note On이 없다");
    let off_at = got
        .iter()
        .find(|(_, m)| is_off(m, 70))
        .map(|(t, _)| *t)
        .expect("미리듣기 Note Off가 없다");
    let held = HostTime::from_ticks(off_at).duration_since(HostTime::from_ticks(on_at));
    assert!(
        held >= Duration::from_millis(150),
        "미리듣기가 일찍 끊겼다: {held:?}"
    );
    assert!(!s.take_ended(), "미리듣기가 곡 끝 정지를 다시 보고했다");
}

/// 곡 끝 직전에 걸린 미리듣기 노트는 곡 끝 정리가 끈다(정지 명령과 같다). 남지 않는다.
#[test]
fn preview_during_playback_is_released_at_song_end() {
    let lb = loopback("held");
    let mut s = start(snapshot(&lb, Vec::new(), Some(480)));
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    s.preview(
        0,
        ch1(),
        Pitch::new(72).unwrap(),
        Velocity::from_midi1(90).unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_secs(1)),
        "곡 끝에서 멈추지 않았다"
    );
    let got = drain(&lb, Duration::from_millis(400));
    assert!(got.iter().any(|(_, m)| is_on(m, 72)), "{got:?}");
    assert!(
        got.iter().any(|(_, m)| is_off(m, 72)),
        "걸린 미리듣기를 끄지 않았다: {got:?}"
    );
}

/// 루프 중 곡 끝을 지난 뒤 루프를 끄면 곧바로 멈추고 울리던 노트를 끈다.
#[test]
fn clearing_loop_past_end_stops_at_once() {
    let lb = loopback("unloop");
    let mut s = start(snapshot(&lb, vec![on(600, 64), off(900, 64)], Some(480)));
    s.set_loop(Some((Tick::ZERO, Tick::new(960)))).unwrap();
    s.play(Tick::ZERO).unwrap();
    // 0.35 s ≈ 틱 672: 64가 울리는 중
    std::thread::sleep(Duration::from_millis(350));
    s.set_loop(None).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_millis(200)),
        "루프를 끈 순간 곡 끝을 지났으면 멈춰야 한다"
    );
    let got = drain(&lb, Duration::from_millis(1200));
    let on64 = got.iter().filter(|(_, m)| is_on(m, 64)).count();
    let off64 = got.iter().filter(|(_, m)| is_off(m, 64)).count();
    assert!(on64 <= off64, "울리던 64가 남았다: {got:?}");
}

/// 곡 끝으로 멈춘 뒤 읽지 않고 `stop` → `play`를 거듭해도 새 재생의 곡 끝은 꼭 한 번 보고된다.
#[test]
fn stale_end_then_stop_and_play_reports_new_end_once() {
    let lb = loopback("stale");
    let mut s = start(snapshot(&lb, vec![on(0, 60), off(240, 60)], Some(240)));
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    s.stop().unwrap();
    s.play(Tick::ZERO).unwrap();
    s.stop().unwrap();
    s.play(Tick::ZERO).unwrap();
    assert!(!s.take_ended());
    assert!(
        wait_ended(&mut s, Duration::from_secs(1)),
        "새 재생의 곡 끝을 보고하지 않았다"
    );
    assert!(!s.take_ended(), "두 번 보고했다");
    drop(drain(&lb, Duration::from_millis(100)));
}
