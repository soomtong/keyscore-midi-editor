use crate::{
    Bank, Channel, Clip, ClipId, Key, ModelError, Note, OutputPort, Program, Project, Tempo, Tick,
    TimeSignature, Track, TrackColor, TrackId, Velocity,
};

/// 트랙의 속성 하나(REQ SEQ-7). [`Edit::SetTrackProp`]이 바꾼다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackProp {
    Name(String),
    Color(TrackColor),
    Port(OutputPort),
    Channel(Channel),
    Bank(Option<Bank>),
    Program(Option<Program>),
    DefaultVelocity(Velocity),
    Mute(bool),
    Solo(bool),
    LyricLang(Option<String>),
}

/// 모델을 바꾸는 편집(EDT-1). [`crate::Document::apply`]로 적용한다.
///
/// 정방향 편집만 공개한다. undo가 쓰는 역편집(지운 노트 되살리기, 분할 되돌리기, 원래 id로
/// 클립 되살리기)은 crate 안의 `Op`에만 있고 히스토리 안에만 보관된다. 역편집을 밖에서
/// 만들거나 보관했다 다른 맥락에 적용할 수 있으면, 분할 없이 숨김 꼬리를 만들거나 원자성을
/// 깨뜨릴 수 있기 때문이다(S0-T4 리뷰 1 지적 1–3).
///
/// 역편집 전용 연산이 공개 변형이 아니라는 확인은 [`crate::Document`] 문서에 있다.
///
/// ```compile_fail
/// // 편집을 히스토리 밖에서 적용해 역편집을 얻을 수 없다.
/// let mut p = keyscore_core::Project::new();
/// let _ = keyscore_core::Edit::SetKey(keyscore_core::Key::None).apply(&mut p);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// `index` 자리에 트랙을 넣는다. 같은 [`TrackId`]의 트랙이 이미 있으면 실패한다.
    InsertTrack {
        index: usize,
        track: Box<Track>,
    },
    RemoveTrack {
        track: TrackId,
    },
    SetTrackProp {
        track: TrackId,
        prop: TrackProp,
    },
    /// 클립 id는 새로 발급된다.
    InsertClip {
        track: TrackId,
        clip: Box<Clip>,
    },
    RemoveClip {
        track: TrackId,
        id: ClipId,
    },
    MoveClip {
        track: TrackId,
        id: ClipId,
        to: Tick,
    },
    /// 오른쪽 클립 id는 새로 발급된다. 분할점을 가로지르는 노트는 왼쪽에 숨김 꼬리로 남는다.
    SplitClip {
        track: TrackId,
        id: ClipId,
        at: Tick,
    },
    /// 클립 길이를 `length`로 늘린다(REQ 5.10 「입력이 끝을 넘으면 클립이 늘어남」). 줄이기는 받지
    /// 않는다. 비파괴 트림은 S4-T2이고, undo가 쓰는 줄이기는 crate 내부에만 있다.
    ExtendClip {
        track: TrackId,
        id: ClipId,
        length: u64,
    },
    SetClipName {
        track: TrackId,
        id: ClipId,
        name: Option<String>,
    },
    SetClipColor {
        track: TrackId,
        id: ClipId,
        color: Option<TrackColor>,
    },
    InsertNote {
        track: TrackId,
        clip: ClipId,
        note: Note,
    },
    RemoveNote {
        track: TrackId,
        clip: ClipId,
        note: Note,
    },
    /// 노트 하나를 다른 노트로 바꾼다(일괄 연산의 이동·조옮김). 새 노트의 시작은 클립 안이어야 하고,
    /// 끝은 클립 안이어야 한다. 다만 원래 노트가 숨김 꼬리(끝 > 클립 길이)였으면 새 노트도 꼬리일 수
    /// 있다. 꼬리는 분할로 생긴 것에서만 이어지므로 「꼬리는 분할로만 생긴다」가 유지된다.
    ReplaceNote {
        track: TrackId,
        clip: ClipId,
        from: Note,
        to: Note,
    },
    /// 클립 하나의 길이를 `length`(늘리기만)로 정하고 노트들을 동시에 바꾼다(일괄 연산 전용). 모든
    /// `from`은 편집 직전의 노트에서 찾는다. 앞 pair의 `to`를 뒤 pair의 `from`으로 쓸 수 없다.
    /// 꼬리 여부는 이 편집 직전의 길이로 판정한다: 원래 꼬리였던 노트만 새 길이를 넘는 꼬리일 수
    /// 있다. `ExtendClip` 뒤에 `ReplaceNote`를 따로 두면 늘린 길이로 꼬리를 판정해, 늘림 때문에
    /// 꼬리가 아니게 된 노트가 거부된다(S0-T7 리뷰 2 지적 1).
    ReplaceNotes {
        track: TrackId,
        clip: ClipId,
        length: u64,
        pairs: Vec<(Note, Note)>,
    },
    /// `tempo`가 `None`이면 그 틱의 변화를 지운다.
    SetTempo {
        at: Tick,
        tempo: Option<Tempo>,
    },
    /// `signature`가 `None`이면 그 틱의 변화를 지운다.
    SetMeter {
        at: Tick,
        signature: Option<TimeSignature>,
    },
    SetKey(Key),
}

