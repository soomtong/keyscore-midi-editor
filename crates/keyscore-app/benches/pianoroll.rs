//! 피아노롤 장면 계산 벤치(S3-T1 게이트: 노트 10,000개 화면). GPUI `paint_quad` 비용은 SP-2가 쟀다.
//! 기준값은 `benches/baseline.json`, 비교는 `scripts/bench-check.py`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use keyscore_app::pianoroll::scene::{self, Scene, SceneInput};
use keyscore_app::pianoroll::{Cursor, MaxLenCache, Selection, Viewport};
use keyscore_core::{
    Channel, Clip, Key, MeterMap, Note, Pitch, PitchClass, Tick, Track, TrackColor, Velocity,
};

const PPQ: u64 = 960;

/// SP-2와 같은 배치: 32마디(4/4) 안에 10,000개를 고르게, 길이는 16분음표 1–4개.
fn track_10k() -> Track {
    let bars = 32u64;
    let span = bars * PPQ * 4;
    let count = 10_000u64;
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let mut clip = Clip::new(Tick::ZERO, span + PPQ * 4).unwrap();
    for i in 0..count {
        let start = (i * span / count) / (PPQ / 4) * (PPQ / 4);
        let len = u32::try_from(PPQ / 4 * (1 + next() % 4)).unwrap();
        let pitch = u8::try_from(24 + next() % 72).unwrap();
        let velocity = u8::try_from(30 + next() % 97).unwrap();
        clip.insert_note(
            Note::new(
                Tick::new(start),
                len,
                Pitch::new(pitch).unwrap(),
                Velocity::from_midi1(velocity).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
    let mut track = Track::new(
        "bench",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    track.insert_clip(clip).unwrap();
    track
}

fn bench(c: &mut Criterion) {
    let track = track_10k();
    let clip = track.clips().next().unwrap().0;
    let mut cache = MaxLenCache::default();
    cache.refresh(Some(&track), 0);
    let meter = MeterMap::default();
    let selection = Selection::default();
    // 전체가 들어가는 배율(1600 px), 모든 행(128 × 8 px)
    let viewport = Viewport {
        first_tick: 0.0,
        px_per_tick: 1600.0 / (32.0 * 4.0 * 960.0),
        width: 1600.0,
        first_row: 0.0,
        row_height: 8.0,
        height: 1024.0,
    };
    let input = SceneInput {
        track: Some(&track),
        current: Some(clip),
        cache: &cache,
        meter: &meter,
        key: Key::Major(PitchClass::new(0).unwrap()),
        cursor: Cursor {
            tick: Tick::new(960),
            pitch: Pitch::new(60).unwrap(),
        },
        step_ticks: 960,
        selection: &selection,
        playhead: Some(Tick::new(5000)),
        viewport,
    };
    let mut scene = Scene::default();
    scene::build(&input, &mut scene);
    assert_eq!(scene.notes.len(), 10_000);
    c.bench_function("pianoroll/scene_10k_notes", |b| {
        b.iter(|| {
            scene::build(black_box(&input), &mut scene);
            black_box(scene.notes.len())
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
