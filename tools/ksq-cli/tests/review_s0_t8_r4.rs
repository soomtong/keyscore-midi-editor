//! S0-T8 적대적 리뷰 4회차(`docs/reports/s0-t8/review-4.md`)가 남긴 테스트.
//!
//! - 실패하는 테스트: `-o`의 `/dev/` 판정이 경로를 정규화하지 않아, `/dev/..`로 시작하는 일반 파일
//!   경로를 잘라 쓰기 없이 그대로 덮어써 옛 꼬리가 남는 것(두 바이너리), `..`가 든 상대 심볼릭 링크로
//!   `/dev/stdout`을 가리키면 일반 파일로 돌린 표준 출력에 쓰지 못하는 것.
//! - 통과하는 테스트: 어림이 실제 출력의 위쪽 한계인 경계(타이 사슬·짧은 마디·드럼·가사·몰린 사건),
//!   평범한 곡과 README의 「약 1,400마디」 경계, `--max-output` 인자 파싱 경계.
//!
//! 바이너리를 띄우는 테스트는 시간 제한을 둔다.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use keyscore_core::ksq;
use keyscore_core::{
    Channel, Clip, MeterMap, Note, Pitch, Project, Tick, TimeSignature, Track, TrackColor, Velocity,
};
use ksq_cli::{CliError, project_to_text, project_to_text_limited};

const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

const WHOLE: u64 = 3840;
const TIMEOUT: Duration = Duration::from_secs(20);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-cli-review4-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixtures")
        .join(name)
}

fn note(start: u64, length: u64, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        u32::try_from(length).unwrap(),
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 시작 0, 길이 `length`인 클립 하나에 `notes`를 담은 채널 `channel` 트랙.
fn track(name: &str, channel: u8, length: u64, notes: impl IntoIterator<Item = Note>) -> Track {
    let mut clip = Clip::new(Tick::ZERO, length).unwrap();
    for n in notes {
        clip.insert_note(n).unwrap();
    }
    let mut t = Track::new(
        name,
        Channel::new(channel).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    t.insert_clip(clip).unwrap();
    t
}

fn project(time: (u8, u16), tracks: impl IntoIterator<Item = Track>) -> Project {
    let mut p = Project::new();
    *p.meter_map_mut() = MeterMap::new(TimeSignature::new(time.0, time.1).unwrap());
    for t in tracks {
        p.push_track(t).unwrap();
    }
    p
}

/// 만들기 전에 구한 크기의 위쪽 한계(어림). 상한 1이면 헤더만으로도 넘으므로 늘 어림에서 거부된다.
fn estimate(p: &Project) -> u64 {
    match project_to_text_limited(p, 1) {
        Err(CliError::OutputTooLarge {
            estimated, detail, ..
        }) => {
            assert!(!detail.contains("어림보다 실제 출력이 컸다"), "{detail}");
            estimated
        }
        other => panic!("{other:?}"),
    }
}

fn actual(p: &Project) -> u64 {
    let (text, _) = project_to_text_limited(p, u64::MAX).unwrap();
    text.len() as u64
}

/// 어림이 실제 이상이고, 상한을 실제 − 1로 주면 텍스트를 만들기 전에 어림이 거부한다.
fn assert_estimate_bounds(p: &Project, what: &str) {
    let (est, act) = (estimate(p), actual(p));
    assert!(est >= act, "{what}: 어림 {est} < 실제 {act}");
    match project_to_text_limited(p, act - 1) {
        Err(CliError::OutputTooLarge { detail, .. }) => {
            assert!(
                !detail.contains("어림보다 실제 출력이 컸다"),
                "{what}: {detail}"
            );
        }
        other => panic!("{what}: {other:?}"),
    }
}

/// 자식 프로세스를 기다리되 `TIMEOUT`을 넘으면 죽이고 실패한다.
fn wait_with_timeout(mut child: Child, what: &str) -> (ExitStatus, String) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                std::io::Read::read_to_string(&mut e, &mut err).unwrap();
            }
            return (status, err);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what}: {TIMEOUT:?} 안에 끝나지 않았다");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn run(bin: &str, args: &[&std::ffi::OsStr], stdout: Stdio) -> (ExitStatus, String) {
    let child = Command::new(bin)
        .args(args)
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_with_timeout(child, bin)
}

