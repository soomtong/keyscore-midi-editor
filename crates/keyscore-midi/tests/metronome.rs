//! S1-T5 메트로놈·미리듣기 루프백 통합 테스트(MET-1~3, PRE-1).
//!
//! 가상 출력 포트로 보내고 같은 프로세스의 `coremidi` crate 수신기로 받는다. 시각은 패킷 타임스탬프의
//! 간격으로 비교한다(스케줄러가 정한 예약 시각이다).

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::{Channel, MeterMap, Pitch, Tempo, TempoMap, Tick, TimeSignature, Velocity};
use keyscore_midi::{
    DEFAULT_PREVIEW_LENGTH, HostTime, MetronomeConfig, MidiClient, MidiPort, Scheduler,
    SchedulerConfig, Snapshot,
};

/// 받은 메시지: (도착 호스트 시각, 패킷 타임스탬프, 바이트).
type Received = (u64, u64, Vec<u8>);

struct Loopback {
    port: Arc<dyn MidiPort>,
    rx: mpsc::Receiver<Received>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

/// 이 바이너리의 첫 CoreMIDI 클라이언트를 `keyscore-midi`가 만든다(알림 조건, S1-T2).
fn midi() -> MidiClient {
    MidiClient::new("ks-metro").unwrap()
}

fn loopback(name: &str) -> Loopback {
    let client = midi();
    let name = format!("KS metro {} {name}", std::process::id());
    let port: Arc<dyn MidiPort> = Arc::new(client.create_virtual_output(&name).unwrap());
    let (tx, rx) = mpsc::channel();
    let receiver = coremidi::Client::new("ks-metro-rx").unwrap();
    let input = receiver
        .input_port("in", move |list: &coremidi::PacketList| {
            let arrived = HostTime::now().ticks();
            for p in list {
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

/// 240 BPM: 4분음표 250 ms, 틱 하나 250/960 ms.
fn tempo_240() -> TempoMap {
    TempoMap::new(Tempo::from_bpm(240).unwrap())
}

fn snapshot(ports: &[&Loopback], meter: MeterMap) -> Arc<Snapshot> {
    let ports = ports.iter().map(|lb| Arc::clone(&lb.port)).collect();
    Arc::new(
        Snapshot::new(tempo_240(), ports, Vec::new())
            .unwrap()
            .with_meter(meter),
    )
}

fn four_four() -> MeterMap {
    MeterMap::new(TimeSignature::COMMON)
}

/// 두 호스트 시각 사이(µs, 부호 있음).
fn us_between(earlier: u64, later: u64) -> f64 {
    if later >= earlier {
        HostTime::from_ticks(later)
            .duration_since(HostTime::from_ticks(earlier))
            .as_secs_f64()
            * 1e6
    } else {
        -us_between(later, earlier)
    }
}

/// 240 BPM에서 `ticks`만큼의 시간(µs).
fn us_of_ticks(ticks: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let t = ticks as f64;
    t * 250_000.0 / 960.0
}

/// `limit` 동안 또는 `filter`에 맞는 메시지 `count`개까지 모은다.
fn collect_matching(
    rx: &mpsc::Receiver<Received>,
    count: usize,
    limit: Duration,
    filter: impl Fn(&[u8]) -> bool,
) -> Vec<Received> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    while got.len() < count {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(r) if filter(&r.2) => got.push(r),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    got
}

fn is_note_on(b: &[u8]) -> bool {
    b.len() == 3 && b[0] & 0xF0 == 0x90 && b[2] > 0
}

fn is_note_off(b: &[u8]) -> bool {
    b.len() == 3 && (b[0] & 0xF0 == 0x80 || (b[0] & 0xF0 == 0x90 && b[2] == 0))
}

fn start(snap: Arc<Snapshot>) -> Scheduler {
    Scheduler::start(snap, SchedulerConfig::default()).unwrap()
}

/// 클릭 Note On들의 타임스탬프가 첫 클릭 기준으로 `ticks`(첫 클릭 기준 틱)와 1 µs 안인지 본다.
fn assert_grid(clicks: &[Received], ticks: &[u64]) {
    assert_eq!(clicks.len(), ticks.len(), "클릭 수: {clicks:?}");
    let t0 = clicks[0].1;
    for (c, &tick) in clicks.iter().zip(ticks) {
        let got = us_between(t0, c.1);
        let want = us_of_ticks(tick - ticks[0]);
        assert!(
            (got - want).abs() <= 1.0,
            "틱 {tick}: {got} µs, 기대 {want} µs"
        );
    }
}

#[test]
fn metronome_config_default_matches_req() {
    let c = MetronomeConfig::default();
    assert_eq!(c.port(), 0);
    assert_eq!(c.channel(), ch(10));
    assert_eq!(c.accent(), pitch(37));
    assert_eq!(c.beat(), pitch(42));
    assert_eq!(c.length(), Duration::from_millis(50));
    assert!(c.accent_velocity().get() > c.beat_velocity().get());
    assert_eq!(DEFAULT_PREVIEW_LENGTH, Duration::from_millis(200));
    // 길이는 [1 ms, 10 s]로 맞춘다.
    assert_eq!(
        c.with_length(Duration::ZERO).length(),
        Duration::from_millis(1)
    );
    assert_eq!(
        c.with_length(Duration::from_mins(1)).length(),
        Duration::from_secs(10)
    );
}

#[test]
fn metronome_clicks_on_beat_grid_with_accent_on_bar_start() {
    let lb = loopback("grid");
    let mut s = start(snapshot(&[&lb], four_four()));
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    let clicks = collect_matching(&lb.rx, 9, Duration::from_secs(4), is_note_on);
    s.stop().unwrap();
    let ticks: Vec<u64> = (0..9).map(|k| k * 960).collect();
    assert_grid(&clicks, &ticks);
    for (k, c) in clicks.iter().enumerate() {
        let note = if k % 4 == 0 { 37 } else { 42 };
        assert_eq!(c.2[0], 0x99, "채널 10");
        assert_eq!(c.2[1], note, "클릭 {k}");
    }
}

#[test]
fn metronome_follows_meter_change_and_short_bar() {
    let lb = loopback("meter");
    let mut meter = MeterMap::new(TimeSignature::new(3, 4).unwrap());
    // 둘째 마디(2880에서 시작) 중간에 6/8로 바꾼다. 둘째 마디는 한 박짜리 짧은 마디가 된다.
    meter.set(Tick::new(3840), TimeSignature::new(6, 8).unwrap());
    let mut s = start(snapshot(&[&lb], meter));
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    let clicks = collect_matching(&lb.rx, 11, Duration::from_secs(4), is_note_on);
    s.stop().unwrap();
    let expected: [(u64, u8); 11] = [
        (0, 37),
        (960, 42),
        (1920, 42),
        (2880, 37),
        (3840, 37),
        (4320, 42),
        (4800, 42),
        (5280, 42),
        (5760, 42),
        (6240, 42),
        (6720, 37),
    ];
    let ticks: Vec<u64> = expected.iter().map(|e| e.0).collect();
    assert_grid(&clicks, &ticks);
    for (c, (tick, note)) in clicks.iter().zip(expected) {
        assert_eq!(c.2[1], note, "틱 {tick}");
    }
}

#[test]
fn metronome_click_note_off_after_configured_length() {
    let lb = loopback("length");
    let mut s = start(snapshot(&[&lb], four_four()));
    let config = MetronomeConfig::default().with_length(Duration::from_millis(30));
    s.set_metronome(config).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    let got = collect_matching(&lb.rx, 6, Duration::from_secs(3), |b| {
        is_note_on(b) || is_note_off(b)
    });
    s.stop().unwrap();
    assert_eq!(got.len(), 6, "{got:?}");
    for pair in got.chunks(2) {
        assert!(is_note_on(&pair[0].2), "{pair:?}");
        assert!(is_note_off(&pair[1].2), "{pair:?}");
        assert_eq!(pair[0].2[1], pair[1].2[1]);
        let len = us_between(pair[0].1, pair[1].1);
        assert!((len - 30_000.0).abs() <= 1.0, "길이 {len} µs");
    }
}

#[test]
fn metronome_off_sends_no_click_and_forced_overrides_button() {
    let lb = loopback("forced");
    let mut s = start(snapshot(&[&lb], four_four()));
    s.set_metronome(MetronomeConfig::default()).unwrap();
    // 버튼 기본값은 끔이다.
    s.play(Tick::ZERO).unwrap();
    let none = collect_matching(&lb.rx, 1, Duration::from_millis(600), is_note_on);
    assert!(none.is_empty(), "버튼이 꺼져 있는데 클릭: {none:?}");
    s.set_metronome_forced(true).unwrap();
    let forced = collect_matching(&lb.rx, 2, Duration::from_secs(2), is_note_on);
    assert_eq!(forced.len(), 2, "강제 켬이면 버튼과 무관하게 클릭");
    // 강제를 끄고 버튼을 켜도 클릭은 이어진다. 둘 다 끄면 멈춘다.
    s.set_metronome_enabled(true).unwrap();
    s.set_metronome_forced(false).unwrap();
    let on = collect_matching(&lb.rx, 1, Duration::from_secs(2), is_note_on);
    assert_eq!(on.len(), 1);
    s.set_metronome_enabled(false).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    while lb.rx.try_recv().is_ok() {}
    let after = collect_matching(&lb.rx, 1, Duration::from_millis(600), is_note_on);
    assert!(after.is_empty(), "둘 다 끈 뒤 클릭: {after:?}");
    s.stop().unwrap();
}

#[test]
fn metronome_uses_its_own_port_and_channel() {
    let track = loopback("track");
    let drum = loopback("drum");
    let mut s = start(snapshot(&[&track, &drum], four_four()));
    let config = MetronomeConfig::new(1, ch(3), pitch(76), pitch(77));
    s.set_metronome(config).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    let clicks = collect_matching(&drum.rx, 2, Duration::from_secs(2), is_note_on);
    s.stop().unwrap();
    assert_eq!(clicks.len(), 2);
    assert_eq!(clicks[0].2, vec![0x92, 76, clicks[0].2[2]]);
    assert_eq!(clicks[1].2[..2], [0x92, 77]);
    let stray = collect_matching(&track.rx, 1, Duration::from_millis(100), |b| {
        is_note_on(b) || is_note_off(b)
    });
    assert!(stray.is_empty(), "트랙 포트로 클릭: {stray:?}");
}

#[test]
fn metronome_restarts_grid_after_locate_and_loop_wrap() {
    let lb = loopback("loop");
    let mut s = start(snapshot(&[&lb], four_four()));
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
    // 루프 [480, 2400). 480에서 재생하면 첫 클릭은 다음 박 960이다.
    s.set_loop(Some((Tick::new(480), Tick::new(2400)))).unwrap();
    s.play(Tick::new(480)).unwrap();
    let clicks = collect_matching(&lb.rx, 4, Duration::from_secs(3), is_note_on);
    // 960, 1920, (되감기 2400 → 480), 960, 1920. 첫 클릭 기준 틱으로 적는다.
    assert_grid(&clicks, &[960, 1920, 960 + 1920, 1920 + 1920]);
    // 박 위로 위치를 옮기면 그 자리에서 클릭한다(마디 첫 박이면 강박).
    s.set_loop(None).unwrap();
    s.locate(Tick::new(3840)).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    while lb.rx.try_recv().is_ok() {}
    s.locate(Tick::new(3840)).unwrap();
    let after = collect_matching(&lb.rx, 2, Duration::from_secs(2), is_note_on);
    s.stop().unwrap();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].2[1], 37, "마디 첫 박");
    assert_grid(&after, &[3840, 4800]);
}

#[test]
fn beats_are_reported_for_every_beat_while_playing() {
    let lb = loopback("beats");
    let mut s = start(snapshot(
        &[&lb],
        MeterMap::new(TimeSignature::new(3, 4).unwrap()),
    ));
    s.set_metronome(MetronomeConfig::default()).unwrap();
    // 클릭이 꺼져 있어도 박은 알린다(시각 메트로놈, MET-2).
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    let mut beats = Vec::new();
    while let Some(b) = s.poll_beat() {
        beats.push(b);
    }
    assert!(beats.len() >= 4, "{beats:?}");
    let want = [(0, 0), (0, 1), (0, 2), (1, 0)];
    for (b, (bar, beat)) in beats.iter().zip(want) {
        assert_eq!((b.bar(), b.beat()), (bar, beat));
    }
    for w in beats.windows(2).take(3) {
        let gap = us_between(w[0].at().ticks(), w[1].at().ticks());
        assert!((gap - 250_000.0).abs() <= 1.0, "박 간격 {gap} µs");
    }
    // 클릭을 켜면 박 시각과 클릭 타임스탬프가 같다.
    s.stop().unwrap();
    while s.poll_beat().is_some() {}
    s.set_metronome_enabled(true).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    while lb.rx.try_recv().is_ok() {}
    s.play(Tick::ZERO).unwrap();
    let clicks = collect_matching(&lb.rx, 2, Duration::from_secs(2), is_note_on);
    s.stop().unwrap();
    let first = s.poll_beat().unwrap();
    let second = s.poll_beat().unwrap();
    assert_eq!(first.at().ticks(), clicks[0].1);
    assert_eq!(second.at().ticks(), clicks[1].1);
}

#[test]
fn preview_sends_note_off_after_default_length() {
    let lb = loopback("preview");
    let mut s = start(snapshot(&[&lb], four_four()));
    s.preview(0, ch(1), pitch(60), vel(), DEFAULT_PREVIEW_LENGTH)
        .unwrap();
    let got = collect_matching(&lb.rx, 2, Duration::from_secs(1), |b| {
        is_note_on(b) || is_note_off(b)
    });
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(got[0].2[..2], [0x90, 60]);
    assert!(is_note_off(&got[1].2));
    assert_eq!(got[1].2[..2], [0x80, 60]);
    let stamp = us_between(got[0].1, got[1].1);
    assert!(
        (stamp - 200_000.0).abs() <= 1.0,
        "타임스탬프 간격 {stamp} µs"
    );
    let arrival = us_between(got[0].0, got[1].0);
    assert!(
        (arrival - 200_000.0).abs() <= 2_000.0,
        "도착 간격 {arrival} µs"
    );
}

#[test]
fn preview_retrigger_same_note_restarts_length() {
    let lb = loopback("retrigger");
    let mut s = start(snapshot(&[&lb], four_four()));
    s.preview(0, ch(1), pitch(60), vel(), DEFAULT_PREVIEW_LENGTH)
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    s.preview(0, ch(1), pitch(60), vel(), DEFAULT_PREVIEW_LENGTH)
        .unwrap();
    let got = collect_matching(&lb.rx, 5, Duration::from_millis(600), |b| {
        is_note_on(b) || is_note_off(b)
    });
    assert_eq!(got.len(), 4, "{got:?}");
    assert!(is_note_on(&got[0].2));
    assert!(is_note_off(&got[1].2), "다시 켜기 전에 먼저 끈다");
    assert!(is_note_on(&got[2].2));
    assert!(is_note_off(&got[3].2));
    // 앞 노트를 끈 시각은 두 번째 Note On 이하, 마지막 Note Off는 두 번째 + 200 ms다.
    assert!(got[1].1 <= got[2].1);
    let len = us_between(got[2].1, got[3].1);
    assert!((len - 200_000.0).abs() <= 1.0, "두 번째 길이 {len} µs");
}

#[test]
fn many_previews_never_leave_a_note_on() {
    let lb = loopback("many");
    let mut s = start(snapshot(&[&lb], four_four()));
    // 서로 다른 노트 100개(끄기 대기 표보다 많다)와 되풀이 노트를 섞는다.
    for k in 0..100u8 {
        let note = if k % 10 == 0 { 60 } else { 20 + k };
        s.preview(
            0,
            ch(1 + k % 16),
            pitch(note),
            vel(),
            Duration::from_millis(150),
        )
        .unwrap();
    }
    let got = collect_matching(&lb.rx, 400, Duration::from_millis(800), |b| {
        is_note_on(b) || is_note_off(b)
    });
    let mut on = std::collections::HashMap::<(u8, u8), i32>::new();
    for (_, _, b) in &got {
        let key = (b[0] & 0x0F, b[1]);
        let d = if is_note_on(b) { 1 } else { -1 };
        *on.entry(key).or_default() += d;
        assert!(on[&key] >= 0, "Note Off가 Note On보다 먼저: {key:?}");
    }
    let ons = got.iter().filter(|r| is_note_on(&r.2)).count();
    assert_eq!(ons, 100);
    let left: Vec<_> = on.iter().filter(|&(_, &n)| n != 0).collect();
    assert!(left.is_empty(), "남은 노트: {left:?}");
}
