//! S0-T8 적대적 리뷰 5회차(`docs/reports/s0-t8/review-5.md`)가 남긴 테스트. 범위는 리뷰 4 수정 두
//! 커밋(`/dev/` 판정을 정규화한 부모로 하기, 거부 메시지가 어림 값을 안내하기)이다.
//!
//! - 실패하는 테스트: 끝이 `/`인 `-o`(`/dev/null/`, `/dev/stdout/`)를 커널(ENOTDIR)과 달리 받아들여
//!   쓰는 것, 어림이 u64를 넘는 곡에서 안내한 `--max-output` 값을 주어도 다시 거부되는 것.
//! - 통과하는 테스트: `..`·상대·절대 링크 사슬·`/dev`를 가리키는 디렉터리 링크·`/private` 별칭의
//!   판정 경계, 없는·권한 없는 부모, 빈 이름, `.`, 비 UTF-8 이름, `/dev/fd/1`이 파이프·일반 파일일 때,
//!   안내값이 표준 출력과 `-o` 모두에서 통과하는 것.
//!
//! 바이너리를 띄우는 테스트는 시간 제한을 둔다.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use ksq_cli::project_to_text;

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const WHOLE: u64 = 3840;
const TIMEOUT: Duration = Duration::from_secs(30);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-cli-review5-{}-{name}", std::process::id()));
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

/// 표준 출력을 `stdout`으로 두고 바이너리를 띄운다. 파이프면 표준 출력을 모아 돌려준다.
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

/// `dir`(정규 경로)에서 루트까지 올라가는 `../` 사슬.
fn up_to_root(dir: &Path) -> String {
    let depth = fs::canonicalize(dir)
        .unwrap()
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();
    "../".repeat(depth)
}

// --- 끝이 `/`인 경로 ---

/// 커널은 `/dev/null/`을 ENOTDIR로 거부하고, 리뷰 4 전 코드도 그대로 열어 종료 1이었다. 새 판정은
/// 링크를 따라간 경로를 부모 + 파일 이름으로 다시 짜며 끝의 `/`를 잃어 `/dev/null`에 쓰고 종료 0이다.
#[test]
fn ksq2txt_output_dev_null_with_trailing_slash_fails_not_a_directory() {
    let r = ksq2txt_o(Path::new("/dev/null/"));

    assert_eq!(r.code, Some(1), "{}", r.stderr);
}

/// 같은 까닭으로 `/dev/stdout/`도 받아들여, 일반 파일로 돌린 표준 출력에 텍스트를 쓴다. 셸에서
/// `> /dev/stdout/`는 실패한다.
#[test]
fn ksq2txt_output_dev_stdout_with_trailing_slash_fails_and_writes_nothing() {
    let dir = temp_dir("slash-stdout");
    let redirected = dir.join("stdout.txt");
    let r = run_with(
        KSQ2TXT,
        &[
            fixture("01-melody.ksq").as_os_str(),
            "-o".as_ref(),
            "/dev/stdout/".as_ref(),
        ],
        Stdio::from(fs::File::create(&redirected).unwrap()),
    );

    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert_eq!(fs::read(&redirected).unwrap().len(), 0);
    fs::remove_dir_all(&dir).unwrap();
}

