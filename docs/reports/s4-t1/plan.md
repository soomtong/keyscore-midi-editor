# Task 계획: S4-T1 배치 뷰 Element

- 브랜치: `s4/t1-arrangement-element`
- 관련 REQ: 5.10(클립과 배치 뷰, 배치 뷰 키 표의 「커서」 행), 5.11(배치 뷰의 커서 시간 유지), 7.1(화면 구성: 배치 뷰는 트랙×시간 격자에 클립 블록), 7.6(피아노롤 ↔ 배치 뷰 `v`), 9.4(자체 Element), UI-3, UI-4, SEQ-2
- 선행 Task: S3 전부(S3 출구 서명 2026-11-04). 특히 S3-T1(피아노롤 Element의 장면·Element 구조), S3-T2(마크·우클릭 메뉴), S3-T4c(배치 뷰 Step 표시)
- 관련 스파이크: `docs/spikes/sp2-element-instancing.md`(분기 A: GPUI 프리미티브, quad는 `paint_layer` 안)
- 이월 받음(단계 파일 S4-T1 줄 셋과 S3 출구 보고서 「뒷 단계 파일에 이미 있는 S3발 이월 줄」):
  - (S3-T1 리뷰 1 제안 7) `view::ZoomIn/ZoomOut/ZoomReset/CenterCursor`를 현재 뷰에 따라 나눈다.
  - (S3-T2) 배치 뷰에도 우클릭 마크 메뉴. `select::All`은 배치 뷰에서 클립 전부 선택. `transport::PlayFromCursor`는 배치 뷰 커서부터. `cursor::NextClip/PrevClip/ClipStart/ClipEnd`·마크는 배치 뷰 커서를 따로 두면 나눈다.
  - (S3-T4c, 결정 `2026-10-17-mode-entry-view.md`) 배치 뷰 Step의 글 표시를 격자 위 커서·고스트 클립으로 바꾼다.

## 끝의 정의 (내 말로)

1. 배치 뷰: `View::Arrangement`일 때 가운데 주 뷰가 자리표시자 대신 자체 `Element`를 그린다.
   - 세로는 트랙 행(프로젝트의 트랙 순서, 위가 첫 트랙), 가로는 틱. 행 높이는 고정(기본 28 px). 마디 선은 진하게, 박 선은 옅게(박자표 맵). 박 선은 한 박이 8 px보다 좁으면 그리지 않는다(피아노롤은 늘 그리지만 배치 뷰는 훨씬 넓은 범위를 보이므로).
   - 클립 블록: (트랙 행, 시작 틱, 길이)의 사각형. 색은 클립 색, 없으면 트랙 색(`Clip::color`가 `None`이면 트랙 색을 따른다는 모델 규칙). 블록 안 왼쪽 위에 이름(없으면 비움), 그 아래에 첫 마디(클립 시작부터 박자표 맵의 그 마디 끝까지. 마디 중간에서 시작한 클립이면 한 마디보다 짧다) 노트의 가사를 공백으로 이어 흐리게. 글은 블록 폭 안에서 자른다(말줄임 없음).
   - 커서: (현재 트랙, 커서 틱) 한 칸을 테두리로. 칸 너비는 Step 길이(`PianoRollState::step_ticks`)로 피아노롤과 같다.
   - 선택: 선택 클립은 `selection` 색 테두리와 밝기로 구분한다.
   - 재생 헤드: 피아노롤과 같은 규칙(재생 중이면 보간한 틱을 세로선으로 매 프레임, 일시정지면 그 자리, 정지면 없음. UI-4).
   - Step 모드(S3-T4c 이월): 커서 칸은 Step 길이 너비로 그리고, 커서가 클립 밖이면 「입력하면 생길 클립」(마디 a–b)을 고스트 블록(점선 테두리, 반투명)으로 그린다. 다음 클립에 막혀 쓸 수 없으면 고스트를 `danger` 색으로 그린다. 마디 계산은 `arrangement_step`과 같은 계획(`plan_step_write`)에서 가져온다. 기존 글 표시(`arrangement-step-cursor`·`arrangement-step-clip`)는 접근성 이름으로만 남긴다(보이지 않는 `test_support` 요소). `tests/arrangement_step.rs`의 기대는 바꾸지 않는다.
