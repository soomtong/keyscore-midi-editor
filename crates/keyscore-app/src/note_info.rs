//! 노트 정보 팝업(S3-T7, REQ 5.8 `i` 「커서 노트 정보 팝업(틱·길이·채널)」, 5.8.1 vim `K`).
//!
//! 규칙은 결정 `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md` 6절이다.

use keyscore_core::{ClipId, Note, PitchOffset, Project, Tick, TrackId};

use crate::selection::pitch_name;

/// 팝업의 한 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoLine {
    pub label: &'static str,
    pub value: String,
}

/// `track`의 `clip`에 든 `note`의 정보 줄(결정 6절 2). 트랙·클립이 없으면 빈 목록.
#[must_use]
pub fn info_lines(project: &Project, track: TrackId, clip: ClipId, note: &Note) -> Vec<InfoLine> {
    let Some(t) = project.track(track) else {
        return Vec::new();
    };
    let Some(c) = t.clip(clip) else {
        return Vec::new();
    };
    let start = Tick::new(c.start().get().saturating_add(note.start().get()));
    let pos = project.meter_map().position(start);
    let mut lines = vec![
        InfoLine {
            label: "음높이",
            value: pitch_text(note),
        },
        InfoLine {
            label: "시작",
            value: format!(
                "{}.{}.{} ({}틱)",
                pos.bar() + 1,
                pos.beat() + 1,
                pos.tick(),
                start.get()
            ),
        },
        InfoLine {
            label: "길이",
            value: match length_name(note.length()) {
                Some(name) => format!("{}틱 ({name})", note.length()),
                None => format!("{}틱", note.length()),
            },
        },
        InfoLine {
            label: "채널",
            value: t.channel().get().to_string(),
        },
        InfoLine {
            label: "벨로시티",
            value: note.velocity().to_midi1().to_string(),
        },
    ];
    if let Some(lyric) = note.lyric() {
        lines.push(InfoLine {
            label: "가사",
            value: lyric.to_owned(),
        });
    }
    lines.push(InfoLine {
        label: "트랙",
        value: t.name().to_owned(),
    });
    lines
}

/// 「C#4 (61)」, 오프셋이 있으면 「 +25 cent」를 덧붙인다(반음 = 100 cent, 반올림).
fn pitch_text(note: &Note) -> String {
    let pitch = note.pitch();
    let text = format!("{} ({})", pitch_name(pitch), pitch.get());
    let units = i64::from(note.offset().units());
    if units == 0 {
        return text;
    }
    let semitone = i64::from(PitchOffset::SEMITONE);
    // 가장 가까운 정수 cent(절반은 0에서 먼 쪽)
    let cents = (units * 200 + units.signum() * semitone) / (2 * semitone);
    format!("{text} {cents:+} cent")
}

/// 길이 코드(온음표–64분음표, 점, 셋잇단)와 같은 틱이면 그 이름.
fn length_name(ticks: u32) -> Option<String> {
    const NAMES: [&str; 7] = [
        "온음표",
        "2분음표",
        "4분음표",
        "8분음표",
        "16분음표",
        "32분음표",
        "64분음표",
    ];
    NAMES.iter().enumerate().find_map(|(i, name)| {
        let plain = WHOLE >> i;
        if ticks == plain {
            Some((*name).to_owned())
        } else if ticks == plain + plain / 2 {
            Some(format!("점{name}"))
        } else if u64::from(ticks) * 3 == u64::from(plain) * 2 {
            Some(format!("{name} 셋잇단"))
        } else {
            None
        }
    })
}

/// 온음표 틱(PPQ 960).
const WHOLE: u32 = 3840;
