use std::mem::{MaybeUninit, size_of};
use std::ptr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use coremidi_sys::{
    ByteCount, ItemCount, MIDIClientCreate, MIDIClientRef, MIDIEndpointDispose, MIDIEndpointRef,
    MIDIEventList, MIDIEventListAdd, MIDIEventListInit, MIDIGetNumberOfSources, MIDIGetSource,
    MIDIObjectGetStringProperty, MIDIObjectSetStringProperty, MIDIProtocolID,
    MIDIReceivedEventList, MIDISourceCreateWithProtocol, kMIDIPropertyName, kMIDIProtocol_1_0,
};

use crate::{HostTime, MidiError, MidiPort, ump_word_count};

/// 한 `MidiClient`가 만들 수 있는 가상 출력 포트 수(OUT-2). 앱은 `MidiClient`를 하나만 둔다.
pub const MAX_VIRTUAL_OUTPUTS: usize = 16;

/// v1 가상 포트는 MIDI 1.0 프로토콜이다(OUT-1). 바인딩의 상수 타입이 `MIDIProtocolID`와 달라 맞춘다.
#[allow(clippy::cast_possible_wrap)]
const PROTOCOL_1_0: MIDIProtocolID = kMIDIProtocol_1_0 as MIDIProtocolID;

fn check(call: &'static str, status: i32) -> Result<(), MidiError> {
    if status == 0 {
        Ok(())
    } else {
        Err(MidiError::Os { call, status })
    }
}

/// 앞뒤 공백을 뗀 이름. 신디의 입력 목록에서 공백만 다른 이름은 구분되지 않으므로, 떼어 낸 이름을
/// 포트 이름으로 쓰고 중복도 그 이름으로 판정한다.
fn normalized_name(name: &str) -> Result<&str, MidiError> {
    let name = name.trim();
    if name.is_empty() {
        Err(MidiError::InvalidName)
    } else {
        Ok(name)
    }
}

/// 프로세스의 CoreMIDI 클라이언트. 한 번 만들면 해제하지 않는다.
///
/// `MIDIServices.h`: 앱의 마지막 클라이언트를 해제하면 MIDI 서버가 종료할 수 있고, 그 뒤 그
/// 프로세스의 `MIDIClientCreate`는 모두 실패한다(S1-T1 리뷰 1 지적 1). 그래서 모든 `MidiClient`가
/// 이 클라이언트 하나를 나눠 쓰고, 프로세스가 끝날 때 OS가 정리하게 둔다.
static PROCESS_CLIENT: Mutex<Option<MIDIClientRef>> = Mutex::new(None);

/// 서버가 막 내려가거나 올라오는 중이면 생성이 잠깐 실패한다(리뷰 1 지적 3). 그동안만 다시 시도한다.
const CLIENT_CREATE_PATIENCE: Duration = Duration::from_secs(3);

#[allow(unsafe_code)]
fn process_client(name: &str) -> Result<MIDIClientRef, MidiError> {
    let mut slot = PROCESS_CLIENT
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(raw) = *slot {
        return Ok(raw);
    }
    let cf_name = CFString::new(name);
    let start = Instant::now();
    let mut delay = Duration::from_millis(20);
    loop {
        let mut raw: MIDIClientRef = 0;
        // SAFETY: 이름 문자열은 호출 동안 살아 있고, 출력 포인터는 이 스택 변수를 가리킨다.
        // 알림 함수가 없으므로 refCon은 쓰이지 않는다.
        let status = unsafe {
            MIDIClientCreate(
                cf_name.as_concrete_TypeRef(),
                None,
                ptr::null_mut(),
                &raw mut raw,
            )
        };
        if status == 0 {
            *slot = Some(raw);
            return Ok(raw);
        }
        if start.elapsed() >= CLIENT_CREATE_PATIENCE {
            return Err(MidiError::Os {
                call: "MIDIClientCreate",
                status,
            });
        }
        std::thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_millis(500));
    }
}

/// 프로세스 클라이언트 핸들과, 이 `MidiClient`가 만든 가상 포트의 이름 목록.
struct ClientInner {
    raw: MIDIClientRef,
    // 이름 중복·개수 검사용. 생성·이름 변경·해제에서만 잠그고 송신 경로에서는 쓰지 않는다.
    names: Mutex<Vec<String>>,
}

