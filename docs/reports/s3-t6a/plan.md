# Task 계획: S3-T6a 마우스 선택

- 브랜치: `s3/t6a-mouse-select`
- 관련 REQ: 7.4(선택 표, MS-1의 러버밴드 미리보기, MS-5, MS-7), 5.3, 5.7, P2(`Esc`는 진행 중 조작 취소), 5.5(`Esc`는 가장 안쪽 하나만), PRE-1, EDT-1
- 선행 Task: S3-T1(피아노롤 Element, 클릭·휠), S3-T2(선택·기준 칸·`region_notes`), S3-T3a(`edit::Delete`, `plan_paste`), S3-T3c(비주얼·오퍼레이터 불변식), S3-T4a(Step 길이 코드)
- 관련 스파이크: 없음
- 결정: `docs/decisions/2026-10-31-s3-t6-mouse-editing.md`(1절 나눔, 2–5절이 이 Task의 규칙)

## 끝의 정의 (내 말로)

Normal 모드의 피아노롤에서 마우스로 노트를 고르고, 더블클릭으로 만들고 지우고, `Cmd+휠`·핀치로 가로 줌한다. 마우스가 만든 선택은 키가 만든 선택과 같은 상태라서 고른 뒤 `Alt+k`·`x`가 그대로 된다. 규칙은 결정 문서 2–5절이고, 여기서는 구현 단위로 다시 적는다.

1. 누름(Normal, 왼쪽 단추, 트랙이 있음): 커서가 누른 칸으로(스냅 없음), 비주얼·오퍼레이터 대기가 끝나고 되돌리기 묶음이 끊긴다. 그 뒤 `click_count`와 수식키로 갈린다.
   - `click_count == 2`: 노트 위면 그 노트 하나를 선택하고 `edit::Delete`를 `dispatch::run_action`으로 낸다. 빈 곳이면 노트 하나를 만든다(규칙 5).
   - 노트 위, 수식키 없음: 그 노트가 선택에 없으면 선택 = 그 노트. 있으면 그대로 두고 「떼면 그 노트 하나로」를 기억한다.
   - 노트 위, `Cmd`: 같은 클립이면 넣기·빼기, 아니면 그 노트 하나.
   - 노트 위, `Shift`: 결정 3절 4의 시간 범위를 선택에 더한다.
   - 빈 곳, `Shift`: 같은 시간 범위 규칙(누른 틱 한 틱).
   - 빈 곳(`Shift` 없음): 러버밴드 대기. `Cmd`면 「더하기」.
   - 클릭이나 `Cmd+클릭`으로 선택에 넣은 노트는 「마지막 선택」(`Shift+클릭`의 기준)이 된다.
2. 끌기: 누른 점에서 3 px 이상 움직이면 끌기다. 빈 곳에서 시작했으면 러버밴드가 되어 매 움직임마다 선택을 다시 계산하고(누를 때의 선택은 `before`로 둔다) 직사각형을 그린다. 노트 위에서 시작한 끌기는 이 Task에서 아무것도 하지 않는다(S3-T6b가 이동을 붙인다). 그 뒤 떼어도 선택을 그 노트 하나로 줄이지 않는다.
3. 뗌: 러버밴드 대기에서 끌지 않고 떼면 선택을 비운다(`Cmd`면 그대로). 러버밴드면 직사각형을 지우고 선택을 남긴다. 「떼면 그 노트 하나로」가 남아 있으면 그렇게 한다. 선택이 생겼고 커서가 그 클립 밖이면 커서를 선택의 첫 노트 칸으로 옮긴다(결정 2절 5).
4. 취소: 누른 채 `Esc`(수식키 없음)는 진행 중 조작을 끝낸다. 러버밴드면 선택을 `before`로 되돌린다. `Esc`는 그것만 하고 선택 해제(`select::Clear`)로 가지 않는다. 그 뒤 뗌은 아무것도 하지 않는다. 누른 채 문서가 바뀌어도(키 undo 등) 같은 취소를 하되 선택은 바뀐 문서에 맞춘(`retain_existing`) 그대로 둔다.
5. 더블클릭 생성: 시작 = 누른 틱을 스냅 격자로 내림(`Cmd`면 그대로), 격자 = 현재 Step 길이 코드 틱(점 포함, 셋잇단·절반 없음), 기준은 그 틱이 든 마디의 시작. 길이 = 같은 틱. 음높이 = 누른 행, 벨로시티 = 트랙 기본. `edit::plan_paste`에 노트 하나짜리 `ClipNotes`를 넘겨 계산하고 붙여넣기와 같은 `apply_placed`로 적용한다(undo 한 단위, 새 노트가 선택). 커서는 새 노트 시작 칸으로 가고 PRE-1 설정이 켜져 있으면 들려준다.
6. 다른 모드(Step 등)에서 누르면 지금처럼 커서만 옮긴다(`click_piano_roll_cell`). 오른쪽 단추(마크 메뉴)는 그대로다.
7. 줌: `Cmd+휠`은 세로 델타 20 px마다 1.25배(위로 = 확대), 핀치는 `1 + delta`배. 둘 다 포인터 x 아래 틱을 제자리에 둔다. 상·하한은 키 줌과 같다. 커서를 따라가지 않는다.
8. 가장자리: 피아노롤을 4 px 여백 안에 둔다. 여백 밖 래퍼에 `main-view` id를 두지 않고 피아노롤 쪽 `main-view`가 Element와 같은 크기를 유지한다(창 시험이 `main-view` 경계로 좌표를 잡는다).
9. 러버밴드 기하(결정 3절 5): 누른 점은 내용 위치(틱·행), 지금 점은 피아노롤 안으로 자른 포인터. 틱 범위는 `[floor(작은 틱), ceil(큰 틱))`(폭 0이면 한 틱), 음높이는 두 행 양끝 포함(리뷰 1 지적 3, 제안 5). 누른 채 모드·뷰가 바뀌면 조작을 버린다(리뷰 1 지적 1·2). 클립은 누른 점 → 지금 점 → 시간 범위 첫 클립, `Cmd`면 선택의 클립(지금 트랙)이 먼저.

