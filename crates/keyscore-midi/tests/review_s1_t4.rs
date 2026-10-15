//! S1-T4 적대적 리뷰 1회차 테스트.
//!
//! 스케줄러가 `MidiPort::send`에 넘긴 (타임스탬프, 메시지)를 그대로 적는 기록 포트를 쓴다. `CoreMIDI`를 거치지
//! 않으므로 타임스탬프를 정확히 비교할 수 있다. 기록 포트는 락과 할당을 쓰지만, 이 바이너리는
//! `assert_no_alloc` 전역 할당자를 두지 않으므로 검사에 걸리지 않는다(할당 검사는
//! `review_s1_t4_alloc.rs`).
//!
//! 여러 테스트가 lookahead를 300 ms로 넓힌다. 기본 10 ms에서도 같은 일이 그 창 안에서 일어나지만,
//! 시점을 결정적으로 만들려고 창을 넓혔다.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use keyscore_core::{Channel, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    HostTime, Midi1Message, MidiError, MidiPort, ScheduledEvent, Scheduler, SchedulerConfig,
    Snapshot, control_change, note_off, note_on, pitch_bend,
};

/// (스케줄러가 넘긴 호스트 타임스탬프, MIDI 1.0 바이트).
type Log = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

struct Rec {
    log: Log,
    /// drop된 스레드 이름을 적는다(포트 해제 스레드 검사).
    dropped_on: Arc<Mutex<Option<String>>>,
}

impl MidiPort for Rec {
    fn name(&self) -> &'static str {
        "rec"
    }

    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
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
        let bytes = [status, byte(8), byte(0)][..len].to_vec();
        self.log.lock().unwrap().push((at.ticks(), bytes));
        Ok(())
    }
}

impl Drop for Rec {
    fn drop(&mut self) {
        let name = std::thread::current().name().unwrap_or("").to_owned();
        *self.dropped_on.lock().unwrap() = Some(name);
    }
}

fn rec() -> (Arc<dyn MidiPort>, Log) {
    let log: Log = Arc::default();
    let port: Arc<dyn MidiPort> = Arc::new(Rec {
        log: Arc::clone(&log),
        dropped_on: Arc::default(),
    });
    (port, log)
}

fn ch1() -> Channel {
    Channel::new(1).unwrap()
}

