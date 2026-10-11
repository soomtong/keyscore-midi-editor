//! 가상 소스를 만들고 mach 기준 미래 타임스탬프로 Note On을 보낸다.
//! 사용: sender <legacy|proto> <rounds> <out.csv>
#![allow(unsafe_code)]

use std::io::Write;
use std::ptr;

use sp3_coremidi::{describe, now_ticks, ns_to_ticks, pump, ticks_to_ns};

/// 클래스별 예약 오프셋(ms). u64::MAX는 타임스탬프 0(즉시)을 뜻한다.
const OFFSETS_MS: [u64; 5] = [0, 5, 20, 100, u64::MAX];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map_or("legacy", String::as_str).to_string();
    let rounds: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let out = args.get(3).cloned().unwrap_or_else(|| "sender.csv".into());

    let name = "SP3 Virtual Source";
    let sender: Box<dyn Fn(u8, u8, u64)> = if mode == "proto" {
        Box::new(make_proto(name))
    } else {
        Box::new(make_legacy(name))
    };

    // 수신 프로세스가 연결할 시간을 준다.
    println!("source ready mode={mode}; waiting 3s");
    pump(3.0);

    let mut csv = std::fs::File::create(&out).unwrap();
    writeln!(csv, "chan,note,send_ns,ts_ns").unwrap();
    for r in 0..rounds {
        let note = (r % 128) as u8;
        for (chan, off) in OFFSETS_MS.iter().enumerate() {
            let now = now_ticks();
            let ts = if *off == u64::MAX { 0 } else { now + ns_to_ticks(off * 1_000_000) };
            sender(chan as u8, note, ts);
            writeln!(csv, "{chan},{note},{},{}", ticks_to_ns(now), ticks_to_ns(ts)).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    pump(1.0);
    println!("done");
}

/// coremidi 크레이트 경로: MIDISourceCreate + MIDIReceived.
fn make_legacy(name: &str) -> impl Fn(u8, u8, u64) {
    let client = coremidi::Client::new_with_notifications("sp3-sender", |n: &coremidi::Notification| {
        println!("[notif] {}", describe(n));
    })
    .unwrap();
    let source = client.virtual_source(name).unwrap();
    move |chan, note, ts| {
        let _keep = &client;
        let pb = coremidi::PacketBuffer::new(ts, &[0x90 | chan, note, 100]);
        source.received(&pb).unwrap();
    }
}

/// core-midi-sys 직접 호출 경로: MIDISourceCreateWithProtocol(1.0) + MIDIReceivedEventList.
/// coremidi 0.9.2의 Client는 원시 핸들을 노출하지 않으므로 클라이언트도 직접 만든다.
fn make_proto(name: &str) -> impl Fn(u8, u8, u64) {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use coremidi_sys::*;

    let cname = CFString::new("sp3-sender-proto");
    let sname = CFString::new(name);
    let mut client: MIDIClientRef = 0;
    let mut ep: MIDIEndpointRef = 0;
    // 출력 포인터는 이 스택 변수를 가리키고 CFString은 호출 동안 살아 있다.
    unsafe {
        let s = MIDIClientCreate(cname.as_concrete_TypeRef(), None, ptr::null_mut(), &mut client);
        assert_eq!(s, 0, "MIDIClientCreate {s}");
        let s = MIDISourceCreateWithProtocol(
            client,
            sname.as_concrete_TypeRef(),
            kMIDIProtocol_1_0 as MIDIProtocolID,
            &mut ep,
        );
        assert_eq!(s, 0, "MIDISourceCreateWithProtocol {s}");
    }
    move |chan, note, ts| {
        // MIDIEventList는 가변 길이라 충분한 정렬된 버퍼를 직접 잡는다.
        let mut buf = [0u32; 64];
        let word = 0x2000_0000u32 | (u32::from(0x90 | chan) << 16) | (u32::from(note) << 8) | 100;
        // buf는 MIDIEventList 헤더(+패킷)에 충분하고 u32 정렬이다. 포인터는 호출 동안만 쓴다.
        unsafe {
            let list = buf.as_mut_ptr() as *mut MIDIEventList;
            let cur = MIDIEventListInit(list, kMIDIProtocol_1_0 as MIDIProtocolID);
            let cur = MIDIEventListAdd(list, std::mem::size_of_val(&buf) as u64, cur, ts, 1, &word);
            assert!(!cur.is_null());
            let s = MIDIReceivedEventList(ep, list);
            assert_eq!(s, 0, "MIDIReceivedEventList {s}");
        }
    }
}
