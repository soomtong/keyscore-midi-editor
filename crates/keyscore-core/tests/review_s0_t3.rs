//! S0-T3 적대적 리뷰(1회차)의 깨뜨리기 시도. 공개 API만 쓴다.
//!
//! `docs/reports/s0-t3/review-1.md` 3절과 짝을 이룬다. 기준 모델은 구현의 캐시·이진 탐색을 쓰지
//! 않고 `BTreeMap`을 처음부터 걸어 가며 u128로 계산한다. 박자표는 틱을 하나씩 세는 무차별 모델과
//! 비교한다.

use std::collections::BTreeMap;

use keyscore_core::{MeterMap, ModelError, MusicTime, Tempo, TempoMap, Tick, TimeSignature};
use proptest::prelude::*;

const MAX_US: u32 = Tempo::MAX_MICROS;

/// 기준 모델: 틱 → 시각(단위 1/960 ns). 1틱 = us/960 µs = us × 1000 단위.
fn ref_tick_to_units(model: &BTreeMap<u64, u32>, t: u64) -> u128 {
    let mut acc: u128 = 0;
    let entries: Vec<(u64, u32)> = model.iter().map(|(&k, &v)| (k, v)).collect();
    for (i, &(start, us)) in entries.iter().enumerate() {
        if start > t {
            break;
        }
        let end = entries.get(i + 1).map_or(t, |&(next, _)| next.min(t));
        acc += u128::from(end - start) * u128::from(us) * 1000;
    }
    acc
}

/// 기준 모델: 시각 → 틱(내림, u64 끝에서 자름).
fn ref_units_to_tick(model: &BTreeMap<u64, u32>, x: u128) -> u64 {
    let entries: Vec<(u64, u32)> = model.iter().map(|(&k, &v)| (k, v)).collect();
    let mut acc: u128 = 0;
    for (i, &(start, us)) in entries.iter().enumerate() {
        let per = u128::from(us) * 1000;
        if let Some(&(next, _)) = entries.get(i + 1) {
            let len = u128::from(next - start) * per;
            if x < acc + len {
                // next - start 안쪽이라 u64에 들어간다.
                return start + u64::try_from((x - acc) / per).unwrap();
            }
            acc += len;
        } else {
            let span = (x - acc) / per;
            return u64::try_from(span)
                .ok()
                .and_then(|s| start.checked_add(s))
                .unwrap_or(u64::MAX);
        }
    }
    unreachable!("틱 0 항목이 있다")
}

fn tempo(us: u32) -> Tempo {
    Tempo::from_micros_per_quarter(us).unwrap()
}

fn any_us() -> impl Strategy<Value = u32> {
    prop_oneof![Just(1u32), Just(MAX_US), Just(500_000u32), 1..=MAX_US,]
}

fn any_change_tick() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        1..10_000u64,
        any::<u64>(),
        (u64::MAX - 1_000)..=u64::MAX,
    ]
}

/// 조작 하나: 설정 또는 삭제. 순서를 섞어 삽입 순서 의존·캐시 갱신 누락을 노린다.
#[derive(Debug, Clone)]
enum Op {
    Set(u64, u32),
    Remove(u64),
}

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (any_change_tick(), any_us()).prop_map(|(t, us)| Op::Set(t, us)),
        1 => any_change_tick().prop_map(Op::Remove),
    ]
}

fn apply(ops: &[Op], initial: u32) -> (TempoMap, BTreeMap<u64, u32>) {
    let mut map = TempoMap::new(tempo(initial));
    let mut model = BTreeMap::from([(0u64, initial)]);
    for op in ops {
        match *op {
            Op::Set(t, us) => {
                map.set(Tick::new(t), tempo(us));
                model.insert(t, us);
            }
            Op::Remove(t) => {
                let got = map.remove(Tick::new(t));
                if t == 0 {
                    assert_eq!(got, Err(ModelError::RemoveInitialChange));
                } else {
                    assert_eq!(got, Ok(model.remove(&t).is_some()));
                }
            }
        }
    }
    (map, model)
}

