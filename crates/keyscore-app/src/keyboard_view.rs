//! 건반 뷰(S3-T5, REQ 5.4 「화면 하단에 2옥타브 건반」, 5.13, 2 「조성 표시」). GPUI와 무관한 계산이다. 그리기는
//! `keyboard_view_window.rs`. 규칙은 `docs/reports/s3-t5/plan.md` 「끝의 정의」.

use std::fmt::Write as _;

use keyscore_core::{Key, Pitch};
use keyscore_text::drum::label_of;

use crate::actions::args::KeyLayout;
use crate::pianoroll::scene::{Shade, row_shade};
use crate::selection::pitch_name;
use crate::step::pitch_of;

/// 건반 수(2옥타브).
pub const KEY_COUNT: usize = 24;

/// 흰건반 최대 너비(논리 px). 넓은 창에서 건반이 지나치게 커지지 않게 한다(결정
/// `2026-10-31-keyboard-view-direct-labels.md` 3).
pub const WHITE_KEY_MAX_WIDTH: f32 = 40.0;

/// 건반 최대 높이(논리 px).
pub const KEY_MAX_HEIGHT: f32 = 120.0;

/// 건반 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyCap {
    /// 127을 넘으면 `None`(없음).
    pub pitch: Option<Pitch>,
    pub black: bool,
    /// Step에서 이 음을 쓰는 키 입력.
    pub key: Option<String>,
    /// 음 이름(`C4`) 또는 드럼 트랙의 GM 라벨(`bd`, 표에 없으면 `n40`).
    pub name: Option<String>,
    /// 조성 밖이면 `true`. 조성이 없으면 늘 `false`.
    pub out_of_key: bool,
}

/// 건반형(P3 (a))의 음 키, 옥타브의 C부터 반음 순서. `pianoroll::keys::semitone_of_key`의 표와 같다.
const PIANO_KEYS: [&str; 16] = [
    "A", "W", "S", "E", "D", "F", "T", "G", "Y", "H", "U", "J", "K", "O", "L", "P",
];

/// 계이름형(P3 (b))의 한 옥타브 입력. 검은건반은 올림표(REQ 5.6 예 `f#`).
const SOLFEGE_KEYS: [&str; 12] = [
    "c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b",
];

/// 검은건반 자리(옥타브 안 반음).
const BLACK: [bool; 12] = [
    false, true, false, true, false, false, true, false, true, false, true, false,
];

/// 옥타브 `octave`의 C부터 24건반.
#[must_use]
pub fn keys(layout: KeyLayout, octave: u8, key: Key, drum: bool) -> Vec<KeyCap> {
    (0..KEY_COUNT)
        .map(|i| {
            let black = BLACK[i % 12];
            let semitone = i16::try_from(i).unwrap_or(0);
            let Some(pitch) = pitch_of(octave, semitone) else {
                return KeyCap {
                    pitch: None,
                    black,
                    key: None,
                    name: None,
                    out_of_key: false,
                };
            };
            // 키 하나로 닿는 건반에만 단다. 「키 + 옥타브 숫자」(`D5`)는 음 이름과 꼴이 같아 헷갈리고 Play 모드에는
            // 그 문법이 없다(결정 `2026-10-31-keyboard-view-direct-labels.md` 1).
            let table: &[&str] = match layout {
                KeyLayout::Piano => &PIANO_KEYS,
                KeyLayout::Solfege => &SOLFEGE_KEYS,
            };
            let label = table.get(i).map(|k| (*k).to_owned());
            let name = if drum {
                // 표에 없는 노트는 `n<번호>`(REQ 5.13, 텍스트 내보내기와 같다, 리뷰 1 지적 2)
                Some(
                    label_of(pitch.get())
                        .map_or_else(|| format!("n{}", pitch.get()), str::to_owned),
                )
            } else {
                Some(pitch_name(pitch))
            };
            KeyCap {
                pitch: Some(pitch),
                black,
                key: label,
                name,
                // 드럼은 조성 명암이 없다(결정 `2026-10-31-key-shading-off-and-drums.md` 2)
                out_of_key: !drum && key != Key::None && row_shade(key, pitch) == Shade::Dark,
            }
        })
        .collect()
}

/// 시험 스냅샷용 글. 건반마다 한 줄: `<음 이름|-> <white|black> <키|-> <in|out>`, 없음이면 `<i> absent`.
#[must_use]
pub fn describe(keys: &[KeyCap]) -> String {
    let mut out = String::new();
    for (i, cap) in keys.iter().enumerate() {
        if cap.pitch.is_none() {
            let _ = writeln!(out, "{i} absent");
            continue;
        }
        let _ = writeln!(
            out,
            "{} {} {} {}",
            cap.name.as_deref().unwrap_or("-"),
            if cap.black { "black" } else { "white" },
            cap.key.as_deref().unwrap_or("-"),
            if cap.out_of_key { "out" } else { "in" },
        );
    }
    out
}
