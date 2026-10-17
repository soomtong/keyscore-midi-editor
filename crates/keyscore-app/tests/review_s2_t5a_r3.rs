//! S2-T5a 적대적 리뷰 3. 리뷰 2 대응(교체 직전 `collect_garbage`, 같은 순간 기준점, 들리지 않는 트랙의
//! 틱 0 뱅크·프로그램, 목적지·채널·묶음 단위 `Setup` 비교)과 그 회귀를 본다. 여기 남은 테스트는 깨뜨리지
//! 못한 시도다. 깨뜨린 시도는 `docs/reports/s2-t5a/review-3.md` 부록에 있다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Edit, OutputPort, Program, Project, TrackProp, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn unique(name: &str) -> String {
    format!("KS rv5a3 {} {name}", std::process::id())
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-rv5a3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(name);
    let options = EngineOptions {
        client_name: format!("ks-rv5a3-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    loopback(&port)
}

fn loopback(name: &str) -> Loopback {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-rv5a3-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                for msg in split(p.data()) {
                    let _ = tx.send(msg);
                }
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let source = loop {
        if let Some(s) = coremidi::Source::from_name(name) {
            break s;
        }
        assert!(Instant::now() < deadline, "{name}이 보이지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    };
    input.connect_source(&source).unwrap();
    Loopback {
        rx,
        _input: input,
        _client: client,
    }
}

/// 이름 포트용 가상 목적지. 앱이 OS 목적지로 열어 보낸다.
struct Destination {
    rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

fn destination(name: &str) -> Destination {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-rv5a3-dest").unwrap();
    let dest = client
        .virtual_destination(name, move |list: &coremidi::PacketList| {
            for p in list {
                for msg in split(p.data()) {
                    let _ = tx.send(msg);
                }
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !keyscore_midi::destinations()
        .iter()
        .any(|i| i.name() == name)
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

fn drain_for(rx: &mpsc::Receiver<Vec<u8>>, limit: Duration) -> Vec<Vec<u8>> {
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

fn wait_for(
    rx: &mpsc::Receiver<Vec<u8>>,
    limit: Duration,
    pred: impl Fn(&[u8]) -> bool,
) -> Option<Vec<Vec<u8>>> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
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

fn note_on(pitch: u8) -> impl Fn(&[u8]) -> bool {
    move |m| m.len() == 3 && m[0] & 0xF0 == 0x90 && m[1] == pitch && m[2] > 0
}

/// 120 BPM(480틱 = 0.25 s), 기본 가상 포트·채널 1·프로그램 5.
fn project(notes: &str) -> Project {
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{{"name": "Lead", "port": "virtual", "channel": 1, "program": 5,
            "clips": [{{"start": 0, "length": 15360, "notes": [{notes}]}}]}}]}}"#
    );
    ksq::from_str(&text).unwrap().0
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    (handle, view)
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn state(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> TransportState {
    cx.read(|cx| view.read(cx).transport_state())
}

const WAIT: Duration = Duration::from_secs(2);

fn edit(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    make: impl FnOnce(&Project) -> Vec<Edit>,
) {
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let e = make(doc.project());
            doc.apply(e).unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
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

/// 4분음표마다 같은 음(`pitch`)을 32번(시작은 델타).
fn every_beat(pitch: u8) -> String {
    (0..32)
        .map(|i| format!("[{}, 240, {pitch}, 51492]", if i == 0 { 0 } else { 480 }))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `got`에서 `before` 앞의 채널 1 마지막 프로그램.
fn last_program_before(got: &[Vec<u8>], before: usize, status: u8) -> Option<u8> {
    got[..before]
        .iter()
        .rev()
        .find(|m| m.len() == 2 && m[0] == status)
        .map(|m| m[1])
}

// ---------- 지적 1(collect_garbage): 섞인 긴 시퀀스 ----------

/// Play·재생 중 편집·메트로놈 토글·패닉·일시정지를 섞어 900번 넘게 누른 뒤에도 Play가 들리고 Stop이 멈춘다.
/// 교체는 600번 넘게 일어난다(돌려받기 큐 256 + 대기 자리 4를 훨씬 넘는다).
#[gpui_kit::test]
fn mixed_long_sequence_keeps_transport_working(cx: &mut TestAppContext) {
    let lb = start(cx, "mixed-long");
    let (handle, view) = open(cx, project(&every_beat(60)));
    for i in 0..320 {
        match i % 4 {
            1 => run(cx, handle, transport::Pause),
            2 => run(cx, handle, transport::Toggle),
            _ => run(cx, handle, transport::Play),
        }
        if state(cx, &view) == TransportState::Playing {
            edit(cx, &view, |p| {
                vec![Edit::SetTrackProp {
                    track: p.tracks()[0].id(),
                    prop: TrackProp::Name(format!("n{i}")),
                }]
            });
        }
        run(cx, handle, transport::ToggleMetronome);
        if i % 50 == 0 {
            run(cx, handle, transport::Panic);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    run(cx, handle, transport::Stop);
    drain_for(&lb.rx, Duration::from_millis(100));
    run(cx, handle, transport::Play);
    assert_eq!(state(cx, &view), TransportState::Playing);
    let heard = wait_for(&lb.rx, WAIT, note_on(60));
    run(cx, handle, transport::Stop);
    drain_for(&lb.rx, Duration::from_millis(150));
    let after = drain_for(&lb.rx, Duration::from_millis(800));
    let msgs = messages(cx, &view);
    assert!(heard.is_some(), "마지막 Play가 들리지 않는다: {msgs:?}");
    assert!(
        !after.iter().any(|m| note_on(60)(m)),
        "정지 뒤에도 재생된다: {after:?}"
    );
    assert!(msgs.is_empty(), "{msgs:?}");
}

/// 재생 중 메트로놈 토글만 400번(1 ms 간격): 명령 큐가 넘치지 않고 마지막 상태가 맞는다.
#[gpui_kit::test]
fn many_metronome_toggles_while_playing_stay_in_sync(cx: &mut TestAppContext) {
    let lb = start(cx, "metro-many");
    let (handle, view) = open(cx, project(&every_beat(60)));
    run(cx, handle, transport::Play);
    for _ in 0..401 {
        run(cx, handle, transport::ToggleMetronome);
        std::thread::sleep(Duration::from_millis(1));
    }
    drain_for(&lb.rx, Duration::from_millis(100));
    // 401번 토글: 켜짐. 채널 10 클릭(37 또는 42)이 와야 한다.
    let click = wait_for(&lb.rx, WAIT, |m| {
        m.len() == 3 && m[0] == 0x99 && (m[1] == 37 || m[1] == 42) && m[2] > 0
    });
    let msgs = messages(cx, &view);
    run(cx, handle, transport::Stop);
    assert!(click.is_some(), "메트로놈이 켜져 있지 않다: {msgs:?}");
    assert!(msgs.is_empty(), "{msgs:?}");
}

// ---------- 지적 3: 들리지 않는 트랙의 틱 0 프로그램 ----------

/// 뮤트 트랙(채널 2)의 프로그램이 중간 위치에서 재개할 때도 chase로 나간다(채널을 공유하지 않을 때).
#[gpui_kit::test]
fn resume_mid_song_chases_muted_track_program(cx: &mut TestAppContext) {
    let lb = start(cx, "mute-resume");
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
        {{"port": "virtual", "channel": 1, "program": 5,
         "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}},
        {{"port": "virtual", "channel": 2, "program": 9, "bank": 3, "mute": true,
         "clips": [{{"start": 0, "length": 15360, "notes": [[0, 240, 64, 51492]]}}]}}]}}"#,
        every_beat(60)
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_millis(600));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    assert!(at.get() > 480, "{at:?}");
    drain_for(&lb.rx, Duration::from_millis(100));
    run(cx, handle, transport::Play);
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    let note = got.len() - 1;
    let bank = got.iter().position(|m| m == &[0xB1, 32, 3]);
    let program = got.iter().position(|m| m == &[0xC1, 9]);
    assert!(
        matches!((bank, program), (Some(b), Some(p)) if b < p && p < note),
        "재개 때 뮤트 트랙의 뱅크·프로그램이 첫 노트보다 먼저 나가지 않았다: {got:?}"
    );
}

/// 다른 채널의 두 트랙에서 재생 중 솔로를 A → B로 옮기면 B의 첫 노트 앞에 B의 프로그램이 이미 나가 있다.
#[gpui_kit::test]
fn solo_switch_while_playing_keeps_program(cx: &mut TestAppContext) {
    let lb = start(cx, "solo-switch");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [
        {"port": "virtual", "channel": 1, "program": 5, "solo": true,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492]]}]},
        {"port": "virtual", "channel": 2, "program": 9,
         "clips": [{"start": 0, "length": 7680, "notes": [[1920, 240, 64, 51492]]}]}]}"#;
    let (handle, view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    let mut got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![
            Edit::SetTrackProp {
                track: p.tracks()[0].id(),
                prop: TrackProp::Solo(false),
            },
            Edit::SetTrackProp {
                track: p.tracks()[1].id(),
                prop: TrackProp::Solo(true),
            },
        ]
    });
    got.extend(wait_for(&lb.rx, WAIT, |m| note_on(64)(m) && m[0] == 0x91).expect("Note On 64"));
    run(cx, handle, transport::Stop);
    let note = got.iter().position(|m| note_on(64)(m) && m[0] == 0x91);
    let program = got.iter().position(|m| m == &[0xC1, 9]);
    assert!(program.is_some() && program < note, "{got:?}");
}

/// 둘 다 들리는 두 트랙이 같은 채널을 공유하면(SEQ-8 경고 대상) 트랙 순서상 마지막 프로그램이 이긴다.
/// 단계 파일 이월(「트랙 순서대로」)과 맞는다. 지적 1(리뷰 3)과 대조용.
#[gpui_kit::test]
fn shared_channel_audible_tracks_follow_track_order(cx: &mut TestAppContext) {
    let lb = start(cx, "shared-order");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [
        {"port": "virtual", "channel": 1, "program": 9,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 62, 51492]]}]},
        {"port": "virtual", "channel": 1, "program": 5,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492]]}]}]}"#;
    let (handle, _view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    let note = got
        .iter()
        .position(|m| note_on(60)(m) || note_on(62)(m))
        .unwrap();
    assert_eq!(last_program_before(&got, note, 0xC0), Some(5), "{got:?}");
}