/// 검사할 틱: 0, u64 끝, 각 변화 틱과 그 ±1.
fn probe_ticks(model: &BTreeMap<u64, u32>, extra: u64) -> Vec<u64> {
    let mut v = vec![0, 1, u64::MAX, u64::MAX - 1, extra];
    for &k in model.keys() {
        v.push(k);
        v.push(k.saturating_sub(1));
        v.push(k.saturating_add(1));
    }
    v
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// 임의 조작 수열 뒤 맵이 기준 모델과 같고, 변환이 경계마다 기준 모델과 일치한다.
    #[test]
    fn review_tempo_map_matches_reference_after_random_ops(
        initial in any_us(),
        ops in prop::collection::vec(any_op(), 0..12),
        extra in any::<u64>(),
        raw in any::<u128>(),
    ) {
        let (map, model) = apply(&ops, initial);

        // 불변식: 틱 0 항목, 엄격한 정렬(중복 없음), 기준 모델과 같은 내용.
        let got: Vec<(u64, u32)> = map.changes().map(|(k, v)| (k.get(), v.micros_per_quarter())).collect();
        let want: Vec<(u64, u32)> = model.iter().map(|(&k, &v)| (k, v)).collect();
        prop_assert_eq!(&got, &want);
        prop_assert_eq!(got[0].0, 0);
        prop_assert!(got.windows(2).all(|w| w[0].0 < w[1].0));

        // 캐시 일치: 같은 내용을 틱 순서로 새로 만든 맵과 같아야 한다(PartialEq는 캐시까지 비교).
        let mut fresh = TempoMap::new(tempo(want[0].1));
        for &(k, us) in &want[1..] {
            fresh.set(Tick::new(k), tempo(us));
        }
        prop_assert_eq!(&map, &fresh);

        for t in probe_ticks(&model, extra) {
            let units = map.tick_to_time(Tick::new(t)).units();
            prop_assert_eq!(units, ref_tick_to_units(&model, t), "tick_to_time({})", t);
            prop_assert_eq!(map.time_to_tick(MusicTime::from_units(units)).get(), t, "왕복 {}", t);
            prop_assert_eq!(map.tempo_at(Tick::new(t)).micros_per_quarter(),
                *model.range(..=t).next_back().unwrap().1);
        }

        // 시각 경계: 각 구간 시작 시각, 그 −1·+1, 임의 u128.
        let mut times = vec![0u128, 1, raw, raw >> 30, u128::MAX];
        for &k in model.keys() {
            let s = ref_tick_to_units(&model, k);
            times.extend([s, s.saturating_sub(1), s + 1]);
        }
        for x in times {
            prop_assert_eq!(
                map.time_to_tick(MusicTime::from_units(x)).get(),
                ref_units_to_tick(&model, x),
                "time_to_tick({})", x
            );
        }
    }
}

/// 최소·최대 템포가 번갈아 u64 끝까지 이어질 때 오버플로·패닉이 없고 기준 모델과 같다.
#[test]
fn review_tempo_map_extreme_tempos_at_u64_end_match_reference() {
    let mut map = TempoMap::new(tempo(MAX_US));
    let mut model = BTreeMap::from([(0u64, MAX_US)]);
    for (i, k) in [u64::MAX / 2, u64::MAX - 2, u64::MAX - 1, u64::MAX]
        .into_iter()
        .enumerate()
    {
        let us = if i % 2 == 0 { 1 } else { MAX_US };
        map.set(Tick::new(k), tempo(us));
        model.insert(k, us);
    }
    for t in probe_ticks(&model, 12_345) {
        let u = map.tick_to_time(Tick::new(t)).units();
        assert_eq!(u, ref_tick_to_units(&model, t), "{t}");
        assert_eq!(map.time_to_tick(MusicTime::from_units(u)).get(), t);
    }
    // 마지막 틱 이후 시각은 u64::MAX로 자른다.
    let end = map.tick_to_time(Tick::new(u64::MAX)).units();
    assert_eq!(
        map.time_to_tick(MusicTime::from_units(end + 1)).get(),
        u64::MAX
    );
    assert_eq!(
        map.time_to_tick(MusicTime::from_units(u128::MAX)).get(),
        u64::MAX
    );
}