fn on(p: u8) -> Midi1Message {
    note_on(
        ch1(),
        Pitch::new(p).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
}

fn off(p: u8) -> Midi1Message {
    note_off(ch1(), Pitch::new(p).unwrap(), Velocity::new(0))
}

fn cc(n: u8, v7: u32) -> Midi1Message {
    control_change(ch1(), n, v7 << 25).unwrap()
}

fn ev(tick: u64, msg: Midi1Message) -> ScheduledEvent {
    ScheduledEvent::new(Tick::new(tick), 0, msg)
}

fn bpm(n: u32) -> TempoMap {
    TempoMap::new(Tempo::from_bpm(n).unwrap())
}

fn snap(port: &Arc<dyn MidiPort>, tempo: TempoMap, events: Vec<ScheduledEvent>) -> Arc<Snapshot> {
    Arc::new(Snapshot::new(tempo, vec![Arc::clone(port)], events).unwrap())
}

/// 이미 예약된 미래 이벤트가 있게 lookahead를 넓힌 설정.
fn wide() -> SchedulerConfig {
    SchedulerConfig {
        lookahead: Duration::from_millis(300),
        wake: Duration::from_millis(1),
    }
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// 두 호스트 시각 사이 ns(정확한 timebase 비율).
fn ns_between(earlier: u64, later: u64) -> u128 {
    HostTime::from_ticks(later)
        .duration_since(HostTime::from_ticks(earlier))
        .as_nanos()
}

/// 기록을 타임스탬프 순으로(같은 시각은 보낸 순서) 정렬한 사본. `CoreMIDI`가 수신기에 건네는 순서다.
fn by_time(log: &Log) -> Vec<(u64, Vec<u8>)> {
    let mut v = log.lock().unwrap().clone();
    v.sort_by_key(|(t, _)| *t);
    v
}

/// 받는 신디 입장에서 마지막에 울리고 있는 (채널, 음).
fn sounding_at_end(log: &Log) -> BTreeSet<(u8, u8)> {
    let mut sounding = BTreeSet::new();
    for (_, b) in by_time(log) {
        let ch = b[0] & 0x0F;
        match b[0] & 0xF0 {
            0x90 if b[2] > 0 => {
                sounding.insert((ch, b[1]));
            }
            0x80 | 0x90 => {
                sounding.remove(&(ch, b[1]));
            }
            0xB0 if b[1] == 120 || b[1] == 123 => sounding.retain(|(c, _)| *c != ch),
            _ => {}
        }
    }
    sounding
}

fn note_ons(log: &Log, pitch: u8) -> Vec<u64> {
    by_time(log)
        .into_iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0x90 && b[1] == pitch && b[2] > 0)
        .map(|(t, _)| t)
        .collect()
}

// ---- 깨뜨린 사례(현재 구현에서 실패한다) ----

/// 지적 1: drop은 `now`에 Note Off를 예약한다. 이미 미래로 예약한 Note On이 그 뒤에 도착해 걸린다.
#[test]
fn drop_with_note_on_already_scheduled_leaves_no_stuck_note() {
    let (port, log) = rec();
    let events = vec![ev(480, on(60)), ev(1920, off(60))];
    let mut s = Scheduler::start(snap(&port, bpm(120), events), wide()).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(60);
    // 틱 480(250 ms)의 Note On은 lookahead 300 ms 안이라 이미 넘겼다.
    assert_eq!(note_ons(&log, 60).len(), 1);
    drop(s);
    let left = sounding_at_end(&log);
    assert!(left.is_empty(), "걸린 노트 {left:?}: {:?}", by_time(&log));
}

/// 지적 2: 패닉은 `now`에 보내고 울리는 노트 표를 비운다. 그 뒤 도착할 Note On은 정지해도 꺼지지 않는다.
#[test]
fn panic_then_stop_with_note_on_already_scheduled_leaves_no_stuck_note() {
    let (port, log) = rec();
    let events = vec![ev(480, on(60)), ev(1920, off(60))];
    let mut s = Scheduler::start(snap(&port, bpm(120), events), wide()).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(60);
    s.panic().unwrap();
    s.stop().unwrap();
    sleep_ms(100);
    let left = sounding_at_end(&log);
    assert!(left.is_empty(), "걸린 노트 {left:?}: {:?}", by_time(&log));
}

/// 지적 3: 내용이 같은 스냅샷으로 바꿔도, 이미 예약한 Note On은 한 번만 가고 노트 길이가 남아야 한다.
/// 지금은 `last_sent`에 Note Off를 붙여 길이 0이 되거나, `tick_at` 내림 때문에 같은 Note On을 다시 보낸다.
#[test]
fn swap_with_identical_snapshot_sends_scheduled_note_on_once_and_keeps_length() {
    let (port, log) = rec();
    let events = || vec![ev(480, on(60)), ev(1440, off(60))];
    let mut s = Scheduler::start(snap(&port, bpm(120), events()), wide()).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(60);
    s.swap(snap(&port, bpm(120), events())).unwrap();
    sleep_ms(900);
    s.stop().unwrap();
    sleep_ms(20);
    let ons = note_ons(&log, 60);
    assert_eq!(ons.len(), 1, "Note On 중복: {:?}", by_time(&log));
    let off_at = by_time(&log)
        .into_iter()
        .find(|(t, b)| *t >= ons[0] && b[0] & 0xF0 == 0x80 && b[1] == 60)
        .map(|(t, _)| t)
        .unwrap();
    // 노트 길이는 960틱(500 ms). 400 ms보다 짧으면 교체가 노트를 잘랐다.
    assert!(
        ns_between(ons[0], off_at) >= 400_000_000,
        "길이 {} ns: {:?}",
        ns_between(ons[0], off_at),
        by_time(&log)
    );
}

/// 지적 4: 교체마다 기준점을 (내림한 틱, 지금)으로 다시 잡아 박자 격자가 최대 한 틱씩 뒤로 밀린다.
/// 재생 중 편집(TRN-3)이 잦으면 누적돼 NF-4(드리프트 ≤ 1 ms)를 넘는다.
#[test]
fn repeated_swaps_keep_beat_grid_within_1ms() {
    let (port, log) = rec();
    let events = || {
        (0..40u64)
            .flat_map(|k| [ev(k * 960, on(60)), ev(k * 960 + 60, off(60))])
            .collect::<Vec<_>>()
    };
    let mut s =
        Scheduler::start(snap(&port, bpm(120), events()), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(30);
    let h0 = note_ons(&log, 60)[0];
    for _ in 0..200 {
        s.swap(snap(&port, bpm(120), events())).unwrap();
        sleep_ms(3);
        s.collect_garbage();
    }
    let swaps_done = HostTime::now().ticks();
    sleep_ms(1200);
    s.stop().unwrap();
    let beat: u128 = 500_000_000;
    let later: Vec<u128> = note_ons(&log, 60)
        .into_iter()
        .filter(|t| *t > swaps_done)
        .map(|t| {
            let r = ns_between(h0, t) % beat;
            r.min(beat - r)
        })
        .collect();
    assert!(!later.is_empty());
    let worst = later.iter().copied().max().unwrap();
    assert!(worst <= 1_000_000, "박 격자에서 {worst} ns 벗어남");
}

/// 지적 5: 교체 때 `next_clock`을 `tick_at(last_sent)`의 올림으로 다시 잡아, 이미 보낸 Clock을 또 보낸다.
/// 받는 쪽 템포·위치가 교체마다 한 Clock씩 앞선다.
#[test]
fn swaps_during_play_do_not_duplicate_midi_clock() {
    let (port, log) = rec();
    let mut s =
        Scheduler::start(snap(&port, bpm(120), vec![]), SchedulerConfig::default()).unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(20);
    for _ in 0..100 {
        s.swap(snap(&port, bpm(120), vec![])).unwrap();
        sleep_ms(3);
        s.collect_garbage();
    }
    s.stop().unwrap();
    sleep_ms(20);
    let clocks: Vec<u64> = by_time(&log)
        .into_iter()
        .filter(|(_, b)| b[0] == 0xF8)
        .map(|(t, _)| t)
        .collect();
    // 120 BPM의 Clock 간격은 20.83 ms다.
    let min_gap = clocks
        .windows(2)
        .map(|w| ns_between(w[0], w[1]))
        .min()
        .unwrap();
    assert!(min_gap >= 10_000_000, "Clock 간격 최소 {min_gap} ns(중복)");
}

/// 지적 6: 16분음표에 맞지 않는 위치에서 재생하면 SPP는 내림한 위치를 알리는데 첫 Clock은 올림한
/// 위치(40틱 격자)에 온다. 받는 쪽은 첫 Clock을 SPP 위치로 해석하므로 위치가 어긋난다.
#[test]
fn play_from_unaligned_tick_sends_spp_matching_first_clock() {
    let (port, log) = rec();
    let mut s =
        Scheduler::start(snap(&port, bpm(120), vec![]), SchedulerConfig::default()).unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::new(100)).unwrap();
    sleep_ms(80);
    s.stop().unwrap();
    sleep_ms(20);
    let log = by_time(&log);
    let spp = log.iter().find(|(_, b)| b[0] == 0xF2).unwrap();
    let spp_tick = (u64::from(spp.1[1]) | (u64::from(spp.1[2]) << 7)) * 240;
    let cont = log.iter().find(|(_, b)| b[0] == 0xFB).unwrap().0;
    let first_clock = log.iter().find(|(_, b)| b[0] == 0xF8).unwrap().0;
    // Continue는 틱 100의 시각이다. 첫 Clock의 틱 = 100 + 경과(틱, 반올림).
    let ns = ns_between(cont, first_clock);
    let elapsed = u64::try_from((ns * 960 + 250_000_000) / 500_000_000).unwrap();
    assert_eq!(
        spp_tick,
        100 + elapsed,
        "SPP {spp_tick}, 첫 Clock 틱 {}",
        100 + elapsed
    );
}

/// 지적 7: 재생 중 위치 이동은 Stop 없이 SPP·Continue를 보낸다. MIDI 1.0에서 SPP는 정지 상태에서 받는
/// 메시지라 재생 중인 장치는 대개 무시한다.
#[test]
fn locate_while_playing_with_clock_sends_stop_before_spp() {
    let (port, log) = rec();
    let mut s =
        Scheduler::start(snap(&port, bpm(120), vec![]), SchedulerConfig::default()).unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(50);
    s.locate(Tick::new(1920)).unwrap();
    sleep_ms(30);
    s.stop().unwrap();
    sleep_ms(20);
    let transport: Vec<u8> = by_time(&log)
        .into_iter()
        .map(|(_, b)| b[0])
        .filter(|b| matches!(b, 0xFA | 0xFB | 0xFC | 0xF2))
        .collect();
    assert_eq!(transport, vec![0xFA, 0xFC, 0xF2, 0xFB, 0xFC]);
}

/// 지적 8: 공개 API로 받은 큰 틱에서 `div_ceil(40) * 40`이 넘친다. 디버그 빌드에서 스케줄러 스레드가
/// 패닉으로 죽고, 그 뒤 명령은 큐에 쌓이기만 한다(호출자는 알 수 없다).
#[test]
fn play_at_max_tick_keeps_scheduler_thread_alive() {
    let (port, log) = rec();
    let mut s =
        Scheduler::start(snap(&port, bpm(120), vec![]), SchedulerConfig::default()).unwrap();
    s.play(Tick::new(u64::MAX)).unwrap();
    sleep_ms(30);
    s.send_now(0, on(61)).unwrap();
    sleep_ms(200);
    assert_eq!(note_ons(&log, 61).len(), 1, "스케줄러 스레드가 멈췄다");
}

/// 지적 9: 돌려받기 큐(256)와 대기 자리(4)가 차면 교체 명령이 큐 머리에서 막히고, 뒤의 패닉·정지도
/// 처리되지 않는다(OUT-6). `collect_garbage`를 불러야 한다는 계약이 문서에 없다.
#[test]
fn panic_after_many_uncollected_swaps_still_reaches_ports() {
    let (port, log) = rec();
    let mut s =
        Scheduler::start(snap(&port, bpm(120), vec![]), SchedulerConfig::default()).unwrap();
    for _ in 0..300 {
        let mut tries = 0;
        while s.swap(snap(&port, bpm(120), vec![])).is_err() {
            tries += 1;
            assert!(tries < 200, "교체 큐가 비지 않는다");
            sleep_ms(1);
        }
    }
    sleep_ms(20);
    assert_eq!(s.panic(), Ok(()));
    sleep_ms(200);
    let panics = by_time(&log)
        .into_iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xB0 && b[1] == 120)
        .count();
    assert_eq!(panics, 16, "패닉이 포트에 닿지 않았다");
}