// ---------- 지적 4·5: Setup 비교 ----------

/// 재생 중 채널만 바꾸면 새 채널로 뱅크·프로그램 묶음이 나간다.
#[gpui_kit::test]
fn channel_edit_while_playing_sends_setup_on_new_channel(cx: &mut TestAppContext) {
    let lb = start(cx, "channel-edit");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [{"port": "virtual", "channel": 1, "bank": 2, "program": 5,
        "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492], [1920, 240, 62, 51492]]}]}]}"#;
    let (handle, view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: TrackProp::Channel(keyscore_core::Channel::new(3).unwrap()),
        }]
    });
    let got = wait_for(&lb.rx, WAIT, |m| note_on(62)(m) && m[0] == 0x92).expect("Note On 62");
    run(cx, handle, transport::Stop);
    let bank = got.iter().position(|m| m == &[0xB2, 32, 2]);
    let program = got.iter().position(|m| m == &[0xC2, 5]);
    assert!(
        matches!((bank, program), (Some(b), Some(p)) if b < p),
        "{got:?}"
    );
}

/// 가상 → 이름 포트 Y → 가상으로 되돌리면 되돌아온 가상 포트에도 프로그램이 다시 나간다(지운 뒤 다시 생긴
/// 항목을 「이미 보냄」으로 치지 않는다).
#[gpui_kit::test]
fn port_round_trip_while_playing_resends_program(cx: &mut TestAppContext) {
    let lb = start(cx, "port-trip");
    let y = unique("trip y");
    let dy = destination(&y);
    let (handle, view) = open(cx, project(&every_beat(60)));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    let target = y.clone();
    edit(cx, &view, move |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: TrackProp::Port(OutputPort::Named(target)),
        }]
    });
    wait_for(&dy.rx, WAIT, note_on(60)).expect("Y Note On 60");
    drain_for(&lb.rx, Duration::from_millis(50));
    edit(cx, &view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: TrackProp::Port(OutputPort::Virtual),
        }]
    });
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("가상 Note On 60");
    run(cx, handle, transport::Stop);
    assert!(got.iter().any(|m| m == &[0xC0, 5]), "{got:?}");
}

