//! S3-T4c 적대적 리뷰 1회차의 깨뜨리기 시험(`docs/reports/s3-t4c/review-1.md`). 배치 뷰 Step 표시가 실제 쓰기와
//! 같은 말을 하는지, 정지 중 미리듣기의 기억(경로표·음색)이 어긋나지 않는지 본다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, LengthCode};
use keyscore_app::actions::{step, track, transport};
use keyscore_app::app_state::AppSettings;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, Note, OutputPort, Pitch, Project, Tick, Track, TrackColor, Velocity, ksq,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-t4c-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(handle, key);
        cx.run_until_parked();
        render(cx, handle);
    }
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    e: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| this.apply_mode_event(e, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn set_cursor(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    tick: u64,
) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(60).unwrap(),
        });
    });
    cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
    cx.run_until_parked();
    render(cx, handle);
}

fn open_window(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = KeyLayout::Solfege));
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn clip_text(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    label(cx, handle, "arrangement-step-clip").unwrap_or_default()
}

fn note(start: u64, length: u32) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(72).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 클립 하나: (시작, 길이, 이름, 클립 기준 노트(시작, 길이)).
type ClipSpec<'a> = (u64, u64, Option<&'a str>, &'a [(u64, u32)]);

/// 트랙 「Lead」 하나와 클립들.
fn arrangement(clips: &[ClipSpec<'_>]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    for &(s, l, name, notes) in clips {
        let mut clip = Clip::new(Tick::new(s), l).unwrap();
        for &(ns, nl) in notes {
            clip.insert_note(note(ns, nl)).unwrap();
        }
        if let Some(name) = name {
            clip.set_name(name);
        }
        track.insert_clip(clip).unwrap();
    }
    project.push_track(track).unwrap();
    project
}

/// 배치 뷰 Step 창.
fn open_arrangement_step(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
    cursor: u64,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = open_window(cx, p);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    set_cursor(cx, handle, &view, cursor);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

/// 트랙 0에서 절대 틱 `at`에 시작하는 노트가 있는가.
fn note_at(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, at: u64) -> bool {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track.clips().any(|(_, c)| {
            c.notes()
                .iter()
                .any(|n| c.start().get() + n.start().get() == at)
        })
    })
}

/// 트랙 0에서 `at`을 품는 클립의 마디 범위(1부터).
fn bars_of_clip_at(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, at: u64) -> (u64, u64) {
    cx.read(|cx| {
        let project = view.read(cx).document().read(cx).project();
        let meter = project.meter_map();
        let (_, c) = project.tracks()[0]
            .clips()
            .find(|(_, c)| c.start().get() <= at && at < c.end())
            .expect("쓴 노트를 품는 클립");
        let first = meter.position(c.start()).bar() + 1;
        let last = meter.position(Tick::new(c.end() - 1)).bar() + 1;
        (first, last)
    })
}

// ---------- 배치 뷰 표시와 실제 쓰기 ----------

/// 클립 끝에 이어 쓰는 자리인데 다음 클립이 스텝 안에서 시작하면 쓰기는 거부된다. 표시가 「이어 쓴다」고 말하면
/// 실제로 써져야 한다(틈의 같은 경우는 「쓸 수 없다」를 보인다).
#[gpui_kit::test]
fn arrangement_extend_display_agrees_with_write_when_next_clip_blocks(cx: &mut TestAppContext) {
    let (handle, view) = open_arrangement_step(
        cx,
        "extend-blocked",
        arrangement(&[
            (0, BAR, Some("A"), &[(0, 240)]),
            (BAR + 480, BAR, Some("B"), &[(0, 240)]),
        ]),
        BAR,
    );
    let shown = clip_text(cx, handle);
    press(cx, handle, "c enter");
    let written = note_at(cx, &view, BAR);
    assert!(
        !shown.contains("이어 쓴다") || written,
        "표시 「{shown}」, 실제로 쓰였나 {written}"
    );
}