impl ClientInner {
    fn names(&self) -> MutexGuard<'_, Vec<String>> {
        // 잠근 쪽이 패닉해도 목록은 항상 일관된 상태로만 바뀌므로 그대로 쓴다.
        self.names.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// CoreMIDI 클라이언트. 복제하면 같은 클라이언트를 공유한다.
#[derive(Clone)]
pub struct MidiClient {
    inner: Arc<ClientInner>,
}

impl MidiClient {
    /// 포트 목록을 따로 갖는 클라이언트 핸들을 만든다.
    ///
    /// CoreMIDI 클라이언트 자체는 프로세스에 하나이고 해제하지 않는다(`PROCESS_CLIENT`). `name`은
    /// 그 클라이언트를 처음 만들 때만 쓰인다. 16개 상한과 이름 중복 검사는 이 핸들 단위이므로, 앱은
    /// 핸들을 하나만 만들어 복제해 쓴다. 알림 콜백은 두지 않는다. 핫플러그 알림은 run loop을 소유하는
    /// S1-T2의 감시 스레드가 받는다.
    ///
    /// # Errors
    ///
    /// 이름이 비었으면 `InvalidName`, CoreMIDI 호출이 실패하면 `Os`.
    pub fn new(name: &str) -> Result<Self, MidiError> {
        let name = normalized_name(name)?;
        let raw = process_client(name)?;
        Ok(MidiClient {
            inner: Arc::new(ClientInner {
                raw,
                names: Mutex::new(Vec::new()),
            }),
        })
    }

    /// MIDI 1.0 프로토콜 가상 소스(앱의 출력 포트)를 만든다(OUT-1, OUT-2).
    ///
    /// # Errors
    ///
    /// 이름이 비었으면 `InvalidName`, 같은 클라이언트에 같은 이름이 있으면 `DuplicateName`,
    /// 이미 16개면 `TooManyPorts`, CoreMIDI 호출이 실패하면 `Os`.
    #[allow(unsafe_code)]
    pub fn create_virtual_output(&self, name: &str) -> Result<VirtualOutput, MidiError> {
        let name = normalized_name(name)?;
        // 검사와 등록 사이에 다른 스레드가 같은 이름을 넣지 못하게 끝까지 잠근다.
        let mut names = self.inner.names();
        if names.iter().any(|n| n == name) {
            return Err(MidiError::DuplicateName(name.to_owned()));
        }
        if names.len() >= MAX_VIRTUAL_OUTPUTS {
            return Err(MidiError::TooManyPorts);
        }
        let cf_name = CFString::new(name);
        let mut endpoint: MIDIEndpointRef = 0;
        // SAFETY: 프로세스 클라이언트는 해제되지 않아 유효하고, 이름 문자열은 호출 동안 살아 있으며,
        // 출력 포인터는 이 스택 변수를 가리킨다.
        let status = unsafe {
            MIDISourceCreateWithProtocol(
                self.inner.raw,
                cf_name.as_concrete_TypeRef(),
                PROTOCOL_1_0,
                &raw mut endpoint,
            )
        };
        check("MIDISourceCreateWithProtocol", status)?;
        names.push(name.to_owned());
        Ok(VirtualOutput {
            client: Arc::clone(&self.inner),
            endpoint,
            name: name.to_owned(),
        })
    }
}

/// 가상 출력 포트. drop되면 CoreMIDI 쪽에서도 해제된다.
pub struct VirtualOutput {
    client: Arc<ClientInner>,
    endpoint: MIDIEndpointRef,
    name: String,
}

impl VirtualOutput {
    /// 포트 이름을 바꾼다. 같은 클라이언트의 다른 포트 이름과 겹치면 안 된다.
    ///
    /// # Errors
    ///
    /// `InvalidName`, `DuplicateName`, `Os`. 실패하면 이름은 그대로다.
    #[allow(unsafe_code)]
    pub fn rename(&mut self, name: &str) -> Result<(), MidiError> {
        let name = normalized_name(name)?;
        if name == self.name {
            return Ok(());
        }
        let mut names = self.client.names();
        if names.iter().any(|n| n == name) {
            return Err(MidiError::DuplicateName(name.to_owned()));
        }
        let cf_name = CFString::new(name);
        // SAFETY: 엔드포인트는 이 값이 소유해 아직 유효하고, 속성 키는 CoreMIDI가 제공하는 상수이며,
        // 이름 문자열은 호출 동안 살아 있다.
        let status = unsafe {
            MIDIObjectSetStringProperty(
                self.endpoint,
                kMIDIPropertyName,
                cf_name.as_concrete_TypeRef(),
            )
        };
        check("MIDIObjectSetStringProperty", status)?;
        if let Some(slot) = names.iter_mut().find(|n| **n == self.name) {
            name.clone_into(slot);
        }
        name.clone_into(&mut self.name);
        Ok(())
    }
}

impl Drop for VirtualOutput {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let mut names = self.client.names();
        if let Some(i) = names.iter().position(|n| *n == self.name) {
            names.swap_remove(i);
        }
        // SAFETY: 엔드포인트는 이 값만 소유하고 drop에서 한 번만 해제한다. 프로세스 클라이언트는
        // 해제되지 않는다. 실패해도 할 수 있는 일이 없다. 이름 목록 잠금을 해제가 끝날 때까지 쥐어,
        // 같은 이름의 새 포트가 해제 전에 생기지 않게 한다.
        let _ = unsafe { MIDIEndpointDispose(self.endpoint) };
    }
}

