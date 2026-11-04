# 적대적 리뷰: S4-T1 배치 뷰 Element (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 세션(구현 맥락 없이 코드·계획·보고서·REQ만 읽고 시작). 서브 에이전트.
- 대상 커밋: `b3e0b7c` (브랜치 `s4/t1-arrangement-element`, `main..HEAD` 10개 커밋)
- 추가한 시험: `crates/keyscore-app/tests/review_s4_t1_r1.rs` (12개. `brk_` 접두 2개는 코드 근거로 「실패할 것」이라고 본 시험이며 실패한 채로 둔다)
- 실행 환경 제약: 리뷰어 컨테이너는 Linux라 `keyscore-app`을 빌드·실행하지 못한다(`objc2`, CoreMIDI). 한 것은 (a) `CARGO_TARGET_DIR=target/alt cargo clippy -p keyscore-app --test review_s4_t1_r1 --target aarch64-apple-darwin -- -D warnings` 경고 0, `cargo fmt --all -- --check` 통과, (b) `cargo test -p keyscore-core --lib track` 16 통과(새 `clips_ending_after_skips_clips_that_end_at_or_before_tick` 포함), (c) 원격 CI 상태 조회(`gh api …/actions/runs`). 따라서 3절의 「깨짐/못 깨뜨림」은 실행 결과가 아니라 코드를 읽은 근거(어느 줄이 어떻게 동작해서)이고, 시험 실행은 구현자가 CI(macos-15)로 한다. 아래 각 시도에 근거 줄을 적었다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`crates/keyscore-app/src/arrangement/{mod,scene,element}.rs`, `arrangement_window.rs`, `window.rs`·`selection.rs`·`pianoroll_window.rs`·`transport.rs`·`step_input.rs`·`track_list.rs` 변경분, `keyscore-core/src/track.rs`의 `clips_ending_after`를 읽고 `tests/arrangement.rs`를 열기 전에 적었다.

