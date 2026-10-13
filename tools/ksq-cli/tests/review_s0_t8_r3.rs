//! S0-T8 적대적 리뷰 3회차(`docs/reports/s0-t8/review-3.md`)가 남긴 테스트.
//!
//! - 실패하는 테스트: 평범한 곡(번들 픽스처 `08-bass-drums`, 16마디 루프)을 `.ksqz`로 저장하면
//!   어림이 압축 파일 바이트의 256배를 넘어 거부되는 것(지적 1), 한 자리에 사건이 몰려 성부가 예산
//!   밖에서 계속 열리는 클립에서 어림이 실제보다 작은 것(지적 2), `-o /dev/stdout`이 일반 파일로
//!   돌려진 표준 출력에서 실패하는 것(지적 3).
//! - 통과하는 테스트: 평범한 긴 곡을 거부하지 않는 경계, 특수 파일(심볼릭 링크 너머 `/dev/null`,
//!   FIFO)과 일반 파일의 쓰기 경로, 특수 파일에 쓰다 실패하는 경로.
//!
//! FIFO 테스트는 바이너리와 읽는 쪽 모두에 시간 제한을 둔다. 읽는 쪽이 막혀 있으면 FIFO를 읽기·쓰기
//! (`O_RDWR`)로 열어 풀어 준다. FIFO를 `O_RDWR`로 여는 것은 상대가 없어도 막히지 않는다.

use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use ksq_cli::{CliError, project_to_text, project_to_text_limited};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const WHOLE: u64 = 3840;
const TIMEOUT: Duration = Duration::from_secs(20);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-cli-review3-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

fn note(start: u64, length: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 시작 0, 길이 `length`인 클립 하나에 `notes`를 담은 트랙.
fn track(name: &str, length: u64, notes: impl IntoIterator<Item = Note>) -> Track {
    let mut clip = Clip::new(Tick::ZERO, length).unwrap();
    for n in notes {
        clip.insert_note(n).unwrap();
    }
    let mut t = Track::new(name, Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    t.insert_clip(clip).unwrap();
    t
}

fn project(tracks: impl IntoIterator<Item = Track>) -> Project {
    let mut p = Project::new();
    for t in tracks {
        p.push_track(t).unwrap();
    }
    p
}

/// 온음표 `wholes`개 뒤 한 자리에 길이만 다른(1..=n틱) 노트 `n`개. 같은 시작·길이가 아니라 화음으로
/// 묶이지 않고, 모두 겹치므로 사건마다 성부가 필요하다.
fn crammed(n: u32, wholes: u64) -> Project {
    let start = wholes * WHOLE;
    project([track(
        "T",
        start + u64::from(n) + 1,
        (1..=n).map(|len| note(start, len, 60)),
    )])
}

/// 자식 프로세스를 기다리되 `TIMEOUT`을 넘으면 죽이고 실패한다.
fn wait_with_timeout(mut child: Child, what: &str) -> (ExitStatus, String) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                std::io::Read::read_to_string(&mut e, &mut err).unwrap();
            }
            return (status, err);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what}: {TIMEOUT:?} 안에 끝나지 않았다");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 읽는 쪽 스레드를 기다린다. 시간 안에 끝나지 않으면 `unblock`(같은 FIFO를 가리키는 경로)을
/// `O_RDWR`로 열었다 닫아 풀어 준다. 쓰기 전용 열기와 달리 읽는 쪽이 이미 끝났어도 막히지 않는다.
fn join_reader<T>(reader: JoinHandle<T>, unblock: &Path) -> T {
    let deadline = Instant::now() + TIMEOUT;
    while !reader.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !reader.is_finished() {
        drop(
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(unblock)
                .unwrap(),
        );
    }
    reader.join().unwrap()
}

fn mkfifo(path: &Path) {
    let made = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(made.success());
}

// --- 지적 2: 어림 ---

/// 한 자리에 사건 1,000개가 몰리면 성부 16개를 넘는 사건은 앞 성부에 8개쯤 접혀 들어간 뒤
/// (커서 앞지름 한도 480틱 / 접기 60틱) 예산과 상관없이 새 성부를 연다(`voices`의 「남는 경우」).
/// 새 성부마다 클립 시작부터 쉼표를 다시 쓰므로 실제는 약 (사건 수 / 8) × 온음표 수 × 5바이트인데,
/// 어림은 16개를 넘는 성부를 예산 하나로만 센다.
#[test]
fn project_to_text_crammed_events_far_start_estimate_bounds_actual() {
    let p = crammed(1000, 10_000);
    let (text, _) = project_to_text(&p).unwrap();

    let tight = project_to_text_limited(&p, text.len() as u64 - 1);

    match tight {
        Err(CliError::OutputTooLarge { detail, .. }) => assert!(
            !detail.contains("어림보다 실제 출력이 컸다"),
            "어림이 실제({}바이트)보다 작다: {detail}",
            text.len()
        ),
        other => panic!("{other:?}"),
    }
}

