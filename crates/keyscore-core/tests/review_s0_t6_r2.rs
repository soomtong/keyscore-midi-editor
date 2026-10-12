//! S0-T6 적대적 리뷰 2회차의 깨뜨리기 테스트. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 지적 사항의 재현이다. 고친 뒤 통과해야 한다.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use keyscore_core::ksq::{self, Compression};
use keyscore_core::{
    Document, Edit, Key, MeterMap, Note, OutputPort, Pitch, PitchClass, Project, Tempo, TempoMap,
    Tick, TimeSignature, TrackProp, Velocity,
};
use proptest::prelude::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = Path::new("/tmp").join(format!(
            "keyscore-review-s0-t6-r2-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn leftovers(&self) -> Vec<String> {
        let mut found = Vec::new();
        for entry in walk(&self.0) {
            let name = entry.file_name().unwrap().to_string_lossy().into_owned();
            if name.contains(".tmp-") {
                found.push(entry.display().to_string());
            }
        }
        found
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let meta = std::fs::symlink_metadata(&path).unwrap();
        if meta.is_dir() {
            out.extend(walk(&path));
        }
        out.push(path);
    }
    out
}

impl Drop for TempDir {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // 읽기 전용으로 바꾼 디렉터리가 있어도 지울 수 있게 한다.
            for p in walk(&self.0) {
                if std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir()) {
                    let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn project_named(name: &str) -> Project {
    ksq::from_str(&format!(r#"{{"tracks": [{{"name": "{name}"}}]}}"#))
        .unwrap()
        .0
}

// ---------------------------------------------------------------- from_sorted (지적 3 수정)

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 읽기는 이제 `from_sorted`로 맵을 한 번에 만든다. `set`을 차례로 부른 맵과 캐시까지 같아야
    /// 한다(`PartialEq`가 캐시를 포함한다). 틱 차이가 커서 곱이 큰 경우도 섞는다.
    #[test]
    fn ksq_read_maps_equal_maps_built_with_set(
        gaps in prop::collection::vec(prop_oneof![1u64..2000, 1u64..(u64::MAX / 64)], 0..12),
        tempos in prop::collection::vec(1u32..16_777_216, 13),
        meters in prop::collection::vec((1u8..=32, 0u32..6), 13),
    ) {
        let mut ticks = vec![0u64];
        for g in &gaps {
            let Some(next) = ticks.last().unwrap().checked_add(*g) else { break };
            ticks.push(next);
        }
        let mut tempo_json = String::new();
        let mut meter_json = String::new();
        let mut tmap = TempoMap::default();
        let mut mmap = MeterMap::default();
        for (i, t) in ticks.iter().enumerate() {
            let us = tempos[i];
            let (num, den_pow) = meters[i];
            let den = 1u16 << den_pow;
            let sep = if i == 0 { "" } else { ", " };
            write!(tempo_json, "{sep}[{t}, {us}]").unwrap();
            write!(meter_json, "{sep}[{t}, {num}, {den}]").unwrap();
            tmap.set(Tick::new(*t), Tempo::from_micros_per_quarter(us).unwrap());
            mmap.set(Tick::new(*t), TimeSignature::new(num, den).unwrap());
        }
        let text = format!(r#"{{"tempo": [{tempo_json}], "meter": [{meter_json}]}}"#);
        let (p, _) = ksq::from_str(&text).unwrap();
        prop_assert_eq!(p.tempo_map(), &tmap);
        prop_assert_eq!(p.meter_map(), &mmap);
    }
}

// ---------------------------------------------------------------- restore_notes (지적 4 수정)

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 같은 시작 틱의 노트를 파일에 아무 순서로 적어도, 읽은 클립은 `insert_note`를 아무 순서로
    /// 부른 클립과 같은 노트 열이어야 하고 쓰기 → 읽기 → 쓰기가 같은 바이트여야 한다.
    #[test]
    fn ksq_notes_in_any_file_order_read_as_note_order(
        notes in prop::collection::vec(
            (0u64..3, 1u32..5, 58u8..62, 0u16..3, prop::option::of("[ab]")),
            1..24,
        ),
        shuffle in any::<u64>(),
    ) {
        // 파일은 시작이 늘어나는 순서여야 하므로(Δt ≥ 0) 시작으로만 정렬하고, 같은 시작 안의
        // 순서는 섞는다.
        let mut order: Vec<usize> = (0..notes.len()).collect();
        order.sort_by_key(|&i| (notes[i].0, (i as u64).wrapping_mul(shuffle | 1).rotate_left(17)));
        let mut items = Vec::new();
        let mut previous = 0;
        for &i in &order {
            let (start, len, pitch, vel, lyric) = &notes[i];
            let mut s = format!("[{}, {len}, {pitch}, {vel}", start - previous);
            if let Some(l) = lyric {
                write!(s, r#", {{"lyric": "{l}"}}"#).unwrap();
            }
            s.push(']');
            items.push(s);
            previous = *start;
        }
        let text = format!(
            r#"{{"tracks": [{{"clips": [{{"start": 0, "length": 100, "notes": [{}]}}]}}]}}"#,
            items.join(", ")
        );
        let (p, _) = ksq::from_str(&text).unwrap();
        let mut expected = keyscore_core::Clip::new(Tick::ZERO, 100).unwrap();
        for (start, len, pitch, vel, lyric) in notes.iter().rev() {
            let mut n = Note::new(Tick::new(*start), *len, Pitch::new(*pitch).unwrap(), Velocity::new(*vel)).unwrap();
            if let Some(l) = lyric {
                n = n.with_lyric(l.clone());
            }
            expected.insert_note(n).unwrap();
        }
        let (_, clip) = p.tracks()[0].clips().next().unwrap();
        prop_assert_eq!(clip.notes(), expected.notes());
        let first = ksq::to_string(&p);
        let second = ksq::to_string(&ksq::from_str(&first).unwrap().0);
        prop_assert_eq!(second, first);
    }
}

/// 실패 위치는 정렬 뒤가 아니라 파일 안의 번호여야 한다. 잘못된 노트가 정렬하면 앞으로 오는
/// 경우(같은 시작, 낮은 음높이)를 만든다. 시작이 클립 길이 밖인 노트가 둘이면 첫 번째를 가리킨다.
#[test]
fn ksq_bad_note_error_points_to_file_index() {
    let text = r#"{"tracks": [{"clips": [{"start": 0, "length": 10, "notes": [
        [0, 1, 70, 0], [0, 1, 69, 0], [10, 1, 60, 0], [0, 1, 50, 0], [5, 1, 40, 0]
    ]}]}]}"#;
    let err = ksq::from_str(text).unwrap_err().to_string();
    assert!(
        err.contains("tracks[0].clips[0].notes[2]"),
        "파일 안의 번호가 아니다: {err}"
    );
}

