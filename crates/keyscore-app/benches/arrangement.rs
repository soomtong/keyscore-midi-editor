//! 배치 뷰 장면 계산 벤치(S4-T1 게이트: 트랙 64 × 클립 200 화면). GPUI `paint_quad` 비용은 SP-2가 쟀다.
//! 기준값은 `benches/baseline.json`, 비교는 `scripts/bench-check.py`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use keyscore_app::arrangement::scene::{self, Scene, SceneInput};
use keyscore_app::arrangement::{ClipSelection, Viewport};
use keyscore_core::{
    Channel, Clip, Note, Pitch, Project, Tick, TimeSignature, Track, TrackColor, Velocity,
};

const PPQ: u64 = 960;
const BAR: u64 = PPQ * 4;

/// 트랙 64개, 트랙마다 한 마디짜리 클립 200개가 잇달아. 클립마다 노트 넷(첫 마디 가사 셋).
fn project_64x200() -> Project {
    let mut project = Project::new();
    for t in 0..64u8 {
        let mut track = Track::new(
            format!("T{t}"),
            Channel::new(1 + t % 16).unwrap(),
            TrackColor::new(t % 12).unwrap(),
        );
        for i in 0..200u64 {
            let mut clip = Clip::new(Tick::new(i * BAR), BAR).unwrap();
            for k in 0..4u64 {
                let note = Note::new(
                    Tick::new(k * PPQ),
                    480,
                    Pitch::new(60 + u8::try_from(k).unwrap()).unwrap(),
                    Velocity::from_midi1(100).unwrap(),
                )
                .unwrap();
                let note = if k < 3 { note.with_lyric("라") } else { note };
                clip.insert_note(note).unwrap();
            }
            if i % 8 == 0 {
                clip.set_name("절");
            }
            track.insert_clip(clip).unwrap();
        }
        project.push_track(track).unwrap();
    }
    assert_eq!(
        project.meter_map().signature_at(Tick::ZERO),
        TimeSignature::new(4, 4).unwrap()
    );
    project
}

fn bench(c: &mut Criterion) {
    let project = project_64x200();
    let current = project.tracks()[3].id();
    let selection = ClipSelection::default();
    // 전체가 들어가는 배율(1600 px), 모든 행(64 × 16 px)
    let viewport = Viewport {
        first_tick: 0.0,
        px_per_tick: 1600.0 / (200.0 * 4.0 * 960.0),
        width: 1600.0,
        first_row: 0.0,
        row_height: 16.0,
        height: 1024.0,
    };
    let input = SceneInput {
        project: &project,
        current_track: Some(current),
        cursor: Tick::new(960),
        step_ticks: 960,
        selection: &selection,
        ghost: None,
        playhead: Some(Tick::new(5000)),
        viewport,
    };
    let mut scene = Scene::default();
    scene::build(&input, &mut scene);
    assert_eq!(scene.clips.len(), 64 * 200);
    c.bench_function("arrangement/scene_64x200", |b| {
        b.iter(|| {
            scene::build(black_box(&input), &mut scene);
            black_box(scene.clips.len())
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
