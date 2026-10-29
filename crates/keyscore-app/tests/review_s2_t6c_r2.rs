//! S2-T6c 적대적 리뷰 2회차. 도우미는 `tests/review_s2_t6c.rs`에서 복사했다.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, SettingsSection};
use keyscore_app::actions::{settings, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::settings::MetronomeSettings;
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
    // 곡 끝 정지(S3-T3d) 전에 클릭이 오도록 빈 긴 클립(100마디, 100 s)을 둔다. 클립이 없으면 곡 끝은 틱 0이다.
    ksq::from_str(
        r#"{"tempo": [[0, 250000]], "tracks": [{"name": "A", "channel": 1,
            "clips": [{"start": 0, "length": 384000}]}]}"#,
    )
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

fn focused(
    cx: &mut TestAppContext,
    sh: AnyWindowHandle,
    view: &Entity<SettingsWindow>,
) -> Option<String> {
    cx.update_window(sh, |_, window, cx| {
        view.read(cx).focused_control(window, cx)
    })
    .unwrap()
}

// ---------- 2회차 깨뜨리기 시도 ----------

/// space로 연 선택 상자의 검색란에서 공백은 글자다. 「Side Stick」(37)을 공백을 넣어 검색해 고른다.
#[gpui_kit::test]
fn r2_space_in_open_search_is_text(cx: &mut TestAppContext) {
    start(cx, "r2-space-text");
    let (handle, _) = open_project(cx);
    let (sh, view) = open_settings(cx, handle, SettingsSection::Metronome);
    press(cx, sh, "tab tab tab");
    assert_eq!(
        focused(cx, sh, &view).as_deref(),
        Some("settings-metronome-beat")
    );
    press(cx, sh, "space");
    type_text(cx, sh, "Side");
    press(cx, sh, "space");
    type_text(cx, sh, "Stick");
    press(cx, sh, "enter");
    assert_eq!(
        metronome(cx).beat,
        37,
        "검색란의 공백이 글자로 들어가지 않았다"
    );
}

/// space로 열어 고른 뒤 포커스가 선택 상자로 돌아와 다시 space로 열린다. 프로젝트 창 재생으로 새지 않는다.
#[gpui_kit::test]
fn r2_space_reopens_after_choice(cx: &mut TestAppContext) {
    start(cx, "r2-space-again");
    let (handle, pw) = open_project(cx);
    let (sh, view) = open_settings(cx, handle, SettingsSection::Input);
    press(cx, sh, "space down enter");
    assert_eq!(key_layout(cx), KeyLayout::Solfege);
    assert_eq!(
        focused(cx, sh, &view).as_deref(),
        Some("settings-key-layout")
    );
    press(cx, sh, "space up enter");
    assert_eq!(
        key_layout(cx),
        KeyLayout::Piano,
        "고른 뒤 space로 다시 열리지 않았다"
    );
    let state = cx.read(|cx| pw.read(cx).transport_state());
    assert_eq!(format!("{state:?}"), "Stopped");
}

/// space로 열고 escape로 닫으면 값은 그대로이고, 다시 space로 열린다.
#[gpui_kit::test]
fn r2_escape_then_space_reopens(cx: &mut TestAppContext) {
    start(cx, "r2-escape");
    let (handle, _) = open_project(cx);
    let (sh, view) = open_settings(cx, handle, SettingsSection::Metronome);
    press(cx, sh, "tab");
    assert_eq!(
        focused(cx, sh, &view).as_deref(),
        Some("settings-metronome-channel")
    );
    press(cx, sh, "space down escape");
    assert_eq!(metronome(cx).channel, 10);
    press(cx, sh, "space");
    type_text(cx, sh, "12");
    press(cx, sh, "enter");
    assert_eq!(metronome(cx).channel, 12);
}