/// 클립 안에서 밀어 넣기가 다음 클립과 겹쳐 거부되는 자리. 표시가 「안」이라고만 하면 쓰기 실패를 알 수 없다.
#[gpui_kit::test]
fn arrangement_inside_display_agrees_with_write_when_push_blocks(cx: &mut TestAppContext) {
    let (handle, view) = open_arrangement_step(
        cx,
        "inside-blocked",
        arrangement(&[
            (0, BAR, Some("A"), &[(3 * QUARTER + 480, 240)]),
            (BAR, BAR, Some("B"), &[(0, 240)]),
        ]),
        QUARTER,
    );
    let shown = clip_text(cx, handle);
    press(cx, handle, "c enter");
    let written = note_at(cx, &view, QUARTER);
    assert!(
        !shown.ends_with(" 안") || written,
        "표시 「{shown}」, 실제로 쓰였나 {written}"
    );
}

/// 셋잇단 음을 조합하는 동안(`n` 뒤 음 키, 확정 전) 표시는 그 음이 만들 클립 마디를 말해야 한다. 점4분 셋잇단(960)은
/// 마디 4박에서 마디 3에 들고, 셋잇단이 아닌 점4분(1440)은 마디 4로 넘는다.
#[gpui_kit::test]
fn arrangement_display_while_composing_triplet_matches_write(cx: &mut TestAppContext) {
    let at = 2 * BAR + 3 * QUARTER;
    let (handle, view) = open_arrangement_step(cx, "compose-triplet", arrangement(&[]), at);
    run(cx, handle, step::SetDottedLength(LengthCode::Quarter));
    press(cx, handle, "n");
    let before_key = clip_text(cx, handle);
    press(cx, handle, "c");
    let composing = clip_text(cx, handle);
    press(cx, handle, "enter");
    let (first, last) = bars_of_clip_at(cx, &view, at);
    let actual = format!("클립 밖: 입력하면 마디 {first}–{last}에 새 클립");
    assert_eq!(before_key, actual, "`n` 뒤");
    assert_eq!(composing, actual, "조합 중");
}

/// 절반 길이 음(`step::HalfNote`)을 조합하는 동안의 표시. 2분음표의 절반(960)은 마디 3에 든다.
#[gpui_kit::test]
fn arrangement_display_while_composing_half_matches_write(cx: &mut TestAppContext) {
    let at = 2 * BAR + 3 * QUARTER;
    let (handle, view) = open_arrangement_step(cx, "compose-half", arrangement(&[]), at);
    run(cx, handle, step::SetLength(LengthCode::Half));
    run(cx, handle, step::HalfNote("c".into()));
    let composing = clip_text(cx, handle);
    press(cx, handle, "enter");
    let (first, last) = bars_of_clip_at(cx, &view, at);
    assert_eq!(
        composing,
        format!("클립 밖: 입력하면 마디 {first}–{last}에 새 클립")
    );
}

/// 박자가 바뀐 뒤(마디 2부터 3/4)의 마디 번호가 실제로 생기는 클립과 같다.
#[gpui_kit::test]
fn arrangement_new_clip_bars_follow_meter_change(cx: &mut TestAppContext) {
    let p = ksq::from_str(r#"{"meter": [[0, 4, 4], [3840, 3, 4]], "tracks": [{"name": "Lead"}]}"#)
        .unwrap()
        .0;
    // 마디 3(3/4)의 셋째 박: 3840 + 2880 + 1920
    let at = 3840 + 2880 + 1920;
    let (handle, view) = open_arrangement_step(cx, "meter", p, at);
    run(cx, handle, step::SetLength(LengthCode::Half));
    let shown = clip_text(cx, handle);
    press(cx, handle, "c enter");
    let (first, last) = bars_of_clip_at(cx, &view, at);
    assert_eq!((first, last), (3, 4));
    assert_eq!(
        shown,
        format!("클립 밖: 입력하면 마디 {first}–{last}에 새 클립")
    );
}

/// 앞 클립 끝 = 다음 클립 시작이면 다음 클립에 쓴다. 표시도 그 클립을 가리킨다.
#[gpui_kit::test]
fn arrangement_boundary_names_next_clip_like_write(cx: &mut TestAppContext) {
    let (handle, view) = open_arrangement_step(
        cx,
        "boundary",
        arrangement(&[
            (0, BAR, Some("A"), &[(0, 240)]),
            (BAR, BAR, Some("B"), &[(480, 240)]),
        ]),
        BAR,
    );
    assert_eq!(clip_text(cx, handle), "클립 「B」 안");
    press(cx, handle, "c enter");
    let in_b = cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .find(|(_, c)| c.name() == Some("B"))
            .is_some_and(|(_, c)| c.notes().iter().any(|n| n.start().get() == 0))
    });
    assert!(in_b);
}

