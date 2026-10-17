//! 바이너리의 인자 해석과 출력. 인자 파서 crate를 더하지 않으려고 std만 쓴다.
//!
//! 종료 코드: 0 성공, 1 실패(검사 실패·변환 오류·입출력 오류), 2 잘못된 사용법.
//! 결과는 표준 출력에, 경고·손실·오류는 표준 오류에 쓴다. 그래서 `ksq2txt a.ksq > a.txt`처럼
//! 파이프로 받아도 결과에 경고가 섞이지 않는다.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use keyscore_core::ksq::{self, Compression, KsqError};
use keyscore_text::Mode;

use crate::{
    CliError, Loss, check, project_to_text, project_to_text_limited, text_to_project_with,
    write_fixtures,
};

const OK: u8 = 0;
const FAILED: u8 = 1;
const USAGE: u8 = 2;

const USAGE_KSQ: &str = "사용법: ksq check <파일.ksq|.ksqz>...\n       ksq fixtures <디렉터리>";
const USAGE_KSQ2TXT: &str = "사용법: ksq2txt <입력.ksq> [-o 출력.txt] [--max-output <바이트>]";
const USAGE_TXT2KSQ: &str = "사용법: txt2ksq <입력.txt> [-o 출력.ksq|.ksqz] [--strict] [--legacy]\n  --legacy  `#`로 시작하는 줄을 주석으로 읽는다(옛 기록)";

// 표준 오류에 쓰지 못하면 알릴 곳이 없으므로 쓰기 실패는 무시한다(`eprintln!`과 같은 태도이되
// 패닉하지 않는다). 표준 출력의 결과 쓰기는 따로 검사한다.
macro_rules! say {
    ($w:expr, $($arg:tt)*) => {
        let _ = writeln!($w, $($arg)*);
    };
}

fn usage(err: &mut dyn Write, message: &str, usage: &str) -> u8 {
    say!(err, "{message}\n{usage}");
    USAGE
}

fn wants_help(args: &[OsString]) -> bool {
    args.iter().any(|a| a == "-h" || a == "--help")
}

/// `ksq check <파일>...` · `ksq fixtures <디렉터리>`.
pub fn ksq(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if wants_help(args) {
        say!(out, "{USAGE_KSQ}");
        return OK;
    }
    match args.split_first() {
        Some((command, files)) if command == "check" && !files.is_empty() => {
            check_files(files, out, err)
        }
        Some((command, [dir])) if command == "fixtures" => match write_fixtures(Path::new(dir)) {
            Ok(paths) => {
                for p in paths {
                    say!(out, "{}", p.display());
                }
                OK
            }
            Err(e) => {
                say!(err, "{e}");
                FAILED
            }
        },
        Some((command, _)) if command == "check" || command == "fixtures" => {
            usage(err, "인자 수가 맞지 않는다", USAGE_KSQ)
        }
        Some((command, _)) => usage(
            err,
            &format!("모르는 명령: {}", command.to_string_lossy()),
            USAGE_KSQ,
        ),
        None => usage(err, "명령이 없다", USAGE_KSQ),
    }
}

/// 파일마다 따로 검사하고 모두 보고한다. 하나라도 실패하면 실패다.
fn check_files(files: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut code = OK;
    for file in files {
        let path = Path::new(file);
        match check(path) {
            Ok(report) => {
                for w in report.warnings() {
                    say!(err, "{}: 경고: {w}", path.display());
                }
                let version = report
                    .version()
                    .map_or_else(|| "없음".to_owned(), |v| v.to_string());
                say!(
                    out,
                    "ok {} (형식 버전 {version}, 트랙 {}, 클립 {}, 노트 {})",
                    path.display(),
                    report.tracks(),
                    report.clips(),
                    report.notes()
                );
            }
            Err(e) => {
                say!(err, "실패: {e}");
                code = FAILED;
            }
        }
    }
    code
}

struct Options {
    tool: Tool,
    input: PathBuf,
    output: Option<PathBuf>,
    strict: bool,
    /// 레거시 `#` 주석 줄(결정 Q1 2차 개정).
    legacy: bool,
    max_output: Option<u64>,
}