/// 메트로놈 버튼이 꺼진 채 재생 중 포트·노트를 바꾸면 어느 포트로도 클릭이 나지 않는다. 그 뒤 켜면 새
/// 포트·노트로 울린다(끄기 → 교체 → 켜기 순서가 버튼 꺼짐을 켜 버리지 않는가).
#[gpui_kit::test]
fn r2_button_off_change_while_playing_stays_silent(cx: &mut TestAppContext) {
    let main = start_with_engine(cx, "r2-off");
    let name = unique("r2-off-dest");
    let dest = Receiver::destination(&name);
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    run(cx, handle, transport::Play);
    choose(cx, sh, "settings-metronome-port", &name);
    choose(cx, sh, "settings-metronome-accent", "Cowbell");
    choose(cx, sh, "settings-metronome-beat", "Claves");
    choose(cx, sh, "settings-metronome-channel", "3");
    let a = dest.drain_for(Duration::from_millis(700));
    let b = main.drain_for(Duration::from_millis(10));
    assert!(
        !a.iter().any(|m| any_click(m)) && !b.iter().any(|m| any_click(m)),
        "버튼이 꺼졌는데 클릭이 났다: dest={a:?} main={b:?}"
    );
    run(cx, handle, transport::ToggleMetronome);
    dest.wait_for(WAIT, click_on(3, &[56, 75]))
        .expect("켠 뒤 새 포트·노트로 클릭이 오지 않았다");
    run(cx, handle, transport::ToggleMetronome);
    dest.drain_for(Duration::from_millis(100));
    let later = dest.drain_for(Duration::from_millis(700));
    assert!(
        !later.iter().any(|m| any_click(m)),
        "끈 뒤에도 클릭: {later:?}"
    );
    run(cx, handle, transport::Stop);
}

/// 메트로놈 포트가 없는 채 재생 중 버튼을 껐다 켜고 다른 값을 바꿔도(교체 경로) 옛 포트 번호로 새지 않는다.
#[gpui_kit::test]
fn r2_missing_port_toggle_and_edit_while_playing_does_not_leak(cx: &mut TestAppContext) {
    let main = start_with_engine(cx, "r2-missing");
    let x = unique("r2-missing-x");
    let y = unique("r2-missing-y");
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
    assert!(cx.read(|cx| view.read(cx).metronome_port_missing()));
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::ToggleMetronome);
    choose(cx, sh, "settings-metronome-channel", "5");
    dx.drain_for(Duration::from_millis(100));
    main.drain_for(Duration::from_millis(10));
    let xs = dx.drain_for(Duration::from_millis(700));
    let ms = main.drain_for(Duration::from_millis(10));
    assert!(
        !xs.iter().any(|m| any_click(m)) && !ms.iter().any(|m| any_click(m)),
        "없는 포트인데 클릭이 샜다: x={xs:?} main={ms:?}"
    );
    choose(cx, sh, "settings-metronome-port", &x);
    dx.wait_for(WAIT, click_on(5, &[37, 42]))
        .expect("X로 되돌린 뒤 클릭이 돌아오지 않았다");
    run(cx, handle, transport::Stop);
}

/// 재생 중 메트로놈 포트를 A·B로 빠르게 여러 번 바꾼 뒤 마지막 포트(A)로만 울린다.
#[gpui_kit::test]
fn r2_rapid_port_switches_settle_on_last(cx: &mut TestAppContext) {
    let _main = start_with_engine(cx, "r2-rapid");
    let a = unique("r2-rapid-a");
    let b = unique("r2-rapid-b");
    let da = Receiver::destination(&a);
    let db = Receiver::destination(&b);
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    for i in 0..10 {
        let to = if i % 2 == 0 { &b } else { &a };
        choose(cx, sh, "settings-metronome-port", to);
    }
    assert_eq!(metronome(cx).port, OutputPort::Named(a.clone()));
    db.drain_for(Duration::from_millis(100));
    da.drain_for(Duration::from_millis(10));
    let bs = db.drain_for(Duration::from_millis(700));
    assert!(
        !bs.iter().any(|m| any_click(m)),
        "B로 클릭이 계속 간다: {bs:?}"
    );
    da.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("A로 클릭이 오지 않았다");
    run(cx, handle, transport::Stop);
}
