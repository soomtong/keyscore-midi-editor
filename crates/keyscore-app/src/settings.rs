//! 앱 설정 파일(`~/Library/Application Support/KeyScore/settings.json`).
//!
//! 패널 열림·크기와 테마(S2-T1), 키맵(S2-T6a), 입력·메트로놈(S2-T6c)을 저장한다.
//! 모르는 키는 보존하고 없는 키는 기본값을 쓴다(REQ IO-1과 같은 태도). 쓰기는 임시 파일 + rename이라
//! 쓰는 도중 죽어도 원래 파일이 남는다.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use std::time::Duration;

use keyscore_core::transform::Grid;
use keyscore_core::{Channel, OutputPort, Pitch};
use keyscore_midi::MetronomeConfig;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::actions::args::{KeyLayout, LengthCode};

/// 패널 크기의 허용 범위(논리 픽셀). 밖의 값은 기본값으로 되돌린다.
pub const PANEL_SIZE_RANGE: std::ops::RangeInclusive<f32> = 120.0..=1200.0;

/// 테마 선택.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// macOS 외관을 따른다.
    #[default]
    System,
    Light,
    Dark,
}

/// 7.1 레이아웃의 접히는 패널 셋.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LayoutSettings {
    pub track_list_open: bool,
    pub track_list_width: f32,
    pub inspector_open: bool,
    pub inspector_width: f32,
    pub bottom_open: bool,
    pub bottom_height: f32,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        LayoutSettings {
            track_list_open: true,
            track_list_width: 220.0,
            inspector_open: true,
            inspector_width: 260.0,
            bottom_open: true,
            bottom_height: 160.0,
        }
    }
}

impl LayoutSettings {
    /// `layout` 객체를 필드마다 읽는다. 잘못된 필드 하나는 그 필드만 기본값으로 두고 경고한다.
    /// 모르는 키는 돌려준다(다시 쓸 때 보존한다).
    fn read(mut map: Map<String, Value>, warnings: &mut Vec<String>) -> (Self, Map<String, Value>) {
        let mut layout = LayoutSettings::default();
        let mut bool_field =
            |name: &str, slot: &mut bool, map: &mut Map<String, Value>| match map.remove(name) {
                None => {}
                Some(Value::Bool(b)) => *slot = b,
                Some(_) => warnings.push(format!("layout.{name}는 참·거짓이어야 해 기본값을 쓴다")),
            };
        bool_field("track_list_open", &mut layout.track_list_open, &mut map);
        bool_field("inspector_open", &mut layout.inspector_open, &mut map);
        bool_field("bottom_open", &mut layout.bottom_open, &mut map);
        for (name, slot) in [
            ("track_list_width", &mut layout.track_list_width),
            ("inspector_width", &mut layout.inspector_width),
            ("bottom_height", &mut layout.bottom_height),
        ] {
            match map.remove(name) {
                None => {}
                // 설정 파일의 크기는 f32로 충분하다(논리 픽셀). 범위는 아래 sanitize가 본다.
                #[allow(clippy::cast_possible_truncation)]
                Some(Value::Number(n)) if n.as_f64().is_some() => {
                    *slot = n.as_f64().unwrap_or_default() as f32;
                }
                Some(_) => warnings.push(format!("layout.{name}는 숫자여야 해 기본값을 쓴다")),
            }
        }
        (layout, map)
    }

    /// 범위 밖·유한하지 않은 크기를 기본값으로 바꾸고, 바꾼 항목 이름을 돌려준다.
    fn sanitize(&mut self) -> Vec<&'static str> {
        let defaults = LayoutSettings::default();
        let mut fixed = Vec::new();
        for (name, value, default) in [
            (
                "track_list_width",
                &mut self.track_list_width,
                defaults.track_list_width,
            ),
            (
                "inspector_width",
                &mut self.inspector_width,
                defaults.inspector_width,
            ),
            (
                "bottom_height",
                &mut self.bottom_height,
                defaults.bottom_height,
            ),
        ] {
            if !value.is_finite() || !PANEL_SIZE_RANGE.contains(value) {
                *value = default;
                fixed.push(name);
            }
        }
        fixed
    }
}

