//! S0-T8 적대적 리뷰 9회차(`docs/reports/s0-t8/review-9.md`)가 남긴 테스트. 범위는 결정
//! `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`(A: `-o`는 없는 경로와 일반 파일만)를
//! 구현한 두 커밋 `8c5697e`(`guard_output`)와 `784f715`(리뷰 2–8 테스트 기대 변경)뿐이다.
//!
//! 결정 A를 깨뜨리지 못한 경계를 확인하는 통과 테스트만 있다. 리뷰에서 찾은 흔들림(리뷰 3 테스트가
//! FIFO 읽는 쪽 스레드에서 멈춤)은 확률적이라 결정적인 테스트로 남기지 못했고, 재현 방법은 리뷰 문서에
//! 적었다. 그래서 이 파일의 테스트는 FIFO에 읽는 쪽을 두지 않는다. 도구가 FIFO를 열면 쓰기 열기에서
//! 막히므로, 바이너리 시간 제한이 그것을 실패로 잡는다.
//!
//! 바이너리를 띄우는 테스트는 모두 시간 제한을 둔다.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const TIMEOUT: Duration = Duration::from_secs(30);

/// macOS 커널의 심볼릭 링크 한도(`MAXSYMLINKS`).
const KERNEL_LINKS: usize = 32;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-r9-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

/// root는 권한 검사를 건너뛰어 권한 테스트의 전제가 서지 않는다.
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

/// `bin`을 띄우고 `TIMEOUT` 안에 끝나지 않으면 죽이고 실패한다. 표준 출력·오류는 스레드로 비워
/// 파이프가 차서 멈추지 않게 한다. 표준 입력은 비운다.
fn run(bin: &str, args: &[&OsStr]) -> Ran {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut v = Vec::new();
        std::io::Read::read_to_end(&mut out, &mut v).unwrap();
        v
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut err, &mut s).unwrap();
        s
    });
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{bin} {args:?}: {TIMEOUT:?} 안에 끝나지 않았다(대상을 연 것으로 보인다)");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Ran {
        code: status.code(),
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

/// 두 바이너리를 같은 `-o`로 돌린다. `ksq2txt`는 `01-melody.ksq`, `txt2ksq`는 그 텍스트를 읽는다.
fn both_o(txt: &Path, output: &Path) -> [Ran; 2] {
    let ksq = fixture("01-melody.ksq");
    [
        run(
            KSQ2TXT,
            &[ksq.as_os_str(), "-o".as_ref(), output.as_os_str()],
        ),
        run(
            TXT2KSQ,
            &[txt.as_os_str(), "-o".as_ref(), output.as_os_str()],
        ),
    ]
}

/// `dir`에 `01-melody.ksq`의 텍스트를 `in.txt`로 쓴다.
fn melody_txt(dir: &Path) -> PathBuf {
    let txt = dir.join("in.txt");
    let ran = run(KSQ2TXT, &[fixture("01-melody.ksq").as_os_str()]);
    assert_eq!(ran.code, Some(0), "{}", ran.stderr);
    fs::write(&txt, ran.stdout).unwrap();
    txt
}

/// 디렉터리에 남은 임시 파일(`.이름.tmp-…`).
fn temp_leftovers(dir: &Path) -> Vec<OsString> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n.as_bytes().windows(5).any(|w| w == b".tmp-"))
        .collect()
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    v.sort();
    v
}

// --- 경계: 끊긴 링크는 core `write_file`과 같이 링크 대상에 만든다 ---

/// 결정: 「`NotFound`면 새로 만든다. 링크는 core `ksq::write_file`과 같이 대상 파일을 바꾼다」.
/// 대상이 없는 링크(두 단계 사슬 포함)는 링크를 일반 파일로 바꾸지 않고 링크 대상 경로에 만든다.
/// `ksq2txt`(이 crate의 `resolve_symlink`)와 `txt2ksq`(core)가 같다.
#[test]
fn binaries_output_dangling_link_creates_link_target_and_keeps_link() {
    let dir = temp_dir("dangling");
    let txt = melody_txt(&dir);
    let expected_txt = fs::read(&txt).unwrap();
    let expected_ksq = fs::read(fixture("01-melody.ksq")).unwrap();

    for (bin, ext, expected) in [
        (KSQ2TXT, "txt", &expected_txt),
        (TXT2KSQ, "ksq", &expected_ksq),
    ] {
        let target = dir.join(format!("made.{ext}"));
        let mid = dir.join(format!("mid.{ext}"));
        let link = dir.join(format!("link.{ext}"));
        std::os::unix::fs::symlink(format!("made.{ext}"), &mid).unwrap();
        std::os::unix::fs::symlink(&mid, &link).unwrap();
        let input = if bin == KSQ2TXT {
            fixture("01-melody.ksq")
        } else {
            txt.clone()
        };

        let r = run(bin, &[input.as_os_str(), "-o".as_ref(), link.as_os_str()]);

        assert_eq!(r.code, Some(0), "{bin}: {}", r.stderr);
        assert_eq!(&fs::read(&target).unwrap(), expected, "{bin}");
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink(), "{bin}");
        assert!(fs::symlink_metadata(&mid).unwrap().is_symlink(), "{bin}");
        assert!(fs::symlink_metadata(&target).unwrap().is_file(), "{bin}");
    }
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: 링크 사슬 한도는 커널이 정한다 ---

