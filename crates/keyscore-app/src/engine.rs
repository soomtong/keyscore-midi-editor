//! MIDI 엔진 연결(S2-T5a): 앱 전역 `CoreMIDI` 클라이언트, 기본 가상 포트, 스케줄러.
//!
//! 바이너리는 창을 열기 전에 [`install`]을 부른다. 프로세스의 첫 `CoreMIDI` 호출이 `MidiClient::new`여야
//! 포트 알림이 온다(S1-T2). [`crate::init`]은 `CoreMIDI`를 건드리지 않아, MIDI가 필요 없는 테스트는 MIDI
//! 서버와 무관하게 돈다. 설치하지 않았거나 실패하면 트랜스포트 Action은 메시지만 남긴다.
//!
//! 앱은 스케줄러의 제어 쪽 API만 부른다. 실시간 스레드는 S1 그대로다.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::{App, Global, WeakEntity};
use keyscore_core::{OutputPort, Project, TempoMap, Tick};
use keyscore_midi::{
    DEFAULT_PORT_NAME, MAX_SCHEDULER_PORTS, Midi1Message, MidiClient, MidiError, MidiPort,
    OsOutput, PortInfo, ScheduledEvent, Scheduler, SchedulerConfig, Snapshot, VirtualOutput,
    bank_select, destinations, program_change, render,
};

use crate::app_state::AppSettings;
use crate::window::ProjectWindow;

/// 엔진 설치 옵션. 테스트는 포트 이름을 고유하게 준다.
#[derive(Clone, Debug)]
pub struct EngineOptions {
    /// `CoreMIDI` 클라이언트 이름
    pub client_name: String,
    /// 기본 가상 출력 포트 이름(OUT-1)
    pub port_name: String,
}

impl Default for EngineOptions {
    fn default() -> Self {
        EngineOptions {
            client_name: "KeyScore".to_owned(),
            port_name: DEFAULT_PORT_NAME.to_owned(),
        }
    }
}

/// 창 하나의 트랜스포트 상태.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
    /// 일시정지. 다음 재생은 이 틱에서 이어진다.
    Paused(Tick),
}

/// 앱 전역 엔진.
pub(crate) struct Engine {
    client: MidiClient,
    /// 기본 가상 포트. 스냅샷의 포트 0이다. MIDI 서버가 다시 뜨면 다시 등록하므로 구체 타입으로 갖는다.
    main: Arc<VirtualOutput>,
    /// 연 OS 목적지(이름별). 재생 시작 준비 때 그 프로젝트가 쓰지 않는 것은 놓고, 다시 쓰기 전에
    /// `is_present`로 확인한다(S2-T5d). 스냅샷이 들고 있는 동안은 살아 있다.
    opened: HashMap<String, (PortInfo, Arc<OsOutput>)>,
    scheduler: Scheduler,
    /// 추가 가상 포트(OUT-2). 트랙은 이름으로 가리킨다. 스냅샷이 쓰는 동안은 그 스냅샷도 쥔다.
    extras: Vec<Arc<VirtualOutput>>,
    /// 추가 포트 가운데 대체 매핑(OUT-8)으로 만든 것의 이름(설정 `mapped_ports`와 같다). 같은 이름의
    /// 목적지가 있으면 그 목적지가 이긴다(결정 `2026-10-22-out8-port-mapping-sheet.md` 3 A).
    mapped: Vec<String>,
    /// 지웠지만 스냅샷이 아직 쥐고 있는 추가 포트. 스냅샷이 놓으면 버려 CoreMIDI 소스가 사라진다.
    retired: Vec<Arc<VirtualOutput>>,
    /// 스케줄러에 켜 둔 클럭 포트(이름, 번호). `None`이면 모른다(처음, 또는 지난 명령 묶음이 실패했다).
    clock_state: Option<Vec<(String, u16)>>,
    /// 마지막으로 보낸 테스트 노트(시각, 포트 이름, PRE-2·PRE-3)
    last_test: Option<(std::time::SystemTime, String)>,
    /// 지금 재생 중인 창. 한 번에 한 창만 재생한다.
    owner: Option<WeakEntity<ProjectWindow>>,
    /// 지난 재생 시작 때 찾지 못한 이름 포트. 재생 중 편집(스냅샷 교체)마다 메인 스레드에서 목적지를
    /// 다시 열거하지 않도록 다음 재생 시작까지 기억한다(S2-T5a 리뷰 1 제안 3).
    known_missing: HashSet<String>,
}

/// (목적지, 채널) 하나의 실효 음색(뱅크·프로그램). 재생 시작의 chase와 같은 규칙으로 정한다: 들리지 않는
/// 트랙을 먼저, 들리는 트랙은 그 뒤 트랙 순서로 보고 뱅크와 프로그램은 각각 마지막 값을 쓴다. 재생 중에는
/// 실효 음색이 바뀐 채널만 다시 보낸다. 뮤트·솔로를 바꾸거나 채널을 나눠 쓰는 트랙을 지워도 지금 들리는
/// 음색이 맞는다(S2-T5a 리뷰 2–4, S2-T5b). 스냅샷의 포트 번호는 스냅샷마다 다시 매겨지므로 비교에 쓰지
/// 않는다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Setup {
    /// 목적지(`OutputPort`의 이름, 가상은 빈 문자열)
    pub(crate) destination: String,
    pub(crate) channel: u8,
    /// 이 스냅샷의 포트 번호(보낼 때만 쓴다)
    pub(crate) port: u16,
    /// 뱅크 MSB·LSB(있으면)와 프로그램(있으면). 뱅크만 바뀌어도 프로그램을 다시 보내야 음색이 바뀌므로
    /// 한 묶음으로 보낸다.
    pub(crate) messages: Vec<Midi1Message>,
}

impl Setup {
    /// 같은 목적지·채널의 같은 음색인가(포트 번호는 보지 않는다).
    pub(crate) fn same_as(&self, other: &Setup) -> bool {
        self.destination == other.destination
            && self.channel == other.channel
            && self.messages == other.messages
    }
}

/// 트랙이 소리 내지 못하는 이유.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PortProblem {
    /// 그 이름의 목적지가 없다
    NotFound,
    /// 목적지는 있는데 열지 못했다
    OpenFailed(String),
    /// 한 스냅샷의 포트 상한(`MAX_SCHEDULER_PORTS`)을 넘었다
    TooMany,
}

impl Global for Engine {}

/// 재생할 준비물.
pub(crate) struct Prepared {
    pub(crate) snapshot: Arc<Snapshot>,
    /// 트랙들의 뱅크·프로그램(OUT-4). 재생 시작 위치의 값은 스케줄러가 따라잡아 보내므로(chase) 재생
    /// 시작 때는 쓰지 않는다. 재생 중 편집으로 바뀐 것만 그 자리에서 보낸다.
    pub(crate) setup: Vec<Setup>,
    /// 소리 내지 못하는 이름 포트와 이유.
    pub(crate) missing: Vec<(String, PortProblem)>,
    /// 목적지(`OutputPort`의 이름, 가상은 빈 문자열) → 이 스냅샷의 포트 번호. 테스트 노트처럼 스냅샷
    /// 밖에서 보내는 메시지의 포트를 찾는다.
    pub(crate) routes: Vec<(String, u16)>,
    /// 메트로놈 포트의 이 스냅샷 번호. 찾지 못하면 `None`이고 그 이유는 `metronome_problem`이다.
    pub(crate) metronome: Option<u16>,
    pub(crate) metronome_problem: Option<(String, PortProblem)>,
    /// 클럭을 켤 포트(OUT-5): (포트 이름, 이 스냅샷 번호). 기본 포트의 이름은 빈 문자열이다.
    pub(crate) clock: Vec<(String, u16)>,
    /// 찾지 못한 클럭 포트와 이유.
    pub(crate) clock_missing: Vec<(String, PortProblem)>,
}