2. 커서는 창에 하나다(에이전트 해석). 배치 뷰 커서의 틱은 피아노롤 커서의 틱이고 트랙은 `current_track`이다. 따로 두지 않는 까닭: REQ 5.11(「트랙을 바꿀 때 커서의 시간 위치는 유지」), 배치 뷰 Step 입력이 이미 이 커서를 쓴다(S3-T4c), `Enter`로 들어갈 때의 자리(S3-T1 「클립 밖이면 첫 클립 시작」)가 같은 틱에서 이어진다. 따라서 이월 줄의 「따로 두면 나눈다」는 「따로 두지 않으므로 나누지 않는다」가 된다: `cursor::NextClip/PrevClip/ClipStart/ClipEnd`와 마크 점프는 배치 뷰에서도 그대로 동작하고, `transport::PlayFromCursor`는 두 뷰 모두 커서 틱부터 재생한다(지금은 배치 뷰에서 처음부터).
   - 배치 뷰 커서 키(REQ 5.10 표 「커서」 행, 키맵 `Normal > Arrangement`에 이미 묶여 있고 처리기만 없던 것): `cursor::PrevClipBoundary/NextClipBoundary`(현재 트랙의 클립 시작·끝 가운데 커서 앞·뒤의 가장 가까운 것. 그런 경계가 없으면 움직이지 않음. 0틱은 늘 경계), `cursor::TrackUp/TrackDown`(이웃 트랙으로 `current_track`을 옮김, 틱 유지), `cursor::FirstTrack/LastTrack`(이미 있음). `Ctrl+h/l`은 `Normal` 컨텍스트의 `cursor::PrevBar/NextBar`가 공유 커서를 옮기므로 그대로 쓴다.
   - 커서가 뷰포트 밖으로 가면 뷰포트가 따라간다(가로는 커서 칸이 보이게, 세로는 트랙 행이 보이게). `v`나 `Esc`로 배치 뷰에 올 때 커서 칸이 보이지 않으면 가운데로 둔다(S3-T1 리뷰 2 제안 1·2와 같은 규칙).
   - 커서가 하나이므로 따라가기도 두 뷰 모두 한다: 배치 뷰에서 커서를 옮기면 숨은 피아노롤 뷰포트도 커서를 따라가고(S3-T1부터 그랬다), 피아노롤에서 옮기면 배치 뷰 뷰포트는 그대로다가 `v`로 돌아올 때 커서가 안 보이면 가운데로 온다. 줌만 뷰별로 가른다(리뷰 1 제안 3).
3. 뷰 전환(단계 파일 게이트): `v`(`view::Toggle`)로 두 뷰를 오가고, 배치 뷰에서 `Enter`(`clip::Open`)로 피아노롤에 들어간다. 나올 때도 `v`다(REQ 5.10 「나올 때도 `v`다」). 단계 파일의 「`Esc` 복귀」는 REQ와 어긋나므로 하지 않는다. `Esc`는 지금처럼 선택 해제(`Normal > Selection`의 `select::Clear`)만 한다. 이 차이는 단계 파일에 적는다.
4. 선택 상태: 창이 `ClipSelection`(트랙별 `ClipId` 집합)을 갖는다. 이 Task는 `select::All`(배치 뷰에서 모든 트랙의 클립 전부)과 `select::Clear`(`Esc`)만 채우고, 모드 FSM의 `selection` 플래그를 클립 선택에도 맞춘다(선택이 있으면 컨텍스트 경로에 `Selection`). `Shift+이동` 확장은 S4-T2. 문서가 바뀌면(`revision`) 없어진 클립은 선택에서 뺀다.
5. 줌·스크롤: 배치 뷰는 자기 `px_per_tick`·가로 스크롤·세로 스크롤(트랙 행)을 갖는다. `view::ZoomIn/ZoomOut`(1.25배, 커서 칸 유지), `view::ZoomReset`, `view::CenterCursor`는 현재 뷰의 상태에만 적용한다(이월). 마우스 휠은 세로·가로 스크롤(`Shift`+휠 가로), `Cmd`+휠·핀치는 포인터 기준 줌(피아노롤 S3-T6a와 같은 규칙). 왼쪽 클릭은 커서를 (그 행의 트랙, Step 길이로 내림한 틱)으로 옮긴다. 러버밴드·드래그·더블클릭은 S4-T4.
6. 우클릭 메뉴(이월): 피아노롤과 같은 「마크 설정 ▸ a–z」「마크로 이동 ▸ a–z」. 클릭한 칸으로 커서를 옮긴 뒤 연다(`open_piano_roll_menu`와 같은 흐름, `PopupKind::ArrangementMenu`).
7. 성능: 장면(행·선·클립 블록·라벨·커서·고스트·재생 헤드)은 순수 함수 `arrangement::scene::build`가 만들고 `Element::paint`는 `paint_layer` 하나 안에서 `paint_quad`로 그린다. 보이는 클립은 트랙의 정렬된 클립 열(`Track::clips`는 시작 순, 겹침 없음 → 끝도 단조)에서 `partition_point`로 찾고 오른쪽 끝에서 멈춘다. 글(이름·가사)은 보이는 블록에만 `shape_line`으로 만들어 그린다. 매 프레임 할당은 재사용 버퍼로 막되, 글 모양 만들기는 보이는 블록 수에 비례한다(알려진 비용, 보고서에 적는다).
   - criterion 벤치 `arrangement/scene_64x200`: 트랙 64 × 클립 200이 모두 보이는 뷰포트에서 `scene::build`(단계 파일 게이트). 기준값은 사람 Mac에서 `scripts/bench-check.py --update`로 `benches/baseline.json`에 넣는다(기준값은 기계별이라 이 컨테이너에서 잡을 수 없다).

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/arrangement/mod.rs
pub struct ArrangementViewport { pub first_tick: f64, pub px_per_tick: f64, pub width: f32,
                                 pub first_row: f32, pub row_height: f32, pub height: f32 }
