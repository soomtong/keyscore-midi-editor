//! 바이너리의 인자 해석과 출력. 인자 파서 crate를 더하지 않으려고 std만 쓴다.
//!
//! 종료 코드: 0 성공, 1 실패(검사 실패·변환 오류·입출력 오류), 2 잘못된 사용법.
//! 결과는 표준 출력에, 경고·손실·오류는 표준 오류에 쓴다. 그래서 `ksq2txt a.ksq > a.txt`처럼
//! 파이프로 받아도 결과에 경고가 섞이지 않는다.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use keyscore_core::ksq::{self, Compression};
use keyscore_text::Mode;

use crate::{CliError, Loss, check, project_to_text, text_to_project, write_fixtures};

const OK: u8 = 0;
const FAILED: u8 = 1;
const USAGE: u8 = 2;

const USAGE_KSQ: &str = "사용법: ksq check <파일.ksq|.ksqz>...\n       ksq fixtures <디렉터리>";
const USAGE_KSQ2TXT: &str = "사용법: ksq2txt <입력.ksq> [-o 출력.txt]";
const USAGE_TXT2KSQ: &str = "사용법: txt2ksq <입력.txt> [-o 출력.ksq|.ksqz] [--strict]";

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
    input: PathBuf,
    output: Option<PathBuf>,
    strict: bool,
}

fn parse_options(args: &[OsString], allow_strict: bool) -> Result<Options, String> {
    let mut input = None;
    let mut output = None;
    let mut strict = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "-o" {
            let Some(path) = rest.next() else {
                return Err("-o 뒤에 출력 경로가 없다".to_owned());
            };
            if output.replace(PathBuf::from(path)).is_some() {
                return Err("-o가 두 번 나왔다".to_owned());
            }
        } else if allow_strict && arg == "--strict" {
            strict = true;
        } else if is_option(arg) {
            return Err(format!("모르는 옵션: {}", arg.to_string_lossy()));
        } else if input.replace(PathBuf::from(arg)).is_some() {
            return Err("입력 파일은 하나만 받는다".to_owned());
        }
    }
    let input = input.ok_or_else(|| "입력 파일이 없다".to_owned())?;
    Ok(Options {
        input,
        output,
        strict,
    })
}

/// `-`로 시작하는 인자는 옵션으로 본다. `-`로 시작하는 파일은 `./-x`처럼 쓴다.
fn is_option(arg: &OsStr) -> bool {
    arg.as_encoded_bytes().first() == Some(&b'-') && arg.len() > 1
}

/// 두 경로가 같은 파일인가. 심볼릭 링크·`.`·하드 링크로 철자가 달라도 같은 파일이면 참이다.
/// 출력이 아직 없으면 거짓이다.
fn same_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (fs::metadata(a), fs::metadata(b)) {
            (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
    }
}

/// 출력이 입력과 같은 파일이면 거부한다. 변환은 정보를 잃을 수 있어 입력을 덮어쓰면 되돌릴 수 없다.
fn guard_output(options: &Options) -> Result<(), CliError> {
    match &options.output {
        Some(output) if same_file(&options.input, output) => Err(CliError::SameInputOutput {
            path: output.clone(),
        }),
        _ => Ok(()),
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
    let options = match parse_options(args, false) {
        Ok(o) => o,
        Err(message) => return usage(err, &message, USAGE_KSQ2TXT),
    };
    match run_ksq2txt(&options, out, err) {
        Ok(()) => OK,
        Err(e) => {
            say!(err, "실패: {e}");
            FAILED
        }
    }
}

fn run_ksq2txt(
    options: &Options,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    guard_output(options)?;
    let input = &options.input;
    let (project, warnings) = ksq::read_file(input).map_err(|source| CliError::Ksq {
        path: input.clone(),
        source,
    })?;
    for w in warnings {
        say!(err, "{}: 경고: {w}", input.display());
    }
    let (text, losses) = project_to_text(&project)?;
    for loss in &losses {
        say!(err, "{}: 손실: {loss}", input.display());
    }
    refuse_mismatch(&losses)?;
    match &options.output {
        Some(path) => fs::write(path, text).map_err(|source| CliError::Io {
            path: path.clone(),
            source,
        }),
        None => write_stdout(out, text.as_bytes()),
    }
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
    let options = match parse_options(args, true) {
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
    guard_output(options)?;
    let input = &options.input;
    let text = fs::read_to_string(input).map_err(|source| CliError::Io {
        path: input.clone(),
        source,
    })?;
    let mode = if options.strict {
        Mode::Strict
    } else {
        Mode::Lenient
    };
    // 텍스트 오류는 경로가 없는 CliError::Parse라서, 어느 파일인지 여기서 붙인다.
    let (project, diagnostics) = text_to_project(&text, mode).map_err(|e| match e {
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
            ksq::write_file(path, &project, compression).map_err(|source| CliError::Ksq {
                path: path.clone(),
                source,
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
