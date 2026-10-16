//! `.ksq` 프로젝트 파일(REQ IO-1, 9.5).
//!
//! JSON이고, 노트는 `[Δt, 길이, 음높이, 벨로시티]`에 선택 속성 객체(`offset`, `lyric`)를 붙인다
//! (`docs/decisions/2026-10-12-ksq-note-format.md`). 쓰기는 결정적이다: 키 순서가 고정되고 2칸
//! 들여쓰기이며 노트·템포·박자표 항목은 한 줄에 하나라서 git diff가 노트 단위로 나온다.
//! 읽기는 모르는 키를 보존하고 없는 키는 기본값으로 채운다. `TrackId`·`ClipId`는 쓰지 않고
//! 읽을 때 새로 발급한다.

use std::fmt::{self, Write as _};
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    Bank, Channel, Clip, Key, MeterMap, ModelError, Note, OutputPort, Pitch, PitchClass,
    PitchOffset, Program, Project, Tempo, TempoMap, Tick, TimeSignature, Track, TrackColor,
    Velocity,
};

/// 이 코드가 쓰는 형식 버전.
pub const FORMAT_VERSION: u64 = 1;

/// 파일 압축. 읽기는 내용으로 판별하므로 쓰기에만 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// 평문 `.ksq`(기본, git 친화).
    None,
    /// gzip `.ksqz`.
    Gzip,
}

