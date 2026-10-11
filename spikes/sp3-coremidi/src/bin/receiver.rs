//! 별도 프로세스의 CoreMIDI 클라이언트. 이름으로 소스를 찾아 연결하고 도착 시각을 기록한다.
//! 사용: receiver <out.csv> <seconds>
use std::io::Write;
use std::sync::{Arc, Mutex};

use coremidi::{Client, Properties, Source};
use sp3_coremidi::{describe, now_ticks, pump, ticks_to_ns};

fn list_sources() {
    println!("sources: {}", coremidi::Sources::count());
    for s in coremidi::Sources.into_iter() {
        println!(
            "  - name={:?} display={:?} uid={:?} protocol={:?}",
            s.name(),
            s.display_name(),
            s.unique_id(),
            s.get_property::<i32>(&Properties::protocol_id())
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "receiver.csv".into());
    let secs: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(60.0);

    let rows: Arc<Mutex<Vec<(u8, u8, u64, u64)>>> = Arc::default();
    let client = Client::new_with_notifications("sp3-receiver", |n: &coremidi::Notification| {
        println!("[{}] [notif] {}", ticks_to_ns(now_ticks()) / 1_000_000, describe(n));
    })
    .unwrap();
    let rows2 = rows.clone();
    let port = client
        .input_port("in", move |pl: &coremidi::PacketList| {
            // 콜백 진입 직후에 도착 시각을 찍는다.
            let arrival = ticks_to_ns(now_ticks());
            let mut g = rows2.lock().unwrap();
            for p in pl.iter() {
                let d = p.data();
                if d.len() >= 3 && d[0] & 0xf0 == 0x90 {
                    g.push((d[0] & 0x0f, d[1], ticks_to_ns(p.timestamp()), arrival));
                }
            }
        })
        .unwrap();

    list_sources();
    let start = std::time::Instant::now();
    let mut connected = false;
    while start.elapsed().as_secs_f64() < secs {
        pump(0.2);
        if !connected {
            if let Some(src) = Source::from_name("SP3 Virtual Source") {
                println!("[{}] connecting", ticks_to_ns(now_ticks()) / 1_000_000);
                port.connect_source(&src).unwrap();
                list_sources();
                connected = true;
            }
        }
        if connected && rows.lock().unwrap().len() >= 500 && start.elapsed().as_secs_f64() > 5.0 {
            // 마지막 수신 이후 2초 조용하면 종료
            let n = rows.lock().unwrap().len();
            pump(2.0);
            if rows.lock().unwrap().len() == n {
                break;
            }
        }
    }
    let g = rows.lock().unwrap();
    let mut f = std::fs::File::create(&out).unwrap();
    writeln!(f, "chan,note,pkt_ts_ns,arrival_ns").unwrap();
    for (c, n, t, a) in g.iter() {
        writeln!(f, "{c},{n},{t},{a}").unwrap();
    }
    println!("received {} note-ons", g.len());
}
