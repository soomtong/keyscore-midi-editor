# Task 보고: S4-T1 배치 뷰 Element

- PR: https://github.com/soomtong/keyscore-midi-editor/pull/4 (브랜치 `s4/t1-arrangement-element`)
- 계획 파일: docs/reports/s4-t1/plan.md
- 계획 대비 달라진 점
  - `ArrangementState`의 커서 관련 메서드는 (틱, 스텝, 행)을 따로 받지 않고 `Cell { tick, step_ticks, row }` 하나를 받는다(`ensure_visible(cell)`, `center(cell)`, `reveal_on_entry(cell)`, `zoom(factor, cell)`). 계획의 `scroll_by`는 행 수를 받지 않고 마지막 그리기 때의 트랙 수(`Inner::rows`)로 범위를 자른다.
  - `Viewport::cell_at`가 트랙 수(`rows`)도 받아 트랙 행 밖을 `None`으로 돌려준다(빈 자리 클릭·우클릭은 아무 일도 하지 않는다).
  - 계획의 `ClipSelection::iter`·`insert`를 더했다(시험·S4-T2용). `Scene`의 행 띠는 `RowBand { index, rect, current }`다(계획은 `Vec<Rect>`).
  - `cursor_moved_quietly`(selection.rs)가 배치 뷰에서는 배치 뷰 뷰포트의 따라가기까지 한다. 마디 이동·마크 점프·검색 이동처럼 공유 커서를 옮기는 모든 길이 이 함수를 지나므로 배치 뷰 전용 처리기에 따로 두지 않았다. `set_current_track`도 배치 뷰에서는 커서 행을 보이게 한다(트랙 목록 클릭·`Ctrl+j/k`).
  - 배치 뷰 Step 고스트(`step_ghost`)는 `plan_step_write`가 거부하면 커서가 든 마디를 `blocked`로 돌려준다(계획은 「막힌 고스트」만 적고 범위는 정하지 않았다).
  - core에 `Track::clips_ending_after(tick)`를 더했다(클립 열이 비공개라 `partition_point`를 트랙 안에서 한다). 계획의 인터페이스 초안에는 없던 것이라 계획 7절 설명과 맞춘다.
  - 자동 게이트 실행 환경: 이 Task의 구현 세션은 Linux 컨테이너라 앱 crate를 빌드하지 못했다(`objc2`·CoreMIDI). 「자동 게이트」 절에 무엇을 어디서 돌렸는지 적는다.
  - 배치 뷰의 `main-view`는 피아노롤과 달리 4 px 여백 바깥(패널 크기)이고 Element가 그 안쪽이다(S2-T6f 리뷰 3 시험이 패널 최소 너비를 `main-view`로 잰다). 배치 뷰 좌표 시험은 여백을 더한다.
  - 기존 시험 둘을 고쳤다(지우지 않음, 커밋 9323162 본문): `review_s3_t1_r2`의 「그리기 전 줌」은 배치 뷰 `cmd-=`가 이제 배치 뷰를 줌하므로 피아노롤 상태 API로 줌한다. `review_s3_t2`의 「배치 뷰 `Cmd+A`는 아무 일도 안 함」은 이월 줄대로 「클립 전부 선택, 노트 선택은 없음」으로.
  - lockfile: `async-executor 1.14.0`이 yank되어 `cargo deny`가 실패해 1.14.1로 올렸다(이 PR 밖의 사건, 커밋 10321ec). 같은 재해결로 `tempfile`의 `getrandom` 항목이 0.4.3에서 0.3.4로 바뀌었다(두 판 모두 lockfile에 남아 있고 새 crate는 없다).
  - 리뷰 1 뒤: 첫 마디 가사는 「클립 시작부터 박자표 맵의 그 마디 끝까지」로 정했다(제안 1, 계획 1절 고침). 클립 끝 이어 쓰기의 늘어나는 구간도 고스트로 그린다(제안 5). 따라가기는 두 뷰 모두 하고 줌만 가른다(제안 3, 계획 2절에 적음). 벤치 `arrangement/scene_labels_5x36`을 더했다(제안 2).

## 구현 요약

