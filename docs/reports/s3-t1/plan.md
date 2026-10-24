# Task 계획: S3-T1 피아노롤 Element

- 브랜치: `s3/t1-pianoroll-element`
- 관련 REQ: 5.3(피아노롤 화면), 5.7(커서 이동의 수식키 없음·`Ctrl` 행만), 5.8(`Cmd+=` `Cmd+-` `Cmd+0`), 7.1(화면 구성), 9.4(자체 Element), UI-3, UI-4, NF-8
- 선행 Task: S2 전부(창·모드 FSM·키맵·트랜스포트)
- 관련 스파이크: `docs/spikes/sp2-element-instancing.md`(분기 A: GPUI 프리미티브, 노트 quad는 `paint_layer` 안)
- 이월 받음: 없음(S3-T1 앞에 걸린 이월 줄은 없다. `clip::Open`·`view::Toggle`의 모드 전이는 S2-T3가 끝냈다)

## 끝의 정의 (내 말로)

1. 피아노롤 뷰: `View::PianoRoll`일 때 가운데 주 뷰가 자리표시자 대신 자체 `Element`를 그린다.
   - 세로 음높이(행 0–127, 위가 높은 음), 가로 틱. 배경은 행마다 조성 안·밖(조성이 없으면 흰건반·검은건반)으로 명암을 달리한다. 마디 선은 진하게, 박 선은 옅게(박자표 맵을 따른다).
   - 현재 클립: 현재 트랙에서 커서 틱을 품은 클립. 그 노트를 트랙 색으로 그린다. 같은 트랙의 다른 클립 노트는 흐리게 그린다(5.3 「앞뒤 클립의 노트는 흐리게」). 다른 트랙은 그리지 않는다.
   - 커서는 (틱, 음높이) 한 칸을 테두리로, 선택 노트는 `selection` 색으로 강조한다(선택 내용은 S3-T2가 채운다. 이 Task는 빈 선택을 그린다).
   - 재생 헤드: 트랜스포트가 재생 중이면 기준점에서 모노토닉 클록으로 보간한 틱을 세로선으로 그리고 매 프레임 다시 그린다(UI-4). 일시정지면 그 자리, 정지면 그리지 않는다.
2. 커서 상태: 창이 `PianoRollState`(커서 틱·음높이, 가로 스크롤 틱, px/틱, 세로 스크롤 행, 행 높이, 한 스텝 틱, 선택)를 갖는다.
   - 수식키 없는 이동 `cursor::Left/Right`(한 스텝, 기본 4분음표 960틱. S3-T4가 Step 길이와 묶는다), `cursor::Up/Down`(반음), `Ctrl` 이동 `cursor::PrevBar/NextBar`(마디, 박자표 맵), `cursor::OctaveDown/OctaveUp`. 틱은 0 아래로, 음높이는 0–127 밖으로 나가지 않는다.
   - 커서가 뷰포트 밖으로 가면 뷰포트가 따라간다(가로는 커서 칸이 보이게, 세로도 같게).
   - 피아노롤로 들어갈 때(`clip::Open`, `view::Toggle`) 커서가 현재 트랙의 어느 클립 안도 아니면 첫 클립 시작으로 옮긴다. 클립이 없으면 그대로 둔다.
