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
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::midi_thread::on_midi_thread;
use crate::{HostTime, MidiError, MidiPort, ump_word_count};

/// 프로세스가 만들 수 있는 가상 출력 포트 수(OUT-2). 모든 `MidiClient` 핸들을 합친 수다.
pub const MAX_VIRTUAL_OUTPUTS: usize = 16;

/// 앱의 기본 가상 출력 포트 이름(OUT-1).
pub const DEFAULT_PORT_NAME: &str = "KeyScore Out";

/// v1 가상 포트는 MIDI 1.0 프로토콜이다(OUT-1). 바인딩의 상수 타입이 `MIDIProtocolID`와 달라 맞춘다.
#[allow(clippy::cast_possible_wrap)]
pub(crate) const PROTOCOL_1_0: MIDIProtocolID = kMIDIProtocol_1_0 as MIDIProtocolID;

pub(crate) fn check(call: &'static str, status: i32) -> Result<(), MidiError> {
    if status == 0 {
        Ok(())
    } else {
        Err(MidiError::Os { call, status })
    }
}

/// 포트 이름으로 쓸 정규형. 신디의 입력 목록에서 구분되지 않는 이름이 둘 생기지 않게 한다.
///
/// - NFC로 맞춘다. 한글 NFD(Finder에서 복사한 이름 등)와 NFC는 같은 이름이다(리뷰 3 지적 1).
/// - 앞뒤 공백을 떼고, 안쪽 공백 연속은 종류(NBSP 등)와 상관없이 U+0020 한 칸으로 줄인다
///   (리뷰 1 지적 4, 결정 `2026-10-15-port-name-visual-duplicates.md`).
/// - 제어 문자, 줄·문단 구분자, 표시되지 않는 문자(유니코드 `Default_Ignorable_Code_Point`)는
///   거부한다. CoreMIDI 서버는 이름을 NUL 앞에서 잘라 저장하고(리뷰 2 지적 2), 양방향 제어 문자는 글자
///   순서를 뒤집어 다른 이름처럼 보이게 한다(리뷰 3 지적 2). 다만 이모지와 페르시아어 같은 정상
///   텍스트에 쓰이는 결합자(ZWNJ, ZWJ), 이체자 선택자, 태그 문자는 받는다(리뷰 3 제안 3).
/// - 보이는 글자가 하나도 없으면(공백·점자 빈칸 등만 남으면) 거부한다.
pub(crate) fn normalized_name(name: &str) -> Result<String, MidiError> {
    let nfc: String = name.nfc().collect();
    // 앞뒤 공백(탭·줄바꿈 포함)은 먼저 뗀다. 안쪽의 제어 문자·줄 구분자는 공백이기도 해서, 거부
    // 검사는 안쪽 공백을 줄이기 전에 한다.
    let nfc = nfc.trim();
    let mut visible = false;
    for c in nfc.chars() {
        if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') || rejected_ignorable(c) {
            return Err(MidiError::InvalidName);
        }
        visible |= !(c.is_whitespace() || is_blank_looking(c) || allowed_ignorable(c));
    }
    if visible {
        Ok(nfc.split_whitespace().collect::<Vec<_>>().join(" "))
    } else {
        Err(MidiError::InvalidName)
    }
}

/// 정상 텍스트에 쓰이는 표시되지 않는 문자: ZWNJ·ZWJ(이모지, 페르시아어), 이체자 선택자, 태그 문자.
fn allowed_ignorable(c: char) -> bool {
    matches!(
        c,
        '\u{200C}' | '\u{200D}' | '\u{FE00}'..='\u{FE0F}' | '\u{E0020}'..='\u{E007F}' | '\u{E0100}'..='\u{E01EF}'
    )
}

/// `Default_Ignorable_Code_Point`(Unicode `DerivedCoreProperties.txt`) 가운데 `allowed_ignorable`이 아닌 것.
fn rejected_ignorable(c: char) -> bool {
    !allowed_ignorable(c)
        && matches!(
            c,
            '\u{00AD}'
                | '\u{034F}'
                | '\u{061C}'
                | '\u{115F}'..='\u{1160}'
                | '\u{17B4}'..='\u{17B5}'
                | '\u{180B}'..='\u{180F}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{3164}'
                | '\u{FEFF}'
                | '\u{FFA0}'
                | '\u{FFF0}'..='\u{FFF8}'
                | '\u{1BCA0}'..='\u{1BCA3}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0000}'..='\u{E0FFF}'
        )
}

