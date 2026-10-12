# Task 계획: S0-T7 퀀타이즈·조옮김·길이 스케일 연산

- 브랜치: `s0/t7-edit-ops`
- 관련 REQ: 6.2 EDT-2·EDT-3, 5.10(클립 규칙), 2(녹음 후 자동 퀀타이즈), 5.12(옥타브 접기)
- 선행 Task: S0-T2, S0-T3, S0-T4
- 관련 결정: `docs/decisions/2026-10-12-quantize-and-transform-rules.md`

## 끝의 정의 (내 말로)

- 선택한 노트 집합(트랙·클립·노트 값)에 대해 퀀타이즈, 조옮김, 시간 이동, 길이 스케일을 계산하는 함수가 있다. 함수는 프로젝트를 바꾸지 않고 `Edit` 묶음을 돌려준다. 호출자가 `Document::apply`로 한 undo 단위로 적용한다.
- 퀀타이즈(EDT-2): 그리드 1/4·1/8·1/16·1/32·1/64, 셋잇단(그리드 × 2/3), 강도 0–100%, 스윙 0–75%. 시작만 옮기고 길이는 유지한다. 그리드는 노트의 절대 위치가 속한 마디의 시작부터 잰다.
- 조옮김(EDT-3): ±반음. 결과가 음역(기본 0–127, 호출자가 좁힐 수 있음) 밖이면 옥타브 단위로 접어 음역 안에 넣는다. 음역 안이면 정확히 ±n이다.
- 시간 이동(EDT-3): ±틱. 노트는 자기 클립 안에서만 움직인다.
- 길이 스케일(EDT-3): 비율 `분자/분모`. 선택의 가장 이른 절대 시작을 기준으로 시작 간격과 길이를 함께 늘이고 줄인다. 길이는 최소 1틱이다.
- 클립 경계: 결과 노트의 끝이 클립 길이를 넘으면 클립을 마디 단위로 늘린다(`Edit::ExtendClip`). 늘린 클립이 다음 클립과 겹치거나, 노트가 클립 시작 앞으로 나가면 연산 전체가 `Err`이다.
- `Edit::ExtendClip`을 새로 둔다. 늘리기만 받는다. undo가 쓰는 줄이기는 crate 내부 `Op`에만 있다(비파괴 트림은 S4-T2).

원문과 다른 점: 없음. 「선택 집합에 대한 일괄 연산」은 노트 참조 목록으로 표현한다. 선택 UI는 S3이다.

## 해석(REQ에 식이 없어 업계 관례를 따름)

- 스윙: 그리드 칸을 0부터 셀 때 홀수 칸의 위치를 `그리드 × 스윙%`만큼 뒤로 민다. 0%는 스윙 없음이다. 최대 75%라 밀린 점이 다음 칸을 넘지 않는다. 노트는 「스윙이 적용된 그리드 점」 중 가장 가까운 점으로 간다(같으면 앞쪽).
- 강도: `새 시작 = 원래 + (목표 − 원래) × 강도 / 100`. 0으로 향해 자른다. 강도 100%면 목표 그대로라 멱등이다.
- 마디 끝 처리: 마디 길이가 그리드의 배수가 아니면(예: 7/8에 4분음표 그리드), 그 마디의 그리드 점은 다음 마디 시작 전까지이고, 다음 마디 시작도 후보다.
- 옥타브 접기: 음역은 최소 한 옥타브(12반음) 이상이어야 한다. 그래야 항상 접을 수 있다. 피치 오프셋이 있는 노트는 「음높이 + 오프셋」이 MIDI 2.0 범위(결정 B)에 들도록 접는다.
- 숨김 꼬리(분할로 생긴, 끝 > 클립 길이) 노트를 옮기면, 옮긴 결과의 끝까지 클립을 늘린다. 꼬리가 드러난다.

## 공개 인터페이스 초안

