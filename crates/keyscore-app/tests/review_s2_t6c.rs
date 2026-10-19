//! S2-T6c 적대적 리뷰 1회차. 도우미는 `tests/settings_tabs.rs`에서 복사했다.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, SettingsSection};
use keyscore_app::actions::{settings, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::settings::{MetronomeSettings, Settings};
use keyscore_app::settings_window::{self, SettingsWindow};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{OutputPort, Project, ksq};

const WAIT: Duration = Duration::from_secs(2);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-settings-tabs-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = temp_dir(name);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn unique(name: &str) -> String {
    format!("KS tabs {} {name}", std::process::id())
}

/// 앱과 엔진을 띄우고 기본 가상 포트를 받는 수신기를 붙인다.
fn start_with_engine(cx: &mut TestAppContext, name: &str) -> Receiver {
    start(cx, name);
    let port = unique(name);
    let options = EngineOptions {
        client_name: format!("ks-tabs-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    Receiver::source(&port)
}

fn project() -> Project {
    // 240 BPM이면 박이 0.25 s마다 온다.
    ksq::from_str(r#"{"tempo": [[0, 250000]], "tracks": [{"name": "A", "channel": 1}]}"#)
        .unwrap()
        .0
}

fn open_project(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    activate(cx, handle);
    (handle, view)
}

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    cx.run_until_parked();
    render(cx, handle);
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn settings_view(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<SettingsWindow>) {
    let (handle, view) = cx
        .update(|cx| settings_window::current(cx))
        .expect("설정 창이 없다");
    activate(cx, handle);
    (handle, view)
}

fn open_settings(
    cx: &mut TestAppContext,
    from: AnyWindowHandle,
    section: SettingsSection,
) -> (AnyWindowHandle, Entity<SettingsWindow>) {
    run(
        cx,
        from,
        settings::Open(keyscore_app::actions::args::Opt(Some(section))),
    );
    settings_view(cx)
}

/// 선택 상자를 눌러 열고, 검색어를 친 뒤 첫 항목을 고른다.
fn choose(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str, search: &str) {
    click(cx, handle, id);
    type_text(cx, handle, search);
    press(cx, handle, "enter");
}

fn metronome(cx: &mut TestAppContext) -> MetronomeSettings {
    cx.read(|cx| AppSettings::get(cx).metronome.clone())
}

fn key_layout(cx: &mut TestAppContext) -> KeyLayout {
    cx.read(|cx| AppSettings::get(cx).input.key_layout)
}

// ---------- 수신기 ----------

struct Receiver {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: Option<coremidi::InputPort>,
    _dest: Option<coremidi::VirtualDestination>,
    _client: coremidi::Client,
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

impl Receiver {
    /// 앱의 가상 출력 포트(소스)를 받는다.
    fn source(name: &str) -> Receiver {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-tabs-rx").unwrap();
        let input = client
            .input_port("in", move |list: &coremidi::PacketList| {
                for p in list {
                    for msg in split(p.data()) {
                        let _ = tx.send(msg);
                    }
                }
            })
            .unwrap();
        let deadline = Instant::now() + WAIT;
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
        Receiver {
            rx,
            _input: Some(input),
            _dest: None,
            _client: client,
        }
    }

    /// 앱이 이름으로 고를 목적지를 만든다.
    fn destination(name: &str) -> Receiver {
        let client = coremidi::Client::new(&format!("ks-tabs-dest-{name}")).unwrap();
        let (tx, rx) = mpsc::channel();
        let dest = client
            .virtual_destination(name, move |list: &coremidi::PacketList| {
                for p in list {
                    for m in split(p.data()) {
                        let _ = tx.send(m);
                    }
                }
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
        Receiver {
            rx,
            _input: None,
            _dest: Some(dest),
            _client: client,
        }
    }

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

/// 채널(1–16)의 Note On(벨로시티 > 0)이고 노트가 `notes` 가운데 하나인가.
fn click_on(channel: u8, notes: &'static [u8]) -> impl Fn(&[u8]) -> bool {
    move |m| m.len() == 3 && m[0] == 0x90 | (channel - 1) && notes.contains(&m[1]) && m[2] > 0
}

fn any_click(m: &[u8]) -> bool {
    m.len() == 3 && m[0] & 0xF0 == 0x90 && m[2] > 0
}

// ---------- 1. 설정 파일 ----------
// ---------- 리뷰 도우미 ----------

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

/// 감시 주기만큼 시계를 돌리며 조건을 기다린다(`port_watch.rs`의 `pump`).
fn pump(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    done: impl Fn(&mut TestAppContext) -> bool,
    why: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        cx.executor()
            .advance_clock(keyscore_app::ports::POLL_INTERVAL);
        cx.run_until_parked();
        render(cx, handle);
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "{why}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn start_monitor(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update(keyscore_app::ports::start_monitor);
    pump(
        cx,
        handle,
        |cx| cx.read(keyscore_app::ports::is_monitoring),
        "감시가 시작되지 않았다",
    );
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

fn project_on(port: &str) -> Project {
    ksq::from_str(&format!(
        r#"{{"tempo": [[0, 250000]], "tracks": [{{"name": "A", "channel": 1, "port": {{"named": "{port}"}}}}]}}"#
    ))
    .unwrap()
    .0
}

// ---------- 깨뜨리기 시도 ----------

/// 재생 중 이름 포트 → 가상 포트: 클릭이 가상 포트로 옮기고 이름 포트에는 더 가지 않는다.
#[gpui_kit::test]
fn brk_named_to_virtual_while_playing_moves_clicks(cx: &mut TestAppContext) {
    let main = start_with_engine(cx, "r-n2v");
    let name = unique("r-n2v-dest");
    let dest = Receiver::destination(&name);
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-port", &name);
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    dest.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("이름 포트로 클릭이 오지 않았다");
    choose(cx, sh, "settings-metronome-port", "가상 포트");
    assert_eq!(metronome(cx).port, OutputPort::Virtual);
    dest.drain_for(Duration::from_millis(100));
    main.drain_for(Duration::from_millis(100));
    main.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("가상 포트로 옮긴 뒤 클릭이 오지 않았다");
    let later = dest.drain_for(Duration::from_millis(700));
    assert!(
        !later.iter().any(|m| any_click(m)),
        "옮긴 뒤에도 이름 포트로 클릭이 간다: {later:?}"
    );
    run(cx, handle, transport::Stop);
}

/// 재생 중 이름 포트 → 목록에는 있지만 이미 사라진 이름: 클릭이 멈추고 알린다. 다시 있는 이름으로 고르면
/// 클릭이 돌아온다(감시 없음).
#[gpui_kit::test]
fn brk_switch_to_vanished_name_while_playing_silences_then_recovers(cx: &mut TestAppContext) {
    let main = start_with_engine(cx, "r-vanished");
    let x = unique("r-vanished-x");
    let y = unique("r-vanished-y");
    let dx = Receiver::destination(&x);
    let dy = Receiver::destination(&y);
    let (handle, view) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-port", &x);
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    dx.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("X로 클릭이 오지 않았다");
    drop(dy);
    wait_gone(&y);
    choose(cx, sh, "settings-metronome-port", &y);
    assert_eq!(metronome(cx).port, OutputPort::Named(y.clone()));
    render(cx, handle);
    assert!(
        cx.read(|cx| view.read(cx).metronome_port_missing()),
        "사라진 이름인데 missing 표시가 없다"
    );
    dx.drain_for(Duration::from_millis(100));
    main.drain_for(Duration::from_millis(100));
    let x_later = dx.drain_for(Duration::from_millis(700));
    let main_later = main.drain_for(Duration::from_millis(10));
    assert!(
        !x_later.iter().any(|m| any_click(m)) && !main_later.iter().any(|m| any_click(m)),
        "사라진 포트를 골랐는데 다른 포트로 클릭이 샜다: x={x_later:?} main={main_later:?}"
    );
    assert!(
        messages(cx, &view)
            .iter()
            .any(|m| m.contains("메트로놈") && m.contains(&y)),
        "{:?}",
        messages(cx, &view)
    );
    choose(cx, sh, "settings-metronome-port", &x);
    dx.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("X로 되돌린 뒤 클릭이 돌아오지 않았다");
    assert!(!cx.read(|cx| view.read(cx).metronome_port_missing()));
    run(cx, handle, transport::Stop);
}

/// 메트로놈 포트가 트랙 포트와 같은 번호를 쓰다가(트랙이 연 포트) 다른 이름으로 바뀌면 번호가 다시
/// 매겨진다. 옛 번호(트랙 포트)로 클릭이 새지 않는다.
#[gpui_kit::test]
fn brk_metronome_leaves_shared_track_port(cx: &mut TestAppContext) {
    let _main = start_with_engine(cx, "r-shared");
    let c = unique("r-shared-c");
    let a = unique("r-shared-a");
    let dc = Receiver::destination(&c);
    let da = Receiver::destination(&a);
    let (handle, _) = cx
        .update(|cx| open_project_window(project_on(&c), cx))
        .unwrap();
    activate(cx, handle);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-port", &c);
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    dc.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("트랙과 같은 포트로 클릭이 오지 않았다");
    choose(cx, sh, "settings-metronome-port", &a);
    da.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("새 포트로 클릭이 오지 않았다");
    dc.drain_for(Duration::from_millis(50));
    let later = dc.drain_for(Duration::from_millis(700));
    assert!(
        !later.iter().any(|m| any_click(m)),
        "옛 포트(트랙 포트)로 클릭이 계속 간다: {later:?}"
    );
    run(cx, handle, transport::Stop);
}

/// 재생 중 메트로놈 이름 포트가 사라졌다 돌아온다(포트 감시). 사라진 동안 다른 포트로 새지 않고, 돌아오면
/// 클릭이 다시 난다.
#[gpui_kit::test]
fn brk_metronome_port_vanish_and_return_while_playing(cx: &mut TestAppContext) {
    let main = start_with_engine(cx, "r-return");
    let name = unique("r-return-dest");
    let dest = Receiver::destination(&name);
    let (handle, view) = open_project(cx);
    start_monitor(cx, handle);
    cx.update(|cx| {
        AppSettings::update(cx, |s| s.metronome.port = OutputPort::Named(name.clone()));
    });
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    dest.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("이름 포트로 클릭이 오지 않았다");
    drop(dest);
    wait_gone(&name);
    pump(
        cx,
        handle,
        |cx| cx.read(|cx| view.read(cx).metronome_port_missing()),
        "메트로놈 포트가 사라졌는데 missing 표시가 없다",
    );
    main.drain_for(Duration::from_millis(100));
    let leaked = main.drain_for(Duration::from_millis(600));
    assert!(
        !leaked.iter().any(|m| any_click(m)),
        "사라진 동안 가상 포트로 클릭이 샜다: {leaked:?}"
    );
    let back = Receiver::destination(&name);
    pump(
        cx,
        handle,
        |cx| !cx.read(|cx| view.read(cx).metronome_port_missing()),
        "돌아왔는데 missing 표시가 그대로다",
    );
    back.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("돌아온 포트로 클릭이 다시 나지 않았다");
    assert!(
        messages(cx, &view).iter().any(|m| m.contains(&name)),
        "{:?}",
        messages(cx, &view)
    );
    run(cx, handle, transport::Stop);
}

/// 설정 창이 열린 채 새 목적지가 생기면(포트 감시) 메트로놈 포트 목록에 들어와 고를 수 있다.
#[gpui_kit::test]
fn brk_port_list_refreshes_on_hotplug(cx: &mut TestAppContext) {
    let _main = start_with_engine(cx, "r-hotplug");
    let (handle, _) = open_project(cx);
    start_monitor(cx, handle);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    let name = unique("r-hotplug-late");
    let _dest = Receiver::destination(&name);
    pump(
        cx,
        sh,
        |cx| cx.read(|cx| keyscore_app::ports::port_present(cx, &name)) == Some(true),
        "감시가 새 목적지를 보지 못했다",
    );
    render(cx, sh);
    choose(cx, sh, "settings-metronome-port", &name);
    assert_eq!(metronome(cx).port, OutputPort::Named(name.clone()));
}

/// 설정 창을 닫았다 다시 열면 선택 상자가 지금 설정을 보이고 다시 고를 수 있다.
#[gpui_kit::test]
fn brk_reopen_settings_keeps_selection_in_sync(cx: &mut TestAppContext) {
    start(cx, "r-reopen");
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-channel", "9");
    cx.update_window(sh, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    // 창이 닫힌 동안 밖에서 바꾼다
    cx.update(|cx| AppSettings::update(cx, |s| s.metronome.accent = 56));
    let (sh, view) = open_settings(cx, handle, SettingsSection::Metronome);
    assert!(
        label(cx, sh, "settings-metronome-accent").is_some_and(|l| l.contains("56 · Cowbell")),
        "{:?}",
        label(cx, sh, "settings-metronome-accent")
    );
    let focused = cx
        .update_window(sh, |_, window, cx| {
            view.read(cx).focused_control(window, cx)
        })
        .unwrap();
    assert_eq!(focused.as_deref(), Some("settings-metronome-port"));
    choose(cx, sh, "settings-metronome-channel", "12");
    assert_eq!(metronome(cx).channel, 12);
}

/// GM 이름으로 검색해 고르면 걸러진 목록의 자리가 아니라 그 노트 번호가 들어간다.
#[gpui_kit::test]
fn brk_search_by_gm_name_picks_right_note(cx: &mut TestAppContext) {
    start(cx, "r-gm");
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-accent", "Cowbell");
    choose(cx, sh, "settings-metronome-beat", "Claves");
    let m = metronome(cx);
    assert_eq!((m.accent, m.beat), (56, 75));
    // 채널 「1」 검색은 1, 10–16을 남긴다. 첫 항목을 고르면 1이다
    choose(cx, sh, "settings-metronome-channel", "1");
    assert_eq!(metronome(cx).channel, 1);
}

/// 파일의 목록 밖 길이(75 ms)는 목록에 덧붙여 보이고, 다른 값을 고를 수 있다.
#[gpui_kit::test]
fn brk_length_from_file_shown_and_replaceable(cx: &mut TestAppContext) {
    let dir = temp_dir("r-length");
    std::fs::write(
        dir.join("settings.json"),
        r#"{"metronome": {"length_ms": 75}}"#,
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    assert_eq!(metronome(cx).length_ms, 75);
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    assert!(
        label(cx, sh, "settings-metronome-length").is_some_and(|l| l.contains("75 ms")),
        "{:?}",
        label(cx, sh, "settings-metronome-length")
    );
    choose(cx, sh, "settings-metronome-length", "75");
    assert_eq!(metronome(cx).length_ms, 75);
    choose(cx, sh, "settings-metronome-length", "500");
    assert_eq!(metronome(cx).length_ms, 500);
    cx.update(AppSettings::flush);
    let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(text.contains("\"length_ms\": 500"), "{text}");
}

/// 엔진 없이 메트로놈 값을 바꾸고 재생·메트로놈 버튼을 눌러도 무너지지 않는다.
#[gpui_kit::test]
fn brk_metronome_change_without_engine(cx: &mut TestAppContext) {
    start(cx, "r-noengine");
    let (handle, view) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-channel", "3");
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    choose(cx, sh, "settings-metronome-beat", "70");
    let m = metronome(cx);
    assert_eq!((m.channel, m.beat), (3, 70));
    assert!(!cx.read(|cx| view.read(cx).metronome_port_missing()));
}

/// 보정 마법사 칸은 Tab으로 닿고 space로도 연다(계획: Enter/space).
#[gpui_kit::test]
fn brk_calibrate_by_space(cx: &mut TestAppContext) {
    start(cx, "r-space");
    let (handle, _) = open_project(cx);
    let (sh, view) = open_settings(cx, handle, SettingsSection::Input);
    press(cx, sh, "tab");
    let focused = cx
        .update_window(sh, |_, window, cx| {
            view.read(cx).focused_control(window, cx)
        })
        .unwrap();
    assert_eq!(focused.as_deref(), Some("settings-calibrate"));
    press(cx, sh, "space");
    assert!(
        cx.read(|cx| view.read(cx).message())
            .is_some_and(|m| m.contains("보정 마법사")),
        "{:?}",
        cx.read(|cx| view.read(cx).message())
    );
}

/// 선택 상자는 space로도 연다(계획 끝의 정의 5: 「선택 상자는 `Enter`/`space`로 열고 화살표로 고른다」).
#[gpui_kit::test]
fn brk_select_opens_with_space(cx: &mut TestAppContext) {
    start(cx, "r-space-select");
    let (handle, pw) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Input);
    press(cx, sh, "space");
    press(cx, sh, "down");
    press(cx, sh, "enter");
    let state = cx.read(|cx| pw.read(cx).transport_state());
    assert_eq!(
        key_layout(cx),
        KeyLayout::Solfege,
        "space로 선택 상자가 열리지 않았다(프로젝트 창 트랜스포트: {state:?})"
    );
}

/// 팔레트로 바꾼 키 레이아웃이 열린 설정 창의 선택 상자(표시와 커서)에 바로 맞는다.
#[gpui_kit::test]
fn brk_key_layout_external_sync(cx: &mut TestAppContext) {
    start(cx, "r-layout");
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Input);
    press(cx, sh, "enter");
    press(cx, sh, "down");
    press(cx, sh, "enter");
    assert_eq!(key_layout(cx), KeyLayout::Solfege);
    run(cx, handle, settings::SetKeyLayout(KeyLayout::Piano));
    render(cx, sh);
    assert!(
        label(cx, sh, "settings-key-layout").is_some_and(|l| l.contains("건반")),
        "{:?}",
        label(cx, sh, "settings-key-layout")
    );
    // 커서도 바깥 값에 맞았는가: 열고 바로 Enter면 그대로 건반형
    click(cx, sh, "settings-key-layout");
    press(cx, sh, "enter");
    assert_eq!(key_layout(cx), KeyLayout::Piano);
}

/// 설정 파일 경계: 빈 이름·여분 키가 붙은 포트·소수 채널·문자열 숫자는 그 필드만 기본값, 모르는 키는 남는다.
#[test]
fn brk_settings_file_edge_values() {
    let path = temp_dir("r-file").join("settings.json");
    std::fs::write(
        &path,
        r#"{"metronome": {"port": {"named": ""}, "channel": 9.0, "accent": "37",
                          "beat": 42, "length_ms": 4294967296, "x": [1]},
            "input": {"key_layout": "Piano", "y": null}, "z": 3}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s.metronome, MetronomeSettings::default());
    assert_eq!(s.input.key_layout, KeyLayout::Piano);
    assert_eq!(warnings.len(), 5, "{warnings:?}");
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metronome"]["x"], serde_json::json!([1]));
    assert!(v["input"].as_object().unwrap().contains_key("y"), "{text}");
    assert_eq!(v["z"], 3);
    assert_eq!(v["metronome"]["port"], "virtual");
}