/// 트랙이 없으면 표시가 없고 자리표시가 그대로다.
#[gpui_kit::test]
fn arrangement_step_without_tracks_shows_nothing(cx: &mut TestAppContext) {
    let (handle, view) = open_arrangement_step(cx, "no-track", Project::new(), 0);
    assert_eq!(cx.read(|cx| view.read(cx).arrangement_step(cx)), None);
    assert_eq!(label(cx, handle, "arrangement-step-cursor"), None);
    assert_eq!(label(cx, handle, "main-view").as_deref(), Some("배치 뷰"));
}

// ---------- 미리듣기(CoreMIDI) ----------

fn install_engine(cx: &mut TestAppContext, tag: &str) -> String {
    let port = format!("KS review t4c {} {tag}", std::process::id());
    cx.update(|cx| {
        engine::install(
            cx,
            &EngineOptions {
                client_name: format!("ks-review-t4c-{tag}"),
                port_name: port.clone(),
            },
        )
    })
    .unwrap();
    port
}

fn named_port(cx: &mut TestAppContext, name: &str) -> String {
    cx.update(|cx| engine::add_named_virtual_port(cx, name))
        .unwrap()
}

fn open_step(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = open_window(cx, p);
    event(cx, handle, &view, ModeEvent::ToggleView);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

fn project(json: &str) -> Project {
    ksq::from_str(json).unwrap().0
}

fn prepares(cx: &mut TestAppContext) -> u64 {
    cx.read(engine::preview_prepares)
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
        assert!(
            Instant::now() < deadline,
            "가상 포트 「{name}」이 보이지 않는다"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let client = coremidi::Client::new("ks-review-t4c-listen").unwrap();
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

/// 조용해질 때까지 받아 모은다(늦게 오는 메시지도 본다).
fn settle(rx: &mpsc::Receiver<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    while let Ok(m) = rx.recv_timeout(Duration::from_millis(300)) {
        got.extend(split(&m));
    }
    got
}

fn note_ons(got: &[Vec<u8>]) -> usize {
    got.iter()
        .filter(|m| m[0] & 0xF0 == 0x90 && m.get(2).copied().unwrap_or(0) > 0)
        .count()
}

fn programs(got: &[Vec<u8>]) -> Vec<Vec<u8>> {
    got.iter()
        .filter(|m| m[0] & 0xF0 == 0xC0)
        .cloned()
        .collect()
}

/// 서버 복구(기본 포트 다시 등록)는 스냅샷을 바꾼다. 다음 미리듣기는 다시 준비하고 그 포트로 간다.
#[gpui_kit::test]
fn preview_after_recover_prepares_again(cx: &mut TestAppContext) {
    start(cx, "recover");
    install_engine(cx, "recover");
    let synth = named_port(cx, &format!("KS review synth {} r", std::process::id()));
    let l = listen(&synth);
    let (handle, _view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "S", "port": {{"named": "{synth}"}}, "channel": 2, "program": 3}}]}}"#
        )),
    );
    let before = prepares(cx);
    press(cx, handle, "c enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(programs(&got), vec![vec![0xC1, 3]], "{got:?}");
    cx.update(engine::recover_virtual_port).unwrap();
    cx.run_until_parked();
    press(cx, handle, "d enter");
    assert_eq!(prepares(cx) - before, 2);
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(note_ons(&got), 1, "{got:?}");
    // 복구는 음색 기록도 잊는다
    assert_eq!(programs(&got), vec![vec![0xC1, 3]], "{got:?}");
}

