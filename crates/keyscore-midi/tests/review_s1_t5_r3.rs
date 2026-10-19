//! S1-T5 적대적 리뷰 3회차 테스트(리뷰 2 수정 `5128dc5`의 `off_until` 확인, MET-1, PRE-1).
//!
//! 이 파일 안의 기록 포트(`Rec`)만 쓴다. CoreMIDI 클라이언트를 만들지 않는다. 신디가 듣는 순서는
//! 「타임스탬프 순, 같은 시각이면 넘긴 순」이라고 보고 두 가지를 본다.
//!
//! - (채널, 노트)마다 Note On과 Note Off가 번갈아 오고 끝에 울리는 노트가 없다.
//! - 미리듣기(벨로시티 77로 구별)가 설정한 길이보다 일찍 끊기지 않는다. 다만 같은 시각에 같은 노트의
//!   새 Note On이 뒤따르는 끊김(클릭·미리듣기의 다시 켜기)은 설계대로라 허용한다.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use keyscore_core::{Channel, MeterMap, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, MetronomeConfig, Midi1Message, MidiError, MidiPort, ScheduledEvent, Scheduler,
    SchedulerConfig, Snapshot, control_change, note_off, note_on,
};

/// (스케줄러가 넘긴 호스트 타임스탬프, MIDI 1.0 바이트).
type Log = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

/// 미리듣기 노트를 기록에서 구별하는 벨로시티. 클릭·시퀀스 노트는 이 값을 쓰지 않는다.
const PREVIEW_VELOCITY: u8 = 77;

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

fn vel(n: u8) -> Velocity {
    Velocity::from_midi1(n).unwrap()
}

fn snap_with(bpm: u32, port: &Arc<dyn MidiPort>, events: Vec<ScheduledEvent>) -> Arc<Snapshot> {
    Arc::new(
        Snapshot::new(
            TempoMap::new(Tempo::from_bpm(bpm).unwrap()),
            vec![Arc::clone(port)],
            events,
        )
        .unwrap()
        .with_meter(MeterMap::default()),
    )
}

