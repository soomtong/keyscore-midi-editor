//! S2-T5a 적대적 리뷰 1. 기본 가상 포트(와 리뷰가 만든 가상 목적지)를 같은 프로세스의 `coremidi`
//! 수신기로 받는다. 여기 남은 테스트는 깨뜨리지 못한 시도다. 깨뜨린 시도는
//! `docs/reports/s2-t5a/review-1.md` 부록에 있다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Edit, Program, Project, Tempo, Tick, TrackProp, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn unique(name: &str) -> String {
    format!("KS rv5a {} {name}", std::process::id())
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-rv5a-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(name);
    let options = EngineOptions {
        client_name: format!("ks-rv5a-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    loopback(&port)
}

fn loopback(name: &str) -> Loopback {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-rv5a-rx").unwrap();
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
    let client = coremidi::Client::new("ks-rv5a-dest").unwrap();
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

fn note_off(pitch: u8) -> impl Fn(&[u8]) -> bool {
    move |m| {
        m.len() == 3 && m[1] == pitch && (m[0] & 0xF0 == 0x80 || (m[0] & 0xF0 == 0x90 && m[2] == 0))
    }
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

/// 4분음표마다 같은 음(`pitch`)을 32번(시작은 델타).
fn every_beat(pitch: u8) -> String {
    (0..32)
        .map(|i| format!("[{}, 240, {pitch}, 51492]", if i == 0 { 0 } else { 480 }))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------- 창 닫기 ----------

#[gpui_kit::test]
fn closing_playing_window_stops_playback(cx: &mut TestAppContext) {
    let lb = start(cx, "close-playing");
    let (handle, view) = open(cx, project("[0, 15360, 60, 51492]"));
    // 테스트가 든 강한 참조를 놓아, 창을 닫으면 뷰가 해제되게 한다.
    drop(view);
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    wait_for(&lb.rx, WAIT, note_off(60)).expect("닫힌 창의 노트가 걸린 채 남았다");
}

#[gpui_kit::test]
fn closing_other_window_keeps_owner_playing(cx: &mut TestAppContext) {
    let lb = start(cx, "close-other");
    let (first, first_view) = open(cx, project(&every_beat(60)));
    let (second, _) = open(cx, project("[0, 240, 64, 51492]"));
    run(cx, first, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("첫 창");
    cx.update_window(second, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    drain_for(&lb.rx, Duration::from_millis(100));
    wait_for(&lb.rx, WAIT, note_on(60)).expect("다른 창을 닫자 재생이 멈췄다");
    assert_eq!(state(cx, &first_view), TransportState::Playing);
    run(cx, first, transport::Stop);
}

// ---------- Play 연타, 일시정지 ----------

#[gpui_kit::test]
fn repeated_play_leaves_no_hanging_notes(cx: &mut TestAppContext) {
    let lb = start(cx, "play-mash");
    let (handle, _) = open(cx, project("[0, 3840, 60, 51492], [0, 3840, 64, 51492]"));
    for _ in 0..8 {
        run(cx, handle, transport::Play);
        std::thread::sleep(Duration::from_millis(15));
    }
    std::thread::sleep(Duration::from_millis(100));
    run(cx, handle, transport::Stop);
    let got = drain_for(&lb.rx, Duration::from_millis(500));
    for pitch in [60u8, 64] {
        let on = got.iter().filter(|m| note_on(pitch)(m)).count();
        let off = got.iter().filter(|m| note_off(pitch)(m)).count();
        assert!(on > 0, "{got:?}");
        assert!(off >= on, "음 {pitch}: on {on}, off {off}: {got:?}");
    }
}

#[gpui_kit::test]
fn pause_releases_sounding_notes_and_resume_sends_program(cx: &mut TestAppContext) {
    let lb = start(cx, "pause-hang");
    let (handle, view) = open(cx, project("[0, 15360, 60, 51492], [1920, 240, 64, 51492]"));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    std::thread::sleep(Duration::from_millis(200));
    run(cx, handle, transport::Pause);
    assert!(matches!(state(cx, &view), TransportState::Paused(_)));
    wait_for(&lb.rx, WAIT, note_off(60)).expect("일시정지가 울리는 노트를 끄지 않았다");
    drain_for(&lb.rx, Duration::from_millis(100));
    run(cx, handle, transport::Play);
    let got = wait_for(&lb.rx, WAIT, note_on(64)).expect("이어서 64");
    let program = got.iter().position(|m| m == &[0xC0, 5]);
    let first = got.iter().position(|m| note_on(64)(m));
    assert!(program.is_some() && program < first, "{got:?}");
    run(cx, handle, transport::Stop);
}

#[gpui_kit::test]
fn pause_position_follows_tempo_change_in_song(cx: &mut TestAppContext) {
    // 틱 960(0.5 s)에서 60 BPM으로 느려진다. 1.0 s 뒤 자리는 960 + 0.5 s × 960 = 1440 근처.
    let _lb = start(cx, "pause-tempo-map");
    let text = r#"{"tempo": [[0, 500000], [960, 1000000]], "tracks": [{"port": "virtual", "channel": 1,
        "clips": [{"start": 0, "length": 15360, "notes": [[0, 240, 60, 51492]]}]}]}"#;
    let (handle, view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_secs(1));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    assert!(
        at >= Tick::new(1300) && at <= Tick::new(1600),
        "템포 변화를 따르지 않은 자리: {at:?}"
    );
}

#[gpui_kit::test]
fn playback_past_song_end_stays_playing(cx: &mut TestAppContext) {
    // 곡 끝에서 스스로 멈추지 않는다(동작 기록). 일시정지 자리는 곡 끝 뒤가 된다.
    let _lb = start(cx, "past-end");
    let (handle, view) = open(cx, project("[0, 120, 60, 51492]"));
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_millis(600));
    cx.run_until_parked();
    assert_eq!(state(cx, &view), TransportState::Playing);
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    assert!(at > Tick::new(960), "{at:?}");
}

// ---------- 메트로놈 ----------

#[gpui_kit::test]
fn metronome_toggle_while_playing_takes_effect(cx: &mut TestAppContext) {
    let lb = start(cx, "metronome-live");
    let (handle, _) = open(cx, project(""));
    run(cx, handle, transport::Play);
    let got = drain_for(&lb.rx, Duration::from_millis(600));
    assert!(!got.iter().any(|m| m.len() == 3 && m[0] == 0x99 && m[2] > 0));
    run(cx, handle, transport::ToggleMetronome);
    wait_for(&lb.rx, WAIT, |m| m.len() == 3 && m[0] == 0x99 && m[2] > 0)
        .expect("재생 중 켠 메트로놈이 울리지 않는다");
    run(cx, handle, transport::ToggleMetronome);
    drain_for(&lb.rx, Duration::from_millis(100));
    let got = drain_for(&lb.rx, Duration::from_millis(1100));
    assert!(
        !got.iter().any(|m| m.len() == 3 && m[0] == 0x99 && m[2] > 0),
        "재생 중 끈 메트로놈이 울린다: {got:?}"
    );
    run(cx, handle, transport::Stop);
}

#[gpui_kit::test]
fn metronome_flag_of_other_window_does_not_leak(cx: &mut TestAppContext) {
    // 재생하지 않는 창의 메트로놈 토글은 재생 중인 창의 클릭을 바꾸지 않는다.
    let lb = start(cx, "metronome-other");
    let (first, _) = open(cx, project(""));
    let (second, second_view) = open(cx, project(""));
    run(cx, first, transport::Play);
    run(cx, second, transport::ToggleMetronome);
    assert!(cx.read(|cx| second_view.read(cx).metronome_on()));
    let got = drain_for(&lb.rx, Duration::from_millis(1100));
    assert!(
        !got.iter().any(|m| m.len() == 3 && m[0] == 0x99 && m[2] > 0),
        "{got:?}"
    );
    // 둘째 창이 재생을 가져가면 자기 설정(켬)을 쓴다
    run(cx, second, transport::Play);
    wait_for(&lb.rx, WAIT, |m| m.len() == 3 && m[0] == 0x99 && m[2] > 0).expect("둘째 창 메트로놈");
    run(cx, second, transport::Stop);
}

// ---------- 포트 라우팅 ----------

#[gpui_kit::test]
fn tracks_sharing_named_port_reach_that_destination(cx: &mut TestAppContext) {
    let lb = start(cx, "named-shared");
    let name = unique("dest shared");
    let dest = destination(&name);
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
            {{"port": {{"named": "{name}"}}, "channel": 1, "program": 3,
              "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 60, 51492]]}}]}},
            {{"port": {{"named": "{name}"}}, "channel": 2,
              "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 62, 51492]]}}]}}]}}"#
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    let got = drain_for(&dest.rx, Duration::from_millis(600));
    assert!(
        got.iter()
            .any(|m| m == &[0x90, 60, 100] || note_on(60)(m) && m[0] == 0x90),
        "{got:?}"
    );
    assert!(
        got.iter().any(|m| note_on(62)(m) && m[0] == 0x91),
        "{got:?}"
    );
    assert!(got.iter().any(|m| m == &[0xC0, 3]), "{got:?}");
    let main = drain_for(&lb.rx, Duration::from_millis(50));
    assert!(
        !main.iter().any(|m| note_on(60)(m) || note_on(62)(m)),
        "{main:?}"
    );
    assert!(cx.read(|cx| view.read(cx).messages().is_empty()));
    run(cx, handle, transport::Stop);
}

#[gpui_kit::test]
fn ports_over_scheduler_limit_are_silent_and_reported(cx: &mut TestAppContext) {
    // 기본 가상 포트가 0번이라 이름 포트는 31개까지. 32번째 트랙은 소리 내지 않고 알린다.
    let _lb = start(cx, "port-limit");
    let names: Vec<String> = (0..32).map(|i| unique(&format!("lim {i}"))).collect();
    let dests: Vec<Destination> = names.iter().map(|n| destination(n)).collect();
    let tracks: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            format!(
                r#"{{"port": {{"named": "{n}"}}, "channel": {},
                  "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 60, 51492]]}}]}}"#,
                i % 16 + 1
            )
        })
        .collect();
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(",")
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    assert_eq!(state(cx, &view), TransportState::Playing);
    std::thread::sleep(Duration::from_millis(400));
    let first = drain_for(&dests[0].rx, Duration::from_millis(50));
    assert!(first.iter().any(|m| note_on(60)(m)), "{first:?}");
    let last = drain_for(&dests[31].rx, Duration::from_millis(50));
    assert!(!last.iter().any(|m| note_on(60)(m)), "{last:?}");
    let messages = cx.read(|cx| view.read(cx).messages().to_vec());
    assert!(
        messages.iter().any(|m| m.contains(&names[31])),
        "{messages:?}"
    );
    run(cx, handle, transport::Stop);
}

// ---------- 엔진 없음 ----------

#[gpui_kit::test]
fn every_transport_action_without_engine_is_harmless(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("ks-rv5a-{}-none", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let (handle, view) = open(cx, project("[0, 240, 60, 51492]"));
    run(cx, handle, transport::Toggle);
    run(cx, handle, transport::PlayFromCursor);
    run(cx, handle, transport::Pause);
    run(cx, handle, transport::Stop);
    run(cx, handle, transport::ToggleMetronome);
    run(cx, handle, transport::Panic);
    assert_eq!(state(cx, &view), TransportState::Stopped);
    assert!(cx.read(|cx| view.read(cx).metronome_on()));
    for _ in 0..40 {
        run(cx, handle, transport::Play);
    }
    let messages = cx.read(|cx| view.read(cx).messages().len());
    assert!(messages <= 32, "{messages}");
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

// ---------- 리뷰 1 부록 A(지적 1–3): 고친 뒤 회귀 방지로 옮김 ----------

fn edit(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    make: impl FnOnce(&Project) -> Edit,
) {
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let e = make(doc.project());
            doc.apply(vec![e]).unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
}

/// 깨뜨림 1: 재생 중 템포를 바꾸면(TRN-3) 일시정지 자리가 새 템포로 처음부터 다시 계산되어 뒤로 튄다.
/// 스케줄러는 교체 때 지난 구간을 옛 템포로 두고 이어 가므로, 재개하면 이미 들은 노트가 다시 울린다.
#[gpui_kit::test]
fn pause_after_tempo_edit_while_playing_keeps_position(cx: &mut TestAppContext) {
    let lb = start(cx, "tempo-edit-pause");
    // 120 BPM: 62는 틱 1440(0.75 s)
    let (handle, view) = open(
        cx,
        project("[0, 240, 60, 51492], [1440, 240, 62, 51492], [2400, 240, 64, 51492]"),
    );
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(62)).expect("Note On 62");
    // 약 1.0 s(틱 1920 근처)에서 60 BPM으로
    std::thread::sleep(Duration::from_millis(220));
    edit(cx, &view, |_| Edit::SetTempo {
        at: Tick::ZERO,
        tempo: Some(Tempo::from_bpm(60).unwrap()),
    });
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    assert!(at >= Tick::new(1700), "일시정지 자리가 뒤로 튀었다: {at:?}");
    drain_for(&lb.rx, Duration::from_millis(100));
    run(cx, handle, transport::Play);
    let got = drain_for(&lb.rx, Duration::from_millis(1200));
    assert!(
        !got.iter().any(|m| note_on(62)(m)),
        "이미 들은 62가 다시 울렸다: {got:?}"
    );
    run(cx, handle, transport::Stop);
}

/// 깨뜨림 2: 재생 중 트랙 프로그램을 바꿔도(TRN-3, OUT-4) 프로그램 체인지가 나가지 않는다.
/// 스냅샷의 프로그램은 틱 0에만 있고 교체는 chase·준비 송신을 하지 않는다.
#[gpui_kit::test]
fn program_edit_while_playing_is_sent(cx: &mut TestAppContext) {
    let lb = start(cx, "program-edit");
    let (handle, view) = open(cx, project("[0, 240, 60, 51492], [1920, 240, 62, 51492]"));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| Edit::SetTrackProp {
        track: p.tracks()[0].id(),
        prop: TrackProp::Program(Some(Program::new(9).unwrap())),
    });
    let got = wait_for(&lb.rx, WAIT, note_on(62)).expect("Note On 62");
    assert!(
        got.iter().any(|m| m == &[0xC0, 9]),
        "재생 중 바꾼 프로그램이 나가지 않았다: {got:?}"
    );
    run(cx, handle, transport::Stop);
}

/// 깨뜨림 3: 뱅크·프로그램이 있는 트랙이 많으면 재생 시작의 `send_now`가 명령 큐(256)를 넘쳐 재생이
/// 실패한다. 실패 전에 이미 `stop`·`swap`을 보내 앞 재생도 멈춘다. SEQ-1은 트랙 수 제한이 없다.
#[gpui_kit::test]
fn many_tracks_with_programs_can_play(cx: &mut TestAppContext) {
    let lb = start(cx, "many-tracks");
    let tracks: Vec<String> = (0..100)
        .map(|i| {
            format!(
                r#"{{"port": "virtual", "channel": {}, "bank": 1, "program": 2,
                  "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 60, 51492]]}}]}}"#,
                i % 16 + 1
            )
        })
        .collect();
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(",")
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    let messages = cx.read(|cx| view.read(cx).messages().to_vec());
    assert_eq!(state(cx, &view), TransportState::Playing, "{messages:?}");
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
}

/// 깨뜨림 3의 변형: 실용 상한(SEQ-1, 64트랙)에서 Play를 연달아 누르면 큐가 넘친다. 둘째 Play가
/// 실패해도 창은 「재생 중」으로 남지만, 실패 전에 보낸 `stop`으로 스케줄러는 멈춰 소리가 없다.
#[gpui_kit::test]
fn sixty_four_tracks_survive_repeated_play(cx: &mut TestAppContext) {
    let lb = start(cx, "sixty-four");
    let tracks: Vec<String> = (0..64)
        .map(|i| {
            format!(
                r#"{{"port": "virtual", "channel": {}, "bank": 1, "program": 2,
                  "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 60, 51492], [1920, 240, 62, 51492]]}}]}}"#,
                i % 16 + 1
            )
        })
        .collect();
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(",")
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    run(cx, handle, transport::Play);
    let messages = cx.read(|cx| view.read(cx).messages().to_vec());
    assert_eq!(state(cx, &view), TransportState::Playing, "{messages:?}");
    assert!(
        wait_for(&lb.rx, WAIT, note_on(62)).is_some(),
        "창은 재생 중인데 소리가 없다: {messages:?}"
    );
    run(cx, handle, transport::Stop);
}
