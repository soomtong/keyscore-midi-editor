//! S3-T8 적대적 리뷰 3회차의 깨뜨리기 시험(`docs/reports/s3-t8/review-3.md`).
//!
//! 2회차 뒤의 `paused_playing` 분리를 지나, 아직 보지 않은 길을 본다: 재생 직후·짧은 간격의 재생·일시정지
//! 반복(교체 섞음), 루프 되감기 뒤의 일시정지. 「OS에 넘김」은 `MidiPort::send` 호출이다. 앞 회차와 같이
//! 할당 없이 기록하는 가짜 포트로 받는다.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use keyscore_core::{Channel, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, MidiError, MidiPort, PauseReply, ScheduledEvent, Scheduler, SchedulerConfig,
    Snapshot, note_off, note_on,
};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

const CAP: usize = 1 << 15;

struct Recorder {
    words: Box<[AtomicU32]>,
    len: AtomicUsize,
}

impl Recorder {
    fn new() -> Arc<Self> {
        Arc::new(Recorder {
            words: (0..CAP).map(|_| AtomicU32::new(0)).collect(),
            len: AtomicUsize::new(0),
        })
    }

    fn words(&self) -> Vec<u32> {
        (0..self.len.load(Ordering::Acquire).min(CAP))
            .map(|i| self.words[i].load(Ordering::Relaxed))
            .collect()
    }
}

struct Port(Arc<Recorder>);

impl MidiPort for Port {
    fn name(&self) -> String {
        "review-s3-t8-r3".to_owned()
    }

    fn send(&self, _at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        let i = self.0.len.load(Ordering::Relaxed);
        if i < CAP {
            self.0.words[i].store(ump[0], Ordering::Relaxed);
        }
        self.0.len.store(i + 1, Ordering::Release);
        Ok(())
    }
}

fn is_note_on(w: u32) -> bool {
    (w >> 16) & 0xF0 == 0x90 && w & 0x7F > 0
}

fn pitch_of(w: u32) -> u8 {
    ((w >> 8) & 0x7F) as u8
}

fn ev(tick: u64, msg: keyscore_midi::Midi1Message) -> ScheduledEvent {
    ScheduledEvent::new(Tick::new(tick), 0, msg)
}

/// `i`번째 노트는 틱 `i × gap`, 음 `20 + i`.
fn numbered(count: u8, gap: u64) -> Vec<ScheduledEvent> {
    let c = Channel::new(1).unwrap();
    (0..count)
        .flat_map(|i| {
            let t = u64::from(i) * gap;
            let p = Pitch::new(20 + i).unwrap();
            [
                ev(t, note_on(c, p, Velocity::from_midi1(100).unwrap())),
                ev(t + gap / 2, note_off(c, p, Velocity::new(0))),
            ]
        })
        .collect()
}

fn snapshot(rec: &Arc<Recorder>, events: Vec<ScheduledEvent>) -> Arc<Snapshot> {
    let port: Arc<dyn MidiPort> = Arc::new(Port(Arc::clone(rec)));
    Arc::new(
        Snapshot::new(
            TempoMap::new(Tempo::from_bpm(120).unwrap()),
            vec![port],
            events,
        )
        .unwrap(),
    )
}

/// 넘긴 Note On의 번호(`음 - 20`).
fn sent_numbers(rec: &Recorder) -> Vec<u8> {
    rec.words()
        .into_iter()
        .filter(|&w| is_note_on(w))
        .map(|w| pitch_of(w) - 20)
        .collect()
}

/// 결정적인 의사 난수(시험을 다시 돌려도 같은 간격).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

/// 재생 직후(0 ms, 시작 여유 안)부터 lookahead 몇 배까지 섞인 간격으로 재생·일시정지를 수백 번 거듭하고,
/// 때때로 재생 중 같은 스냅샷으로 교체한다. 모든 노트가 순서대로 정확히 한 번 넘어간다.
#[test]
fn rapid_play_pause_cycles_with_swaps_neither_repeat_nor_drop() {
    const COUNT: u8 = 100;
    const GAP: u64 = 30; // 약 15.6 ms
    let _serial = serial();
    let rec = Recorder::new();
    let mut s = Scheduler::start(
        snapshot(&rec, numbered(COUNT, GAP)),
        SchedulerConfig::default(),
    )
    .unwrap();
    let mut rng = Lcg(0x5EED);
    let mut from = Tick::ZERO;
    let mut cycles = 0u32;
    let mut zero_waits = 0u32;
    while from.get() <= u64::from(COUNT - 1) * GAP {
        assert!(cycles < 2000, "끝나지 않는다: 자리 {from:?}");
        cycles += 1;
        s.play(from).unwrap();
        let wait = rng.next(14);
        if wait == 0 {
            zero_waits += 1;
        } else {
            std::thread::sleep(Duration::from_millis(wait));
        }
        if rng.next(4) == 0 {
            s.collect_garbage();
            s.swap(snapshot(&rec, numbered(COUNT, GAP))).unwrap();
            if rng.next(2) == 0 {
                std::thread::sleep(Duration::from_millis(rng.next(4)));
            }
        }
        match s.pause(Duration::from_millis(50)).unwrap() {
            PauseReply::At(at) => {
                assert!(at >= from, "{cycles}회차 자리 {at:?} < 시작 {from:?}");
                from = at;
            }
            other => panic!("{cycles}회차 재생 중이었다: {other:?}"),
        }
    }
    std::thread::sleep(Duration::from_millis(20));
    let got = sent_numbers(&rec);
    let expected: Vec<u8> = (0..COUNT).collect();
    assert_eq!(
        got, expected,
        "{cycles}번 반복(0 ms {zero_waits}번) 뒤 겹침·빠짐"
    );
}

