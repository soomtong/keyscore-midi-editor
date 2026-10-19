//! S2-T6d 적대적 리뷰 1회차: `VirtualOutput::set_name`의 잠금 순서와 동시 실행.

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use keyscore_midi::{HostTime, MidiClient, MidiPort};

fn name(tag: &str) -> String {
    format!("KS rv6d midi {} {tag}", std::process::id())
}

/// 한 포트의 이름을 계속 바꾸는 동안 다른 스레드가 포트를 만들고 버리고(이름 목록 잠금 → 포트 이름 잠금), 또
/// 다른 스레드가 이름을 읽고 실시간 송신을 한다. 교착하면 제한 시간 안에 끝나지 않는다. 바꾼 이름과 같은 이름의
/// 새 포트는 만들어지지 않는다(중복 검사가 경쟁에서도 지켜진다).
#[test]
fn set_name_races_with_create_drop_and_send_without_deadlock() {
    let client = MidiClient::new("ks-rv6d-midi").unwrap();
    let port = Arc::new(client.create_virtual_output(&name("a")).unwrap());
    let (a, b) = (name("a"), name("b"));
    let (done_tx, done_rx) = mpsc::channel();

    let renamer = {
        let port = port.clone();
        let (a, b) = (a.clone(), b.clone());
        let done = done_tx.clone();
        thread::spawn(move || {
            for i in 0..400 {
                let _ = port.set_name(if i % 2 == 0 { &b } else { &a });
            }
            let _ = done.send("rename");
        })
    };
    let creator = {
        let client = client.clone();
        let (a, b) = (a.clone(), b.clone());
        let done = done_tx.clone();
        thread::spawn(move || {
            let mut dup = 0;
            for i in 0..400 {
                // 지금 바꾸는 포트와 같은 이름을 노린다. 만들어지면 곧 버린다.
                let target = if i % 2 == 0 { &a } else { &b };
                if let Ok(p) = client.create_virtual_output(target) {
                    dup += 1;
                    drop(p);
                }
                let other = client.create_virtual_output(&format!("{a} x")).unwrap();
                drop(other);
            }
            let _ = done.send(if dup == 0 { "create" } else { "create-dup" });
        })
    };
    let sender = {
        let port = port.clone();
        let done = done_tx;
        thread::spawn(move || {
            let msg = [0x2000_0000_u32 | (0x90 << 16) | (60 << 8) | 0x01];
            for _ in 0..400 {
                let _ = port.name();
                let _ = port.send(HostTime::now(), &msg);
            }
            let _ = done.send("send");
        })
    };
    let mut finished = Vec::new();
    for _ in 0..3 {
        let tag = done_rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or_else(|e| panic!("교착({e}): 끝난 스레드 {finished:?}"));
        finished.push(tag);
    }
    renamer.join().unwrap();
    creator.join().unwrap();
    sender.join().unwrap();
    // 경쟁 중 다른 포트가 그 이름을 잡았을 수는 있다(옛 이름이 풀린 뒤). 그러나 이 포트의 이름과 같은 포트가
    // 동시에 둘 있지는 않다: 지금 이름으로 새로 만들면 거부된다.
    let now = port.name();
    assert!(client.create_virtual_output(&now).is_err());
}
