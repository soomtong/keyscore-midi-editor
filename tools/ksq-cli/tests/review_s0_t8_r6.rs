//! S0-T8 적대적 리뷰 6회차(`docs/reports/s0-t8/review-6.md`)가 남긴 테스트. 범위는 리뷰 5 수정 두
//! 커밋(끝이 `/`인 `-o`를 원래 경로로 열기, u64를 넘는 어림을 「어떤 상한으로도 변환 불가」로 알리기)이다.
//!
//! - 실패하는 테스트: 끝이 `/.`인 `-o`(`/dev/null/.`, `/dev/stdout/.`, `/dev/null/.`을 가리키는 링크)를
//!   커널(ENOTDIR)과 달리 받아들여 쓰는 것. `Path`의 구성요소 분해가 끝의 `.`을 버려 `/dev/null`로
//!   다시 짜기 때문이며, 리뷰 5 지적 1과 같은 갈래다.
//! - 통과하는 테스트: 링크 사슬 중간의 `/`, `//`, 디렉터리·없는 경로 뒤의 `/`, 디렉터리 `-o`, 일반 파일의
//!   원자적 쓰기, `/dev/null`·`/dev/stdout`, 어림이 u64 바로 아래·위일 때의 오류와 안내값.
//!
//! 바이너리를 띄우는 테스트는 시간 제한을 둔다. 어떤 테스트도 거대한 출력을 실제로 만들지 않는다.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use ksq_cli::{CliError, project_to_text, project_to_text_limited};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const TIMEOUT: Duration = Duration::from_secs(30);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-cli-review6-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

fn melody_text() -> String {
    project_to_text(&ksq::read_file(&fixture("01-melody.ksq")).unwrap().0)
        .unwrap()
        .0
}

struct Ran {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

/// 자식 프로세스를 기다리되 `TIMEOUT`을 넘으면 죽이고 실패한다. 표준 출력·오류는 스레드로 비워
/// 파이프가 차서 멈추지 않게 한다.
fn wait(mut child: Child, what: &str) -> Ran {
    let out = child.stdout.take().map(|mut o| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            std::io::Read::read_to_end(&mut o, &mut v).unwrap();
            v
        })
    });
    let err = child.stderr.take().map(|mut e| {
        std::thread::spawn(move || {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut e, &mut s).unwrap();
            s
        })
    });
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what}: {TIMEOUT:?} 안에 끝나지 않았다");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ran {
        code: status.code(),
        stdout: out.map(|h| h.join().unwrap()).unwrap_or_default(),
        stderr: err.map(|h| h.join().unwrap()).unwrap_or_default(),
    }
}

fn run_with(bin: &str, args: &[&OsStr], stdout: Stdio) -> Ran {
    let child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait(child, bin)
}

fn run(bin: &str, args: &[&OsStr]) -> Ran {
    run_with(bin, args, Stdio::piped())
}

fn ksq2txt_o(output: &Path) -> Ran {
    run(
        KSQ2TXT,
        &[
            fixture("01-melody.ksq").as_os_str(),
            "-o".as_ref(),
            output.as_os_str(),
        ],
    )
}

/// 디렉터리에 남은 임시 파일(`.이름.tmp-…`).
fn temp_leftovers(dir: &Path) -> Vec<OsString> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n.as_bytes().windows(5).any(|w| w == b".tmp-"))
        .collect()
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

