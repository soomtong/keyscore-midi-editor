//! 템포·박자표 맵의 속성 기반 테스트(S0-T3 게이트).

use keyscore_core::{MeterMap, MusicTime, Tempo, TempoMap, Tick, TimeSignature};
use proptest::prelude::*;

/// 템포 변화가 정확히 3개인 맵(틱 0 포함 4구간). 게이트 「템포 변화 3개」.
fn three_change_map() -> impl Strategy<Value = TempoMap> {
    (
        1..=Tempo::MAX_MICROS,
        prop::collection::btree_map(1..2_000_000u64, 1..=Tempo::MAX_MICROS, 3),
    )
        .prop_map(|(first, rest)| {
            let mut map = TempoMap::new(Tempo::from_micros_per_quarter(first).unwrap());
            for (tick, us) in rest {
                map.set(Tick::new(tick), Tempo::from_micros_per_quarter(us).unwrap());
            }
            map
        })
}

/// 0 근처, 변화 근처, u64 끝 근처를 고루.
fn any_tick() -> impl Strategy<Value = u64> {
    prop_oneof![
        0..3_000_000u64,
        any::<u64>(),
        (u64::MAX - 10_000)..=u64::MAX
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn tempo_map_three_changes_round_trip_is_exact(map in three_change_map(), t in any_tick()) {
        prop_assert_eq!(map.changes().count(), 4);
        let time = map.tick_to_time(Tick::new(t));
        prop_assert_eq!(map.time_to_tick(time), Tick::new(t));
    }

    #[test]
    fn tempo_map_time_to_tick_is_floor(map in three_change_map(), units in any::<u128>(), small in 0..10u128.pow(18)) {
        for x in [units >> 40, small] {
            let x = MusicTime::from_units(x);
            let k = map.time_to_tick(x);
            prop_assert!(map.tick_to_time(k) <= x);
            if k.get() < u64::MAX {
                prop_assert!(x < map.tick_to_time(Tick::new(k.get() + 1)));
            }
        }
    }

    #[test]
    fn tempo_map_tick_to_time_is_strictly_increasing(map in three_change_map(), t in 0..(u64::MAX - 1)) {
        prop_assert!(map.tick_to_time(Tick::new(t)) < map.tick_to_time(Tick::new(t + 1)));
    }

    #[test]
    fn tempo_map_segment_sum_equals_direct_conversion(map in three_change_map(), t in 0..3_000_000u64) {
        // 구간 길이를 공개 API의 템포로 직접 더한다. 캐시와 다르면 누적 계산이 틀린 것이다.
        let changes: Vec<(u64, u32)> = map.changes().map(|(k, v)| (k.get(), v.micros_per_quarter())).collect();
        let mut units: u128 = 0;
        for (i, &(start, us)) in changes.iter().enumerate() {
            if start >= t { break; }
            let end = changes.get(i + 1).map_or(t, |&(next, _)| next.min(t));
            units += u128::from(end - start) * u128::from(us) * 1000;
        }
        prop_assert_eq!(map.tick_to_time(Tick::new(t)).units(), units);
    }

    #[test]
    fn meter_map_bar_start_inverts_position(
        changes in prop::collection::btree_map(1..200_000u64, (1..=16u8, 0..7u32), 0..5),
        bar in 0..200u64,
        t in 0..300_000u64,
    ) {
        let mut map = MeterMap::default();
        for (tick, (n, dpow)) in changes {
            map.set(Tick::new(tick), TimeSignature::new(n, 1 << dpow).unwrap());
        }
        let start = map.bar_start(bar).unwrap();
        let p = map.position(start);
        prop_assert_eq!((p.bar(), p.beat(), p.tick()), (bar, 0, 0));
        let p = map.position(Tick::new(t));
        prop_assert!(map.bar_start(p.bar()).unwrap() <= Tick::new(t));
        prop_assert!(Tick::new(t) < map.bar_start(p.bar() + 1).unwrap());
        let sig = map.signature_at(Tick::new(t));
        prop_assert_eq!(
            map.bar_start(p.bar()).unwrap().get() + u64::from(p.beat()) * sig.beat_ticks() + p.tick(),
            t
        );
    }
}