1. 클립 경계 키(`clip_boundary_before/after`, `mod.rs:158-183`): 맞닿은 클립(앞 클립 끝 = 뒤 클립 시작)에서 `h`/`l`이 같은 자리에 두 번 멈추거나 건너뛰지 않는가. 클립 한가운데에서 `h`는 그 클립 시작, `l`은 그 클립 끝인가. 0틱에서 `h`, 마지막 경계 뒤에서 `l`은 움직이지 않는가.
2. 세로 따라가기의 범위 기준: `Inner::clamp_rows`(`mod.rs:300-303`)가 마지막 그리기 때의 트랙 수 `Inner::rows`로 자른다. 그리기 전에 트랙 수가 바뀌는 길(`track::New` → `set_current_track`, `window.rs:474-479`)에서 새 트랙 행이 뷰포트 밖에 남지 않는가.
3. 틱 u64 끝: `step_ghost`의 `Err` 분기(`step_input.rs:246-256`)가 `bar_start(bar + 1).map_or(at + bar_ticks, …)`로 끝을 잡는다. `map_or`의 기본값은 늘 계산되므로 `at + bar_ticks`가 넘치면 debug 패닉이다. `scene::block_rect`(`scene.rs:147-155`)의 `end - start`도 `end < start`면 언더플로다. 장면 계산(`build_lines`의 `bar + 1`, `tick_u` 포화, 거대한 `first_tick`)은 패닉·무한 루프 없이 끝나는가.
4. 트랙 0개·현재 트랙 없음: 커서 칸 없음(`scene.rs:111`), `cell_at`의 `rows = 0`(`mod.rs:84`), `move_track`·`jump_to_boundary`의 조기 반환, 클릭·우클릭·`Cmd+A`·줌·Step 진입이 패닉 없이 도는가.
5. 줌 상한·하한(`MAX_PX_PER_TICK` 2.0, `MIN_PX_PER_TICK` 0.0005)에서 `zoom`이 정확히 멈추고 커서 칸이 보이는가(상한에서 칸 1920 px은 뷰포트보다 넓으니 왼쪽 끝 맞춤 규칙 `mod.rs:252, 266-270`). `cmd-0` 뒤 보이는가.
6. 배치 뷰 진입(`reveal_on_entry`, `mod.rs:395-402`): 피아노롤에 있는 동안 커서가 가로로 멀리 가고 현재 트랙이 마지막 행이 됐을 때 `v`로 돌아오면 가로·세로 모두 보이는가(S3-T1 리뷰 1 지적 1의 배치 뷰 판).
7. FSM `selection` 플래그 = 현재 뷰의 선택 유무: `sync_selection_mode`(`selection.rs:140-147`)가 뷰별로 가른 뒤, Step 모드 왕복(`with_mode`는 선택을 지킨다), 커서 이동, 문서 변경(새 트랙), `Enter`·`v`·`w`(뷰 전환) 뒤에도 같은가. 배치 뷰에서 노트 선택이 생기는 길이 남아 있는가(`edit_allowed`가 피아노롤 밖 편집을 막는지, vim 비주얼이 `in_piano_roll_normal`로 막히는지).
8. 뷰 격리: 피아노롤 뷰의 이동·마크 점프·줌이 배치 뷰 뷰포트를 바꾸지 않는가(`cursor_moved_quietly`의 `in_arrangement()` 가드 `selection.rs:106`, `set_current_track`의 가드 `window.rs:477`, 줌 처리기 분기 `pianoroll_window.rs:236, 253, 270, 286`). 반대로 배치 뷰의 이동이 숨은 피아노롤 뷰포트를 움직이는 것(`selection.rs:103` `pianoroll.ensure_cursor_visible()`은 뷰를 가리지 않는다)이 의도인가.
9. 트랙 행 밖(마지막 행 아래) 클릭·우클릭이 커서·트랙·메뉴를 건드리지 않는가(`cell_at`의 `row >= rows` → `None`, `element.rs:422-434, 449-461`).
10. REQ 5.11: 배치 뷰 `Tab`/`Shift+Tab`(`track_next/prev`, `track_list.rs:233-243`)이 틱을 유지하는가.
11. `first_bar_lyric`(`scene.rs:193-212`): 「클립 시작부터 한 마디」(계획 끝의 정의 1)와 「클립 시작부터 그 마디 끝까지」(구현) 가운데 어느 것인가. 클립이 마디 중간에서 시작하면(`clip_for_notes`는 앞 클립이 마디 안에서 끝나면 그 끝에서 시작한다, `transform.rs:411-418`) 둘이 다르다. 박자표 변화가 첫 마디 안에 있을 때 `bar_start(bar + 1)`이 그 변화점인가.
12. `paint`의 할당: `paint_scene`은 quad만, 라벨은 보이는 블록마다 `SharedString::from(text.to_owned())`+`shape_line`(`element.rs:390-395`). 장면 버퍼는 재사용하되 `ClipBlock::name/lyric`은 매 프레임 새 `String`(`scene.rs:175`). 벤치(`benches/arrangement.rs`)가 라벨 경로를 재는가.
13. `RefCell` 이중 빌림: `prepaint`의 `borrow_mut`(`element.rs:205`) 동안 콜백이 없는가, `paint`의 `borrow()`(`element.rs:250`)가 `paint_labels`의 `shaped.paint`(GPUI 안)에서 되돌아오지 않는가, 클릭 처리기의 빌림이 `on_press` 호출 전에 끝나는가(`element.rs:422-425`).
14. 크기를 알기 전의 뷰포트 요청(`pending`, `mod.rs:216, 257, 285, 398`)과 상태 API의 극단값(`zoom(0.0)`, `zoom_at(INF)`, `scroll_by(f32::MAX)`)이 패닉 없이 범위 안에 머무는가.
15. `v`/`Enter`/`Esc` 전이와 「나올 때도 `v`」(REQ 5.10), 줌 키 분리(이월), `Cmd+A`(이월), `PlayFromCursor`(이월), 우클릭 마크 메뉴(이월), Step 고스트(이월)가 모두 있는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 맞닿은 클립의 경계 | 부분 | 시험 6·8은 클립 사이에 빈 마디가 있다. 맞닿은 경우(앞 끝 = 뒤 시작)는 없다 |
| 2. 그리기 전 트랙 수 변화 뒤 따라가기 | 없음 | 시험 9는 트랙 수가 고정이다 |
| 3. 틱 u64 끝 | 없음 | 계획 「불변식」에 「커서 틱 ≥ 0」만 있다 |
| 4. 트랙 0개 | 없음 | 시험 5가 `current: None`의 장면만 본다 |
| 5. 줌 상한·하한 | 없음 | 시험 10은 한 단계씩만 |
| 6. 먼 커서·마지막 행으로 진입 | 부분 | 시험 7은 커서가 0틱·첫 행이라 `reveal_on_entry`의 가운데 맞춤 분기가 돌지 않는다 |
| 7. FSM 플래그 일치 | 부분 | 시험 11·12는 `Cmd+A`·`Esc`·뷰 전환·문서 변경. Step 왕복·커서 이동 뒤는 없다 |
| 8. 뷰 격리 | 부분 | 시험 10은 줌만. 피아노롤의 이동·마크 점프가 배치 뷰 뷰포트를 건드리지 않는 것은 없다 |
| 9. 행 밖 클릭 | 부분 | 시험 20은 `cell_at`만. 실제 클릭·우클릭 이벤트는 행 안만(시험 14·16) |
| 10. 배치 뷰 `Tab` 시간 유지 | 없음 | 보고서가 「`Tab`은 아직 피아노롤 규칙 그대로」라고만 적었다 |
| 11. 마디 중간 시작 클립의 첫 마디 가사 | 없음 | 시험 3은 마디 시작 클립뿐 |
| 12. 라벨 경로 비용 | 없음 | 벤치는 블록 8 px(< `MIN_LABEL_PX` 24)라 라벨을 한 번도 만들지 않는다(아래 제안 2) |
| 13. RefCell | 없음(코드로 판단) | 문제 없음: `prepaint`의 빌림 안에서 부르는 것은 `scene::build`(순수)뿐, `paint`의 빌림은 `paint_layer` 클로저 안에서 끝나고 `register_mouse`는 그 뒤, 클릭 처리기는 블록으로 빌림을 끝낸 뒤 `on_press` |
| 14. 크기 전 요청·극단값 | 없음 | |
| 15. 전이·이월 항목 | 있음(시험 7·10·11·13·14·15·16) | |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s4_t1_r1.rs`에 있다. 이 컨테이너에서는 실행하지 못했다(위 「실행 환경 제약」). 「결과」는 코드를 읽은 예상이며 근거 줄을 함께 적는다. 구현자가 CI로 돌린 결과가 다르면 그 시도의 근거를 다시 본다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클립 밖에서 커서를 `u64::MAX − 100`에 두고 배치 뷰 Step 진입(고스트 계산) | 깨짐(코드 근거). `next_step_write` → `plan_step_write` → `step_host` = `Gap` → `Note::new(at, 960)`의 `checked_add`가 `TickOverflow`(`note.rs:185`) → `step_ghost`의 `Err(_)` 분기(`step_input.rs:246`) → `.map_or(at.get() + bar_ticks, Tick::get)`(`:252`)의 기본값이 먼저 계산돼 `18446744073709551515 + 3840` 오버플로. debug(`cargo test`)는 패닉, release(`[profile.release]` 없음 → 검사 끔)는 감겨 `end` 3739 < `start` 18446744073709551360이 되고 `scene::block_rect`의 `end - start`(`scene.rs:148`)가 다시 감긴다. 시험은 debug에서 패닉으로, release에서 `end >= start` 단언으로 실패한다 | `brk_step_ghost_near_u64_end_does_not_overflow` |
| 2 | 트랙 40개(창보다 많음), `ctrl-j`로 마지막 트랙 → `cmd-t`(새 트랙, 현재 트랙이 됨) 뒤 새 행이 보이는가 | 깨짐(코드 근거). `insert_new_track`(`track_list.rs:73-81`)은 `edit_document` 직후(그리기 전) `set_current_track`을 부르고, 배치 뷰면 `ensure_visible`(`window.rs:477-478`)이 `first_row = 41 − visible`로 두지만 `clamp_rows`(`mod.rs:300-303`)가 마지막 그리기 때의 `rows` 40으로 `max_first = 40 − visible`에 자른다. 다음 `prepaint`의 `set_size`(`mod.rs:318`)는 범위만 넓히고 따라가기를 다시 하지 않는다. 행 40의 아래쪽 41 > `first_row + visible` = 40 | `brk_new_track_beyond_viewport_bottom_is_revealed` |
| 3 | 배치 뷰 `tab`/`shift-tab` 네 번 뒤 틱 | 못 깨뜨림(코드 근거). `track_next/prev`(`track_list.rs:233-243`)는 `select_track_at`만 부르고 커서를 건드리지 않는다 | `tab_in_arrangement_keeps_cursor_tick` |
| 4 | 트랙 0개 프로젝트에서 커서 키·`Cmd+A`·`Esc`·줌·가운데·마크·클릭·우클릭·Step 진입 | 못 깨뜨림(코드 근거). `jump_to_boundary`·`move_track`은 트랙이 없으면 반환, `select_track_at`은 `tracks.get(0)` = `None`, `cell_at(rows = 0)` = `None`, `step_ghost`는 `current_track()?`로 `None`, 장면은 행·커서 없음 | `empty_project_arrangement_keys_and_mouse_do_not_panic` |
| 5 | `cmd-=` 40번·`cmd--` 60번·`cmd-0` 뒤 배율이 정확히 상한·하한·기본이고 커서 칸(2마디)이 보이는가 | 못 깨뜨림(코드 근거). `zoom`(`mod.rs:405-421`)은 `clamp` 뒤 같은 값이면 반환, 아니면 `ensure_visible`. 상한에서 칸 1920 px > 뷰포트는 `fit.min(tick)`으로 왼쪽 끝 맞춤(`:266-270`) | `zoom_keys_clamp_at_bounds_and_keep_cursor_cell_visible` |
| 6 | 피아노롤에서 커서를 100마디·마지막 트랙(40개 중)으로 보낸 뒤 `v`로 돌아올 때 커서 칸이 보이는가, 그 사이 배치 뷰 뷰포트는 그대로인가 | 못 깨뜨림(코드 근거). 피아노롤에 있는 동안 `in_arrangement()` 가드로 배치 뷰 상태는 그대로(`selection.rs:106`, `window.rs:477`), 진입 때 `reveal_on_entry` → `cell_visible` 거짓 → `center`(`mod.rs:283-297`) → `clamp_rows` → `ensure_visible` | `entering_arrangement_with_far_cursor_and_last_track_reveals_cell` |
| 7 | `Cmd+A` 뒤 Step 진입·복귀·`l j`·`cmd-t`(문서 변경)·`Esc`·`Enter`에서 FSM 플래그 = 클립 선택 유무, 노트 선택은 비어 있음 | 못 깨뜨림(코드 근거). `with_mode`는 선택을 지키고(`mode.rs:261-`), `cursor_moved_quietly`·`document_changed`가 `sync_selection_mode`를 부르며, `OpenClip`의 `with_view`가 플래그를 끄면 `apply_mode_event`(`window.rs:355-358`)가 클립 선택을 비운다. 배치 뷰에서 노트 선택이 생기는 길은 `edit_allowed`(`note_edit.rs:60`)와 `toggle_visual`(`vim.rs:928`)이 막는다 | `clip_selection_and_mode_flag_stay_consistent_through_step_mode` |
| 8 | 피아노롤 뷰의 `shift-n ctrl-l cmd-right cmd-down cmd-up`·마크 설정·점프·`cmd-=` 뒤 배치 뷰 뷰포트 | 못 깨뜨림(코드 근거). 모두 `cursor_moved`/`set_current_track`/`view_zoom_in`을 지나고 각각 `in_arrangement()`로 가른다 | `cursor_moves_in_piano_roll_do_not_touch_arrangement_viewport` |
| 9 | 맞닿은 클립 `[1마디, 2마디)`·`[2마디, 3마디)`에서 `l`×4·`h`×4, 클립 한가운데에서 `h`·`l` | 못 깨뜨림(코드 근거). `clips_ending_after(2마디)`는 끝 = 틱인 앞 클립을 빼고(`track.rs:310`), `clip_boundary_before`는 `c.end() < tick`만 경계로 세어(`mod.rs:166`) 같은 자리에 머물지 않는다 | `clip_boundary_keys_with_adjacent_clips` |
| 10 | 트랙 3개에서 행 3(트랙 없음) 자리를 클릭·우클릭 | 못 깨뜨림(코드 근거). `cell_at`의 `row >= rows` → `None`(`mod.rs:84-86`), `arrangement_press`는 `cell`이 없으면 반환(`arrangement_window.rs:224`), 우클릭도 `cell`이 있을 때만(`element.rs:458`) | `clicks_outside_track_rows_do_nothing` |
| 11 | 장면 계산: 커서·재생 헤드 `u64::MAX`, `first_tick` `u64::MAX − 10⁵`·`10³⁰`·`f32::MAX`, 세 배율, u64 끝에 닿는 클립과 고스트, 격자선 틱 단조 증가, 크기·배율 0 | 못 깨뜨림(코드 근거). `tick_u` 포화, `bar_start`의 `checked_mul/checked_add`가 `None`이면 `build_lines`가 끝나고(`scene.rs:255-257`), 박 선은 `checked_add` 실패에서 끊긴다(`:235`). 고스트는 `end ≥ start`인 입력이라 `block_rect`가 안전하다(시도 1의 경로만 `end < start`를 만든다) | `scene_at_tick_max_and_huge_viewport_does_not_panic` |
| 12 | 크기를 알기 전 상태 API 극단값(`zoom(1e300)`·`zoom(0)`·`zoom_at(INF)`·`scroll_by(±f32::MAX)`·`zoom_reset`) 뒤 배율·첫 틱·첫 행이 범위 안이고 유한한가 | 못 깨뜨림(코드 근거). `clamp`와 `max(0.0)`, `clamp_rows`(`rows` 0 → 0) | `state_api_before_first_paint_does_not_panic_and_keeps_bounds` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 선택은 존재하는 클립만 | 잠시 가능, 다음 그리기에서 복구 | `ArrangementState::set_selection`(pub, `mod.rs:367`)에 아무 id나 넣을 수 있다. `prepaint`가 `revision`이 바뀌었을 때만 `retain_existing`(`element.rs:214-217`)하므로 revision이 그대로면 낡은 id가 장면에 남지만 `scene::build`는 `selection.contains`로 읽기만 해 영향이 그리기 표시뿐이다. `document_changed`(`selection.rs:183-188`)가 문서 변경 때 걸러 FSM도 맞춘다. 계획의 문구(「다음 문서 변경 때 걸러진다」)와 같다. 제안 4 |
| FSM `selection` 플래그 = 현재 뷰의 선택 유무 | 같은 pub `set_selection`으로 가능(다음 `sync_selection_mode`까지) | 창의 길(`set_clip_selection`, `select_all`, `select_clear`, `cursor_moved_quietly`, `document_changed`, `apply_mode_event`)은 모두 맞춘다. 배치 뷰에서 노트 선택이 생기는 길은 없다(`edit_allowed`, `toggle_visual`). 시도 7 |
| 장면 계산·그리기는 문서를 바꾸지 않음 | 불가 | `SceneInput.project: &Project`, `prepaint`는 `document.read` |
| `paint`는 클립 수에 비례하는 정렬·할당 없음 | 보이는 라벨 블록 수에 비례하는 할당은 있음(계획이 허용) | `paint_scene`은 `paint_quad`뿐(`element.rs:277-354`). 라벨은 `SharedString::from(text.to_owned())`+`shape_line`(`:390-395`). `prepaint`의 `scene::build`도 라벨 블록마다 `String` 둘(`scene.rs:175`). 폭 24 px 미만이면 없음. 벤치는 이 경로를 재지 않는다(제안 2) |
| 모든 quad는 `paint_layer` 하나 안 | 불가 | `element.rs:252-254`. 라벨의 `with_content_mask`는 레이어를 바꾸지 않는다(GPUI `window.rs:4029-4044`는 `content_mask_stack`만 밀고, `paint_layer`는 `:4399-4417`) |
| 커서 행은 `0..트랙 수`, 트랙 없으면 커서 칸 없음 | 불가 | `current_row`는 `position`(`scene.rs:91-93`), `cell_at`의 `row >= rows` |
| 줌은 상한·하한 사이 | 불가 | `zoom`·`zoom_at`의 `clamp`, `zoom_reset`은 기본값. 시도 5·12 |
| 커서 틱 ≥ 0, 뷰포트 첫 틱·첫 행 ≥ 0 | 불가 | `Tick`은 u64, `first_tick`은 `max(0.0)`, `clamp_rows` |
| 뷰포트가 커서 칸을 따라감(계획 끝의 정의 2) | 가능 | 시도 2: 그리기 전에 트랙이 늘면 `clamp_rows`가 낡은 `rows`로 자른다. 지적 2 |
| 틱 산술에 오버플로 없음 | 가능 | 시도 1: `step_ghost`의 `at + bar_ticks`. 지적 1 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.10 두 뷰 전환 `v`, 배치 뷰 `Enter`로 진입, 나올 때도 `v` | 충족 | `window.rs:919-920`(`clip::Open`, `view::Toggle`), `mode.rs:208-214`. 단계 파일의 「`Esc` 복귀」는 REQ와 어긋나 하지 않음(단계 파일 `14-S4:14`에 적음). 시험 7 |
| 5.10 배치 뷰 키 표 「(없음) 커서」 행: `h`/`l` 이전·다음 클립 경계, `j`/`k` 위·아래 트랙, `Ctrl+h/l` 마디 | 충족 | `default.json:147-156`, `arrangement_window.rs:63-127`, `mod.rs:158-183`. 시험 8, 시도 9 |
| 5.10 「`Ctrl` 맨 위/맨 아래 트랙」 | 충족 | `default.json:155-156` → `track_first/last`(`track_list.rs:245-261`). 시험 8 |
| 5.10 클립 규칙 「색은 트랙 색을 기본으로」 | 충족 | `scene.rs:183` `clip.color().unwrap_or_else(track.color)`. 시험 1 |
| 5.10 클립 밖 Step 입력 → 마디 단위 새 클립(표시) | 충족(표시 범위) | `step_ghost`(`step_input.rs:219-258`)가 `plan_step_write`의 `InsertClip`을 그대로 쓴다. 시험 15. 클립 끝 이어 쓰기(「클립이 마디 단위로 늘어난다」)는 고스트가 없다(제안 5) |
| 5.10 `Shift+이동`·`Delete`·`Cmd+X/C/V/D`·`r`·`/`·`Cmd+J`·`Alt` 행 | 범위 밖(S4-T2) | 계획 「범위 밖」. 배치 뷰에서 `x`·`cmd-v` 등은 `edit_allowed`가 막고 안내한다(`note_edit.rs:60`) |
| 5.11 「Normal · 배치 뷰: `Tab` 다음 트랙」, 「트랙을 바꿀 때 커서의 시간 위치는 유지」 | 충족(시간 유지), 음높이 규칙은 S4-T3 | `track_list.rs:233-243`, `arrangement_window.rs:101-109`. 시도 3, 시험 8 |
| 7.1 「배치 뷰: 트랙×시간 격자에 클립 블록」, 메인 영역에서 `v`로 전환 | 충족 | `scene.rs:79-137`, `window.rs:1319-1322` |
| 7.6 「피아노롤 ↔ 배치 뷰: 메인 영역 상단의 세그먼트 버튼, 메뉴 보기, `v`」 | 부분 충족(메뉴·키는 있음, 세그먼트 버튼 없음) | `commands/table.rs:1967`(메뉴 보기 > 배치 뷰). 세그먼트 버튼은 이 Task 계획에도, 단계 파일에도 없다. UI-11(적어도 한 곳)은 메뉴로 충족. 제안 6으로 이월 자리를 정한다 |
| UI-3 키 입력 뒤 60 fps | 확인 대기 | 시험 19는 release·사람 Mac 몫(계획 리스크 절). 보고서 「자동 게이트」의 빈칸. 지적 3 |
| UI-4 재생 헤드 모노토닉 보간 | 충족 | `piano_roll_playhead()` 공유, `animating` → `request_animation_frame`(`element.rs:257-259`). 시험 18 |
| 9.4 자체 Element | 충족 | `element.rs` |
| SEQ-2 클립 모델 | 충족(core 그대로) | 이 Task는 모델을 바꾸지 않는다. `clips_ending_after`는 읽기 전용 |

「부분 충족」은 미충족으로 센다: 7.6 세그먼트 버튼은 이 Task의 범위가 아니었으므로 판정에 넣지 않고 이월 제안으로 둔다. UI-3은 측정이 끝나야 닫힌다(지적 3).

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `transport.rs` 변경은 `play_from_cursor_tick`(UI 스레드, `play_from`의 인자)뿐이고 엔진 코드는 닿지 않았다. `git diff main...HEAD --stat`에 `engine.rs`·`keyscore-midi` 변경 없음.
- 제품 코드의 `unwrap`/`expect`: `arrangement/*.rs`·`arrangement_window.rs`·변경분에 없음. `unwrap_or`·`unwrap_or_else`만: `scene.rs:183`(클립 색 → 트랙 색, 모델 규칙), `arrangement_window.rs:138`(`u8::try_from(0..12)`은 늘 Ok), `step_input.rs:249`(`bar_start(bar)`는 `position(at).bar()`에서 왔으므로 늘 Some. `unwrap_or(at)`는 안전한 대체).
- 에러가 삼켜지는 곳:
  - `element.rs:398` `let _ = shaped.paint(...)`: 글꼴 없음 등. 보고서가 밝혔고 라벨은 보조 정보라 받아들인다.
  - `step_input.rs:246` `Err(_)`: `plan_step_write`의 모든 오류를 「막힘」 고스트로 뭉갠다. 지금 도달 가능한 오류는 다음 클립 충돌과 틱 오버플로(시도 1)뿐이라 표시로는 맞지만, 오버플로 경우를 같은 분기에서 다시 넘친다(지적 1).
  - `arrangement_window.rs:76-78` 경계 없음 → 조용히 머묾(의도, 계획 2절).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `step_ghost`의 `Err` 분기가 `bar_start(bar + 1).map_or(at.get() + bar_ticks, Tick::get)`로 끝을 잡는다. `map_or`의 기본값은 `Some`이어도 먼저 계산되므로 커서가 u64 끝에서 `bar_ticks` 안이면 덧셈이 넘친다. debug(`cargo test`·개발 빌드)는 패닉이고 release는 감겨 `end < start`가 되며 `scene::block_rect`의 `end - start`가 다시 감겨 폭이 10¹⁸ px인 고스트가 된다. 클립 밖 커서는 `Note::new`의 `checked_add`가 `TickOverflow`를 돌려 이 분기로 들어온다. 도달: u64 끝에 닿는 클립은 모델상 유효하고(`Clip::new`는 `start + length ≤ u64::MAX`만 본다) `.ksq`로 들어올 수 있으며, `shift-n`·`cmd-right`로 커서가 그 근처에 간 뒤 배치 뷰에서 Step에 들어가면 된다. 재현: `brk_step_ghost_near_u64_end_does_not_overflow`. 고치는 길: `map_or_else(|| at.get().saturating_add(bar_ticks), Tick::get)`(또는 `checked_add` 실패면 `u64::MAX`), 그리고 `block_rect`는 `end.saturating_sub(start)`로 입력을 믿지 않거나 `StepGhost`가 `end ≥ start`를 생성자에서 보장한다 | `step_input.rs:246-256`, `scene.rs:147-155` |
| 2 | 수정 | 배치 뷰의 세로 따라가기가 마지막 그리기 때의 트랙 수(`Inner::rows`)로 범위를 자른다. `track::New`(`Cmd+T`, vim `o`/`O`)는 문서에 트랙을 넣은 직후, 아직 그리기 전에 `set_current_track`으로 새 트랙을 현재 트랙으로 삼고, `set_current_track`의 `ensure_visible`이 `clamp_rows`에서 낡은 `rows`에 걸려 새 행(번호 `rows`)이 뷰포트 바로 아래에 남는다. 다음 `prepaint`의 `set_size`는 `clamp_rows`만 하고 따라가기를 다시 하지 않는다. 트랙 수가 보이는 행 수보다 많을 때 드러난다(트랙 40개에 창은 20여 행). 계획 끝의 정의 2 「커서가 뷰포트 밖으로 가면 뷰포트가 따라간다」와 어긋난다. S4-T3(새 트랙)이 바로 이 길을 쓴다. 재현: `brk_new_track_beyond_viewport_bottom_is_revealed`. 고치는 길 예: (a) `Cell`에 트랙 수를 넣거나 `ArrangementState::set_rows(n)`을 두어 `arrangement_cell`이 문서의 트랙 수를 함께 넘기고 `ensure_visible`이 그 값으로 자른다, (b) `ensure_visible`이 `max_first = max(rows, row + 1) − visible`로 자른다(요청한 행은 늘 들어가게), (c) `ensure_visible`이 요청을 `pending`에 남기고 `set_size`가 크기가 바뀌지 않아도 `pending`을 적용한다 | `window.rs:474-479`, `mod.rs:271-281, 300-303, 306-319` |
| 3 | 수정 | 자동 게이트가 닫히지 않았다. 보고서는 「CI 실행 27(lockfile 수정) 대기」라 적었는데 `gh api …/actions/runs?branch=s4/t1-arrangement-element`로 보면 실행 27(`10321ec`)은 `cancelled`, 실행 28(`b3e0b7c`)은 `queued`다. 즉 `cargo deny check`가 통과한 실행이 아직 없다(실행 26은 yank로 실패). 또 단계 파일 게이트의 벤치 `arrangement/scene_64x200` 기준값은 `benches/baseline.json`에 없고(`pianoroll/scene_10k_notes`만) release 프레임 시험(19)도 결과가 없다. 병합 조건 「자동 게이트 통과」를 만족하려면 (1) 실행 28(또는 재실행)의 deny 통과, (2) 사람 Mac의 `python3 -I scripts/bench-check.py --update`로 기준값 커밋, (3) `cargo test -p keyscore-app --release --test arrangement frame_with_64x200`의 중앙값·최대값을 보고서 「테스트 결과」에 적는다. 지적 1·2의 수정과 이 리뷰 시험(`review_s4_t1_r1`)의 CI 결과도 같은 실행에서 본다 | `docs/reports/s4-t1/report.md` 「자동 게이트」, `benches/baseline.json` |

제안(구현자가 거절할 수 있음. 거절 이유를 보고서에 적는다):

| # | 내용 | 파일:줄 |
| --- | --- | --- |
| 제안 1 | `first_bar_lyric`은 「클립 시작부터 그 마디 끝까지」인데 계획 끝의 정의 1은 「첫 마디(클립 시작부터 한 마디)」라 적었고 보고서 「계획 대비 달라진 점」에는 없다. 클립이 마디 중간에서 시작하면(`clip_for_notes`는 앞 클립이 마디 안에서 끝나면 그 끝에서 새 클립을 시작한다, S3-T3a 리뷰 1 지적 2) 둘이 다르다: 앞 클립이 셋째 박에서 끝나면 새 클립의 「첫 마디」 가사는 두 박만이다. 단계 파일 「첫 소절 가사」는 어느 쪽도 가능하다. 어느 쪽이든 결정을 계획 문구와 보고서에 적고, 마디 중간 시작 클립의 시험을 하나 둔다(`tests/arrangement.rs` 시험 3은 마디 시작 클립뿐) | `scene.rs:193-212`, `docs/reports/s4-t1/plan.md:17` |
| 제안 2 | 벤치 `arrangement/scene_64x200`의 배율은 1600 px에 200마디(블록 8 px)라 `MIN_LABEL_PX` 24 미만이고 라벨(이름·가사)을 한 번도 만들지 않는다. 벤치 데이터의 「클립마다 노트 넷(첫 마디 가사 셋)」과 「절」 이름은 죽은 데이터다. 보고서가 「알려진 한계」로 적은 라벨 비용(`String` 둘 + `shape_line`, 보이는 블록 수에 비례)은 어디서도 재지 않는다. 기본 배율 근처(블록 384 px, 가로 4마디 × 세로 36행 ≈ 150 라벨)의 벤치를 하나 더 두거나, release 프레임 시험(19)에 라벨이 보이는 배율의 프레임을 함께 재면 「알려진 비용」이 숫자가 된다 | `benches/arrangement.rs:55-63`, `scene.rs:174-178`, `element.rs:359-403` |
| 제안 3 | `cursor_moved_quietly`는 배치 뷰에서도 `self.pianoroll.ensure_cursor_visible()`을 불러 숨은 피아노롤 뷰포트를 커서 뒤로 끌고 간다(`v`로 돌아갈 때 피아노롤의 스크롤 자리가 배치 뷰에서 한 이동만큼 바뀐다). 공유 커서 해석(계획 2절)에서는 자연스럽고 `entered_piano_roll`의 `reveal_cursor_on_entry`가 어차피 커서를 보이게 하므로 결함은 아니다. 다만 줌 키는 뷰별로 갈랐으면서 따라가기는 두 뷰에 다 하므로, 의도라면 계획 2절에 한 줄로 적어 두고 아니면 `in_arrangement()`로 가른다 | `selection.rs:103-109` |
| 제안 4 | `ArrangementState::set_selection`이 `pub`이라 창의 `set_clip_selection`(FSM 동기화)을 거치지 않고 선택을 바꿀 수 있다. `PianoRollState::set_selection`도 같은 꼴이라 선례를 따랐지만, S4-T2가 `Shift+이동`을 붙이기 전에 `pub(crate)`로 내리고 시험은 `set_clip_selection`·`clip_selection`만 쓰게 하면 「FSM 플래그 = 선택 유무」가 공개 API로는 깨지지 않는다 | `mod.rs:366-369` |
| 제안 5 | Step 고스트는 「클립 밖 → 새 클립」과 「막힘」만 그린다. 커서가 클립 끝에 있어 이어 쓰기가 되는 경우(REQ 5.10 「입력이 클립 끝을 넘으면 클립이 마디 단위로 늘어난다」)는 고스트가 없어 다음 확정이 클립을 한 마디 늘린다는 것이 격자에 보이지 않는다(글 표시 `arrangement_step`은 「클립 … 끝」이라고만 한다). `plan_step_write`가 돌려주는 `Edit`에서 `ResizeClip`(또는 그에 해당하는 편집)을 읽어 늘어나는 구간을 같은 고스트로 그리면 S3-T4c 이월 「입력하면 생길 클립」의 뜻이 끝 이어 쓰기에도 이어진다 | `step_input.rs:238-245` |
| 제안 6 | REQ 7.6은 뷰 전환의 눈에 보이는 자리로 「메인 영역 상단의 세그먼트 버튼」을 든다. 지금은 메뉴 보기 > 배치 뷰와 `v`만 있어 UI-11의 「적어도 한 곳」은 넘지만 7.6의 자리는 비어 있다. 이 Task 범위가 아니었으므로 판정에 넣지 않는다. S4 출구 게이트나 S8(테마·설정) 줄에 「메인 영역 상단 세그먼트 버튼(피아노롤 | 배치 뷰)」을 이월 항목으로 적어 둔다 | `plan/stages/14-S4-clips-and-arrangement.md` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(틱 오버플로 → 패닉)과 지적 2(새 트랙 뒤 따라가기 실패)는 공개 API·키 입력으로 재현되는 결함이고, 지적 3은 병합 조건(자동 게이트 통과)이 아직 닫히지 않은 것이다. 셋을 고치고 `review_s4_t1_r1`의 `brk_` 시험 2개가 통과하며 나머지 10개가 회귀 없이 통과하는 것을 CI로 보인 뒤 재리뷰한다. 제안 6건은 병합을 막지 않는다. `review_s4_t1_r1.rs`는 PR에 넣는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [ ] 깨뜨리는 테스트를 실제로 실행했다 — 실행 불가(Linux 컨테이너, 앱 crate 빌드 불가). 대신 교차 타입 검사·clippy·fmt를 통과시켰고 각 시도의 근거 줄을 적었다. 실행은 구현자의 CI 몫이며 결과가 예상과 다르면 재리뷰에서 다시 본다
- [x] REQ ID를 하나씩 대조했다(5.10 항목별, 5.11, 7.1, 7.6, UI-3, UI-4, 9.4, SEQ-2)
- [x] `git diff`에 Task 범위 밖의 변경이 없다 — `Cargo.lock`의 `async-executor 1.14.0 → 1.14.1`(yank 대응, 보고서에 적음)과 `tempfile`의 `getrandom 0.4.3 → 0.3.4` 되돌림이 함께 들어 있다. 후자는 lockfile 갱신의 부수 효과로 보이나 보고서에 없다. 새 crate는 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다 — `git diff main...HEAD -- crates/keyscore-app/tests`의 삭제 `fn`은 `brk_cmd_a_in_arrangement_view_does_nothing`의 이름 바꿈 1건뿐(기대를 이월 줄대로 고침, 커밋 9323162 본문). `ignore]` 검색 결과 없음
- [x] 새 의존성이 있으면 라이선스를 확인했다 — 새 의존성 없음(`Cargo.toml`은 `[[bench]]` 추가뿐). `async-executor 1.14.1`은 기존 crate의 패치 판(MIT/Apache-2.0)
- [x] 스타일 지적을 적지 않았다