/// 보이는 모양이 없는 일반 문자: 점자 빈칸, 음악 기호 빈 머리, 한자 빈칸, 주석 구분 문자
/// (리뷰 4 제안 3).
fn is_blank_looking(c: char) -> bool {
    matches!(
        c,
        '\u{2800}' | '\u{1D159}' | '\u{16FE4}' | '\u{FFF9}'..='\u{FFFB}'
    )
}

/// 중복 판정 키. 신디 목록에서 같아 보이는 이름이 같은 키를 갖고, 달라 보이는 이름은 다른 키를 갖게
/// 한다(결정 `2026-10-15-port-name-visual-duplicates.md`).
///
/// - 앞뒤의 공백·빈칸 모양 문자·허용 예외 문자는 보이지 않으므로 뗀다.
/// - 안쪽의 허용 예외 문자(ZWNJ, ZWJ, 이체자 선택자, 태그)는 뺀다. 다만 그것이 글자를 갈라 놓아 따로
///   그려지게 하던 자리, 곧 ZWJ·ZWNJ 뒤의 결합 문자와 뺀 문자 뒤의 한글 중성·종성 낱자 앞에는 CGJ
///   (U+034F)를 둔다. CGJ는 NFC 합성을 막고, 이름에서는 거부되는 문자라 실제 이름과 겹치지 않는다
///   (리뷰 6 지적 1). 이체자 선택자는 결합 문자를 떼어 놓지 않으므로 그냥 뺀다.
/// - 폭이 없는 주석 구분 문자(U+FFF9–FFFB)는 위와 같이 빼고, 점자 빈칸처럼 폭이 있는 빈칸 모양 문자는
///   안쪽에서 남긴다.
/// - 그 뒤 NFC로 맞추고 공백 연속을 한 칸으로 줄인다(빼낸 문자가 감싸던 공백이 다시 이웃하므로).
fn duplicate_key(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let edge = |c: char| c.is_whitespace() || is_blank_looking(c) || allowed_ignorable(c);
    let start = chars.iter().position(|&c| !edge(c)).unwrap_or(chars.len());
    let end = chars
        .iter()
        .rposition(|&c| !edge(c))
        .map_or(start, |i| i + 1);
    let mut kept = String::new();
    let mut i = start;
    while i < end {
        if !removable(chars[i]) {
            kept.push(chars[i]);
            i += 1;
            continue;
        }
        let run_start = i;
        while i < end && removable(chars[i]) {
            i += 1;
        }
        // `end` 바로 앞 글자는 가장자리 문자가 아니므로 이 묶음 뒤에는 항상 글자가 있다.
        let next = chars[i];
        let joiner = chars[run_start..i]
            .iter()
            .any(|&c| matches!(c, '\u{200C}' | '\u{200D}'));
        if (joiner && is_combining_mark(next)) || is_hangul_medial_or_final(next) {
            kept.push('\u{034F}');
        }
    }
    let nfc: String = kept.nfc().collect();
    nfc.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 키에서 빼는 문자: 허용한 표시되지 않는 문자와 폭이 없는 주석 구분 문자(U+FFF9–FFFB).
/// 점자 빈칸처럼 폭이 있는 빈칸 모양 문자는 안쪽에서 남긴다.
fn removable(c: char) -> bool {
    allowed_ignorable(c) || matches!(c, '\u{FFF9}'..='\u{FFFB}')
}

/// 한글 자모의 중성·종성 낱자(옛한글 확장 포함). 앞 글자와 NFC로 합쳐져 음절이 될 수 있다.
fn is_hangul_medial_or_final(c: char) -> bool {
    matches!(c, '\u{1160}'..='\u{11FF}' | '\u{D7B0}'..='\u{D7FB}')
}

/// 프로세스의 CoreMIDI 클라이언트. 한 번 만들면 해제하지 않는다.
///
/// `MIDIServices.h`: 앱의 마지막 클라이언트를 해제하면 MIDI 서버가 종료할 수 있고, 그 뒤 그
/// 프로세스의 `MIDIClientCreate`는 모두 실패한다(S1-T1 리뷰 1 지적 1). 그래서 모든 `MidiClient`가
/// 이 클라이언트 하나를 나눠 쓰고, 프로세스가 끝날 때 OS가 정리하게 둔다.
static PROCESS_CLIENT: Mutex<Option<MIDIClientRef>> = Mutex::new(None);

/// 서버가 막 내려가거나 올라오는 중이면 생성이 잠깐 실패한다(리뷰 1 지적 3). 그동안만 다시 시도한다.
/// 리뷰 4에서 `-304`가 3초 넘게 이어진 실행이 있었다(유휴 뒤 서버 재기동). 앱 시작 때 한 번만 기다리는
/// 값이라 넉넉히 잡는다.
const CLIENT_CREATE_PATIENCE: Duration = Duration::from_secs(10);

/// `slot`에 클라이언트가 없으면 `create`로 만든다. 실패하면 `patience` 동안 다시 시도하되, 기다리는
/// 동안에는 잠금을 놓는다(리뷰 5 제안 2: 잠금을 쥔 채 잠들면 동시에 부른 스레드가 차례로 막힌다).
fn create_with_retry(
    slot: &Mutex<Option<MIDIClientRef>>,
    patience: Duration,
    mut create: impl FnMut() -> Result<MIDIClientRef, i32>,
) -> Result<MIDIClientRef, MidiError> {
    let start = Instant::now();
    let mut delay = Duration::from_millis(20);
    loop {
        let status = {
            let mut guard = slot.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(raw) = *guard {
                return Ok(raw);
            }
            match create() {
                Ok(raw) => {
                    *guard = Some(raw);
                    return Ok(raw);
                }
                Err(status) => status,
            }
        };
        if start.elapsed() >= patience {
            return Err(MidiError::Os {
                call: "MIDIClientCreate",
                status,
            });
        }
        std::thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_millis(500));
    }
}

