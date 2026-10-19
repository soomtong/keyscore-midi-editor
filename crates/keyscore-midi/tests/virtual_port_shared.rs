//! S2-T6d: 스냅샷이 `Arc`로 쥔 가상 포트의 이름을 재생을 멈추지 않고 바꾼다(OUT-2).

use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use keyscore_midi::{HostTime, MidiClient, MidiError, MidiPort, source_names};

fn unique(name: &str) -> String {
    format!("KS shared {} {name}", std::process::id())
}

fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cond()
}

fn has_source(name: &str) -> bool {
    source_names().iter().any(|n| n == name)
}

fn note_on_ump(note: u8) -> u32 {
    0x2000_0000 | (0x90 << 16) | (u32::from(note) << 8) | 0x64
}

#[test]
fn virtual_output_renames_while_shared() {
    let client = MidiClient::new("ks-test-shared").unwrap();
    let before = unique("a");
    let after = unique("b");
    let port = Arc::new(client.create_virtual_output(&before).unwrap());
    let held: Arc<dyn MidiPort> = port.clone();
    assert!(wait_until(Duration::from_secs(2), || has_source(&before)));

    // 수신기를 옛 이름의 소스에 붙인다
    let (tx, rx) = mpsc::channel();
    let rx_client = coremidi::Client::new("ks-test-shared-rx").unwrap();
    let input = rx_client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
            }
        })
        .unwrap();
    let mut source = None;
    assert!(wait_until(Duration::from_secs(2), || {
        source = coremidi::Source::from_name(&before);
        source.is_some()
    }));
    input.connect_source(&source.unwrap()).unwrap();

    // 다른 핸들이 쥔 채로 이름을 바꾼다
    port.set_name(&after).unwrap();
    assert_eq!(held.name(), after);
    assert!(wait_until(Duration::from_secs(2), || has_source(&after)
        && !has_source(&before)));
    // 같은 엔드포인트라 붙어 있던 수신기가 계속 받는다
    held.send(HostTime::now(), &[note_on_ump(64)]).unwrap();
    let got = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("이름을 바꾼 뒤 받지 못했다");
    assert_eq!(got, vec![0x90, 64, 100]);

    // 중복·잘못된 이름은 거부하고 이름을 바꾸지 않는다
    let other_name = unique("other");
    let _other = client.create_virtual_output(&other_name).unwrap();
    assert_eq!(
        port.set_name(&other_name),
        Err(MidiError::DuplicateName(other_name.clone()))
    );
    assert_eq!(port.set_name(" "), Err(MidiError::InvalidName));
    assert_eq!(held.name(), after);
    // 옛 이름은 비었다
    let again = client.create_virtual_output(&before).unwrap();
    assert_eq!(again.name(), before);
    drop(held);
}
