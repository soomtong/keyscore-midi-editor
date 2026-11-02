//! S3-T8 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s3-t8/review-1.md`).
//!
//! 「OS에 넘김」은 `MidiPort::send` 호출이다. CoreMIDI 대신 할당 없이 기록하는 가짜 포트로 받아, 일시정지가
//! 돌려준 틱과 그때까지 넘긴 이벤트를 정확히 비교한다. 교체·재생 직후·위치 이동 직후·곡 끝 뒤·교체로 막힌 큐·
//! 항목 상한에 걸린 바퀴·가득 찬 큐에서 본다.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use keyscore_core::{Channel, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, MidiError, MidiPort, ScheduledEvent, Scheduler, SchedulerConfig, Snapshot, note_off,
    note_on,
};

/// 시각을 재는 시험이라 서로 겹치지 않게 돈다.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

const CAP: usize = 1 << 17;
const GAP: u64 = 60;

/// 스케줄러 스레드에서 할당·락 없이 넘긴 워드를 적는다.
struct Recorder {
    words: Box<[AtomicU32]>,
    hosts: Box<[AtomicU64]>,
    len: AtomicUsize,
}

impl Recorder {
    fn new() -> Arc<Self> {
        Arc::new(Recorder {
            words: (0..CAP).map(|_| AtomicU32::new(0)).collect(),
            hosts: (0..CAP).map(|_| AtomicU64::new(0)).collect(),
            len: AtomicUsize::new(0),
        })
    }

    fn len(&self) -> usize {
        self.len.load(Ordering::Acquire).min(CAP)
    }

    /// 지금까지 넘긴 워드.
    fn words(&self) -> Vec<u32> {
        (0..self.len())
            .map(|i| self.words[i].load(Ordering::Relaxed))
            .collect()
    }
}

struct Port(Arc<Recorder>);

impl MidiPort for Port {
    fn name(&self) -> String {
        "review-s3-t8".to_owned()
    }

    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        let i = self.0.len.load(Ordering::Relaxed);
        if i < CAP {
            self.0.words[i].store(ump[0], Ordering::Relaxed);
            self.0.hosts[i].store(at.ticks(), Ordering::Relaxed);
        }
        self.0.len.store(i + 1, Ordering::Release);
        Ok(())
    }
}

fn status(w: u32) -> u8 {
    ((w >> 16) & 0xFF) as u8
}

fn d1(w: u32) -> u8 {
    ((w >> 8) & 0x7F) as u8
}

fn d2(w: u32) -> u8 {
    (w & 0x7F) as u8
}

fn is_note_on(w: u32) -> bool {
    status(w) & 0xF0 == 0x90 && d2(w) > 0
}

fn ch(n: u8) -> Channel {
    Channel::new(n).unwrap()
}

fn ev(tick: u64, msg: keyscore_midi::Midi1Message) -> ScheduledEvent {
    ScheduledEvent::new(Tick::new(tick), 0, msg)
}

