//! S0-T3 적대적 리뷰 2회차: 분모 1–256 경계와 `bar_start`의 `None` 경계를 공개 API로만 시험한다.

use keyscore_core::{MeterMap, ModelError, Tick, TimeSignature};

#[test]
fn time_signature_accepts_exactly_powers_of_two_up_to_256() {
    for d in 0..=u16::MAX {
        let ok = d.is_power_of_two() && d <= 256;
        let got = TimeSignature::new(1, d);
        if ok {
            let s = got.unwrap_or_else(|e| panic!("분모 {d}: {e}"));
            assert_eq!(s.denominator(), d);
            // 박 길이가 정수이고 분모 박 개수가 온음표(3840틱)와 정확히 맞아야 한다.
            assert_eq!(s.beat_ticks() * u64::from(d), 3840, "분모 {d}");
        } else {
            assert_eq!(got, Err(ModelError::TimeSignatureInvalid(1, d)), "분모 {d}");
        }
    }
    assert_eq!(
        TimeSignature::new(0, 256),
        Err(ModelError::TimeSignatureInvalid(0, 256))
    );
    assert!(TimeSignature::new(255, 256).is_ok());
}

/// 분모 256(박 15틱)에서 각 박·틱 경계의 position이 무차별 계산과 같다.
#[test]
fn meter_map_denominator_256_positions_match_brute_force() {
    let s = TimeSignature::new(7, 256).unwrap();
    let mut map = MeterMap::default();
    map.set(Tick::new(100), s); // 마디 중간 변화
    let start_bar = map.position(Tick::new(100)).bar();
    assert_eq!(start_bar, 1);
    for t in 100..100 + 105 * 4 {
        let p = map.position(Tick::new(t));
        let off = t - 100;
        assert_eq!(p.bar(), 1 + off / 105, "t={t}");
        assert_eq!(u64::from(p.beat()), off % 105 / 15, "t={t}");
        assert_eq!(p.tick(), off % 15, "t={t}");
        assert_eq!(map.bar_start(p.bar()).unwrap().get(), 100 + off / 105 * 105);
    }
}

/// `Some`과 `None`의 경계가 마디 시작이 u64에 들어가는지와 정확히 같다.
#[test]
fn bar_start_none_boundary_is_exact_for_default_and_denominator_256() {
    let map = MeterMap::default();
    let last = u64::MAX / 3840;
    assert_eq!(map.bar_start(last), Some(Tick::new(last * 3840)));
    assert_eq!(map.bar_start(last + 1), None);
    assert_eq!(map.bar_start(u64::MAX), None);

    // 255/256 = 3825틱 마디. 변화 틱이 0이 아닌 곳에서 시작해도 경계가 정확해야 한다.
    let mut map = MeterMap::default();
    let change = 3840 * 5 + 17;
    map.set(Tick::new(change), TimeSignature::new(255, 256).unwrap());
    let first_bar = map.position(Tick::new(change)).bar();
    let n = (u64::MAX - change) / 3825;
    assert_eq!(
        map.bar_start(first_bar + n),
        Some(Tick::new(change + n * 3825))
    );
    assert_eq!(map.bar_start(first_bar + n + 1), None);
    let end = map.position(Tick::new(u64::MAX));
    assert_eq!(end.bar(), first_bar + n);
    assert_eq!(map.bar_start(end.bar()), Some(Tick::new(change + n * 3825)));
}

/// 곱셈은 넘치지 않지만 덧셈이 넘치는 경우와, 곱셈이 넘치는 경우 모두 `None`이고 패닉이 없다.
#[test]
fn bar_start_never_panics_for_any_bar_near_u64_max() {
    let mut map = MeterMap::default();
    map.set(
        Tick::new(u64::MAX - 1),
        TimeSignature::new(255, 256).unwrap(),
    );
    for bar in (u64::MAX - 10_000..=u64::MAX).chain(0..10_000) {
        let _ = map.bar_start(bar);
    }
    let p = map.position(Tick::new(u64::MAX));
    let start = map.bar_start(p.bar()).unwrap().get();
    assert_eq!(start + u64::from(p.beat()) * 15 + p.tick(), u64::MAX);
}
