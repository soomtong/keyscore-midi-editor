use std::collections::HashMap;

use keyscore_core::{
    Bank, Channel, Clip, Key, ModelError, Note, Pitch, PitchClass, Program, Tempo, Tick,
    TimeSignature, Velocity,
};

use crate::lengths::{self, WHOLE};
use crate::{Diagnostic, Header, Mode, ParseError, Parsed, ParsedTrack, drum};

/// 블록을 시작할 때의 옥타브·기본 길이·벨로시티. REQ에 값이 없어 MML 관례(o4, l4)와
/// 트랙 기본 벨로시티(MIDI 1.0 100)를 쓴다. 내보내기는 이 값에 기대지 않고 늘 명시한다.
const START_OCTAVE: i32 = 4;
const START_LENGTH: u64 = WHOLE / 4;
const START_VELOCITY: u8 = 100;

/// `@tempo`의 정수부·소수부 자릿수 한도. 내보내기가 쓰는 자릿수(최대 12)를 받고,
/// 60,000,000 × 10^12이 u128에 들어가게 한다.
const TEMPO_DIGITS: usize = 12;

/// 계이름 텍스트를 읽는다.
///
/// 결정 문서가 정하지 않아 이 구현이 정한 해석(보고서 「리뷰어에게」에 같은 목록이 있다):
///
/// - `@track` 블록 밖에는 헤더만 쓸 수 있다. 헤더는 첫 `@track` 앞에 한 번씩만 온다.
/// - 모르는 `@지시어`는 그 줄 끝까지를 한 토큰으로 건너뛴다(인자까지 음으로 읽지 않게).
/// - 길이·점·`~`·가사·`&`는 앞 토큰에 붙어 있어야 한다. 떨어진 것은 모르는 토큰이다.
///   점은 길이 숫자 뒤에만 오고, `~`의 두 값은 부호(`+`/`-`)가 있어야 한다.
/// - 드럼 트랙의 `n<번호>`는 숫자를 모두 번호로 읽으므로 길이를 붙일 수 없다(`[n75]4`로 쓴다).
///   드럼 트랙에서 음이름은 오류다.
/// - `<`·`>`는 옥타브를 0–9 밖으로도 옮긴다. 범위 검사는 음높이(0–127)로 한다.
/// - 화음 안에는 음과 옥타브 명령만, 셋잇단 안에는 원소와 `o`·`<`·`>`·`l`·`v`만 온다.
/// - 타이는 다음 음·화음과 음높이 집합이 같을 때 잇는다. 이어지는 쪽에 가사가 있으면 오류다.
/// - 클립 길이는 성부들의 노트 끝과 쉼표 끝 가운데 가장 늦은 것이다. 노트 뒤의 명목 커서는
///   세지 않는다(`c8~+13`의 클립은 493틱).
/// - 같은 이름의 블록은 이름을 대소문자까지 같게 비교한다.
///
/// # Errors
///
/// 문법 오류(엄격 모드에서는 경고가 될 것도)의 첫 위치를 담은 [`ParseError`].
pub fn parse(text: &str, mode: Mode) -> Result<Parsed, ParseError> {
    let mut p = Parser::new(text, mode);
    p.run()?;
    p.finish()
}

type R<T> = Result<T, ParseError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pos {
    line: usize,
    column: usize,
}

fn error(pos: Pos, message: impl Into<String>) -> ParseError {
    ParseError::new(Diagnostic::new(pos.line, pos.column, message))
}

fn model_error(pos: Pos, e: &ModelError) -> ParseError {
    error(pos, e.to_string())
}

/// 문자 단위 커서. 한 번 지난 문자로 돌아가지 않는다(입력 길이에 선형).
struct Scanner {
    chars: Vec<char>,
    i: usize,
    line: usize,
    column: usize,
}