/// 히스토리가 적용·보관하는 연산. 공개 [`Edit`]에 역편집 전용 연산을 더한 것이다.
///
/// 역편집 전용 연산(`InsertClipWithId`, `SplitClipWithId`, `UnsplitClip`, `RestoreNote`)은
/// 실제로 일어난 편집의 역으로만 만들어진다. 그래서 그 편집 직후 상태에 대해서는 항상 성공한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Op {
    InsertTrack {
        index: usize,
        track: Box<Track>,
    },
    RemoveTrack {
        track: TrackId,
    },
    SetTrackProp {
        track: TrackId,
        prop: TrackProp,
    },
    InsertClip {
        track: TrackId,
        clip: Box<Clip>,
    },
    InsertClipWithId {
        track: TrackId,
        clip: Box<Clip>,
        id: ClipId,
    },
    RemoveClip {
        track: TrackId,
        id: ClipId,
    },
    MoveClip {
        track: TrackId,
        id: ClipId,
        to: Tick,
    },
    SplitClip {
        track: TrackId,
        id: ClipId,
        at: Tick,
    },
    SplitClipWithId {
        track: TrackId,
        id: ClipId,
        at: Tick,
        right: ClipId,
    },
    ExtendClip {
        track: TrackId,
        id: ClipId,
        length: u64,
    },
    /// `ExtendClip`의 역. 늘리기 전 길이로 되돌린다.
    ShrinkClip {
        track: TrackId,
        id: ClipId,
        length: u64,
    },
    UnsplitClip {
        track: TrackId,
        left: ClipId,
        right: ClipId,
        original: Box<Clip>,
    },
    SetClipName {
        track: TrackId,
        id: ClipId,
        name: Option<String>,
    },
    SetClipColor {
        track: TrackId,
        id: ClipId,
        color: Option<TrackColor>,
    },
    InsertNote {
        track: TrackId,
        clip: ClipId,
        note: Note,
    },
    RemoveNote {
        track: TrackId,
        clip: ClipId,
        note: Note,
    },
    ReplaceNote {
        track: TrackId,
        clip: ClipId,
        from: Note,
        to: Note,
    },
    ReplaceNotes {
        track: TrackId,
        clip: ClipId,
        length: u64,
        pairs: Vec<(Note, Note)>,
    },
    /// `ReplaceNotes`의 역. 노트를 되돌리고 원래 길이로 줄인다.
    RestoreNotes {
        track: TrackId,
        clip: ClipId,
        length: u64,
        pairs: Vec<(Note, Note)>,
    },
    /// `ReplaceNote`의 역. 원래 노트(꼬리였을 수 있다)를 그대로 되돌린다.
    UnreplaceNote {
        track: TrackId,
        clip: ClipId,
        from: Note,
        to: Note,
    },
    RestoreNote {
        track: TrackId,
        clip: ClipId,
        note: Note,
    },
    SetTempo {
        at: Tick,
        tempo: Option<Tempo>,
    },
    SetMeter {
        at: Tick,
        signature: Option<TimeSignature>,
    },
    /// 조성. `extra`가 `None`이면 정방향 편집이다: 값이 바뀌면 `key` 객체의 모르는 키를 지운다
    /// (결정 `2026-10-12-ksq-unknown-keys-on-value-change.md` B). `Some`이면 역편집이 담은 옛 모르는 키를
    /// 그대로 되돌린다.
    SetKey {
        key: Key,
        extra: Option<Box<JsonMap>>,
    },
    /// 트랙 출력 포트. `extra`의 뜻은 [`Op::SetKey`]와 같다(`port` 객체의 모르는 키).
    SetPort {
        track: TrackId,
        port: OutputPort,
        extra: Option<Box<JsonMap>>,
    },
}

/// `.ksq` 모르는 키 묶음.
type JsonMap = serde_json::Map<String, serde_json::Value>;