/// 같은 입력을 상한 근처까지 키우면 상한의 약 두 배 텍스트를 다 만든 뒤에야 안전망이 거부한다.
/// 어림이 걸러야 한다(`project_to_text` 문서). 사건 수에 대해 제곱으로 커진다(리뷰 3 측정: 67 KB
/// `.ksq` → 61 MB 텍스트·RSS 566 MB, 169 KB → 336 MB·RSS 2.7 GB).
#[test]
fn project_to_text_crammed_events_near_limit_rejected_by_estimate_before_building() {
    let p = crammed(1000, 20_000);

    let err = project_to_text(&p).unwrap_err();

    let CliError::OutputTooLarge { detail, .. } = &err else {
        panic!("{err}");
    };
    assert!(!detail.contains("어림보다 실제 출력이 컸다"), "{err}");
}

/// 평범한 긴 곡(1,000마디, 4분음표 멜로디·온음표 베이스, 1,000마디에 흩은 노트 20개 트랙,
/// 먼 곳에 노트 셋인 트랙)은 보수적인 어림에도 거부되지 않는다.
#[test]
fn project_to_text_thousand_bar_song_with_sparse_tracks_is_accepted() {
    let song_bars = 1000;
    let melody = track(
        "Melody",
        song_bars * WHOLE,
        (0..song_bars * 4).map(|i| note(i * 960, 960, 60 + u8::try_from(i % 12).unwrap())),
    );
    let bass = track(
        "Bass",
        song_bars * WHOLE,
        (0..song_bars).map(|i| note(i * WHOLE, 3840, 36)),
    );
    let crash = track(
        "Crash",
        song_bars * WHOLE,
        (0..20).map(|i| note(i * (song_bars / 20) * WHOLE, 240, 49)),
    );
    let pad = track(
        "Pad",
        song_bars * WHOLE,
        [0, 500, 999].map(|b| note(b * WHOLE, 3840, 72)),
    );

    let r = project_to_text(&project([melody, bass, crash, pad]));

    assert!(r.is_ok(), "{:?}", r.err());
}

// --- 지적 1: `.ksqz` 입력의 상한 ---

/// `.ksq`를 그대로 `.ksqz`로 저장해 `ksq2txt`에 주고, 종료 코드·표준 오류와 표준 출력을 돌려준다.
fn ksq2txt_as_ksqz(dir: &Path, name: &str, p: &Project) -> (ExitStatus, String, Vec<u8>) {
    let input = dir.join(format!("{name}.ksqz"));
    ksq::write_file(&input, p, Compression::Gzip).unwrap();
    let child = Command::new(KSQ2TXT)
        .arg(&input)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status,
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.stdout,
    )
}

