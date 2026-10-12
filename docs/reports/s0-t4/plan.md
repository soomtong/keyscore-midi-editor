# Task 계획: S0-T4 undo 엔진

- 브랜치: `s0/t4-undo`
- 관련 REQ: 6.2 EDT-1, 2(Step·Sequence undo 단위 결정), 4 P6, 5.7(되돌리기 묶음), CTL-8, AI-5, MS-1
- 선행 Task: S0-T2, S0-T3
- 이월 조건: `plan/stages/10-S0-foundation.md` S0-T4 「이월」 — undo로 되살린 클립은 원래 `ClipId`를 유지한다.

## 끝의 정의 (내 말로)

- 모델을 바꾸는 모든 편집은 원시 편집 `Edit`의 열로 표현된다. `Edit`를 적용하면 그 역편집 `Edit`이 나온다. 적용은 항상 모델의 검사 API(겹침, 노트 범위, 값 범위)를 거친다. 그래서 undo·redo로도 불변식을 깨뜨릴 수 없다.
- `History`는 편집 묶음(트랜잭션)을 한 undo 단위로 적용한다. 묶음 안의 편집이 하나라도 실패하면 이미 적용한 것을 되돌려 프로젝트는 호출 전과 같다.
- undo는 역편집을 적용하고, 그 결과로 나온 역의 역(= 원래 편집)을 redo 스택에 쌓는다. redo도 대칭이다. 새 편집을 적용하면 redo 스택이 비워진다.
- 연속 같은 조작 묶기(coalesce): 호출자가 묶음 키를 주면, 직전 undo 단위가 같은 키이고 그 사이에 다른 단위·undo·redo·`seal()`이 없을 때 한 단위로 합쳐진다. undo 한 번에 모두 풀린다(EDT-1, 5.7 「`Alt+Shift+l` 다섯 번 → `Cmd+Z` 한 번」).
- 클립 식별자 보존: 클립 삽입·분할로 새로 생긴 `ClipId`는 편집에 기록되어, redo가 같은 id를 다시 쓴다. undo로 되살린 클립도 원래 id다.
- 히스토리는 개수 제한이 없고, 메모리 상한(추정 바이트)을 넘으면 가장 오래된 undo 단위부터 버린다.

원문과 다른 점: 없음. 「무제한」은 개수 무제한, 메모리 상한으로 해석한다(단계 파일의 괄호 그대로).

## 공개 인터페이스 초안

리뷰 1 뒤 개정했다. 처음 초안은 역편집 전용 변형과 `Edit::apply`를 공개했고, 트랙을 인덱스로 가리켰다(리뷰 1 지적 1–6).

~~~rust
// track.rs
pub struct TrackId(u64);              // 프로세스 안에서 유일. Track::id()
// project.rs: push_track/insert_track -> Result (같은 TrackId 거부), track(id), track_by_id_mut(id), track_index(id)
// edit.rs
pub enum TrackProp { Name(String), Color(TrackColor), Port(OutputPort), Channel(Channel),
                     Bank(Option<Bank>), Program(Option<Program>), DefaultVelocity(Velocity),
                     Mute(bool), Solo(bool), LyricLang(Option<String>) }
