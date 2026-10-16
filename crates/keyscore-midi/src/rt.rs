//! 실시간 스레드 지원: 호스트 시각 비율, 시각까지 기다리기, time-constraint 우선순위(REQ 9.2).
//!
//! libc 의존을 더하지 않으려고 필요한 mach·pthread 함수만 직접 선언한다. 시그니처는
//! `<mach/mach_time.h>`, `<mach/thread_policy.h>`, `<pthread.h>`와 같다.

use std::ffi::c_void;
use std::time::Duration;

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

/// `THREAD_TIME_CONSTRAINT_POLICY`의 정책 값(단위: 호스트 틱).
#[repr(C)]
struct TimeConstraintPolicy {
    period: u32,
    computation: u32,
    constraint: u32,
    preemptible: i32,
}

const THREAD_TIME_CONSTRAINT_POLICY: i32 = 2;
const THREAD_TIME_CONSTRAINT_POLICY_COUNT: u32 = 4;

#[allow(unsafe_code)]
unsafe extern "C" {
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
    // 인자로 받은 호스트 시각까지 이 스레드를 재운다. 메모리를 건드리지 않는다.
    safe fn mach_wait_until(deadline: u64) -> i32;
    fn pthread_self() -> *mut c_void;
    fn pthread_mach_thread_np(thread: *mut c_void) -> u32;
    fn thread_policy_set(thread: u32, flavor: i32, policy: *mut i32, count: u32) -> i32;
}

/// 호스트 틱과 ns의 비율: `틱 × numer / denom = ns`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timebase {
    pub(crate) numer: u32,
    pub(crate) denom: u32,
}

impl Timebase {
    #[allow(unsafe_code)]
    pub(crate) fn get() -> Self {
        let mut info = MachTimebaseInfo { numer: 1, denom: 1 };
        // SAFETY: 출력 포인터는 이 스택 변수를 가리킨다. 실패하면 1/1(ns 그대로)로 둔다.
        let status = unsafe { mach_timebase_info(&raw mut info) };
        if status != 0 || info.numer == 0 || info.denom == 0 {
            return Timebase { numer: 1, denom: 1 };
        }
        Timebase {
            numer: info.numer,
            denom: info.denom,
        }
    }

    /// 기간을 호스트 틱으로(내림). 너무 크면 `u64::MAX`.
    pub(crate) fn ticks(self, d: Duration) -> u64 {
        crate::scheduler::nanos_to_host_ticks(d.as_nanos(), self.numer, self.denom)
    }
}

/// 호스트 시각 `deadline`까지 기다린다. 스케줄러 루프의 유일한 비송신 시스템 콜이다(계획 참고).
pub(crate) fn wait_until(deadline: u64) {
    let _ = mach_wait_until(deadline);
}

/// 지금 스레드에 time-constraint 실시간 우선순위를 준다. 성공하면 `true`.
#[allow(unsafe_code)]
pub(crate) fn make_realtime(timebase: Timebase, period: Duration) -> bool {
    let to_u32 = |d: Duration| u32::try_from(timebase.ticks(d)).unwrap_or(u32::MAX);
    let period_ticks = to_u32(period);
    // 한 바퀴 계산은 짧다(명령 몇 개, 이벤트 몇 개). 주기의 1/4을 계산 시간으로, 주기 안에 끝내도록 한다.
    let mut policy = TimeConstraintPolicy {
        period: period_ticks,
        computation: (period_ticks / 4).max(1),
        constraint: period_ticks,
        preemptible: 1,
    };
    // SAFETY: `pthread_self`는 지금 스레드를 돌려주고, `pthread_mach_thread_np`는 해제할 필요가 없는
    // 그 스레드의 mach 포트를 돌려준다. 정책 포인터는 이 스택 값을 가리키며 크기는 COUNT(정수 4개)와
    // 같다.
    let status = unsafe {
        let thread = pthread_mach_thread_np(pthread_self());
        thread_policy_set(
            thread,
            THREAD_TIME_CONSTRAINT_POLICY,
            (&raw mut policy).cast::<i32>(),
            THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        )
    };
    status == 0
}