키 `Shift+이동`의 클립 규칙은 바꾸지 않는다(결정 3절 5 마지막 항목, S3-T2 리뷰 4 이월의 결론).

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/pianoroll/mouse.rs (pub mod, GPUI와 무관한 순수 계산)
/// 끌기로 보는 최소 이동(px).
pub const DRAG_THRESHOLD: f32 = 3.0;

/// 러버밴드가 덮는 범위. 틱은 절대 `[lo, hi)`, 음높이는 양끝 포함.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band { pub lo: Tick, pub hi: Tick, pub low: Pitch, pub high: Pitch }

/// 뷰 기준 두 점의 러버밴드 범위(점은 뷰포트 안으로 자른다). 뷰포트가 비었으면 `None`.
pub fn band(viewport: &Viewport, a: (f32, f32), b: (f32, f32)) -> Option<Band>;
/// 클립에서 범위 안의 노트.
pub fn band_notes(clip: &Clip, band: Band) -> BTreeSet<Note>;
/// 러버밴드의 클립(결정 3절 5).
pub fn band_clip(track: &Track, press: Tick, now: Tick, band: Band, preferred: Option<ClipId>) -> Option<ClipId>;
/// `Shift+클릭`이 더할 노트: `[from, to)`에 걸치는 클립의 모든 노트(음높이 무관). 틱은 절대.
pub fn time_range_notes(clip: &Clip, from: Tick, to: Tick) -> BTreeSet<Note>;
/// 스냅 격자로 내림(그 틱이 든 마디 시작 기준).
pub fn snap_floor(meter: &MeterMap, tick: Tick, grid: u64) -> Tick;
/// 두 점 사이 거리가 문턱 이상인가.
pub fn past_threshold(a: (f32, f32), b: (f32, f32)) -> bool;
/// 선택의 첫 노트(시작이 가장 이르고, 같으면 높은 음)의 절대 칸.
pub fn first_cell(clip: &Clip, notes: &BTreeSet<Note>) -> Option<Cursor>;

// crates/keyscore-app/src/pianoroll/mod.rs
impl PianoRollState {
    /// 포인터 x(뷰 기준) 아래 틱을 제자리에 두고 가로 배율을 `factor`배.
    pub fn zoom_at(&self, factor: f64, x: f32);
    /// 지금 그리는 러버밴드 직사각형(뷰 기준 px). 시험용.
    pub fn band_rect(&self) -> Option<scene::Rect>;
    /// 진행 중인 마우스 조작이 있는가.
    pub fn gesture_active(&self) -> bool;
}