/// 번들 픽스처를 `.ksqz`로 저장해도 `.ksq`와 같은 텍스트가 나와야 한다. `08-bass-drums`는 564바이트
/// `.ksqz`가 되어 상한이 144,384바이트(파일의 256배)인데, 어림이 161,470바이트라 거부된다. 실제
/// 텍스트는 1,007바이트(파일의 1.8배)다. 어림이 노트마다 500바이트 넘게, 노트가 16개를 넘는 트랙은
/// 온음표마다 약 340바이트를 잡는데, 압축 파일은 노트 하나에 몇 바이트뿐이라서다.
#[test]
fn ksq2txt_bundled_fixtures_saved_as_ksqz_convert_like_ksq() {
    let dir = temp_dir("fixtures-ksqz");
    let mut failed = Vec::new();
    for name in [
        "01-melody",
        "02-melody-chords",
        "03-drums",
        "04-band",
        "05-lyrics",
        "06-waltz",
        "07-triplets",
        "08-bass-drums",
    ] {
        let path = fixture(&format!("{name}.ksq"));
        let expected = Command::new(KSQ2TXT).arg(&path).output().unwrap().stdout;
        let (p, _) = ksq::read_file(&path).unwrap();

        let (status, err, out) = ksq2txt_as_ksqz(&dir, name, &p);

        if status.code() != Some(0) || out != expected {
            failed.push(format!("{name}: {status} {err}"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
    let _ = fs::remove_dir_all(&dir);
}

/// 16마디 드럼(8분 하이햇과 킥·스네어)과 4분음표 베이스를 퀀타이즈한 평범한 루프. `.ksq`는 통과하지만
/// `.ksqz`(약 400바이트)는 어림 214 KB > 상한 101 KB로 거부된다. 실제 텍스트는 약 1.8 KB다.
#[test]
fn ksq2txt_sixteen_bar_quantized_loop_as_ksqz_converts() {
    let dir = temp_dir("loop-ksqz");
    let loop_bars = 16;
    let mut clip = Clip::new(Tick::ZERO, loop_bars * WHOLE).unwrap();
    for i in 0..loop_bars * 8 {
        let t = i * 480;
        clip.insert_note(note(t, 240, 42)).unwrap();
        match i % 4 {
            0 => clip.insert_note(note(t, 240, 36)).unwrap(),
            2 => clip.insert_note(note(t, 240, 38)).unwrap(),
            _ => {}
        }
    }
    let mut drums = Track::new(
        "Drums",
        Channel::new(10).unwrap(),
        TrackColor::new(1).unwrap(),
    );
    drums.insert_clip(clip).unwrap();
    let bass = track(
        "Bass",
        loop_bars * WHOLE,
        (0..loop_bars * 4).map(|i| note(i * 960, 960, 36 + 2 * u8::try_from(i % 4).unwrap())),
    );

    let (status, err, out) = ksq2txt_as_ksqz(&dir, "loop", &project([drums, bass]));

    assert_eq!(status.code(), Some(0), "{err}");
    assert!(!out.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

// --- 지적 3·특수 파일 쓰기 ---

/// 표준 출력을 일반 파일로 돌려 `-o /dev/stdout`을 주면 `/dev/stdout`의 대상(`/dev/fd/1`)이 일반
/// 파일이라 원자적 쓰기 경로로 가고, `/dev/fd`에 임시 파일을 만들지 못해 실패한다. 리뷰 1 수정 전의
/// `ksq2txt`는 이 쓰임에 종료 0이었다.
fn dev_stdout_to_file(bin: &str, input: &Path, out: &Path) -> (ExitStatus, String) {
    let file = fs::File::create(out).unwrap();
    let child = Command::new(bin)
        .arg(input)
        .args(["-o", "/dev/stdout"])
        .stdout(Stdio::from(file))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_with_timeout(child, bin)
}

#[test]
fn ksq2txt_output_dev_stdout_redirected_to_regular_file_writes_text() {
    let dir = temp_dir("stdout-ksq2txt");
    let input = fixture("01-melody.ksq");
    let expected = Command::new(KSQ2TXT).arg(&input).output().unwrap().stdout;
    let out = dir.join("out.txt");

    let (status, err) = dev_stdout_to_file(KSQ2TXT, &input, &out);

    assert_eq!(status.code(), Some(0), "{err}");
    assert_eq!(fs::read(&out).unwrap(), expected);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn txt2ksq_output_dev_stdout_redirected_to_regular_file_writes_project() {
    let dir = temp_dir("stdout-txt2ksq");
    let input = fixture("01-melody.ksq");
    let txt = dir.join("in.txt");
    fs::write(
        &txt,
        Command::new(KSQ2TXT).arg(&input).output().unwrap().stdout,
    )
    .unwrap();
    let out = dir.join("out.ksq");

    let (status, err) = dev_stdout_to_file(TXT2KSQ, &txt, &out);

    assert_eq!(status.code(), Some(0), "{err}");
    assert_eq!(fs::read(&out).unwrap(), fs::read(&input).unwrap());
    let _ = fs::remove_dir_all(&dir);
}

/// `/dev/null`·FIFO를 가리키는 심볼릭 링크는 대상에 바로 쓰고 링크를 그대로 둔다.
#[test]
fn ksq2txt_output_symlinks_to_dev_null_and_fifo_write_through_and_keep_links() {
    let dir = temp_dir("special-links");
    let input = fixture("01-melody.ksq");
    let expected = Command::new(KSQ2TXT).arg(&input).output().unwrap().stdout;
    let null_link = dir.join("null.txt");
    std::os::unix::fs::symlink("/dev/null", &null_link).unwrap();
    let fifo = dir.join("pipe");
    mkfifo(&fifo);
    let fifo_link = dir.join("pipe.txt");
    std::os::unix::fs::symlink(&fifo, &fifo_link).unwrap();
    let reader = {
        let fifo = fifo.clone();
        std::thread::spawn(move || fs::read(fifo).unwrap())
    };

    let a = Command::new(KSQ2TXT)
        .arg(&input)
        .arg("-o")
        .arg(&null_link)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (a, a_err) = wait_with_timeout(a, "null link");
    let b = Command::new(KSQ2TXT)
        .arg(&input)
        .arg("-o")
        .arg(&fifo_link)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (b, b_err) = wait_with_timeout(b, "fifo link");
    let got = join_reader(reader, &fifo);

    assert_eq!(a.code(), Some(0), "{a_err}");
    assert_eq!(b.code(), Some(0), "{b_err}");
    assert_eq!(got, expected);
    assert!(fs::symlink_metadata(&null_link).unwrap().is_symlink());
    assert!(fs::symlink_metadata(&fifo_link).unwrap().is_symlink());
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    let _ = fs::remove_dir_all(&dir);
}

/// 읽는 쪽이 한 바이트만 읽고 닫으면 FIFO 쓰기가 중간에 실패한다(EPIPE). 종료 1과 진단을 내고,
/// FIFO는 그대로이며 임시 파일이 남지 않는다.
#[test]
fn ksq2txt_output_fifo_reader_closes_early_exits_1_and_keeps_fifo() {
    let dir = temp_dir("fifo-epipe");
    // 텍스트가 파이프 버퍼(64 KiB)보다 훨씬 크게: 4분음표 4만 개.
    let big = project([track(
        "Big",
        10_000 * WHOLE,
        (0..40_000u64).map(|i| note(i * 960, 960, 48 + u8::try_from(i % 37).unwrap())),
    )]);
    let input = dir.join("big.ksq");
    ksq::write_file(&input, &big, Compression::None).unwrap();
    let fifo = dir.join("pipe");
    mkfifo(&fifo);
    let reader = {
        let fifo = fifo.clone();
        std::thread::spawn(move || {
            let mut f = fs::File::open(fifo).unwrap();
            let mut one = [0u8; 1];
            std::io::Read::read(&mut f, &mut one).unwrap()
        })
    };

    let child = Command::new(KSQ2TXT)
        .arg(&input)
        .arg("-o")
        .arg(&fifo)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (status, err) = wait_with_timeout(child, "epipe");
    let read = join_reader(reader, &fifo);

    assert_eq!(read, 1);
    assert_eq!(status.code(), Some(1), "{err}");
    assert!(err.contains("입출력 오류"), "{err}");
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    let _ = fs::remove_dir_all(&dir);
}

/// 일반 파일 출력은 여전히 임시 파일 + 이름 바꾸기로 쓴다: 새 inode가 되어 하드 링크는 옛 내용을
/// 유지하고, 임시 파일이 남지 않는다. 두 바이너리 모두.
#[test]
fn binaries_output_regular_file_still_replaced_by_rename() {
    let dir = temp_dir("rename");
    let input = fixture("01-melody.ksq");
    let txt = dir.join("in.txt");
    fs::write(
        &txt,
        Command::new(KSQ2TXT).arg(&input).output().unwrap().stdout,
    )
    .unwrap();
    for (bin, src, out) in [
        (KSQ2TXT, input.as_path(), dir.join("out.txt")),
        (TXT2KSQ, txt.as_path(), dir.join("out.ksq")),
    ] {
        fs::write(&out, b"old").unwrap();
        let link = dir.join("link");
        fs::hard_link(&out, &link).unwrap();
        let before = fs::metadata(&out).unwrap().ino();

        let child = Command::new(bin)
            .arg(src)
            .arg("-o")
            .arg(&out)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (status, err) = wait_with_timeout(child, bin);

        assert_eq!(status.code(), Some(0), "{err}");
        assert_ne!(fs::metadata(&out).unwrap().ino(), before);
        assert_eq!(fs::read(&link).unwrap(), b"old");
        assert_ne!(fs::read(&out).unwrap(), b"old");
        fs::remove_file(&link).unwrap();
    }
    let mut names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["in.txt", "out.ksq", "out.txt"]);
    let _ = fs::remove_dir_all(&dir);
}