3. 줌·스크롤: `view::ZoomIn/ZoomOut`(1.25배, 커서 칸이 보이게 유지), `view::ZoomReset`(기본 배율), `view::CenterCursor`(커서를 가운데로). 마우스 휠은 세로·가로 스크롤(`Shift`+휠은 가로), 클릭은 커서를 그 칸으로 옮긴다(러버밴드·드래그는 S3-T6).
4. 가로 스크롤 동기 훅: `ProjectWindow::piano_roll_viewport()`가 (첫 틱, px/틱, 너비)를 돌려준다. 인스펙터·레인(S6)은 이것을 읽어 같은 가로축을 그린다. 이 Task는 메서드와 바뀔 때의 `cx.notify()`까지만 한다.
5. 성능: 장면(노트·격자 사각형 목록)은 순수 함수 `scene::build`가 만들고, `Element::paint`는 그 목록을 `paint_layer` 하나 안에서 `paint_quad`로 그린다. 보이는 노트는 클립의 정렬된 노트 열(`Clip::notes`는 시작 틱 순)에서 `partition_point`로 찾고 오른쪽 끝에서 멈춘다. 클립마다 최대 노트 길이를 문서 `revision`별로 캐시해 왼쪽 경계를 정한다. 매 프레임 할당은 재사용 버퍼로 막는다.
   - criterion 벤치 `pianoroll/scene_10k_notes`: 노트 10,000개가 모두 보이는 뷰포트에서 `scene::build`. 기준값을 `benches/baseline.json`에 넣는다. GPUI 쪽 `paint_quad` 비용은 SP-2 측정(10,000개 p95 약 2 ms)이 덮는다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/pianoroll/mod.rs
pub struct Cursor { pub tick: Tick, pub pitch: Pitch }           // 트랙 기준 틱
pub struct Viewport { pub first_tick: f64, pub px_per_tick: f64, pub width: f32,
                      pub first_row: f32, pub row_height: f32, pub height: f32 }
pub struct PianoRollState { /* cursor, viewport, step_ticks, selection, caches */ }
impl PianoRollState {
    pub fn cursor(&self) -> Cursor;
    pub fn viewport(&self) -> Viewport;
    pub fn step_ticks(&self) -> u64;
    pub fn move_cursor(&mut self, delta: CursorMove, meter: &MeterMap); // Step/Semitone/Bar/Octave ± 
    pub fn zoom(&mut self, factor: f64); pub fn zoom_reset(&mut self); pub fn center_cursor(&mut self);
    pub fn scroll_by(&mut self, dx_px: f32, dy_px: f32);
    pub fn ensure_cursor_visible(&mut self);
    pub fn cell_at(&self, local: Point<Pixels>) -> Option<Cursor>;   // 클릭 → 칸
}
pub struct Selection { /* BTreeSet<Note>, 현재 클립 기준 */ }

