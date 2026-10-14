//! S0-T8 적대적 리뷰 11회차(`docs/reports/s0-t8/review-11.md`)가 남긴 테스트. 범위는 리뷰 10 대응
//! 두 커밋 `af2c855`(`-o` 범위 판정을 입력 열기 앞으로), `473d0d2`(만들지 못한 `-o`의 문구 분기)뿐이다.
//!
//! 깨뜨리지 못해 모두 경계를 확인하는 통과 테스트다.
//!
//! - FIFO 입력(쓰는 쪽 없음)에 거부될 `-o`(같은 FIFO의 하드 링크·심볼릭 링크, 장치, 디렉터리)는 입력을
//!   열지 않고 곧바로 종료 1이다.
//! - 같은 파일 판정은 순서를 바꾼 뒤에도 하드 링크·심볼릭 링크·`/dev/stdin` 입력에서 입력을 지킨다.
//! - 만들지 못한 `-o`의 문구는 특수 파일 자리(`/dev/` 아래·끝 `/`·`/.`, ENOENT·ENOTDIR·EPERM)면
//!   리다이렉트 안내, 그 밖(EACCES, 없는 디렉터리 안)이면 「다른 경로를 주거나」 안내이고, 경로는 한
//!   번만 적힌다.
//!
//! FIFO에는 읽는 쪽도 쓰는 쪽도 두지 않는다. 바이너리를 띄우는 테스트는 모두 시간 제한을 두고, 넘으면
//! 자식을 죽인 뒤 실패로 보고한다.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

/// 정상 실행의 시간 제한.
const TIMEOUT: Duration = Duration::from_secs(30);
/// 거부가 곧바로 나야 하는 실행의 시간 제한. 멈춤을 빨리 잡으려고 짧게 둔다.
const REFUSE_TIMEOUT: Duration = Duration::from_secs(5);

const SPECIAL_HINT: &str = "특수 파일에 쓰려면 -o 없이 실행하고 셸로 리다이렉트하라";
const GENERAL_HINT: &str = "다른 경로를 주거나 표준 출력으로 받는다";

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-r11-{}-{name}", std::process::id()));
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

struct Ran {
    /// `None`이면 신호로 끝났다.
    code: Option<i32>,
    timed_out: bool,
    stderr: String,
}

/// `cmd`를 띄우고 `limit` 안에 끝나지 않으면 죽인다. 표준 출력·오류는 `log` 디렉터리의 파일로 받아
/// 파이프가 차서 멈추는 일을 없앤다.
fn run_cmd(mut cmd: Command, log: &Path, limit: Duration) -> Ran {
    let err_path = log.join("stderr");
    let out_path = log.join("stdout");
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
    Ran {
        code: status.code(),
        timed_out,
        stderr: fs::read_to_string(&err_path).unwrap_or_default(),
    }
}

/// `dir` 안의 이름을 정렬해 돌려준다.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
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

/// `af2c855`의 새 갈래: 일반 파일이 아닌 `-o`가 입력 경로와 같은 파일이면 「입력과 같다」. 입력은
/// `stat`만 하므로 FIFO의 하드 링크·심볼릭 링크를 `-o`로 줘도 열기에서 막히지 않는다. 장치·디렉터리는
/// 범위 거부. FIFO는 그대로 남고 아무것도 생기지 않는다.
#[test]
fn binaries_fifo_input_with_fifo_alias_or_special_output_exit_1_without_opening_input() {
    let dir = temp_dir("fifo-alias");
    let log = temp_dir("fifo-alias-log");
    let fifo = dir.join("in.pipe");
    mkfifo(&fifo);
    let hard = dir.join("hard.pipe");
    fs::hard_link(&fifo, &hard).unwrap();
    let soft = dir.join("soft.pipe");
    std::os::unix::fs::symlink("in.pipe", &soft).unwrap();
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();

    let cases: [(&Path, &str); 5] = [
        (hard.as_path(), "입력 파일과 같다"),
        (soft.as_path(), "입력 파일과 같다"),
        (fifo.as_path(), "입력 파일과 같다"),
        (sub.as_path(), "디렉터리"),
        (Path::new("/dev/null"), "문자 장치"),
    ];
    let mut bad = Vec::new();
    for bin in [KSQ2TXT, TXT2KSQ] {
        for (target, want) in cases {
            let mut cmd = Command::new(bin);
            cmd.arg(&fifo).arg("-o").arg(target).stdin(Stdio::null());
            let r = run_cmd(cmd, &log, REFUSE_TIMEOUT);
            if r.timed_out || r.code != Some(1) || !r.stderr.contains(want) {
                bad.push(format!(
                    "{bin} -o {}: 멈춤={} 종료={:?} {}",
                    target.display(),
                    r.timed_out,
                    r.code,
                    r.stderr
                ));
            }
        }
    }

    let is_fifo = |p: &Path| {
        fs::metadata(p).is_ok_and(|m| std::os::unix::fs::FileTypeExt::is_fifo(&m.file_type()))
    };
    assert!(is_fifo(&fifo) && is_fifo(&hard) && is_fifo(&soft));
    assert_eq!(names(&dir), ["hard.pipe", "in.pipe", "soft.pipe", "sub"]);
    assert!(names(&sub).is_empty());
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&log);
    assert!(bad.is_empty(), "거부가 어긋났다:\n{}", bad.join("\n"));
}