/// 기본 가상 포트를 이름으로 가리키는 트랙과 「가상」 트랙이 같은 채널을 번갈아 쓴다. 같은 포트이므로 매번
/// 그 트랙의 프로그램이 먼저 가야 한다.
#[gpui_kit::test]
fn preview_alternating_virtual_and_main_by_name_resend_program(cx: &mut TestAppContext) {
    start(cx, "main-by-name");
    let main = install_engine(cx, "main-by-name");
    let l = listen(&main);
    let (handle, _view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "A", "channel": 1, "program": 1}},
                {{"name": "B", "port": {{"named": "{main}"}}, "channel": 1, "program": 2}}]}}"#
        )),
    );
    press(cx, handle, "c enter");
    run(cx, handle, track::Next);
    press(cx, handle, "d enter");
    run(cx, handle, track::Prev);
    press(cx, handle, "e enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 3);
    assert_eq!(note_ons(&got), 3, "{got:?}");
    assert_eq!(
        programs(&got),
        vec![vec![0xC0, 1], vec![0xC0, 2], vec![0xC0, 1]],
        "{got:?}"
    );
}

/// 뱅크만 다른 두 트랙(같은 채널·같은 프로그램)을 번갈아 들으면 뱅크가 매번 다시 간다.
#[gpui_kit::test]
fn preview_alternating_bank_only_resend_bank(cx: &mut TestAppContext) {
    start(cx, "bank-only");
    let main = install_engine(cx, "bank-only");
    let l = listen(&main);
    let (handle, _view) = open_step(
        cx,
        project(
            r#"{"tracks": [{"name": "A", "channel": 4, "program": 5, "bank": 1},
                {"name": "B", "channel": 4, "program": 5, "bank": 2}]}"#,
        ),
    );
    press(cx, handle, "c enter");
    run(cx, handle, track::Next);
    press(cx, handle, "d enter");
    run(cx, handle, track::Prev);
    press(cx, handle, "e enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 3);
    let lsb: Vec<u8> = got
        .iter()
        .filter(|m| m[0] == 0xB3 && m[1] == 32)
        .map(|m| m[2])
        .collect();
    let msb: Vec<u8> = got
        .iter()
        .filter(|m| m[0] == 0xB3 && m[1] == 0)
        .map(|m| m[2])
        .collect();
    assert_eq!(lsb.len(), 3, "{got:?}");
    assert_eq!(msb.len(), 3, "{got:?}");
    assert_eq!(programs(&got).len(), 3, "{got:?}");
}

