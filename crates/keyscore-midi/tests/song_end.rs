//! S3-T3d 곡 끝 정지 루프백 통합 시험(결정 `2026-10-25-playback-end-of-song.md` A).
//!
//! 스냅샷의 곡 끝 틱에 닿으면 스케줄러가 스스로 멈춘다. 그 틱의 Note Off는 보내고, 그 틱의 클릭은 보내지
//! 않는다(다음 마디의 것이다).

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::{Channel, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, MetronomeConfig, Midi1Message, MidiClient, MidiPort, ScheduledEvent, Scheduler,
    SchedulerConfig, Snapshot, note_off, note_on,
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
    MidiClient::new("ks-end").unwrap()
}

fn loopback(name: &str) -> Loopback {
    let client = midi();
    let name = format!("KS end {} {name}", std::process::id());
    let port: Arc<dyn MidiPort> = Arc::new(client.create_virtual_output(&name).unwrap());
    let (tx, rx) = mpsc::channel();
    let receiver = coremidi::Client::new("ks-end-rx").unwrap();
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

fn is_on(m: &[u8], n: u8) -> bool {
    m.len() == 3 && m[0] & 0xF0 == 0x90 && m[1] == n && m[2] > 0
}

fn is_off(m: &[u8], n: u8) -> bool {
    m.len() == 3 && m[1] == n && (m[0] & 0xF0 == 0x80 || (m[0] & 0xF0 == 0x90 && m[2] == 0))
}

/// 메트로놈 클릭(채널 10의 Note On).
fn is_click(m: &[u8]) -> bool {
    m.len() == 3 && m[0] == 0x99 && m[2] > 0
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

#[test]
fn song_end_stops_playback_and_sends_offs() {
    let lb = loopback("offs");
    // 62는 곡 끝(1920)에서 끝나고, 64는 곡 끝을 넘어 울린다(곡 끝이 끈다). 67은 곡 끝 뒤라 나가지 않는다.
    let events = vec![
        on(0, 60),
        off(960, 60),
        on(960, 62),
        off(1920, 62),
        on(1440, 64),
        off(2400, 64),
        on(2400, 67),
        off(2880, 67),
    ];
    let mut s = start(snapshot(&lb, events, Some(1920)));
    s.play(Tick::ZERO).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_secs(3)),
        "곡 끝에서 멈추지 않았다"
    );
    let got = drain(&lb, Duration::from_millis(700));
    let msgs: Vec<&Vec<u8>> = got.iter().map(|(_, m)| m).collect();
    assert!(
        msgs.iter().any(|m| is_off(m, 62)),
        "곡 끝 틱의 Note Off가 없다: {msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| is_off(m, 64)),
        "울리던 64를 끄지 않았다: {msgs:?}"
    );
    assert!(
        !msgs.iter().any(|m| is_on(m, 67)),
        "곡 끝 뒤 노트가 나갔다: {msgs:?}"
    );
    assert!(!s.take_ended(), "곡 끝 정지는 한 번만 보고한다");
}

#[test]
fn song_end_does_not_click_on_end_tick() {
    let lb = loopback("click");
    // 곡 끝 1920은 세 번째 박이다. 박 0·960의 클릭 둘만 난다.
    let mut s = start(snapshot(&lb, Vec::new(), Some(1920)));
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_secs(3)),
        "곡 끝에서 멈추지 않았다"
    );
    let got = drain(&lb, Duration::from_millis(700));
    let clicks = got.iter().filter(|(_, m)| is_click(m)).count();
    assert_eq!(clicks, 2, "{got:?}");
    // 곡 끝 뒤 박 보고도 없다
    let mut beats = 0;
    while s.poll_beat().is_some() {
        beats += 1;
    }
    assert_eq!(beats, 2);
}

#[test]
fn play_from_at_or_after_end_stops_at_once() {
    let lb = loopback("past");
    let mut s = start(snapshot(&lb, vec![on(2400, 67), off(2880, 67)], Some(1920)));
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::new(1920)).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_millis(300)),
        "곡 끝에서 시작해도 멈춰야 한다"
    );
    s.play(Tick::new(2000)).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_millis(300)),
        "곡 끝 뒤에서 시작해도 멈춰야 한다"
    );
    let got = drain(&lb, Duration::from_millis(1500));
    assert!(
        !got.iter().any(|(_, m)| is_click(m) || is_on(m, 67)),
        "곡 끝 뒤에서 소리가 났다: {got:?}"
    );
}

