//! S1-T4 적대적 리뷰 1회차: 구현 테스트(`scheduler_alloc.rs`)가 지나지 않는 경로의 할당 검사.
//!
//! `assert_no_alloc` 전역 할당자를 둔다. 디버그 빌드에서 스케줄러 루프가 할당하면 프로세스가 중단된다.
//! 지나는 경로: 돌려받기 큐가 가득 찬 뒤 대기 자리에 스냅샷을 들고 있기(`flush_garbage`의 `Full` 갈래),
//! 템포·포트 수가 다른 스냅샷으로 교체, 여러 포트의 Clock·chase, 루프 되감기, 패닉.

use std::sync::Arc;
use std::time::{Duration, Instant};

use assert_no_alloc::AllocDisabler;
use keyscore_core::{Channel, Pitch, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    MidiClient, MidiPort, ScheduledEvent, Scheduler, SchedulerConfig, Snapshot, control_change,
    note_off, note_on, pitch_bend,
};

#[global_allocator]
static ALLOC: AllocDisabler = AllocDisabler;

fn snapshot(ports: &[Arc<dyn MidiPort>], bpm: u32) -> Arc<Snapshot> {
    let mut events = Vec::new();
    for (i, _) in ports.iter().enumerate() {
        let port = u16::try_from(i).unwrap();
        for c in 1..=16u8 {
            let ch = Channel::new(c).unwrap();
            events.push(ScheduledEvent::new(
                Tick::ZERO,
                port,
                control_change(ch, 7, 0x6000_0000).unwrap(),
            ));
            events.push(ScheduledEvent::new(
                Tick::ZERO,
                port,
                pitch_bend(ch, 0x9000_0000),
            ));
        }
        for k in 0..200u64 {
            let ch = Channel::new(1).unwrap();
            let p = Pitch::new(u8::try_from(36 + k % 48).unwrap()).unwrap();
            events.push(ScheduledEvent::new(
                Tick::new(k * 60),
                port,
                note_on(ch, p, Velocity::new(0x8000)),
            ));
            events.push(ScheduledEvent::new(
                Tick::new(k * 60 + 90),
                port,
                note_off(ch, p, Velocity::new(0)),
            ));
        }
    }
    let tempo = TempoMap::new(Tempo::from_bpm(bpm).unwrap());
    Arc::new(Snapshot::new(tempo, ports.to_vec(), events).unwrap())
}

#[test]
fn scheduler_loop_does_not_allocate_when_garbage_queue_is_full() {
    let client = MidiClient::new("ks-review-alloc").unwrap();
    let ports: Vec<Arc<dyn MidiPort>> = (0..3)
        .map(|i| {
            let name = format!("KS review alloc {} {i}", std::process::id());
            Arc::new(client.create_virtual_output(&name).unwrap()) as Arc<dyn MidiPort>
        })
        .collect();
    let mut s = Scheduler::start(snapshot(&ports[..1], 120), SchedulerConfig::default()).unwrap();
    for p in 0..3 {
        s.set_clock(p, true).unwrap();
    }
    s.set_loop(Some((Tick::new(480), Tick::new(2400)))).unwrap();
    s.play(Tick::new(960)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    // 돌려받지 않고 262번 교체한다: 256개가 큐를 채우고 4개가 대기 자리에 남고, 나머지는 기다린다.
    for i in 0..262u32 {
        let n = 1 + usize::try_from(i % 3).unwrap();
        let bpm = 60 + i % 120;
        while s.swap(snapshot(&ports[..n], bpm)).is_err() {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    std::thread::sleep(Duration::from_millis(100));
    // 이제 돌려받으면 막혔던 교체가 진행된다.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut returned = 0;
    while returned < 262 && Instant::now() < deadline {
        returned += s.collect_garbage();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(returned, 262);
    s.panic().unwrap();
    s.locate(Tick::new(1200)).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    s.stop().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    // 스케줄러 스레드에서 할당했다면 여기 오기 전에 프로세스가 중단된다.
}
