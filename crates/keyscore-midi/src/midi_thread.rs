//! CoreMIDI 클라이언트를 만드는 전용 스레드.
//!
//! `MIDIServices.h`: 알림 함수는 프로세스에서 `MIDIClientCreate`가 처음 불렸을 때의 run loop에서 불린다.
//! 그래서 이 crate의 모든 클라이언트 생성·해제를 run loop을 계속 돌리는 이 스레드에서 한다. 그러면 알림이
//! 어느 스레드에서 처음 클라이언트를 만들었는지와 상관없이 이 스레드로 온다(S1-T2). 스레드는 프로세스가
//! 끝날 때까지 산다.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};

use crate::MidiError;

type Job = Box<dyn FnOnce() + Send>;

/// run loop을 한 번에 돌리는 시간. 작업이 들어와 처리되기까지 걸리는 최대 지연이기도 하다.
const RUN_SLICE: Duration = Duration::from_millis(20);

const THREAD_NAME: &str = "keyscore-coremidi";

// 스레드를 만들지 못했으면 `None`으로 남아, 이후 호출은 모두 오류다.
static JOBS: OnceLock<Option<Sender<Job>>> = OnceLock::new();

fn jobs() -> Option<&'static Sender<Job>> {
    JOBS.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || run(&rx))
            .ok()
            .map(|_| tx)
    })
    .as_ref()
}

fn thread_gone() -> MidiError {
    MidiError::Os {
        call: "CoreMIDI thread",
        status: -1,
    }
}

#[allow(unsafe_code)]
fn run(rx: &Receiver<Job>) {
    loop {
        while let Ok(job) = rx.try_recv() {
            // 작업 하나의 패닉이 이 스레드를 끝내면 이후 모든 CoreMIDI 작업이 막힌다. 패닉한 작업의
            // 호출자는 결과 채널이 닫혀 오류를 받는다.
            let _ = catch_unwind(AssertUnwindSafe(job));
        }
        // SAFETY: `kCFRunLoopDefaultMode`는 CoreFoundation이 제공하는 불변 상수다.
        let mode = unsafe { kCFRunLoopDefaultMode };
        CFRunLoop::run_in_mode(mode, RUN_SLICE, false);
    }
}

/// `f`를 CoreMIDI 스레드에서 돌리고 결과를 기다린다. 그 스레드 안에서 부르면 바로 돌린다.
///
/// # Errors
///
/// 스레드를 만들지 못했거나 작업이 돌지 않았으면 `Os`.
pub(crate) fn on_midi_thread<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, MidiError> {
    if std::thread::current().name() == Some(THREAD_NAME) {
        return Ok(f());
    }
    let (tx, rx) = mpsc::channel();
    let job: Job = Box::new(move || {
        let _ = tx.send(f());
    });
    jobs()
        .ok_or_else(thread_gone)?
        .send(job)
        .map_err(|_| thread_gone())?;
    rx.recv().map_err(|_| thread_gone())
}
