//! S0-T8 적대적 리뷰 7회차(`docs/reports/s0-t8/review-7.md`)가 남긴 테스트. 범위는 리뷰 6 수정 두
//! 커밋(`-o`를 언제나 원래 경로로 자르기 없이 열고 연 핸들의 종류로 고르기, `--max-output` 안내 문구)이다.
//!
//! - 실패하는 테스트: 원래 경로를 쓰기로 여는 데 실패하면 오류 종류와 상관없이 원자적 쓰기로 넘긴다
//!   (`Err(_) => Ok(false)`). 그래서 쓰기 권한 없는 FIFO·유닉스 소켓을 이름 바꾸기로 일반 파일로 바꾸고
//!   종료 0이며(리뷰 2 지적 3의 회귀. 리뷰 6 이전 코드는 열기 오류로 종료 1), 읽기 전용으로 열린
//!   `/dev/stdin`의 EACCES를 `/dev/fd`의 임시 파일 ENOENT로 바꿔 알린다.
//! - 통과하는 테스트: 읽는 쪽이 있는 FIFO, 일반 파일·링크의 원자적 쓰기(옛 꼬리 없음), 읽기 전용 파일과
//!   읽기 전용 디렉터리, `/dev/stdout`·`/dev/fd/1`·링크, 끝 `/`·`/.`, `/dev/..`, `txt2ksq`의 두 번
//!   직렬화가 같은 바이트를 낸다는 것.
//!
//! 바이너리를 띄우는 테스트는 시간 제한을 둔다. 어떤 테스트도 거대한 출력을 만들지 않는다.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use keyscore_core::ksq;
use ksq_cli::project_to_text;

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const TIMEOUT: Duration = Duration::from_secs(30);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-r7-{}-{name}", std::process::id()));
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

/// root는 권한 검사를 건너뛰어 쓰기 권한 없는 FIFO도 열린다(읽는 쪽이 없어 막힌다). 그런 환경에서는
/// 권한 테스트의 전제가 서지 않는다.
fn is_root(dir: &Path) -> bool {
    fs::metadata(dir).unwrap().uid() == 0
}

fn mkfifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success(), "mkfifo 실패");
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

fn run_io(bin: &str, args: &[&OsStr], stdin: Stdio, stdout: Stdio) -> Ran {
    let child = Command::new(bin)
        .args(args)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait(child, bin)
}

fn run_with(bin: &str, args: &[&OsStr], stdout: Stdio) -> Ran {
    run_io(bin, args, Stdio::null(), stdout)
}

fn ksq2txt_o(output: &Path) -> Ran {
    ksq2txt_o_to(output, Stdio::piped())
}

fn ksq2txt_o_to(output: &Path, stdout: Stdio) -> Ran {
    run_with(
        KSQ2TXT,
        &[
            fixture("01-melody.ksq").as_os_str(),
            "-o".as_ref(),
            output.as_os_str(),
        ],
        stdout,
    )
}

