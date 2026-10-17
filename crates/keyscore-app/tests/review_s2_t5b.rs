//! S2-T5b 적대적 리뷰 1(트랜스포트 바와 트랙 목록). 깨뜨리지 못한 시도와 진단을 남긴다.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{Opt, Real};
use keyscore_app::actions::{edit, track, transport};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, Tempo, Tick, TimeSignature, TrackId, ksq};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-t5b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
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
    cx.run_until_parked();
    render(cx, handle);
}

fn project_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Project {
    cx.read(|cx| view.read(cx).document().read(cx).project().clone())
}

fn ids(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<TrackId> {
    project_of(cx, view)
        .tracks()
        .iter()
        .map(keyscore_core::Track::id)
        .collect()
}

fn current(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<TrackId> {
    cx.read(|cx| view.read(cx).current_track())
}

fn tempo(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Tempo {
    project_of(cx, view).tempo_map().tempo_at(Tick::ZERO)
}

fn three_tracks() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "A", "channel": 1}, {"name": "B", "channel": 2},
            {"name": "C", "channel": 3}]}"#,
    )
    .unwrap()
    .0
}

// ---------- 현재 트랙 ----------

/// 트랙 순서 이동 뒤에도 현재 트랙은 같은 트랙이고, undo·redo가 순서를 되돌린다.
#[gpui_kit::test]
fn move_keeps_current_track_and_undo_restores_order(cx: &mut TestAppContext) {
    start(cx, "move");
    let (handle, view) = open(cx, three_tracks());
    let before = ids(cx, &view);
    run(cx, handle, track::Next);
    run(cx, handle, track::MoveUp);
    let moved = ids(cx, &view);
    assert_eq!(moved, vec![before[1], before[0], before[2]]);
    assert_eq!(current(cx, &view), Some(before[1]));
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view), before);
    assert_eq!(current(cx, &view), Some(before[1]));
    run(cx, handle, edit::Redo);
    assert_eq!(ids(cx, &view), moved);
    // 맨 위에서 위로, 맨 아래에서 아래로는 아무것도 하지 않고 undo 단위도 만들지 않는다
    run(cx, handle, cursor_first());
    run(cx, handle, track::MoveUp);
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view), before);
}

fn cursor_first() -> keyscore_app::actions::cursor::FirstTrack {
    keyscore_app::actions::cursor::FirstTrack
}

/// 모든 트랙을 지우면 현재 트랙이 없음이 되고, 그때 트랙 Action은 아무것도 하지 않는다. undo로
/// 돌아오면 다시 현재 트랙이 생긴다.
#[gpui_kit::test]
fn deleting_all_tracks_leaves_no_current_and_undo_restores(cx: &mut TestAppContext) {
    start(cx, "delete-all");
    let (handle, view) = open(cx, three_tracks());
    for _ in 0..3 {
        run(cx, handle, track::Delete);
    }
    assert!(ids(cx, &view).is_empty());
    assert_eq!(current(cx, &view), None);
    run(cx, handle, track::Delete);
    run(cx, handle, track::ToggleMute);
    run(cx, handle, track::MoveDown);
    run(cx, handle, track::Next);
    run(cx, handle, edit::Undo);
    assert_eq!(ids(cx, &view).len(), 1);
    let c = current(cx, &view);
    assert!(c.is_some() && ids(cx, &view).contains(&c.unwrap()));
}

/// M/S 버튼은 그 줄의 트랙을 현재 트랙으로 삼고 그 트랙만 바꾼다.
#[gpui_kit::test]
fn solo_button_targets_its_row(cx: &mut TestAppContext) {
    start(cx, "solo-row");
    let (handle, view) = open(cx, three_tracks());
    click(cx, handle, "track-solo-2");
    let p = project_of(cx, &view);
    assert!(p.tracks()[2].solo());
    assert!(!p.tracks()[0].solo());
    assert_eq!(current(cx, &view), Some(p.tracks()[2].id()));
}

// ---------- SEQ-7 ----------