/// `i`번째 노트는 틱 `i × gap`, 음 `20 + i`.
fn numbered(count: u8, gap: u64) -> Vec<ScheduledEvent> {
    (0..count)
        .flat_map(|i| {
            let t = u64::from(i) * gap;
            [
                ev(
                    t,
                    note_on(
                        ch(1),
                        Pitch::new(20 + i).unwrap(),
                        Velocity::from_midi1(100).unwrap(),
                    ),
                ),
                ev(
                    t + gap / 2,
                    note_off(ch(1), Pitch::new(20 + i).unwrap(), Velocity::new(0)),
                ),
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

/// 넘긴 Note On의 틱(`numbered`의 음 번호에서 거꾸로 구한다).
fn sent_on_ticks(rec: &Recorder, gap: u64) -> Vec<u64> {
    rec.words()
        .into_iter()
        .filter(|&w| is_note_on(w))
        .map(|w| u64::from(d1(w) - 20) * gap)
        .collect()
}

fn settle() {
    std::thread::sleep(Duration::from_millis(20));
}

/// 재생 중 교체를 거듭한 뒤 일시정지해도 자리는 「넘기지 않은 첫 틱」이고, 이어서 재생하면 겹침도 빠짐도 없다.
#[test]
fn pause_after_swaps_neither_repeats_nor_drops() {
    let _serial = serial();
    let rec = Recorder::new();
    let mut s = Scheduler::start(
        snapshot(&rec, numbered(64, GAP)),
        SchedulerConfig::default(),
    )
    .unwrap();
    let mut from = Tick::ZERO;
    for i in 0..10u64 {
        s.play(from).unwrap();
        for j in 0..=(i % 3) {
            std::thread::sleep(Duration::from_millis(23 + 5 * j + i));
            s.collect_garbage();
            s.swap(snapshot(&rec, numbered(64, GAP))).unwrap();
        }
        // 교체 바로 뒤(같은 바퀴일 수 있다)에 누른다.
        let at = s
            .pause(Duration::from_millis(50))
            .unwrap()
            .expect("재생 중이었다");
        settle();
        let ticks = sent_on_ticks(&rec, GAP);
        assert!(
            ticks.iter().all(|&t| t < at.get()),
            "{i}회차 자리 {at:?} 이상의 노트를 넘겼다: {ticks:?}"
        );
        from = at;
    }
    s.play(from).unwrap();
    std::thread::sleep(Duration::from_millis(2500));
    let mut ons: Vec<u8> = rec
        .words()
        .into_iter()
        .filter(|&w| is_note_on(w))
        .map(d1)
        .collect();
    let in_order = ons.clone();
    ons.dedup();
    assert_eq!(ons.len(), in_order.len(), "겹쳤다: {in_order:?}");
    assert_eq!(in_order, (20..84).collect::<Vec<u8>>(), "겹치거나 빠졌다");
}

/// 재생·위치 이동 직후(같은 바퀴) 일시정지하면 시작 자리 이상이고, 그 앞의 노트는 넘기지 않았다.
#[test]
fn pause_right_after_play_or_locate_returns_start() {
    let _serial = serial();
    let rec = Recorder::new();
    let mut s = Scheduler::start(
        snapshot(&rec, numbered(64, GAP)),
        SchedulerConfig::default(),
    )
    .unwrap();
    for round in 0..20u64 {
        let from = Tick::new(round * 37);
        let before = rec.len();
        if round % 2 == 0 {
            s.play(from).unwrap();
        } else {
            s.play(Tick::new(3000)).unwrap();
            s.locate(from).unwrap();
        }
        let at = s
            .pause(Duration::from_millis(50))
            .unwrap()
            .expect("재생 중이었다");
        settle();
        assert!(at >= from, "{round}회차 자리 {at:?}가 시작 {from:?} 앞이다");
        let after: Vec<u64> = rec.words()[before..]
            .iter()
            .filter(|&&w| is_note_on(w))
            .map(|&w| u64::from(d1(w) - 20) * GAP)
            .collect();
        assert!(
            after.iter().all(|&t| t >= from.get() && t < at.get()),
            "{round}회차 자리 {at:?}, 시작 {from:?}, 넘긴 노트 {after:?}"
        );
    }
}

/// 곡 끝에서 스스로 멈춘 뒤의 일시정지는 `None`이다(재생 중이 아니었다). 앱은 이때 추정 자리를 쓴다.
#[test]
fn pause_after_song_end_returns_none() {
    let _serial = serial();
    let rec = Recorder::new();
    let snap = Arc::new(
        Arc::try_unwrap(snapshot(&rec, numbered(2, 240)))
            .ok()
            .unwrap()
            .with_end(Some(Tick::new(480))),
    );
    let mut s = Scheduler::start(snap, SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(s.pause(Duration::from_millis(50)).unwrap(), None);
    assert!(s.take_ended(), "곡 끝 신호는 남아 있다");
}

/// 옛 스냅샷을 받지 않은 교체가 큐 머리를 막아도 `pause`는 제때 답한다.
#[test]
fn pause_with_swaps_blocking_queue_head_still_answers() {
    let _serial = serial();
    let rec = Recorder::new();
    let mut s =
        Scheduler::start(snapshot(&rec, numbered(64, 60)), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    for _ in 0..40 {
        s.swap(snapshot(&rec, numbered(64, 60))).unwrap();
    }
    let started = Instant::now();
    let at = s.pause(Duration::from_millis(50)).unwrap();
    assert!(at.is_some(), "답하지 않았다({:?})", started.elapsed());
    settle();
    let ticks = sent_on_ticks(&rec, 60);
    assert!(
        ticks.iter().all(|&t| t < at.unwrap().get()),
        "{at:?} {ticks:?}"
    );
}

/// 같은 틱에 항목 상한(4096)을 넘는 이벤트가 있으면 한 틱을 여러 바퀴에 나눠 넘긴다. 그 사이에 일시정지하면
/// 자리는 그 틱이 되어, 이어서 재생할 때 이미 넘긴 같은 틱의 노트를 다시 넘긴다.
#[test]
fn pause_inside_capped_tick_does_not_resend_its_events() {
    let _serial = serial();
    let rec = Recorder::new();
    // 틱 480에 서로 다른 (채널, 음, 벨로시티) Note On 12288개.
    let mut events = Vec::new();
    for c in 1..=16u8 {
        for p in 0..128u8 {
            for v in 1..=6u8 {
                events.push(ev(
                    480,
                    note_on(
                        ch(c),
                        Pitch::new(p).unwrap(),
                        Velocity::from_midi1(v * 20).unwrap(),
                    ),
                ));
            }
        }
    }
    let total = events.len();
    let mut s = Scheduler::start(snapshot(&rec, events), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    // 틱 480의 첫 바퀴가 넘어가는 순간을 기다린다(약 240 ms 뒤).
    let deadline = Instant::now() + Duration::from_secs(2);
    while rec.len() == 0 {
        assert!(Instant::now() < deadline, "넘긴 것이 없다");
        std::hint::spin_loop();
    }
    let at = s
        .pause(Duration::from_millis(50))
        .unwrap()
        .expect("재생 중이었다");
    settle();
    let sent_before = rec.words().into_iter().filter(|&w| is_note_on(w)).count();
    s.play(at).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let mut seen: HashMap<u32, usize> = HashMap::new();
    for w in rec.words().into_iter().filter(|&w| is_note_on(w)) {
        *seen.entry(w).or_default() += 1;
    }
    let repeated = seen.values().filter(|&&n| n > 1).count();
    assert!(
        sent_before == 0 || sent_before == total || at.get() > 480,
        "틱 480의 {sent_before}/{total}개를 넘긴 뒤 자리 {at:?}"
    );
    assert_eq!(
        repeated, 0,
        "자리 {at:?}, 앞서 넘긴 {sent_before}개, 다시 넘긴 노트 {repeated}개"
    );
}

/// 큐가 가득이면 `pause`는 `QueueFull`이고 정지는 들어가지 않는다(재생이 계속된다). 앱은 이때 주인을 놓고
/// 일시정지 상태가 되므로, 이 소리를 멈출 길이 정지 Action에는 없다.
#[test]
fn pause_with_full_queue_errors_and_playback_continues() {
    let _serial = serial();
    let rec = Recorder::new();
    let mut s =
        Scheduler::start(snapshot(&rec, numbered(64, 60)), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let mut filled = false;
    for _ in 0..10_000 {
        if s.set_metronome_enabled(false).is_err() {
            filled = true;
            break;
        }
    }
    assert!(filled, "큐를 채우지 못했다");
    let result = s.pause(Duration::from_millis(50));
    assert!(matches!(result, Err(MidiError::QueueFull)), "{result:?}");
    let before = sent_on_ticks(&rec, 60).len();
    std::thread::sleep(Duration::from_millis(200));
    let after = sent_on_ticks(&rec, 60).len();
    assert!(after > before, "재생이 멈췄다({before} → {after})");
    s.stop().unwrap();
}