/// 키맵 설정(KM-2, KM-11). 사용자 오버라이드는 설정이 아니라 `keymap.json`에 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeymapSettings {
    pub preset: crate::keymap::Preset,
    pub swap_alt_cmd: bool,
}

/// 입력 설정(설정 > 입력). 음 키 레이아웃(P3)은 S3 Step 입력이 읽는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputSettings {
    pub key_layout: KeyLayout,
    /// Step 입력의 기본 길이(REQ 5.6 「`0`은 길이를 기본값으로 … 기본 길이 자체는 설정에서 바꿀 수 있다」).
    pub step_default_length: LengthCode,
    /// 음 키 뒤 옥타브 숫자를 기다리는 시간(ms, 0–2000, REQ 5.6 「기본 300 ms(설정 가능)」).
    pub octave_wait_ms: u32,
}

/// 옥타브 대기 시간 상한(ms). 그보다 길면 입력이 멈춘 것처럼 보인다.
pub const OCTAVE_WAIT_MAX_MS: u32 = 2000;

impl Default for InputSettings {
    fn default() -> Self {
        InputSettings {
            key_layout: KeyLayout::Piano,
            step_default_length: LengthCode::Quarter,
            octave_wait_ms: 300,
        }
    }
}

/// 메트로놈 설정(MET-1, 설정 > 메트로놈). 값은 늘 범위 안이다(읽을 때 고친다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetronomeSettings {
    /// 클릭을 보낼 포트. 트랙 포트와 같은 모양이다.
    pub port: OutputPort,
    /// 채널 1–16
    pub channel: u8,
    /// 강박(마디 첫 박) 노트 0–127
    pub accent: u8,
    /// 약박 노트 0–127
    pub beat: u8,
    /// 클릭 길이(ms) 1–10000
    pub length_ms: u32,
}

impl Default for MetronomeSettings {
    /// 가상 포트, 채널 10, 강박 37, 약박 42(MET-1), 50 ms(S1-T5의 클릭 길이).
    fn default() -> Self {
        MetronomeSettings {
            port: OutputPort::Virtual,
            channel: 10,
            accent: 37,
            beat: 42,
            length_ms: 50,
        }
    }
}

impl InputSettings {
    fn read(mut map: Map<String, Value>, warnings: &mut Vec<String>) -> (Self, Map<String, Value>) {
        let mut input = InputSettings::default();
        if let Some(v) = map.remove("key_layout") {
            match serde_json::from_value(v) {
                Ok(layout) => input.key_layout = layout,
                Err(_) => warnings
                    .push("input.key_layout은 piano 또는 solfege라 기본값을 쓴다".to_string()),
            }
        }
        if let Some(v) = map.remove("step_default_length") {
            match serde_json::from_value(v) {
                Ok(code) => input.step_default_length = code,
                Err(_) => warnings.push(
                    "input.step_default_length는 whole·half·quarter·eighth·sixteenth·thirtysecond·sixtyfourth라 기본값을 쓴다"
                        .to_string(),
                ),
            }
        }
        if let Some(v) = map.remove("octave_wait_ms") {
            match v.as_u64().and_then(|ms| u32::try_from(ms).ok()) {
                Some(ms) if ms <= OCTAVE_WAIT_MAX_MS => input.octave_wait_ms = ms,
                _ => warnings.push(format!(
                    "input.octave_wait_ms는 0–{OCTAVE_WAIT_MAX_MS} 정수라 기본값을 쓴다"
                )),
            }
        }
        (input, map)
    }

    fn write(self, map: &mut Map<String, Value>) {
        let layout = match self.key_layout {
            KeyLayout::Piano => "piano",
            KeyLayout::Solfege => "solfege",
        };
        map.insert("key_layout".to_string(), Value::from(layout));
        map.insert(
            "step_default_length".to_string(),
            Value::from(length_code_name(self.step_default_length)),
        );
        map.insert(
            "octave_wait_ms".to_string(),
            Value::from(self.octave_wait_ms),
        );
    }
}

/// 길이 코드의 설정 파일 표기(키맵 인자와 같다).
const fn length_code_name(code: LengthCode) -> &'static str {
    match code {
        LengthCode::Whole => "whole",
        LengthCode::Half => "half",
        LengthCode::Quarter => "quarter",
        LengthCode::Eighth => "eighth",
        LengthCode::Sixteenth => "sixteenth",
        LengthCode::ThirtySecond => "thirtysecond",
        LengthCode::SixtyFourth => "sixtyfourth",
    }
}

