//! 인코딩 경로가 힙을 쓰지 않는지 본다(REQ 9.2). 이 파일의 테스트는 하나뿐이다.
//!
//! 할당 횟수는 스레드별로 센다. 테스트 하네스의 다른 스레드 할당이 섞이지 않게 하기 위해서다.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use keyscore_core::{Bank, Channel, Pitch, Program, Velocity};
use keyscore_midi as m;

thread_local! {
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

struct Counting;

// SAFETY: 모든 호출을 `System`에 그대로 넘기고, 스레드 로컬 카운터만 더한다. 카운터는
// const 초기화에 `Drop`이 없어 할당자 안에서 접근해도 재진입 할당이 없다.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|c| c.set(c.get() + 1));
        // SAFETY: 호출자가 `GlobalAlloc::alloc`의 계약을 지킨다.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr`과 `layout`은 같은 할당자(`System`)가 준 것이다.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        COUNT.with(|c| c.set(c.get() + 1));
        // SAFETY: 호출자가 `GlobalAlloc::realloc`의 계약을 지킨다.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

#[test]
fn encoding_does_not_allocate() {
    // 입력 값 생성(할당 없음)은 센 구간 밖에서 한다.
    let ch = Channel::new(10).unwrap();
    let pitch = Pitch::new(60).unwrap();
    let vel = Velocity::new(0xC924);
    let prog = Program::new(5).unwrap();
    let bank = Bank::new(300).unwrap();

    let before = COUNT.with(Cell::get);
    let mut sink = 0u32;
    let mut take = |msg: m::Midi1Message| {
        sink = sink.wrapping_add(msg.ump(1)) ^ u32::from(msg.bytes()[0]);
    };
    take(m::note_on(ch, pitch, vel));
    take(m::note_off(ch, pitch, vel));
    take(m::program_change(ch, prog));
    for msg in m::bank_select(ch, bank) {
        take(msg);
    }
    take(m::control_change(ch, 7, 0x8000_0000).unwrap_or_else(|_| m::timing_clock()));
    take(m::pitch_bend(ch, 0x8000_0000));
    take(m::timing_clock());
    take(m::start());
    take(m::continue_());
    take(m::stop());
    take(m::song_position(100).unwrap_or_else(|_| m::stop()));
    for msg in m::panic_messages(ch) {
        take(msg);
    }
    let after = COUNT.with(Cell::get);

    assert_ne!(sink, 0, "최적화로 지워지지 않게 결과를 쓴다");
    assert_eq!(after - before, 0, "인코딩 경로에서 할당이 일어났다");
}