/// 순서를 바꾼 뒤에도(범위 판정 → 입력 열기 → 연 핸들과 출력 비교) 하드 링크·심볼릭 링크로 철자가
/// 다른 `-o`와, 입력을 `/dev/stdin`(표준 입력 = 그 파일)으로 준 경우를 모두 거부하고 입력 바이트는
/// 그대로다.
#[test]
fn binaries_same_file_after_reorder_through_links_and_dev_stdin_keep_input() {
    let dir = temp_dir("same-file");
    let log = temp_dir("same-file-log");
    let ksq = dir.join("in.ksq");
    fs::copy(fixture("01-melody.ksq"), &ksq).unwrap();
    let txt = dir.join("in.txt");
    fs::write(&txt, melody_text()).unwrap();
    let ksq_bytes = fs::read(&ksq).unwrap();
    let txt_bytes = fs::read(&txt).unwrap();
    for (input, stem) in [(&ksq, "ksq"), (&txt, "txt")] {
        fs::hard_link(input, dir.join(format!("hard.{stem}"))).unwrap();
        std::os::unix::fs::symlink(input, dir.join(format!("soft.{stem}"))).unwrap();
    }

    let mut bad = Vec::new();
    for (bin, input, stem) in [(KSQ2TXT, &ksq, "ksq"), (TXT2KSQ, &txt, "txt")] {
        for name in [
            format!("in.{stem}"),
            format!("hard.{stem}"),
            format!("soft.{stem}"),
        ] {
            let target = dir.join(&name);
            for via_stdin in [false, true] {
                let mut cmd = Command::new(bin);
                if via_stdin {
                    cmd.arg("/dev/stdin").stdin(fs::File::open(input).unwrap());
                } else {
                    cmd.arg(input).stdin(Stdio::null());
                }
                cmd.arg("-o").arg(&target);
                let r = run_cmd(cmd, &log, REFUSE_TIMEOUT);
                if r.timed_out || r.code != Some(1) || !r.stderr.contains("입력 파일과 같다")
                {
                    bad.push(format!(
                        "{bin} stdin={via_stdin} -o {name}: 멈춤={} 종료={:?} {}",
                        r.timed_out, r.code, r.stderr
                    ));
                }
            }
        }
    }

    assert_eq!(fs::read(&ksq).unwrap(), ksq_bytes);
    assert_eq!(fs::read(&txt).unwrap(), txt_bytes);
    assert_eq!(
        names(&dir),
        [
            "hard.ksq", "hard.txt", "in.ksq", "in.txt", "soft.ksq", "soft.txt"
        ]
    );
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&log);
    assert!(
        bad.is_empty(),
        "같은 파일 거부가 어긋났다:\n{}",
        bad.join("\n")
    );
}