/// 트랙 "A" 하나, 클립 하나가 `end`에서 끝나고 노트 `notes`개가 클립 앞쪽에 띄엄띄엄 놓인 곡.
fn far_project(end: u64, notes: u64) -> Project {
    let length = 2_000_000;
    let mut clip = Clip::new(Tick::new(end - length), length).unwrap();
    for i in 0..notes {
        clip.insert_note(note(i * 1920, 960, 30 + u8::try_from(i % 60).unwrap()))
            .unwrap();
    }
    let mut t = Track::new("A", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    t.insert_clip(clip).unwrap();
    let mut p = Project::new();
    p.push_track(t).unwrap();
    p
}

/// 거부 메시지의 `--max-output <값>`에서 값을 꺼낸다.
fn hinted(stderr: &str) -> Option<u64> {
    let after = stderr.split("`--max-output ").nth(1)?;
    after
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

// --- 끝이 `/.`인 경로: 리뷰 5 지적 1과 같은 갈래 ---

/// 커널은 `/dev/null/.`을 ENOTDIR로 거부한다(`: > /dev/null/.`). `special_output`은 끝이 `/`인지만
/// 보고, `Path::file_name`·`parent`가 끝의 `.`을 버려 `/dev/null`로 다시 짠 뒤 쓰고 종료 0이다.
#[test]
fn ksq2txt_output_dev_null_with_trailing_dot_fails_not_a_directory() {
    let r = ksq2txt_o(Path::new("/dev/null/."));

    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert!(r.stderr.contains("Not a directory"), "{}", r.stderr);
}

/// 같은 까닭으로 `/dev/stdout/.`도 받아들여, 일반 파일로 돌린 표준 출력에 텍스트를 쓴다. 리뷰 5가
/// `/dev/stdout/`로 깨뜨린 것과 같은 결과다.
#[test]
fn ksq2txt_output_dev_stdout_with_trailing_dot_fails_and_writes_nothing() {
    let dir = temp_dir("dot-stdout");
    let redirected = dir.join("stdout.txt");
    let r = run_with(
        KSQ2TXT,
        &[
            fixture("01-melody.ksq").as_os_str(),
            "-o".as_ref(),
            "/dev/stdout/.".as_ref(),
        ],
        Stdio::from(fs::File::create(&redirected).unwrap()),
    );

    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert_eq!(fs::read(&redirected).unwrap().len(), 0);
    fs::remove_dir_all(&dir).unwrap();
}

/// 대상이 `/dev/null/.`인 링크. 링크를 따라간 경로가 `/`로 끝나지 않아 `Slash` 판정을 비켜 간다.
/// 리뷰 4 전(`6b17d05`) 코드는 이 링크를 ENOTDIR로 거부했다.
#[test]
fn ksq2txt_output_link_to_dev_null_with_trailing_dot_fails_not_a_directory() {
    let dir = temp_dir("dot-link");
    let link = dir.join("link");
    std::os::unix::fs::symlink("/dev/null/.", &link).unwrap();

    let r = ksq2txt_o(&link);

    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert!(r.stderr.contains("Not a directory"), "{}", r.stderr);
    fs::remove_dir_all(&dir).unwrap();
}

/// `txt2ksq`도 같은 `write_special`을 거쳐 `/dev/null/.`에 쓰고 종료 0이다.
#[test]
fn txt2ksq_output_dev_null_with_trailing_dot_fails_not_a_directory() {
    let dir = temp_dir("dot-txt2ksq");
    let txt = dir.join("in.txt");
    fs::write(&txt, melody_text()).unwrap();

    let r = run(
        TXT2KSQ,
        &[txt.as_os_str(), "-o".as_ref(), "/dev/null/.".as_ref()],
    );

    assert_eq!(r.code, Some(1), "{}", r.stderr);
    fs::remove_dir_all(&dir).unwrap();
}

// --- 끝이 `/`인 경로의 경계(못 깨뜨림) ---

/// 사슬 중간의 링크 대상이 `/`로 끝나면(`l3 -> …/l2/`, `l2 -> /dev/null` 또는 일반 파일) 커널은
/// ENOTDIR로 거부하고, 판정도 `Slash`로 가 같은 오류를 낸다. 일반 파일은 바뀌지 않는다.
#[test]
fn ksq2txt_output_link_chain_with_slash_in_middle_fails_not_a_directory() {
    let dir = temp_dir("chain-slash");
    let kept = dir.join("kept.txt");
    fs::write(&kept, "keep").unwrap();
    for (name, target) in [("dev", Path::new("/dev/null")), ("reg", kept.as_path())] {
        let middle = dir.join(format!("{name}-l2"));
        std::os::unix::fs::symlink(target, &middle).unwrap();
        let first = dir.join(format!("{name}-l3"));
        let mut slashed = OsString::from(format!("{name}-l2"));
        slashed.push("/");
        std::os::unix::fs::symlink(&slashed, &first).unwrap();

        let r = ksq2txt_o(&first);

        assert_eq!(r.code, Some(1), "{name}: {}", r.stderr);
        assert!(r.stderr.contains("Not a directory"), "{name}: {}", r.stderr);
    }
    assert_eq!(fs::read(&kept).unwrap(), b"keep");
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// `//`, `/dev/null//`, `/dev/null/./`, 디렉터리 뒤의 `/`, 없는 이름 뒤의 `/`, 디렉터리 자체는 모두
/// 커널 오류로 종료 1이고 아무것도 만들지 않는다.
#[test]
fn ksq2txt_output_slash_and_directory_paths_exit_1_without_leftovers() {
    let dir = temp_dir("slash-many");
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();
    let with_slash = |p: &Path| {
        let mut s = p.as_os_str().to_owned();
        s.push("/");
        PathBuf::from(s)
    };
    let cases = [
        (PathBuf::from("//"), "Is a directory"),
        (PathBuf::from("/dev/null//"), "Not a directory"),
        (PathBuf::from("/dev/null/./"), "Not a directory"),
        (with_slash(&sub), "Is a directory"),
        (with_slash(&dir.join("new")), "No such file"),
        (sub.clone(), "Is a directory"),
    ];
    for (path, expected) in &cases {
        let r = ksq2txt_o(path);

        assert_eq!(r.code, Some(1), "{}: {}", path.display(), r.stderr);
        assert!(
            r.stderr.contains(expected),
            "{}: {}",
            path.display(),
            r.stderr
        );
    }
    assert!(!dir.join("new").exists());
    assert_eq!(fs::read_dir(&sub).unwrap().count(), 0);
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// 일반 파일 `-o`는 두 바이너리 모두 여전히 원자적으로 바꾼다(새 inode, 옛 꼬리 없음, 임시 파일 없음).
#[test]
fn binaries_output_regular_file_still_replaced_atomically() {
    let dir = temp_dir("atomic");
    let txt_out = dir.join("out.txt");
    let ksq_out = dir.join("out.ksq");
    let long = "old-".repeat(10_000);
    fs::write(&txt_out, &long).unwrap();
    fs::write(&ksq_out, &long).unwrap();
    let (txt_inode, ksq_inode) = (
        fs::metadata(&txt_out).unwrap().ino(),
        fs::metadata(&ksq_out).unwrap().ino(),
    );

    let a = ksq2txt_o(&txt_out);
    let input = dir.join("in.txt");
    fs::write(&input, melody_text()).unwrap();
    let b = run(
        TXT2KSQ,
        &[input.as_os_str(), "-o".as_ref(), ksq_out.as_os_str()],
    );

    assert_eq!(a.code, Some(0), "{}", a.stderr);
    assert_eq!(b.code, Some(0), "{}", b.stderr);
    assert_eq!(fs::read_to_string(&txt_out).unwrap(), melody_text());
    assert_ne!(fs::metadata(&txt_out).unwrap().ino(), txt_inode);
    assert_ne!(fs::metadata(&ksq_out).unwrap().ino(), ksq_inode);
    assert!(ksq::read_file(&ksq_out).is_ok());
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// 리뷰 4·5에서 통과한 `/dev/` 판정이 그대로다. `/dev/null`은 종료 0, `/dev/stdout`은 일반 파일로 돌린
/// 표준 출력에 `-o` 없는 실행과 같은 바이트를 쓴다.
///
/// 결정 `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`로 기대를 바꿨다: `-o`는 없는 경로와
/// 일반 파일만 받는다. `/dev/null`·`/dev/stdout`은 종료 1이고 대상은 그대로다(돌린 파일은 비어 있다).
#[test]
fn ksq2txt_output_dev_paths_still_written_directly() {
    let dir = temp_dir("dev");
    let null = ksq2txt_o(Path::new("/dev/null"));
    let redirected = dir.join("stdout.txt");
    let stdout = run_with(
        KSQ2TXT,
        &[
            fixture("01-melody.ksq").as_os_str(),
            "-o".as_ref(),
            "/dev/stdout".as_ref(),
        ],
        Stdio::from(fs::File::create(&redirected).unwrap()),
    );

    assert_eq!(null.code, Some(1), "{}", null.stderr);
    assert_eq!(stdout.code, Some(1), "{}", stdout.stderr);
    assert!(!melody_text().is_empty());
    assert_eq!(fs::read_to_string(&redirected).unwrap(), "");
    fs::remove_dir_all(&dir).unwrap();
}

// --- u64 경계 ---

/// 노트 639개(성부 많아야 640개), 이름 "A"인 트랙 하나의 어림은 `277_568 + 3840 × W`다
/// (W = 클립 끝 / 3840 + 1). `W = 4_803_839_602_528_456`이면 `u64::MAX − 3007`, 하나 더 크면
/// `u64::MAX + 833`이다.
const NOTES: u64 = 639;
const W_BELOW: u64 = 4_803_839_602_528_456;
const BELOW: u64 = u64::MAX - 3007;

fn end_for(wholes: u64) -> u64 {
    (wholes - 1) * 3840
}

/// u64 바로 아래: 어림을 자르지 않고 정확히 안내한다. 안내값보다 1 작은 상한은 같은 어림으로
/// 거부된다(안내값이 통과하는 가장 작은 상한이다). 안내값 자체로는 실행하지 않는다(거대한 텍스트를
/// 실제로 만들려 한다).
#[test]
fn ksq2txt_estimate_just_below_u64_is_reported_exactly() {
    let p = far_project(end_for(W_BELOW), NOTES);

    for limit in [1_000_000, BELOW - 1] {
        let err = project_to_text_limited(&p, limit).unwrap_err();

        let CliError::OutputTooLarge {
            estimated,
            limit: l,
            ..
        } = err
        else {
            panic!("{err}");
        };
        assert_eq!(estimated, BELOW);
        assert_eq!(l, limit);
    }
}

/// u64 바로 위: 상한이 `u64::MAX`여도 「어떤 상한으로도 변환할 수 없다」다.
#[test]
fn ksq2txt_estimate_just_above_u64_is_unconvertible_even_at_max_limit() {
    let p = far_project(end_for(W_BELOW + 1), NOTES);

    for limit in [1_000_000, u64::MAX] {
        let err = project_to_text_limited(&p, limit).unwrap_err();

        assert!(
            matches!(err, CliError::OutputBeyondAnyLimit { .. }),
            "{err}"
        );
    }
}

/// 바이너리 메시지: 아래쪽은 「어림 X가 상한 Y를 넘는다」(X > Y)와 `--max-output X`를 안내하고,
/// 위쪽은 안내도 「상한 … 넘는다」도 없이 변환 불가만 알린다. 둘 다 표준 출력은 비어 있다.
#[test]
fn ksq2txt_messages_around_u64_are_consistent() {
    let dir = temp_dir("u64-msg");
    let below = dir.join("below.ksq");
    let above = dir.join("above.ksq");
    let bytes = |w| ksq::to_bytes(&far_project(end_for(w), NOTES), Compression::None).unwrap();
    fs::write(&below, bytes(W_BELOW)).unwrap();
    fs::write(&above, bytes(W_BELOW + 1)).unwrap();

    let b = run(KSQ2TXT, &[below.as_os_str()]);
    let a = run(
        KSQ2TXT,
        &[
            above.as_os_str(),
            "--max-output".as_ref(),
            u64::MAX.to_string().as_ref(),
        ],
    );

    assert_eq!(b.code, Some(1), "{}", b.stderr);
    assert!(b.stdout.is_empty());
    assert_eq!(hinted(&b.stderr), Some(BELOW), "{}", b.stderr);
    assert!(
        b.stderr
            .contains(&format!("어림(위쪽 한계) {BELOW}바이트가 상한")),
        "{}",
        b.stderr
    );
    assert_eq!(a.code, Some(1), "{}", a.stderr);
    assert!(a.stdout.is_empty());
    assert_eq!(hinted(&a.stderr), None, "{}", a.stderr);
    assert!(a.stderr.contains("어떤 상한으로도"), "{}", a.stderr);
    assert!(!a.stderr.contains("바이트가 상한"), "{}", a.stderr);
    fs::remove_dir_all(&dir).unwrap();
}
