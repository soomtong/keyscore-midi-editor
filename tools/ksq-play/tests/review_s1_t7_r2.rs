//! S1-T7 적대적 리뷰 2: 리뷰 1 지적 1·2(없는 이름 포트를 따로 만든다)와 제안 4–6 반영을 깨뜨리려는 시도.
//!
//! 가상 포트 이름 목록과 16개 상한은 프로세스 전체가 함께 쓴다. 이 파일의 테스트가 동시에 돌면 서로의
//! 포트 때문에 상한에 닿을 수 있어, 재생하는 테스트는 `serial()`로 하나씩 돌린다. 상한을 넘기는 시도는
//! 따로 띄운 바이너리에서 한다.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, mpsc};
use std::time::{Duration, Instant};

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{
    Channel, Clip, Note, OutputPort, Pitch, Project, Tempo, Tick, Track, TrackColor, Velocity,
};
use keyscore_midi::{MAX_VIRTUAL_OUTPUTS, MidiClient};
use ksq_play::run;

type Received = (u64, Vec<u8>);

fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 이 바이너리의 첫 `CoreMIDI` 클라이언트를 `keyscore-midi`가 만든다(S1-T2 알림 조건).
fn first_client() {
    let _ = MidiClient::new("ks-play-review2").unwrap();
}