/// 캐시된 클라이언트가 더는 쓸 수 없음을 뜻하는 상태 값(S1-T2 리뷰 1 지적 5에서 좁혔다).
///
/// - `kMIDIInvalidClient`(-10830): 클라이언트 참조가 무효다(서버 재시작 뒤).
/// - `kMIDIServerStartErr`(-10839): 서버를 띄우지 못했다.
/// - `MIG_SERVER_DIED`(-308): 서버와의 Mach IPC 상대가 죽었다.
///
/// `MIG_BAD_ARGUMENTS`(-304) 같은 다른 MIG 오류는 서버 소멸이 아니라 생성 재시도(`create_with_retry`)가
/// 다룬다. `kMIDIMessageSendErr`(-10838)는 송신 하나의 실패라 캐시를 버릴 근거가 아니다.
fn is_server_gone(status: i32) -> bool {
    matches!(status, -10830 | -10839 | -308)
}

/// 서버가 사라진 뒤의 오류를 받으면 캐시된 프로세스 클라이언트를 버린다. 다음 `MidiClient` 호출이 새
/// 서버에 클라이언트를 다시 만든다. 그 서버의 가상 포트는 서버와 함께 사라졌으므로, 다시 등록하는 일은
/// 앱(S2)이 맡는다.
pub(crate) fn forget_client_if_server_gone(raw: MIDIClientRef, status: i32) {
    forget_in(&PROCESS_CLIENT, raw, status);
}

/// `slot`이 아직 `raw`를 들고 있고 `status`가 서버 소멸이면 비운다. 그사이 다른 호출이 새 클라이언트를
/// 만들었으면 그것은 건드리지 않는다.
fn forget_in(slot: &Mutex<Option<MIDIClientRef>>, raw: MIDIClientRef, status: i32) {
    if is_server_gone(status) {
        let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
        if *slot == Some(raw) {
            *slot = None;
        }
    }
}