fn ev(tick: u64, msg: Midi1Message) -> ScheduledEvent {
    ScheduledEvent::new(Tick::new(tick), 0, msg)
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn lookahead(ms: u64) -> SchedulerConfig {
    SchedulerConfig {
        lookahead: Duration::from_millis(ms),
        wake: Duration::from_millis(1),
    }
}

/// 강박·약박 모두 42(채널 10)인 클릭. 벨로시티는 미리듣기 표식(77)과 다르게 둔다.
fn same_note_clicks(length: Duration) -> MetronomeConfig {
    MetronomeConfig::new(0, Channel::DRUMS, pitch(42), pitch(42))
        .with_velocities(vel(110), vel(90))
        .with_length(length)
}

fn preview(s: &mut Scheduler, channel: Channel, note: u8, length: Duration) {
    s.preview(0, channel, pitch(note), vel(PREVIEW_VELOCITY), length)
        .unwrap();
}

fn sorted(log: &Log) -> Vec<(u64, Vec<u8>)> {
    let mut msgs = log.lock().unwrap().clone();
    // 안정 정렬이라 같은 시각은 넘긴 순서를 지킨다.
    msgs.sort_by_key(|m| m.0);
    msgs
}

fn is_on(b: &[u8]) -> bool {
    b.len() == 3 && b[0] & 0xF0 == 0x90 && b[2] > 0
}

fn is_off(b: &[u8]) -> bool {
    b.len() == 3 && (b[0] & 0xF0 == 0x80 || (b[0] & 0xF0 == 0x90 && b[2] == 0))
}

/// (채널, 노트)마다 Note On·Off 교대 위반 목록과 끝에 울리는 노트.
fn check(log: &Log) -> (Vec<String>, Vec<(u8, u8)>) {
    let mut on: BTreeMap<(u8, u8), u64> = BTreeMap::new();
    let mut bad = Vec::new();
    for (t, b) in &sorted(log) {
        if b.len() != 3 {
            continue;
        }
        let chan = b[0] & 0x0F;
        if is_on(b) {
            if let Some(prev) = on.insert((chan, b[1]), *t) {
                bad.push(format!(
                    "채널 {chan} 노트 {}: 시각 {prev}에 켠 노트가 꺼지기 전 시각 {t}에 다시 켜짐",
                    b[1]
                ));
            }
        } else if is_off(b) {
            on.remove(&(chan, b[1]));
        } else if b[0] & 0xF0 == 0xB0 && (b[1] == 120 || b[1] == 123) {
            on.retain(|&(c, _), _| c != chan);
        }
    }
    (bad, on.into_keys().collect())
}

/// 미리듣기 Note On(벨로시티 77)마다 다음 Note Off까지가 `length`(1 ms 여유)보다 짧은 것. 끊은 Note Off
/// 바로 뒤에 같은 시각의 같은 노트 Note On이 오면(다시 켜기) 허용한다.
fn short_previews(log: &Log, length: Duration) -> Vec<String> {
    let msgs = sorted(log);
    let floor = length.saturating_sub(Duration::from_millis(1));
    let mut bad = Vec::new();
    for (i, (t, b)) in msgs.iter().enumerate() {
        if !(is_on(b) && b[2] == PREVIEW_VELOCITY) {
            continue;
        }
        let same = |m: &[u8]| m.len() == 3 && m[0] & 0x0F == b[0] & 0x0F && m[1] == b[1];
        let Some(j) = (i + 1..msgs.len()).find(|&j| is_off(&msgs[j].1) && same(&msgs[j].1)) else {
            bad.push(format!(
                "노트 {}: 시각 {t}의 미리듣기에 Note Off 없음",
                b[1]
            ));
            continue;
        };
        let off_t = msgs[j].0;
        let held = HostTime::from_ticks(off_t).duration_since(HostTime::from_ticks(*t));
        if held >= floor {
            continue;
        }
        let retrigger = msgs[j + 1..]
            .iter()
            .take_while(|m| m.0 == off_t)
            .any(|m| is_on(&m.1) && same(&m.1));
        if !retrigger {
            bad.push(format!(
                "노트 {}: 시각 {t}의 미리듣기가 {held:?} 만에 끊김(설정 {length:?})",
                b[1]
            ));
        }
    }
    bad
}

fn assert_clean(log: &Log, length: Duration) {
    let (bad, left) = check(log);
    assert!(
        left.is_empty(),
        "끝에 울리는 노트(채널 0-15, 노트): {left:?}"
    );
    assert!(
        bad.is_empty(),
        "On/Off 교대 위반 {}건: {:?}",
        bad.len(),
        &bad[..bad.len().min(5)]
    );
    let short = short_previews(log, length);
    assert!(
        short.is_empty(),
        "짧게 끊긴 미리듣기 {}건: {:?}",
        short.len(),
        &short[..short.len().min(5)]
    );
    let count = sorted(log)
        .iter()
        .filter(|(_, b)| is_on(b) && b[2] == PREVIEW_VELOCITY)
        .count();
    assert!(count > 0, "미리듣기 Note On이 하나도 없음");
}

/// 재생 중 위치 이동 직후(새 위치의 박 클릭 Note On이 lookahead로 미래에 넘어가 있음) 클릭 노트와 같은
/// 노트를 미리듣기한다. 박 위·박 사이 위치를 섞는다.
#[test]
fn preview_right_after_locate_with_future_click_keeps_length() {
    let length = Duration::from_millis(120);
    let (port, log) = rec();
    let mut s = Scheduler::start(snap_with(120, &port, vec![]), lookahead(300)).unwrap();
    s.set_metronome(same_note_clicks(Duration::from_millis(50)))
        .unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    for k in 0..8u64 {
        // 위치 이동은 울리는 노트를 끊으므로, 앞 미리듣기(최대 lookahead만큼 미뤄짐)가 끝난 뒤에 옮긴다.
        sleep_ms(500 + (k * 37) % 200);
        // 짝수는 박 위(960의 배수), 홀수는 박 사이.
        let to = 960 * k + if k % 2 == 0 { 0 } else { 100 + k * 30 };
        s.locate(Tick::new(to)).unwrap();
        sleep_ms(k % 3);
        preview(&mut s, Channel::DRUMS, 42, length);
    }
    sleep_ms(400);
    s.stop().unwrap();
    sleep_ms(600);
    drop(s);
    assert_clean(&log, length);
}

/// 한 마디(2 박, 1 s) 루프를 도는 동안 클릭 노트와 같은 노트를 71 ms 간격으로 미리듣기한다. 루프 되감기
/// 직전·직후(되감기의 끄기와 새 바퀴 박 0 클릭이 미래로 넘어가 있음)를 모두 지난다.
#[test]
fn preview_around_loop_wrap_with_future_click_keeps_length() {
    let length = Duration::from_millis(60);
    for config in [SchedulerConfig::default(), lookahead(300)] {
        let (port, log) = rec();
        let mut s = Scheduler::start(snap_with(120, &port, vec![]), config).unwrap();
        s.set_metronome(same_note_clicks(Duration::from_millis(50)))
            .unwrap();
        s.set_metronome_enabled(true).unwrap();
        s.set_loop(Some((Tick::ZERO, Tick::new(1920)))).unwrap();
        s.play(Tick::ZERO).unwrap();
        for _ in 0..50 {
            preview(&mut s, Channel::DRUMS, 42, length);
            sleep_ms(71);
        }
        // 마지막 미리듣기가 정지로 끊기지 않게 기다린다.
        sleep_ms(200);
        s.stop().unwrap();
        sleep_ms(600);
        drop(s);
        assert_clean(&log, length);
    }
}

/// 재생 중 교체가 울리는 시퀀스 노트를 미래 시각(`max(now, last_sent)`)에 끈 직후, 같은 노트를 그보다
/// 길게 미리듣기한다. 미리듣기는 교체의 Note Off 뒤에 켜져야 끊기지 않는다.
#[test]
fn preview_right_after_swap_releasing_note_keeps_length() {
    let length = Duration::from_millis(800);
    for round in 0..3u64 {
        let (port, log) = rec();
        // 채널 2의 CC를 25 ms(48틱)마다 둬 `last_sent`가 lookahead 끝까지 앞서 있게 한다.
        let cc_stream = |n: u64| -> Vec<ScheduledEvent> {
            (0..n)
                .map(|i| {
                    let v = u32::try_from(i % 128).unwrap();
                    ev(i * 48, control_change(ch(2), 7, v << 25).unwrap())
                })
                .collect()
        };
        let mut a = cc_stream(160);
        a.push(ev(0, note_on(ch(1), pitch(60), vel(100))));
        a.push(ev(7680, note_off(ch(1), pitch(60), vel(0))));
        let b = cc_stream(160);
        let mut s = Scheduler::start(snap_with(120, &port, a), lookahead(500)).unwrap();
        s.play(Tick::ZERO).unwrap();
        sleep_ms(100 + round * 40);
        s.swap(snap_with(120, &port, b)).unwrap();
        sleep_ms(round);
        preview(&mut s, ch(1), 60, length);
        sleep_ms(1600);
        s.stop().unwrap();
        sleep_ms(600);
        drop(s);
        assert_clean(&log, length);
    }
}

/// lookahead로 미리 넘긴 시퀀스 노트의 Note Off(스냅샷 이벤트)가 있는 동안 같은 노트를 미리듣기한다.
/// 시퀀스 노트가 끝난 뒤(정지 상태에서 넘긴 끄기만 남은 경우 포함) 같은 노트 미리듣기가 그 Note Off에
/// 끊기지 않는다.
#[test]
fn preview_after_scheduled_sequence_note_off_keeps_length() {
    let length = Duration::from_millis(300);
    for stop_first in [false, true] {
        let (port, log) = rec();
        // 노트 60: 0틱~480틱(250 ms). 그 뒤 960틱마다 짧은 노트 60을 되풀이한다.
        let mut events = Vec::new();
        for k in 0..6u64 {
            events.push(ev(k * 1920, note_on(ch(1), pitch(60), vel(100))));
            events.push(ev(k * 1920 + 480, note_off(ch(1), pitch(60), vel(0))));
        }
        let mut s = Scheduler::start(snap_with(120, &port, events), lookahead(500)).unwrap();
        s.play(Tick::ZERO).unwrap();
        // 첫 노트의 On·Off(약 250 ms 뒤)가 모두 넘어간 뒤.
        sleep_ms(30);
        if stop_first {
            s.stop().unwrap();
        }
        preview(&mut s, ch(1), 60, length);
        sleep_ms(700);
        s.stop().unwrap();
        sleep_ms(600);
        drop(s);
        assert_clean(&log, length);
    }
}
