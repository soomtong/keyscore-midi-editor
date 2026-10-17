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
use keyscore_core::{OutputPort, Project, TempoMap, Tick, TrackId};
use keyscore_midi::{
    DEFAULT_PORT_NAME, MAX_SCHEDULER_PORTS, Midi1Message, MidiClient, MidiError, MidiPort,
    OsOutput, PortInfo, ScheduledEvent, Scheduler, SchedulerConfig, Snapshot, bank_select,
    destinations, program_change, render,
};

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
    /// 기본 가상 포트. 스냅샷의 포트 0이다.
    main: Arc<dyn MidiPort>,
    /// 한 번 연 OS 목적지(이름별). 포트가 사라졌을 때의 확인·재연결은 S2-T5c(OUT-3)다.
    opened: HashMap<String, Arc<dyn MidiPort>>,
    scheduler: Scheduler,
    /// 지금 재생 중인 창. 한 번에 한 창만 재생한다.
    owner: Option<WeakEntity<ProjectWindow>>,
    /// 지난 재생 시작 때 찾지 못한 이름 포트. 재생 중 편집(스냅샷 교체)마다 메인 스레드에서 목적지를
    /// 다시 열거하지 않도록 다음 재생 시작까지 기억한다(S2-T5a 리뷰 1 제안 3).
    known_missing: HashSet<String>,
}

/// 트랙 하나의 음색 설정(뱅크·프로그램). 같은 트랙의 옛 설정과 목적지 이름·채널·메시지로 비교한다.
/// 스냅샷의 포트 번호는 스냅샷마다 다시 매겨지므로 비교에 쓰지 않는다(S2-T5a 리뷰 2 지적 5). 다른 트랙의
/// 옛 설정과 비교하면, 같은 포트·채널을 나눠 쓰는 트랙에서 되돌린 프로그램이 걸러진다(리뷰 3 지적 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Setup {
    pub(crate) track: TrackId,
    /// 목적지(`OutputPort`의 이름, 가상은 빈 문자열)
    pub(crate) destination: String,
    pub(crate) channel: u8,
    /// 이 스냅샷의 포트 번호(보낼 때만 쓴다)
    pub(crate) port: u16,
    /// 뱅크 MSB·LSB(있으면)와 프로그램(있으면). 뱅크만 바뀌어도 프로그램을 다시 보내야 음색이 바뀌므로
    /// 한 묶음으로 보낸다(리뷰 2 지적 4).
    pub(crate) messages: Vec<Midi1Message>,
}

