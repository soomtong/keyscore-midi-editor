//! S3-T4c 미리듣기 가벼운 경로(S3-T2 리뷰 1 제안 9 이월, PRE-1·PRE-3·OUT-4). 정지 중 이름 포트 미리듣기는 포트
//! 구성이 같으면 스냅샷을 다시 준비하지 않고, 뱅크·프로그램은 바뀔 때만 보낸다. 시험 목록은
//! `docs/reports/s3-t4c/plan.md`. 이름 포트는 앱의 추가 가상 포트로 만든다(외부 장치 없이 이름 경로를 탄다).

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::actions::{midi, track, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Edit, OutputPort, Program, Project, TrackProp, ksq};

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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
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

fn set_track_prop(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    index: usize,
    prop: TrackProp,
) {
    view.update(cx, |w, cx| {
        let id = w.document().read(cx).project().tracks()[index].id();
        w.document().update(cx, |d, cx| {
            d.apply(vec![Edit::SetTrackProp { track: id, prop }])
                .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
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

/// 조용해질 때까지(200 ms 동안 아무것도 오지 않을 때까지) 받아 버린다.
fn drain(rx: &mpsc::Receiver<Vec<u8>>) {
    while rx.recv_timeout(Duration::from_millis(200)).is_ok() {}
}

fn note_ons(got: &[Vec<u8>]) -> usize {
    got.iter()
        .filter(|m| m[0] & 0xF0 == 0x90 && m.get(2).copied().unwrap_or(0) > 0)
        .count()
}

fn programs(got: &[Vec<u8>]) -> Vec<Vec<u8>> {
    got.iter()
        .filter(|m| m[0] & 0xF0 == 0xC0)
        .cloned()
        .collect()
}

// ---------- 1–3. 스냅샷 재사용 ----------

#[gpui_kit::test]
fn preview_repeat_on_named_port_prepares_once(cx: &mut TestAppContext) {
    start(cx, "repeat");
    install_engine(cx, "repeat");
    let synth = named_port(cx, &format!("KS light synth {} a", std::process::id()));
    let l = listen(&synth);
    let (handle, _view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "S", "port": {{"named": "{synth}"}}, "channel": 3}}]}}"#
        )),
    );
    let before = prepares(cx);
    press(cx, handle, "c enter d enter e enter");
    assert_eq!(prepares(cx) - before, 1);
    let got = collect(&l.rx, |g| note_ons(g) >= 3);
    assert_eq!(note_ons(&got), 3, "{got:?}");
    assert!(got.iter().all(|m| m[0] & 0x0F == 2), "{got:?}");
}

#[gpui_kit::test]
fn preview_after_track_port_change_prepares_again(cx: &mut TestAppContext) {
    start(cx, "port-change");
    install_engine(cx, "port-change");
    let a = named_port(cx, &format!("KS light synth {} b1", std::process::id()));
    let b = named_port(cx, &format!("KS light synth {} b2", std::process::id()));
    let la = listen(&a);
    let lb = listen(&b);
    let (handle, view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "S", "port": {{"named": "{a}"}}, "channel": 1}}]}}"#
        )),
    );
    let before = prepares(cx);
    press(cx, handle, "c enter");
    let got = collect(&la.rx, |g| note_ons(g) >= 1);
    assert_eq!(note_ons(&got), 1);
    set_track_prop(cx, &view, 0, TrackProp::Port(OutputPort::Named(b.clone())));
    press(cx, handle, "d enter");
    assert_eq!(prepares(cx) - before, 2);
    let got = collect(&lb.rx, |g| note_ons(g) >= 1);
    assert_eq!(note_ons(&got), 1, "{got:?}");
    drain(&la.rx);
}

#[gpui_kit::test]
fn preview_after_play_stop_prepares_again_and_resends_program(cx: &mut TestAppContext) {
    start(cx, "play-stop");
    install_engine(cx, "play-stop");
    let synth = named_port(cx, &format!("KS light synth {} c", std::process::id()));
    let l = listen(&synth);
    let (handle, _view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "S", "port": {{"named": "{synth}"}}, "channel": 2, "program": 9,
                "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 72, 51492]]}}]}}]}}"#
        )),
    );
    let before = prepares(cx);
    press(cx, handle, "c enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(programs(&got), vec![vec![0xC1, 9]], "{got:?}");
    run(cx, handle, transport::Play);
    run(cx, handle, transport::Stop);
    drain(&l.rx);
    press(cx, handle, "d enter");
    assert_eq!(prepares(cx) - before, 2);
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    let program = got.iter().position(|m| m == &[0xC1, 9]);
    let on = got.iter().position(|m| m[0] == 0x91);
    assert!(program.is_some() && program < on, "{got:?}");
}

// ---------- 4–7. 뱅크·프로그램 ----------

