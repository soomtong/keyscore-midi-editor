//! OS 출력 포트(CoreMIDI 목적지)의 열거와 송신(OUT-3).

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use coremidi_sys::{
    ItemCount, MIDIEndpointRef, MIDIGetDestination, MIDIGetNumberOfDestinations,
    MIDIObjectFindByUniqueID, MIDIObjectGetIntegerProperty, MIDIObjectRef, MIDIObjectType,
    MIDIOutputPortCreate, MIDIPortDispose, MIDIPortRef, MIDISendEventList, kMIDIPropertyUniqueID,
};

use crate::client::{
    ENUMERATION_ATTEMPTS, check, endpoint_name, forget_client_if_server_gone, with_event_list,
};
use crate::{HostTime, MidiClient, MidiError, MidiPort, PortInfo};

/// 사라진 엔드포인트에 보낼 때 CoreMIDI 호출이 돌려주는 상태 값: `kMIDIUnknownEndpoint`,
/// `kMIDIObjectNotFound`, `kMIDINoConnection`.
fn is_endpoint_gone(status: i32) -> bool {
    matches!(status, -10834 | -10842 | -10833)
}

/// 시스템의 MIDI 목적지(OS 출력 포트)를 열거 순서대로 돌려준다. 이름이나 고유 ID를 읽지 못한 것은
/// 건너뛴다. 열거 중 목록이 바뀌면 다시 읽는다(`source_names`와 같은 이유).
#[must_use]
pub fn destinations() -> Vec<PortInfo> {
    let mut ports = Vec::new();
    for _ in 0..ENUMERATION_ATTEMPTS {
        let endpoints = destination_endpoints();
        ports = endpoints.iter().filter_map(|&e| port_info(e)).collect();
        if destination_endpoints() == endpoints {
            break;
        }
    }
    ports
}

#[allow(unsafe_code)]
fn destination_endpoints() -> Vec<MIDIEndpointRef> {
    // SAFETY: 인자가 없는 조회 함수다.
    let count = unsafe { MIDIGetNumberOfDestinations() };
    (0..count)
        // SAFETY: 범위 밖 인덱스면 0을 돌려주는 조회 함수다.
        .map(|i| unsafe { MIDIGetDestination(i as ItemCount) })
        .collect()
}

#[allow(unsafe_code)]
fn port_info(endpoint: MIDIEndpointRef) -> Option<PortInfo> {
    let name = endpoint_name(endpoint)?;
    let mut id: i32 = 0;
    // SAFETY: 출력 포인터는 이 스택 변수를 가리키고, 속성 키는 CoreMIDI 상수다.
    let status =
        unsafe { MIDIObjectGetIntegerProperty(endpoint, kMIDIPropertyUniqueID, &raw mut id) };
    (status == 0).then(|| PortInfo::new(name, id).with_endpoint(endpoint))
}

/// 고유 ID로 지금의 목적지 엔드포인트를 찾는다.
#[allow(unsafe_code)]
fn find_destination(unique_id: i32) -> Option<MIDIEndpointRef> {
    let mut object: MIDIObjectRef = 0;
    let mut kind: MIDIObjectType = 0;
    // SAFETY: 출력 포인터는 이 스택 변수들을 가리킨다.
    let status = unsafe { MIDIObjectFindByUniqueID(unique_id, &raw mut object, &raw mut kind) };
    // kMIDIObjectType_Destination = 3. 같은 ID의 다른 종류 객체(소스 등)는 받지 않는다.
    (status == 0 && object != 0 && kind == 3).then_some(object)
}

/// OS 출력 포트로 보내는 연결. drop하면 출력 포트를 해제한다.
pub struct OsOutput {
    port: MIDIPortRef,
    destination: MIDIEndpointRef,
    unique_id: i32,
    name: String,
}

impl OsOutput {
    /// `port`를 연다.
    ///
    /// # Errors
    ///
    /// 그 고유 ID의 목적지가 없으면 `PortGone`, CoreMIDI 호출이 실패하면 `Os`.
    #[allow(unsafe_code)]
    pub fn open(client: &MidiClient, port: &PortInfo) -> Result<Self, MidiError> {
        let Some(destination) = find_destination(port.unique_id()) else {
            return Err(MidiError::PortGone(port.name().to_owned()));
        };
        let raw = client.raw()?;
        let cf_name = CFString::new(port.name());
        let mut out: MIDIPortRef = 0;
        // SAFETY: 클라이언트는 해제되지 않아 유효하고, 이름 문자열은 호출 동안 살아 있으며, 출력
        // 포인터는 이 스택 변수를 가리킨다.
        let status =
            unsafe { MIDIOutputPortCreate(raw, cf_name.as_concrete_TypeRef(), &raw mut out) };
        forget_client_if_server_gone(raw, status);
        check("MIDIOutputPortCreate", status)?;
        Ok(OsOutput {
            port: out,
            destination,
            unique_id: port.unique_id(),
            name: port.name().to_owned(),
        })
    }
}

impl OsOutput {
    /// 연결한 목적지가 아직 시스템에 있는가. CoreMIDI 서버는 사라진 목적지로의 송신을 오류 없이 버리므로,
    /// 송신 결과로는 알 수 없다. 스케줄러가 아니라 제어 스레드에서 부른다(조회 호출이 든다).
    #[must_use]
    pub fn is_present(&self) -> bool {
        find_destination(self.unique_id) == Some(self.destination)
    }
}

impl Drop for OsOutput {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: 출력 포트는 이 값만 소유하고 drop에서 한 번만 해제한다. 실패해도 할 일이 없다.
        let _ = unsafe { MIDIPortDispose(self.port) };
    }
}

impl MidiPort for OsOutput {
    fn name(&self) -> &str {
        &self.name
    }

    #[allow(unsafe_code)]
    fn send(&self, at: HostTime, ump: &[u32]) -> Result<(), MidiError> {
        let status = with_event_list(at, ump, |list| {
            // SAFETY: 리스트는 `with_event_list`의 유효한 스택 버퍼이고, 출력 포트는 이 값이 소유한다.
            // 목적지가 사라졌으면 CoreMIDI가 오류 상태를 돌려준다.
            unsafe { MIDISendEventList(self.port, self.destination, list) }
        })?;
        if is_endpoint_gone(status) {
            // CoreMIDI가 이 오류를 돌려주는 경우에만 만든다(대개는 오류 없이 버린다, `is_present` 참고).
            // 성공 경로에는 할당이 없다.
            return Err(MidiError::PortGone(self.name.clone()));
        }
        check("MIDISendEventList", status)
    }
}
