//! 출력 포트 감시와 재연결(S2-T5d, OUT-1, OUT-3).
//!
//! `PortWatcher`(S1-T2)의 목적지 변화 이벤트를 메인 스레드에서 주기적으로 비워 `Reconnector`에 넣는다.
//! 원하는 이름은 열린 창들의 트랙이 쓰는 이름 포트 전체다. 사라지면 엔진의 핸들을 놓고 그 포트를 쓰는
//! 창에 알린다. 같은 이름이 돌아오면 그 창들에 알리고, 재생 중이면 새 핸들로 바꿔 보낸다.
//!
//! `PortWatcher::start`는 알림 탐침 때문에 블로킹하므로(약 0.25 s, 최악 2 s) 백그라운드에서 부른다.
//! 감시자는 1 s마다 기본 가상 포트의 등록도 본다. MIDI 서버가 다시 떴으면 다시 등록한다.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use gpui_kit::{App, AppContext as _, BorrowAppContext as _, Global};
use keyscore_core::OutputPort;
use keyscore_midi::{
    Action as PortAction, PortEvent, PortInfo, PortWatcher, Reconnector, destinations,
};

use crate::engine::{self, Engine};
use crate::window::{ProjectWindow, project_windows};

/// 감시 주기.
pub const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 가상 포트 등록을 확인하는 주기(감시 주기의 배수).
const SERVER_CHECK_EVERY: u32 = 10;

/// 앱 전역 포트 감시 상태.
struct PortMonitor {
    /// 돌고 있는 감시자. 시작 중이거나 실패했으면 없다.
    watcher: Option<(PortWatcher, Receiver<PortEvent>)>,
    /// 지금 있는 목적지(감시자 시작 때의 목록 + 이벤트).
    present: Vec<PortInfo>,
    reconnector: Reconnector,
    ticks: u32,
    /// 가상 포트 다시 등록이 잇달아 실패한 횟수. 실패하면 알리는 것은 처음 한 번이고, 시도 간격을 늘린다.
    recover_failures: u32,
    /// 감시자를 시작하는 중인가(겹쳐 시작하지 않는다).
    starting: bool,
}

impl Global for PortMonitor {}

/// 감시를 시작한다. 엔진이 없거나 이미 시작했으면 아무 일도 하지 않는다. 앱은 엔진 설치 뒤 부른다.
pub fn start_monitor(cx: &mut App) {
    if !engine::is_installed(cx) || cx.has_global::<PortMonitor>() {
        return;
    }
    cx.set_global(PortMonitor {
        watcher: None,
        present: Vec::new(),
        reconnector: Reconnector::new(),
        ticks: 0,
        recover_failures: 0,
        starting: false,
    });
    restart_watcher(cx);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            cx.update(tick);
        }
    })
    .detach();
}

/// 감시가 돌고 있는가.
#[must_use]
pub fn is_monitoring(cx: &App) -> bool {
    cx.has_global::<PortMonitor>() && cx.global::<PortMonitor>().watcher.is_some()
}

/// 그 이름의 목적지가 지금 있는가. 감시가 돌지 않으면 모른다(`None`).
#[must_use]
pub fn port_present(cx: &App, name: &str) -> Option<bool> {
    is_monitoring(cx).then(|| {
        cx.global::<PortMonitor>()
            .present
            .iter()
            .any(|p| p.name() == name)
    })
}

/// 감시자를 (다시) 시작한다. MIDI 서버가 다시 뜬 뒤 옛 감시자는 알림을 받지 못한다.
pub(crate) fn restart_watcher(cx: &mut App) {
    if !cx.has_global::<PortMonitor>() || cx.global::<PortMonitor>().starting {
        return;
    }
    {
        let monitor = cx.global_mut::<PortMonitor>();
        monitor.watcher = None;
        monitor.starting = true;
    }
    let started = cx.background_spawn(async move {
        PortWatcher::start().map(|watcher| (watcher, destinations()))
    });
    cx.spawn(async move |cx| {
        let result = started.await;
        cx.update(|cx| {
            let failed = {
                let monitor = cx.global_mut::<PortMonitor>();
                monitor.starting = false;
                match result {
                    Ok(((watcher, events), now)) => {
                        monitor.watcher = Some((watcher, events));
                        monitor.present = now;
                        // 시작 전의 연결 상태를 지금 목록으로 다시 정한다
                        monitor.reconnector = Reconnector::new();
                        None
                    }
                    Err(e) => Some(e),
                }
            };
            if let Some(e) = failed {
                crate::window::say_everywhere(
                    cx,
                    &format!("출력 포트 감시를 시작하지 못했다(포트 소멸·복귀를 알 수 없다): {e}"),
                );
            }
            refresh(cx);
        });
    })
    .detach();
}