/// 지적 10: chase가 CC를 번호 순으로 보내 RPN 선택(101·100)보다 데이터 엔트리(6)가 먼저 간다.
/// 피치 벤드 범위 같은 RPN 값이 이전에 선택된 다른 파라미터에 들어간다.
#[test]
fn chase_sends_rpn_select_before_data_entry() {
    let (port, log) = rec();
    let events = vec![
        ev(0, cc(101, 0)),
        ev(0, cc(100, 0)),
        ev(0, cc(6, 12)),
        ev(0, cc(38, 0)),
        ev(1920, on(60)),
    ];
    let mut s =
        Scheduler::start(snap(&port, bpm(120), events), SchedulerConfig::default()).unwrap();
    s.play(Tick::new(960)).unwrap();
    sleep_ms(30);
    s.stop().unwrap();
    let order: Vec<u8> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xB0)
        .map(|(_, b)| b[1])
        .collect();
    let pos = |n: u8| order.iter().position(|c| *c == n).unwrap();
    assert!(
        pos(101) < pos(6) && pos(100) < pos(6),
        "chase CC 순서 {order:?}"
    );
}

/// 지적 11: chase가 피치 벤드를 보내지 않는다. 벤드 레인(CTL-4)도 CTL-6의 「현재 위치 값」 대상이다.
#[test]
fn play_from_middle_chases_pitch_bend() {
    let (port, log) = rec();
    let events = vec![ev(0, pitch_bend(ch1(), 0xC000_0000)), ev(1920, on(60))];
    let mut s =
        Scheduler::start(snap(&port, bpm(120), events), SchedulerConfig::default()).unwrap();
    s.play(Tick::new(960)).unwrap();
    sleep_ms(30);
    s.stop().unwrap();
    let bends = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, b)| b[0] & 0xF0 == 0xE0)
        .count();
    assert_eq!(bends, 1, "벤드 chase 없음: {:?}", by_time(&log));
}

