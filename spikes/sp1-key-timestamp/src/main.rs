//! SP-1: GPUI 키 핸들러에서 NSEvent.timestamp를 얻을 수 있는지, 얻은 값이 원래 이벤트와
//! 정확히 짝지어지는지 확인한다.
//!
//! GPUI(gpui-pre 0.3.8)의 `KeyDownEvent`/`Keystroke`에는 타임스탬프 필드가 없다(소스 분석).
//! 그래서 두 가지 우회 경로를 핸들러 안에서 동시에 읽어 비교한다.
//! - monitor: `NSEvent.addLocalMonitorForEventsMatchingMask`가 마지막으로 본 키 이벤트
//! - current: `NSApp.currentEvent`(지금 `sendEvent:` 중인 이벤트)
//!
//! 실행: `cargo run --release -- [phase]`
//! - `synth` (기본): 앱 안에서 `NSApp.postEvent`로 키 이벤트를 합성. 타임스탬프에 게시 시각을
//!   넣으므로 핸들러가 받은 값이 원래 이벤트의 값인지 정확히 대조할 수 있다.
//! - `osascript`: System Events로 실제 키 입력 합성(접근성 권한 필요).
//! - `manual`: 창을 띄우고 사람이 `a`/`s`를 친다. 20초 뒤 또는 `q`로 종료.
//!
//! 환경 변수 `SP1_LOAD_MS`: 매 프레임 render에서 메인 스레드를 바쁘게 붙잡는 시간(ms).
//! 프레임 지연이 「수신 즉시 시각」에는 섞이고 이벤트 타임스탬프에는 안 섞임을 보이려는 것.

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use gpui_kit::*;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSPoint, NSProcessInfo, NSString};

actions!(sp1, [KeyA, Quit]);

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

unsafe extern "C" {
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
}

/// `mach_absolute_time()`을 초로. NSEvent.timestamp와 같은 기준(부팅 후, 잠자기 제외)인지 확인용.
fn mach_now_s() -> f64 {
    let mut tb = MachTimebaseInfo { numer: 0, denom: 0 };
    // SAFETY: 커널이 채우는 출력 구조체 포인터만 넘긴다.
    let t = unsafe {
        mach_timebase_info(&mut tb);
        mach_absolute_time()
    };
    t as f64 * f64::from(tb.numer) / f64::from(tb.denom) / 1e9
}

fn uptime_s() -> f64 {
    NSProcessInfo::processInfo().systemUptime()
}

#[derive(Clone, Copy, Debug)]
struct MonitorRec {
    seq: u64,
    timestamp: f64,
    key_code: u16,
    is_down: bool,
    is_repeat: bool,
    /// 모니터 블록이 불린 시각(uptime). 핸들러 시각과 비교해 같은 dispatch 안인지 본다.
    seen_at: f64,
}

thread_local! {
    static MONITOR_SEQ: Cell<u64> = const { Cell::new(0) };
    static LAST_MONITOR: Cell<Option<MonitorRec>> = const { Cell::new(None) };
}

#[derive(Clone, Debug)]
struct HandlerRec {
    path: &'static str,
    key: String,
    is_held: bool,
    monitor: Option<MonitorRec>,
    current_ts: Option<f64>,
    current_key_code: Option<u16>,
    handler_uptime: f64,
    handler_mach: f64,
    /// 합성 이벤트에서 게시할 때 넣은 타임스탬프. 실제 키 입력에서는 None.
    injected_ts: Option<f64>,
}

#[derive(Default)]
struct Log {
    recs: Vec<HandlerRec>,
    /// 합성 이벤트: 게시 순서대로 (key_code, timestamp).
    posted: Vec<(u16, f64)>,
    next_expected: usize,
}

fn current_event() -> (Option<f64>, Option<u16>) {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    match app.currentEvent() {
        Some(ev) => {
            let ty = ev.r#type();
            let code = (ty == NSEventType::KeyDown || ty == NSEventType::KeyUp).then(|| ev.keyCode());
            (Some(ev.timestamp()), code)
        }
        None => (None, None),
    }
}

fn record(log: &Rc<RefCell<Log>>, path: &'static str, key: &str, is_held: bool) {
    let handler_uptime = uptime_s();
    let handler_mach = mach_now_s();
    let monitor = LAST_MONITOR.with(Cell::get);
    let (current_ts, current_key_code) = current_event();
    let mut log = log.borrow_mut();
    let injected_ts = if log.posted.is_empty() {
        None
    } else {
        let i = log.next_expected;
        log.next_expected += 1;
        log.posted.get(i).map(|p| p.1)
    };
    log.recs.push(HandlerRec {
        path,
        key: key.to_string(),
        is_held,
        monitor,
        current_ts,
        current_key_code,
        handler_uptime,
        handler_mach,
        injected_ts,
    });
}