/// `prepare`가 트랙 밖 포트를 열 때 함께 넘기는 상태.
struct ExtraPorts<'a, 'b> {
    main_name: &'b str,
    ports: &'b mut Vec<Arc<dyn MidiPort>>,
    index: &'b mut HashMap<&'a str, u16>,
    missing: &'b [(String, PortProblem)],
    live: bool,
    available: &'b mut Option<Vec<PortInfo>>,
}

/// 클럭 포트의 이름 열쇠(`Virtual`은 빈 문자열, 이름 포트는 그 이름). 이름으로 가리킨 기본 포트는 그 이름을
/// 쓰지만 번호는 0이라 클럭 명령 계산에는 차이가 없다.
fn port_key(port: &OutputPort) -> String {
    match port {
        OutputPort::Virtual => String::new(),
        OutputPort::Named(name) => name.clone(),
    }
}

/// 가상 포트를 추가·이름 변경·삭제하지 못한 이유.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PortEditError {
    /// MIDI 엔진이 없다
    #[error("MIDI 엔진이 없다")]
    NoEngine,
    /// 그 이름의 앱 가상 포트가 없다
    #[error("가상 포트 「{0}」이 없다")]
    NotFound(String),
    /// 기본 포트는 지울 수 없다
    #[error("기본 가상 포트는 지울 수 없다")]
    Main,
    /// 지금 보이는 CoreMIDI 목적지와 이름이 같다
    #[error("「{0}」은 이미 있는 MIDI 목적지 이름이다")]
    DestinationName(String),
    /// keyscore-midi가 거부했다(이름 규칙, 중복, 16개 상한)
    #[error("{0}")]
    Midi(#[from] MidiError),
}

/// 앱의 가상 포트 이름. 기본 포트가 먼저다. 엔진이 없으면 설정의 이름이다.
#[must_use]
pub fn virtual_port_names(cx: &App) -> Vec<String> {
    if cx.has_global::<Engine>() {
        let engine = cx.global::<Engine>();
        std::iter::once(engine.main.name())
            .chain(engine.extras.iter().map(|p| p.name()))
            .collect()
    } else if cx.has_global::<AppSettings>() {
        let m = &AppSettings::get(cx).midi_output;
        std::iter::once(m.main_or_default())
            .chain(m.virtual_ports.iter().cloned())
            .collect()
    } else {
        vec![DEFAULT_PORT_NAME.to_owned()]
    }
}

/// 이 이름이 앱의 가상 포트인가(기본 포트 포함). 포트 감시·상태줄은 앱 포트를 「없는 목적지」로 보지 않는다.
#[must_use]
pub fn is_own_port(cx: &App, name: &str) -> bool {
    virtual_port_names(cx).iter().any(|n| n == name)
}

/// 포트 메뉴 선택지: 기본 가상 포트, 추가 가상 포트(이름), 지금 보이는 CoreMIDI 목적지(이름). 앱 포트와
/// 이름이 같은 목적지는 빼고(앱 포트가 이긴다) 같은 이름은 한 번만 넣는다.
#[must_use]
pub fn port_choices(cx: &App) -> Vec<OutputPort> {
    let own = virtual_port_names(cx);
    let mut choices = vec![OutputPort::Virtual];
    choices.extend(own.iter().skip(1).cloned().map(OutputPort::Named));
    for d in destinations() {
        let port = OutputPort::Named(d.name().to_owned());
        if !own
            .iter()
            .any(|n| keyscore_midi::same_port_name(n, d.name()))
            && !choices.contains(&port)
        {
            choices.push(port);
        }
    }
    choices
}

/// 새 이름이 쓸 수 있는가: 규칙에 맞고, 지금 보이는 목적지 이름과 보이는 모양이 다르다. 다른 가상 포트와의
/// 중복은 keyscore-midi가 본다. 고친 이름을 돌려준다.
fn check_new_name(name: &str) -> Result<String, PortEditError> {
    let name = keyscore_midi::normalize_port_name(name)?;
    if destinations()
        .iter()
        .any(|d| keyscore_midi::same_port_name(d.name(), &name))
    {
        return Err(PortEditError::DestinationName(name));
    }
    Ok(name)
}

/// 가상 포트를 하나 더 만든다(OUT-2). 이름은 「기본 포트 이름 N」 가운데 빈 것이다. 합쳐 16개를 넘으면
/// keyscore-midi가 거부한다.
///
/// # Errors
///
/// [`PortEditError`].
pub fn add_virtual_port(cx: &mut App) -> Result<String, PortEditError> {
    if !cx.has_global::<Engine>() {
        return Err(PortEditError::NoEngine);
    }
    let names = virtual_port_names(cx);
    if names.len() >= keyscore_midi::MAX_VIRTUAL_OUTPUTS {
        return Err(MidiError::TooManyPorts.into());
    }
    let base = names[0].clone();
    let taken = |candidate: &str| {
        names
            .iter()
            .any(|n| keyscore_midi::same_port_name(n, candidate))
            || destinations()
                .iter()
                .any(|d| keyscore_midi::same_port_name(d.name(), candidate))
    };
    // 앱 포트는 16개 이하라 목적지가 아주 많지 않으면 앞 번호에서 빈 이름이 나온다.
    let name = (2..=u16::MAX)
        .map(|n| format!("{base} {n}"))
        .find(|c| !taken(c))
        .ok_or(MidiError::TooManyPorts)?;
    let engine = cx.global_mut::<Engine>();
    let port = engine.client.create_virtual_output(&name)?;
    let name = port.name();
    engine.extras.push(Arc::new(port));
    let added = name.clone();
    AppSettings::update(cx, |s| s.midi_output.virtual_ports.push(added));
    ports_edited(cx, &name, true);
    Ok(name)
}

/// 주어진 이름으로 추가 가상 포트를 만들고 설정에 저장한다(OUT-8 대체 매핑). 검사는 [`add_virtual_port`]와
/// 같다: 이름 규칙, 보이는 목적지와 같은 모양 금지, 다른 가상 포트와 같은 모양 금지, 합쳐 16개. 고친 이름을
/// 돌려준다.
///
/// # Errors
///
/// [`PortEditError`].
pub fn add_named_virtual_port(cx: &mut App, name: &str) -> Result<String, PortEditError> {
    if !cx.has_global::<Engine>() {
        return Err(PortEditError::NoEngine);
    }
    let name = check_new_name(name)?;
    let engine = cx.global_mut::<Engine>();
    // 다른 가상 포트와의 같은 모양·16개 상한은 keyscore-midi가 거부한다
    let port = engine.client.create_virtual_output(&name)?;
    let name = port.name();
    engine.extras.push(Arc::new(port));
    let added = name.clone();
    AppSettings::update(cx, |s| s.midi_output.virtual_ports.push(added));
    ports_edited(cx, &name, true);
    Ok(name)
}

/// 없는 포트의 대체 매핑(OUT-8)으로 가상 포트를 만들고 표시한다. 같은 이름의 장치가 나타나면 지운다.
///
/// # Errors
///
/// [`PortEditError`].
pub(crate) fn add_mapped_virtual_port(cx: &mut App, name: &str) -> Result<String, PortEditError> {
    let made = add_named_virtual_port(cx, name)?;
    cx.global_mut::<Engine>().mapped.push(made.clone());
    let marked = made.clone();
    AppSettings::update(cx, |s| s.midi_output.mapped_ports.push(marked));
    Ok(made)
}

