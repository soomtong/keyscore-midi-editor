//! SP-3/SP-4 공용 보조 코드. 제품 코드가 아니다.
#![allow(unsafe_code, deprecated)]

use std::sync::OnceLock;

use coremidi::Notification;

/// mach 틱을 나노초로 바꾸는 비율. Apple Silicon은 125/3이라 1:1이 아니다.
fn timebase() -> (u64, u64) {
    static TB: OnceLock<(u64, u64)> = OnceLock::new();
    *TB.get_or_init(|| {
        let mut info = libc::mach_timebase_info { numer: 0, denom: 0 };
        // 출력 구조체 포인터만 넘기는 읽기 전용 시스템 호출이라 안전하다.
        unsafe { libc::mach_timebase_info(&mut info) };
        (u64::from(info.numer), u64::from(info.denom))
    })
}

pub fn now_ticks() -> u64 {
    // 인자 없는 시스템 호출이다.
    unsafe { libc::mach_absolute_time() }
}

pub fn ticks_to_ns(t: u64) -> u64 {
    let (n, d) = timebase();
    (u128::from(t) * u128::from(n) / u128::from(d)) as u64
}

pub fn ns_to_ticks(ns: u64) -> u64 {
    let (n, d) = timebase();
    (u128::from(ns) * u128::from(d) / u128::from(n)) as u64
}

pub fn describe(n: &Notification) -> String {
    match n {
        Notification::SetupChanged => "SetupChanged".into(),
        Notification::ObjectAdded(i) => format!("ObjectAdded parent={:?} child={:?}", i.parent, i.child),
        Notification::ObjectRemoved(i) => format!("ObjectRemoved parent={:?} child={:?}", i.parent, i.child),
        Notification::PropertyChanged(i) => format!("PropertyChanged object={:?} prop={}", i.object, i.property_name),
        Notification::ThruConnectionsChanged => "ThruConnectionsChanged".into(),
        Notification::SerialPortOwnerChanged => "SerialPortOwnerChanged".into(),
        Notification::IoError(i) => format!("IoError code={}", i.error_code),
    }
}

/// 현재 스레드의 run loop를 `secs`초 동안 돌려 CoreMIDI 알림을 배달시킨다.
pub fn pump(secs: f64) {
    // run loop 호출은 현재 스레드 상태만 건드린다.
    core_foundation::runloop::CFRunLoop::run_in_mode(
        unsafe { core_foundation::runloop::kCFRunLoopDefaultMode },
        std::time::Duration::from_secs_f64(secs),
        false,
    );
}
