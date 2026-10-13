use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
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
/// - 클립 길이가 노트 끝도 아니고 쉼표 합(15틱 단위)으로도 닿지 않으면, 다시 읽었을 때 나오는
///   길이 가운데 위아래로 가장 가까운 것으로 적는다(같은 거리면 짧은 쪽). 차이는 클립이 150틱
///   이상이면 7틱 이하, 60–149틱이면 15틱 이하, 60틱 미만이면 (60 − 클립 길이)틱 이하다
///   (`docs/decisions/2026-10-12-ktext-implementation-details.md`).
///
/// 적는 방식: 같은 시작·길이의 노트는 화음으로 묶고, 겹치는 것은 같은 이름의 블록(성부)으로
/// 나눈다. 표기 가능한 길이로 그리드에 놓인 노트는 접미 없이, 그 밖은 가장 가까운 길이에
/// `~<±시작>[:<±길이>]`를 붙인다. 온음표보다 긴 노트는 온음표 배수 + 표기 길이면 타이로
/// (조각 16개까지), 셋 연속으로 표기 길이의 2/3인 노트는 셋잇단으로 적는다.
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
    /// 앞 사건이 아직 울리는 성부에 겹쳐 적는다(`voices`의 쉼표 예산). 커서를 64분음표만큼만
    /// 옮기고 나머지 길이는 `~+…:+차이`로 적는다.
    folded: bool,
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
        // 높은 음부터, 같은 음이면 가사 있는 것부터. 가장 높은 음이 가사 없는 음 모두와 화음이 되고,
        // 남은 가사 있는 음은 하나씩 따로 선다. 한 번 훑어 나눈다(같은 시작 노트가 많아도 선형).
        notes.sort_by_key(|&(p, l)| (Reverse(p), l.is_none(), l));
        let mut rest = notes.into_iter();
        let Some((top, lyric)) = rest.next() else {
            continue;
        };
        let mut pitches = vec![top];
        let mut singles = Vec::new();
        for (p, l) in rest {
            match l {
                None => pitches.push(p),
                Some(_) => singles.push((p, l)),
            }
        }
        pitches.sort_unstable();
        out.push(Event {
            start,
            length,
            pitches,
            lyric,
            folded: false,
        });
        for (p, l) in singles {
            out.push(Event {
                start,
                length,
                pitches: vec![p],
                lyric: l,
                folded: false,
            });
        }
    }
    out
}

/// 한 성부 안에서 커서가 실제 시각을 앞지를 수 있는 양의 한도. 가장 가까운 표기가 실제보다
/// 길면(예: 1틱 노트를 64분음표로) 다음 음은 음수 시작 보정을 받는데, 짧은 노트가 이어지면 이
/// 보정이 쌓여 파서의 한도(`MAX_START_OFFSET`)를 넘을 수 있다. 한도를 넘을 사건은 다른 성부로
/// 보낸다. 480은 표기 길이 사이 가장 큰 간격(960)의 절반이라, 녹음된 레가토 노트가 한 성부에 남는다.
const MAX_DRIFT: u64 = 480;
const _: () = assert!(MAX_DRIFT <= crate::parse::MAX_START_OFFSET.unsigned_abs());