/// 1틱 간격의 연속 변화(최소·최대 번갈아)에서 구간 시작 시각이 엄격히 증가하고 변환이 맞다.
#[test]
fn review_tempo_map_one_tick_apart_changes_match_reference() {
    let mut map = TempoMap::new(tempo(1));
    let mut model = BTreeMap::from([(0u64, 1u32)]);
    // 역순 삽입으로 삽입 위치 계산과 뒤쪽 캐시 갱신을 함께 노린다.
    for k in (1..=64u64).rev() {
        let us = if k % 2 == 0 { 1 } else { MAX_US };
        map.set(Tick::new(k), tempo(us));
        model.insert(k, us);
    }
    for t in 0..=70u64 {
        let u = map.tick_to_time(Tick::new(t)).units();
        assert_eq!(u, ref_tick_to_units(&model, t), "{t}");
        assert_eq!(map.time_to_tick(MusicTime::from_units(u)).get(), t);
        if u > 0 {
            assert_eq!(map.time_to_tick(MusicTime::from_units(u - 1)).get(), t - 1);
        }
    }
}

/// `from_bpm`의 반올림을 독립 계산과 비교하고, 경계(4 BPM은 들어가고 3 BPM은 안 됨, 매우 큰 BPM)에서 패닉이 없다.
#[test]
fn review_tempo_from_bpm_rounding_and_bounds() {
    for bpm in [
        1u32,
        2,
        3,
        4,
        7,
        90,
        120,
        140,
        999,
        7_746,
        120_000_000,
        120_000_001,
        u32::MAX,
    ] {
        // 60,000,000 / bpm을 반올림(0.5는 올림)한 값.
        let q = 60_000_000u64 / u64::from(bpm);
        let r = 60_000_000u64 % u64::from(bpm);
        let us = if 2 * r >= u64::from(bpm) { q + 1 } else { q };
        let got = Tempo::from_bpm(bpm);
        if (1..=u64::from(MAX_US)).contains(&us) {
            assert_eq!(
                got.map(Tempo::micros_per_quarter),
                Ok(u32::try_from(us).unwrap()),
                "{bpm}"
            );
        } else {
            assert!(got.is_err(), "{bpm}");
        }
    }
    // 정수 BPM 중 60,000,000을 나누지 않는 흔한 값은 정확히 표현되지 않는다(리뷰 지적 #1의 근거).
    let inexact: Vec<u32> = (40..=240).filter(|b| 60_000_000 % b != 0).collect();
    assert!(inexact.contains(&90) && inexact.contains(&140));
}

/// 박자표 무차별 기준 모델: 틱을 0부터 하나씩 세며 마디·박·박 안 틱을 계산한다.
/// 변화 틱에서 새 마디를 시작한다(구현 문서의 「짧은 마디」 규칙).
fn brute_positions(model: &BTreeMap<u64, (u8, u8)>, upto: u64) -> Vec<(u64, u32, u64, u64)> {
    let mut out = Vec::new();
    let (mut bar, mut in_bar) = (0u64, 0u64);
    let mut sig = model[&0];
    for t in 0..=upto {
        if t > 0 {
            in_bar += 1;
            let bar_ticks = u64::from(sig.0) * (3840 / u64::from(sig.1));
            if let Some(&s) = model.get(&t) {
                sig = s;
                bar += 1;
                in_bar = 0;
            } else if in_bar == bar_ticks {
                bar += 1;
                in_bar = 0;
            }
        }
        let beat_ticks = 3840 / u64::from(sig.1);
        let beat = u32::try_from(in_bar / beat_ticks).unwrap();
        // (마디, 박, 박 안 틱, 이 틱이 마디 시작이면 그 틱)
        out.push((bar, beat, in_bar % beat_ticks, t - in_bar));
    }
    out
}

