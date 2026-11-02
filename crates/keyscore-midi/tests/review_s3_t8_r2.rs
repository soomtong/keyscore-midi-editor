//! S3-T8 적대적 리뷰 2회차의 깨뜨리기 시험(`docs/reports/s3-t8/review-2.md`).
//!
//! 1회차 뒤에 더한 `PauseReply`(`At`·`NotPlaying`·`NoAnswer`)와 `QueueFull`을 본다. 「OS에 넘김」은
//! `MidiPort::send` 호출이다. 1회차 시험과 같이 할당 없이 기록하는 가짜 포트로 받는다.

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

const CAP: usize = 1 << 14;
const GAP: u64 = 60;

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
        "review-s3-t8-r2".to_owned()
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

fn snapshot(rec: &Arc<Recorder>, events: Vec<ScheduledEvent>, end: Option<Tick>) -> Arc<Snapshot> {
    let port: Arc<dyn MidiPort> = Arc::new(Port(Arc::clone(rec)));
    Arc::new(
        Snapshot::new(
            TempoMap::new(Tempo::from_bpm(120).unwrap()),
            vec![port],
            events,
        )
        .unwrap()
        .with_end(end),
    )
}

/// 넘긴 Note On의 틱.
fn sent_ticks(rec: &Recorder) -> Vec<u64> {
    rec.words()
        .into_iter()
        .filter(|&w| is_note_on(w))
        .map(|w| u64::from(pitch_of(w) - 20) * GAP)
        .collect()
}

fn settle() {
    std::thread::sleep(Duration::from_millis(20));
}

/// 기다리지 않는 `pause`는 답이 없을 수 있다(`NoAnswer`). 정지 명령은 큐에 남아 곧 처리되어 소리가 멈추고,
/// 뒤의 재생·일시정지는 남은 정지와 섞이지 않고 새 자리를 답한다. 남은 정지 뒤 다시 `pause`하면
/// `NotPlaying`이다(이미 멈췄다).
#[test]
fn no_answer_leaves_stop_queued_and_later_pause_answers_its_own_stop() {
    let _serial = serial();
    let rec = Recorder::new();
    let mut s = Scheduler::start(
        snapshot(&rec, numbered(100, GAP), None),
        SchedulerConfig::default(),
    )
    .unwrap();
    let mut no_answers = 0;
    let mut from = Tick::ZERO;
    for round in 0..20u64 {
        s.play(from).unwrap();
        std::thread::sleep(Duration::from_millis(15 + round));
        let reply = s.pause(Duration::ZERO).unwrap();
        settle();
        let n = sent_ticks(&rec).len();
        settle();
        assert_eq!(
            sent_ticks(&rec).len(),
            n,
            "{round}회차 {reply:?} 뒤에도 재생이 계속된다"
        );
        match reply {
            PauseReply::NoAnswer => {
                no_answers += 1;
                // 남은 정지는 이미 처리됐다. 그 뒤의 정지는 재생 중이 아니었다.
                assert_eq!(
                    s.pause(Duration::from_millis(50)).unwrap(),
                    PauseReply::NotPlaying,
                    "{round}회차"
                );
                // 앱처럼 추정 자리에서 다시 재생하고 기다려 일시정지하면 새 정지의 답이 온다.
                let resume = Tick::new(sent_ticks(&rec).last().copied().unwrap_or(0) + GAP);
                s.play(resume).unwrap();
                std::thread::sleep(Duration::from_millis(30));
                let before = rec.words().len();
                let at = s
                    .pause(Duration::from_millis(50))
                    .unwrap()
                    .tick()
                    .expect("재생 중이었다");
                settle();
                assert!(
                    at >= resume,
                    "{round}회차 자리 {at:?} < 재생 시작 {resume:?}"
                );
                let after: Vec<u64> = rec.words()[before..]
                    .iter()
                    .filter(|&&w| is_note_on(w))
                    .map(|&w| u64::from(pitch_of(w) - 20) * GAP)
                    .collect();
                assert!(
                    after.iter().all(|&t| t < at.get()),
                    "{round}회차 자리 {at:?} 뒤에 {after:?}"
                );
                from = at;
            }
            PauseReply::At(at) => from = at,
            PauseReply::NotPlaying => panic!("{round}회차 재생 중이었다"),
        }
        if from.get() > 70 * GAP {
            from = Tick::ZERO;
        }
    }
    assert!(no_answers > 0, "NoAnswer를 한 번도 만들지 못했다");
}

