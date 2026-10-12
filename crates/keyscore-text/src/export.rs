use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use keyscore_core::{Bank, Channel, Clip, Key, Program, Tempo, TimeSignature};

use crate::Header;
use crate::drum;
use crate::lengths::{self, Notation, WHOLE};
use crate::parse::micros_from_decimal_bpm;

/// 내보낼 트랙 하나. 입력 묶음이라 공개 필드로 둔다(`NoteRef`와 같은 이유).
#[derive(Debug, Clone, Copy)]
pub struct ExportTrack<'a> {
    pub name: &'a str,
    pub channel: Channel,
    pub program: Option<Program>,
    pub bank: Option<Bank>,
    pub clip: &'a Clip,
}

/// 헤더와 트랙들을 계이름 텍스트로 적는다.
///
/// 문법 표의 토큰만 쓰고(`v`와 피치 오프셋은 쓰지 않는다), 같은 입력이면 같은 문자열이다.
/// 결과를 [`crate::parse`]로 읽으면 노트의 (시작, 길이, 음높이, 가사), 클립 길이, 트랙 속성,
/// 헤더가 원본과 같다. 숨김 꼬리는 클립 끝에서 자른다. 예외 둘:
///
/// - 트랙 이름이 같은 트랙들은 가져올 때 한 클립의 성부로 합쳐진다(결정 Q2). 호출자가 이름을
///   서로 다르게 넘긴다.
/// - 클립 길이가 노트 끝도 아니고 쉼표 합(15틱 단위)으로도 닿지 않으면 닿는 가장 긴 길이로
///   줄인다. 노트가 없어 그 길이가 0이면 가장 짧은 쉼표(60틱)로 늘린다.
///
/// 적는 방식: 같은 시작·길이의 노트는 화음으로 묶고, 겹치는 것은 같은 이름의 블록(성부)으로
/// 나눈다. 표기 가능한 길이로 그리드에 놓인 노트는 접미 없이, 그 밖은 가장 가까운 길이에
/// `~<±시작>[:<±길이>]`를 붙인다. 온음표보다 긴 노트는 온음표 배수 + 표기 길이면 타이로,
/// 셋 연속으로 표기 길이의 2/3인 노트는 셋잇단으로 적는다.
#[must_use]
pub fn export(header: &Header, tracks: &[ExportTrack<'_>]) -> String {
    let mut out = String::new();
    if let Some(t) = header.tempo() {
        let _ = writeln!(out, "@tempo {}", bpm_text(t));
    }
    if let Some(s) = header.time() {
        let _ = writeln!(out, "@time {}/{}", s.numerator(), s.denominator());
    }
    if let Some(k) = header.key() {
        let _ = writeln!(out, "@key {}", key_text(k));
    }
    let bar = header.time().unwrap_or(TimeSignature::COMMON).bar_ticks();
    for t in tracks {
        if !out.is_empty() {
            out.push('\n');
        }
        write_track(&mut out, t, bar);
    }
    out
}

/// 다시 읽었을 때 같은 µs가 되는 가장 짧은 십진 BPM.
fn bpm_text(tempo: Tempo) -> String {
    let us = u128::from(tempo.micros_per_quarter());
    let mut last = String::new();
    for decimals in 0..=12u32 {
        let scale = 10u128.pow(decimals);
        let numerator = 60_000_000 * scale;
        let scaled = (2 * numerator + us) / (2 * us);
        let whole = scaled / scale;
        let frac = scaled % scale;
        last = if decimals == 0 {
            whole.to_string()
        } else {
            format!("{whole}.{frac:0width$}", width = decimals as usize)
        };
        if micros_from_decimal_bpm(scaled, decimals) == Some(tempo.micros_per_quarter()) {
            break;
        }
    }
    last
}

fn key_text(key: Key) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
    ];
    match key {
        Key::None => "none".to_owned(),
        Key::Major(pc) => format!("{} major", NAMES[usize::from(pc.get())]),
        Key::Minor(pc) => format!("{} minor", NAMES[usize::from(pc.get())]),
    }
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn track_line(t: &ExportTrack<'_>) -> String {
    // 파서의 따옴표 없는 이름은 공백·#·"에서 끝난다.
    let bare = !t.name.is_empty()
        && t.name
            .chars()
            .all(|c| !c.is_whitespace() && c != '#' && c != '"');
    let mut line = format!(
        "@track {} ch{}",
        if bare {
            t.name.to_owned()
        } else {
            quote(t.name)
        },
        t.channel.get()
    );
    if let Some(p) = t.program {
        let _ = write!(line, " prog{}", p.get());
    }
    if let Some(b) = t.bank {
        let _ = write!(line, " bank{}", b.get());
    }
    line
}

