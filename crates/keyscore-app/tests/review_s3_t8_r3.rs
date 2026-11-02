//! S3-T8 적대적 리뷰 3회차의 깨뜨리기 시험(`docs/reports/s3-t8/review-3.md`).
//!
//! 앞 회차가 보지 않은 앱 길: 재생 직후·짧은 간격의 재생·일시정지 Action 반복, 일시정지 사이에 다른 창 재생,
//! 일시정지 상태에서의 편집, 일시정지 중 MIDI 서버 재시작 복구. 기본 가상 포트를 같은 프로세스의 `coremidi`
//! 수신기로 받는다. 루프백 시험이라 `--test-threads=1`로 돌린다.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions, TransportState};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Edit, Project, TrackProp, ksq};

struct Loopback {
    rx: mpsc::Receiver<Vec<u8>>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

const FIRST: u8 = 40;

fn start(cx: &mut TestAppContext, name: &str) -> Loopback {
    let dir = std::env::temp_dir().join(format!("ks-review3-s3t8-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let port = format!("KS review3 s3t8 {} {name}", std::process::id());
    let options = EngineOptions {
        client_name: format!("ks-review3-s3t8-{name}"),
        port_name: port.clone(),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-review3-s3t8-rx").unwrap();
    let input = client
        .input_port("in", move |list: &coremidi::PacketList| {
            for p in list {
                let d = p.data();
                let mut i = 0;
                while i < d.len() {
                    let len = match d[i] & 0xF0 {
                        0xC0 | 0xD0 => 2,
                        0xF0 => 1,
                        _ => 3,
                    };
                    let _ = tx.send(d[i..(i + len).min(d.len())].to_vec());
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

/// 120 BPM, `delta`틱마다 음 번호가 다른 짧은 노트 `count`개(`first`부터).
fn notes(first: u8, count: u8, delta: u64) -> Project {
    let notes: Vec<String> = (0..count)
        .map(|i| {
            format!(
                "[{}, 30, {}, 51492]",
                if i == 0 { 0 } else { delta },
                first + i
            )
        })
        .collect();
    let length = (u64::from(count) * delta).div_ceil(1920) * 1920;
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{{"name": "N", "port": "virtual", "channel": 1,
            "clips": [{{"start": 0, "length": {length}, "notes": [{}]}}]}}]}}"#,
        notes.join(", ")
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

/// `limit` 동안 받은 Note On의 (상태 바이트, 음 번호).
fn heard_for(rx: &mpsc::Receiver<Vec<u8>>, limit: Duration) -> Vec<(u8, u8)> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(m) if m.len() == 3 && m[0] & 0xF0 == 0x90 && m[2] > 0 => got.push((m[0], m[1])),
            Ok(_) => {}
            Err(_) => return got,
        }
    }
}

fn pitches(got: &[(u8, u8)]) -> Vec<u8> {
    got.iter().map(|&(_, p)| p).collect()
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

/// 재생 Action 직후(0 ms)부터 노트 간격 몇 배까지 섞어 재생·일시정지 Action을 거듭한다. 들은 노트는 순서대로
/// 정확히 한 번이다(재생 직후 일시정지가 시작 자리를 잃거나 앞 정지의 답을 읽지 않는다).
#[gpui_kit::test]
fn rapid_play_pause_actions_hear_each_note_once(cx: &mut TestAppContext) {
    const COUNT: u8 = 40;
    let lb = start(cx, "rapid");
    // 노트 간격 120틱 = 62.5 ms
    let (handle, view) = open(cx, notes(FIRST, COUNT, 120));
    let mut rng = Lcg(0xBEEF);
    let mut heard = Vec::new();
    let mut cycles = 0;
    while heard.len() < usize::from(COUNT) && cycles < 400 {
        cycles += 1;
        run(cx, handle, transport::Play);
        assert_eq!(state(cx, &view), TransportState::Playing, "{cycles}회차");
        let wait = rng.next(40);
        if wait > 0 {
            std::thread::sleep(Duration::from_millis(wait));
        }
        run(cx, handle, transport::Pause);
        assert!(
            matches!(state(cx, &view), TransportState::Paused(_)),
            "{cycles}회차 {:?}",
            state(cx, &view)
        );
        heard.extend(pitches(&heard_for(&lb.rx, Duration::from_millis(15))));
    }
    run(cx, handle, transport::Stop);
    let expected: Vec<u8> = (FIRST..FIRST + COUNT).collect();
    assert_eq!(heard, expected, "{cycles}번 반복 뒤 겹침·빠짐");
}

/// 창 A를 일시정지하고 창 B가 재생·정지한 뒤 A를 이어서 재생한다. A는 일시정지 자리를 지키고, 이어서 들은
/// A의 노트는 겹치지도 빠지지도 않는다.
#[gpui_kit::test]
fn other_window_plays_between_pause_and_resume(cx: &mut TestAppContext) {
    let lb = start(cx, "other");
    let (ha, va) = open(cx, notes(FIRST, 24, 240));
    let (hb, vb) = open(cx, notes(90, 8, 120));
    // A의 음(40–63)만 모은다. 느린 CI 러너에서는 B의 음(90–97)이 20 ms 창보다 늦게 와 다음 회차 A의
    // 창에 섞였다(원격 CI, `[.., 45, 93, 46, ..]`). B 쪽 단언은 아래에서 따로 한다.
    let only_a = |p: Vec<u8>| p.into_iter().filter(|&p| p < 90).collect::<Vec<_>>();
    let mut heard_a = Vec::new();
    for round in 0..4 {
        run(cx, ha, transport::Play);
        std::thread::sleep(Duration::from_millis(180 + 37 * round));
        run(cx, ha, transport::Pause);
        let TransportState::Paused(at) = state(cx, &va) else {
            panic!("{round}회차 A {:?}", state(cx, &va));
        };
        heard_a.extend(only_a(pitches(&heard_for(
            &lb.rx,
            Duration::from_millis(20),
        ))));
        run(cx, hb, transport::Play);
        std::thread::sleep(Duration::from_millis(100));
        if round % 2 == 0 {
            run(cx, hb, transport::Stop);
        }
        // B의 재생은 A의 일시정지 자리를 건드리지 않는다
        assert_eq!(state(cx, &va), TransportState::Paused(at), "{round}회차");
        let b = pitches(&heard_for(&lb.rx, Duration::from_millis(20)));
        assert!(
            b.iter().all(|&p| p >= 90),
            "{round}회차 B 사이에 A 노트 {b:?}"
        );
        assert!(!b.is_empty(), "{round}회차 B가 울리지 않았다");
    }
    run(cx, ha, transport::Play);
    assert_ne!(
        state(cx, &vb),
        TransportState::Playing,
        "B가 재생 중으로 남았다"
    );
    heard_a.extend(only_a(pitches(&heard_for(
        &lb.rx,
        Duration::from_millis(400),
    ))));
    run(cx, ha, transport::Stop);
    let n = u8::try_from(heard_a.len()).unwrap();
    let expected: Vec<u8> = (FIRST..FIRST + n).collect();
    assert_eq!(heard_a, expected, "A의 노트가 겹치거나 빠졌다");
}

/// 일시정지 상태에서 트랙 채널을 바꾸고 이어서 재생한다. 이어서 들은 노트는 새 채널이고, 일시정지 전과 합쳐
/// 겹침·빠짐이 없다(이어서 재생할 틱은 옛 스냅샷에서 구했지만 새 스냅샷의 같은 틱부터 보낸다).
#[gpui_kit::test]
fn edit_while_paused_then_resume_continues_on_new_snapshot(cx: &mut TestAppContext) {
    let lb = start(cx, "edit");
    let (handle, view) = open(cx, notes(FIRST, 24, 240));
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_millis(370));
    run(cx, handle, transport::Pause);
    assert!(matches!(state(cx, &view), TransportState::Paused(_)));
    let before = heard_for(&lb.rx, Duration::from_millis(20));
    assert!(!before.is_empty());
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let track = doc.project().tracks()[0].id();
            doc.apply(vec![Edit::SetTrackProp {
                track,
                prop: TrackProp::Channel(Channel::new(2).unwrap()),
            }])
            .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    assert!(
        matches!(state(cx, &view), TransportState::Paused(_)),
        "편집이 일시정지를 바꿨다: {:?}",
        state(cx, &view)
    );
    run(cx, handle, transport::Play);
    let after = heard_for(&lb.rx, Duration::from_millis(400));
    run(cx, handle, transport::Stop);
    assert!(!after.is_empty(), "이어서 재생이 울리지 않았다");
    assert!(
        after.iter().all(|&(s, _)| s == 0x91),
        "이어서 재생에 옛 채널 노트: {after:?}"
    );
    let all: Vec<u8> = pitches(&before)
        .into_iter()
        .chain(pitches(&after))
        .collect();
    let n = u8::try_from(all.len()).unwrap();
    assert_eq!(all, (FIRST..FIRST + n).collect::<Vec<_>>());
}

/// 일시정지 중 MIDI 서버 재시작 복구(`recover_virtual_port`)가 와도 창은 일시정지 자리를 지키고, 이어서 재생하면
/// 겹침·빠짐 없이 울린다.
#[gpui_kit::test]
fn recover_while_paused_keeps_pause_position(cx: &mut TestAppContext) {
    let lb = start(cx, "recover");
    let (handle, view) = open(cx, notes(FIRST, 24, 240));
    run(cx, handle, transport::Play);
    std::thread::sleep(Duration::from_millis(430));
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    let before = pitches(&heard_for(&lb.rx, Duration::from_millis(20)));
    cx.update(engine::recover_virtual_port).unwrap();
    cx.run_until_parked();
    assert_eq!(state(cx, &view), TransportState::Paused(at));
    run(cx, handle, transport::Play);
    let after = pitches(&heard_for(&lb.rx, Duration::from_millis(400)));
    run(cx, handle, transport::Stop);
    assert!(!after.is_empty(), "복구 뒤 이어서 재생이 울리지 않았다");
    let all: Vec<u8> = before.into_iter().chain(after).collect();
    let n = u8::try_from(all.len()).unwrap();
    assert_eq!(all, (FIRST..FIRST + n).collect::<Vec<_>>());
}
