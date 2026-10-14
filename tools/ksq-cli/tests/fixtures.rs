//! 번들 픽스처 8곡의 게이트(S0-T8, 계획 테스트 1–4): `ksq check` 통과, 텍스트 왕복, 생성기와
//! 디스크의 일치, 엄격 모드 읽기.

use std::fs;
use std::path::{Path, PathBuf};

use keyscore_core::ksq;
use keyscore_text::{Mode, parse};
use ksq_cli::{check, fixtures, project_to_text, text_to_project, write_fixtures};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixtures")
}

fn ksq_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "ksq"))
        .collect();
    names.sort();
    names
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-cli-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn fixtures_pass_check_all_eight() {
    let dir = fixture_dir();
    let names = ksq_files(&dir);
    let expected: Vec<_> = fixtures().into_iter().map(|(n, _)| n.to_owned()).collect();

    assert_eq!(names.len(), 8, "{names:?}");
    assert_eq!(names, expected);
    for name in names {
        let report = check(&dir.join(&name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(report.version(), Some(ksq::FORMAT_VERSION), "{name}");
        assert!(
            report.warnings().is_empty(),
            "{name}: {:?}",
            report.warnings()
        );
        assert!(report.tracks() >= 1 && report.notes() > 0, "{name}");
    }
}

#[test]
fn fixtures_round_trip_through_text_unchanged() {
    let dir = fixture_dir();
    let names = ksq_files(&dir);
    assert_eq!(names.len(), 8, "{names:?}");
    for name in names {
        let (original, warnings) = ksq::read_file(&dir.join(&name)).unwrap();
        assert!(warnings.is_empty(), "{name}");

        let (text, losses) = project_to_text(&original).unwrap();
        let (back, diagnostics) = text_to_project(&text, Mode::Lenient).unwrap();

        assert_eq!(losses, vec![], "{name}");
        assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
        assert_eq!(ksq::to_string(&back), ksq::to_string(&original), "{name}");
    }
}

#[test]
fn fixtures_match_generator_output() {
    let dir = fixture_dir();
    for (name, project) in fixtures() {
        let on_disk = fs::read(dir.join(name)).unwrap();
        assert!(
            on_disk == ksq::to_string(&project).into_bytes(),
            "{name}: 디스크 파일이 생성기 결과와 다르다. `cargo run -p ksq-cli --bin ksq -- fixtures assets/fixtures`로 다시 만든다"
        );
    }
    // 생성 명령의 함수도 같은 바이트를 쓴다.
    let out = temp_dir("generator");
    let written = write_fixtures(&out).unwrap();
    assert_eq!(written.len(), 8);
    for path in written {
        let name = path.file_name().unwrap();
        assert_eq!(fs::read(&path).unwrap(), fs::read(dir.join(name)).unwrap());
    }
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn fixtures_text_reads_in_strict_mode_without_warnings() {
    for (name, project) in fixtures() {
        let (text, losses) = project_to_text(&project).unwrap();
        assert_eq!(losses, vec![], "{name}");

        let parsed = parse(&text, Mode::Strict).unwrap_or_else(|e| panic!("{name}: {e}\n{text}"));

        assert!(parsed.warnings().is_empty(), "{name}");
        assert!(!text.contains('~'), "{name}: 보정 접미가 있다\n{text}");
    }
}

#[test]
fn fixtures_sources_read_in_strict_mode_without_warnings() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures-src");
    let mut count = 0;
    for entry in fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let (_, warnings) = text_to_project(&text, Mode::Strict)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(warnings.is_empty(), "{}", path.display());
        count += 1;
    }
    assert_eq!(count, 8);
}