fn unique(name: &str) -> String {
    format!("KS r2 {} {name}", std::process::id())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-play-r2-{}-{name}", std::process::id()));
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

/// `name` 가상 소스(ksq-play가 만든 포트)에 붙는 수신기.
struct Source {
    rx: mpsc::Receiver<Received>,
    _input: coremidi::InputPort,
    _client: coremidi::Client,
}

fn listen(name: String) -> std::thread::JoinHandle<Source> {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new("ks-play-r2-rx").unwrap();
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

fn drain(rx: &mpsc::Receiver<Received>, quiet: Duration) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    while let Ok(m) = rx.recv_timeout(quiet) {
        got.push(m.1);
    }
    got
}

fn is_note_on(b: &[u8]) -> bool {
    b.len() == 3 && b[0] & 0xF0 == 0x90 && b[2] > 0
}

fn note_ons(msgs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    msgs.iter().filter(|b| is_note_on(b)).cloned().collect()
}

fn assert_all_released(msgs: &[Vec<u8>]) {
    let mut on = std::collections::HashMap::<(u8, u8), i64>::new();
    for b in msgs {
        if is_note_on(b) {
            *on.entry((b[0] & 0x0F, b[1])).or_default() += 1;
        } else if b.len() == 3 && (b[0] & 0xF0 == 0x80 || b[0] & 0xF0 == 0x90) {
            let e = on.entry((b[0] & 0x0F, b[1])).or_default();
            *e = (*e - 1).max(0);
        }
    }
    let left: Vec<_> = on.iter().filter(|&(_, &n)| n > 0).collect();
    assert!(left.is_empty(), "걸린 노트: {left:?}");
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

fn track(name: &str, channel: u8, port: OutputPort, notes: &[Note]) -> Track {
    let mut t = Track::new(
        name,
        Channel::new(channel).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    t.set_port(port);
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    t.insert_clip(clip).unwrap();
    t
}

fn write(dir: &Path, tracks: Vec<Track>, bpm: u32) -> PathBuf {
    let mut p = Project::new();
    for t in tracks {
        p.push_track(t).unwrap();
    }
    p.tempo_map_mut()
        .set(Tick::ZERO, Tempo::from_bpm(bpm).unwrap());
    let path = dir.join("song.ksq");
    ksq::write_file(&path, &p, Compression::None).unwrap();
    path
}

/// 같은 프로세스에서 재생한다. `--wait 0.5`로 수신기가 붙을 시간을 준다.
fn run_in_process(args: Vec<OsString>) -> std::thread::JoinHandle<(u8, String)> {
    std::thread::spawn(move || {
        let (_tx, rx) = mpsc::channel::<String>();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &rx, &mut out, &mut err);
        (code, String::from_utf8_lossy(&err).into_owned())
    })
}

fn args(path: &Path, extra: &[&str]) -> Vec<OsString> {
    let mut v: Vec<OsString> = vec![path.into()];
    v.extend(extra.iter().map(OsString::from));
    v
}

// ---------- A. 없는 이름 포트 ----------

/// 같은 없는 이름을 쓰는 두 트랙은 그 이름의 가상 포트 하나를 함께 쓴다(이름 = 목적지). 기본 포트에는
/// 아무것도 가지 않는다.
#[test]
fn two_tracks_same_missing_port_share_one_virtual_port() {
    let _g = serial();
    first_client();
    let dir = temp_dir("share");
    let main = unique("share-main");
    let missing = unique("share-missing");
    let path = write(
        &dir,
        vec![
            track(
                "A",
                1,
                OutputPort::Named(missing.clone()),
                &[note(0, 480, 60)],
            ),
            track(
                "B",
                2,
                OutputPort::Named(missing.clone()),
                &[note(0, 480, 64)],
            ),
        ],
        480,
    );
    let main_rx = listen(main.clone());
    let own_rx = listen(missing.clone());
    let handle = run_in_process(args(&path, &["--port", &main, "--wait", "0.5"]));
    let (main_rx, own_rx) = (main_rx.join().unwrap(), own_rx.join().unwrap());
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        err.matches(&missing).count(),
        1,
        "경고가 한 번이 아니다: {err}"
    );
    let own = drain(&own_rx.rx, Duration::from_millis(200));
    assert_all_released(&own);
    assert_eq!(
        note_ons(&own),
        vec![vec![0x90, 60, 90], vec![0x91, 64, 90]],
        "{own:?}"
    );
    let stray = drain(&main_rx.rx, Duration::from_millis(50));
    assert!(stray.is_empty(), "기본 포트로 감: {stray:?}");
}

/// 없는 이름이 `--port` 이름과 같으면 기본 포트가 곧 그 목적지다. 새 포트를 만들려다 이름 중복으로
/// 실패하지 않는다. 앞뒤 공백만 다른 이름도 같은 포트다(가상 포트 이름 정규화).
#[test]
fn missing_port_equal_to_main_name_routes_to_main_port() {
    let _g = serial();
    first_client();
    let dir = temp_dir("same-main");
    let main = unique("same-main");
    let path = write(
        &dir,
        vec![
            track("A", 1, OutputPort::Named(main.clone()), &[note(0, 480, 60)]),
            track(
                "B",
                2,
                OutputPort::Named(format!("  {main} ")),
                &[note(0, 480, 64)],
            ),
            track("C", 3, OutputPort::Virtual, &[note(0, 480, 67)]),
        ],
        480,
    );
    let main_rx = listen(main.clone());
    let handle = run_in_process(args(&path, &["--port", &main, "--wait", "0.5"]));
    let main_rx = main_rx.join().unwrap();
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let got = drain(&main_rx.rx, Duration::from_millis(200));
    assert_all_released(&got);
    assert_eq!(note_ons(&got).len(), 3, "{got:?}");
}

/// 가상 포트 이름으로 못 쓰는 이름(제어 문자)의 트랙은 「기본 이름 N」 포트로 따로 간다. 경고에는 이름을
/// 이스케이프해 적는다(터미널 제어 문자를 그대로 내보내지 않는다).
#[test]
fn control_char_track_port_gets_numbered_fallback_not_main() {
    let _g = serial();
    first_client();
    let dir = temp_dir("ctrl-track");
    let main = unique("ctrl-track");
    let bad = format!("Bad\u{1b}[31m{}", std::process::id());
    let path = write(
        &dir,
        vec![
            track("Piano", 1, OutputPort::Virtual, &[note(0, 480, 60)]),
            track(
                "Odd",
                1,
                OutputPort::Named(bad.clone()),
                &[note(0, 480, 60)],
            ),
        ],
        480,
    );
    let main_rx = listen(main.clone());
    let fallback_rx = listen(format!("{main} 2"));
    let handle = run_in_process(args(&path, &["--port", &main, "--wait", "0.5"]));
    let (main_rx, fallback_rx) = (main_rx.join().unwrap(), fallback_rx.join().unwrap());
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(
        !err.contains('\u{1b}'),
        "제어 문자가 그대로 나갔다: {err:?}"
    );
    assert!(err.contains(&format!("{main} 2")), "{err}");
    let to_main = drain(&main_rx.rx, Duration::from_millis(200));
    let to_fallback = drain(&fallback_rx.rx, Duration::from_millis(50));
    assert_all_released(&to_main);
    assert_all_released(&to_fallback);
    assert_eq!(note_ons(&to_main).len(), 1, "{to_main:?}");
    assert_eq!(note_ons(&to_fallback).len(), 1, "{to_fallback:?}");
}

/// 메트로놈 포트 이름이 제어 문자를 품어도 클릭은 기본 포트(멜로디)로 가지 않는다(MET-3).
#[test]
fn control_char_metronome_port_clicks_on_fallback_not_main() {
    let _g = serial();
    first_client();
    let dir = temp_dir("ctrl-met");
    let main = unique("ctrl-met");
    let path = write(
        &dir,
        vec![track("T", 1, OutputPort::Virtual, &[note(0, 1920, 60)])],
        240,
    );
    let main_rx = listen(main.clone());
    let fallback_rx = listen(format!("{main} 2"));
    let handle = run_in_process(args(
        &path,
        &[
            "--port",
            &main,
            "--metronome",
            "--metronome-port",
            "Click\u{7}",
            "--wait",
            "0.5",
        ],
    ));
    let (main_rx, fallback_rx) = (main_rx.join().unwrap(), fallback_rx.join().unwrap());
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let to_main = drain(&main_rx.rx, Duration::from_millis(200));
    let to_fallback = drain(&fallback_rx.rx, Duration::from_millis(50));
    assert!(
        !to_main.iter().any(|b| is_note_on(b) && b[0] == 0x99),
        "클릭이 멜로디 포트로 갔다: {to_main:?}"
    );
    assert!(
        to_fallback.iter().any(|b| is_note_on(b) && b[0] == 0x99),
        "대체 포트에 클릭이 없다: {to_fallback:?}"
    );
    assert_all_released(&to_fallback);
}

/// 메트로놈 포트와 트랙이 같은 없는 이름을 가리키면 그 이름의 포트 하나를 함께 쓴다. 기본 포트에는
/// 클릭도 노트도 가지 않는다.
#[test]
fn metronome_and_track_share_same_missing_port() {
    let _g = serial();
    first_client();
    let dir = temp_dir("met-share");
    let main = unique("met-share-main");
    let missing = unique("met-share-missing");
    let path = write(
        &dir,
        vec![track(
            "T",
            1,
            OutputPort::Named(missing.clone()),
            &[note(0, 1920, 60)],
        )],
        240,
    );
    let main_rx = listen(main.clone());
    let own_rx = listen(missing.clone());
    let handle = run_in_process(args(
        &path,
        &[
            "--port",
            &main,
            "--metronome",
            "--metronome-port",
            &missing,
            "--wait",
            "0.5",
        ],
    ));
    let (main_rx, own_rx) = (main_rx.join().unwrap(), own_rx.join().unwrap());
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let own = drain(&own_rx.rx, Duration::from_millis(200));
    assert_all_released(&own);
    assert!(own.contains(&vec![0x90, 60, 90]), "{own:?}");
    assert!(own.iter().any(|b| is_note_on(b) && b[0] == 0x99), "{own:?}");
    let stray = drain(&main_rx.rx, Duration::from_millis(50));
    assert!(note_ons(&stray).is_empty(), "기본 포트로 감: {stray:?}");
}

/// 대체 이름 「기본 이름 N」이 다른 트랙의 없는 이름과 같으면 이름 중복이다. 트랙을 합치지 않고 오류
/// 1로 끝난다(패닉 없음). 대체 이름을 비어 있는 이름으로 고르게 고쳐도 통과한다(리뷰 2 제안 2).
#[test]
fn fallback_name_colliding_with_another_missing_name_fails_cleanly() {
    let _g = serial();
    first_client();
    let dir = temp_dir("collide");
    let main = unique("collide");
    let path = write(
        &dir,
        vec![
            track(
                "Odd",
                1,
                OutputPort::Named("X\u{1}".into()),
                &[note(0, 480, 60)],
            ),
            track(
                "Named",
                1,
                OutputPort::Named(format!("{main} 2")),
                &[note(0, 480, 62)],
            ),
        ],
        480,
    );
    let handle = run_in_process(args(&path, &["--port", &main]));
    let (code, err) = handle.join().unwrap();
    assert!(code == 0 || code == 1, "{code} {err}");
    if code == 1 {
        assert!(err.contains(&format!("{main} 2")), "{err}");
    }
}

/// 뒤 공백만 다른 두 없는 이름은 `by_name`(원문 키)에서는 다르지만 가상 포트 이름(정규화)으로는 같다.
/// 지금은 둘째 포트 생성이 이름 중복으로 실패해 오류 1로 끝난다(리뷰 2 제안 1). 패닉하거나 조용히 기본
/// 포트로 합치지 않으면 된다. 같은 포트를 함께 쓰게 고쳐도 이 테스트는 통과한다.
#[test]
fn whitespace_variant_missing_names_fail_cleanly_or_share() {
    let _g = serial();
    first_client();
    let dir = temp_dir("ws");
    let main = unique("ws-main");
    let base = unique("ws");
    let path = write(
        &dir,
        vec![
            track("A", 1, OutputPort::Named(base.clone()), &[note(0, 480, 60)]),
            track(
                "B",
                1,
                OutputPort::Named(format!("{base} ")),
                &[note(0, 480, 62)],
            ),
        ],
        480,
    );
    let handle = run_in_process(args(&path, &["--port", &main]));
    let (code, err) = handle.join().unwrap();
    assert!(code == 0 || code == 1, "{code} {err}");
}

fn binary(path: &Path, main: &str, extra: &[&str]) -> std::process::Child {
    std::process::Command::new(env!("CARGO_BIN_EXE_ksq-play"))
        .arg(path)
        .args(["--port", main])
        .args(extra)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap()
}

fn wait_child(child: std::process::Child, limit: Duration) -> (Option<i32>, String) {
    let deadline = Instant::now() + limit;
    let mut child = child;
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "자식이 끝나지 않는다");
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn many_missing(dir: &Path, count: usize) -> PathBuf {
    let tracks = (0..count)
        .map(|i| {
            track(
                &format!("T{i}"),
                1,
                OutputPort::Named(unique(&format!("many {i}"))),
                &[note(0, 240, 60)],
            )
        })
        .collect();
    write(dir, tracks, 960)
}

/// 기본 포트 + 없는 이름 15개 = 16개(OUT-2 상한)는 재생된다. 별도 프로세스라 다른 테스트의 포트와
/// 상한을 나누지 않는다.
#[test]
fn missing_names_up_to_virtual_port_limit_play_in_binary() {
    first_client();
    let dir = temp_dir("limit-ok");
    let path = many_missing(&dir, MAX_VIRTUAL_OUTPUTS - 1);
    let child = binary(&path, &unique("limit-ok-main"), &[]);
    let (code, err) = wait_child(child, Duration::from_secs(10));
    assert_eq!(code, Some(0), "{err}");
}

/// 상한을 넘기면 트랙을 합치지 않고, 패닉 없이 오류 1과 상한 문구로 끝난다.
#[test]
fn missing_names_over_virtual_port_limit_fail_cleanly_in_binary() {
    first_client();
    let dir = temp_dir("limit-over");
    let path = many_missing(&dir, MAX_VIRTUAL_OUTPUTS);
    let child = binary(&path, &unique("limit-over-main"), &[]);
    let (code, err) = wait_child(child, Duration::from_secs(10));
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("16"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
}

// ---------- B. 제안 4–6 ----------

/// 제안 4: `--metronome` 없는 `--metronome-port`는 경고하고, 그 포트를 만들지 않으며, 클릭이 없다.
#[test]
fn metronome_port_without_metronome_warns_and_creates_nothing() {
    let _g = serial();
    first_client();
    let dir = temp_dir("met-only-port");
    let main = unique("met-only-main");
    let missing = unique("met-only-missing");
    let path = write(
        &dir,
        vec![track("T", 1, OutputPort::Virtual, &[note(0, 1920, 60)])],
        240,
    );
    let main_rx = listen(main.clone());
    let handle = run_in_process(args(
        &path,
        &[
            "--port",
            &main,
            "--metronome-port",
            &missing,
            "--wait",
            "0.5",
        ],
    ));
    let main_rx = main_rx.join().unwrap();
    // 재생 중에 그 이름의 소스가 생기지 않았다.
    assert!(coremidi::Source::from_name(&missing).is_none());
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("--metronome"), "경고 없음: {err}");
    assert!(!err.contains(&missing), "포트를 만들려 했다: {err}");
    let got = drain(&main_rx.rx, Duration::from_millis(200));
    assert!(!got.iter().any(|b| b[0] == 0x99), "{got:?}");
    assert_eq!(note_ons(&got).len(), 1, "{got:?}");
}

/// 제안 5: 곡 끝 뒤 마디는 오류 1. 곡 끝 마디의 바로 앞 마디는 그대로 재생한다.
#[test]
fn from_bar_after_song_end_exits_one_and_last_bar_still_plays() {
    let _g = serial();
    first_client();
    let dir = temp_dir("from-end");
    // 두 마디 클립, 둘째 마디에 노트. 끝 = 마디 3 시작.
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 7680).unwrap();
    clip.insert_note(note(3840, 480, 60)).unwrap();
    t.insert_clip(clip).unwrap();
    let mut p = Project::new();
    p.push_track(t).unwrap();
    p.tempo_map_mut()
        .set(Tick::ZERO, Tempo::from_bpm(960).unwrap());
    let path = dir.join("song.ksq");
    ksq::write_file(&path, &p, Compression::None).unwrap();

    let (code, err) = run_in_process(args(
        &path,
        &["--port", &unique("from-end-a"), "--from", "4"],
    ))
    .join()
    .unwrap();
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("곡 끝 뒤"), "{err}");

    let main = unique("from-end-b");
    let rx = listen(main.clone());
    let handle = run_in_process(args(
        &path,
        &["--port", &main, "--from", "2", "--wait", "0.5"],
    ));
    let rx = rx.join().unwrap();
    let (code, err) = handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let got = drain(&rx.rx, Duration::from_millis(200));
    assert_eq!(note_ons(&got), vec![vec![0x90, 60, 90]], "{got:?}");
}