/// 옵션을 받는 바이너리. 바이너리마다 받는 옵션이 다르다.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Ksq2Txt,
    Txt2Ksq,
}

impl Tool {
    /// `-o` 대신 셸 리다이렉션을 쓰는 예. 오류 메시지가 그 바이너리에 맞는 예를 보이게 한다
    /// (S0-T8 리뷰 9 제안 3).
    const fn redirect_example(self) -> &'static str {
        match self {
            Tool::Ksq2Txt => "ksq2txt in.ksq > out.txt",
            Tool::Txt2Ksq => "txt2ksq in.txt > out.ksq",
        }
    }
}

fn parse_options(args: &[OsString], tool: Tool) -> Result<Options, String> {
    let mut input = None;
    let mut output = None;
    let mut strict = false;
    let mut legacy = false;
    let mut max_output = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if tool == Tool::Ksq2Txt && arg == "--max-output" {
            let value = rest
                .next()
                .ok_or_else(|| "--max-output 뒤에 바이트 수가 없다".to_owned())?;
            // 0은 「상한 없음」으로 읽힐 수 있어 받지 않는다. 상한을 사실상 없애려면 큰 수를 준다.
            let bytes = value
                .to_str()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|&b| b > 0)
                .ok_or_else(|| {
                    format!(
                        "--max-output은 1 이상의 바이트 수여야 한다: {}",
                        value.to_string_lossy()
                    )
                })?;
            if max_output.replace(bytes).is_some() {
                return Err("--max-output이 두 번 나왔다".to_owned());
            }
            continue;
        }
        if arg == "-o" {
            let Some(path) = rest.next() else {
                return Err("-o 뒤에 출력 경로가 없다".to_owned());
            };
            if output.replace(PathBuf::from(path)).is_some() {
                return Err("-o가 두 번 나왔다".to_owned());
            }
        } else if tool == Tool::Txt2Ksq && arg == "--strict" {
            strict = true;
        } else if tool == Tool::Txt2Ksq && arg == "--legacy" {
            legacy = true;
        } else if is_option(arg) {
            return Err(format!("모르는 옵션: {}", arg.to_string_lossy()));
        } else if input.replace(PathBuf::from(arg)).is_some() {
            return Err("입력 파일은 하나만 받는다".to_owned());
        }
    }
    let input = input.ok_or_else(|| "입력 파일이 없다".to_owned())?;
    Ok(Options {
        tool,
        input,
        output,
        strict,
        legacy,
        max_output,
    })
}

/// `-`로 시작하는 인자는 옵션으로 본다. `-`로 시작하는 파일은 `./-x`처럼 쓴다.
fn is_option(arg: &OsStr) -> bool {
    arg.as_encoded_bytes().first() == Some(&b'-') && arg.len() > 1
}

/// 입력을 연다. 같은 파일 판정에 쓸 신원(dev, ino)은 연 핸들의 `fstat`으로 얻는다. 경로를
/// `stat`하면 `/dev/stdin`·`/dev/fd/N`이 `/dev`의 장치 번호로 보여, 그 fd가 출력 파일을 가리켜도
/// 다른 파일로 판정해 입력을 덮었다(S0-T8 리뷰 9 제안 4).
fn open_input(path: &Path) -> Result<(fs::File, (u64, u64)), CliError> {
    use std::os::unix::fs::MetadataExt;

    let io = |source| CliError::Io {
        path: path.to_owned(),
        source,
    };
    let file = fs::File::open(path).map_err(io)?;
    let meta = file.metadata().map_err(io)?;
    Ok((file, (meta.dev(), meta.ino())))
}

