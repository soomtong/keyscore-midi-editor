//! S2-T5c 적대적 리뷰 2. 깨뜨리지 못한 시도를 남긴다. 깨진 시도는 `docs/reports/s2-t5c/review-2.md`
//! 부록에 있다.
//! 키는 `simulate_keystrokes`(키 바인딩을 거침), 클릭은 `window.click`, 메뉴 막대는 포커스로의 Action 디스패치다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{midi, transport};
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Program, Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review2-t5c-{}-{name}", std::process::id()));
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

fn bounds(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    id: &'static str,
) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    cx.update_window(handle, |_, window, _| window.find(id).bounds())
        .unwrap()
}

fn click_point(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: gpui_kit::Point<gpui_kit::Pixels>,
) {
    gpui_kit::VisualTestContext::from_window(handle, cx)
        .simulate_click(at, gpui_kit::Modifiers::none());
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

fn dirty_tempo(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, text);
}

// ---------- 리뷰 1 지적 1 수정(바깥 처리기)의 회귀 찾기 ----------

/// 주 뷰를 누르면 고치던 이름은 버려지고 모드 키가 산다(주 뷰의 `track_focus`와 바깥 처리기가 겹쳐도).
#[gpui_kit::test]
fn clicking_main_view_cancels_dirty_name(cx: &mut TestAppContext) {
    start(cx, "main-click");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Gone");
    click(cx, handle, "main-view");
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Lead"));
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
    assert_eq!(undo_len(cx, &view), len + 1);
}

/// 고치던 템포 입력란에서 인스펙터 이름 입력란을 누르면 포커스는 이름 입력란으로 간다(바깥 처리기가
/// 빼앗지 않는다). 템포는 버려진다.
#[gpui_kit::test]
fn clicking_inspector_input_is_not_stolen_by_content_handler(cx: &mut TestAppContext) {
    start(cx, "input-click");
    let (handle, view) = open(cx, two_tracks());
    dirty_tempo(cx, handle, "95");
    click(cx, handle, "inspector-name");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, "Xm");
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Xm"));
    assert!(!project_of(cx, &view).tracks()[0].mute());
    assert_ne!(shown(cx, handle, "transport-tempo").as_deref(), Some("95"));
    press(cx, handle, "enter");
    assert_eq!(project_of(cx, &view).tracks()[0].name(), "Xm");
}

/// Vim: 고치던 뱅크 입력란(`12`)에서 인스펙터 빈 자리를 누른 뒤 `)`는 카운트 없이 트랙 하나만 넘긴다.
#[gpui_kit::test]
fn vim_click_inspector_background_then_motion(cx: &mut TestAppContext) {
    start(cx, "vim-bg");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let p = ksq::from_str(
        r#"{"tracks": [{"name": "A"}, {"name": "B"}, {"name": "C"}, {"name": "D"}]}"#,
    )
    .unwrap()
    .0;
    let (handle, view) = open(cx, p);
    click(cx, handle, "inspector-bank");
    type_text(cx, handle, "12");
    click(cx, handle, "inspector");
    assert_eq!(cx.read(keymap::pending_count), None);
    assert_eq!(shown(cx, handle, "inspector-bank").as_deref(), Some(""));
    press(cx, handle, ")");
    assert_eq!(current_index(cx, &view), Some(1));
}

/// 트랙 목록 줄 클릭은 트랙 목록 처리기와 바깥 처리기가 모두 문서로 포커스를 옮긴다. 편집이 생기지 않고
/// 모드 키는 한 번만 먹는다.
#[gpui_kit::test]
fn track_row_click_double_focus_is_harmless(cx: &mut TestAppContext) {
    start(cx, "row-double");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Zz");
    click(cx, handle, "track-row-1");
    assert_eq!(undo_len(cx, &view), len);
    press(cx, handle, "m");
    let p = project_of(cx, &view);
    assert!(p.tracks()[1].mute() && !p.tracks()[0].mute());
    assert_eq!(undo_len(cx, &view), len + 1);
}

/// 뮤트 토글 클릭: 고치던 이름은 확정되지 않고, 뮤트 한 번(undo 한 단위), 뒤의 `m`은 다시 끈다.
#[gpui_kit::test]
fn clicking_mute_toggle_with_dirty_name(cx: &mut TestAppContext) {
    start(cx, "mute-toggle");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Zz");
    click(cx, handle, "inspector-mute");
    let p = project_of(cx, &view);
    assert!(p.tracks()[0].mute());
    assert_eq!(p.tracks()[0].name(), "Lead");
    assert_eq!(undo_len(cx, &view), len + 1);
    press(cx, handle, "m");
    assert!(!project_of(cx, &view).tracks()[0].mute());
}