/// `473d0d2`의 문구 분기. 확인 때 없던 `-o`를 만들지 못하면
/// - 특수 파일 자리(`/dev/` 아래, 끝 `/`·`/.`)이고 EPERM·ENOENT면 리다이렉트 안내,
/// - EACCES(끝 `/`가 붙어도)나 없는 디렉터리 안이면 「다른 경로를 주거나」 안내다.
///
/// 두 바이너리 모두 종료 1, 경로는 한 번, 그 바이너리의 예, 아무것도 생기지 않는다.
#[test]
fn binaries_uncreatable_output_wording_splits_by_special_place_and_errno() {
    let dir = temp_dir("wording");
    let log = temp_dir("wording-log");
    let ksq = dir.join("in.ksq");
    fs::copy(fixture("01-melody.ksq"), &ksq).unwrap();
    let txt = dir.join("in.txt");
    fs::write(&txt, melody_text()).unwrap();
    let ro = dir.join("ro");
    fs::create_dir(&ro).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
    let dev_name = format!("/dev/ksq-r11-{}", std::process::id());

    let cases: Vec<(PathBuf, bool)> = vec![
        (PathBuf::from(&dev_name), true),
        (PathBuf::from(format!("{dev_name}/")), true),
        (PathBuf::from("/dev/fd/ksq-r11"), true),
        (dir.join("new/"), true),
        (dir.join("new/."), true),
        (dir.join("nodir/x/"), true),
        (dir.join("nodir/x"), false),
        (ro.join("new"), false),
        (ro.join("new/"), false),
        (ro.join("new/."), false),
    ];
    let mut bad = Vec::new();
    for (bin, input, example) in [
        (KSQ2TXT, &ksq, "ksq2txt in.ksq > out.txt"),
        (TXT2KSQ, &txt, "txt2ksq in.txt > out.ksq"),
    ] {
        for (target, special) in &cases {
            let mut cmd = Command::new(bin);
            cmd.arg(input).arg("-o").arg(target).stdin(Stdio::null());
            let r = run_cmd(cmd, &log, TIMEOUT);
            let shown = target.display().to_string();
            let ok = !r.timed_out
                && r.code == Some(1)
                && r.stderr.contains("-o 대상을 만들지 못했다")
                && r.stderr.matches(shown.as_str()).count() == 1
                && r.stderr.contains(example)
                && r.stderr.contains(SPECIAL_HINT) == *special
                && r.stderr.contains(GENERAL_HINT) != *special;
            if !ok {
                bad.push(format!(
                    "{bin} -o {shown} (특수 자리 기대={special}): 멈춤={} 종료={:?} {}",
                    r.timed_out, r.code, r.stderr
                ));
            }
        }
    }

    let ro_names = names(&ro);
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(ro_names.is_empty(), "{ro_names:?}");
    assert!(!Path::new(&dev_name).exists());
    assert_eq!(names(&dir), ["in.ksq", "in.txt", "ro"]);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&log);
    assert!(bad.is_empty(), "문구 분기가 어긋났다:\n{}", bad.join("\n"));
}

/// 문구 분기는 경로 글자만 본다(결정: 경로 모양은 메시지를 고르는 데만 쓴다). `/dev/..`로 시작하지만
/// 실제로는 없는 디렉터리 안인 경로, `/dev`를 가리키는 끊긴 링크는 문구가 원인과 어긋날 수 있다(리뷰 11
/// 제안 1). 문구는 단언하지 않고, 어느 쪽이든 종료 1이고 아무것도 생기지 않는 동작만 확인한다.
#[test]
fn binaries_uncreatable_output_with_misleading_path_shape_exit_1_without_creating() {
    let dir = temp_dir("shape");
    let log = temp_dir("shape-log");
    let ksq = dir.join("in.ksq");
    fs::copy(fixture("01-melody.ksq"), &ksq).unwrap();
    let link = dir.join("to-dev");
    std::os::unix::fs::symlink(format!("/dev/ksq-r11-{}", std::process::id()), &link).unwrap();
    let dotdot = PathBuf::from("/dev/..")
        .join(dir.strip_prefix("/").unwrap())
        .join("nodir/x");

    for target in [&dotdot, &link] {
        let mut cmd = Command::new(KSQ2TXT);
        cmd.arg(&ksq).arg("-o").arg(target).stdin(Stdio::null());
        let r = run_cmd(cmd, &log, TIMEOUT);
        assert!(!r.timed_out, "{}: 멈춤", target.display());
        assert_eq!(r.code, Some(1), "{}: {}", target.display(), r.stderr);
        assert!(r.stderr.contains("-o 대상을 만들지 못했다"), "{}", r.stderr);
    }

    assert_eq!(names(&dir), ["in.ksq", "to-dev"]);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&log);
}
