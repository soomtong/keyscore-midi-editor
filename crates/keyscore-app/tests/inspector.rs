//! S2-T5c 트랙 인스펙터(UI-12, 7.1 오른쪽, 7.6, PRE-3).
//! 키는 `simulate_keystrokes`(키 바인딩을 거침), 클릭은 실제 마우스 사건(`window.click`)이다. 창은 활성으로
//! 연다. 비활성 시험 창에서는 포커스·Blur 사건이 나지 않는다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::Int;
use keyscore_app::actions::{edit, midi, track, transport};
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Bank, OutputPort, Program, Project, TrackColor, Velocity, ksq};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-inspector-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
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

fn click_menu_item(cx: &mut TestAppContext, handle: AnyWindowHandle, index: u64) {
    cx.update_window(handle, |_, window, cx| {
        window
            .within("popup-menu")
            .click(gpui_kit::ElementId::Integer(index), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
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

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn shown(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.value().map(str::to_owned))
    })
    .unwrap()
}

fn exists(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> bool {
    cx.update_window(handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn project_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Project {
    cx.read(|cx| view.read(cx).document().read(cx).project().clone())
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
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

/// 입력란을 눌러 내용을 지우고 새로 친 뒤 Enter.
fn enter_field(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str, text: &str) {
    click(cx, handle, id);
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, text);
    press(cx, handle, "enter");
}

fn two_tracks() -> Project {
    // `.ksq`의 기본 벨로시티는 16비트 값이다
    let v90 = Velocity::from_midi1(90).unwrap().get();
    ksq::from_str(&format!(
        r#"{{"tracks": [{{"name": "Lead", "channel": 1, "program": 0}},
            {{"name": "Bass", "channel": 2, "program": 33, "bank": 5, "default_velocity": {v90}}}]}}"#
    ))
    .unwrap()
    .0
}

// ---------- 2. 현재 트랙을 보인다 ----------

#[gpui_kit::test]
fn inspector_shows_current_track(cx: &mut TestAppContext) {
    start(cx, "shows");
    let (handle, _view) = open(cx, two_tracks());
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Lead"));
    assert_eq!(label(cx, handle, "inspector-channel").as_deref(), Some("1"));
    assert_eq!(shown(cx, handle, "inspector-bank").as_deref(), Some(""));
    assert_eq!(
        shown(cx, handle, "inspector-velocity").as_deref(),
        Some("100")
    );
    assert_eq!(
        label(cx, handle, "inspector-program").as_deref(),
        Some("1 Acoustic Grand Piano")
    );
    run(cx, handle, track::Next);
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Bass"));
    assert_eq!(label(cx, handle, "inspector-channel").as_deref(), Some("2"));
    assert_eq!(shown(cx, handle, "inspector-bank").as_deref(), Some("5"));
    assert_eq!(
        shown(cx, handle, "inspector-velocity").as_deref(),
        Some("90")
    );
    assert_eq!(
        label(cx, handle, "inspector-program").as_deref(),
        Some("34 Electric Bass (finger)")
    );
    // 트랙이 없으면 안내만
    start(cx, "shows-empty");
    let (handle, _) = open(cx, Project::new());
    assert!(exists(cx, handle, "inspector-empty"));
    assert!(!exists(cx, handle, "inspector-name"));
}

// ---------- 3. 입력란 편집은 undo 단위 ----------

#[gpui_kit::test]
fn inspector_fields_edit_track_as_undo_units(cx: &mut TestAppContext) {
    start(cx, "fields");
    let (handle, view) = open(cx, two_tracks());
    let base = undo_len(cx, &view);
    enter_field(cx, handle, "inspector-name", "Melody");
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "Melody");
    assert_eq!(undo_len(cx, &view), base + 1);
    enter_field(cx, handle, "inspector-bank", "130");
    assert_eq!(
        project_of(cx, &view).tracks()[0].bank(),
        Some(Bank::new(130).unwrap())
    );
    enter_field(cx, handle, "inspector-velocity", "64");
    assert_eq!(
        project_of(cx, &view).tracks()[0].default_velocity(),
        Velocity::from_midi1(64).unwrap()
    );
    assert_eq!(undo_len(cx, &view), base + 3);
    // 같은 값은 편집하지 않는다
    enter_field(cx, handle, "inspector-velocity", "64");
    assert_eq!(undo_len(cx, &view), base + 3);
    // 잘못된 값은 바꾸지 않고 알린다
    let before = messages(cx, &view).len();
    enter_field(cx, handle, "inspector-velocity", "200");
    enter_field(cx, handle, "inspector-bank", "x");
    assert_eq!(undo_len(cx, &view), base + 3);
    assert!(messages(cx, &view).len() >= before + 2);
    // 뱅크를 비우면 없음
    enter_field(cx, handle, "inspector-bank", "");
    assert_eq!(project_of(cx, &view).tracks()[0].bank(), None);
    // 하나씩 되돌린다. 되돌리면 필드도 따라간다
    run(cx, handle, edit::Undo);
    assert_eq!(shown(cx, handle, "inspector-bank").as_deref(), Some("130"));
    run(cx, handle, edit::Undo);
    run(cx, handle, edit::Undo);
    assert_eq!(project_of(cx, &view).tracks()[0].bank(), None);
    run(cx, handle, edit::Undo);
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "Lead");
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Lead"));
}

