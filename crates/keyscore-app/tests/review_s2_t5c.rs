//! S2-T5c 적대적 리뷰 1. 깨뜨리지 못한 시도를 남긴다. 깨진 시도는 `docs/reports/s2-t5c/review-1.md`
//! 부록에 있다.
//! 키는 `simulate_keystrokes`(키 바인딩을 거침), 클릭은 `window.click`, 메뉴 막대는 포커스로의 Action 디스패치다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, midi, track, transport};
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Program, Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review1-t5c-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
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

fn current_index(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<usize> {
    cx.read(|cx| {
        let w = view.read(cx);
        let id = w.current_track()?;
        w.document()
            .read(cx)
            .project()
            .tracks()
            .iter()
            .position(|t| t.id() == id)
    })
}

fn two_tracks() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1, "program": 0},
            {"name": "Bass", "channel": 2, "program": 33, "bank": 5}]}"#,
    )
    .unwrap()
    .0
}

/// 이름 입력란을 눌러 지우고 글자를 친다(Enter는 치지 않는다).
fn dirty_name(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    click(cx, handle, "inspector-name");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, text);
}

// ---------- 입력란의 키 ----------

/// KM-14: 이름 입력란의 `cmd-z`는 입력란의 undo이고 문서 undo가 아니다.
#[gpui_kit::test]
fn cmd_z_in_inspector_name_undoes_text_not_document(cx: &mut TestAppContext) {
    start(cx, "cmdz");
    let (handle, view) = open(cx, two_tracks());
    run(cx, handle, track::Rename("Pad".into()));
    let len = undo_len(cx, &view);
    click(cx, handle, "inspector-name");
    type_text(cx, handle, "X");
    press(cx, handle, "cmd-z");
    assert_eq!(undo_len(cx, &view), len);
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "Pad");
}

/// 이름 입력란의 `tab`은 트랙을 바꾸지 않고(Default의 `tab`=`track::Next`는 Normal 안), 포커스도
/// 트랜스포트 입력란으로 넘기지 않는다(`root::Tab` 삼킴).
#[gpui_kit::test]
fn tab_in_inspector_name_does_not_switch_track(cx: &mut TestAppContext) {
    start(cx, "tab");
    let (handle, view) = open(cx, two_tracks());
    dirty_name(cx, handle, "New");
    press(cx, handle, "tab");
    assert_eq!(current_index(cx, &view), Some(0));
    let tempo_focused = cx
        .update_window(handle, |_, window, cx| {
            view.read(cx).transport_tempo_focused(window, cx)
        })
        .unwrap();
    assert!(!tempo_focused);
    press(cx, handle, "enter");
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "New");
}

/// 고치던 이름 입력란에서 `ctrl-tab`(전역 모드 순환)이면 입력을 버리고 문서로 돌아간다.
#[gpui_kit::test]
fn ctrl_tab_in_dirty_inspector_field_cancels_and_returns(cx: &mut TestAppContext) {
    start(cx, "ctrltab");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Gone");
    press(cx, handle, "ctrl-tab");
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Lead"));
    assert_eq!(undo_len(cx, &view), len);
    // 문서 포커스: 모드를 Normal로 돌린 뒤 m이 뮤트한다
    press(cx, handle, "ctrl-shift-tab");
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
}

/// Vim 프리셋: 뱅크 입력란에 숫자·`j`·`k`·`space`·`)`를 쳐도 카운트·트랙 이동·재생이 없다.
#[gpui_kit::test]
fn vim_keys_in_bank_field_stay_in_field(cx: &mut TestAppContext) {
    start(cx, "vim-bank");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, two_tracks());
    click(cx, handle, "inspector-bank");
    press(cx, handle, "cmd-a backspace");
    press(cx, handle, "1 2 j k space )");
    assert_eq!(cx.read(keymap::pending_count), None);
    assert_eq!(current_index(cx, &view), Some(0));
    assert_eq!(
        cx.read(|cx| view.read(cx).transport_state()),
        engine::TransportState::Stopped
    );
    // 잘못된 값은 메시지, 편집 없음
    let len = undo_len(cx, &view);
    press(cx, handle, "enter");
    assert_eq!(undo_len(cx, &view), len);
    assert!(
        messages(cx, &view)
            .last()
            .is_some_and(|m| m.contains("뱅크"))
    );
}

