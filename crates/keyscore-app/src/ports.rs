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
    // 앱의 가상 포트는 소스라 목적지 목록에 없다. 있는 포트다(S2-T6d).
    if crate::engine::is_own_port(cx, name) {
        return Some(true);
    }
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
    // 메트로놈 포트도 이름으로 연다(MET-1). 사라졌다 돌아오면 트랙 포트처럼 다시 준비한다.
    if let OutputPort::Named(name) = crate::transport::metronome_settings(cx).port {
        names.push(name);
    }
    // 클럭 포트(OUT-5)도 같다(S2-T6e 리뷰 1 지적 1)
    for port in crate::transport::clock_settings(cx) {
        if let OutputPort::Named(name) = port
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    for window in project_windows(cx) {
        for track in window.read(cx).document().read(cx).project().tracks() {
            if let OutputPort::Named(name) = track.port()
                && !names.contains(name)
            {
                names.push(name.clone());
            }
        }
    }
    // 앱의 가상 포트(S2-T6d)는 목적지가 아니라 감시하지 않는다.
    let own = crate::engine::virtual_port_names(cx);
    names.retain(|n| !own.contains(n));
    names
}

/// 그 이름 포트를 쓰는 창들.
fn windows_using(cx: &App, name: &str) -> Vec<gpui_kit::Entity<ProjectWindow>> {
    // 앱 가상 포트 이름이면 같은 이름의 목적지는 아무도 쓰지 않는다(앱 포트가 이긴다). 그 목적지의 오감은 모든
    // 창에 핫플러그 알림으로 보인다(S2-T6e 리뷰 2 지적 1, 리뷰 3 제안).
    if crate::engine::is_own_port(cx, name) {
        return Vec::new();
    }
    // 메트로놈 포트와 클럭 포트는 모든 창이 쓴다.
    let port = OutputPort::Named(name.to_owned());
    if crate::transport::metronome_settings(cx).port == port
        || crate::transport::clock_settings(cx).contains(&port)
    {
        return project_windows(cx);
    }
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
            // 동작마다 그 이름이 이 이벤트 전에 연결돼 있었는지 함께 둔다. 끊긴 적 없이 같은 이름의 더 작은 ID
            // 포트로 옮기는 것은 「다시 연결」이 아니다(리뷰 2 제안 2).
            let connected_before = |name: &str| {
                matches!(
                    monitor.reconnector.link(name),
                    Some(keyscore_midi::Link::Connected(_))
                )
            };
            let before: Vec<(String, bool)> = wanted
                .iter()
                .map(|n| (n.clone(), connected_before(n)))
                .collect();
            let applied = monitor.reconnector.apply(event);
            for action in &applied {
                let was = match action {
                    // 이 이벤트로 끊겼다가 같은 이름의 남은 포트로 옮기는 것은 「다시 연결」이다
                    PortAction::Reconnect { name, .. } => {
                        before.iter().any(|(n, connected)| n == name && *connected)
                            && !applied
                                .iter()
                                .any(|a| matches!(a, PortAction::Lost { name: l } if l == name))
                    }
                    PortAction::Lost { .. } => true,
                };
                actions.push((action.clone(), was));
            }
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
    announce_hotplug(cx, &events);
    release_returned_devices(cx, &events);
    let changed = !events.is_empty();
    for (action, was_connected) in actions {
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
                        // 재생 중이면 스냅샷에서 뺀다. 이름만 바뀐 포트는 사라지지 않아, 그대로 두면 옛 이름
                        // 트랙의 소리가 그 포트로 계속 간다(리뷰 2 지적 1).
                        w.transport_port_lost(cx);
                    });
                }
            }
            PortAction::Reconnect { name, .. } => {
                cx.update_global::<Engine, _>(|engine, _| engine.port_returned(&name));
                let message = if was_connected {
                    format!("출력 포트 「{name}」을 같은 이름의 다른 포트로 바꿨다")
                } else {
                    format!("출력 포트 「{name}」에 다시 연결했다")
                };
                for window in windows_using(cx, &name) {
                    window.update(cx, |w, cx| {
                        w.say(message.clone(), cx);
                        w.transport_port_returned(&name, cx);
                    });
                }
            }
        }
    }
    housekeeping(cx, changed);
    if check_server && engine::is_installed(cx) {
        check_server_ports(cx, failures);
    }
}