/// 성부 앞 쉼표 예산(온음표 쉼표 수)은 이 계수 × (사건 수 + 마지막 사건 시작까지의 온음표 수)에
/// 기본량을 더한 것이다(결정 2026-10-13 A). 단위를 노트 끝이 아니라 시작으로 세는 것은, 가져온 텍스트가 시작에
/// 닿으려면 쉼표·음 토큰을 실제로 써야 하기 때문이다. 노트 끝은 길이 보정 하나(`c1~+0:+4294000000`)로
/// 부풀릴 수 있어 예산을 사실상 무한하게 만든다(S0-T5 리뷰 4 지적 1). 계수 16은 결정의 범위(16–32)
/// 가운데 출력/입력 비가 가장 작은 쪽이다. 늦게 들어오는 그리드 패드에 `~`가 붙지 않게 한 것(리뷰
/// 4 지적 2)은 이 계수다. 계수와 기본량은 리뷰 5의 351/352마디 경계 테스트가 고정한다.
const REST_BUDGET_PER_UNIT: u64 = 16;
const REST_BUDGET_BASE: u64 = 64;
/// 성부가 이 수 이하인 동안은 예산을 보지 않는다(결정 2026-10-13 A). 이때 쓴 쉼표도 예산에서 빼 두므로
/// 이 성부들의 쉼표와 예산이 따로 쌓이지 않는다. 계수 16에서는 이 면제가 출력을 바꾸지 않는다:
/// 성부 16개의 쉼표 합은 16 × 마지막 시작까지의 온음표 수 이하라 늘 예산 안이다(리뷰 5 지적 2).
/// 계수를 16보다 낮추면 그때부터 작동한다.
const FREE_VOICES: usize = 16;
/// 온음표 쉼표 하나의 출력 바이트(4/4에서 `r1 |`와 줄바꿈). 성부 블록 헤더 줄의 바이트를 예산
/// 단위로 환산할 때 쓴다(S0-T5 리뷰 6 지적 1).
const REST_UNIT_BYTES: u64 = 5;
/// 성부 블록 헤더 줄에서 비용을 매기지 않는 바이트. 보통 이름(`@track Lead ch1 prog0 bank0`)은
/// 이 안이라 예산 경계가 바뀌지 않는다. 헤더 하나는 성부를 강제하는 입력 사건(겹친 노트 하나는
/// `c~+0:+N`처럼 몇 바이트) 하나에 붙으므로 이만큼은 입력에 선형이다.
const HEADER_FREE_BYTES: u64 = 64;