/// 큐가 가득이라 `pause`가 `QueueFull`이면 정지 명령은 들어가지 않았다. 큐가 빈 뒤 다시 `pause`하면 그 정지의
/// 답(`At`)이 오고, 그 자리 앞의 노트만 넘겼다. 실패한 `pause`가 명령 번호를 어긋내지 않는다.
#[test]
fn pause_after_queue_full_answers_once_queue_drains() {
    let _serial = serial();
    let rec = Recorder::new();
    let mut s = Scheduler::start(
        snapshot(&rec, numbered(100, GAP), None),
        SchedulerConfig::default(),
    )
    .unwrap();
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
    assert!(matches!(
        s.pause(Duration::from_millis(50)),
        Err(MidiError::QueueFull)
    ));
    std::thread::sleep(Duration::from_millis(30));
    let at = s
        .pause(Duration::from_millis(50))
        .unwrap()
        .tick()
        .expect("재생 중이었다");
    settle();
    let ticks = sent_ticks(&rec);
    assert!(!ticks.is_empty());
    assert!(
        ticks.iter().all(|&t| t < at.get()),
        "자리 {at:?} 뒤의 노트를 넘겼다: {ticks:?}"
    );
    // 이어서 재생하면 빠짐·겹침이 없다.
    s.play(at).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    s.stop().unwrap();
    let mut ons: Vec<u8> = rec
        .words()
        .into_iter()
        .filter(|&w| is_note_on(w))
        .map(pitch_of)
        .collect();
    let n = ons.len();
    ons.dedup();
    assert_eq!(ons.len(), n, "겹쳤다");
    assert_eq!(
        ons,
        (20..20 + u8::try_from(n).unwrap()).collect::<Vec<u8>>(),
        "빠졌다"
    );
}

/// 곡 끝 바로 앞(lookahead 안)에서 일시정지를 노린다. 답이 `At(t)`면 `t`는 곡 끝 이하이고 그 앞의 노트만
/// 넘겼다. `NotPlaying`이면 스케줄러는 곡 끝에서 멈췄고 `take_ended`가 남아 있다. 어느 쪽이든 섞이지 않는다.
#[test]
fn pause_racing_song_end_is_either_at_before_end_or_not_playing_with_ended() {
    let _serial = serial();
    let mut at_count = 0;
    let mut not_playing = 0;
    for round in 0..24u64 {
        let rec = Recorder::new();
        // 곡 끝 480(약 250 ms + 시작 여유 2 ms). 노트는 0, 60, …, 420.
        let mut s = Scheduler::start(
            snapshot(&rec, numbered(8, GAP), Some(Tick::new(480))),
            SchedulerConfig::default(),
        )
        .unwrap();
        s.play(Tick::ZERO).unwrap();
        // 곡 끝 24 ms 전부터 0.5 ms씩 늦춘다(곡 끝 정리는 lookahead 10 ms만큼 일찍 한다).
        std::thread::sleep(Duration::from_millis(228 + round / 2));
        match s.pause(Duration::from_millis(50)).unwrap() {
            PauseReply::At(t) => {
                at_count += 1;
                assert!(t <= Tick::new(480), "{round}회차 자리 {t:?}가 곡 끝 뒤다");
                settle();
                let ticks = sent_ticks(&rec);
                assert!(
                    ticks.iter().all(|&x| x < t.get()),
                    "{round}회차 {t:?} {ticks:?}"
                );
                assert!(
                    !s.take_ended(),
                    "{round}회차 일시정지했는데 곡 끝 신호가 있다"
                );
                // 이어서 재생하면 남은 노트를 내고 곡 끝에서 멈춘다.
                s.play(t).unwrap();
                std::thread::sleep(Duration::from_millis(60));
                assert!(
                    s.take_ended(),
                    "{round}회차 이어서 재생이 곡 끝에서 멈추지 않았다"
                );
                assert_eq!(
                    sent_ticks(&rec),
                    (0..8).map(|i| i * GAP).collect::<Vec<_>>()
                );
            }
            PauseReply::NotPlaying => {
                not_playing += 1;
                assert!(
                    s.take_ended(),
                    "{round}회차 NotPlaying인데 곡 끝 신호가 없다"
                );
                assert_eq!(
                    sent_ticks(&rec),
                    (0..8).map(|i| i * GAP).collect::<Vec<_>>()
                );
            }
            PauseReply::NoAnswer => panic!("{round}회차 답이 없다"),
        }
    }
    println!("At {at_count}, NotPlaying {not_playing}");
}

/// 재생 중인데 기준점 틱이 `u64::MAX`면(`play(Tick::new(u64::MAX))`) 이어서 재생할 틱이 「재생 중이 아님」 표식
/// (`NOT_PLAYING = u64::MAX`)과 같아져 `NotPlaying`으로 읽힌다. 앱은 그 답을 곡 끝 정지로 처리한다.
#[test]
fn pause_while_playing_at_max_tick_is_not_reported_as_not_playing() {
    let _serial = serial();
    let rec = Recorder::new();
    let mut s = Scheduler::start(
        snapshot(&rec, numbered(4, GAP), None),
        SchedulerConfig::default(),
    )
    .unwrap();
    s.play(Tick::new(u64::MAX)).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let reply = s.pause(Duration::from_millis(50)).unwrap();
    assert_eq!(reply, PauseReply::At(Tick::new(u64::MAX)), "재생 중이었다");
}
