//! S3-T8 일시정지 자리. 스케줄러는 lookahead(10 ms) 안의 노트를 OS에 미리 넘긴다. 박 직전에 일시정지해도
//! 자리는 이미 넘긴(들린) 박 뒤여야 하고, 이어서 재생하면 그 박이 다시 울리지 않아야 한다.
//! 기본 가상 포트를 같은 프로세스의 `coremidi` 수신기로 받는다.

use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

/// 받은 메시지: (도착 순간, 바이트).
type Received = (Instant, Vec<u8>);

struct Loopback {
    rx: mpsc::Receiver<Received>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

/// 박 하나 = 480틱 = 250 ms(120 BPM).
const BEAT: Duration = Duration::from_millis(250);
/// 박 직전에 누르는 여유. lookahead(10 ms)보다 짧아 그 박은 이미 넘겨졌다.
const BEFORE: Duration = Duration::from_millis(4);
/// 첫 박의 음 번호. `i`번째 박(틱 `480 × i`)은 음 `FIRST + i`.
const FIRST: u8 = 40;
const BEATS: u8 = 32;

/// 두 시험은 박 시각을 재므로 서로 겹치지 않게 돈다.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-s3t8-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = format!("KS s3t8 {} {name}", std::process::id());
    let options = EngineOptions {
        client_name: format!("ks-s3t8-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    loopback(&port)
}

fn loopback(name: &str) -> Loopback {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-s3t8-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            let arrived = Instant::now();
            for p in list {
                for msg in split(p.data()) {
                    let _ = tx.send((arrived, msg));
                }
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let source = loop {
        if let Some(s) = coremidi::Source::from_name(name) {
            break s;
        }
        assert!(Instant::now() < deadline, "{name}이 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    };
    input.connect_source(&source).unwrap();
    Loopback {
        rx,
        _input: input,
        _client: client,
    }
}

fn split(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let len = match data[i] & 0xF0 {
            0xC0 | 0xD0 => 2,
            0xF0 => 1,
            _ => 3,
        };
        out.push(data[i..(i + len).min(data.len())].to_vec());
        i += len;
    }
    out
}

/// `limit` 동안 받은 Note On의 음 번호.
fn heard_for(rx: &mpsc::Receiver<Received>, limit: Duration) -> Vec<u8> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok((_, m)) if m.len() == 3 && m[0] & 0xF0 == 0x90 && m[2] > 0 => got.push(m[1]),
            Ok(_) => {}
            Err(_) => return got,
        }
    }
}

/// 다음 Note On 하나를 기다려 (도착 순간, 음 번호).
fn next_note(rx: &mpsc::Receiver<Received>) -> (Instant, u8) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let (at, m) = rx.recv_timeout(left).expect("노트가 오지 않는다");
        if m.len() == 3 && m[0] & 0xF0 == 0x90 && m[2] > 0 {
            return (at, m[1]);
        }
    }
}

/// 120 BPM, 기본 가상 포트·채널 1. 박마다 음 번호가 다른 짧은 노트.
fn beats() -> Project {
    let notes: Vec<String> = (0..BEATS)
        .map(|i| {
            format!(
                "[{}, 120, {}, 51492]",
                if i == 0 { 0 } else { 480 },
                FIRST + i
            )
        })
        .collect();
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{{"name": "Beat", "port": "virtual", "channel": 1,
            "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}}]}}"#,
        notes.join(", ")
    );
    ksq::from_str(&text).unwrap().0
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    (handle, view)
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn paused_at(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> u64 {
    match cx.read(|cx| view.read(cx).transport_state()) {
        TransportState::Paused(at) => at.get(),
        other => panic!("일시정지가 아니다: {other:?}"),
    }
}

/// 박 직전에 일시정지하면 자리는 이미 들린 박 뒤다.
#[gpui_kit::test]
fn pause_position_never_precedes_heard_beat(cx: &mut TestAppContext) {
    let _serial = serial();
    let lb = start(cx, "precede");
    let (handle, view) = open(cx, beats());
    for round in 0..8u32 {
        run(cx, handle, transport::Play);
        let (first, pitch) = next_note(&lb.rx);
        assert_eq!(pitch, FIRST, "{round}회차 첫 박");
        // 1–3번째 박 직전
        let k = 1 + round % 3;
        std::thread::sleep(
            (first + (BEAT * k).saturating_sub(BEFORE)).saturating_duration_since(Instant::now()),
        );
        run(cx, handle, transport::Pause);
        let at = paused_at(cx, &view);
        let heard = heard_for(&lb.rx, Duration::from_millis(40));
        let last = heard.last().copied().unwrap_or(FIRST);
        let last_tick = u64::from(last - FIRST) * 480;
        assert!(
            at > last_tick,
            "{round}회차 일시정지 자리 {at}가 이미 들린 박(틱 {last_tick}, 음 {last})보다 앞이다"
        );
        run(cx, handle, transport::Stop);
        let _ = heard_for(&lb.rx, Duration::from_millis(20));
    }
}

/// 일시정지와 이어서 재생을 거듭해도 들린 박이 다시 울리지 않고 빠지지도 않는다.
#[gpui_kit::test]
fn resume_after_pause_does_not_repeat_heard_note(cx: &mut TestAppContext) {
    let _serial = serial();
    let lb = start(cx, "resume");
    let (handle, view) = open(cx, beats());
    let mut heard = Vec::new();
    run(cx, handle, transport::Play);
    for _ in 0..6 {
        let (at, pitch) = next_note(&lb.rx);
        heard.push(pitch);
        std::thread::sleep(
            (at + BEAT.saturating_sub(BEFORE)).saturating_duration_since(Instant::now()),
        );
        run(cx, handle, transport::Pause);
        let _ = paused_at(cx, &view);
        heard.extend(heard_for(&lb.rx, Duration::from_millis(40)));
        run(cx, handle, transport::Play);
    }
    heard.extend(heard_for(&lb.rx, BEAT * 3));
    run(cx, handle, transport::Stop);
    let n = u8::try_from(heard.len()).unwrap();
    let expected: Vec<u8> = (FIRST..FIRST + n).collect();
    assert_eq!(heard, expected, "들린 박이 겹치거나 빠졌다");
}