/// `/dev/..` 뒤에 절대 경로를 붙인 경로. 커널은 `/dev/..`를 `/`로 풀므로 같은 일반 파일이다.
fn via_dev_dotdot(path: &Path) -> PathBuf {
    let rest = path.strip_prefix("/").unwrap();
    Path::new("/dev/..").join(rest)
}

// --- 어림이 위쪽 한계인가 ---

/// 노트마다 출력이 가장 큰 꼴을 모았다: 16조각 타이 사슬(마디 7/4면 조각마다 `|`와 줄바꿈, 1/256이면
/// 조각마다 줄바꿈), 라벨 없는 드럼(`[n127]`), 옥타브 양 끝, 쉼표로 못 채워 여러 조각으로 나뉘는
/// 사건 앞 간격, 따옴표만으로 된 가사. 어림의 노트당 320바이트와 성부당 헤더 줄·꼬리 쉼표가 이 꼴들을
/// 모두 덮는다.
#[test]
fn project_to_text_long_tie_chains_with_awkward_gaps_estimate_bounds_actual() {
    let lengths = [15 * 6720 + 6705, 16 * 6720, 15 * 6720 + 3765];
    let gaps = [0, 7, 3765, 3825];
    let shapes: [(u8, u8, Option<&str>); 4] = [
        (10, 127, None),
        (10, 42, Some("\"\"\"\"")),
        (1, 0, None),
        (1, 127, Some("\\\"x")),
    ];
    for time in [(7, 4), (1, 256), (4, 4), (255, 1)] {
        for length in lengths {
            for gap in gaps {
                for (channel, pitch, lyric) in shapes {
                    let step = length + gap;
                    let notes = (0..12u64).map(|i| {
                        let n = note(gap + i * step, length, pitch);
                        match lyric {
                            Some(l) => n.with_lyric(l),
                            None => n,
                        }
                    });
                    let p = project(time, [track("T", channel, 12 * step + gap, notes)]);
                    assert_estimate_bounds(
                        &p,
                        &format!("{time:?} 길이 {length} 간격 {gap} ch{channel} 음 {pitch}"),
                    );
                }
            }
        }
    }
}

/// 리뷰 3의 몰린 사건을 박자표·가사·화음·같은 이름 트랙·긴 이름으로 바꿔 본다. 성부는 사건마다
/// 많아야 하나라 어림(성부 ≤ 노트 수 + 1)이 실제를 덮는다. 실제는 성부가 약 사건 수 / 8개라 어림의
/// 15% 안팎이다.
#[test]
fn project_to_text_crammed_events_in_many_shapes_estimate_bounds_actual() {
    let wholes = 600u64;
    let start = wholes * WHOLE;
    let long_name = "\"".repeat(300);
    for time in [(4, 4), (1, 256), (3, 8), (255, 1)] {
        let crammed = |name: &str, lyric: bool, chord: bool| {
            let notes = (1..=300u64).flat_map(move |len| {
                let top = note(start, len, 72);
                let top = if lyric { top.with_lyric("\"la\"") } else { top };
                let mut v = vec![top];
                if chord {
                    v.push(note(start, len, 0));
                }
                v
            });
            track(name, 1, start + 301, notes)
        };
        let p = project(
            time,
            [
                crammed("T", false, false),
                crammed("T", true, false),
                crammed(&long_name, false, true),
                track("빈 트랙", 1, start, []),
            ],
        );
        assert_estimate_bounds(&p, &format!("몰린 사건 {time:?}"));
    }
}

// --- 평범한 곡 ---

/// 4트랙 곡(4분음표 멜로디, 8분음표 베이스, 2분음표 3화음, 16분 하이햇 + 킥 넷 + 스네어 둘). 4/4.
fn band(measures: u64) -> Project {
    let len = measures * WHOLE;
    let melody = track(
        "Melody",
        1,
        len,
        (0..measures * 4).map(|i| note(i * 960, 960, 60 + u8::try_from(i % 12).unwrap())),
    );
    let bass = track(
        "Bass",
        2,
        len,
        (0..measures * 8).map(|i| note(i * 480, 480, 36 + u8::try_from(i % 5).unwrap())),
    );
    let chords = track(
        "Chords",
        3,
        len,
        (0..measures * 2).flat_map(|i| [60, 64, 67].map(|p| note(i * 1920, 1920, p))),
    );
    let drums = track(
        "Drums",
        10,
        len,
        (0..measures).flat_map(|b| {
            let s = b * WHOLE;
            let mut v: Vec<Note> = (0..16).map(|k| note(s + k * 240, 240, 42)).collect();
            v.extend((0..4).map(|k| note(s + k * 960, 240, 36)));
            v.extend([1, 3].map(|k| note(s + k * 960, 240, 38)));
            v
        }),
    );
    project((4, 4), [melody, bass, chords, drums])
}

