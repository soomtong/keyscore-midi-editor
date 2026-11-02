//! S3-T8 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s3-t8/review-1.md`).
//!
//! 스케줄러가 곡 끝에서 스스로 멈춘 뒤, 창의 곡 끝 감시(GPUI 타이머)가 깨기 전에 일시정지하는 경우를 본다.
//! 시험 시계를 돌리지 않으면 감시는 깨지 않으므로 그 틈을 그대로 재현할 수 있다. 소리는 실제 시간으로 흐른다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

struct Loopback {
    _rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-review-s3t8-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = format!("KS review s3t8 {} {name}", std::process::id());
    let options = EngineOptions {
        client_name: format!("ks-review-s3t8-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-review-s3t8-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
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
        _rx: rx,
        _input: input,
        _client: client,
    }
}

/// 120 BPM(960틱 = 0.5 s). 클립 하나 `[0, 960)`.
fn project() -> Project {
    let text = r#"{"tempo": [[0, 500000]], "tracks": [{"name": "Lead", "port": "virtual", "channel": 1,
        "clips": [{"start": 0, "length": 960, "notes": [[0, 240, 60, 51492]]}]}]}"#;
    ksq::from_str(text).unwrap().0
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

/// Normal 모드(곡 끝에서 멈춤). 스케줄러가 곡 끝(0.5 s)에서 멈춘 뒤, 창이 그것을 알기 전(곡 끝 감시 주기
/// 20 ms 안)에 일시정지한다. 스케줄러는 「재생 중이 아니었다」고 답하지만(`pause` → `None`) 창은 시간 초과와
/// 구별하지 못해 곡 끝을 지난 추정 자리로 일시정지한다(리뷰 1 제안 3, 기존 동작과 같다). 그 경우 이어서
/// 재생하면 곧바로 정지 상태(틱 0)로 돌아오는지 본다.
#[gpui_kit::test]
fn pause_after_scheduler_song_end_pauses_past_end_then_play_stops(cx: &mut TestAppContext) {
    let _lb = start(cx, "end");
    let (handle, view) = open(cx, project());
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_millis(800));
    run(cx, handle, transport::Pause);
    let state = cx.read(|cx| view.read(cx).transport_state());
    // 지금은 곡 끝 뒤 추정 자리로 일시정지한다. 제안대로 고치면 정지 상태가 된다. 어느 쪽이든 아래가 성립해야 한다.
    if state == TransportState::Stopped {
        return;
    }
    assert!(
        matches!(state, TransportState::Paused(_)),
        "일시정지도 정지도 아니다: {state:?}"
    );
    run(cx, handle, transport::Play);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        cx.executor().advance_clock(engine::END_POLL);
        cx.run_until_parked();
        let state = cx.read(|cx| view.read(cx).transport_state());
        if state == TransportState::Stopped {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "정지로 돌아오지 않는다: {state:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
