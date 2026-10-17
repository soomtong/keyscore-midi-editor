//! S2-T5a 적대적 리뷰 2. 리뷰 1 대응(기준점 교체, 바뀐 프로그램 송신, chase 의존)과 그 회귀를 본다.
//! 기본 가상 포트와 리뷰가 만든 가상 목적지를 같은 프로세스의 `coremidi` 수신기로 받는다. 여기 남은
//! 테스트는 깨뜨리지 못한 시도다. 깨뜨린 시도는 `docs/reports/s2-t5a/review-2.md` 부록에 있다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Edit, OutputPort, Project, Tempo, Tick, TrackProp, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn unique(name: &str) -> String {
    format!("KS rv5a2 {} {name}", std::process::id())
}

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-rv5a2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = unique(name);
    let options = EngineOptions {
        client_name: format!("ks-rv5a2-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    loopback(&port)
}

fn loopback(name: &str) -> Loopback {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-rv5a2-rx").unwrap();
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
    let client = coremidi::Client::new("ks-rv5a2-dest").unwrap();
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

fn many_tracks(n: usize, notes: &str) -> Project {
    let tracks: Vec<String> = (0..n)
        .map(|i| {
            format!(
                r#"{{"port": "virtual", "channel": {}, "bank": 1, "program": 2,
                  "clips": [{{"start": 0, "length": 7680, "notes": [{notes}]}}]}}"#,
                i % 16 + 1
            )
        })
        .collect();
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(",")
    );
    ksq::from_str(&text).unwrap().0
}

// ---------- 기준점: 템포 편집 여러 번 ----------

/// 120 → 60 → 240 BPM으로 0.3 s마다 바꾼 뒤 일시정지. 기대 자리 약 576 + 288 + 1152 = 2016틱.
#[gpui_kit::test]
fn pause_after_several_tempo_edits_matches_elapsed_time(cx: &mut TestAppContext) {
    let _lb = start(cx, "tempo-multi");
    let (handle, view) = open(cx, project("[0, 240, 60, 51492], [15000, 240, 62, 51492]"));
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_millis(300));
    edit(cx, &view, |_| {
        vec![Edit::SetTempo {
            at: Tick::ZERO,
            tempo: Some(Tempo::from_bpm(60).unwrap()),
        }]
    });
    std::thread::sleep(Duration::from_millis(300));
    edit(cx, &view, |_| {
        vec![Edit::SetTempo {
            at: Tick::ZERO,
            tempo: Some(Tempo::from_bpm(240).unwrap()),
        }]
    });
    std::thread::sleep(Duration::from_millis(300));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    assert!(
        (1800..=2300).contains(&at.get()),
        "일시정지 자리가 경과 시간과 맞지 않는다: {at:?}"
    );
}

/// 템포 변화가 있는 곡에서 템포 아닌 편집을 거듭해도 자리가 흔들리지 않는다.
#[gpui_kit::test]
fn non_tempo_edits_in_song_with_tempo_change_keep_position(cx: &mut TestAppContext) {
    let _lb = start(cx, "tempo-song-edits");
    // 틱 960(0.5 s)에서 60 BPM
    let text = r#"{"tempo": [[0, 500000], [960, 1000000]], "tracks": [{"port": "virtual", "channel": 1,
        "clips": [{"start": 0, "length": 15360, "notes": [[0, 240, 60, 51492]]}]}]}"#;
    let (handle, view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    let begin = Instant::now();
    for i in 0..10 {
        std::thread::sleep(Duration::from_millis(80));
        edit(cx, &view, |p| {
            vec![Edit::SetTrackProp {
                track: p.tracks()[0].id(),
                prop: TrackProp::Name(format!("n{i}")),
            }]
        });
    }
    let secs = begin.elapsed().as_secs_f64();
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    // 0.5 s까지 1920틱/s, 그 뒤 960틱/s
    let expected = 960.0 + (secs - 0.5) * 960.0;
    let got = f64::from(u32::try_from(at.get()).unwrap());
    assert!(
        (got - expected).abs() < 150.0,
        "자리 {got}, 기대 {expected:.0}"
    );
}

// ---------- chase 의존: 시작 위치와 트랙 수 ----------

