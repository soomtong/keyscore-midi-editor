//! S3-T3d 곡 끝 정지(결정 `docs/decisions/2026-10-25-playback-end-of-song.md` A). 기본 가상 포트를 같은
//! 프로세스의 `coremidi` 수신기로 받는다(`tests/engine.rs`와 같은 루프백).
//!
//! 창의 곡 끝 감시는 GPUI 타이머라 시험 시계(`advance_clock`)를 돌려야 깬다. 소리는 실제 시간으로 흐른다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Edit, Note, Pitch, Project, Tick, Velocity, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn unique(name: &str) -> String {
    format!("KS end {} {name}", std::process::id())
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-song-end-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(name);
    let options = EngineOptions {
        client_name: format!("ks-end-{name}"),
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

fn is_click(m: &[u8]) -> bool {
    m.len() == 3 && m[0] == 0x99 && m[2] > 0
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

// ---------- 곡 끝 ----------

#[test]
fn song_end_is_latest_clip_end() {
    let text = r#"{"tempo": [[0, 500000]], "tracks": [
        {"name": "A", "port": "virtual", "channel": 1,
         "clips": [{"start": 0, "length": 960, "notes": [[0, 3840, 60, 51492]]},
                   {"start": 1920, "length": 960, "notes": []}]},
        {"name": "B", "port": "virtual", "channel": 2,
         "clips": [{"start": 480, "length": 1920, "notes": []}]}]}"#;
    let p = ksq::from_str(text).unwrap().0;
    // A의 둘째 클립 끝 2880. 첫 클립의 숨김 꼬리(3840)는 세지 않는다.
    assert_eq!(engine::song_end(&p), Tick::new(2880));
    let empty = ksq::from_str(r#"{"tracks": [{"name": "A", "port": "virtual", "channel": 1}]}"#)
        .unwrap()
        .0;
    assert_eq!(engine::song_end(&empty), Tick::ZERO);
}

#[gpui_kit::test]
fn playback_stops_at_song_end_and_rewinds(cx: &mut TestAppContext) {
    let lb = start(cx, "stop");
    // 곡 끝 1920(1 s). 64는 곡 끝에서 끝난다.
    let (handle, view) = open(
        cx,
        project(1920, "[0, 480, 60, 51492], [1440, 480, 64, 51492]"),
    );
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    assert_eq!(state(cx, &view), TransportState::Playing);
    lb.wait_for(WAIT, note_off(64))
        .expect("곡 끝의 Note Off 64를 받지 못했다");
    assert!(
        pump_until(cx, &view, TransportState::Stopped, Duration::from_secs(1)),
        "곡 끝 뒤에도 재생 중이다"
    );
    let (position, playhead) = cx.read(|cx| {
        let v = view.read(cx);
        (v.transport_position(), v.piano_roll_playhead())
    });
    assert_eq!(position, Tick::ZERO, "정지 위치는 처음이다");
    assert_eq!(playhead, None, "정지하면 재생 헤드가 없다");
    let got = lb.drain_for(Duration::from_millis(800));
    assert!(
        !got.iter().any(|m| is_click(m)),
        "곡 끝 뒤에 메트로놈이 울린다: {got:?}"
    );
}

#[gpui_kit::test]
fn space_after_song_end_plays_from_start(cx: &mut TestAppContext) {
    let lb = start(cx, "again");
    let (handle, view) = open(
        cx,
        project(960, "[0, 240, 60, 51492], [480, 240, 62, 51492]"),
    );
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_off(62)).expect("Note Off 62");
    assert!(pump_until(
        cx,
        &view,
        TransportState::Stopped,
        Duration::from_secs(1)
    ));
    lb.drain_for(Duration::from_millis(100));
    run(cx, handle, transport::Toggle);
    assert_eq!(state(cx, &view), TransportState::Playing);
    let got = lb
        .wait_for(WAIT, note_on(62))
        .expect("다시 재생하지 않았다");
    assert!(
        got.iter().any(|m| note_on(60)(m)),
        "처음부터 재생하지 않았다: {got:?}"
    );
    assert!(pump_until(
        cx,
        &view,
        TransportState::Stopped,
        Duration::from_secs(2)
    ));
}