impl From<Edit> for Op {
    fn from(edit: Edit) -> Op {
        match edit {
            Edit::InsertTrack { index, track } => Op::InsertTrack { index, track },
            Edit::RemoveTrack { track } => Op::RemoveTrack { track },
            Edit::SetTrackProp {
                track,
                prop: TrackProp::Port(port),
            } => Op::SetPort {
                track,
                port,
                extra: None,
            },
            Edit::SetTrackProp { track, prop } => Op::SetTrackProp { track, prop },
            Edit::InsertClip { track, clip } => Op::InsertClip { track, clip },
            Edit::RemoveClip { track, id } => Op::RemoveClip { track, id },
            Edit::MoveClip { track, id, to } => Op::MoveClip { track, id, to },
            Edit::SplitClip { track, id, at } => Op::SplitClip { track, id, at },
            Edit::ExtendClip { track, id, length } => Op::ExtendClip { track, id, length },
            Edit::SetClipName { track, id, name } => Op::SetClipName { track, id, name },
            Edit::SetClipColor { track, id, color } => Op::SetClipColor { track, id, color },
            Edit::InsertNote { track, clip, note } => Op::InsertNote { track, clip, note },
            Edit::RemoveNote { track, clip, note } => Op::RemoveNote { track, clip, note },
            Edit::ReplaceNote {
                track,
                clip,
                from,
                to,
            } => Op::ReplaceNote {
                track,
                clip,
                from,
                to,
            },
            Edit::ReplaceNotes {
                track,
                clip,
                length,
                pairs,
            } => Op::ReplaceNotes {
                track,
                clip,
                length,
                pairs,
            },
            Edit::SetTempo { at, tempo } => Op::SetTempo { at, tempo },
            Edit::SetMeter { at, signature } => Op::SetMeter { at, signature },
            Edit::SetKey(key) => Op::SetKey { key, extra: None },
        }
    }
}

