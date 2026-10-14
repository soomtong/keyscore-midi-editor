//! S0-T8 적대적 리뷰 1회차(`docs/reports/s0-t8/review-1.md`)가 남긴 테스트.
//!
//! - 실패하는 테스트: 작은 `.ksq`가 거대한 텍스트를 만드는 출력 증폭(지적 1).
//! - 통과하는 테스트: 손실 보고의 빠짐 경계, `check`의 버전 경계, CLI의 같은 파일 판정과 쓰기
//!   실패, 픽스처의 계획 표 대조.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use keyscore_core::{
    Channel, Clip, Note, OutputPort, Pitch, PitchOffset, Project, Tempo, Tick, TimeSignature,
    Track, TrackColor, Velocity, ksq,
};
use keyscore_text::Mode;
use ksq_cli::{CliError, Loss, check, cli, fixtures, project_to_text, text_to_project};

const KSQ: &str = env!("CARGO_BIN_EXE_ksq");
const KSQ2TXT: &str = env!("CARGO_BIN_EXE_ksq2txt");
const TXT2KSQ: &str = env!("CARGO_BIN_EXE_txt2ksq");

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ksq-cli-review-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn one_note_project(clip_start: u64) -> Project {
    let mut clip = Clip::new(Tick::new(clip_start), 960).unwrap();
    clip.insert_note(
        Note::new(
            Tick::ZERO,
            960,
            Pitch::new(60).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut track = Track::new("A", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(clip).unwrap();
    let mut p = Project::new();
    p.push_track(track).unwrap();
    p
}

// ---------------------------------------------------------------- 지적 1: 출력 증폭

/// 먼 틱(100만 마디 뒤)에서 시작하는 클립 하나짜리 `.ksq`(약 300바이트)가 시작 0 클립으로
/// 합쳐지며 마디마다 `r1 |`를 써 수 MB 텍스트가 된다. 10^12틱이면 수 GB 문자열과 되읽기로
/// 메모리를 다 쓴다. S0-T5가 지킨 「입력에 선형, 256배 이내」 정신에 맞게 출력이 상한 안이거나,
/// 상한을 넘으면 오류로 거부해야 한다.
#[test]
fn ksq2txt_far_clip_start_stays_within_256x_or_errors() {
    let p = one_note_project(3840 * 1_000_000);
    let ksq_len = ksq::to_string(&p).len();

    match project_to_text(&p) {
        Err(_) => {}
        Ok((text, _)) => assert!(
            text.len() <= 256 * ksq_len,
            "{ksq_len}바이트 .ksq가 {}바이트 텍스트가 됐다({}배)",
            text.len(),
            text.len() / ksq_len
        ),
    }
}

// ---------------------------------------------------------------- 손실 보고의 빠짐

/// 텍스트가 나르지 않는 속성 하나를 바꾼 프로젝트들. 각각 왕복 바이트가 달라지므로 손실이
/// 하나 이상 보고되어야 한다(「손실 없음 ⇒ 바이트 같음」의 대우). 무작위 proptest는 손실 없는
/// 사례가 드물어(보고서 「리뷰어에게」 3) 한 속성씩 확정적으로 본다.
fn perturbations(base: &Project) -> Vec<(&'static str, Project)> {
    let mut out = Vec::new();
    let mut add = |label: &'static str, f: &dyn Fn(&mut Project)| {
        let mut p = base.clone();
        f(&mut p);
        out.push((label, p));
    };
    let first_clip = |p: &Project| p.tracks()[0].clips().next().unwrap().0;
    let first_note = |p: &Project| p.tracks()[0].clips().next().unwrap().1.notes()[0].clone();
    add("velocity", &|p| {
        let id = first_clip(p);
        let n = first_note(p);
        let loud = Note::new(n.start(), n.length(), n.pitch(), Velocity::new(1)).unwrap();
        let t = p.track_mut(0).unwrap();
        assert!(t.remove_note(id, &n).unwrap());
        t.insert_note(id, loud).unwrap();
    });
    add("velocity off by one 16-bit step", &|p| {
        let id = first_clip(p);
        let n = first_note(p);
        let v = Velocity::new(n.velocity().get() + 1);
        let changed = Note::new(n.start(), n.length(), n.pitch(), v).unwrap();
        let t = p.track_mut(0).unwrap();
        assert!(t.remove_note(id, &n).unwrap());
        t.insert_note(id, changed).unwrap();
    });
    add("offset", &|p| {
        let id = first_clip(p);
        let n = first_note(p);
        let bent = n.clone().with_offset(PitchOffset::new(1).unwrap()).unwrap();
        let t = p.track_mut(0).unwrap();
        assert!(t.remove_note(id, &n).unwrap());
        t.insert_note(id, bent).unwrap();
    });
    add("mute", &|p| p.track_mut(0).unwrap().set_mute(true));
    add("solo", &|p| p.track_mut(0).unwrap().set_solo(true));
    add("port", &|p| {
        p.track_mut(0)
            .unwrap()
            .set_port(OutputPort::Named("IAC".to_owned()));
    });
    add("default_velocity", &|p| {
        p.track_mut(0)
            .unwrap()
            .set_default_velocity(Velocity::new(7));
    });
    add("lyric_lang", &|p| {
        p.track_mut(0)
            .unwrap()
            .set_lyric_lang(Some("ko".to_owned()));
    });
    add("color", &|p| {
        p.track_mut(0)
            .unwrap()
            .set_color(TrackColor::new(11).unwrap());
    });
    add("clip name", &|p| {
        let id = first_clip(p);
        p.track_mut(0).unwrap().set_clip_name(id, "x").unwrap();
    });
    add("clip color", &|p| {
        let id = first_clip(p);
        p.track_mut(0)
            .unwrap()
            .set_clip_color(id, Some(TrackColor::new(3).unwrap()))
            .unwrap();
    });
    add("clip moved", &|p| {
        let id = first_clip(p);
        p.track_mut(0)
            .unwrap()
            .move_clip(id, Tick::new(15))
            .unwrap();
    });
    add("clip split", &|p| {
        let id = first_clip(p);
        p.track_mut(0)
            .unwrap()
            .split_clip(id, Tick::new(1000))
            .unwrap();
    });
    add("clip removed", &|p| {
        let id = first_clip(p);
        p.track_mut(0).unwrap().remove_clip(id);
    });
    add("tempo change", &|p| {
        p.tempo_map_mut()
            .set(Tick::new(960), Tempo::from_bpm(61).unwrap());
    });
    add("meter change", &|p| {
        p.meter_map_mut()
            .set(Tick::new(7680), TimeSignature::new(5, 8).unwrap());
    });
    if base.tracks().len() > 1 {
        add("duplicate name", &|p| {
            let name = p.tracks()[0].name().to_owned();
            p.track_mut(1).unwrap().set_name(name);
        });
    }
    out
}

#[test]
fn ksq2txt_single_untextable_change_on_every_fixture_reports_loss() {
    for (fixture, base) in fixtures() {
        let (_, base_losses) = project_to_text(&base).unwrap();
        assert_eq!(base_losses, vec![], "{fixture}");
        for (label, p) in perturbations(&base) {
            let (text, losses) = project_to_text(&p).unwrap();
            let (back, _) = text_to_project(&text, Mode::Lenient).unwrap();

            assert_ne!(
                ksq::to_string(&back),
                ksq::to_string(&p),
                "{fixture} {label}: 바뀐 것이 없다(테스트 전제 오류)"
            );
            assert!(!losses.is_empty(), "{fixture} {label}: 손실 보고 없음");
            assert!(
                !losses.iter().any(|l| matches!(l, Loss::Mismatch { .. })),
                "{fixture} {label}: {losses:?}"
            );
        }
    }
}

/// 모르는 키는 모든 층(최상위·조·트랙·포트·클립·노트)에서 보고된다. 이 도구의 키 목록이 core와
/// 어긋나면(보고서 「리뷰어에게」 1) 손실 없는 원본에서 거짓 손실이 나므로 그 방향도 본다.
#[test]
fn ksq2txt_unknown_keys_at_every_level_are_reported_and_known_keys_are_not() {
    let base = fixtures().remove(0).1;
    let text = ksq::to_string(&base);
    let (p, _) = ksq::from_str(&text).unwrap();
    assert_eq!(project_to_text(&p).unwrap().1, vec![]);

    let mut port = base.clone();
    port.track_mut(0)
        .unwrap()
        .set_port(OutputPort::Named("IAC".to_owned()));
    let port_text = ksq::to_string(&port).replacen(
        "{\"named\": \"IAC\"}",
        "{\"named\": \"IAC\", \"x_port\": 1}",
        1,
    );
    // 01-melody는 C장조라 조가 객체로 쓰인다.
    let key_text = text.replacen(
        "\"mode\": \"major\"",
        "\"mode\": \"major\", \"x_key\": 1",
        1,
    );
    for (label, edited, path) in [
        (
            "root",
            text.replacen(
                "\"format_version\": 1,",
                "\"format_version\": 1, \"x\": 1,",
                1,
            ),
            "x",
        ),
        ("key", key_text, "key.x_key"),
        (
            "clip",
            text.replacen("\"start\": 0,", "\"start\": 0, \"x_clip\": [1],", 1),
            "tracks[0].clips[0].x_clip",
        ),
        ("port", port_text, "tracks[0].port.x_port"),
    ] {
        assert_ne!(edited, text, "{label}: 치환 실패");
        let (p, _) = ksq::from_str(&edited).unwrap();
        let (_, losses) = project_to_text(&p).unwrap();
        let unknown: Vec<_> = losses
            .iter()
            .filter_map(|l| match l {
                Loss::UnknownKeys { paths } => Some(paths.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(unknown, vec![path.to_owned()], "{label}: {losses:?}");
    }
}

/// 텍스트 문법과 부딪치는 이름·가사도 손실 없이 바이트까지 왕복한다(못 깨뜨린 경계).
#[test]
fn ksq2txt_hostile_names_and_lyrics_round_trip_bytes_without_losses() {
    for name in [
        "", " A", "A ", "A\nB", "A ch2", "A prog5", "@track B", "A\"", "a#b", "A\u{0}", "//x",
        "A\tB", "A\rB",
    ] {
        for lyric in [
            None,
            Some(""),
            Some(" "),
            Some("-"),
            Some("a\nb"),
            Some("\""),
            Some("|"),
        ] {
            let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
            let mut note = Note::new(
                Tick::ZERO,
                960,
                Pitch::new(60).unwrap(),
                Velocity::from_midi1(100).unwrap(),
            )
            .unwrap();
            if let Some(l) = lyric {
                note = note.with_lyric(l);
            }
            clip.insert_note(note).unwrap();
            let mut t = Track::new(name, Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
            t.insert_clip(clip).unwrap();
            let mut p = Project::new();
            p.push_track(t).unwrap();
            // 대소문자·공백만 다른 이름이 같은 블록으로 합쳐지지 않는지도 본다.
            let mut t2 = Track::new(
                format!("{name} "),
                Channel::new(1).unwrap(),
                TrackColor::new(1).unwrap(),
            );
            t2.insert_clip(Clip::new(Tick::ZERO, 3840).unwrap())
                .unwrap();
            p.push_track(t2).unwrap();

            let (text, losses) = project_to_text(&p).unwrap();
            let (back, _) = text_to_project(&text, Mode::Lenient).unwrap();

            assert_eq!(losses, vec![], "{name:?} {lyric:?}\n{text}");
            assert_eq!(
                ksq::to_string(&back),
                ksq::to_string(&p),
                "{name:?} {lyric:?}"
            );
        }
    }
}

// ---------------------------------------------------------------- check

fn check_text(dir: &Path, name: &str, text: &str) -> Result<ksq_cli::CheckReport, CliError> {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    check(&path)
}

/// 정수가 아닌 `format_version`(2.0, u64 밖)이 「버전 없음」 경고로 통과하지 않는다. core가
/// 형식 오류로 거부하므로 실패다.
#[test]
fn check_non_integer_or_overflowing_version_fails() {
    let dir = temp_dir("version");
    for v in ["2.0", "1.0", "18446744073709551616", "-1", "\"2\"", "null"] {
        let r = check_text(
            &dir,
            "v.ksq",
            &format!("{{\"format_version\": {v}, \"tracks\": []}}"),
        );
        assert!(r.is_err(), "{v}: {r:?}");
    }
    // 경계: 아는 버전과 옛 버전은 통과, 다음 버전은 실패.
    assert!(check_text(&dir, "v.ksq", "{\"format_version\": 1}").is_ok());
    assert!(check_text(&dir, "v.ksq", "{\"format_version\": 0}").is_ok());
    assert!(matches!(
        check_text(&dir, "v.ksq", "{\"format_version\": 2}"),
        Err(CliError::NewerVersion { version: 2, .. })
    ));
    // 같은 키가 두 번이면 check와 core가 같은 값(나중 것)을 본다.
    let r = check_text(
        &dir,
        "v.ksq",
        "{\"format_version\": 9, \"format_version\": 1}",
    )
    .unwrap();
    assert_eq!(r.version(), Some(1));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn check_empty_bom_and_non_utf8_path_exit_one_without_panic() {
    let dir = temp_dir("check-edge");
    let empty = dir.join("empty.ksq");
    fs::write(&empty, "").unwrap();
    let bom = dir.join("bom.ksq");
    fs::write(&bom, "\u{feff}{\"format_version\": 1}").unwrap();

    let o = Command::new(KSQ)
        .arg("check")
        .args([&empty, &bom])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));

    // 비 UTF-8 경로는 함수로 직접 넘긴다(APFS는 그런 이름의 파일을 만들지 못한다).
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let args = vec![
            OsString::from("check"),
            OsString::from_vec(vec![b'x', 0xff]),
        ];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(cli::ksq(&args, &mut out, &mut err), 1);
        assert!(!err.is_empty());
    }
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- CLI

#[test]
fn cli_same_file_via_relative_symlink_hardlink_and_case_is_refused_and_input_kept() {
    let dir = temp_dir("same");
    let input = dir.join("in.ksq");
    fs::write(&input, ksq::to_string(&one_note_project(0))).unwrap();
    let before = fs::read(&input).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("in.ksq", dir.join("link.ksq")).unwrap();
    fs::hard_link(&input, dir.join("hard.ksq")).unwrap();

    for out in [
        "in.ksq",
        "./in.ksq",
        "link.ksq",
        "hard.ksq",
        "../same/in.ksq",
    ] {
        for bin in [KSQ2TXT, TXT2KSQ] {
            let o = Command::new(bin)
                .current_dir(&dir)
                .args(["in.ksq", "-o", out])
                .output()
                .unwrap();
            assert_eq!(o.status.code(), Some(1), "{bin} -o {out}");
        }
    }
    assert_eq!(fs::read(&input).unwrap(), before);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cli_output_in_missing_or_readonly_dir_fails_and_input_kept() {
    let dir = temp_dir("write-fail");
    let input = dir.join("in.ksq");
    fs::write(&input, ksq::to_string(&one_note_project(0))).unwrap();
    let before = fs::read(&input).unwrap();

    for out in [dir.join("nope/out.txt"), dir.clone()] {
        let o = Command::new(KSQ2TXT)
            .arg(&input)
            .arg("-o")
            .arg(&out)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(1), "{}", out.display());
    }
    let o = Command::new(TXT2KSQ)
        .arg(&input)
        .arg("-o")
        .arg(dir.join("nope/out.ksq"))
        .output()
        .unwrap();
    // 입력이 .ksq(JSON)라 텍스트로 읽히지 않아도, 어느 쪽이든 실패이고 입력은 그대로다.
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(fs::read(&input).unwrap(), before);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cli_argument_edges_are_usage_errors() {
    let run = |f: fn(&[OsString], &mut dyn std::io::Write, &mut dyn std::io::Write) -> u8,
               args: &[&str]| {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        f(&args, &mut out, &mut err)
    };
    assert_eq!(run(cli::ksq2txt, &[]), 2);
    assert_eq!(run(cli::ksq2txt, &["a", "-o"]), 2);
    assert_eq!(run(cli::ksq2txt, &["a", "-o", "b", "-o", "c"]), 2);
    assert_eq!(run(cli::ksq2txt, &["a", "b"]), 2);
    assert_eq!(run(cli::ksq2txt, &["a", "--strict"]), 2);
    assert_eq!(run(cli::txt2ksq, &["a", "-x"]), 2);
    assert_eq!(run(cli::ksq, &["check"]), 2);
    assert_eq!(run(cli::ksq, &["fixtures"]), 2);
    assert_eq!(run(cli::ksq, &["fixtures", "a", "b"]), 2);
    assert_eq!(run(cli::ksq, &["frob"]), 2);
    // 「-」 하나는 옵션이 아니라 파일 이름이다(없는 파일이라 실패 1).
    assert_eq!(run(cli::ksq2txt, &["-"]), 1);
}

// ---------------------------------------------------------------- 픽스처

/// 계획 표의 종류와 「8마디」를 대조한다. 모든 트랙의 클립이 시작 0, 길이 8마디다.
#[test]
fn fixtures_match_plan_table_and_are_eight_bars() {
    let all = fixtures();
    assert_eq!(all.len(), 8);
    for (name, p) in &all {
        let bar = p.meter_map().signature_at(Tick::ZERO).bar_ticks();
        for t in p.tracks() {
            let clips: Vec<_> = t.clips().map(|(_, c)| c).collect();
            assert_eq!(clips.len(), 1, "{name}");
            assert_eq!(clips[0].start(), Tick::ZERO, "{name}");
            assert_eq!(clips[0].length(), 8 * bar, "{name} {}", t.name());
            assert!(
                clips[0]
                    .notes()
                    .iter()
                    .all(|n| n.end() <= clips[0].length()),
                "{name}: 숨김 꼬리"
            );
        }
    }
    let get = |n: &str| &all.iter().find(|(f, _)| *f == n).unwrap().1;
    let drum = |t: &Track| t.channel().get() == 10;

    assert_eq!(get("01-melody.ksq").tracks().len(), 1);
    assert_eq!(get("02-melody-chords.ksq").tracks().len(), 2);
    let drums = get("03-drums.ksq");
    assert!(drums.tracks().len() == 1 && drum(&drums.tracks()[0]));
    let band = get("04-band.ksq");
    assert_eq!(band.tracks().len(), 4);
    assert!(band.tracks().iter().any(drum));
    let lyrics = get("05-lyrics.ksq").tracks()[0].clips().next().unwrap().1;
    assert!(lyrics.notes().iter().all(|n| n.lyric().is_some()));
    let waltz = get("06-waltz.ksq");
    let sig = waltz.meter_map().signature_at(Tick::ZERO);
    assert_eq!((sig.numerator(), sig.denominator()), (3, 4));
    assert!(matches!(waltz.key(), keyscore_core::Key::Minor(_)));
    let triplets = get("07-triplets.ksq");
    assert!(triplets.tracks().iter().any(|t| {
        t.clips()
            .flat_map(|(_, c)| c.notes())
            .any(|n| n.length() % 320 == 0 && n.length() % 480 != 0)
    }));
    let bass_drums = get("08-bass-drums.ksq");
    assert_eq!(bass_drums.tracks().len(), 2);
    assert!(
        bass_drums
            .tracks()
            .iter()
            .all(|t| t.bank().is_some() && t.program().is_some())
    );
}
