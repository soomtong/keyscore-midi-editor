# Task 계획: S0-T2 핵심 모델 타입

- 브랜치: `s0/t2-core-model`
- 관련 REQ: 2(조성 결정 행, 트랙 색 12색), 3(용어), 5.10(클립 규칙), 5.15(LYR-1 모델 부분), 6.1 SEQ-1·SEQ-2·SEQ-3·SEQ-4·SEQ-7
- 선행 Task: S0-T1
- 관련 결정: `docs/decisions/2026-10-12-clip-split-and-velocity.md`(분할 시 숨김 꼬리, 벨로시티 0–127), `docs/decisions/2026-10-11-license-allowlist.md`(thiserror의 Unicode-3.0)

## 끝의 정의 (내 말로)

`keyscore-core`에 곡을 담는 값 타입이 있고, 공개 API만으로는 다음을 깨뜨릴 수 없다.

1. 같은 트랙의 두 클립 구간 `[start, start+length)`가 겹치지 않는다.
2. 클립 안 모든 노트의 시작은 `0 ≤ start < 클립 길이`다. `insert_note`로 넣는 노트는 끝(`start+length`)까지 클립 안이어야 한다. 끝이 클립 길이를 넘는 노트(숨김 꼬리)는 분할로만 생긴다(결정 Q1).
3. 채널은 1–16, 음높이는 0–127, 피치 오프셋은 ±1옥타브, v1 벨로시티는 0–127, 노트 길이는 1틱 이상, 클립 길이는 1틱 이상이다. 틱 계산은 u64 오버플로 없이 검사된다.
4. 실패한 연산은 상태를 바꾸지 않는다(`Err`이면 호출 전과 같음).

클립 연산은 삽입·삭제·이동·분할까지 만든다. 원문과 다른 점은 아래와 같다.

- 「노트 시작은 클립 기준」은 그대로다. 「클립 길이 밖이면 `Err`」은 `insert_note`에만 적용한다. 분할로 생긴 꼬리는 결정 Q1에 따라 허용한다.
- 템포·박자표 맵(SEQ-1의 나머지)은 S0-T3이 `Project`에 더한다.
- 새 트랙 채널 자동 배정(SEQ-7 후반)은 S4-T3의 범위다(그 Task의 게이트에 있다).

## 공개 인터페이스 초안

~~~rust
// tick.rs
pub const PPQ: u32 = 960;
pub struct Tick(u64);                 // Copy, Ord. new(u64), get(), checked_add
// note.rs
pub struct Pitch(u8);                 // 0..=127. new(u8) -> Result<Self, ModelError>
pub struct PitchOffset(i16);          // 센트, -1200..=1200. ZERO, new(i16) -> Result
pub struct Velocity(u16);             // v1: 0..=127. new(u16) -> Result, get()
pub struct Note { start: Tick, length: u32, pitch: Pitch, offset: PitchOffset,
                  velocity: Velocity, lyric: Option<String> }
impl Note {
    pub fn new(start: Tick, length: u32, pitch: Pitch, velocity: Velocity) -> Result<Self, ModelError>;
    pub fn with_offset(self, offset: PitchOffset) -> Self;
    pub fn with_lyric(self, lyric: impl Into<String>) -> Self;   // 빈 문자열은 None
    pub fn start(&self) -> Tick; pub fn length(&self) -> u32; pub fn end(&self) -> u64;
    pub fn pitch(&self) -> Pitch; pub fn offset(&self) -> PitchOffset;
    pub fn velocity(&self) -> Velocity; pub fn lyric(&self) -> Option<&str>;
}
pub struct CcPoint { tick: Tick, value: u32 }   // SEQ-4, 32비트 보관
// clip.rs
pub struct ClipId(u64);
pub struct Clip { start: Tick, length: u64, name: Option<String>,
                  color: Option<TrackColor>, notes: Vec<Note> }   // notes: (start, pitch) 정렬
impl Clip {
    pub fn new(start: Tick, length: u64) -> Result<Self, ModelError>;   // length ≥ 1, 끝 오버플로 없음
    pub fn start(&self) -> Tick; pub fn length(&self) -> u64; pub fn end(&self) -> u64;
    pub fn notes(&self) -> &[Note];
    pub fn insert_note(&mut self, note: Note) -> Result<(), ModelError>;   // 끝까지 클립 안
    pub fn remove_note(&mut self, note: &Note) -> bool;                    // 같은 값 하나 제거
    pub fn name/set_name, color/set_color;
}
// track.rs
pub struct Channel(u8);               // 1..=16. new(u8) -> Result, is_drum() (= 10)
pub struct TrackColor(u8);            // 고정 12색 팔레트 인덱스 0..12
pub struct Program(u8);               // 0..=127
pub struct Bank(u16);                 // 0..=16383 (14비트 MSB/LSB)
pub enum OutputPort { Virtual, Named(String) }
pub struct Track { name, color, port, channel, bank: Option<Bank>, program: Option<Program>,
                   default_velocity: Velocity, mute: bool, solo: bool,
                   lyric_lang: Option<String>, clips: Vec<(ClipId, Clip)>, next_clip_id: u64 }