impl Op {
    /// 적용하고 역연산을 돌려준다. 실패하면 프로젝트는 바뀌지 않는다.
    #[allow(
        clippy::too_many_lines,
        reason = "변형마다 한 갈래인 match라서 나누면 역연산 대응을 한눈에 볼 수 없다"
    )]
    pub(crate) fn apply(self, project: &mut Project) -> Result<Op, ModelError> {
        match self {
            Op::InsertTrack { index, track } => {
                let id = track.id();
                project.insert_track(index, *track)?;
                Ok(Op::RemoveTrack { track: id })
            }
            Op::RemoveTrack { track } => {
                let index = project.track_index(track).ok_or(ModelError::UnknownTrack)?;
                let removed = project
                    .remove_track(index)
                    .ok_or(ModelError::UnknownTrack)?;
                Ok(Op::InsertTrack {
                    index,
                    track: Box::new(removed),
                })
            }
            Op::SetTrackProp { track, prop } => {
                let old = set_track_prop(track_mut(project, track)?, prop);
                Ok(Op::SetTrackProp { track, prop: old })
            }
            Op::InsertClip { track, clip } => {
                let id = track_mut(project, track)?.insert_clip(*clip)?;
                Ok(Op::RemoveClip { track, id })
            }
            Op::InsertClipWithId { track, clip, id } => {
                track_mut(project, track)?.insert_clip_with_id(id, *clip)?;
                Ok(Op::RemoveClip { track, id })
            }
            Op::RemoveClip { track, id } => {
                let clip = track_mut(project, track)?
                    .remove_clip(id)
                    .ok_or(ModelError::UnknownClip)?;
                Ok(Op::InsertClipWithId {
                    track,
                    clip: Box::new(clip),
                    id,
                })
            }
            Op::MoveClip { track, id, to } => {
                let t = track_mut(project, track)?;
                let from = t.clip(id).ok_or(ModelError::UnknownClip)?.start();
                t.move_clip(id, to)?;
                Ok(Op::MoveClip {
                    track,
                    id,
                    to: from,
                })
            }
            Op::SplitClip { track, id, at } => {
                let t = track_mut(project, track)?;
                let original = t.clip(id).ok_or(ModelError::UnknownClip)?.clone();
                let right = t.split_clip(id, at)?;
                Ok(Op::UnsplitClip {
                    track,
                    left: id,
                    right,
                    original: Box::new(original),
                })
            }
            Op::SplitClipWithId {
                track,
                id,
                at,
                right,
            } => {
                let t = track_mut(project, track)?;
                let original = t.clip(id).ok_or(ModelError::UnknownClip)?.clone();
                t.split_clip_with_id(id, at, Some(right))?;
                Ok(Op::UnsplitClip {
                    track,
                    left: id,
                    right,
                    original: Box::new(original),
                })
            }
            Op::ExtendClip { track, id, length } => {
                let t = track_mut(project, track)?;
                let current = t.clip(id).ok_or(ModelError::UnknownClip)?.length();
                if length < current {
                    return Err(ModelError::ClipWouldShrink {
                        current,
                        requested: length,
                    });
                }
                t.resize_clip(id, length)?;
                Ok(Op::ShrinkClip {
                    track,
                    id,
                    length: current,
                })
            }
            Op::ShrinkClip { track, id, length } => {
                let t = track_mut(project, track)?;
                let current = t.clip(id).ok_or(ModelError::UnknownClip)?.length();
                t.resize_clip(id, length)?;
                Ok(Op::ExtendClip {
                    track,
                    id,
                    length: current,
                })
            }
            Op::UnsplitClip {
                track,
                left,
                right,
                original,
            } => {
                let t = track_mut(project, track)?;
                let at = Tick::new(t.clip(left).ok_or(ModelError::UnknownClip)?.end());
                t.unsplit_clip(left, right, *original)?;
                Ok(Op::SplitClipWithId {
                    track,
                    id: left,
                    at,
                    right,
                })
            }
            Op::SetClipName { track, id, name } => {
                let t = track_mut(project, track)?;
                let old = t
                    .clip(id)
                    .ok_or(ModelError::UnknownClip)?
                    .name()
                    .map(str::to_owned);
                t.set_clip_name(id, name.unwrap_or_default())?;
                Ok(Op::SetClipName {
                    track,
                    id,
                    name: old,
                })
            }
            Op::SetClipColor { track, id, color } => {
                let t = track_mut(project, track)?;
                let old = t.clip(id).ok_or(ModelError::UnknownClip)?.color();
                t.set_clip_color(id, color)?;
                Ok(Op::SetClipColor {
                    track,
                    id,
                    color: old,
                })
            }
            Op::InsertNote { track, clip, note } => {
                track_mut(project, track)?.insert_note(clip, note.clone())?;
                Ok(Op::RemoveNote { track, clip, note })
            }
            Op::RemoveNote { track, clip, note } => {
                if !track_mut(project, track)?.remove_note(clip, &note)? {
                    return Err(ModelError::NoteNotFound);
                }
                Ok(Op::RestoreNote { track, clip, note })
            }
            Op::ReplaceNotes {
                track,
                clip,
                length,
                pairs,
            } => {
                let t = track_mut(project, track)?;
                let before = t.clip(clip).ok_or(ModelError::UnknownClip)?.length();
                t.replace_notes(clip, length, &pairs, false)?;
                let back = pairs
                    .into_iter()
                    .rev()
                    .map(|(from, to)| (to, from))
                    .collect();
                Ok(Op::RestoreNotes {
                    track,
                    clip,
                    length: before,
                    pairs: back,
                })
            }
            Op::RestoreNotes {
                track,
                clip,
                length,
                pairs,
            } => {
                let t = track_mut(project, track)?;
                let before = t.clip(clip).ok_or(ModelError::UnknownClip)?.length();
                t.replace_notes(clip, length, &pairs, true)?;
                let back = pairs
                    .into_iter()
                    .rev()
                    .map(|(from, to)| (to, from))
                    .collect();
                Ok(Op::ReplaceNotes {
                    track,
                    clip,
                    length: before,
                    pairs: back,
                })
            }
            Op::ReplaceNote {
                track,
                clip,
                from,
                to,
            } => {
                track_mut(project, track)?.replace_note(clip, &from, to.clone(), false)?;
                Ok(Op::UnreplaceNote {
                    track,
                    clip,
                    from: to,
                    to: from,
                })
            }
            Op::UnreplaceNote {
                track,
                clip,
                from,
                to,
            } => {
                track_mut(project, track)?.replace_note(clip, &from, to.clone(), true)?;
                Ok(Op::ReplaceNote {
                    track,
                    clip,
                    from: to,
                    to: from,
                })
            }
            Op::RestoreNote { track, clip, note } => {
                track_mut(project, track)?.restore_note(clip, note.clone())?;
                Ok(Op::RemoveNote { track, clip, note })
            }
            Op::SetTempo { at, tempo } => {
                let old = project
                    .tempo_map()
                    .changes()
                    .find(|&(t, _)| t == at)
                    .map(|(_, v)| v);
                match tempo {
                    Some(tempo) => project.tempo_map_mut().set(at, tempo),
                    None if project.tempo_map_mut().remove(at)? => {}
                    None => return Err(ModelError::NoChangeAt(at.get())),
                }
                Ok(Op::SetTempo { at, tempo: old })
            }
            Op::SetMeter { at, signature } => {
                let old = project
                    .meter_map()
                    .changes()
                    .find(|&(t, _)| t == at)
                    .map(|(_, v)| v);
                match signature {
                    Some(signature) => project.meter_map_mut().set(at, signature),
                    None if project.meter_map_mut().remove(at)? => {}
                    None => return Err(ModelError::NoChangeAt(at.get())),
                }
                Ok(Op::SetMeter { at, signature: old })
            }
            Op::SetKey { key, extra } => {
                let old = project.key();
                let old_extra = project.key_extra().clone();
                project.set_key(key);
                match extra {
                    Some(extra) => project.set_key_extra(*extra),
                    None if key != old => project.set_key_extra(JsonMap::new()),
                    None => {}
                }
                Ok(Op::SetKey {
                    key: old,
                    extra: Some(Box::new(old_extra)),
                })
            }
            Op::SetPort { track, port, extra } => {
                let t = track_mut(project, track)?;
                let old = t.port().clone();
                let old_extra = t.port_extra().clone();
                let changed = port != old;
                t.set_port(port);
                match extra {
                    Some(extra) => t.set_port_extra(*extra),
                    None if changed => t.set_port_extra(JsonMap::new()),
                    None => {}
                }
                Ok(Op::SetPort {
                    track,
                    port: old,
                    extra: Some(Box::new(old_extra)),
                })
            }
        }
    }

    /// 히스토리 메모리 상한 계산용 추정 크기(바이트). 힙에 있는 노트·문자열을 더한다.
    pub(crate) fn approx_bytes(&self) -> usize {
        let heap = match self {
            Op::InsertTrack { track, .. } => track_bytes(track),
            Op::InsertClip { clip, .. }
            | Op::InsertClipWithId { clip, .. }
            | Op::UnsplitClip { original: clip, .. } => clip_bytes(clip),
            Op::SetTrackProp {
                prop: TrackProp::Name(s),
                ..
            }
            | Op::SetTrackProp {
                prop: TrackProp::LyricLang(Some(s)),
                ..
            }
            | Op::SetTrackProp {
                prop: TrackProp::Port(OutputPort::Named(s)),
                ..
            }
            | Op::SetClipName { name: Some(s), .. } => s.len(),
            Op::SetPort { port, extra, .. } => {
                let name = match port {
                    OutputPort::Named(s) => s.len(),
                    OutputPort::Virtual => 0,
                };
                name + extra.as_deref().map_or(0, map_bytes)
            }
            Op::SetKey { extra, .. } => extra.as_deref().map_or(0, map_bytes),
            Op::InsertNote { note, .. }
            | Op::RemoveNote { note, .. }
            | Op::RestoreNote { note, .. } => note_heap_bytes(note),
            Op::ReplaceNote { from, to, .. } | Op::UnreplaceNote { from, to, .. } => {
                note_heap_bytes(from) + note_heap_bytes(to)
            }
            Op::ReplaceNotes { pairs, .. } | Op::RestoreNotes { pairs, .. } => pairs
                .iter()
                .map(|(from, to)| {
                    2 * std::mem::size_of::<Note>() + note_heap_bytes(from) + note_heap_bytes(to)
                })
                .sum(),
            _ => 0,
        };
        std::mem::size_of::<Op>() + heap
    }
}