- `crates/keyscore-core/src/track.rs`: `Track::clips_ending_after(tick)`. 끝 틱이 `tick` 뒤인 클립부터 시작 순으로(클립이 겹치지 않아 끝도 단조라 이진 탐색).
- `crates/keyscore-app/src/arrangement/mod.rs`(새 파일): `Viewport`(틱↔x, 행↔y, `cell_at`), `ClipSelection`(트랙별 `ClipId` 집합: `set_all`·`clear`·`retain_existing`·`contains`·`iter`·`insert`), `ArrangementState`(`Rc<RefCell<Inner>>`: 뷰포트, 선택, 장면 버퍼, 그린 수, 크기를 알기 전의 미룬 요청. `ensure_visible`·`center`·`reveal_on_entry`·`zoom`·`zoom_reset`·`zoom_at`·`scroll_by`), `clip_boundary_before/after`(클립 시작·끝과 0틱).
- `crates/keyscore-app/src/arrangement/scene.rs`(새 파일, 순수): `build`가 보이는 트랙 행의 띠, 마디·박 선(한 박이 8 px 미만이면 박 선 없음), 클립 블록(`clips_ending_after`로 왼쪽을 건너뛰고 오른쪽 끝에서 멈춤, 블록이 24 px 이상일 때만 이름·첫 마디 가사 글), 커서 칸(현재 트랙 행, Step 너비), Step 고스트, 재생 헤드를 채운다. `first_bar_lyric`은 클립 시작부터 그 마디 끝까지의 가사를 공백으로 잇는다.
- `crates/keyscore-app/src/arrangement/element.rs`(새 파일): `ArrangementElement`. `prepaint`에서 크기·트랙 수를 상태에 넣고 문서 `revision`이 바뀌면 선택을 걸러 장면을 만든다. `paint`는 `paint_layer` 하나 안에서 띠·선·블록·라벨(`shape_line`, 블록 폭으로 content mask)·고스트·커서·재생 헤드를 그리고 그린 블록 수를 센다. 마우스는 왼쪽 누름(`Press`, 칸 포함), 우클릭(뗄 때, 칸이 있을 때만), 휠(세로, `Shift` 가로, `Cmd` 줌), 핀치(줌). 피아노롤 Element와 같은 구조·규칙.
- `crates/keyscore-app/src/arrangement_window.rs`(새 파일): 처리기 `cursor::PrevClipBoundary/NextClipBoundary`(현재 트랙의 클립 경계), `cursor::TrackUp/TrackDown`(이웃 트랙, 틱 유지). `render_arrangement`(테마에서 팔레트, 트랙 색 12개, Element, `main-view` 라벨 「배치 뷰」, Step 글 표시 두 개는 크기 0의 접근성 요소로). 클릭(`arrangement_press`: 행의 트랙 + 스텝 내림 틱), 우클릭 메뉴(`open_arrangement_menu`: 마크 설정·마크로 이동, `PopupKind::ArrangementMenu`), 진입 시 `reveal_on_entry`.
- `crates/keyscore-app/src/window.rs`: 필드 `arrangement`, 접근자 `arrangement()`·`clip_selection()`·`set_clip_selection()`, `context_menu_open`이 두 메뉴를 봄, 배치 뷰 진입 훅, FSM이 선택을 비우면 클립 선택도 비움, 처리기 등록(`arrangement_handlers`), 주 뷰 그리기 분기.
- `crates/keyscore-app/src/selection.rs`: `sync_selection_mode`가 뷰에 따라 노트 선택·클립 선택을 본다. `select_all`은 배치 뷰에서 모든 트랙의 클립 전부, `select_clear`는 두 선택을 비움. 문서 변경 때 클립 선택도 걸러 FSM을 바로 맞춤. `cursor_moved_quietly`가 배치 뷰 따라가기.
- `crates/keyscore-app/src/pianoroll_window.rs`: `view::ZoomIn/ZoomOut/ZoomReset/CenterCursor`를 현재 뷰에 따라 나눔(이월).
- `crates/keyscore-app/src/transport.rs`: `play_from_cursor_tick()`(두 뷰 모두 커서 틱), `transport_play_from_cursor`가 이것을 씀(이월).
- `crates/keyscore-app/src/step_input.rs`: `step_ghost()`. `render_arrangement_step`은 지웠다(격자 위 표시로 바뀜, 글은 접근성 이름).
- `crates/keyscore-app/src/track_list.rs`: `track_index`·`current_index`·`select_track_at`를 `pub(crate)`로.
- `crates/keyscore-app/src/pianoroll/scene.rs`: `Rect::overlaps`를 `pub(crate)`로.
- `crates/keyscore-app/benches/arrangement.rs`(새 파일), `Cargo.toml` `[[bench]]`: `arrangement/scene_64x200`.
- `plan/stages/14-S4-clips-and-arrangement.md`: S4-T1 줄에 「`Esc` 복귀」 차이와 커서 공유를 적음.