#[allow(unsafe_code)]
fn process_client(name: &str) -> Result<MIDIClientRef, MidiError> {
    create_with_retry(&PROCESS_CLIENT, CLIENT_CREATE_PATIENCE, || {
        let name = name.to_owned();
        // 클라이언트는 CoreMIDI 스레드에서 만든다. 알림이 그 스레드의 run loop으로 오게 하려는 것이다
        // (`midi_thread` 모듈 문서).
        on_midi_thread(move || {
            let cf_name = CFString::new(&name);
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
            if status == 0 { Ok(raw) } else { Err(status) }
        })
        .unwrap_or(Err(-1))
    })
}

/// 프로세스의 가상 포트 이름의 중복 판정 키(`duplicate_key`) 목록. 16개 상한과 이름 중복은 프로세스 전체에서 지킨다(OUT-2,
/// 결정 `docs/decisions/2026-10-15-virtual-port-limit-scope.md`). 생성·이름 변경·해제에서만 잠그고
/// 송신 경로에서는 쓰지 않는다.
static PORT_NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn port_names() -> MutexGuard<'static, Vec<String>> {
    // 잠근 쪽이 패닉해도 목록은 항상 일관된 상태로만 바뀌므로 그대로 쓴다.
    PORT_NAMES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 프로세스 CoreMIDI 클라이언트의 핸들. 몇 개를 만들든 같은 클라이언트와 같은 포트 목록을 쓴다.
#[derive(Clone)]
pub struct MidiClient {
    // 서버가 다시 떠 클라이언트를 새로 만들 때 같은 이름을 쓴다.
    name: Arc<str>,
}

impl MidiClient {
    /// 클라이언트 핸들을 돌려준다.
    ///
    /// CoreMIDI 클라이언트 자체는 프로세스에 하나이고 해제하지 않는다(`PROCESS_CLIENT`). `name`은
    /// 그 클라이언트를 처음 만들 때만 쓰인다. 알림 콜백은 두지 않는다. 핫플러그 알림은 run loop을
    /// 소유하는 S1-T2의 감시 스레드가 받는다.
    ///
    /// # Errors
    ///
    /// 이름이 `normalized_name` 규칙에 맞지 않으면 `InvalidName`, CoreMIDI 호출이 실패하면 `Os`.
    pub fn new(name: &str) -> Result<Self, MidiError> {
        let name = &normalized_name(name)?;
        process_client(name)?;
        Ok(MidiClient {
            name: Arc::from(name.as_str()),
        })
    }

    /// 프로세스 클라이언트 핸들. 서버가 사라져 캐시가 비었으면 같은 이름으로 다시 만든다.
    pub(crate) fn raw(&self) -> Result<MIDIClientRef, MidiError> {
        process_client(&self.name)
    }

    /// MIDI 1.0 프로토콜 가상 소스(앱의 출력 포트)를 만든다(OUT-1, OUT-2).
    ///
    /// # Errors
    ///
    /// 이름이 `normalized_name` 규칙에 맞지 않으면 `InvalidName`, 프로세스에 같은 이름의 가상 포트가
    /// 있으면 `DuplicateName`,
    /// 이미 16개면 `TooManyPorts`, CoreMIDI 호출이 실패하면 `Os`.
    #[allow(unsafe_code)]
    pub fn create_virtual_output(&self, name: &str) -> Result<VirtualOutput, MidiError> {
        let name = &normalized_name(name)?;
        // 검사와 등록 사이에 다른 스레드가 같은 이름을 넣지 못하게 끝까지 잠근다.
        let mut names = port_names();
        let key = duplicate_key(name);
        if names.contains(&key) {
            return Err(MidiError::DuplicateName(name.to_owned()));
        }
        if names.len() >= MAX_VIRTUAL_OUTPUTS {
            return Err(MidiError::TooManyPorts);
        }
        let raw = self.raw()?;
        let cf_name = CFString::new(name);
        let mut endpoint: MIDIEndpointRef = 0;
        // SAFETY: 프로세스 클라이언트는 해제되지 않아 유효하고, 이름 문자열은 호출 동안 살아 있으며,
        // 출력 포인터는 이 스택 변수를 가리킨다.
        let status = unsafe {
            MIDISourceCreateWithProtocol(
                raw,
                cf_name.as_concrete_TypeRef(),
                PROTOCOL_1_0,
                &raw mut endpoint,
            )
        };
        forget_client_if_server_gone(raw, status);
        check("MIDISourceCreateWithProtocol", status)?;
        names.push(key);
        Ok(VirtualOutput {
            endpoint,
            name: name.to_owned(),
        })
    }
}

