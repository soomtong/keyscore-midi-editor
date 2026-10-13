//! 실제 바이너리로 하는 검사(S0-T8, 계획 테스트 5·6·10·11). 종료 코드와 파일 쓰기를 본다.

use std::fs;
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
    // 클립 길이를 노트 끝보다 짧게 바꾼다: 「노트는 클립 안」 불변식 위반.
    let broken = small_ksq().replacen("\"length\": 960", "\"length\": 10", 1);
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