/// 읽기는 성공했지만 사용자에게 알릴 것.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning(String);

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `.ksq` 읽기·쓰기 오류. 내용 오류는 어디서 났는지(`tracks[0].clips[2].notes[15]` 꼴)를 담는다.
#[derive(Debug, Error)]
pub enum KsqError {
    #[error("JSON 문법 오류({line}행 {column}열): {message}")]
    Json {
        line: usize,
        column: usize,
        message: String,
    },
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
    #[error("{path}: {source}")]
    Model { path: String, source: ModelError },
    #[error("파일 입출력 오류: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------- 쓰기

/// 평문 `.ksq` 텍스트로 쓴다.
#[must_use]
pub fn to_string(project: &Project) -> String {
    let mut fields = vec![
        (
            "format_version".to_owned(),
            Item::Inline(FORMAT_VERSION.to_string()),
        ),
        (
            "key".to_owned(),
            Item::Inline(key_text(project.key(), project.key_extra())),
        ),
        (
            "tempo".to_owned(),
            Item::Lines(
                project
                    .tempo_map()
                    .changes()
                    .map(|(t, v)| format!("[{}, {}]", t.get(), v.micros_per_quarter()))
                    .collect(),
            ),
        ),
        (
            "meter".to_owned(),
            Item::Lines(
                project
                    .meter_map()
                    .changes()
                    .map(|(t, s)| format!("[{}, {}, {}]", t.get(), s.numerator(), s.denominator()))
                    .collect(),
            ),
        ),
        (
            "tracks".to_owned(),
            Item::Objects(project.tracks().iter().map(track_fields).collect()),
        ),
    ];
    push_extra(&mut fields, project.extra());
    let mut out = String::new();
    write_object(&mut out, &fields, 0);
    out.push('\n');
    out
}

/// 객체 필드의 값. 한 줄 값, 한 줄에 하나씩 쓰는 배열, 객체 배열 중 하나다.
enum Item {
    Inline(String),
    Lines(Vec<String>),
    Objects(Vec<Vec<(String, Item)>>),
}

fn track_fields(t: &Track) -> Vec<(String, Item)> {
    let opt = |v: Option<String>| v.unwrap_or_else(|| "null".to_owned());
    let mut fields = vec![
        ("name".to_owned(), Item::Inline(json_string(t.name()))),
        (
            "color".to_owned(),
            Item::Inline(t.color().index().to_string()),
        ),
        (
            "port".to_owned(),
            Item::Inline(match t.port() {
                OutputPort::Virtual => "\"virtual\"".to_owned(),
                OutputPort::Named(name) => {
                    let mut parts = vec![format!("\"named\": {}", json_string(name))];
                    parts.extend(
                        t.port_extra()
                            .iter()
                            .map(|(k, v)| format!("{}: {}", json_string(k), inline(v))),
                    );
                    format!("{{{}}}", parts.join(", "))
                }
            }),
        ),
        (
            "channel".to_owned(),
            Item::Inline(t.channel().get().to_string()),
        ),
        (
            "bank".to_owned(),
            Item::Inline(opt(t.bank().map(|b| b.get().to_string()))),
        ),
        (
            "program".to_owned(),
            Item::Inline(opt(t.program().map(|p| p.get().to_string()))),
        ),
        (
            "default_velocity".to_owned(),
            Item::Inline(t.default_velocity().get().to_string()),
        ),
        ("mute".to_owned(), Item::Inline(t.mute().to_string())),
        ("solo".to_owned(), Item::Inline(t.solo().to_string())),
        (
            "lyric_lang".to_owned(),
            Item::Inline(opt(t.lyric_lang().map(json_string))),
        ),
        (
            "clips".to_owned(),
            Item::Objects(t.clips().map(|(_, c)| clip_fields(c)).collect()),
        ),
    ];
    push_extra(&mut fields, t.extra());
    fields
}

fn clip_fields(c: &Clip) -> Vec<(String, Item)> {
    let mut previous = 0u64;
    let notes = c
        .notes()
        .iter()
        .map(|n| {
            // 노트는 전체 순서로 정렬되어 있어 시작이 줄지 않는다. 그래서 Δt ≥ 0이다.
            let delta = n.start().get() - previous;
            previous = n.start().get();
            note_line(n, delta)
        })
        .collect();
    let mut fields = vec![
        (
            "start".to_owned(),
            Item::Inline(c.start().get().to_string()),
        ),
        ("length".to_owned(), Item::Inline(c.length().to_string())),
        (
            "name".to_owned(),
            Item::Inline(c.name().map_or_else(|| "null".to_owned(), json_string)),
        ),
        (
            "color".to_owned(),
            Item::Inline(
                c.color()
                    .map_or_else(|| "null".to_owned(), |k| k.index().to_string()),
            ),
        ),
        ("notes".to_owned(), Item::Lines(notes)),
    ];
    push_extra(&mut fields, c.extra());
    fields
}

fn note_line(n: &Note, delta: u64) -> String {
    let mut line = format!(
        "[{delta}, {}, {}, {}",
        n.length(),
        n.pitch().get(),
        n.velocity().get()
    );
    let mut attrs: Vec<String> = Vec::new();
    if n.offset() != PitchOffset::ZERO {
        attrs.push(format!("\"offset\": {}", n.offset().units()));
    }
    if let Some(lyric) = n.lyric() {
        attrs.push(format!("\"lyric\": {}", json_string(lyric)));
    }
    if let Some(Value::Object(extra)) = n.extra().and_then(|e| serde_json::from_str(e).ok()) {
        attrs.extend(
            extra
                .iter()
                .map(|(k, v)| format!("{}: {}", json_string(k), inline(v))),
        );
    }
    if !attrs.is_empty() {
        let _ = write!(line, ", {{{}}}", attrs.join(", "));
    }
    line.push(']');
    line
}

/// 모르는 키를 알려진 키 뒤에 이름 순으로 붙인다.
fn push_extra(fields: &mut Vec<(String, Item)>, extra: &Map<String, Value>) {
    fields.extend(
        by_name(extra)
            .into_iter()
            .map(|(k, v)| (k.clone(), Item::Inline(inline(v)))),
    );
}

/// 객체의 항목을 이름 순으로. `serde_json::Map`의 순서는 feature에 따라 다르다. 다른 crate가
/// `preserve_order`를 켜면(워크스페이스에서 gpui-kit, S2-T1) 넣은 순서가 된다. 그래서 순서에 기대지
/// 않고 여기서 정렬해 쓰기를 결정적으로 둔다.
fn by_name(map: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut items: Vec<_> = map.iter().collect();
    items.sort_by(|a, b| a.0.cmp(b.0));
    items
}

fn write_object(out: &mut String, fields: &[(String, Item)], indent: usize) {
    out.push_str("{\n");
    for (i, (key, item)) in fields.iter().enumerate() {
        pad(out, indent + 2);
        let _ = write!(out, "{}: ", json_string(key));
        match item {
            Item::Inline(text) => out.push_str(text),
            Item::Lines(lines) if lines.is_empty() => out.push_str("[]"),
            Item::Lines(lines) => {
                out.push_str("[\n");
                for (j, line) in lines.iter().enumerate() {
                    pad(out, indent + 4);
                    out.push_str(line);
                    out.push_str(if j + 1 < lines.len() { ",\n" } else { "\n" });
                }
                pad(out, indent + 2);
                out.push(']');
            }
            Item::Objects(objects) if objects.is_empty() => out.push_str("[]"),
            Item::Objects(objects) => {
                out.push_str("[\n");
                for (j, object) in objects.iter().enumerate() {
                    pad(out, indent + 4);
                    write_object(out, object, indent + 4);
                    out.push_str(if j + 1 < objects.len() { ",\n" } else { "\n" });
                }
                pad(out, indent + 2);
                out.push(']');
            }
        }
        out.push_str(if i + 1 < fields.len() { ",\n" } else { "\n" });
    }
    pad(out, indent);
    out.push('}');
}

fn pad(out: &mut String, n: usize) {
    out.extend(std::iter::repeat_n(' ', n));
}

fn json_string(s: &str) -> String {
    Value::String(s.to_owned()).to_string()
}

/// 한 줄 JSON. `serde_json`의 압축 표기와 달리 `, `·`: ` 뒤에 공백을 둬 노트 줄과 모양을 맞춘다.
fn inline(v: &Value) -> String {
    match v {
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(inline).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            by_name(map)
                .into_iter()
                .map(|(k, v)| format!("{}: {}", json_string(k), inline(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    }
}

/// `key` 값. 객체이면 알려진 키 뒤에 모르는 키를 이름 순으로 붙인다(리뷰 1 지적 1).
fn key_text(key: Key, extra: &Map<String, Value>) -> String {
    let (tonic, mode) = match key {
        Key::None => return "\"none\"".to_owned(),
        Key::Major(pc) => (pc, "major"),
        Key::Minor(pc) => (pc, "minor"),
    };
    let mut parts = vec![
        format!("\"tonic\": {}", tonic.get()),
        format!("\"mode\": \"{mode}\""),
    ];
    parts.extend(
        by_name(extra)
            .into_iter()
            .map(|(k, v)| format!("{}: {}", json_string(k), inline(v))),
    );
    format!("{{{}}}", parts.join(", "))
}

/// 바이트로 쓴다. `Compression::Gzip`이면 gzip으로 감싼다.
///
/// # Errors
///
/// gzip 인코더의 입출력 오류면 [`KsqError::Io`]. 메모리 버퍼에 쓰므로 실제로는 생기지 않지만,
/// 패닉 대신 오류로 돌려준다.
pub fn to_bytes(project: &Project, compression: Compression) -> Result<Vec<u8>, KsqError> {
    let text = to_string(project);
    match compression {
        Compression::None => Ok(text.into_bytes()),
        Compression::Gzip => {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(text.as_bytes())?;
            Ok(encoder.finish()?)
        }
    }
}

// ---------------------------------------------------------------- 읽기

/// 평문 `.ksq` 텍스트를 읽는다.
///
/// # Errors
///
/// JSON 문법 오류, 형식 오류(타입·범위), 모델 불변식 위반이면 [`KsqError`].
pub fn from_str(text: &str) -> Result<(Project, Vec<Warning>), KsqError> {
    let root: Value = serde_json::from_str(text).map_err(|e| KsqError::Json {
        line: e.line(),
        column: e.column(),
        message: e.to_string(),
    })?;
    let mut warnings = Vec::new();
    let mut fields = Fields::of(root, String::new())?;
    let mut project = Project::new();

    let version = fields
        .take("format_version")
        .map(|v| fields.u64_of(&v, "format_version"))
        .transpose()?;
    if let Some(v) = version
        && v > FORMAT_VERSION
    {
        warnings.push(Warning(format!(
            "파일 형식 버전 {v}은 이 앱이 아는 버전({FORMAT_VERSION})보다 새롭다. 모르는 내용은 보존하지만 해석하지 못한 부분이 있을 수 있다"
        )));
    }
    if let Some(v) = fields.take("key") {
        let (key, extra) = read_key(v, "key")?;
        project.set_key(key);
        project.set_key_extra(extra);
    }
    if let Some(v) = fields.take("tempo") {
        let mut changes: Vec<(Tick, Tempo)> = Vec::new();
        for (i, entry, path) in array_items(v, "tempo")? {
            let [tick, us] = fixed::<2>(&entry, &path)?;
            let tick = Tick::new(u64_value(tick, &format!("{path}[0]"))?);
            check_change_order(i, tick, changes.last().map(|c| c.0), &path)?;
            let us = u32_value(us, &format!("{path}[1]"))?;
            let tempo = Tempo::from_micros_per_quarter(us).map_err(model(&path))?;
            changes.push((tick, tempo));
        }
        // 항목마다 set하면 매번 전체를 다시 계산해 O(n²)이다(리뷰 1 지적 3).
        if !changes.is_empty() {
            *project.tempo_map_mut() = TempoMap::from_sorted(changes);
        }
    }
    if let Some(v) = fields.take("meter") {
        let mut changes: Vec<(Tick, TimeSignature)> = Vec::new();
        for (i, entry, path) in array_items(v, "meter")? {
            let [tick, num, den] = fixed::<3>(&entry, &path)?;
            let tick = Tick::new(u64_value(tick, &format!("{path}[0]"))?);
            check_change_order(i, tick, changes.last().map(|c| c.0), &path)?;
            let num = small::<u8>(num, &format!("{path}[1]"))?;
            let den = small::<u16>(den, &format!("{path}[2]"))?;
            let sig = TimeSignature::new(num, den).map_err(model(&path))?;
            changes.push((tick, sig));
        }
        if !changes.is_empty() {
            *project.meter_map_mut() = MeterMap::from_sorted(changes);
        }
    }
    if let Some(v) = fields.take("tracks") {
        for (_, entry, path) in array_items(v, "tracks")? {
            let track = read_track(entry, &path)?;
            project.push_track(track).map_err(model(&path))?;
        }
    }
    project.set_extra(fields.rest());
    Ok((project, warnings))
}

/// 템포·박자표 변화는 틱 0부터 시작하고 틱이 엄격히 늘어야 한다. 그래야 쓰기 → 읽기가 같은 맵이다.
fn check_change_order(
    index: usize,
    tick: Tick,
    last: Option<Tick>,
    path: &str,
) -> Result<(), KsqError> {
    let ok = if index == 0 {
        tick == Tick::ZERO
    } else {
        last.is_some_and(|l| tick > l)
    };
    if ok {
        Ok(())
    } else {
        Err(invalid(
            path,
            "첫 항목은 틱 0이어야 하고, 틱은 엄격히 늘어야 한다",
        ))
    }
}

fn read_key(v: Value, path: &str) -> Result<(Key, Map<String, Value>), KsqError> {
    if v.as_str() == Some("none") {
        return Ok((Key::None, Map::new()));
    }
    let mut f = Fields::of(v, path.to_owned())
        .map_err(|_| invalid(path, "\"none\" 또는 {\"tonic\", \"mode\"} 객체여야 한다"))?;
    let tonic = f
        .take("tonic")
        .ok_or_else(|| invalid(path, "tonic이 없다"))?;
    let tonic =
        PitchClass::new(small::<u8>(&tonic, &format!("{path}.tonic"))?).map_err(model(path))?;
    let mode = f.take("mode");
    match mode
        .as_ref()
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(path, "mode가 없다"))?
    {
        "major" => Ok((Key::Major(tonic), f.rest())),
        "minor" => Ok((Key::Minor(tonic), f.rest())),
        _ => Err(invalid(path, "mode는 \"major\" 또는 \"minor\"여야 한다")),
    }
}

fn read_track(v: Value, path: &str) -> Result<Track, KsqError> {
    let mut f = Fields::of(v, path.to_owned())?;
    let name = f
        .take("name")
        .map(|v| f.string_of(&v, "name"))
        .transpose()?
        .unwrap_or_default();
    let color = match f.take("color") {
        Some(v) => {
            TrackColor::new(f.small_of::<u8>(&v, "color")?).map_err(model(&f.path("color")))?
        }
        None => TrackColor::new(0).map_err(model(path))?,
    };
    let channel = match f.take("channel") {
        Some(v) => {
            Channel::new(f.small_of::<u8>(&v, "channel")?).map_err(model(&f.path("channel")))?
        }
        None => Channel::new(1).map_err(model(path))?,
    };
    let mut track = Track::new(name, channel, color);
    if let Some(v) = f.take("port") {
        let (port, extra) = read_port(v, &f.path("port"))?;
        track.set_port(port);
        track.set_port_extra(extra);
    }
    if let Some(v) = f.take("bank").filter(|v| !v.is_null()) {
        let bank = Bank::new(f.small_of::<u16>(&v, "bank")?).map_err(model(&f.path("bank")))?;
        track.set_bank(Some(bank));
    }
    if let Some(v) = f.take("program").filter(|v| !v.is_null()) {
        let program =
            Program::new(f.small_of::<u8>(&v, "program")?).map_err(model(&f.path("program")))?;
        track.set_program(Some(program));
    }
    if let Some(v) = f.take("default_velocity") {
        track.set_default_velocity(Velocity::new(f.small_of::<u16>(&v, "default_velocity")?));
    }
    if let Some(v) = f.take("mute") {
        track.set_mute(f.bool_of(&v, "mute")?);
    }
    if let Some(v) = f.take("solo") {
        track.set_solo(f.bool_of(&v, "solo")?);
    }
    if let Some(v) = f.take("lyric_lang").filter(|v| !v.is_null()) {
        track.set_lyric_lang(Some(f.string_of(&v, "lyric_lang")?));
    }
    if let Some(v) = f.take("clips") {
        for (_, entry, clip_path) in array_items(v, &f.path("clips"))? {
            let clip = read_clip(entry, &clip_path)?;
            track.insert_clip(clip).map_err(model(&clip_path))?;
        }
    }
    track.set_extra(f.rest());
    Ok(track)
}

fn read_port(v: Value, path: &str) -> Result<(OutputPort, Map<String, Value>), KsqError> {
    if v.as_str() == Some("virtual") {
        return Ok((OutputPort::Virtual, Map::new()));
    }
    let shape = || invalid(path, "\"virtual\" 또는 {\"named\": 이름}이어야 한다");
    let mut f = Fields::of(v, path.to_owned()).map_err(|_| shape())?;
    let name = f.take("named").ok_or_else(shape)?;
    let name = name.as_str().ok_or_else(shape)?.to_owned();
    Ok((OutputPort::Named(name), f.rest()))
}

fn read_clip(v: Value, path: &str) -> Result<Clip, KsqError> {
    let mut f = Fields::of(v, path.to_owned())?;
    let start = f
        .take("start")
        .map(|v| f.u64_of(&v, "start"))
        .transpose()?
        .unwrap_or(0);
    let notes_path = f.path("notes");
    let mut notes = Vec::new();
    if let Some(v) = f.take("notes") {
        let Value::Array(items) = v else {
            return Err(invalid(&notes_path, "배열이어야 한다"));
        };
        notes.reserve(items.len());
        let mut previous = 0u64;
        for (index, entry) in items.into_iter().enumerate() {
            notes.push((read_note(entry, &notes_path, index, &mut previous)?, index));
        }
    }
    let length = match f.take("length") {
        Some(v) => f.u64_of(&v, "length")?,
        // 「없는 키는 기본값」을 지키면서 길이 0 클립을 피한다: 가장 늦은 노트 끝, 최소 1.
        None => notes.iter().map(|(n, _)| n.end()).max().unwrap_or(1).max(1),
    };
    let mut clip = Clip::new(Tick::new(start), length).map_err(model(path))?;
    if let Some(v) = f.take("name").filter(|v| !v.is_null()) {
        clip.set_name(f.string_of(&v, "name")?);
    }
    if let Some(v) = f.take("color").filter(|v| !v.is_null()) {
        let color =
            TrackColor::new(f.small_of::<u8>(&v, "color")?).map_err(model(&f.path("color")))?;
        clip.set_color(Some(color));
    }
    // 저장된 분할 상태를 되살리므로 숨김 꼬리(끝 > 길이)도 받는다. 시작은 클립 안이어야 한다.
    // 한꺼번에 넣어 역순 입력에서도 O(n log n)이다(리뷰 1 지적 4).
    let indices: Vec<usize> = notes.iter().map(|(_, i)| *i).collect();
    clip.restore_notes(notes.into_iter().map(|(n, _)| n).collect())
        .map_err(|(at, source)| KsqError::Model {
            path: format!("{notes_path}[{}]", indices[at]),
            source,
        })?;
    clip.set_extra(f.rest());
    Ok(clip)
}

/// 노트 하나를 읽는다. 오류가 없으면 경로 문자열을 만들지 않는다. 노트 10만 개 파일 읽기의 대부분이
/// 이 함수라서, 노트마다 경로를 미리 만들면 그것만으로 수십 ms가 든다(S0-T6 벤치).
fn read_note(
    v: Value,
    notes_path: &str,
    index: usize,
    previous: &mut u64,
) -> Result<Note, KsqError> {
    let path = || format!("{notes_path}[{index}]");
    let at = |i: usize| format!("{notes_path}[{index}][{i}]");
    let Value::Array(mut items) = v else {
        return Err(invalid(&path(), "노트는 배열이어야 한다"));
    };
    if !(4..=5).contains(&items.len()) {
        return Err(invalid(
            &path(),
            "노트는 [Δt, 길이, 음높이, 벨로시티] 또는 그 뒤에 선택 속성 객체 하나다",
        ));
    }
    let number = |i: usize| {
        items[i]
            .as_u64()
            .ok_or_else(|| invalid(&at(i), "0 이상의 정수여야 한다"))
    };
    let delta = number(0)?;
    let length = u32::try_from(number(1)?).map_err(|_| invalid(&at(1), "범위를 넘는 정수다"))?;
    let pitch = u8::try_from(number(2)?).map_err(|_| invalid(&at(2), "범위를 넘는 정수다"))?;
    let velocity = u16::try_from(number(3)?).map_err(|_| invalid(&at(3), "범위를 넘는 정수다"))?;
    let model_err = |source| KsqError::Model {
        path: path(),
        source,
    };
    let start = previous
        .checked_add(delta)
        .ok_or_else(|| invalid(&path(), "시작 틱이 u64를 넘는다"))?;
    *previous = start;
    let pitch = Pitch::new(pitch).map_err(model_err)?;
    let mut note =
        Note::new(Tick::new(start), length, pitch, Velocity::new(velocity)).map_err(model_err)?;
    if items.len() == 5
        && let Some(attrs) = items.pop()
    {
        let mut f = Fields::of(attrs, at(4))?;
        if let Some(v) = f.take("offset") {
            let units = v
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| invalid(&f.path("offset"), "정수여야 한다"))?;
            let offset = PitchOffset::new(units).map_err(model_err)?;
            note = note.with_offset(offset).map_err(model_err)?;
        }
        if let Some(v) = f.take("lyric") {
            note = note.with_lyric(f.string_of(&v, "lyric")?);
        }
        let rest = f.rest();
        if !rest.is_empty() {
            note = note.with_extra(Some(inline(&Value::Object(rest)).into_boxed_str()));
        }
    }
    Ok(note)
}

/// 객체 하나의 필드를 경로와 함께 다룬다. 꺼내지 않은 키가 「모르는 키」로 남는다.
struct Fields {
    map: Map<String, Value>,
    base: String,
}

impl Fields {
    /// 값을 소유해 받는다. 복제하면 트랙 → 클립 → 노트 배열이 깊이마다 다시 복사된다.
    fn of(v: Value, base: String) -> Result<Fields, KsqError> {
        match v {
            Value::Object(map) => Ok(Fields { map, base }),
            _ => Err(invalid(
                if base.is_empty() { "파일" } else { &base },
                "객체여야 한다",
            )),
        }
    }

    fn path(&self, key: &str) -> String {
        if self.base.is_empty() {
            key.to_owned()
        } else {
            format!("{}.{key}", self.base)
        }
    }

    /// 키를 꺼낸다. 꺼낸 키는 「모르는 키」에서 빠진다.
    fn take(&mut self, key: &str) -> Option<Value> {
        self.map.remove(key)
    }

    fn rest(self) -> Map<String, Value> {
        self.map
    }

    fn u64_of(&self, v: &Value, key: &str) -> Result<u64, KsqError> {
        u64_value(v, &self.path(key))
    }

    fn small_of<T: TryFrom<u64>>(&self, v: &Value, key: &str) -> Result<T, KsqError> {
        small(v, &self.path(key))
    }

    fn bool_of(&self, v: &Value, key: &str) -> Result<bool, KsqError> {
        v.as_bool()
            .ok_or_else(|| invalid(&self.path(key), "true 또는 false여야 한다"))
    }

    fn string_of(&self, v: &Value, key: &str) -> Result<String, KsqError> {
        v.as_str()
            .map(str::to_owned)
            .ok_or_else(|| invalid(&self.path(key), "문자열이어야 한다"))
    }
}

/// 배열의 (번호, 항목, 경로). 항목을 소유해 넘긴다.
fn array_items(v: Value, path: &str) -> Result<Vec<(usize, Value, String)>, KsqError> {
    let Value::Array(items) = v else {
        return Err(invalid(path, "배열이어야 한다"));
    };
    Ok(items
        .into_iter()
        .enumerate()
        .map(|(i, item)| (i, item, format!("{path}[{i}]")))
        .collect())
}

/// 길이가 정확히 `N`인 배열.
fn fixed<'a, const N: usize>(v: &'a Value, path: &str) -> Result<[&'a Value; N], KsqError> {
    let items = v
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or_else(|| invalid(path, &format!("원소 {N}개짜리 배열이어야 한다")))?;
    let mut out = [&Value::Null; N];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item;
    }
    Ok(out)
}

