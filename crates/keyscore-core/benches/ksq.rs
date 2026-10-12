//! `.ksq` 읽기 벤치. 게이트: 노트 10만 개 파일 읽기 < 100 ms(S0-T6).
//! 기준값은 `benches/baseline.json`, 비교는 `scripts/bench-check.py`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity, ksq};

/// 트랙 8개, 클립 100개씩(4마디), 클립마다 노트 125개 = 노트 10만 개. 가사는 10%.
fn project_100k() -> Project {
    let mut p = Project::new();
    for t in 0..8u8 {
        let mut track = Track::new(
            format!("T{t}"),
            Channel::new(t + 1).unwrap(),
            TrackColor::new(t).unwrap(),
        );
        for c in 0..100u64 {
            let mut clip = Clip::new(Tick::new(c * 15_360), 15_360).unwrap();
            for n in 0..125u64 {
                let pitch = Pitch::new(u8::try_from(36 + (n * 7 + c) % 60).unwrap()).unwrap();
                let mut note =
                    Note::new(Tick::new(n * 120), 110, pitch, Velocity::new(40_000)).unwrap();
                if n % 10 == 0 {
                    note = note.with_lyric("라");
                }
                clip.insert_note(note).unwrap();
            }
            track.insert_clip(clip).unwrap();
        }
        p.push_track(track).unwrap();
    }
    p
}

fn bench(c: &mut Criterion) {
    let text = ksq::to_string(&project_100k());
    c.bench_function("ksq/read_100k_notes", |b| {
        b.iter(|| black_box(ksq::from_str(black_box(&text)).unwrap()));
    });
    c.bench_function("ksq/write_100k_notes", |b| {
        let project = ksq::from_str(&text).unwrap().0;
        b.iter(|| black_box(ksq::to_string(black_box(&project))));
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