/// 가상 출력 포트. drop되면 CoreMIDI 쪽에서도 해제된다.
pub struct VirtualOutput {
    endpoint: MIDIEndpointRef,
    name: String,
}

impl VirtualOutput {
    /// 포트 이름을 바꾼다. 프로세스의 다른 가상 포트 이름과 겹치면 안 된다.
    ///
    /// # Errors
    ///
    /// `InvalidName`, `DuplicateName`, `Os`. 실패하면 이름은 그대로다.
    #[allow(unsafe_code)]
    pub fn rename(&mut self, name: &str) -> Result<(), MidiError> {
        let name = &normalized_name(name)?;
        if *name == self.name {
            return Ok(());
        }
        let mut names = port_names();
        let (key, own) = (duplicate_key(name), duplicate_key(&self.name));
        if key != own && names.contains(&key) {
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
        if let Some(slot) = names.iter_mut().find(|k| **k == own) {
            *slot = key;
        }
        name.clone_into(&mut self.name);
        Ok(())
    }

    /// CoreMIDI 서버에 이 포트가 아직 등록돼 있는가. MIDI 서버가 다시 뜨면 이전 서버의 엔드포인트는 사라지고
    /// 이 값은 거짓이 된다. 제어 스레드에서 부른다(조회 호출이 든다).
    #[must_use]
    pub fn is_registered(&self) -> bool {
        endpoint_name(self.endpoint).is_some()
    }

    /// 같은 이름으로 엔드포인트를 다시 만든다. MIDI 서버가 다시 뜬 뒤(`is_registered`가 거짓) 앱이
    /// 부른다(S1-T2 리뷰 1 지적 7). 포트 목록의 자리(이름·상한)는 그대로 쓴다. 새 엔드포인트를 만든 뒤
    /// 옛 것을 해제하므로, 실패하면 옛 엔드포인트가 그대로 남는다.
    ///
    /// # Errors
    ///
    /// CoreMIDI 호출이 실패하면 `Os`.
    #[allow(unsafe_code)]
    pub fn reregister(&mut self, client: &MidiClient) -> Result<(), MidiError> {
        let raw = client.raw()?;
        let cf_name = CFString::new(&self.name);
        let mut endpoint: MIDIEndpointRef = 0;
        // SAFETY: 클라이언트는 캐시에서 얻은 유효한 참조이고, 이름 문자열은 호출 동안 살아 있으며, 출력
        // 포인터는 이 스택 변수를 가리킨다.
        let status = unsafe {
            MIDISourceCreateWithProtocol(
                raw,
                cf_name.as_concrete_TypeRef(),
                PROTOCOL_1_0,
                &raw mut endpoint,
            )
        };
        forget_client_if_server_gone(raw, status);
        check("MIDISourceCreateWithProtocol", status)?;
        let old = std::mem::replace(&mut self.endpoint, endpoint);
        // SAFETY: 이 값이 소유하던 옛 엔드포인트이고 한 번만 해제한다. 서버가 바뀌었으면 이미 사라져
        // 오류가 오지만 할 일이 없다.
        let _ = unsafe { MIDIEndpointDispose(old) };
        Ok(())
    }
}

impl Drop for VirtualOutput {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let mut names = port_names();
        let own = duplicate_key(&self.name);
        if let Some(i) = names.iter().position(|k| *k == own) {
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
        let status = with_event_list(at, ump, |list| {
            // SAFETY: 리스트는 `with_event_list`가 만든 유효한 스택 버퍼이고, 엔드포인트는 이 값이
            // 소유해 유효하다.
            unsafe { MIDIReceivedEventList(self.endpoint, list) }
        })?;
        check("MIDIReceivedEventList", status)
    }
}

/// UMP 메시지 하나를 검사해 스택의 `MIDIEventList`에 담고 `deliver`에 넘긴다. `deliver`의 상태 값을
/// 돌려준다. 가상 포트와 OS 포트의 송신이 같이 쓴다.
///
/// 실시간 스레드에서 불리므로 힙을 쓰지 않는다. `MIDIEventList`는 패킷 하나와 64워드를 담는 고정 크기
/// 구조체라 메시지 하나(최대 4워드)에 충분하다.
#[allow(unsafe_code)]
pub(crate) fn with_event_list(
    at: HostTime,
    ump: &[u32],
    deliver: impl FnOnce(*const MIDIEventList) -> i32,
) -> Result<i32, MidiError> {
    if at.ticks() == 0 {
        return Err(MidiError::ZeroTimestamp);
    }
    let Some(&first) = ump.first() else {
        return Err(MidiError::InvalidMessage);
    };
    if ump.len() != ump_word_count(first) {
        return Err(MidiError::InvalidMessage);
    }
    let mut list = MaybeUninit::<MIDIEventList>::uninit();
    let list_ptr = list.as_mut_ptr();
    // 워드 수는 1–4다.
    let words = ump.len() as ByteCount;
    // SAFETY: `list_ptr`는 이 스택 버퍼를 가리키고, Init이 헤더를 채운 뒤 Add가 그 안에만 쓴다
    // (버퍼 크기를 정확히 넘긴다). 워드 포인터는 길이를 검사한 슬라이스다. 버퍼는 `deliver`가 끝날
    // 때까지 살아 있다.
    let packet = unsafe {
        let packet = MIDIEventListInit(list_ptr, PROTOCOL_1_0);
        MIDIEventListAdd(
            list_ptr,
            size_of::<MIDIEventList>() as ByteCount,
            packet,
            at.ticks(),
            words,
            ump.as_ptr(),
        )
    };
    if packet.is_null() {
        return Err(MidiError::InvalidMessage);
    }
    Ok(deliver(list_ptr))
}

/// 열거가 흔들릴 때 다시 시도하는 횟수. 소스가 계속 생기고 사라지면 마지막 결과를 쓴다.
pub(crate) const ENUMERATION_ATTEMPTS: usize = 8;

/// 시스템의 MIDI 소스 이름을 열거 순서대로 돌려준다. 이름을 읽지 못한 소스는 건너뛴다.
///
/// CoreMIDI 열거는 인덱스 기반이라, 열거 중 앞쪽 소스가 사라지면 인덱스가 당겨져 살아 있는 소스를
/// 건너뛴다(리뷰 1 지적 2). 그래서 엔드포인트 목록을 읽고 이름을 모은 뒤 목록이 그대로인지 다시
/// 확인하고, 바뀌었으면 처음부터 다시 읽는다.
/// 8번 다시 읽어도 목록이 계속 바뀌면 마지막 결과를 돌려준다. 이 목록은 한 시점의 스냅숏이 아니라
/// 최선의 근사이므로, 소멸 판정은 S1-T2의 CoreMIDI 알림과 함께 쓴다(리뷰 2 제안 4).
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
pub(crate) fn endpoint_name(endpoint: MIDIEndpointRef) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::normalized_name;
    use crate::MidiError;