/// 이름 포트를 쓰는 트랙의 채널은 기본 가상 포트의 빈 채널 계산에 들어가지 않는다.
#[gpui_kit::test]
fn named_port_channels_do_not_block_virtual_channels(cx: &mut TestAppContext) {
    start(cx, "named-port");
    let p = ksq::from_str(r#"{"tracks": [{"port": {"named": "Other"}, "channel": 1}]}"#)
        .unwrap()
        .0;
    let (handle, view) = open(cx, p);
    run(cx, handle, track::New);
    assert_eq!(project_of(cx, &view).tracks()[1].channel().get(), 1);
}

// ---------- 트랜스포트 필드 ----------

/// 범위 끝 값과 범위 밖 값.
#[gpui_kit::test]
fn set_tempo_action_bounds(cx: &mut TestAppContext) {
    start(cx, "tempo-bounds");
    let (handle, view) = open(cx, Project::new());
    let set = |bpm: f64| transport::SetTempo(Opt(Some(Real(bpm))));
    run(cx, handle, set(999.9));
    assert!((tempo(cx, &view).bpm_f64() - 999.9).abs() < 0.05);
    run(cx, handle, set(1000.0));
    assert!((tempo(cx, &view).bpm_f64() - 999.9).abs() < 0.05);
    run(cx, handle, set(f64::NAN));
    assert!((tempo(cx, &view).bpm_f64() - 999.9).abs() < 0.05);
    run(cx, handle, set(9.9));
    assert!((tempo(cx, &view).bpm_f64() - 999.9).abs() < 0.05);
}

/// 탭 템포: 2초 넘게 쉬면 새로 센다(쉰 시간이 평균에 들어가지 않는다).
#[gpui_kit::test]
fn tap_tempo_resets_after_pause(cx: &mut TestAppContext) {
    start(cx, "tap-reset");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tap");
    std::thread::sleep(Duration::from_millis(2100));
    click(cx, handle, "transport-tap");
    // 쉰 뒤의 첫 탭만으로는 템포를 정하지 않는다
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
}

// ---------- 결정 C ----------

/// 트랜스포트·트랙 목록의 버튼은 쌓인 카운트를 비운다.
#[gpui_kit::test]
fn every_button_clears_count(cx: &mut TestAppContext) {
    start(cx, "buttons-count");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, _view) = open(cx, three_tracks());
    for id in [
        "transport-play",
        "transport-stop",
        "transport-metronome",
        "transport-tap",
        "track-mute-0",
        "track-solo-1",
        "track-add",
    ] {
        press(cx, handle, "3");
        click(cx, handle, id);
        assert_eq!(cx.update(keymap::take_count), None, "{id}");
    }
}

// ---------- 필드 키 입력 ----------

/// 템포 필드에 실제 키로 소수점을 친다.
#[gpui_kit::test]
fn typing_decimal_tempo_with_keys(cx: &mut TestAppContext) {
    start(cx, "type-decimal");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace 9 0 . 5 enter");
    assert!(
        (tempo(cx, &view).bpm_f64() - 90.5).abs() < 0.05,
        "{}",
        tempo(cx, &view).bpm_f64()
    );
}

/// 박자표 필드에 실제 키로 `6/8`을 친다.
#[gpui_kit::test]
fn typing_meter_with_keys(cx: &mut TestAppContext) {
    start(cx, "type-meter");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-meter");
    press(cx, handle, "cmd-a backspace 6 / 8 enter");
    assert_eq!(
        project_of(cx, &view).meter_map().signature_at(Tick::ZERO),
        TimeSignature::new(6, 8).unwrap()
    );
}

/// 조성 메뉴로 고르면 쌓인 카운트가 비워진다(결정 C, 툴바).
#[gpui_kit::test]
fn key_menu_choice_clears_count(cx: &mut TestAppContext) {
    start(cx, "key-count");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, _view) = open(cx, Project::new());
    press(cx, handle, "3");
    click(cx, handle, "transport-key");
    click_menu_item(cx, handle, 1);
    assert_eq!(cx.update(keymap::take_count), None);
}

// ---------- 실효 음색(루프백) ----------

mod loopback {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use gpui_kit::{AnyWindowHandle, Entity, TestAppContext};
    use keyscore_app::actions::args::{Opt, Real};
    use keyscore_app::actions::{track, transport};
    use keyscore_app::engine::{self, EngineOptions};
    use keyscore_app::init;
    use keyscore_app::window::ProjectWindow;
    use keyscore_core::{Project, ksq};

    pub(super) struct Loopback {
        pub(super) rx: mpsc::Receiver<Vec<u8>>,
        _input: coremidi::InputPort,
        _client: coremidi::Client,
    }

