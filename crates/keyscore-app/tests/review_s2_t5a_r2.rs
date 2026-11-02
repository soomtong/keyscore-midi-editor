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
use keyscore_core::{Bank, Edit, OutputPort, Project, Tempo, Tick, TrackProp, ksq};
use keyscore_midi::SchedulerConfig;

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
    let before_play = Instant::now();
    run(cx, handle, transport::Play);
    let after_play = Instant::now();
    std::thread::sleep(Duration::from_millis(300));
    let before_60 = Instant::now();
    edit(cx, &view, |_| {
        vec![Edit::SetTempo {
            at: Tick::ZERO,
            tempo: Some(Tempo::from_bpm(60).unwrap()),
        }]
    });
    let after_60 = Instant::now();
    std::thread::sleep(Duration::from_millis(300));
    let before_240 = Instant::now();
    edit(cx, &view, |_| {
        vec![Edit::SetTempo {
            at: Tick::ZERO,
            tempo: Some(Tempo::from_bpm(240).unwrap()),
        }]
    });
    let after_240 = Instant::now();
    std::thread::sleep(Duration::from_millis(300));
    let before_pause = Instant::now();
    run(cx, handle, transport::Pause);
    let after_pause = Instant::now();
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    // 고정 구간(1800–2300)은 느린 CI 러너에서 넘쳤다(원격 CI, 2834). 마지막 구간이 240 BPM이라 잠이
    // 0.2 s만 길어져도 약 800틱이 는다. 그래서 각 구간의 실제 길이로 범위를 잡는다. 템포는 편집 호출
    // 안의 어느 순간에 바뀌므로, 위 끝은 틱이 가장 많이 가게(120 BPM을 늦게 끝내고 240 BPM을 일찍
    // 시작), 아래 끝은 가장 적게 가게 잡는다.
    // lookahead(`SchedulerConfig::default()`, 엔진이 쓰는 값)는 위 끝에 더한다. 교체는 보낸 구간 뒤부터
    // 새 템포를 쓰므로 바뀌는 순간이 lookahead만큼 늦을 수 있다. 그 몫과 내림은 여유 `slack`이 덮는다.
    // 템포 편집을 기준점에 반영하지 않는 구현(0.9 s를 120 또는 240 BPM 하나로 셈)은 약 1730 또는 3460틱이라
    // 이 범위를 벗어난다.
    let slack: u128 = 100;
    let lookahead = SchedulerConfig::default().lookahead;
    let us = |a: Instant, b: Instant| b.saturating_duration_since(a).as_micros();
    let ticks = |at_120: u128, at_60: u128, at_240: u128| {
        (at_120 * 1920 + at_60 * 960 + at_240 * 3840) / 1_000_000
    };
    let latest = ticks(
        us(before_play, after_60),
        us(after_60, before_240),
        us(before_240, after_pause) + lookahead.as_micros(),
    ) + slack;
    let earliest = ticks(
        us(after_play, before_60),
        us(before_60, after_240),
        us(after_240, before_pause),
    )
    .saturating_sub(slack);
    let got = u128::from(at.get());
    assert!(
        (earliest..=latest).contains(&got),
        "일시정지 자리가 경과 시간과 맞지 않는다: {at:?}(범위 {earliest}..={latest})"
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

// ---------- 리뷰 2 부록 A(지적 1–5): 고친 뒤 회귀 방지로 옮김 ----------

/// 4분음표마다 같은 음(`pitch`)을 32번(시작은 델타).
fn every_beat(pitch: u8) -> String {
    (0..32)
        .map(|i| format!("[{}, 240, {pitch}, 51492]", if i == 0 { 0 } else { 480 }))
        .collect::<Vec<_>>()
        .join(", ")
}

/// 깨뜨림 1: 앱이 `Scheduler::collect_garbage`를 부르지 않는다. 교체가 260번쯤(돌려받기 큐 256 +
/// 대기 자리 4) 쌓이면 다음 교체가 큐 머리에서 영영 막히고, 그 뒤의 명령은 처리되지 않는다. Play마다
/// 교체가 하나다. 막힌 Play의 `stop`은 이미 처리돼 소리가 멎는데 창은 「재생 중」이고 메시지도 없다.
#[gpui_kit::test]
fn play_still_sounds_after_many_plays(cx: &mut TestAppContext) {
    let lb = start(cx, "garbage-play");
    let (handle, view) = open(cx, project(&every_beat(60)));
    for _ in 0..280 {
        run(cx, handle, transport::Play);
        std::thread::sleep(Duration::from_millis(3));
    }
    assert_eq!(
        state(cx, &view),
        TransportState::Playing,
        "{:?}",
        messages(cx, &view)
    );
    drain_for(&lb.rx, Duration::from_millis(50));
    let heard = wait_for(&lb.rx, WAIT, note_on(60));
    let msgs = messages(cx, &view);
    run(cx, handle, transport::Stop);
    assert!(heard.is_some(), "창은 재생 중인데 소리가 없다: {msgs:?}");
}

/// 깨뜨림 1의 변형(TRN-3의 주 사용): 재생 중 편집을 300번 하면 그 뒤 교체가 막혀 편집이 들리지 않고,
/// 정지(Stop)도 막힌 교체 뒤에 줄 서서 재생이 멈추지 않는다. 메시지도 없다.
#[gpui_kit::test]
fn stop_works_after_many_live_edits(cx: &mut TestAppContext) {
    let lb = start(cx, "garbage-edit");
    let (handle, view) = open(cx, project(&every_beat(60)));
    run(cx, handle, transport::Play);
    for i in 0..300 {
        edit(cx, &view, |p| {
            vec![Edit::SetTrackProp {
                track: p.tracks()[0].id(),
                prop: TrackProp::Name(format!("n{i}")),
            }]
        });
        std::thread::sleep(Duration::from_millis(2));
    }
    run(cx, handle, transport::Stop);
    assert_eq!(state(cx, &view), TransportState::Stopped);
    drain_for(&lb.rx, Duration::from_millis(150));
    let after = drain_for(&lb.rx, Duration::from_millis(1200));
    let msgs = messages(cx, &view);
    run(cx, handle, transport::Panic);
    assert!(
        !after.iter().any(|m| note_on(60)(m)),
        "정지 뒤에도 재생된다: {after:?}, 메시지 {msgs:?}"
    );
}

/// 깨뜨림 2: 재생 시작 때 뮤트였던 트랙은 렌더러가 빼서 chase가 그 프로그램을 보내지 않는다. 재생 중
/// 뮤트를 풀면 `setup`은 그대로라(뮤트 트랙도 들어 있다) 차이가 없어 프로그램이 나가지 않는다(OUT-4, TRN-3).
#[gpui_kit::test]
fn unmuting_track_while_playing_sends_its_program(cx: &mut TestAppContext) {
    let lb = start(cx, "unmute-program");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [
        {"port": "virtual", "channel": 1, "program": 5,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492]]}]},
        {"port": "virtual", "channel": 2, "program": 9, "mute": true,
         "clips": [{"start": 0, "length": 7680, "notes": [[1920, 240, 64, 51492]]}]}]}"#;
    let (handle, view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    // 구현 대응: 들리지 않는 트랙의 뱅크·프로그램도 스냅샷 틱 0에 넣어 재생 시작 때 chase로 보낸다.
    // 그래서 프로그램은 뮤트를 풀기 전에 이미 나간다. 뮤트를 푼 트랙의 첫 노트까지 받은 것 전체에서 찾는다.
    let mut got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[1].id(),
            prop: TrackProp::Mute(false),
        }]
    });
    got.extend(wait_for(&lb.rx, WAIT, |m| note_on(64)(m) && m[0] == 0x91).expect("Note On 64"));
    run(cx, handle, transport::Stop);
    let program = got.iter().position(|m| m == &[0xC1, 9]);
    let note = got.iter().position(|m| note_on(64)(m) && m[0] == 0x91);
    assert!(
        program.is_some() && program < note,
        "뮤트를 푼 트랙의 프로그램이 나가지 않았다: {got:?}"
    );
}