/// 퀀타이즈 설정(EDT-2, REQ 5.8 `q` 「현재 설정」). 앱 설정이고 모든 프로젝트에 같다(결정
/// `2026-10-26-s3-t3b-note-edit-details.md` 3-A). 값은 늘 범위 안이다(읽을 때 고친다).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantizeSettings {
    /// 그리드 1/4–1/64.
    pub grid: Grid,
    /// 셋잇단 그리드(2/3).
    pub triplet: bool,
    /// 강도 0–100%.
    pub strength: u8,
    /// 스윙 0–75%.
    pub swing: u8,
}

impl Default for QuantizeSettings {
    /// 1/16, 셋잇단 끔, 강도 100%, 스윙 0%(결정 3-A).
    fn default() -> Self {
        QuantizeSettings {
            grid: Grid::Sixteenth,
            triplet: false,
            strength: 100,
            swing: 0,
        }
    }
}

/// 퀀타이즈 그리드의 설정 파일 표기. 순서는 거친 것부터다(설정 시트의 한 칸).
pub(crate) const GRIDS: [(Grid, &str); 5] = [
    (Grid::Quarter, "1/4"),
    (Grid::Eighth, "1/8"),
    (Grid::Sixteenth, "1/16"),
    (Grid::ThirtySecond, "1/32"),
    (Grid::SixtyFourth, "1/64"),
];

impl QuantizeSettings {
    /// core 연산에 넘길 설정. 공개 필드에 범위 밖 값이 들어 있으면 그 값만 범위 끝으로 줄인다.
    ///
    /// # Panics
    ///
    /// 없다. 줄인 값은 core의 범위 안이다.
    #[must_use]
    pub fn to_core(&self) -> keyscore_core::transform::QuantizeSettings {
        keyscore_core::transform::QuantizeSettings::new(
            self.grid,
            self.triplet,
            self.strength.min(100),
            self.swing.min(75),
        )
        .expect("범위로 줄인 강도·스윙")
    }

    fn read(mut map: Map<String, Value>, warnings: &mut Vec<String>) -> (Self, Map<String, Value>) {
        let mut q = QuantizeSettings::default();
        if let Some(v) = map.remove("grid") {
            match GRIDS.iter().find(|(_, name)| v.as_str() == Some(name)) {
                Some((grid, _)) => q.grid = *grid,
                None => warnings.push(
                    "quantize.grid는 1/4·1/8·1/16·1/32·1/64 가운데 하나라 기본값을 쓴다"
                        .to_string(),
                ),
            }
        }
        if let Some(v) = map.remove("triplet") {
            match v {
                Value::Bool(b) => q.triplet = b,
                _ => warnings.push("quantize.triplet은 참·거짓이어야 해 기본값을 쓴다".to_string()),
            }
        }
        for (key, max, slot) in [
            ("strength", 100, &mut q.strength),
            ("swing", 75, &mut q.swing),
        ] {
            if let Some(v) = map.remove(key) {
                match v.as_u64().and_then(|n| u8::try_from(n).ok()) {
                    Some(n) if n <= max => *slot = n,
                    _ => warnings.push(format!(
                        "quantize.{key}는 0–{max} 사이의 정수여야 해 기본값을 쓴다"
                    )),
                }
            }
        }
        (q, map)
    }

    fn write(self, map: &mut Map<String, Value>) {
        let grid = GRIDS
            .iter()
            .find(|(g, _)| *g == self.grid)
            .map_or("1/16", |(_, name)| name);
        map.insert("grid".to_string(), Value::from(grid));
        map.insert("triplet".to_string(), Value::Bool(self.triplet));
        map.insert("strength".to_string(), Value::from(self.strength));
        map.insert("swing".to_string(), Value::from(self.swing));
    }
}

/// 클릭 길이(ms)의 허용 범위. 스케줄러의 노트 길이 상한(`MAX_NOTE_LENGTH`, 10 s)과 같다.
pub const METRONOME_LENGTH_RANGE: std::ops::RangeInclusive<u32> = 1..=10_000;