## 테스트 결과

~~~
CI 실행 26(macos-15, 커밋 9323162, cargo test --workspace --no-fail-fast, 건너뛰기 19개):
  2841 passed, 0 failed, 2 ignored (시험 바이너리 311개)
  tests/arrangement.rs: 22 passed
  tests/arrangement_step.rs: 9 passed (S3-T4c 글 표시 시험, 접근성 이름으로 그대로 통과)
  tests/review_s2_t6f_r3.rs 8, review_s3_t1_r2.rs 10, review_s3_t2.rs 13 passed (실행 25 실패분 수정 뒤)
CI 실행 25(커밋 2f2d7aa): 새 시험 2개(step 고스트) + 기존 시험 3개 실패 → 커밋 bc622df·e9df587·9323162로 고침
core: cargo test -p keyscore-core (이 컨테이너) 새 시험 통과
~~~

구현 전 실패 확인: 시험·벤치 파일을 먼저 커밋(06e238b)하고 교차 타입 검사(`cargo check --tests --benches --target aarch64-apple-darwin`)에서 `keyscore_app::arrangement` 미해결(E0432)로 컴파일되지 않음을 확인했다. core의 `clips_ending_after_skips_clips_that_end_at_or_before_tick`는 메서드 없음(E0599)으로 실패한 뒤 구현으로 통과했다.

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `scene_lays_out_clip_blocks_by_track_row_and_tick` | 통과(CI 26) |
| 2 | `scene_skips_offscreen_clips_with_sorted_search` | 통과(CI 26) |
| 3 | `scene_labels_use_name_and_first_bar_lyric_only` | 통과(CI 26) |
| 4 | `scene_grid_lines_follow_meter_map_and_hide_dense_beats` | 통과(CI 26) |
| 5 | `scene_cursor_selection_ghost_playhead_positions` | 통과(CI 26) |
| 6 | `clip_boundaries_on_current_track` | 통과(CI 26) |
| 7 | `v_toggles_views_enter_opens_clip_and_escape_keeps_view` | 통과(CI 26) |
| 8 | `arrangement_cursor_keys_move_by_clip_boundary_track_and_bar` | 통과(CI 26) |
| 9 | `cursor_moves_scroll_arrangement_viewport_to_follow` | 통과(CI 26) |
| 10 | `zoom_keys_apply_only_to_the_visible_view` | 통과(CI 26) |
| 11 | `select_all_in_arrangement_selects_every_clip_and_escape_clears` | 통과(CI 26) |
| 12 | `clip_selection_drops_removed_clips` | 통과(CI 26) |
| 13 | `play_from_cursor_uses_cursor_tick_in_both_views` | 통과(CI 26) |
| 14 | `right_click_in_arrangement_opens_mark_menu_and_sets_mark` | 통과(CI 26) |
| 15 | `step_in_arrangement_draws_cursor_and_ghost_on_grid`, `step_ghost_is_blocked_when_next_clip_is_in_the_way` | 통과(CI 26. 실행 25에서 둘 다 실패: 클립 끝 이어 쓰기가 막힌 경우를 고스트 없이 지나침 → 수정, 라벨 기대값 오타 → 수정) |
| 16 | `click_places_cursor_and_wheel_scrolls` | 통과(CI 26) |
| 17 | `element_paints_every_visible_clip_once` | 통과(CI 26) |
| 18 | `playhead_follows_transport_in_arrangement` | 통과(CI 26) |
| 19 | `frame_with_64x200_clips_fits_60fps_budget_in_release` | 사람 Mac(release) |
| 20 | 벤치 `arrangement/scene_64x200` | 사람 Mac(기준값) |
| (추가) | `cell_at_maps_click_point_to_track_row_and_step`, `clip_selection_set_all_and_retain_existing`, core `clips_ending_after_skips_clips_that_end_at_or_before_tick` | 통과 |

계획에 있었지만 쓰지 않은 테스트와 이유: 없음.

## 자동 게이트

구현 세션(Linux 컨테이너)에서 돌린 것:

- [x] `cargo fmt --all -- --check`: 통과
- [x] clippy: `cargo clippy -p keyscore-app --all-targets --target aarch64-apple-darwin -- -D warnings` 경고 0(앱 crate·시험·벤치). `cargo clippy -p keyscore-core -p keyscore-text -p keyscore-ai --all-targets -- -D warnings` 경고 0. 교차 검사는 gpui의 Metal 셰이더 빌드 산출물(`stitched_shaders.metal`, `shaders.metallib`)을 빈 파일로 두어 타입 검사만 지나게 한 로컬 우회다. 링크·실행은 하지 않는다.
- [x] `cargo test -p keyscore-core`: 새 시험 통과. `ksq_write_file_failure_keeps_original` 하나는 이 컨테이너가 root라 읽기 전용 경로 쓰기가 막히지 않아 `main`에서도 같은 자리에서 실패한다(이 변경과 무관).
- [x] `cargo test --workspace`: CI 실행 26에서 2841 passed, 0 failed, 2 ignored(건너뛰기 19개는 결정 `2026-11-02-ci-runner-scope` A). fmt·clippy도 CI에서 통과.
- [ ] `cargo deny check`: 이 컨테이너에 `cargo-deny`가 없다. CI 실행 26에서 advisories 실패: `smol` → `async-executor 1.14.0`이 yank됨(이 PR 밖, `main`도 같음). 커밋 10321ec에서 1.14.1로 올렸다. CI 실행 27 결과로 닫는다.
- [x] `scripts/check-deps.sh` ok, `scripts/check-deps.test.sh` 16 passed, `scripts/bench-check.test.sh` 8 passed
- [ ] 단계별 추가 게이트: 벤치 `arrangement/scene_64x200` 기준값은 기계별이라 사람 Mac에서 `python3 -I scripts/bench-check.py --update`로 `benches/baseline.json`에 넣는다. release 프레임 시험(19)도 사람 Mac.

CI(macos-15) 결과: 실행 25 실패(시험 5, 위 표) → 실행 26 시험 전부 통과, deny만 yank로 실패 → 실행 27(lockfile 수정) 대기.

## 리뷰 대응

### 리뷰 1 (`review-1.md`, 수정 후 재리뷰)

| # | 종류 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 지적 1 | 수정 | `step_ghost`의 막힌 고스트 끝을 포화 덧셈으로 잡고 `end ≥ start`를 지킨다. `scene::block_rect`도 `saturating_sub`. 시험 `brk_step_ghost_near_u64_end_does_not_overflow`는 CI로 확인 | 167bc68 |
| 지적 2 | 수정 | 따라가기·가운데 맞추기에서 커서 행까지는 트랙이 있는 것으로 보고 자른다(`admit_row`). 다음 그리기가 참값으로 되돌린다. 시험 `brk_new_track_beyond_viewport_bottom_is_revealed`는 CI로 확인 | 8cfb7dd |
| 지적 3 | 수정 | `cargo deny`: CI 실행 28 이후의 결과로 닫는다(아래 「자동 게이트」). 벤치 기준값 둘(`arrangement/scene_64x200`, `arrangement/scene_labels_5x36`)과 release 프레임 시험(19)은 기계별 값이라 이 세션에서 잴 수 없다. 사람 Mac에서 `python3 -I scripts/bench-check.py --update`와 `cargo test -p keyscore-app --release --test arrangement frame_with_64x200 -- --nocapture`를 돌려 보고서에 적고 기준값을 커밋해야 닫힌다. 병합 전 사람에게 요청했다 | — |
| 제안 1 | 받음 | 「클립 시작부터 그 마디 끝까지」로 정하고 계획 1절을 고쳤다. 마디 중간 시작 클립 시험을 더했다 | 2ad4908 |
| 제안 2 | 받음 | 기본 배율의 라벨 장면 벤치 `arrangement/scene_labels_5x36`(블록 384 px, 36행) | 2ad4908 |
| 제안 3 | 받음(문서) | 의도다. 계획 2절에 「따라가기는 두 뷰 모두, 줌만 가른다」를 적었다 | 문서 커밋 |
| 제안 4 | 받음 | `ArrangementState::set_selection`을 `pub(crate)`로 | 8cfb7dd |
| 제안 5 | 받음 | 클립 끝 이어 쓰기의 `ExtendClip` 구간을 고스트로. 시험 `step_ghost_at_clip_end_shows_the_extension` | 2ad4908 |
| 제안 6 | 받음(이월) | 단계 파일 S4-T5에 「메인 영역 상단 세그먼트 버튼」 이월 줄 | 문서 커밋 |

