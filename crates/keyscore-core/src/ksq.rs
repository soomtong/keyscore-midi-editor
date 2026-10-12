//! `.ksq` 프로젝트 파일(REQ IO-1, 9.5).
//!
//! JSON이고, 노트는 `[Δt, 길이, 음높이, 벨로시티]`에 선택 속성 객체(`offset`, `lyric`)를 붙인다
//! (`docs/decisions/2026-10-12-ksq-note-format.md`). 쓰기는 결정적이다: 키 순서가 고정되고 2칸
//! 들여쓰기이며 노트·템포·박자표 항목은 한 줄에 하나라서 git diff가 노트 단위로 나온다.
//! 읽기는 모르는 키를 보존하고 없는 키는 기본값으로 채운다. `TrackId`·`ClipId`는 쓰지 않고
//! 읽을 때 새로 발급한다.

use std::fmt;
use std::path::Path;

use thiserror::Error;

use crate::{ModelError, Project};

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

/// 평문 `.ksq` 텍스트로 쓴다.
#[must_use]
pub fn to_string(project: &Project) -> String {
    todo!("{}", project.tracks().len())
}

/// 평문 `.ksq` 텍스트를 읽는다.
///
/// # Errors
///
/// JSON 문법 오류, 형식 오류(타입·범위), 모델 불변식 위반이면 [`KsqError`].
pub fn from_str(text: &str) -> Result<(Project, Vec<Warning>), KsqError> {
    todo!("{}", text.len())
}

/// 바이트로 쓴다. `Compression::Gzip`이면 gzip으로 감싼다.
#[must_use]
pub fn to_bytes(project: &Project, compression: Compression) -> Vec<u8> {
    todo!("{} {compression:?}", project.tracks().len())
}

/// 바이트를 읽는다. 첫 두 바이트가 gzip 표시(1f 8b)면 풀어서 읽는다.
///
/// # Errors
///
/// [`from_str`]와 같고, gzip이 깨졌거나 UTF-8이 아니면 [`KsqError::Invalid`].
pub fn from_bytes(bytes: &[u8]) -> Result<(Project, Vec<Warning>), KsqError> {
    todo!("{}", bytes.len())
}

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
    todo!("{path:?} {} {compression:?}", project.tracks().len())
}

/// 파일을 읽는다.
///
/// # Errors
///
/// [`from_bytes`]와 같고, 읽기 실패면 [`KsqError::Io`].
pub fn read_file(path: &Path) -> Result<(Project, Vec<Warning>), KsqError> {
    todo!("{path:?}")
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
    fn ksq_gzip_round_trip_and_detection() {
        let p = sample();
        let plain = to_bytes(&p, Compression::None);
        let gz = to_bytes(&p, Compression::Gzip);
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
