//! S2-T5b 트랜스포트 바(7.1 상단, TRN-1·2·4, 7.6).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, transport};
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::tracks::{format_position, key_choices};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Key, OutputPort, PitchClass, Project, Tempo, TimeSignature, ksq};

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-transportbar-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    // 시험 창은 활성이 아니면 포커스·Blur 사건이 나지 않는다. 실제 앱처럼 활성으로 둔다(리뷰 3 제안 6)
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: impl Into<gpui_kit::ElementId>) {
    let id = id.into();
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn click_menu_item(cx: &mut TestAppContext, handle: AnyWindowHandle, index: usize) {
    cx.update_window(handle, |_, window, cx| {
        window
            .within("popup-menu")
            .click(gpui_kit::ElementId::Integer(index as u64), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
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

fn project_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Project {
    cx.read(|cx| view.read(cx).document().read(cx).project().clone())
}

/// 입력란을 눌러 내용을 지우고 새로 친 뒤 Enter.
fn enter_field(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str, text: &str) {
    click(cx, handle, id);
    press(cx, handle, "cmd-a");
    press(cx, handle, "backspace");
    type_text(cx, handle, text);
    press(cx, handle, "enter");
    cx.run_until_parked();
    render(cx, handle);
}

// ---------- 5. 버튼은 Action을 낸다 ----------

#[gpui_kit::test]
fn transport_buttons_dispatch_actions(cx: &mut TestAppContext) {
    start(cx, "buttons");
    let (handle, view) = open(cx, Project::new());
    // 엔진이 없으므로 재생 버튼은 「엔진 없음」 메시지를 남긴다(Action이 처리기에 닿았다는 뜻)
    click(cx, handle, "transport-play");
    assert!(!cx.read(|cx| view.read(cx).messages().is_empty()));
    click(cx, handle, "transport-stop");
    assert_eq!(
        cx.read(|cx| view.read(cx).transport_state()),
        TransportState::Stopped
    );
    // gpui-component 버튼의 꺼짐은 접근성 스냅샷에 실리지 않는다(라이브러리 한계). 동작으로 본다.
    // 녹음은 S5까지 꺼짐: 눌러도 Sequence 모드가 되지 않는다
    click(cx, handle, "transport-record");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        keyscore_app::mode::Mode::Normal
    );
    // AI✦는 S7까지 꺼짐(누를 수 있는 요소로는 있다)
    assert!(label(cx, handle, "transport-ai").is_some());
    // 트랙이 없으면 포트 드롭다운도 꺼짐: 눌러도 메뉴가 열리지 않는다
    click(cx, handle, "transport-port");
    assert!(cx.read(|cx| view.read(cx).port_choices().is_empty()));
}

// ---------- 6. 템포·박자표·조성 ----------

#[gpui_kit::test]
fn tempo_meter_key_fields_edit_document(cx: &mut TestAppContext) {
    start(cx, "fields");
    let (handle, view) = open(cx, Project::new());
    enter_field(cx, handle, "transport-tempo", "90");
    assert_eq!(
        project_of(cx, &view)
            .tempo_map()
            .tempo_at(keyscore_core::Tick::ZERO),
        Tempo::from_bpm(90).unwrap()
    );
    run(cx, handle, edit::Undo);
    assert_eq!(
        project_of(cx, &view)
            .tempo_map()
            .tempo_at(keyscore_core::Tick::ZERO),
        Tempo::DEFAULT
    );
    // 잘못된 값은 바꾸지 않고 알린다
    let before = cx.read(|cx| view.read(cx).messages().len());
    enter_field(cx, handle, "transport-tempo", "abc");
    assert_eq!(
        project_of(cx, &view)
            .tempo_map()
            .tempo_at(keyscore_core::Tick::ZERO),
        Tempo::DEFAULT
    );
    assert!(cx.read(|cx| view.read(cx).messages().len()) > before);

    enter_field(cx, handle, "transport-meter", "3/4");
    assert_eq!(
        project_of(cx, &view)
            .meter_map()
            .signature_at(keyscore_core::Tick::ZERO),
        TimeSignature::new(3, 4).unwrap()
    );

    click(cx, handle, "transport-key");
    let c_major = key_choices()
        .iter()
        .position(|k| *k == Key::Major(PitchClass::new(0).unwrap()))
        .unwrap();
    click_menu_item(cx, handle, c_major);
    assert_eq!(
        project_of(cx, &view).key(),
        Key::Major(PitchClass::new(0).unwrap())
    );
    assert_eq!(label(cx, handle, "transport-key").as_deref(), Some("C장조"));
}

// ---------- 7. 탭 템포 ----------

#[gpui_kit::test]
fn tap_tempo_sets_tempo_from_intervals(cx: &mut TestAppContext) {
    start(cx, "tap");
    let (handle, view) = open(cx, Project::new());
    // 탭 시각은 클릭 처리 안에서 찍힌다. 클릭 처리 시간이 기계마다 달라 고정 범위로 보면 흔들린다. 첫 탭과
    // 마지막 탭이 찍혔을 수 있는 구간으로 기대 BPM의 범위를 잡는다.
    let mut spans = Vec::new();
    for i in 0..4 {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(500));
        }
        let before = Instant::now();
        click(cx, handle, "transport-tap");
        spans.push((before, Instant::now()));
    }
    let bpm = project_of(cx, &view)
        .tempo_map()
        .tempo_at(keyscore_core::Tick::ZERO)
        .bpm_f64();
    let shortest = spans[3].0.duration_since(spans[0].1).as_secs_f64() / 3.0;
    let longest = spans[3].1.duration_since(spans[0].0).as_secs_f64() / 3.0;
    // 표시는 소수 한 자리로 반올림한다
    let (low, high) = (60.0 / longest - 0.1, 60.0 / shortest + 0.1);
    assert!((low..=high).contains(&bpm), "{bpm} not in {low}..={high}");
    assert!((90.0..=125.0).contains(&bpm), "{bpm}");
}

// ---------- 8. 메트로놈 버튼 ----------

#[gpui_kit::test]
fn metronome_button_reflects_state(cx: &mut TestAppContext) {
    start(cx, "metronome");
    let (handle, view) = open(cx, Project::new());
    assert!(!cx.read(|cx| view.read(cx).metronome_on()));
    click(cx, handle, "transport-metronome");
    assert!(cx.read(|cx| view.read(cx).metronome_on()));
    let checked = cx
        .update_window(handle, |_, window, _| {
            window.find("transport-metronome").checked()
        })
        .unwrap();
    assert_eq!(checked, Some(true));
    click(cx, handle, "transport-metronome");
    assert!(!cx.read(|cx| view.read(cx).metronome_on()));
}

// ---------- 9. 포트 드롭다운 ----------

#[gpui_kit::test]
fn port_dropdown_sets_current_track_port(cx: &mut TestAppContext) {
    start(cx, "port");
    // OS 목적지 하나를 만든다(다른 앱의 가상 입력처럼 보인다)
    let name = format!("KS dest {}", std::process::id());
    let client = coremidi::Client::new("ks-transportbar-dest").unwrap();
    let _dest = client
        .virtual_destination(&name, |_: &coremidi::PacketList| {})
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !keyscore_midi::destinations()
        .iter()
        .any(|d| d.name() == name)
    {
        assert!(Instant::now() < deadline, "목적지가 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    }
    let p = ksq::from_str(r#"{"tracks": [{"port": "virtual", "channel": 1}]}"#)
        .unwrap()
        .0;
    let (handle, view) = open(cx, p);
    click(cx, handle, "transport-port");
    let choices = cx.read(|cx| view.read(cx).port_choices());
    assert_eq!(choices[0], OutputPort::Virtual);
    let index = choices
        .iter()
        .position(|c| *c == OutputPort::Named(name.clone()))
        .expect("드롭다운에 OS 목적지가 없다");
    click_menu_item(cx, handle, index);
    assert_eq!(
        project_of(cx, &view).tracks()[0].port(),
        &OutputPort::Named(name.clone())
    );
    assert_eq!(
        label(cx, handle, "transport-port").as_deref(),
        Some(name.as_str())
    );
}

// ---------- 12. 위치 표시 ----------

#[gpui_kit::test]
fn position_display_follows_transport(cx: &mut TestAppContext) {
    start(cx, "position");
    let port = format!("KS bar {}", std::process::id());
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: "ks-transportbar".into(),
                port_name: port,
            },
        )
    })
    .unwrap();
    let p = ksq::from_str(
        r#"{"tempo": [[0, 500000]], "tracks": [{"channel": 1,
            "clips": [{"start": 0, "length": 15360, "notes": [[0, 240, 60, 51492]]}]}]}"#,
    )
    .unwrap()
    .0;
    let (handle, view) = open(cx, p);
    assert_eq!(
        label(cx, handle, "transport-position").as_deref(),
        Some("1.1.0")
    );
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_millis(600));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = cx.read(|cx| view.read(cx).transport_state()) else {
        panic!("일시정지가 아니다");
    };
    let expected = format_position(&project_of(cx, &view), at);
    assert_eq!(label(cx, handle, "transport-position"), Some(expected));
    run(cx, handle, transport::Stop);
    assert_eq!(
        label(cx, handle, "transport-position").as_deref(),
        Some("1.1.0")
    );
}