// ---------- 리뷰 3 부록 A(지적 1·2): 결정 2026-10-17-s2-t5a-review-limit A로 고친 뒤 옮김 ----------

fn set_program(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, track: usize, value: u8) {
    edit(cx, view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[track].id(),
            prop: TrackProp::Program(Some(Program::new(value).unwrap())),
        }]
    });
}

/// 깨뜨림 1: 같은 포트·채널의 두 트랙 가운데 A(프로그램 5)만 솔로다. 들리지 않는 B(프로그램 9)의 프로그램이
/// 틱 0 이벤트로 렌더러 결과 뒤에 붙어(같은 틱·같은 무리는 넣은 순서) chase·재생에서 마지막이 되어, 솔로한
/// A가 B의 음색으로 연주된다. B를 트랙 순서상 앞에 두어 「트랙 순서대로」 규칙으로도 5여야 함을 보인다.
#[gpui_kit::test]
fn soloed_track_on_shared_channel_plays_its_own_program(cx: &mut TestAppContext) {
    let lb = start(cx, "solo-shared");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [
        {"port": "virtual", "channel": 1, "program": 9,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 62, 51492]]}]},
        {"port": "virtual", "channel": 1, "program": 5, "solo": true,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492]]}]}]}"#;
    let (handle, _view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    let note = got.len() - 1;
    assert_eq!(
        last_program_before(&got, note, 0xC0),
        Some(5),
        "솔로한 트랙이 다른 트랙의 프로그램으로 연주된다: {got:?}"
    );
}