/// 미리듣기 길이(ms)의 허용 범위(PRE-1). 메트로놈 클릭과 같은 상한.
pub const PREVIEW_LENGTH_RANGE: std::ops::RangeInclusive<u32> = 1..=10_000;

/// 노트 미리듣기 설정(REQ 5.3 「이동 시 미리듣기」, PRE-1 「길이는 설정(기본 200 ms)」). S3-T2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSettings {
    /// 커서가 노트 위로 가면 들려준다(기본 켬).
    pub on_move: bool,
    /// 미리듣기 길이(ms) 1–10000.
    pub length_ms: u32,
}

impl Default for PreviewSettings {
    fn default() -> Self {
        PreviewSettings {
            on_move: true,
            length_ms: 200,
        }
    }
}

impl PreviewSettings {
    fn read(mut map: Map<String, Value>, warnings: &mut Vec<String>) -> (Self, Map<String, Value>) {
        let mut p = PreviewSettings::default();
        if let Some(v) = map.remove("on_move") {
            match v {
                Value::Bool(b) => p.on_move = b,
                _ => warnings.push("preview.on_move는 참·거짓이어야 해 기본값을 쓴다".to_string()),
            }
        }
        if let Some(v) = map.remove("length_ms") {
            match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
                Some(ms) if PREVIEW_LENGTH_RANGE.contains(&ms) => p.length_ms = ms,
                _ => warnings.push(format!(
                    "preview.length_ms는 {}–{} 사이의 정수여야 해 기본값을 쓴다",
                    PREVIEW_LENGTH_RANGE.start(),
                    PREVIEW_LENGTH_RANGE.end()
                )),
            }
        }
        (p, map)
    }

    fn write(&self, map: &mut Map<String, Value>) {
        map.insert("on_move".to_string(), Value::Bool(self.on_move));
        map.insert("length_ms".to_string(), Value::from(self.length_ms));
    }

    /// 스케줄러에 넘길 미리듣기 길이.
    #[must_use]
    pub fn length(&self) -> Duration {
        Duration::from_millis(u64::from(self.length_ms))
    }
}

impl MetronomeSettings {
    /// 스케줄러에 넘길 설정. `port`는 이 스냅샷에서 메트로놈 포트의 번호다.
    #[must_use]
    pub fn config(&self, port: u16) -> MetronomeConfig {
        // 값은 읽을 때 범위 안으로 고쳤다. 그래도 밖이면(코드가 직접 넣은 값) 기본값을 쓴다.
        let defaults = MetronomeConfig::default();
        let channel = Channel::new(self.channel).unwrap_or(defaults.channel());
        let accent = Pitch::new(self.accent).unwrap_or(defaults.accent());
        let beat = Pitch::new(self.beat).unwrap_or(defaults.beat());
        MetronomeConfig::new(port, channel, accent, beat)
            .with_length(Duration::from_millis(u64::from(self.length_ms)))
    }

    fn read(mut map: Map<String, Value>, warnings: &mut Vec<String>) -> (Self, Map<String, Value>) {
        let mut m = MetronomeSettings::default();
        if let Some(v) = map.remove("port") {
            match port_from_value(&v) {
                Some(port) => m.port = port,
                None => warnings.push(
                    "metronome.port는 \"virtual\" 또는 {\"named\": 이름}이라 기본값을 쓴다"
                        .to_string(),
                ),
            }
        }
        let mut int_field = |name: &str, range: std::ops::RangeInclusive<u32>| -> Option<u32> {
            let v = map.remove(name)?;
            let n = v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| range.contains(n));
            if n.is_none() {
                warnings.push(format!(
                    "metronome.{name}는 {}–{} 사이 정수여야 해 기본값을 쓴다",
                    range.start(),
                    range.end()
                ));
            }
            n
        };
        let byte = |n: u32| u8::try_from(n).ok();
        if let Some(n) = int_field("channel", 1..=16).and_then(byte) {
            m.channel = n;
        }
        if let Some(n) = int_field("accent", 0..=127).and_then(byte) {
            m.accent = n;
        }
        if let Some(n) = int_field("beat", 0..=127).and_then(byte) {
            m.beat = n;
        }
        if let Some(n) = int_field("length_ms", METRONOME_LENGTH_RANGE) {
            m.length_ms = n;
        }
        (m, map)
    }

    fn write(&self, map: &mut Map<String, Value>) {
        map.insert("port".to_string(), port_to_value(&self.port));
        map.insert("channel".to_string(), Value::from(self.channel));
        map.insert("accent".to_string(), Value::from(self.accent));
        map.insert("beat".to_string(), Value::from(self.beat));
        map.insert("length_ms".to_string(), Value::from(self.length_ms));
    }
}