/// `-o`를 쓰기 전에 거른다(경로만 본다). 입력을 열기 전에, 변환하기 전에 하므로 거부할 때 아무것도
/// 하지 않는다. 입력이 쓰는 쪽 없는 FIFO여도 열기에서 멈추지 않고 곧바로 거부한다(S0-T8 리뷰 10
/// 지적 1).
///
/// 대상은 「없는 경로」와 「일반 파일」만 받는다(`docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`).
/// 둘 다 원자적 쓰기(임시 파일 → fsync → 이름 바꾸기)로 쓴다. 디렉터리·FIFO·소켓·장치
/// (`/dev/null`, `/dev/stdout`, `/dev/fd/N`)와 `NotFound`가 아닌 확인 오류(링크 한도를 넘는 사슬의
/// ELOOP, EACCES, 끝 `/`의 ENOTDIR 등)는 대상을 건드리지 않고 실패한다. 특수 파일에 쓰려면 `-o`
/// 없이 셸 리다이렉션을 쓴다. 커널의 경로 풀이를 도구가 다시 흉내 내지 않으려는 것이다(S0-T8 리뷰
/// 2–8). 확인은 링크를 따라가는 `fs::metadata` 하나로, 커널이 푼다.
///
/// 대상이 없었으면(`NotFound`) `true`를 돌려준다. 그런 대상을 만들지 못한 실패를 따로 알리려는
/// 것이다([`output_error`]).
fn guard_output(options: &Options) -> Result<bool, CliError> {
    let Some(output) = &options.output else {
        return Ok(false);
    };
    let example = options.tool.redirect_example();
    match fs::metadata(output) {
        Ok(meta) if meta.is_file() && !on_devfs(&meta) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        // 일반 파일이 아닌 대상이 입력 경로와 같은 파일이면(입력 디렉터리를 `-o`로 준 경우 등) 더
        // 정확한 「입력과 같다」로 알린다. 입력은 열지 않고 경로만 `stat`한다(FIFO도 막히지 않는다).
        Ok(meta) if same_identity(&meta, &options.input) => Err(CliError::SameInputOutput {
            path: output.clone(),
        }),
        Ok(meta) => Err(CliError::OutputNotRegular {
            path: output.clone(),
            kind: file_kind(&meta),
            example,
        }),
        Err(source) => Err(CliError::OutputUnchecked {
            path: output.clone(),
            source,
            example,
        }),
    }
}

/// `meta`가 `path`(링크를 따라간 `stat`)와 같은 파일인가.
fn same_identity(meta: &fs::Metadata, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path).is_ok_and(|m| (m.dev(), m.ino()) == (meta.dev(), meta.ino()))
}

/// 입력과 같은 파일이면 거부한다. 변환은 정보를 잃을 수 있어 입력을 덮어쓰면 되돌릴 수 없다.
/// 입력은 연 핸들의 신원(`input`, [`open_input`])과 출력 대상의 신원(링크를 따라간 `stat`)을
/// 비교한다. 심볼릭 링크·하드 링크·`/dev/stdin`으로 철자가 달라도 같은 파일이면 거부한다. 범위
/// 판정([`guard_output`])을 통과한 일반 파일만 여기 온다.
fn refuse_same_file(options: &Options, input: (u64, u64)) -> Result<(), CliError> {
    use std::os::unix::fs::MetadataExt;

    match &options.output {
        Some(output) if fs::metadata(output).is_ok_and(|m| (m.dev(), m.ino()) == input) => {
            Err(CliError::SameInputOutput {
                path: output.clone(),
            })
        }
        _ => Ok(()),
    }
}

