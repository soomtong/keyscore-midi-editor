//! S0-T6 적대적 리뷰 4회차의 깨뜨리기 테스트. 공개 API만 쓴다.
//! umask는 프로세스 전체 상태라 테스트 안에서 바꾸지 않고, `sh -c 'umask ..; exec'`로 자식을 띄운다.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use keyscore_core::Project;
use keyscore_core::ksq::{self, Compression};

const CHILD_ENV: &str = "KS_R4_CHILD_DIR";

fn mode_of(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o7777
}

/// 지정한 umask로 이 테스트 바이너리를 다시 실행한다.
fn run_child(umask: &str, test: &str) -> PathBuf {
    let dir = Path::new("/tmp").join(format!(
        "keyscore-review-s0-t6-r4-{test}-{umask}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let exe = std::env::current_exe().unwrap();
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "umask {umask}; exec \"$0\" --exact {test} --nocapture --test-threads=1"
        ))
        .arg(exe)
        .env(CHILD_ENV, &dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

fn child_dir() -> Option<PathBuf> {
    std::env::var_os(CHILD_ENV).map(PathBuf::from)
}

fn write(path: &Path) {
    ksq::write_file(path, &Project::new(), Compression::None).unwrap();
}

/// 자식 안: 여러 원본 권한을 umask 아래서 저장하고 결과 권한이 원본과 같은지 본다.
#[test]
fn child_preserve_modes() {
    let Some(dir) = child_dir() else { return };
    for orig in [0o644, 0o755, 0o444, 0o600, 0o666, 0o640, 0o000, 0o400] {
        let path = dir.join(format!("m{orig:o}.ksq"));
        write(&path);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(orig)).unwrap();
        write(&path);
        assert_eq!(mode_of(&path), orig, "원본 {orig:o}");
        write(&path);
        assert_eq!(mode_of(&path), orig, "재저장 {orig:o}");
    }
}

/// 자식 안: 원본이 없는 새 파일의 권한은 umask 결과(0666 & !umask)여야 한다.
#[test]
fn child_new_file_mode() {
    let Some(dir) = child_dir() else { return };
    let path = dir.join("new.ksq");
    write(&path);
    let umask: u32 = std::env::var("KS_R4_UMASK")
        .ok()
        .map_or(0o022, |s| u32::from_str_radix(&s, 8).unwrap());
    assert_eq!(mode_of(&path), 0o666 & !umask);
}

#[test]
fn write_file_preserves_original_mode_under_narrow_umask() {
    let dir = run_child("077", "child_preserve_modes");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn write_file_preserves_original_mode_under_wide_umask() {
    let dir = run_child("000", "child_preserve_modes");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn write_file_new_file_keeps_default_mode_under_umask_022() {
    let dir = run_child("022", "child_new_file_mode");
    assert_eq!(mode_of(&dir.join("new.ksq")), 0o644);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn write_file_new_file_keeps_default_mode_under_umask_077() {
    let dir = run_child("077", "child_new_file_mode_077");
    assert_eq!(mode_of(&dir.join("new.ksq")), 0o600);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn child_new_file_mode_077() {
    let Some(dir) = child_dir() else { return };
    write(&dir.join("new.ksq"));
}