/// 제안 6: 두 번째 SIGINT는 정리를 기다리지 않고 130으로 끝난다(이 테스트가 띄운 자식에만 보낸다).
/// 두 신호가 거의 함께 와도 패닉이 나가 걸린 노트가 남지 않는지 본다.
#[test]
fn binary_double_sigint_exits_130_and_leaves_no_note() {
    let _g = serial();
    first_client();
    let dir = temp_dir("double-int");
    let main = unique("double-int-main");
    let notes: Vec<Note> = (0..8u8)
        .map(|k| note(u64::from(k) * 3840, 3840, 60 + k))
        .collect();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840 * 8).unwrap();
    for n in notes {
        clip.insert_note(n).unwrap();
    }
    t.insert_clip(clip).unwrap();
    let mut p = Project::new();
    p.push_track(t).unwrap();
    p.tempo_map_mut()
        .set(Tick::ZERO, Tempo::from_bpm(60).unwrap());
    let path = dir.join("song.ksq");
    ksq::write_file(&path, &p, Compression::None).unwrap();

    let rx = listen(main.clone());
    let child = binary(&path, &main, &["--wait", "0.5"]);
    let rx = rx.join().unwrap();
    let first = rx.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(is_note_on(&first.1), "{first:?}");
    let pid = child.id().to_string();
    for _ in 0..2 {
        let status = std::process::Command::new("kill")
            .args(["-INT", &pid])
            .status()
            .unwrap();
        assert!(status.success());
    }
    let (code, err) = wait_child(child, Duration::from_secs(5));
    assert_eq!(code, Some(130), "{err}");
    let mut all = vec![first.1];
    all.extend(drain(&rx.rx, Duration::from_millis(200)));
    assert_all_released(&all);
}