impl Scanner {
    fn new(text: &str) -> Self {
        Scanner {
            chars: text.chars().collect(),
            i: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn peek_at(&self, k: usize) -> Option<char> {
        self.chars.get(self.i + k).copied()
    }

    /// 대소문자 구분 없는 비교용(REQ 5.14).
    fn lower_at(&self, k: usize) -> Option<char> {
        self.peek_at(k).map(|c| c.to_ascii_lowercase())
    }

    fn digit_at(&self, k: usize) -> bool {
        self.peek_at(k).is_some_and(|c| c.is_ascii_digit())
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn pos(&self) -> Pos {
        Pos {
            line: self.line,
            column: self.column,
        }
    }

    /// `word`(소문자)로 시작하고 바로 뒤가 숫자인가.
    fn word_then_digit(&self, word: &str) -> bool {
        word.chars()
            .enumerate()
            .all(|(k, c)| self.lower_at(k) == Some(c))
            && self.digit_at(word.chars().count())
    }

    fn skip(&mut self, n: usize) {
        for _ in 0..n {
            self.bump();
        }
    }

    fn skip_to_line_end(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek().filter(|&c| c != '\n') {
            s.push(c);
            self.bump();
        }
        s
    }

    fn digits(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek().filter(char::is_ascii_digit) {
            s.push(c);
            self.bump();
        }
        s
    }
}

/// 한 성부의 노트(합치기 전). 타이로 끝이 늘어날 수 있어 길이 대신 끝을 둔다.
struct Draft {
    pos: Pos,
    start: u64,
    end: u64,
    pitch: u8,
    velocity: Velocity,
    lyric: Option<String>,
}

/// 같은 이름의 `@track` 블록들. 클립 하나가 된다(결정 Q2, 성부 블록).
struct Block {
    pos: Pos,
    name: String,
    channel: Channel,
    program: Option<Program>,
    bank: Option<Bank>,
    drafts: Vec<Draft>,
    end: u64,
}

/// 맺지 못한 타이: `&`의 위치와 이어 붙일 노트(음높이 오름차순).
struct Tie {
    pos: Pos,
    drafts: Vec<usize>,
}

/// 지금 읽는 `@track` 블록 하나의 상태. 블록마다 처음 값에서 시작한다.
struct Voice {
    block: usize,
    drum: bool,
    octave: i32,
    default_length: u64,
    velocity: Velocity,
    cursor: u64,
    bar_start: u64,
    bar_number: u64,
    tie: Option<Tie>,
}

enum EventKind {
    Rest,
    Notes(Vec<u8>),
}

/// 커서를 나아가게 하는 것 하나(음, 화음, 쉼표).
struct Event {
    pos: Pos,
    kind: EventKind,
    nominal: u64,
    offset: Option<(Pos, i64, i64)>,
    lyric: Option<String>,
    tie: Option<Pos>,
}

/// 음·화음 뒤 접미의 순서(결정 Q6): 길이 점 ~보정 "가사" &.
struct Suffix {
    nominal: u64,
    offset: Option<(Pos, i64, i64)>,
    lyric: Option<String>,
    tie: Option<Pos>,
}

struct Parser {
    s: Scanner,
    mode: Mode,
    warnings: Vec<Diagnostic>,
    header: Header,
    bar_ticks: u64,
    blocks: Vec<Block>,
    by_name: HashMap<String, usize>,
    voice: Option<Voice>,
}

impl Parser {
    fn new(text: &str, mode: Mode) -> Self {
        Parser {
            s: Scanner::new(text),
            mode,
            warnings: Vec::new(),
            header: Header::default(),
            bar_ticks: TimeSignature::COMMON.bar_ticks(),
            blocks: Vec::new(),
            by_name: HashMap::new(),
            voice: None,
        }
    }

    /// 느슨 모드에서는 경고, 엄격 모드에서는 오류.
    fn lenient(&mut self, pos: Pos, message: impl Into<String>) -> R<()> {
        match self.mode {
            Mode::Lenient => {
                self.warnings
                    .push(Diagnostic::new(pos.line, pos.column, message));
                Ok(())
            }
            Mode::Strict => Err(error(pos, message)),
        }
    }

    fn voice(&mut self, pos: Pos) -> R<&mut Voice> {
        self.voice
            .as_mut()
            .ok_or_else(|| error(pos, "@track 블록 밖에는 음을 쓸 수 없다"))
    }

    fn run(&mut self) -> R<()> {
        loop {
            self.skip_trivia();
            let pos = self.s.pos();
            let Some(c) = self.s.peek() else { break };
            if c == '@' {
                self.directive(pos)?;
            } else if self.voice.is_none() {
                return Err(error(
                    pos,
                    "@track 블록 밖에는 헤더(@tempo·@time·@key)만 쓸 수 있다",
                ));
            } else {
                self.body_token(pos)?;
            }
        }
        self.end_voice()
    }

    /// 공백과 주석. 이 함수가 부르는 자리는 토큰 첫머리라서 `#`는 언제나 주석이다(결정 Q1).
    /// 음 바로 뒤의 `#`는 [`Parser::note_head`]가 먼저 올림표로 가져간다.
    fn skip_trivia(&mut self) {
        while let Some(c) = self.s.peek() {
            if c.is_whitespace() {
                self.s.bump();
            } else if c == '#' {
                self.s.skip_to_line_end();
            } else {
                break;
            }
        }
    }

    fn directive(&mut self, pos: Pos) -> R<()> {
        self.s.bump();
        let mut word = String::new();
        while let Some(c) = self.s.peek().filter(char::is_ascii_alphanumeric) {
            word.push(c.to_ascii_lowercase());
            self.s.bump();
        }
        match word.as_str() {
            "track" => {
                self.end_voice()?;
                self.track_header(pos)
            }
            "tempo" | "time" | "key" => {
                if !self.blocks.is_empty() {
                    return Err(error(
                        pos,
                        format!("헤더 @{word}는 첫 @track 앞에만 쓸 수 있다"),
                    ));
                }
                let seen = match word.as_str() {
                    "tempo" => self.header.tempo().is_some(),
                    "time" => self.header.time().is_some(),
                    _ => self.header.key().is_some(),
                };
                if seen {
                    return Err(error(pos, format!("헤더 @{word}가 두 번 나왔다")));
                }
                self.skip_trivia();
                let at = self.s.pos();
                match word.as_str() {
                    "tempo" => self.tempo(at),
                    "time" => self.time(at),
                    _ => self.key(at),
                }
            }
            _ => {
                let rest = self.s.skip_to_line_end();
                self.lenient(pos, format!("모르는 지시어 '@{word}{rest}'를 건너뛴다"))
            }
        }
    }

    fn tempo(&mut self, pos: Pos) -> R<()> {
        let whole = self.s.digits();
        let mut frac = String::new();
        if self.s.peek() == Some('.') && self.s.digit_at(1) {
            self.s.bump();
            frac = self.s.digits();
        }
        if whole.is_empty() {
            return Err(error(pos, "@tempo 뒤에는 BPM 숫자를 쓴다"));
        }
        if whole.len() > TEMPO_DIGITS || frac.len() > TEMPO_DIGITS {
            return Err(error(pos, "@tempo 값의 자릿수가 너무 많다"));
        }
        let digits = format!("{whole}{frac}");
        let scaled: u128 = digits
            .parse()
            .map_err(|_| error(pos, "@tempo 값을 읽을 수 없다"))?;
        // 자릿수 한도(12)라 u32에 들어간다.
        let decimals = u32::try_from(frac.len()).unwrap_or(u32::MAX);
        let us = micros_from_decimal_bpm(scaled, decimals)
            .ok_or_else(|| error(pos, "@tempo는 0보다 커야 한다"))?;
        let tempo = Tempo::from_micros_per_quarter(us).map_err(|e| model_error(pos, &e))?;
        self.header.set_tempo(tempo);
        Ok(())
    }

    fn time(&mut self, pos: Pos) -> R<()> {
        let bad = || error(pos, "@time은 <분자>/<분모> 꼴이다(예: 4/4)");
        let num = self.s.digits();
        if num.is_empty() || self.s.peek() != Some('/') {
            return Err(bad());
        }
        self.s.bump();
        let den = self.s.digits();
        let num: u8 = num.parse().map_err(|_| bad())?;
        let den: u16 = den.parse().map_err(|_| bad())?;
        let sig = TimeSignature::new(num, den).map_err(|e| model_error(pos, &e))?;
        self.header.set_time(sig);
        self.bar_ticks = sig.bar_ticks();
        Ok(())
    }

    fn key(&mut self, pos: Pos) -> R<()> {
        let alnum = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
        if self.s.word_is("none") {
            self.s.skip(4);
            self.header.set_key(Key::None);
            return Ok(());
        }
        let bad = || {
            error(
                pos,
                "@key는 <으뜸음> major|minor 또는 none이다(예: C major, F# minor)",
            )
        };
        let Some(pc) = self.s.lower_at(0).and_then(pitch_class_of) else {
            return Err(bad());
        };
        self.s.bump();
        let mut pc = i32::from(pc);
        match self.s.lower_at(0) {
            Some('#') => {
                pc += 1;
                self.s.bump();
            }
            Some('b') if !alnum(self.s.peek_at(1)) => {
                pc -= 1;
                self.s.bump();
            }
            _ => {}
        }
        if alnum(self.s.peek()) {
            return Err(bad());
        }
        let pc = PitchClass::new(u8::try_from(pc.rem_euclid(12)).unwrap_or(0))
            .map_err(|e| model_error(pos, &e))?;
        self.skip_trivia();
        let mode_pos = self.s.pos();
        let key = if self.s.word_is("major") {
            Key::Major(pc)
        } else if self.s.word_is("minor") {
            Key::Minor(pc)
        } else {
            return Err(error(
                mode_pos,
                "@key의 으뜸음 뒤에는 major 또는 minor를 쓴다",
            ));
        };
        self.s.skip(5);
        self.header.set_key(key);
        Ok(())
    }

    fn track_header(&mut self, pos: Pos) -> R<()> {
        self.skip_trivia();
        let name = if self.s.peek() == Some('"') {
            self.quoted()?
        } else {
            let mut name = String::new();
            while let Some(c) = self
                .s
                .peek()
                .filter(|&c| !c.is_whitespace() && c != '#' && c != '"')
            {
                name.push(c);
                self.s.bump();
            }
            if name.is_empty() {
                return Err(error(self.s.pos(), "@track 뒤에는 트랙 이름을 쓴다"));
            }
            name
        };
        self.skip_trivia();
        let ch_pos = self.s.pos();
        if !self.s.word_then_digit("ch") {
            return Err(error(ch_pos, "@track 이름 뒤에는 ch<1–16>을 쓴다"));
        }
        self.s.skip(2);
        let channel = self.number(ch_pos)?;
        let channel = Channel::new(u8::try_from(channel).unwrap_or(u8::MAX))
            .map_err(|e| model_error(ch_pos, &e))?;
        self.skip_trivia();
        let mut program = None;
        if self.s.word_then_digit("prog") {
            let at = self.s.pos();
            self.s.skip(4);
            let v = self.number(at)?;
            program = Some(
                Program::new(u8::try_from(v).unwrap_or(u8::MAX))
                    .map_err(|e| model_error(at, &e))?,
            );
            self.skip_trivia();
        }
        let mut bank = None;
        if self.s.word_then_digit("bank") {
            let at = self.s.pos();
            self.s.skip(4);
            let v = self.number(at)?;
            bank = Some(
                Bank::new(u16::try_from(v).unwrap_or(u16::MAX)).map_err(|e| model_error(at, &e))?,
            );
        }

        let block = if let Some(&i) = self.by_name.get(&name) {
            let b = &self.blocks[i];
            if (b.channel, b.program, b.bank) != (channel, program, bank) {
                return Err(error(
                    pos,
                    format!("같은 이름 '{name}'의 @track 블록은 ch·prog·bank가 같아야 한다"),
                ));
            }
            i
        } else {
            self.blocks.push(Block {
                pos,
                name: name.clone(),
                channel,
                program,
                bank,
                drafts: Vec::new(),
                end: 0,
            });
            self.by_name.insert(name, self.blocks.len() - 1);
            self.blocks.len() - 1
        };
        let velocity = Velocity::from_midi1(START_VELOCITY).map_err(|e| model_error(pos, &e))?;
        self.voice = Some(Voice {
            block,
            drum: channel.is_drum(),
            octave: START_OCTAVE,
            default_length: START_LENGTH,
            velocity,
            cursor: 0,
            bar_start: 0,
            bar_number: 0,
            tie: None,
        });
        Ok(())
    }

    fn end_voice(&mut self) -> R<()> {
        if let Some(tie) = self.voice.take().and_then(|v| v.tie) {
            self.lenient(tie.pos, "타이(&) 뒤에 이어질 같은 음이 없다")?;
        }
        Ok(())
    }

    fn body_token(&mut self, pos: Pos) -> R<()> {
        match self.s.peek() {
            Some('|') => {
                self.s.bump();
                self.bar(pos)
            }
            Some('[') => {
                let ev = self.chord(pos)?;
                self.emit(ev, false)
            }
            Some(c @ ('<' | '>')) => {
                self.s.bump();
                self.shift_octave(pos, c)
            }
            Some(c @ (']' | ')')) => Err(error(pos, format!("짝이 없는 '{c}'"))),
            Some(c) if c.is_ascii_alphabetic() => self.letter(pos, false).map(|_| ()),
            _ => self.unknown(pos),
        }
    }

    fn shift_octave(&mut self, pos: Pos, c: char) -> R<()> {
        let v = self.voice(pos)?;
        v.octave = if c == '>' {
            v.octave.saturating_add(1)
        } else {
            v.octave.saturating_sub(1)
        };
        Ok(())
    }

    /// 마디선. 틱 계산에는 쓰지 않고 지난 마디선 이후의 명목 길이만 검사한다(결정 Q5).
    fn bar(&mut self, pos: Pos) -> R<()> {
        let bar_ticks = self.bar_ticks;
        let sig = self.header.time().unwrap_or(TimeSignature::COMMON);
        let v = self.voice(pos)?;
        v.bar_number += 1;
        let measure = v.cursor - v.bar_start;
        let number = v.bar_number;
        v.bar_start = v.cursor;
        if measure != bar_ticks {
            self.lenient(
                pos,
                format!(
                    "마디 {number}: 길이 {}, 박자표 {}/{}의 마디 길이 {}",
                    describe_length(measure),
                    sig.numerator(),
                    sig.denominator(),
                    describe_length(bar_ticks)
                ),
            )?;
        }
        Ok(())
    }

    /// 글자로 시작하는 토큰. 음·쉼표를 냈으면 `true`(셋잇단 원소 세기용).
    fn letter(&mut self, pos: Pos, in_triplet: bool) -> R<bool> {
        let drum = self.voice(pos)?.drum;
        let a = self.s.lower_at(0).unwrap_or(' ');
        let b = self.s.lower_at(1).unwrap_or(' ');
        let label = drum::note_of_chars(a, b);
        if drum {
            if label.is_some() {
                let ev = self.drum_single(pos)?;
                self.emit(ev, in_triplet)?;
                return Ok(true);
            }
        } else if label.is_some() && !(is_melody_start(a) && is_melody_start(b)) {
            // 음으로 읽을 수 없는 라벨만 오류다. bd·cr·rd처럼 음과 쉼표로 읽히는 것은 그렇게 읽는다(결정 Q4).
            return Err(error(
                pos,
                format!("드럼 라벨 '{a}{b}'는 드럼 트랙(ch10)에서만 쓸 수 있다"),
            ));
        }
        match a {
            'r' => {
                self.s.bump();
                let sfx = self.suffix(false)?;
                self.emit(
                    Event {
                        pos,
                        kind: EventKind::Rest,
                        nominal: sfx.nominal,
                        offset: None,
                        lyric: None,
                        tie: None,
                    },
                    in_triplet,
                )?;
                Ok(true)
            }
            'n' if b == '(' => {
                if in_triplet {
                    return Err(error(pos, "셋잇단 안에 셋잇단을 둘 수 없다"));
                }
                self.triplet(pos)?;
                Ok(false)
            }
            'n' if drum && self.s.digit_at(1) => {
                let ev = self.drum_single(pos)?;
                self.emit(ev, in_triplet)?;
                Ok(true)
            }
            'n' if self.s.digit_at(1) => {
                Err(error(pos, "n<번호>는 드럼 트랙(ch10)에서만 쓸 수 있다"))
            }
            'n' => Err(error(pos, "n 뒤에는 셋잇단 '(' 또는 드럼 번호를 쓴다")),
            'a'..='g' if drum => Err(error(
                pos,
                "드럼 트랙(ch10)에서는 음이름 대신 드럼 라벨이나 n<번호>를 쓴다",
            )),
            'a'..='g' => {
                let pitch = self.note_head(pos)?;
                let sfx = self.suffix(true)?;
                self.emit(note_event(pos, vec![pitch], sfx), in_triplet)?;
                Ok(true)
            }
            'o' | 'l' | 'v' => {
                self.command(pos, a)?;
                Ok(false)
            }
            _ => {
                self.unknown(pos)?;
                Ok(false)
            }
        }
    }

    fn drum_single(&mut self, pos: Pos) -> R<Event> {
        let pitch = self.drum_head(pos)?;
        let sfx = self.suffix(true)?;
        Ok(note_event(pos, vec![pitch], sfx))
    }

    /// `o<0–9>`, `l<길이>[.]`, `v<0–127>`.
    fn command(&mut self, pos: Pos, c: char) -> R<()> {
        self.s.bump();
        if !self.s.digit_at(0) {
            let what = match c {
                'o' => "옥타브 숫자(0–9)",
                'l' => "길이(1·2·4·8·16·32·64)",
                _ => "벨로시티(0–127)",
            };
            return Err(error(pos, format!("{c} 뒤에는 {what}를 쓴다")));
        }
        match c {
            'o' => {
                let n = self.number(pos)?;
                if n > 9 {
                    return Err(error(pos, format!("옥타브는 o0–o9다: o{n}")));
                }
                // 0–9라 i32에 들어간다.
                self.voice(pos)?.octave = i32::try_from(n).unwrap_or(START_OCTAVE);
            }
            'l' => {
                let len = self.length()?;
                self.voice(pos)?.default_length = len;
            }
            _ => {
                let n = self.number(pos)?;
                let v = u8::try_from(n)
                    .ok()
                    .and_then(|n| Velocity::from_midi1(n).ok())
                    .ok_or_else(|| error(pos, format!("벨로시티는 v0–v127이다: v{n}")))?;
                self.voice(pos)?.velocity = v;
            }
        }
        Ok(())
    }

    /// 음이름과 바로 붙은 변화표. 변화표 뒤의 `#`도 올림표다(결정 Q1).
    fn note_head(&mut self, pos: Pos) -> R<u8> {
        let pc = self.s.lower_at(0).and_then(pitch_class_of).unwrap_or(0);
        self.s.bump();
        let mut accidental: i64 = 0;
        while let Some(c) = self.s.peek() {
            match c {
                '+' | '#' => accidental += 1,
                '-' => accidental -= 1,
                _ => break,
            }
            self.s.bump();
        }
        let octave = i64::from(self.voice(pos)?.octave);
        let pitch = (octave + 1) * 12 + i64::from(pc) + accidental;
        u8::try_from(pitch)
            .ok()
            .filter(|&p| p <= 127)
            .ok_or_else(|| error(pos, format!("음높이 {pitch}가 MIDI 범위(0–127) 밖이다")))
    }

    /// 드럼 라벨(두 글자) 또는 `n<번호>`.
    fn drum_head(&mut self, pos: Pos) -> R<u8> {
        let a = self.s.lower_at(0).unwrap_or(' ');
        let b = self.s.lower_at(1).unwrap_or(' ');
        if let Some(note) = drum::note_of_chars(a, b) {
            self.s.skip(2);
            return Ok(note);
        }
        self.s.bump();
        let n = self.number(pos)?;
        u8::try_from(n)
            .ok()
            .filter(|&n| n <= 127)
            .ok_or_else(|| error(pos, format!("드럼 번호는 n0–n127이다: n{n}")))
    }

    /// 길이 숫자와 점. 숫자는 쓴 그대로 1·2·4·8·16·32·64여야 한다(`04`도 오류).
    fn length(&mut self) -> R<u64> {
        let pos = self.s.pos();
        let digits = self.s.digits();
        let mut dots = 0u8;
        while self.s.peek() == Some('.') {
            if dots == 2 {
                return Err(error(self.s.pos(), "점은 둘(겹점)까지만 쓴다"));
            }
            dots += 1;
            self.s.bump();
        }
        lengths::ticks_of(&digits, dots).ok_or_else(|| {
            error(
                pos,
                format!("길이 {digits}는 쓸 수 없다. 1·2·4·8·16·32·64만 쓴다"),
            )
        })
    }

    /// 접미. 쉼표(`allow_extras` 거짓)는 길이와 점만 받는다.
    fn suffix(&mut self, allow_extras: bool) -> R<Suffix> {
        let nominal = if self.s.digit_at(0) {
            self.length()?
        } else if self.s.peek() == Some('.') {
            return Err(error(self.s.pos(), "점은 길이 숫자 뒤에만 쓴다"));
        } else {
            let pos = self.s.pos();
            self.voice(pos)?.default_length
        };
        let mut sfx = Suffix {
            nominal,
            offset: None,
            lyric: None,
            tie: None,
        };
        if !allow_extras {
            if let Some(c @ ('~' | '"' | '&')) = self.s.peek() {
                return Err(error(
                    self.s.pos(),
                    format!("쉼표에는 '{c}'를 붙일 수 없다"),
                ));
            }
            return Ok(sfx);
        }
        if self.s.peek() == Some('~') {
            let pos = self.s.pos();
            if self.mode == Mode::Strict {
                return Err(error(
                    pos,
                    "AI 응답(엄격 모드)에서는 틱 보정 ~를 쓸 수 없다",
                ));
            }
            self.s.bump();
            let start = self.signed(pos)?;
            let mut length = 0;
            if self.s.peek() == Some(':') {
                self.s.bump();
                length = self.signed(pos)?;
            }
            sfx.offset = Some((pos, start, length));
        }
        if self.s.peek() == Some('"') {
            sfx.lyric = Some(self.quoted()?);
        }
        if self.s.peek() == Some('&') {
            sfx.tie = Some(self.s.pos());
            self.s.bump();
        }
        Ok(sfx)
    }

    /// `~` 보정의 부호 있는 정수. 부호는 꼭 쓴다.
    fn signed(&mut self, pos: Pos) -> R<i64> {
        let sign = match self.s.peek() {
            Some('+') => 1,
            Some('-') => -1,
            _ => {
                return Err(error(
                    pos,
                    "~ 보정은 ~<±시작>[:<±길이>] 꼴이다(예: ~+13, ~+0:-20)",
                ));
            }
        };
        self.s.bump();
        if !self.s.digit_at(0) {
            return Err(error(
                pos,
                "~ 보정은 ~<±시작>[:<±길이>] 꼴이다(예: ~+13, ~+0:-20)",
            ));
        }
        let n = self.number(pos)?;
        i64::try_from(n)
            .map(|n| sign * n)
            .map_err(|_| error(pos, "~ 보정 값이 너무 크다"))
    }

    /// 큰따옴표 문자열. `\"`와 `\\`만 이스케이프다. 그 밖의 `\`는 글자 그대로 둔다.
    fn quoted(&mut self) -> R<String> {
        let pos = self.s.pos();
        self.s.bump();
        let mut out = String::new();
        loop {
            match self.s.bump() {
                None => return Err(error(pos, "닫히지 않은 따옴표")),
                Some('"') => return Ok(out),
                Some('\\') if matches!(self.s.peek(), Some('"' | '\\')) => {
                    out.extend(self.s.bump());
                }
                Some(c) => out.push(c),
            }
        }
    }

    fn number(&mut self, pos: Pos) -> R<u64> {
        let digits = self.s.digits();
        digits
            .parse()
            .map_err(|_| error(pos, format!("숫자 {digits}가 너무 크다")))
    }

    fn chord(&mut self, pos: Pos) -> R<Event> {
        self.s.bump();
        let drum = self.voice(pos)?.drum;
        let mut pitches = Vec::new();
        let misplaced = "화음 [ ] 안에는 음과 옥타브 명령(o < >)만 쓸 수 있다";
        loop {
            self.skip_trivia();
            let at = self.s.pos();
            match self.s.peek() {
                None => return Err(error(pos, "닫히지 않은 화음 '['")),
                Some(']') => {
                    self.s.bump();
                    break;
                }
                Some(c @ ('<' | '>')) => {
                    self.s.bump();
                    self.shift_octave(at, c)?;
                }
                Some(c) if c.is_ascii_digit() => {
                    return Err(error(
                        at,
                        "화음 안의 음에는 길이를 쓰지 않는다. 길이는 ] 뒤에 쓴다",
                    ));
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    let a = c.to_ascii_lowercase();
                    let b = self.s.lower_at(1).unwrap_or(' ');
                    let label = drum::note_of_chars(a, b);
                    if drum && (label.is_some() || (a == 'n' && self.s.digit_at(1))) {
                        pitches.push(self.drum_head(at)?);
                    } else if !drum
                        && label.is_some()
                        && !(is_melody_start(a) && is_melody_start(b))
                    {
                        return Err(error(
                            at,
                            format!("드럼 라벨 '{a}{b}'는 드럼 트랙(ch10)에서만 쓸 수 있다"),
                        ));
                    } else if a == 'o' && self.s.digit_at(1) {
                        self.command(at, 'o')?;
                    } else if ('a'..='g').contains(&a) {
                        if drum {
                            return Err(error(
                                at,
                                "드럼 트랙(ch10)에서는 음이름 대신 드럼 라벨이나 n<번호>를 쓴다",
                            ));
                        }
                        pitches.push(self.note_head(at)?);
                    } else if matches!(a, 'r' | 'l' | 'v' | 'n' | 'o') {
                        return Err(error(at, misplaced));
                    } else {
                        self.unknown(at)?;
                    }
                }
                Some('[' | '|' | '@' | ')' | '(') => return Err(error(at, misplaced)),
                Some(_) => self.unknown(at)?,
            }
        }
        if pitches.is_empty() {
            return Err(error(pos, "빈 화음 []"));
        }
        let sfx = self.suffix(true)?;
        Ok(note_event(pos, pitches, sfx))
    }

    /// `n( x y z )`: 원소 셋, 각 원소 길이의 2/3(결정 Q2, 문법).
    fn triplet(&mut self, pos: Pos) -> R<()> {
        self.s.skip(2);
        let count_error = || error(pos, "셋잇단 n( )에는 음·쉼표·화음이 정확히 3개 와야 한다");
        let mut count = 0;
        loop {
            self.skip_trivia();
            let at = self.s.pos();
            let emitted = match self.s.peek() {
                None => return Err(error(pos, "닫히지 않은 셋잇단 'n('")),
                Some(')') => {
                    self.s.bump();
                    break;
                }
                Some('[') => {
                    let ev = self.chord(at)?;
                    self.emit(ev, true)?;
                    true
                }
                Some(c @ ('<' | '>')) => {
                    self.s.bump();
                    self.shift_octave(at, c)?;
                    false
                }
                Some(c) if c.is_ascii_alphabetic() => self.letter(at, true)?,
                Some('|' | '@' | ']' | '(') => {
                    return Err(error(at, "셋잇단 n( ) 안에 쓸 수 없는 토큰"));
                }
                Some(_) => {
                    self.unknown(at)?;
                    false
                }
            };
            if emitted {
                count += 1;
                if count > 3 {
                    return Err(count_error());
                }
            }
        }
        if count != 3 {
            return Err(count_error());
        }
        if self.s.digit_at(0) {
            return Err(error(
                self.s.pos(),
                "셋잇단 ) 뒤에는 길이를 쓰지 않는다. 길이는 괄호 안의 각 음에 쓴다",
            ));
        }
        Ok(())
    }

    /// 모르는 토큰 하나를 건너뛴다(느슨 모드는 경고, 엄격 모드는 오류).
    fn unknown(&mut self, pos: Pos) -> R<()> {
        let mut text = String::new();
        match self.s.peek() {
            Some('"') => {
                let q = self.quoted()?;
                text = format!("\"{q}\"");
            }
            Some(c) if c.is_ascii_digit() => {
                text = self.s.digits();
                while let Some('.') = self.s.peek() {
                    text.push('.');
                    self.s.bump();
                }
            }
            Some(c) => {
                text.push(c);
                self.s.bump();
                if c.is_ascii_alphabetic() {
                    text.push_str(&self.s.digits());
                }
            }
            None => {}
        }
        self.lenient(pos, format!("모르는 토큰 '{text}'를 건너뛴다"))
    }

    /// 사건 하나를 성부에 넣고 커서를 명목 길이만큼 옮긴다.
    fn emit(&mut self, ev: Event, triplet: bool) -> R<()> {
        // 표기 길이는 모두 15틱의 배수라 2/3이 정수다.
        let nominal = if triplet {
            ev.nominal * 2 / 3
        } else {
            ev.nominal
        };
        let Some(voice) = self.voice.as_mut() else {
            return Err(error(ev.pos, "@track 블록 밖에는 음을 쓸 수 없다"));
        };
        let cursor = voice.cursor;
        let next_cursor = cursor
            .checked_add(nominal)
            .ok_or_else(|| error(ev.pos, "틱이 u64 범위를 넘는다"))?;
        let pending = voice.tie.take();
        let velocity = voice.velocity;
        let block_index = voice.block;
        voice.cursor = next_cursor;

        let EventKind::Notes(mut pitches) = ev.kind else {
            if let Some(tie) = pending {
                self.lenient(tie.pos, "타이(&) 뒤가 같은 음이 아니라 쉼표다")?;
            }
            let block = &mut self.blocks[block_index];
            block.end = block.end.max(next_cursor);
            return Ok(());
        };

        let (start_offset, length_offset, offset_pos) = match ev.offset {
            Some((p, s, l)) => (s, l, p),
            None => (0, 0, ev.pos),
        };
        let start = i128::from(cursor) + i128::from(start_offset);
        let length = i128::from(nominal) + i128::from(length_offset);
        if start < 0 {
            return Err(error(
                offset_pos,
                format!("보정한 시작({start}틱)이 클립 시작(0) 앞이다"),
            ));
        }
        if length < 1 {
            return Err(error(
                offset_pos,
                format!("보정한 길이({length}틱)가 1틱보다 짧다"),
            ));
        }
        let start =
            u64::try_from(start).map_err(|_| error(offset_pos, "틱이 u64 범위를 넘는다"))?;
        let end = u64::try_from(length)
            .ok()
            .and_then(|l| start.checked_add(l))
            .ok_or_else(|| error(offset_pos, "틱이 u64 범위를 넘는다"))?;

        pitches.sort_unstable();
        let block = &mut self.blocks[block_index];
        block.end = block.end.max(end);

        if let Some(tie) = pending {
            let same = tie.drafts.len() == pitches.len()
                && tie
                    .drafts
                    .iter()
                    .zip(&pitches)
                    .all(|(&d, &p)| block.drafts[d].pitch == p);
            if same {
                if ev.lyric.is_some() {
                    return Err(error(
                        ev.pos,
                        "타이로 이어지는 음에는 가사를 쓸 수 없다(가사는 첫 음에 쓴다)",
                    ));
                }
                for &d in &tie.drafts {
                    let draft = &mut block.drafts[d];
                    draft.end = draft.end.max(end);
                }
                self.set_tie(ev.tie.map(|pos| Tie {
                    pos,
                    drafts: tie.drafts,
                }));
                return Ok(());
            }
            self.lenient(tie.pos, "타이(&) 뒤가 같은 음이 아니다")?;
        }

        let block = &mut self.blocks[block_index];
        let first = block.drafts.len();
        let top = pitches.len() - 1;
        for (i, &pitch) in pitches.iter().enumerate() {
            block.drafts.push(Draft {
                pos: ev.pos,
                start,
                end,
                pitch,
                velocity,
                // 화음의 가사는 가장 높은 음에 저장한다(결정 Q6).
                lyric: if i == top { ev.lyric.clone() } else { None },
            });
        }
        self.set_tie(ev.tie.map(|pos| Tie {
            pos,
            drafts: (first..first + pitches.len()).collect(),
        }));
        Ok(())
    }

    fn set_tie(&mut self, tie: Option<Tie>) {
        if let Some(v) = self.voice.as_mut() {
            v.tie = tie;
        }
    }

    fn finish(self) -> R<Parsed> {
        let mut tracks = Vec::with_capacity(self.blocks.len());
        for b in self.blocks {
            if b.end == 0 {
                return Err(error(
                    b.pos,
                    format!("@track '{}' 블록에 음이나 쉼표가 없다", b.name),
                ));
            }
            let mut clip = Clip::new(Tick::ZERO, b.end).map_err(|e| model_error(b.pos, &e))?;
            let mut notes = Vec::with_capacity(b.drafts.len());
            for d in b.drafts {
                let length = u32::try_from(d.end - d.start).map_err(|_| {
                    error(
                        d.pos,
                        format!("노트 길이 {}틱이 너무 길다", d.end - d.start),
                    )
                })?;
                let pitch = Pitch::new(d.pitch).map_err(|e| model_error(d.pos, &e))?;
                let note = Note::new(Tick::new(d.start), length, pitch, d.velocity)
                    .map_err(|e| model_error(d.pos, &e))?;
                notes.push(match d.lyric {
                    Some(l) => note.with_lyric(l),
                    None => note,
                });
            }
            // 정렬한 뒤 넣으면 insert_note가 매번 끝에 붙여 성부가 많아도 제곱이 되지 않는다.
            notes.sort();
            for note in notes {
                clip.insert_note(note).map_err(|e| model_error(b.pos, &e))?;
            }
            tracks.push(ParsedTrack::new(b.name, b.channel, b.program, b.bank, clip));
        }
        Ok(Parsed::new(self.header, tracks, self.warnings))
    }
}

impl Scanner {
    /// `word`(소문자)가 낱말 하나로 오는가(뒤가 영숫자가 아님).
    fn word_is(&self, word: &str) -> bool {
        let n = word.chars().count();
        word.chars()
            .enumerate()
            .all(|(k, c)| self.lower_at(k) == Some(c))
            && !self.peek_at(n).is_some_and(|c| c.is_ascii_alphanumeric())
    }
}

fn note_event(pos: Pos, pitches: Vec<u8>, sfx: Suffix) -> Event {
    Event {
        pos,
        kind: EventKind::Notes(pitches),
        nominal: sfx.nominal,
        offset: sfx.offset,
        lyric: sfx.lyric,
        tie: sfx.tie,
    }
}

fn pitch_class_of(c: char) -> Option<u8> {
    match c {
        'c' => Some(0),
        'd' => Some(2),
        'e' => Some(4),
        'f' => Some(5),
        'g' => Some(7),
        'a' => Some(9),
        'b' => Some(11),
        _ => None,
    }
}

/// 멜로디 트랙에서 토큰을 시작할 수 있는 글자(음이름, 쉼표).
fn is_melody_start(c: char) -> bool {
    matches!(c, 'a'..='g' | 'r')
}

/// 틱을 온음표 분수로(예: 2880 → 「3/4(2880틱)」). AI 재요청 사유에 그대로 쓴다(AI-6).
fn describe_length(ticks: u64) -> String {
    let g = gcd(ticks, WHOLE);
    let (n, d) = (ticks / g, WHOLE / g);
    if d == 1 {
        format!("{n}({ticks}틱)")
    } else {
        format!("{n}/{d}({ticks}틱)")
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a.max(1) } else { gcd(b, a % b) }
}

/// 십진 BPM(`scaled` / 10^`decimals`)을 4분음표당 µs로. `Tempo::from_bpm`과 같은 반올림
/// (반은 올림)이라 정수 BPM은 그 결과와 같다. 0이면 `None`.
pub(crate) fn micros_from_decimal_bpm(scaled: u128, decimals: u32) -> Option<u32> {
    if scaled == 0 {
        return None;
    }
    let numerator = 60_000_000u128 * 10u128.checked_pow(decimals)?;
    let us = (numerator + scaled / 2) / scaled;
    u32::try_from(us).ok()
}