// ---------------------------------------------------------------- key·port 모르는 키와 편집

/// 조성을 「없음」으로 바꿨다가 undo하면 `key` 객체의 모르는 키가 다시 쓰여야 한다.
#[test]
fn ksq_key_extra_returns_after_set_key_none_undo() {
    let text = r#"{"key": {"tonic": 2, "mode": "major", "accidentals": "sharps"}}"#;
    let (p, _) = ksq::from_str(text).unwrap();
    let mut doc = Document::new(p, 1 << 20).unwrap();
    doc.apply(vec![Edit::SetKey(Key::None)], None).unwrap();
    assert!(!ksq::to_string(doc.project()).contains("accidentals"));
    doc.undo().unwrap();
    let out = ksq::to_string(doc.project());
    assert!(out.contains(r#""accidentals": "sharps""#), "{out}");
    assert_eq!(doc.project().key(), Key::Major(PitchClass::new(2).unwrap()));
}

/// 포트를 가상으로 바꿨다가 undo하면 `port` 객체의 모르는 키가 다시 쓰여야 한다.
#[test]
fn ksq_port_extra_returns_after_set_port_virtual_undo() {
    let text = r#"{"tracks": [{"port": {"named": "IAC Bus 1", "uid": 12345}}]}"#;
    let (p, _) = ksq::from_str(text).unwrap();
    let id = p.tracks()[0].id();
    let mut doc = Document::new(p, 1 << 20).unwrap();
    doc.apply(
        vec![Edit::SetTrackProp {
            track: id,
            prop: TrackProp::Port(OutputPort::Virtual),
        }],
        None,
    )
    .unwrap();
    assert!(!ksq::to_string(doc.project()).contains("uid"));
    doc.undo().unwrap();
    let out = ksq::to_string(doc.project());
    assert!(out.contains(r#""uid": 12345"#), "{out}");
}

// ---------------------------------------------------------------- 히스토리 메모리 상한

/// 모르는 키(클립·트랙·port·노트)도 힙을 차지한다. 히스토리의 메모리 상한이 이것을 세지 않으면,
/// 큰 모르는 키가 든 파일에서 분할·지우기를 반복할 때 실제 메모리가 상한을 마음대로 넘는다.
/// 분할은 원래 클립(모르는 키 포함)을 복제해 히스토리에 둔다.
#[test]
fn history_bytes_count_unknown_keys_of_split_clip() {
    let big = "x".repeat(1 << 20);
    let text =
        format!(r#"{{"tracks": [{{"clips": [{{"start": 0, "length": 960, "blob": "{big}"}}]}}]}}"#);
    let (p, _) = ksq::from_str(&text).unwrap();
    let track = p.tracks()[0].id();
    let clip = p.tracks()[0].clips().next().unwrap().0;
    let mut doc = Document::new(p, 64 << 20).unwrap();
    doc.apply(
        vec![Edit::SplitClip {
            track,
            id: clip,
            at: Tick::new(480),
        }],
        None,
    )
    .unwrap();
    assert!(
        doc.history_bytes() >= 1 << 20,
        "분할이 1 MiB 모르는 키가 든 클립을 히스토리에 복제했는데 {} 바이트로 센다",
        doc.history_bytes()
    );
}

// ---------------------------------------------------------------- 심볼릭 링크 (지적 5 수정)

/// 순환 링크(자기 자신, 둘이 서로)는 오류여야 하고 패닉·무한 루프가 없어야 한다. 링크는 그대로다.
#[cfg(unix)]
#[test]
fn ksq_write_file_symlink_loop_is_error() {
    let dir = TempDir::new("loop");
    let me = dir.0.join("me.ksq");
    std::os::unix::fs::symlink("me.ksq", &me).unwrap();
    let a = dir.0.join("a.ksq");
    let b = dir.0.join("b.ksq");
    std::os::unix::fs::symlink("b.ksq", &a).unwrap();
    std::os::unix::fs::symlink("a.ksq", &b).unwrap();
    for path in [&me, &a, &b] {
        assert!(ksq::write_file(path, &Project::new(), Compression::None).is_err());
        assert!(
            std::fs::symlink_metadata(path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    assert!(dir.leftovers().is_empty(), "{:?}", dir.leftovers());
}

/// 상대 링크가 다른 디렉터리를 거치는 사슬: `a/link → ../b/link2 → real.ksq`. 상대 경로는 링크가
/// 있는 디렉터리 기준이다. 저장 뒤 두 링크는 링크로 남고 실제 파일만 바뀌어야 한다.
#[cfg(unix)]
#[test]
fn ksq_write_file_relative_symlink_chain_updates_final_target() {
    let dir = TempDir::new("chain");
    std::fs::create_dir_all(dir.0.join("a")).unwrap();
    std::fs::create_dir_all(dir.0.join("b")).unwrap();
    let real = dir.0.join("b/real.ksq");
    ksq::write_file(&real, &project_named("old"), Compression::None).unwrap();
    std::os::unix::fs::symlink("real.ksq", dir.0.join("b/link2")).unwrap();
    std::os::unix::fs::symlink("../b/link2", dir.0.join("a/link")).unwrap();

    let new = project_named("new");
    ksq::write_file(&dir.0.join("a/link"), &new, Compression::None).unwrap();

    for l in ["a/link", "b/link2"] {
        assert!(
            std::fs::symlink_metadata(dir.0.join(l))
                .unwrap()
                .file_type()
                .is_symlink(),
            "{l}가 일반 파일로 바뀌었다"
        );
    }
    assert_eq!(
        std::fs::read_to_string(&real).unwrap(),
        ksq::to_string(&new)
    );
    assert!(dir.leftovers().is_empty(), "{:?}", dir.leftovers());
}

/// 대상이 없는 링크는 대상 자리에 새로 만들고 링크는 남는다. 디렉터리를 가리키는 링크는 오류이고
/// 디렉터리·링크가 그대로다.
#[cfg(unix)]
#[test]
fn ksq_write_file_dangling_and_directory_symlinks() {
    let dir = TempDir::new("dangling");
    let link = dir.0.join("new.ksq");
    std::os::unix::fs::symlink("made.ksq", &link).unwrap();
    ksq::write_file(&link, &project_named("n"), Compression::None).unwrap();
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(dir.0.join("made.ksq").is_file());

    std::fs::create_dir_all(dir.0.join("d")).unwrap();
    let dlink = dir.0.join("d.ksq");
    std::os::unix::fs::symlink("d", &dlink).unwrap();
    assert!(ksq::write_file(&dlink, &project_named("n"), Compression::None).is_err());
    assert!(
        std::fs::symlink_metadata(&dlink)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(dir.0.join("d").is_dir());
    assert!(dir.leftovers().is_empty(), "{:?}", dir.leftovers());
}

// ---------------------------------------------------------------- 권한 (제안 7 수정)

/// 읽기 전용(0444) 원본. 권한 복사가 쓰기를 깨뜨리지 않는지 본다. 결과 권한은 0444로 남고,
/// 두 번째 저장도 같다.
#[cfg(unix)]
#[test]
fn ksq_write_file_over_read_only_file_keeps_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new("ro");
    let path = dir.0.join("song.ksq");
    ksq::write_file(&path, &project_named("old"), Compression::None).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    for name in ["a", "b"] {
        let p = project_named(name);
        ksq::write_file(&path, &p, Compression::Gzip).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o444
        );
        assert_eq!(ksq::read_file(&path).unwrap().0.tracks()[0].name(), name);
    }
    assert!(dir.leftovers().is_empty(), "{:?}", dir.leftovers());
}

/// 원본이 0600(남이 못 읽는 파일)이면, 내용을 담은 임시 파일도 그보다 넓은 권한이면 안 된다.
/// 지금은 임시 파일을 umask 기본 권한(보통 0644)으로 만들고 내용을 다 쓴 뒤에야 권한을 옮긴다.
/// 그 사이에 다른 사용자가 임시 파일을 열면 내용을 읽을 수 있고, 연 핸들은 권한이 바뀐 뒤에도
/// 유효하다. 관찰 스레드가 내용이 있는 임시 파일의 권한을 본다.
#[cfg(unix)]
#[test]
fn ksq_write_file_temp_never_wider_than_original_mode() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    let dir = TempDir::new("tmpmode");
    let path = dir.0.join("secret.ksq");
    // 쓰기가 수 ms 걸리도록 크게 만든다.
    let mut notes = String::new();
    for i in 0..200_000 {
        let sep = if i == 0 { "" } else { ", " };
        write!(notes, "{sep}[1, 1, 60, 1]").unwrap();
    }
    let text = format!(
        r#"{{"tracks": [{{"clips": [{{"start": 0, "length": 1000000, "notes": [{notes}]}}]}}]}}"#
    );
    let (big, _) = ksq::from_str(&text).unwrap();
    ksq::write_file(&path, &Project::new(), Compression::None).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

    let stop = AtomicBool::new(false);
    let wider = std::thread::scope(|s| {
        let watcher = s.spawn(|| {
            let mut seen = Vec::new();
            while !stop.load(Ordering::Relaxed) {
                for entry in std::fs::read_dir(&dir.0).unwrap().flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !name.contains(".tmp-") {
                        continue;
                    }
                    if let Ok(meta) = std::fs::metadata(entry.path())
                        && meta.len() > 0
                        && meta.permissions().mode() & 0o077 != 0
                    {
                        seen.push(format!("{name} {:o}", meta.permissions().mode() & 0o777));
                    }
                }
            }
            seen
        });
        for _ in 0..20 {
            ksq::write_file(&path, &big, Compression::None).unwrap();
        }
        stop.store(true, Ordering::Relaxed);
        watcher.join().unwrap()
    });
    assert!(
        wider.is_empty(),
        "0600 파일의 내용이 더 넓은 권한의 임시 파일로 보였다: {:?}",
        &wider[..wider.len().min(3)]
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
