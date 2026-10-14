//! `ksq` 바이너리. 동작은 [`ksq_cli::cli::ksq`]에 있다.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let code = ksq_cli::cli::ksq(
        &args,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(code)
}
