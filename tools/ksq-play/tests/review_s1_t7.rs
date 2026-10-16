//! S1-T7 적대적 리뷰 1: `ksq-play`를 깨뜨리려는 시도. 통과하는 테스트는 회귀 테스트로 남는다.
//!
//! 재생 전에 붙어 있어야 하는 관찰은 테스트가 만든 가상 목적지(OS 목적지)로 받는다. 트랙의
//! `OutputPort::Named`나 `--metronome-port`가 그 이름을 가리키면 `ksq-play`가 그 목적지를 연다.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{
    Channel, Clip, Note, OutputPort, Pitch, Program, Project, Tempo, Tick, Track, TrackColor,
    Velocity,
};
use keyscore_midi::{HostTime, MidiClient, destinations};
use ksq_play::{INTERRUPT, parse_args, run};

type Received = (u64, Vec<u8>);

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// 이 바이너리의 첫 `CoreMIDI` 클라이언트를 `keyscore-midi`가 만든다(S1-T2 알림 조건).
fn first_client() {
    let _ = MidiClient::new("ks-play-review").unwrap();
}

fn unique(name: &str) -> String {
    format!("KS play review {} {name}", std::process::id())
}

fn next_id() -> usize {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-play-review-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

/// 테스트가 만든 OS 목적지. `destinations()`에 보일 때까지 기다린다.
struct Dest {
    rx: mpsc::Receiver<Received>,
    _endpoint: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

fn destination(name: &str) -> Dest {
    let (tx, rx) = mpsc::channel();
    let client = coremidi::Client::new("ks-play-review-dest").unwrap();
    let dest = client
        .virtual_destination(name, move |list: &coremidi::PacketList| {
            for p in list {
                for msg in split(p.data()) {
                    let _ = tx.send((p.timestamp(), msg));
                }
            }
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !destinations().iter().any(|d| d.name() == name) {
        assert!(Instant::now() < deadline, "목적지가 보이지 않는다: {name}");
        std::thread::sleep(Duration::from_millis(5));
    }
    Dest {
        rx,
        _endpoint: dest,
        _client: client,
    }
}

/// `name` 가상 소스(ksq-play의 기본 포트)에 붙는 수신기.
struct Source {
    rx: mpsc::Receiver<Received>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn listen(name: String) -> std::thread::JoinHandle<Source> {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-play-review-rx").unwrap();
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
        Source {
            rx,
            _input: input,
            _client: client,
        }
    })
}

fn drain(rx: &mpsc::Receiver<Received>, quiet: Duration) -> Vec<Received> {
    let mut got = Vec::new();
    while let Ok(m) = rx.recv_timeout(quiet) {
        got.push(m);
    }
    got
}

fn bytes(msgs: &[Received]) -> Vec<Vec<u8>> {
    msgs.iter().map(|m| m.1.clone()).collect()
}

fn is_note_on(b: &[u8]) -> bool {
    b.len() == 3 && b[0] & 0xF0 == 0x90 && b[2] > 0
}

fn assert_all_released(msgs: &[Vec<u8>]) -> usize {
    let mut on = std::collections::HashMap::<(u8, u8), i64>::new();
    let mut ons = 0;
    for b in msgs {
        if is_note_on(b) {
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

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(90).unwrap(),
    )
    .unwrap()
}

fn track(name: &str, channel: u8, port: OutputPort, clip_len: u64, notes: &[Note]) -> Track {
    let mut t = Track::new(
        name,
        Channel::new(channel).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    t.set_port(port);
    let mut clip = Clip::new(Tick::ZERO, clip_len).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    t.insert_clip(clip).unwrap();
    t
}

fn write(dir: &Path, tracks: Vec<Track>, tempos: &[(u64, u32)]) -> PathBuf {
    let mut p = Project::new();
    for t in tracks {
        p.push_track(t).unwrap();
    }
    for &(at, bpm) in tempos {
        p.tempo_map_mut()
            .set(Tick::new(at), Tempo::from_bpm(bpm).unwrap());
    }
    let path = dir.join("song.ksq");
    ksq::write_file(&path, &p, Compression::None).unwrap();
    path
}

fn spawn_run(
    args: Vec<OsString>,
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
) -> (
    mpsc::Sender<String>,
    std::thread::JoinHandle<(u8, String, Duration)>,
) {
    let mut args = args;
    if !args.iter().any(|a| a == "--port") {
        args.push("--port".into());
        args.push(unique(&format!("auto {}", next_id())).into());
    }
    let handle = std::thread::spawn(move || {
        let started = Instant::now();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &rx, &mut out, &mut err);
        (
            code,
            String::from_utf8_lossy(&err).into_owned(),
            started.elapsed(),
        )
    });
    (tx, handle)
}

fn ticks_to_duration(from: u64, to: u64) -> Duration {
    HostTime::from_ticks(to).duration_since(HostTime::from_ticks(from))
}

// ---------- 인자 ----------

#[test]
fn parse_args_edge_values_do_not_panic_and_reject_out_of_range() {
    // 음의 0은 범위 검사(0.0..=3600)를 통과한다. Duration 변환에서 패닉하지 않아야 한다.
    let o = parse_args(&os(&["a.ksq", "--wait", "-0"])).unwrap();
    assert_eq!(o.wait(), Duration::ZERO);
    for bad in [
        &["a.ksq", "--wait", "inf"][..],
        &["a.ksq", "--wait", "3600.5"],
        &["a.ksq", "--from", "-1"],
        &["a.ksq", "--from", ""],
        &["a.ksq", "--metronome-port"],
        &["--port", "x"],
    ] {
        assert!(parse_args(&os(bad)).is_err(), "{bad:?}");
    }
    // 같은 옵션을 두 번 주면 뒤가 이긴다.
    let o = parse_args(&os(&["a.ksq", "--port", "A", "--port", "B"])).unwrap();
    assert_eq!(o.port_name(), "B");
}

#[test]
fn run_missing_file_and_unreachable_bar_exit_one_without_midi() {
    let (_tx, rx) = mpsc::channel::<String>();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        run(&os(&["/nonexistent/x.ksq"]), &rx, &mut out, &mut err),
        1
    );
    let dir = temp_dir("bar");
    let path = write(
        &dir,
        vec![track(
            "T",
            1,
            OutputPort::Virtual,
            3840,
            &[note(0, 480, 60)],
        )],
        &[],
    );
    let path = path.to_string_lossy().into_owned();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    // u64::MAX 번째 마디는 틱 범위 밖이다.
    assert_eq!(
        run(
            &os(&[&path, "--from", "18446744073709551615"]),
            &rx,
            &mut out,
            &mut err
        ),
        1
    );
    assert!(String::from_utf8_lossy(&err).contains("범위 밖"));
}

// ---------- 곡 끝 ----------

#[test]
fn song_end_waits_for_last_note_across_tempo_change() {
    first_client();
    let dir = temp_dir("tempo");
    let dest_name = unique("tempo");
    let dest = destination(&dest_name);
    // 첫 마디는 960 BPM(0.25 초), 둘째 마디부터 60 BPM. 마지막 노트는 둘째 마디의 한 박(1 초)이다.
    // 곡 길이를 첫 템포로 셈하면 그 노트가 정지에 잘린다.
    let path = write(
        &dir,
        vec![track(
            "T",
            1,
            OutputPort::Named(dest_name.clone()),
            3840 * 2,
            &[note(0, 480, 60), note(3840, 960, 62)],
        )],
        &[(0, 960), (3840, 60)],
    );
    let (tx, rx) = mpsc::channel();
    let (_tx, handle) = spawn_run(vec![path.into()], tx, rx);
    let (code, err, elapsed) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let msgs = drain(&dest.rx, Duration::from_millis(200));
    assert_eq!(assert_all_released(&bytes(&msgs)), 2);
    let on = msgs.iter().find(|m| m.1 == vec![0x90, 62, 90]).unwrap().0;
    let off = msgs
        .iter()
        .find(|m| m.1.len() == 3 && m.1[0] == 0x80 && m.1[1] == 62)
        .unwrap()
        .0;
    let held = ticks_to_duration(on, off);
    assert!(
        held >= Duration::from_millis(950) && held <= Duration::from_millis(1050),
        "마지막 노트 길이 {held:?}"
    );
    // 곡 끝(1.25 초) 뒤 오래 기다리지 않는다.
    assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
}

#[test]
fn from_bar_skips_earlier_notes_and_chases_program() {
    first_client();
    let dir = temp_dir("from");
    let dest_name = unique("from");
    let dest = destination(&dest_name);
    let mut t = track(
        "T",
        3,
        OutputPort::Named(dest_name.clone()),
        3840 * 2,
        &[note(0, 3840, 60), note(3840, 480, 62)],
    );
    t.set_program(Some(Program::new(40).unwrap()));
    let path = write(&dir, vec![t], &[(0, 480)]);
    let (tx, rx) = mpsc::channel();
    let (_tx, handle) = spawn_run(vec![path.into(), "--from".into(), "2".into()], tx, rx);
    let (code, err, _) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let msgs = bytes(&drain(&dest.rx, Duration::from_millis(200)));
    let pc = msgs.iter().position(|b| b == &vec![0xC2, 40]);
    let on = msgs.iter().position(|b| is_note_on(b));
    assert!(
        pc.is_some() && pc < on,
        "프로그램 chase가 노트 앞에 없다: {msgs:?}"
    );
    assert!(
        !msgs.iter().any(|b| is_note_on(b) && b[1] == 60),
        "{msgs:?}"
    );
    assert_eq!(assert_all_released(&msgs), 1);
}

#[test]
fn empty_project_finishes_quickly_with_zero() {
    first_client();
    let dir = temp_dir("empty");
    let path = dir.join("song.ksq");
    ksq::write_file(&path, &Project::new(), Compression::None).unwrap();
    let (tx, rx) = mpsc::channel();
    let (_tx, handle) = spawn_run(vec![path.into()], tx, rx);
    let (code, err, elapsed) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

// ---------- 끝내는 경로 ----------

/// 8마디 긴 노트(60 BPM에서 32초) 트랙을 `dest_name`으로 보내는 곡.
fn long_song(dir: &Path, dest_name: &str) -> PathBuf {
    let notes: Vec<Note> = (0..8u8)
        .map(|k| note(u64::from(k) * 3840, 3840, 60 + k))
        .collect();
    write(
        dir,
        vec![track(
            "Lead",
            1,
            OutputPort::Named(dest_name.to_string()),
            3840 * 8,
            &notes,
        )],
        &[(0, 60)],
    )
}

#[test]
fn quit_queued_before_play_sends_no_note() {
    first_client();
    let dir = temp_dir("early-q");
    let dest_name = unique("early-q");
    let dest = destination(&dest_name);
    let path = long_song(&dir, &dest_name);
    let (tx, rx) = mpsc::channel();
    // 재생 전에 이미 q가 와 있다(파일을 읽는 동안 사용자가 누름).
    tx.send("q".into()).unwrap();
    let (_tx, handle) = spawn_run(vec![path.into()], tx, rx);
    let (code, err, _) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let msgs = bytes(&drain(&dest.rx, Duration::from_millis(200)));
    assert_all_released(&msgs);
    let ons: Vec<_> = msgs.iter().filter(|b| is_note_on(b)).collect();
    assert!(ons.is_empty(), "정지 명령 뒤에 노트가 울렸다: {ons:?}");
}

#[test]
fn interrupt_queued_before_play_sends_no_note_and_exits_130() {
    first_client();
    let dir = temp_dir("early-int");
    let dest_name = unique("early-int");
    let dest = destination(&dest_name);
    let path = long_song(&dir, &dest_name);
    let (tx, rx) = mpsc::channel();
    tx.send(INTERRUPT.into()).unwrap();
    let (_tx, handle) = spawn_run(vec![path.into()], tx, rx);
    let (code, err, _) = handle.join().unwrap();
    assert_eq!(code, 130, "{err}");
    let msgs = bytes(&drain(&dest.rx, Duration::from_millis(200)));
    assert_all_released(&msgs);
    let ons: Vec<_> = msgs.iter().filter(|b| is_note_on(b)).collect();
    assert!(ons.is_empty(), "Ctrl-C 뒤에 노트가 울렸다: {ons:?}");
}

#[test]
fn interrupt_during_wait_exits_130_without_notes() {
    first_client();
    let dir = temp_dir("wait-int");
    let dest_name = unique("wait-int");
    let dest = destination(&dest_name);
    let path = long_song(&dir, &dest_name);
    let (tx, rx) = mpsc::channel();
    let (tx, handle) = spawn_run(vec![path.into(), "--wait".into(), "5".into()], tx, rx);
    std::thread::sleep(Duration::from_millis(300));
    tx.send(INTERRUPT.into()).unwrap();
    let (code, err, elapsed) = handle.join().unwrap();
    assert_eq!(code, 130, "{err}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    let msgs = bytes(&drain(&dest.rx, Duration::from_millis(200)));
    assert!(!msgs.iter().any(|b| is_note_on(b)), "{msgs:?}");
}

#[test]
fn panic_then_quit_and_closed_input_leave_nothing_sounding() {
    first_client();
    let dir = temp_dir("pq");
    let dest_name = unique("pq");
    let dest = destination(&dest_name);
    let path = long_song(&dir, &dest_name);
    let (tx, rx) = mpsc::channel();
    let (tx, handle) = spawn_run(vec![path.into()], tx, rx);
    let first = dest.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    // p와 q를 붙여 보낸다.
    tx.send("p".into()).unwrap();
    tx.send("q".into()).unwrap();
    let (code, err, _) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let mut all = vec![first.1];
    all.extend(bytes(&drain(&dest.rx, Duration::from_millis(200))));
    assert_all_released(&all);
    assert!(all.contains(&vec![0xB0, 123, 0]), "패닉 없음");

    // 입력이 닫힌(보낼 쪽이 없는) 채널: 곡 끝까지 재생하고 0으로 끝난다.
    let short = write(
        &dir,
        vec![track(
            "T",
            1,
            OutputPort::Named(dest_name.clone()),
            3840,
            &[note(0, 960, 64), note(960, 960, 65)],
        )],
        &[(0, 480)],
    );
    let (tx, rx) = mpsc::channel::<String>();
    drop(tx);
    let started = Instant::now();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let port = unique("pq-closed");
    let code = run(
        &[short.into(), "--port".into(), port.into()],
        &rx,
        &mut out,
        &mut err,
    );
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert!(started.elapsed() < Duration::from_secs(3));
    let msgs = bytes(&drain(&dest.rx, Duration::from_millis(200)));
    assert_eq!(assert_all_released(&msgs), 2);
}

// ---------- 포트 ----------

#[test]
fn named_port_present_routes_track_to_os_destination_only() {
    first_client();
    let dir = temp_dir("named-present");
    let dest_name = unique("named-present");
    let dest = destination(&dest_name);
    let main = unique("named-present-main");
    // 같은 이름을 쓰는 트랙 둘은 같은 목적지 하나로 간다.
    let path = write(
        &dir,
        vec![
            track(
                "A",
                1,
                OutputPort::Named(dest_name.clone()),
                3840,
                &[note(0, 480, 60)],
            ),
            track(
                "B",
                2,
                OutputPort::Named(dest_name.clone()),
                3840,
                &[note(480, 480, 62)],
            ),
            track("C", 3, OutputPort::Virtual, 3840, &[note(0, 480, 64)]),
        ],
        &[(0, 480)],
    );
    let listener = listen(main.clone());
    let (tx, rx) = mpsc::channel();
    let (_tx, handle) = spawn_run(
        vec![
            path.into(),
            "--port".into(),
            main.into(),
            "--wait".into(),
            "0.5".into(),
        ],
        tx,
        rx,
    );
    let source = listener.join().unwrap();
    let (code, err, _) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains(&dest_name), "있는 목적지인데 경고: {err}");
    let to_dest = bytes(&drain(&dest.rx, Duration::from_millis(200)));
    let to_main = bytes(&drain(&source.rx, Duration::from_millis(200)));
    assert_eq!(assert_all_released(&to_dest), 2);
    assert_eq!(assert_all_released(&to_main), 1);
    assert!(to_main.iter().all(|b| !is_note_on(b) || b[1] == 64));
}

#[test]
fn metronome_port_present_gets_clicks_and_main_port_none() {
    first_client();
    let dir = temp_dir("met-present");
    let click_name = unique("met-present");
    let click = destination(&click_name);
    let main = unique("met-present-main");
    let path = write(
        &dir,
        vec![track(
            "T",
            1,
            OutputPort::Virtual,
            3840,
            &[note(0, 1920, 60)],
        )],
        &[(0, 240)],
    );
    let listener = listen(main.clone());
    let (tx, rx) = mpsc::channel();
    let (_tx, handle) = spawn_run(
        vec![
            path.into(),
            "--port".into(),
            main.into(),
            "--metronome".into(),
            "--metronome-port".into(),
            click_name.into(),
            "--wait".into(),
            "0.5".into(),
        ],
        tx,
        rx,
    );
    let source = listener.join().unwrap();
    let (code, err, _) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let clicks = bytes(&drain(&click.rx, Duration::from_millis(200)));
    let to_main = bytes(&drain(&source.rx, Duration::from_millis(200)));
    assert!(
        clicks.iter().any(|b| is_note_on(b) && b[0] == 0x99),
        "{clicks:?}"
    );
    assert_all_released(&clicks);
    assert!(
        !to_main.iter().any(|b| b.len() == 3 && b[0] & 0x0F == 9),
        "{to_main:?}"
    );
}

/// MET-3: 메트로놈 전용 포트를 두는 까닭은 멜로디 신디에 클릭을 보내지 않기 위해서다. 그 포트가
/// 없을 때 클릭을 기본 가상 포트(멜로디 신디가 듣는 곳)로 보내면 그 목적을 거스른다.
#[test]
fn missing_metronome_port_does_not_click_on_main_port() {
    first_client();
    let dir = temp_dir("met-missing");
    let missing = unique("met-missing-nothing");
    let main = unique("met-missing-main");
    let path = write(
        &dir,
        vec![track(
            "T",
            1,
            OutputPort::Virtual,
            3840,
            &[note(0, 1920, 60)],
        )],
        &[(0, 240)],
    );
    let listener = listen(main.clone());
    let (tx, rx) = mpsc::channel();
    let (_tx, handle) = spawn_run(
        vec![
            path.into(),
            "--port".into(),
            main.into(),
            "--metronome".into(),
            "--metronome-port".into(),
            missing.into(),
            "--wait".into(),
            "0.5".into(),
        ],
        tx,
        rx,
    );
    let source = listener.join().unwrap();
    let (code, err, _) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let to_main = bytes(&drain(&source.rx, Duration::from_millis(200)));
    let clicks: Vec<_> = to_main
        .iter()
        .filter(|b| is_note_on(b) && b[0] == 0x99)
        .collect();
    assert!(
        clicks.is_empty(),
        "없는 메트로놈 포트의 클릭이 멜로디 포트로 갔다: {clicks:?}"
    );
}

/// SEQ-7·OUT-4: 트랙은 (포트, 채널) 하나다. 없는 이름 포트의 트랙을 기본 가상 포트로 합치면, 같은 채널을
/// 쓰는 다른 트랙과 섞여 프로그램이 덮이고 같은 음높이 노트가 서로를 끊는다.
#[test]
fn missing_named_port_does_not_merge_into_other_tracks_channel() {
    first_client();
    let dir = temp_dir("merge");
    let missing = unique("merge-nothing");
    let main = unique("merge-main");
    let mut piano = track("Piano", 1, OutputPort::Virtual, 3840, &[note(0, 3840, 60)]);
    piano.set_program(Some(Program::new(0).unwrap()));
    let mut strings = track(
        "Strings",
        1,
        OutputPort::Named(missing.clone()),
        3840,
        &[note(960, 960, 60)],
    );
    strings.set_program(Some(Program::new(48).unwrap()));
    let path = write(&dir, vec![piano, strings], &[(0, 480)]);
    let listener = listen(main.clone());
    let (tx, rx) = mpsc::channel();
    let (_tx, handle) = spawn_run(
        vec![
            path.into(),
            "--port".into(),
            main.into(),
            "--wait".into(),
            "0.5".into(),
        ],
        tx,
        rx,
    );
    let source = listener.join().unwrap();
    let (code, err, _) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let to_main = bytes(&drain(&source.rx, Duration::from_millis(200)));
    let programs: Vec<_> = to_main.iter().filter(|b| b[0] == 0xC0).collect();
    assert_eq!(
        programs,
        vec![&vec![0xC0, 0]],
        "다른 트랙의 프로그램이 피아노 채널을 덮었다: {to_main:?}"
    );
}

// ---------- 바이너리: 실제 SIGINT, 닫힌 표준 입력 ----------

#[test]
fn binary_sigint_with_closed_stdin_releases_notes_and_exits_130() {
    first_client();
    let dir = temp_dir("bin-int");
    let dest_name = unique("bin-int");
    let dest = destination(&dest_name);
    let path = long_song(&dir, &dest_name);
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ksq-play"))
        .arg(&path)
        .args(["--port", &unique("bin-int-main")])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // 표준 입력이 곧바로 닫혀도 재생이 이어지고 첫 노트가 온다.
    let first = dest.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(is_note_on(&first.1), "{first:?}");
    // 이 테스트가 띄운 자식 프로세스에만 SIGINT를 보낸다.
    let status = std::process::Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let deadline = Instant::now() + Duration::from_secs(5);
    let code = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s.code();
        }
        assert!(Instant::now() < deadline, "SIGINT 뒤 끝나지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(code, Some(130));
    let mut all = vec![first.1];
    all.extend(bytes(&drain(&dest.rx, Duration::from_millis(200))));
    assert_all_released(&all);
    assert!(all.contains(&vec![0xB0, 123, 0]), "패닉 없음: {all:?}");
}
