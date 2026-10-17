//! S2-T5a MIDI 엔진 연결과 트랜스포트 Action. 기본 가상 포트를 같은 프로세스의 `coremidi` 수신기로 받는다
//! (keyscore-midi 스케줄러 테스트와 같은 루프백).

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Edit, Note, Pitch, Project, Tick, Velocity, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn unique(name: &str) -> String {
    format!("KS app {} {name}", std::process::id())
}

/// 앱을 띄우고 엔진을 설치한 뒤, 기본 가상 포트를 받는 수신기를 붙인다.
fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-engine-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(name);
    let options = EngineOptions {
        client_name: format!("ks-app-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, options)).unwrap();
    assert!(cx.update(|cx| engine::is_installed(cx)));
    loopback(&port)
}

fn loopback(name: &str) -> Loopback {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-app-rx").unwrap();
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
    /// `limit` 동안 받은 것을 모두 모은다.
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

    /// `pred`에 맞는 메시지가 올 때까지 기다린다. 그때까지 받은 것(맞는 것 포함)을 돌려준다.
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

/// 120 BPM(480틱 = 0.25 s). 첫 트랙은 기본 가상 포트·채널 1·프로그램 5.
fn project(notes: &str) -> Project {
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{{"name": "Lead", "port": "virtual", "channel": 1, "program": 5,
            "clips": [{{"start": 0, "length": 15360, "notes": [{notes}]}}]}}]}}"#
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

fn state(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> TransportState {
    cx.read(|cx| view.read(cx).transport_state())
}

const WAIT: Duration = Duration::from_secs(2);

// ---------- 4. 단계 게이트: 트랜스포트가 스케줄러를 돌린다 ----------

#[gpui_kit::test]
fn play_action_sends_notes_to_virtual_port(cx: &mut TestAppContext) {
    let lb = start(cx, "play");
    let (handle, view) = open(cx, project("[0, 480, 60, 51492]"));
    run(cx, handle, transport::Play);
    assert_eq!(state(cx, &view), TransportState::Playing);
    let got = lb
        .wait_for(WAIT, note_on(60))
        .expect("Note On 60을 받지 못했다");
    // 재생 시작 때 프로그램 체인지가 먼저 온다(OUT-4)
    let program = got.iter().position(|m| m == &[0xC0, 5]);
    let first_note = got.iter().position(|m| note_on(60)(m));
    assert!(program.is_some() && program < first_note, "{got:?}");
    run(cx, handle, transport::Stop);
}

// ---------- 5. 정지와 패닉 ----------

#[gpui_kit::test]
fn stop_and_panic_silence_output(cx: &mut TestAppContext) {
    let lb = start(cx, "stop");
    let (handle, view) = open(cx, project("[0, 3840, 60, 51492]"));
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    assert_eq!(state(cx, &view), TransportState::Stopped);
    lb.wait_for(WAIT, note_off(60))
        .expect("정지가 울리는 노트를 끄지 않았다");
    run(cx, handle, transport::Panic);
    let got = lb.drain_for(Duration::from_millis(500));
    for cc in [120u8, 123, 64] {
        assert!(
            got.iter()
                .any(|m| m.len() == 3 && m[0] & 0xF0 == 0xB0 && m[1] == cc),
            "패닉에 CC {cc}가 없다: {got:?}"
        );
    }
}

// ---------- 6. 일시정지 ----------

#[gpui_kit::test]
fn pause_resumes_from_paused_position(cx: &mut TestAppContext) {
    let lb = start(cx, "pause");
    // 60은 틱 0, 64는 틱 1920(1 s)
    let (handle, view) = open(cx, project("[0, 240, 60, 51492], [1920, 240, 64, 51492]"));
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("Note On 60");
    std::thread::sleep(Duration::from_millis(300));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("일시정지 상태가 아니다: {:?}", state(cx, &view));
    };
    assert!(at > Tick::ZERO && at < Tick::new(1920), "{at:?}");
    lb.drain_for(Duration::from_millis(200));
    run(cx, handle, transport::Play);
    let got = lb
        .wait_for(WAIT, note_on(64))
        .expect("이어서 재생해 64를 받지 못했다");
    assert!(
        !got.iter().any(|m| note_on(60)(m)),
        "일시정지 뒤 처음부터 다시 재생했다: {got:?}"
    );
    run(cx, handle, transport::Stop);
    assert_eq!(state(cx, &view), TransportState::Stopped);
    lb.drain_for(Duration::from_millis(200));
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_on(60))
        .expect("정지 뒤 재생은 처음부터다");
    run(cx, handle, transport::Stop);
}

