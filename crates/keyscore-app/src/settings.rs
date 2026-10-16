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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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

/// 앱 설정 전체.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Settings {
    pub layout: LayoutSettings,
    pub theme: ThemeChoice,
    /// 모르는 최상위 키. 다시 쓸 때 그대로 둔다.
    extra: Map<String, Value>,
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
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Settings::default(), vec![]),
            Err(e) => {
                return (
                    Settings::default(),
                    vec![format!("설정을 읽지 못해 기본값을 쓴다: {e}")],
                );
            }
        };
        let mut warnings = Vec::new();
        let Ok(Value::Object(mut map)) = serde_json::from_str::<Value>(&text) else {
            warnings.push("설정 파일이 JSON 객체가 아니라 기본값을 쓴다".to_string());
            return (Settings::default(), warnings);
        };
        let mut settings = Settings::default();
        if let Some(v) = map.remove("layout") {
            match serde_json::from_value::<LayoutSettings>(v) {
                Ok(layout) => settings.layout = layout,
                Err(e) => warnings.push(format!("layout 설정이 잘못되어 기본값을 쓴다: {e}")),
            }
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
        map.insert(
            "layout".to_string(),
            serde_json::to_value(self.layout).map_err(io::Error::other)?,
        );
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