~~~rust
// transform.rs
pub struct NoteRef { pub track: TrackId, pub clip: ClipId, pub note: Note }   // 선택 항목(값 그대로)
pub enum Grid { Quarter, Eighth, Sixteenth, ThirtySecond, SixtyFourth }
pub struct QuantizeSettings { grid: Grid, triplet: bool, strength: u8, swing: u8 }
impl QuantizeSettings { pub fn new(grid, triplet, strength, swing) -> Result<Self, ModelError>; pub fn grid_ticks(&self) -> u64; }
pub struct PitchRange { low: Pitch, high: Pitch }   // high − low ≥ 11
impl PitchRange { pub const FULL; pub fn new(low, high) -> Result<Self, ModelError>; }
pub fn quantize(project: &Project, notes: &[NoteRef], settings: &QuantizeSettings) -> Result<Vec<Edit>, ModelError>;
pub fn transpose(project: &Project, notes: &[NoteRef], semitones: i32, range: PitchRange) -> Result<Vec<Edit>, ModelError>;
pub fn shift(project: &Project, notes: &[NoteRef], delta_ticks: i64) -> Result<Vec<Edit>, ModelError>;
pub fn scale(project: &Project, notes: &[NoteRef], numerator: u32, denominator: u32) -> Result<Vec<Edit>, ModelError>;
// edit.rs: Edit::ExtendClip { track: TrackId, id: ClipId, length: u64 }   // 늘리기만. 내부 Op::ShrinkClip
~~~

`NoteRef`는 공개 필드를 갖는다. 모델 불변식을 담는 타입이 아니라 「무엇을 고를지」를 담는 값이고, 잘못된 참조는 `Edit` 적용에서 거부된다(`NoteNotFound` 등).

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `quantize_full_strength_is_idempotent` | 속성 | 강도 100%를 두 번 = 한 번(게이트) |
| 2 | `quantize_swing_zero_equals_plain_grid` | 속성 | 스윙 0 = 그리드(게이트) |
| 3 | `quantize_snaps_to_bar_relative_grid` | 단위 | 클립이 박에서 어긋나도 마디 그리드, 7/8 마디 끝, 박자표 변화 |
| 4 | `quantize_strength_and_swing_values` | 단위 | 강도 50%, 스윙 50%·75% 수치 |
| 5 | `quantize_triplet_grid` | 단위 | 셋잇단 그리드 |
| 6 | `transpose_by_octave_inside_range_is_exact` | 속성 | ±12 음역 안이면 정확히 ±12(게이트) |
| 7 | `transpose_folds_into_range` | 단위 | 음역 밖은 옥타브 접기, 좁은 음역, 오프셋 노트 |
| 8 | `shift_past_end_extends_clip_by_bars` / `shift_before_start_is_rejected` | 단위 | 결정의 경계 규칙 |
| 9 | `extend_into_next_clip_is_rejected` | 단위 | 겹치면 연산 전체 거부 |
| 10 | `scale_doubles_spacing_and_length` | 단위 | ×2, ×1/2, 기준점, 최소 길이 1 |
| 11 | `transform_edits_apply_as_one_undo_unit` | 속성 | 임의 연산 → `Document::apply` → undo 한 번 = 이전 상태, 불변식 유지 |
| 12 | `extend_clip_edit_round_trip_and_rejects_shrink` | 단위 | `Edit::ExtendClip`과 역연산 |

## 불변식

- 연산 함수는 프로젝트를 바꾸지 않는다. 결과 `Edit`은 모델 검사를 거쳐 적용된다.
- 연산 결과는 클립 겹침·노트 범위·절대 피치 불변식을 지킨다. 지킬 수 없으면 `Err`이고 `Edit`을 하나도 돌려주지 않는다.

## 범위 밖

- 퀀타이즈 설정 UI·`q` 키(S3), 녹음 후 자동 퀀타이즈의 호출(S5), 트랙 음역 설정(S6·S7에서 필요할 때), 휴머나이즈(EDT-4, C).
- 클립 줄이기·비파괴 트림(S4-T2).