/// `.ksq` 트랙 포트와 같은 모양: `"virtual"` 또는 `{"named": 이름}`(빈 이름은 받지 않는다).
fn port_from_value(v: &Value) -> Option<OutputPort> {
    if v.as_str() == Some("virtual") {
        return Some(OutputPort::Virtual);
    }
    let obj = v.as_object().filter(|o| o.len() == 1)?;
    let name = obj.get("named")?.as_str().filter(|n| !n.is_empty())?;
    Some(OutputPort::Named(name.to_owned()))
}

fn port_to_value(port: &OutputPort) -> Value {
    match port {
        OutputPort::Virtual => Value::from("virtual"),
        OutputPort::Named(name) => {
            let mut obj = Map::new();
            obj.insert("named".to_string(), Value::from(name.as_str()));
            Value::Object(obj)
        }
    }
}

/// MIDI 출력 설정(OUT-1·2, 설정 > MIDI 출력). 추가 가상 포트는 엔진에 실제로 있는 것과 같게 둔다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MidiOutputSettings {
    /// 기본 가상 포트 이름. 없으면 OUT-1의 기본 이름이다.
    pub main_name: Option<String>,
    /// 추가 가상 포트 이름(기본 포트와 합쳐 16개 이하)
    pub virtual_ports: Vec<String>,
    /// 추가 가상 포트 가운데 없는 포트의 대체 매핑(OUT-8)으로 만든 것. 같은 이름의 목적지가 나타나면 지운다
    /// (결정 `2026-10-22-out8-port-mapping-sheet.md` 3 A). `virtual_ports`에 있는 이름만 둔다.
    pub mapped_ports: Vec<String>,
    /// MIDI Clock을 보낼 포트(OUT-5, 기본 없음). 메트로놈 포트와 같은 모양이고 중복이 없다.
    pub clock: Vec<OutputPort>,
}

impl MidiOutputSettings {
    /// 기본 포트 이름(설정에 없으면 OUT-1의 기본 이름).
    #[must_use]
    pub fn main_or_default(&self) -> String {
        self.main_name
            .clone()
            .unwrap_or_else(|| keyscore_midi::DEFAULT_PORT_NAME.to_owned())
    }

