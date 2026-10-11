//! 가상 소스 소유자. peer가 연결/해제하는 동안 CoreMIDI 알림(raw messageID)과
//! 소스 속성 변화를 기록한다. 사용: owner <seconds>
#![allow(unsafe_code, deprecated)]

use std::ffi::c_void;
use std::ptr;
use std::time::{Duration, Instant};

use core_foundation::base::TCFType;
use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};
use core_foundation::string::CFString;
use core_foundation_sys::base::{CFCopyDescription, CFRelease, CFTypeRef};
use core_foundation_sys::propertylist::CFPropertyListRef;
use core_foundation_sys::string::CFStringRef;
use coremidi_sys::*;

static mut T0: Option<Instant> = None;

fn ts() -> u128 {
    // 단일 스레드 초기화 후 읽기만 한다.
    unsafe { (*ptr::addr_of!(T0)).unwrap().elapsed().as_millis() }
}

/// 알림 콜백은 run loop 스레드(메인)에서 불린다. 로그만 찍는다.
unsafe extern "C" fn on_notify(msg: *const MIDINotification, _: *mut c_void) {
    let id = (*msg).messageID;
    let extra = match id as u32 {
        kMIDIMsgObjectAdded | kMIDIMsgObjectRemoved => {
            let m = &*(msg as *const MIDIObjectAddRemoveNotification);
            format!(" parentType={} childType={}", m.parentType, m.childType)
        }
        kMIDIMsgPropertyChanged => {
            let m = &*(msg as *const MIDIObjectPropertyChangeNotification);
            let name = CFString::wrap_under_get_rule(m.propertyName);
            format!(" objType={} prop={}", m.objectType, name)
        }
        _ => String::new(),
    };
    println!("[{:>6} ms] notif id={}{}", ts(), id, extra);
}

/// 객체의 전체 속성을 문자열로 덤프한다(MIDIObjectGetProperties, deep=false).
unsafe fn dump(obj: MIDIObjectRef) -> String {
    let mut plist: CFPropertyListRef = ptr::null();
    let s = MIDIObjectGetProperties(obj, &mut plist, 0);
    if s != 0 || plist.is_null() {
        return format!("<err {s}>");
    }
    let d = CFCopyDescription(plist as CFTypeRef);
    let out = CFString::wrap_under_create_rule(d).to_string();
    CFRelease(plist as CFTypeRef);
    out
}

unsafe fn int_prop(obj: MIDIObjectRef, key: CFStringRef) -> String {
    let mut v: i32 = 0;
    let s = MIDIObjectGetIntegerProperty(obj, key, &mut v);
    if s == 0 { v.to_string() } else { format!("err{s}") }
}

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(40);
    // SAFETY: 아직 다른 스레드가 없다.
    unsafe { T0 = Some(Instant::now()) };
    let cname = CFString::new("sp4-owner");
    let sname = CFString::new("SP4 Virtual Source");
    let (mut client, mut src): (MIDIClientRef, MIDIEndpointRef) = (0, 0);
    // 출력 포인터는 스택 변수이고 CFString은 호출 동안 산다.
    unsafe {
        assert_eq!(MIDIClientCreate(cname.as_concrete_TypeRef(), Some(on_notify), ptr::null_mut(), &mut client), 0);
        assert_eq!(MIDISourceCreate(client, sname.as_concrete_TypeRef(), &mut src), 0);
        println!("[{:>6} ms] source created", ts());
        let mut last = dump(src);
        println!("--- initial properties ---\n{last}\n---");
        let end = Instant::now() + Duration::from_secs(secs);
        let mut last_cu = String::new();
        while Instant::now() < end {
            CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, Duration::from_millis(50), false);
            let cur = dump(src);
            if cur != last {
                println!("[{:>6} ms] PROPERTIES CHANGED\n{cur}\n---", ts());
                last = cur;
            }
            let cu = int_prop(src, kMIDIPropertyConnectionUniqueID);
            if cu != last_cu {
                println!("[{:>6} ms] ConnectionUniqueID = {cu}", ts());
                last_cu = cu;
            }
        }
        // 알림이 없는 상태에서 MIDIReceived가 리스너 유무와 무관하게 성공하는지 본다.
        let mut buf = [0u8; 64];
        let list = buf.as_mut_ptr() as *mut MIDIPacketList;
        let p = MIDIPacketListInit(list);
        MIDIPacketListAdd(list, 64, p, 0, 3, [0x90u8, 60, 100].as_ptr());
        println!("MIDIReceived with listeners present/absent -> OSStatus {}", MIDIReceived(src, list));
        MIDIEndpointDispose(src);
        MIDIClientDispose(client);
    }
}