/// 깨뜨림 3: 재생 중 뱅크만 바꾸면 바뀐 뱅크 셀렉트(CC 32)만 나가고 프로그램 체인지가 따라가지 않는다.
/// 뱅크 셀렉트는 다음 프로그램 체인지에서야 적용되므로 소리가 바뀌지 않는다(OUT-4, TRN-3).
#[gpui_kit::test]
fn bank_edit_while_playing_is_followed_by_program(cx: &mut TestAppContext) {
    let lb = start(cx, "bank-edit");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [{"port": "virtual", "channel": 1, "bank": 1, "program": 5,
        "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492], [1920, 240, 62, 51492]]}]}]}"#;
    let (handle, view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: TrackProp::Bank(Some(Bank::new(2).unwrap())),
        }]
    });
    let got = wait_for(&lb.rx, WAIT, note_on(62)).expect("Note On 62");
    run(cx, handle, transport::Stop);
    let bank = got.iter().position(|m| m == &[0xB0, 32, 2]);
    let program = got.iter().rposition(|m| m == &[0xC0, 5]);
    assert!(
        matches!((bank, program), (Some(b), Some(p)) if b < p),
        "바뀐 뱅크 뒤에 프로그램 체인지가 없다: {got:?}"
    );
}

/// 깨뜨림 4: `setup` 차이는 (스냅샷 포트 번호, 메시지)로 비교한다. 포트 번호는 스냅샷마다 트랙 순서로
/// 다시 매겨지므로, 트랙을 다른 이름 포트로 옮겨 번호가 당겨지면 새 목적지에 처음 가는 프로그램이 옛
/// 목적지의 같은 번호 항목과 같다고 보고 빠진다.
#[gpui_kit::test]
fn moving_track_to_other_named_port_sends_program_there(cx: &mut TestAppContext) {
    let _lb = start(cx, "move-port");
    let x = unique("move x");
    let y = unique("move y");
    let dx = destination(&x);
    let dy = destination(&y);
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
            {{"port": {{"named": "{x}"}}, "channel": 1, "program": 5,
              "clips": [{{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492], [1920, 240, 62, 51492]]}}]}},
            {{"port": {{"named": "{y}"}}, "channel": 2, "program": 9,
              "clips": [{{"start": 0, "length": 7680, "notes": [[0, 240, 64, 51492]]}}]}}]}}"#
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    wait_for(&dx.rx, WAIT, note_on(60)).expect("X Note On 60");
    wait_for(&dy.rx, WAIT, note_on(64)).expect("Y Note On 64");
    let target = y.clone();
    edit(cx, &view, move |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: TrackProp::Port(OutputPort::Named(target)),
        }]
    });
    let got = wait_for(&dy.rx, WAIT, |m| note_on(62)(m) && m[0] == 0x90).expect("Y Note On 62");
    run(cx, handle, transport::Stop);
    assert!(
        got.iter().any(|m| m == &[0xC0, 5]),
        "옮긴 트랙의 프로그램이 새 포트에 가지 않았다: {got:?}"
    );
}