    fn read(mut map: Map<String, Value>, warnings: &mut Vec<String>) -> (Self, Map<String, Value>) {
        let mut m = MidiOutputSettings::default();
        match map.remove("main_name") {
            None => {}
            Some(Value::String(name)) => match keyscore_midi::normalize_port_name(&name) {
                Ok(name) => m.main_name = Some(name),
                Err(_) => warnings.push(format!(
                    "midi_output.main_name 「{name}」은 포트 이름으로 쓸 수 없어 기본값을 쓴다"
                )),
            },
            Some(_) => {
                warnings.push("midi_output.main_name은 글자여야 해 기본값을 쓴다".to_string());
            }
        }
        match map.remove("virtual_ports") {
            None => {}
            Some(Value::Array(items)) => {
                let main = m.main_or_default();
                for item in items {
                    let Value::String(raw) = item else {
                        warnings.push(
                            "midi_output.virtual_ports의 글자가 아닌 항목을 뺐다".to_string(),
                        );
                        continue;
                    };
                    let Ok(name) = keyscore_midi::normalize_port_name(&raw) else {
                        warnings.push(format!(
                            "midi_output.virtual_ports의 「{raw}」은 포트 이름으로 쓸 수 없어 뺐다"
                        ));
                        continue;
                    };
                    if keyscore_midi::same_port_name(&name, &main)
                        || m.virtual_ports
                            .iter()
                            .any(|n| keyscore_midi::same_port_name(n, &name))
                    {
                        warnings.push(format!(
                            "midi_output.virtual_ports의 「{name}」은 다른 가상 포트와 이름이 같아 뺐다"
                        ));
                        continue;
                    }
                    if m.virtual_ports.len() + 1 >= keyscore_midi::MAX_VIRTUAL_OUTPUTS {
                        warnings.push(format!(
                            "가상 포트는 {}개까지라 「{name}」을 뺐다",
                            keyscore_midi::MAX_VIRTUAL_OUTPUTS
                        ));
                        continue;
                    }
                    m.virtual_ports.push(name);
                }
            }
            Some(_) => warnings
                .push("midi_output.virtual_ports는 이름 목록이어야 해 기본값을 쓴다".to_string()),
        }
        match map.remove("mapped_ports") {
            None => {}
            Some(Value::Array(items)) => {
                for item in items {
                    match item {
                        Value::String(name)
                            if m.virtual_ports.contains(&name)
                                && !m.mapped_ports.contains(&name) =>
                        {
                            m.mapped_ports.push(name);
                        }
                        // 추가 포트에 없는 이름은 표시할 포트가 없다(손으로 고친 파일)
                        _ => warnings.push(
                            "midi_output.mapped_ports의 추가 가상 포트가 아니거나 겹친 항목을 뺐다"
                                .to_string(),
                        ),
                    }
                }
            }
            Some(_) => warnings
                .push("midi_output.mapped_ports는 이름 목록이어야 해 기본값을 쓴다".to_string()),
        }
        match map.remove("clock") {
            None => {}
            Some(Value::Array(items)) => {
                for item in items {
                    match port_from_value(&item) {
                        Some(port) if !m.clock.contains(&port) => m.clock.push(port),
                        Some(_) => warnings.push(
                            "midi_output.clock에 같은 포트가 두 번 있어 하나를 뺐다".to_string(),
                        ),
                        None => warnings.push(
                            "midi_output.clock의 포트가 아닌 항목을 뺐다(\"virtual\" 또는 {\"named\": 이름})"
                                .to_string(),
                        ),
                    }
                }
            }
            Some(_) => {
                warnings.push("midi_output.clock은 포트 목록이어야 해 기본값을 쓴다".to_string());
            }
        }
        (m, map)
    }

    fn write(&self, map: &mut Map<String, Value>) {
        match &self.main_name {
            Some(name) => {
                map.insert("main_name".to_string(), Value::from(name.as_str()));
            }
            None => {
                map.remove("main_name");
            }
        }
        map.insert(
            "virtual_ports".to_string(),
            Value::Array(
                self.virtual_ports
                    .iter()
                    .map(|n| Value::from(n.as_str()))
                    .collect(),
            ),
        );
        // 대체 매핑을 쓰지 않은 설정 파일은 그대로 둔다
        if self.mapped_ports.is_empty() {
            map.remove("mapped_ports");
        } else {
            map.insert(
                "mapped_ports".to_string(),
                Value::Array(
                    self.mapped_ports
                        .iter()
                        .map(|n| Value::from(n.as_str()))
                        .collect(),
                ),
            );
        }
        map.insert(
            "clock".to_string(),
            Value::Array(self.clock.iter().map(port_to_value).collect()),
        );
    }
}

/// 최근 파일 목록을 읽는다. 문자열만 받고, 겹치거나 넘치는 항목은 버린다(손으로 고친 파일).
fn read_recent_files(value: Value, warnings: &mut Vec<String>) -> Vec<String> {
    let Value::Array(items) = value else {
        warnings.push("recent_files가 배열이 아니라 비운다".to_string());
        return Vec::new();
    };
    let mut recent = Vec::new();
    for item in items {
        match item {
            Value::String(path) if !recent.contains(&path) => recent.push(path),
            Value::String(_) => {}
            _ => warnings.push("recent_files의 문자열이 아닌 항목을 버린다".to_string()),
        }
    }
    recent.truncate(crate::files::MAX_RECENT_FILES);
    recent
}