pub enum Edit {                        // 정방향 편집만. apply는 공개하지 않는다
    InsertTrack { index: usize, track: Box<Track> },
    RemoveTrack { track: TrackId },
    SetTrackProp { track: TrackId, prop: TrackProp },
    InsertClip { track: TrackId, clip: Box<Clip> },              // id는 새로 발급
    RemoveClip { track: TrackId, id: ClipId },
    MoveClip { track: TrackId, id: ClipId, to: Tick },
    SplitClip { track: TrackId, id: ClipId, at: Tick },           // 오른쪽 id는 새로 발급
    SetClipName { .. }, SetClipColor { .. },
    InsertNote { track: TrackId, clip: ClipId, note: Note },
    RemoveNote { track: TrackId, clip: ClipId, note: Note },
    SetTempo { at: Tick, tempo: Option<Tempo> }, SetMeter { at: Tick, signature: Option<TimeSignature> },
    SetKey(Key),
}
pub(crate) enum Op { /* Edit + InsertClipWithId, SplitClipWithId, UnsplitClip, RestoreNote */ }
// history.rs (리뷰 2 뒤 crate 내부)
pub struct CoalesceKey(pub u64);
pub(crate) struct History { .. }        // Entry는 Vec<Op>를 보관
// document.rs (리뷰 2 뒤 추가, 편집의 유일한 공개 진입점)
pub struct Document { project: Project, history: History }   // Clone은 둘을 함께 복제
impl Document {
    pub fn new(project: Project, limit_bytes: usize) -> Result<Self, ModelError>;   // TrackId·ClipId 중복 거부
    pub fn project(&self) -> &Project;                                            // &mut는 없다
    pub fn into_project(self) -> Project;
    pub fn apply(&mut self, edits: Vec<Edit>, coalesce: Option<CoalesceKey>) -> Result<(), ModelError>;
    pub fn undo(&mut self) -> Result<bool, ModelError>;
    pub fn redo(&mut self) -> Result<bool, ModelError>;
    pub fn seal(&mut self);
    pub fn undo_len/redo_len/history_bytes;
}
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `history_random_edits_undo_all_restores_initial` | 속성 | 임의 편집 N개 → undo N번 = 초기 상태(게이트) |
| 2 | `history_undo_then_redo_all_restores_final` | 속성 | undo 전부 → redo 전부 = 마지막 상태(`ClipId` 포함) |
| 3 | `history_new_edit_after_undo_clears_redo` | 단위 | 게이트 |
| 4 | `history_coalesced_edits_undo_at_once` | 단위 | 같은 키 다섯 번 → undo 한 번(게이트) |
| 5 | `history_coalesce_breaks_on_other_key_seal_or_undo` | 단위 | 다른 키, `seal`, undo/redo 뒤에는 묶이지 않음 |
| 6 | `history_failed_transaction_leaves_project_unchanged` | 속성 | 묶음 중간 실패 시 원자성, 히스토리도 그대로 |
| 7 | `history_undo_redo_preserve_invariants` | 속성 | undo·redo 각 단계 뒤 S0-T2 불변식 검사기 통과 |
| 8 | `history_split_redo_reuses_right_clip_id` | 단위 | 분할 → 오른쪽에 노트 → undo 2 → redo 2 성공 |
| 9 | `history_memory_limit_drops_oldest` | 단위 | 상한을 넘으면 오래된 단위부터 버림, 최신은 남음 |
| 10 | `history_undo_after_outside_mutation_fails_without_change` | 단위 | 히스토리 밖 변경으로 역편집이 맞지 않으면 `Err`, 상태 불변 |
| 11 | `edit_apply_returns_inverse_for_each_variant` | 단위 | 변형마다 apply → inverse.apply = 원래 상태 |

## 불변식

- `Edit::apply`는 실패하면 프로젝트를 바꾸지 않는다(모델 API의 「실패 시 불변」에 기댄다).
- `History::apply`·`undo`·`redo`는 원자적이다. 실패하면 프로젝트와 히스토리 모두 그대로다.
- 역편집 적용은 모델 검사를 거친다. 히스토리가 모델 내부를 직접 고치지 않는다.

## 범위 밖

- 앱의 Action과 묶음 키 배정(S2 이후), 드래그 한 번 묶기(MS-1, S3)·테이크 묶기(S5)의 호출 쪽.
- 퀀타이즈 등 상위 연산(S0-T7)은 이 `Edit`들의 묶음으로 표현한다. 그 Task에서 만든다.
- 노트 외 이벤트 레인(S6). 그때 `Edit` 변형을 더한다.

## 리스크·결정 요청

- 메모리 사용량은 편집 크기의 추정치다(노트·클립·트랙의 대략 바이트). 정확한 측정은 NF-7(S8)에서 한다.
- 기본 상한 값은 REQ에 없다. 이 Task는 상한을 인자로 받기만 하고, 앱 기본값(예: 256 MB)은 S2 설정에서 정한다.