/// 프로그램 드롭다운 목록을 마우스로 고르면(목록은 `content` 위에 뜬다) 편집 한 단위, 문서로 포커스.
#[gpui_kit::test]
fn program_list_mouse_pick_edits_once_and_returns(cx: &mut TestAppContext) {
    start(cx, "program-mouse");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    let b = bounds(cx, handle, "inspector-program");
    click(cx, handle, "inspector-program");
    // 「없음」, 1, 2, … 순서. 세 번째 줄 = 2 Bright Acoustic Piano
    click_point(
        cx,
        handle,
        gpui_kit::point(
            b.origin.x + gpui_kit::px(20.),
            b.origin.y + b.size.height / 2. + gpui_kit::px(120.),
        ),
    );
    assert_eq!(
        project_of(cx, &view).tracks()[0].program(),
        Some(Program::new(1).unwrap())
    );
    assert_eq!(undo_len(cx, &view), len + 1);
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
}

/// 고치던 이름에서 색 메뉴를 열고 항목을 누르면 색만 바뀌고(한 단위) 이름은 버려진다.
#[gpui_kit::test]
fn color_menu_click_with_dirty_name(cx: &mut TestAppContext) {
    start(cx, "color-dirty");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Zz");
    click(cx, handle, "inspector-color");
    click_menu_item(cx, handle, 4);
    let p = project_of(cx, &view);
    assert_eq!(
        p.tracks()[0].color(),
        keyscore_core::TrackColor::new(4).unwrap()
    );
    assert_eq!(p.tracks()[0].name(), "Lead");
    assert_eq!(undo_len(cx, &view), len + 1);
    press(cx, handle, "m");
    assert!(project_of(cx, &view).tracks()[0].mute());
}

/// 고치던 이름에서 상태줄 모드 표시를 눌러 메뉴 항목(STEP)을 고르면 모드가 바뀌고 이름은 확정되지 않는다.
#[gpui_kit::test]
fn mode_menu_click_with_dirty_name(cx: &mut TestAppContext) {
    start(cx, "mode-dirty");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    dirty_name(cx, handle, "Zz");
    click(cx, handle, "status-mode");
    assert!(cx.read(|cx| view.read(cx).mode_menu_open()));
    click_menu_item(cx, handle, 1);
    assert_eq!(label(cx, handle, "status-mode").as_deref(), Some("STEP"));
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("Lead"));
    assert_eq!(undo_len(cx, &view), len);
}

// ---------- 같은 값·Action ----------

