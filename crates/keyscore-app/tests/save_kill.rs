//! S2-T7 테스트 게이트: 저장 중 프로세스가 죽어도 원본은 온전하다.
//!
//! 이 테스트는 자기 실행 파일을 자식 프로세스로 다시 띄운다. 자식은 환경 변수 `KS_SAVE_KILL_CHILD`의 경로에
//! 큰 프로젝트를 끝없이 저장한다. 부모는 임시 파일이 생기면 잠깐 기다렸다가 `SIGKILL`로 죽이고, 원본이
//! 「저장 전 내용」 또는 「완전한 새 내용」인지 본다.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use keyscore_app::files::save_project;
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity, ksq};

const CHILD_ENV: &str = "KS_SAVE_KILL_CHILD";
const ROUNDS: u32 = 20;

/// 노트 10만 개(약 3 MB). 쓰는 데 몇 ms가 걸려 쓰는 도중에 죽일 수 있다.
fn big() -> Project {
    let mut p = Project::new();
    for t in 0..8u8 {
        let mut track = Track::new(
            format!("T{t}"),
            Channel::new(t + 1).unwrap(),
            TrackColor::new(t).unwrap(),
        );
        for c in 0..100u64 {
            let mut clip = Clip::new(Tick::new(c * 15_360), 15_360).unwrap();
            for n in 0..125u64 {
                let pitch = Pitch::new(u8::try_from(36 + (n * 7 + c) % 60).unwrap()).unwrap();
                clip.insert_note(
                    Note::new(Tick::new(n * 120), 110, pitch, Velocity::new(40_000)).unwrap(),
                )
                .unwrap();
            }
            track.insert_clip(clip).unwrap();
        }
        p.push_track(track).unwrap();
    }
    p
}

fn small() -> Project {
    ksq::from_str(r#"{"tracks": [{"name": "Original", "channel": 1}]}"#)
        .unwrap()
        .0
}

fn temp_files(dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains(".tmp-"))
        })
        .collect()
}

#[test]
fn killed_during_save_keeps_original_intact() {
    if let Ok(path) = std::env::var(CHILD_ENV) {
        let project = big();
        loop {
            save_project(Path::new(&path), &project).unwrap();
        }
    }
    let dir = std::env::temp_dir().join(format!("ks-save-kill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.ksq");
    let old = ksq::to_string(&small());
    let new = ksq::to_string(&big());
    let mut killed_mid_write = 0;
    for round in 0..ROUNDS {
        save_project(&path, &small()).unwrap();
        for t in temp_files(&dir) {
            std::fs::remove_file(t).unwrap();
        }
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "killed_during_save_keeps_original_intact",
                "--nocapture",
            ])
            .env(CHILD_ENV, &path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while temp_files(&dir).is_empty() {
            assert!(Instant::now() < deadline, "자식이 저장을 시작하지 않았다");
            std::thread::sleep(Duration::from_micros(200));
        }
        // 회차마다 죽이는 때를 조금씩 늦춘다(쓰기 앞·중간·rename 근처).
        std::thread::sleep(Duration::from_micros(u64::from(round) * 300));
        child.kill().unwrap();
        child.wait().unwrap();
        let text = std::fs::read_to_string(&path).expect("원본을 읽을 수 없다");
        assert!(
            text == old || text == new,
            "{round}회차: 원본이 반쯤 쓴 내용이다({}바이트)",
            text.len()
        );
        ksq::from_str(&text).expect("원본이 깨졌다");
        if !temp_files(&dir).is_empty() {
            killed_mid_write += 1;
        }
    }
    assert!(
        killed_mid_write > 0,
        "쓰는 도중에 한 번도 죽이지 못했다(시험이 게이트를 보지 못했다)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
