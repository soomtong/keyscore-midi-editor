//! S2-T5b: 같은 포트·채널을 나눠 쓰는 트랙의 실효 음색(S2-T5a 이월, SEQ-8·OUT-4).
//! 실효 음색이 바뀌면(뒤 트랙 삭제, 솔로 이동, 뮤트 변경) 그 채널의 뱅크·프로그램을 다시 보낸다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Edit, Project, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn unique(name: &str) -> String {
    format!("KS rv5a4 {} {name}", std::process::id())
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-rv5a4-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(name);
    let options = EngineOptions {
        client_name: format!("ks-rv5a4-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    loopback(&port)
}

fn loopback(name: &str) -> Loopback {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-rv5a4-rx").unwrap();
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

fn wait_for(
    rx: &mpsc::Receiver<Vec<u8>>,
    limit: Duration,
    pred: impl Fn(&[u8]) -> bool,
) -> Option<Vec<Vec<u8>>> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
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

fn note_on(pitch: u8) -> impl Fn(&[u8]) -> bool {
    move |m| m.len() == 3 && m[0] & 0xF0 == 0x90 && m[1] == pitch && m[2] > 0
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

const WAIT: Duration = Duration::from_secs(2);

fn edit(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    make: impl FnOnce(&Project) -> Vec<Edit>,
) {
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let e = make(doc.project());
            doc.apply(e).unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
}

/// 4분음표마다 같은 음(`pitch`)을 32번(시작은 델타).
fn every_beat(pitch: u8) -> String {
    (0..32)
        .map(|i| format!("[{}, 240, {pitch}, 51492]", if i == 0 { 0 } else { 480 }))
        .collect::<Vec<_>>()
        .join(", ")
}

fn shared(a: &str, b: &str) -> Project {
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
        {{"port": "virtual", "channel": 1, "program": 5{a},
         "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}},
        {{"port": "virtual", "channel": 1, "program": 9{b},
         "clips": [{{"start": 0, "length": 15360, "notes": [[0, 240, 64, 51492]]}}]}}]}}"#,
        every_beat(60)
    );
    ksq::from_str(&text).unwrap().0
}

// ---------- 리뷰 4 부록 A: 고친 뒤 회귀 방지로 옮김 ----------

fn drain_for(rx: &mpsc::Receiver<Vec<u8>>, limit: Duration) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(m) => got.push(m),
            Err(_) => return got,
        }
    }
}

/// S2-T5a 리뷰 4 수용 테스트(그대로): 공유 채널의 뒤 트랙을 재생 중 지우면 남은 트랙의 프로그램이 나간다.
#[gpui_kit::test]
fn removing_later_shared_track_while_playing_restores_remaining_program(cx: &mut TestAppContext) {
    let lb = start(cx, "remove");
    let (handle, view) = open(cx, shared("", ""));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![Edit::RemoveTrack {
            track: p.tracks()[1].id(),
        }]
    });
    drain_for(&lb.rx, Duration::from_millis(50));
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    assert!(got.iter().any(|m| m == &[0xC0, 5]), "{got:?}");
}

/// 재생 중 솔로를 앞 트랙으로 옮기면 그 트랙의 음색이 실효가 되어 나간다.
#[gpui_kit::test]
fn moving_solo_on_shared_channel_sends_new_effective_program(cx: &mut TestAppContext) {
    let lb = start(cx, "solo-move");
    let (handle, view) = open(cx, shared("", ""));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: keyscore_core::TrackProp::Solo(true),
        }]
    });
    drain_for(&lb.rx, Duration::from_millis(50));
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    assert!(got.iter().any(|m| m == &[0xC0, 5]), "{got:?}");
}

/// 들리지 않는 트랙의 프로그램만 바뀌면 실효 음색이 그대로라 보내지 않는다.
#[gpui_kit::test]
fn inaudible_program_change_does_not_override_audible_track(cx: &mut TestAppContext) {
    let lb = start(cx, "inaudible-edit");
    let (handle, view) = open(cx, shared("", r#", "mute": true"#));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[1].id(),
            prop: keyscore_core::TrackProp::Program(Some(keyscore_core::Program::new(20).unwrap())),
        }]
    });
    let got = drain_for(&lb.rx, Duration::from_millis(600));
    run(cx, handle, transport::Stop);
    assert!(!got.iter().any(|m| m == &[0xC0, 20]), "{got:?}");
}