/// 같은 시작·길이로 함께 적는 노트들(화음 또는 단음). 음높이 오름차순.
struct Event<'a> {
    start: u64,
    length: u64,
    pitches: Vec<u8>,
    lyric: Option<&'a str>,
}

impl Event<'_> {
    fn end(&self) -> u64 {
        self.start + self.length
    }
}

/// 사건으로 묶기 전의 노트 하나: (음높이, 가사).
type Member<'a> = (u8, Option<&'a str>);

/// 클립의 노트를 사건으로 묶는다. 화음 가사는 가장 높은 음에만 둘 수 있으므로(결정 Q6),
/// 가사가 있는 음은 그보다 낮은 가사 없는 음들과만 한 화음이 된다.
fn events(clip: &Clip) -> Vec<Event<'_>> {
    let mut groups: BTreeMap<(u64, u64), Vec<Member<'_>>> = BTreeMap::new();
    for n in clip.notes() {
        let start = n.start().get();
        // 숨김 꼬리는 클립 끝에서 자른다. 노트 시작은 늘 클립 안이다.
        let end = n.end().min(clip.length());
        groups
            .entry((start, end - start))
            .or_default()
            .push((n.pitch().get(), n.lyric()));
    }
    let mut out = Vec::new();
    for ((start, length), mut notes) in groups {
        // 높은 음부터, 같은 음이면 가사 있는 것부터.
        notes.sort_by_key(|&(p, l)| (Reverse(p), l.is_none(), l));
        while !notes.is_empty() {
            let (top, lyric) = notes.remove(0);
            let mut pitches = vec![top];
            notes.retain(|&(p, l)| {
                if l.is_none() {
                    pitches.push(p);
                    false
                } else {
                    true
                }
            });
            pitches.sort_unstable();
            out.push(Event {
                start,
                length,
                pitches,
                lyric,
            });
        }
    }
    out
}

/// 겹치지 않게 성부로 나눈다. 시작 순으로 끝이 그 시작 이하인 첫 성부에 넣는다(구간 분할 탐욕).
fn voices(events: Vec<Event<'_>>) -> Vec<Vec<Event<'_>>> {
    let mut voices: Vec<Vec<Event<'_>>> = Vec::new();
    let mut ends: Vec<u64> = Vec::new();
    for ev in events {
        if let Some(i) = ends.iter().position(|&e| e <= ev.start) {
            ends[i] = ev.end();
            voices[i].push(ev);
        } else {
            ends.push(ev.end());
            voices.push(vec![ev]);
        }
    }
    voices
}

fn write_track(out: &mut String, t: &ExportTrack<'_>, bar: u64) {
    let drum = t.channel.is_drum();
    let evs = events(t.clip);
    let max_end = evs.iter().map(Event::end).max().unwrap_or(0);
    let mut writers: Vec<VoiceWriter> = voices(evs)
        .into_iter()
        .map(|v| {
            let mut w = VoiceWriter::new(drum, bar);
            w.events(&v);
            w
        })
        .collect();
    if writers.is_empty() {
        writers.push(VoiceWriter::new(drum, bar));
    }

    // 클립 길이: 가져온 클립의 길이는 노트 끝과 쉼표 끝 가운데 가장 늦은 것이다.
    let length = t.clip.length();
    if length != max_end {
        let c0 = writers[0].cursor;
        match nearest_reachable(length, max_end, c0) {
            (_, Fill::None) => {}
            (target, Fill::FirstVoice) => writers[0].rests(&lengths::rests_of(target - c0)),
            (target, Fill::RestVoice) => {
                // 첫 성부의 커서로는 닿지 않는다. 쉼표만 있는 성부를 더한다.
                let mut w = VoiceWriter::new(drum, bar);
                w.rests(&lengths::rests_of(target));
                writers.push(w);
            }
        }
    }

    let header = track_line(t);
    for (i, w) in writers.into_iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&header);
        out.push('\n');
        for line in w.finish() {
            out.push_str(&line);
            out.push('\n');
        }
    }
}

/// 클립 끝에 닿게 하는 방법. 순서는 같은 길이일 때의 우선순위다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Fill {
    /// 노트 끝이 그 길이다.
    None,
    /// 첫 성부의 커서 뒤에 쉼표를 쓴다.
    FirstVoice,
    /// 쉼표만 있는 성부를 더한다.
    RestVoice,
}