struct Spike {
    focus: FocusHandle,
    log: Rc<RefCell<Log>>,
    load_ms: u64,
}

thread_local! {
    static RENDERS: Cell<u64> = const { Cell::new(0) };
}

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        RENDERS.with(|r| r.set(r.get() + 1));
        if self.load_ms > 0 {
            // 메인 스레드 부하 모사: 프레임마다 load_ms 동안 바쁘게 돈다.
            let until = Instant::now() + Duration::from_millis(self.load_ms);
            while Instant::now() < until {
                std::hint::spin_loop();
            }
            window.request_animation_frame();
        }
        let n = self.log.borrow().recs.len();
        div()
            .track_focus(&self.focus)
            .key_context("Spike")
            .size_full()
            .bg(rgb(0x202020))
            .text_color(rgb(0xffffff))
            .p_4()
            .child(format!(
                "SP-1: a = Action 경로, s = on_key_down 경로, q = 종료. 기록 {n}개"
            ))
            .on_action(cx.listener(|this, _: &KeyA, _, cx| {
                record(&this.log, "action", "a", false);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "s" {
                    record(&this.log, "key_down", "s", ev.is_held);
                    cx.notify();
                }
            }))
    }
}

fn install_monitor() {
    let block = RcBlock::new(|ev: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit이 넘긴 유효한 NSEvent 포인터. 블록 안에서만 빌린다.
        let e = unsafe { ev.as_ref() };
        let ty = e.r#type();
        let seq = MONITOR_SEQ.with(|s| {
            s.set(s.get() + 1);
            s.get()
        });
        LAST_MONITOR.with(|m| {
            m.set(Some(MonitorRec {
                seq,
                timestamp: e.timestamp(),
                key_code: e.keyCode(),
                is_down: ty == NSEventType::KeyDown,
                is_repeat: ty == NSEventType::KeyDown && e.isARepeat(),
                seen_at: uptime_s(),
            }));
        });
        ev.as_ptr()
    });
    // SAFETY: 블록은 받은 이벤트를 그대로 돌려준다(유효한 포인터). 반환된 모니터 토큰은
    // 프로세스 수명 동안 유지하려고 leak한다.
    let token = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::KeyDown | NSEventMask::KeyUp,
            &block,
        )
    };
    std::mem::forget(token);
    std::mem::forget(block);
}

/// 키 하나(down, up)를 큐 끝에 넣는다. 타임스탬프는 게시 시각(uptime).
fn post_key(code: u16, ch: &str, log: &Rc<RefCell<Log>>) {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    let win_num = app.keyWindow().map_or(0, |w| w.windowNumber());
    let s = NSString::from_str(ch);
    let ts = uptime_s();
    for ty in [NSEventType::KeyDown, NSEventType::KeyUp] {
        let ev = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            ty,
            NSPoint::new(0., 0.),
            NSEventModifierFlags::empty(),
            ts,
            win_num,
            None,
            &s,
            &s,
            false,
            code,
        )
        .expect("keyEvent");
        app.postEvent_atStart(&ev, false);
    }
    log.borrow_mut().posted.push((code, ts));
}

