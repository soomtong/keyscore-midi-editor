//! S3-T3b 노트 편집의 순수 계산(`pianoroll::edit`, `pianoroll::keys`)과 퀀타이즈 설정 읽기·쓰기. 규칙은
//! `docs/reports/s3-t3b/plan.md` 「끝의 정의」.

use keyscore_app::actions::args::{KeyLayout, LengthCode};
use keyscore_app::pianoroll::edit::{
    Changed, DEFAULT_LENGTH, length_ticks, plan_join, plan_legato, plan_quantize, plan_restore,
    plan_set_length, plan_set_pitch, plan_split, selection_after,
};
use keyscore_app::pianoroll::keys::{pitch_in_octave_of, semitone_of_key};
use keyscore_app::settings::{QuantizeSettings, Settings};
use keyscore_core::transform::Grid;
use keyscore_core::{
    Channel, Clip, ClipId, Document, Note, Pitch, Project, Tick, Track, TrackColor, TrackId,
    Velocity,
};

const BAR: u64 = 3840;

type Triple = (u64, u32, u8);

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn triple(n: &Note) -> Triple {
    (n.start().get(), n.length(), n.pitch().get())
}

/// 트랙 하나, 클립 `clips`((시작, 길이, 노트)). 클립 id는 차례대로.
fn project(clips: &[(u64, u64, &[Triple])]) -> (Project, TrackId, Vec<ClipId>) {
    let mut p = Project::new();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let tid = t.id();
    let mut ids = Vec::new();
    for &(start, length, notes) in clips {
        let mut c = Clip::new(Tick::new(start), length).unwrap();
        for &(s, l, pitch) in notes {
            c.insert_note(note(s, l, pitch)).unwrap();
        }
        ids.push(t.insert_clip(c).unwrap());
    }
    p.push_track(t).unwrap();
    (p, tid, ids)
}

fn notes(t: &[Triple]) -> Vec<Note> {
    t.iter().map(|&(s, l, p)| note(s, l, p)).collect()
}

/// 편집을 적용한 뒤 첫 트랙의 클립들((시작, 길이, 노트)).
fn applied(project: &Project, changed: Changed) -> Vec<(u64, u64, Vec<Triple>)> {
    let mut doc = Document::new(project.clone(), usize::MAX).unwrap();
    doc.apply(changed.edits, None).unwrap();
    doc.project().tracks()[0]
        .clips()
        .map(|(_, c)| {
            (
                c.start().get(),
                c.length(),
                c.notes().iter().map(triple).collect(),
            )
        })
        .collect()
}

// ---------- 1. 길이 코드 ----------

#[test]
fn length_ticks_match_req_table() {
    let table = [
        (LengthCode::Whole, 3840),
        (LengthCode::Half, 1920),
        (LengthCode::Quarter, 960),
        (LengthCode::Eighth, 480),
        (LengthCode::Sixteenth, 240),
        (LengthCode::ThirtySecond, 120),
        (LengthCode::SixtyFourth, 60),
    ];
    for (code, ticks) in table {
        assert_eq!(length_ticks(code, false), ticks, "{code:?}");
        assert_eq!(length_ticks(code, true), ticks * 3 / 2, "{code:?} 점음표");
    }
    assert_eq!(DEFAULT_LENGTH, 960);
}

// ---------- 2·3. 분할 ----------

#[test]
fn split_cuts_notes_crossing_cursor_only() {
    let (p, t, c) = project(&[(0, BAR, &[(0, 960, 60), (480, 960, 64), (960, 960, 67)])]);
    // 절대 틱 960: (0,960)은 끝, (960,960)은 시작이라 그대로. (480,960)만 가로지른다.
    let targets = notes(&[(0, 960, 60), (480, 960, 64), (960, 960, 67)]);
    let changed = plan_split(&p, t, c[0], &targets, Tick::new(960))
        .unwrap()
        .unwrap();
    assert!(changed.moved.is_empty());
    assert_eq!(
        applied(&p, changed)[0].2,
        vec![(0, 960, 60), (480, 480, 64), (960, 480, 64), (960, 960, 67)]
    );
    // 가로지르는 노트가 없으면 None
    assert_eq!(
        plan_split(&p, t, c[0], &notes(&[(0, 960, 60)]), Tick::new(960)).unwrap(),
        None
    );
    assert_eq!(
        plan_split(&p, t, c[0], &notes(&[(0, 960, 60)]), Tick::new(0)).unwrap(),
        None
    );
}