#[test]
fn end_none_keeps_playing() {
    let lb = loopback("none");
    let mut s = start(snapshot(&lb, vec![on(1440, 64), off(1920, 64)], None));
    s.play(Tick::ZERO).unwrap();
    let got = drain(&lb, Duration::from_millis(1200));
    assert!(got.iter().any(|(_, m)| is_on(m, 64)), "{got:?}");
    assert!(!s.take_ended());
    s.stop().unwrap();
}

#[test]
fn swap_extending_end_keeps_playing() {
    let lb = loopback("extend");
    let mut s = start(snapshot(&lb, vec![on(240, 60), off(480, 60)], Some(960)));
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    s.swap(snapshot(
        &lb,
        vec![on(240, 60), off(480, 60), on(1440, 64), off(1920, 64)],
        Some(1920),
    ))
    .unwrap();
    // 옛 곡 끝(960, 0.5 s)을 지나 새 곡 끝(1920, 1 s) 뒤까지 받는다
    let got = drain(&lb, Duration::from_millis(1300));
    assert!(
        got.iter().any(|(_, m)| is_on(m, 64)),
        "늘린 곡의 노트가 들리지 않는다: {got:?}"
    );
    assert!(
        wait_ended(&mut s, Duration::from_millis(200)),
        "새 곡 끝에서 멈추지 않았다"
    );
    s.collect_garbage();
}

#[test]
fn swap_to_end_before_position_stops_at_once() {
    let lb = loopback("shrink");
    let events = vec![on(0, 60), off(1920, 60), on(1440, 64), off(1920, 64)];
    let mut s = start(snapshot(&lb, events.clone(), None));
    s.play(Tick::ZERO).unwrap();
    // 0.5 s(틱 960) 뒤 곡 끝을 480으로 줄인다
    std::thread::sleep(Duration::from_millis(500));
    s.swap(snapshot(&lb, events, Some(480))).unwrap();
    assert!(
        wait_ended(&mut s, Duration::from_millis(200)),
        "지난 곡 끝으로 바꾸면 곧바로 멈춰야 한다"
    );
    let got = drain(&lb, Duration::from_millis(1200));
    assert!(
        got.iter().any(|(_, m)| is_off(m, 60)),
        "울리던 60을 끄지 않았다: {got:?}"
    );
    assert!(
        !got.iter().any(|(_, m)| is_on(m, 64)),
        "멈춘 뒤 노트가 나갔다: {got:?}"
    );
    s.collect_garbage();
}

#[test]
fn take_ended_is_false_after_new_play() {
    let lb = loopback("replay");
    let mut s = start(snapshot(&lb, vec![on(0, 60), off(240, 60)], Some(480)));
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    // 끝으로 멈춘 것을 아직 읽지 않은 채 다시 재생한다. 옛 정지는 새 재생의 것이 아니다.
    s.play(Tick::ZERO).unwrap();
    assert!(!s.take_ended(), "다시 재생한 뒤 옛 정지를 보고했다");
    assert!(
        wait_ended(&mut s, Duration::from_secs(2)),
        "두 번째 재생이 곡 끝에서 멈추지 않았다"
    );
    drop(drain(&lb, Duration::from_millis(100)));
}

#[test]
fn song_end_in_loop_is_ignored() {
    let lb = loopback("loop");
    // 루프 [0, 960) 안의 720 노트가 두 번 들려야 한다. 곡 끝 480은 루프 중에는 무시한다.
    let mut s = start(snapshot(&lb, vec![on(720, 64), off(840, 64)], Some(480)));
    s.set_loop(Some((Tick::ZERO, Tick::new(960)))).unwrap();
    s.play(Tick::ZERO).unwrap();
    let got = drain(&lb, Duration::from_millis(1300));
    let hits = got.iter().filter(|(_, m)| is_on(m, 64)).count();
    assert!(hits >= 2, "{got:?}");
    assert!(!s.take_ended());
    s.stop().unwrap();
}
