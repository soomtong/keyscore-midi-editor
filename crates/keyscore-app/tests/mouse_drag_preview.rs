//! S3-T6b 드래그 중 미리듣기(REQ 7.4 MS-4, PRE-1). 결정 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md`
//! 6절 12: 본체 이동에서 고스트 음높이가 바뀔 때마다 한 번, 설정 「이동 시 미리듣기」를 따른다. CoreMIDI 루프백.
#![allow(clippy::cast_possible_truncation)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Entity, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::pianoroll::Viewport;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-drag-preview-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

/// 엔진을 띄우고 기본 가상 포트 이름을 돌려준다.
fn install_engine(cx: &mut TestAppContext, tag: &str) -> String {
    let port = format!("KS drag preview {} {tag}", std::process::id());
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-drag-preview-{tag}"),
                port_name: port.clone(),
            },
        )
    })
    .unwrap();
    port
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn project() -> Project {
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 7680).unwrap();
    clip.insert_note(
        Note::new(
            Tick::new(960),
            960,
            Pitch::new(60).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    track.insert_clip(clip).unwrap();
    project.push_track(track).unwrap();
    project
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    cx.simulate_keystrokes(handle, "enter");
    cx.run_until_parked();
    cx.update(|cx| view.read(cx).piano_roll().zoom(0.5));
    render(cx, handle);
    (handle, view)
}

fn spot(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    tick: u64,
    pitch: u8,
) -> Point<Pixels> {
    let vp = cx.read(|cx| view.read(cx).piano_roll().viewport());
    #[allow(clippy::cast_precision_loss)]
    let x = ((tick as f64 - vp.first_tick) * vp.px_per_tick) as f32;
    let row = Viewport::row_of(Pitch::new(pitch).unwrap());
    let y = (row - vp.first_row + 0.5) * vp.row_height;
    let origin = cx
        .update_window(handle, |_, window, _| window.find("main-view").bounds())
        .unwrap()
        .origin;
    origin + point(px(x), px(y))
}

fn down(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn drag_to(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn up(cx: &mut TestAppContext, handle: AnyWindowHandle, at: Point<Pixels>) {
    VisualTestContext::from_window(handle, cx).simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
    cx.run_until_parked();
    render(cx, handle);
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
    let client = coremidi::Client::new("ks-drag-preview-listen").unwrap();
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

/// 조용해질 때까지(300 ms 동안 아무것도 오지 않을 때까지) 받아 노트 온의 음높이를 돌려준다.
fn note_ons(rx: &mpsc::Receiver<Vec<u8>>) -> Vec<u8> {
    let mut got = Vec::new();
    while let Ok(m) = rx.recv_timeout(Duration::from_millis(300)) {
        got.extend(split(&m));
    }
    got.iter()
        .filter(|m| m[0] & 0xF0 == 0x90 && m.get(2).copied().unwrap_or(0) > 0)
        .map(|m| m[1])
        .collect()
}

#[gpui_kit::test]
fn body_drag_previews_each_new_pitch_once(cx: &mut TestAppContext) {
    start(cx, "pitch");
    let port = install_engine(cx, "pitch");
    let l = listen(&port);
    let (handle, view) = open(cx);
    let from = spot(cx, handle, &view, 1440, 60);
    down(cx, handle, from);
    // 누름의 커서 이동 미리듣기(PRE-1)는 따로 센다
    let _ = note_ons(&l.rx);
    let at = |cx: &mut TestAppContext, dt: i64, dp: i32| {
        let vp = cx.read(|cx| view.read(cx).piano_roll().viewport());
        #[allow(clippy::cast_precision_loss)]
        let dx = (dt as f64 * vp.px_per_tick) as f32;
        #[allow(clippy::cast_precision_loss)]
        let dy = -(dp as f32) * vp.row_height;
        from + point(px(dx), px(dy))
    };
    let p = at(cx, 480, 0);
    drag_to(cx, handle, p);
    assert_eq!(
        note_ons(&l.rx),
        Vec::<u8>::new(),
        "음높이가 그대로면 보내지 않는다"
    );
    let p = at(cx, 480, 1);
    drag_to(cx, handle, p);
    assert_eq!(note_ons(&l.rx), vec![61]);
    let p = at(cx, 960, 1);
    drag_to(cx, handle, p);
    assert_eq!(note_ons(&l.rx), Vec::<u8>::new());
    let p = at(cx, 960, 2);
    drag_to(cx, handle, p);
    let p = at(cx, 960, 1);
    drag_to(cx, handle, p);
    assert_eq!(note_ons(&l.rx), vec![62, 61]);
    up(cx, handle, p);
    let _ = note_ons(&l.rx);

    // 설정을 끄면 보내지 않는다
    cx.update(|cx| AppSettings::update(cx, |s| s.preview.on_move = false));
    let from = spot(cx, handle, &view, 1440 + 960, 61);
    down(cx, handle, from);
    let p = from + point(px(0.0), px(-40.0));
    drag_to(cx, handle, p);
    up(cx, handle, p);
    assert_eq!(note_ons(&l.rx), Vec::<u8>::new());
}