#[test]
fn split_keeps_lyric_on_first_half_only() {
    let mut p = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let t = track.id();
    let mut clip = Clip::new(Tick::new(BAR), BAR).unwrap();
    let sung = note(0, 960, 60).with_lyric("라");
    clip.insert_note(sung.clone()).unwrap();
    let id = track.insert_clip(clip).unwrap();
    p.push_track(track).unwrap();
    let changed = plan_split(&p, t, id, &[sung], Tick::new(BAR + 240))
        .unwrap()
        .unwrap();
    let mut doc = Document::new(p, usize::MAX).unwrap();
    doc.apply(changed.edits, None).unwrap();
    let clip = doc.project().tracks()[0].clip(id).unwrap();
    let lyrics: Vec<(Triple, Option<&str>)> = clip
        .notes()
        .iter()
        .map(|n| (triple(n), n.lyric()))
        .collect();
    assert_eq!(
        lyrics,
        vec![((0, 240, 60), Some("라")), ((240, 720, 60), None)]
    );
}

#[test]
fn split_keeps_hidden_tail_as_tail() {
    // 클립 [0, 2마디)에 2마디 노트를 두고 1마디에서 클립을 자르면 왼쪽 클립에 꼬리 노트(0, 7680)가 남는다.
    let (mut p, t, c) = project(&[(0, 2 * BAR, &[(0, 2 * BAR as u32, 60)])]);
    let mut track = p.tracks()[0].clone();
    track.split_clip(c[0], Tick::new(BAR)).unwrap();
    p = {
        let mut q = Project::new();
        q.push_track(track).unwrap();
        q
    };
    let tail = note(0, 2 * BAR as u32, 60);
    let changed = plan_split(&p, t, c[0], &[tail], Tick::new(960))
        .unwrap()
        .unwrap();
    let clips = applied(&p, changed);
    assert_eq!(clips[0].1, BAR, "클립은 늘지 않는다");
    assert_eq!(
        clips[0].2,
        vec![(0, 960, 60), (960, 2 * BAR as u32 - 960, 60)]
    );
}

// ---------- 4·5. 합치기 ----------

#[test]
fn join_merges_same_pitch_groups() {
    let (p, t, c) = project(&[(
        0,
        2 * BAR,
        &[
            (0, 480, 60),
            (960, 240, 60),
            (1440, 960, 60),
            (0, 480, 64),
            (1920, 480, 67),
        ],
    )]);
    let targets = notes(&[
        (0, 480, 60),
        (960, 240, 60),
        (1440, 960, 60),
        (0, 480, 64),
        (1920, 480, 67),
    ]);
    let changed = plan_join(&p, t, c[0], &targets).unwrap().unwrap();
    assert_eq!(
        applied(&p, changed)[0].2,
        vec![(0, 2400, 60), (0, 480, 64), (1920, 480, 67)]
    );
    // 같은 음높이가 둘 이상인 묶음이 없으면 None
    assert_eq!(
        plan_join(&p, t, c[0], &notes(&[(0, 480, 64), (1920, 480, 67)])).unwrap(),
        None
    );
    assert_eq!(
        plan_join(&p, t, c[0], &notes(&[(0, 480, 60)])).unwrap(),
        None
    );
}

#[test]
fn join_takes_values_from_earliest_and_latest_end() {
    // 늦게 시작한 노트가 먼저 끝나도 끝은 가장 늦은 끝이다. 값은 가장 이른 노트의 것.
    let mut p = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let t = track.id();
    let mut clip = Clip::new(Tick::ZERO, BAR).unwrap();
    let first = Note::new(
        Tick::new(0),
        1920,
        Pitch::new(60).unwrap(),
        Velocity::from_midi1(30).unwrap(),
    )
    .unwrap()
    .with_lyric("가");
    let second = note(480, 240, 60);
    clip.insert_note(first.clone()).unwrap();
    clip.insert_note(second.clone()).unwrap();
    let id = track.insert_clip(clip).unwrap();
    p.push_track(track).unwrap();
    let changed = plan_join(&p, t, id, &[first.clone(), second])
        .unwrap()
        .unwrap();
    let mut doc = Document::new(p, usize::MAX).unwrap();
    doc.apply(changed.edits, None).unwrap();
    let after = doc.project().tracks()[0].clip(id).unwrap().notes().to_vec();
    assert_eq!(after, vec![first]);
}