fn track_mut(project: &mut Project, id: TrackId) -> Result<&mut Track, ModelError> {
    project.track_by_id_mut(id).ok_or(ModelError::UnknownTrack)
}

/// 속성을 바꾸고 이전 값을 돌려준다.
fn set_track_prop(t: &mut Track, prop: TrackProp) -> TrackProp {
    match prop {
        TrackProp::Name(v) => {
            let old = t.name().to_owned();
            t.set_name(v);
            TrackProp::Name(old)
        }
        TrackProp::Color(v) => {
            let old = t.color();
            t.set_color(v);
            TrackProp::Color(old)
        }
        TrackProp::Port(v) => {
            let old = t.port().clone();
            t.set_port(v);
            TrackProp::Port(old)
        }
        TrackProp::Channel(v) => {
            let old = t.channel();
            t.set_channel(v);
            TrackProp::Channel(old)
        }
        TrackProp::Bank(v) => {
            let old = t.bank();
            t.set_bank(v);
            TrackProp::Bank(old)
        }
        TrackProp::Program(v) => {
            let old = t.program();
            t.set_program(v);
            TrackProp::Program(old)
        }
        TrackProp::DefaultVelocity(v) => {
            let old = t.default_velocity();
            t.set_default_velocity(v);
            TrackProp::DefaultVelocity(old)
        }
        TrackProp::Mute(v) => {
            let old = t.mute();
            t.set_mute(v);
            TrackProp::Mute(old)
        }
        TrackProp::Solo(v) => {
            let old = t.solo();
            t.set_solo(v);
            TrackProp::Solo(old)
        }
        TrackProp::LyricLang(v) => {
            let old = t.lyric_lang().map(str::to_owned);
            t.set_lyric_lang(v);
            TrackProp::LyricLang(old)
        }
    }
}

fn note_heap_bytes(note: &Note) -> usize {
    note.lyric().map_or(0, str::len) + note.extra().map_or(0, str::len)
}

/// `.ksq` 모르는 키의 추정 크기. 메모리 상한이 이것을 세지 않으면 큰 모르는 키가 든 클립을
/// 분할·삭제할 때 히스토리가 상한을 넘는다(S0-T6 리뷰 2 지적 2). 직렬화 길이로 어림한다.
fn map_bytes(map: &serde_json::Map<String, serde_json::Value>) -> usize {
    if map.is_empty() {
        0
    } else {
        serde_json::to_string(map).map_or(0, |s| s.len())
    }
}