/// `-o` 쓰기 실패. 확인 때 없던 대상을 만들지 못했으면 원인에 따라 다르게 알린다(계획자 결정 A,
/// S0-T8 리뷰 10 제안 2). 경로는 이 오류가 한 번만 적는다.
///
/// - 특수 파일 자리(경로가 `/dev/` 아래이거나 끝이 `/`·`/.`이고, 커널 오류가 ENOENT·ENOTDIR·EPERM):
///   다른 거부와 같은 「특수 파일에 쓰려면 … 리다이렉트하라」 안내([`CliError::OutputUncreatable`]).
/// - 그 밖(권한 없음, 없는 디렉터리 안 등): 셸 리다이렉션도 같은 이유로 실패할 수 있으므로 특수
///   파일 안내 대신 다른 경로나 표준 출력을 권한다([`CliError::OutputCreateFailed`]).
///
/// 경로 모양은 메시지를 고르는 데만 쓴다. 쓰기 동작은 바꾸지 않는다.
fn output_error(options: &Options, path: &Path, was_missing: bool, cause: CliError) -> CliError {
    use std::io::ErrorKind;

    if !was_missing {
        return cause;
    }
    let io = match &cause {
        CliError::Io { source, .. }
        | CliError::Ksq {
            source: KsqError::Io(source),
            ..
        } => Some(source),
        _ => None,
    };
    let special_kind = io.is_some_and(|e| {
        matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
            || e.raw_os_error() == Some(1)
    });
    let bytes = path.as_os_str().as_encoded_bytes();
    let special_place = path.starts_with("/dev") || bytes.ends_with(b"/") || bytes.ends_with(b"/.");
    let detail = match cause {
        CliError::Io { source, .. } => source.to_string(),
        CliError::Ksq { source, .. } => source.to_string(),
        other => other.to_string(),
    };
    let example = options.tool.redirect_example();
    let path = path.to_owned();
    if special_kind && special_place {
        CliError::OutputUncreatable {
            path,
            detail,
            example,
        }
    } else {
        CliError::OutputCreateFailed {
            path,
            detail,
            example,
        }
    }
}

/// `/dev`와 같은 장치 번호로 보고되는 파일인가. `/dev/stdout`·`/dev/fd/N`은 표준 출력 등이 일반
/// 파일이면 `stat`에 일반 파일로 나오지만 장치 번호는 `/dev`(devfs·fdesc)의 것이다. 경로를 분류하지
/// 않고 커널이 돌려준 장치 번호 하나로 거른다. 그대로 원자적 쓰기로 가면 `/dev/fd`에 임시 파일을
/// 만들다 엉뚱한 ENOENT로 실패한다.
fn on_devfs(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    fs::metadata("/dev").is_ok_and(|d| d.dev() == meta.dev())
}

/// 오류 메시지에 쓸 파일 종류. 디렉터리는 커널 문구(EISDIR)도 함께 적는다.
fn file_kind(meta: &fs::Metadata) -> &'static str {
    use std::os::unix::fs::FileTypeExt;

    let t = meta.file_type();
    if on_devfs(meta) && t.is_file() {
        "/dev/fd 등 장치 파일 시스템의 파일"
    } else if t.is_dir() {
        "디렉터리, Is a directory"
    } else if t.is_fifo() {
        "FIFO"
    } else if t.is_socket() {
        "소켓"
    } else if t.is_char_device() {
        "문자 장치"
    } else if t.is_block_device() {
        "블록 장치"
    } else {
        "일반 파일이 아닌 것"
    }
}

fn write_stdout(out: &mut dyn Write, bytes: &[u8]) -> Result<(), CliError> {
    out.write_all(bytes)
        .and_then(|()| out.flush())
        .map_err(|source| CliError::Io {
            path: PathBuf::from("<표준 출력>"),
            source,
        })
}

/// `ksq2txt <in.ksq> [-o out.txt]`.
pub fn ksq2txt(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if wants_help(args) {
        say!(out, "{USAGE_KSQ2TXT}");
        return OK;
    }
    let options = match parse_options(args, Tool::Ksq2Txt) {
        Ok(o) => o,
        Err(message) => return usage(err, &message, USAGE_KSQ2TXT),
    };
    match run_ksq2txt(&options, out, err) {
        Ok(()) => OK,
        Err(e) => {
            say!(err, "실패: {e}");
            // 어림(또는 안전망의 실제 크기)은 이 실행이 크기 검사를 통과하는 가장 작은 상한이다.
            // 고정 예시는 정작 거부되는 긴 곡의 어림보다 작아 따라 해도 다시 거부된다(S0-T8 리뷰 4
            // 제안 2). 「변환한다」고 약속하지 않는 것은 상한을 크게 주면 정말 큰 텍스트를 만들기
            // 때문이다(S0-T8 리뷰 6 제안 2).
            if let CliError::OutputTooLarge { estimated, .. } = e {
                say!(
                    err,
                    "`--max-output {estimated}` 이상을 주면 크기 검사를 통과한다(실제 텍스트는 대개 이보다 훨씬 작다)."
                );
            }
            FAILED
        }
    }
}

