//! 앱 설정 파일(`~/Library/Application Support/KeyScore/settings.json`).
//!
//! 패널 열림·크기와 테마를 저장한다(S2-T1). 이후 Task가 입력·MIDI·메트로놈·키맵 설정을 더한다.
//! 모르는 키는 보존하고 없는 키는 기본값을 쓴다(REQ IO-1과 같은 태도). 쓰기는 임시 파일 + rename이라
//! 쓰는 도중 죽어도 원래 파일이 남는다.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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

/// 앱 설정 전체.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Settings {
    pub layout: LayoutSettings,
    pub theme: ThemeChoice,
    pub keymap: KeymapSettings,
    /// `keymap` 객체 안의 모르는 키.
    keymap_extra: Map<String, Value>,
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
        map.insert(
            "theme".to_string(),
            serde_json::to_value(self.theme).map_err(io::Error::other)?,
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
