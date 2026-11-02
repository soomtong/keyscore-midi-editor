//! ksq-cli macOS 15 출력 경로 수정(`fix/ksq-cli-macos15-output-paths`) 적대적 리뷰 1회차의 테스트.
//! 커밋하지 않는 리뷰 산출물이다(`docs/reports/ksq-cli-macos15/review-1.md`).
//!
//! - 링크 대상 문자열이 `/`로 끝나고 그 뒤가 끊긴 링크·없는 이름인 사슬은 새 `refuse_directory_form`만
//!   막는다(일반 파일 쪽 사슬은 `guard_output`이 먼저 막는다). macOS 15 CI에서 확인하려고 둔다.
//! - `/dev/..`로 시작하지만 `/dev` 밖인 쓰기 권한 없는 디렉터리의 EACCES는 일반 안내여야 한다(바뀐
//!   `output_error` 주석의 기준). 일반 사용자로 돌린다.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const SPECIAL_HINT: &str = "특수 파일에 쓰려면 -o 없이 실행하고 셸로 리다이렉트하라";
const GENERAL_HINT: &str = "다른 경로를 주거나 표준 출력으로 받는다";

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-m15r1-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    v.sort();
    v
}

struct Ran {
    code: Option<i32>,
    stderr: String,
}

fn run(bin: &str, input: &Path, output: &Path) -> Ran {
    let o = Command::new(bin)
        .arg(input)
        .arg("-o")
        .arg(output)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    Ran {
        code: o.status.code(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn inputs(dir: &Path) -> [(&'static str, PathBuf); 2] {
    let ksq = dir.join("in.ksq");
    fs::copy(fixture("01-melody.ksq"), &ksq).unwrap();
    let o = Command::new(KSQ2TXT).arg(&ksq).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let txt = dir.join("in.txt");
    fs::write(&txt, o.stdout).unwrap();
    [(KSQ2TXT, ksq), (TXT2KSQ, txt)]
}

/// `l3 -> l2/`, `l2 -> missing`(끊긴 링크), `ls -> newdir/`, `ld -> x/.`. 링크를 푼 경로가 디렉터리
/// 꼴이라 이름 바꾸기 전에 막혀야 한다. 종료 1, 아무것도 생기지 않고 링크는 그대로.
#[test]
fn binaries_output_link_with_slashed_target_to_missing_fails_without_creating() {
    let dir = temp_dir("slashed-target");
    let ins = inputs(&dir);
    let link = |target: &str, name: &str| {
        std::os::unix::fs::symlink(target, dir.join(name)).unwrap();
        dir.join(name)
    };
    link("missing", "l2");
    let cases = [
        link("l2/", "l3"),
        link("newdir/", "ls"),
        link("x/.", "ld"),
        link("l2//", "l4"),
    ];
    let before = names(&dir);

    for (bin, input) in &ins {
        for out in &cases {
            let r = run(bin, input, out);
            assert_eq!(r.code, Some(1), "{bin} -o {}: {}", out.display(), r.stderr);
        }
    }

    assert_eq!(names(&dir), before);
    for n in ["l2", "l3", "ls", "ld", "l4"] {
        assert!(fs::symlink_metadata(dir.join(n)).unwrap().is_symlink(), "{n}");
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// `/dev/..`로 시작하지만 실제로는 `/dev` 밖의 쓰기 권한 없는 디렉터리. 원인은 EACCES(권한)이고
/// 셸 리다이렉션도 같은 이유로 실패하므로 일반 안내여야 한다. 수정 전(`a1e85fd`)은 일반 안내였다.
/// 루트로 돌리면 쓰기가 성공해 뜻이 없으므로 건너뛴다.
#[test]
fn binaries_output_dev_dotdot_into_read_only_dir_gets_general_hint() {
    let dir = temp_dir("dev-dotdot-ro");
    let ins = inputs(&dir);
    let ro = dir.join("ro");
    fs::create_dir(&ro).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::File::create(ro.join("probe")).is_ok() {
        let _ = fs::remove_file(ro.join("probe"));
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        return;
    }
    let target = PathBuf::from("/dev/..")
        .join(ro.strip_prefix("/").unwrap())
        .join("x");

    let mut bad = Vec::new();
    for (bin, input) in &ins {
        let r = run(bin, input, &target);
        if r.code != Some(1)
            || !r.stderr.contains("Permission denied")
            || r.stderr.contains(SPECIAL_HINT)
            || !r.stderr.contains(GENERAL_HINT)
        {
            bad.push(format!("{bin}: {:?} {}", r.code, r.stderr));
        }
    }

    let ro_names = names(&ro);
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    assert!(ro_names.is_empty(), "{ro_names:?}");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
