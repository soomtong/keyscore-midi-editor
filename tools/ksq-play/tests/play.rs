//! S1-T7 `ksq-play` 테스트. 인자 해석과, 가상 포트 루프백으로 본 재생 결과.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{
    Channel, Clip, Note, OutputPort, Pitch, Project, Tempo, Tick, Track, TrackColor, Velocity,
};
use keyscore_midi::{HostTime, MidiClient};
use ksq_play::{INTERRUPT, parse_args, run};

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

// ---------- 6. 인자 ----------

#[test]
fn parse_args_accepts_options_and_rejects_bad_usage() {
    let o = parse_args(&os(&["song.ksq"])).unwrap();
    assert_eq!(o.path(), Path::new("song.ksq"));
    assert_eq!(o.port_name(), "KeyScore Out");
    assert_eq!(o.from_bar(), 1);
    assert!(!o.metronome());
    assert_eq!(o.metronome_port(), None);
    assert_eq!(o.wait(), Duration::ZERO);
    let o = parse_args(&os(&[
        "--port",
        "My Out",
        "a.ksqz",
        "--from",
        "9",
        "--metronome",
        "--metronome-port",
        "Drums",
        "--wait",
        "2.5",
    ]))
    .unwrap();
    assert_eq!(o.path(), Path::new("a.ksqz"));
    assert_eq!(o.port_name(), "My Out");
    assert_eq!(o.from_bar(), 9);
    assert!(o.metronome());
    assert_eq!(o.metronome_port(), Some("Drums"));
    assert_eq!(o.wait(), Duration::from_millis(2500));
    for bad in [
        &[][..],
        &["a.ksq", "b.ksq"],
        &["a.ksq", "--from"],
        &["a.ksq", "--from", "0"],
        &["a.ksq", "--from", "x"],
        &["a.ksq", "--wait", "-1"],
        &["a.ksq", "--wait", "nan"],
        &["a.ksq", "--bogus"],
        &["a.ksq", "--port"],
    ] {
        assert!(parse_args(&os(bad)).is_err(), "{bad:?}");
    }
    // 잘못된 사용법은 종료 코드 2, 도움말은 0.
    let (_tx, rx) = mpsc::channel();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run(&os(&["--bogus"]), &rx, &mut out, &mut err), 2);
    assert!(String::from_utf8_lossy(&err).contains("사용법"));
    assert_eq!(run(&os(&["--help"]), &rx, &mut out, &mut err), 0);
}

// ---------- 루프백 도우미 ----------

/// 받은 메시지: (패킷 타임스탬프, 바이트).
type Received = (u64, Vec<u8>);

struct Receiver {
    rx: mpsc::Receiver<Received>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

/// 이 바이너리의 첫 CoreMIDI 클라이언트를 `keyscore-midi`가 만든다(알림 조건, S1-T2). 재생 도구는
/// 같은 프로세스에서 `MidiClient::new`를 다시 부르지만 프로세스 클라이언트는 하나다.
fn first_client() {
    let _ = MidiClient::new("ks-play-test").unwrap();
}

/// `name` 소스가 나타나면 연결한다. 재생이 시작되기 전에 붙어야 하므로 `--wait`와 함께 쓴다.
fn listen(name: String) -> std::thread::JoinHandle<Receiver> {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-play-rx").unwrap();
        let input = client
            .input_port("in", move |list: &coremidi::PacketList| {
                for p in list {
                    for msg in split(p.data()) {
                        let _ = tx.send((p.timestamp(), msg));
                    }
                }
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let source = loop {
            if let Some(s) = coremidi::Source::from_name(&name) {
                break s;
            }
            assert!(Instant::now() < deadline, "소스가 보이지 않는다: {name}");
            std::thread::sleep(Duration::from_millis(5));
        };
        input.connect_source(&source).unwrap();
        Receiver {
            rx,
            _input: input,
            _client: client,
        }
    })
}

fn split(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let len = match (data[i] & 0xF0, data[i]) {
            (0xC0 | 0xD0, _) | (0xF0, 0xF1 | 0xF3) => 2,
            (0xF0, 0xF2) => 3,
            (0xF0, _) => 1,
            _ => 3,
        };
        out.push(data[i..(i + len).min(data.len())].to_vec());
        i += len;
    }
    out
}

