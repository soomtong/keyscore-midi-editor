//! S2-T6c 설정 > 입력·메트로놈·AI(MET-1, P3, UI-11, 7.6). 메트로놈 클릭은 같은 프로세스의 `coremidi`
//! 수신기로 받는다(engine.rs와 같은 루프백).

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

#[test]
fn settings_input_and_metronome_round_trip() {
    let path = temp_dir("round-trip").join("settings.json");
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty());
    assert_eq!(s.input.key_layout, KeyLayout::Piano);
    let m = &s.metronome;
    assert_eq!(
        (m.port.clone(), m.channel, m.accent, m.beat, m.length_ms),
        (OutputPort::Virtual, 10, 37, 42, 50),
        "MET-1 기본값"
    );
    std::fs::write(
        &path,
        r#"{"input": {"key_layout": "solfege", "later": 1},
            "metronome": {"port": {"named": "Drum Synth"}, "channel": 9, "accent": 60,
                          "beat": 61, "length_ms": 120, "swing": true}}"#,
    )
    .unwrap();
    let (mut s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(s.input.key_layout, KeyLayout::Solfege);
    assert_eq!(s.metronome.port, OutputPort::Named("Drum Synth".to_owned()));
    assert_eq!(
        (
            s.metronome.channel,
            s.metronome.accent,
            s.metronome.beat,
            s.metronome.length_ms
        ),
        (9, 60, 61, 120)
    );
    s.metronome.beat = 62;
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("\"later\"") && text.contains("\"swing\""),
        "{text}"
    );
    let (again, _) = Settings::load(&path);
    assert_eq!(again, s);

    // 잘못된 필드는 그 필드만 기본값이고 경고 하나씩
    std::fs::write(
        &path,
        r#"{"input": {"key_layout": "dvorak"},
            "metronome": {"port": 3, "channel": 17, "accent": 128, "beat": -1, "length_ms": 0}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s.input.key_layout, KeyLayout::Piano);
    assert_eq!(s.metronome, MetronomeSettings::default());
    assert_eq!(warnings.len(), 6, "{warnings:?}");
    // 객체가 아니면 통째로 기본값 + 경고 하나
    std::fs::write(&path, r#"{"input": 1, "metronome": "loud"}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s, Settings::default());
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    // 범위 끝 값은 받는다
    std::fs::write(
        &path,
        r#"{"metronome": {"channel": 16, "accent": 0, "beat": 127, "length_ms": 10000}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        (
            s.metronome.channel,
            s.metronome.accent,
            s.metronome.beat,
            s.metronome.length_ms
        ),
        (16, 0, 127, 10000)
    );
}

// ---------- 2–3. 입력 ----------

#[gpui_kit::test]
fn set_key_layout_updates_settings_and_input_tab(cx: &mut TestAppContext) {
    let dir = start(cx, "layout");
    let (handle, _) = open_project(cx);
    // 팔레트·메뉴 경로(앱 전역 처리기)
    run(cx, handle, settings::SetKeyLayout(KeyLayout::Solfege));
    assert_eq!(key_layout(cx), KeyLayout::Solfege);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Input);
    assert!(
        label(cx, sh, "settings-key-layout").is_some_and(|l| l.contains("계이름")),
        "{:?}",
        label(cx, sh, "settings-key-layout")
    );
    // 입력 탭의 선택 상자
    choose(cx, sh, "settings-key-layout", "건반");
    assert_eq!(key_layout(cx), KeyLayout::Piano);
    assert!(label(cx, sh, "settings-key-layout").is_some_and(|l| l.contains("건반")));
    cx.update(AppSettings::flush);
    let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(text.contains("\"key_layout\": \"piano\""), "{text}");
}

#[gpui_kit::test]
fn calibrate_opens_input_section_with_notice(cx: &mut TestAppContext) {
    start(cx, "calibrate");
    let (handle, _) = open_project(cx);
    run(cx, handle, settings::Calibrate);
    let (sh, view) = settings_view(cx);
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::Input
    );
    assert!(
        cx.read(|cx| view.read(cx).message())
            .is_some_and(|m| m.contains("보정 마법사")),
        "{:?}",
        cx.read(|cx| view.read(cx).message())
    );
    // 입력 탭의 버튼도 같은 Action이다
    click(cx, sh, "settings-section-keymap");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::Keymap
    );
    click(cx, sh, "settings-section-input");
    click(cx, sh, "settings-calibrate");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::Input
    );
    assert!(
        cx.read(|cx| view.read(cx).message())
            .is_some_and(|m| m.contains("보정 마법사"))
    );
}

// ---------- 4–6. 메트로놈 ----------

#[gpui_kit::test]
fn metronome_settings_change_clicks(cx: &mut TestAppContext) {
    let rx = start_with_engine(cx, "met-change");
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-channel", "9");
    choose(cx, sh, "settings-metronome-accent", "60");
    choose(cx, sh, "settings-metronome-beat", "61");
    choose(cx, sh, "settings-metronome-length", "100");
    let m = metronome(cx);
    assert_eq!((m.channel, m.accent, m.beat, m.length_ms), (9, 60, 61, 100));
    assert!(label(cx, sh, "settings-metronome-channel").is_some_and(|l| l.contains('9')));
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    let got = rx
        .wait_for(WAIT, click_on(9, &[60, 61]))
        .expect("바꾼 채널·노트로 클릭이 오지 않았다");
    assert!(
        !got.iter().any(|m| click_on(10, &[37, 42])(m)),
        "옛 설정의 클릭이 왔다: {got:?}"
    );
    run(cx, handle, transport::Stop);
}