fn u64_value(v: &Value, path: &str) -> Result<u64, KsqError> {
    v.as_u64()
        .ok_or_else(|| invalid(path, "0 이상의 정수여야 한다"))
}

fn u32_value(v: &Value, path: &str) -> Result<u32, KsqError> {
    small(v, path)
}

fn small<T: TryFrom<u64>>(v: &Value, path: &str) -> Result<T, KsqError> {
    let n = u64_value(v, path)?;
    T::try_from(n).map_err(|_| invalid(path, "범위를 넘는 정수다"))
}

fn invalid(path: &str, message: &str) -> KsqError {
    KsqError::Invalid {
        path: path.to_owned(),
        message: message.to_owned(),
    }
}

fn model(path: &str) -> impl Fn(ModelError) -> KsqError + '_ {
    move |source| KsqError::Model {
        path: path.to_owned(),
        source,
    }
}

/// 바이트를 읽는다. 첫 두 바이트가 gzip 표시(1f 8b)면 풀어서 읽는다.
///
/// # Errors
///
/// [`from_str`]와 같고, gzip이 깨졌거나 UTF-8이 아니면 [`KsqError::Invalid`].
pub fn from_bytes(bytes: &[u8]) -> Result<(Project, Vec<Warning>), KsqError> {
    from_bytes_limited(bytes, MAX_UNPACKED_BYTES)
}

