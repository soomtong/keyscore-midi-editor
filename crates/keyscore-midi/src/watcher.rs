//! 포트 목록 감시(OUT-3, REQ 9.2 백그라운드).

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use coremidi_sys::{
    MIDIClientCreate, MIDIClientDispose, MIDIClientRef, MIDINotification, kMIDIMsgSetupChanged,
};

use crate::client::check;
use crate::midi_thread::on_midi_thread;
use crate::{
    DEFAULT_PORT_NAME, MidiClient, MidiError, PortEvent, PortInfo, destinations, diff_ports,
};

/// 알림 콜백이 쓰는 상태. 콜백은 CoreMIDI 스레드에서 불린다.
struct State {
    last: Vec<PortInfo>,
    events: Sender<PortEvent>,
}

impl State {
    fn refresh(&mut self) {
        let now = destinations();
        for event in diff_ports(&self.last, &now) {
            // 받는 쪽이 사라졌으면 더 보낼 곳이 없다. 감시는 drop에서 멈춘다.
            let _ = self.events.send(event);
        }
        self.last = now;
    }
}

type Shared = Mutex<State>;

/// CoreMIDI 알림 콜백. 설정이 바뀔 때마다 목적지 목록을 다시 읽어 비교한다.
///
/// `ObjectAdded`·`ObjectRemoved` 뒤에는 늘 `SetupChanged`가 오므로, 그것 하나로 묶어 처리한다.
#[allow(unsafe_code)]
unsafe extern "C" fn on_notification(message: *const MIDINotification, refcon: *mut c_void) {
    // 패닉이 FFI 경계를 넘으면 정의되지 않은 동작이라 여기서 멈춘다.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if message.is_null() || refcon.is_null() {
            return;
        }
        // SAFETY: CoreMIDI가 넘기는 알림 포인터는 콜백 동안 유효하다. 머리 필드만 읽는다.
        let id = unsafe { (*message).messageID };
        if id != kMIDIMsgSetupChanged.cast_signed() {
            return;
        }
        // SAFETY: refcon은 `PortWatcher`가 `Arc::into_raw`로 붙잡은 `Shared`이고, 알림 클라이언트를
        // 해제한 뒤에야 그 참조를 놓으므로 콜백 동안 살아 있다. 공유 참조만 만든다.
        let shared = unsafe { &*refcon.cast::<Shared>() };
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .refresh();
    }));
}

/// CoreMIDI 알림을 받아 목적지 목록의 변화를 이벤트로 보낸다. drop하면 알림을 끊고 채널을 닫는다.
///
/// 알림은 CoreMIDI 전용 스레드(`midi_thread`)의 run loop에서 처리되므로 따로 스레드를 두지 않는다.
pub struct PortWatcher {
    client: MIDIClientRef,
    // `Arc::into_raw`로 콜백에 넘긴 참조. drop에서 클라이언트를 해제한 뒤 되돌려 놓는다.
    refcon: usize,
}

impl PortWatcher {
    /// 감시를 시작한다. 받는 쪽은 `PortEvent`를 순서대로 받는다(`diff_ports`의 순서).
    ///
    /// 시작 시점의 목록은 이벤트로 보내지 않는다. 지금 목록은 `destinations()`로 읽는다.
    ///
    /// # Errors
    ///
    /// 알림 클라이언트를 만들지 못하면 `Os`.
    #[allow(unsafe_code)]
    pub fn start() -> Result<(Self, Receiver<PortEvent>), MidiError> {
        // 프로세스 클라이언트를 먼저 붙잡아 둔다. 그래야 알림 클라이언트를 해제해도 「마지막 클라이언트
        // 해제」로 MIDI 서버가 내려가지 않는다(S1-T1 리뷰 1 지적 1).
        MidiClient::new(DEFAULT_PORT_NAME)?;
        let (tx, rx) = mpsc::channel();
        let shared: Arc<Shared> = Arc::new(Mutex::new(State {
            last: destinations(),
            events: tx,
        }));
        let refcon = Arc::into_raw(Arc::clone(&shared)) as usize;
        let created = on_midi_thread(move || {
            let name = CFString::new("KeyScore port watcher");
            let mut client: MIDIClientRef = 0;
            // SAFETY: 이름 문자열은 호출 동안 살아 있고, 출력 포인터는 이 스택 변수를 가리킨다.
            // refcon은 위에서 붙잡은 `Shared`를 가리키며 클라이언트 해제 뒤까지 산다.
            let status = unsafe {
                MIDIClientCreate(
                    name.as_concrete_TypeRef(),
                    Some(on_notification),
                    refcon as *mut c_void,
                    &raw mut client,
                )
            };
            check("MIDIClientCreate", status).map(|()| client)
        });
        let client = match created.and_then(|r| r) {
            Ok(client) => client,
            Err(e) => {
                // SAFETY: 위의 `into_raw`가 넘긴 참조를 되돌린다. 클라이언트가 없어 콜백이 오지 않는다.
                drop(unsafe { Arc::from_raw(refcon as *const Shared) });
                return Err(e);
            }
        };
        // 클라이언트를 만들기 전과 사이에 생긴 변화를 놓치지 않게 한 번 다시 맞춘다.
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .refresh();
        Ok((PortWatcher { client, refcon }, rx))
    }
}

impl Drop for PortWatcher {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let client = self.client;
        // 알림 클라이언트는 그것을 만든 스레드에서 해제한다. 해제가 끝나면 콜백이 더 오지 않는다.
        let _ = on_midi_thread(move || {
            // SAFETY: `start`가 만든 클라이언트이고 여기서 한 번만 해제한다. 프로세스 클라이언트가 살아
            // 있어 마지막 클라이언트가 아니다.
            let _ = unsafe { MIDIClientDispose(client) };
        });
        // SAFETY: `start`의 `into_raw`가 넘긴 참조다. 클라이언트를 해제했으므로 콜백이 더 쓰지 않는다.
        // 이것이 마지막 참조라 `Sender`가 사라져 받는 쪽 채널이 닫힌다.
        drop(unsafe { Arc::from_raw(self.refcon as *const Shared) });
    }
}