/// Esc는 입력을 버리고 문서로 돌아간다. 문서 포커스에서 모드 키가 산다.
#[gpui_kit::test]
fn inspector_field_escape_cancels(cx: &mut TestAppContext) {
    start(cx, "escape");
    let (handle, view) = open(cx, two_tracks());
    click(cx, handle, "inspector-name");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, "Zzz");
    press(cx, handle, "escape");
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "Lead");
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Lead"));
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
}

// ---------- 4. 입력란의 키는 모드로 빠지지 않는다 ----------

#[gpui_kit::test]
fn inspector_typing_does_not_reach_mode_keys(cx: &mut TestAppContext) {
    for (name, preset) in [("default", Preset::Default), ("vim", Preset::Vim)] {
        start(cx, &format!("typing-{name}"));
        cx.update(|cx| keymap::set_preset(cx, preset));
        let (handle, view) = open(cx, two_tracks());
        click(cx, handle, "inspector-name");
        press(cx, handle, "cmd-a backspace");
        press(cx, handle, "m space o 3 j");
        assert_eq!(
            shown(cx, handle, "inspector-name").as_deref(),
            Some("m o3j"),
            "{name}"
        );
        let p = project_of(cx, &view);
        assert_eq!(p.tracks().len(), 2, "{name}");
        assert!(!p.tracks()[0].mute(), "{name}");
        assert_eq!(
            cx.read(|cx| view.read(cx).transport_state()),
            engine::TransportState::Stopped,
            "{name}"
        );
        assert_eq!(cx.read(keymap::pending_count), None, "{name}");
        press(cx, handle, "enter");
        assert_eq!(project_of(cx, &view).tracks()[0].name(), "m o3j", "{name}");
    }
}

// ---------- 5. 메뉴 ----------

#[gpui_kit::test]
fn inspector_menus_edit_track(cx: &mut TestAppContext) {
    start(cx, "menus");
    let (handle, view) = open(cx, two_tracks());
    let base = undo_len(cx, &view);
    // 색: 12색, 지금 색(0)을 다시 고르면 편집 없음
    click(cx, handle, "inspector-color");
    click_menu_item(cx, handle, 0);
    assert_eq!(undo_len(cx, &view), base);
    click(cx, handle, "inspector-color");
    click_menu_item(cx, handle, 4);
    assert_eq!(
        project_of(cx, &view).tracks()[0].color(),
        TrackColor::new(4).unwrap()
    );
    // 채널: 1–16, 10은 드럼
    click(cx, handle, "inspector-channel");
    click_menu_item(cx, handle, 9);
    assert_eq!(project_of(cx, &view).tracks()[0].channel().get(), 10);
    assert_eq!(
        label(cx, handle, "inspector-channel").as_deref(),
        Some("10 드럼")
    );
    click(cx, handle, "inspector-channel");
    click_menu_item(cx, handle, 9);
    // 가사 언어: 없음, ko, en, ja
    click(cx, handle, "inspector-lyric-lang");
    click_menu_item(cx, handle, 1);
    assert_eq!(project_of(cx, &view).tracks()[0].lyric_lang(), Some("ko"));
    click(cx, handle, "inspector-lyric-lang");
    click_menu_item(cx, handle, 0);
    assert_eq!(project_of(cx, &view).tracks()[0].lyric_lang(), None);
    // 포트: 가상 포트(지금 값)는 편집 없음
    click(cx, handle, "inspector-port");
    click_menu_item(cx, handle, 0);
    assert_eq!(undo_len(cx, &view), base + 4);
    // 메뉴 뒤에도 모드 키가 산다
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
    run(cx, handle, edit::Undo);
    run(cx, handle, edit::Undo);
    assert_eq!(project_of(cx, &view).tracks()[0].lyric_lang(), Some("ko"));
}

// ---------- 6. 프로그램 Select ----------