/// Vim 프리셋: 프로그램 `Select` 검색란에 친 `j`·`k`·`space`·숫자도 모드로 빠지지 않는다.
#[gpui_kit::test]
fn vim_keys_in_program_search_stay_in_select(cx: &mut TestAppContext) {
    start(cx, "vim-select");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, two_tracks());
    click(cx, handle, "inspector-program");
    press(cx, handle, "3 j k");
    assert_eq!(cx.read(keymap::pending_count), None);
    assert_eq!(current_index(cx, &view), Some(0));
    press(cx, handle, "escape");
    // Esc로 닫은 뒤 문서 포커스: Vim의 `)`는 track::Next
    press(cx, handle, ")");
    assert_eq!(current_index(cx, &view), Some(1));
}

// ---------- 현재 트랙이 바뀌는 경로 ----------

/// 고치던 이름 입력란이 있는 채로 트랙 목록에서 다른 줄을 누르면 글자는 버려지고, 그 글자가 새 트랙에
/// 확정되지 않는다.
#[gpui_kit::test]
fn dirty_name_then_track_row_click_does_not_leak(cx: &mut TestAppContext) {
    start(cx, "row-click");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Leak");
    click(cx, handle, "track-row-1");
    assert_eq!(current_index(cx, &view), Some(1));
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Bass"));
    press(cx, handle, "enter");
    let p = project_of(cx, &view);
    assert_eq!(p.tracks()[0].name(), "Lead");
    assert_eq!(p.tracks()[1].name(), "Bass");
    assert_eq!(undo_len(cx, &view), len);
}

/// `cmd-t`(전역 `track::New`)를 고치던 이름 입력란에서 치면 새 트랙이 생긴다. 그 뒤 Enter가 엉뚱한
/// 트랙 이름을 바꾸지 않는다.
#[gpui_kit::test]
fn cmd_t_from_dirty_name_field_keeps_names_consistent(cx: &mut TestAppContext) {
    start(cx, "cmd-t");
    let (handle, view) = open(cx, two_tracks());
    dirty_name(cx, handle, "Typed");
    let before_current = current_index(cx, &view);
    press(cx, handle, "cmd-t");
    let after_current = current_index(cx, &view);
    let p = project_of(cx, &view);
    assert_eq!(p.tracks().len(), 3);
    press(cx, handle, "enter");
    let p = project_of(cx, &view);
    if before_current == after_current {
        // 현재 트랙이 그대로면 고치던 글자를 그 트랙에 확정해도 된다
        assert_eq!(p.tracks()[0].name(), "Typed");
    } else {
        // 현재 트랙이 바뀌었으면 고치던 글자는 버려져야 한다
        assert!(p.tracks().iter().all(|t| t.name() != "Typed"), "{p:?}");
    }
}

/// undo가 현재 트랙을 없애면 인스펙터와 상태줄이 남은 트랙을 보인다.
#[gpui_kit::test]
fn undo_removing_current_track_moves_inspector_and_status(cx: &mut TestAppContext) {
    start(cx, "undo-remove");
    let (handle, view) = open(cx, two_tracks());
    click(cx, handle, "track-add");
    assert_eq!(project_of(cx, &view).tracks().len(), 3);
    run(cx, handle, track::Next);
    run(cx, handle, track::Next);
    run(cx, handle, track::Rename("Third".into()));
    assert_eq!(
        shown(cx, handle, "inspector-name").as_deref(),
        Some("Third")
    );
    run(cx, handle, edit::Undo);
    run(cx, handle, edit::Undo);
    assert_eq!(project_of(cx, &view).tracks().len(), 2);
    let idx = current_index(cx, &view).unwrap();
    let name = project_of(cx, &view).tracks()[idx].name().to_owned();
    assert_eq!(
        shown(cx, handle, "inspector-name").as_deref(),
        Some(name.as_str())
    );
    assert!(
        label(cx, handle, "status-track")
            .unwrap()
            .starts_with(&format!("{name} · "))
    );
    let program = project_of(cx, &view).tracks()[idx].program();
    assert_eq!(
        label(cx, handle, "inspector-program"),
        Some(keyscore_app::gm::program_label(program))
    );
}

// ---------- 같은 값·Select ----------

/// 프로그램 `Select`에서 지금 값을 다시 고르면 편집이 없다.
#[gpui_kit::test]
fn select_same_program_is_no_edit(cx: &mut TestAppContext) {
    start(cx, "select-same");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    click(cx, handle, "inspector-program");
    type_text(cx, handle, "Acoustic Grand");
    press(cx, handle, "enter");
    assert_eq!(undo_len(cx, &view), len);
    assert_eq!(
        project_of(cx, &view).tracks()[0].program(),
        Some(Program::new(0).unwrap())
    );
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
}