/// gzip을 풀었을 때의 최대 크기. 노트 10만 개 파일이 3 MB 남짓이라 넉넉하다. 작은 파일이 거대하게
/// 풀리는 압축 폭탄으로 메모리를 다 쓰지 않게 한다(리뷰 1 제안 8).
const MAX_UNPACKED_BYTES: u64 = 512 * 1024 * 1024;

fn from_bytes_limited(bytes: &[u8], limit: u64) -> Result<(Project, Vec<Warning>), KsqError> {
    let text = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut text = String::new();
        flate2::read::GzDecoder::new(bytes)
            .take(limit.saturating_add(1))
            .read_to_string(&mut text)
            .map_err(|e| invalid("파일", &format!("gzip을 풀 수 없다: {e}")))?;
        if text.len() as u64 > limit {
            return Err(invalid(
                "파일",
                &format!("풀린 크기가 상한 {limit}바이트를 넘는다"),
            ));
        }
        text
    } else {
        String::from_utf8(bytes.to_vec()).map_err(|_| invalid("파일", "UTF-8 텍스트가 아니다"))?
    };
    from_str(&text)
}

// ---------------------------------------------------------------- 파일

/// 파일로 쓴다. 같은 디렉터리의 임시 파일에 쓰고 fsync한 뒤 이름을 바꾼다. 그래서 쓰는 도중
/// 실패하거나 프로세스가 죽어도 원래 파일은 온전하다. 실패하면 임시 파일을 지운다.
///
/// # Errors
///
/// 입출력 실패면 [`KsqError::Io`].
pub fn write_file(
    path: &Path,
    project: &Project,
    compression: Compression,
) -> Result<(), KsqError> {
    let bytes = to_bytes(project, compression)?;
    // 심볼릭 링크면 링크를 일반 파일로 바꾸지 않고 대상 파일을 바꾼다(리뷰 1 지적 5).
    let path = resolve_symlink(path)?;
    let path = path.as_path();
    let temp = temp_path(path);
    let result = write_then_rename(&temp, path, &bytes);
    if result.is_err() {
        // 만들지 못했으면 지울 것도 없다. 지우기 실패는 원래 오류를 가리지 않게 무시한다.
        let _ = fs::remove_file(&temp);
    }
    result.map_err(KsqError::Io)
}