#[gpui_kit::test]
fn inspector_program_select_searches_gm_names(cx: &mut TestAppContext) {
    start(cx, "program");
    let (handle, view) = open(cx, two_tracks());
    let base = undo_len(cx, &view);
    click(cx, handle, "inspector-program");
    type_text(cx, handle, "vibra");
    press(cx, handle, "enter");
    assert_eq!(
        project_of(cx, &view).tracks()[0].program(),
        Some(Program::new(11).unwrap())
    );
    assert_eq!(undo_len(cx, &view), base + 1);
    assert_eq!(
        label(cx, handle, "inspector-program").as_deref(),
        Some("12 Vibraphone")
    );
    // 번호로도 찾는다. 「없음」을 고르면 프로그램이 없어진다
    click(cx, handle, "inspector-program");
    type_text(cx, handle, "없음");
    press(cx, handle, "enter");
    assert_eq!(project_of(cx, &view).tracks()[0].program(), None);
    run(cx, handle, edit::Undo);
    assert_eq!(
        project_of(cx, &view).tracks()[0].program(),
        Some(Program::new(11).unwrap())
    );
    assert_eq!(
        label(cx, handle, "inspector-program").as_deref(),
        Some("12 Vibraphone")
    );
}

// ---------- 7. Action 인자 ----------

#[gpui_kit::test]
fn inspector_actions_validate_arguments(cx: &mut TestAppContext) {
    start(cx, "actions");
    let (handle, view) = open(cx, two_tracks());
    run(cx, handle, track::Rename("Pad".into()));
    run(cx, handle, track::SetColor("blue".into()));
    run(cx, handle, track::SetDefaultVelocity(Int(127)));
    run(cx, handle, track::SetLyricLang("ja".into()));
    run(
        cx,
        handle,
        track::SetProgram {
            bank: Int(16383),
            program: Int(128),
        },
    );
    let t = project_of(cx, &view).tracks()[0].clone();
    assert_eq!(t.name(), "Pad");
    assert_eq!(t.color(), TrackColor::new(4).unwrap());
    assert_eq!(t.default_velocity(), Velocity::from_midi1(127).unwrap());
    assert_eq!(t.lyric_lang(), Some("ja"));
    assert_eq!(t.bank(), Some(Bank::new(16383).unwrap()));
    assert_eq!(t.program(), Some(Program::new(127).unwrap()));
    run(cx, handle, track::SetColor("11".into()));
    run(
        cx,
        handle,
        track::SetProgram {
            bank: Int(-1),
            program: Int(0),
        },
    );
    run(cx, handle, track::SetLyricLang(String::new()));
    let t = project_of(cx, &view).tracks()[0].clone();
    assert_eq!(t.color(), TrackColor::new(11).unwrap());
    assert_eq!(t.bank(), None);
    assert_eq!(t.program(), None);
    assert_eq!(t.lyric_lang(), None);
    // 이름 포트, 그리고 가상 포트로 되돌리기
    run(cx, handle, track::SetPort("Some Synth".into()));
    assert_eq!(
        project_of(cx, &view).tracks()[0].port(),
        &OutputPort::Named("Some Synth".into())
    );
    run(cx, handle, track::SetPort("virtual".into()));
    assert_eq!(
        project_of(cx, &view).tracks()[0].port(),
        &OutputPort::Virtual
    );
    // 잘못된 값: 편집 없이 메시지
    let len = undo_len(cx, &view);
    let before = messages(cx, &view).len();
    run(cx, handle, track::SetColor("mauve".into()));
    run(cx, handle, track::SetColor("12".into()));
    run(cx, handle, track::SetDefaultVelocity(Int(0)));
    run(cx, handle, track::SetDefaultVelocity(Int(128)));
    run(
        cx,
        handle,
        track::SetProgram {
            bank: Int(16384),
            program: Int(1),
        },
    );
    run(
        cx,
        handle,
        track::SetProgram {
            bank: Int(0),
            program: Int(129),
        },
    );
    run(cx, handle, track::Rename(String::new()));
    assert_eq!(undo_len(cx, &view), len);
    assert_eq!(messages(cx, &view).len(), before + 7);
    // 같은 값은 편집하지 않는다
    run(cx, handle, track::Rename("Pad".into()));
    run(cx, handle, track::SetColor("11".into()));
    assert_eq!(undo_len(cx, &view), len);
    // 트랙이 없으면 아무 일도 없다
    start(cx, "actions-empty");
    let (handle, view) = open(cx, Project::new());
    run(cx, handle, track::Rename("X".into()));
    assert_eq!(undo_len(cx, &view), 0);
}

// ---------- 8. 버튼 ----------

#[gpui_kit::test]
fn inspector_mute_solo_test_buttons_run_actions(cx: &mut TestAppContext) {
    start(cx, "buttons");
    let (handle, view) = open(cx, two_tracks());
    click(cx, handle, "inspector-mute");
    assert!(project_of(cx, &view).tracks()[0].mute());
    click(cx, handle, "inspector-solo");
    assert!(project_of(cx, &view).tracks()[0].solo());
    // 버튼은 실행 함수를 거친다: 쌓인 카운트가 비워진다
    press(cx, handle, "3");
    click(cx, handle, "inspector-mute");
    assert_eq!(cx.read(keymap::pending_count), None);
    assert!(!project_of(cx, &view).tracks()[0].mute());
    // 엔진이 없으면 테스트 버튼은 알린다
    let before = messages(cx, &view).len();
    click(cx, handle, "inspector-test");
    assert!(messages(cx, &view).len() > before);
    // 버튼 뒤에도 모드 키가 산다
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
}