fn drain(rx: &mpsc::Receiver<Received>, quiet: Duration) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    while let Ok((_, b)) = rx.recv_timeout(quiet) {
        got.push(b);
    }
    got
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-play-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn port_name(name: &str) -> String {
    format!("KS play {} {name}", std::process::id())
}

/// (채널, 음높이)마다 켜짐 수가 끝에 0인지. Note On 수를 돌려준다.
fn assert_all_released(msgs: &[Vec<u8>]) -> usize {
    let mut on = std::collections::HashMap::<(u8, u8), i64>::new();
    let mut ons = 0;
    for b in msgs {
        if b.len() == 3 && b[0] & 0xF0 == 0x90 && b[2] > 0 {
            *on.entry((b[0] & 0x0F, b[1])).or_default() += 1;
            ons += 1;
        } else if b.len() == 3 && (b[0] & 0xF0 == 0x80 || b[0] & 0xF0 == 0x90) {
            let e = on.entry((b[0] & 0x0F, b[1])).or_default();
            *e = (*e - 1).max(0);
        }
    }
    let left: Vec<_> = on.iter().filter(|&(_, &n)| n > 0).collect();
    assert!(left.is_empty(), "걸린 노트: {left:?}");
    ons
}

/// 빠른 템포의 프로젝트를 쓴다.
fn write_fast(dir: &Path, mut project: Project, bpm: u32) -> PathBuf {
    project
        .tempo_map_mut()
        .set(Tick::ZERO, Tempo::from_bpm(bpm).unwrap());
    let path = dir.join("song.ksq");
    ksq::write_file(&path, &project, Compression::None).unwrap();
    path
}

fn long_note_project(port: OutputPort) -> Project {
    let mut p = Project::new();
    let mut t = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    t.set_port(port);
    let mut clip = Clip::new(Tick::ZERO, 3840 * 8).unwrap();
    for k in 0..8u64 {
        clip.insert_note(
            Note::new(
                Tick::new(k * 3840),
                3840,
                Pitch::new(60 + u8::try_from(k).unwrap()).unwrap(),
                Velocity::from_midi1(90).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
    t.insert_clip(clip).unwrap();
    p.push_track(t).unwrap();
    p
}

/// 재생을 다른 스레드에서 돌린다. (명령 보내기, 종료 코드·표준 오류 받기).
fn spawn_run(args: Vec<OsString>) -> (mpsc::Sender<String>, std::thread::JoinHandle<(u8, String)>) {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &rx, &mut out, &mut err);
        (code, String::from_utf8_lossy(&err).into_owned())
    });
    (tx, handle)
}

// ---------- 7–11. 재생 ----------

