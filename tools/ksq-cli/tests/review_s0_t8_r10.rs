//! S0-T8 적대적 리뷰 10회차(`docs/reports/s0-t8/review-10.md`)가 남긴 테스트. 범위는 리뷰 9 대응
//! 네 커밋 `f5dceee`, `845a917`, `ba8047a`, `4cd5dad`뿐이다.
//!
//! - 실패하는 테스트: `845a917`이 입력을 `-o` 확인보다 먼저 열게 되면서, FIFO 입력에 쓰는 쪽이 없으면
//!   거부할 `-o`(장치, 디렉터리, 같은 FIFO)에서도 열기에서 멈춘다. `b0670c9`에서는 곧바로 종료 1이었다.
//! - 통과하는 테스트: 입력 핸들 `fstat`과 출력 `stat`을 비교하는 같은 파일 판정의 경계(`/dev/fd/N`,
//!   하드 링크, 심볼릭 링크 양쪽, firmlink 경로, `.ksqz`, 파이프 표준 입력), 디렉터리 입력, 새 오류
//!   `OutputUncreatable`의 안내.
//!
//! FIFO에는 읽는 쪽도 쓰는 쪽도 두지 않는다. 바이너리를 띄우는 테스트는 모두 시간 제한을 두고, 넘으면
//! 자식을 죽인 뒤 실패로 보고한다.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

/// 정상 실행의 시간 제한.
const TIMEOUT: Duration = Duration::from_secs(30);
/// 거부가 곧바로 나야 하는 실행의 시간 제한. 멈춤을 빨리 잡으려고 짧게 둔다.
const REFUSE_TIMEOUT: Duration = Duration::from_secs(5);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-r10-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

fn mkfifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success(), "mkfifo 실패");
}

/// 표준 오류·출력은 `dir` 안 파일로 받아 파이프가 차서 멈추는 일을 없앤다.
struct Ran {
    /// `None`이면 신호로 끝났다.
    code: Option<i32>,
    timed_out: bool,
    stderr: String,
}

/// `cmd`를 띄우고 `limit` 안에 끝나지 않으면 죽인다. 멈춤을 `timed_out`으로 돌려주어 테스트가
/// 이유를 적고 실패하게 한다.
fn run_cmd(mut cmd: Command, dir: &Path, limit: Duration) -> Ran {
    let err_path = dir.join(".stderr");
    let out_path = dir.join(".stdout");
    let mut child = cmd
        .stdout(fs::File::create(&out_path).unwrap())
        .stderr(fs::File::create(&err_path).unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + limit;
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break (status, false);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break (child.wait().unwrap(), true);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stderr = fs::read_to_string(&err_path).unwrap_or_default();
    let _ = fs::remove_file(&err_path);
    let _ = fs::remove_file(&out_path);
    Ran {
        code: status.code(),
        timed_out,
        stderr,
    }
}

fn run(bin: &str, args: &[&Path], dir: &Path) -> Ran {
    let mut cmd = Command::new(bin);
    cmd.args(args).stdin(Stdio::null());
    run_cmd(cmd, dir, TIMEOUT)
}

/// `dir` 안에서 점 파일(이 테스트의 표준 출력·오류 파일)을 빼고 이름을 정렬해 돌려준다.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| !n.starts_with('.'))
        .collect();
    v.sort();
    v
}