fn main() {
    let phase = std::env::args().nth(1).unwrap_or_else(|| "synth".into());
    let load_ms: u64 = std::env::var("SP1_LOAD_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let log = Rc::new(RefCell::new(Log::default()));

    gpui_kit::application().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("a", KeyA, Some("Spike")),
            KeyBinding::new("q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        install_monitor();

        let bounds = Bounds::centered(None, size(px(700.), px(200.)), cx);
        let log_view = log.clone();
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    cx.new(|_| Spike {
                        focus,
                        log: log_view,
                        load_ms,
                    })
                },
            )
            .expect("open window");
        cx.activate(true);
        let _ = window;

        let log_task = log.clone();
        let phase_task = phase.clone();
        cx.spawn(async move |cx| {
            let timer = |ms| cx.background_executor().timer(Duration::from_millis(ms));
            timer(1500).await;
            match phase_task.as_str() {
                "synth" => {
                    // 프레임(16.7 ms)과 맞물리지 않는 간격으로 a/s를 번갈아 60번.
                    for i in 0..60u64 {
                        let (code, ch) = if i % 2 == 0 { (0, "a") } else { (1, "s") };
                        let _ = cx.update(|_| post_key(code, ch, &log_task));
                        timer(37 + (i * 7) % 11).await;
                    }
                    timer(500).await;
                }
                "osascript" => {
                    // 메인 스레드를 막지 않도록 별도 스레드에서 실제 키 입력을 만든다.
                    // 이벤트는 WindowServer를 거쳐 메인 스레드 상태와 무관한 시각에 도착한다.
                    let driver = std::thread::spawn(|| {
                        for i in 0..30 {
                            let ch = if i % 2 == 0 { "a" } else { "s" };
                            let out = std::process::Command::new("osascript")
                                .arg("-e")
                                .arg(format!(
                                    "tell application \"System Events\" to keystroke \"{ch}\""
                                ))
                                .output();
                            if let Ok(o) = &out
                                && !o.status.success()
                            {
                                eprintln!("osascript failed: {}", String::from_utf8_lossy(&o.stderr));
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(23 + (i * 7) % 13));
                        }
                    });
                    while !driver.is_finished() {
                        timer(100).await;
                    }
                    timer(500).await;
                }
                _ => timer(20_000).await,
            }
            report(&log_task.borrow(), &phase_task, load_ms);
            let _ = cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

fn stats(v: &mut [f64]) -> String {
    if v.is_empty() {
        return "n=0".into();
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    format!(
        "n={} min={:.3} p50={:.3} p95={:.3} max={:.3}",
        v.len(),
        v[0],
        p(0.5),
        p(0.95),
        v[v.len() - 1]
    )
}

fn report(log: &Log, phase: &str, load_ms: u64) {
    println!("phase={phase} load_ms={load_ms} handler_calls={} posted={}", log.recs.len(), log.posted.len());
    let mut monitor_matches_current = 0;
    let mut monitor_keycode_ok = 0;
    let mut injected_match = 0;
    let mut delay_ms = Vec::new();
    let mut monitor_to_handler_ms = Vec::new();
    let mut mach_minus_uptime_ms = Vec::new();
    for r in &log.recs {
        let expected_code = if r.key == "a" { 0 } else { 1 };
        if let (Some(m), Some(c)) = (r.monitor, r.current_ts)
            && m.timestamp == c
        {
            monitor_matches_current += 1;
        }
        if let Some(m) = r.monitor
            && m.key_code == expected_code
            && m.is_down
            && r.current_key_code == Some(expected_code)
        {
            monitor_keycode_ok += 1;
        }
        // NSEvent는 타임스탬프를 정수 ns로 보관하므로 f64 왕복에서 1 µs 미만 오차가 생긴다.
        if let (Some(i), Some(c)) = (r.injected_ts, r.current_ts)
            && (i - c).abs() < 1e-6
        {
            injected_match += 1;
        }
        if let Some(c) = r.current_ts {
            delay_ms.push((r.handler_uptime - c) * 1e3);
        }
        if let Some(m) = r.monitor {
            monitor_to_handler_ms.push((r.handler_uptime - m.seen_at) * 1e3);
        }
        mach_minus_uptime_ms.push((r.handler_mach - r.handler_uptime) * 1e3);
    }
    // 핸들러마다 모니터가 정확히 down/up 한 쌍만큼 전진했는지(다른 키 이벤트가 끼지 않았는지).
    let seq_ok = log
        .recs
        .windows(2)
        .filter(|w| match (w[0].monitor, w[1].monitor) {
            (Some(a), Some(b)) => b.seq == a.seq + 2 && !b.is_repeat,
            _ => false,
        })
        .count();
    let n = log.recs.len();
    println!(
        "renders={} monitor seq step==2 between handlers: {seq_ok}/{}",
        RENDERS.with(Cell::get),
        n.saturating_sub(1)
    );
    let actions = log.recs.iter().filter(|r| r.path == "action").count();
    let keydowns = log.recs.iter().filter(|r| r.path == "key_down").count();
    let held = log.recs.iter().filter(|r| r.is_held).count();
    println!("paths: action={actions} key_down={keydowns} is_held={held}");
    println!("monitor.timestamp == currentEvent.timestamp: {monitor_matches_current}/{n}");
    println!("monitor/currentEvent keyCode == handler key (KeyDown): {monitor_keycode_ok}/{n}");
    if !log.posted.is_empty() {
        println!("currentEvent.timestamp == posted timestamp (in order): {injected_match}/{n}");
    }
    println!("handler_uptime - event.timestamp (ms): {}", stats(&mut delay_ms));
    println!("handler_uptime - monitor_seen_at (ms): {}", stats(&mut monitor_to_handler_ms));
    println!("mach_absolute_time_s - systemUptime (ms): {}", stats(&mut mach_minus_uptime_ms));
    for r in log.recs.iter().take(4) {
        println!("sample: {r:?}");
    }
}
