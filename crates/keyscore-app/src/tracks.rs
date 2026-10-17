//! 트랙 목록·트랜스포트 바·인스펙터·상태줄의 계산(S2-T5b, S2-T5c). GPUI와 무관한 함수라 단위 테스트한다.

use std::collections::{BTreeSet, HashMap};

use keyscore_core::{
    Bank, Channel, Key, OutputPort, PitchClass, Project, Tempo, Tick, TimeSignature, TrackColor,
    TrackId, Velocity,
};

/// 템포 필드가 받는 범위(BPM의 10배). 너무 느리거나 빠른 값은 입력 실수로 본다.
const TEMPO_TENTHS: std::ops::RangeInclusive<u32> = 100..=9999;

/// 그 포트에서 비어 있는 가장 낮은 채널(SEQ-7). 16채널이 다 찼으면 `None`.
#[must_use]
pub fn lowest_free_channel(project: &Project, port: &OutputPort) -> Option<Channel> {
    let used: BTreeSet<u8> = project
        .tracks()
        .iter()
        .filter(|t| t.port() == port)
        .map(|t| t.channel().get())
        .collect();
    (1..=16)
        .find(|c| !used.contains(c))
        .and_then(|c| Channel::new(c).ok())
}

/// 같은 포트·채널을 쓰는 트랙들(id, 프로그램).
type Members = Vec<(TrackId, Option<u8>)>;

/// SEQ-8: 같은 포트·채널을 둘 이상의 트랙이 쓰는데 프로그램이 서로 다른 트랙들. 프로그램이 없는 트랙도
/// 「없음」이라는 값으로 센다.
#[must_use]
pub fn program_conflicts(project: &Project) -> BTreeSet<TrackId> {
    let mut groups: HashMap<(&OutputPort, u8), Members> = HashMap::new();
    for t in project.tracks() {
        groups
            .entry((t.port(), t.channel().get()))
            .or_default()
            .push((t.id(), t.program().map(keyscore_core::Program::get)));
    }
    groups
        .into_values()
        .filter(|members| members.len() > 1 && members.iter().any(|(_, p)| *p != members[0].1))
        .flatten()
        .map(|(id, _)| id)
        .collect()
}

/// 위치 표시 `마디.박.틱`(1부터 센다, 7.1).
#[must_use]
pub fn format_position(project: &Project, at: Tick) -> String {
    let p = project.meter_map().position(at);
    format!("{}.{}.{}", p.bar() + 1, p.beat() + 1, p.tick())
}

/// 템포 필드 표시(소수 한 자리).
#[must_use]
pub fn format_tempo(tempo: Tempo) -> String {
    // 600,000,000 / µs = BPM × 10. 정수 반올림으로 표시가 입력과 같게 돌아온다.
    let us = u64::from(tempo.micros_per_quarter());
    let tenths = (600_000_000 + us / 2) / us;
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// 템포 필드 입력(BPM, 소수 한 자리까지, 10.0–999.9).
///
/// # Errors
///
/// 숫자가 아니거나 범위 밖이면 사람이 읽을 이유.
pub fn parse_tempo(text: &str) -> Result<Tempo, String> {
    let text = text.trim();
    let bad = || format!("템포 「{text}」를 읽을 수 없다(10.0–999.9 BPM, 소수 한 자리까지)");
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) || frac.len() > 1 {
        return Err(bad());
    }
    if !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad());
    }
    let whole: u32 = whole.parse().map_err(|_| bad())?;
    let frac: u32 = if frac.is_empty() {
        0
    } else {
        frac.parse().map_err(|_| bad())?
    };
    let tenths = whole
        .checked_mul(10)
        .and_then(|w| w.checked_add(frac))
        .ok_or_else(bad)?;
    if !TEMPO_TENTHS.contains(&tenths) {
        return Err(bad());
    }
    let us = (600_000_000 + u64::from(tenths) / 2) / u64::from(tenths);
    Tempo::from_micros_per_quarter(u32::try_from(us).map_err(|_| bad())?).map_err(|_| bad())
}

/// 박자표 필드 표시(`4/4`).
#[must_use]
pub fn format_meter(signature: TimeSignature) -> String {
    format!("{}/{}", signature.numerator(), signature.denominator())
}

/// 박자표 필드 입력(`3/4`, 분모는 2의 거듭제곱).
///
/// # Errors
///
/// 형식이나 값이 틀리면 사람이 읽을 이유.
pub fn parse_meter(text: &str) -> Result<TimeSignature, String> {
    let bad = || format!("박자표 「{}」를 읽을 수 없다(예: 4/4, 6/8)", text.trim());
    let (num, den) = text.split_once('/').ok_or_else(bad)?;
    let num: u8 = num.trim().parse().map_err(|_| bad())?;
    let den: u16 = den.trim().parse().map_err(|_| bad())?;
    TimeSignature::new(num, den).map_err(|_| bad())
}

/// 조성 선택지: 없음, 장조 12, 단조 12.
#[must_use]
pub fn key_choices() -> Vec<Key> {
    let mut keys = vec![Key::None];
    let classes: Vec<PitchClass> = (0..12).filter_map(|c| PitchClass::new(c).ok()).collect();
    keys.extend(classes.iter().map(|&c| Key::Major(c)));
    keys.extend(classes.iter().map(|&c| Key::Minor(c)));
    keys
}

/// 음이름(샵 표기).
const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// 조성 표시(`C장조`, `A단조`, `조성 없음`).
#[must_use]
pub fn key_label(key: Key) -> String {
    match key {
        Key::None => "조성 없음".to_owned(),
        Key::Major(c) => format!("{}장조", NAMES[usize::from(c.get())]),
        Key::Minor(c) => format!("{}단조", NAMES[usize::from(c.get())]),
    }
}

/// 포트 표시(가상 포트는 기본 가상 포트 이름).
#[must_use]
pub fn port_label(port: &OutputPort, virtual_name: &str) -> String {
    match port {
        OutputPort::Virtual => virtual_name.to_owned(),
        OutputPort::Named(name) => name.clone(),
    }
}

/// 뱅크 입력(0–16383, 비우면 없음).
///
/// # Errors
///
/// 숫자가 아니거나 범위 밖이면 사람이 읽을 이유.
pub fn parse_bank(text: &str) -> Result<Option<Bank>, String> {
    let _ = text;
    todo!()
}

/// 기본 벨로시티 입력(1–127).
///
/// # Errors
///
/// 숫자가 아니거나 범위 밖이면 사람이 읽을 이유.
pub fn parse_velocity(text: &str) -> Result<Velocity, String> {
    let _ = text;
    todo!()
}

/// 트랙 색 인자: 팔레트 이름(`red` …) 또는 번호 0–11.
///
/// # Errors
///
/// 모르는 이름이거나 범위 밖이면 사람이 읽을 이유.
pub fn parse_color(text: &str) -> Result<TrackColor, String> {
    let _ = text;
    todo!()
}

/// 색 메뉴의 표시 이름.
#[must_use]
pub fn color_label(color: TrackColor) -> String {
    let _ = color;
    todo!()
}

/// 상태줄 가운데: `트랙 이름 · 포트 chN`. 이름이 비면 `트랙 N`(목록 순서).
#[must_use]
pub fn status_track_text(project: &Project, track: TrackId, virtual_name: &str) -> String {
    let _ = (project, track, virtual_name);
    todo!()
}