/// `length`에 가장 가까운, 다시 읽었을 때 나오는 클립 길이와 그 방법. 같은 거리면 짧은 쪽이다
/// (긴 쪽은 다음 클립과 겹칠 수 있다). 아래쪽만 보면 쉼표로 만들 수 없는 135틱, 60틱 미만의 짧은
/// 클립에서 차이가 커지므로 위쪽도 본다(S0-T5 리뷰 1 지적 1).
///
/// 고른 길이는 다시 내보낼 때 거리 0인 후보라서 같은 방법이 다시 뽑힌다(재내보내기 안정).
fn nearest_reachable(length: u64, max_end: u64, c0: u64) -> (u64, Fill) {
    let mut candidates = vec![(max_end, Fill::None)];
    for (base, fill) in [(c0, Fill::FirstVoice), (0, Fill::RestVoice)] {
        let x = length.saturating_sub(base);
        let below = lengths::largest_restable(x);
        let above = lengths::smallest_restable_from(x);
        for r in [below, above] {
            if r == 0 {
                continue;
            }
            let end = base + r;
            // 노트 끝보다 일찍 끝나는 쉼표는 길이를 바꾸지 않으므로 쓰지 않는다.
            if end <= max_end {
                candidates.push((max_end, Fill::None));
            } else {
                candidates.push((end, fill));
            }
        }
    }
    candidates
        .into_iter()
        .filter(|&(end, _)| end > 0)
        .min_by_key(|&(end, fill)| (end.abs_diff(length), end, fill))
        .unwrap_or((lengths::NOTATIONS[20].ticks, Fill::RestVoice))
}

/// 성부 블록 하나의 본문을 적는다. 커서는 파서와 같이 명목 길이로 나아간다.
struct VoiceWriter {
    drum: bool,
    bar: u64,
    cursor: u64,
    octave: Option<i32>,
    lines: Vec<String>,
    tokens: Vec<String>,
    line_start: u64,
    last_bar_line: u64,
}

const NAMES: [&str; 12] = [
    "c", "c+", "d", "d+", "e", "f", "f+", "g", "g+", "a", "a+", "b",
];

impl VoiceWriter {
    fn new(drum: bool, bar: u64) -> Self {
        VoiceWriter {
            drum,
            bar,
            cursor: 0,
            octave: None,
            lines: Vec::new(),
            tokens: Vec::new(),
            line_start: 0,
            last_bar_line: 0,
        }
    }

    fn finish(mut self) -> Vec<String> {
        if !self.tokens.is_empty() {
            self.lines.push(self.tokens.join(" "));
        }
        self.lines
    }

    /// 사건 하나를 적은 뒤. 마디 경계를 지나면 줄을 바꾸고, 지난 마디선에서 정확히 한 마디
    /// 왔으면 `|`를 적는다. 마디가 어긋난 뒤로는 `|`를 쓰지 않아 파서가 경고하지 않는다.
    fn advance(&mut self, by: u64) {
        self.cursor += by;
        if self.cursor / self.bar > self.line_start / self.bar {
            if self.cursor == self.last_bar_line + self.bar {
                self.tokens.push("|".to_owned());
                self.last_bar_line = self.cursor;
            }
            self.lines.push(self.tokens.join(" "));
            self.tokens.clear();
            self.line_start = self.cursor;
        }
    }

    fn rests(&mut self, pieces: &[u64]) {
        for &r in pieces {
            if let Some(n) = lengths::exact(r) {
                self.tokens.push(format!("r{}", n.text()));
                self.advance(r);
            }
        }
    }

    /// 옥타브를 `octave`로 옮기는 토큰. o0 아래(MIDI 0–11)는 `o0 <`로 간다.
    fn octave_tokens(&mut self, target: i32) -> Vec<String> {
        let tokens = match self.octave {
            Some(cur) if cur == target => vec![],
            Some(cur) if cur + 1 == target => vec![">".to_owned()],
            Some(cur) if cur - 1 == target => vec!["<".to_owned()],
            _ if target < 0 => vec!["o0".to_owned(), "<".to_owned()],
            _ => vec![format!("o{target}")],
        };
        self.octave = Some(target);
        tokens
    }

    /// 음 하나의 머리. 멜로디는 옥타브 토큰을 앞에 따로 둔다.
    fn head(&mut self, pitch: u8) -> (Vec<String>, String) {
        if self.drum {
            return (
                vec![],
                drum::label_of(pitch).map_or_else(|| format!("n{pitch}"), str::to_owned),
            );
        }
        let octave = i32::from(pitch / 12) - 1;
        (
            self.octave_tokens(octave),
            NAMES[usize::from(pitch % 12)].to_owned(),
        )
    }

    /// 사건의 머리: 단음은 (옥타브 토큰, 이름), 화음은 옥타브 토큰을 괄호 안에 넣는다.
    /// 드럼 `n<번호>`는 뒤에 길이를 붙일 수 없어 한 음이어도 괄호로 감싼다.
    fn event_head(&mut self, pitches: &[u8]) -> (Vec<String>, String) {
        if let [p] = pitches {
            let (pre, name) = self.head(*p);
            if !(self.drum && drum::label_of(*p).is_none()) {
                return (pre, name);
            }
            return (pre, format!("[{name}]"));
        }
        let mut inner = Vec::new();
        for &p in pitches {
            let (pre, name) = self.head(p);
            inner.extend(pre);
            inner.push(name);
        }
        (vec![], format!("[{}]", inner.join(" ")))
    }

