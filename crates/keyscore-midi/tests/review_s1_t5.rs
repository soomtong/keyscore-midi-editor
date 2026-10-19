//! S1-T5 적대적 리뷰 1회차 테스트(메트로놈·미리듣기, MET-1~3, PRE-1).
//!
//! 이 파일 안의 기록 포트(`Rec`)만 쓴다. CoreMIDI 클라이언트를 만들지 않으며, 이 바이너리는
//! `assert_no_alloc` 전역 할당자를 두지 않는다. 신디가 듣는 순서는 「타임스탬프 순, 같은 시각이면 넘긴
//! 순」이라고 보고, 기록을 그 순서로 다시 재생해 끝에 울리는 노트가 있는지 본다.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use keyscore_core::{Channel, MeterMap, Pitch, Tempo, TempoMap, Tick, TimeSignature, Velocity};
use keyscore_midi::{
    DEFAULT_PREVIEW_LENGTH, HostTime, MetronomeConfig, MidiError, MidiPort, ScheduledEvent,
    Scheduler, SchedulerConfig, Snapshot, note_on,
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

fn ch(n: u8) -> Channel {
    Channel::new(n).unwrap()
}

fn pitch(n: u8) -> Pitch {
    Pitch::new(n).unwrap()
}

fn vel() -> Velocity {
    Velocity::from_midi1(100).unwrap()
}

fn sig(n: u8, d: u16) -> TimeSignature {
    TimeSignature::new(n, d).unwrap()
}

fn tempo(bpm: u32) -> TempoMap {
    TempoMap::new(Tempo::from_bpm(bpm).unwrap())
}

fn snap(
    bpm: u32,
    meter: MeterMap,
    ports: &[&Arc<dyn MidiPort>],
    events: Vec<ScheduledEvent>,
) -> Arc<Snapshot> {
    let ports = ports.iter().map(|p| Arc::clone(p)).collect();
    Arc::new(
        Snapshot::new(tempo(bpm), ports, events)
            .unwrap()
            .with_meter(meter),
    )
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn is_on(b: &[u8]) -> bool {
    b.len() == 3 && b[0] & 0xF0 == 0x90 && b[2] > 0
}

fn is_off(b: &[u8]) -> bool {
    b.len() == 3 && (b[0] & 0xF0 == 0x80 || (b[0] & 0xF0 == 0x90 && b[2] == 0))
}

/// 신디처럼(세지 않고) 재생해 끝에 울리는 (채널 0–15, 노트)를 돌려준다. All Notes Off·All Sound Off는
/// 그 채널을 모두 끈다.
fn sounding(log: &Log) -> BTreeSet<(u8, u8)> {
    let mut msgs = log.lock().unwrap().clone();
    // 안정 정렬: 같은 시각이면 넘긴 순서.
    msgs.sort_by_key(|m| m.0);
    let mut on = BTreeSet::new();
    for (_, b) in &msgs {
        let chan = b[0] & 0x0F;
        if is_on(b) {
            on.insert((chan, b[1]));
        } else if is_off(b) {
            on.remove(&(chan, b[1]));
        } else if b.len() == 3 && b[0] & 0xF0 == 0xB0 && (b[1] == 120 || b[1] == 123) {
            on.retain(|&(c, _)| c != chan);
        }
    }
    on
}

/// 기록의 클릭 Note On(채널 10)만: (호스트 시각, 노트).
fn clicks(log: &Log) -> Vec<(u64, u8)> {
    let mut v: Vec<_> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| is_on(b) && b[0] == 0x99)
        .map(|(t, b)| (*t, b[1]))
        .collect();
    v.sort_by_key(|c| c.0);
    v
}

/// 두 호스트 시각 사이(ns).
fn ns_between(earlier: u64, later: u64) -> i128 {
    let ns = |a: u64, b: u64| {
        i128::try_from(
            HostTime::from_ticks(b)
                .duration_since(HostTime::from_ticks(a))
                .as_nanos(),
        )
        .unwrap()
    };
    if later >= earlier {
        ns(earlier, later)
    } else {
        -ns(later, earlier)
    }
}

/// 240 BPM에서 `ticks`틱의 길이(ns): 4분음표 250 ms.
fn ns_of_ticks_240(ticks: u64) -> i128 {
    i128::from(ticks) * 250_000_000 / 960
}

/// 첫 클릭 기준으로 클릭이 (틱, 노트) 목록과 같고 시각이 2 µs 안인지 본다(240 BPM).
fn assert_clicks_240(got: &[(u64, u8)], want: &[(u64, u8)]) {
    let notes: Vec<_> = got.iter().map(|c| c.1).collect();
    let want_notes: Vec<_> = want.iter().map(|c| c.1).collect();
    assert_eq!(notes, want_notes, "클릭 노트 순서");
    let t0 = got[0].0;
    for (g, w) in got.iter().zip(want) {
        let d = ns_between(t0, g.0) - ns_of_ticks_240(w.0 - want[0].0);
        assert!(d.abs() <= 2_000, "틱 {}: 오차 {d} ns", w.0);
    }
}

fn metronome_on(s: &mut Scheduler) {
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
}

/// 클릭의 Note On이 lookahead 창에 미리 넘어가 있는 동안 같은 노트를 미리듣기하면, `play_note`는 아직
/// 넘기지 않은 클릭 Note Off를 `min(off, now)` = 지금에 앞당겨 보낸다. 그러면 미래 시각의 클릭 Note On
/// 뒤에 오는 Note Off가 없다. 미리듣기의 Note Off도 클릭 Note On보다 앞이다. 울리는 노트 수는 0으로
/// 세어져 정지도 끄지 않는다(PRE-1, MET-1).
#[test]
fn preview_of_click_note_during_lookahead_leaves_no_note_sounding() {
    let (port, log) = rec();
    let config = SchedulerConfig {
        lookahead: Duration::from_millis(500),
        wake: Duration::from_millis(1),
    };
    // 120 BPM: 박 500 ms. 재생 직후 박 0(37)과 박 1(42, 약 500 ms 뒤)의 Note On이 넘어간다. 박 1의
    // Note Off(550 ms 뒤)는 창 밖이라 아직 대기 표에 있다.
    let mut s = Scheduler::start(snap(120, MeterMap::default(), &[&port], vec![]), config).unwrap();
    metronome_on(&mut s);
    s.play(Tick::ZERO).unwrap();
    sleep_ms(20);
    // 채널 10 드럼 트랙에서 닫힌 하이햇(42)을 기본 길이로 미리듣기.
    s.preview(0, Channel::DRUMS, pitch(42), vel(), DEFAULT_PREVIEW_LENGTH)
        .unwrap();
    sleep_ms(60);
    // 박 2가 창에 들어오기 전에 정지한다.
    s.stop().unwrap();
    sleep_ms(800);
    drop(s);
    let left = sounding(&log);
    assert!(
        left.is_empty(),
        "끝에 울리는 노트(채널0-15, 노트): {left:?}"
    );
}

/// 위와 같은 결함을 기본 설정(lookahead 10 ms)에서 본다. 미리듣기 길이가 lookahead보다 짧으면(설정값
/// 5 ms) 클릭 Note On 직전 창에 든 미리듣기가 그 클릭 노트를 남긴다. 4/4의 마지막 약박(박 3)을 겨눠
/// 다음 같은 노트 클릭(박 5)이 오기 전에 정지한다.
#[test]
fn short_preview_just_before_click_leaves_no_note_sounding_with_default_lookahead() {
    let (port, log) = rec();
    let mut s = Scheduler::start(
        snap(240, MeterMap::default(), &[&port], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    metronome_on(&mut s);
    s.play(Tick::ZERO).unwrap();
    // 첫 클릭(박 0) 시각을 기록에서 얻는다.
    let first = loop {
        if let Some(c) = clicks(&log).first() {
            break c.0;
        }
        std::thread::sleep(Duration::from_micros(200));
    };
    // 박 3(750 ms)의 4 ms 앞: 그 클릭 Note On은 이미 넘어갔고(창 10 ms) 아직 울리지 않았다.
    let target = 750_000_000 - 4_000_000;
    while ns_between(first, HostTime::now().ticks()) < target {
        std::thread::sleep(Duration::from_micros(100));
    }
    s.preview(
        0,
        Channel::DRUMS,
        pitch(42),
        vel(),
        Duration::from_millis(1),
    )
    .unwrap();
    // 박 4(1000 ms, 37) 뒤, 박 5(1250 ms, 42) 앞에서 정지.
    while ns_between(first, HostTime::now().ticks()) < 1_100_000_000 {
        std::thread::sleep(Duration::from_millis(1));
    }
    s.stop().unwrap();
    sleep_ms(100);
    drop(s);
    let left = sounding(&log);
    assert!(
        left.is_empty(),
        "끝에 울리는 노트(채널0-15, 노트): {left:?}"
    );
}

/// 클릭이 울리는 도중 정지·패닉·위치 이동을 여러 시점에 걸어도 남는 노트가 없다(회귀).
#[test]
fn stop_panic_locate_mid_click_leave_no_note_sounding() {
    let (port, log) = rec();
    let mut s = Scheduler::start(
        snap(240, MeterMap::default(), &[&port], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_metronome(MetronomeConfig::default().with_length(Duration::from_millis(120)))
        .unwrap();
    s.set_metronome_enabled(true).unwrap();
    for k in 0..12u64 {
        s.play(Tick::new(k * 37)).unwrap();
        sleep_ms(30 + k * 17 % 90);
        match k % 3 {
            0 => s.stop().unwrap(),
            1 => s.panic().unwrap(),
            _ => s.locate(Tick::new(k * 101)).unwrap(),
        }
        sleep_ms(3 + k * 7 % 20);
    }
    s.stop().unwrap();
    sleep_ms(400);
    drop(s);
    let left = sounding(&log);
    assert!(left.is_empty(), "남은 노트: {left:?}");
}

/// 재생 중 박자표만 바꾼 스냅샷으로 교체하면 다음 박부터 새 박자표의 강박을 따르고, 박이 빠지거나
/// 겹치지 않으며 격자가 그대로다(회귀).
#[test]
fn swap_to_new_meter_keeps_click_grid_and_follows_new_accents() {
    let (port, log) = rec();
    let mut s = Scheduler::start(
        snap(240, MeterMap::default(), &[&port], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    metronome_on(&mut s);
    s.play(Tick::ZERO).unwrap();
    // 틱 약 2300(박 0·960·1920은 보냈고 2880은 아직).
    sleep_ms(600);
    s.swap(snap(240, MeterMap::new(sig(3, 4)), &[&port], vec![]))
        .unwrap();
    // 틱 6240쯤까지.
    sleep_ms(1025);
    s.stop().unwrap();
    sleep_ms(50);
    drop(s);
    let got = clicks(&log);
    let want = [
        (0, 37),
        (960, 42),
        (1920, 42),
        (2880, 37),
        (3840, 42),
        (4800, 42),
        (5760, 37),
    ];
    assert_clicks_240(&got[..want.len().min(got.len())], &want);
    assert_eq!(got.len(), want.len(), "{got:?}");
}

/// 클릭 포트 번호의 포트를 바꾼 스냅샷으로 교체해도 옛 포트·새 포트 모두에 노트가 남지 않는다(회귀).
#[test]
fn swap_changing_metronome_port_leaves_no_note_sounding() {
    let (a, log_a) = rec();
    let (b, log_b) = rec();
    let mut s = Scheduler::start(
        snap(240, MeterMap::default(), &[&a], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_metronome(MetronomeConfig::default().with_length(Duration::from_millis(200)))
        .unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    for k in 0..6u64 {
        sleep_ms(40 + k * 23);
        let port = if k % 2 == 0 { &b } else { &a };
        s.swap(snap(240, MeterMap::default(), &[port], vec![]))
            .unwrap();
        s.preview(
            0,
            Channel::DRUMS,
            pitch(42),
            vel(),
            Duration::from_millis(80),
        )
        .unwrap();
    }
    sleep_ms(30);
    s.stop().unwrap();
    sleep_ms(400);
    drop(s);
    assert!(
        sounding(&log_a).is_empty(),
        "옛 포트: {:?}",
        sounding(&log_a)
    );
    assert!(
        sounding(&log_b).is_empty(),
        "새 포트: {:?}",
        sounding(&log_b)
    );
}

/// 같은 틱이면 이벤트, 클릭, Clock 순으로 넘긴다(보고서의 순서, 회귀).
#[test]
fn same_tick_event_then_click_then_clock() {
    let (port, log) = rec();
    let events = vec![ScheduledEvent::new(
        Tick::new(960),
        0,
        note_on(ch(1), pitch(60), vel()),
    )];
    let mut s = Scheduler::start(
        snap(240, MeterMap::default(), &[&port], events),
        SchedulerConfig::default(),
    )
    .unwrap();
    metronome_on(&mut s);
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(400);
    s.stop().unwrap();
    sleep_ms(50);
    drop(s);
    let msgs = log.lock().unwrap().clone();
    let at = msgs
        .iter()
        .find(|(_, b)| b.as_slice() == [0x90, 60, 100])
        .map(|m| m.0)
        .unwrap();
    let same: Vec<u8> = msgs
        .iter()
        .filter(|(t, b)| *t == at && (is_on(b) || b[0] == 0xF8))
        .map(|(_, b)| b[0])
        .collect();
    assert_eq!(same, vec![0x90, 0x99, 0xF8], "같은 시각의 순서");
}

/// 클릭 길이(10 s 상한)가 박 간격보다 길어도 클릭이 밀리지 않고, 정지 뒤 남는 노트가 없다(회귀).
#[test]
fn click_longer_than_beat_stays_on_grid() {
    let (port, log) = rec();
    let mut s = Scheduler::start(
        snap(240, MeterMap::default(), &[&port], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_metronome(MetronomeConfig::default().with_length(Duration::from_mins(1)))
        .unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(1400);
    s.stop().unwrap();
    sleep_ms(50);
    drop(s);
    let got = clicks(&log);
    let want: Vec<(u64, u8)> = (0..6u64)
        .map(|k| (k * 960, if k % 4 == 0 { 37 } else { 42 }))
        .collect();
    assert_clicks_240(&got, &want);
    assert!(sounding(&log).is_empty(), "{:?}", sounding(&log));
}

/// 박자표 변화가 마디 중간(짧은 마디)이고 루프 끝이 박 위여도: 루프 끝 박은 울리지 않고, 되감은 뒤
/// 같은 박이 같은 간격으로 되풀이된다(회귀).
#[test]
fn loop_over_short_bar_repeats_same_clicks() {
    let (port, log) = rec();
    let mut meter = MeterMap::default();
    meter.set(Tick::new(1440), sig(3, 4));
    let mut s = Scheduler::start(
        snap(240, meter, &[&port], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    metronome_on(&mut s);
    s.set_loop(Some((Tick::new(480), Tick::new(2400)))).unwrap();
    s.play(Tick::new(480)).unwrap();
    // 한 바퀴 500 ms. 세 바퀴와 조금.
    sleep_ms(1560);
    s.stop().unwrap();
    sleep_ms(50);
    drop(s);
    let got = clicks(&log);
    assert!(got.len() >= 6, "{got:?}");
    let notes: Vec<u8> = got.iter().map(|c| c.1).take(6).collect();
    assert_eq!(notes, vec![42, 37, 42, 37, 42, 37]);
    // 960→1440: 480틱, 1440→(2400=480)→960: 1440틱.
    for w in got.windows(2).take(5) {
        let d = ns_between(w[0].0, w[1].0);
        let want = if w[0].1 == 42 {
            ns_of_ticks_240(480)
        } else {
            ns_of_ticks_240(1440)
        };
        assert!((d - want).abs() <= 2_000, "간격 {d} ns, 기대 {want}");
    }
}

/// 틱 범위 끝 근처에서 재생해도 스케줄러가 죽지 않는다(넘침, 회귀). 죽었으면 `panic`이 `Err`다.
#[test]
fn play_near_tick_max_keeps_scheduler_alive() {
    let (port, log) = rec();
    let mut meter = MeterMap::default();
    meter.set(Tick::new(u64::MAX - 7), sig(7, 256));
    meter.set(Tick::new(u64::MAX), sig(1, 1));
    let mut s = Scheduler::start(
        snap(240, meter, &[&port], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    metronome_on(&mut s);
    s.set_loop(Some((Tick::new(u64::MAX - 20), Tick::new(u64::MAX))))
        .unwrap();
    for from in [u64::MAX, u64::MAX - 1, u64::MAX - 7, u64::MAX - 30] {
        s.play(Tick::new(from)).unwrap();
        sleep_ms(20);
        s.swap(snap(240, MeterMap::new(sig(5, 8)), &[&port], vec![]))
            .unwrap();
        sleep_ms(20);
        while s.poll_beat().is_some() {}
    }
    s.stop().unwrap();
    sleep_ms(30);
    assert!(s.panic().is_ok(), "스케줄러 스레드가 끝났다");
    drop(s);
    assert!(sounding(&log).is_empty());
}

/// UI가 박 큐를 오래 비우지 않아 넘쳐도 스케줄러는 멈추지 않고, 비운 뒤에는 새 박이 다시 온다(MET-2,
/// 회귀).
#[test]
fn beat_queue_overflow_recovers_after_drain() {
    let (port, _log) = rec();
    // 4/256: 박 15틱, 240 BPM에서 약 3.9 ms. 1.5 s면 384박으로 큐(256)를 넘긴다.
    let mut s = Scheduler::start(
        snap(240, MeterMap::new(sig(4, 256)), &[&port], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(1500);
    let mut drained = 0;
    let mut last_bar = 0;
    while let Some(b) = s.poll_beat() {
        drained += 1;
        last_bar = b.bar();
    }
    assert_eq!(drained, 256, "큐 크기만큼");
    sleep_ms(100);
    let fresh = s.poll_beat().expect("비운 뒤 새 박");
    assert!(fresh.bar() > last_bar + 10, "{} vs {last_bar}", fresh.bar());
    s.stop().unwrap();
}

/// 끄기 대기 표(64칸)를 클릭이 도는 동안 미리듣기로 넘쳐도 남는 노트가 없다(회귀).
#[test]
fn preview_overflow_while_clicking_leaves_no_note_sounding() {
    let (port, log) = rec();
    let mut s = Scheduler::start(
        snap(240, MeterMap::default(), &[&port], vec![]),
        SchedulerConfig::default(),
    )
    .unwrap();
    metronome_on(&mut s);
    s.play(Tick::ZERO).unwrap();
    for k in 0..150u8 {
        let n = 30 + k % 90;
        s.preview(0, ch(1 + k % 16), pitch(n), vel(), Duration::from_secs(5))
            .unwrap();
        if k % 20 == 0 {
            sleep_ms(30);
        }
    }
    sleep_ms(200);
    s.stop().unwrap();
    sleep_ms(100);
    drop(s);
    assert!(sounding(&log).is_empty(), "{:?}", sounding(&log));
}
