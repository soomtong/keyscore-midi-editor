//! S0-T8 적대적 리뷰 8회차(`docs/reports/s0-t8/review-8.md`)가 남긴 테스트. 범위는 리뷰 7 수정 세
//! 커밋(`546a776` 열기 실패 갈래, `c53641d` 열기 전 `/dev/` 분류와 신원 비교, `1b88fc7` 리뷰 5 테스트의
//! 링크 대상)이다.
//!
//! - 실패하는 테스트: 커널의 심볼릭 링크 한도(macOS 32)를 넘고 `resolve_symlink`의 한도(40)는 넘지 않는
//!   사슬. 쓰기 열기는 ELOOP로 실패하고 `fs::metadata`도 실패하므로 「경로가 없다」로 보고 원자적 쓰기로
//!   가며, `resolve_symlink`가 사슬 끝의 FIFO·소켓을 찾아 이름 바꾸기로 일반 파일로 바꾼다(종료 0).
//! - 선택 실행 테스트(`KSQ_R8_RAMDISK=1`): 두 RAM 디스크의 같은 inode 번호. `/dev/fd/N` 신원은 inode만
//!   비교하므로, 분류는 `/dev/stdout`을 보고 열기는 다른 볼륨의 같은 inode 일반 파일을 보면 자르지 않고
//!   써 옛 꼬리가 남는다. 디스크를 붙이는 테스트라 기본 실행에서는 건너뛴다.
//! - 통과하는 테스트: 열기 실패 갈래의 대상 종류별 종료 코드·남은 파일·원래 대상, 그리고 `/dev/fd/N`·
//!   `/dev/stdout`이 일반 파일·파이프를 가리키는 경우.
//!
//! 바이너리를 띄우는 테스트는 시간 제한을 둔다. 어떤 테스트도 거대한 출력을 만들지 않는다.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use keyscore_core::ksq;
use ksq_cli::project_to_text;

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const TIMEOUT: Duration = Duration::from_secs(30);

/// macOS 커널 한도(`MAXSYMLINKS` 32)보다 길고 `resolve_symlink`의 한도(40)보다 짧은 사슬 길이.
const OVER_KERNEL_LINKS: usize = 35;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-r8-{}-{name}", std::process::id()));
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

/// root는 권한 검사를 건너뛰어 권한 테스트의 전제가 서지 않는다.
fn is_root(dir: &Path) -> bool {
    fs::metadata(dir).unwrap().uid() == 0
}

fn mkfifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success(), "mkfifo 실패");
}