/// 지적 12: 스케줄러가 스냅샷의 마지막 소유자면 종료 때 스케줄러 스레드에서 포트가 해제된다.
/// `MidiPort` 문서는 「스케줄러 스레드가 포트의 마지막 소유자가 되면 안 된다」고 한다.
#[test]
fn drop_does_not_release_ports_on_scheduler_thread() {
    let log: Log = Arc::default();
    let dropped_on: Arc<Mutex<Option<String>>> = Arc::default();
    let port: Arc<dyn MidiPort> = Arc::new(Rec {
        log,
        dropped_on: Arc::clone(&dropped_on),
    });
    let snapshot = snap(&port, bpm(120), vec![]);
    drop(port);
    let s = Scheduler::start(snapshot, SchedulerConfig::default()).unwrap();
    sleep_ms(10);
    drop(s);
    let thread = dropped_on.lock().unwrap().clone().unwrap();
    assert_ne!(thread, "keyscore-scheduler");
}

// ---- 못 깨뜨린 경계(통과한다) ----

#[test]
fn stop_with_note_on_already_scheduled_releases_after_it() {
    let (port, log) = rec();
    let events = vec![ev(480, on(60)), ev(1920, off(60))];
    let mut s = Scheduler::start(snap(&port, bpm(120), events), wide()).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(60);
    s.stop().unwrap();
    sleep_ms(50);
    assert!(sounding_at_end(&log).is_empty(), "{:?}", by_time(&log));
}