#[test]
fn join_with_tail_keeps_tail() {
    let (mut p, t, c) = project(&[(0, 3 * BAR, &[(0, 480, 60), (960, 2 * BAR as u32, 60)])]);
    let mut track = p.tracks()[0].clone();
    track.split_clip(c[0], Tick::new(BAR + 960)).unwrap();
    p = {
        let mut q = Project::new();
        q.push_track(track).unwrap();
        q
    };
    let targets = notes(&[(0, 480, 60), (960, 2 * BAR as u32, 60)]);
    let changed = plan_join(&p, t, c[0], &targets).unwrap().unwrap();
    let clips = applied(&p, changed);
    assert_eq!(clips[0].1, BAR + 960, "클립은 늘지 않는다");
    assert_eq!(clips[0].2, vec![(0, 960 + 2 * BAR as u32, 60)]);
}

// ---------- 6. 길이 ----------

#[test]
fn set_length_extends_clip_by_bar_and_skips_equal() {
    let (p, t, c) = project(&[
        (0, BAR, &[(0, 960, 60), (2880, 960, 62)]),
        (3 * BAR, BAR, &[]),
    ]);
    // 2분음표: (0)은 1920, (2880)은 끝 4800 > 3840 → 클립이 2마디로
    let changed = plan_set_length(&p, t, c[0], &notes(&[(0, 960, 60), (2880, 960, 62)]), 1920)
        .unwrap()
        .unwrap();
    assert_eq!(
        changed
            .moved
            .iter()
            .map(|(a, b)| (triple(a), triple(b)))
            .collect::<Vec<_>>(),
        vec![
            ((0, 960, 60), (0, 1920, 60)),
            ((2880, 960, 62), (2880, 1920, 62))
        ]
    );
    let clips = applied(&p, changed);
    assert_eq!(clips[0].1, 2 * BAR);
    assert_eq!(clips[0].2, vec![(0, 1920, 60), (2880, 1920, 62)]);
    // 전부 같은 길이면 None
    assert_eq!(
        plan_set_length(&p, t, c[0], &notes(&[(0, 960, 60), (2880, 960, 62)]), 960).unwrap(),
        None
    );
    // 다음 클립과 겹치게 늘려야 하면 거부(온음표 셋 = 3마디 → 끝이 3마디를 넘음)
    assert!(plan_set_length(&p, t, c[0], &notes(&[(2880, 960, 62)]), 3 * 3840).is_err());
}

// ---------- 음높이 ----------

#[test]
fn set_pitch_changes_only_pitch() {
    let (p, t, c) = project(&[(0, BAR, &[(0, 960, 60)])]);
    let changed = plan_set_pitch(&p, t, c[0], &note(0, 960, 60), Pitch::new(67).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        changed
            .moved
            .iter()
            .map(|(a, b)| (triple(a), triple(b)))
            .collect::<Vec<_>>(),
        vec![((0, 960, 60), (0, 960, 67))]
    );
    assert_eq!(applied(&p, changed)[0].2, vec![(0, 960, 67)]);
    assert_eq!(
        plan_set_pitch(&p, t, c[0], &note(0, 960, 60), Pitch::new(60).unwrap()).unwrap(),
        None
    );
}

// ---------- 7. 레가토 ----------