/// `dir` 안에 `target`을 가리키는 링크 `n`개 사슬을 만들고 맨 앞 링크를 돌려준다.
fn link_chain(dir: &Path, target: &Path, n: usize) -> PathBuf {
    let mut prev = target.to_path_buf();
    for i in 0..n {
        let link = dir.join(format!("l{i}"));
        std::os::unix::fs::symlink(&prev, &link).unwrap();
        prev = link;
    }
    prev
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
        std::thread::sleep(Duration::from_millis(5));
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

fn ksq2txt_o_to(output: &Path, stdout: Stdio) -> Ran {
    run_io(
        KSQ2TXT,
        &[
            fixture("01-melody.ksq").as_os_str(),
            "-o".as_ref(),
            output.as_os_str(),
        ],
        Stdio::null(),
        stdout,
    )
}

fn ksq2txt_o(output: &Path) -> Ran {
    ksq2txt_o_to(output, Stdio::piped())
}

fn txt2ksq_o(input: &Path, output: &Path) -> Ran {
    run_io(
        TXT2KSQ,
        &[input.as_os_str(), "-o".as_ref(), output.as_os_str()],
        Stdio::null(),
        Stdio::piped(),
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

// --- 깨짐: 커널 한도를 넘는 링크 사슬은 「경로 없음」으로 분류된다 ---

/// 링크 35개 사슬 끝의 쓰기 가능한 FIFO. 커널은 쓰기 열기를 ELOOP로 거부하고 `fs::metadata`도 같은
/// 오류라 `write_special`은 「경로가 없다」로 보고 원자적 쓰기로 넘긴다. `resolve_symlink`(한도 40)는
/// 사슬 끝의 FIFO를 찾아 이름 바꾸기로 일반 파일로 바꾸고 종료 0이다. `546a776`이 약속한 「일반 파일이
/// 아닌 대상은 열기 오류로 실패한다」가 깨진다.
#[test]
fn ksq2txt_output_link_chain_over_kernel_limit_to_fifo_fails_and_keeps_fifo() {
    let dir = temp_dir("chain-fifo");
    let fifo = dir.join("ff");
    mkfifo(&fifo);
    let head = link_chain(&dir, &fifo, OVER_KERNEL_LINKS);

    let r = ksq2txt_o(&head);

    let kind = fs::symlink_metadata(&fifo).unwrap().file_type();
    assert!(kind.is_fifo(), "FIFO가 다른 종류로 바뀌었다: {kind:?}");
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// 같은 사슬 끝의 유닉스 소켓. `txt2ksq`는 `ksq::write_file`(역시 한도 40)로 소켓을 일반 파일로 바꾼다.
#[test]
fn txt2ksq_output_link_chain_over_kernel_limit_to_socket_fails_and_keeps_socket() {
    let dir = temp_dir("chain-sock");
    let input = dir.join("in.txt");
    fs::write(&input, melody_text()).unwrap();
    let sock = dir.join("s");
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    let head = link_chain(&dir, &sock, OVER_KERNEL_LINKS);

    let r = txt2ksq_o(&input, &head);

    let kind = fs::symlink_metadata(&sock).unwrap().file_type();
    drop(listener);
    assert!(kind.is_socket(), "소켓이 다른 종류로 바뀌었다: {kind:?}");
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: 열기 실패 갈래가 지키는 것 ---

/// 대상 종류별로 종료 코드, 남은 임시 파일, 원래 대상을 본다. 커널 한도 안의 경로는 모두 문서대로다.
#[test]
fn ksq2txt_output_open_failure_kinds_follow_documented_exit_codes() {
    let dir = temp_dir("kinds");
    let root = is_root(&dir);
    let text = melody_text();

    // 없는 경로는 만든다.
    let new = dir.join("new.txt");
    let r = ksq2txt_o(&new);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert_eq!(fs::read_to_string(&new).unwrap(), text);

    // 끊긴 링크는 링크로 남고 대상 경로에 만든다.
    std::os::unix::fs::symlink("made.txt", dir.join("dangling")).unwrap();
    let r = ksq2txt_o(&dir.join("dangling"));
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(
        fs::symlink_metadata(dir.join("dangling"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(dir.join("made.txt")).unwrap(), text);

    // 없는 디렉터리 안을 가리키는 끊긴 링크, 순환 링크는 실패.
    std::os::unix::fs::symlink("nowhere/x", dir.join("dangling-dir")).unwrap();
    std::os::unix::fs::symlink("loop-b", dir.join("loop-a")).unwrap();
    std::os::unix::fs::symlink("loop-a", dir.join("loop-b")).unwrap();
    for p in ["dangling-dir", "loop-a"] {
        let r = ksq2txt_o(&dir.join(p));
        assert_eq!(r.code, Some(1), "{p}: {}", r.stderr);
    }
    assert!(!dir.join("nowhere").exists());

    // 디렉터리는 EISDIR로 실패하고 그대로다.
    fs::create_dir(dir.join("d")).unwrap();
    let r = ksq2txt_o(&dir.join("d"));
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert!(fs::metadata(dir.join("d")).unwrap().is_dir());

    // `/dev/` 아래 없는 이름을 가리키는 링크는 원자적 쓰기로 가지 않는다.
    std::os::unix::fs::symlink("/dev/ksq-r8-nonexistent", dir.join("dev-missing")).unwrap();
    let r = ksq2txt_o(&dir.join("dev-missing"));
    assert_eq!(r.code, Some(1), "{}", r.stderr);

    // 유닉스 소켓은 열기 오류로 실패하고 소켓으로 남는다.
    let sock = dir.join("s");
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    let r = ksq2txt_o(&sock);
    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert!(fs::symlink_metadata(&sock).unwrap().file_type().is_socket());
    drop(listener);

    if !root {
        // 읽기 전용 일반 파일은 이름 바꾸기로 바뀌고 권한을 지킨다.
        let ro = dir.join("ro.txt");
        fs::write(&ro, "old").unwrap();
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o444)).unwrap();
        let r = ksq2txt_o(&ro);
        assert_eq!(r.code, Some(0), "{}", r.stderr);
        assert_eq!(fs::read_to_string(&ro).unwrap(), text);
        assert_eq!(fs::metadata(&ro).unwrap().mode() & 0o777, 0o444);

        // 쓰기 권한 없는 FIFO는 EACCES로 실패하고 FIFO로 남는다.
        let fifo = dir.join("ff");
        mkfifo(&fifo);
        fs::set_permissions(&fifo, fs::Permissions::from_mode(0o444)).unwrap();
        let r = ksq2txt_o(&fifo);
        assert_eq!(r.code, Some(1), "{}", r.stderr);
        assert!(r.stderr.contains("os error 13"), "{}", r.stderr);
        assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());

        // 권한 없는 부모 안의 파일은 실패하고 그대로다.
        let locked = dir.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("f"), "keep").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let r = ksq2txt_o(&locked.join("f"));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(r.code, Some(1), "{}", r.stderr);
        assert_eq!(fs::read_to_string(locked.join("f")).unwrap(), "keep");
        assert!(temp_leftovers(&locked).is_empty());
    }

    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// `txt2ksq`도 같은 갈래를 탄다. 디렉터리·순환 링크·소켓은 실패하고, 없는 경로는 만든다.
#[test]
fn txt2ksq_output_open_failure_kinds_follow_documented_exit_codes() {
    let dir = temp_dir("kinds-txt2ksq");
    let input = dir.join("in.txt");
    fs::write(&input, melody_text()).unwrap();

    let r = txt2ksq_o(&input, &dir.join("new.ksq"));
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(ksq::read_file(&dir.join("new.ksq")).is_ok());

    fs::create_dir(dir.join("d.ksq")).unwrap();
    std::os::unix::fs::symlink("loop-b", dir.join("loop-a")).unwrap();
    std::os::unix::fs::symlink("loop-a", dir.join("loop-b")).unwrap();
    let sock = dir.join("s.ksq");
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    for p in ["d.ksq", "loop-a", "s.ksq"] {
        let r = txt2ksq_o(&input, &dir.join(p));
        assert_eq!(r.code, Some(1), "{p}: {}", r.stderr);
    }
    assert!(fs::metadata(dir.join("d.ksq")).unwrap().is_dir());
    assert!(fs::symlink_metadata(&sock).unwrap().file_type().is_socket());
    drop(listener);
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: 신원 비교 ---

/// 표준 출력을 일반 파일로 돌리고 `/dev/fd/1`, `/dev/stdout`, 그리고 그 둘을 가리키는 링크. 셸처럼
/// 자른 파일에 쓰므로 옛 꼬리가 없고 `-o` 없는 출력과 같다. 신원(inode만 비교)이 맞아 직접 쓴다.
#[test]
fn ksq2txt_output_dev_fd_to_redirected_regular_file_matches_identity() {
    let dir = temp_dir("fd-regular");
    let text = melody_text();
    std::os::unix::fs::symlink("/dev/stdout", dir.join("to-stdout")).unwrap();
    std::os::unix::fs::symlink("/dev/fd/1", dir.join("to-fd1")).unwrap();
    for (i, target) in [
        PathBuf::from("/dev/fd/1"),
        PathBuf::from("/dev/stdout"),
        dir.join("to-stdout"),
        dir.join("to-fd1"),
    ]
    .iter()
    .enumerate()
    {
        let sink = dir.join(format!("sink{i}"));
        fs::write(&sink, "OLD".repeat(200)).unwrap();
        let ino = fs::metadata(&sink).unwrap().ino();
        let r = ksq2txt_o_to(target, Stdio::from(fs::File::create(&sink).unwrap()));
        assert_eq!(r.code, Some(0), "{}: {}", target.display(), r.stderr);
        assert_eq!(
            fs::read_to_string(&sink).unwrap(),
            text,
            "{}",
            target.display()
        );
        assert_eq!(
            fs::metadata(&sink).unwrap().ino(),
            ino,
            "직접 쓰기여야 한다"
        );
    }
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// 표준 출력이 파이프면 `/dev/stdout`·`/dev/fd/1`은 FIFO 핸들이라 신원과 상관없이 직접 쓴다.
#[test]
fn ksq2txt_output_dev_stdout_to_pipe_writes_pipe() {
    let text = melody_text();
    for target in ["/dev/stdout", "/dev/fd/1"] {
        let r = ksq2txt_o(Path::new(target));
        assert_eq!(r.code, Some(0), "{target}: {}", r.stderr);
        assert_eq!(String::from_utf8(r.stdout).unwrap(), text, "{target}");
    }
}

// --- 선택 실행: 다른 볼륨의 같은 inode ---

/// `hdiutil`로 붙인 RAM 디스크. 끝나면 떼어 낸다.
struct RamDisk {
    device: String,
    mount: PathBuf,
}

impl RamDisk {
    fn new(name: &str) -> Self {
        let out = Command::new("hdiutil")
            .args(["attach", "-nomount", "ram://4096"])
            .output()
            .unwrap();
        assert!(out.status.success(), "hdiutil attach 실패");
        let device = String::from_utf8(out.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();
        let status = Command::new("diskutil")
            .args(["erasevolume", "HFS+", name, &device])
            .stdout(Stdio::null())
            .status()
            .unwrap();
        let disk = Self {
            device,
            mount: Path::new("/Volumes").join(name),
        };
        assert!(
            status.success() && disk.mount.is_dir(),
            "RAM 디스크 준비 실패"
        );
        disk
    }
}

impl Drop for RamDisk {
    fn drop(&mut self) {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force", &self.device])
            .stdout(Stdio::null())
            .status();
    }
}

/// 링크 `l`을 `/dev/stdout`과 다른 볼륨의 `Y` 사이에서 계속 바꾼다. 표준 출력은 볼륨 A의 `X`로 돌리고,
/// `Y`는 볼륨 B에서 `X`와 같은 inode 번호를 갖는다. 분류가 `/dev/stdout`(신원: 장치 번호 없이 inode
/// `X`)을 보고 열기가 `Y`를 보면 `Identity::matches`가 참이라 `Y`를 자르지 않고 써 옛 꼬리가 남는다.
/// `Y`의 디렉터리는 읽기 전용이라 원자적 쓰기는 실패하고 `Y`를 바꾸지 않는다. 그래서 `Y`의 머리가
/// 바뀌면 직접 쓰기가 일어난 것이다. 리뷰어 실행에서는 500회 중 74회 깨졌다(같은 볼륨 대조군은 0회).
///
/// 디스크를 붙이므로 `KSQ_R8_RAMDISK=1`일 때만 돈다.
#[test]
fn ksq2txt_output_link_raced_to_same_inode_on_other_volume_keeps_file_unchanged() {
    if std::env::var_os("KSQ_R8_RAMDISK").is_none_or(|v| v != "1") {
        eprintln!("KSQ_R8_RAMDISK=1이 아니라 건너뛴다");
        return;
    }
    let pid = std::process::id();
    let disk_a = RamDisk::new(&format!("ksqr8a{pid}"));
    let disk_b = RamDisk::new(&format!("ksqr8b{pid}"));
    let ro = disk_b.mount.join("ro");
    fs::create_dir(&ro).unwrap();
    let y = ro.join("Y");
    let old = "OLD".repeat(20_000);
    fs::write(&y, &old).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
    let y_ino = fs::metadata(&y).unwrap().ino();
    // HFS+ 노드 번호는 볼륨마다 1씩 늘어난다. 볼륨 A에 파일을 만들어 같은 번호에 맞춘다.
    let x_file = (0..64)
        .map(|i| {
            let p = disk_a.mount.join(format!("x{i}"));
            fs::write(&p, "").unwrap();
            p
        })
        .find(|p| fs::metadata(p).unwrap().ino() == y_ino)
        .expect("같은 inode 번호를 만들지 못했다");

    let dir = temp_dir("race");
    let link = dir.join("l");
    std::os::unix::fs::symlink("/dev/stdout", &link).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (stop, dir, link, y) = (stop.clone(), dir.clone(), link.clone(), y.clone());
        std::thread::spawn(move || {
            let tmp = dir.join(".lt");
            let mut i = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let target = if i.is_multiple_of(2) {
                    Path::new("/dev/stdout")
                } else {
                    y.as_path()
                };
                let _ = fs::remove_file(&tmp);
                std::os::unix::fs::symlink(target, &tmp).unwrap();
                fs::rename(&tmp, &link).unwrap();
                i += 1;
            }
        })
    };

    let deadline = Instant::now() + Duration::from_mins(2);
    let mut broken = None;
    for run in 0..500 {
        if Instant::now() > deadline {
            break;
        }
        let r = ksq2txt_o_to(&link, Stdio::from(fs::File::create(&x_file).unwrap()));
        if !fs::read_to_string(&y).unwrap().starts_with("OLD") {
            broken = Some((run, r.code));
            break;
        }
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().unwrap();
    let mixed = fs::read_to_string(&y).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    drop((disk_a, disk_b));

    assert!(
        broken.is_none(),
        "{broken:?}: 다른 볼륨의 같은 inode 파일에 직접 썼다. 끝 {:?}",
        &mixed[mixed.len().saturating_sub(12)..]
    );
}
