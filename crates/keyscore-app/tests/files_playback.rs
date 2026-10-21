//! S2-T7a: 창 닫기 훅에서 재생을 멈춘다(S2-T5a 이월). 시험이 뷰의 강한 참조를 쥐고 있어 뷰 해제(`on_release`)는
//! 일어나지 않는다. 그래도 닫으면 노트가 꺼져야 한다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::{AnyWindowHandle, AppContext as _, TestAppContext, VisualTestContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-files-pb-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = format!("KS files {} {name}", std::process::id());
    let options = EngineOptions {
        client_name: format!("ks-files-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-files-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let data = p.data();
                let mut i = 0;
                while i < data.len() {
                    let len = match data[i] & 0xF0 {
                        0xC0 | 0xD0 => 2,
                        0xF0 => 1,
                        _ => 3,
                    };
                    let _ = tx.send(data[i..(i + len).min(data.len())].to_vec());
                    i += len;
                }
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let source = loop {
        if let Some(s) = coremidi::Source::from_name(&port) {
            break s;
        }
        assert!(Instant::now() < deadline, "{port}이 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    };
    input.connect_source(&source).unwrap();
    Loopback {
        rx,
        _input: input,
        _client: client,
    }
}

fn wait_for(rx: &mpsc::Receiver<Vec<u8>>, pred: impl Fn(&[u8]) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(m) if pred(&m) => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

fn note_on(m: &[u8]) -> bool {
    m.len() == 3 && m[0] & 0xF0 == 0x90 && m[1] == 60 && m[2] > 0
}

fn note_off(m: &[u8]) -> bool {
    m.len() == 3 && m[1] == 60 && (m[0] & 0xF0 == 0x80 || (m[0] & 0xF0 == 0x90 && m[2] == 0))
}

/// 120 BPM, 기본 가상 포트, 긴 노트 하나.
fn project() -> Project {
    ksq::from_str(
        r#"{"tempo": [[0, 500000]], "tracks": [{"name": "Lead", "port": "virtual", "channel": 1,
            "clips": [{"start": 0, "length": 15360, "notes": [[0, 15360, 60, 51492]]}]}]}"#,
    )
    .unwrap()
    .0
}

fn play(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.activate_window();
        window.dispatch_action(Box::new(transport::Play), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn closing_playing_window_stops_transport_in_close_hook(cx: &mut TestAppContext) {
    let lb = start(cx, "close-hook");
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    play(cx, handle);
    assert!(wait_for(&lb.rx, note_on), "Note On 60");
    assert!(VisualTestContext::from_window(handle, cx).simulate_close());
    cx.run_until_parked();
    assert!(
        wait_for(&lb.rx, note_off),
        "닫힌 창의 노트가 걸린 채 남았다"
    );
    drop(view);
}

#[gpui_kit::test]
fn closing_dirty_playing_window_stops_after_dont_save(cx: &mut TestAppContext) {
    let lb = start(cx, "close-dirty");
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.activate_window();
        window.dispatch_action(Box::new(keyscore_app::actions::track::New), cx);
    })
    .unwrap();
    cx.run_until_parked();
    play(cx, handle);
    assert!(wait_for(&lb.rx, note_on), "Note On 60");
    assert!(!VisualTestContext::from_window(handle, cx).simulate_close());
    cx.run_until_parked();
    cx.simulate_prompt_answer("저장 안 함");
    cx.run_until_parked();
    assert!(
        wait_for(&lb.rx, note_off),
        "닫힌 창의 노트가 걸린 채 남았다"
    );
    drop(view);
}