impl MidiPort for VirtualOutput {
    fn name(&self) -> &str {
        &self.name
    }

    #[allow(unsafe_code)]
    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        if at.ticks() == 0 {
            return Err(MidiError::ZeroTimestamp);
        }
        let Some(&first) = ump.first() else {
            return Err(MidiError::InvalidMessage);
        };
        if ump.len() != ump_word_count(first) {
            return Err(MidiError::InvalidMessage);
        }
        // 실시간 스레드에서 불리므로 힙을 쓰지 않는다. `MIDIEventList`는 패킷 하나와 64워드를 담는
        // 고정 크기 구조체라 메시지 하나(최대 4워드)에 충분하다.
        let mut list = MaybeUninit::<MIDIEventList>::uninit();
        let list_ptr = list.as_mut_ptr();
        // 워드 수는 1–4다.
        let words = ump.len() as ByteCount;
        // SAFETY: `list_ptr`는 이 스택 버퍼를 가리키고, Init이 헤더를 채운 뒤 Add가 그 안에만 쓴다
        // (버퍼 크기를 정확히 넘긴다). 워드 포인터는 길이를 검사한 슬라이스다. 엔드포인트는 이 값이
        // 소유해 유효하다. 버퍼는 호출이 끝날 때까지 살아 있다.
        let status = unsafe {
            let packet = MIDIEventListInit(list_ptr, PROTOCOL_1_0);
            let packet = MIDIEventListAdd(
                list_ptr,
                size_of::<MIDIEventList>() as ByteCount,
                packet,
                at.ticks(),
                words,
                ump.as_ptr(),
            );
            if packet.is_null() {
                return Err(MidiError::InvalidMessage);
            }
            MIDIReceivedEventList(self.endpoint, list_ptr)
        };
        check("MIDIReceivedEventList", status)
    }
}

/// 열거가 흔들릴 때 다시 시도하는 횟수. 소스가 계속 생기고 사라지면 마지막 결과를 쓴다.
const ENUMERATION_ATTEMPTS: usize = 8;

/// 시스템의 MIDI 소스 이름을 열거 순서대로 돌려준다. 이름을 읽지 못한 소스는 건너뛴다.
///
/// CoreMIDI 열거는 인덱스 기반이라, 열거 중 앞쪽 소스가 사라지면 인덱스가 당겨져 살아 있는 소스를
/// 건너뛴다(리뷰 1 지적 2). 그래서 엔드포인트 목록을 읽고 이름을 모은 뒤 목록이 그대로인지 다시
/// 확인하고, 바뀌었으면 처음부터 다시 읽는다.
#[must_use]
pub fn source_names() -> Vec<String> {
    let mut names = Vec::new();
    for _ in 0..ENUMERATION_ATTEMPTS {
        let endpoints = source_endpoints();
        names = endpoints.iter().filter_map(|&e| endpoint_name(e)).collect();
        if source_endpoints() == endpoints {
            break;
        }
    }
    names
}

#[allow(unsafe_code)]
fn source_endpoints() -> Vec<MIDIEndpointRef> {
    // SAFETY: 인자가 없는 조회 함수다.
    let count = unsafe { MIDIGetNumberOfSources() };
    (0..count)
        // SAFETY: 범위 밖 인덱스면 0을 돌려주는 조회 함수다. 열거 중 소스가 사라져도 0이 온다.
        .map(|i| unsafe { MIDIGetSource(i as ItemCount) })
        .collect()
}

#[allow(unsafe_code)]
fn endpoint_name(endpoint: MIDIEndpointRef) -> Option<String> {
    if endpoint == 0 {
        return None;
    }
    let mut raw: CFStringRef = ptr::null();
    // SAFETY: 출력 포인터는 이 스택 변수를 가리키고, 속성 키는 CoreMIDI 상수다. 사라진
    // 엔드포인트면 오류 상태가 온다.
    let status = unsafe { MIDIObjectGetStringProperty(endpoint, kMIDIPropertyName, &raw mut raw) };
    if status != 0 || raw.is_null() {
        return None;
    }
    // SAFETY: Get 계열 CoreMIDI 속성 함수는 호출자가 해제할 새 참조를 돌려준다(create rule).
    let name = unsafe { CFString::wrap_under_create_rule(raw) };
    Some(name.to_string())
}