    pub(super) fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
        let dir = std::env::temp_dir().join(format!("ks-rv5b-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        cx.update(|cx| init(cx, dir.join("settings.json")));
        let port = format!("KS rv5b {} {name}", std::process::id());
        let options = EngineOptions {
            client_name: format!("ks-rv5b-{name}"),
            port_name: port.clone(),
        };
        cx.update(|cx| engine::install(cx, &options)).unwrap();
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-rv5b-rx").unwrap();
        let input = client
            .input_port("in", move |list: &coremidi::PacketList| {
                for p in list {
                    let data = p.data();
                    let mut i = 0;
                    while i < data.len() {
                        let len = match data[i] & 0xF0 {
                            0xC0 | 0xD0 => 2,
                            0xF0 => 1,
                            _ => 3,
                        };
                        let _ = tx.send(data[i..(i + len).min(data.len())].to_vec());
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

    pub(super) fn drain(rx: &mpsc::Receiver<Vec<u8>>, limit: Duration) -> Vec<Vec<u8>> {
        let mut got = Vec::new();
        let deadline = Instant::now() + limit;
        while let Ok(m) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            got.push(m);
        }
        got
    }

    pub(super) fn wait_note(rx: &mpsc::Receiver<Vec<u8>>, pitch: u8) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let m = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("Note On");
            if m.len() == 3 && m[0] & 0xF0 == 0x90 && m[1] == pitch && m[2] > 0 {
                return;
            }
        }
    }

    /// 채널 1을 나눠 쓰는 A(프로그램 5)·B(프로그램 9)와 채널 2의 C(프로그램 30).
    pub(super) fn shared() -> Project {
        let notes = (0..32)
            .map(|i| format!("[{}, 240, 60, 51492]", if i == 0 { 0 } else { 480 }))
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!(
            r#"{{"tempo": [[0, 500000]], "tracks": [
            {{"port": "virtual", "channel": 1, "program": 5,
             "clips": [{{"start": 0, "length": 15360, "notes": [{notes}]}}]}},
            {{"port": "virtual", "channel": 1, "program": 9}},
            {{"port": "virtual", "channel": 2, "program": 30}}]}}"#
        );
        ksq::from_str(&text).unwrap().0
    }

    pub(super) fn play(
        cx: &mut TestAppContext,
        handle: AnyWindowHandle,
        _view: &Entity<ProjectWindow>,
        lb: &Loopback,
    ) {
        super::run(cx, handle, transport::Play);
        wait_note(&lb.rx, 60);
    }

    pub(super) fn set_tempo() -> transport::SetTempo {
        transport::SetTempo(Opt(Some(Real(100.0))))
    }

    pub(super) fn mute() -> track::ToggleMute {
        track::ToggleMute
    }
}

/// 재생 중 공유 채널의 트랙 순서를 바꾸면(A를 B 아래로) 실효 음색이 A(5)로 바뀌어 나간다.
#[gpui_kit::test]
fn moving_shared_track_while_playing_sends_new_effective_program(cx: &mut TestAppContext) {
    let lb = loopback::start(cx, "move");
    let (handle, view) = open(cx, loopback::shared());
    loopback::play(cx, handle, &view, &lb);
    run(cx, handle, track::MoveDown);
    let got = loopback::drain(&lb.rx, Duration::from_millis(300));
    run(cx, handle, transport::Stop);
    assert!(got.iter().any(|m| m == &[0xC0, 5]), "{got:?}");
    assert!(!got.iter().any(|m| m == &[0xC1, 30]), "{got:?}");
}

/// 재생 중 템포를 바꾸거나 다른 채널 트랙을 뮤트해도 프로그램을 다시 보내지 않는다.
#[gpui_kit::test]
fn unrelated_edits_while_playing_send_no_program(cx: &mut TestAppContext) {
    let lb = loopback::start(cx, "unrelated");
    let (handle, view) = open(cx, loopback::shared());
    loopback::play(cx, handle, &view, &lb);
    run(cx, handle, loopback::set_tempo());
    run(cx, handle, keyscore_app::actions::cursor::LastTrack);
    run(cx, handle, loopback::mute());
    let got = loopback::drain(&lb.rx, Duration::from_millis(300));
    run(cx, handle, transport::Stop);
    assert!(!got.iter().any(|m| m[0] & 0xF0 == 0xC0), "{got:?}");
}

/// 재생 중 공유 채널의 뒤 트랙(B)을 다른 채널로 옮기면 채널 1은 A(5), 채널 3은 B(9)가 나간다.
#[gpui_kit::test]
fn changing_shared_track_channel_while_playing(cx: &mut TestAppContext) {
    let lb = loopback::start(cx, "channel");
    let (handle, view) = open(cx, loopback::shared());
    loopback::play(cx, handle, &view, &lb);
    run(cx, handle, track::Next);
    run(
        cx,
        handle,
        track::SetChannel(keyscore_app::actions::args::Int(3)),
    );
    let got = loopback::drain(&lb.rx, Duration::from_millis(300));
    run(cx, handle, transport::Stop);
    assert!(got.iter().any(|m| m == &[0xC0, 5]), "{got:?}");
    assert!(got.iter().any(|m| m == &[0xC2, 9]), "{got:?}");
}