fn run_ksq2txt(
    options: &Options,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let input = &options.input;
    let was_missing = guard_output(options)?;
    let (mut file, identity) = open_input(input)?;
    refuse_same_file(options, identity)?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut bytes).map_err(|source| CliError::Io {
        path: input.clone(),
        source,
    })?;
    let (project, warnings) = ksq::from_bytes(&bytes).map_err(|source| CliError::Ksq {
        path: input.clone(),
        source,
    })?;
    for w in warnings {
        say!(err, "{}: 경고: {w}", input.display());
    }
    // 기본 상한은 텍스트가 나르는 내용으로 다시 만든 프로젝트만 기준으로 한다. 파일 바이트를 기준에
    // 넣으면 잘 압축되는 평범한 `.ksqz`가 거부된다(S0-T8 리뷰 3 지적 1). 다시 만든 프로젝트는 압축과
    // 무관하므로 압축으로 부풀릴 수도 없다. 사용자는 `--max-output`으로 상한을 바꿀 수 있다.
    let (text, losses) = match options.max_output {
        Some(bytes) => project_to_text_limited(&project, bytes)?,
        None => project_to_text(&project)?,
    };
    for loss in &losses {
        say!(err, "{}: 손실: {loss}", input.display());
    }
    refuse_mismatch(&losses)?;
    match &options.output {
        Some(path) => write_atomic(path, text.as_bytes()).map_err(|source| {
            output_error(
                options,
                path,
                was_missing,
                CliError::Io {
                    path: path.clone(),
                    source,
                },
            )
        }),
        None => write_stdout(out, text.as_bytes()),
    }
}

/// 같은 디렉터리의 임시 파일에 쓰고 fsync한 뒤 이름을 바꾼다. 쓰다 실패해도 기존 출력 파일이
/// 잘린 채 남지 않는다. `txt2ksq`가 쓰는 `ksq::write_file`과 같은 보장이다(S0-T8 리뷰 1 지적 3).
/// 출력이 심볼릭 링크면 링크를 일반 파일로 바꾸지 않고 대상 파일을 바꾸며, 기존 파일의 권한을
/// 유지한다.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let path = resolve_symlink(path)?;
    let temp = temp_path(&path);
    let result = write_then_rename(&temp, &path, bytes);
    if result.is_err() {
        // 지우기 실패가 원래 오류를 가리지 않게 무시한다.
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_then_rename(temp: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let original = fs::metadata(path).ok();
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    // 처음부터 원본 권한으로 만들어, 쓰는 동안 원본보다 넓은 권한으로 열려 있지 않게 한다.
    #[cfg(unix)]
    if let Some(meta) = &original {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(meta.permissions().mode() & 0o777);
    }
    let mut file = options.open(temp)?;
    // umask가 만들 때의 권한을 좁혔을 수 있어 원본 권한을 정확히 다시 맞춘다.
    if let Some(meta) = original {
        file.set_permissions(meta.permissions())?;
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)?;
    // 이름 바꾸기를 디스크에 남긴다. 디렉터리 fsync가 안 되는 파일 시스템도 있어 실패는 무시한다.
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let _ = fs::File::open(dir).and_then(|d| d.sync_all());
    Ok(())
}

/// 심볼릭 링크를 따라간 실제 경로. 대상이 없는 링크는 링크가 가리키는 경로에 새로 만든다.
fn resolve_symlink(path: &Path) -> std::io::Result<PathBuf> {
    let mut current = path.to_path_buf();
    // 순환 링크로 끝없이 돌지 않게 횟수를 제한한다.
    for _ in 0..40 {
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = fs::read_link(&current)?;
                current = match current.parent() {
                    Some(parent) if target.is_relative() => parent.join(target),
                    _ => target,
                };
            }
            _ => return Ok(current),
        }
    }
    Err(std::io::Error::other("심볼릭 링크가 너무 깊거나 순환한다"))
}