#[test]
fn stop_with_same_pitch_overlap_releases_note() {
    let (port, log) = rec();
    let events = vec![ev(0, on(60)), ev(10, on(60)), ev(20, off(60))];
    let mut s = Scheduler::start(snap(&port, bpm(120), events), wide()).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(40);
    s.stop().unwrap();
    sleep_ms(30);
    assert!(sounding_at_end(&log).is_empty(), "{:?}", by_time(&log));
}

#[test]
fn swap_to_slower_tempo_keeps_new_grid_exact() {
    let (port, log) = rec();
    let events = || (0..20u64).map(|k| ev(k * 960, on(60))).collect::<Vec<_>>();
    let mut s =
        Scheduler::start(snap(&port, bpm(120), events()), SchedulerConfig::default()).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(100);
    s.swap(snap(&port, bpm(60), events())).unwrap();
    let swapped = HostTime::now().ticks();
    sleep_ms(2300);
    s.stop().unwrap();
    let after: Vec<u64> = note_ons(&log, 60)
        .into_iter()
        .filter(|t| *t > swapped)
        .collect();
    assert!(after.len() >= 2, "{after:?}");
    for w in after.windows(2) {
        let gap = ns_between(w[0], w[1]);
        // 60 BPM 한 박 = 1 s. 호스트 틱 내림 오차(수십 ns)만 허용한다.
        assert!(gap.abs_diff(1_000_000_000) < 1_000, "{gap}");
    }
}

#[test]
fn loop_wrap_keeps_grid_exact_over_many_iterations() {
    let (port, log) = rec();
    let events = vec![ev(0, on(60)), ev(240, off(60))];
    let mut s =
        Scheduler::start(snap(&port, bpm(120), events), SchedulerConfig::default()).unwrap();
    s.set_loop(Some((Tick::ZERO, Tick::new(480)))).unwrap();
    s.play(Tick::ZERO).unwrap();
    sleep_ms(2600);
    s.stop().unwrap();
    sleep_ms(30);
    let ons = note_ons(&log, 60);
    assert!(ons.len() >= 10, "{}", ons.len());
    let first = ons[0];
    for (k, t) in ons.iter().enumerate() {
        let k = u128::try_from(k).unwrap();
        let ns = ns_between(first, *t);
        assert!(ns.abs_diff(k * 250_000_000) < 1_000, "{k}: {ns}");
    }
    assert!(sounding_at_end(&log).is_empty());
}
