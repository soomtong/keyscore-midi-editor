//! 검색의 순수 계산(S3-T7, REQ 5.8 `Cmd+F`, 5.8.1 `/`·`n`·`N`). GPUI와 무관하다.
//!
//! 규칙은 결정 `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md` 2·3절이다. 창 쪽(막대·이동)은
//! `search_bar.rs`.

use std::collections::BTreeMap;

use keyscore_core::{Clip, Note, Pitch, Project, Tick, Track, TrackId};

/// 읽은 검색어.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Query {
    /// 옥타브가 있는 음높이(`c#4`).
    Pitch(Pitch),
    /// 옥타브가 없는 음이름(`c#`). 0–11, C = 0.
    PitchClass(u8),
    /// 트랙 이름·가사의 부분 문자열(소문자로 바꾼 것).
    Text(String),
}

/// 검색어를 읽지 못한 까닭.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    /// 빈 검색어.
    Empty,
    /// 음높이 꼴이지만 0–127 밖.
    PitchOutOfRange,
}

impl QueryError {
    /// 막대·상태줄에 보일 글.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            QueryError::Empty => "검색어가 비었다",
            QueryError::PitchOutOfRange => "음높이 범위 밖",
        }
    }
}

impl Query {
    /// 검색어를 읽는다(결정 2절).
    ///
    /// # Errors
    ///
    /// 비었으면 [`QueryError::Empty`], 음높이 꼴인데 0–127 밖이면 [`QueryError::PitchOutOfRange`].
    pub fn parse(text: &str) -> Result<Query, QueryError> {
        let text = text.trim();
        // 큰따옴표로 감싸면 음높이 꼴이어도 글자다(결정 2절 2). 안쪽 공백은 그대로 둔다
        if let Some(inner) = text
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .filter(|_| text.len() >= 2)
        {
            return if inner.is_empty() {
                Err(QueryError::Empty)
            } else {
                Ok(Query::Text(inner.to_lowercase()))
            };
        }
        if text.is_empty() {
            return Err(QueryError::Empty);
        }
        match parse_pitch(text) {
            Some(PitchForm::Exact(Some(pitch))) => Ok(Query::Pitch(pitch)),
            Some(PitchForm::Exact(None)) => Err(QueryError::PitchOutOfRange),
            Some(PitchForm::Class(class)) => Ok(Query::PitchClass(class)),
            None => Ok(Query::Text(text.to_lowercase())),
        }
    }

    /// `note`(트랙 이름이 아닌 노트)가 맞는가.
    fn matches_note(&self, note: &Note) -> bool {
        match self {
            Query::Pitch(pitch) => note.pitch() == *pitch,
            Query::PitchClass(class) => note.pitch().get() % 12 == *class,
            Query::Text(text) => note
                .lyric()
                .is_some_and(|lyric| lyric.to_lowercase().contains(text.as_str())),
        }
    }

    /// 트랙 이름이 맞는가. 음높이 꼴은 트랙 이름을 보지 않는다(결정 2절 1).
    fn matches_track(&self, track: &Track) -> bool {
        match self {
            Query::Text(text) => track.name().to_lowercase().contains(text.as_str()),
            Query::Pitch(_) | Query::PitchClass(_) => false,
        }
    }
}

/// 음높이 꼴의 검색어.
enum PitchForm {
    /// 옥타브가 있다. 0–127 밖이면 `None`.
    Exact(Option<Pitch>),
    Class(u8),
}

/// `[a-g]`, 선택적 `#`/`b`, 선택적 옥타브 `-1`–`9`. 그 꼴이 아니면 `None`.
fn parse_pitch(text: &str) -> Option<PitchForm> {
    let lower = text.to_ascii_lowercase();
    let mut rest = lower.as_str();
    let letter = rest.chars().next()?;
    let base: i16 = match letter {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    rest = &rest[1..];
    let accidental: i16 = if let Some(r) = rest.strip_prefix('#') {
        rest = r;
        1
    } else if let Some(r) = rest.strip_prefix('b') {
        rest = r;
        -1
    } else {
        0
    };
    let semitone = base + accidental;
    if rest.is_empty() {
        let class = u8::try_from(semitone.rem_euclid(12)).expect("0–11");
        return Some(PitchForm::Class(class));
    }
    let octave: i16 = match rest {
        "-1" => -1,
        _ if rest.len() == 1 && rest.as_bytes()[0].is_ascii_digit() => {
            i16::from(rest.as_bytes()[0] - b'0')
        }
        _ => return None,
    };
    // C4 = 60(앱의 음이름 표시와 같다, `selection::pitch_name`)
    let value = (octave + 1) * 12 + semitone;
    let pitch = u8::try_from(value).ok().and_then(|v| Pitch::new(v).ok());
    Some(PitchForm::Exact(pitch))
}

/// 결과 하나: 트랙과 그 안의 자리.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub track: TrackId,
    /// 트랙 기준 틱(노트 결과는 클립 시작 + 노트 시작).
    pub tick: Tick,
    pub pitch: Pitch,
    /// 자리에 노트가 있는가. 노트 없는 트랙 결과는 거짓이고, 이동할 때 커서 음높이를 그대로 둔다.
    pub has_note: bool,
}