## 병합 메모 (2026-11-05)

사람이 2026-11-05에 「그냥 병합」을 지시해 병합 조건을 채우지 않은 채 `main`에 넣는다. 남은 것:

- 리뷰 1 대응 커밋(167bc68, 8cfb7dd, 2ad4908)의 시험 실행이 없다. CI 실행 29(1ba78f8)는 두 번 모두 15분 만에 취소됐다(로그 없음, Actions 사용 한도로 추정). 교차 타입 검사·clippy만 통과했다. `tests/review_s4_t1_r1.rs` 12개(그 가운데 `brk_` 2개)와 새 시험 `step_ghost_at_clip_end_shows_the_extension`·첫 마디 가사 마디 중간 시작 시험을 사람 Mac이나 다음 CI에서 돌려야 한다.
- `cargo deny check` 통과 실행이 없다(lockfile 수정 10321ec 뒤).
- 벤치 기준값 `arrangement/scene_64x200`·`arrangement/scene_labels_5x36`이 `benches/baseline.json`에 없고 release 프레임 시험(19) 결과가 없다.
- 재리뷰(2회차)를 하지 않았다. 리뷰 1 판정은 「수정 후 재리뷰」다.

다음 세션은 S4-T2 전에 위 넷을 닫는다(HANDOFF 「남은 작업」).

## 알려진 한계

- 라벨(이름·가사)은 보이는 블록마다 매 프레임 `shape_line`으로 다시 만든다(블록이 24 px 이상일 때만). 보이는 블록 수에 비례하는 비용이며 장면 버퍼처럼 캐시하지 않았다. 느리면 (클립 id, revision, 폭) 키의 캐시를 둔다.
- 행 높이 28 px·박 선 숨김 8 px·라벨 최소 폭 24 px·기본 배율은 REQ에 없는 값(S8 테마와 함께).
- 러버밴드·드래그·더블클릭(S4-T4), `Shift+이동` 선택 확장·클립 편집 키(S4-T2), `Tab` 트랙 이동(S4-T3)은 없다. 배치 뷰에서 `Tab`은 아직 피아노롤 규칙 그대로다.
- 배치 뷰 커서는 창에 하나인 커서라 피아노롤에서 옮긴 틱이 배치 뷰에도 그대로다(계획 2절, 에이전트 해석).
- 빈 자리(트랙 행 밖)를 눌러도 아무 일이 없다. 우클릭 메뉴도 트랙 행 위에서만 열린다.
- 블록 안 노트 미리보기는 그리지 않는다.
- 마우스 줌은 커서를 따라가지 않는다(피아노롤과 같음).

## 리뷰어에게

- 이 세션은 시험을 실행하지 못했다. CI(macos-15)가 통과했더라도 CI 건너뛰기 목록 밖의 시험만 본 것이고 release 프레임 시험·벤치 기준값은 사람 Mac 몫이다. 리뷰어도 Mac이 없으면 깨뜨리는 시험을 써서 CI로 확인한다(`CARGO_TARGET_DIR=target/review`, 전체 시험·MIDI 시험은 돌리지 않는다).
- 먼저 볼 곳: `selection.rs`의 `sync_selection_mode`(뷰에 따라 선택을 가름)와 `window.rs` `apply_mode_event`의 클립 선택 비우기. FSM `selection` 플래그와 두 선택의 불변식이 여기서 지켜진다.
- `scene.rs` `build_clips`의 왼쪽 건너뛰기(`clips_ending_after`)와 `first_bar_lyric`의 마디 경계(박자표 변화가 클립 첫 마디 안에 있을 때).
- `element.rs` `paint_labels`: Element 안에서 글을 그리는 첫 사례. content mask를 레이어 안에서 겹쳐 쓴다. 글꼴 없음 등으로 `paint`가 `Err`면 조용히 블록만 남긴다.
- `arrangement_window.rs` `render_arrangement`의 Step 글 표시: 크기 0·absolute의 `test_support` 요소로 남겼다. `tests/arrangement_step.rs`의 `label(...)` 찾기가 그대로 통과해야 한다.
- `step_ghost`의 막힌 경우 범위(커서가 든 마디)는 에이전트가 정했다.