/// 일반 파일 뒤의 `/`는 원자적 쓰기에서 ENOTDIR로 실패하고, 파일과 디렉터리에 아무것도 남기지 않는다.
#[test]
fn ksq2txt_output_regular_file_with_trailing_slash_fails_and_keeps_file() {
    let dir = temp_dir("slash-regular");
    let out = dir.join("out.txt");
    fs::write(&out, "keep").unwrap();
    let mut slashed = out.clone().into_os_string();
    slashed.push("/");

    let r = ksq2txt_o(Path::new(&slashed));

    assert_eq!(r.code, Some(1), "{}", r.stderr);
    assert_eq!(fs::read_to_string(&out).unwrap(), "keep");
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

// --- `/dev/` 판정 경계 ---

/// 상대 링크 → 상대 링크(`..`) → `.`가 섞인 `/dev/./stdout`(`..`는 `/dev` 밖 사슬에만 둔다. macOS에서 `/dev/fd/..`를 거치는 조회는 devfs 노드가 바뀌는 동안 가끔 ENOENT를 내기 때문, S0-T8 리뷰 7 제안 4), 그리고 그 사슬을 가리키는
/// 절대 링크. 둘 다 일반 파일로 돌린 표준 출력에 쓴다.
///
/// 결정 `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`로 기대를 바꿨다: `-o`는 없는 경로와
/// 일반 파일만 받는다. 특수 파일·`/dev/stdout`·`/dev/fd/N`(과 그 링크)은 종료 1이고 대상은 그대로다.
#[test]
fn ksq2txt_output_mixed_link_chain_to_dev_stdout_writes_redirected_file() {
    let dir = temp_dir("chain");
    fs::create_dir_all(dir.join("a/b")).unwrap();
    fs::create_dir_all(dir.join("c")).unwrap();
    std::os::unix::fs::symlink("../../c/l2", dir.join("a/b/l1")).unwrap();
    let up = up_to_root(&dir.join("c"));
    std::os::unix::fs::symlink(up + "dev/./stdout", dir.join("c/l2")).unwrap();
    std::os::unix::fs::symlink(dir.join("a/b/l1"), dir.join("abs")).unwrap();
    let expected = melody_text();

    for link in ["a/b/l1", "abs"] {
        let redirected = dir.join(format!("{}.out", link.replace('/', "_")));
        let r = run_with(
            KSQ2TXT,
            &[
                fixture("01-melody.ksq").as_os_str(),
                "-o".as_ref(),
                dir.join(link).as_os_str(),
            ],
            Stdio::from(fs::File::create(&redirected).unwrap()),
        );
        assert_eq!(r.code, Some(1), "{link}: {}", r.stderr);
        assert!(!expected.is_empty());
        assert_eq!(fs::read_to_string(&redirected).unwrap(), "", "{link}");
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// `/dev` 디렉터리를 가리키는 링크 `d`: `d/null`은 `/dev/` 아래라 그대로 쓰고, `d/../<임시>/out`은
/// 커널이 `/` 아래로 푸는 일반 파일이라 원자적으로 바꾼다(옛 꼬리 없음, 새 inode). 두 바이너리 모두.
///
/// 결정 `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`로 기대를 바꿨다: `-o`는 없는 경로와
/// 일반 파일만 받는다. 특수 파일·`/dev/stdout`·`/dev/fd/N`(과 그 링크)은 종료 1이고 대상은 그대로다.
/// `d/null`(문자 장치)이 여기에 해당하고, `d/..`를 거친 일반 파일은 그대로 원자적으로 바꾼다.
#[test]
fn output_through_link_to_dev_directory_judged_by_kernel_resolution() {
    let dir = temp_dir("devlink");
    let d = dir.join("d");
    std::os::unix::fs::symlink("/dev", &d).unwrap();
    let real = fs::canonicalize(&dir).unwrap();
    let rest = real.strip_prefix("/").unwrap();

    assert_eq!(ksq2txt_o(&d.join("null")).code, Some(1));
    assert!(std::os::unix::fs::FileTypeExt::is_char_device(
        &fs::metadata("/dev/null").unwrap().file_type()
    ));
    let r = ksq2txt_o(&d);
    assert_eq!(r.code, Some(1), "디렉터리 /dev는 실패해야 한다");

    let expected = melody_text();
    let out = dir.join("out.txt");
    fs::write(&out, "X".repeat(expected.len() + 50)).unwrap();
    let inode = fs::metadata(&out).unwrap().ino();
    let r = ksq2txt_o(&d.join("..").join(rest).join("out.txt"));
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert_eq!(fs::read_to_string(&out).unwrap(), expected);
    assert_ne!(fs::metadata(&out).unwrap().ino(), inode);

    let input = dir.join("in.txt");
    fs::write(&input, &expected).unwrap();
    let ksq_out = dir.join("out.ksq");
    fs::write(&ksq_out, " ".repeat(200_000) + "garbage").unwrap();
    let r = run(
        TXT2KSQ,
        &[
            input.as_os_str(),
            "-o".as_ref(),
            d.join("..").join(rest).join("out.ksq").as_os_str(),
        ],
    );
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(ksq::read_file(&ksq_out).is_ok());
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// macOS의 `/var` → `/private/var`, `/tmp` → `/private/tmp` 별칭. 어느 철자로 적어도 일반 파일은
/// 원자적으로 바뀐다. `/tmp/../dev/null`은 커널이 `/private/dev/null`로 풀어 없는 경로이고, 판정도
/// 커널을 따라 실패한다.
#[test]
fn ksq2txt_output_private_alias_spellings_replace_atomically() {
    let dir = temp_dir("alias");
    let real = fs::canonicalize(&dir).unwrap();
    let expected = melody_text();
    let spellings: Vec<PathBuf> = [real.clone(), dir.clone()]
        .into_iter()
        .chain(
            real.strip_prefix("/private")
                .ok()
                .map(|r| Path::new("/").join(r)),
        )
        .collect();
    for (i, base) in spellings.iter().enumerate() {
        let out = real.join(format!("o{i}.txt"));
        fs::write(&out, "X".repeat(expected.len() + 50)).unwrap();
        let inode = fs::metadata(&out).unwrap().ino();
        let r = ksq2txt_o(&base.join(format!("o{i}.txt")));
        assert_eq!(r.code, Some(0), "{}: {}", base.display(), r.stderr);
        assert_eq!(fs::read_to_string(&out).unwrap(), expected);
        assert_ne!(fs::metadata(&out).unwrap().ino(), inode);
    }
    if Path::new("/tmp").is_symlink() {
        let r = ksq2txt_o(Path::new("/tmp/../dev/null"));
        assert_eq!(r.code, Some(1), "{}", r.stderr);
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// 판정이 실패(정규화 불가)하면 원자적 쓰기로 가 그쪽 오류로 끝난다. 없는 부모, 권한 없는 부모,
/// 빈 경로, `.`, `..`로 끝나는 `/dev/fd/..` 모두 종료 1이고 임시 파일을 남기지 않는다.
#[test]
fn ksq2txt_output_unresolvable_paths_exit_1_without_leftovers() {
    let dir = temp_dir("bad");
    let locked = dir.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let cases: Vec<PathBuf> = vec![
        dir.join("missing/out.txt"),
        locked.join("out.txt"),
        locked.join("sub/out.txt"),
        PathBuf::new(),
        dir.join("."),
        PathBuf::from("/dev/fd/.."),
    ];
    for case in &cases {
        let r = ksq2txt_o(case);
        assert_eq!(r.code, Some(1), "{}: {}", case.display(), r.stderr);
    }
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(temp_leftovers(&dir).is_empty());
    assert!(fs::read_dir(&locked).unwrap().next().is_none());
    fs::remove_dir_all(&dir).unwrap();
}

/// 비 UTF-8 이름. APFS는 거부(EILSEQ)하고, 받는 파일 시스템이면 그 이름에 정확히 쓴다. 어느 쪽이든
/// 임시 파일을 남기지 않는다.
#[test]
fn ksq2txt_output_non_utf8_name_writes_or_fails_cleanly() {
    let dir = temp_dir("nonutf8");
    let out = dir.join(OsStr::from_bytes(b"\xff\xfe.txt"));

    let r = ksq2txt_o(&out);

    match r.code {
        Some(0) => assert_eq!(fs::read_to_string(&out).unwrap(), melody_text()),
        Some(1) => assert!(!out.exists()),
        other => panic!("{other:?}: {}", r.stderr),
    }
    assert!(temp_leftovers(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

/// `/dev/fd/1`이 파이프면 그 파이프로, 일반 파일(잘라 쓰지 않고 연 것)이면 `-o` 없이 표준 출력에 쓴
/// 것과 바이트가 같다. 셸의 `1<>`처럼 잘라 쓰지 않고 연 표준 출력에서는 둘 다 옛 꼬리를 남기는데,
/// 이것은 macOS fdesc가 `O_TRUNC`를 무시하는 것과 같은 동작이라 원자적 쓰기의 보장 밖이다.
///
/// 결정 `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`로 기대를 바꿨다: `-o`는 없는 경로와
/// 일반 파일만 받는다. 특수 파일·`/dev/stdout`·`/dev/fd/N`(과 그 링크)은 종료 1이고 대상은 그대로다.
/// 파이프든 일반 파일이든 `-o /dev/fd/1`은 아무것도 쓰지 않고, `-o` 없는 실행은 그대로 표준 출력에 쓴다.
#[test]
fn ksq2txt_output_dev_fd_1_matches_plain_stdout_for_pipe_and_file() {
    let dir = temp_dir("fd1");
    let input = fixture("01-melody.ksq");
    let expected = melody_text();

    let piped = run(
        KSQ2TXT,
        &[input.as_os_str(), "-o".as_ref(), "/dev/fd/1".as_ref()],
    );
    assert_eq!(piped.code, Some(1), "{}", piped.stderr);
    assert!(piped.stdout.is_empty());

    let mut results = Vec::new();
    for (name, args) in [
        (
            "o.txt",
            vec![input.as_os_str(), "-o".as_ref(), "/dev/fd/1".as_ref()],
        ),
        ("plain.txt", vec![input.as_os_str()]),
    ] {
        let path = dir.join(name);
        fs::write(&path, "Y".repeat(expected.len() + 30)).unwrap();
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let r = run_with(KSQ2TXT, &args, Stdio::from(file));
        let want = if name == "o.txt" { Some(1) } else { Some(0) };
        assert_eq!(r.code, want, "{name}: {}", r.stderr);
        results.push(fs::read(&path).unwrap());
    }
    assert_eq!(results[0], "Y".repeat(expected.len() + 30).into_bytes());
    assert!(results[1].starts_with(expected.as_bytes()));
    fs::remove_dir_all(&dir).unwrap();
}

// --- 거부 메시지의 안내값 ---

fn note(start: u64, length: u64, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        u32::try_from(length).unwrap(),
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn one_clip_project(start: u64, length: u64, notes: impl IntoIterator<Item = Note>) -> Project {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(n).unwrap();
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

/// 4분음표 트랙 1,450마디(기본 상한에서 거부). 안내값을 주면 표준 출력으로도 `-o`로도 변환되고,
/// 거부될 때 표준 출력에는 아무것도 나가지 않는다. 입력이 `.ksqz`여도 같다.
#[test]
fn ksq2txt_hint_value_converts_to_stdout_and_file_for_ksq_and_ksqz() {
    let dir = temp_dir("hint");
    let measures = 1450;
    let p = one_clip_project(
        0,
        measures * WHOLE,
        (0..measures * 4).map(|i| note(i * 960, 960, 60 + u8::try_from(i % 12).unwrap())),
    );
    for (name, compression) in [
        ("dense.ksq", Compression::None),
        ("dense.ksqz", Compression::Gzip),
    ] {
        let input = dir.join(name);
        fs::write(&input, ksq::to_bytes(&p, compression).unwrap()).unwrap();

        let refused = run(KSQ2TXT, &[input.as_os_str()]);
        assert_eq!(refused.code, Some(1), "{}", refused.stderr);
        assert!(refused.stdout.is_empty());
        let hint = hinted(&refused.stderr).unwrap_or_else(|| panic!("{}", refused.stderr));
        let hint_arg = hint.to_string();

        let to_stdout = run(
            KSQ2TXT,
            &[
                input.as_os_str(),
                "--max-output".as_ref(),
                hint_arg.as_ref(),
            ],
        );
        assert_eq!(to_stdout.code, Some(0), "{}", to_stdout.stderr);
        assert!(to_stdout.stdout.len() as u64 <= hint);
        assert!(to_stdout.stdout.starts_with(b"@tempo"));

        let out = dir.join(format!("{name}.txt"));
        let to_file = run(
            KSQ2TXT,
            &[
                input.as_os_str(),
                "--max-output".as_ref(),
                hint_arg.as_ref(),
                "-o".as_ref(),
                out.as_os_str(),
            ],
        );
        assert_eq!(to_file.code, Some(0), "{}", to_file.stderr);
        assert_eq!(fs::read(&out).unwrap(), to_stdout.stdout);
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// 어림(u128)이 u64를 넘는 곡: u64 끝 근처의 클립에 노트 1,000개. 어림은 `u64::MAX`로 잘리고, 안내는
/// `--max-output 18446744073709551615 이상을 주면 변환한다`인데 그 값을 주어도 어림이 다시 넘어 거부된다.
/// 안내는 따라 하면 통과하는 값이거나, 그런 값이 없으면 약속하지 않아야 한다.
#[test]
fn ksq2txt_hint_beyond_u64_estimate_is_not_promised_or_converts() {
    let dir = temp_dir("hint-u64");
    let length = 2_000_000;
    let p = one_clip_project(
        u64::MAX - length,
        length,
        (0..1000).map(|i| note(i * 1920, 960, 30 + u8::try_from(i % 60).unwrap())),
    );
    let input = dir.join("far.ksq");
    fs::write(&input, ksq::to_bytes(&p, Compression::None).unwrap()).unwrap();

    let refused = run(KSQ2TXT, &[input.as_os_str()]);
    assert_eq!(refused.code, Some(1), "{}", refused.stderr);

    if let Some(hint) = hinted(&refused.stderr) {
        let again = run(
            KSQ2TXT,
            &[
                input.as_os_str(),
                "--max-output".as_ref(),
                hint.to_string().as_ref(),
                "-o".as_ref(),
                "/dev/null".as_ref(),
            ],
        );
        assert_eq!(
            again.code,
            Some(0),
            "안내값 {hint}로도 거부된다: {}",
            again.stderr
        );
    }
    fs::remove_dir_all(&dir).unwrap();
}