#[test]
fn legato_sets_end_to_next_start_any_pitch() {
    // 화음 (0: 60, 64), 그다음 960(67, 선택 밖), 1440(60, 끝이 3000까지 겹침), 마지막 2400(62)
    let all: &[Triple] = &[
        (0, 240, 60),
        (0, 480, 64),
        (960, 240, 67),
        (1440, 1560, 60),
        (2400, 240, 62),
    ];
    let (p, t, c) = project(&[(0, BAR, all)]);
    let targets = notes(&[
        (0, 240, 60),
        (0, 480, 64),
        (1440, 1560, 60),
        (2400, 240, 62),
    ]);
    let changed = plan_legato(&p, t, c[0], &targets).unwrap().unwrap();
    let pairs = changed.moved.clone();
    assert_eq!(
        applied(&p, changed)[0].2,
        vec![
            (0, 960, 60),
            (0, 960, 64),
            (960, 240, 67),
            (1440, 960, 60),
            (2400, 240, 62),
        ],
        "화음은 같은 다음 시작, 겹친 노트는 줄고, 마지막 노트는 그대로"
    );
    // 되돌리기
    let mut doc = Document::new(p.clone(), usize::MAX).unwrap();
    doc.apply(
        plan_legato(&p, t, c[0], &targets).unwrap().unwrap().edits,
        None,
    )
    .unwrap();
    let restore = plan_restore(doc.project(), t, c[0], &pairs)
        .unwrap()
        .unwrap();
    doc.apply(restore.edits, None).unwrap();
    assert_eq!(
        doc.project().tracks()[0]
            .clip(c[0])
            .unwrap()
            .notes()
            .iter()
            .map(triple)
            .collect::<Vec<_>>(),
        all.to_vec()
    );
    // 선택 밖 노트도 늘인다: (960, 67)의 다음 시작은 1440
    let one = plan_legato(&p, t, c[0], &notes(&[(960, 240, 67)]))
        .unwrap()
        .unwrap();
    assert_eq!(triple(&one.moved[0].1), (960, 480, 67));
}

#[test]
fn legato_none_when_already_connected_or_last() {
    let (p, t, c) = project(&[(0, BAR, &[(0, 960, 60), (960, 960, 62)])]);
    assert_eq!(
        plan_legato(&p, t, c[0], &notes(&[(0, 960, 60), (960, 960, 62)])).unwrap(),
        None
    );
    assert_eq!(plan_restore(&p, t, c[0], &[]).unwrap(), None);
}

// ---------- 퀀타이즈 ----------

#[test]
fn quantize_uses_given_settings_and_none_when_on_grid() {
    let (p, t, c) = project(&[(0, BAR, &[(250, 100, 60), (480, 100, 62)])]);
    let sixteenth = QuantizeSettings::default().to_core();
    let changed = plan_quantize(&p, t, c[0], &notes(&[(250, 100, 60)]), &sixteenth)
        .unwrap()
        .unwrap();
    assert_eq!(
        changed
            .moved
            .iter()
            .map(|(a, b)| (triple(a), triple(b)))
            .collect::<Vec<_>>(),
        vec![((250, 100, 60), (240, 100, 60))]
    );
    assert_eq!(
        plan_quantize(&p, t, c[0], &notes(&[(480, 100, 62)]), &sixteenth).unwrap(),
        None
    );
    let quarter = QuantizeSettings {
        grid: Grid::Quarter,
        ..QuantizeSettings::default()
    }
    .to_core();
    let changed = plan_quantize(&p, t, c[0], &notes(&[(480, 100, 62)]), &quarter)
        .unwrap()
        .unwrap();
    // 480은 0과 960의 가운데: 앞쪽(0)을 고른다(S0-T7)
    assert_eq!(triple(&changed.moved[0].1), (0, 100, 62));
}

// ---------- 선택 ----------

#[test]
fn selection_after_keeps_untouched_targets_and_adds_new_notes() {
    let before = {
        let mut c = Clip::new(Tick::ZERO, BAR).unwrap();
        for n in notes(&[(0, 960, 60), (960, 960, 62), (1920, 960, 64)]) {
            c.insert_note(n).unwrap();
        }
        c
    };
    let mut after2 = Clip::new(Tick::ZERO, BAR).unwrap();
    for n in notes(&[
        (0, 480, 60),
        (480, 480, 60),
        (960, 960, 62),
        (1920, 960, 64),
    ]) {
        after2.insert_note(n).unwrap();
    }
    let got = selection_after(&before, &after2, &notes(&[(0, 960, 60), (960, 960, 62)]));
    let got: Vec<Triple> = got.iter().map(triple).collect();
    assert_eq!(got, vec![(0, 480, 60), (480, 480, 60), (960, 960, 62)]);
}

// ---------- 8. 음 키 ----------