fn any_sig() -> impl Strategy<Value = (u8, u8)> {
    prop_oneof![
        Just((255u8, 64u8)),
        Just((1u8, 64u8)),
        Just((1u8, 1u8)),
        (1..=255u8, (0..7u32).prop_map(|p| 1u8 << p)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 마디 중간 변화·1틱 간격 연속 변화를 포함해 position·bar_start가 무차별 모델과 같다.
    #[test]
    fn review_meter_map_matches_brute_force(
        initial in any_sig(),
        changes in prop::collection::vec((1..6_000u64, any_sig()), 0..10),
        burst_at in 1..5_000u64,
        burst in prop::collection::vec(any_sig(), 0..5),
    ) {
        let mut map = MeterMap::new(TimeSignature::new(initial.0, u16::from(initial.1)).unwrap());
        let mut model = BTreeMap::from([(0u64, initial)]);
        let mut ops: Vec<(u64, (u8, u8))> = changes;
        // 1틱 간격 연속 변화
        for (i, s) in burst.into_iter().enumerate() {
            ops.push((burst_at + i as u64, s));
        }
        for (t, s) in ops {
            map.set(Tick::new(t), TimeSignature::new(s.0, u16::from(s.1)).unwrap());
            model.insert(t, s);
        }
        let upto = 8_000u64;
        let brute = brute_positions(&model, upto);
        let mut bar_starts: BTreeMap<u64, u64> = BTreeMap::new();
        for (t, &(bar, beat, tick, bar_start)) in brute.iter().enumerate() {
            let p = map.position(Tick::new(t as u64));
            prop_assert_eq!((p.bar(), p.beat(), p.tick()), (bar, beat, tick), "position({})", t);
            bar_starts.entry(bar).or_insert(bar_start);
        }
        for (&bar, &start) in &bar_starts {
            prop_assert_eq!(map.bar_start(bar).unwrap().get(), start, "bar_start({})", bar);
        }
    }
}

/// u64 끝 근처의 박자표 변화(255/64, 1/64, 1틱 간격)에서 패닉이 없고 `position`·`bar_start`가 일관된다.
#[test]
fn review_meter_map_u64_end_is_consistent() {
    let s = |n, d| TimeSignature::new(n, d).unwrap();
    let mut map = MeterMap::new(s(1, 64));
    map.set(Tick::new(u64::MAX - 20_000), s(255, 64));
    map.set(Tick::new(u64::MAX - 2), s(1, 64));
    map.set(Tick::new(u64::MAX - 1), s(255, 64));
    map.set(Tick::new(u64::MAX), s(7, 8));
    for t in [
        0,
        u64::MAX - 20_001,
        u64::MAX - 20_000,
        u64::MAX - 20_000 + 15_300,
        u64::MAX - 3,
        u64::MAX - 2,
        u64::MAX - 1,
        u64::MAX,
    ] {
        let p = map.position(Tick::new(t));
        let start = map.bar_start(p.bar()).unwrap().get();
        let sig = map.signature_at(Tick::new(t));
        assert_eq!(
            start + u64::from(p.beat()) * sig.beat_ticks() + p.tick(),
            t,
            "{t}"
        );
        if p.beat() == 0 && p.tick() == 0 {
            assert_eq!(start, t);
        }
    }
    // 마지막 변화(u64::MAX)는 새 마디를 시작한다.
    let last = map.position(Tick::new(u64::MAX));
    assert_eq!((last.beat(), last.tick()), (0, 0));
    assert_eq!(map.bar_start(last.bar()), Some(Tick::new(u64::MAX)));
    // 그 뒤 마디는 존재하지 않는다. 리뷰 1 당시에는 u64::MAX로 잘랐으나, 제안 6을 받아
    // 실제 마디와 구분되도록 None을 돌려준다.
    assert_eq!(map.bar_start(last.bar() + 1), None);
    assert_eq!(map.bar_start(u64::MAX), None);
}