/// 일반 파일로 끝나는 사슬. 커널 한도(32)까지는 대상 파일을 원자적으로 바꾸고 링크는 그대로다.
/// 하나 더 길면 `fs::metadata`가 ELOOP라 종료 1이고 대상은 그대로다. 도구 쪽 `resolve_symlink` 한도
/// (40)가 판정에 끼지 않는다.
#[test]
fn binaries_output_link_chain_to_regular_file_kernel_limit_decides() {
    // 정규화해 둔다. `/var` 같은 경로 안의 링크도 커널 한도에 들어간다.
    let dir = fs::canonicalize(temp_dir("chain")).unwrap();
    let txt = melody_txt(&dir);
    let target = dir.join("target");
    fs::write(&target, "OLD").unwrap();
    // 같은 까닭으로 링크 대상은 이름만 쓴다.
    let mut prev = OsString::from("target");
    let mut chain = Vec::new();
    for i in 0..=KERNEL_LINKS {
        let name = format!("c{i}");
        let link = dir.join(&name);
        std::os::unix::fs::symlink(&prev, &link).unwrap();
        chain.push(link);
        prev = name.into();
    }
    let at_limit = &chain[KERNEL_LINKS - 1];
    let over_limit = &chain[KERNEL_LINKS];

    for r in both_o(&txt, over_limit) {
        assert_eq!(r.code, Some(1), "{}", r.stderr);
        assert!(r.stderr.contains("os error 62"), "{}", r.stderr);
        assert!(r.stderr.contains("리다이렉트"), "{}", r.stderr);
        assert_eq!(fs::read_to_string(&target).unwrap(), "OLD");
    }
    let [a, b] = both_o(&txt, at_limit);
    assert_eq!(a.code, Some(0), "{}", a.stderr);
    assert_eq!(b.code, Some(0), "{}", b.stderr);
    assert_eq!(
        fs::read(&target).unwrap(),
        fs::read(fixture("01-melody.ksq")).unwrap()
    );
    for link in &chain {
        assert!(fs::symlink_metadata(link).unwrap().is_symlink());
    }
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: NotFound가 아닌 확인 오류는 쓰지 않는다 ---

/// ENOTDIR(일반 파일 아래 이름), ELOOP(두 링크 순환), EACCES(권한 없는 부모 안 파일). 모두 종료 1,
/// 「확인할 수 없다」와 셸 리다이렉션 안내, 대상과 링크는 그대로, 임시 파일 없음. 두 바이너리 모두.
#[test]
fn binaries_output_unchecked_errors_exit_1_and_keep_targets() {
    let dir = temp_dir("unchecked");
    let txt = melody_txt(&dir);
    let regular = dir.join("regular");
    fs::write(&regular, "keep").unwrap();
    let a = dir.join("loop-a");
    let b = dir.join("loop-b");
    std::os::unix::fs::symlink("loop-b", &a).unwrap();
    std::os::unix::fs::symlink("loop-a", &b).unwrap();
    let locked = dir.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("f"), "keep").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let mut cases = vec![
        (regular.join("x"), "os error 20"),
        (a.clone(), "os error 62"),
    ];
    if !is_root(&dir) {
        cases.push((locked.join("f"), "os error 13"));
    }
    for (output, errno) in &cases {
        for r in both_o(&txt, output) {
            assert_eq!(r.code, Some(1), "{}: {}", output.display(), r.stderr);
            assert!(r.stderr.contains(errno), "{}", r.stderr);
            assert!(r.stderr.contains("확인할 수 없다"), "{}", r.stderr);
            assert!(r.stderr.contains("리다이렉트"), "{}", r.stderr);
            assert!(r.stdout.is_empty());
        }
    }

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(fs::read_to_string(&regular).unwrap(), "keep");
    assert_eq!(fs::read_to_string(locked.join("f")).unwrap(), "keep");
    assert!(fs::symlink_metadata(&a).unwrap().is_symlink());
    assert!(fs::symlink_metadata(&b).unwrap().is_symlink());
    assert!(temp_leftovers(&dir).is_empty());
    assert!(temp_leftovers(&locked).is_empty());
    assert_eq!(
        names(&dir),
        vec!["in.txt", "locked", "loop-a", "loop-b", "regular"]
    );
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: 일반 파일이 아닌 대상은 열지도 않는다 ---

/// 읽는 쪽 없는 FIFO, 그것을 가리키는 링크·세 단계 사슬, 유닉스 소켓. 도구가 FIFO를 쓰기로 열면
/// 읽는 쪽이 없어 막히므로 시간 제한에 걸린다. 모두 종료 1, 종류를 밝힌 안내, 대상·링크 그대로,
/// 임시 파일 없음. 두 바이너리 모두.
#[test]
fn binaries_output_fifo_socket_and_links_refused_without_opening_or_temp_files() {
    let dir = temp_dir("special");
    let txt = melody_txt(&dir);
    let fifo = dir.join("pipe");
    mkfifo(&fifo);
    let link = dir.join("to-pipe");
    std::os::unix::fs::symlink("pipe", &link).unwrap();
    let mut chain = fifo.clone();
    for i in 0..3 {
        let l = dir.join(format!("hop{i}"));
        std::os::unix::fs::symlink(&chain, &l).unwrap();
        chain = l;
    }
    let sock = dir.join("sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

    for (output, kind) in [
        (&fifo, "FIFO"),
        (&link, "FIFO"),
        (&chain, "FIFO"),
        (&sock, "소켓"),
    ] {
        for r in both_o(&txt, output) {
            assert_eq!(r.code, Some(1), "{}: {}", output.display(), r.stderr);
            assert!(r.stderr.contains(kind), "{}", r.stderr);
            assert!(r.stderr.contains("리다이렉트"), "{}", r.stderr);
            assert!(r.stdout.is_empty());
        }
    }

    drop(listener);
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    assert!(fs::symlink_metadata(&sock).unwrap().file_type().is_socket());
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert!(fs::symlink_metadata(&chain).unwrap().is_symlink());
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: `on_devfs`가 `/dev` 밖 일반 파일을 거르지 않는다 ---

/// 임시 디렉터리의 일반 파일을 그대로, `/dev/..`를 거친 절대 경로로, 그리고 `/dev` 쪽 링크
/// (`/dev/../<경로>`를 가리키는 링크)로 준다. 모두 `/dev`와 장치 번호가 다른 일반 파일이므로 종료 0이고
/// 원자적으로 바뀐다(새 inode).
#[test]
fn binaries_output_regular_file_off_devfs_accepted_through_dev_dotdot() {
    let dir = temp_dir("off-devfs");
    let txt = melody_txt(&dir);
    let real = fs::canonicalize(&dir).unwrap();
    assert_ne!(
        fs::metadata(&real).unwrap().dev(),
        fs::metadata("/dev").unwrap().dev(),
        "전제: 임시 디렉터리는 devfs가 아니다"
    );
    let plain = real.join("plain.out");
    let via_dev = PathBuf::from("/dev/..").join(real.strip_prefix("/").unwrap().join("via.out"));
    let link = real.join("link.out");
    let linked = real.join("linked.out");
    std::os::unix::fs::symlink(
        PathBuf::from("/dev/..").join(linked.strip_prefix("/").unwrap()),
        &link,
    )
    .unwrap();

    for (output, written) in [
        (&plain, &plain),
        (&via_dev, &real.join("via.out")),
        (&link, &linked),
    ] {
        fs::write(written, "OLD".repeat(1000)).unwrap();
        let ino = fs::metadata(written).unwrap().ino();
        let [a, b] = both_o(&txt, output);
        assert_eq!(a.code, Some(0), "{}: {}", output.display(), a.stderr);
        assert_eq!(b.code, Some(0), "{}: {}", output.display(), b.stderr);
        assert_eq!(
            fs::read(written).unwrap(),
            fs::read(fixture("01-melody.ksq")).unwrap(),
            "{}",
            output.display()
        );
        assert_ne!(fs::metadata(written).unwrap().ino(), ino);
    }
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert!(temp_leftovers(&real).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: 입력과 같은 파일의 거부가 범위 판정보다 먼저다 ---

/// 입력 그 자체, 하드 링크, 심볼릭 링크. 모두 종료 1이고 「입력 파일과 같다」로 거부하며 입력 바이트는
/// 그대로다. 두 바이너리 모두.
#[test]
fn binaries_output_same_file_as_input_refused_and_input_kept() {
    let dir = temp_dir("same");
    let ksq = dir.join("in.ksq");
    fs::copy(fixture("01-melody.ksq"), &ksq).unwrap();
    let txt = melody_txt(&dir);

    for (bin, input) in [(KSQ2TXT, &ksq), (TXT2KSQ, &txt)] {
        let before = fs::read(input).unwrap();
        let hard = dir.join("hard");
        let soft = dir.join("soft");
        let _ = fs::remove_file(&hard);
        let _ = fs::remove_file(&soft);
        fs::hard_link(input, &hard).unwrap();
        std::os::unix::fs::symlink(input, &soft).unwrap();
        for output in [input, &hard, &soft] {
            let r = run(bin, &[input.as_os_str(), "-o".as_ref(), output.as_os_str()]);
            assert_eq!(r.code, Some(1), "{bin} {}: {}", output.display(), r.stderr);
            assert!(r.stderr.contains("입력 파일과 같다"), "{}", r.stderr);
            assert_eq!(fs::read(input).unwrap(), before, "{bin}");
        }
    }
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: 받은 일반 파일에 쓰기가 실패해도 임시 파일이 남지 않는다 ---

/// 변경 불가(`chflags uchg`) 일반 파일은 확인을 통과하지만 이름 바꾸기가 EPERM으로 실패한다. 종료 1,
/// 내용 그대로, 임시 파일 없음. `chflags`가 없으면 건너뛴다.
#[test]
fn binaries_output_immutable_regular_file_fails_without_temp_files() {
    let dir = temp_dir("immutable");
    let txt = melody_txt(&dir);
    let out = dir.join("frozen.out");
    fs::write(&out, "keep").unwrap();
    let locked = Command::new("chflags").arg("uchg").arg(&out).status();
    if !locked.is_ok_and(|s| s.success()) {
        fs::remove_dir_all(&dir).unwrap();
        return;
    }

    let results = both_o(&txt, &out);

    let _ = Command::new("chflags").arg("nouchg").arg(&out).status();
    for r in results {
        assert_eq!(r.code, Some(1), "{}", r.stderr);
        assert!(r.stderr.contains("os error 1"), "{}", r.stderr);
    }
    assert_eq!(fs::read_to_string(&out).unwrap(), "keep");
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

// --- 경계: 없는 경로지만 만들 수 없는 꼴 ---

/// 끝이 `/`·`/.`인 없는 경로, 끝이 `/`인 끊긴 링크, `/dev/fd` 아래 없는 이름. `fs::metadata`는
/// `NotFound`라 확인을 통과하지만 만들기에서 실패한다. 종료 1, 아무것도 만들지 않고 링크는 그대로,
/// 임시 파일 없음.
#[test]
fn binaries_output_uncreatable_missing_paths_fail_without_creating() {
    let dir = temp_dir("uncreatable");
    let txt = melody_txt(&dir);
    let dangling = dir.join("dl");
    std::os::unix::fs::symlink("dl-target", &dangling).unwrap();
    let mut slash = dir.join("newf").into_os_string();
    slash.push("/");
    let mut slash_dot = dir.join("newg").into_os_string();
    slash_dot.push("/.");
    let mut link_slash = dangling.clone().into_os_string();
    link_slash.push("/");
    let fd_name = PathBuf::from(format!("/dev/fd/ksq-r9-{}", std::process::id()));

    for output in [
        PathBuf::from(slash),
        PathBuf::from(slash_dot),
        PathBuf::from(link_slash),
        fd_name.clone(),
    ] {
        for r in both_o(&txt, &output) {
            assert_eq!(r.code, Some(1), "{}: {}", output.display(), r.stderr);
            assert!(r.stdout.is_empty());
        }
    }
    assert!(fs::symlink_metadata(&dangling).unwrap().is_symlink());
    assert!(fs::symlink_metadata(&fd_name).is_err());
    assert_eq!(names(&dir), vec!["dl", "in.txt"]);
    fs::remove_dir_all(&dir).unwrap();
}