#[test]
fn semitone_of_key_follows_p3_layouts() {
    let piano = [
        ("a", 0),
        ("w", 1),
        ("s", 2),
        ("e", 3),
        ("d", 4),
        ("f", 5),
        ("t", 6),
        ("g", 7),
        ("y", 8),
        ("h", 9),
        ("u", 10),
        ("j", 11),
        ("k", 12),
        ("o", 13),
        ("l", 14),
        ("p", 15),
    ];
    for (key, semi) in piano {
        assert_eq!(semitone_of_key(KeyLayout::Piano, key), Some(semi), "{key}");
    }
    let solfege = [
        ("c", 0),
        ("d", 2),
        ("e", 4),
        ("f", 5),
        ("g", 7),
        ("a", 9),
        ("b", 11),
    ];
    for (key, semi) in solfege {
        assert_eq!(
            semitone_of_key(KeyLayout::Solfege, key),
            Some(semi),
            "{key}"
        );
    }
    for key in ["z", "x", "q", "r", "1", "escape", "space", "b"] {
        assert_eq!(semitone_of_key(KeyLayout::Piano, key), None, "{key}");
    }
    for key in ["h", "s", "#", "+", "1", "escape"] {
        assert_eq!(semitone_of_key(KeyLayout::Solfege, key), None, "{key}");
    }
    // 대문자 글자도 같은 음(입력 글자가 대문자로 올 수 있다)
    assert_eq!(semitone_of_key(KeyLayout::Solfege, "G"), Some(7));
}

#[test]
fn pitch_in_octave_of_uses_cursor_octave() {
    let p = |v| Pitch::new(v).unwrap();
    assert_eq!(pitch_in_octave_of(p(64), 0), Some(p(60)));
    assert_eq!(pitch_in_octave_of(p(71), 14), Some(p(74)));
    assert_eq!(pitch_in_octave_of(p(60), 11), Some(p(71)));
    assert_eq!(pitch_in_octave_of(p(120), 9), None, "120 + 9 > 127");
    assert_eq!(pitch_in_octave_of(p(5), 2), Some(p(2)));
}

// ---------- 9. 설정 ----------

#[test]
fn quantize_settings_read_write_and_defaults() {
    let dir = std::env::temp_dir().join(format!("ks-quantize-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    // 파일이 없으면 기본값
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty());
    assert_eq!(s.quantize, QuantizeSettings::default());
    assert_eq!(
        s.quantize,
        QuantizeSettings {
            grid: Grid::Sixteenth,
            triplet: false,
            strength: 100,
            swing: 0
        }
    );
    // 필드마다 읽고, 잘못된 필드만 기본값과 경고, 모르는 키 보존
    std::fs::write(
        &path,
        r#"{"quantize": {"grid": "1/8", "triplet": true, "strength": 150, "swing": 50, "future": 1}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(
        s.quantize,
        QuantizeSettings {
            grid: Grid::Eighth,
            triplet: true,
            strength: 100,
            swing: 50
        }
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("quantize.strength"), "{warnings:?}");
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["quantize"]["grid"], "1/8");
    assert_eq!(v["quantize"]["triplet"], true);
    assert_eq!(v["quantize"]["strength"], 100);
    assert_eq!(v["quantize"]["swing"], 50);
    assert_eq!(v["quantize"]["future"], 1);
    // 그리드 다섯 가지 왕복, 잘못된 그리드·스윙
    for (text, grid) in [
        ("1/4", Grid::Quarter),
        ("1/8", Grid::Eighth),
        ("1/16", Grid::Sixteenth),
        ("1/32", Grid::ThirtySecond),
        ("1/64", Grid::SixtyFourth),
    ] {
        std::fs::write(&path, format!(r#"{{"quantize": {{"grid": "{text}"}}}}"#)).unwrap();
        let (s, w) = Settings::load(&path);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(s.quantize.grid, grid);
    }
    std::fs::write(
        &path,
        r#"{"quantize": {"grid": "1/3", "swing": 80, "triplet": 1}}"#,
    )
    .unwrap();
    let (s, w) = Settings::load(&path);
    assert_eq!(s.quantize, QuantizeSettings::default());
    assert_eq!(w.len(), 3, "{w:?}");
    std::fs::write(&path, r#"{"quantize": 3}"#).unwrap();
    let (s, w) = Settings::load(&path);
    assert_eq!(s.quantize, QuantizeSettings::default());
    assert_eq!(w.len(), 1, "{w:?}");
    // 범위 밖 공개 필드는 to_core에서 줄인다
    let wild = QuantizeSettings {
        grid: Grid::Quarter,
        triplet: true,
        strength: 200,
        swing: 200,
    }
    .to_core();
    assert_eq!(wild.grid_ticks(), 640);
}