    fn events(&mut self, evs: &[Event<'_>]) {
        let mut i = 0;
        while i < evs.len() {
            if let Some(group) = evs.get(i..i + 3).filter(|g| self.is_triplet(g)) {
                self.fill_gap(group[0].start);
                self.triplet(group);
                i += 3;
            } else {
                self.event(&evs[i]);
                i += 1;
            }
        }
    }

    /// 셋이 이어 붙어 있고, 모두 표기 길이의 2/3이며, 하나라도 그냥은 표기할 수 없을 때만
    /// 셋잇단으로 적는다(4분음표 셋을 `n(c2. d2. e2.)`로 적지 않게).
    fn is_triplet(&self, g: &[Event<'_>]) -> bool {
        let gap = i128::from(g[0].start) - i128::from(self.cursor);
        gap >= 0
            && gap % 60 == 0
            && g[1].start == g[0].end()
            && g[2].start == g[1].end()
            && g.iter()
                .all(|e| (e.length * 3) % 2 == 0 && lengths::exact(e.length * 3 / 2).is_some())
            && g.iter().any(|e| lengths::exact(e.length).is_none())
    }

    fn triplet(&mut self, g: &[Event<'_>]) {
        let mut parts = Vec::new();
        for e in g {
            let (pre, name) = self.event_head(&e.pitches);
            parts.extend(pre);
            let n = lengths::exact(e.length * 3 / 2).map_or_else(String::new, Notation::text);
            let lyric = e.lyric.map(quote).unwrap_or_default();
            parts.push(format!("{name}{n}{lyric}"));
        }
        self.tokens.push(format!("n({})", parts.join(" ")));
        self.advance(g.iter().map(|e| e.length).sum());
    }

    /// 다음 사건의 시작까지 60틱 단위 쉼표로 채우고, 남은 시작 보정을 돌려준다.
    /// 쉼표를 시작보다 늦게 끝내지 않는다(쉼표 끝도 클립 길이에 들어가므로).
    fn fill_gap(&mut self, start: u64) -> i64 {
        let gap = i128::from(start) - i128::from(self.cursor);
        if gap >= 60 {
            // gap ≥ 60이라 u64에 들어간다.
            let fill = u64::try_from(gap - gap % 60).unwrap_or(0);
            self.rests(&lengths::rests_of_multiple_of_60(fill));
        }
        // 남은 간격은 다음 음의 시작 보정이다. 틱은 u64지만 차이는 클립 길이 안이다.
        i64::try_from(i128::from(start) - i128::from(self.cursor)).unwrap_or(i64::MAX)
    }

    fn event(&mut self, ev: &Event<'_>) {
        let so = self.fill_gap(ev.start);
        let lyric = ev.lyric.map(quote).unwrap_or_default();
        let len = ev.length;
        let rem = len % WHOLE;
        if so == 0
            && len > WHOLE
            && lengths::exact(len).is_none()
            && (rem == 0 || lengths::exact(rem).is_some())
        {
            // 온음표 타이 사슬. 가사는 첫 조각에만 둔다(이어지는 쪽 가사는 오류).
            let mut pieces = vec![WHOLE; usize::try_from(len / WHOLE).unwrap_or(0)];
            if rem > 0 {
                pieces.push(rem);
            }
            let last = pieces.len() - 1;
            for (i, &p) in pieces.iter().enumerate() {
                let (pre, name) = self.event_head(&ev.pitches);
                self.tokens.extend(pre);
                let n = lengths::exact(p).map_or_else(String::new, Notation::text);
                let lyric = if i == 0 { lyric.as_str() } else { "" };
                let tie = if i == last { "" } else { "&" };
                self.tokens.push(format!("{name}{n}{lyric}{tie}"));
                self.advance(p);
            }
            return;
        }
        let notation = match lengths::exact(len) {
            Some(n) => n,
            None => lengths::nearest(len),
        };
        // 길이 차이는 u32 노트 길이와 표기 길이(≤ 6720) 사이라 i64에 들어간다.
        let lo = i64::try_from(i128::from(len) - i128::from(notation.ticks)).unwrap_or(i64::MAX);
        let mut suffix = String::new();
        if so != 0 || lo != 0 {
            let _ = write!(suffix, "~{so:+}");
            if lo != 0 {
                let _ = write!(suffix, ":{lo:+}");
            }
        }
        let (pre, name) = self.event_head(&ev.pitches);
        self.tokens.extend(pre);
        self.tokens
            .push(format!("{name}{}{suffix}{lyric}", notation.text()));
        self.advance(notation.ticks);
    }
}