/// 곡 끝 뒤에서 일시정지한 뒤 재개해도 앞서 바뀐 프로그램을 먼저 보낸다.
#[gpui_kit::test]
fn resume_past_song_end_chases_program(cx: &mut TestAppContext) {
    let lb = start(cx, "resume-past-end");
    let (handle, view) = open(cx, project("[0, 120, 60, 51492]"));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_off(60)).expect("Note Off 60");
    std::thread::sleep(Duration::from_millis(200));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    assert!(at.get() > 240, "{at:?}");
    drain_for(&lb.rx, Duration::from_millis(50));
    run(cx, handle, transport::Play);
    let got = drain_for(&lb.rx, Duration::from_millis(300));
    assert!(got.iter().any(|m| m == &[0xC0, 5]), "{got:?}");
    run(cx, handle, transport::Stop);
}

/// 뱅크·프로그램 있는 128트랙에서 Play 20회 연타. 매번 재생 상태이고 마지막 재생이 소리 난다.
#[gpui_kit::test]
fn many_tracks_survive_play_burst(cx: &mut TestAppContext) {
    let lb = start(cx, "burst-128");
    let (handle, view) = open(
        cx,
        many_tracks(128, "[0, 240, 60, 51492], [1920, 240, 62, 51492]"),
    );
    for _ in 0..20 {
        run(cx, handle, transport::Play);
    }
    assert_eq!(
        state(cx, &view),
        TransportState::Playing,
        "{:?}",
        messages(cx, &view)
    );
    assert!(messages(cx, &view).is_empty(), "{:?}", messages(cx, &view));
    wait_for(&lb.rx, WAIT, note_on(62)).expect("Note On 62");
    run(cx, handle, transport::Stop);
}

/// 다른 창이 128트랙을 재생 중일 때 Play: 앞 창은 정지, 이 창이 소리 난다.
#[gpui_kit::test]
fn play_while_other_large_window_plays_takes_over(cx: &mut TestAppContext) {
    let lb = start(cx, "takeover-128");
    let (first, first_view) = open(cx, many_tracks(128, "[0, 240, 60, 51492]"));
    let (second, second_view) = open(cx, project("[480, 240, 70, 51492]"));
    run(cx, first, transport::Play);
    run(cx, second, transport::Play);
    assert_eq!(state(cx, &first_view), TransportState::Stopped);
    assert_eq!(state(cx, &second_view), TransportState::Playing);
    let got = wait_for(&lb.rx, WAIT, note_on(70)).expect("Note On 70");
    let at = got.iter().position(|m| note_on(70)(m)).unwrap();
    assert!(got[..at].iter().any(|m| m == &[0xC0, 5]), "{got:?}");
    run(cx, second, transport::Stop);
}

// ---------- 포트 문제 기억(리뷰 1 제안 3) ----------

/// 재생 중 편집으로 새 이름 포트를 처음 가리키면 그때 찾는다(기억은 「못 찾은」 이름만).
#[gpui_kit::test]
fn live_port_change_to_existing_named_port_is_heard(cx: &mut TestAppContext) {
    let _lb = start(cx, "live-named");
    let name = unique("live dest");
    let dest = destination(&name);
    let (handle, view) = open(cx, project("[0, 240, 60, 51492], [1920, 240, 62, 51492]"));
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_millis(100));
    let target = name.clone();
    edit(cx, &view, move |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: TrackProp::Port(OutputPort::Named(target)),
        }]
    });
    let got = wait_for(&dest.rx, WAIT, note_on(62)).expect("이름 포트로 옮긴 노트");
    let at = got.iter().position(|m| note_on(62)(m)).unwrap();
    assert!(got[..at].iter().any(|m| m == &[0xC0, 5]), "{got:?}");
    assert!(messages(cx, &view).is_empty(), "{:?}", messages(cx, &view));
    run(cx, handle, transport::Stop);
}

/// 재생 시작 때 없던 포트는 재생 중 편집으로 다시 찾지 않지만, 문구는 한 번만 남고 다음 재생 시작에서
/// 다시 찾는다.
#[gpui_kit::test]
fn missing_port_is_retried_on_next_play(cx: &mut TestAppContext) {
    let _lb = start(cx, "retry-missing");
    let name = unique("late dest");
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{{"port": {{"named": "{name}"}}, "channel": 1,
            "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 60, 51492]]}}]}}]}}"#
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    for i in 0..5 {
        edit(cx, &view, |p| {
            vec![Edit::SetTrackProp {
                track: p.tracks()[0].id(),
                prop: TrackProp::Name(format!("n{i}")),
            }]
        });
    }
    let found = messages(cx, &view);
    assert_eq!(
        found.iter().filter(|m| m.contains(&name)).count(),
        1,
        "{found:?}"
    );
    let dest = destination(&name);
    run(cx, handle, transport::Play);
    wait_for(&dest.rx, WAIT, note_on(60)).expect("다음 재생에서 찾아야 한다");
    run(cx, handle, transport::Stop);
}
