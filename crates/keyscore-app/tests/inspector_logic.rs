//! S2-T5c 인스펙터 계산(GPUI 없음): GM 이름, 입력 파싱, 상태줄 트랙 글.

use keyscore_app::gm::{GM_PROGRAMS, program_label};
use keyscore_app::tracks::{
    color_label, parse_bank, parse_color, parse_velocity, status_track_text,
};
use keyscore_core::{Bank, Program, TrackColor, Velocity, ksq};

#[test]
fn gm_program_names_have_128_entries_and_one_based_labels() {
    assert_eq!(GM_PROGRAMS.len(), 128);
    assert_eq!(GM_PROGRAMS[0], "Acoustic Grand Piano");
    assert_eq!(GM_PROGRAMS[127], "Gunshot");
    assert_eq!(program_label(None), "없음");
    assert_eq!(
        program_label(Some(Program::new(0).unwrap())),
        "1 Acoustic Grand Piano"
    );
    assert_eq!(
        program_label(Some(Program::new(33).unwrap())),
        "34 Electric Bass (finger)"
    );
    assert_eq!(
        program_label(Some(Program::new(127).unwrap())),
        "128 Gunshot"
    );
    // 이름이 겹치지 않아야 검색 결과가 하나로 정해진다
    let mut names = GM_PROGRAMS.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 128);
}

#[test]
fn parse_bank_accepts_empty_and_range() {
    assert_eq!(parse_bank(""), Ok(None));
    assert_eq!(parse_bank("  "), Ok(None));
    assert_eq!(parse_bank("0"), Ok(Some(Bank::new(0).unwrap())));
    assert_eq!(parse_bank(" 16383 "), Ok(Some(Bank::new(16383).unwrap())));
    assert!(parse_bank("16384").is_err());
    assert!(parse_bank("-1").is_err());
    assert!(parse_bank("1.5").is_err());
    assert!(parse_bank("x").is_err());
}

#[test]
fn parse_velocity_accepts_1_to_127() {
    assert_eq!(parse_velocity("1"), Ok(Velocity::from_midi1(1).unwrap()));
    assert_eq!(
        parse_velocity(" 127"),
        Ok(Velocity::from_midi1(127).unwrap())
    );
    assert!(parse_velocity("0").is_err());
    assert!(parse_velocity("128").is_err());
    assert!(parse_velocity("").is_err());
    assert!(parse_velocity("abc").is_err());
}

#[test]
fn parse_color_accepts_names_and_indices() {
    assert_eq!(parse_color("red"), Ok(TrackColor::new(0).unwrap()));
    assert_eq!(parse_color("blue"), Ok(TrackColor::new(4).unwrap()));
    assert_eq!(
        parse_color("magenta_light"),
        Ok(TrackColor::new(11).unwrap())
    );
    assert_eq!(parse_color("11"), Ok(TrackColor::new(11).unwrap()));
    assert_eq!(parse_color("0"), Ok(TrackColor::new(0).unwrap()));
    assert!(parse_color("12").is_err());
    assert!(parse_color("mauve").is_err());
    assert!(parse_color("").is_err());
    // 표시 이름은 12개 모두 다르다
    let labels: std::collections::BTreeSet<String> = (0..12)
        .map(|i| color_label(TrackColor::new(i).unwrap()))
        .collect();
    assert_eq!(labels.len(), 12);
}

#[test]
fn status_track_text_names_track_port_and_channel() {
    let p = ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1},
            {"name": "", "port": {"named": "Synth"}, "channel": 10}]}"#,
    )
    .unwrap()
    .0;
    let ids: Vec<_> = p.tracks().iter().map(keyscore_core::Track::id).collect();
    assert_eq!(status_track_text(&p, ids[0], "KS Out"), "Lead · KS Out ch1");
    assert_eq!(
        status_track_text(&p, ids[1], "KS Out"),
        "트랙 2 · Synth ch10"
    );
}
