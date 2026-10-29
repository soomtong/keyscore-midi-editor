//! S3-T3d 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s3-t3d/review-1.md`). 도우미는 `tests/song_end.rs`와
//! 같다. 감시 작업의 수명(다른 창 재생, 일시정지·재개)을 본다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, Tick, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn unique(name: &str) -> String {
    format!("KS rv-end {} {name}", std::process::id())
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-rv-song-end-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(name);
    let options = EngineOptions {
        client_name: format!("ks-rv-end-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    loopback(&port)
}

fn loopback(name: &str) -> Loopback {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-end-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                for msg in split(p.data()) {
                    let _ = tx.send(msg);
                }
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let source = loop {
        if let Some(s) = coremidi::Source::from_name(name) {
            break s;
        }
        assert!(
            Instant::now() < deadline,
            "가상 포트 {name}이 보이지 않는다"
        );
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

impl Loopback {
    fn drain_for(&self, limit: Duration) -> Vec<Vec<u8>> {
        let mut got = Vec::new();
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(m) => got.push(m),
                Err(_) => return got,
            }
        }
    }

    fn wait_for(&self, limit: Duration, pred: impl Fn(&[u8]) -> bool) -> Option<Vec<Vec<u8>>> {
        let mut got = Vec::new();
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(m) => {
                    let hit = pred(&m);
                    got.push(m);
                    if hit {
                        return Some(got);
                    }
                }
                Err(_) => return None,
            }
        }
    }
}

fn note_on(pitch: u8) -> impl Fn(&[u8]) -> bool {
    move |m| m.len() == 3 && m[0] & 0xF0 == 0x90 && m[1] == pitch && m[2] > 0
}

fn note_off(pitch: u8) -> impl Fn(&[u8]) -> bool {
    move |m| {
        m.len() == 3 && m[1] == pitch && (m[0] & 0xF0 == 0x80 || (m[0] & 0xF0 == 0x90 && m[2] == 0))
    }
}

/// 120 BPM(960틱 = 0.5 s). 클립 하나 `[0, length)`.
fn project(length: u64, notes: &str) -> Project {
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{{"name": "Lead", "port": "virtual", "channel": 1,
            "clips": [{{"start": 0, "length": {length}, "notes": [{notes}]}}]}}]}}"#
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

fn mode_event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    event: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |v, cx| v.apply_mode_event(event, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
}

fn state(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> TransportState {
    cx.read(|cx| view.read(cx).transport_state())
}

/// 곡 끝 감시를 깨우며 `limit` 동안 상태가 `want`가 되기를 기다린다.
fn pump_until(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    want: TransportState,
    limit: Duration,
) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        cx.executor().advance_clock(engine::END_POLL);
        cx.run_until_parked();
        if state(cx, view) == want {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// 곡 끝 감시를 `limit` 동안 깨운다(상태를 바꿀 기회를 준다).
fn pump_for(cx: &mut TestAppContext, limit: Duration) {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        cx.executor().advance_clock(engine::END_POLL);
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
}

const WAIT: Duration = Duration::from_secs(3);

/// 창 A가 재생하다 창 B가 재생을 가져간다. B는 자기 곡 끝에서 멈추고, A는 정지 그대로이며 A의 옛 감시가
/// B의 곡 끝 정지를 가로채지 않는다. 그 뒤 A를 다시 재생하면 A의 곡 끝에서 멈춘다.
#[gpui_kit::test]
fn other_window_takes_over_and_each_stops_at_own_end(cx: &mut TestAppContext) {
    let lb = start(cx, "two");
    let (ha, a) = open(cx, project(3840, "[0, 240, 60, 51492]"));
    let (hb, b) = open(cx, project(960, "[0, 240, 62, 51492]"));
    run(cx, ha, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("A의 Note On 60");
    run(cx, hb, transport::Play);
    assert_eq!(
        state(cx, &a),
        TransportState::Stopped,
        "B가 가져가면 A는 정지"
    );
    assert_eq!(state(cx, &b), TransportState::Playing);
    lb.wait_for(WAIT, note_on(62)).expect("B의 Note On 62");
    assert!(
        pump_until(cx, &b, TransportState::Stopped, Duration::from_secs(2)),
        "B가 곡 끝에서 멈추지 않았다"
    );
    assert_eq!(state(cx, &a), TransportState::Stopped);
    // A를 다시 재생: 2 s 곡 끝까지 재생 중이어야 하고, 그 뒤 멈춘다
    lb.drain_for(Duration::from_millis(50));
    run(cx, ha, transport::Play);
    pump_for(cx, Duration::from_millis(800));
    assert_eq!(
        state(cx, &a),
        TransportState::Playing,
        "곡 끝 전인데 A가 멈췄다(옛 정지 보고)"
    );
    assert!(
        pump_until(cx, &a, TransportState::Stopped, Duration::from_secs(3)),
        "A가 곡 끝에서 멈추지 않았다"
    );
}

/// 곡 안에서 일시정지 → 재개하면 새 감시가 붙어 곡 끝에서 멈춘다(일시정지가 감시를 끝낸 뒤에도).
#[gpui_kit::test]
fn pause_inside_song_then_resume_still_stops_at_end(cx: &mut TestAppContext) {
    let lb = start(cx, "pause");
    let (handle, view) = open(
        cx,
        project(1920, "[0, 240, 60, 51492], [1440, 480, 64, 51492]"),
    );
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("Note On 60");
    std::thread::sleep(Duration::from_millis(300));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("일시정지가 아니다: {:?}", state(cx, &view));
    };
    assert!(at < Tick::new(1920), "{at:?}");
    // 옛 감시가 끝날 기회를 준다
    pump_for(cx, Duration::from_millis(100));
    run(cx, handle, transport::Play);
    assert_eq!(state(cx, &view), TransportState::Playing);
    lb.wait_for(WAIT, note_off(64))
        .expect("곡 끝의 Note Off 64");
    assert!(
        pump_until(cx, &view, TransportState::Stopped, Duration::from_secs(1)),
        "재개한 재생이 곡 끝에서 멈추지 않았다"
    );
    assert_eq!(cx.read(|cx| view.read(cx).transport_position()), Tick::ZERO);
}

/// 정지 중 Sequence 진입·이탈은 재생을 시작하지 않는다.
#[gpui_kit::test]
fn sequence_toggle_while_stopped_does_not_play(cx: &mut TestAppContext) {
    let lb = start(cx, "seqstop");
    let (handle, view) = open(cx, project(960, "[0, 240, 60, 51492]"));
    mode_event(cx, handle, &view, ModeEvent::Enter(Mode::Sequence));
    mode_event(cx, handle, &view, ModeEvent::ToNormal);
    pump_for(cx, Duration::from_millis(100));
    assert_eq!(state(cx, &view), TransportState::Stopped);
    let got = lb.drain_for(Duration::from_millis(300));
    assert!(!got.iter().any(|m| note_on(60)(m)), "{got:?}");
}
