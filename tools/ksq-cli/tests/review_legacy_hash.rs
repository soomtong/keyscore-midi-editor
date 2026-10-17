//! 결정 Q1 2차 개정(레거시 `#` 주석 줄) 적대적 리뷰 1회차: `txt2ksq --legacy`의 깨뜨리기 시도
//! (`docs/reports/s0-t5-comments/review-legacy-1.md`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use keyscore_core::ksq;
use keyscore_text::{Mode, ParseOptions};
use ksq_cli::{text_to_project, text_to_project_with};

const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");
const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ksq-cli-review-legacy-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(bin: &str, args: &[&Path]) -> Output {
    Command::new(bin).args(args).output().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn note_count(path: &Path) -> usize {
    let (p, _) = ksq::read_file(path).unwrap();
    p.tracks()
        .iter()
        .flat_map(keyscore_core::Track::clips)
        .map(|(_, c)| c.notes().len())
        .sum()
}

#[test]
fn txt2ksq_legacy_reads_bom_crlf_hash_lines_with_option_anywhere() {
    let dir = temp_dir("bom");
    let old = dir.join("old.txt");
    fs::write(
        &old,
        "\u{feff}# 옛 머리말\r\n@track A ch1\r\n  # 옛 주석\r\nc d\r\n",
    )
    .unwrap();
    let out = dir.join("out.ksq");
    // 옵션 위치는 입력 앞·뒤·`-o` 사이 어디든 같고, 두 번 써도 받는다(`--strict`와 같은 태도).
    for args in [
        vec![Path::new("--legacy"), &old, Path::new("-o"), &out],
        vec![&old, Path::new("-o"), &out, Path::new("--legacy")],
        vec![
            Path::new("--strict"),
            Path::new("--legacy"),
            &old,
            Path::new("--legacy"),
            Path::new("-o"),
            &out,
        ],
    ] {
        let _ = fs::remove_file(&out);
        let o = run(TXT2KSQ, &args);
        assert_eq!(o.status.code(), Some(0), "{args:?}: {}", stderr(&o));
        assert!(stderr(&o).is_empty(), "{args:?}: {}", stderr(&o));
        assert_eq!(note_count(&out), 2);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn legacy_option_is_rejected_by_other_tools_and_variants() {
    let dir = temp_dir("reject");
    let txt = dir.join("a.txt");
    fs::write(&txt, "@track A ch1\nc\n").unwrap();
    // ksq2txt는 텍스트를 읽지 않으므로 --legacy를 모르는 옵션으로 거절한다.
    let o = run(KSQ2TXT, &[Path::new("x.ksq"), Path::new("--legacy")]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    for bad in ["--legacy=1", "--Legacy", "-legacy"] {
        let o = run(TXT2KSQ, &[&txt, Path::new(bad)]);
        assert_eq!(o.status.code(), Some(2), "{bad}: {}", stderr(&o));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn text_to_project_default_equals_with_legacy_off() {
    let text = "@track A ch1\nc d # e\n";
    for mode in [Mode::Lenient, Mode::Strict] {
        let a = text_to_project(text, mode).map(|(p, d)| (ksq::to_string(&p), d));
        let b = text_to_project_with(text, mode, ParseOptions::default())
            .map(|(p, d)| (ksq::to_string(&p), d));
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}
