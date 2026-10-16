//! 메트로놈·박 보고·미리듣기 경로의 할당 0 검사(REQ 9.2, S1-T5 테스트 11).
//!
//! `assert_no_alloc` 전역 할당자 아래에서 스케줄러 루프가 할당하면 프로세스가 중단된다. 다른 테스트와
//! 섞이지 않게 별도 바이너리에 둔다.

use std::sync::Arc;
use std::time::Duration;

use assert_no_alloc::AllocDisabler;
use keyscore_core::{Channel, MeterMap, Pitch, Tempo, TempoMap, Tick, TimeSignature, Velocity};
use keyscore_midi::{MetronomeConfig, MidiClient, MidiPort, Scheduler, SchedulerConfig, Snapshot};

#[global_allocator]
static ALLOC: AllocDisabler = AllocDisabler;

#[test]
fn metronome_and_preview_do_not_allocate() {
    let client = MidiClient::new("ks-metro-alloc").unwrap();
    let ports: Vec<Arc<dyn MidiPort>> = (0..2)
        .map(|i| {
            let name = format!("KS metro alloc {} {i}", std::process::id());
            Arc::new(client.create_virtual_output(&name).unwrap()) as Arc<dyn MidiPort>
        })
        .collect();
    let mut meter = MeterMap::new(TimeSignature::new(3, 4).unwrap());
    meter.set(Tick::new(2400), TimeSignature::new(7, 8).unwrap());
    // 아주 빠른 템포로 박·클릭·끄기 대기를 많이 지나게 한다.
    let tempo = TempoMap::new(Tempo::from_bpm(600).unwrap());
    let snap = Arc::new(
        Snapshot::new(tempo, ports, Vec::new())
            .unwrap()
            .with_meter(meter),
    );
    let mut s = Scheduler::start(snap, SchedulerConfig::default()).unwrap();
    let ch = Channel::new(10).unwrap();
    let config = MetronomeConfig::new(1, ch, Pitch::new(37).unwrap(), Pitch::new(42).unwrap())
        .with_length(Duration::from_millis(300));
    s.set_metronome(config).unwrap();
    s.set_metronome_forced(true).unwrap();
    s.set_loop(Some((Tick::new(480), Tick::new(4800)))).unwrap();
    s.play(Tick::ZERO).unwrap();
    // 끄기 대기 표보다 많은 미리듣기와 되풀이 노트.
    for k in 0..150u8 {
        let p = Pitch::new(20 + k % 90).unwrap();
        s.preview(
            0,
            Channel::new(1 + k % 16).unwrap(),
            p,
            Velocity::new(0x8000),
            Duration::from_millis(100),
        )
        .unwrap();
        if k % 50 == 0 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    // 박 큐를 비우지 않아 가득 찬 경로도 지난다.
    std::thread::sleep(Duration::from_millis(1500));
    let mut beats = 0;
    while s.poll_beat().is_some() {
        beats += 1;
    }
    assert!(beats > 0);
    s.locate(Tick::new(2000)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    s.panic().unwrap();
    s.stop().unwrap();
    std::thread::sleep(Duration::from_millis(400));
}
