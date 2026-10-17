//! MIDI 엔진 연결(S2-T5a): 앱 전역 `CoreMIDI` 클라이언트, 기본 가상 포트, 스케줄러.
//!
//! 바이너리는 창을 열기 전에 [`install`]을 부른다. 프로세스의 첫 `CoreMIDI` 호출이 `MidiClient::new`여야
//! 포트 알림이 온다(S1-T2). [`crate::init`]은 `CoreMIDI`를 건드리지 않아, MIDI가 필요 없는 테스트는 MIDI
//! 서버와 무관하게 돈다. 설치하지 않았거나 실패하면 트랜스포트 Action은 메시지만 남긴다.
//!
//! 앱은 스케줄러의 제어 쪽 API만 부른다. 실시간 스레드는 S1 그대로다.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{App, Global, WeakEntity};
use keyscore_core::{OutputPort, Project, TempoMap, Tick};
use keyscore_midi::{
    DEFAULT_PORT_NAME, MAX_SCHEDULER_PORTS, Midi1Message, MidiClient, MidiError, MidiPort,
    OsOutput, PortInfo, Scheduler, SchedulerConfig, Snapshot, bank_select, destinations,
    program_change, render,
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
}

impl Global for Engine {}

/// 재생할 준비물.
pub(crate) struct Prepared {
    pub(crate) snapshot: Arc<Snapshot>,
    /// 재생 시작 때 보낼 뱅크·프로그램(OUT-4). 틱 0이 아닌 곳에서 시작해도 음색이 맞게 한다.
    pub(crate) setup: Vec<(u16, Midi1Message)>,
    /// 열 수 없는 이름 포트. 그 트랙은 소리 내지 않는다.
    pub(crate) missing: Vec<String>,
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
    /// # Errors
    ///
    /// 스냅샷을 만들 수 없으면(포트·이벤트 범위) 그 오류.
    pub(crate) fn prepare(&mut self, project: &Project) -> Result<Prepared, MidiError> {
        let mut ports: Vec<Arc<dyn MidiPort>> = vec![self.main.clone()];
        let mut index: HashMap<&str, u16> = HashMap::new();
        let mut missing: Vec<String> = Vec::new();
        let mut available: Option<Vec<PortInfo>> = None;
        let mut track_ports = Vec::with_capacity(project.tracks().len());
        for track in project.tracks() {
            let port = match track.port() {
                OutputPort::Virtual => Some(0),
                OutputPort::Named(name) => {
                    if let Some(&i) = index.get(name.as_str()) {
                        Some(i)
                    } else {
                        let opened = (ports.len() < MAX_SCHEDULER_PORTS)
                            .then(|| self.open_named(name, &mut available))
                            .flatten();
                        let pushed = opened.and_then(|p| {
                            ports.push(p);
                            u16::try_from(ports.len() - 1).ok()
                        });
                        if let Some(i) = pushed {
                            index.insert(name, i);
                            Some(i)
                        } else {
                            if !missing.contains(name) {
                                missing.push(name.clone());
                            }
                            None
                        }
                    }
                }
            };
            track_ports.push(port);
        }
        let mut setup = Vec::new();
        for (track, port) in project.tracks().iter().zip(&track_ports) {
            let Some(port) = *port else { continue };
            let ch = track.channel();
            if let Some(bank) = track.bank() {
                setup.extend(bank_select(ch, bank).map(|m| (port, m)));
            }
            if let Some(program) = track.program() {
                setup.push((port, program_change(ch, program)));
            }
        }
        let rendered = render(project, |i, _| track_ports.get(i).copied().flatten());
        let snapshot = Snapshot::new(project.tempo_map().clone(), ports, rendered.into_events())?
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
        available: &mut Option<Vec<PortInfo>>,
    ) -> Option<Arc<dyn MidiPort>> {
        if let Some(port) = self.opened.get(name) {
            return Some(port.clone());
        }
        let list = available.get_or_insert_with(destinations);
        let info = list.iter().find(|i| i.name() == name)?;
        let port: Arc<dyn MidiPort> = Arc::new(OsOutput::open(&self.client, info).ok()?);
        self.opened.insert(name.to_owned(), port.clone());
        Some(port)
    }
}