fn write_then_rename(temp: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let original = fs::metadata(path).ok();
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    // 처음부터 원본 권한으로 만든다. 기본 권한(0644)으로 만든 뒤 바꾸면 그 사이 다른 사용자가
    // 빈 파일을 열어 쥐고, 나중에 쓰인 내용을 읽을 수 있다(S0-T6 리뷰 3 지적 1).
    #[cfg(unix)]
    if let Some(meta) = &original {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(meta.permissions().mode() & 0o777);
    }
    let mut file = options.open(temp)?;
    // 원래 파일의 권한을 유지한다(리뷰 1 제안 7). 내용을 쓰기 전에 맞춰야, 쓰는 동안 임시 파일이
    // 원본보다 넓은 권한(예: 0600 원본에 0644)으로 열려 있지 않다(리뷰 2 지적 1). 새 파일이면
    // 기본 권한 그대로다. 이미 연 핸들로 쓰므로 원본이 읽기 전용이어도 쓰기는 된다.
    // umask가 만들 때의 권한을 더 좁혔을 수 있어, 원본 권한을 정확히 다시 맞춘다(좁은 쪽에서 넓히는 것뿐이다).
    if let Some(meta) = original {
        file.set_permissions(meta.permissions())?;
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)?;
    // 이름 바꾸기 자체를 디스크에 남긴다. 상대 경로의 부모는 빈 경로라 "."로 바꾼다(리뷰 1 제안 10).
    // 디렉터리 fsync가 안 되는 파일 시스템도 있어 실패는 무시한다.
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let _ = fs::File::open(dir).and_then(|d| d.sync_all());
    Ok(())
}