/// 대체 매핑으로 만든 포트 가운데 이 목적지와 이름이 같은(보이는 모양, 결정 2026-10-15-port-name-visual-duplicates)
/// 것이 있으면 지우고 알린다(포트 감시가 목적지가 나타날 때 부른다). 포트 이름이 장치 이름과 글자로 다르면(시트가
/// 정규화한 이름) 열린 창의 트랙을 장치 이름으로 되돌린다(창마다 undo 한 단위, 가상 포트 이름 변경과 같은 방식,
/// 리뷰 2 지적 2). 닫힌 파일은 열 때 시트가 장치로 바꾸자고 제안한다.
pub(crate) fn release_mapped(cx: &mut App, destination: &str) {
    if !cx.has_global::<Engine>() {
        return;
    }
    let Some(mapped) = cx
        .global::<Engine>()
        .mapped
        .iter()
        .find(|m| keyscore_midi::same_port_name(m, destination))
        .cloned()
    else {
        return;
    };
    // 장치가 없을 때 재생을 준비하며 남긴 「없음」 기억을 지운다. 지우지 않으면 재생 중 다시 준비가 돌아온
    // 장치를 찾지 않는다
    cx.global_mut::<Engine>().port_returned(destination);
    // 재생 중인 창의 음색 기록에서 두 이름을 잊는다. 대체 포트에 보낸 뱅크·프로그램이 돌아온 장치에 보낸 것으로
    // 남으면 다시 보내지 않는다(일반 재연결 `transport_port_returned`와 같다, 리뷰 2 지적 1)
    if let Some(owner) = cx.global::<Engine>().owner() {
        owner.update(cx, |w, _| {
            w.transport_forget_setup(&mapped);
            w.transport_forget_setup(destination);
        });
    }
    if mapped != destination {
        let device = OutputPort::Named(destination.to_owned());
        for window in crate::window::project_windows(cx) {
            window.update(cx, |w, cx| {
                let edits: Vec<keyscore_core::Edit> = w
                    .document()
                    .read(cx)
                    .project()
                    .tracks()
                    .iter()
                    .filter(|t| matches!(t.port(), OutputPort::Named(n) if *n == mapped))
                    .map(|t| keyscore_core::Edit::SetTrackProp {
                        track: t.id(),
                        prop: keyscore_core::TrackProp::Port(device.clone()),
                    })
                    .collect();
                if !edits.is_empty() {
                    w.edit_document(edits, cx);
                }
            });
        }
    }
    match remove_virtual_port(cx, &mapped) {
        Ok(()) => crate::window::say_everywhere(
            cx,
            &format!("장치 「{destination}」이 돌아와 대체로 만든 가상 포트 「{mapped}」을 지웠다"),
        ),
        Err(e) => crate::window::say_everywhere(
            cx,
            &format!(
                "장치 「{destination}」이 돌아왔지만 대체 가상 포트 「{mapped}」을 지우지 못했다: {e}"
            ),
        ),
    }
}

/// 앱 가상 포트의 이름을 바꾸고 고친 새 이름을 돌려준다(엔드포인트는 그대로라 외부 신디 연결이 유지된다). 열린 창의 트랙과 메트로놈
/// 포트도 새 이름으로 고친다(결정 2026-10-19-virtual-port-rename-tracks A, 창마다 undo 한 단위).
///
/// # Errors
///
/// [`PortEditError`].
pub fn rename_virtual_port(cx: &mut App, old: &str, new: &str) -> Result<String, PortEditError> {
    if !cx.has_global::<Engine>() {
        return Err(PortEditError::NoEngine);
    }
    let engine = cx.global::<Engine>();
    let is_main = engine.main.name() == old;
    let port = if is_main {
        engine.main.clone()
    } else {
        engine
            .extras
            .iter()
            .find(|p| p.name() == old)
            .cloned()
            .ok_or_else(|| PortEditError::NotFound(old.to_owned()))?
    };
    let new = check_new_name(new)?;
    if new == old {
        return Ok(new);
    }
    port.set_name(&new)?;
    let new = port.name();
    // 이름을 바꾼 포트는 사용자의 포트다. 대체 매핑 표시를 지운다(같은 이름의 장치를 기다리지 않는다)
    cx.global_mut::<Engine>().mapped.retain(|m| m != old);
    AppSettings::update(cx, |s| {
        s.midi_output.mapped_ports.retain(|m| m != old);
        if is_main {
            s.midi_output.main_name = Some(new.clone());
        } else if let Some(slot) = s.midi_output.virtual_ports.iter_mut().find(|n| *n == old) {
            slot.clone_from(&new);
        }
        let was = OutputPort::Named(old.to_owned());
        if s.metronome.port == was {
            s.metronome.port = OutputPort::Named(new.clone());
        }
        // 클럭 포트 목록도 새 이름을 따른다(OUT-5). 새 이름이 이미 있으면 하나만 남긴다
        if s.midi_output.clock.contains(&was) {
            let renamed = OutputPort::Named(new.clone());
            s.midi_output.clock.retain(|p| *p != was && *p != renamed);
            s.midi_output.clock.push(renamed);
        }
    });
    let renamed = OutputPort::Named(new.clone());
    let owner = cx.global::<Engine>().owner();
    let mut owner_edited = false;
    for window in crate::window::project_windows(cx) {
        let is_owner = owner.as_ref() == Some(&window);
        window.update(cx, |w, cx| {
            // 같은 엔드포인트다. 지금 재생의 음색 기록도 새 이름으로 옮겨, 트랙 편집 뒤 교체가 같은 장치에
            // 뱅크·프로그램을 다시 보내지 않게 한다(S2-T6d 리뷰 2 제안 3)
            w.transport_port_renamed(old, &new);
            let edits: Vec<keyscore_core::Edit> = w
                .document()
                .read(cx)
                .project()
                .tracks()
                .iter()
                .filter(|t| matches!(t.port(), OutputPort::Named(n) if n == old))
                .map(|t| keyscore_core::Edit::SetTrackProp {
                    track: t.id(),
                    prop: keyscore_core::TrackProp::Port(renamed.clone()),
                })
                .collect();
            // 편집이 거부되면 교체도 없다. 그때는 아래에서 재생 창을 따로 다시 준비한다(리뷰 3 제안 3)
            if !edits.is_empty() && w.edit_document(edits, cx) {
                owner_edited |= is_owner;
            }
        });
    }
    // 재생 중인 창의 트랙을 고쳤으면 그 편집이 스냅샷을 다시 만든다. 한 번 더 바꾸지 않는다(리뷰 2 제안 4)
    ports_edited(cx, &new, !owner_edited);
    Ok(new)
}

/// 추가 가상 포트를 지운다. 그 포트를 쓰던 트랙은 이름을 그대로 둔다(「포트 없음」이 된다). 재생 중이면
/// 스냅샷을 다시 만들어 그 포트를 뺀다. 스냅샷이 놓으면 CoreMIDI 소스가 사라진다.
///
/// # Errors
///
/// [`PortEditError`].
pub fn remove_virtual_port(cx: &mut App, name: &str) -> Result<(), PortEditError> {
    if !cx.has_global::<Engine>() {
        return Err(PortEditError::NoEngine);
    }
    let engine = cx.global_mut::<Engine>();
    if engine.main.name() == name {
        return Err(PortEditError::Main);
    }
    let Some(i) = engine.extras.iter().position(|p| p.name() == name) else {
        return Err(PortEditError::NotFound(name.to_owned()));
    };
    let port = engine.extras.remove(i);
    engine.retired.push(port);
    engine.mapped.retain(|m| m != name);
    AppSettings::update(cx, |s| {
        s.midi_output.virtual_ports.retain(|n| n != name);
        s.midi_output.mapped_ports.retain(|n| n != name);
    });
    let owner = cx.global::<Engine>().owner();
    if let Some(owner) = owner {
        // 재생 중: 그 포트 없이 다시 준비한다(트랙은 「포트 없음」으로 소리 내지 않는다)
        owner.update(cx, ProjectWindow::transport_refresh);
    } else {
        // 정지 중: 지금 스냅샷이 그 포트를 쥐고 있을 수 있다. 기본 포트만 든 빈 스냅샷으로 바꾼다
        let engine = cx.global_mut::<Engine>();
        let main: Arc<dyn MidiPort> = engine.main.clone();
        let swapped = Snapshot::new(TempoMap::default(), vec![main], Vec::new())
            .and_then(|snapshot| engine.scheduler.swap(Arc::new(snapshot)));
        if let Err(e) = swapped {
            // 소스는 다음 재생 준비까지 남는다(지운 목록에서는 이미 빠졌다)
            crate::window::say_everywhere(
                cx,
                &format!("지운 가상 포트 「{name}」을 바로 놓지 못했다: {e}"),
            );
        }
    }
    cx.global_mut::<Engine>()
        .release_retired(std::time::Duration::from_millis(200));
    crate::settings_window::ports_changed(cx);
    cx.refresh_windows();
    Ok(())
}

