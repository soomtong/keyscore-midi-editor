//! owner의 소스에 연결/해제를 반복한다. 사용: peer <cycles>
//! 세 방식을 차례로 쓴다: (1) 레거시 입력 포트 (2) MIDIInputPortCreateWithProtocol (3) 두 번째 클라이언트
#![allow(unsafe_code, deprecated)]

use std::ptr;
use std::time::{Duration, Instant};

use core_foundation::base::TCFType;
use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};
use core_foundation::string::CFString;
use core_foundation_sys::string::CFStringRef;
use coremidi_sys::*;

fn wait(ms: u64) {
    CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, Duration::from_millis(ms), false);
}

fn find_source(name: &str) -> Option<MIDIEndpointRef> {
    unsafe {
        for i in 0..MIDIGetNumberOfSources() {
            let e = MIDIGetSource(i);
            let mut s: CFStringRef = ptr::null();
            if MIDIObjectGetStringProperty(e, kMIDIPropertyName, &mut s) == 0 {
                let n = CFString::wrap_under_create_rule(s).to_string();
                if n == name {
                    return Some(e);
                }
            }
        }
    }
    None
}

fn main() {
    let t0 = Instant::now();
    let log = |m: &str| println!("[peer {:>6} ms] {m}", t0.elapsed().as_millis());
    let src = loop {
        if let Some(s) = find_source("SP4 Virtual Source") { break s; }
        wait(100);
    };
    let pname = CFString::new("in");
    unsafe extern "C" fn rd(_: *const MIDIPacketList, _: *mut std::ffi::c_void, _: *mut std::ffi::c_void) {}
    // 같은 프로세스의 클라이언트 둘로 연결/해제를 반복한다(수신 앱이 하나일 때와 둘일 때를 구분).
    let mut clients: Vec<MIDIClientRef> = Vec::new();
    let mut ports: Vec<MIDIPortRef> = Vec::new();
    for i in 0..2 {
        let cname = CFString::new(&format!("sp4-peer-{i}"));
        let (mut client, mut port): (MIDIClientRef, MIDIPortRef) = (0, 0);
        // 출력 포인터는 스택 변수이고 CFString은 호출 동안 산다. rd는 아무것도 하지 않는 콜백이다.
        unsafe {
            assert_eq!(MIDIClientCreate(cname.as_concrete_TypeRef(), None, ptr::null_mut(), &mut client), 0);
            assert_eq!(MIDIInputPortCreate(client, pname.as_concrete_TypeRef(), Some(rd), ptr::null_mut(), &mut port), 0);
        }
        clients.push(client);
        ports.push(port);
    }
    wait(1500);
    for i in 0..2 {
        log(&format!("client {i}: connect"));
        unsafe { assert_eq!(MIDIPortConnectSource(ports[i], src, ptr::null_mut()), 0) };
        wait(1500);
    }
    for i in 0..2 {
        log(&format!("client {i}: disconnect"));
        unsafe { assert_eq!(MIDIPortDisconnectSource(ports[i], src), 0) };
        wait(1500);
    }
    log("client 0: connect again, then dispose the client without disconnecting");
    unsafe { assert_eq!(MIDIPortConnectSource(ports[0], src, ptr::null_mut()), 0) };
    wait(1500);
    unsafe { MIDIClientDispose(clients[0]) };
    wait(1500);
    log("client 1: connect, then process exit follows");
    unsafe { assert_eq!(MIDIPortConnectSource(ports[1], src, ptr::null_mut()), 0) };
    wait(1500);
    std::process::exit(0);
}