/// 깨뜨림 5: 기준점을 다시 잡을 때 자리는 교체 전에 구하고 시각은 `prepare`·`swap` 뒤에 찍는다. 그 사이
/// 시간(렌더링)이 편집마다 빠져 일시정지 자리가 실제보다 뒤로 밀린다. 큰 프로젝트에서 편집을 거듭하면
/// 재개 때 이미 들은 부분을 다시 연주한다.
#[gpui_kit::test]
fn pause_position_does_not_drift_with_many_live_edits(cx: &mut TestAppContext) {
    let lb = start(cx, "anchor-drift");
    // 노트는 아주 뒤에 두어 소리 없이 렌더링만 무겁게 한다
    let notes: Vec<String> = (0..4000)
        .map(|i| format!("[{}, 48, 60, 51492]", if i == 0 { 2_000_000 } else { 96 }))
        .collect();
    let notes = notes.join(", ");
    let tracks: Vec<String> = (0..16)
        .map(|i| {
            format!(
                r#"{{"port": "virtual", "channel": {}, "clips": [{{"start": 0, "length": 3000000, "notes": [{notes}]}}]}}"#,
                i + 1
            )
        })
        .collect();
    // 실제 재생 위치를 재는 박 트랙(채널 16, 480틱마다 노트 70)
    let beats: Vec<String> = (0..32)
        .map(|i| format!("[{}, 120, 70, 51492]", if i == 0 { 0 } else { 480 }))
        .collect();
    let beat = format!(
        r#"{{"port": "virtual", "channel": 16, "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}}"#,
        beats.join(", ")
    );
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}, {beat}]}}"#,
        tracks.join(",")
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    let begin = Instant::now();
    for i in 0..40 {
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
    // 일시정지 전에 들은 박 수: 마지막 박의 틱이 실제 위치의 하한이다
    let heard = drain_for(&lb.rx, Duration::from_millis(100))
        .iter()
        .filter(|m| note_on(70)(m))
        .count();
    let floor = u64::try_from(heard.saturating_sub(1) * 480).unwrap();
    assert!(heard >= 4, "박을 {heard}개만 들었다");
    assert!(
        at.get() >= floor,
        "일시정지 자리 {at:?}가 이미 들은 박(틱 {floor}, {heard}개, 경과 {secs:.2} s)보다 앞이다"
    );
}
