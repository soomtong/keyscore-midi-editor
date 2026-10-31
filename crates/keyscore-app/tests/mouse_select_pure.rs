//! S3-T6a 마우스 선택의 순수 계산(REQ 7.4, 결정 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 3절).

use std::collections::BTreeSet;

use keyscore_app::pianoroll::mouse::{
    Band, band, band_clip, band_notes, first_cell, past_threshold, snap_floor, time_range_notes,
};
use keyscore_app::pianoroll::{Cursor, Viewport};
use keyscore_core::{
    Channel, Clip, MeterMap, Note, Pitch, Tick, TimeSignature, Track, TrackColor, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn pitch(value: u8) -> Pitch {
    Pitch::new(value).unwrap()
}

fn clip(start: u64, length: u64, notes: &[Note]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    clip
}

/// 틱 0부터 0.1 px/틱(4분 = 96 px), 맨 위 행 40(음높이 87), 행 12 px, 800×480.
fn viewport() -> Viewport {
    Viewport {
        first_tick: 0.0,
        px_per_tick: 0.1,
        width: 800.0,
        first_row: 40.0,
        row_height: 12.0,
        height: 480.0,
    }
}

fn set(notes: &[Note]) -> BTreeSet<Note> {
    notes.iter().cloned().collect()
}

#[test]
fn band_covers_ticks_floor_ceil_and_rows_inclusive() {
    let vp = viewport();
    // x 10.05 → 틱 100.5(내림 100), x 50.01 → 500.1(올림 501). y 13 → 행 41(음 86), y 37 → 행 43(음 84)
    let b = band(&vp, (10.05, 13.0), (50.01, 37.0)).unwrap();
    assert_eq!(
        b,
        Band {
            lo: Tick::new(100),
            hi: Tick::new(501),
            low: pitch(84),
            high: pitch(86),
        }
    );
    assert_eq!(
        band(&vp, (50.01, 37.0), (10.05, 13.0)),
        Some(b),
        "점 순서 무관"
    );
    // 뷰 밖 점은 뷰포트 안으로 잘린다: 왼쪽 위 밖 → 틱 0, 행 40(음 87)
    let clipped = band(&vp, (-30.0, -30.0), (10.0, 0.5)).unwrap();
    assert_eq!(clipped.lo, Tick::new(0));
    assert_eq!(clipped.high, pitch(87));
    assert_eq!(clipped.low, pitch(87));
    // 오른쪽 아래 밖 → 오른쪽 끝 틱 8000, 맨 아래 행 79(음 48)
    let far = band(&vp, (0.0, 0.0), (5000.0, 5000.0)).unwrap();
    assert_eq!(far.hi, Tick::new(8000));
    assert_eq!(far.low, pitch(48));
    let empty = Viewport {
        width: 0.0,
        ..viewport()
    };
    assert_eq!(band(&empty, (1.0, 1.0), (2.0, 2.0)), None);
}

#[test]
fn band_notes_use_overlap_rule() {
    let ends_at_lo = note(0, 960, 60);
    let starts_at_hi = note(1920, 480, 60);
    let long = note(0, 3000, 62);
    let inside = note(1000, 100, 61);
    let other_pitch = note(1000, 100, 70);
    let c = clip(
        BAR,
        BAR,
        &[
            ends_at_lo.clone(),
            starts_at_hi.clone(),
            long.clone(),
            inside.clone(),
            other_pitch,
        ],
    );
    // 절대 [BAR+960, BAR+1920), 음 60–65
    let b = Band {
        lo: Tick::new(BAR + 960),
        hi: Tick::new(BAR + 1920),
        low: pitch(60),
        high: pitch(65),
    };
    assert_eq!(band_notes(&c, b), set(&[long, inside]));
    // 클립 앞에서 끝나는 범위는 비어 있다
    let before = Band {
        lo: Tick::new(0),
        hi: Tick::new(BAR),
        ..b
    };
    assert!(band_notes(&c, before).is_empty());
}

fn two_clip_track() -> Track {
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    // A: 0–1마디, B: 2–3마디(사이 1마디 틈)
    track
        .insert_clip(clip(0, BAR, &[note(0, 960, 60)]))
        .unwrap();
    track
        .insert_clip(clip(2 * BAR, BAR, &[note(0, 960, 60)]))
        .unwrap();
    track
}

#[test]
fn band_clip_prefers_press_then_now_then_first_overlap() {
    let track = two_clip_track();
    let ids: Vec<_> = track.clips().map(|(id, _)| id).collect();
    let (a, b) = (ids[0], ids[1]);
    let whole = Band {
        lo: Tick::new(0),
        hi: Tick::new(3 * BAR),
        low: pitch(0),
        high: pitch(127),
    };
    // 누른 점이 B면 B
    assert_eq!(
        band_clip(&track, Tick::new(2 * BAR + 10), Tick::new(10), whole, None),
        Some(b)
    );
    // 누른 점이 틈이면 지금 점의 클립
    assert_eq!(
        band_clip(
            &track,
            Tick::new(BAR + 10),
            Tick::new(2 * BAR + 10),
            whole,
            None
        ),
        Some(b)
    );
    assert_eq!(
        band_clip(&track, Tick::new(BAR + 10), Tick::new(10), whole, None),
        Some(a)
    );
    // 둘 다 틈이면 시간 범위에 걸치는 첫 클립
    let gap_band = Band {
        lo: Tick::new(BAR - 100),
        hi: Tick::new(2 * BAR + 100),
        ..whole
    };
    assert_eq!(
        band_clip(
            &track,
            Tick::new(BAR + 10),
            Tick::new(BAR + 20),
            gap_band,
            None
        ),
        Some(a)
    );
    // `preferred`가 범위에 걸치면 먼저
    assert_eq!(
        band_clip(&track, Tick::new(10), Tick::new(10), whole, Some(b)),
        Some(b)
    );
    // `preferred`가 범위에 걸치지 않으면 무시
    let only_a = Band {
        lo: Tick::new(0),
        hi: Tick::new(100),
        ..whole
    };
    assert_eq!(
        band_clip(&track, Tick::new(10), Tick::new(10), only_a, Some(b)),
        Some(a)
    );
    // 틈만 덮으면 없음
    let in_gap = Band {
        lo: Tick::new(BAR + 1),
        hi: Tick::new(BAR + 100),
        ..whole
    };
    assert_eq!(
        band_clip(
            &track,
            Tick::new(BAR + 10),
            Tick::new(BAR + 50),
            in_gap,
            None
        ),
        None
    );
}

#[test]
fn time_range_notes_ignore_pitch() {
    let low = note(0, 480, 30);
    let high = note(480, 480, 100);
    let late = note(1920, 480, 60);
    let c = clip(BAR, BAR, &[low.clone(), high.clone(), late]);
    assert_eq!(
        time_range_notes(&c, Tick::new(BAR + 100), Tick::new(BAR + 481)),
        set(&[low, high.clone()])
    );
    // 한 틱 범위
    assert_eq!(
        time_range_notes(&c, Tick::new(BAR + 500), Tick::new(BAR + 501)),
        set(&[high])
    );
}

#[test]
fn snap_floor_uses_bar_start_grid() {
    let meter = MeterMap::new(TimeSignature::COMMON);
    assert_eq!(snap_floor(&meter, Tick::new(700), 480), Tick::new(480));
    assert_eq!(snap_floor(&meter, Tick::new(480), 480), Tick::new(480));
    assert_eq!(snap_floor(&meter, Tick::new(4000), 960), Tick::new(3840));
    // 셋째 박 중간(2400)에서 3/4로 바뀌면 그 뒤 마디는 2400부터 센다
    let mut changed = MeterMap::new(TimeSignature::COMMON);
    changed.set(Tick::new(2400), TimeSignature::new(3, 4).unwrap());
    assert_eq!(snap_floor(&changed, Tick::new(3500), 960), Tick::new(3360));
    assert_eq!(snap_floor(&meter, Tick::new(777), 0), Tick::new(777));
}

#[test]
fn drag_threshold_is_three_px() {
    assert!(!past_threshold((10.0, 10.0), (12.9, 10.0)));
    assert!(
        !past_threshold((10.0, 10.0), (12.0, 12.0)),
        "대각선 2.83 px"
    );
    assert!(past_threshold((10.0, 10.0), (13.0, 10.0)));
    assert!(past_threshold((10.0, 10.0), (10.0, 7.0)));
}

#[test]
fn first_cell_is_earliest_then_highest() {
    let a = note(480, 480, 60);
    let b = note(480, 480, 67);
    let c = note(960, 480, 72);
    let cl = clip(BAR, BAR, &[a.clone(), b.clone(), c.clone()]);
    assert_eq!(
        first_cell(&cl, &set(&[a, b, c])),
        Some(Cursor {
            tick: Tick::new(BAR + 480),
            pitch: pitch(67),
        })
    );
    assert_eq!(first_cell(&cl, &BTreeSet::new()), None);
}
