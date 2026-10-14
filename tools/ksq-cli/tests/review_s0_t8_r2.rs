//! S0-T8 적대적 리뷰 2회차(`docs/reports/s0-t8/review-2.md`)가 남긴 테스트.
//!
//! - 실패하는 테스트: 크기 상한의 기준이 텍스트가 나르지 않는 바이트(긴 트랙 이름, 모르는 키)와
//!   압축 전 크기라 작은 `.ksqz`가 거대한 텍스트를 만드는 것(지적 1), 성부 16개에서 어림이
//!   모자라 텍스트를 다 만든 뒤에야 거부하는 것(지적 2), `-o /dev/null`이 실패하는 것(지적 3).
//! - 통과하는 테스트: 평범한 긴 곡을 거부하지 않는 경계, 원자적 쓰기의 실패 경로, BOM 경계.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::Compression as GzLevel;
use flate2::write::GzEncoder;
use keyscore_core::ksq::{self, Compression};
use keyscore_core::{
    Channel, Clip, Note, Pitch, Project, Tempo, Tick, TimeSignature, Track, TrackColor, Velocity,
};
use ksq_cli::{CliError, MAX_EXPANSION, fixtures, project_to_text};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const WHOLE: u64 = 3840;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-cli-review2-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

fn run(bin: &str, args: &[&Path]) -> Output {
    Command::new(bin).args(args).output().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn note(start: u64, length: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 이름이 `name`이고, `clip_start`에서 시작하는 960틱 클립에 노트 하나가 든 트랙 하나.
fn far_clip_project(name: &str, clip_start: u64) -> Project {
    let mut clip = Clip::new(Tick::new(clip_start), 960).unwrap();
    clip.insert_note(note(0, 960, 60)).unwrap();
    let mut track = Track::new(name, Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip).unwrap();
    let mut p = Project::new();
    p.push_track(track).unwrap();
    p
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), GzLevel::best());
    enc.write_all(bytes).unwrap();
    enc.finish().unwrap()
}

/// `.ksqz` 파일을 `ksq2txt`로 돌려, 거부(종료 1)하거나 출력이 입력 파일 바이트의 256배
/// 이내인지 본다.
fn assert_ksqz_bounded_by_file(dir: &Path, bytes: &[u8]) {
    let input = dir.join("in.ksqz");
    fs::write(&input, bytes).unwrap();

    let o = run(KSQ2TXT, &[&input]);

    let file_len = bytes.len();
    match o.status.code() {
        Some(1) => assert!(o.stdout.is_empty()),
        Some(0) => assert!(
            o.stdout.len() <= 256 * file_len,
            "{file_len}바이트 .ksqz가 {}바이트 텍스트가 됐다({}배)",
            o.stdout.len(),
            o.stdout.len() / file_len
        ),
        other => panic!("종료 코드 {other:?}: {}", stderr(&o)),
    }
}

// ---------------------------------------------------------------- 지적 1: 상한의 기준

/// 긴 트랙 이름(2만 바이트)은 정규 `.ksq`를 키워 상한(256배)을 5 MB로 올리지만, 텍스트에는
/// 한 번만 나온다. 어림은 이름을 두 배로만 세므로 먼 클립의 쉼표 2.5 MB가 통과한다. gzip이 이름을
/// 거의 0으로 줄여, 수백 바이트 `.ksqz`가 MB 단위 텍스트가 된다. release 측정: 이름 10만 바이트면
/// 390바이트 `.ksqz`가 15 MB 텍스트, RSS 200 MB.
#[test]
fn ksq2txt_small_ksqz_with_long_track_name_stays_within_256x_file_or_errors() {
    let dir = temp_dir("long-name");
    let p = far_clip_project(&"A".repeat(20_000), WHOLE * 500_000);
    let bytes = ksq::to_bytes(&p, Compression::Gzip).unwrap();

    assert_ksqz_bounded_by_file(&dir, &bytes);
    let _ = fs::remove_dir_all(&dir);
}