impl ArrangementViewport {
    pub fn x_of(&self, tick: f64) -> f32; pub fn y_of(&self, row: f32) -> f32;
    pub fn last_tick(&self) -> f64; pub fn rows_visible(&self) -> f32;
    pub fn cell_at(&self, x: f32, y: f32, step_ticks: u64) -> Option<(usize, Tick)>; // (트랙 행, 내림한 틱)
}
pub struct ClipSelection { /* BTreeMap<TrackId, BTreeSet<ClipId>> */ }
impl ClipSelection {
    pub fn is_empty(&self) -> bool; pub fn len(&self) -> usize;
    pub fn contains(&self, track: TrackId, clip: ClipId) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = (TrackId, ClipId)> + '_;
    pub fn set_all(&mut self, project: &Project); pub fn clear(&mut self);
    pub fn retain_existing(&mut self, project: &Project);
}
pub struct ArrangementState { /* Rc<RefCell<Inner>>: viewport, selection, scene 버퍼, painted, pending Follow */ }
impl ArrangementState {
    pub fn viewport(&self) -> ArrangementViewport;
    pub fn selection(&self) -> ClipSelection;            // 복제본
    pub fn zoom(&mut self, factor: f64, cursor: Tick); pub fn zoom_reset(&mut self, cursor: Tick);
    pub fn zoom_at(&mut self, factor: f64, x: f32);
    pub fn center_cursor(&mut self, cursor: Tick, row: usize);
    pub fn ensure_visible(&mut self, cursor: Tick, step_ticks: u64, row: usize);
    pub fn reveal_on_entry(&mut self, cursor: Tick, step_ticks: u64, row: usize);
    pub fn scroll_by(&mut self, dx_px: f32, dy_px: f32, rows: usize);
    pub fn painted_clips(&self) -> usize;                 // 시험용
}
pub fn clip_boundary_before(track: &Track, tick: Tick) -> Option<Tick>;
pub fn clip_boundary_after(track: &Track, tick: Tick) -> Option<Tick>;

// crates/keyscore-app/src/arrangement/scene.rs (순수, GPUI 없음)
pub struct ClipBlock { pub track: TrackId, pub clip: ClipId, pub rect: Rect, pub color: TrackColor,
                       pub selected: bool, pub name: Option<String>, pub lyric: Option<String> }
pub struct Ghost { pub rect: Rect, pub blocked: bool }
pub struct Scene { pub rows: Vec<Rect>, pub lines: Vec<GridLine>, pub clips: Vec<ClipBlock>,
                   pub cursor: Option<Rect>, pub ghost: Option<Ghost>, pub playhead: Option<f32> }
pub struct SceneInput<'a> { pub project: &'a Project, pub meter: &'a MeterMap,
    pub current_track: Option<TrackId>, pub cursor: Tick, pub step_ticks: u64,
    pub selection: &'a ClipSelection, pub ghost: Option<(Tick, Tick, bool)>, // (시작, 끝, 막힘)
    pub playhead: Option<Tick>, pub viewport: ArrangementViewport }