/// 앱 설정 전체.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Settings {
    pub layout: LayoutSettings,
    pub theme: ThemeChoice,
    pub keymap: KeymapSettings,
    pub input: InputSettings,
    pub metronome: MetronomeSettings,
    pub midi_output: MidiOutputSettings,
    /// 노트 미리듣기(S3-T2).
    pub preview: PreviewSettings,
    /// 퀀타이즈(S3-T3b).
    pub quantize: QuantizeSettings,
    /// 최근 파일 경로(최근이 먼저, S2-T7a).
    pub recent_files: Vec<String>,
    /// `keymap` 객체 안의 모르는 키.
    keymap_extra: Map<String, Value>,
    /// `input` 객체 안의 모르는 키.
    input_extra: Map<String, Value>,
    /// `metronome` 객체 안의 모르는 키.
    metronome_extra: Map<String, Value>,
    /// `midi_output` 객체 안의 모르는 키.
    midi_output_extra: Map<String, Value>,
    /// `preview` 객체 안의 모르는 키.
    preview_extra: Map<String, Value>,
    /// `quantize` 객체 안의 모르는 키.
    quantize_extra: Map<String, Value>,
    /// 모르는 최상위 키. 다시 쓸 때 그대로 둔다.
    extra: Map<String, Value>,
    /// `layout` 객체 안의 모르는 키(뒤 버전이 더한 패널 등).
    layout_extra: Map<String, Value>,
}

impl Settings {
    /// 기본 설정 파일 위치.
    #[must_use]
    pub fn default_path() -> PathBuf {
        let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
        home.join("Library/Application Support/KeyScore/settings.json")
    }

