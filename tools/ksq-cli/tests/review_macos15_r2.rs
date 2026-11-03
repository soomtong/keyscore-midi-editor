//! ksq-cli macOS 15 출력 경로 수정(`fix/ksq-cli-macos15-output-paths`) 적대적 리뷰 2회차의 테스트.
//! 리뷰 산출물이다(`docs/reports/ksq-cli-macos15/review-2.md`).
//!
//! - `parent_on_devfs`는 `Path::parent`가 돌려준 빈 경로(한 구성 요소짜리 상대 경로)를 `stat`해
//!   실패하므로, 작업 디렉터리가 `/dev`일 때 `-o <새 이름>/`과 `-o ./<새 이름>/`의 안내가 갈린다.
//!   커널에게 둘은 같은 자리다. 일반 사용자로 돌린다(루트면 `/dev`에 만들 수 있어 건너뛴다).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const SPECIAL_HINT: &str = "특수 파일에 쓰려면 -o 없이 실행하고 셸로 리다이렉트하라";

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-m15r2-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

/// 작업 디렉터리를 `/dev`로 두고 실행한다. 종료 코드와 표준 오류.
fn run_in_dev(bin: &str, input: &Path, output: &str) -> (Option<i32>, String) {
    let o = Command::new(bin)
        .current_dir("/dev")
        .arg(input)
        .arg("-o")
        .arg(output)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// `cd /dev`에서 `-o name/`과 `-o ./name/`은 커널에게 같은 자리(`/dev/name/`)다. 둘 다 종료 1이고
/// 같은 안내를 받아야 한다. 수정 전(`625fbc5`)은 둘 다 일반 안내, 지금은 `name/`만 일반 안내다.
#[test]
fn binaries_relative_output_in_dev_cwd_same_hint_with_or_without_dot_slash() {
    let probe = format!("/dev/ksq-m15r2-probe-{}", std::process::id());
    if fs::File::create(&probe).is_ok() {
        let _ = fs::remove_file(&probe);
        return;
    }
    let dir = temp_dir("dev-cwd");
    let ksq = dir.join("in.ksq");
    fs::copy(fixture("01-melody.ksq"), &ksq).unwrap();
    let o = Command::new(KSQ2TXT).arg(&ksq).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let txt = dir.join("in.txt");
    fs::write(&txt, o.stdout).unwrap();
    let name = format!("ksq-m15r2-{}", std::process::id());

    let mut bad = Vec::new();
    for (bin, input) in [(KSQ2TXT, &ksq), (TXT2KSQ, &txt)] {
        let bare = run_in_dev(bin, input, &format!("{name}/"));
        let dotted = run_in_dev(bin, input, &format!("./{name}/"));
        let ok = bare.0 == Some(1)
            && dotted.0 == Some(1)
            && bare.1.contains(SPECIAL_HINT) == dotted.1.contains(SPECIAL_HINT);
        if !ok {
            bad.push(format!(
                "{bin}\n  {name}/: {:?} {}\n  ./{name}/: {:?} {}",
                bare.0, bare.1, dotted.0, dotted.1
            ));
        }
    }

    fs::remove_dir_all(&dir).unwrap();
    assert!(!Path::new("/dev").join(&name).exists());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