/// 열린 창들이 쓰는 이름 포트.
fn wanted_names(cx: &App) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for window in project_windows(cx) {
        for track in window.read(cx).document().read(cx).project().tracks() {
            if let OutputPort::Named(name) = track.port()
                && !names.contains(name)
            {
                names.push(name.clone());
            }
        }
    }
    names
}

/// 그 이름 포트를 쓰는 창들.
fn windows_using(cx: &App, name: &str) -> Vec<gpui_kit::Entity<ProjectWindow>> {
    project_windows(cx)
        .into_iter()
        .filter(|w| {
            w.read(cx)
                .document()
                .read(cx)
                .project()
                .tracks()
                .iter()
                .any(|t| matches!(t.port(), OutputPort::Named(n) if n == name))
        })
        .collect()
}

/// 감시 한 번: 이벤트를 비우고, 원하는 이름을 맞추고, 동작을 처리한다.
fn tick(cx: &mut App) {
    if !cx.has_global::<PortMonitor>() {
        return;
    }
    let wanted = wanted_names(cx);
    let (events, actions, check_server, failures) = {
        let monitor = cx.global_mut::<PortMonitor>();
        monitor.ticks = monitor.ticks.wrapping_add(1);
        // 다시 등록이 실패하고 있으면 열 배로 드물게 시도한다(서버가 돌아올 때까지)
        let every = if monitor.recover_failures == 0 {
            SERVER_CHECK_EVERY
        } else {
            SERVER_CHECK_EVERY * 10
        };
        let check_server = monitor.ticks.is_multiple_of(every);
        let mut events = Vec::new();
        if let Some((_, rx)) = &monitor.watcher {
            events.extend(rx.try_iter());
        }
        let mut actions = Vec::new();
        for event in &events {
            match event {
                PortEvent::Disappeared(port) => monitor.present.retain(|p| p != port),
                PortEvent::Appeared(port) => {
                    if !monitor.present.contains(port) {
                        monitor.present.push(port.clone());
                    }
                }
            }
            actions.extend(monitor.reconnector.apply(event));
        }
        // 새로 원하게 된 이름은 이벤트를 반영한 지금 목록으로 등록한다. 먼저 등록하면 옛 목록에서 「없음」으로
        // 정해졌다가 쌓여 있던 나타남 이벤트로 「다시 연결」을 잘못 알린다.
        for name in &wanted {
            if monitor.reconnector.link(name).is_none() {
                monitor.reconnector.want(name, &monitor.present);
            }
        }
        (events, actions, check_server, monitor.recover_failures)
    };
    // 핫플러그(OUT-3): 쓰지 않는 포트의 나타남·사라짐도 상태줄 메시지로 알린다. 그 포트를 쓰는 창은 아래의
    // 재연결 동작이 더 자세히 알린다.
    for event in &events {
        let (port, text) = match event {
            PortEvent::Appeared(p) => (p, "출력 포트 「{}」이 새로 보인다"),
            PortEvent::Disappeared(p) => (p, "출력 포트 「{}」이 없어졌다"),
        };
        let users = windows_using(cx, port.name());
        for window in project_windows(cx) {
            if users.iter().any(|u| u.entity_id() == window.entity_id()) {
                continue;
            }
            let message = text.replace("{}", port.name());
            window.update(cx, |w, cx| w.say(message, cx));
        }
    }
    let changed = !events.is_empty();
    for action in actions {
        match action {
            PortAction::Lost { name } => {
                cx.update_global::<Engine, _>(|engine, _| engine.port_lost(&name));
                for window in windows_using(cx, &name) {
                    window.update(cx, |w, cx| {
                        w.say(
                            format!(
                                "출력 포트 「{name}」이 사라졌다. 같은 이름의 포트가 돌아오면 다시 연결한다"
                            ),
                            cx,
                        );
                    });
                }
            }
            PortAction::Reconnect { name, .. } => {
                cx.update_global::<Engine, _>(|engine, _| engine.port_returned(&name));
                for window in windows_using(cx, &name) {
                    window.update(cx, |w, cx| {
                        w.say(format!("출력 포트 「{name}」에 다시 연결했다"), cx);
                        w.transport_port_returned(&name, cx);
                    });
                }
            }
        }
    }
    if changed {
        refresh(cx);
    }
    if check_server && engine::is_installed(cx) && !cx.global::<Engine>().main_registered() {
        let ok = engine::recover(cx, failures == 0).is_ok();
        let monitor = cx.global_mut::<PortMonitor>();
        monitor.recover_failures = if ok { 0 } else { failures.saturating_add(1) };
    }
}

/// 상태줄의 포트 표시를 다시 그린다.
fn refresh(cx: &mut App) {
    for window in project_windows(cx) {
        window.update(cx, |_, cx| cx.notify());
    }
}