    #[test]
    fn normalized_name_composes_nfd_and_keeps_joiners_in_real_text() {
        // NFD 「드럼」은 NFC 「드럼」으로 저장된다.
        assert_eq!(
            normalized_name("\u{1103}\u{1173}\u{1105}\u{1165}\u{11B7}").unwrap(),
            "\u{B4DC}\u{B7FC}"
        );
        // ZWJ 이모지 가족, ZWNJ가 든 페르시아어, 이체자 선택자는 정상 텍스트다.
        for name in [
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
            "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\u{0627}\u{0647}\u{0645}",
            "\u{2764}\u{FE0F} Out",
        ] {
            assert!(normalized_name(name).is_ok(), "{}", name.escape_unicode());
        }
    }

    #[test]
    fn normalized_name_collapses_inner_whitespace_and_key_ignores_joiners() {
        assert_eq!(
            normalized_name("Lead\u{00A0}\u{00A0} Synth").unwrap(),
            "Lead Synth"
        );
        assert_eq!(
            super::duplicate_key("Pad\u{200D}Synth\u{FE0F}"),
            super::duplicate_key("PadSynth")
        );
    }

    #[test]
    fn duplicate_key_keeps_separated_letters_and_inner_blanks_distinct() {
        use super::duplicate_key;
        // 결합자가 갈라 놓은 글자는 합쳐지지 않는다(따로 그려진다).
        assert_ne!(
            duplicate_key("Cafe\u{200D}\u{301}"),
            duplicate_key("Caf\u{E9}")
        );
        assert_ne!(
            duplicate_key("\u{1100}\u{200D}\u{1161}"),
            duplicate_key("\u{AC00}")
        );
        // 이체자 선택자는 결합 문자를 떼어 놓지 않는다.
        assert_eq!(
            duplicate_key("Cafe\u{FE00}\u{301}"),
            duplicate_key("Caf\u{E9}")
        );
        // 안쪽 빈칸 모양 문자는 폭이 있고, 끝의 것은 보이지 않는다.
        assert_ne!(
            duplicate_key("Lead\u{2800}Synth"),
            duplicate_key("LeadSynth")
        );
        assert_eq!(
            duplicate_key("Lead Synth\u{2800}"),
            duplicate_key("Lead Synth")
        );
    }