/// 뱅크·프로그램이 없는 트랙에 `SetProgram{bank:-1, program:0}`(둘 다 없음)은 편집이 없다.
#[gpui_kit::test]
fn set_program_none_none_is_no_edit(cx: &mut TestAppContext) {
    start(cx, "program-none");
    let p = ksq::from_str(r#"{"tracks": [{"name": "E"}]}"#).unwrap().0;
    let (handle, view) = open(cx, p);
    let len = undo_len(cx, &view);
    let action = cx
        .update(|cx| {
            cx.build_action(
                "track::SetProgram",
                Some(serde_json::json!({"bank": -1, "program": 0})),
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(undo_len(cx, &view), len);
}

/// `SetProgram`의 범위 밖 값(`i64::MAX`, 129, -1)은 메시지만 남긴다.
#[gpui_kit::test]
fn set_program_out_of_range_reports(cx: &mut TestAppContext) {
    start(cx, "program-big");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    for program in [i64::MAX.to_string(), "129".to_owned(), "-1".to_owned()] {
        let action = cx
            .update(|cx| {
                cx.build_action(
                    "track::SetProgram",
                    Some(serde_json::json!({"bank": 0, "program": program})),
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
            .unwrap();
        cx.run_until_parked();
        assert_eq!(undo_len(cx, &view), len, "{program}");
        assert!(
            messages(cx, &view).last().unwrap().contains("프로그램"),
            "{program}"
        );
    }
}

// ---------- 테스트 노트 ----------

fn install_engine(cx: &mut TestAppContext, tag: &str) -> String {
    let port = format!("KS review2 t5c out {} {tag}", std::process::id());
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-review2-t5c-{tag}"),
                port_name: port.clone(),
            },
        )
    })
    .unwrap();
    port
}

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
    let client = coremidi::Client::new("ks-review2-t5c-listen").unwrap();
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

/// 다른 창이 가상 포트로 재생 중일 때 뱅크가 있는 트랙의 테스트 노트도 뱅크 선택(CC0·CC32)을 보내지 않는다.
#[gpui_kit::test]
fn test_note_while_other_plays_sends_no_bank(cx: &mut TestAppContext) {
    start(cx, "note-other-bank");
    let port = install_engine(cx, "other-bank");
    let l = listen(&port);
    let playing = ksq::from_str(
        r#"{"tracks": [{"channel": 3, "clips": [{"start": 0, "length": 15360, "notes": [[0, 240, 72, 51492]]}]}]}"#,
    )
    .unwrap()
    .0;
    let (h1, _v1) = open(cx, playing);
    run(cx, h1, transport::Play);
    let p =
        ksq::from_str(r#"{"tracks": [{"name": "T", "channel": 3, "program": 40, "bank": 129}]}"#)
            .unwrap()
            .0;
    let (h2, v2) = open(cx, p);
    while l.rx.try_recv().is_ok() {}
    run(cx, h2, midi::SendTestNote);
    let got = collect(&l.rx, |g| g.iter().any(|m| m[0] == 0x82 && m[1] == 60));
    run(cx, h1, transport::Stop);
    assert!(got.iter().any(|m| m[0] == 0x92 && m[1] == 60), "{got:?}");
    assert!(
        !got.iter()
            .any(|m| m[0] == 0xB2 && (m[1] == 0 || m[1] == 32)),
        "다른 창 재생 중에 뱅크 선택을 보냈다: {got:?}"
    );
    assert!(!got.iter().any(|m| m[0] == 0xC2), "{got:?}");
    assert!(messages(cx, &v2).last().unwrap().contains("보냈다"));
}

/// 다른 창이 재생하다 멈춘 뒤에는 다시 뱅크·프로그램이 먼저 나간다(주인이 남지 않는다).
#[gpui_kit::test]
fn test_note_after_other_stops_sends_program_again(cx: &mut TestAppContext) {
    start(cx, "note-after-stop");
    let port = install_engine(cx, "after-stop");
    let l = listen(&port);
    let playing = ksq::from_str(
        r#"{"tracks": [{"channel": 1, "clips": [{"start": 0, "length": 15360, "notes": [[0, 240, 72, 51492]]}]}]}"#,
    )
    .unwrap()
    .0;
    let (h1, _v1) = open(cx, playing);
    run(cx, h1, transport::Play);
    run(cx, h1, transport::Pause);
    let p = ksq::from_str(r#"{"tracks": [{"name": "T", "channel": 1, "program": 40}]}"#)
        .unwrap()
        .0;
    let (h2, _v2) = open(cx, p);
    while l.rx.try_recv().is_ok() {}
    run(cx, h2, midi::SendTestNote);
    let got = collect(&l.rx, |g| g.iter().any(|m| m[0] == 0x80 && m[1] == 60));
    assert!(got.iter().any(|m| m == &[0xC0, 40]), "{got:?}");
}

/// 정지 중 없는 이름 포트: 이유(없음)를 알리고, 이어서 테스트 노트를 보내지 못했다고 알린다. 상태줄은 마지막
/// 메시지를 보인다.
#[gpui_kit::test]
fn test_note_missing_named_port_reports_reason(cx: &mut TestAppContext) {
    start(cx, "note-missing-reason");
    install_engine(cx, "missing-reason");
    let p = ksq::from_str(
        r#"{"tracks": [{"name": "X", "port": {"named": "No Such Synth r2"}, "channel": 4},
            {"name": "Y", "port": {"named": "Other Missing r2"}, "channel": 5}]}"#,
    )
    .unwrap()
    .0;
    let (handle, view) = open(cx, p);
    run(cx, handle, midi::SendTestNote);
    let m = messages(cx, &view);
    assert!(
        m.iter()
            .any(|m| m.contains("No Such Synth r2") && m.contains("없어")),
        "{m:?}"
    );
    assert!(
        !m.iter().any(|m| m.contains("Other Missing r2")),
        "다른 트랙의 포트까지 알렸다: {m:?}"
    );
    let last = m.last().unwrap();
    assert!(
        last.contains("No Such Synth r2") && last.contains("보내지 못했다"),
        "{last}"
    );
    assert_eq!(
        label(cx, handle, "status-message").as_deref(),
        Some(last.as_str())
    );
}

// ---------- 리뷰 2 부록 A(지적 1·2) ----------

/// 상태줄 빈 자리를 눌러도 문서로 포커스가 돌아와야 한다. `main`(S2-T5b)에서는 상태줄이 겹 안이라 돌아왔다.
#[gpui_kit::test]
fn clicking_status_bar_returns_focus_to_document(cx: &mut TestAppContext) {
    start(cx, "status-click");
    let (handle, view) = open(cx, two_tracks());
    dirty_tempo(cx, handle, "95");
    click(cx, handle, "status-bar");
    press(cx, handle, "m");
    assert!(
        project_of(cx, &view).tracks()[0].mute(),
        "상태줄을 누른 뒤에도 포커스가 템포 입력란에 남는다: tempo={:?}",
        shown(cx, handle, "transport-tempo")
    );
}

/// 인스펙터 빈 자리를 오른쪽 버튼으로 눌러도 문서로 돌아와야 한다(GPUI `track_focus`는 모든 버튼에서 옮긴다).
#[gpui_kit::test]
fn right_clicking_inspector_background_returns_focus(cx: &mut TestAppContext) {
    start(cx, "inspector-right");
    let (handle, view) = open(cx, two_tracks());
    dirty_name(cx, handle, "Zz");
    cx.update_window(handle, |_, window, cx| window.right_click("inspector", cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "m");
    assert!(
        project_of(cx, &view).tracks()[0].mute(),
        "오른쪽 클릭 뒤에도 포커스가 이름 입력란에 남는다: name={:?}",
        shown(cx, handle, "inspector-name")
    );
}
