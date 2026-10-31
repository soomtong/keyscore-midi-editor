# Task 계획: S3-T6b 드래그 이동·조절

- 브랜치: `s3/t6b-mouse-drag`
- 관련 REQ: 7.4(이동·조절 표, MS-1 고스트·undo 한 단위, MS-2, MS-3, MS-4, MS-6), 5.7(`Alt` 조작과 같은 계산), 5.3·PRE-1(미리듣기 설정), P2(`Esc`는 진행 중 조작 취소), EDT-1
- 선행 Task: S3-T2(`plan_note_op`, 선택), S3-T3a(`insert_notes`·`Cmd+D`), S3-T4c(미리듣기 가벼운 경로), S3-T6a(누름·`Gesture`·`live_gesture`·`piano_roll_view_changed`)
- 관련 스파이크: 없음
- 결정: `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 6절(이 Task에서 세부를 이어 적음)

## 끝의 정의 (내 말로)

Normal 모드의 피아노롤에서 선택한 노트를 마우스로 끌어 옮기고(본체), 끝을 끌어 길이와 시작을 바꾸고, `Alt`로 복제한다. 결과는 키 5.7 `Alt` 조작과 같은 일반 변환(`NoteShift`)이고 키 조작은 그 특수한 경우다. 끄는 동안 고스트가 결과를 보이고, 놓으면 한 번에 적용된다(undo 하나). 규칙은 결정 6절 1–12이고, 여기서는 구현 단위로 적는다.

1. 일반 변환(`pianoroll/ops.rs`): `NoteShift { time, pitch, start, end }`. `time`은 틱 수 또는 「한 마디 앞·뒤」(노트마다 그 자리의 마디 길이, 지금 `Alt+Ctrl+h/l` 규칙). 노트 하나의 계산 `shift_note`가 시간 → 시작 Δ(끝 고정) → 끝 Δ(시작 고정) → 음높이 순으로 바꾸고, 결과 길이가 1틱 미만이면 그 노트는 그대로다(`None`). `plan_shift`는 그 계산을 `map_notes`로 한 클립에 적용한다(거부 규칙 그대로). `plan_note_op(op, step)`은 `plan_shift(op.shift(step))`이다. `plan_shift_copy`는 같은 `shift_note`로 사본을 만들어 `insert_notes`로 넣는다.
2. 드래그 양(`pianoroll/mouse.rs`, 순수): `drag_shift(DragInput) -> NoteShift`. 잡는 자리(`Grip::Body/Start/End`), 클립 시작, 대상 노트(클립 기준), 잡은 노트, 원시 틱 Δ(소수), 행 Δ, 스냅 격자(0이면 스냅 없음), 박자 지도, 한 축 잠금(`Option<Axis>`)을 받아 결정 6절 3–6대로 스냅·자르기를 한다. `grip_at(note_x, note_w, x)`(MS-6), `snap_nearest(meter, tick, grid)`.
3. 누름: T6a 누름 처리 뒤 노트 위면 `Gesture::Note`에 잡는 자리·잡은 노트·누른 내용 위치를 더 담는다.
4. 끌기: 문턱을 넘고 잡은 노트가 선택에 있으면 `Gesture::Move`가 된다(대상 = 그 때의 선택 노트 목록). 움직임·수식키 변화·뷰포트 변화마다 `drag_shift` → `plan_shift`/`plan_shift_copy`로 계획을 다시 만들고(시프트가 같으면 다시 계산하지 않는다) 그 결과를 고스트로 둔다. 거부면 고스트를 거부 색으로.
5. 수식키: 움직임 이벤트의 수식키를 읽는다. 움직이지 않고 수식키만 바뀌면 창 루트의 `on_modifiers_changed`가 같은 계산을 부른다. `Cmd` = 스냅 없음, `Shift` = 한 축(본체만), `Alt` = 복제(본체만, 놓을 때의 상태).
6. 놓음: 마지막 계획을 다시 계산해(뗌 이벤트의 위치·수식키) `apply_edit(.., None)`으로 적용한다. 선택 = 바뀐 노트 + 그대로 둔 노트(복제면 사본, `apply_placed`처럼). 본체 이동이면 커서가 같은 Δ만큼 따라간다. 시프트가 항등이거나 바뀌는 노트가 없으면 편집 없음. 거부면 상태줄에 이유.
7. 취소: `Esc`·문서 변경·모드·뷰 전환은 T6a와 같은 길(`cancel_piano_roll_gesture`, `live_gesture`)로 `Gesture::Move`를 버린다. 고스트도 없어진다.
8. 미리듣기(MS-4): 본체 이동에서 잡은 노트 고스트의 음높이가 직전에 들려준 음높이와 다르면 `sound_note`로 한 번. 설정 「이동 시 미리듣기」가 꺼져 있으면 보내지 않는다.
9. 스냅 단위 설정(MS-3): `InputSettings.mouse_snap: MouseSnap`(`"length"` 기본, `"1/4"`…`"1/64"`), Action `settings::SetMouseSnap(MouseSnap)`(앱 전역 처리기, 명령 표 「설정 > 입력」 자리), 설정 창 입력 구역의 선택 상자. 더블클릭 생성도 같은 격자 함수(`mouse_grid`)를 쓴다.
10. 고스트 그리기: 장면 `ghosts: Vec<Rect>`·`ghost_blocked: bool`을 창의 조작이 채운다(러버밴드 `band`와 같은 방식). 테두리만 그리고 채움은 옅게, 거부면 재생 헤드 색.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/pianoroll/ops.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeShift { Ticks(i64), BarEarlier, BarLater }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteShift { pub time: TimeShift, pub pitch: i32, pub start: i64, pub end: i64 }
impl NoteShift {
    pub const IDENTITY: NoteShift;
    pub fn is_identity(&self) -> bool;
}
impl NoteOp { pub fn shift(self, step: u64) -> NoteShift; }
pub fn plan_shift(project: &Project, track: TrackId, clip: ClipId, notes: &[Note], shift: NoteShift)
    -> Result<Option<Planned>, OpError>;
pub fn plan_shift_copy(project: &Project, track: TrackId, clip: ClipId, notes: &[Note], shift: NoteShift)
    -> Result<Option<Planned>, OpError>;
pub fn plan_note_op(project, track, clip, notes, op, step) -> Result<Option<Planned>, OpError>; // 그대로, 위를 부름

// crates/keyscore-app/src/pianoroll/mouse.rs
pub const EDGE_PX: f32 = 6.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Grip { Body, Start, End }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Axis { Time, Pitch }
pub fn grip_at(note_x: f32, note_w: f32, x: f32) -> Grip;
pub fn snap_nearest(meter: &MeterMap, tick: Tick, grid: u64) -> Tick;
pub struct DragInput<'a> {
    pub meter: &'a MeterMap, pub clip_start: u64, pub notes: &'a [Note], pub grabbed: &'a Note,
    pub grip: Grip, pub ticks: f64, pub rows: i32, pub grid: u64, pub axis: Option<Axis>,
}
pub fn drag_shift(input: &DragInput<'_>) -> NoteShift;
/// 한 축 잠금: 화면 이동이 가로 ≥ 세로면 시간.
pub fn dominant_axis(dx: f32, dy: f32) -> Axis;

// crates/keyscore-app/src/actions.rs (args)
pub enum MouseSnap { Length, Quarter, Eighth, Sixteenth, ThirtySecond, SixtyFourth }
// settings::SetMouseSnap(pub MouseSnap)

// crates/keyscore-app/src/settings.rs
pub struct InputSettings { …, pub mouse_snap: MouseSnap }

// crates/keyscore-app/src/pianoroll/mod.rs (시험용)
impl PianoRollState {
    pub fn ghost_rects(&self) -> Vec<scene::Rect>;
    pub fn ghost_blocked(&self) -> bool;
}

// crates/keyscore-app/src/pianoroll/element.rs
pub type PointerHandler = Rc<dyn Fn((f32, f32), Modifiers, &mut Window, &mut App)>; // 수식키를 더함
~~~

창 쪽(`pianoroll_mouse.rs`)은 `piano_roll_drag(at, modifiers, cx)`, `piano_roll_release(at, modifiers, cx)`, `piano_roll_modifiers(modifiers, cx)`를 둔다.

## 테스트 목록

순수 시험 `crates/keyscore-app/tests/mouse_drag_pure.rs`, UI 시험 `crates/keyscore-app/tests/mouse_drag.rs`(헤드리스 창, MIDI 없음), 미리듣기 시험 `crates/keyscore-app/tests/mouse_drag_preview.rs`(CoreMIDI 루프백, `preview_light.rs`와 같은 방식).

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `note_ops_equal_plan_shift_of_their_shift` | 단위 | 열 가지 `NoteOp` 모두 `plan_note_op` == `plan_shift(op.shift(step))`(마디 경계·박자표 변화 포함) |
| 2 | `plan_shift_keeps_notes_that_would_vanish` | 단위 | 끝 Δ·시작 Δ로 길이가 1틱 미만이 되는 노트는 그대로, 전부 그러면 `None` |
| 3 | `plan_shift_rejects_before_clip_start_pitch_range_and_next_clip` | 단위 | 거부 세 가지, 클립 끝 넘으면 마디 단위 연장 |
| 4 | `plan_shift_copy_inserts_copies_and_keeps_originals` | 단위 | 원본 유지, 사본은 변환 결과, 클립 연장은 `insert_notes` 규칙 |
| 5 | `grip_edge_zone_is_six_px_or_third_of_narrow_note` | 단위 | 넓은 노트 6 px, 12 px 노트는 4 px, 경계값 |
| 6 | `snap_nearest_rounds_to_bar_relative_grid` | 단위 | 가까운 쪽, 같으면 앞, 다음 마디 시작, 3/4 변화 뒤 마디 기준, 격자 0은 그대로 |
| 7 | `drag_shift_body_snaps_grabbed_start` | 단위 | 잡은 노트 시작이 격자에, 음높이 Δ = 행 차 |
| 8 | `drag_shift_free_rounds_raw_ticks` | 단위 | 격자 0이면 원시 Δ 반올림 |
| 9 | `drag_shift_clamps_clip_start_and_pitch_range` | 단위 | 가장 이른 노트가 클립 시작에서 멈춤, 가장 높은 노트가 127에서 멈춤 |
| 10 | `drag_shift_end_snaps_and_keeps_grabbed_length` | 단위 | 끝 스냅, 시작 이하로 끌면 시작 뒤 첫 격자선 |
| 11 | `drag_shift_start_keeps_end_and_clamps` | 단위 | 시작 스냅·끝 고정, 끝 이상이면 끝 앞 마지막 격자선, 클립 시작 앞 자르기 |
| 12 | `drag_shift_axis_lock_zeroes_other_axis` | 단위 | `Axis::Time`이면 음높이 0, `Pitch`면 시간 0, 끝 조절은 무시 |
| 13 | `mouse_snap_setting_round_trips_and_rejects_bad_values` | 단위 | `input.mouse_snap` 읽기·쓰기, 모르는 값은 경고 + 기본값 |
| 14 | `drag_selection_keeps_relative_positions_with_one_undo` | UI | 세 노트 선택, 본체 끌기 → 모두 같은 Δ(시간·음높이), `cmd-z` 한 번에 원래대로(단계 게이트) |
| 15 | `ghost_shows_result_while_document_stays` | UI | 끄는 중 문서 그대로, 고스트 사각형 = 결과 노트 자리, 놓으면 고스트 없음 |
| 16 | `escape_during_drag_cancels_and_release_does_nothing` | UI | `Esc` → 고스트 없음, 떼어도 문서·선택 그대로, undo 쌓이지 않음(단계 게이트) |
| 17 | `cmd_during_drag_disables_snap` | UI | 같은 움직임이 `Cmd` 없이는 격자, 있으면 원시 틱. 수식키만 바꿔도 고스트가 바뀜 |
| 18 | `shift_drag_moves_on_one_axis` | UI | 대각선 끌기 + `Shift` → 큰 쪽 축만 |
| 19 | `alt_drag_duplicates_with_one_undo` | UI | 원본 남고 사본 선택, undo 한 번에 사본만 사라짐 |
| 20 | `right_edge_drag_moves_all_ends_by_same_amount` | UI | MS-2 길이 조절, 짧은 노트가 사라질 양이면 그 노트는 그대로 |
| 21 | `left_edge_drag_moves_start_and_keeps_end` | UI | 시작만 바뀜 |
| 22 | `narrow_note_edge_zone_is_a_third` | UI | 좁은 노트에서 1/3 안쪽은 끝, 가운데는 본체 |
| 23 | `drag_past_clip_end_extends_clip_by_bars` | UI | 클립 끝을 넘긴 이동 → 마디 단위 연장, undo 한 번에 클립 길이도 돌아옴 |
| 24 | `blocked_drag_shows_blocked_ghost_and_does_not_edit` | UI | 다음 클립과 겹치는 연장 → 고스트 거부, 놓아도 편집 없음, 상태줄 이유 |
| 25 | `cursor_follows_body_move` | UI | 놓은 뒤 커서 = 누른 칸 + Δ, 끝 조절이면 그대로 |
| 26 | `drag_of_cmd_toggled_out_note_does_nothing` | UI | `Cmd+클릭`으로 뺀 노트를 끌어도 편집 없음 |
| 27 | `document_change_during_drag_cancels` | UI | 끄는 중 `cmd-z` → 고스트 없음, 떼어도 편집 없음 |
| 28 | `wheel_during_drag_recomputes_ghost` | UI | 누른 채 휠 스크롤 → 포인터를 움직이지 않아도 고스트가 새 내용 위치 기준 |
| 29 | `mouse_snap_setting_changes_drag_and_double_click_grid` | UI | `settings::SetMouseSnap(1/16)` → 드래그·더블클릭 생성이 16분 격자 |
| 30 | `drag_in_step_mode_does_nothing` | UI | Step에서 노트를 끌어도 편집 없음(T6a 2절 1) |
| 31 | `body_drag_previews_each_new_pitch_once` | MIDI | 두 반음 위로 끄는 동안 노트 온이 음높이마다 한 번, 같은 음높이 안 움직임은 보내지 않음, 설정을 끄면 0 |

기존 `plan_note_op`·키 조작 시험(S3-T2 `pianoroll_select.rs` 등)은 그대로 통과해야 한다.

## 불변식

- 키 `Alt` 조작과 마우스 드래그는 같은 노트 계산(`shift_note`)과 같은 편집 계획(`map_notes`/`insert_notes`)을 쓴다. 마우스 전용 편집 경로를 만들지 않는다(리뷰 초점).
- 고스트와 놓을 때 적용하는 편집은 같은 계획 함수의 결과다.
- 문서는 놓을 때 한 번만 바뀐다(`apply`, 묶음 없음). 끄는 중·취소에는 바뀌지 않는다.
- 선택은 한 클립이다. 커서는 그 클립 안에 남는다(대상 노트는 클립을 바꾸지 않는다).
- 실시간 스레드와 무관하다. 미리듣기는 기존 `sound_note` 경로다.

## 범위 밖

- 배치 뷰 클립 드래그(S4-T4). 트랜스포트 끌어서 조정(S3-T6c).
- 끝 영역 위 포인터 모양(좌우 화살표) 같은 호버 표시(REQ에 없다).
- 드래그 중 자동 스크롤(REQ에 없다).
- 여러 클립에 걸친 대상(선택이 한 클립이다).

## 리스크·결정 요청

- 결정 6절은 에이전트 해석이다. 특히 「가장 가까운 격자선 스냅」(6절 4), 「클립 시작·음높이 범위에서 자르기」(6절 5), 「마우스 편집은 `.`이 기억하지 않음」(6절 10), 「미리듣기는 이동 시 미리듣기 설정을 따름」(6절 12)은 사람이 다르게 보면 고친다.
- `PointerHandler`에 수식키를 더하면 T6a 시험의 콜백 모양은 바뀌지 않는다(시험은 이벤트를 보낸다).
- Action이 하나 늘어난다(`settings::SetMouseSnap`). 명령 표·카탈로그 수를 맞춘다.