/// 이름 앞뒤 공백만 다른 값은 같은 값이다.
#[gpui_kit::test]
fn name_with_surrounding_spaces_is_same_value(cx: &mut TestAppContext) {
    start(cx, "name-spaces");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "  Lead ");
    press(cx, handle, "enter");
    assert_eq!(undo_len(cx, &view), len);
    run(cx, handle, track::Rename(" Lead".into()));
    assert_eq!(undo_len(cx, &view), len);
}

// ---------- 상태줄 ----------

/// 이름·채널 편집 뒤 undo하면 상태줄이 되돌아간다.
#[gpui_kit::test]
fn status_bar_follows_undo(cx: &mut TestAppContext) {
    start(cx, "status-undo");
    let (handle, _view) = open(cx, two_tracks());
    let before = label(cx, handle, "status-track").unwrap();
    run(cx, handle, track::Rename("Pad".into()));
    run(
        cx,
        handle,
        track::SetChannel(keyscore_app::actions::args::Int(9)),
    );
    assert!(label(cx, handle, "status-track").unwrap().contains("Pad"));
    run(cx, handle, edit::Undo);
    run(cx, handle, edit::Undo);
    assert_eq!(label(cx, handle, "status-track").unwrap(), before);
}

/// 키맵 오류가 있는 동안에는 메시지가 가려지고, 고치면 마지막 메시지가 다시 보인다.
#[gpui_kit::test]
fn keymap_error_hides_then_restores_message(cx: &mut TestAppContext) {
    let dir = start(cx, "keymap-msg");
    let (handle, view) = open(cx, two_tracks());
    run(cx, handle, track::Rename(String::new()));
    let last = messages(cx, &view).last().cloned().unwrap();
    let path = dir.join("keymap.json");
    std::fs::write(&path, "[{\"bindings\": {\n \"f6\": \"no::Such\"\n}}]").unwrap();
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
    render(cx, handle);
    assert!(label(cx, handle, "status-keymap-error").is_some());
    assert_eq!(label(cx, handle, "status-message"), None);
    std::fs::write(&path, "[]").unwrap();
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(label(cx, handle, "status-keymap-error"), None);
    assert_eq!(label(cx, handle, "status-message"), Some(last));
}

// ---------- 창 비활성화 ----------

/// 비활성화 뒤 돌아와 다른 곳(트랙 목록)을 누르면 그때는 취소된다.
#[gpui_kit::test]
fn inactive_then_click_elsewhere_cancels(cx: &mut TestAppContext) {
    start(cx, "inactive-click");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Kept");
    gpui_kit::VisualTestContext::from_window(handle, cx).deactivate_window();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    click(cx, handle, "track-row-0");
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Lead"));
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
    assert_eq!(undo_len(cx, &view), len + 1);
}

// ---------- 테스트 노트 ----------

fn install_engine(cx: &mut TestAppContext, tag: &str) -> String {
    let port = format!("KS review t5c out {} {tag}", std::process::id());
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-review-t5c-{tag}"),
                port_name: port.clone(),
            },
        )
    })
    .unwrap();
    port
}