pub fn build(input: &SceneInput<'_>, out: &mut Scene);
pub fn first_bar_lyric(clip: &Clip, meter: &MeterMap) -> Option<String>;

// crates/keyscore-app/src/arrangement/element.rs
pub struct ArrangementElement { /* state, document, palette, playhead, animating, 콜백 */ }
impl Element for ArrangementElement { /* prepaint: scene::build, paint: paint_layer 안 paint_quad + 라벨 */ }

// ProjectWindow (window.rs, arrangement_window.rs)
pub fn arrangement(&self) -> &ArrangementState;
pub fn clip_selection(&self) -> ClipSelection;
pub(crate) fn step_ghost(&self, cx) -> Option<(Tick, Tick, bool)>;   // arrangement_step과 같은 계획
pub(crate) fn play_from_cursor_tick(&self) -> Tick;                  // 두 뷰 모두 커서 틱
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `scene_lays_out_clip_blocks_by_track_row_and_tick` | 단위(scene) | 트랙 셋·클립 여럿에서 블록 사각형이 (행, 틱→x, 길이→w)와 같고 색은 클립 색, 없으면 트랙 색 |
| 2 | `scene_skips_offscreen_clips_with_sorted_search` | 단위(scene) | 트랙 64 × 클립 200에서 일부만 덮는 뷰포트의 결과가 전수 검사와 같다(왼쪽 경계에 걸친 클립 포함) |
| 3 | `scene_labels_use_name_and_first_bar_lyric_only` | 단위(scene) | 이름 있음·없음, 첫 마디 가사만(둘째 마디 가사 제외), 가사 없는 노트는 건너뜀, 3/4에서 첫 마디 길이 |
| 4 | `scene_grid_lines_follow_meter_map_and_hide_dense_beats` | 단위(scene) | 3/4→4/4 변화 맵의 마디 선·박 선, 박이 8 px 미만이면 박 선 없음 |
| 5 | `scene_cursor_selection_ghost_playhead_positions` | 단위(scene) | 커서 칸(현재 트랙 행, Step 너비), 선택 클립 `selected`, 고스트 사각형·`blocked`, 재생 헤드 x, 뷰포트 밖이면 `None` |
| 6 | `clip_boundaries_on_current_track` | 단위 | 클립 시작·끝과 0틱이 경계, 클립 없으면 앞뒤 `None`(0틱 제외), 클립 안에서 앞 경계는 그 시작 |
| 7 | `v_toggles_views_enter_opens_clip_and_escape_keeps_view` | UI | 배치 뷰 → `v` 피아노롤 → `v` 배치 뷰, `enter`는 피아노롤, 피아노롤의 `escape`는 뷰를 바꾸지 않음, 배치 뷰로 올 때 커서 칸이 보임(단계 파일 게이트 + REQ 5.10) |
| 8 | `arrangement_cursor_keys_move_by_clip_boundary_track_and_bar` | UI | `h`/`l` 클립 경계, `j`/`k` 트랙(틱 유지), `ctrl-k`/`ctrl-j` 첫·끝 트랙, `ctrl-h`/`ctrl-l` 마디, 경계에서 멈춤 |
| 9 | `cursor_moves_scroll_arrangement_viewport_to_follow` | UI | 오른쪽 끝을 넘으면 `first_tick` 증가, 아래 트랙으로 내려가면 `first_row` 증가 |
| 10 | `zoom_keys_apply_only_to_the_visible_view` | UI | 배치 뷰에서 `cmd-=`는 배치 뷰 `px_per_tick`만 바꾸고 피아노롤은 그대로, 피아노롤에서는 반대(이월) |
| 11 | `select_all_in_arrangement_selects_every_clip_and_escape_clears` | UI | `cmd-a` 뒤 선택 수 = 전체 클립 수, 컨텍스트에 `Selection`, `escape` 뒤 비고 컨텍스트에서 빠짐. 피아노롤의 `cmd-a`는 그대로 노트 |
| 12 | `clip_selection_drops_removed_clips` | UI | 선택 뒤 클립을 지우면(문서 편집) 선택에서 빠짐 |
| 13 | `play_from_cursor_uses_cursor_tick_in_both_views` | 단위 | `play_from_cursor_tick`이 배치 뷰에서도 커서 틱(이월) |
| 14 | `right_click_in_arrangement_opens_mark_menu_and_sets_mark` | UI | 우클릭 → 팝업에 「마크 설정」「마크로 이동」, 커서가 클릭한 칸(트랙·틱)으로, 마크 설정 뒤 `mark('a')`의 트랙이 그 트랙 |
| 15 | `step_in_arrangement_draws_cursor_and_ghost_on_grid` | UI | Step 진입 뒤 장면에 커서, 클립 밖이면 고스트(마디 a–b), 막히면 `blocked`. 기존 `tests/arrangement_step.rs` 라벨 시험은 그대로 통과 |
| 16 | `click_places_cursor_and_wheel_scrolls` | UI | 클릭 → (트랙, Step 내림 틱), 휠 → 세로 스크롤, `shift`+휠 → 가로 |
| 17 | `element_paints_every_visible_clip_once` | UI | 헤드리스 `render_frame` 뒤 그린 블록 수 = 장면의 블록 수 |
| 18 | `playhead_follows_transport_in_arrangement` | UI | 일시정지 자리에 재생 헤드, 정지면 없음 |
| 19 | `frame_with_64x200_clips_fits_60fps_budget_in_release` | UI(release) | 프레임 중앙값 16.7 ms 안(UI-3 기준을 배치 뷰에도) |
| 20 | `arrangement/scene_64x200` | 벤치 | 트랙 64 × 클립 200이 모두 보이는 뷰포트의 `scene::build` 기준값 |

## 불변식

- 장면 계산과 그리기는 문서를 바꾸지 않는다. 이 Task에는 문서 편집이 없다.
- 커서 틱 ≥ 0(피아노롤 커서가 보장), 트랙 행은 0..트랙 수. 트랙이 없으면 커서 칸을 그리지 않는다. 줌은 상한·하한 사이.
- `ClipSelection`은 존재하는 클립만 가리킨다(`prepaint`에서 `revision`이 바뀌면 `retain_existing`).
- `paint`는 클립 수에 비례하는 정렬·할당을 하지 않는다(장면 버퍼 재사용, 글 모양은 보이는 블록만).
- 모든 quad는 `paint_layer` 하나 안에서 그린다(SP-2 조건). 글은 같은 layer 안에서 블록 뒤에 그린다.
- 모드 FSM의 `selection` 플래그는 「현재 뷰의 선택이 비어 있지 않음」과 같다(뷰를 바꾸면 `with_view`가 비우고, 두 선택도 함께 비운다).

## 범위 밖

- REQ 5.10 표의 `Shift`·`Alt`·`Alt+Shift`·`Alt+Ctrl` 행, 클립 편집 키(`x`, `Cmd+X/C/V/D`, `r`, `/`, `Cmd+J`), 비파괴 트림(S4-T2)
- `Tab`/`Shift+Tab` 트랙 이동, 새 트랙·삭제·순서(S4-T3)
- 러버밴드·드래그 이동·끝 트림·`Alt` 복제·더블클릭 생성·진입(S4-T4)
- `Shift+Space` 커서 클립부터 재생, `Shift+Enter` 선택 클립 루프(S5)
- 컨트롤러 레인·Sequence 그리드, 클립 블록 안의 노트 미리보기(그리지 않는다. 필요하면 S8 테마와 함께 본다)
- 트랙 행 높이·기본 배율 설정(S8 테마)

## 리스크·결정 요청

- 단계 파일의 「`Esc` 복귀」는 REQ 5.10 「나올 때도 `v`다」와 어긋난다. REQ를 따르고 단계 파일에 차이를 적는다. 결정 요청은 쓰지 않는다(REQ가 명시적이다).
- 커서를 창에 하나로 두는 것(2절)은 에이전트 해석이다(REQ 5.11의 시간 유지 규칙과 이미 있는 Step 동작에서 따랐다). 사람이 배치 뷰 커서를 따로 원하면 결정 요청을 쓴다.
- 행 높이 28 px, 박 선 숨김 기준 8 px, 기본 배율(피아노롤과 같은 0.1 px/틱 = 4분음표 96 px)은 REQ에 없는 세부 값이다. S8 테마와 함께 설정으로 뺄지 본다.
- Element 안의 글 그리기(`shape_line`)는 이 앱에서 처음이다. GPUI 스냅샷의 API 차이로 막히면 라벨을 Element 위에 겹치는 `div` 자식으로 바꾸되, 그 경우 보고서에 프레임 비용을 적는다.
- 이 세션의 컨테이너(Linux)는 앱 crate를 빌드하지 못한다(`objc2`·CoreMIDI). 타입 검사는 `--target aarch64-apple-darwin` 교차 검사로, 시험은 원격 CI(macos-15)와 사람 Mac으로 확인한다. 벤치 기준값과 release 프레임 시험(19)은 사람 Mac에서 잰다.
- 결정 요청 없음.