#[gpui_kit::test]
fn metronome_change_applies_while_playing(cx: &mut TestAppContext) {
    let rx = start_with_engine(cx, "met-live");
    let (handle, _) = open_project(cx);
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    rx.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("기본 클릭이 오지 않았다");
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-beat", "70");
    choose(cx, sh, "settings-metronome-accent", "70");
    rx.drain_for(Duration::from_millis(100));
    rx.wait_for(WAIT, click_on(10, &[70]))
        .expect("재생 중 바꾼 노트가 반영되지 않았다");
    let later = rx.drain_for(Duration::from_millis(700));
    assert!(
        !later.iter().any(|m| click_on(10, &[37, 42])(m)),
        "바꾼 뒤에도 옛 노트가 온다: {later:?}"
    );
    run(cx, handle, transport::Stop);
}

#[gpui_kit::test]
fn metronome_named_port_routes_and_missing_is_reported(cx: &mut TestAppContext) {
    let main = start_with_engine(cx, "met-port");
    let name = unique("met-port-dest");
    let dest = Receiver::destination(&name);
    let (handle, view) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Metronome);
    choose(cx, sh, "settings-metronome-port", &name);
    assert_eq!(metronome(cx).port, OutputPort::Named(name.clone()));
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Play);
    dest.wait_for(WAIT, click_on(10, &[37, 42]))
        .expect("이름 포트로 클릭이 오지 않았다");
    assert!(
        !main
            .drain_for(Duration::from_millis(300))
            .iter()
            .any(|m| any_click(m)),
        "기본 가상 포트에도 클릭이 갔다"
    );
    assert!(!cx.read(|cx| view.read(cx).metronome_port_missing()));
    run(cx, handle, transport::Stop);

    // 없는 이름: 클릭 없음 + 알림
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.metronome.port = OutputPort::Named(unique("met-port-nowhere"));
        });
    });
    main.drain_for(Duration::from_millis(100));
    run(cx, handle, transport::Play);
    assert!(cx.read(|cx| view.read(cx).metronome_on()));
    assert!(cx.read(|cx| view.read(cx).metronome_port_missing()));
    let got = main.drain_for(Duration::from_millis(800));
    assert!(
        !got.iter().any(|m| any_click(m)),
        "없는 포트인데 클릭이 났다: {got:?}"
    );
    let messages: Vec<String> = cx.read(|cx| {
        view.read(cx)
            .messages()
            .iter()
            .map(ToString::to_string)
            .collect()
    });
    assert!(
        messages
            .iter()
            .any(|m| m.contains("메트로놈") && m.contains("met-port-nowhere")),
        "{messages:?}"
    );
    run(cx, handle, transport::Stop);
}

// ---------- 7. 키보드 ----------

#[gpui_kit::test]
fn settings_sections_keyboard_reachable(cx: &mut TestAppContext) {
    start(cx, "keyboard");
    let (handle, _) = open_project(cx);
    let (sh, view) = open_settings(cx, handle, SettingsSection::Input);
    // 구역을 열면 첫 컨트롤에 포커스가 있다: Enter로 열고 아래 화살표로 다음 항목을 고른다
    let focused = |cx: &mut TestAppContext| {
        cx.update_window(sh, |_, window, cx| {
            view.read(cx).focused_control(window, cx)
        })
        .unwrap()
    };
    assert_eq!(focused(cx).as_deref(), Some("settings-key-layout"));
    press(cx, sh, "enter");
    press(cx, sh, "down");
    press(cx, sh, "enter");
    assert_eq!(key_layout(cx), KeyLayout::Solfege);
    press(cx, sh, "tab");
    assert_eq!(focused(cx).as_deref(), Some("settings-calibrate"));
    // 메트로놈 구역: Tab으로 차례로 지난다
    press(cx, sh, "ctrl-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::MidiOutput
    );
    press(cx, sh, "ctrl-tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).section()),
        SettingsSection::Metronome
    );
    let mut order = vec![focused(cx)];
    for _ in 0..4 {
        press(cx, sh, "tab");
        order.push(focused(cx));
    }
    let order: Vec<_> = order.into_iter().flatten().collect();
    assert_eq!(
        order,
        [
            "settings-metronome-port",
            "settings-metronome-channel",
            "settings-metronome-accent",
            "settings-metronome-beat",
            "settings-metronome-length",
        ]
    );
    press(cx, sh, "shift-tab shift-tab shift-tab");
    assert_eq!(focused(cx).as_deref(), Some("settings-metronome-channel"));
    press(cx, sh, "enter");
    type_text(cx, sh, "16");
    press(cx, sh, "enter");
    assert_eq!(metronome(cx).channel, 16);
}

// ---------- 8. AI ----------

#[gpui_kit::test]
fn ai_section_is_disabled_placeholder(cx: &mut TestAppContext) {
    start(cx, "ai");
    let (handle, _) = open_project(cx);
    let (sh, _) = open_settings(cx, handle, SettingsSection::Ai);
    for id in [
        "settings-ai-endpoint",
        "settings-ai-models",
        "settings-ai-key",
        "settings-ai-templates",
        "settings-ai-taste",
    ] {
        let l = label(cx, sh, id).unwrap_or_else(|| panic!("{id}가 없다"));
        assert!(l.contains("S7"), "{id}: {l}");
    }
    let before = cx.read(|cx| AppSettings::get(cx).clone());
    click(cx, sh, "settings-ai-endpoint");
    press(cx, sh, "enter");
    assert_eq!(cx.read(|cx| AppSettings::get(cx).clone()), before);
}