/// 가상 포트가 생기거나 이름이 바뀌었다. 그 이름을 이미 가리키며 기다리던 트랙(「포트 없음」)이 재생 중이면
/// 다시 준비해 바로 잇는다(OUT-3의 자동 재연결과 같은 태도, S2-T6d 리뷰 1 지적 1). 설정 창의 포트 목록도
/// 다시 읽는다.
fn ports_edited(cx: &mut App, name: &str, refresh_owner: bool) {
    cx.global_mut::<Engine>().forget_missing(name);
    if refresh_owner && let Some(owner) = cx.global::<Engine>().owner() {
        owner.update(cx, ProjectWindow::transport_refresh);
    }
    crate::settings_window::ports_changed(cx);
    cx.refresh_windows();
}

/// 클럭 포트 짝(이름, 스냅샷 번호)이 `old`에서 `new`로 바뀔 때 스케줄러에 보낼 (끌 번호, 켤 번호).
/// 짝이 같으면 아무것도 보내지 않는다. `old`가 없으면(처음, 또는 지난 묶음이 실패해 스케줄러 상태를 모른다)
/// 모든 번호를 끄고 새 번호를 켠다.
#[must_use]
pub fn clock_commands(
    old: Option<&[(String, u16)]>,
    new: &[(String, u16)],
) -> (Vec<u16>, Vec<u16>) {
    let mut on: Vec<u16> = new.iter().map(|(_, i)| *i).collect();
    on.sort_unstable();
    on.dedup();
    let Some(old) = old else {
        let all = (0..u16::try_from(MAX_SCHEDULER_PORTS).unwrap_or(u16::MAX)).collect();
        return (all, on);
    };
    let mut was: Vec<(&str, u16)> = old.iter().map(|(n, i)| (n.as_str(), *i)).collect();
    let mut now: Vec<(&str, u16)> = new.iter().map(|(n, i)| (n.as_str(), *i)).collect();
    was.sort_unstable();
    was.dedup();
    now.sort_unstable();
    now.dedup();
    if was == now {
        return (Vec::new(), Vec::new());
    }
    let mut off: Vec<u16> = old.iter().map(|(_, i)| *i).collect();
    off.sort_unstable();
    off.dedup();
    (off, on)
}

/// 앱이 마지막으로 보낸 테스트 노트(시각, 포트 이름). 리스너 연결은 알 수 없어(SP-4) 앱이 아는 것만 보인다.
#[must_use]
pub fn last_test_note(cx: &App) -> Option<(std::time::SystemTime, String)> {
    if cx.has_global::<Engine>() {
        cx.global::<Engine>().last_test.clone()
    } else {
        None
    }
}

/// 앱 가상 포트 하나로 테스트 노트(채널 1, 가운데 C, 벨로시티 100, 미리듣기 길이)를 보낸다(PRE-3, 설정 >
/// MIDI 출력). 정지 중이면 기본 포트와 그 포트를 든 스냅샷을 넣는다(인스펙터의 정지 중 경로와 같다). 다른
/// 창이 재생 중이면 그 재생의 스냅샷을 바꾸지 않는다. 그 재생이 이 포트를 쓸 때만 보낸다.
///
/// # Errors
///
/// 보내지 못한 이유(사람이 읽는 글).
pub fn send_port_test_note(cx: &mut App, name: &str) -> Result<(), String> {
    if !cx.has_global::<Engine>() {
        return Err(
            "MIDI 엔진이 없어 테스트 노트를 보낼 수 없다(CoreMIDI를 열지 못했다)".to_owned(),
        );
    }
    let engine = cx.global::<Engine>();
    let is_main = engine.main.name() == name;
    let extra = engine.extras.iter().find(|p| p.name() == name).cloned();
    if !is_main && extra.is_none() {
        return Err(format!("가상 포트 「{name}」이 없다"));
    }
    let port = if is_main {
        // 기본 포트는 어느 스냅샷에서나 포트 0이다
        0
    } else if let Some(owner) = engine.owner() {
        owner
            .read(cx)
            .transport
            .routes
            .iter()
            .find(|(d, _)| d == name)
            .map(|(_, i)| *i)
            .ok_or_else(|| {
                format!(
                    "다른 창이 재생 중이고 그 재생이 「{name}」을 쓰지 않아 테스트 노트를 보낼 수 없다. 정지한 뒤 다시 한다"
                )
            })?
    } else {
        let engine = cx.global_mut::<Engine>();
        let main: Arc<dyn MidiPort> = engine.main.clone();
        let extra: Arc<dyn MidiPort> = extra.unwrap_or_else(|| engine.main.clone());
        Snapshot::new(TempoMap::default(), vec![main, extra], Vec::new())
            .and_then(|snapshot| engine.scheduler.swap(Arc::new(snapshot)))
            .map_err(|e| format!("테스트 노트를 보내지 못했다: {e}"))?;
        1
    };
    let pitch = keyscore_core::Pitch::new(60).map_err(|e| e.to_string())?;
    let velocity = keyscore_core::Velocity::from_midi1(100).map_err(|e| e.to_string())?;
    let engine = cx.global_mut::<Engine>();
    engine
        .scheduler
        .preview(
            port,
            keyscore_core::Channel::new(1).map_err(|e| e.to_string())?,
            pitch,
            velocity,
            keyscore_midi::DEFAULT_PREVIEW_LENGTH,
        )
        .map_err(|e| format!("테스트 노트를 보내지 못했다: {e}"))?;
    engine.note_test_sent(name.to_owned());
    Ok(())
}

/// 클럭 포트 설정이 바뀌었다(OUT-5). 재생 중이면 스냅샷을 다시 만들어 바로 반영한다.
pub(crate) fn clock_changed(cx: &mut App) {
    if !cx.has_global::<Engine>() {
        return;
    }
    for port in crate::transport::clock_settings(cx) {
        if let OutputPort::Named(name) = port {
            cx.global_mut::<Engine>().forget_missing(&name);
        }
    }
    if let Some(owner) = cx.global::<Engine>().owner() {
        owner.update(cx, ProjectWindow::transport_refresh);
    }
}

/// 메트로놈 설정이 바뀌었다(MET-1). 재생 중인 창이 있으면 스냅샷을 다시 만들어 클릭 포트·채널·노트·길이를
/// 바로 바꾼다.
pub(crate) fn metronome_changed(cx: &mut App) {
    if !cx.has_global::<Engine>() {
        return;
    }
    let port = crate::transport::metronome_settings(cx).port;
    if let OutputPort::Named(name) = &port {
        cx.global_mut::<Engine>().forget_missing(name);
    }
    if let Some(owner) = cx.global::<Engine>().owner() {
        owner.update(cx, ProjectWindow::transport_refresh);
    }
}

