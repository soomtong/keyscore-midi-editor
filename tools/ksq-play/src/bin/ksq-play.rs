//! `ksq-play` 바이너리. 동작은 [`ksq_play::run`]에 있다.

use std::io::BufRead;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (tx, rx) = mpsc::channel();
    // 표준 입력의 줄과 Ctrl-C를 같은 명령 채널로 넘긴다. 재생 쪽은 채널만 본다.
    let stdin_tx = tx.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if stdin_tx.send(line).is_err() {
                break;
            }
        }
    });
    // 처리기를 못 달면 Ctrl-C가 기본 동작(즉시 종료)이 된다. 재생은 그래도 된다. 두 번째 Ctrl-C는
    // 정리를 기다리지 않고 바로 끝낸다(정리가 멈췄을 때의 출구).
    let pressed = AtomicBool::new(false);
    let _ = ctrlc::set_handler(move || {
        if pressed.swap(true, Ordering::SeqCst) {
            std::process::exit(130);
        }
        let _ = tx.send(ksq_play::INTERRUPT.to_string());
    });
    let code = ksq_play::run(
        &args,
        &rx,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(code)
}
