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

/// 표의 상한: 겹점 온음표(6720틱) 넷. 긴 노트는 겹점 온음표 몇 개 + 이 안의 나머지로 나눈다.
const TABLE_MAX: u64 = 4 * 6720;

/// `TABLE_MAX` 이하 15틱 배수 값의 최소 조각 분해. 칸 `v / 15`에 (조각 수, 첫 조각)이 있다.
/// 같은 조각 수면 첫 조각이 긴 쪽이다. 한 번만 만든다.
fn min_pieces_table() -> &'static [Option<(u8, u64)>] {
    static TABLE: std::sync::OnceLock<Vec<Option<(u8, u64)>>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let size = usize::try_from(TABLE_MAX / 15).unwrap_or(0) + 1;
        let mut t: Vec<Option<(u8, u64)>> = vec![None; size];
        t[0] = Some((0, 0));
        for i in 1..size {
            let v = u64::try_from(i).unwrap_or(0) * 15;
            for n in NOTATIONS {
                if n.ticks > v {
                    continue;
                }
                let rest = usize::try_from((v - n.ticks) / 15).unwrap_or(0);
                if let Some((c, _)) = t[rest] {
                    let better = match t[i] {
                        None => true,
                        Some((best, _)) => c + 1 < best,
                    };
                    if better {
                        t[i] = Some((c + 1, n.ticks));
                    }
                }
            }
        }
        t
    })
}

/// `ticks`(≤ `TABLE_MAX`)를 표기 길이들로 나눈 최소 조각(긴 것부터). 나눌 수 없으면 `None`.
fn min_pieces(ticks: u64) -> Option<Vec<u64>> {
    if ticks > TABLE_MAX || !ticks.is_multiple_of(15) {
        return None;
    }
    let table = min_pieces_table();
    let mut out = Vec::new();
    let mut v = ticks;
    while v > 0 {
        let (_, first) = table[usize::try_from(v / 15).ok()?]?;
        out.push(first);
        v -= first;
    }
    Some(out)
}

/// 표기 길이 하나로 적을 수 없는 노트 길이를 타이로 잇는 조각들(조각 수가 가장 적은 분해).
/// 그리드 길이로 정확히 적을 수 있는 노트에 `~`를 붙이지 않게 한다(REQ 5.14, S0-T5 리뷰 6
/// 지적 2). 그래서 「`max_pieces` 조각 이하의 타이로 적을 수 있으면 타이」가 성립한다(리뷰 7 지적 2).
/// 조각이 `max_pieces`를 넘거나 나눌 수 없으면 `None`.
///
/// 후보 둘 가운데 조각이 적은 쪽이고, 같으면 앞쪽이다.
/// - 온음표들 + 나머지의 최소 분해. 읽기 쉬운 꼴(`c1&c1`)이라 같은 조각 수면 이쪽을 쓴다.
/// - 겹점 온음표(6720틱)들 + 나머지(`TABLE_MAX` 이하)의 최소 분해. 긴 노트는 이쪽이 조각이 적다
///   (16.5온음표: 온음표 16개 + 2분 점음표 17조각, 겹점 온음표 9개 + 2분 점음표 10조각).
pub(crate) fn tie_pieces(ticks: u64, max_pieces: u64) -> Option<Vec<u64>> {
    [by_wholes(ticks), by_double_dotted_wholes(ticks)]
        .into_iter()
        .flatten()
        .min_by_key(Vec::len)
        .filter(|p| u64::try_from(p.len()).is_ok_and(|n| n <= max_pieces))
}

/// 온음표들 + 나머지의 최소 분해. 나머지가 너무 작으면(예: 30틱) 온음표 하나와 합쳐 나눈다.
fn by_wholes(ticks: u64) -> Option<Vec<u64>> {
    let (mut wholes, mut rem) = (ticks / WHOLE, ticks % WHOLE);
    let mut tail = min_pieces(rem);
    if tail.is_none() && wholes > 0 {
        wholes -= 1;
        rem += WHOLE;
        tail = min_pieces(rem);
    }
    let mut pieces = vec![WHOLE; usize::try_from(wholes).ok()?];
    pieces.extend(tail?);
    Some(pieces)
}

/// 겹점 온음표들 + 나머지의 최소 분해. 나머지를 `TABLE_MAX` 안에서 넓혀 가며(겹점 온음표를 하나씩
/// 덜 쓰며) 조각이 가장 적은 것을 고른다. 같으면 겹점 온음표가 많은 쪽이다.
fn by_double_dotted_wholes(ticks: u64) -> Option<Vec<u64>> {
    const DDW: u64 = 6720;
    let most = ticks / DDW;
    (0..=3u64)
        .filter_map(|less| {
            let k = most.checked_sub(less)?;
            let tail = min_pieces(ticks - k * DDW)?;
            let mut pieces = vec![DDW; usize::try_from(k).ok()?];
            pieces.extend(tail);
            Some(pieces)
        })
        .min_by_key(Vec::len)
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
    fn tie_pieces_are_minimal_and_sum_exactly() {
        assert_eq!(tie_pieces(1200, 16), Some(vec![960, 240]));
        assert_eq!(tie_pieces(2400, 16), Some(vec![1920, 480]));
        assert_eq!(tie_pieces(7680, 16), Some(vec![3840, 3840]));
        assert_eq!(tie_pieces(3840 + 210, 16), Some(vec![3840, 210]));
        assert_eq!(tie_pieces(3840 + 30, 16), Some(vec![3360, 420, 90]));
        assert_eq!(tie_pieces(30, 16), None);
        assert_eq!(tie_pieces(1201, 16), None);
        assert_eq!(tie_pieces(17 * 3840, 16).map(|p| p.len()), Some(10));
        assert_eq!(tie_pieces(63_360, 16).map(|p| p.len()), Some(10));
        assert_eq!(tie_pieces(57_630, 16).map(|p| p.len()), Some(11));
        assert_eq!(tie_pieces(16 * 6720 + 960, 16), None);
        for t in (0..3 * WHOLE).step_by(15) {
            if let Some(p) = tie_pieces(t, 64) {
                assert_eq!(p.iter().sum::<u64>(), t, "{t}");
                assert!(p.iter().all(|&x| exact(x).is_some()), "{t}");
            }
        }
    }

    /// 타이 분해의 조각 수가 모든 길이에서 최소다(전수 동적 계획법과 비교, 17온음표 남짓까지).
    #[test]
    fn tie_pieces_count_is_minimal_for_every_length() {
        let limit = 18 * 6720;
        let size = usize::try_from(limit / 15).unwrap() + 1;
        let mut best = vec![u32::MAX; size];
        best[0] = 0;
        for i in 1..size {
            let v = u64::try_from(i).unwrap() * 15;
            for n in NOTATIONS {
                if n.ticks <= v {
                    let r = best[usize::try_from((v - n.ticks) / 15).unwrap()];
                    if r != u32::MAX {
                        best[i] = best[i].min(r + 1);
                    }
                }
            }
        }
        for (i, &b) in best.iter().enumerate() {
            let v = u64::try_from(i).unwrap() * 15;
            let got = tie_pieces(v, 64).map(|p| u32::try_from(p.len()).unwrap());
            let want = (b != u32::MAX && b <= 64).then_some(b);
            assert_eq!(got, want, "{v}");
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