impl Setup {
    /// 같은 트랙의 같은 목적지·채널·메시지인가(포트 번호는 보지 않는다).
    pub(crate) fn same_as(&self, other: &Setup) -> bool {
        self.track == other.track
            && self.destination == other.destination
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
    let main: Arc<dyn MidiPort> = Arc::new(client.create_virtual_output(&options.port_name)?);
    let empty = Snapshot::new(TempoMap::default(), vec![main.clone()], Vec::new())?;
    let scheduler = Scheduler::start(Arc::new(empty), SchedulerConfig::default())?;
    cx.set_global(Engine {
        client,
        main,
        opened: HashMap::new(),
        scheduler,
        owner: None,
        known_missing: HashSet::new(),
    });
    Ok(())
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

    /// 이 창이 재생 중인 주인인가.
    pub(crate) fn is_owner(&self, window: &WeakEntity<ProjectWindow>) -> bool {
        self.owner.as_ref().is_some_and(|o| o == window)
    }

    /// 프로젝트를 스냅샷으로 바꾼다. 트랙 → 포트 라우팅(SEQ-7, OUT-4).
    ///
    /// `live`는 재생 중 편집의 교체다. 그때는 지난 재생 시작 때 못 찾은 포트를 다시 찾지 않는다.
    ///
    /// # Errors
    ///
    /// 스냅샷을 만들 수 없으면(포트·이벤트 범위) 그 오류.
    pub(crate) fn prepare(&mut self, project: &Project, live: bool) -> Result<Prepared, MidiError> {
        if !live {
            self.known_missing.clear();
        }
        let mut ports: Vec<Arc<dyn MidiPort>> = vec![self.main.clone()];
        let mut index: HashMap<&str, u16> = HashMap::new();
        let mut missing: Vec<(String, PortProblem)> = Vec::new();
        let mut available: Option<Vec<PortInfo>> = None;
        let mut track_ports = Vec::with_capacity(project.tracks().len());
        for track in project.tracks() {
            let port = match track.port() {
                OutputPort::Virtual => Some(0),
                OutputPort::Named(name) => match index.get(name.as_str()) {
                    Some(&i) => Some(i),
                    None if missing.iter().any(|(n, _)| n == name) => None,
                    None => {
                        let opened = if ports.len() >= MAX_SCHEDULER_PORTS {
                            Err(PortProblem::TooMany)
                        } else {
                            self.open_named(name, live, &mut available)
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
        let mut setup = Vec::new();
        for (track, port) in project.tracks().iter().zip(&track_ports) {
            let Some(port) = *port else { continue };
            let ch = track.channel();
            let mut messages = Vec::new();
            if let Some(bank) = track.bank() {
                messages.extend(bank_select(ch, bank));
            }
            if let Some(program) = track.program() {
                messages.push(program_change(ch, program));
            }
            if messages.is_empty() {
                continue;
            }
            let destination = match track.port() {
                OutputPort::Virtual => String::new(),
                OutputPort::Named(name) => name.clone(),
            };
            setup.push(Setup {
                track: track.id(),
                destination,
                channel: ch.get(),
                port,
                messages,
            });
        }
        // 트랙 순서대로 이벤트를 모은다. 들리는 트랙은 렌더러의 결과를, 들리지 않는 트랙(뮤트, 다른 트랙의
        // 솔로)은 뱅크·프로그램만 틱 0에 넣는다. 렌더러는 들리지 않는 트랙을 통째로 빼므로, 그대로 두면 재생
        // 시작 때 chase가 그 음색을 보내지 않아 재생 중 뮤트를 풀었을 때 음색이 틀린다(OUT-4, 리뷰 2 지적 3).
        // 같은 틱의 프로그램은 스냅샷 정렬이 넣은 순서를 지키므로, 같은 포트·채널을 나눠 쓰는 트랙끼리는
        // 트랙 순서에서 뒤 트랙이 이긴다(리뷰 3 지적 1: 들리지 않는 트랙을 뒤에 몰아 넣으면 늘 그것이 이겼다).
        let any_solo = project.tracks().iter().any(keyscore_core::Track::solo);
        let mut events = Vec::new();
        for (i, (track, port)) in project.tracks().iter().zip(&track_ports).enumerate() {
            let Some(port) = *port else { continue };
            let audible = if any_solo {
                track.solo()
            } else {
                !track.mute()
            };
            if audible {
                let only_this = render(project, |j, _| (j == i).then_some(port));
                events.extend(only_this.into_events());
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
        let snapshot = Snapshot::new(project.tempo_map().clone(), ports, events)?
            .with_meter(project.meter_map().clone());
        Ok(Prepared {
            snapshot: Arc::new(snapshot),
            setup,
            missing,
        })
    }

    /// 이름 포트를 연다. 한 번 연 포트는 다시 쓴다. 목적지 목록은 한 번만 읽는다.
    fn open_named(
        &mut self,
        name: &str,
        live: bool,
        available: &mut Option<Vec<PortInfo>>,
    ) -> Result<Arc<dyn MidiPort>, PortProblem> {
        if let Some(port) = self.opened.get(name) {
            return Ok(port.clone());
        }
        if live && self.known_missing.contains(name) {
            return Err(PortProblem::NotFound);
        }
        let list = available.get_or_insert_with(destinations);
        let Some(info) = list.iter().find(|i| i.name() == name) else {
            self.known_missing.insert(name.to_owned());
            return Err(PortProblem::NotFound);
        };
        let output = OsOutput::open(&self.client, info)
            .map_err(|e| PortProblem::OpenFailed(e.to_string()))?;
        let port: Arc<dyn MidiPort> = Arc::new(output);
        self.opened.insert(name.to_owned(), port.clone());
        Ok(port)
    }
}