#[gpui_kit::test]
fn resume_after_song_end_stops_at_once(cx: &mut TestAppContext) {
    let lb = start(cx, "resume");
    let (handle, view) = open(cx, project(960, "[0, 240, 60, 51492]"));
    // Sequence 모드에서 곡 끝(0.5 s)을 지나 일시정지하고 Normal로 나온다
    mode_event(cx, handle, &view, ModeEvent::Enter(Mode::Sequence));
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("Note On 60");
    std::thread::sleep(Duration::from_millis(800));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("일시정지 상태가 아니다: {:?}", state(cx, &view));
    };
    assert!(at > Tick::new(960), "{at:?}");
    mode_event(cx, handle, &view, ModeEvent::ToNormal);
    assert_eq!(
        state(cx, &view),
        TransportState::Paused(at),
        "일시정지는 모드 전환으로 바뀌지 않는다"
    );
    run(cx, handle, transport::ToggleMetronome);
    lb.drain_for(Duration::from_millis(100));
    run(cx, handle, transport::Play);
    assert!(
        pump_until(
            cx,
            &view,
            TransportState::Stopped,
            Duration::from_millis(500)
        ),
        "곡 끝 뒤에서 재개하면 곧바로 멈춰야 한다"
    );
    assert_eq!(cx.read(|cx| view.read(cx).transport_position()), Tick::ZERO);
    let got = lb.drain_for(Duration::from_millis(600));
    assert!(!got.iter().any(|m| is_click(m)), "{got:?}");
}

#[gpui_kit::test]
fn edit_extending_song_while_playing_plays_to_new_end(cx: &mut TestAppContext) {
    let lb = start(cx, "extend");
    let (handle, view) = open(cx, project(960, "[0, 240, 60, 51492]"));
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("Note On 60");
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let track = doc.project().tracks()[0].id();
            let clip = doc.project().tracks()[0].clips().next().unwrap().0;
            let note = Note::new(
                Tick::new(1440),
                240,
                Pitch::new(67).unwrap(),
                Velocity::new(51492),
            )
            .unwrap();
            doc.apply(vec![
                Edit::ExtendClip {
                    track,
                    id: clip,
                    length: 1920,
                },
                Edit::InsertNote { track, clip, note },
            ])
            .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    lb.wait_for(WAIT, note_on(67))
        .expect("늘린 곡의 노트가 들리지 않는다");
    assert_eq!(state(cx, &view), TransportState::Playing);
    assert!(pump_until(
        cx,
        &view,
        TransportState::Stopped,
        Duration::from_secs(2)
    ));
}

// ---------- Sequence 모드 예외 ----------

#[gpui_kit::test]
fn sequence_mode_keeps_playing_past_song_end(cx: &mut TestAppContext) {
    let lb = start(cx, "sequence");
    let (handle, view) = open(cx, project(960, "[0, 240, 60, 51492]"));
    mode_event(cx, handle, &view, ModeEvent::Enter(Mode::Sequence));
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("Note On 60");
    // 곡 끝(0.5 s)을 지나 1 s 동안 감시를 깨운다
    pump_for(cx, Duration::from_secs(1));
    assert_eq!(
        state(cx, &view),
        TransportState::Playing,
        "Sequence 모드는 곡 끝에서 멈추지 않는다"
    );
    lb.wait_for(Duration::from_secs(1), is_click)
        .expect("Sequence 모드에서 곡 끝 뒤 메트로놈이 멈췄다");
    // 곡 끝 뒤에 Sequence에서 나오면 곧바로 멈춘다
    mode_event(cx, handle, &view, ModeEvent::ToNormal);
    assert!(
        pump_until(
            cx,
            &view,
            TransportState::Stopped,
            Duration::from_millis(500)
        ),
        "곡 끝 뒤 Sequence에서 나오면 멈춰야 한다"
    );
    lb.drain_for(Duration::from_millis(100));
    let got = lb.drain_for(Duration::from_millis(800));
    assert!(!got.iter().any(|m| is_click(m)), "{got:?}");
}
