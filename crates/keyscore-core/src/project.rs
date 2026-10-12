use crate::{Key, Track};

/// 곡 하나(REQ SEQ-1). 템포 맵·박자표 맵은 S0-T3에서 더한다.
///
/// 트랙 수에 제한을 두지 않는다(SEQ-1, 실용 상한 64는 UI의 몫).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Project {
    key: Key,
    tracks: Vec<Track>,
}

impl Project {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn key(&self) -> Key {
        self.key
    }

    pub fn set_key(&mut self, key: Key) {
        self.key = key;
    }

    #[must_use]
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// 트랙은 자기 불변식을 스스로 지키므로 `&mut Track`을 내줘도 된다.
    pub fn track_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
    }

    pub fn push_track(&mut self, track: Track) {
        self.tracks.push(track);
    }

    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        (index < self.tracks.len()).then(|| self.tracks.remove(index))
    }
}