/// 엔진을 설치한다. 이미 설치돼 있으면 아무 일도 하지 않는다.
///
/// # Errors
///
/// `CoreMIDI` 클라이언트·가상 포트·스케줄러를 만들지 못하면 그 오류. 앱은 엔진 없이 계속 뜬다.
pub fn install(cx: &mut App, options: &EngineOptions) -> Result<(), MidiError> {
    if is_installed(cx) {
        return Ok(());
    }
    // 프로세스의 첫 CoreMIDI 호출이다(S1-T2 알림 조건).
    let client = MidiClient::new(&options.client_name)?;
    // 설정의 기본 포트 이름이 지금 보이는 목적지와 같으면 그 목적지를 가로챈다(이름 포트는 앱 포트가 먼저).
    // OUT-1의 기본 이름으로 대신 만들고 알린다(S2-T6d 리뷰 2 지적 1). 설정은 그대로 둔다.
    let present = destinations();
    let collides = |name: &str| {
        present
            .iter()
            .any(|d| keyscore_midi::same_port_name(d.name(), name))
    };
    // 설정의 추가 가상 포트(OUT-2). 기본 포트의 대체 이름이 이것과 겹치지 않게 먼저 읽는다.
    let (mut wanted, mut mapped) = if cx.has_global::<AppSettings>() {
        let m = &AppSettings::get(cx).midi_output;
        (m.virtual_ports.clone(), m.mapped_ports.clone())
    } else {
        (Vec::new(), Vec::new())
    };
    // 대체 매핑으로 만든 포트의 장치가 이미 있으면 그 포트는 더 필요 없다. 만들지 않고 설정에서 뺀다(결정
    // 2026-10-22-out8-port-mapping-sheet 3 A)
    // 같은 이름은 보이는 모양으로 본다(시트가 정규화한 이름과 장치의 원래 이름, 리뷰 2 지적 2). 이 포트를 쓰던
    // 닫힌 파일은 열 때 시트가 장치로 바꾸자고 제안한다
    let returned: Vec<String> = mapped.iter().filter(|n| collides(n)).cloned().collect();
    if !returned.is_empty() {
        wanted.retain(|n| !returned.contains(n));
        mapped.retain(|n| !returned.contains(n));
        AppSettings::update(cx, |s| {
            s.midi_output
                .virtual_ports
                .retain(|n| !returned.contains(n));
            s.midi_output.mapped_ports.retain(|n| !returned.contains(n));
        });
        for name in &returned {
            // 첫 창보다 먼저 돌아 받을 창이 없을 수 있다. 로그에도 남긴다(리뷰 2 제안 3)
            let message = format!("장치 「{name}」이 있어 대체로 만든 가상 포트를 지웠다");
            eprintln!("{message}");
            crate::window::say_everywhere(cx, &message);
        }
    }
    let mut main_name = options.port_name.clone();
    if collides(&main_name) {
        // 대체 이름은 목적지와도, 설정의 추가 포트와도 겹치지 않는 첫 이름(「KeyScore Out」, 「KeyScore Out 2」…)
        // 이다. 추가 포트와 겹치면 그 포트를 만들지 못한다(S2-T6d 리뷰 3 제안 2).
        let free =
            |c: &str| !collides(c) && !wanted.iter().any(|w| keyscore_midi::same_port_name(w, c));
        let fallback = std::iter::once(DEFAULT_PORT_NAME.to_owned())
            .chain((2..=u16::MAX).map(|n| format!("{DEFAULT_PORT_NAME} {n}")))
            .find(|c| free(c));
        if let Some(fallback) = fallback {
            let message = format!(
                "기본 가상 포트 이름 「{main_name}」은 이미 있는 MIDI 목적지 이름이라 「{fallback}」으로 만들었다"
            );
            eprintln!("{message}");
            crate::window::say_everywhere(cx, &message);
            main_name = fallback;
        }
    }
    let main = Arc::new(client.create_virtual_output(&main_name)?);
    // 추가 가상 포트를 만든다. 만들지 못한 것은 알리고 설정에는 남긴다(다음 시작 때 다시 해 본다).
    let mut extras = Vec::new();
    for name in wanted {
        // 지금 보이는 목적지와 이름이 같으면 만들지 않는다. 만들면 그 목적지를 쓰던 트랙이 앱 포트로 간다(편집기의
        // 새 이름 검사와 같은 규칙, S2-T6d 리뷰 1 지적 2). 설정에는 남긴다.
        if collides(&name) {
            let message =
                format!("가상 포트 「{name}」은 이미 있는 MIDI 목적지 이름이라 만들지 않았다");
            eprintln!("{message}");
            crate::window::say_everywhere(cx, &message);
            continue;
        }
        match client.create_virtual_output(&name) {
            Ok(port) => extras.push(Arc::new(port)),
            Err(e) => {
                let message = format!("가상 포트 「{name}」을 만들지 못했다: {e}");
                eprintln!("{message}");
                crate::window::say_everywhere(cx, &message);
            }
        }
    }
    // 만들지 못한 포트는 표시하지 않는다(설정에는 남아 다음 시작 때 다시 해 본다). 남기면 사용자가 자기 포트를 그
    // 이름으로 바꿀 때 대체 표시가 붙는다(리뷰 2 제안 4)
    mapped.retain(|n| extras.iter().any(|p| p.name() == *n));
    let empty = Snapshot::new(
        TempoMap::default(),
        vec![main.clone() as Arc<dyn MidiPort>],
        Vec::new(),
    )?;
    let scheduler = Scheduler::start(Arc::new(empty), SchedulerConfig::default())?;
    cx.set_global(Engine {
        client,
        main,
        opened: HashMap::new(),
        scheduler,
        extras,
        mapped,
        retired: Vec::new(),
        clock_state: None,
        last_test: None,
        owner: None,
        known_missing: HashSet::new(),
    });
    Ok(())
}

/// 기본 가상 포트 이름. 엔진이 없으면 기본값(OUT-1 「KeyScore Out」).
#[must_use]
pub fn virtual_port_name(cx: &App) -> String {
    if cx.has_global::<Engine>() {
        cx.global::<Engine>().main.name()
    } else {
        DEFAULT_PORT_NAME.to_owned()
    }
}

/// 엔진이 지금 열어 둔 OS 목적지 이름(테스트·진단용).
#[must_use]
pub fn opened_ports(cx: &App) -> Vec<String> {
    if !cx.has_global::<Engine>() {
        return Vec::new();
    }
    let mut names: Vec<String> = cx.global::<Engine>().opened.keys().cloned().collect();
    names.sort();
    names
}