    #[test]
    fn normalized_name_rejects_separators_bidi_controls_and_blank_only_names() {
        for name in [
            "a\u{2028}b",
            "a\u{2029}b",
            "a\u{202E}b",
            "a\u{2067}b",
            "\u{200D}",
            "\u{2800}",
            "\u{3164}",
            "\u{FE0F}",
            "\u{1D159}",
            "\u{FFFA}",
        ] {
            assert_eq!(
                normalized_name(name),
                Err(MidiError::InvalidName),
                "{}",
                name.escape_unicode()
            );
        }
    }

    #[test]
    fn process_client_retry_does_not_hold_lock_while_sleeping() {
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

        let slot = Arc::new(Mutex::new(None));
        let attempts = Arc::new(Mutex::new(0u32));
        let worker = {
            let (slot, attempts) = (Arc::clone(&slot), Arc::clone(&attempts));
            std::thread::spawn(move || {
                super::create_with_retry(&slot, Duration::from_millis(600), || {
                    *attempts.lock().unwrap() += 1;
                    Err(-304)
                })
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        // 재시도 사이에는 잠금이 풀려 있어야 한다.
        let start = Instant::now();
        drop(slot.lock().unwrap());
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "{:?}",
            start.elapsed()
        );
        assert_eq!(
            worker.join().unwrap(),
            Err(MidiError::Os {
                call: "MIDIClientCreate",
                status: -304
            })
        );
        assert!(*attempts.lock().unwrap() >= 2);
        // 성공하면 저장하고, 다음 호출은 만들지 않고 돌려준다.
        let mut tries = 0;
        let got = super::create_with_retry(&slot, Duration::from_secs(1), || {
            tries += 1;
            if tries < 3 { Err(-10839) } else { Ok(42) }
        });
        assert_eq!(got, Ok(42));
        assert_eq!(
            super::create_with_retry(&slot, Duration::ZERO, || panic!("다시 만들었다")),
            Ok(42)
        );
    }

    #[test]
    fn server_gone_status_clears_cached_client() {
        use std::sync::Mutex;

        for status in [-10830, -10839, -308] {
            assert!(super::is_server_gone(status), "{status}");
        }
        for status in [0, -10842, -10838, -50, -300, -304, -309] {
            assert!(!super::is_server_gone(status), "{status}");
        }
        // 서버 소멸이면 캐시를 비운다.
        let slot = Mutex::new(Some(7));
        super::forget_in(&slot, 7, -10839);
        assert_eq!(*slot.lock().unwrap(), None);
        // 다른 오류는 그대로 둔다.
        let slot = Mutex::new(Some(7));
        super::forget_in(&slot, 7, -304);
        assert_eq!(*slot.lock().unwrap(), Some(7));
        // 그사이 새로 만든 클라이언트는 건드리지 않는다.
        let slot = Mutex::new(Some(8));
        super::forget_in(&slot, 7, -10839);
        assert_eq!(*slot.lock().unwrap(), Some(8));
    }
}
