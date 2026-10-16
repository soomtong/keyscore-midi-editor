//! S1-T5 적대적 리뷰 2회차 테스트(메트로놈·미리듣기 확인 리뷰, MET-1, PRE-1).
//!
//! 이 파일 안의 기록 포트(`Rec`)만 쓴다. CoreMIDI 클라이언트를 만들지 않는다. 신디가 듣는 순서는
//! 「타임스탬프 순, 같은 시각이면 넘긴 순」이라고 보고, 그 순서로 (채널, 노트)마다 Note On과 Note Off가
//! 번갈아 오는지, 끝에 울리는 노트가 없는지 본다.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use keyscore_core::{Channel, MeterMap, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, MetronomeConfig, MidiError, MidiPort, Scheduler, SchedulerConfig, Snapshot,
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
    fn name(&self) -> &'static str {
        "rec"
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

fn snap(bpm: u32, port: &Arc<dyn MidiPort>) -> Arc<Snapshot> {
    Arc::new(
        Snapshot::new(
            TempoMap::new(Tempo::from_bpm(bpm).unwrap()),
            vec![Arc::clone(port)],
            vec![],
        )
        .unwrap()
        .with_meter(MeterMap::default()),
    )
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

/// 타임스탬프 순(같으면 넘긴 순)으로 재생해 (채널, 노트)마다 Note On·Off가 번갈아 오는지 본다.
/// 울리는 중에 다시 오는 Note On은 위반이다(앞 노트의 Note Off가 새 노트를 끊는다). 이미 꺼진 노트의
/// Note Off는 무해하므로 세지 않는다. All Sound Off(120)·All Notes Off(123)는 그 채널을 끈다.
/// 돌려주는 값: (위반 목록, 끝에 울리는 노트).
fn check(log: &Log) -> (Vec<String>, Vec<(u8, u8)>) {
    let mut msgs = log.lock().unwrap().clone();
    msgs.sort_by_key(|m| m.0);
    let mut on: BTreeMap<(u8, u8), u64> = BTreeMap::new();
    let mut bad = Vec::new();
    for (t, b) in &msgs {
        if b.len() != 3 {
            continue;
        }
        let chan = b[0] & 0x0F;
        let kind = b[0] & 0xF0;
        if kind == 0x90 && b[2] > 0 {
            if let Some(prev) = on.insert((chan, b[1]), *t) {
                bad.push(format!(
                    "채널 {chan} 노트 {}: 시각 {prev}에 켠 노트가 꺼지기 전 시각 {t}에 다시 켜짐",
                    b[1]
                ));
            }
        } else if kind == 0x80 || kind == 0x90 {
            on.remove(&(chan, b[1]));
        } else if kind == 0xB0 && (b[1] == 120 || b[1] == 123) {
            on.retain(|&(c, _), _| c != chan);
        }
    }
    (bad, on.into_keys().collect())
}

fn assert_clean(log: &Log) {
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
}

/// 클릭 노트를 강박·약박 모두 42로(같은 노트가 박마다 다시 켜진다).
fn same_note_clicks(length: Duration) -> MetronomeConfig {
    MetronomeConfig::new(0, Channel::DRUMS, pitch(42), pitch(42)).with_length(length)
}

/// 넘긴 끄기 때문에 미래로 미룬 미리듣기 Note On(짝 끄기는 아직 대기 표에 있음)을 짧은 미리듣기로 다시
/// 켜도 교대가 맞고 남는 노트가 없다(지적 1의 둘째 경로).
#[test]
fn retrigger_of_delayed_preview_keeps_on_off_alternating() {
    let (port, log) = rec();
    let mut s = Scheduler::start(snap(120, &port), lookahead(500)).unwrap();
    for round in 0..8u64 {
        let n = pitch(60);
        // 끄기가 창 안이라 바로 넘어간다.
        s.preview(0, ch(1), n, vel(), Duration::from_millis(40 + round * 10))
            .unwrap();
        sleep_ms(3);
        // 넘긴 끄기 뒤로 미룬 Note On, 끄기는 창 밖(대기).
        s.preview(0, ch(1), n, vel(), Duration::from_secs(2))
            .unwrap();
        // 미룬 Note On보다 앞인 지금에 다시 켠다.
        s.preview(0, ch(1), n, vel(), Duration::from_millis(1 + round))
            .unwrap();
        s.preview(0, ch(1), n, vel(), Duration::from_millis(1))
            .unwrap();
        sleep_ms(round * 7);
    }
    sleep_ms(700);
    drop(s);
    assert_clean(&log);
}

/// 클릭 Note On이 lookahead로 미래에 넘어가 있는 동안 서로 다른 미리듣기로 끄기 대기 표(64칸)를 넘쳐도
/// 클릭 노트의 끄기가 그 Note On보다 앞서지 않는다.
#[test]
fn overflow_while_click_note_on_is_in_future_keeps_alternating() {
    let (port, log) = rec();
    let mut s = Scheduler::start(snap(120, &port), lookahead(500)).unwrap();
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(20);
    for k in 0..100u8 {
        // 37·42(클릭 노트)를 피한 채널 1의 노트.
        s.preview(0, ch(1), pitch(k), vel(), Duration::from_secs(3))
            .unwrap();
    }
    sleep_ms(60);
    s.stop().unwrap();
    sleep_ms(800);
    drop(s);
    assert_clean(&log);
}

/// 클릭 길이(1 s)가 박 간격(250 ms)보다 길고 강박·약박이 같은 노트여도 교대가 맞고 남는 노트가 없다.
/// lookahead 기본값과 500 ms 둘 다.
#[test]
fn same_note_click_longer_than_beat_keeps_alternating() {
    for config in [SchedulerConfig::default(), lookahead(500)] {
        let (port, log) = rec();
        let mut s = Scheduler::start(snap(240, &port), config).unwrap();
        s.set_metronome(same_note_clicks(Duration::from_secs(1)))
            .unwrap();
        s.set_metronome_enabled(true).unwrap();
        s.play(Tick::ZERO).unwrap();
        sleep_ms(1300);
        s.stop().unwrap();
        sleep_ms(1600);
        drop(s);
        assert_clean(&log);
    }
}

/// 클릭 노트와 같은 노트를 클릭 앞뒤 여러 지점에서, 여러 길이로 미리듣기한다(훑기). 중간에 정지·패닉·
/// 위치 이동을 섞는다.
#[test]
fn preview_of_click_note_sweep_with_stop_and_panic_keeps_alternating() {
    for config in [SchedulerConfig::default(), lookahead(120)] {
        let (port, log) = rec();
        let mut s = Scheduler::start(snap(240, &port), config).unwrap();
        s.set_metronome(same_note_clicks(Duration::from_millis(30)))
            .unwrap();
        s.set_metronome_enabled(true).unwrap();
        s.play(Tick::ZERO).unwrap();
        let lengths = [1u64, 3, 20, 90, 300];
        for k in 0..120u64 {
            let len = Duration::from_millis(lengths[usize::try_from(k % 5).unwrap()]);
            s.preview(0, Channel::DRUMS, pitch(42), vel(), len).unwrap();
            // 7 ms와 박 250 ms는 서로소에 가까워 클릭 기준 위상이 고루 퍼진다.
            sleep_ms(7);
            match k {
                40 => s.stop().unwrap(),
                41 => s.play(Tick::new(123)).unwrap(),
                70 => s.panic().unwrap(),
                90 => s.locate(Tick::new(5000)).unwrap(),
                _ => {}
            }
        }
        s.stop().unwrap();
        sleep_ms(700);
        drop(s);
        assert_clean(&log);
    }
}

/// 넘긴 끄기(창 안이라 이미 `CoreMIDI`에 넘긴 Note Off)의 칸이 표 넘침으로 비워진 뒤 같은 노트를 다시
/// 켜면, 새 Note On이 그 넘긴 Note Off보다 앞에 간다. 넘긴 Note Off가 새 노트를 끊고(교대 위반),
/// `play_note` 주석의 「새 노트가 앞 노트의 Note Off로 끊기지 않는다」가 깨진다.
#[test]
fn retrigger_after_overflow_evicts_sent_off_keeps_alternating() {
    let (port, log) = rec();
    let mut s = Scheduler::start(snap(120, &port), lookahead(500)).unwrap();
    // 끄기(100 ms 뒤)가 창 안이라 다음 바퀴에 넘어간다.
    s.preview(0, ch(1), pitch(60), vel(), Duration::from_millis(100))
        .unwrap();
    sleep_ms(5);
    // 다른 노트 64개(끄기 3 s 뒤, 창 밖)로 표를 넘친다. 가장 이른 칸은 노트 60의 넘긴 끄기다.
    for k in 0..64u8 {
        s.preview(0, ch(2), pitch(k), vel(), Duration::from_secs(3))
            .unwrap();
    }
    // 노트 60의 넘긴 끄기(약 95 ms 뒤)보다 앞에 다시 켠다.
    s.preview(0, ch(1), pitch(60), vel(), Duration::from_millis(300))
        .unwrap();
    sleep_ms(3600);
    drop(s);
    assert_clean(&log);
}

/// 정지가 lookahead로 미리 넘긴 클릭 Note On 시각(미래)에 Note Off를 예약한 뒤, 그 시각 앞에서 같은
/// 노트를 미리듣기하면 미리듣기 Note On이 지금에 나가고, 정지의 Note Off가 미리듣기 노트를 끊는다.
/// 정지가 보낸 Note Off를 대기 표가 기억하지 않기 때문이다.
#[test]
fn preview_after_stop_with_future_click_keeps_alternating() {
    let (port, log) = rec();
    let mut s = Scheduler::start(snap(120, &port), lookahead(500)).unwrap();
    s.set_metronome(MetronomeConfig::default()).unwrap();
    s.set_metronome_enabled(true).unwrap();
    s.play(Tick::ZERO).unwrap();
    // 박 1(42, 약 500 ms 뒤)의 Note On이 넘어간 뒤 정지한다. 정지의 Note Off는 그 시각에 간다.
    sleep_ms(20);
    s.stop().unwrap();
    sleep_ms(10);
    // 박 1 시각 앞에서, 그보다 길게 미리듣기.
    s.preview(0, Channel::DRUMS, pitch(42), vel(), Duration::from_secs(1))
        .unwrap();
    sleep_ms(1500);
    drop(s);
    assert_clean(&log);
}