/// 루프 구간 안에서 되감기를 여러 번 지나며 일시정지·이어서 재생한다. 넘긴 노트는 루프 순서(0, 1, …, 15, 0, …)를
/// 겹침·빠짐 없이 잇는다. 이어서 재생할 틱이 루프 끝과 같은 경우(계획 「범위 밖」, S5)는 그 경계만 따로 센다.
#[test]
fn pause_across_loop_wraps_keeps_cyclic_order() {
    const COUNT: u8 = 16;
    const GAP: u64 = 60; // 약 31 ms, 루프 한 바퀴 약 0.5 s
    let _serial = serial();
    let end = u64::from(COUNT) * GAP;
    let rec = Recorder::new();
    let mut s = Scheduler::start(
        snapshot(&rec, numbered(COUNT, GAP)),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.set_loop(Some((Tick::ZERO, Tick::new(end)))).unwrap();
    let mut rng = Lcg(0xA11CE);
    let mut from = Tick::ZERO;
    let mut at_loop_end = 0u32;
    for round in 0..60u32 {
        s.play(from).unwrap();
        std::thread::sleep(Duration::from_millis(5 + rng.next(60)));
        match s.pause(Duration::from_millis(50)).unwrap() {
            PauseReply::At(at) => {
                assert!(at.get() <= end, "{round}회차 자리 {at:?}가 루프 밖");
                if at.get() == end {
                    // S5 경계: 루프 끝에서 이으면 루프가 걸리지 않는다. 여기서는 되감은 자리로 잇는다.
                    at_loop_end += 1;
                    from = Tick::ZERO;
                } else {
                    from = at;
                }
            }
            other => panic!("{round}회차 재생 중이었다: {other:?}"),
        }
    }
    std::thread::sleep(Duration::from_millis(20));
    let got = sent_numbers(&rec);
    assert!(
        got.len() > usize::from(COUNT) * 2,
        "되감기를 지나지 않았다: {got:?}"
    );
    for (i, pair) in got.windows(2).enumerate() {
        assert_eq!(
            pair[1],
            (pair[0] + 1) % COUNT,
            "{i}번째에서 루프 순서가 끊겼다(루프 끝 경계 {at_loop_end}번): {got:?}"
        );
    }
    assert_eq!(got[0], 0);
    eprintln!("루프 끝 경계 {at_loop_end}번, 넘긴 노트 {}개", got.len());
}

/// 계획 「범위 밖」(S5)의 경계가 공개 API로 실제 닿는지 확인한다. 루프 끝(틱 960, 500 ms) 앞의 lookahead 안에서
/// 일시정지하면 이어서 재생할 틱이 루프 끝과 같다. 그 틱에서 `play`하면 루프가 걸리지 않는다(`loop_end`가
/// `end > base_tick`만 본다). 앱은 아직 루프를 걸지 않으므로(`set_loop` 호출 없음) S5에서 다룬다는 근거는 맞다.
#[test]
fn pause_just_before_loop_end_can_return_loop_end() {
    let _serial = serial();
    let end = Tick::new(960);
    let rec = Recorder::new();
    // 노트는 앞 절반(0–420)에만. 그 뒤 다음 항목은 루프 끝이다.
    let mut s =
        Scheduler::start(snapshot(&rec, numbered(8, 60)), SchedulerConfig::default()).unwrap();
    s.set_loop(Some((Tick::ZERO, end))).unwrap();
    let mut hits = 0;
    for i in 0..24u64 {
        s.play(Tick::new(480)).unwrap();
        // 틱 480 → 960은 250 ms. 시작 여유를 넘겨 그 끝 직전(238–250 ms)을 고른다.
        std::thread::sleep(Duration::from_micros(238_000 + i * 500));
        let reply = s.pause(Duration::from_millis(50)).unwrap();
        if reply == PauseReply::At(end) {
            hits += 1;
        }
        assert!(reply.tick().is_some_and(|t| t <= end), "{i}회차 {reply:?}");
    }
    eprintln!("루프 끝과 같은 자리 {hits}/24");
    assert!(hits > 0, "루프 끝 경계에 닿지 못했다");
}
