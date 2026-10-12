use crate::{Key, MeterMap, ModelError, TempoMap, Track};

/// 곡 하나(REQ SEQ-1): 템포 맵 + 박자표 맵 + 트랙 목록, 그리고 전역 조성.
///
/// 새 프로젝트는 120 BPM, 4/4, 조성 없음이다(REQ 2 「새 프로젝트 기본값」의 모델 부분.
/// 기본 트랙 2개는 앱이 만든다).
///
/// 트랙 수에 제한을 두지 않는다(SEQ-1, 실용 상한 64는 UI의 몫).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Project {
    tempo_map: TempoMap,
    meter_map: MeterMap,
    key: Key,
    tracks: Vec<Track>,
}

impl Project {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn tempo_map(&self) -> &TempoMap {
        &self.tempo_map
    }

    /// 템포 맵은 스스로 불변식(틱 0 항목, 정렬, 캐시)을 지키므로 `&mut`를 내줘도 된다.
    pub fn tempo_map_mut(&mut self) -> &mut TempoMap {
        &mut self.tempo_map
    }

    #[must_use]
    pub fn meter_map(&self) -> &MeterMap {
        &self.meter_map
    }

    pub fn meter_map_mut(&mut self) -> &mut MeterMap {
        &mut self.meter_map
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

    /// `index` 자리에 트랙을 넣는다. `index == 트랙 수`이면 맨 뒤에 넣는다.
    ///
    /// # Errors
    ///
    /// `index`가 트랙 수보다 크면 [`ModelError::TrackIndexOutOfRange`].
    pub fn insert_track(&mut self, index: usize, track: Track) -> Result<(), ModelError> {
        if index > self.tracks.len() {
            return Err(ModelError::TrackIndexOutOfRange {
                index,
                len: self.tracks.len(),
            });
        }
        self.tracks.insert(index, track);
        Ok(())
    }

    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        (index < self.tracks.len()).then(|| self.tracks.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tempo, Tick, TimeSignature};

    #[test]
    fn project_new_has_default_tempo_and_meter() {
        let p = Project::new();
        let tempos: Vec<_> = p.tempo_map().changes().collect();
        assert_eq!(tempos, vec![(Tick::ZERO, Tempo::DEFAULT)]);
        assert_eq!(Tempo::DEFAULT.micros_per_quarter(), 500_000);
        let meters: Vec<_> = p.meter_map().changes().collect();
        assert_eq!(meters, vec![(Tick::ZERO, TimeSignature::COMMON)]);
        assert_eq!(p.key(), Key::None);
        assert!(p.tracks().is_empty());
    }
}
