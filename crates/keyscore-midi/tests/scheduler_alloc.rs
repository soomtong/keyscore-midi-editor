//! 스케줄러 루프 할당 0 검사(REQ 9.2, S1-T4 테스트 12).
//!
//! `assert_no_alloc`의 전역 할당자를 쓴다. 디버그 빌드에서 스케줄러 루프는 한 바퀴를
//! `assert_no_alloc`으로 감싸므로, 루프 안에서 할당하면 프로세스가 중단돼 이 테스트가 실패한다.
//! 다른 테스트와 섞이지 않게 별도 바이너리에 둔다.

use std::sync::Arc;
use std::time::Duration;

use assert_no_alloc::AllocDisabler;
use keyscore_core::{Bank, Channel, Pitch, Program, Tempo, TempoMap, Tick, Velocity};
use keyscore_midi::{
    MidiClient, MidiPort, ScheduledEvent, Scheduler, SchedulerConfig, Snapshot, bank_select,
    control_change, note_off, note_on, program_change,
};

#[global_allocator]
static ALLOC: AllocDisabler = AllocDisabler;

fn snapshot(port: &Arc<dyn MidiPort>, shift: u64) -> Arc<Snapshot> {
    let ch = Channel::new(1).unwrap();
    let [msb, lsb] = bank_select(ch, Bank::new(3).unwrap());
    let mut events = vec![
        ScheduledEvent::new(Tick::ZERO, 0, msb),
        ScheduledEvent::new(Tick::ZERO, 0, lsb),
        ScheduledEvent::new(Tick::ZERO, 0, program_change(ch, Program::new(4).unwrap())),
        ScheduledEvent::new(Tick::ZERO, 0, control_change(ch, 1, 0x1234_5678).unwrap()),
    ];
    for i in 0..400 {
        let t = i * 48 + shift;
        let p = Pitch::new(u8::try_from(40 + i % 40).unwrap()).unwrap();
        events.push(ScheduledEvent::new(
            Tick::new(t),
            0,
            note_on(ch, p, Velocity::new(0x9000)),
        ));
        events.push(ScheduledEvent::new(
            Tick::new(t + 30),
            0,
            note_off(ch, p, Velocity::new(0)),
        ));
    }
    let tempo = TempoMap::new(Tempo::from_bpm(240).unwrap());
    Arc::new(Snapshot::new(tempo, vec![Arc::clone(port)], events).unwrap())
}

#[test]
fn scheduler_loop_does_not_allocate() {
    let client = MidiClient::new("ks-sched-alloc").unwrap();
    let name = format!("KS sched alloc {}", std::process::id());
    let port: Arc<dyn MidiPort> = Arc::new(client.create_virtual_output(&name).unwrap());
    let mut s = Scheduler::start(snapshot(&port, 0), SchedulerConfig::default()).unwrap();
    s.set_clock(0, true).unwrap();
    s.play(Tick::ZERO).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    s.swap(snapshot(&port, 7)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    s.locate(Tick::new(4800)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    s.set_loop(Some((Tick::new(960), Tick::new(1920)))).unwrap();
    std::thread::sleep(Duration::from_millis(600));
    s.send_now(
        0,
        note_on(
            Channel::new(2).unwrap(),
            Pitch::new(60).unwrap(),
            Velocity::new(1),
        ),
    )
    .unwrap();
    s.panic().unwrap();
    s.stop().unwrap();
    s.play(Tick::new(960)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    s.set_loop(None).unwrap();
    s.stop().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert!(s.collect_garbage() >= 1);
    // 스케줄러 스레드에서 할당했다면 여기 오기 전에 프로세스가 중단된다.
}