fn quarter_track(measures: u64) -> Project {
    project(
        (4, 4),
        [track(
            "Melody",
            1,
            measures * WHOLE,
            (0..measures * 4).map(|i| note(i * 960, 960, 60 + u8::try_from(i % 12).unwrap())),
        )],
    )
}

/// 120 BPM 4/4에서 5분은 150마디, 10분은 300마디다. 두 배(600마디)까지 기본 상한에서 통과한다.
#[test]
fn project_to_text_five_to_ten_minute_band_song_is_accepted_by_default() {
    for bars in [150, 300, 600] {
        let p = band(bars);
        let result = project_to_text(&p);
        assert!(result.is_ok(), "{bars}마디: {result:?}");
    }
}

/// README 「4분음표 트랙이면 약 1,400마디부터」의 경계. 측정으로는 4분음표 트랙 1,423마디, 위 4트랙
/// 곡 1,408마디에서 처음 거부된다. 어림만큼 `--max-output`을 주면 통과한다.
#[test]
fn project_to_text_dense_tracks_rejected_from_about_1400_bars_and_max_output_admits() {
    assert!(project_to_text(&quarter_track(1400)).is_ok());
    assert!(project_to_text(&band(1400)).is_ok());

    for p in [quarter_track(1450), band(1450)] {
        let err = project_to_text(&p).unwrap_err();
        assert!(matches!(err, CliError::OutputTooLarge { .. }), "{err}");
        assert!(project_to_text_limited(&p, estimate(&p)).is_ok());
    }
}

/// 번들 픽스처 곡의 노트를 마디째 되풀이해 400마디로 늘려도 기본 상한에서 통과한다.
#[test]
fn project_to_text_bundled_fixture_tiled_to_400_bars_is_accepted() {
    let (src, _) = ksq::read_file(&fixture("04-band.ksq")).unwrap();
    let span = src
        .tracks()
        .iter()
        .flat_map(Track::clips)
        .map(|(_, c)| c.end())
        .max()
        .unwrap()
        .div_ceil(WHOLE)
        * WHOLE;
    let repeats = 400 * WHOLE / span;
    let tracks = src.tracks().iter().map(|t| {
        let notes: Vec<Note> = t
            .clips()
            .flat_map(|(_, c)| {
                c.notes()
                    .iter()
                    .map(move |n| (c.start().get() + n.start().get(), n))
            })
            .flat_map(|(s, n)| {
                (0..repeats)
                    .map(move |k| note(s + k * span, u64::from(n.length()), n.pitch().get()))
            })
            .collect();
        track(t.name(), t.channel().get(), repeats * span, notes)
    });
    let p = project((4, 4), tracks);

    let result = project_to_text(&p);

    assert!(result.is_ok(), "{result:?}");
}

// --- --max-output ---

fn ksq2txt_code(args: &[&str]) -> (u8, String) {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ksq_cli::cli::ksq2txt(&args, &mut out, &mut err);
    (code, String::from_utf8(err).unwrap())
}

#[test]
fn ksq2txt_max_output_bad_values_and_duplicates_exit_2_with_usage() {
    let input = fixture("01-melody.ksq");
    let input = input.to_str().unwrap();
    for bad in ["0", "-1", "18446744073709551616", "1e9", " 5", "", "0x10"] {
        let (code, err) = ksq2txt_code(&[input, "--max-output", bad]);
        assert_eq!(code, 2, "{bad:?}: {err}");
        assert!(err.contains("사용법"), "{bad:?}: {err}");
    }
    let (code, _) = ksq2txt_code(&[input, "--max-output"]);
    assert_eq!(code, 2);
    let (code, _) = ksq2txt_code(&["--max-output", "99999", input, "--max-output", "99999"]);
    assert_eq!(code, 2);
    let (code, _) = ksq2txt_code(&["--max-output", "99999", input]);
    assert_eq!(code, 0);
    let (code, _) = ksq2txt_code(&[input, "--max-output", "18446744073709551615"]);
    assert_eq!(code, 0);
}