/// 깨뜨림 1의 뮤트 변형: 뮤트한 B(프로그램 9)가 들리는 A(프로그램 5)의 음색을 덮는다. 일시정지 뒤 중간
/// 위치에서 재개해도(chase) 같다.
#[gpui_kit::test]
fn muted_track_on_shared_channel_does_not_override_audible_program(cx: &mut TestAppContext) {
    let lb = start(cx, "mute-shared");
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
        {{"port": "virtual", "channel": 1, "program": 9, "mute": true,
         "clips": [{{"start": 0, "length": 15360, "notes": [[0, 240, 62, 51492]]}}]}},
        {{"port": "virtual", "channel": 1, "program": 5,
         "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}}]}}"#,
        every_beat(60)
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    let start_got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    std::thread::sleep(Duration::from_millis(600));
    run(cx, handle, transport::Pause);
    drain_for(&lb.rx, Duration::from_millis(100));
    run(cx, handle, transport::Play);
    let resume_got = wait_for(&lb.rx, WAIT, note_on(60)).expect("재개 Note On 60");
    run(cx, handle, transport::Stop);
    let _ = view;
    assert_eq!(
        (
            last_program_before(&start_got, start_got.len() - 1, 0xC0),
            last_program_before(&resume_got, resume_got.len() - 1, 0xC0)
        ),
        (Some(5), Some(5)),
        "뮤트한 트랙의 프로그램이 들리는 트랙의 음색을 덮는다: 시작 {start_got:?}, 재개 {resume_got:?}"
    );
}

/// 깨뜨림 2: 같은 포트·채널의 두 트랙이 같은 프로그램 5다(SEQ-8의 「한 신디에 두 성부」, 경고 대상 아님).
/// 재생 중 B를 9로 바꾸면 9가 나간다. 되돌리기(undo)로 B를 5로 돌리면, 새 항목 B(채널 1, 5)가 옛 A 항목과
/// 같다고 보고 걸러져 5가 나가지 않는다. 두 트랙 모두 5인데 신디는 9에 남는다.
#[gpui_kit::test]
fn undoing_program_edit_on_shared_channel_restores_program(cx: &mut TestAppContext) {
    let lb = start(cx, "shared-undo");
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
        {{"port": "virtual", "channel": 1, "program": 5,
         "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}},
        {{"port": "virtual", "channel": 1, "program": 5,
         "clips": [{{"start": 0, "length": 15360, "notes": [[0, 240, 64, 51492]]}}]}}]}}"#,
        every_beat(60)
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    set_program(cx, &view, 1, 9);
    let first = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60(편집 뒤)");
    assert!(first.iter().any(|m| m == &[0xC0, 9]), "{first:?}");
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            assert!(doc.undo().unwrap());
            cx.notify();
        });
    });
    cx.run_until_parked();
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60(되돌린 뒤)");
    let programs: Vec<u8> = cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .project()
            .tracks()
            .iter()
            .map(|t| t.program().unwrap().get())
            .collect()
    });
    run(cx, handle, transport::Stop);
    assert_eq!(programs, vec![5, 5]);
    assert!(
        got.iter().any(|m| m == &[0xC0, 5]),
        "두 트랙 모두 프로그램 5인데 신디는 9에 남는다: {got:?}"
    );
}