/// 대체 매핑으로 만든 가상 포트의 장치가 돌아왔으면 그 포트를 지운다(OUT-3 재연결, 결정
/// 2026-10-22-out8-port-mapping-sheet 3 A). 다음 감시에서 그 이름을 목적지로 원하게 된다.
fn release_returned_devices(cx: &mut App, events: &[PortEvent]) {
    for event in events {
        if let PortEvent::Appeared(port) = event {
            engine::release_mapped(cx, port.name());
        }
    }
}

/// 서버 확인: 기본 포트가 풀렸으면 서버가 다시 뜬 것으로 보고 복구한다. 기본 포트는 멀쩡한데 추가 포트만
/// 풀렸으면 재생을 멈추지 않고 그 포트만 다시 등록한다(S2-T6d 리뷰 2 지적 2).
fn check_server_ports(cx: &mut App, failures: u32) {
    if cx.global::<Engine>().main_registered() {
        for name in cx.global_mut::<Engine>().reregister_extras() {
            crate::window::say_everywhere(cx, &format!("가상 포트 「{name}」을 다시 등록했다"));
        }
        return;
    }
    let ok = engine::recover(cx, failures == 0).is_ok();
    let monitor = cx.global_mut::<PortMonitor>();
    monitor.recover_failures = if ok { 0 } else { failures.saturating_add(1) };
}

/// 감시 한 번의 뒷정리: 포트가 오갔으면 창을 다시 그리고, 지운 가상 포트를 스냅샷이 놓았으면 버린다(소스가
/// 사라진다, S2-T6d).
fn housekeeping(cx: &mut App, changed: bool) {
    if changed {
        // 같은 이름의 더 앞선 목적지가 생겼을 수 있다. 미리듣기는 다음에 포트를 다시 연다(S3-T4c)
        if engine::is_installed(cx) {
            cx.global_mut::<Engine>().forget_preview();
        }
        refresh(cx);
    }
    if engine::is_installed(cx) {
        cx.global_mut::<Engine>()
            .release_retired(std::time::Duration::ZERO);
    }
}

/// 핫플러그(OUT-3): 쓰지 않는 포트의 나타남·사라짐도 상태줄 메시지로 알린다. 그 포트를 쓰는 창은 아래의
/// 재연결 동작이 더 자세히 알린다. 한 번에 여러 포트가 오가면(장치 하나가 포트를 여럿 냄) 종류마다 한
/// 줄로 묶는다. 메시지 목록이 한 장치로 밀려나지 않게 한다(리뷰 2 제안 3).
fn announce_hotplug(cx: &mut App, events: &[PortEvent]) {
    for window in project_windows(cx) {
        let mut came = Vec::new();
        let mut gone = Vec::new();
        for event in events {
            let (port, list) = match event {
                PortEvent::Appeared(p) => (p, &mut came),
                PortEvent::Disappeared(p) => (p, &mut gone),
            };
            let used = windows_using(cx, port.name())
                .iter()
                .any(|u| u.entity_id() == window.entity_id());
            if !used && !list.contains(&port.name()) {
                list.push(port.name());
            }
        }
        for (list, verb) in [(came, "새로 보인다"), (gone, "없어졌다")] {
            if let Some(message) = hotplug_message(&list, verb) {
                window.update(cx, |w, cx| w.say(message, cx));
            }
        }
    }
}

/// 쓰지 않는 포트의 핫플러그 한 줄. 이름은 모두 적는다(줄 수만 묶는다. 상태줄은 넘치는 글을 자른다).
fn hotplug_message(names: &[&str], verb: &str) -> Option<String> {
    match names {
        [] => None,
        [one] => Some(format!("출력 포트 「{one}」이 {verb}")),
        _ => {
            let listed: Vec<String> = names.iter().map(|n| format!("「{n}」")).collect();
            Some(format!(
                "출력 포트 {}개가 {verb}: {}",
                names.len(),
                listed.join(", ")
            ))
        }
    }
}

/// 상태줄의 포트 표시를 다시 그린다.
fn refresh(cx: &mut App) {
    for window in project_windows(cx) {
        window.update(cx, |_, cx| cx.notify());
    }
    crate::settings_window::ports_changed(cx);
}