/// 가상 포트를 다시 등록하지 못한 이유.
#[derive(Debug, thiserror::Error)]
pub enum RecoverError {
    #[error(transparent)]
    Midi(#[from] MidiError),
    #[error("스케줄러가 가상 포트를 놓지 않았다")]
    PortBusy,
}

/// MIDI 서버가 다시 뜬 뒤 기본 가상 포트를 다시 등록한다(OUT-1). 재생 중인 창은 멈추고, 열린 창에
/// 알리고, 포트 감시자를 다시 시작한다.
///
/// # Errors
///
/// 다시 등록하지 못하면 그 오류.
pub fn recover_virtual_port(cx: &mut App) -> Result<(), RecoverError> {
    recover(cx, true)
}

/// `announce_failure`가 거짓이면 실패를 알리지 않는다. 서버가 내려가 있는 동안 포트 감시가 되풀이해 시도해도
/// 같은 실패 메시지가 쌓이지 않게 한다(리뷰 1 지적 3).
pub(crate) fn recover(cx: &mut App, announce_failure: bool) -> Result<(), RecoverError> {
    if !cx.has_global::<Engine>() {
        return Ok(());
    }
    // 재생을 멈춘다. 새 서버에는 옛 엔드포인트가 없어 소리가 나가지 않는다.
    if let Some(owner) = cx.global_mut::<Engine>().take_owner(None) {
        let _ = owner.update(cx, |w, cx| {
            w.transport_stopped_by_server(cx);
        });
    }
    let result = cx.global_mut::<Engine>().reregister_main();
    let name = virtual_port_name(cx);
    match &result {
        Ok(()) => {
            crate::window::say_everywhere(
                cx,
                &format!("MIDI 서버가 다시 시작되어 가상 포트 「{name}」을 다시 등록했다"),
            );
            // 옛 서버의 감시자는 알림을 받지 못한다. 등록에 성공했을 때만 다시 시작한다
            crate::ports::restart_watcher(cx);
        }
        Err(e) if announce_failure => crate::window::say_everywhere(
            cx,
            &format!(
                "MIDI 서버가 다시 시작되었는데 가상 포트 「{name}」을 다시 등록하지 못했다: {e}"
            ),
        ),
        Err(_) => {}
    }
    result
}

/// 엔진이 설치돼 있는가.
#[must_use]
pub fn is_installed(cx: &App) -> bool {
    cx.has_global::<Engine>()
}

impl Engine {
    pub(crate) fn scheduler(&mut self) -> &mut Scheduler {
        &mut self.scheduler
    }

    /// 재생 중인 창을 바꾸고 앞 주인을 돌려준다.
    pub(crate) fn take_owner(
        &mut self,
        owner: Option<WeakEntity<ProjectWindow>>,
    ) -> Option<WeakEntity<ProjectWindow>> {
        std::mem::replace(&mut self.owner, owner)
    }

    /// 이름 포트가 사라졌다(포트 감시). 핸들을 놓고, 재생 중 편집이 다시 찾지 않게 기억한다.
    pub(crate) fn port_lost(&mut self, name: &str) {
        self.opened.remove(name);
        self.known_missing.insert(name.to_owned());
    }

    /// 이름 포트가 돌아왔다(포트 감시). 옛 핸들을 놓고 「없음」 기억을 지워 다음 준비가 새로 연다.
    pub(crate) fn port_returned(&mut self, name: &str) {
        self.opened.remove(name);
        self.known_missing.remove(name);
    }

