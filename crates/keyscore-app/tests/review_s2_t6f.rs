//! S2-T6f 적대적 리뷰 1회차. 창 크기 변경 때 패널 크기 상태를 새로 만드는 수정, undo 상한 알림, 바탕 레이어
//! 디버그 키, 오버레이 인자 라벨의 새 매칭을 깨뜨리려는 시도.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext, px, size};
use keyscore_app::actions::view::ToggleTrackList;
use keyscore_app::actions::{edit, help, mode, track};
use keyscore_app::app_state::AppSettings;
use keyscore_app::settings::{LayoutSettings, Settings};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window, open_project_window_with_history_limit};
use keyscore_core::ksq;

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t6f-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start_with(
    cx: &mut TestAppContext,
    name: &str,
    user_keymap: Option<&str>,
    edit: impl FnOnce(&mut LayoutSettings),
) {
    let dir = dir(name);
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    let path = dir.join("settings.json");
    let mut s = Settings::default();
    edit(&mut s.layout);
    s.save(&path).unwrap();
    cx.update(|cx| init(cx, path));
}

fn one_track() -> keyscore_core::Project {
    ksq::from_str(r#"{"tracks": [{"name": "A", "channel": 1}]}"#)
        .unwrap()
        .0
}

fn draw(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx
        .update(|cx| open_project_window(one_track(), cx))
        .unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    draw(cx, handle);
    (handle, view)
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl gpui_kit::Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    draw(cx, handle);
}

fn region(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> (f32, f32) {
    cx.update_window(handle, |_, window, _| {
        let b = window.find(id).bounds();
        (f32::from(b.origin.x), f32::from(b.size.width))
    })
    .unwrap()
}

fn resize_panel(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    vertical: bool,
    index: usize,
    to: f32,
) {
    let state = cx.read(|cx| {
        if vertical {
            view.read(cx).vertical_state().clone()
        } else {
            view.read(cx).horizontal_state().clone()
        }
    });
    cx.update_window(handle, |_, window, cx| {
        state.update(cx, |s, cx| s.resize_panel(index, px(to), window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    draw(cx, handle);
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 2.0
}

fn messages(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<String> {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .iter()
            .map(ToString::to_string)
            .collect()
    })
}

const NOTICE: &str = "되돌리기 기록이 메모리 상한";

fn notices(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    messages(cx, view)
        .iter()
        .filter(|m| m.contains(NOTICE))
        .count()
}

fn dropped(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> u64 {
    cx.read(|cx| view.read(cx).document().read(cx).dropped_undo_units())
}

// ---------- 패널 크기: 창 크기 변경 뒤 상태 새로 만들기 ----------

/// 저장된 사이드 패널 너비의 합이 창보다 크면(넓은 창에서 늘려 둔 패널, 창을 줄임) 인스펙터만 조절해도 트랙
/// 목록의 저장 너비는 그대로여야 한다.
#[gpui_kit::test]
fn overflowing_panels_inspector_resize_keeps_track_list_width(cx: &mut TestAppContext) {
    start_with(cx, "overflow-inspector", None, |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1600.0), px(900.0)));
    draw(cx, handle);
    cx.simulate_window_resize(handle, size(px(900.0), px(600.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, false, 2, 450.0);
    let layout = cx.read(|cx| view.read(cx).layout());
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(
        close(layout.track_list_width, 500.0) && close(saved.track_list_width, 500.0),
        "창 {layout:?}, 설정 {saved:?}"
    );
}

/// 창을 저장된 패널 합보다 좁게 줄여도 인스펙터가 창 밖으로 밀려나지 않는다(이 Task 전에는 비율로 줄여 창
/// 안에 들어왔다).
#[gpui_kit::test]
fn shrinking_window_keeps_inspector_inside_window(cx: &mut TestAppContext) {
    start_with(cx, "overflow-visible", None, |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (handle, _view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1600.0), px(900.0)));
    draw(cx, handle);
    cx.simulate_window_resize(handle, size(px(900.0), px(600.0)));
    draw(cx, handle);
    let (x, w) = region(cx, handle, "inspector");
    assert!(
        x + w <= 900.0 + 1.0,
        "인스펙터가 창 밖으로 나갔다: 왼쪽 {x}, 너비 {w}, 창 900"
    );
}

/// 트랙 목록을 접은 채 창 크기를 바꾸고 다시 펴면 저장된 너비로 펴진다.
#[gpui_kit::test]
fn collapsed_track_list_survives_window_resize(cx: &mut TestAppContext) {
    start_with(cx, "collapsed", None, |_| {});
    let (handle, view) = open(cx);
    run(cx, handle, ToggleTrackList);
    cx.simulate_window_resize(handle, size(px(1100.0), px(700.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, false, 1, 300.0);
    run(cx, handle, ToggleTrackList);
    let (_, w) = region(cx, handle, "track-list");
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(
        close(w, 220.0) && close(saved.track_list_width, 220.0),
        "그린 {w}, 설정 {saved:?}"
    );
}

/// 창 높이를 바꾼 뒤 아래 패널을 조절해도 사이드 패널 저장 너비는 그대로, 이어 트랙 목록을 조절해도 아래 패널
/// 저장 높이는 그대로다.
#[gpui_kit::test]
fn vertical_then_horizontal_after_window_resize(cx: &mut TestAppContext) {
    start_with(cx, "vertical", None, |_| {});
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1000.0), px(900.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, true, 0, 600.0);
    let after_bottom = cx.read(|cx| AppSettings::get(cx).layout);
    cx.simulate_window_resize(handle, size(px(1200.0), px(650.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, false, 0, 260.0);
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(
        close(saved.bottom_height, after_bottom.bottom_height)
            && close(saved.inspector_width, 260.0)
            && close(saved.track_list_width, 260.0),
        "아래 조절 뒤 {after_bottom:?}, 끝 {saved:?}"
    );
}

/// 두 창: 창 B에서 트랙 목록을 바꾸고, 창 A의 크기를 바꾼 뒤 A의 인스펙터를 조절해도 B가 남긴 트랙 목록
/// 너비가 되돌아가지 않는다.
#[gpui_kit::test]
fn window_resize_in_one_window_keeps_other_windows_width(cx: &mut TestAppContext) {
    start_with(cx, "two-windows", None, |_| {});
    let (ha, a) = open(cx);
    let (hb, b) = open(cx);
    resize_panel(cx, hb, &b, false, 0, 320.0);
    cx.simulate_window_resize(ha, size(px(1000.0), px(700.0)));
    draw(cx, ha);
    resize_panel(cx, ha, &a, false, 2, 200.0);
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    let la = cx.read(|cx| a.read(cx).layout());
    assert!(
        close(saved.track_list_width, 320.0)
            && close(saved.inspector_width, 200.0)
            && close(la.track_list_width, 220.0),
        "설정 {saved:?}, 창 A {la:?}"
    );
}

// ---------- undo 상한 알림 ----------

/// 편집으로는 버리지 않고 되돌리기(redo 쪽이 커짐)로만 처음 버려도 알린다. 그 뒤 다시 실행·편집으로 더 버려도
/// 한 번뿐이다.
#[gpui_kit::test]
fn undo_only_drop_notices_once(cx: &mut TestAppContext) {
    start_with(cx, "undo-drop", None, |_| {});
    let (h, view) = cx
        .update(|cx| open_project_window_with_history_limit(one_track(), 1500, cx))
        .unwrap();
    for i in 0..4 {
        run(cx, h, track::Rename(format!("n{i}")));
    }
    run(cx, h, track::Rename("x".repeat(3000)));
    assert_eq!(
        dropped(cx, &view),
        0,
        "편집으로 이미 버렸다(시험 설정 잘못)"
    );
    assert_eq!(notices(cx, &view), 0);
    run(cx, h, edit::Undo);
    assert!(dropped(cx, &view) > 0, "되돌리기로 버리지 않았다");
    assert_eq!(notices(cx, &view), 1, "{:?}", messages(cx, &view));
    // 상태줄은 마지막 메시지를 보인다. 알림이 바로 다른 메시지에 덮이지 않았는지
    let last = messages(cx, &view).last().cloned().unwrap_or_default();
    assert!(last.contains(NOTICE), "마지막 메시지: {last}");
    run(cx, h, edit::Redo);
    for i in 0..10 {
        run(cx, h, track::Rename(format!("{i:0>200}")));
    }
    assert_eq!(notices(cx, &view), 1, "{:?}", messages(cx, &view));
}

/// 알림은 창마다 한 번이다. 한 창이 버려도 다른 창은 알리지 않고, 그 창이 버릴 때 따로 한 번 알린다.
#[gpui_kit::test]
fn undo_limit_notice_is_per_window(cx: &mut TestAppContext) {
    start_with(cx, "undo-two", None, |_| {});
    let (ha, a) = cx
        .update(|cx| open_project_window_with_history_limit(one_track(), 1500, cx))
        .unwrap();
    let (hb, b) = cx
        .update(|cx| open_project_window_with_history_limit(one_track(), 1500, cx))
        .unwrap();
    for i in 0..30 {
        run(cx, ha, track::Rename(format!("{i:0>100}")));
    }
    assert_eq!((notices(cx, &a), notices(cx, &b)), (1, 0));
    for i in 0..30 {
        run(cx, hb, track::Rename(format!("{i:0>100}")));
    }
    assert_eq!((notices(cx, &a), notices(cx, &b)), (1, 1));
}

// ---------- 바탕 레이어 디버그 키 ----------

/// 편집기가 「충돌 없음」으로 보는 `alt-cmd-i`를 사용자가 잡으면 그 키가 실제로 사용자 Action을 부른다(바탕
/// 레이어의 디버그 키가 가로채지 않는다). 오버레이에도 그 키가 보인다.
#[gpui_kit::test]
fn user_binding_on_debug_key_wins_over_base_layer(cx: &mut TestAppContext) {
    start_with(
        cx,
        "debug-key-user",
        Some(r#"[{"bindings": {"alt-cmd-i": "track::ToggleMute"}}]"#),
        |_| {},
    );
    let (h, view) = open(cx);
    cx.simulate_keystrokes(h, "alt-cmd-i");
    cx.run_until_parked();
    draw(cx, h);
    let muted = cx.read(|cx| view.read(cx).document().read(cx).project().tracks()[0].mute());
    assert!(muted, "alt-cmd-i가 사용자 바인딩(뮤트)을 부르지 않았다");
    run(cx, h, help::KeymapOverlay);
    let lines = cx.read(|cx| view.read(cx).overlay_lines(cx)).unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("alt-cmd-i") && l.contains("track::ToggleMute")),
        "{lines:#?}"
    );
}

// ---------- 오버레이 인자 라벨 ----------

/// 같은 Action·같은 인자 값이 여러 키에 표기만 다르게(`2`, `"02"`) 걸리면 한 줄로 묶이고 두 키가 다 보인다.
#[gpui_kit::test]
fn same_argument_different_spelling_groups_into_one_line(cx: &mut TestAppContext) {
    start_with(
        cx,
        "same-arg",
        Some(
            r#"[{"bindings": {"f1": "play::SelectTrack(2)", "f2": ["play::SelectTrack", "02"]}}]"#,
        ),
        |_| {},
    );
    let (h, view) = open(cx);
    run(cx, h, mode::Play);
    run(cx, h, help::KeymapOverlay);
    let lines = cx.read(|cx| view.read(cx).overlay_lines(cx)).unwrap();
    let mine: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("f1") || l.contains("f2"))
        .collect();
    assert!(
        mine.len() == 1 && mine[0].contains("f1") && mine[0].contains("f2"),
        "{mine:#?}"
    );
}

// ---------- 포트: 앱 포트 이름의 트랙(windows_using 변경) ----------

mod ports_isolated {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
    use keyscore_app::actions::transport;
    use keyscore_app::engine::{self, EngineOptions};
    use keyscore_app::window::ProjectWindow;
    use keyscore_app::{init, open_project_window, ports};
    use keyscore_core::{Project, Velocity, ksq};

    use super::{dir, messages, run};

    const WAIT: Duration = Duration::from_secs(2);

    fn unique(name: &str) -> String {
        format!("KS rv6f {} {name}", std::process::id())
    }

    fn start(cx: &mut TestAppContext, tag: &str, settings_json: &str) {
        let dir = dir(tag);
        std::fs::write(dir.join("settings.json"), settings_json).unwrap();
        cx.update(|cx| init(cx, dir.join("settings.json")));
        let options = EngineOptions {
            client_name: format!("ks-rv6f-{tag}"),
            port_name: unique(&format!("main {tag}")),
        };
        cx.update(|cx| engine::install(cx, &options)).unwrap();
        cx.update(ports::start_monitor);
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            cx.executor().advance_clock(ports::POLL_INTERVAL);
            cx.run_until_parked();
            if cx.read(ports::is_monitoring) {
                return;
            }
            assert!(Instant::now() < deadline, "감시가 시작되지 않았다");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn project_on(name: &str) -> Project {
        let v = Velocity::from_midi1(100).unwrap().get();
        let notes: Vec<String> = (0..128)
            .map(|i| format!("[{}, 240, 60, {v}]", if i == 0 { 0 } else { 960 }))
            .collect();
        ksq::from_str(&format!(
            r#"{{"tempo": [[0, 500000]], "tracks": [{{"name": "T", "port": {{"named": "{name}"}}, "channel": 1,
                "clips": [{{"start": 0, "length": 122880, "notes": [{}]}}]}}]}}"#,
            notes.join(", ")
        ))
        .unwrap()
        .0
    }

    fn said(
        cx: &mut TestAppContext,
        view: &Entity<ProjectWindow>,
        name: &str,
        word: &str,
    ) -> usize {
        messages(cx, view)
            .iter()
            .filter(|m| m.contains(name) && m.contains(word))
            .count()
    }

    fn pump_for(cx: &mut TestAppContext, handle: AnyWindowHandle, limit: Duration) {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            cx.executor().advance_clock(ports::POLL_INTERVAL);
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                .unwrap();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn settle(cx: &mut TestAppContext) {
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            cx.executor().advance_clock(ports::POLL_INTERVAL);
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_gone(name: &str) {
        let deadline = Instant::now() + WAIT;
        while keyscore_midi::destinations()
            .iter()
            .any(|d| d.name() == name)
        {
            assert!(Instant::now() < deadline, "목적지 {name}가 사라지지 않는다");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn drain(rx: &mpsc::Receiver<Vec<u8>>, limit: Duration) -> Vec<Vec<u8>> {
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

    fn note_ons(got: &[Vec<u8>]) -> usize {
        got.iter()
            .filter(|m| m.first().is_some_and(|s| s & 0xF0 == 0x90))
            .count()
    }

    fn packets(list: &coremidi::PacketList, tx: &mpsc::Sender<Vec<u8>>) {
        for p in list {
            let _ = tx.send(p.data().to_vec());
        }
    }

    struct Receiver {
        rx: mpsc::Receiver<Vec<u8>>,
        _input: coremidi::InputPort,
        _client: coremidi::Client,
    }

    impl Receiver {
        fn source(name: &str) -> Receiver {
            let (tx, rx) = mpsc::channel();
            let client = coremidi::Client::new("ks-rv6f-rx").unwrap();
            let input = client
                .input_port("in", move |list: &coremidi::PacketList| packets(list, &tx))
                .unwrap();
            let deadline = Instant::now() + WAIT;
            let source = loop {
                if let Some(s) = coremidi::Source::from_name(name) {
                    break s;
                }
                assert!(Instant::now() < deadline, "소스 {name}이 보이지 않는다");
                std::thread::sleep(Duration::from_millis(10));
            };
            input.connect_source(&source).unwrap();
            Receiver {
                rx,
                _input: input,
                _client: client,
            }
        }
    }

    struct Destination {
        rx: mpsc::Receiver<Vec<u8>>,
        _dest: coremidi::VirtualDestination,
        _client: coremidi::Client,
    }

    impl Destination {
        fn new(name: &str) -> Destination {
            let (tx, rx) = mpsc::channel();
            let client = coremidi::Client::new(&format!("ks-rv6f-dest-{name}")).unwrap();
            let dest = client
                .virtual_destination(name, move |list: &coremidi::PacketList| {
                    packets(list, &tx);
                })
                .unwrap();
            let deadline = Instant::now() + WAIT;
            while !keyscore_midi::destinations()
                .iter()
                .any(|d| d.name() == name)
            {
                assert!(Instant::now() < deadline, "목적지 {name}이 보이지 않는다");
                std::thread::sleep(Duration::from_millis(10));
            }
            Destination {
                rx,
                _dest: dest,
                _client: client,
            }
        }
    }

    fn run_isolated(test: &str) -> bool {
        const ENV: &str = "KS_APP_ISOLATED_TEST";
        if std::env::var(ENV).as_deref() == Ok(test) {
            return false;
        }
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(ENV, test)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && stdout.contains("test result: ok. 1 passed"),
            "{test}: 자식 프로세스에서 실패했다\n{stdout}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        true
    }

    /// 트랙이 목적지 Y를 쓰며 재생 중에 Y가 사라지고, 앱 추가 포트의 이름을 Y로 바꾸면 노트는 앱 포트 Y로
    /// 간다. 그 뒤 목적지 Y가 돌아와도(감시는 Y를 기억한다) 앱 포트가 이기므로 노트는 목적지로 가지 않고,
    /// 「다시 연결했다」 대신 「새로 보인다」가 한 번 나온다. `windows_using`이 앱 포트 이름이면 빈 목록을 돌려
    /// 재연결 처리(`transport_port_returned`)를 건너뛰어도 소리가 끊기지 않는지 본다.
    #[gpui_kit::test]
    fn track_on_lost_destination_then_own_port_takes_name(cx: &mut TestAppContext) {
        if run_isolated("ports_isolated::track_on_lost_destination_then_own_port_takes_name") {
            return;
        }
        let y = unique("track-takeover");
        let z = unique("track-takeover-own");
        start(
            cx,
            "track-takeover",
            &format!(r#"{{"midi_output": {{"virtual_ports": ["{z}"]}}}}"#),
        );
        let dest = Destination::new(&y);
        settle(cx);
        let (h, view) = cx
            .update(|cx| open_project_window(project_on(&y), cx))
            .unwrap();
        cx.update_window(h, |_, window, _| window.activate_window())
            .unwrap();
        run(cx, h, transport::Play);
        pump_for(cx, h, Duration::from_millis(600));
        assert!(note_ons(&drain(&dest.rx, Duration::from_millis(600))) > 0);
        drop(dest);
        wait_gone(&y);
        pump_for(cx, h, Duration::from_secs(1));
        assert_eq!(
            said(cx, &view, &y, "사라졌다"),
            1,
            "{:?}",
            messages(cx, &view)
        );
        let renamed = cx
            .update(|cx| engine::rename_virtual_port(cx, &z, &y))
            .unwrap();
        assert_eq!(renamed, y);
        let own = Receiver::source(&y);
        pump_for(cx, h, Duration::from_millis(600));
        assert!(
            note_ons(&drain(&own.rx, Duration::from_millis(600))) > 0,
            "이름을 바꾼 앱 포트로 노트가 가지 않는다: {:?}",
            messages(cx, &view)
        );
        let back = Destination::new(&y);
        pump_for(cx, h, Duration::from_secs(1));
        let _ = drain(&back.rx, Duration::from_millis(50));
        let _ = drain(&own.rx, Duration::from_millis(50));
        pump_for(cx, h, Duration::from_millis(600));
        let to_back = drain(&back.rx, Duration::from_millis(500));
        let to_own = drain(&own.rx, Duration::from_millis(500));
        run(cx, h, transport::Stop);
        let all = messages(cx, &view);
        assert_eq!(
            note_ons(&to_back),
            0,
            "앱 포트와 같은 이름 목적지로 노트가 갔다"
        );
        assert!(
            note_ons(&to_own) > 0,
            "목적지가 돌아온 뒤 앱 포트 노트가 끊겼다"
        );
        assert_eq!(said(cx, &view, &y, "다시 연결했다"), 0, "{all:?}");
        assert_eq!(said(cx, &view, &y, "새로 보인다"), 1, "{all:?}");
    }
}

// ---------- 창 최소 크기 ----------

/// 최소 크기 창(720×480)에서 기본 크기 세 패널이 저장 크기로 그려지고 메인 영역이 남는다.
#[gpui_kit::test]
fn min_size_window_fits_default_panels(cx: &mut TestAppContext) {
    start_with(cx, "min-fit", None, |_| {});
    let (handle, _view) = open(cx);
    let min = keyscore_app::PROJECT_WINDOW_MIN_SIZE;
    cx.simulate_window_resize(handle, min);
    draw(cx, handle);
    let (_, track) = region(cx, handle, "track-list");
    let (ix, iw) = region(cx, handle, "inspector");
    let (_, main_width) = region(cx, handle, "main-view");
    let height = cx
        .update_window(handle, |_, window, _| {
            f32::from(window.find("bottom-panel").bounds().size.height)
        })
        .unwrap();
    assert!(
        close(track, 220.0)
            && close(iw, 260.0)
            && close(height, 160.0)
            && main_width >= 100.0
            && ix + iw <= f32::from(min.width) + 1.0,
        "트랙 {track}, 인스펙터 {ix}+{iw}, 메인 {main_width}, 아래 {height}"
    );
}
