//! 통합 테스트 공통 도우미. 공개 API만 쓴다.
#![allow(dead_code, reason = "테스트 파일마다 쓰는 도우미가 다르다")]

use std::fmt::Write as _;

use keyscore_core::Clip;
use keyscore_text::{Mode, Parsed, parse};

/// 노트를 (시작, 길이, 음높이, 가사)로. 왕복 비교 기준(계획 「왕복의 비교 기준」)과 같은 필드다.
pub type NoteView = (u64, u32, u8, Option<String>);

pub fn notes(clip: &Clip) -> Vec<NoteView> {
    clip.notes()
        .iter()
        .map(|n| {
            (
                n.start().get(),
                n.length(),
                n.pitch().get(),
                n.lyric().map(str::to_owned),
            )
        })
        .collect()
}

pub fn n(start: u64, length: u32, pitch: u8) -> NoteView {
    (start, length, pitch, None)
}

pub fn nl(start: u64, length: u32, pitch: u8, lyric: &str) -> NoteView {
    (start, length, pitch, Some(lyric.to_owned()))
}

pub fn ok(text: &str, mode: Mode) -> Parsed {
    match parse(text, mode) {
        Ok(p) => p,
        Err(e) => panic!("파싱 실패: {e}\n입력:\n{text}"),
    }
}

/// 멜로디 트랙(ch1) 하나의 본문을 느슨 모드로 읽어 클립을 돌려준다.
pub fn melody(body: &str) -> Clip {
    one_track(&format!("@track T ch1\n{body}"), Mode::Lenient)
}

/// 드럼 트랙(ch10) 하나의 본문을 느슨 모드로 읽어 클립을 돌려준다.
pub fn drums(body: &str) -> Clip {
    one_track(&format!("@track D ch10\n{body}"), Mode::Lenient)
}

pub fn one_track(text: &str, mode: Mode) -> Clip {
    let p = ok(text, mode);
    assert_eq!(p.tracks().len(), 1, "트랙이 하나가 아니다: {text}");
    p.tracks()[0].clip().clone()
}

/// 오류의 (줄, 열, 메시지).
pub fn err(text: &str, mode: Mode) -> (usize, usize, String) {
    match parse(text, mode) {
        Ok(p) => panic!("오류여야 한다: {text}\n결과: {p:?}"),
        Err(e) => {
            let d = e.diagnostic();
            (d.line(), d.column(), d.message().to_owned())
        }
    }
}

/// 경고들의 (줄, 열).
pub fn warning_positions(p: &Parsed) -> Vec<(usize, usize)> {
    p.warnings()
        .iter()
        .map(|w| (w.line(), w.column()))
        .collect()
}

/// 스냅샷용 사람이 읽는 표현.
pub fn render(p: &Parsed) -> String {
    let mut s = String::new();
    let h = p.header();
    let _ = writeln!(
        s,
        "header: tempo={:?} time={:?} key={:?}",
        h.tempo().map(keyscore_core::Tempo::micros_per_quarter),
        h.time()
            .map(|t| format!("{}/{}", t.numerator(), t.denominator())),
        h.key()
    );
    for t in p.tracks() {
        let _ = writeln!(
            s,
            "track {:?} ch{} prog={:?} bank={:?} length={}",
            t.name(),
            t.channel().get(),
            t.program().map(keyscore_core::Program::get),
            t.bank().map(keyscore_core::Bank::get),
            t.clip().length()
        );
        for note in t.clip().notes() {
            let _ = write!(
                s,
                "  {:>6} {:>5} {:>3} v{}",
                note.start().get(),
                note.length(),
                note.pitch().get(),
                note.velocity().to_midi1()
            );
            if let Some(l) = note.lyric() {
                let _ = write!(s, " {l:?}");
            }
            s.push('\n');
        }
    }
    for w in p.warnings() {
        let _ = writeln!(s, "warning: {w}");
    }
    s
}