// crates/keyscore-app/src/pianoroll/element.rs
/// 누름: 뷰 기준 좌표, 그 칸(뷰포트 밖이면 None), 수식키, 누른 횟수.
pub struct Press { pub x: f32, pub y: f32, pub cell: Option<Cursor>, pub modifiers: Modifiers, pub click_count: usize }
impl PianoRollElement {
    pub fn on_press(self, f: impl Fn(&Press, &mut Window, &mut App) + 'static) -> Self;   // on_click 대신
    pub fn on_drag(self, f: impl Fn((f32, f32), &mut Window, &mut App) + 'static) -> Self;  // 왼쪽 단추 누른 채 이동(어디서든)
    pub fn on_release(self, f: impl Fn((f32, f32), &mut Window, &mut App) + 'static) -> Self; // 왼쪽 단추 뗌(어디서든)
}

// crates/keyscore-app/src/pianoroll_window.rs (ProjectWindow)
pub fn piano_roll_press(&mut self, press: &Press, window: &mut Window, cx: &mut Context<Self>);
pub fn piano_roll_drag(&mut self, at: (f32, f32), cx: &mut Context<Self>);
pub fn piano_roll_release(&mut self, at: (f32, f32), window: &mut Window, cx: &mut Context<Self>);
~~~

진행 중 조작(`Gesture`: 노트 누름, 빈 곳 누름, 러버밴드, 취소됨)과 「마지막 선택」 노트는 `PianoRollState` 안(`pub(crate)`)에 둔다. `select::notes_in`을 `pub(crate)`로 열어 러버밴드·시간 범위가 같은 걸침 기준을 쓴다. `Esc`는 창의 키 가로채기(`intercept_mouse_keys`, `intercept_pitch_keys`와 같은 방식)가 받는다.

## 테스트 목록

순수 시험 `crates/keyscore-app/tests/mouse_select_pure.rs`, UI 시험 `crates/keyscore-app/tests/mouse_select.rs`(헤드리스 창, `VisualTestContext::simulate_event`로 `MouseDownEvent`·`MouseMoveEvent`·`MouseUpEvent`·`ScrollWheelEvent`·`PinchEvent`. MIDI 없음).

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `band_covers_ticks_floor_ceil_and_rows_inclusive` | 단위 | 두 점 → 틱 `[floor, ceil)`, 행 양끝, 점 순서 무관, 뷰 밖 점은 잘림 |
| 2 | `band_notes_use_overlap_rule` | 단위 | 끝이 `lo`인 노트는 밖, 시작이 `hi`인 노트는 밖, 걸친 긴 노트는 안, 음 범위 밖은 밖 |
| 3 | `band_clip_prefers_press_then_now_then_first_overlap` | 단위 | 누른 점 클립 → 지금 점 클립 → 첫 걸침, `preferred`가 먼저, 클립 없으면 `None` |
| 4 | `time_range_notes_ignore_pitch` | 단위 | 시간 범위 걸침, 음높이 무관, 클립 기준 변환 |
| 5 | `snap_floor_uses_bar_start_grid` | 단위 | 4/4에서 8분 격자, 마디 중간 3/4 변화 뒤 마디 시작 기준, 격자 0은 그대로 |
| 6 | `drag_threshold_is_three_px` | 단위 | 2.9 px 아래는 클릭, 3 px부터 끌기 |
| 7 | `first_cell_is_earliest_then_highest` | 단위 | 같은 시작이면 높은 음 |
| 8 | `click_note_selects_only_that_note` | UI | 다른 선택이 있어도 노트 클릭 → 그 노트 하나, 커서 그 칸, 모드 선택 상태 |
| 9 | `click_empty_clears_selection_and_moves_cursor` | UI | 빈 곳 클릭 → 선택 없음, 커서 그 칸 |
| 10 | `click_selected_note_keeps_selection_until_release` | UI | 선택된 노트를 누르면 선택 유지, 떼면 그 노트 하나 |
| 11 | `cmd_click_toggles_in_same_clip_and_replaces_in_other_clip` | UI | 같은 클립 넣기·빼기, 다른 클립 노트는 그것 하나 |
| 12 | `shift_click_extends_by_time_range_from_last_clicked` | UI | 클릭 후 `Shift+클릭` → 시간 범위의 모든 음, 마지막 선택 기준, 좁히지 않음 |
| 13 | `rubber_band_selects_notes_inside_and_previews_live` | UI | 끄는 중에 선택이 바뀌고 직사각형이 있으며, 떼면 직사각형이 없어지고 선택이 남음 |
| 14 | `cmd_rubber_band_adds_to_selection` | UI | 기존 선택 + 직사각형 |
| 15 | `escape_during_rubber_band_restores_selection_and_keeps_mode` | UI | `Esc` → 누르기 전 선택, 그 뒤 떼어도 그대로, 선택 해제로 가지 않음 |
| 16 | `rubber_band_across_clips_picks_press_clip` | UI | 두 클립에 걸쳐도 누른 클립의 노트만 |
| 17 | `rubber_band_from_gap_moves_cursor_into_selected_clip_and_alt_k_works` | UI | 틈에서 시작 → 커서가 선택 첫 노트 칸, 이어 `alt-k`가 선택을 올림 |
| 18 | `mouse_selection_then_alt_k_transposes` | UI | 클릭 선택 뒤 `alt-k`(같은 Action), undo 한 번에 원래대로 |
| 19 | `double_click_note_deletes_it_with_one_undo` | UI | 더블클릭 → 노트 없음, `cmd-z` 한 번에 돌아옴 |
| 20 | `double_click_empty_creates_note_of_length_code_snapped` | UI | 8분 길이 코드에서 틱 700 더블클릭 → 시작 480, 길이 480, 그 음, 선택됨, undo 한 번 |
| 21 | `cmd_double_click_empty_does_not_snap` | UI | 시작이 누른 틱 그대로 |
| 22 | `double_click_in_gap_creates_bar_clip` | UI | 틈에 마디 단위 새 클립, 뒤 노트 밀지 않음 |
| 23 | `drag_below_threshold_is_click` | UI | 2 px 움직이고 떼면 클릭(선택 비움) |
| 24 | `step_mode_click_only_moves_cursor` | UI | Step에서 노트 클릭 → 선택 없음, 커서만 |
| 25 | `cmd_wheel_zooms_around_pointer` | UI | 포인터 아래 틱 고정, 배율 1.25배, 세로 스크롤 없음 |
| 26 | `pinch_zooms_around_pointer` | UI | `delta 0.5` → 1.5배, 포인터 틱 고정 |
| 27 | `edge_pixel_of_piano_roll_is_clickable` | UI | 피아노롤 왼쪽 위 1 px 안쪽 클릭이 커서를 옮긴다(4 px 띠 이월) |
| 28 | `document_change_during_band_cancels_gesture` | UI | 끄는 중 `cmd-z` → 조작 끝, 이어진 움직임이 선택을 바꾸지 않음 |
| 29 | `visual_mode_ends_on_mouse_press` | UI | `v` 비주얼 중 클릭 → 비주얼 끝, 선택 = 누른 노트 |
| 30 | `release_outside_piano_roll_finishes_band` | UI | 피아노롤 밖에서 떼어도 러버밴드가 끝나고 직사각형이 없어짐 |

## 불변식

- 선택은 한 클립(`Selection`)이고 모드 FSM의 선택과 맞는다(`set_selection`·`replace_selection`만 쓴다).
- 오퍼레이터 대기와 선택은 함께 없다(S3-T3c): 누름이 대기를 끝낸다.
- 문서는 `apply`·`apply_coalesced`·undo로만 바뀐다. 더블클릭 삭제는 `edit::Delete` 처리기, 생성은 붙여넣기의 `plan_paste`·`apply_placed`를 쓴다(마우스 전용 편집 경로를 새로 만들지 않는다).
- 진행 중 조작은 누름 하나에 하나다. 새 누름은 이전 조작(남아 있으면)을 버린다.
- 실시간 스레드와 무관하다.

## 범위 밖

- 노트 드래그 이동·끝 조절·`Alt` 복제·`Shift` 한 축·고스트·미리듣기·스냅 단위 설정(S3-T6b).
- 트랜스포트 끌어서 조정(S3-T6c). 배치 뷰 클립 마우스(S4-T4).
- 러버밴드·드래그 중 자동 스크롤(REQ에 없다).
- 커서 아래 노트 테두리 강조 같은 호버 표시(REQ에 없다).

## 리스크·결정 요청

- 결정 `docs/decisions/2026-10-31-s3-t6-mouse-editing.md`는 에이전트 해석이다. 특히 「마우스 편집은 Normal에서만」(2절 1)과 「더블클릭 생성은 뒤 노트를 밀지 않음」(3절 6)은 사람이 다르게 보면 고친다.
- 기존 시험 `click_moves_cursor_to_cell_under_pointer`(S3-T1)는 커서가 스냅 없는 칸으로 오는 것을 단언한다. 이 Task는 그 동작을 지킨다.