#[test]
fn play_fixture_sends_every_note_on_and_off_to_virtual_port() {
    first_client();
    let dir = temp_dir("fixture");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixtures/01-melody.ksq");
    let (project, _) = ksq::read_file(&fixture).unwrap();
    let notes: usize = project
        .tracks()
        .iter()
        .flat_map(Track::clips)
        .map(|(_, c)| c.notes().len())
        .sum();
    let path = write_fast(&dir, project, 960);
    let name = port_name("fixture");
    let listener = listen(name.clone());
    let started = Instant::now();
    let (_tx, handle) = spawn_run(vec![
        path.into(),
        "--port".into(),
        name.into(),
        "--wait".into(),
        "0.5".into(),
    ]);
    let receiver = listener.join().unwrap();
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let msgs = drain(&receiver.rx, Duration::from_millis(200));
    assert_eq!(assert_all_released(&msgs), notes);
    // 곡 끝에서 스스로 끝난다(무한히 기다리지 않는다).
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn panic_command_from_stdin_silences_all_channels() {
    first_client();
    let dir = temp_dir("panic");
    let path = write_fast(&dir, long_note_project(OutputPort::Virtual), 60);
    let name = port_name("panic");
    let listener = listen(name.clone());
    let (tx, handle) = spawn_run(vec![
        path.into(),
        "--port".into(),
        name.into(),
        "--wait".into(),
        "0.5".into(),
    ]);
    let receiver = listener.join().unwrap();
    // 첫 노트가 울린 뒤 패닉, 그 뒤 끝낸다.
    let first = receiver.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(first.1[0] & 0xF0, 0x90);
    tx.send("p".into()).unwrap();
    let after = drain(&receiver.rx, Duration::from_millis(300));
    for ch in 0..16u8 {
        for cc in [120u8, 123, 64] {
            assert!(after.contains(&vec![0xB0 | ch, cc, 0]), "채널 {ch} CC {cc}");
        }
    }
    tx.send("q".into()).unwrap();
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let mut all = vec![first.1];
    all.extend(after);
    all.extend(drain(&receiver.rx, Duration::from_millis(200)));
    assert_all_released(&all);
}

#[test]
fn quit_command_stops_and_releases_notes() {
    first_client();
    let dir = temp_dir("quit");
    let path = write_fast(&dir, long_note_project(OutputPort::Virtual), 60);
    let name = port_name("quit");
    let listener = listen(name.clone());
    let started = Instant::now();
    let (tx, handle) = spawn_run(vec![
        path.into(),
        "--port".into(),
        name.into(),
        "--wait".into(),
        "0.5".into(),
    ]);
    let receiver = listener.join().unwrap();
    let first = receiver.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send("q".into()).unwrap();
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    // 곡은 8마디(60 BPM에서 32초)다. q로 바로 끝났다.
    assert!(started.elapsed() < Duration::from_secs(10));
    let mut all = vec![first.1];
    all.extend(drain(&receiver.rx, Duration::from_millis(200)));
    assert_all_released(&all);
    // Ctrl-C는 패닉을 보내고 130으로 끝난다.
    let path = dir.join("song.ksq");
    let name = port_name("interrupt");
    let listener = listen(name.clone());
    let (tx, handle) = spawn_run(vec![
        path.into(),
        "--port".into(),
        name.into(),
        "--wait".into(),
        "0.5".into(),
    ]);
    let receiver = listener.join().unwrap();
    let _ = receiver.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    tx.send(INTERRUPT.into()).unwrap();
    let (code, _) = handle.join().unwrap();
    assert_eq!(code, 130);
    let after = drain(&receiver.rx, Duration::from_millis(200));
    assert!(after.contains(&vec![0xB0, 123, 0]), "패닉 없음: {after:?}");
}

#[test]
fn named_port_missing_falls_back_to_virtual_with_warning() {
    first_client();
    let dir = temp_dir("named");
    let missing = format!("No Such Port {}", std::process::id());
    let mut project = long_note_project(OutputPort::Named(missing.clone()));
    // 한 마디만 남긴다.
    let id = project.tracks()[0].clips().next().unwrap().0;
    project
        .track_mut(0)
        .unwrap()
        .split_clip(id, Tick::new(3840))
        .unwrap();
    let right = project.tracks()[0].clips().nth(1).unwrap().0;
    project.track_mut(0).unwrap().remove_clip(right);
    let path = write_fast(&dir, project, 480);
    let name = port_name("named");
    let listener = listen(name.clone());
    let (_tx, handle) = spawn_run(vec![
        path.into(),
        "--port".into(),
        name.into(),
        "--wait".into(),
        "0.5".into(),
    ]);
    let receiver = listener.join().unwrap();
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(err.contains(&missing), "경고 없음: {err}");
    let msgs = drain(&receiver.rx, Duration::from_millis(200));
    assert_eq!(assert_all_released(&msgs), 1);
}

#[test]
fn metronome_option_clicks_on_channel_ten() {
    first_client();
    let dir = temp_dir("metronome");
    let path = write_fast(&dir, long_note_project(OutputPort::Virtual), 240);
    let name = port_name("metronome");
    let listener = listen(name.clone());
    let started = HostTime::now();
    let (tx, handle) = spawn_run(vec![
        path.into(),
        "--port".into(),
        name.into(),
        "--metronome".into(),
        "--wait".into(),
        "0.5".into(),
    ]);
    let receiver = listener.join().unwrap();
    let mut clicks = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while clicks.len() < 5 && Instant::now() < deadline {
        if let Ok((t, b)) = receiver.rx.recv_timeout(Duration::from_millis(500))
            && b.len() == 3
            && b[0] == 0x99
            && b[2] > 0
        {
            clicks.push((t, b[1]));
        }
    }
    tx.send("q".into()).unwrap();
    let _ = handle.join().unwrap();
    assert!(clicks.len() >= 5, "{clicks:?}");
    let notes: Vec<u8> = clicks.iter().map(|c| c.1).collect();
    assert_eq!(&notes[..5], &[37, 42, 42, 42, 37]);
    assert!(clicks[0].0 >= started.ticks());
}