/// 두 창이 서로 다른 포트 구성으로 같은 이름 포트를 번갈아 듣는다. 기억한 경로표가 다른 창의 것이면 엉뚱한
/// 번호로 간다. 각 미리듣기는 제 포트로만 가야 한다.
#[gpui_kit::test]
fn preview_two_windows_with_different_port_layouts_route_correctly(cx: &mut TestAppContext) {
    start(cx, "two-windows");
    install_engine(cx, "two-windows");
    let x = named_port(cx, &format!("KS review synth {} x", std::process::id()));
    let y = named_port(cx, &format!("KS review synth {} y", std::process::id()));
    let lx = listen(&x);
    let ly = listen(&y);
    let (ha, _va) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "A", "port": {{"named": "{x}"}}, "channel": 1}}]}}"#
        )),
    );
    // B는 Y가 먼저라 X의 번호가 다르다
    let (hb, _vb) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "BY", "port": {{"named": "{y}"}}, "channel": 2}},
                {{"name": "BX", "port": {{"named": "{x}"}}, "channel": 3}}]}}"#
        )),
    );
    let before = prepares(cx);
    press(cx, ha, "c enter");
    press(cx, hb, "c enter");
    press(cx, ha, "d enter");
    press(cx, hb, "d enter");
    let gx = settle(&lx.rx);
    let gy = settle(&ly.rx);
    assert_eq!(prepares(cx) - before, 4);
    // X에는 A의 채널 1 두 번만, Y에는 B의 첫 트랙(채널 2) 두 번만
    let on_x: Vec<u8> = gx
        .iter()
        .filter(|m| m[0] & 0xF0 == 0x90 && m[2] > 0)
        .map(|m| m[0])
        .collect();
    let on_y: Vec<u8> = gy
        .iter()
        .filter(|m| m[0] & 0xF0 == 0x90 && m[2] > 0)
        .map(|m| m[0])
        .collect();
    assert_eq!(on_x, vec![0x90, 0x90], "{gx:?}");
    assert_eq!(on_y, vec![0x91, 0x91], "{gy:?}");
}

/// 메트로놈 포트 설정이 바뀌면 포트 번호가 바뀔 수 있다. 다시 준비해야 한다.
#[gpui_kit::test]
fn preview_after_metronome_port_change_prepares_again(cx: &mut TestAppContext) {
    start(cx, "metronome");
    install_engine(cx, "metronome");
    let x = named_port(cx, &format!("KS review synth {} mx", std::process::id()));
    let m = named_port(cx, &format!("KS review synth {} mm", std::process::id()));
    let lx = listen(&x);
    let lm = listen(&m);
    let (handle, _view) = open_step(
        cx,
        project(&format!(
            r#"{{"tracks": [{{"name": "S", "port": {{"named": "{x}"}}, "channel": 1}}]}}"#
        )),
    );
    let before = prepares(cx);
    press(cx, handle, "c enter");
    cx.update(|cx| AppSettings::update(cx, |s| s.metronome.port = OutputPort::Named(m.clone())));
    cx.run_until_parked();
    press(cx, handle, "d enter");
    assert_eq!(prepares(cx) - before, 2);
    let gx = settle(&lx.rx);
    assert_eq!(note_ons(&gx), 2, "{gx:?}");
    let gm = settle(&lm.rx);
    assert_eq!(note_ons(&gm), 0, "{gm:?}");
}

/// 다른 창이 재생하는 동안(가상 포트, 음색을 보내지 않음) 들은 뒤, 그 재생이 멈추면 다음 미리듣기는 음색을
/// 보낸다(그 재생의 chase가 같은 채널 음색을 바꿨을 수 있다).
#[gpui_kit::test]
fn preview_after_other_window_playback_resends_program(cx: &mut TestAppContext) {
    start(cx, "other-play");
    let main = install_engine(cx, "other-play");
    let l = listen(&main);
    let (ha, _va) = open_step(
        cx,
        project(r#"{"tracks": [{"name": "A", "channel": 1, "program": 1}]}"#),
    );
    press(cx, ha, "c enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    assert_eq!(programs(&got), vec![vec![0xC0, 1]], "{got:?}");
    let (hb, _vb) = open_step(
        cx,
        project(
            r#"{"tracks": [{"name": "B", "channel": 1, "program": 2,
                "clips": [{"start": 0, "length": 3840, "notes": [[0, 240, 72, 51492]]}]}]}"#,
        ),
    );
    run(cx, hb, transport::Play);
    press(cx, ha, "d enter");
    run(cx, hb, transport::Stop);
    let _ = settle(&l.rx);
    press(cx, ha, "e enter");
    let got = collect(&l.rx, |g| note_ons(g) >= 1);
    let program = got.iter().position(|m| m == &[0xC0, 1]);
    let on = got.iter().position(|m| m[0] == 0x90 && m[2] > 0);
    assert!(program.is_some() && program < on, "{got:?}");
}