impl Track {
    pub fn new(name: impl Into<String>, channel: Channel, color: TrackColor) -> Self;
    // 속성 getter·setter (불변식이 없는 속성만 setter)
    pub fn clips(&self) -> impl Iterator<Item = (ClipId, &Clip)>;        // 시작 순
    pub fn clip(&self, id: ClipId) -> Option<&Clip>;
    pub fn insert_note(&mut self, id: ClipId, note: Note) -> Result<(), ModelError>;
    pub fn remove_note(&mut self, id: ClipId, note: &Note) -> Result<bool, ModelError>;
    pub fn insert_clip(&mut self, clip: Clip) -> Result<ClipId, ModelError>;   // 겹치면 Err
    pub fn remove_clip(&mut self, id: ClipId) -> Option<Clip>;
    pub fn move_clip(&mut self, id: ClipId, new_start: Tick) -> Result<(), ModelError>;
    pub fn split_clip(&mut self, id: ClipId, at: Tick) -> Result<ClipId, ModelError>; // start < at < end
}
// key.rs
pub struct PitchClass(u8);            // 0..12
pub enum Key { None, Major(PitchClass), Minor(PitchClass) }
// project.rs
pub struct Project { key: Key, tracks: Vec<Track> }
impl Project {
    pub fn new() -> Self; pub fn key(&self) -> Key; pub fn set_key(&mut self, key: Key);
    pub fn tracks(&self) -> &[Track]; pub fn track_mut(&mut self, index: usize) -> Option<&mut Track>;
    pub fn push_track(&mut self, track: Track); pub fn remove_track(&mut self, index: usize) -> Option<Track>;
}
// error.rs
#[derive(thiserror::Error)] pub enum ModelError { ... }
~~~

`Track`이 `&mut Clip`을 내주면 클립 길이·시작을 바꿔 겹침을 만들 수 있다. 그래서 `Track`은 `&mut Clip`을 내주지 않고, 트랙 안 클립의 노트는 `Track::insert_note`/`remove_note`로만 고친다. 시작·길이는 `Track`의 메서드로만 바뀐다. `Clip`에는 시작·길이 setter가 없다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `channel_new_zero_or_seventeen_is_rejected` | 단위 | 채널 0·17 생성 불가, 1·16 가능 |
| 2 | `channel_ten_is_drum` | 단위 | 10만 드럼 |
| 3 | `pitch_new_above_127_is_rejected` | 단위 | 음높이 범위 |
| 4 | `pitch_offset_beyond_one_octave_is_rejected` | 단위 | ±1200 센트 |
| 5 | `velocity_new_above_127_is_rejected_in_v1` | 단위 | 결정 Q2 |
| 6 | `note_new_zero_length_is_rejected` | 단위 | 노트 길이 ≥ 1 |
| 7 | `note_with_empty_lyric_has_no_lyric` | 단위 | LYR-1 「비어 있으면 가사 없음」 |
| 8 | `clip_new_zero_length_or_overflowing_end_is_rejected` | 단위 | 클립 길이·오버플로 |
| 9 | `clip_insert_note_past_end_is_rejected` | 단위 | 노트 끝이 클립 밖이면 `Err`, 상태 불변 |
| 10 | `clip_insert_note_keeps_notes_sorted` | 단위 | `(start, pitch)` 정렬 |
| 11 | `track_insert_overlapping_clip_is_rejected` | 단위 | 겹침 `Err`, 맞닿음(`end == start`)은 허용 |
| 12 | `track_move_clip_onto_neighbor_is_rejected_and_state_unchanged` | 단위 | 이동 실패 시 불변 |
| 13 | `track_split_clip_keeps_crossing_note_in_left_as_tail` | 단위 | 결정 Q1 |
| 14 | `track_split_clip_at_boundary_is_rejected` | 단위 | `at == start` 또는 `at == end` |
| 15 | `track_ops_random_sequence_preserve_invariants` | 속성 | 삽입·삭제·이동·분할·노트 삽입·노트 삭제 수열 뒤 불변식 1·2 |
| 16 | `track_ops_failed_op_leaves_track_unchanged` | 속성 | 불변식 4 |
| 17 | `track_split_clip_preserves_absolute_notes` | 속성 | 분할 전후 노트의 (절대 시작, 길이, 음높이, 벨로시티, 가사) 다중집합 같음 |
| 18 | `track_move_clip_moves_notes_with_it` | 속성 | 노트 상대 위치 유지(용어: 노트는 클립 기준) |

15·16은 이후 모든 단계의 회귀 테스트로 남긴다(단계 파일 요구).

## 불변식

- 위 끝의 정의 1~4.
- `pub` 필드 없음. 모든 값 타입은 생성자에서 검증한다. `Clip`의 시작·길이를 바꾸는 공개 API는 `Track`의 겹침 검사를 거치는 것만 있다.
- `unsafe` 없음(`#![deny(unsafe_code)]`).

## 범위 밖

- 템포·박자표 맵, 틱↔시각(S0-T3). undo(S0-T4). 직렬화·serde(S0-T6). 퀀타이즈·조옮김·클립 자동 연장(S0-T7).
- 비파괴 트림, 합치기, 복제, 트랙 간 이동(S4-T2). 새 트랙 채널 자동 배정·팔레트 순서(S4-T3). SEQ-8 경고(S4).
- 노트 외 이벤트(프로그램 체인지·피치 벤드)의 레인 구조. `CcPoint` 타입만 정의한다(S6).

## 리스크·결정 요청

- 피치 오프셋 「±1옥타브 소수」를 정수 센트(i16, ±1200)로 담는다. 9.5의 「모델이 정수만 쓴다」에 맞추기 위해서이며, 1센트 해상도는 v1이 송신하지 않는 값이라 충분하다고 본다. 리뷰에서 다른 해석이 나오면 결정 요청으로 돌린다.
- 같은 클립 안에서 같은 음높이의 노트가 겹치는 것은 막지 않는다(REQ에 규칙 없음). MIDI 1.0 송신 때 Note Off 짝 문제는 S1의 몫이다.
- 리뷰 초점에 추가(S0-T1 review-3 제안 1): `cargo tree -p keyscore-core --target all`로 `thiserror`·`proptest`의 전이 그래프를 확인한다.
