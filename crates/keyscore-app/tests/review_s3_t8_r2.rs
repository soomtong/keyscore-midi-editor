//! S3-T8 적대적 리뷰 2회차의 깨뜨리기 시험(`docs/reports/s3-t8/review-2.md`).
//!
//! 일시정지가 `QueueFull`을 받은 뒤의 창 상태·주인·소리를 본다. 구현자는 앱 시험에서 큐를 채울 길이 없다고
//! 적었다(보고서 리뷰 대응 4). 공개 함수 `engine::send_port_test_note`는 재생 중에도 미리듣기 명령 하나를 넣으므로,
//! 한 번의 창 갱신 안에서 스케줄러가 비우는 속도(1 ms에 64개)보다 빨리 거듭 불러 큐를 채운다(메트로놈 토글
//! Action은 디버그 빌드에서 그보다 느려 채우지 못했다).

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
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

const FIRST: u8 = 40;
/// 박 음 번호는 40–55. 테스트 노트(가운데 C, 60)와 겹치지 않는다.
const BEATS: u8 = 16;
const TEST_NOTE: u8 = 60;

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-review2-s3t8-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = format!("KS review2 s3t8 {} {name}", std::process::id());
    let options = EngineOptions {
        client_name: format!("ks-review2-s3t8-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-review2-s3t8-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let d = p.data();
                let mut i = 0;
                while i < d.len() {
                    let len = match d[i] & 0xF0 {
                        0xC0 | 0xD0 => 2,
                        0xF0 => 1,
                        _ => 3,
                    };
                    let _ = tx.send(d[i..(i + len).min(d.len())].to_vec());
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

/// 120 BPM, 박(480틱 = 250 ms)마다 음 번호가 다른 짧은 노트.
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
            "clips": [{{"start": 0, "length": 7680, "notes": [{}]}}]}}]}}"#,
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

/// `limit` 동안 받은 Note On의 음 번호.
fn heard_for(rx: &mpsc::Receiver<Vec<u8>>, limit: Duration) -> Vec<u8> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(m) if m.len() == 3 && m[0] & 0xF0 == 0x90 && m[2] > 0 && m[1] != TEST_NOTE => {
                got.push(m[1]);
            }
            Ok(_) => {}
            Err(_) => return got,
        }
    }
}

/// 큐가 가득일 때 일시정지하면 창은 재생 상태·엔진 주인으로 남고 소리는 계속된다. 큐가 빈 뒤의 일시정지는
/// 소리를 멈추고 들은 박 뒤에 자리를 잡으며, 이어서 재생하면 겹치지도 빠지지도 않는다.
#[gpui_kit::test]
fn pause_with_full_queue_keeps_playing_and_a_later_pause_works(cx: &mut TestAppContext) {
    let lb = start(cx, "full");
    let (handle, view) = open(cx, beats());
    run(cx, handle, transport::Play);
    let mut heard = heard_for(&lb.rx, Duration::from_millis(100));
    assert_eq!(heard, vec![FIRST], "첫 박");
    let port = cx.update(|cx| engine::virtual_port_name(cx));
    // 한 번의 갱신 안에서 테스트 노트(명령 하나씩)를 쏟아 큐를 채우고, 곧바로 일시정지한다.
    // `dispatch_action`은 갱신이 끝날 때 처리된다(지연). 일시정지는 갱신 직후, 큐가 다시 빌 틈 없이 돈다.
    let (sent, fill_error) = cx
        .update_window(handle, |_, window, cx| {
            let mut sent = 0;
            let mut fill_error = None;
            for _ in 0..100_000 {
                match engine::send_port_test_note(cx, &port) {
                    Ok(()) => sent += 1,
                    Err(e) => {
                        fill_error = Some(e);
                        break;
                    }
                }
            }
            window.dispatch_action(Box::new(transport::Pause), cx);
            (sent, fill_error)
        })
        .unwrap();
    let (state, errors_after) = cx.read(|cx| {
        let w = view.read(cx);
        let n = w
            .messages()
            .iter()
            .filter(|m| m.starts_with("MIDI 오류"))
            .count();
        (w.transport_state(), n)
    });
    cx.run_until_parked();
    assert!(
        fill_error.is_some(),
        "큐를 채우지 못했다(테스트 노트 {sent}개)"
    );
    assert_eq!(
        errors_after, 1,
        "일시정지가 QueueFull을 받지 않았다({fill_error:?}, {sent}개)"
    );
    assert_eq!(
        state,
        TransportState::Playing,
        "큐가 가득인데 재생 상태를 떠났다"
    );
    // 소리는 계속되고, 창은 여전히 주인이라 다음 일시정지가 멈춘다.
    heard.extend(heard_for(&lb.rx, Duration::from_millis(400)));
    assert!(heard.len() >= 2, "QueueFull 뒤 재생이 멈췄다: {heard:?}");
    run(cx, handle, transport::Pause);
    let at = match cx.read(|cx| view.read(cx).transport_state()) {
        TransportState::Paused(at) => at.get(),
        other => panic!("일시정지가 아니다: {other:?}"),
    };
    heard.extend(heard_for(&lb.rx, Duration::from_millis(40)));
    let last_tick = u64::from(heard.last().unwrap() - FIRST) * 480;
    assert!(at > last_tick, "자리 {at}가 들은 박(틱 {last_tick}) 앞이다");
    assert!(
        heard_for(&lb.rx, Duration::from_millis(400)).is_empty(),
        "일시정지 뒤에도 소리가 난다"
    );
    run(cx, handle, transport::Play);
    heard.extend(heard_for(&lb.rx, Duration::from_millis(600)));
    run(cx, handle, transport::Stop);
    let n = u8::try_from(heard.len()).unwrap();
    assert_eq!(
        heard,
        (FIRST..FIRST + n).collect::<Vec<u8>>(),
        "겹치거나 빠졌다"
    );
}