// ---------- 리뷰 1 부록 A(지적 1·2·4): 고친 뒤 회귀 방지로 옮김 ----------

/// 템포 필드에서 `space`를 쳐도 재생이 바뀌지 않는다.
#[gpui_kit::test]
fn space_in_tempo_field_does_not_run_transport(cx: &mut TestAppContext) {
    start(cx, "space");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    let before = cx.read(|cx| view.read(cx).messages().len());
    press(cx, handle, "space");
    assert_eq!(cx.read(|cx| view.read(cx).messages().len()), before);
}

/// vim 프리셋에서 템포 필드에 숫자를 친다.
#[gpui_kit::test]
fn vim_digits_in_tempo_field(cx: &mut TestAppContext) {
    start(cx, "vim-digits");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace 9 0 enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
}

/// undo 뒤 필드가 문서 값을 보인다.
#[gpui_kit::test]
fn tempo_field_follows_undo(cx: &mut TestAppContext) {
    start(cx, "field-undo");
    let (handle, view) = open(cx, Project::new());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace");
    cx.simulate_input(handle, "90");
    press(cx, handle, "enter");
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
    run(cx, handle, edit::Undo);
    assert_eq!(tempo(cx, &view), Tempo::DEFAULT);
    let shown = cx
        .update_window(handle, |_, window, _| {
            window.find("transport-tempo").value().map(str::to_owned)
        })
        .unwrap();
    assert_eq!(shown.as_deref(), Some("120.0"));
}

/// 탭 템포 뒤 필드가 문서 값을 보인다.
#[gpui_kit::test]
fn tempo_field_follows_tap_and_set_tempo(cx: &mut TestAppContext) {
    start(cx, "field-tap");
    let (handle, view) = open(cx, Project::new());
    run(cx, handle, transport::SetTempo(Opt(Some(Real(90.0)))));
    assert_eq!(tempo(cx, &view), Tempo::from_bpm(90).unwrap());
    let shown = cx
        .update_window(handle, |_, window, _| {
            window.find("transport-tempo").value().map(str::to_owned)
        })
        .unwrap();
    assert_eq!(shown.as_deref(), Some("90.0"));
}

/// 새 트랙을 undo하면 현재 트랙은 이웃 트랙으로 간다(계획 「트랙이 지워지면 이웃 트랙으로」).
#[gpui_kit::test]
fn undo_of_new_below_moves_to_neighbour(cx: &mut TestAppContext) {
    start(cx, "undo-neighbour");
    let (handle, view) = open(cx, three_tracks());
    let before = ids(cx, &view);
    run(cx, handle, keyscore_app::actions::cursor::LastTrack);
    run(cx, handle, track::NewBelow);
    assert_eq!(ids(cx, &view).len(), 4);
    run(cx, handle, edit::Undo);
    assert_eq!(current(cx, &view), Some(before[2]));
}

/// 7.6 「패닉: 트랜스포트의 ⏹! 버튼」(리뷰 1 지적 3). 엔진이 없으면 메시지를 남긴다(Action이 닿았다는 뜻).
#[gpui_kit::test]
fn transport_panic_button_dispatches_panic(cx: &mut TestAppContext) {
    start(cx, "panic-button");
    let (handle, view) = open(cx, three_tracks());
    let before = cx.read(|cx| view.read(cx).messages().len());
    click(cx, handle, "transport-panic");
    assert!(cx.read(|cx| view.read(cx).messages().len()) > before);
}

/// 입력란 안의 `Tab`은 아무 일도 하지 않는다: 포커스를 옮기지 않고 트랙도 바꾸지 않는다(트랜스포트 바는
/// 모드 컨텍스트 밖이라 `track::Next`가 닿지 않고, `root::Tab`은 창이 삼킨다).
#[gpui_kit::test]
fn tab_inside_tempo_field_does_nothing(cx: &mut TestAppContext) {
    start(cx, "tab-in-field");
    let (handle, view) = open(cx, three_tracks());
    let before = current(cx, &view);
    click(cx, handle, "transport-tempo");
    press(cx, handle, "tab");
    assert_eq!(current(cx, &view), before);
    let tempo_focused = cx
        .update_window(handle, |_, window, cx| {
            view.read(cx).transport_tempo_focused(window, cx)
        })
        .unwrap();
    assert!(tempo_focused, "Tab이 템포 입력란의 포커스를 옮겼다");
}
