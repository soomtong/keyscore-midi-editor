//! 템포 맵 변환 벤치. 스케줄러(S1)가 이벤트마다 부르는 경로다.
//! 기준값은 `benches/baseline.json`, 비교는 `scripts/bench-check.py`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use keyscore_core::{MusicTime, Tempo, TempoMap, Tick};

/// 템포 변화 100개(4마디마다)인 맵. v1 UI는 하나만 쓰지만 SMF 가져오기로 많아질 수 있다.
fn map_with_changes() -> TempoMap {
    let mut map = TempoMap::default();
    for i in 1..=100u32 {
        map.set(
            Tick::new(u64::from(i) * 15_360),
            Tempo::from_bpm(80 + i).unwrap(),
        );
    }
    map
}

fn bench(c: &mut Criterion) {
    let map = map_with_changes();
    let ticks: Vec<Tick> = (0..1_000u64).map(|i| Tick::new(i * 1_637)).collect();
    let times: Vec<MusicTime> = ticks.iter().map(|&t| map.tick_to_time(t)).collect();

    c.bench_function("tempo_map/tick_to_time_1000", |b| {
        b.iter(|| {
            for &t in &ticks {
                black_box(map.tick_to_time(black_box(t)));
            }
        });
    });
    c.bench_function("tempo_map/time_to_tick_1000", |b| {
        b.iter(|| {
            for &t in &times {
                black_box(map.time_to_tick(black_box(t)));
            }
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
