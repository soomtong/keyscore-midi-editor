//! S2-T5b 트랙 목록·트랜스포트 바 계산(GPUI 없음).

use keyscore_app::tracks::{
    format_meter, format_position, format_tempo, key_choices, key_label, lowest_free_channel,
    parse_meter, parse_tempo, port_label, program_conflicts,
};
use keyscore_core::{Key, OutputPort, PitchClass, Tempo, Tick, TimeSignature, ksq};

fn project(tracks: &str) -> keyscore_core::Project {
    ksq::from_str(&format!(r#"{{"tracks": [{tracks}]}}"#))
        .unwrap()
        .0
}

#[test]
fn lowest_free_channel_fills_gaps_per_port() {
    let p = project(
        r#"{"port": "virtual", "channel": 1}, {"port": "virtual", "channel": 3},
           {"port": {"named": "B"}, "channel": 2}"#,
    );
    assert_eq!(
        lowest_free_channel(&p, &OutputPort::Virtual).map(keyscore_core::Channel::get),
        Some(2)
    );
    assert_eq!(
        lowest_free_channel(&p, &OutputPort::Named("B".into())).map(keyscore_core::Channel::get),
        Some(1)
    );
    let full: Vec<String> = (1..=16)
        .map(|c| format!(r#"{{"port": "virtual", "channel": {c}}}"#))
        .collect();
    let p = project(&full.join(","));
    assert_eq!(lowest_free_channel(&p, &OutputPort::Virtual), None);
}

#[test]
fn program_conflicts_need_same_port_channel_and_different_programs() {
    let p = project(
        r#"{"port": "virtual", "channel": 1, "program": 5},
           {"port": "virtual", "channel": 1, "program": 9},
           {"port": "virtual", "channel": 2, "program": 5},
           {"port": "virtual", "channel": 2, "program": 5},
           {"port": {"named": "B"}, "channel": 1, "program": 7}"#,
    );
    let ids: Vec<_> = p.tracks().iter().map(keyscore_core::Track::id).collect();
    let conflicts = program_conflicts(&p);
    assert!(conflicts.contains(&ids[0]) && conflicts.contains(&ids[1]));
    assert!(!conflicts.contains(&ids[2]) && !conflicts.contains(&ids[3]));
    assert!(!conflicts.contains(&ids[4]));
    // 프로그램이 없는 트랙과 있는 트랙도 다르다
    let p = project(
        r#"{"port": "virtual", "channel": 1}, {"port": "virtual", "channel": 1, "program": 3}"#,
    );
    assert_eq!(program_conflicts(&p).len(), 2);
}

#[test]
fn position_is_one_based_bar_beat_tick() {
    let p = ksq::from_str(r#"{"meter": [[0, 4, 4], [3840, 3, 4]]}"#)
        .unwrap()
        .0;
    assert_eq!(format_position(&p, Tick::ZERO), "1.1.0");
    assert_eq!(format_position(&p, Tick::new(960 + 120)), "1.2.120");
    assert_eq!(format_position(&p, Tick::new(3840)), "2.1.0");
    assert_eq!(format_position(&p, Tick::new(3840 + 2880)), "3.1.0");
}

#[test]
fn tempo_field_round_trips_one_decimal() {
    assert_eq!(format_tempo(Tempo::DEFAULT), "120.0");
    let t = parse_tempo("90").unwrap();
    assert_eq!(format_tempo(t), "90.0");
    let t = parse_tempo(" 120.5 ").unwrap();
    assert_eq!(format_tempo(t), "120.5");
    for bad in ["", "abc", "0", "9.9", "1000", "120.55", "-3"] {
        assert!(parse_tempo(bad).is_err(), "{bad}");
    }
}

#[test]
fn meter_field_parses_power_of_two_denominators() {
    assert_eq!(format_meter(TimeSignature::new(6, 8).unwrap()), "6/8");
    assert_eq!(
        parse_meter("3/4").unwrap(),
        TimeSignature::new(3, 4).unwrap()
    );
    assert_eq!(
        parse_meter(" 7 / 8 ").unwrap(),
        TimeSignature::new(7, 8).unwrap()
    );
    for bad in ["", "3", "3/5", "0/4", "4/0", "x/4", "300/4"] {
        assert!(parse_meter(bad).is_err(), "{bad}");
    }
}

#[test]
fn key_choices_and_labels() {
    let keys = key_choices();
    assert_eq!(keys.len(), 25);
    assert_eq!(keys[0], Key::None);
    assert_eq!(key_label(Key::None), "조성 없음");
    assert_eq!(key_label(Key::Major(PitchClass::new(0).unwrap())), "C장조");
    assert_eq!(key_label(Key::Minor(PitchClass::new(9).unwrap())), "A단조");
    assert_eq!(key_label(Key::Major(PitchClass::new(1).unwrap())), "C#장조");
}

#[test]
fn port_labels() {
    assert_eq!(
        port_label(&OutputPort::Virtual, "KeyScore Out"),
        "KeyScore Out"
    );
    assert_eq!(
        port_label(&OutputPort::Named("IAC".into()), "KeyScore Out"),
        "IAC"
    );
}