#[gpui_kit::test]
fn preview_sends_program_once_while_unchanged(cx: &mut TestAppContext) {
    start(cx, "program-once");
    let port = install_engine(cx, "program-once");
    let l = listen(&port);
    let (handle, _view) = open_step(
        cx,
        project(r#"{"tracks": [{"name": "V", "channel": 5, "program": 7, "bank": 3}]}"#),
    );
    press(cx, handle, "c enter d enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 2);
    assert_eq!(note_ons(&got), 2, "{got:?}");
    assert_eq!(programs(&got), vec![vec![0xC4, 7]], "{got:?}");
    // 뱅크 선택(CC 0·32)도 한 번
    let banks = got
        .iter()
        .filter(|m| m[0] == 0xB4 && (m[1] == 0 || m[1] == 32))
        .count();
    assert_eq!(banks, 2, "{got:?}");
}

#[gpui_kit::test]
fn preview_resends_program_after_track_program_edit(cx: &mut TestAppContext) {
    start(cx, "program-edit");
    let port = install_engine(cx, "program-edit");
    let l = listen(&port);
    let (handle, view) = open_step(
        cx,
        project(r#"{"tracks": [{"name": "V", "channel": 5, "program": 7}]}"#),
    );
    press(cx, handle, "c enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(programs(&got), vec![vec![0xC4, 7]]);
    set_track_prop(
        cx,
        &view,
        0,
        TrackProp::Program(Some(Program::new(11).unwrap())),
    );
    press(cx, handle, "d enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    let program = got.iter().position(|m| m == &[0xC4, 11]);
    let on = got.iter().position(|m| m[0] == 0x94);
    assert!(program.is_some() && program < on, "{got:?}");
}

#[gpui_kit::test]
fn preview_alternating_tracks_on_shared_channel_resend_program(cx: &mut TestAppContext) {
    start(cx, "shared-channel");
    let port = install_engine(cx, "shared-channel");
    let l = listen(&port);
    let (handle, _view) = open_step(
        cx,
        project(
            r#"{"tracks": [{"name": "A", "channel": 1, "program": 1},
                {"name": "B", "channel": 1, "program": 2}]}"#,
        ),
    );
    press(cx, handle, "c enter");
    run(cx, handle, track::Next);
    press(cx, handle, "d enter");
    run(cx, handle, track::Prev);
    press(cx, handle, "e enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 3);
    assert_eq!(note_ons(&got), 3, "{got:?}");
    assert_eq!(
        programs(&got),
        vec![vec![0xC0, 1], vec![0xC0, 2], vec![0xC0, 1]],
        "{got:?}"
    );
}

#[gpui_kit::test]
fn test_note_always_sends_program(cx: &mut TestAppContext) {
    start(cx, "test-note");
    let port = install_engine(cx, "test-note");
    let l = listen(&port);
    let (handle, _view) = open_step(
        cx,
        project(r#"{"tracks": [{"name": "V", "channel": 6, "program": 4}]}"#),
    );
    run(cx, handle, midi::SendTestNote);
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(programs(&got), vec![vec![0xC5, 4]], "{got:?}");
    run(cx, handle, midi::SendTestNote);
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(programs(&got), vec![vec![0xC5, 4]], "{got:?}");
}

// ---------- 8–9. 포트가 생기거나 바뀜 ----------

#[gpui_kit::test]
fn preview_to_missing_port_finds_port_added_later(cx: &mut TestAppContext) {
    start(cx, "added-later");
    install_engine(cx, "added-later");
    let name = format!("KS light synth {} d", std::process::id());
    let (handle, view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "S", "port": {{"named": "{name}"}}, "channel": 1}}]}}"#
        )),
    );
    press(cx, handle, "c enter");
    let said = cx.read(|cx| view.read(cx).messages().last().map(ToString::to_string));
    assert!(said.unwrap_or_default().contains(&name));
    let made = named_port(cx, &name);
    assert_eq!(made, name);
    let l = listen(&name);
    press(cx, handle, "d enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(note_ons(&got), 1, "{got:?}");
}

#[gpui_kit::test]
fn preview_after_virtual_port_rename_follows_new_port(cx: &mut TestAppContext) {
    start(cx, "rename");
    install_engine(cx, "rename");
    let old = named_port(cx, &format!("KS light synth {} e1", std::process::id()));
    let (handle, view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "S", "port": {{"named": "{old}"}}, "channel": 1}}]}}"#
        )),
    );
    press(cx, handle, "c enter");
    let new = format!("KS light synth {} e2", std::process::id());
    let renamed = cx
        .update(|cx| engine::rename_virtual_port(cx, &old, &new))
        .unwrap();
    cx.run_until_parked();
    let port = cx.read(|cx| {
        view.read(cx).document().read(cx).project().tracks()[0]
            .port()
            .clone()
    });
    assert_eq!(port, OutputPort::Named(renamed.clone()));
    let l = listen(&renamed);
    press(cx, handle, "d enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(note_ons(&got), 1, "{got:?}");
}