fn txt2ksq_o_to(input: &Path, output: &Path, stdout: Stdio) -> Ran {
    run_with(
        TXT2KSQ,
        &[input.as_os_str(), "-o".as_ref(), output.as_os_str()],
        stdout,
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

// --- 열기 실패를 모두 「원자적 쓰기로」 넘김: 특수 파일이 일반 파일로 바뀐다 ---

/// 쓰기 권한 없는(0444) FIFO. 쓰기로 열면 EACCES다. `write_special`은 이 오류를 버리고 원자적 쓰기로
/// 가서 이름 바꾸기로 FIFO를 일반 파일로 바꾼다(종료 0). 리뷰 6 이전 코드(`724717a`)는 `Special::Other`로
/// 연 오류를 그대로 내 종료 1이고 FIFO가 남았다. 리뷰 2 지적 3(FIFO가 일반 파일로 바뀜)의 회귀다.
#[test]
fn ksq2txt_output_fifo_without_write_permission_fails_and_keeps_fifo() {
    let dir = temp_dir("fifo-ro");
    if is_root(&dir) {
        fs::remove_dir_all(&dir).unwrap();
        return;
    }
    let fifo = dir.join("ff");
    mkfifo(&fifo);
    fs::set_permissions(&fifo, fs::Permissions::from_mode(0o444)).unwrap();

    let r = ksq2txt_o(&fifo);

    let kind = fs::symlink_metadata(&fifo).unwrap().file_type();
    assert!(kind.is_fifo(), "FIFO가 다른 종류로 바뀌었다: {kind:?}");
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// `txt2ksq`도 같은 `write_special`을 거쳐 `ksq::write_file`로 FIFO를 일반 파일로 바꾼다.
#[test]
fn txt2ksq_output_fifo_without_write_permission_fails_and_keeps_fifo() {
    let dir = temp_dir("fifo-ro-txt2ksq");
    if is_root(&dir) {
        fs::remove_dir_all(&dir).unwrap();
        return;
    }
    let input = dir.join("in.txt");
    fs::write(&input, melody_text()).unwrap();
    let fifo = dir.join("ff");
    mkfifo(&fifo);
    fs::set_permissions(&fifo, fs::Permissions::from_mode(0o444)).unwrap();

    let r = txt2ksq_o_to(&input, &fifo, Stdio::piped());

    let kind = fs::symlink_metadata(&fifo).unwrap().file_type();
    assert!(kind.is_fifo(), "FIFO가 다른 종류로 바뀌었다: {kind:?}");
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    fs::remove_dir_all(&dir).unwrap();
}

/// 유닉스 소켓은 쓰기로 열 수 없다(macOS EOPNOTSUPP). 오류를 버리고 원자적 쓰기로 가서 소켓을 일반
/// 파일로 바꾼다(종료 0). 리뷰 6 이전 코드는 소켓을 `Special::Other`로 보고 열기 오류로 종료 1이었다.
#[test]
fn ksq2txt_output_unix_socket_fails_and_keeps_socket() {
    let dir = temp_dir("sock");
    let sock = dir.join("s");
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

    let r = ksq2txt_o(&sock);

    let kind = fs::symlink_metadata(&sock).unwrap().file_type();
    drop(listener);
    assert!(kind.is_socket(), "소켓이 다른 종류로 바뀌었다: {kind:?}");
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// 표준 입력이 읽기 전용으로 연 일반 파일일 때 `-o /dev/stdin`. 쓰기로 열면 EACCES다. 리뷰 6 이전에는
/// 그 오류(os error 13)로 실패했다. 지금은 오류를 버리고 원자적 쓰기가 `/dev/fd`에 임시 파일을 만들려다
/// 낸 ENOENT(os error 2)를 알린다. 종료 1이고 파일은 그대로지만 사용자가 받는 원인이 틀리다.
#[test]
fn ksq2txt_output_read_only_dev_stdin_reports_permission_denied() {
    let dir = temp_dir("stdin-ro");
    if is_root(&dir) {
        fs::remove_dir_all(&dir).unwrap();
        return;
    }
    let file = dir.join("in.txt");
    fs::write(&file, "keep").unwrap();

    let r = run_io(
        KSQ2TXT,
        &[
            fixture("01-melody.ksq").as_os_str(),
            "-o".as_ref(),
            "/dev/stdin".as_ref(),
        ],
        Stdio::from(fs::File::open(&file).unwrap()),
        Stdio::piped(),
    );

    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert_eq!(fs::read_to_string(&file).unwrap(), "keep");
    assert!(r.stderr.contains("os error 13"), "{}", r.stderr);
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: 리뷰 6 수정이 지킨 것 ---

/// 읽는 쪽이 있는 FIFO는 그대로 열어 쓰고 FIFO로 남는다. 읽는 쪽(`cat`)도 시간 제한 안에서 기다린다.
#[test]
fn ksq2txt_output_fifo_with_reader_written_directly_and_stays_fifo() {
    let dir = temp_dir("fifo");
    let fifo = dir.join("ff");
    mkfifo(&fifo);
    let reader = Command::new("cat")
        .arg(&fifo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let r = ksq2txt_o(&fifo);
    let read = wait(reader, "cat");

    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert_eq!(String::from_utf8(read.stdout).unwrap(), melody_text());
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    fs::remove_dir_all(&dir).unwrap();
}

/// 일반 파일과 일반 파일을 가리키는 링크: 원래 경로를 자르기 없이 열었다 닫아도 결과는 원자적 쓰기다
/// (새 inode, 옛 꼬리 없음, 링크는 링크로 남음, 임시 파일 없음). 두 바이너리 모두.
#[test]
fn binaries_output_regular_file_and_link_replaced_without_old_tail() {
    let dir = temp_dir("regular");
    let long = "old-".repeat(10_000);
    let txt = dir.join("out.txt");
    let ksq_out = dir.join("out.ksq");
    let target = dir.join("target.txt");
    let link = dir.join("link.txt");
    for p in [&txt, &ksq_out, &target] {
        fs::write(p, &long).unwrap();
    }
    std::os::unix::fs::symlink("target.txt", &link).unwrap();
    let inodes: Vec<u64> = [&txt, &ksq_out, &target]
        .iter()
        .map(|p| fs::metadata(p).unwrap().ino())
        .collect();
    let input = dir.join("in.txt");
    fs::write(&input, melody_text()).unwrap();

    let a = ksq2txt_o(&txt);
    let b = ksq2txt_o(&link);
    let c = txt2ksq_o_to(&input, &ksq_out, Stdio::piped());

    for r in [&a, &b, &c] {
        assert_eq!(r.code, Some(0), "{}", r.stderr);
    }
    assert_eq!(fs::read_to_string(&txt).unwrap(), melody_text());
    assert_eq!(fs::read_to_string(&target).unwrap(), melody_text());
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(ksq::read_file(&ksq_out).is_ok());
    let after: Vec<u64> = [&txt, &ksq_out, &target]
        .iter()
        .map(|p| fs::metadata(p).unwrap().ino())
        .collect();
    for (before, after) in inodes.iter().zip(&after) {
        assert_ne!(before, after);
    }
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// 읽기 전용(0444) 일반 파일: 쓰기 열기는 EACCES지만 원자적 쓰기로 바꾸고 권한을 지킨다(README대로).
/// 쓰기 가능한 파일이 읽기 전용 디렉터리 안에 있으면 열기는 되지만 원자적 쓰기에서 실패하고 파일은
/// 그대로다(옛 내용, 옛 길이).
#[test]
fn ksq2txt_output_read_only_file_and_read_only_dir_behave_as_documented() {
    let dir = temp_dir("perm");
    if is_root(&dir) {
        fs::remove_dir_all(&dir).unwrap();
        return;
    }
    let ro = dir.join("ro.txt");
    fs::write(&ro, "old-".repeat(1000)).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o444)).unwrap();
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();
    let inner = sub.join("w.txt");
    fs::write(&inner, "keep").unwrap();
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o555)).unwrap();

    let a = ksq2txt_o(&ro);
    let b = ksq2txt_o(&inner);
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(a.code, Some(0), "{}", a.stderr);
    assert_eq!(fs::read_to_string(&ro).unwrap(), melody_text());
    assert_eq!(
        fs::metadata(&ro).unwrap().permissions().mode() & 0o777,
        0o444
    );
    assert_eq!(b.code, Some(1), "{}", b.stderr);
    assert_eq!(fs::read_to_string(&inner).unwrap(), "keep");
    assert!(temp_leftovers(&dir).is_empty());
    assert!(temp_leftovers(&sub).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// `/dev/stdout`, `/dev/fd/1`, `/dev/stdout`을 가리키는 링크: 일반 파일로 돌린 표준 출력에 `-o` 없는
/// 실행과 같은 바이트를 쓴다. `/dev/null`은 종료 0.
#[test]
fn ksq2txt_output_dev_paths_and_links_write_redirected_stdout() {
    let dir = temp_dir("dev");
    let link = dir.join("to-stdout");
    std::os::unix::fs::symlink("/dev/stdout", &link).unwrap();
    let expected = melody_text();

    let null = ksq2txt_o(Path::new("/dev/null"));
    assert_eq!(null.code, Some(0), "{}", null.stderr);
    for (i, out) in [Path::new("/dev/stdout"), Path::new("/dev/fd/1"), &link]
        .into_iter()
        .enumerate()
    {
        let redirected = dir.join(format!("r{i}.txt"));
        let r = ksq2txt_o_to(out, Stdio::from(fs::File::create(&redirected).unwrap()));
        assert_eq!(r.code, Some(0), "{}: {}", out.display(), r.stderr);
        assert_eq!(
            fs::read_to_string(&redirected).unwrap(),
            expected,
            "{}",
            out.display()
        );
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// 끝이 `/`·`/.`인 경로는 커널 오류(ENOTDIR)로 실패하고 아무것도 쓰지 않는다. 리뷰 5·6의 사례를 한데
/// 확인한다: `/dev/null/`, `/dev/null/.`, `/dev/stdout/.`(표준 출력을 일반 파일로 돌림), 일반 파일 뒤
/// `/.`, 대상이 `/dev/null/.`인 링크, FIFO 뒤 `/.`(읽는 쪽이 없어도 열기 전에 실패해 막히지 않는다).
#[test]
fn ksq2txt_output_trailing_slash_or_dot_fails_not_a_directory() {
    let dir = temp_dir("slashdot");
    let regular = dir.join("out.txt");
    fs::write(&regular, "keep").unwrap();
    let link = dir.join("l");
    std::os::unix::fs::symlink("/dev/null/.", &link).unwrap();
    let fifo = dir.join("ff");
    mkfifo(&fifo);
    let with = |p: &Path, tail: &str| {
        let mut s = p.as_os_str().to_owned();
        s.push(tail);
        PathBuf::from(s)
    };

    for out in [
        PathBuf::from("/dev/null/"),
        PathBuf::from("/dev/null/."),
        with(&regular, "/."),
        link.clone(),
        with(&fifo, "/."),
    ] {
        let r = ksq2txt_o(&out);
        assert_eq!(r.code, Some(1), "{}: {}", out.display(), r.stderr);
        assert!(
            r.stderr.contains("os error 20"),
            "{}: {}",
            out.display(),
            r.stderr
        );
    }
    let redirected = dir.join("stdout.txt");
    let r = ksq2txt_o_to(
        Path::new("/dev/stdout/."),
        Stdio::from(fs::File::create(&redirected).unwrap()),
    );
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert_eq!(fs::read(&redirected).unwrap(), b"");
    assert_eq!(fs::read_to_string(&regular).unwrap(), "keep");
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// `/dev/..`로 시작하는 일반 파일 경로는 `/dev/` 아래로 분류되지 않아, 원래 경로로 연 일반 파일을
/// 그대로 쓰지 않고 원자적으로 바꾼다(새 inode, 옛 꼬리 없음).
#[test]
fn ksq2txt_output_dev_dotdot_regular_file_replaced_atomically() {
    let dir = temp_dir("dotdot");
    let out = dir.join("out.txt");
    fs::write(&out, "old-".repeat(10_000)).unwrap();
    let inode = fs::metadata(&out).unwrap().ino();
    let real = fs::canonicalize(&out).unwrap();
    let via_dev = Path::new("/dev/..").join(real.strip_prefix("/").unwrap());

    let r = ksq2txt_o(&via_dev);

    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert_eq!(fs::read_to_string(&out).unwrap(), melody_text());
    assert_ne!(fs::metadata(&out).unwrap().ino(), inode);
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// `txt2ksq`는 일반 파일이면 `to_bytes`로 한 번, `ksq::write_file`로 한 번 직렬화한다. 두 번째가 쓴
/// 파일과 첫 번째가 `/dev/stdout`으로 쓴 바이트가 같다(`.ksq`·`.ksqz` 모두). `.ksqz`는 다시 읽힌다.
#[test]
fn txt2ksq_double_serialization_gives_same_bytes_as_direct_write() {
    let dir = temp_dir("twice");
    let input = dir.join("in.txt");
    fs::write(&input, melody_text()).unwrap();

    for ext in ["ksq", "ksqz"] {
        let file = dir.join(format!("out.{ext}"));
        let a = txt2ksq_o_to(&input, &file, Stdio::piped());
        assert_eq!(a.code, Some(0), "{}", a.stderr);
        // `/dev/stdout`에는 확장자가 없어 압축 여부를 고르는 링크 이름으로 확장자를 준다.
        let link = dir.join(format!("to-stdout.{ext}"));
        std::os::unix::fs::symlink("/dev/stdout", &link).unwrap();
        let b = txt2ksq_o_to(&input, &link, Stdio::piped());
        assert_eq!(b.code, Some(0), "{}", b.stderr);
        assert_eq!(fs::read(&file).unwrap(), b.stdout, "{ext}");
        assert!(ksq::read_file(&file).is_ok(), "{ext}");
    }
    fs::remove_dir_all(&dir).unwrap();
}