/// 겹치지 않게 성부로 나눈다. 시작 순으로, 이전 사건이 끝났고 커서가 `MAX_DRIFT` 넘게 앞지르지
/// 않을 첫 성부(가장 앞 번호)에 넣는다(구간 분할 탐욕). 가장 일찍 비는 성부가 아니라 앞 번호를
/// 고르는 것은 악구가 한 성부에 이어지게 하려는 것이다. 사건이 시작 순이라 한 번 빈 성부는
/// 계속 비어 있으므로, 바쁜 성부는 빌 시각의 힙에, 빈 성부는 번호의 순서 집합에 두면 n log n이다.
///
/// `busy`는 성부 커서의 위쪽 한계다. 쉼표는 다음 시작을 넘지 않으므로 사건 앞의 커서는
/// `max(busy, 시작)` 이하이고, 사건은 커서를 표기 길이(단음) 또는 실제 길이(타이·셋잇단)만큼
/// 옮긴다. 그래서 내보낸 시작 보정은 −`MAX_DRIFT` 이상이다.
///
/// 쉼표 예산: 성부 블록은 모두 클립 시작에서 시작하므로, 새 성부나 오래 쉰 성부에 사건을 넣으면
/// 그 사이를 쉼표로 다시 채운다. 긴 쉼표 뒤에 겹친 노트가 많으면 이 비용이 겹침 수 × 클립 길이가
/// 된다(S0-T5 리뷰 3 지적 1: 7.5 KB 입력 → 1.4 MB). 그래서 채울 온음표 쉼표 수를 예산에서 뺀다.
/// 성부가 `FREE_VOICES` 이하로 남으면 예산을 보지 않는다. 그 밖에서 예산을 넘으면 새 성부를 열거나
/// 오래 쉰 성부를 쓰는 대신, 커서를 가장 늦게 둘 수 있는 성부에 넣는다.
/// - 그 성부의 앞 사건이 아직 울리면 겹쳐 적는다(`folded`: 커서는 64분음표만큼, 나머지 길이는
///   `~+시작:+길이차`). 가져오기가 이미 받는 표기라 문법을 넓히지 않는다.
/// - 거슬러 접기: 그 성부의 커서가 마지막 사건의 표기 길이 때문에 한도(`MAX_DRIFT`)보다 앞서 있으면,
///   마지막 사건도 같은 방식(64분음표 명목 + 길이 보정)으로 다시 적어 커서를 되돌린다(`low`).
///
/// 겹쳐 적은 음은 그리드 위에 있어도 `~`를 받으므로 엄격 모드로 읽히지 않는다. 그래서 성부 17개가
/// 넘게 필요하고 예산까지 바닥난 클립에서만 이 경로를 탄다.
///
/// 크기: 출력은 입력에 선형이고, 상수는 256배 이내다(결정 2026-10-13 개정). 예산과 면제 성부의
/// 쉼표는 합쳐도 예산(사건 수와 마지막 시작까지의 온음표 수에 선형) 안이다. 성부 블록마다 다시 쓰는
/// `@track` 줄도 `HEADER_FREE_BYTES`를 넘는 부분을 예산에서 빼므로, 긴 트랙 이름이 성부 수만큼
/// 반복되어 「이름 길이 × 성부 수」가 되지 않는다(S0-T5 리뷰 6 지적 1). 예산 밖에서 고른 성부는
/// 커서가 가장 늦은 성부라 다른 성부가 지난 구간을 다시 채우지 않는다. 남는 경우는 모든 성부의
/// 커서가 (마지막 사건을 접어도) 시작보다 `MAX_DRIFT` 넘게 앞서 새 성부를 열 수밖에 없을 때다.
/// 접은 사건은 커서를 60틱 옮기므로, 이는 약 540틱 안에 사건이 성부 수 × 9개 넘게 몰릴 때뿐이다.
/// 텍스트로 그런 밀도를 적으려면 가져오기 쪽 시작 보정 한도(±3840) 때문에 입력에도 블록이
/// 비슷한 수만큼 있어야 한다.
///
/// 상수가 큰 까닭은 압축 입력이다. 느슨 모드에서 `l1..` 뒤의 `c` 한 바이트는 사건 하나와 온음표
/// 1.75개를 만들고, 예산은 그 둘에 각각 온음표 쉼표 16개를 허락하는데, 내보내기는 온음표마다
/// `r1 |`와 줄바꿈(5바이트)을 쓴다. 그래서 바이트당 약 16 × 2.75 × 5 = 220바이트에 다가간다.
/// 측정(release, 리뷰 6 뒤): 압축 쉼표·음(`c`, `r`, `c~+59`, `[ce]`, `n(ccc)`) 최대 30,000바이트 뒤
/// 겹친 노트·몰린 사건, 트랙 이름 1–6,000바이트, 박자표 다섯 가지를 섞어 최악 221배였다(`l1..` 뒤
/// `c`, 겹침 32). 긴 트랙 이름은 이보다 작다. 길이를 매번 적은 입력(`r1.. `)은 33배 이하다.
/// core API로 만든 클립은 「몰린 사건 수 × 클립 길이」까지 갈 수 있다.
///
/// 성부 수의 상한: 64분음표(60틱)보다 짧은 노트도 커서를 60틱 옮기므로, 앞지름이 한도에 닿은
/// 성부는 60틱마다 사건 하나만 받는다. 그래서 짧은 노트가 촘촘하면 성부 수가 겹침 수 ×
/// (60 / 가장 짧은 노트 길이) 근처까지, 최대 약 60배로 는다(1틱 노트 겹침 3 → 176성부,
/// S0-T5 리뷰 2 지적 3). 읽기 좋은 출력이 필요해지면(S3) 다시 본다.
fn voices(events: Vec<Event<'_>>, header_cost: u64) -> Vec<Vec<Event<'_>>> {
    // 사건은 시작 순이라 마지막 사건이 가장 늦게 시작한다.
    let last_start = events.last().map_or(0, |e| e.start);
    let units = u64::try_from(events.len()).unwrap_or(u64::MAX) + last_start / WHOLE;
    let mut budget = units
        .saturating_mul(REST_BUDGET_PER_UNIT)
        .saturating_add(REST_BUDGET_BASE);
    let mut voices: Vec<Vec<Event<'_>>> = Vec::new();
    let mut busy: Vec<u64> = Vec::new();
    // 마지막 사건을 접어 적었다면의 busy. 예산 밖에서 그 사건을 거슬러 접을 수 있다.
    let mut low: Vec<u64> = Vec::new();
    let mut end: Vec<u64> = Vec::new();
    let mut free_at: Vec<u64> = Vec::new();
    // (빌 시각, 성부 번호). 빌 시각 = max(실제 끝, busy − MAX_DRIFT). 바뀐 성부의 옛 항목은
    // 꺼낼 때 `free_at`과 비교해 버린다.
    let mut pending: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
    let mut free: BTreeSet<usize> = BTreeSet::new();
    // 모든 성부의 (low, 번호). 예산 밖에서 커서를 가장 늦게 둘 수 있는 성부를 찾는다.
    let mut by_low: BTreeSet<(u64, usize)> = BTreeSet::new();
    let fold_advance = lengths::NOTATIONS[20].ticks;
    for mut ev in events {
        while let Some(&Reverse((at, i))) = pending.peek() {
            if at > ev.start {
                break;
            }
            pending.pop();
            if free_at[i] == at {
                free.insert(i);
            }
        }
        // 성부 커서 `b`에서 시작까지 채울 온음표 쉼표 수.
        let cost = |b: u64| ev.start.saturating_sub(b) / WHOLE;
        let first = free.first().copied();
        // 성부가 `FREE_VOICES` 이하로 남는 동안은 예산을 보지 않는다(비용은 빼 둔다).
        let fits = match first {
            Some(f) => voices.len() <= FREE_VOICES || cost(busy[f]) <= budget,
            None => voices.len() < FREE_VOICES || cost(0) + header_cost <= budget,
        };
        let chosen = if fits {
            first
        } else {
            let latest = by_low
                .range(..=(ev.start + MAX_DRIFT, usize::MAX))
                .next_back()
                .map(|&(_, g)| g);
            if let Some(g) = latest
                && busy[g] > ev.start + MAX_DRIFT
            {
                // 마지막 사건을 접어 적으면 커서가 한도 안으로 돌아온다.
                if let Some(last) = voices[g].last_mut() {
                    last.folded = true;
                }
                busy[g] = low[g];
            }
            latest.or(first)
        };
        let opened = chosen.is_none();
        let i = chosen.unwrap_or_else(|| {
            voices.push(Vec::new());
            busy.push(0);
            low.push(0);
            end.push(0);
            free_at.push(0);
            voices.len() - 1
        });
        free.remove(&i);
        by_low.remove(&(low[i], i));
        budget = budget.saturating_sub(cost(busy[i]));
        if opened {
            budget = budget.saturating_sub(header_cost);
        }
        ev.folded = end[i] > ev.start;
        let advance = if ev.folded {
            fold_advance
        } else {
            lengths::nearest(ev.length).ticks.max(ev.length)
        };
        let base = busy[i].max(ev.start);
        busy[i] = base + advance;
        low[i] = base + fold_advance;
        end[i] = end[i].max(ev.end());
        free_at[i] = end[i].max(busy[i].saturating_sub(MAX_DRIFT));
        by_low.insert((low[i], i));
        pending.push(Reverse((free_at[i], i)));
        voices[i].push(ev);
    }
    voices
}

fn write_track(out: &mut String, t: &ExportTrack<'_>, bar: u64) {
    let drum = t.channel.is_drum();
    let header = track_line(t);
    // 성부 블록마다 다시 쓰는 `@track` 줄(빈 줄 포함) 가운데 `HEADER_FREE_BYTES`를 넘는 부분을
    // 온음표 쉼표 비용으로 환산한다.
    let header_cost = u64::try_from(header.len() + 2)
        .unwrap_or(u64::MAX)
        .saturating_sub(HEADER_FREE_BYTES)
        .div_ceil(REST_UNIT_BYTES);
    let evs = events(t.clip);
    let max_end = evs.iter().map(Event::end).max().unwrap_or(0);
    let mut writers: Vec<VoiceWriter> = voices(evs, header_cost)
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

/// 타이 사슬의 조각 수 한도. 넘으면 가장 가까운 표기에 `~+0:+<차이>`를 붙인다. 사슬은 길이에
/// 비례해 커지므로, 한도가 없으면 길이 보정 토큰 하나(`c4~+0:+4294963200`, 18바이트)가 온음표
/// 1,118,481개(약 5 MB)로 내보내진다(S0-T5 리뷰 2 지적 1). 16조각(4/4 16마디)이면 실제 곡의 긴
/// 지속음은 타이로 읽기 좋게 남고, 노트 하나의 출력은 상수로 묶인다.
const MAX_TIE_PIECES: u64 = 16;

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
            && g.iter().all(|e| !e.folded)
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
        let pieces_count = len / WHOLE + u64::from(rem > 0);
        if so == 0
            && !ev.folded
            && len > WHOLE
            && pieces_count <= MAX_TIE_PIECES
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
        let notation = if ev.folded {
            // 겹쳐 적는 음은 커서를 가장 적게 옮긴다(`voices`의 busy 계산과 같다).
            lengths::NOTATIONS[20]
        } else {
            match lengths::exact(len) {
                Some(n) => n,
                None => lengths::nearest(len),
            }
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
