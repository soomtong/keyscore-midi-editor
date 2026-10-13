//! 바이너리의 인자 해석과 출력. 인자 파서 crate를 더하지 않으려고 std만 쓴다.
//!
//! 종료 코드: 0 성공, 1 실패(검사 실패·변환 오류·입출력 오류), 2 잘못된 사용법.

use std::ffi::OsString;
use std::io::Write;

/// `ksq check <파일>...` · `ksq fixtures <디렉터리>`.
pub fn ksq(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let _ = (args, out, err);
    todo!()
}

/// `ksq2txt <in.ksq> [-o out.txt]`.
pub fn ksq2txt(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let _ = (args, out, err);
    todo!()
}

/// `txt2ksq <in.txt> [-o out.ksq] [--strict]`.
pub fn txt2ksq(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let _ = (args, out, err);
    todo!()
}
