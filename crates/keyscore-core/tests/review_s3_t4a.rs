//! S3-T4a 적대적 리뷰 1회차: `transform::splice`를 다음 클립·지우기·바꾸기·undo와 함께 흔든다. 픽스처는
//! `transform_splice.rs`에서 옮겨 왔다.
#![allow(dead_code, unused_imports)]

use keyscore_core::transform::{Splice, splice};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Note, Pitch, Project, Tick, Track,
    TrackColor, TrackId, Velocity,
};
use proptest::prelude::*;

const BAR: u64 = 3840;

fn note(start: u64, length: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 트랙 하나에 클립들(시작, 길이, 노트). 박자표는 4/4.
fn doc(clips: &[(u64, u64, Vec<Note>)]) -> (Document, TrackId, Vec<ClipId>) {
    let mut p = Project::new();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
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

fn triples(d: &Document, t: TrackId, c: ClipId) -> Vec<(u64, u32, u8)> {
    d.project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .notes()
        .iter()
        .map(|n| (n.start().get(), n.length(), n.pitch().get()))
        .collect()
}

fn length(d: &Document, t: TrackId, c: ClipId) -> u64 {
    d.project().track(t).unwrap().clip(c).unwrap().length()
}

proptest! {
    /// 다음 클립이 있는 트랙에서 지우기·바꾸기·밀기·넣기를 섞어도: 거부면 문서 그대로, 적용되면 클립이 겹치지
    /// 않고 모든 노트 시작이 클립 안이며, 다음 클립은 그대로이고, undo가 원래 문서로 되돌린다.
    #[test]
    fn splice_with_next_clip_keeps_invariants_and_undoes(
        starts in prop::collection::vec((0u64..BAR * 2, 1u32..3000, 40u8..90), 0..10),
        next_at in BAR * 2..BAR * 4,
        at in 0u64..BAR * 2,
        by in -2000i64..4000,
        remove_mask in any::<u16>(),
        replace_mask in any::<u16>(),
        grow in 0u32..2000,
        insert_len in 0u32..3000,
    ) {
        let notes: Vec<Note> = starts.iter().map(|&(s, l, p)| note(s, l.min(u32::try_from(BAR * 2 - s).unwrap().max(1)), p)).collect();
        let (mut d, t, ids) = doc(&[(0, BAR * 2, notes.clone()), (next_at, BAR, vec![note(0, 100, 60)])]);
        let before = d.project().clone();
        let mut remove = Vec::new();
        let mut replace = Vec::new();
        for (i, n) in notes.iter().enumerate() {
            if remove_mask & (1 << i) != 0 {
                remove.push(n.clone());
            } else if replace_mask & (1 << i) != 0 {
                let to = n.with_length(n.length() + grow).unwrap();
                replace.push((n.clone(), to));
            }
        }
        let insert = if insert_len > 0 { vec![note(at, insert_len, 72)] } else { vec![] };
        let s = Splice { at, by, remove, replace, insert, reach: 0 };
        if let Ok(edits) = splice(d.project(), t, ids[0], &s) {
            d.apply(edits, None).unwrap();
            let track = d.project().track(t).unwrap();
            let clips: Vec<_> = track.clips().map(|(_, c)| (c.start().get(), c.end())).collect();
            for w in clips.windows(2) {
                prop_assert!(w[0].1 <= w[1].0, "클립 겹침 {:?}", clips);
            }
            let c = track.clip(ids[0]).unwrap();
            prop_assert!(c.notes().iter().all(|n| n.start().get() < c.length()));
            let nxt = track.clip(ids[1]).unwrap();
            prop_assert_eq!(nxt.start().get(), next_at);
            d.undo().unwrap();
            prop_assert_eq!(d.project(), &before);
        } else {
            prop_assert_eq!(d.project(), &before);
        }
    }
}