fn melody_text() -> Vec<u8> {
    let o = Command::new(KSQ2TXT)
        .arg(fixture("01-melody.ksq"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    o.stdout
}

/// FIFO 입력에 쓰는 쪽이 없을 때 거부할 `-o`는 입력을 열기 전에 곧바로 종료 1이어야 한다. `845a917`이
/// 입력을 먼저 열게 바꾸면서 열기(`open(O_RDONLY)`)에서 멈춘다. `b0670c9`에서는 다섯 경우 모두 1초 안에
/// 종료 1이었다. 쓰는 쪽이 있으면 도구가 열자마자 닫아 쓰는 쪽이 SIGPIPE로 죽는다(리뷰 10 지적 1).
#[test]
fn binaries_fifo_input_with_refused_output_exit_1_without_waiting_for_writer() {
    let dir = temp_dir("fifo-input");
    let fifo = dir.join("in.pipe");
    mkfifo(&fifo);
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();

    let cases: [(&str, &Path); 5] = [
        (KSQ2TXT, Path::new("/dev/null")),
        (KSQ2TXT, fifo.as_path()),
        (KSQ2TXT, sub.as_path()),
        (TXT2KSQ, fifo.as_path()),
        (TXT2KSQ, Path::new("/dev/null")),
    ];
    let mut hung = Vec::new();
    for (bin, target) in cases {
        let mut cmd = Command::new(bin);
        cmd.arg(&fifo).arg("-o").arg(target).stdin(Stdio::null());
        let r = run_cmd(cmd, &dir, REFUSE_TIMEOUT);
        if r.timed_out || r.code != Some(1) {
            hung.push(format!(
                "{bin} {} -o {}: 멈춤={} 종료={:?} {}",
                fifo.display(),
                target.display(),
                r.timed_out,
                r.code,
                r.stderr
            ));
        }
    }

    assert!(
        fs::symlink_metadata(&fifo)
            .is_ok_and(|m| std::os::unix::fs::FileTypeExt::is_fifo(&m.file_type()))
    );
    assert_eq!(names(&dir), ["in.pipe", "sub"]);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        hung.is_empty(),
        "거부가 입력 열기에서 멈췄다:\n{}",
        hung.join("\n")
    );
}

/// 입력을 `/dev/fd/3`으로 주고 fd 3을 출력 파일에서 열면 같은 파일이다. 연 핸들의 `fstat`이 실제
/// 파일의 (dev, ino)를 주므로 두 바이너리 모두 종료 1이고 파일은 그대로다.
#[test]
fn binaries_refuse_output_equal_to_input_given_as_dev_fd_3() {
    let dir = temp_dir("dev-fd-3");
    let ksq_file = dir.join("same.ksq");
    fs::copy(fixture("01-melody.ksq"), &ksq_file).unwrap();
    let txt_file = dir.join("same.txt");
    fs::write(&txt_file, melody_text()).unwrap();

    for (bin, file) in [(KSQ2TXT, &ksq_file), (TXT2KSQ, &txt_file)] {
        let before = fs::read(file).unwrap();
        // 안전한 Rust로는 자식의 fd 3을 정할 수 없어 셸의 `3<`를 빌린다.
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(r#"exec "$0" /dev/fd/3 -o "$1" 3<"$1""#)
            .arg(bin)
            .arg(file)
            .stdin(Stdio::null());
        let r = run_cmd(cmd, &dir, TIMEOUT);

        assert!(!r.timed_out, "{bin}");
        assert_eq!(r.code, Some(1), "{bin}: {}", r.stderr);
        assert!(r.stderr.contains("입력 파일과 같다"), "{bin}: {}", r.stderr);
        assert_eq!(fs::read(file).unwrap(), before, "{bin}");
    }
    assert_eq!(names(&dir), ["same.ksq", "same.txt"]);
    let _ = fs::remove_dir_all(&dir);
}

/// 하드 링크, 출력이 입력을 가리키는 심볼릭 링크, 입력이 출력을 가리키는 심볼릭 링크, `.ksqz` 입력,
/// firmlink(`/System/Volumes/Data` 경유) 경로 모두 같은 파일로 거부한다. 철자가 아니라 (dev, ino)로
/// 판정하기 때문이다.
#[test]
fn binaries_refuse_output_equal_to_input_through_links_and_firmlink() {
    let dir = temp_dir("links");
    let dir = fs::canonicalize(&dir).unwrap();
    let ksq_file = dir.join("a.ksq");
    fs::copy(fixture("01-melody.ksq"), &ksq_file).unwrap();
    let hard = dir.join("hard.ksq");
    fs::hard_link(&ksq_file, &hard).unwrap();
    let out_link = dir.join("out-link");
    std::os::unix::fs::symlink("a.ksq", &out_link).unwrap();
    let in_link = dir.join("in-link");
    std::os::unix::fs::symlink("a.ksq", &in_link).unwrap();
    let txt = dir.join("t.txt");
    fs::write(&txt, melody_text()).unwrap();
    let ksqz = dir.join("z.ksqz");
    let made = run(TXT2KSQ, &[&txt, Path::new("-o"), &ksqz], &dir);
    assert_eq!(made.code, Some(0), "{}", made.stderr);

    let mut cases: Vec<(PathBuf, PathBuf)> = vec![
        (ksq_file.clone(), hard.clone()),
        (ksq_file.clone(), out_link.clone()),
        (in_link.clone(), ksq_file.clone()),
        (ksqz.clone(), ksqz.clone()),
    ];
    // macOS의 /private/tmp 등은 /System/Volumes/Data 아래 같은 파일이다. 없는 환경이면 건너뛴다.
    let firm = Path::new("/System/Volumes/Data").join(ksq_file.strip_prefix("/").unwrap());
    if firm.exists() {
        cases.push((firm, ksq_file.clone()));
    }

    let a_before = fs::read(&ksq_file).unwrap();
    let z_before = fs::read(&ksqz).unwrap();
    for (input, output) in &cases {
        let r = run(KSQ2TXT, &[input, Path::new("-o"), output], &dir);

        assert!(!r.timed_out);
        assert_eq!(
            r.code,
            Some(1),
            "{} -o {}: {}",
            input.display(),
            output.display(),
            r.stderr
        );
        assert!(r.stderr.contains("입력 파일과 같다"), "{}", r.stderr);
    }
    assert_eq!(fs::read(&ksq_file).unwrap(), a_before);
    assert_eq!(fs::read(&ksqz).unwrap(), z_before);
    assert!(fs::symlink_metadata(&out_link).unwrap().is_symlink());
    assert!(fs::symlink_metadata(&in_link).unwrap().is_symlink());
    assert_eq!(
        names(&dir),
        [
            "a.ksq", "hard.ksq", "in-link", "out-link", "t.txt", "z.ksqz"
        ]
    );
    let _ = fs::remove_dir_all(&dir);
}

/// 표준 입력이 파이프면 `/dev/stdin`의 `fstat`은 파이프다. 있는 일반 파일 `-o`와 같다고 잘못 보지
/// 않고 종료 0으로 원자적으로 바꾼다.
#[test]
fn binaries_dev_stdin_pipe_input_writes_existing_regular_output() {
    let dir = temp_dir("pipe-stdin");
    let ksq_bytes = fs::read(fixture("01-melody.ksq")).unwrap();
    let text = melody_text();
    let out_txt = dir.join("out.txt");
    fs::write(&out_txt, "old").unwrap();
    let out_ksq = dir.join("out.ksq");
    fs::write(&out_ksq, "old").unwrap();

    for (bin, input, out) in [
        (KSQ2TXT, ksq_bytes.clone(), &out_txt),
        (TXT2KSQ, text.clone(), &out_ksq),
    ] {
        let mut child = Command::new(bin)
            .args([Path::new("/dev/stdin"), Path::new("-o"), out])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        {
            let mut stdin = child.stdin.take().unwrap();
            std::io::Write::write_all(&mut stdin, &input).unwrap();
        }
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{bin}: {TIMEOUT:?} 안에 끝나지 않았다");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(0), "{bin}");
    }
    assert_eq!(fs::read(&out_txt).unwrap(), text);
    assert!(
        fs::read_to_string(&out_ksq)
            .unwrap()
            .contains("format_version")
    );
    assert_eq!(names(&dir), ["out.ksq", "out.txt"]);
    let _ = fs::remove_dir_all(&dir);
}

/// 디렉터리 입력은 입력 경로의 EISDIR로 종료 1이고 `-o`는 만들지 않는다. 출력이 같은 디렉터리면
/// 같은 파일로 거부한다.
#[test]
fn binaries_directory_input_fails_without_creating_output() {
    let dir = temp_dir("dir-input");
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();

    for bin in [KSQ2TXT, TXT2KSQ] {
        let out = dir.join("out");
        let r = run(bin, &[&sub, Path::new("-o"), &out], &dir);
        assert_eq!(r.code, Some(1), "{bin}: {}", r.stderr);
        assert!(r.stderr.contains("Is a directory"), "{bin}: {}", r.stderr);
        assert!(r.stderr.contains("sub"), "{bin}: {}", r.stderr);

        let r = run(bin, &[&sub, Path::new("-o"), &sub], &dir);
        assert_eq!(r.code, Some(1), "{bin}: {}", r.stderr);
        assert!(r.stderr.contains("입력 파일과 같다"), "{bin}: {}", r.stderr);
    }
    assert_eq!(names(&dir), ["sub"]);
    let _ = fs::remove_dir_all(&dir);
}

/// 확인 때 없던 대상을 만들지 못하면(쓰기 권한 없는 디렉터리 안, 없는 디렉터리를 가리키는 끊긴 링크)
/// `OutputUncreatable`로 종료 1이다. 그 바이너리의 리다이렉션 예를 붙이고, 경로는 한 번만 적으며,
/// 아무것도 만들지 않는다. 있는 일반 파일의 쓰기 실패에는 안내를 붙이지 않는다.
#[test]
fn binaries_uncreatable_output_reports_once_with_own_example() {
    let dir = temp_dir("uncreatable");
    let ro = dir.join("ro");
    fs::create_dir(&ro).unwrap();
    let existing = ro.join("e.out");
    fs::write(&existing, "keep").unwrap();
    let dangling = dir.join("dl");
    std::os::unix::fs::symlink("nowhere/x", &dangling).unwrap();
    let ksq_in = fixture("01-melody.ksq");
    let txt_in = dir.join("in.txt");
    fs::write(&txt_in, melody_text()).unwrap();
    std::os::unix::fs::PermissionsExt::set_mode(&mut fs::metadata(&ro).unwrap().permissions(), 0);
    let mut perm = fs::metadata(&ro).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perm, 0o555);
    fs::set_permissions(&ro, perm).unwrap();
    // root는 권한 검사를 건너뛰어 전제가 서지 않는다.
    let root = std::os::unix::fs::MetadataExt::uid(&fs::metadata(&dir).unwrap()) == 0;

    for (bin, input, example) in [
        (KSQ2TXT, ksq_in.as_path(), "ksq2txt in.ksq > out.txt"),
        (TXT2KSQ, txt_in.as_path(), "txt2ksq in.txt > out.ksq"),
    ] {
        let mut targets = vec![dangling.clone()];
        if !root {
            targets.push(ro.join("new.out"));
        }
        for target in &targets {
            let r = run(bin, &[input, Path::new("-o"), target], &dir);
            assert_eq!(r.code, Some(1), "{bin} {}: {}", target.display(), r.stderr);
            assert!(r.stderr.contains("만들지 못했다"), "{}", r.stderr);
            assert!(r.stderr.contains(example), "{bin}: {}", r.stderr);
            assert_eq!(
                r.stderr.matches(&*target.to_string_lossy()).count(),
                1,
                "{}",
                r.stderr
            );
        }
        if !root {
            let r = run(bin, &[input, Path::new("-o"), &existing], &dir);
            assert_eq!(r.code, Some(1), "{bin}: {}", r.stderr);
            assert!(r.stderr.contains("Permission denied"), "{}", r.stderr);
            assert!(!r.stderr.contains("리다이렉트"), "{bin}: {}", r.stderr);
        }
    }

    let mut perm = fs::metadata(&ro).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perm, 0o755);
    fs::set_permissions(&ro, perm).unwrap();
    assert_eq!(fs::read(&existing).unwrap(), b"keep");
    assert_eq!(names(&ro), ["e.out"]);
    assert_eq!(names(&dir), ["dl", "in.txt", "ro"]);
    assert!(fs::symlink_metadata(&dangling).unwrap().is_symlink());
    let _ = fs::remove_dir_all(&dir);
}