    /// 읽는다. 파일이 없으면 기본값이고 경고는 없다. 깨졌거나 값이 잘못됐으면 그 부분을 기본값으로
    /// 두고 경고를 돌려준다(설정 때문에 앱이 뜨지 않는 일이 없게).
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "설정 객체마다 같은 모양의 읽기(객체가 아니면 경고)를 파일 순서대로 한곳에 둔다"
    )]
    pub fn load(path: &Path) -> (Settings, Vec<String>) {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Settings::default(), vec![]),
            Err(e) => {
                return (
                    Settings::default(),
                    vec![format!("설정을 읽지 못해 기본값을 쓴다: {e}")],
                );
            }
        };
        // UTF-8이 아닌 파일도 깨진 파일과 같이 백업한다(리뷰 2 제안 2).
        let text = String::from_utf8(bytes).unwrap_or_default();
        let mut warnings = Vec::new();
        let Ok(Value::Object(mut map)) = serde_json::from_str::<Value>(&text) else {
            // 첫 변경 때 덮어쓰기 전에 원본을 남긴다(사용자가 손으로 고칠 수 있게).
            let backup = path.with_extension("json.bak");
            let note = match fs::copy(path, &backup) {
                Ok(_) => format!(" 원본은 {}에 남겼다", backup.display()),
                Err(_) => String::new(),
            };
            warnings.push(format!(
                "설정 파일이 JSON 객체가 아니라 기본값을 쓴다.{note}"
            ));
            return (Settings::default(), warnings);
        };
        let mut settings = Settings::default();
        match map.remove("layout") {
            None => {}
            Some(Value::Object(layout)) => {
                let (layout, rest) = LayoutSettings::read(layout, &mut warnings);
                settings.layout = layout;
                settings.layout_extra = rest;
            }
            Some(_) => warnings.push("layout 설정이 객체가 아니라 기본값을 쓴다".to_string()),
        }
        match map.remove("keymap") {
            None => {}
            Some(Value::Object(mut keymap)) => {
                match keymap.remove("preset") {
                    None => {}
                    Some(v) => match serde_json::from_value(v) {
                        Ok(p) => settings.keymap.preset = p,
                        Err(_) => warnings
                            .push("keymap.preset은 default 또는 vim이라 기본값을 쓴다".to_string()),
                    },
                }
                match keymap.remove("swap_alt_cmd") {
                    None => {}
                    Some(Value::Bool(b)) => settings.keymap.swap_alt_cmd = b,
                    Some(_) => warnings
                        .push("keymap.swap_alt_cmd는 참·거짓이어야 해 기본값을 쓴다".to_string()),
                }
                settings.keymap_extra = keymap;
            }
            Some(_) => warnings.push("keymap 설정이 객체가 아니라 기본값을 쓴다".to_string()),
        }
        match map.remove("input") {
            None => {}
            Some(Value::Object(input)) => {
                (settings.input, settings.input_extra) = InputSettings::read(input, &mut warnings);
            }
            Some(_) => warnings.push("input 설정이 객체가 아니라 기본값을 쓴다".to_string()),
        }
        match map.remove("metronome") {
            None => {}
            Some(Value::Object(metronome)) => {
                (settings.metronome, settings.metronome_extra) =
                    MetronomeSettings::read(metronome, &mut warnings);
            }
            Some(_) => warnings.push("metronome 설정이 객체가 아니라 기본값을 쓴다".to_string()),
        }
        match map.remove("midi_output") {
            None => {}
            Some(Value::Object(midi_output)) => {
                (settings.midi_output, settings.midi_output_extra) =
                    MidiOutputSettings::read(midi_output, &mut warnings);
            }
            Some(_) => warnings.push("midi_output 설정이 객체가 아니라 기본값을 쓴다".to_string()),
        }
        match map.remove("preview") {
            None => {}
            Some(Value::Object(preview)) => {
                (settings.preview, settings.preview_extra) =
                    PreviewSettings::read(preview, &mut warnings);
            }
            Some(_) => warnings.push("preview 설정이 객체가 아니라 기본값을 쓴다".to_string()),
        }
        match map.remove("quantize") {
            None => {}
            Some(Value::Object(quantize)) => {
                (settings.quantize, settings.quantize_extra) =
                    QuantizeSettings::read(quantize, &mut warnings);
            }
            Some(_) => warnings.push("quantize 설정이 객체가 아니라 기본값을 쓴다".to_string()),
        }
        if let Some(v) = map.remove("recent_files") {
            settings.recent_files = read_recent_files(v, &mut warnings);
        }
        if let Some(v) = map.remove("theme") {
            match serde_json::from_value::<ThemeChoice>(v) {
                Ok(theme) => settings.theme = theme,
                Err(e) => warnings.push(format!("theme 설정이 잘못되어 기본값을 쓴다: {e}")),
            }
        }
        for name in settings.layout.sanitize() {
            warnings.push(format!("layout.{name}가 범위 밖이라 기본값을 쓴다"));
        }
        settings.extra = map;
        (settings, warnings)
    }

    /// 쓴다. 디렉터리가 없으면 만든다. 같은 디렉터리의 임시 파일에 쓰고 이름을 바꾼다.
    ///
    /// # Errors
    ///
    /// 입출력 실패.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut map = self.extra.clone();
        let mut layout = self.layout_extra.clone();
        if let Value::Object(known) = serde_json::to_value(self.layout).map_err(io::Error::other)? {
            layout.extend(known);
        }
        map.insert("layout".to_string(), Value::Object(layout));
        let mut keymap = self.keymap_extra.clone();
        keymap.insert(
            "preset".to_string(),
            serde_json::to_value(self.keymap.preset).map_err(io::Error::other)?,
        );
        keymap.insert(
            "swap_alt_cmd".to_string(),
            Value::Bool(self.keymap.swap_alt_cmd),
        );
        map.insert("keymap".to_string(), Value::Object(keymap));
        let mut input = self.input_extra.clone();
        self.input.write(&mut input);
        map.insert("input".to_string(), Value::Object(input));
        let mut metronome = self.metronome_extra.clone();
        self.metronome.write(&mut metronome);
        map.insert("metronome".to_string(), Value::Object(metronome));
        let mut midi_output = self.midi_output_extra.clone();
        self.midi_output.write(&mut midi_output);
        map.insert("midi_output".to_string(), Value::Object(midi_output));
        let mut preview = self.preview_extra.clone();
        self.preview.write(&mut preview);
        map.insert("preview".to_string(), Value::Object(preview));
        let mut quantize = self.quantize_extra.clone();
        self.quantize.write(&mut quantize);
        map.insert("quantize".to_string(), Value::Object(quantize));
        map.insert(
            "theme".to_string(),
            serde_json::to_value(self.theme).map_err(io::Error::other)?,
        );
        map.insert(
            "recent_files".to_string(),
            Value::Array(
                self.recent_files
                    .iter()
                    .map(|p| Value::String(p.clone()))
                    .collect(),
            ),
        );
        let text = serde_json::to_string_pretty(&Value::Object(map)).map_err(io::Error::other)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temp = path.with_extension("json.tmp");
        let result = fs::write(&temp, text).and_then(|()| fs::rename(&temp, path));
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}
