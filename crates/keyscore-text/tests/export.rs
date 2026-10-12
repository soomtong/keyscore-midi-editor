//! 내보내기 테스트(S0-T5 계획 테스트 4와 보조 테스트). 공개 API만 쓴다.

mod common;

use common::{NoteView, n, nl, notes, ok};
use keyscore_core::{
    Channel, Clip, Key, Note, Pitch, PitchClass, Program, Tempo, Tick, TimeSignature, Velocity,
};
use keyscore_text::{ExportTrack, Header, Mode, export};

fn clip(length: u64, ns: &[NoteView]) -> Clip {
    let mut c = Clip::new(Tick::ZERO, length).unwrap();
    for (s, l, p, lyric) in ns {
        let note = Note::new(
            Tick::new(*s),
            *l,
            Pitch::new(*p).unwrap(),
            Velocity::from_midi1(80).unwrap(),
        )
        .unwrap();
        let note = match lyric {
            Some(t) => note.with_lyric(t.clone()),
            None => note,
        };
        c.insert_note(note).unwrap();
    }
    c
}

fn track<'a>(name: &'a str, ch: u8, c: &'a Clip) -> ExportTrack<'a> {
    ExportTrack {
        name,
        channel: Channel::new(ch).unwrap(),
        program: None,
        bank: None,
        clip: c,
    }
}

/// 내보낸 뒤 다시 읽은 첫 트랙의 (노트, 클립 길이).
fn reparse(text: &str) -> (Vec<NoteView>, u64) {
    let p = ok(text, Mode::Lenient);
    assert!(p.warnings().is_empty(), "{:?}\n{text}", p.warnings());
    let c = p.tracks()[0].clip();
    (notes(c), c.length())
}

#[test]
fn export_snapshot() {
    let lead = clip(
        15_360,
        &[
            nl(0, 480, 60, "사"),
            n(480, 480, 62),
            n(960, 960, 64),
            n(1920, 960, 60),
            n(1920, 960, 64),
            nl(1920, 960, 67, "랑"),
            // 녹음된 비그리드 노트.
            n(2893, 455, 65),
            // 겹친 지속음은 성부를 나눈다.
            n(0, 3840, 48),
            // 8분 셋잇단.
            n(3840, 320, 60),
            n(4160, 320, 62),
            n(4480, 320, 64),
            // 온음표 둘 길이.
            n(5760, 7680, 72),
        ],
    );
    let drums = clip(
        1920,
        &[
            n(0, 240, 36),
            n(0, 240, 42),
            n(240, 240, 42),
            n(480, 240, 38),
            n(720, 240, 75),
        ],
    );
    let header = Header::new(
        Some(Tempo::from_bpm(120).unwrap()),
        Some(TimeSignature::COMMON),
        Some(Key::Major(PitchClass::new(0).unwrap())),
    );
    let mut lead_track = track("Lead", 1, &lead);
    lead_track.program = Some(Program::new(0).unwrap());
    let text = export(&header, &[lead_track, track("Drums", 10, &drums)]);
    insta::assert_snapshot!(text);
    let p = ok(&text, Mode::Lenient);
    assert!(p.warnings().is_empty(), "{:?}", p.warnings());
    assert_eq!(notes(p.tracks()[0].clip()), notes(&lead));
    assert_eq!(notes(p.tracks()[1].clip()), notes(&drums));
}

#[test]
fn export_grid_notes_have_no_offset_suffix() {
    let c = clip(
        3840,
        &[
            n(0, 960, 60),
            n(960, 480, 62),
            n(1440, 720, 64),
            n(2400, 240, 65),
            n(2880, 960, 67),
        ],
    );
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert!(!text.contains('~'), "{text}");
    assert_eq!(reparse(&text), (notes(&c), 3840));
}

#[test]
fn export_long_note_uses_ties_and_low_pitch_uses_octave_below_zero() {
    let c = clip(12_000, &[nl(0, 7680 + 960, 0, "길"), n(8640, 3360, 127)]);
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert!(text.contains('&'), "{text}");
    assert!(text.contains("o0 <"), "{text}");
    assert_eq!(reparse(&text), (notes(&c), 12_000));
}

#[test]
fn export_drum_note_without_label_uses_number() {
    let c = clip(960, &[n(0, 480, 75), n(480, 480, 36)]);
    let text = export(&Header::default(), &[track("D", 10, &c)]);
    assert!(text.contains("n75"), "{text}");
    assert!(text.contains("bd"), "{text}");
    assert_eq!(reparse(&text), (notes(&c), 960));
}

#[test]
fn export_tempo_uses_shortest_decimal_that_round_trips() {
    let header = Header::new(
        Some(Tempo::from_micros_per_quarter(497_925).unwrap()),
        Some(TimeSignature::new(7, 8).unwrap()),
        Some(Key::None),
    );
    let c = clip(960, &[n(0, 960, 60)]);
    let text = export(&header, &[track("T", 1, &c)]);
    assert!(text.contains("@tempo 120.5\n"), "{text}");
    assert!(text.contains("@time 7/8\n"), "{text}");
    assert!(text.contains("@key none\n"), "{text}");
    assert_eq!(*ok(&text, Mode::Lenient).header(), header);
}

#[test]
fn export_clip_length_beyond_last_note_is_kept_with_rests() {
    // 클립 끝까지 쉼표를 써서 가져온 클립 길이가 원본과 같다.
    let c = clip(7680, &[n(0, 100, 60)]);
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert_eq!(reparse(&text), (notes(&c), 7680));
    // 노트가 없는 클립도 쉼표로 길이를 지킨다.
    let empty = clip(3840 + 195, &[]);
    let text = export(&Header::default(), &[track("T", 1, &empty)]);
    assert_eq!(reparse(&text), (vec![], 3840 + 195));
}

#[test]
fn export_unreachable_clip_length_rounds_down() {
    // 쉼표 합(15틱 단위)으로도, 노트 끝으로도 닿지 않는 클립 길이는 문법으로 적을 수 없다.
    // 내보내기는 닿을 수 있는 가장 긴 길이로 줄인다(알려진 한계, 보고서 참고).
    let c = clip(1000, &[n(0, 960, 60)]);
    let text = export(&Header::default(), &[track("T", 1, &c)]);
    assert_eq!(reparse(&text), (notes(&c), 990));
    // 노트가 없고 닿을 길이가 0뿐이면 읽을 수 없는 빈 블록이 되므로 가장 짧은 쉼표로 늘린다.
    let tiny = clip(7, &[]);
    let text = export(&Header::default(), &[track("T", 1, &tiny)]);
    assert_eq!(reparse(&text), (vec![], 60));
}