fn clip_bytes(clip: &Clip) -> usize {
    std::mem::size_of::<Clip>()
        + clip.name().map_or(0, str::len)
        + map_bytes(clip.extra())
        + clip
            .notes()
            .iter()
            .map(|n| std::mem::size_of::<Note>() + note_heap_bytes(n))
            .sum::<usize>()
}

fn track_bytes(track: &Track) -> usize {
    std::mem::size_of::<Track>()
        + track.name().len()
        + map_bytes(track.extra())
        + map_bytes(track.port_extra())
        + track.clips().map(|(_, c)| clip_bytes(c)).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pitch;

    fn note(start: u64) -> Note {
        Note::new(
            Tick::new(start),
            10,
            Pitch::new(60).unwrap(),
            Velocity::new(1000),
        )
        .unwrap()
    }

    fn new_track(name: &str, channel: u8) -> Track {
        Track::new(
            name,
            Channel::new(channel).unwrap(),
            TrackColor::new(0).unwrap(),
        )
    }

    /// 트랙 하나, 클립 둘(0..960, 1920..2880), 첫 클립에 노트 하나.
    fn fixture() -> (Project, TrackId, ClipId, ClipId) {
        let mut p = Project::new();
        let track = new_track("Lead", 1);
        let tid = track.id();
        p.push_track(track).unwrap();
        let t = p.track_by_id_mut(tid).unwrap();
        let a = t.insert_clip(Clip::new(Tick::ZERO, 960).unwrap()).unwrap();
        let b = t
            .insert_clip(Clip::new(Tick::new(1920), 960).unwrap())
            .unwrap();
        t.insert_note(a, note(0)).unwrap();
        (p, tid, a, b)
    }

    /// apply → 역연산 apply → 원래 상태. 그리고 역의 역을 다시 적용하면 apply 직후 상태.
    fn round_trip(p: &mut Project, op: Op) {
        let before = p.clone();
        let inverse = op.apply(p).expect("연산 적용");
        let after = p.clone();
        let again = inverse.apply(p).expect("역연산 적용");
        assert_eq!(*p, before, "역연산이 원래 상태로 돌리지 못했다");
        again.apply(p).expect("역의 역 적용");
        assert_eq!(*p, after, "역의 역이 적용 직후 상태와 다르다");
    }

    #[test]
    fn edit_apply_returns_inverse_for_each_variant() {
        let (mut p, t, a, b) = fixture();
        let edits = vec![
            Edit::InsertTrack {
                index: 1,
                track: Box::new(new_track("Bass", 2)),
            },
            Edit::SetTrackProp {
                track: t,
                prop: TrackProp::Name("Lead 2".into()),
            },
            Edit::SetTrackProp {
                track: t,
                prop: TrackProp::Channel(Channel::new(3).unwrap()),
            },
            Edit::SetTrackProp {
                track: t,
                prop: TrackProp::Mute(true),
            },
            Edit::SetTrackProp {
                track: t,
                prop: TrackProp::LyricLang(Some("ko".into())),
            },
            Edit::InsertClip {
                track: t,
                clip: Box::new(Clip::new(Tick::new(4000), 10).unwrap()),
            },
            Edit::MoveClip {
                track: t,
                id: b,
                to: Tick::new(5000),
            },
            Edit::SplitClip {
                track: t,
                id: a,
                at: Tick::new(480),
            },
            Edit::SetClipName {
                track: t,
                id: a,
                name: Some("후렴".into()),
            },
            Edit::SetClipColor {
                track: t,
                id: a,
                color: Some(TrackColor::new(5).unwrap()),
            },
            Edit::InsertNote {
                track: t,
                clip: a,
                note: note(100),
            },
            Edit::RemoveNote {
                track: t,
                clip: a,
                note: note(0),
            },
            Edit::RemoveClip { track: t, id: b },
            Edit::SetTempo {
                at: Tick::new(960),
                tempo: Some(Tempo::from_bpm(90).unwrap()),
            },
            Edit::SetTempo {
                at: Tick::ZERO,
                tempo: Some(Tempo::from_bpm(100).unwrap()),
            },
            Edit::SetMeter {
                at: Tick::new(3840),
                signature: Some(TimeSignature::new(3, 4).unwrap()),
            },
            Edit::SetKey(Key::Minor(crate::PitchClass::new(9).unwrap())),
        ];
        for edit in edits {
            round_trip(&mut p, edit.into());
        }
        p.push_track(new_track("Pad", 4)).unwrap();
        round_trip(&mut p, Op::RemoveTrack { track: t });
    }

    #[test]
    fn edit_apply_failure_leaves_project_unchanged() {
        let (mut p, t, a, _) = fixture();
        let before = p.clone();
        let stranger = new_track("X", 9).id();
        let failing = vec![
            Edit::RemoveTrack { track: stranger },
            Edit::InsertTrack {
                index: 5,
                track: Box::new(new_track("Y", 9)),
            },
            Edit::InsertTrack {
                index: 0,
                track: Box::new(p.tracks()[0].clone()),
            },
            Edit::SetTrackProp {
                track: stranger,
                prop: TrackProp::Solo(true),
            },
            Edit::MoveClip {
                track: t,
                id: a,
                to: Tick::new(1900),
            },
            Edit::InsertClip {
                track: t,
                clip: Box::new(Clip::new(Tick::new(100), 10).unwrap()),
            },
            Edit::SplitClip {
                track: t,
                id: a,
                at: Tick::new(960),
            },
            Edit::RemoveNote {
                track: t,
                clip: a,
                note: note(5),
            },
            Edit::InsertNote {
                track: t,
                clip: a,
                note: note(955),
            },
            Edit::SetTempo {
                at: Tick::ZERO,
                tempo: None,
            },
            Edit::SetMeter {
                at: Tick::ZERO,
                signature: None,
            },
            Edit::SetTempo {
                at: Tick::new(7),
                tempo: None,
            },
        ];
        for edit in failing {
            let shown = format!("{edit:?}");
            assert!(
                Op::from(edit).apply(&mut p).is_err(),
                "실패해야 한다: {shown}"
            );
            assert_eq!(p, before, "실패한 편집이 상태를 바꿨다: {shown}");
        }
    }

    #[test]
    fn edit_split_inverse_keeps_right_id_for_redo() {
        let (mut p, t, a, _) = fixture();
        let inverse = Op::SplitClip {
            track: t,
            id: a,
            at: Tick::new(480),
        }
        .apply(&mut p)
        .unwrap();
        let Op::UnsplitClip { right, .. } = inverse.clone() else {
            panic!("{inverse:?}")
        };
        let redo = inverse.apply(&mut p).unwrap();
        assert!(matches!(redo, Op::SplitClipWithId { right: r, .. } if r == right));
        redo.apply(&mut p).unwrap();
        assert!(
            p.track(t).unwrap().clip(right).is_some(),
            "redo가 같은 오른쪽 id를 썼다"
        );
    }

    #[test]
    fn unsplit_rejects_clips_that_were_not_split_from_original() {
        // 역연산은 히스토리 안에서만 만들어지지만, 상태가 어긋나도 모델을 깨뜨리지 않아야 한다
        // (S0-T4 리뷰 1 지적 3).
        let (mut p, t, a, b) = fixture();
        let before = p.clone();
        let original = Box::new(Clip::new(Tick::ZERO, 2880).unwrap());
        let forged = Op::UnsplitClip {
            track: t,
            left: a,
            right: b,
            original,
        };
        assert!(
            forged.apply(&mut p).is_err(),
            "맞붙지 않은 두 클립은 되돌릴 분할이 아니다"
        );
        assert_eq!(p, before);
    }

    #[test]
    fn replace_notes_judges_tails_by_length_before_the_edit() {
        // 왼쪽 클립 0..960(분할 전 0..1920)에 꼬리 노트 900..1100. 오른쪽 클립은 지운다.
        let mut q = Project::new();
        let mut tr = new_track("Q", 5);
        let qt = tr.id();
        let qc = tr
            .insert_clip(Clip::new(Tick::ZERO, 1920).unwrap())
            .unwrap();
        let tail = Note::new(
            Tick::new(900),
            200,
            Pitch::new(60).unwrap(),
            Velocity::new(1000),
        )
        .unwrap();
        tr.insert_note(qc, tail.clone()).unwrap();
        let right = tr.split_clip(qc, Tick::new(960)).unwrap();
        tr.remove_clip(right);
        q.push_track(tr).unwrap();
        // 꼬리를 3700으로 옮기며 3840까지 늘린다. 늘린 길이로 보면 원래 노트(끝 1100)는 꼬리가 아니지만,
        // 편집 직전 길이(960)로 판정하므로 새 노트(끝 4000 > 3840)는 꼬리로 남는다.
        let far = Note::new(
            Tick::new(3700),
            300,
            Pitch::new(60).unwrap(),
            Velocity::new(1000),
        )
        .unwrap();
        round_trip(
            &mut q,
            Op::ReplaceNotes {
                track: qt,
                clip: qc,
                length: 3840,
                pairs: vec![(tail, far)],
            },
        );
        let clip = q.track(qt).unwrap().clip(qc).unwrap();
        assert_eq!((clip.length(), clip.notes()[0].end()), (3840, 4000));

        // 꼬리가 아니던 노트는 새 길이를 넘을 수 없다. 줄이기와 이웃 겹침도 거부한다.
        let (mut p, t, a, _) = fixture();
        let before = p.clone();
        let over = Op::ReplaceNotes {
            track: t,
            clip: a,
            length: 960,
            pairs: vec![(note(0), note(955))],
        };
        assert!(matches!(
            over.apply(&mut p),
            Err(ModelError::NoteOutsideClip { .. })
        ));
        let shrink = Op::ReplaceNotes {
            track: t,
            clip: a,
            length: 100,
            pairs: vec![],
        };
        assert!(matches!(
            shrink.apply(&mut p),
            Err(ModelError::ClipWouldShrink { .. })
        ));
        let into_b = Op::ReplaceNotes {
            track: t,
            clip: a,
            length: 1921,
            pairs: vec![],
        };
        assert!(matches!(
            into_b.apply(&mut p),
            Err(ModelError::ClipOverlap { .. })
        ));
        assert_eq!(p, before);
    }

    #[test]
    fn replace_note_keeps_tails_only_from_tails() {
        let (mut p, t, a, _) = fixture();
        // 꼬리가 아닌 노트를 꼬리로 바꿀 수 없다(분할 없이 꼬리가 생기는 경로가 된다).
        let before = p.clone();
        let into_tail = Op::ReplaceNote {
            track: t,
            clip: a,
            from: note(0),
            to: note(955),
        };
        assert!(matches!(
            into_tail.apply(&mut p),
            Err(ModelError::NoteOutsideClip { .. })
        ));
        assert_eq!(p, before);
        // 클립 안의 이동은 왕복한다.
        round_trip(
            &mut p,
            Op::ReplaceNote {
                track: t,
                clip: a,
                from: note(0),
                to: note(100),
            },
        );
        // 분할로 꼬리를 만든 뒤에는 꼬리 노트를 꼬리로 옮길 수 있고, undo로 돌아온다.
        let tail = Note::new(
            Tick::new(900),
            200,
            Pitch::new(60).unwrap(),
            Velocity::new(1000),
        )
        .unwrap();
        p.track_by_id_mut(t)
            .unwrap()
            .insert_note(a, note(800))
            .unwrap();
        let mut q = Project::new();
        let mut tr = new_track("Q", 5);
        let qt = tr.id();
        let qc = tr
            .insert_clip(Clip::new(Tick::ZERO, 1920).unwrap())
            .unwrap();
        tr.insert_note(qc, tail.clone()).unwrap();
        tr.split_clip(qc, Tick::new(960)).unwrap();
        q.push_track(tr).unwrap();
        let moved = Note::new(
            Tick::new(910),
            200,
            Pitch::new(62).unwrap(),
            Velocity::new(1000),
        )
        .unwrap();
        round_trip(
            &mut q,
            Op::ReplaceNote {
                track: qt,
                clip: qc,
                from: tail,
                to: moved,
            },
        );
    }

    #[test]
    fn extend_clip_edit_round_trip_and_rejects_shrink() {
        let (mut p, t, a, b) = fixture();
        round_trip(
            &mut p,
            Op::ExtendClip {
                track: t,
                id: a,
                length: 1920,
            },
        );
        assert_eq!(p.track(t).unwrap().clip(a).unwrap().length(), 1920);
        let before = p.clone();
        // 줄이기 거부, 다음 클립(b, 1920부터)과 겹치기 거부.
        assert_eq!(
            Op::ExtendClip {
                track: t,
                id: a,
                length: 100
            }
            .apply(&mut p)
            .unwrap_err(),
            ModelError::ClipWouldShrink {
                current: 1920,
                requested: 100
            }
        );
        assert!(matches!(
            Op::ExtendClip {
                track: t,
                id: a,
                length: 1921
            }
            .apply(&mut p),
            Err(ModelError::ClipOverlap { .. })
        ));
        assert_eq!(
            Op::ExtendClip {
                track: t,
                id: b,
                length: 0
            }
            .apply(&mut p)
            .unwrap_err(),
            ModelError::ClipWouldShrink {
                current: 960,
                requested: 0
            }
        );
        assert_eq!(p, before);
    }

    #[test]
    fn edit_approx_bytes_grows_with_notes() {
        let (_, t, ..) = fixture();
        let mut clip = Clip::new(Tick::ZERO, 100_000).unwrap();
        let small = Op::InsertClip {
            track: t,
            clip: Box::new(clip.clone()),
        }
        .approx_bytes();
        for i in 0..1000 {
            clip.insert_note(note(i * 10)).unwrap();
        }
        let big = Op::InsertClip {
            track: t,
            clip: Box::new(clip),
        }
        .approx_bytes();
        assert!(
            big >= small + 1000 * std::mem::size_of::<Note>(),
            "{small} {big}"
        );
    }
}