// ---------- 7. 메트로놈 ----------

#[gpui_kit::test]
fn metronome_toggle_sends_clicks(cx: &mut TestAppContext) {
    let lb = start(cx, "metronome");
    let (handle, view) = open(cx, project(""));
    run(cx, handle, transport::ToggleMetronome);
    assert!(cx.read(|cx| view.read(cx).metronome_on()));
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, |m| {
        m.len() == 3 && m[0] == 0x99 && (m[1] == 37 || m[1] == 42) && m[2] > 0
    })
    .expect("메트로놈 클릭(채널 10, 37·42)을 받지 못했다");
    run(cx, handle, transport::Stop);
    run(cx, handle, transport::ToggleMetronome);
    assert!(!cx.read(|cx| view.read(cx).metronome_on()));
    lb.drain_for(Duration::from_millis(200));
    run(cx, handle, transport::Play);
    let got = lb.drain_for(Duration::from_millis(1200));
    assert!(
        !got.iter().any(|m| m.len() == 3 && m[0] == 0x99 && m[2] > 0),
        "끈 메트로놈이 울린다: {got:?}"
    );
    run(cx, handle, transport::Stop);
}

// ---------- 8. 재생 중 편집(TRN-3) ----------

#[gpui_kit::test]
fn edit_while_playing_is_heard(cx: &mut TestAppContext) {
    let lb = start(cx, "live-edit");
    let (handle, view) = open(cx, project("[0, 240, 60, 51492]"));
    run(cx, handle, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("Note On 60");
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let track = doc.project().tracks()[0].id();
            let clip = doc.project().tracks()[0].clips().next().unwrap().0;
            let note = Note::new(
                Tick::new(1920),
                240,
                Pitch::new(67).unwrap(),
                Velocity::new(51492),
            )
            .unwrap();
            doc.apply(vec![Edit::InsertNote { track, clip, note }])
                .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    lb.wait_for(WAIT, note_on(67))
        .expect("재생 중 더한 노트가 들리지 않는다");
    run(cx, handle, transport::Stop);
}

// ---------- 9. 없는 이름 포트 ----------

#[gpui_kit::test]
fn missing_named_port_is_reported_and_silent(cx: &mut TestAppContext) {
    let lb = start(cx, "missing");
    let missing = unique("no such port");
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
            {{"name": "Gone", "port": {{"named": "{missing}"}}, "channel": 1,
              "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 60, 51492]]}}]}},
            {{"name": "Here", "port": "virtual", "channel": 2,
              "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 62, 51492]]}}]}}]}}"#
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    let got = lb
        .wait_for(WAIT, note_on(62))
        .expect("있는 포트의 트랙은 울린다");
    assert!(!got.iter().any(|m| note_on(60)(m)), "{got:?}");
    let messages = cx.read(|cx| view.read(cx).messages().to_vec());
    assert!(
        messages.iter().any(|m| m.contains(&missing)),
        "없는 포트를 알리지 않았다: {messages:?}"
    );
    run(cx, handle, transport::Stop);
}

// ---------- 10. 엔진 없음 ----------

#[gpui_kit::test]
fn transport_without_engine_reports_message(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("ks-engine-{}-none", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    assert!(!cx.update(|cx| engine::is_installed(cx)));
    let (handle, view) = open(cx, project("[0, 240, 60, 51492]"));
    run(cx, handle, transport::Play);
    assert_eq!(state(cx, &view), TransportState::Stopped);
    assert!(!cx.read(|cx| view.read(cx).messages().is_empty()));
    run(cx, handle, transport::Panic);
}

// ---------- 11. 한 번에 한 창 ----------

#[gpui_kit::test]
fn only_one_window_plays(cx: &mut TestAppContext) {
    let lb = start(cx, "two-windows");
    let (first, first_view) = open(cx, project("[0, 240, 60, 51492]"));
    let (second, second_view) = open(cx, project("[0, 240, 64, 51492]"));
    run(cx, first, transport::Play);
    lb.wait_for(WAIT, note_on(60)).expect("첫 창");
    run(cx, second, transport::Play);
    assert_eq!(state(cx, &first_view), TransportState::Stopped);
    assert_eq!(state(cx, &second_view), TransportState::Playing);
    lb.wait_for(WAIT, note_on(64)).expect("둘째 창");
    run(cx, second, transport::Stop);
}
