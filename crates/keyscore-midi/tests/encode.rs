//! S1-T3 인코딩 결과가 가상 포트를 거쳐 같은 바이트로 도착하는지 본다.
//!
//! 수신 쪽은 `coremidi` crate로 받아 대조한다. 포트 이름에 프로세스 id를 넣어 겹치지 않게 한다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::{Channel, Pitch, Velocity};
use keyscore_midi::{HostTime, MidiClient, MidiPort, note_on, panic_messages, source_names};

fn find_source(name: &str) -> coremidi::Source {
    let start = Instant::now();
    loop {
        if source_names().iter().any(|n| n == name)
            && let Some(s) = coremidi::Source::from_name(name)
        {
            return s;
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "소스를 찾지 못했다"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn encoded_note_on_reaches_virtual_port_receiver() {
    let client = MidiClient::new("ks-test-encode").unwrap();
    let name = format!("KS test {} encode", std::process::id());
    let port = client.create_virtual_output(&name).unwrap();

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let receiver = coremidi::Client::new("ks-test-encode-receiver").unwrap();
    let input = receiver
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
            }
        })
        .unwrap();
    input.connect_source(&find_source(&name)).unwrap();

    let ch = Channel::new(3).unwrap();
    let msg = note_on(ch, Pitch::new(64).unwrap(), Velocity::new(0xC924));
    port.send(HostTime::now(), &[msg.ump(0)]).unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        msg.bytes()
    );
    assert_eq!(msg.bytes(), &[0x92, 64, 100]);

    // 시스템 메시지(타입 0x1)와 패닉 묶음도 같은 바이트로 도착한다.
    let panic = panic_messages(ch);
    for m in panic {
        port.send(HostTime::now(), &[m.ump(0)]).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), m.bytes());
    }
    let clock = keyscore_midi::timing_clock();
    port.send(HostTime::now(), &[clock.ump(0)]).unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), vec![0xF8]);
}
