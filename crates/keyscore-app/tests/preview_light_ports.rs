//! S3-T4c 미리듣기 가벼운 경로의 무효화(리뷰 1 제안 4). 가상 포트는 프로세스마다 16개가 상한이라
//! (`MAX_VIRTUAL_OUTPUTS`) `preview_light.rs`와 프로세스를 나눈다. 도우미는 그 파일과 같다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-preview-light-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

/// 엔진을 띄우고 기본 가상 포트 이름을 돌려준다.
fn install_engine(cx: &mut TestAppContext, tag: &str) -> String {
    let port = format!("KS preview light {} {tag}", std::process::id());
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-preview-light-{tag}"),
                port_name: port.clone(),
            },
        )
    })
    .unwrap();
    port
}

/// 앱의 추가 가상 포트(이름 포트)를 만든다.
fn named_port(cx: &mut TestAppContext, name: &str) -> String {
    cx.update(|cx| engine::add_named_virtual_port(cx, name))
        .unwrap()
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(handle, key);
        cx.run_until_parked();
        render(cx, handle);
    }
}

fn event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    e: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| this.apply_mode_event(e, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

/// 피아노롤 Step 모드 창(계이름 레이아웃).
fn open_step(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = KeyLayout::Solfege));
    cx.run_until_parked();
    render(cx, handle);
    event(cx, handle, &view, ModeEvent::ToggleView);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

fn project(json: &str) -> Project {
    ksq::from_str(json).unwrap().0
}

fn prepares(cx: &mut TestAppContext) -> u64 {
    cx.read(engine::preview_prepares)
}

// ---------- 듣기 ----------

struct Listener {
    rx: mpsc::Receiver<Vec<u8>>,
    _port: coremidi::InputPort,
    _client: coremidi::Client,
}

fn listen(name: &str) -> Listener {
    let deadline = Instant::now() + Duration::from_secs(2);
    let source = loop {
        if let Some(s) = coremidi::Source::from_name(name) {
            break s;
        }
        assert!(
            Instant::now() < deadline,
            "가상 포트 「{name}」이 보이지 않는다"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let client = coremidi::Client::new("ks-preview-light-listen").unwrap();
    let (tx, rx) = mpsc::channel();
    let port = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
            }
        })
        .unwrap();
    port.connect_source(&source).unwrap();
    Listener {
        rx,
        _port: port,
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

/// `until`이 참이 되거나 2초가 지날 때까지 모은다.
fn collect(rx: &mpsc::Receiver<Vec<u8>>, until: impl Fn(&[Vec<u8>]) -> bool) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut got = Vec::new();
    while Instant::now() < deadline && !until(&got) {
        if let Ok(m) = rx.recv_timeout(Duration::from_millis(20)) {
            got.extend(split(&m));
        }
    }
    got
}

fn note_ons(got: &[Vec<u8>]) -> usize {
    got.iter()
        .filter(|m| m[0] & 0xF0 == 0x90 && m.get(2).copied().unwrap_or(0) > 0)
        .count()
}

/// 미리듣기 뒤 그 가상 포트를 지우고 같은 이름으로 다시 만들면, 기억한 경로표(지운 스냅샷의 번호)를 쓰지 않고
/// 새 포트로 보낸다(리뷰 1 제안 4).
#[gpui_kit::test]
fn preview_after_port_removed_and_recreated_reaches_new_port(cx: &mut TestAppContext) {
    start(cx, "remove-recreate");
    install_engine(cx, "remove-recreate");
    let name = named_port(cx, &format!("KS light synth {} f", std::process::id()));
    let (handle, _view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "S", "port": {{"named": "{name}"}}, "channel": 1}}]}}"#
        )),
    );
    press(cx, handle, "c enter");
    let before = prepares(cx);
    cx.update(|cx| engine::remove_virtual_port(cx, &name))
        .unwrap();
    cx.run_until_parked();
    let again = named_port(cx, &name);
    assert_eq!(again, name);
    let l = listen(&name);
    press(cx, handle, "d enter");
    assert_eq!(prepares(cx) - before, 1);
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(note_ons(&got), 1, "{got:?}");
}
