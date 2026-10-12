//! 길이 표기와 틱 사이의 변환. 파서와 내보내기가 같은 표를 쓴다.

/// 온음표의 틱(PPQ 960 × 4).
pub(crate) const WHOLE: u64 = 3840;

/// 길이 표기 하나: 분음표 숫자와 점 개수.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Notation {
    pub(crate) ticks: u64,
    denominator: u64,
    dots: u8,
}

impl Notation {
    /// `4.`처럼 음 뒤에 붙는 문자열.
    pub(crate) fn text(self) -> String {
        format!("{}{}", self.denominator, ".".repeat(usize::from(self.dots)))
    }
}

/// 표기 가능한 길이 21개, 긴 것부터. 분음표마다 겹점(×1.75), 점(×1.5), 점 없음 순이며
/// 겹점 다음 분음표(×0.875)가 점 없는 앞 분음표보다 짧아 이 순서가 그대로 내림차순이다.
pub(crate) const NOTATIONS: [Notation; 21] = build_notations();

const fn build_notations() -> [Notation; 21] {
    let mut out = [Notation {
        ticks: 0,
        denominator: 0,
        dots: 0,
    }; 21];
    let mut i = 0;
    while i < 7 {
        let denominator = 1 << i;
        let base = WHOLE / denominator;
        out[i * 3] = Notation {
            ticks: base * 7 / 4,
            denominator,
            dots: 2,
        };
        out[i * 3 + 1] = Notation {
            ticks: base * 3 / 2,
            denominator,
            dots: 1,
        };
        out[i * 3 + 2] = Notation {
            ticks: base,
            denominator,
            dots: 0,
        };
        i += 1;
    }
    out
}

/// 분음표 숫자 문자열(앞의 0 없이)과 점 개수의 틱. 허용 밖이면 `None`.
pub(crate) fn ticks_of(digits: &str, dots: u8) -> Option<u64> {
    let denominator = match digits {
        "1" => 1,
        "2" => 2,
        "4" => 4,
        "8" => 8,
        "16" => 16,
        "32" => 32,
        "64" => 64,
        _ => return None,
    };
    NOTATIONS
        .iter()
        .find(|n| n.denominator == denominator && n.dots == dots)
        .map(|n| n.ticks)
}

/// 틱이 정확히 표기 가능한 길이면 그 표기.
pub(crate) fn exact(ticks: u64) -> Option<Notation> {
    NOTATIONS.iter().copied().find(|n| n.ticks == ticks)
}

/// 가장 가까운 표기(REQ 5.14 「가장 가까운 길이 표기」). 같은 거리면 짧은 쪽이다.
/// 짧은 쪽이면 커서가 실제 끝을 앞지르지 않아 다음 음의 시작 보정이 음수가 되는 일이 줄어든다.
pub(crate) fn nearest(ticks: u64) -> Notation {
    let mut best = NOTATIONS[0];
    for n in NOTATIONS {
        if n.ticks.abs_diff(ticks) <= best.ticks.abs_diff(ticks) {
            best = n;
        }
    }
    best
}

/// 60틱(64분음표)의 배수를 쉼표 길이로 나눈다. 온음표를 먼저 쓰고 나머지는 큰 것부터 채운다.
/// 60틱 배수인 표기만 쓰므로(점 64분·겹점 64분·겹점 32분 제외) 탐욕으로 정확히 나뉜다.
pub(crate) fn rests_of_multiple_of_60(mut ticks: u64) -> Vec<u64> {
    debug_assert_eq!(ticks % 60, 0);
    let mut out = Vec::new();
    while ticks >= WHOLE {
        out.push(WHOLE);
        ticks -= WHOLE;
    }
    for n in NOTATIONS {
        if n.ticks % 60 != 0 {
            continue;
        }
        while ticks >= n.ticks {
            out.push(n.ticks);
            ticks -= n.ticks;
        }
    }
    out
}

/// 쉼표만으로 정확히 채울 수 있는 길이인가. 표기 길이는 모두 15틱의 배수이고, 60·90·105의
/// 합으로 15틱 배수 가운데 15·30·45·75·135만 빼고 모두 만들 수 있다.
pub(crate) fn restable(ticks: u64) -> bool {
    ticks.is_multiple_of(15) && !matches!(ticks, 15 | 30 | 45 | 75 | 135)
}

/// `ticks` 이하에서 쉼표로 채울 수 있는 가장 긴 길이(0이면 채울 수 없음).
pub(crate) fn largest_restable(ticks: u64) -> u64 {
    let mut t = ticks - ticks % 15;
    while !restable(t) {
        t -= 15;
    }
    t
}

/// `ticks` 이상에서 쉼표로 채울 수 있는 가장 짧은 길이(0보다 큼).
pub(crate) fn smallest_restable_from(ticks: u64) -> u64 {
    let mut t = ticks.max(1).div_ceil(15) * 15;
    while !restable(t) {
        t += 15;
    }
    t
}

/// `ticks`(쉼표로 채울 수 있는 길이)를 쉼표들로 나눈다. 같은 길이면 늘 같은 결과다.
/// 60틱 배수가 아닌 나머지는 점 64분(90)·겹점 64분(105)으로 끝에 붙인다.
pub(crate) fn rests_of(ticks: u64) -> Vec<u64> {
    debug_assert!(restable(ticks));
    let odd: &[u64] = match ticks % 60 {
        30 => &[90],
        45 => &[105],
        15 => &[90, 105],
        _ => &[],
    };
    let odd_sum: u64 = odd.iter().sum();
    let mut out = rests_of_multiple_of_60(ticks - odd_sum);
    out.extend_from_slice(odd);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notations_are_strictly_descending_and_divisible_by_15() {
        for w in NOTATIONS.windows(2) {
            assert!(w[0].ticks > w[1].ticks);
        }
        assert!(NOTATIONS.iter().all(|n| n.ticks % 15 == 0));
        assert_eq!(NOTATIONS[0].text(), "1..");
        assert_eq!(NOTATIONS[20].ticks, 60);
    }

    #[test]
    fn rests_of_sums_exactly_for_every_restable_length() {
        for t in 0..20_000 {
            if restable(t) {
                let r = rests_of(t);
                assert_eq!(r.iter().sum::<u64>(), t, "{t}");
                assert!(r.iter().all(|&p| exact(p).is_some()), "{t}");
            }
            assert!(largest_restable(t) <= t);
            assert!(restable(largest_restable(t)));
            let above = smallest_restable_from(t);
            assert!(
                above >= t.max(1) && restable(above) && above - t.max(1) < 60,
                "{t}"
            );
        }
    }

    #[test]
    fn nearest_prefers_shorter_on_tie() {
        assert_eq!(nearest(900).ticks, 840);
        assert_eq!(nearest(1).ticks, 60);
        assert_eq!(nearest(100_000).ticks, 6720);
        assert_eq!(nearest(950).ticks, 960);
    }
}