    /// 스케줄러에서 기본 가상 포트의 다른 참조를 모두 돌려받고 다시 등록한다. 실시간 스레드가 보내는 동안
    /// 엔드포인트를 바꾸지 않도록, 빈 스냅샷을 넣고 옛 스냅샷이 돌아올 때까지 기다린다.
    fn reregister_main(&mut self) -> Result<(), RecoverError> {
        // 서버가 바뀌면 연 OS 포트도 옛 서버의 것이다
        self.opened.clear();
        self.known_missing.clear();
        self.scheduler.stop()?;
        self.scheduler.swap(Arc::new(Snapshot::new(
            TempoMap::default(),
            Vec::new(),
            Vec::new(),
        )?))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let result = loop {
            self.scheduler.collect_garbage();
            // 아직 등록된 포트는 그대로 둔다. 다시 만들면 엔드포인트가 바뀌어 외부 신디의 연결이 끊긴다
            if self.main.is_registered() {
                break Ok(());
            }
            if let Some(main) = Arc::get_mut(&mut self.main) {
                break main.reregister(&self.client).map_err(RecoverError::from);
            }
            if std::time::Instant::now() > deadline {
                break Err(RecoverError::PortBusy);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        // 추가 가상 포트도 다시 등록한다(OUT-2). 하나라도 못 하면 그 오류를 낸다(나머지는 계속한다)
        let mut extra_error = None;
        for port in &mut self.extras {
            if port.is_registered() {
                continue;
            }
            let done = loop {
                self.scheduler.collect_garbage();
                if let Some(port) = Arc::get_mut(port) {
                    break port.reregister(&self.client).map_err(RecoverError::from);
                }
                if std::time::Instant::now() > deadline {
                    break Err(RecoverError::PortBusy);
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            };
            if let Err(e) = done {
                extra_error.get_or_insert(e);
            }
        }
        self.retired.clear();
        let result = result.and(extra_error.map_or(Ok(()), Err));
        // 포트 0이 늘 기본 가상 포트인 스냅샷으로 되돌린다(테스트 노트·미리듣기가 쓴다)
        // 등록 결과를 잃지 않도록 되돌리기 실패는 등록이 성공했을 때만 오류로 낸다(리뷰 1 제안 4)
        let main: Arc<dyn MidiPort> = self.main.clone();
        let back = Snapshot::new(TempoMap::default(), vec![main], Vec::new())
            .and_then(|snapshot| self.scheduler.swap(Arc::new(snapshot)));
        result?;
        back.map_err(RecoverError::from)
    }

    /// 지운 추가 포트 가운데 스냅샷이 놓은 것을 버린다. `wait`까지 스케줄러가 옛 스냅샷을 돌려주기를
    /// 기다린다. 남은 것은 다음 호출(포트 감시의 주기)이 버린다.
    pub(crate) fn release_retired(&mut self, wait: std::time::Duration) {
        let deadline = std::time::Instant::now() + wait;
        loop {
            self.scheduler.collect_garbage();
            self.retired.retain(|p| Arc::strong_count(p) > 1);
            if self.retired.is_empty() || std::time::Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// 기본 가상 포트가 MIDI 서버에 등록돼 있는가. 아니면 서버가 다시 뜬 것으로 보고 복구한다.
    pub(crate) fn main_registered(&self) -> bool {
        self.main.is_registered()
    }

    /// 등록이 풀린 추가 가상 포트를 다시 등록한다(S2-T6d 리뷰 1 제안 3, 리뷰 2 지적 2). 기본 포트는 멀쩡하므로
    /// 재생을 멈추지 않는다. 지금 스냅샷이 쥔 포트는 놓일 때까지(재생을 멈추거나 그 포트를 쓰지 않게 될 때까지)
    /// 미루고 다음 서버 확인에서 다시 본다. 다시 등록한 이름을 돌려준다.
    pub(crate) fn reregister_extras(&mut self) -> Vec<String> {
        // 정지 중이면 지난 재생의 스냅샷이 아직 그 포트를 쥐고 있을 수 있다(정지는 스냅샷을 바꾸지 않는다).
        // 기본 포트만 든 스냅샷으로 바꿔 놓게 한다(S2-T6d 리뷰 3 지적 1). 재생 중이면 그 재생이 끝난 뒤 다음
        // 서버 확인에서 다시 본다.
        let lost = self.extras.iter().any(|p| !p.is_registered());
        if lost && !self.has_owner() {
            let main: Arc<dyn MidiPort> = self.main.clone();
            let swapped = Snapshot::new(TempoMap::default(), vec![main], Vec::new())
                .and_then(|snapshot| self.scheduler.swap(Arc::new(snapshot)));
            // 교체가 들어가지 않았으면(큐가 가득 참) 기다려도 놓이지 않는다. 다음 서버 확인에서 다시 한다
            // (리뷰 4 제안 1)
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(200);
            while swapped.is_ok()
                && std::time::Instant::now() < deadline
                && self
                    .extras
                    .iter()
                    .any(|p| !p.is_registered() && Arc::strong_count(p) > 1)
            {
                self.scheduler.collect_garbage();
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        self.scheduler.collect_garbage();
        let mut done = Vec::new();
        for port in &mut self.extras {
            if port.is_registered() {
                continue;
            }
            if let Some(port) = Arc::get_mut(port)
                && port.reregister(&self.client).is_ok()
            {
                done.push(port.name());
            }
        }
        done
    }

    /// 스케줄러의 클럭 상태를 꺼낸다(교체 묶음이 끝까지 성공하면 `set_clock_state`로 다시 적는다).
    pub(crate) fn take_clock_state(&mut self) -> Option<Vec<(String, u16)>> {
        self.clock_state.take()
    }

    pub(crate) fn set_clock_state(&mut self, state: Vec<(String, u16)>) {
        self.clock_state = Some(state);
    }

    /// 테스트 노트를 보냈다(포트 이름).
    pub(crate) fn note_test_sent(&mut self, port: String) {
        self.last_test = Some((std::time::SystemTime::now(), port));
    }

    /// 지금 재생 중인 창(살아 있으면).
    pub(crate) fn owner(&self) -> Option<gpui_kit::Entity<ProjectWindow>> {
        self.owner.as_ref().and_then(WeakEntity::upgrade)
    }

    /// 그 이름 포트를 「지난 재생 시작 때 못 찾음」에서 뺀다. 재생 중에 새로 고른 포트(메트로놈 포트 등)는
    /// 다음 교체에서 목적지 목록을 다시 본다.
    pub(crate) fn forget_missing(&mut self, name: &str) {
        self.known_missing.remove(name);
    }

    /// 재생 중인 창이 있는가.
    pub(crate) fn has_owner(&self) -> bool {
        self.owner.as_ref().is_some_and(|w| w.upgrade().is_some())
    }

    /// 이 창이 재생 중인 주인인가.
    pub(crate) fn is_owner(&self, window: &WeakEntity<ProjectWindow>) -> bool {
        self.owner.as_ref().is_some_and(|o| o == window)
    }

    /// 프로젝트를 스냅샷으로 바꾼다. 트랙 → 포트 라우팅(SEQ-7, OUT-4).
    ///
    /// `live`는 재생 중 편집의 교체다. 그때는 지난 재생 시작 때 못 찾은 포트를 다시 찾지 않는다.
    /// `metronome`은 메트로놈 포트(MET-1)다. 트랙 포트와 같은 규칙으로 열어 스냅샷에 넣는다.
    ///
    /// # Errors
    ///
    /// 스냅샷을 만들 수 없으면(포트·이벤트 범위) 그 오류.
    #[allow(
        clippy::too_many_lines,
        reason = "포트 열기·음색·렌더를 한 스냅샷 준비로 한곳에 둔다"
    )]
    pub(crate) fn prepare(
        &mut self,
        project: &Project,
        live: bool,
        metronome: &OutputPort,
        clock_ports: &[OutputPort],
    ) -> Result<Prepared, MidiError> {
        if !live {
            self.known_missing.clear();
        }
        if !live {
            // 이 프로젝트가 쓰지 않는 연 포트를 놓는다(S2-T5a 리뷰 1 제안 1). 지금 스냅샷이 들고 있으면 그
            // 스냅샷이 놓을 때 닫힌다.
            let used: HashSet<&str> = project
                .tracks()
                .iter()
                .map(keyscore_core::Track::port)
                .chain(std::iter::once(metronome))
                .chain(clock_ports)
                .filter_map(|p| match p {
                    OutputPort::Named(name) => Some(name.as_str()),
                    OutputPort::Virtual => None,
                })
                .collect();
            self.opened.retain(|name, _| used.contains(name.as_str()));
        }
        let main_name = self.main.name();
        let mut ports: Vec<Arc<dyn MidiPort>> = vec![self.main.clone()];
        let mut index: HashMap<&str, u16> = HashMap::new();
        let mut missing: Vec<(String, PortProblem)> = Vec::new();
        let mut available: Option<Vec<PortInfo>> = None;
        let mut track_ports = Vec::with_capacity(project.tracks().len());
        for track in project.tracks() {
            let port = match track.port() {
                OutputPort::Virtual => Some(0),
                // 기본 포트를 이름으로 가리켜도 포트 0이다(앱 포트가 같은 이름의 목적지보다 먼저)
                OutputPort::Named(name) if *name == main_name => Some(0),
                OutputPort::Named(name) => match index.get(name.as_str()) {
                    Some(&i) => Some(i),
                    None if missing.iter().any(|(n, _)| n == name) => None,
                    None => {
                        let opened = if ports.len() >= MAX_SCHEDULER_PORTS {
                            Err(PortProblem::TooMany)
                        } else {
                            self.open_port(name, live, &mut available)
                        };
                        match opened {
                            Ok(p) => {
                                ports.push(p);
                                let i = u16::try_from(ports.len() - 1).unwrap_or(u16::MAX);
                                index.insert(name, i);
                                Some(i)
                            }
                            Err(problem) => {
                                missing.push((name.clone(), problem));
                                None
                            }
                        }
                    }
                },
            };
            track_ports.push(port);
        }
        // 메트로놈 포트(MET-1)와 클럭 포트(OUT-5): 트랙이 이미 연 포트면 그 번호, 아니면 트랙 포트와 같은 규칙으로
        // 연다(트랙이 쓰지 않아도 스냅샷에 넣는다).
        let mut extra = ExtraPorts {
            main_name: &main_name,
            ports: &mut ports,
            index: &mut index,
            missing: &missing,
            live,
            available: &mut available,
        };
        let (metronome, metronome_problem) = match self.extra_port(metronome, &mut extra) {
            Ok(i) => (Some(i), None),
            Err(problem) => (None, Some(problem)),
        };
        let mut clock = Vec::new();
        let mut clock_missing = Vec::new();
        for port in clock_ports {
            match self.extra_port(port, &mut extra) {
                Ok(i) => clock.push((port_key(port), i)),
                Err(problem) => clock_missing.push(problem),
            }
        }
        let setup = effective_setups(project, &track_ports);
        let mut routes = vec![(String::new(), 0), (main_name.clone(), 0)];
        routes.extend(index.iter().map(|(name, &i)| ((*name).to_owned(), i)));
        // 들리지 않는 트랙(뮤트, 다른 트랙의 솔로)의 뱅크·프로그램을 먼저 틱 0에 넣고, 그 뒤에 들리는 트랙을
        // 트랙 순서대로 렌더한다. 렌더러는 들리지 않는 트랙을 통째로 빼므로, 그대로 두면 재생 시작 때 chase가
        // 그 음색을 보내지 않아 재생 중 뮤트를 풀었을 때 음색이 틀린다(OUT-4, 리뷰 2 지적 3). 같은 틱의
        // 프로그램은 스냅샷 정렬이 넣은 순서를 지키므로, 같은 포트·채널을 나눠 쓰면 들리는 트랙의 음색이
        // 늘 이기고(지금 들리는 소리가 맞아야 한다, 리뷰 3 지적 1·리뷰 4), 들리는 트랙끼리는 뒤 트랙이 이긴다.
        let any_solo = project.tracks().iter().any(keyscore_core::Track::solo);
        let audible = |track: &keyscore_core::Track| {
            if any_solo {
                track.solo()
            } else {
                !track.mute()
            }
        };
        let mut events = Vec::new();
        for (track, port) in project.tracks().iter().zip(&track_ports) {
            let Some(port) = *port else { continue };
            if audible(track) {
                continue;
            }
            let ch = track.channel();
            if let Some(bank) = track.bank() {
                events.extend(
                    bank_select(ch, bank).map(|m| ScheduledEvent::new(Tick::ZERO, port, m)),
                );
            }
            if let Some(program) = track.program() {
                events.push(ScheduledEvent::new(
                    Tick::ZERO,
                    port,
                    program_change(ch, program),
                ));
            }
        }
        let rendered = render(project, |i, _| track_ports.get(i).copied().flatten());
        events.extend(rendered.into_events());
        let snapshot = Snapshot::new(project.tempo_map().clone(), ports, events)?
            .with_meter(project.meter_map().clone());
        Ok(Prepared {
            snapshot: Arc::new(snapshot),
            setup,
            missing,
            routes,
            metronome,
            metronome_problem,
            clock,
            clock_missing,
        })
    }

    /// 트랙 밖에서 쓰는 포트(메트로놈, 클럭)의 스냅샷 번호. 트랙이 연 포트면 그 번호, 트랙이 못 찾은 이름이면
    /// 같은 이유, 아니면 트랙 포트와 같은 규칙으로 연다.
    fn extra_port<'a>(
        &mut self,
        port: &'a OutputPort,
        e: &mut ExtraPorts<'a, '_>,
    ) -> Result<u16, (String, PortProblem)> {
        let name = match port {
            OutputPort::Virtual => return Ok(0),
            OutputPort::Named(name) if name == e.main_name => return Ok(0),
            OutputPort::Named(name) => name,
        };
        if let Some(&i) = e.index.get(name.as_str()) {
            return Ok(i);
        }
        let opened = if let Some((_, problem)) = e.missing.iter().find(|(n, _)| n == name) {
            Err(problem.clone())
        } else if e.ports.len() >= MAX_SCHEDULER_PORTS {
            Err(PortProblem::TooMany)
        } else {
            self.open_port(name, e.live, e.available)
        };
        match opened {
            Ok(p) => {
                e.ports.push(p);
                let i = u16::try_from(e.ports.len() - 1).unwrap_or(u16::MAX);
                e.index.insert(name, i);
                Ok(i)
            }
            Err(problem) => Err((name.clone(), problem)),
        }
    }

    /// 이름 포트: 앱의 추가 가상 포트가 그 이름이면 그것(OUT-2), 아니면 CoreMIDI 목적지를 연다. 앱 포트는
    /// 소스라 목적지 목록에 없다. 같은 이름의 목적지가 따로 있어도 앱 포트가 이긴다.
    fn open_port(
        &mut self,
        name: &str,
        live: bool,
        available: &mut Option<Vec<PortInfo>>,
    ) -> Result<Arc<dyn MidiPort>, PortProblem> {
        if let Some(own) = self.extras.iter().find(|p| p.name() == name).cloned() {
            // 대체 매핑으로 만든 포트는 같은 이름의 장치가 돌아왔으면 비킨다. 포트 감시가 곧 지운다(결정
            // 2026-10-22-out8-port-mapping-sheet 3 A, 리뷰 1 지적 1)
            // 장치 이름은 보이는 모양으로 같으면 된다(시트가 정규화한 이름, 리뷰 2 지적 2)
            if self.mapped.iter().any(|m| m == name) {
                let device = available
                    .get_or_insert_with(destinations)
                    .iter()
                    .map(|d| d.name().to_owned())
                    .find(|d| keyscore_midi::same_port_name(d, name));
                if let Some(device) = device
                    && let Ok(port) = self.open_named(&device, live, available)
                {
                    return Ok(port);
                }
            }
            return Ok(own);
        }
        self.open_named(name, live, available)
    }

    /// 이름 포트를 연다. 한 번 연 포트는 다시 쓴다. 목적지 목록은 한 번만 읽는다.
    fn open_named(
        &mut self,
        name: &str,
        live: bool,
        available: &mut Option<Vec<PortInfo>>,
    ) -> Result<Arc<dyn MidiPort>, PortProblem> {
        // 사라진 목적지로의 송신은 CoreMIDI가 오류 없이 버린다. 다시 쓰기 전에 확인하고, 아니면 놓고 다시
        // 찾는다(S2-T5a 리뷰 1 제안 6).
        if let Some((info, port)) = self.opened.get(name) {
            // 재생 중 편집(`live`)은 목적지 목록을 다시 읽지 않는다. 포트 감시가 사라짐·돌아옴을 엔진에 알린다.
            // 재생 시작 준비는 목록에서 그 이름의 대상(고유 ID가 가장 작은 것)이 아직 이 핸들인지 본다. 이름이
            // 바뀌었거나 더 작은 ID의 같은 이름 포트가 생겼으면 다시 연다(S2-T5d 리뷰 1 지적 1·2).
            let still = if live {
                port.is_present()
            } else {
                port.is_present()
                    && best_match(available.get_or_insert_with(destinations), name) == Some(info)
            };
            if still {
                return Ok(port.clone());
            }
            self.opened.remove(name);
        }
        if live && self.known_missing.contains(name) {
            return Err(PortProblem::NotFound);
        }
        let list = available.get_or_insert_with(destinations);
        let Some(info) = best_match(list, name).cloned() else {
            self.known_missing.insert(name.to_owned());
            return Err(PortProblem::NotFound);
        };
        let output = OsOutput::open(&self.client, &info)
            .map_err(|e| PortProblem::OpenFailed(e.to_string()))?;
        let port = Arc::new(output);
        self.opened.insert(name.to_owned(), (info, port.clone()));
        Ok(port)
    }
}

/// 그 이름의 목적지 가운데 고유 ID가 가장 작은 것. 포트 감시의 `Reconnector`와 같은 규칙이라 엔진이 여는
/// 포트와 감시가 연결됐다고 보는 포트가 같다(S2-T5d 리뷰 1 지적 1).
fn best_match<'a>(list: &'a [PortInfo], name: &str) -> Option<&'a PortInfo> {
    list.iter()
        .filter(|p| p.name() == name)
        .min_by_key(|p| p.unique_id())
}

/// (목적지, 채널) 한 묶음의 실효 음색을 모으는 칸.
struct Slot {
    destination: String,
    channel: u8,
    port: u16,
    bank: Option<keyscore_core::Bank>,
    program: Option<keyscore_core::Program>,
}

/// (목적지, 채널)별 실효 음색. [`Setup`]의 규칙을 따른다.
fn effective_setups(project: &Project, track_ports: &[Option<u16>]) -> Vec<Setup> {
    let any_solo = project.tracks().iter().any(keyscore_core::Track::solo);
    let audible = |t: &keyscore_core::Track| if any_solo { t.solo() } else { !t.mute() };
    // 순서를 지키려고 Vec로 모은다(같은 묶음은 처음 나온 자리).
    let mut groups: Vec<Slot> = Vec::new();
    let tracks = project.tracks().iter().zip(track_ports);
    let ordered = tracks
        .clone()
        .filter(|(t, _)| !audible(t))
        .chain(tracks.filter(|(t, _)| audible(t)));
    for (track, port) in ordered {
        let Some(port) = *port else { continue };
        let destination = match track.port() {
            OutputPort::Virtual => String::new(),
            OutputPort::Named(name) => name.clone(),
        };
        let channel = track.channel().get();
        let index = if let Some(i) = groups
            .iter()
            .position(|g| g.destination == destination && g.channel == channel)
        {
            i
        } else {
            groups.push(Slot {
                destination,
                channel,
                port,
                bank: None,
                program: None,
            });
            groups.len() - 1
        };
        let slot = &mut groups[index];
        if let Some(bank) = track.bank() {
            slot.bank = Some(bank);
        }
        if let Some(program) = track.program() {
            slot.program = Some(program);
        }
    }
    groups
        .into_iter()
        .filter_map(|slot| {
            let ch = keyscore_core::Channel::new(slot.channel).ok()?;
            let mut messages = Vec::new();
            if let Some(bank) = slot.bank {
                messages.extend(bank_select(ch, bank));
            }
            if let Some(program) = slot.program {
                messages.push(program_change(ch, program));
            }
            (!messages.is_empty()).then_some(Setup {
                destination: slot.destination,
                channel: slot.channel,
                port: slot.port,
                messages,
            })
        })
        .collect()
}
