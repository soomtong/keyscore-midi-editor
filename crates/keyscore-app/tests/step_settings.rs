//! S3-T4a: Step 입력 설정(`input.step_default_length`, `input.octave_wait_ms`) 읽기·쓰기. 결정
//! `docs/decisions/2026-10-29-s3-t4-step-input-details.md` 6절.

use keyscore_app::actions::args::LengthCode;
use keyscore_app::settings::{InputSettings, Settings};

fn temp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-step-settings-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("settings.json")
}

#[test]
fn input_settings_defaults_quarter_and_300ms() {
    let s = InputSettings::default();
    assert_eq!(s.step_default_length, LengthCode::Quarter);
    assert_eq!(s.octave_wait_ms, 300);
}

#[test]
fn input_settings_read_and_write_step_fields() {
    let path = temp_path("rw");
    std::fs::write(
        &path,
        r#"{"input": {"key_layout": "solfege", "step_default_length": "eighth", "octave_wait_ms": 1000}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(s.input.step_default_length, LengthCode::Eighth);
    assert_eq!(s.input.octave_wait_ms, 1000);
    s.save(&path).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["input"]["step_default_length"], "eighth");
    assert_eq!(v["input"]["octave_wait_ms"], 1000);
}

#[test]
fn input_settings_out_of_range_use_defaults_with_warning() {
    let path = temp_path("bad");
    std::fs::write(
        &path,
        r#"{"input": {"step_default_length": "tenth", "octave_wait_ms": 2001}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s.input.step_default_length, LengthCode::Quarter);
    assert_eq!(s.input.octave_wait_ms, 300);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    let path = temp_path("edge");
    std::fs::write(&path, r#"{"input": {"octave_wait_ms": 0}}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s.input.octave_wait_ms, 0);
    assert!(warnings.is_empty());
}