// crates/keyscore-app/src/pianoroll/scene.rs (순수, GPUI 없음)
pub struct SceneInput<'a> { pub clips: &'a [(ClipId, &'a Clip)], pub current: Option<ClipId>,
    pub max_len: &'a [(ClipId, u32)], pub meter: &'a MeterMap, pub key: Key,
    pub cursor: Cursor, pub selection: &'a Selection, pub playhead: Option<Tick>, pub viewport: Viewport }
pub struct Scene { pub rows: Vec<RowBand>, pub lines: Vec<GridLine>, pub notes: Vec<NoteRect>,
    pub cursor: Rect, pub playhead: Option<f32> }
pub fn build(input: &SceneInput, out: &mut Scene);  // 버퍼 재사용

// crates/keyscore-app/src/pianoroll/element.rs
pub struct PianoRollElement { /* Rc<RefCell<Scene>>, 색, 마우스 콜백 */ }
impl Element for PianoRollElement { /* paint_layer 안에서 paint_quad */ }

// ProjectWindow
pub fn piano_roll(&self) -> &PianoRollState;
pub fn piano_roll_viewport(&self) -> Viewport;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `scene_shows_current_clip_notes_and_dims_neighbors` | 단위(scene) | 현재 클립 노트는 `dim=false`, 같은 트랙 다른 클립은 `dim=true`, 사각형 위치는 틱·행 변환과 같다 |
| 2 | `scene_skips_offscreen_notes_with_sorted_search` | 단위(scene) | 노트 10,000개, 뷰포트가 일부만 덮을 때 결과가 전수 검사와 같다. 긴 노트(최대 길이 캐시)가 왼쪽 경계에 걸려도 빠지지 않는다 |
| 3 | `scene_rows_shade_in_key_pitches` | 단위(scene) | F장조·A단조에서 조성 안 행 표시가 음계와 같다. 조성 없음이면 흰·검은건반 |
| 4 | `scene_grid_lines_follow_meter_map` | 단위(scene) | 3/4와 4/4 변화가 있는 맵에서 마디 선·박 선 틱 |
| 5 | `scene_playhead_and_cursor_positions` | 단위(scene) | 재생 헤드 x, 커서 칸 사각형(한 스텝 너비) |
| 6 | `cursor_keys_move_by_step_semitone_bar_octave` | UI | `l` `h` `j` `k`, `ctrl-l` `ctrl-h` `ctrl-k` `ctrl-j`, `[` `]`가 커서를 옮기고 경계에서 멈춘다 |
| 7 | `cursor_moves_scroll_viewport_to_follow` | UI | 오른쪽 끝을 넘으면 `first_tick`이 늘고, 위로 올리면 `first_row`가 준다(단계 파일 게이트) |
| 8 | `zoom_keys_scale_and_keep_cursor_visible` | UI | `cmd-=` `cmd--` `cmd-0`, 줌 뒤에도 커서 칸이 뷰포트 안 |
| 9 | `entering_piano_roll_places_cursor_in_first_clip` | UI | 배치 뷰에서 `enter`로 들어갈 때 커서가 클립 밖이면 첫 클립 시작으로 |
| 10 | `playhead_follows_transport` | UI + 루프백 | 일시정지 자리에 재생 헤드, 정지면 없음, 재생 중이면 매 프레임 다시 그림(`request_animation_frame`이 불렸는지는 프레임 수로) |
| 11 | `element_paints_every_visible_note_once` | UI | 헤드리스 `render_frame` 뒤 그린 노트 수 = 장면의 노트 수(시험용 카운터) |
| 12 | `cell_at_maps_click_point_to_cursor` | 단위 | 픽셀 → (틱, 음높이) 칸, 뷰포트 밖은 `None` |
| 13 | `pianoroll/scene_10k_notes` | 벤치 | 10,000개 전부 보이는 뷰포트의 `scene::build` 기준값 |

## 불변식

- 장면 계산은 문서를 바꾸지 않는다. 문서 편집은 이 Task에 없다.
- 커서 틱 ≥ 0, 음높이 0–127(`Pitch`가 보장). 줌은 상한·하한 사이에서만.
- `paint`는 할당·정렬을 하지 않는다. 장면 버퍼는 `prepaint`에서 재사용 `Vec`에 채우고 노트 열은 모델의 정렬 순서를 그대로 쓴다.
- 노트 quad는 `paint_layer` 하나 안에서 그린다(SP-2 조건).

## 범위 밖

- 5.7 표의 `Shift`·`Alt`·`Alt+Shift`·`Alt+Ctrl` 행, `n`/`p`, `Cmd+화살표`, 선택 내용, 이동 시 미리듣기(S3-T2)
- 편집 동작(S3-T3), Step 입력(S3-T4), 건반 뷰(S3-T5), 러버밴드·드래그(S3-T6), 노트 정보 팝업(S3-T7)
- 배치 뷰 Element(S4-T1), 컨트롤러 레인(S6-T1). 가로 동기는 훅(뷰포트 조회)만
- 노트 위 글자(음 이름·가사 표시)는 그리지 않는다(S6 가사와 함께)
- 트랙 사이 커서 이동(`cursor::TrackUp/Down`은 배치 뷰 키, S4)

## 리스크·결정 요청

- 기본 배율(4분음표 96 px, 행 12 px)과 줌 배수(1.25)는 REQ에 없는 세부 값이다. 흔한 DAW 값으로 두고 설정은 S8 테마와 함께 본다.
- 「한 스텝」(5.7)은 Step 모드의 현재 길이인데 Step 입력은 S3-T4다. 그때까지 기본 길이 4분음표(5.6 기본값)를 쓴다.
- 커서가 클립 밖일 때의 현재 클립은 「없음」이다. 노트 없이 격자만 그리고 이웃 클립은 흐리게 그린다. REQ 5.3의 「클립 하나의 안쪽」을 벗어난 자리를 막지는 않는다(Step 입력이 클립 밖에서 시작하면 클립을 만든다는 5.6 때문이다).
- 결정 요청 없음.
