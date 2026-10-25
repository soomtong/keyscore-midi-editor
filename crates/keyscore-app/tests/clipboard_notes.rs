//! S3-T3a 순수 계산: 클립보드 내용(`ClipNotes`)의 정규화·SMF·텍스트 왕복, 붙여넣기·복제·삭제 편집, undo 뒤 선택
//! (REQ 7.5, KM-15, 5.10).

use std::collections::BTreeSet;

use keyscore_app::clipboard::ClipNotes;
use keyscore_app::pianoroll::edit::{
    PlacedClip, appeared, duplicate_offset, plan_delete, plan_duplicate, plan_paste, targets,
};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Note, Pitch, PitchOffset, Project, Tempo,
    Tick, Track, TrackColor, TrackId, Velocity,
};

const BAR: u64 = 3840;

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn key(notes: &[Note]) -> Vec<(u64, u32, u8)> {
    notes
        .iter()
        .map(|n| (n.start().get(), n.length(), n.pitch().get()))
        .collect()
}

/// 트랙 하나에 클립들(시작, 길이, 노트).
fn doc(clips: &[(u64, u64, Vec<Note>)]) -> (Document, TrackId, Vec<ClipId>) {
    let mut p = Project::new();
    let mut t = Track::new(
        "Lead",
        Channel::new(3).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let tid = t.id();
    let ids = clips
        .iter()
        .map(|(s, l, notes)| {
            let mut c = Clip::new(Tick::new(*s), *l).unwrap();
            for n in notes {
                c.insert_note(n.clone()).unwrap();
            }
            t.insert_clip(c).unwrap()
        })
        .collect();
    p.push_track(t).unwrap();
    (Document::new(p, usize::MAX).unwrap(), tid, ids)
}

#[test]
fn clip_notes_normalizes_to_earliest_start_and_span() {
    assert_eq!(ClipNotes::new(Vec::new()), None);
    let c = ClipNotes::new(vec![
        note(2000, 960, 64),
        note(960, 480, 60),
        note(960, 2000, 67),
    ])
    .unwrap();
    assert_eq!(
        key(c.notes()),
        vec![(0, 480, 60), (0, 2000, 67), (1040, 960, 64)]
    );
    // 가장 늦은 끝: 1040 + 960
    assert_eq!(c.span(), 2000);
    assert_eq!(
        key(&c.shifted(100).unwrap()),
        vec![(100, 480, 60), (100, 2000, 67), (1140, 960, 64)]
    );
    // 가사·피치 오프셋은 그대로
    let rich = note(500, 10, 60)
        .with_offset(PitchOffset::new(3).unwrap())
        .unwrap()
        .with_lyric("라");
    let c = ClipNotes::new(vec![rich.clone()]).unwrap();
    assert_eq!(c.notes()[0].lyric(), Some("라"));
    assert_eq!(c.notes()[0].offset(), rich.offset());
}

#[test]
fn clip_notes_smf_and_text_round_trip() {
    let notes = vec![
        note(0, 960, 60),
        note(960, 480, 62),
        note(1440, 480, 64),
        note(1920, 1920, 67),
        note(1920, 1920, 72),
    ];
    let c = ClipNotes::new(notes).unwrap();
    let (d, t, _) = doc(&[(0, BAR, Vec::new())]);
    let mut source = d.project().clone();
    // 템포가 다른 곡에서 복사해도 틱은 같다
    source
        .tempo_map_mut()
        .set(Tick::ZERO, Tempo::from_bpm(90).unwrap());
    let track = source.track(t).unwrap().clone();

    let smf = c.to_smf(&source, &track, Tick::ZERO).unwrap();
    assert_eq!(&smf[..4], b"MThd");
    let back = ClipNotes::from_smf(&smf).unwrap();
    assert_eq!(key(back.notes()), key(c.notes()));

    let text = c.to_text(&source, &track, Tick::ZERO).unwrap();
    let back = ClipNotes::from_text(&text).unwrap();
    assert_eq!(key(back.notes()), key(c.notes()), "{text}");
}

#[test]
fn from_text_rejects_non_notation_and_empty() {
    assert!(ClipNotes::from_text("").is_err());
    assert!(ClipNotes::from_text("hello, world! 123 ???").is_err());
    assert!(ClipNotes::from_smf(b"not a midi file").is_err());
    // 트랙 머리가 없는 MML 조각도 읽는다
    assert_eq!(
        key(ClipNotes::from_text("o4 l4 c d e").unwrap().notes()),
        vec![(0, 960, 60), (960, 960, 62), (1920, 960, 64)]
    );
}

#[test]
fn plan_paste_into_clip_gap_and_extension() {
    let (mut d, t, ids) = doc(&[(0, BAR, vec![note(0, 960, 60)]), (4 * BAR, BAR, Vec::new())]);
    let content = ClipNotes::new(vec![note(0, 960, 64), note(960, 960, 65)]).unwrap();

    // 클립 안: 클립 기준 시작으로
    let placed = plan_paste(d.project(), t, Tick::new(960), &content).unwrap();
    assert_eq!(placed.clip, PlacedClip::Existing(ids[0]));
    assert_eq!(key(&placed.notes), vec![(960, 960, 64), (1920, 960, 65)]);
    assert!(
        placed
            .edits
            .iter()
            .all(|e| matches!(e, Edit::InsertNote { .. }))
    );

    // 끝 넘김: 마디 단위로 늘린다
    let placed = plan_paste(d.project(), t, Tick::new(3 * 960), &content).unwrap();
    assert!(matches!(placed.edits[0], Edit::ExtendClip { length, .. } if length == 2 * BAR));
    d.apply(placed.edits, None).unwrap();
    assert_eq!(
        d.project().track(t).unwrap().clip(ids[0]).unwrap().length(),
        2 * BAR
    );

    // 틈: 자리 틱이 든 마디부터 새 클립
    let placed = plan_paste(d.project(), t, Tick::new(2 * BAR + 480), &content).unwrap();
    assert_eq!(
        placed.clip,
        PlacedClip::New {
            start: Tick::new(2 * BAR)
        }
    );
    assert_eq!(key(&placed.notes), vec![(480, 960, 64), (1440, 960, 65)]);
    d.apply(placed.edits, None).unwrap();
    assert_eq!(d.project().track(t).unwrap().clips().count(), 3);

    // 틈에서 다음 클립(4마디)과 겹치면 거부
    let long = ClipNotes::new(vec![note(0, 7680, 60)]).unwrap();
    assert!(matches!(
        plan_paste(d.project(), t, Tick::new(3 * BAR), &long),
        Err(ModelError::ClipOverlap { .. })
    ));
}

#[test]
fn plan_duplicate_offset_is_exact_span() {
    // 간격은 폭 그대로(REQ 7.5 「선택의 길이만큼」, 리뷰 1 지적 1). 스타카토 900틱 셋: 2820
    let staccato = [note(0, 900, 60), note(960, 900, 62), note(1920, 900, 64)];
    assert_eq!(duplicate_offset(&staccato), 2820);
    // 16분음표 하나 → 바로 뒤(240)
    assert_eq!(duplicate_offset(&[note(100, 240, 60)]), 240);
    // 화음은 폭이 가장 긴 노트
    assert_eq!(
        duplicate_offset(&[note(0, 960, 60), note(0, 1000, 64)]),
        1000
    );

    let (d, t, ids) = doc(&[(0, BAR, staccato.to_vec()), (2 * BAR, BAR, Vec::new())]);
    let placed = plan_duplicate(d.project(), t, ids[0], &staccato).unwrap();
    assert_eq!(
        key(&placed.notes),
        vec![(2820, 900, 60), (3780, 900, 62), (4740, 900, 64)]
    );
    assert!(matches!(placed.edits[0], Edit::ExtendClip { length, .. } if length == 2 * BAR));
    // 늘린 클립이 다음 클립과 겹치면 거부(다음 클립이 1.5마디에서 시작)
    let (d, t, ids) = doc(&[
        (0, BAR, staccato.to_vec()),
        (BAR + BAR / 2, BAR, Vec::new()),
    ]);
    assert!(matches!(
        plan_duplicate(d.project(), t, ids[0], &staccato),
        Err(ModelError::ClipOverlap { .. })
    ));
}

#[test]
fn targets_take_every_copy_and_delete_removes_them() {
    let twin = note(0, 960, 60);
    let (mut d, t, ids) = doc(&[(0, BAR, vec![twin.clone(), twin.clone(), note(960, 960, 62)])]);
    let clip = d.project().track(t).unwrap().clip(ids[0]).unwrap().clone();
    let values: BTreeSet<Note> = [twin.clone()].into_iter().collect();
    let picked = targets(&clip, &values);
    assert_eq!(picked.len(), 2);
    d.apply(plan_delete(t, ids[0], &picked), None).unwrap();
    assert_eq!(
        key(d.project().track(t).unwrap().clip(ids[0]).unwrap().notes()),
        vec![(960, 960, 62)]
    );
}

#[test]
fn appeared_picks_new_notes_in_one_clip() {
    let a = note(0, 960, 60);
    let b = note(960, 960, 62);
    let (d, t, ids) = doc(&[(0, BAR, vec![a.clone()]), (BAR, BAR, Vec::new())]);
    let before = d.project().track(t).unwrap().clone();

    // 한 클립에 나타남(같은 값이 하나 더 생긴 것도 나타남)
    let mut d1 = d.clone();
    d1.apply(
        vec![
            Edit::InsertNote {
                track: t,
                clip: ids[0],
                note: b.clone(),
            },
            Edit::InsertNote {
                track: t,
                clip: ids[0],
                note: a.clone(),
            },
        ],
        None,
    )
    .unwrap();
    let (clip, notes) = appeared(&before, d1.project().track(t).unwrap()).unwrap();
    assert_eq!(clip, ids[0]);
    assert_eq!(notes, [a.clone(), b.clone()].into_iter().collect());

    // 사라지기만 하면 None
    let mut d2 = d.clone();
    d2.apply(plan_delete(t, ids[0], std::slice::from_ref(&a)), None)
        .unwrap();
    assert_eq!(appeared(&before, d2.project().track(t).unwrap()), None);

    // 두 클립에 나타나면 None
    let mut d3 = d.clone();
    d3.apply(
        vec![
            Edit::InsertNote {
                track: t,
                clip: ids[0],
                note: b.clone(),
            },
            Edit::InsertNote {
                track: t,
                clip: ids[1],
                note: note(0, 10, 60),
            },
        ],
        None,
    )
    .unwrap();
    assert_eq!(appeared(&before, d3.project().track(t).unwrap()), None);
}