/// 같은 디렉터리의 임시 파일 경로. 다른 디렉터리면 rename이 원자적이지 않다.
fn temp_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map_or_else(|| "txt".into(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.tmp-{}-{n}", std::process::id()))
}

/// 되읽은 결과가 손실로 설명되지 않게 다르면(keyscore-text 버그) 틀린 텍스트를 쓰지 않는다.
/// 손실 목록은 그 전에 표준 오류에 찍혀 무엇이 달랐는지 보인다(S0-T8 리뷰 1 지적 2).
fn refuse_mismatch(losses: &[Loss]) -> Result<(), CliError> {
    let count = losses
        .iter()
        .filter(|l| matches!(l, Loss::Mismatch { .. }))
        .count();
    if count == 0 {
        Ok(())
    } else {
        Err(CliError::ExportMismatch { count })
    }
}

/// `txt2ksq <in.txt> [-o out.ksq] [--strict]`.
pub fn txt2ksq(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if wants_help(args) {
        say!(out, "{USAGE_TXT2KSQ}");
        return OK;
    }
    let options = match parse_options(args, Tool::Txt2Ksq) {
        Ok(o) => o,
        Err(message) => return usage(err, &message, USAGE_TXT2KSQ),
    };
    match run_txt2ksq(&options, out, err) {
        Ok(()) => OK,
        Err(e) => {
            say!(err, "실패: {e}");
            FAILED
        }
    }
}

fn run_txt2ksq(
    options: &Options,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let input = &options.input;
    let was_missing = guard_output(options)?;
    let (mut file, identity) = open_input(input)?;
    refuse_same_file(options, identity)?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).map_err(|source| CliError::Io {
        path: input.clone(),
        source,
    })?;
    // 메모장류 편집기가 맨 앞에 붙이는 UTF-8 BOM 하나를 뗀다. 텍스트 문법이 아니라 파일 인코딩의
    // 표지라서 파일을 읽는 이 자리에서 다룬다(S0-T8 리뷰 1 지적 4). 편집기가 BOM을 보이지 않으므로
    // 열 번호도 BOM을 빼고 센다.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mode = if options.strict {
        Mode::Strict
    } else {
        Mode::Lenient
    };
    // 텍스트 오류는 경로가 없는 CliError::Parse라서, 어느 파일인지 여기서 붙인다.
    let parse_options = keyscore_text::ParseOptions {
        legacy_hash_comments: options.legacy,
    };
    let (project, diagnostics) =
        text_to_project_with(text, mode, parse_options).map_err(|e| match e {
            CliError::Parse(source) => CliError::TextFile {
                path: input.clone(),
                source,
            },
            other => other,
        })?;
    for d in diagnostics {
        say!(err, "{}: 경고: {d}", input.display());
    }
    match &options.output {
        Some(path) => {
            let gzip = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("ksqz"));
            let compression = if gzip {
                Compression::Gzip
            } else {
                Compression::None
            };
            ksq::write_file(path, &project, compression).map_err(|source| {
                output_error(
                    options,
                    path,
                    was_missing,
                    CliError::Ksq {
                        path: path.clone(),
                        source,
                    },
                )
            })
        }
        None => write_stdout(out, ksq::to_string(&project).as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuse_mismatch_with_mismatch_loss_is_error() {
        let losses = vec![
            Loss::TempoChanges { dropped: 1 },
            Loss::Mismatch {
                detail: "노트".to_owned(),
            },
        ];

        let result = refuse_mismatch(&losses);

        assert!(matches!(result, Err(CliError::ExportMismatch { count: 1 })));
    }

    #[test]
    fn refuse_mismatch_with_ordinary_losses_is_ok() {
        let losses = vec![Loss::TempoChanges { dropped: 1 }];

        assert!(refuse_mismatch(&losses).is_ok());
        assert!(refuse_mismatch(&[]).is_ok());
    }
}