/// 엔진 가상 포트를 듣는 입력.
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
        assert!(Instant::now() < deadline, "가상 포트가 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    };
    let client = coremidi::Client::new("ks-review-t5c-listen").unwrap();
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

/// 없는 이름 포트 트랙의 테스트 노트는 보내지 않고 알린다(정지 중).
#[gpui_kit::test]
fn test_note_to_missing_named_port_reports(cx: &mut TestAppContext) {
    start(cx, "note-missing");
    install_engine(cx, "missing");
    let p = ksq::from_str(
        r#"{"tracks": [{"name": "X", "port": {"named": "No Such Synth zz"}, "channel": 4}]}"#,
    )
    .unwrap()
    .0;
    let (handle, view) = open(cx, p);
    run(cx, handle, midi::SendTestNote);
    let last = messages(cx, &view).last().cloned().unwrap();
    assert!(last.contains("No Such Synth zz"), "{last}");
    assert!(!last.contains("보냈다:"), "{last}");
    assert_eq!(label(cx, handle, "status-message"), Some(last));
}

/// 뮤트된 트랙도 가상 포트로 테스트 노트가 나간다(연결 테스트는 뮤트와 무관). 정지 중이라 프로그램이
/// 먼저 간다.
#[gpui_kit::test]
fn test_note_on_muted_virtual_track_sends_program_then_note(cx: &mut TestAppContext) {
    start(cx, "note-muted");
    let port = install_engine(cx, "muted");
    let l = listen(&port);
    let p =
        ksq::from_str(r#"{"tracks": [{"name": "M", "channel": 5, "program": 7, "mute": true}]}"#)
            .unwrap()
            .0;
    let (handle, _view) = open(cx, p);
    run(cx, handle, midi::SendTestNote);
    let got = collect(&l.rx, |g| g.iter().any(|m| m[0] == 0x84 && m[1] == 60));
    let program = got.iter().position(|m| m == &[0xC4, 7]);
    let on = got.iter().position(|m| m[0] == 0x94 && m[1] == 60);
    assert!(program.is_some() && on.is_some() && program < on, "{got:?}");
}

/// 다른 창이 재생 중이고 이름 포트면 보내지 않고 알린다.
#[gpui_kit::test]
fn test_note_named_port_while_other_window_plays_is_refused(cx: &mut TestAppContext) {
    start(cx, "note-other-named");
    install_engine(cx, "other-named");
    let playing = ksq::from_str(
        r#"{"tracks": [{"channel": 1, "clips": [{"start": 0, "length": 15360, "notes": [[0, 240, 72, 51492]]}]}]}"#,
    )
    .unwrap()
    .0;
    let (h1, _v1) = open(cx, playing);
    run(cx, h1, transport::Play);
    let p = ksq::from_str(
        r#"{"tracks": [{"name": "N", "port": {"named": "Whatever"}, "channel": 2}]}"#,
    )
    .unwrap()
    .0;
    let (h2, v2) = open(cx, p);
    run(cx, h2, midi::SendTestNote);
    let last = messages(cx, &v2).last().cloned().unwrap();
    assert!(last.contains("다른 창이 재생 중"), "{last}");
    run(cx, h1, transport::Stop);
}

// ---------- 그 밖의 깨뜨리기 시도 ----------

/// 메뉴 막대의 보기 > 인스펙터(`view::ToggleInspector`)로 고치던 이름 입력란이 있는 인스펙터를 닫아도 모드
/// 키가 살고, 다시 열면 입력란은 문서 값이다.
#[gpui_kit::test]
fn hiding_inspector_from_field_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "hide-inspector");
    let (handle, view) = open(cx, two_tracks());
    dirty_name(cx, handle, "Ghost");
    run(cx, handle, keyscore_app::actions::view::ToggleInspector);
    assert!(label(cx, handle, "inspector").is_none());
    press(cx, handle, "m");
    assert!(
        project_of(cx, &view).tracks()[0].mute(),
        "인스펙터를 닫은 뒤 모드 키 m이 먹지 않는다"
    );
    run(cx, handle, keyscore_app::actions::view::ToggleInspector);
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Lead"));
}

/// 16비트 기본 벨로시티(51492)에 화면에 보이는 값 그대로 `track::SetDefaultVelocity`를 주면 편집이 없다.
#[gpui_kit::test]
fn set_default_velocity_same_displayed_value_is_no_edit(cx: &mut TestAppContext) {
    start(cx, "vel-same");
    let p = ksq::from_str(r#"{"tracks": [{"name": "V", "default_velocity": 51492}]}"#)
        .unwrap()
        .0;
    let (handle, view) = open(cx, p);
    let shown_vel = shown(cx, handle, "inspector-velocity").unwrap();
    let len = undo_len(cx, &view);
    run(
        cx,
        handle,
        track::SetDefaultVelocity(keyscore_app::actions::args::Int(shown_vel.parse().unwrap())),
    );
    assert_eq!(
        undo_len(cx, &view),
        len,
        "표시 {shown_vel}와 같은 값인데 편집이 생겼다"
    );
}

/// 프로그램 드롭다운이 열린 채로 트랙 목록의 다른 줄을 누르면 그 클릭은 드롭다운을 닫는 데 쓰이고(팝오버의
/// 바깥 클릭), 포커스는 문서로 돌아가 모드 키가 산다.
#[gpui_kit::test]
fn open_select_then_track_row_click_keeps_mode_keys(cx: &mut TestAppContext) {
    start(cx, "select-row");
    let (handle, view) = open(cx, two_tracks());
    click(cx, handle, "inspector-program");
    click(cx, handle, "track-row-1");
    press(cx, handle, "m");
    let idx = current_index(cx, &view).unwrap();
    assert!(project_of(cx, &view).tracks()[idx].mute());
}
