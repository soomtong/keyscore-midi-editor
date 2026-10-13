//! 실제 바이너리로 하는 검사(S0-T8, 계획 테스트 5·6·10·11). 종료 코드와 파일 쓰기를 본다.

use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity, ksq};
use ksq_cli::{CliError, check};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-cli-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

fn run(bin: &str, args: &[&Path]) -> Output {
    Command::new(bin).args(args).output().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// 노트 하나(0–960)가 든 960틱 클립 하나짜리 프로젝트의 `.ksq` 텍스트.
fn small_ksq() -> String {
    let mut clip = Clip::new(Tick::ZERO, 960).unwrap();
    clip.insert_note(
        Note::new(
            Tick::ZERO,
            960,
            Pitch::new(60).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut track = Track::new("A", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip).unwrap();
    let mut p = Project::new();
    p.push_track(track).unwrap();
    ksq::to_string(&p)
}

const KSQ: &str = env!("CARGO_BIN_EXE_ksq");
const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

#[test]
fn check_fails_on_invariant_violation_with_nonzero_exit() {
    let dir = temp_dir("invariant");
    let good = dir.join("good.ksq");
    let bad = dir.join("bad.ksq");
    fs::write(&good, small_ksq()).unwrap();
    // 노트 시작을 클립 끝(960) 뒤로 옮긴다: 「노트는 클립 안」 불변식 위반. 끝만 넘는 노트는
    // 분할로 생기는 숨김 꼬리라 합법이므로 시작을 옮긴다.
    let broken = small_ksq().replacen("[0, 960, 60, 51492]", "[2000, 960, 60, 51492]", 1);
    assert_ne!(broken, small_ksq());
    fs::write(&bad, broken).unwrap();

    let o = run(KSQ, &[Path::new("check"), &good, &bad]);

    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    assert!(
        stderr(&o).contains(&bad.display().to_string()),
        "{}",
        stderr(&o)
    );
    // 좋은 파일은 여전히 통과로 보고된다(파일마다 따로 검사).
    assert!(
        String::from_utf8_lossy(&o.stdout).contains(&good.display().to_string()),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn check_fails_on_newer_version() {
    let dir = temp_dir("newer");
    let path = dir.join("newer.ksq");
    let next = ksq::FORMAT_VERSION + 1;
    let text = small_ksq().replacen(
        &format!("\"format_version\": {}", ksq::FORMAT_VERSION),
        &format!("\"format_version\": {next}"),
        1,
    );
    fs::write(&path, text).unwrap();

    let err = check(&path).unwrap_err();
    let o = run(KSQ, &[Path::new("check"), &path]);

    assert!(
        matches!(err, CliError::NewerVersion { version, .. } if version == next),
        "{err}"
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("버전"), "{}", stderr(&o));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn check_reads_gzip_and_warns_without_version() {
    let dir = temp_dir("gzip");
    let gz = dir.join("a.ksqz");
    let (p, _) = ksq::from_str(&small_ksq()).unwrap();
    ksq::write_file(&gz, &p, ksq::Compression::Gzip).unwrap();
    let plain = dir.join("noversion.ksq");
    let text = small_ksq().replacen(
        &format!("\"format_version\": {},", ksq::FORMAT_VERSION),
        "",
        1,
    );
    fs::write(&plain, text).unwrap();

    let gz_report = check(&gz).unwrap();
    let plain_report = check(&plain).unwrap();
    let o = run(KSQ, &[Path::new("check"), &plain]);

    assert_eq!(gz_report.version(), Some(ksq::FORMAT_VERSION));
    assert_eq!(
        (gz_report.tracks(), gz_report.clips(), gz_report.notes()),
        (1, 1, 1)
    );
    assert_eq!(plain_report.version(), None);
    assert_eq!(plain_report.warnings().len(), 1);
    // 경고는 실패가 아니다.
    assert_eq!(o.status.code(), Some(0));
    assert!(stderr(&o).contains("format_version"), "{}", stderr(&o));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
#[allow(clippy::many_single_char_names, reason = "단계별 실행 결과 a–e")]
fn binaries_round_trip_files_via_cli() {
    let dir = temp_dir("roundtrip");
    let input = fixture("04-band.ksq");
    let txt = dir.join("band.txt");
    let back = dir.join("band.ksq");

    let a = run(KSQ2TXT, &[&input, Path::new("-o"), &txt]);
    let b = run(
        TXT2KSQ,
        &[&txt, Path::new("-o"), &back, Path::new("--strict")],
    );

    assert_eq!(a.status.code(), Some(0), "{}", stderr(&a));
    assert_eq!(b.status.code(), Some(0), "{}", stderr(&b));
    assert!(a.stdout.is_empty() && a.stderr.is_empty(), "{}", stderr(&a));
    assert_eq!(fs::read(&back).unwrap(), fs::read(&input).unwrap());

    // -o 없으면 표준 출력에 쓴다.
    let c = run(KSQ2TXT, &[&input]);
    assert_eq!(c.stdout, fs::read(&txt).unwrap());

    // .ksqz로 쓰면 gzip이고, 그대로 다시 읽힌다.
    let gz = dir.join("band.ksqz");
    let d = run(TXT2KSQ, &[&txt, Path::new("-o"), &gz]);
    assert_eq!(d.status.code(), Some(0), "{}", stderr(&d));
    let bytes = fs::read(&gz).unwrap();
    assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
    let (p, _) = ksq::from_bytes(&bytes).unwrap();
    assert_eq!(ksq::to_string(&p).into_bytes(), fs::read(&input).unwrap());

    // ksq fixtures도 같은 바이트를 쓴다.
    let gen_dir = dir.join("gen");
    let e = run(KSQ, &[Path::new("fixtures"), &gen_dir]);
    assert_eq!(e.status.code(), Some(0), "{}", stderr(&e));
    assert_eq!(
        fs::read(gen_dir.join("04-band.ksq")).unwrap(),
        fs::read(&input).unwrap()
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn binaries_refuse_output_equal_to_input() {
    let dir = temp_dir("same");
    let ksq_path = dir.join("a.ksq");
    fs::write(&ksq_path, small_ksq()).unwrap();
    let txt_path = dir.join("a.txt");
    fs::write(&txt_path, "@track A ch1\nc4\n").unwrap();
    // 다른 철자로 같은 파일을 가리켜도 거부한다.
    let dotted = dir.join(".").join("a.txt");
    let link = dir.join("link.ksq");
    std::os::unix::fs::symlink(&ksq_path, &link).unwrap();

    let a = run(TXT2KSQ, &[&txt_path, Path::new("-o"), &dotted]);
    let b = run(KSQ2TXT, &[&ksq_path, Path::new("-o"), &link]);

    assert_eq!(a.status.code(), Some(1), "{}", stderr(&a));
    assert_eq!(b.status.code(), Some(1), "{}", stderr(&b));
    assert_eq!(fs::read_to_string(&txt_path).unwrap(), "@track A ch1\nc4\n");
    assert_eq!(fs::read_to_string(&ksq_path).unwrap(), small_ksq());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn txt2ksq_strict_rejects_offsets() {
    let dir = temp_dir("strict");
    let txt = dir.join("offset.txt");
    fs::write(&txt, "@track A ch1\nc8~+13 d4\n").unwrap();
    let strict_out = dir.join("strict.ksq");
    let lenient_out = dir.join("lenient.ksq");

    let s = run(
        TXT2KSQ,
        &[&txt, Path::new("-o"), &strict_out, Path::new("--strict")],
    );
    let l = run(TXT2KSQ, &[&txt, Path::new("-o"), &lenient_out]);

    assert_eq!(s.status.code(), Some(1));
    assert!(
        stderr(&s).contains(&txt.display().to_string()),
        "{}",
        stderr(&s)
    );
    assert!(!strict_out.exists());
    assert_eq!(l.status.code(), Some(0), "{}", stderr(&l));
    let (p, _) = ksq::read_file(&lenient_out).unwrap();
    let starts: Vec<_> = p.tracks()[0]
        .clips()
        .next()
        .unwrap()
        .1
        .notes()
        .iter()
        .map(|n| n.start().get())
        .collect();
    assert_eq!(starts, vec![13, 480]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn txt2ksq_lenient_writes_warnings_to_stderr_and_succeeds() {
    let dir = temp_dir("lenient");
    let txt = dir.join("t.txt");
    fs::write(&txt, "@track A ch1\nc4 q d4\n").unwrap();

    let o = run(TXT2KSQ, &[&txt]);

    assert_eq!(o.status.code(), Some(0));
    assert!(stderr(&o).contains("줄 2"), "{}", stderr(&o));
    let (p, _) = ksq::from_str(&String::from_utf8(o.stdout).unwrap()).unwrap();
    assert_eq!(p.tracks().len(), 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ksq2txt_writes_losses_to_stderr() {
    let dir = temp_dir("losses");
    let path = dir.join("a.ksq");
    // 벨로시티를 80으로 바꾼다.
    let text = small_ksq().replacen("[0, 960, 60, 51492]", "[0, 960, 60, 40960]", 1);
    fs::write(&path, text).unwrap();

    let o = run(KSQ2TXT, &[&path]);

    assert_eq!(o.status.code(), Some(0));
    assert!(stderr(&o).contains("벨로시티"), "{}", stderr(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("@track A ch1"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn binaries_usage_errors_exit_with_two() {
    for (bin, args) in [
        (KSQ, vec![]),
        (KSQ, vec!["check"]),
        (KSQ, vec!["nope"]),
        (KSQ2TXT, vec![]),
        (KSQ2TXT, vec!["a", "-o"]),
        (TXT2KSQ, vec!["a", "b"]),
        (TXT2KSQ, vec!["--bogus", "a"]),
    ] {
        let o = Command::new(bin).args(&args).output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{bin} {args:?}: {}", stderr(&o));
        assert!(!o.stderr.is_empty());
    }
}

#[test]
fn ksq2txt_far_clip_exits_one_and_writes_nothing() {
    let dir = temp_dir("far");
    let input = dir.join("far.ksq");
    // 클립 시작을 100억 마디 뒤로 옮긴다. 합치면 앞 빈 곳이 쉼표로 풀려 거대해진다.
    let far = small_ksq().replacen("\"start\": 0", "\"start\": 38400000000000", 1);
    assert_ne!(far, small_ksq());
    fs::write(&input, far).unwrap();
    let output = dir.join("far.txt");

    let to_file = run(KSQ2TXT, &[&input, Path::new("-o"), &output]);
    let to_stdout = run(KSQ2TXT, &[&input]);

    assert_eq!(to_file.status.code(), Some(1));
    assert!(
        stderr(&to_file).contains("너무 커진다"),
        "{}",
        stderr(&to_file)
    );
    assert!(!output.exists());
    assert_eq!(to_stdout.status.code(), Some(1));
    assert!(to_stdout.stdout.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ksq2txt_output_replaces_atomically_keeping_symlink_and_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("atomic");
    let input = fixture("01-melody.ksq");
    let target = dir.join("target.txt");
    fs::write(
        &target,
        "옛 내용이 더 길어서 잘리지 않으면 꼬리가 남는다 ".repeat(100),
    )
    .unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let link = dir.join("link.txt");
    std::os::unix::fs::symlink("target.txt", &link).unwrap();

    let o = run(KSQ2TXT, &[&input, Path::new("-o"), &link]);
    let expected = run(KSQ2TXT, &[&input]).stdout;

    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&target).unwrap(), expected);
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // 임시 파일이 남지 않는다.
    let mut names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, vec!["link.txt", "target.txt"]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn txt2ksq_skips_one_leading_utf8_bom() {
    let dir = temp_dir("bom");
    let one = dir.join("one.txt");
    let two = dir.join("two.txt");
    fs::write(&one, "\u{feff}@track A ch1\nc4\n").unwrap();
    fs::write(&two, "\u{feff}\u{feff}@track A ch1\nc4\n").unwrap();
    let plain = dir.join("plain.txt");
    fs::write(&plain, "@track A ch1\nc4\n").unwrap();

    let a = run(TXT2KSQ, &[&one, Path::new("--strict")]);
    let b = run(TXT2KSQ, &[&two]);
    let c = run(TXT2KSQ, &[&plain]);

    assert_eq!(a.status.code(), Some(0), "{}", stderr(&a));
    assert_eq!(a.stdout, c.stdout);
    // 하나만 건너뛴다. 둘째 BOM은 텍스트의 일부다.
    assert_eq!(b.status.code(), Some(1), "{}", stderr(&b));
    let _ = fs::remove_dir_all(&dir);
}

/// 바이너리와 읽는 쪽을 기다리는 시간 한도. 넘으면 테스트가 멈추지 않고 실패한다.
const FIFO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// FIFO를 만들고, 읽는 쪽 스레드를 띄운 채 `bin`이 `-o fifo`로 쓴 바이트를 돌려준다.
///
/// 쓰는 쪽이 FIFO를 열지 않으면(실패하거나 FIFO를 일반 파일로 바꿔치면) 읽는 쪽이 막힌다. 그래서
/// 같은 FIFO에 하드 링크를 하나 더 걸어 두고(이름이 바뀌어도 같은 FIFO를 가리킨다), 시간 안에
/// 읽는 쪽이 끝나지 않으면 그 링크를 읽기·쓰기(`O_RDWR`)로 열었다 닫아 풀어 준다. `O_RDWR` 열기는
/// 상대가 없어도 막히지 않으므로, 확인과 열기 사이에 읽는 쪽이 끝나도 멈추지 않는다(S0-T8 리뷰 3
/// 제안 4). 바이너리도 시간 한도를 넘으면 죽이고 실패한다.
fn write_through_fifo(dir: &Path, name: &str, bin: &str, input: &Path) -> (Output, Vec<u8>) {
    use std::time::Instant;

    let fifo = dir.join(name);
    let keep = dir.join(format!("{name}.keep"));
    let made = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(made.success());
    fs::hard_link(&fifo, &keep).unwrap();
    let reader = {
        let fifo = fifo.clone();
        std::thread::spawn(move || fs::read(fifo).unwrap())
    };

    let mut child = Command::new(bin)
        .args([input.as_os_str(), "-o".as_ref(), fifo.as_os_str()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + FIFO_TIMEOUT;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            drop(fs::OpenOptions::new().read(true).write(true).open(&keep));
            panic!("{name}: {bin}이 {FIFO_TIMEOUT:?} 안에 끝나지 않았다");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let o = child.wait_with_output().unwrap();

    let deadline = Instant::now() + FIFO_TIMEOUT;
    while !reader.is_finished() && Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !reader.is_finished() {
        drop(
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&keep)
                .unwrap(),
        );
    }
    let got = reader.join().unwrap();
    assert!(
        fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo(),
        "{name}: FIFO가 일반 파일로 바뀌었다"
    );
    (o, got)
}

#[test]
fn binaries_write_to_fifo_and_dev_null_without_replacing_them() {
    let dir = temp_dir("special");
    let input = fixture("01-melody.ksq");
    let txt = dir.join("in.txt");
    fs::write(&txt, run(KSQ2TXT, &[&input]).stdout).unwrap();

    let (a, from_ksq2txt) = write_through_fifo(&dir, "a.pipe", KSQ2TXT, &input);
    let (b, from_txt2ksq) = write_through_fifo(&dir, "b.pipe", TXT2KSQ, &txt);
    let c = run(TXT2KSQ, &[&txt, Path::new("-o"), Path::new("/dev/null")]);

    assert_eq!(a.status.code(), Some(0), "{}", stderr(&a));
    assert_eq!(from_ksq2txt, fs::read(&txt).unwrap());
    assert_eq!(b.status.code(), Some(0), "{}", stderr(&b));
    assert_eq!(from_txt2ksq, fs::read(&input).unwrap());
    assert_eq!(c.status.code(), Some(0), "{}", stderr(&c));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ksq2txt_max_output_overrides_limit_and_rejection_mentions_it() {
    let dir = temp_dir("max-output");
    let input = dir.join("far.ksq");
    // 클립을 2만 마디 뒤로 옮긴다. 기본 상한은 넘지만 텍스트는 수백 KB라 상한을 올리면 된다.
    let far = small_ksq().replacen("\"start\": 0", "\"start\": 76800000", 1);
    fs::write(&input, far).unwrap();

    let refused = run(KSQ2TXT, &[&input]);
    let allowed = run(
        KSQ2TXT,
        &[&input, Path::new("--max-output"), Path::new("100000000")],
    );
    let tight = run(
        KSQ2TXT,
        &[
            Path::new("--max-output"),
            Path::new("1"),
            &fixture("01-melody.ksq"),
        ],
    );

    assert_eq!(refused.status.code(), Some(1));
    assert!(
        stderr(&refused).contains("--max-output"),
        "{}",
        stderr(&refused)
    );
    assert_eq!(allowed.status.code(), Some(0), "{}", stderr(&allowed));
    assert!(String::from_utf8_lossy(&allowed.stdout).contains("@track A ch1"));
    // 상한을 낮출 수도 있다.
    assert_eq!(tight.status.code(), Some(1));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn max_output_argument_errors_are_usage_errors() {
    let input = fixture("01-melody.ksq");
    for args in [
        vec![input.as_os_str(), "--max-output".as_ref()],
        vec![input.as_os_str(), "--max-output".as_ref(), "0".as_ref()],
        vec![input.as_os_str(), "--max-output".as_ref(), "-5".as_ref()],
        vec![input.as_os_str(), "--max-output".as_ref(), "1kb".as_ref()],
        vec![
            input.as_os_str(),
            "--max-output".as_ref(),
            "9".as_ref(),
            "--max-output".as_ref(),
            "9".as_ref(),
        ],
    ] {
        let o = Command::new(KSQ2TXT).args(&args).output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
    }
    // txt2ksq에는 이 옵션이 없다.
    let o = Command::new(TXT2KSQ)
        .args(["a.txt", "--max-output", "9"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn ksq2txt_rejection_hint_gives_max_output_that_converts() {
    let dir = temp_dir("hint");
    let input = dir.join("far.ksq");
    // 2만 마디 뒤 클립 하나. 기본 상한은 넘고, 안내한 값을 주면 변환된다.
    let far = small_ksq().replacen("\"start\": 0", "\"start\": 76800000", 1);
    fs::write(&input, far).unwrap();

    let refused = run(KSQ2TXT, &[&input]);
    let err = stderr(&refused);
    let hint: String = err
        .split("--max-output ")
        .nth(1)
        .unwrap_or_else(|| panic!("{err}"))
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let again = run(
        KSQ2TXT,
        &[&input, Path::new("--max-output"), Path::new(&hint)],
    );
    let below = (hint.parse::<u64>().unwrap() - 1).to_string();
    let short = run(
        KSQ2TXT,
        &[&input, Path::new("--max-output"), Path::new(&below)],
    );

    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(again.status.code(), Some(0), "{hint}: {}", stderr(&again));
    // 안내한 값은 이 실행에서 통과하는 가장 작은 상한이다.
    assert_eq!(short.status.code(), Some(1));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn binaries_output_with_trailing_slash_fail_with_kernel_error() {
    let dir = temp_dir("slash");
    let input = fixture("01-melody.ksq");
    let txt = dir.join("in.txt");
    fs::write(&txt, run(KSQ2TXT, &[&input]).stdout).unwrap();
    // 대상이 `/`로 끝나는 링크도 끝의 `/`를 잃지 않는다.
    let link = dir.join("link");
    std::os::unix::fs::symlink("/dev/null/", &link).unwrap();

    let a = run(TXT2KSQ, &[&txt, Path::new("-o"), Path::new("/dev/null/")]);
    let b = run(KSQ2TXT, &[&input, Path::new("-o"), &link]);

    for o in [&a, &b] {
        assert_eq!(o.status.code(), Some(1), "{}", stderr(o));
        assert!(stderr(o).contains("Not a directory"), "{}", stderr(o));
    }
    let _ = fs::remove_dir_all(&dir);
}