// ---------- 9. 테스트 노트(루프백) ----------

/// 이름 포트 목적지 하나를 만들고 받은 메시지를 돌려준다.
struct Destination {
    name: String,
    rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

fn destination(tag: &str) -> Destination {
    let name = format!("KS inspector {} {tag}", std::process::id());
    let client = coremidi::Client::new(&format!("ks-inspector-{tag}")).unwrap();
    let (tx, rx) = mpsc::channel();
    let dest = client
        .virtual_destination(&name, move |list: &coremidi::PacketList| {
            for p in list {
                let _ = tx.send(p.data().to_vec());
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !keyscore_midi::destinations()
        .iter()
        .any(|d| d.name() == name)
    {
        assert!(Instant::now() < deadline, "목적지가 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    }
    Destination {
        name,
        rx,
        _dest: dest,
        _client: client,
    }
}

fn received(rx: &mpsc::Receiver<Vec<u8>>, until: impl Fn(&[Vec<u8>]) -> bool) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut got = Vec::new();
    while Instant::now() < deadline && !until(&got) {
        if let Ok(m) = rx.recv_timeout(Duration::from_millis(20)) {
            got.push(m);
        }
    }
    got
}

#[gpui_kit::test]
fn send_test_note_reaches_track_port(cx: &mut TestAppContext) {
    start(cx, "test-note");
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: "ks-inspector".into(),
                port_name: format!("KS inspector out {}", std::process::id()),
            },
        )
    })
    .unwrap();
    let dest = destination("a");
    let p = ksq::from_str(&format!(
        r#"{{"tracks": [{{"port": {{"named": "{}"}}, "channel": 3, "program": 40, "default_velocity": {},
            "clips": [{{"start": 0, "length": 15360, "notes": [[0, 240, 72, 51492]]}}]}}]}}"#,
        dest.name,
        Velocity::from_midi1(77).unwrap().get()
    ))
    .unwrap()
    .0;
    let (handle, view) = open(cx, p);
    run(cx, handle, midi::SendTestNote);
    let got = received(&dest.rx, |g| g.iter().any(|m| m[0] == 0x82));
    let flat: Vec<Vec<u8>> = got.iter().flat_map(|m| split(m)).collect();
    let program = flat.iter().position(|m| m == &[0xC2, 40]);
    let on = flat.iter().position(|m| m == &[0x92, 60, 77]);
    let off = flat.iter().position(|m| m[0] == 0x82 && m[1] == 60);
    assert!(
        program.is_some() && on.is_some() && off.is_some(),
        "{flat:?}"
    );
    assert!(program < on && on < off, "{flat:?}");
    assert!(
        messages(cx, &view)
            .last()
            .is_some_and(|m| m.contains("테스트 노트"))
    );
    // 재생 중에도 닿는다
    run(cx, handle, transport::Play);
    while dest.rx.try_recv().is_ok() {}
    run(cx, handle, midi::SendTestNote);
    let got = received(&dest.rx, |g| {
        g.iter().flat_map(|m| split(m)).any(|m| m == [0x92, 60, 77])
    });
    run(cx, handle, transport::Stop);
    assert!(
        got.iter()
            .flat_map(|m| split(m))
            .any(|m| m == [0x92, 60, 77]),
        "{got:?}"
    );
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

// ---------- 12. 창 비활성화 ----------

#[gpui_kit::test]
fn inactive_window_keeps_field_text(cx: &mut TestAppContext) {
    start(cx, "inactive");
    let (handle, view) = open(cx, two_tracks());
    for (id, text) in [("inspector-name", "Keep"), ("transport-tempo", "95")] {
        click(cx, handle, id);
        press(cx, handle, "cmd-a backspace");
        type_text(cx, handle, text);
        gpui_kit::VisualTestContext::from_window(handle, cx).deactivate_window();
        cx.run_until_parked();
        render(cx, handle);
        assert_eq!(shown(cx, handle, id).as_deref(), Some(text), "{id}");
        cx.update_window(handle, |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        render(cx, handle);
        assert_eq!(shown(cx, handle, id).as_deref(), Some(text), "{id}");
        press(cx, handle, "enter");
    }
    let p = project_of(cx, &view);
    assert_eq!(p.tracks()[0].name(), "Keep");
    assert_eq!(
        p.tempo_map().tempo_at(keyscore_core::Tick::ZERO),
        keyscore_core::Tempo::from_bpm(95).unwrap()
    );
}