/// 프로젝트에서 `query`의 결과를 (틱, 트랙 순서, 음높이) 순서로 찾는다. 같은 자리는 하나다(결정 3절).
#[must_use]
pub fn find_hits(project: &Project, query: &Query) -> Vec<Hit> {
    // (틱, 트랙 순서, 음높이) → 노트가 있는가. 같은 자리는 하나로 모으고, 노트가 있는 쪽을 남긴다
    let mut found: BTreeMap<(Tick, usize, Pitch), (TrackId, bool)> = BTreeMap::new();
    for (index, track) in project.tracks().iter().enumerate() {
        let id = track.id();
        for (_, clip) in track.clips() {
            for note in clip.notes() {
                if query.matches_note(note) {
                    let tick = absolute(clip, note);
                    found.insert((tick, index, note.pitch()), (id, true));
                }
            }
        }
        if query.matches_track(track) {
            let (tick, pitch, has_note) = track_spot(track);
            found
                .entry((tick, index, pitch))
                .and_modify(|e| e.1 |= has_note)
                .or_insert((id, has_note));
        }
    }
    found
        .into_iter()
        .map(|((tick, _, pitch), (track, has_note))| Hit {
            track,
            tick,
            pitch,
            has_note,
        })
        .collect()
}

/// 노트 시작의 트랙 기준 틱. 클립 끝이 u64 안이므로 넘치지 않는다.
fn absolute(clip: &Clip, note: &Note) -> Tick {
    Tick::new(clip.start().get().saturating_add(note.start().get()))
}

/// 트랙 결과의 자리: 첫 노트(시작이 가장 이르고, 같으면 낮은 음), 없으면 첫 클립 시작, 클립도 없으면 0
/// (결정 3절 3). 노트가 없으면 음높이 0이다.
fn track_spot(track: &Track) -> (Tick, Pitch, bool) {
    let first_note = track
        .clips()
        .flat_map(|(_, clip)| {
            clip.notes()
                .iter()
                .map(move |n| (absolute(clip, n), n.pitch()))
        })
        .min();
    if let Some((tick, pitch)) = first_note {
        return (tick, pitch, true);
    }
    let tick = track.clips().next().map_or(Tick::ZERO, |(_, c)| c.start());
    (tick, Pitch::MIN, false)
}

/// 지금 자리: 현재 트랙 순서, 커서 틱·음높이.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub track_index: usize,
    pub tick: Tick,
    pub pitch: Pitch,
}

/// 찾는 방향.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// 고른 결과.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    /// `hits` 안의 순번.
    pub index: usize,
    /// 끝을 넘어 반대 끝에서 골랐다(결정 3절 5).
    pub wrapped: bool,
}

/// `at` 다음(또는 이전) 결과. 결과가 없으면 `None`.
#[must_use]
pub fn step(project: &Project, hits: &[Hit], at: Position, direction: Direction) -> Option<Found> {
    // 결과의 트랙 순서는 지금 프로젝트의 순서로 본다. 트랙이 없어졌으면 그 결과는 고르지 않는다
    let keyed: Vec<(usize, (Tick, usize, Pitch), bool)> = hits
        .iter()
        .enumerate()
        .filter_map(|(i, h)| {
            let track_index = project.track_index(h.track)?;
            Some((i, (h.tick, track_index, h.pitch), h.has_note))
        })
        .collect();
    let here = (at.tick, at.track_index, at.pitch);
    // 지금 자리에 대한 결과의 앞뒤. 노트 없는 트랙 결과는 이동해도 커서 음높이를 그대로 두므로(결정 4절 1)
    // 음높이를 보지 않는다: (틱, 트랙)이 같으면 그 결과 위에 있는 것이다(리뷰 1 지적 1). 그 트랙에는 노트가
    // 없어 같은 (틱, 트랙)의 다른 결과가 없으므로 결과 사이의 순서는 그대로다.
    let relation = |key: (Tick, usize, Pitch), has_note: bool| {
        if has_note {
            key.cmp(&here)
        } else {
            (key.0, key.1).cmp(&(here.0, here.1))
        }
    };
    let first = keyed
        .iter()
        .min_by_key(|(_, k, _)| *k)
        .map(|(i, _, _)| *i)?;
    let last = keyed
        .iter()
        .max_by_key(|(_, k, _)| *k)
        .map(|(i, _, _)| *i)?;
    let found = match direction {
        Direction::Forward => keyed
            .iter()
            .filter(|(_, k, n)| relation(*k, *n).is_gt())
            .min_by_key(|(_, k, _)| *k)
            .map(|(i, _, _)| *i),
        Direction::Backward => keyed
            .iter()
            .filter(|(_, k, n)| relation(*k, *n).is_lt())
            .max_by_key(|(_, k, _)| *k)
            .map(|(i, _, _)| *i),
    };
    Some(match (found, direction) {
        (Some(index), _) => Found {
            index,
            wrapped: false,
        },
        (None, Direction::Forward) => Found {
            index: first,
            wrapped: true,
        },
        (None, Direction::Backward) => Found {
            index: last,
            wrapped: true,
        },
    })
}
