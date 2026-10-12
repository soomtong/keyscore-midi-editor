//! S0-T6 적대적 리뷰 3회차의 깨뜨리기 테스트. 공개 API만 쓴다.

use std::path::{Path, PathBuf};

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{Document, Edit, Project};

fn big() -> String {
    "x".repeat(1 << 20)
}

fn doc_from(text: &str, cap: usize) -> Document {
    let (p, _) = ksq::from_str(text).unwrap();
    Document::new(p, cap).unwrap()
}

/// 트랙을 지우면 역연산이 트랙 전체를 복제해 보관한다. `port`의 모르는 키도 세어야 한다.
#[test]
fn history_bytes_count_port_unknown_keys_of_removed_track() {
    let text = format!(
        r#"{{"tracks": [{{"port": {{"named": "A", "blob": "{}"}}}}]}}"#,
        big()
    );
    let mut doc = doc_from(&text, 64 << 20);
    let track = doc.project().tracks()[0].id();
    doc.apply(vec![Edit::RemoveTrack { track }], None).unwrap();
    assert!(doc.history_bytes() >= 1 << 20, "{}", doc.history_bytes());
}

/// 노트 선택 객체의 모르는 키를 가진 노트를 지울 때도 센다.
#[test]
fn history_bytes_count_note_unknown_keys_of_removed_note() {
    let text = format!(
        r#"{{"tracks": [{{"clips": [{{"start": 0, "length": 960,
            "notes": [[0, 100, 60, 5, {{"blob": "{}"}}]]}}]}}]}}"#,
        big()
    );
    let mut doc = doc_from(&text, 64 << 20);
    let track = doc.project().tracks()[0].id();
    let (clip, c) = doc.project().tracks()[0].clips().next().unwrap();
    let note = c.notes()[0].clone();
    doc.apply(vec![Edit::RemoveNote { track, clip, note }], None)
        .unwrap();
    assert!(doc.history_bytes() >= 1 << 20, "{}", doc.history_bytes());
}

/// 클립 하나가 상한보다 커도 편집은 패닉 없이 성공하고 히스토리는 상한을 넘지 않는다.
#[test]
fn history_cap_smaller_than_one_unknown_key_clip_keeps_latest_undoable() {
    let text = format!(
        r#"{{"tracks": [{{"clips": [{{"start": 0, "length": 960, "blob": "{}"}}]}}]}}"#,
        big()
    );
    let cap = 64 << 10;
    let mut doc = doc_from(&text, cap);
    let track = doc.project().tracks()[0].id();
    let (id, _) = doc.project().tracks()[0].clips().next().unwrap();
    let before = doc.project().clone();
    let r = doc.apply(
        vec![Edit::SplitClip {
            track,
            id,
            at: keyscore_core::Tick::new(480),
        }],
        None,
    );
    assert!(r.is_ok(), "{r:?}");
    // 정책(history.rs `keeps_latest_even_if_too_large`)상 최신 단위 하나는 상한을 넘어도 남는다.
    assert!(doc.undo_len() >= 1);
    // 상한 때문에 되돌릴 수 없게 되더라도 프로젝트는 일관되어야 한다.
    if doc.undo().unwrap() {
        assert_eq!(doc.project(), &before);
    }
}

struct TempDir(PathBuf);
impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = Path::new("/tmp").join(format!(
            "keyscore-review-s0-t6-r3-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 원본이 0600이어도 임시 파일은 만든 직후(권한을 맞추기 전) 잠깐 기본 권한(umask, 보통 0644)으로
/// 존재한다. 그 사이 다른 사용자가 열어 쥔 핸들은 권한이 바뀐 뒤에도 내용을 읽는다. 같은 사용자
/// 테스트에서는 열기가 항상 성공해 핸들로는 못 보이므로, 크기와 관계없이(빈 파일 포함) 임시 파일이
/// 보이는 모든 순간의 권한을 기록해 0600보다 넓은 순간이 없는지 본다.
#[cfg(unix)]
#[test]
fn ksq_write_file_temp_never_wider_than_original_mode_even_when_empty() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    let dir = TempDir::new("window");
    let path = dir.0.join("secret.ksq");
    ksq::write_file(&path, &Project::new(), Compression::None).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let seen: Arc<Mutex<Vec<(u64, u32)>>> = Arc::default();
    let handle = {
        let (stop, seen, d) = (stop.clone(), seen.clone(), dir.0.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let Ok(rd) = std::fs::read_dir(&d) else {
                    continue;
                };
                for e in rd.flatten() {
                    if !e.file_name().to_string_lossy().contains(".tmp-") {
                        continue;
                    }
                    if let Ok(m) = std::fs::metadata(e.path()) {
                        let mode = m.permissions().mode() & 0o777;
                        if mode & 0o077 != 0 {
                            seen.lock().unwrap().push((m.len(), mode));
                        }
                    }
                }
            }
        })
    };
    let (p, _) = ksq::from_str(r#"{"tracks": [{"name": "secret"}]}"#).unwrap();
    for _ in 0..300 {
        ksq::write_file(&path, &p, Compression::None).unwrap();
        if seen.lock().unwrap().iter().any(|(_, m)| m & 0o077 != 0) {
            break;
        }
    }
    stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
    let wide: Vec<_> = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, mode)| mode & 0o077 != 0)
        .copied()
        .collect();
    assert!(
        wide.is_empty(),
        "0600 원본의 임시 파일이 더 넓은 권한으로 보인 순간 {}번 (길이, 권한): {:?}",
        wide.len(),
        wide.first()
    );
}