/// 모르는 키는 텍스트로 가지 않는데(손실로 보고) 정규 `.ksq`에는 남아 상한을 키운다. 어림에는
/// 들어가지 않는다. release 측정: 640만 바이트 키를 넣은 6.5 KB `.ksqz`가 80 MB 텍스트,
/// RSS 1.06 GB, 2.9초. core의 압축 해제 상한(512 MiB)까지 키우면 상한은 128 기비바이트다.
#[test]
fn ksq2txt_small_ksqz_with_unknown_key_padding_stays_within_256x_file_or_errors() {
    let dir = temp_dir("unknown-pad");
    let json = ksq::to_string(&far_clip_project("A", WHOLE * 500_000));
    let padded = json.replacen(
        '{',
        &format!("{{\n  \"zz_pad\": \"{}\",", "a".repeat(20_000)),
        1,
    );
    assert!(ksq::from_str(&padded).is_ok());

    assert_ksqz_bounded_by_file(&dir, &gzip(padded.as_bytes()));
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- 지적 2: 성부와 어림

/// 긴 클립 끝에 시작이 같고 길이가 다른 노트 16개를 두면 성부 16개가 각각 클립 시작부터 쉼표를
/// 쓴다(keyscore-text의 면제 성부). 어림은 쉼표를 트랙에 한 번만 세므로 실제의 약 1/10이다.
/// 문서는 「텍스트를 만들기 전에 어림해 거른다」지만, 이 입력은 상한의 약 10배 텍스트를 다
/// 만든 뒤 안전망에서야 거부된다. release 측정: 이름 10만 바이트를 더한 485바이트 `.ksqz`가
/// 거부 전에 RSS 783 MB, 1.8초.
#[test]
fn project_to_text_sixteen_voices_far_notes_rejected_by_estimate_before_building() {
    let bars = 30_000;
    let last = (bars - 1) * WHOLE;
    let mut clip = Clip::new(Tick::ZERO, bars * WHOLE).unwrap();
    for k in 0..16u8 {
        clip.insert_note(note(last, 60 * (u32::from(k) + 1), 60 + k))
            .unwrap();
    }
    let mut track = Track::new("A", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip).unwrap();
    let mut p = Project::new();
    p.push_track(track).unwrap();

    let err = project_to_text(&p).unwrap_err();

    let CliError::OutputTooLarge { detail, .. } = &err else {
        panic!("{err}");
    };
    assert!(
        !detail.contains("어림보다 실제 출력이 컸다"),
        "어림을 지나 텍스트를 다 만든 뒤 거부했다: {err}"
    );
}

// ---------------------------------------------------------------- 지적 3: 특수 파일 출력

/// 리뷰 1 전에는 `fs::write`라 `-o /dev/null`(손실 보고만 보려는 흔한 쓰임)이 됐다. 원자적
/// 쓰기가 `/dev`에 임시 파일을 만들려다 EPERM으로 실패한다. FIFO는 일반 파일로 바뀐다(손으로 확인).
///
/// 결정 `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`로 기대를 바꿨다: `-o`는 없는 경로와
/// 일반 파일만 받으므로 `/dev/null`은 종료 1이고 그대로다(`-o` 없이 `> /dev/null`로 리다이렉트한다).
#[test]
fn ksq2txt_output_dev_null_succeeds_like_before() {
    let input = fixture("01-melody.ksq");

    let o = run(KSQ2TXT, &[&input, Path::new("-o"), Path::new("/dev/null")]);

    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    assert!(o.stdout.is_empty());
    assert!(std::os::unix::fs::FileTypeExt::is_char_device(
        &fs::metadata("/dev/null").unwrap().file_type()
    ));
}

// ---------------------------------------------------------------- 경계: 정상 곡을 거부하지 않음

/// 픽스처 8곡 각각을 50번 이어 붙여 400마디 곡으로 만들고, 400마디 뒤 노트 하나만 있는 빈 트랙,
/// 템포·박자 변화를 더한다. 어림(쉼표 8바이트/온음표, 노트 96바이트)이 평범한 긴 곡을 거부하지
/// 않고, 실제 출력도 상한 안이다.
#[test]
fn project_to_text_fixture_tiled_to_400_bars_with_sparse_track_is_accepted() {
    for (name, base) in fixtures() {
        let mut p = Project::new();
        for track in base.tracks() {
            let mut t = Track::new(track.name(), track.channel(), TrackColor::new(0).unwrap());
            for (_, c) in track.clips() {
                for k in 0..50 {
                    let mut tiled =
                        Clip::new(Tick::new(c.start().get() + k * c.length()), c.length()).unwrap();
                    for n in c.notes() {
                        tiled.insert_note(n.clone()).unwrap();
                    }
                    t.insert_clip(tiled).unwrap();
                }
            }
            p.push_track(t).unwrap();
        }
        let bar = base.meter_map().signature_at(Tick::ZERO).bar_ticks();
        p.push_track(far_clip_track(bar * 400)).unwrap();
        p.tempo_map_mut()
            .set(Tick::new(bar * 100), Tempo::from_bpm(90).unwrap());
        p.meter_map_mut()
            .set(Tick::new(bar * 200), TimeSignature::new(5, 8).unwrap());
        let limit = usize::try_from(MAX_EXPANSION).unwrap() * ksq::to_string(&p).len();

        let (text, _) = project_to_text(&p).unwrap_or_else(|e| panic!("{name}: {e}"));

        assert!(text.len() <= limit, "{name}: {} > {limit}", text.len());
    }
}

fn far_clip_track(start: u64) -> Track {
    let mut clip = Clip::new(Tick::new(start), 960).unwrap();
    clip.insert_note(note(0, 960, 72)).unwrap();
    let mut t = Track::new(
        "Sparse",
        Channel::new(2).unwrap(),
        TrackColor::new(1).unwrap(),
    );
    t.insert_clip(clip).unwrap();
    t
}

// ---------------------------------------------------------------- 경계: 원자적 쓰기의 실패 경로

/// `-o`가 디렉터리, 없는 디렉터리 안, 순환 심볼릭 링크면 종료 1이고 임시 파일이 남지 않는다.
/// 대상이 없는 심볼릭 링크는 링크가 가리키는 곳에 만들고 링크는 그대로 둔다.
#[test]
fn ksq2txt_output_dir_missing_dir_and_symlink_loop_fail_without_temp_and_dangling_link_creates_target()
 {
    let dir = temp_dir("atomic-edges");
    let input = fixture("01-melody.ksq");
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();
    std::os::unix::fs::symlink("l2", dir.join("l1")).unwrap();
    std::os::unix::fs::symlink("l1", dir.join("l2")).unwrap();
    std::os::unix::fs::symlink("made.txt", dir.join("dangling")).unwrap();

    let to_dir = run(KSQ2TXT, &[&input, Path::new("-o"), &sub]);
    let to_missing = run(
        KSQ2TXT,
        &[&input, Path::new("-o"), &dir.join("nope/out.txt")],
    );
    let to_loop = run(KSQ2TXT, &[&input, Path::new("-o"), &dir.join("l1")]);
    let to_dangling = run(KSQ2TXT, &[&input, Path::new("-o"), &dir.join("dangling")]);

    for o in [&to_dir, &to_missing, &to_loop] {
        assert_eq!(o.status.code(), Some(1), "{}", stderr(o));
    }
    assert_eq!(
        to_dangling.status.code(),
        Some(0),
        "{}",
        stderr(&to_dangling)
    );
    assert!(
        fs::symlink_metadata(dir.join("dangling"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read(dir.join("made.txt")).unwrap(),
        run(KSQ2TXT, &[&input]).stdout
    );
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["dangling", "l1", "l2", "made.txt", "sub"]);
    assert_eq!(fs::read_dir(&sub).unwrap().count(), 0);
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- 경계: BOM

/// BOM 하나뿐인 파일은 빈 파일과 같고, BOM 뒤 CRLF 텍스트는 엄격 모드로 읽히며, BOM 뒤 줄의
/// 경고 열 번호는 BOM이 없을 때와 같다.
#[test]
fn txt2ksq_bom_only_crlf_and_column_numbers_match_bomless_input() {
    let dir = temp_dir("bom-edges");
    let write = |name: &str, text: &str| {
        let p = dir.join(name);
        fs::write(&p, text).unwrap();
        p
    };
    let bom_only = write("bom-only.txt", "\u{feff}");
    let empty = write("empty.txt", "");
    let crlf = write("crlf.txt", "\u{feff}@track A ch1\r\nc4 d e\r\n");
    let bad_bom = write("bad-bom.txt", "\u{feff}@track A ch1 x\nc4\n");
    let bad = write("bad.txt", "@track A ch1 x\nc4\n");

    let from_bom_only = run(TXT2KSQ, &[&bom_only]);
    let from_empty = run(TXT2KSQ, &[&empty]);
    let from_crlf = run(TXT2KSQ, &[&crlf, Path::new("--strict")]);
    let from_bad_bom = run(TXT2KSQ, &[&bad_bom]);
    let from_bad = run(TXT2KSQ, &[&bad]);

    assert_eq!(from_bom_only.status.code(), from_empty.status.code());
    assert_eq!(from_bom_only.stdout, from_empty.stdout);
    assert_eq!(from_crlf.status.code(), Some(0), "{}", stderr(&from_crlf));
    assert_eq!(from_bad_bom.status.code(), from_bad.status.code());
    assert_eq!(
        stderr(&from_bad_bom).replace("bad-bom.txt", "bad.txt"),
        stderr(&from_bad),
        "BOM이 열 번호나 진단을 바꿨다"
    );
    let _ = fs::remove_dir_all(&dir);
}