/// 심볼릭 링크를 따라간 실제 파일 경로. 링크가 아니거나 경로가 없으면 그대로다. 대상이 없는
/// 링크(dangling)는 링크가 가리키는 경로에 새로 만든다.
fn resolve_symlink(path: &Path) -> Result<PathBuf, KsqError> {
    let mut current = path.to_path_buf();
    // 링크의 링크도 따라간다. 순환 링크로 끝없이 돌지 않게 횟수를 제한한다.
    for _ in 0..40 {
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = fs::read_link(&current)?;
                current = match current.parent() {
                    Some(parent) if target.is_relative() => parent.join(target),
                    _ => target,
                };
            }
            _ => return Ok(current),
        }
    }
    Err(invalid(
        &path.display().to_string(),
        "심볼릭 링크가 너무 깊거나 순환한다",
    ))
}

/// 같은 디렉터리의 임시 파일 경로. 다른 디렉터리면 rename이 원자적이지 않다.
fn temp_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map_or_else(|| "ksq".into(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.tmp-{}-{n}", std::process::id()))
}

/// 파일을 읽는다.
///
/// # Errors
///
/// [`from_bytes`]와 같고, 읽기 실패면 [`KsqError::Io`].
pub fn read_file(path: &Path) -> Result<(Project, Vec<Warning>), KsqError> {
    from_bytes(&fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Channel, Clip, Key, Note, OutputPort, Pitch, PitchClass, PitchOffset, Program, Tempo, Tick,
        TimeSignature, Track, TrackColor, Velocity,
    };

    fn note(start: u64, length: u32, pitch: u8) -> Note {
        Note::new(
            Tick::new(start),
            length,
            Pitch::new(pitch).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap()
    }

    /// 형식의 모든 부분이 한 번씩 나오는 프로젝트.
    fn sample() -> Project {
        let mut p = Project::new();
        p.set_key(Key::Minor(PitchClass::new(9).unwrap()));
        p.tempo_map_mut()
            .set(Tick::new(7680), Tempo::from_bpm(90).unwrap());
        p.meter_map_mut()
            .set(Tick::new(7680), TimeSignature::new(6, 8).unwrap());
        let mut lead = Track::new(
            "Lead",
            Channel::new(1).unwrap(),
            TrackColor::new(3).unwrap(),
        );
        lead.set_program(Some(Program::new(73).unwrap()));
        lead.set_lyric_lang(Some("ko".into()));
        let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
        clip.set_name("후렴");
        clip.insert_note(note(0, 480, 60)).unwrap();
        clip.insert_note(note(480, 480, 62).with_lyric("사"))
            .unwrap();
        let bent = note(960, 960, 64)
            .with_offset(PitchOffset::new(PitchOffset::SEMITONE / 2).unwrap())
            .unwrap();
        clip.insert_note(bent.with_lyric("랑")).unwrap();
        lead.insert_clip(clip).unwrap();
        let mut drums = Track::new("Drums \"GM\"", Channel::DRUMS, TrackColor::new(11).unwrap());
        drums.set_port(OutputPort::Named("IAC Bus 1".into()));
        drums.set_mute(true);
        p.push_track(lead).unwrap();
        p.push_track(drums).unwrap();
        p
    }

    #[test]
    fn ksq_snapshot_format() {
        insta::assert_snapshot!(to_string(&sample()));
    }

    #[test]
    fn ksq_write_read_write_is_identical() {
        let text = to_string(&sample());
        let (back, warnings) = from_str(&text).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(to_string(&back), text);
    }

    #[test]
    fn ksq_missing_keys_take_defaults() {
        let (p, warnings) = from_str("{}").unwrap();
        assert!(warnings.is_empty());
        assert_eq!(to_string(&p), to_string(&Project::new()));

        let (p, _) =
            from_str(r#"{"tracks": [{"clips": [{"notes": [[100, 20, 60, 5]]}]}]}"#).unwrap();
        let t = &p.tracks()[0];
        assert_eq!((t.name(), t.channel().get(), t.color().index()), ("", 1, 0));
        assert_eq!(t.port(), &OutputPort::Virtual);
        assert_eq!(t.default_velocity(), Velocity::from_midi1(100).unwrap());
        let (_, c) = t.clips().next().unwrap();
        // 길이가 없으면 가장 늦은 노트 끝(최소 1).
        assert_eq!((c.start(), c.length(), c.name()), (Tick::ZERO, 120, None));
        let (p, _) = from_str(r#"{"tracks": [{"clips": [{}]}]}"#).unwrap();
        assert_eq!(p.tracks()[0].clips().next().unwrap().1.length(), 1);
    }

    #[test]
    fn ksq_unknown_keys_survive_read_write() {
        let text = r#"{
  "format_version": 1,
  "zz_top": {"a": [1, 2]},
  "tracks": [
    {
      "name": "T",
      "future_track_key": true,
      "clips": [
        {
          "length": 960,
          "loop": {"count": 2},
          "notes": [
            [0, 10, 60, 5, {"lyric": "가", "per_note_cc": [[74, 9]]}],
            [0, 10, 61, 5, {"mpe": 1}]
          ]
        }
      ]
    }
  ]
}"#;
        let (p, warnings) = from_str(text).unwrap();
        assert!(warnings.is_empty());
        let out = to_string(&p);
        for needle in [
            r#""zz_top": {"a": [1, 2]}"#,
            r#""future_track_key": true"#,
            r#""loop": {"count": 2}"#,
            r#"{"lyric": "가", "per_note_cc": [[74, 9]]}"#,
            r#"[0, 10, 61, 5, {"mpe": 1}]"#,
        ] {
            assert!(out.contains(needle), "다시 쓴 파일에 {needle} 없음:\n{out}");
        }
        // 두 번 왕복해도 같다.
        assert_eq!(to_string(&from_str(&out).unwrap().0), out);
    }

    #[test]
    fn ksq_newer_format_version_opens_with_warning() {
        let (p, warnings) = from_str(r#"{"format_version": 7, "tracks": []}"#).unwrap();
        assert_eq!(p.tracks().len(), 0);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].to_string().contains('7'), "{}", warnings[0]);
        let (_, warnings) = from_str(r#"{"format_version": 1}"#).unwrap();
        assert!(warnings.is_empty());
    }

    #[test]
    fn ksq_invalid_files_report_where() {
        let cases = [
            (r#"{"tracks": [{"channel": 0}]}"#, "tracks[0].channel"),
            (
                r#"{"tracks": [{}, {"clips": [{"start": 0, "length": 10}, {"start": 5, "length": 10}]}]}"#,
                "tracks[1].clips[1]",
            ),
            (
                r#"{"tracks": [{"clips": [{"length": 10, "notes": [[0, 5, 60, 1], [20, 5, 60, 1]]}]}]}"#,
                "tracks[0].clips[0].notes[1]",
            ),
            (
                r#"{"tracks": [{"clips": [{"notes": [[0, 5, 127, 1, {"offset": 33554432}]]}]}]}"#,
                "tracks[0].clips[0].notes[0]",
            ),
            (
                r#"{"tracks": [{"clips": [{"notes": [[0, 5, 60]]}]}]}"#,
                "tracks[0].clips[0].notes[0]",
            ),
            (r#"{"tracks": [{"name": 3}]}"#, "tracks[0].name"),
            (r#"{"tempo": [[0, 0]]}"#, "tempo[0]"),
            (r#"{"meter": [[0, 4, 3]]}"#, "meter[0]"),
            (r#"{"key": "lydian"}"#, "key"),
            (
                r#"{"tracks": [{"clips": [{"notes": [[-1, 5, 60, 1]]}]}]}"#,
                "tracks[0].clips[0].notes[0]",
            ),
        ];
        for (text, path) in cases {
            let err = from_str(text).expect_err(text);
            let shown = err.to_string();
            assert!(
                shown.starts_with(path),
                "{text}\n경로 {path}가 아니라: {shown}"
            );
        }
        let err = from_str("{\n  \"tracks\": [,]\n}").unwrap_err();
        assert!(matches!(err, KsqError::Json { line: 2, .. }), "{err:?}");
    }

    #[test]
    fn ksq_gzip_larger_than_limit_is_rejected() {
        // 압축 폭탄 방지(S0-T6 리뷰 1 제안 8). 상한을 작게 주고 시험한다.
        let big = to_bytes(&sample(), Compression::Gzip).unwrap();
        let unpacked = to_string(&sample()).len() as u64;
        assert!(from_bytes_limited(&big, unpacked).is_ok());
        assert!(matches!(
            from_bytes_limited(&big, unpacked - 1),
            Err(KsqError::Invalid { .. })
        ));
    }

    #[test]
    fn ksq_gzip_round_trip_and_detection() {
        let p = sample();
        let plain = to_bytes(&p, Compression::None).unwrap();
        let gz = to_bytes(&p, Compression::Gzip).unwrap();
        assert_eq!(&gz[..2], &[0x1f, 0x8b]);
        assert!(gz.len() < plain.len());
        assert_eq!(plain, to_string(&p).into_bytes());
        for bytes in [&plain, &gz] {
            assert_eq!(to_string(&from_bytes(bytes).unwrap().0), to_string(&p));
        }
        assert!(matches!(
            from_bytes(&[0x1f, 0x8b, 0, 1, 2]),
            Err(KsqError::Invalid { .. })
        ));
        assert!(matches!(
            from_bytes(&[0xff, 0xfe]),
            Err(KsqError::Invalid { .. })
        ));
    }
}