/// 상한을 기본보다 작게 주면 어림에서 거부되고, 메시지가 옵션을 안내한다.
#[test]
fn ksq2txt_max_output_below_estimate_exits_1_with_hint() {
    let input = fixture("01-melody.ksq");
    let (code, err) = ksq2txt_code(&[input.to_str().unwrap(), "--max-output", "100"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("--max-output"), "{err}");
    assert!(!err.contains("어림보다 실제 출력이 컸다"), "{err}");
}

// --- /dev/ 경로 ---

/// `/dev/..`로 시작하는 경로는 `/dev/` 아래가 아니다(커널은 `/`로 푼다). 그런데 `special_output`이
/// `std::path::absolute`(`..`를 풀지 않는다)의 결과를 `starts_with("/dev")`로 보므로, 일반 파일을
/// 원자적 쓰기 대신 `write_in_place`로 보낸다. `write_in_place`는 잘라 쓰기(`truncate`) 없이 열어,
/// 기존 파일이 새 출력보다 길면 옛 꼬리가 남는다. 종료 코드는 0이다.
#[test]
fn ksq2txt_output_dev_dotdot_regular_file_replaced_without_stale_tail() {
    let dir = temp_dir("dotdot-txt");
    let input = fixture("01-melody.ksq");
    let (expected, _) = project_to_text(&ksq::read_file(&input).unwrap().0).unwrap();
    let out = dir.join("out.txt");
    fs::write(&out, "X".repeat(expected.len() + 20)).unwrap();
    let inode = fs::metadata(&out).unwrap().ino();

    let (status, err) = run(
        KSQ2TXT,
        &[
            input.as_os_str(),
            "-o".as_ref(),
            via_dev_dotdot(&out).as_os_str(),
        ],
        Stdio::null(),
    );

    assert!(status.success(), "{err}");
    assert_eq!(fs::read_to_string(&out).unwrap(), expected);
    assert_ne!(
        fs::metadata(&out).unwrap().ino(),
        inode,
        "이름 바꾸기가 아니다"
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// 같은 경로로 `txt2ksq`가 기존 `.ksq`를 덮으면 뒤에 옛 바이트가 남아 프로젝트를 읽을 수 없다.
#[test]
fn txt2ksq_output_dev_dotdot_existing_project_replaced_with_readable_project() {
    let dir = temp_dir("dotdot-ksq");
    let (text, _) = project_to_text(&ksq::read_file(&fixture("01-melody.ksq")).unwrap().0).unwrap();
    let input = dir.join("in.txt");
    fs::write(&input, &text).unwrap();
    let out = dir.join("out.ksq");
    fs::write(&out, " ".repeat(200_000) + "garbage").unwrap();

    let (status, err) = run(
        TXT2KSQ,
        &[
            input.as_os_str(),
            "-o".as_ref(),
            via_dev_dotdot(&out).as_os_str(),
        ],
        Stdio::null(),
    );

    assert!(status.success(), "{err}");
    let read = ksq::read_file(&out);
    assert!(read.is_ok(), "{read:?}");
    fs::remove_dir_all(&dir).unwrap();
}

/// README: 「심볼릭 링크를 따라간 결과가 `/dev/` 아래인 것 포함」. 링크 대상이 `..`로 `/dev/stdout`에
/// 닿는 상대 경로면 `resolve_symlink`의 결과(`<디렉터리>/../../dev/stdout`)가 `/dev`로 시작하지 않고,
/// 메타데이터는 일반 파일(돌린 표준 출력)이라 원자적 쓰기로 가 실패한다(리뷰 3 지적 3과 같은 꼴).
#[test]
fn ksq2txt_output_relative_symlink_to_dev_stdout_redirected_to_file_writes_text() {
    let dir = temp_dir("rel-stdout");
    let real = fs::canonicalize(&dir).unwrap();
    let depth = real
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();
    let target = "../".repeat(depth) + "dev/stdout";
    let link = dir.join("out");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let input = fixture("01-melody.ksq");
    let (expected, _) = project_to_text(&ksq::read_file(&input).unwrap().0).unwrap();
    let redirected = dir.join("stdout.txt");

    let (status, err) = run(
        KSQ2TXT,
        &[input.as_os_str(), "-o".as_ref(), link.as_os_str()],
        Stdio::from(fs::File::create(&redirected).unwrap()),
    );

    assert!(status.success(), "{err}");
    assert_eq!(fs::read_to_string(&redirected).unwrap(), expected);
    fs::remove_dir_all(&dir).unwrap();
}
