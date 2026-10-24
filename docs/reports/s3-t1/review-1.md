# 적대적 리뷰: S3-T1 피아노롤 Element (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 세션(구현 맥락 없이 코드·계획·REQ만 읽고 시작). 서브 에이전트.
- 대상 커밋: `084f73f` (브랜치 `s3/t1-pianoroll-element`, `main..HEAD` 5개 커밋)
- 추가한 시험: `crates/keyscore-app/tests/review_s3_t1.rs` (19개. 실패 3개는 실패한 채로 둔다)
- 실행: `cargo test -p keyscore-app --test review_s3_t1 --test pianoroll` → `pianoroll` 12 통과, `review_s3_t1` 16 통과·3 실패. `cargo clippy -p keyscore-app --test review_s3_t1 -- -D warnings` 통과.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`crates/keyscore-app/src/pianoroll/{mod.rs,scene.rs,element.rs}`, `pianoroll_window.rs`, `window.rs` 변경분을 읽고 `tests/pianoroll.rs`를 열기 전에 적었다.

1. 피아노롤에 처음 들어갈 때(`enter`, `v`) `entered_piano_roll`이 커서를 첫 클립 시작으로 옮기지만, 그 시점에 `Inner::width/height`는 0이라 `ensure_cursor_visible`이 무시된다(`mod.rs:252-255`). 첫 `set_size`는 `center_rows`로 세로만 맞춘다(`mod.rs:282-290`). 첫 클립이 화면 너비 밖(예: 50마디)이면 들어간 직후 커서가 보이지 않을 것이다.
2. 휠 처리기(`element.rs:302-318`)가 `Hitbox::is_hovered`를 쓴다. GPUI 0.3.8의 `is_hovered`는 `last_input_was_keyboard()`면 거짓이고(`gpui-pre-0.3.8/src/window.rs:782-791`), `ScrollWheel`은 입력 모달리티를 바꾸지 않는다(`window.rs:5463-5468`: `KeyDown`·`MouseMove`·`MouseDown`·`Touch`만). 키를 누른 뒤 포인터를 움직이지 않고 휠을 굴리면 무시될 것이다. GPUI `Div`는 같은 이유로 `should_handle_scroll`을 쓴다(`div.rs:396, 3340`).
3. `scene::build_lines`(`scene.rs:158-202`): 박자표가 마디 중간에 바뀌는 SMF 맵에서 박 선이 다음 마디 시작에서 끊기는가, 선 틱이 단조 증가하는가, 뷰포트 왼쪽 끝이 마디 중간일 때 앞 선이 빠지는가.
4. 틱 u64 끝: `StepForward`(`checked_add`), `BarForward`(`bar_start` 넘침 → `None`), `tick_u` 포화, `build_lines`의 `bar + 1`이 u64 끝 근처에서 패닉·무한 루프 없이 끝나는가. 뷰포트 `first_tick`이 u64 범위를 넘어도(`scroll_by`는 상한이 없다) 패닉이 없는가.
5. 음높이 0·127: `row_of`(`mod.rs:81-83`), `build_rows`의 `first..last` 범위, `build_notes`의 행 필터(`scene.rs:233`), `cell_at`의 행 128 경계(`mod.rs:122`)에서 `ROWS - 1 - row`가 언더플로하지 않는가.
6. 트랙이 없는 프로젝트·클립이 없는 트랙에서 장면·키·줌·`CenterCursor`가 패닉 없이 도는가.
7. 뷰포트 크기 0: `build`의 조기 반환(`scene.rs:113`), `cell_at`의 `None`, `ensure_cursor_visible`·`center_cursor`의 무시.
8. `MaxLenCache` 키가 `(revision, TrackId)`뿐이다(`mod.rs:194-197`). revision이 그대로인 채 트랙이 바뀌는 경로가 있는지(`ProjectDocument`의 `project_mut` 유무, 문서 교체 경로).
9. 겹치는 긴 노트: 같은 음높이의 긴 노트가 왼쪽 경계에 걸릴 때 `partition_point` 술어(`scene.rs:223`)가 빠뜨리지 않는가.
10. `RefCell` 이중 빌림: `prepaint`의 `borrow_mut`(`element.rs:137`) 동안 콜백이 없는가, `paint`의 `borrow()`(`element.rs:186`)가 `paint_layer` 클로저 밖으로 새지 않는가, 클릭 처리기의 `viewport()` 빌림이 `on_click` 호출 전에 끝나는가(`element.rs:294-298`).
11. 재생 중 `request_animation_frame`(`element.rs:194-196`)이 `Playing`일 때만 불리는가, 재생 헤드 틱이 모노토닉 클록 보간인가(`transport.rs:102-113` `Anchor::position_at(HostTime::now())`).
12. 클릭 좌표: `event.position - origin`(`element.rs:293`)이 요소 원점 기준인가, 요소 가장자리 픽셀이 칸으로 잡히는가.
13. 줌 상한·하한에서 커서 칸이 보이는가(상한은 칸이 뷰포트보다 넓다), 하한 뒤 `cmd-0`이 기본 배율로 돌아오며 커서가 보이는가.
14. paint 경로의 매 프레임 할당·정렬: `paint_scene`은 `paint_quad`만, `register_mouse`는 `on_mouse_event`로 클로저 두 개를 Box에 넣는다(`element.rs:286, 302`).
15. 단계 파일 게이트 「criterion — 10,000 노트 화면의 paint 시간 기준값」이 실제로 paint를 재는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 먼 클립으로 들어갈 때 뷰포트 추종 | 없음 | 시험 9는 클립이 2마디(768 px, 화면 안)라 드러나지 않는다. 커서 값만 보고 뷰포트는 보지 않는다 |
| 2. 키 입력 뒤 포인터 이동 없는 휠 | 없음 | 휠 시험이 없다. `scroll_by`만 직접 부른다(시험 7) |
| 3. 마디 중간 박자표 변화 | 부분 | 시험 4는 변화가 마디 경계(2마디)에 있다. 마디 중간 변화는 없다 |
| 4. 틱 u64 끝 | 없음 | |
| 5. 음높이 0·127 | 부분 | 시험 6·7이 커서를 0·127까지 보내지만 장면의 노트·행·`cell_at` 경계는 없다 |
| 6. 트랙·클립 없음 | 부분 | 시험 9가 `Project::new()`에서 `enter`만 누른다 |
| 7. 뷰포트 0 | 없음 | |
| 8. 캐시 낡음 | 없음 | 「리뷰어에게」가 물었다 |
| 9. 겹치는 긴 노트 | 부분 | 시험 2의 긴 노트는 음높이가 다르다 |
| 10. RefCell | 없음(코드로 판단) | |
| 11. 재생 헤드 보간 | 있음(시험 10) | `request_animation_frame`은 관찰 불가(보고서가 밝힘) |
| 12. 클릭 좌표 | 부분 | 시험 12는 `cell_at`만. 실제 클릭 이벤트는 없다 |
| 13. 줌 하한 | 부분 | 시험 8은 상한만 |
| 14. paint 할당 | 없음(코드로 판단) | |
| 15. paint 벤치 | 벤치는 `scene::build`만 | 아래 지적 3 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t1.rs`에서 실제로 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 첫 클립이 50마디에 있는 트랙에서 `enter`로 들어간 직후 커서가 뷰포트 안인가 | 깨짐. 커서 192,000틱, 뷰포트 `first_tick` 0, 너비 1056 px | `entering_piano_roll_from_far_clip_scrolls_viewport_to_cursor` |
| 2 | 같은 상황을 `v`(`view::Toggle`)로 | 깨짐(같은 원인) | `toggling_view_into_far_clip_scrolls_viewport_to_cursor` |
| 3 | 대조: 그 뒤 `l`을 한 번 누르면 따라오는가 | 못 깨뜨림(키 경로는 맞다) | `cursor_key_after_far_entry_brings_cursor_into_view` |
| 4 | `enter` 뒤 포인터 이동 없이 요소 가운데에서 휠(픽셀 −120) | 깨짐. 뷰포트가 전혀 움직이지 않는다 | `wheel_after_keyboard_input_without_pointer_move_scrolls` |
| 5 | 대조: 포인터를 먼저 움직인 뒤 휠·`Shift`+휠·왼쪽 끝 | 못 깨뜨림. 세로 −120 px → 첫 행 +10, `Shift`+휠 → 첫 틱 +1200, 왼쪽 끝 0틱 | `wheel_after_pointer_move_scrolls_rows_and_shift_wheel_scrolls_ticks` |
| 6 | 2400틱(4/4 셋째 박 중간)에서 3/4로 바뀌는 맵의 마디·박 선 | 못 깨뜨림. 마디 0·2400·5280·8160, 첫 마디 박 선 960·1920에서 끊김, 틱 단조 증가 | `scene_mid_bar_meter_change_cuts_beats_and_starts_new_bar` |
| 7 | 커서·재생 헤드 u64::MAX, 뷰포트 첫 틱 u64::MAX−10⁵ 및 10³⁰ | 못 깨뜨림(패닉·무한 루프 없음) | `scene_at_tick_max_does_not_panic` |
| 8 | 음높이 0·127 노트와 128행, 소수 `first_row`, 마지막 행만 반 보이는 뷰포트 | 못 깨뜨림 | `scene_draws_pitch_0_and_127_in_outer_rows` |
| 9 | 너비 0·높이 0·배율 0 뷰포트 | 못 깨뜨림(빈 장면, `cell_at` `None`) | `scene_zero_viewport_is_empty_and_cell_at_is_none` |
| 10 | 트랙 없음·클립 없는 트랙 | 못 깨뜨림(격자·커서만) | `scene_without_track_or_clips_has_grid_but_no_notes` |
| 11 | 같은 음높이의 6마디 노트와 겹친 짧은 노트, 뷰포트가 4.5마디부터 | 못 깨뜨림(긴 노트가 왼쪽 밖에서 시작해 들어감) | `scene_overlapping_long_notes_both_drawn_across_left_edge` |
| 12 | revision을 올리지 않고 더 긴 노트를 넣은 뒤 같은 revision으로 `refresh` | 예상대로 긴 노트가 빠진다(캐시 계약 확인. `ProjectDocument`에 `project_mut`이 없어 제품 경로로는 도달하지 않음) | `max_len_cache_same_revision_hides_note_lengthened_later` |
| 13 | 커서 u64::MAX에서 스텝·마디·옥타브 이동, 크기 모를 때 `zoom`·`center_cursor` | 못 깨뜨림. 다만 크기를 모를 때 `zoom`은 커서 x를 지키느라 `first_tick`을 옮긴다(무해) | `state_moves_at_tick_max_do_not_overflow` |
| 14 | 마디 중간 변화 앞뒤의 `Ctrl+h`/`Ctrl+l` | 못 깨뜨림(2500 → 2400 → 0, 0 → 2400 → 5280) | `state_bar_moves_follow_mid_bar_meter_change` |
| 15 | `cell_at` 마지막 행·행 128 밖·소수 `first_row`·가로 끝 픽셀 | 못 깨뜨림 | `cell_at_bottom_row_and_fractional_first_row` |
| 16 | 실제 클릭 이벤트(150, 30)와 오른쪽 아래 구석(12 px 안쪽) | 못 깨뜨림. 좌표는 요소 원점 기준이고 뷰포트는 움직이지 않는다. 가장자리 4 px 띠는 다른 히트박스가 가려 클릭이 안 된다(제안 4) | `click_moves_cursor_to_cell_under_pointer` |
| 17 | `scroll_by`로 커서를 화면 밖에 두고 `l` | 못 깨뜨림(돌아온다) | `cursor_key_after_scrolling_away_returns_to_cursor` |
| 18 | 트랙 없는 프로젝트에서 커서·줌·`CenterCursor` 전부 | 못 깨뜨림 | `keys_on_project_without_tracks_do_not_panic` |
| 19 | `cmd--` 60번 → 하한, 커서 보임, `cmd-0` | 못 깨뜨림 | `zoom_out_stops_at_floor_and_keeps_cursor_visible` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 장면 계산은 문서를 바꾸지 않는다 | 없음 | `scene::build`는 `&SceneInput`만 받는다. `prepaint`는 `document.read` |
| 커서 틱 ≥ 0, 음높이 0–127 | 없음 | `Tick`(u64)·`Pitch` 타입. `pitch_by`는 `clamp(0,127)`(`mod.rs:371-378`), `StepBack`은 `saturating_sub`, `StepForward`는 `checked_add` 실패 시 제자리. 시도 13 |
| 줌은 상한·하한 사이 | 없음 | `zoom`은 `clamp(MIN, MAX)`(`mod.rs:438`). 시도 19와 기존 시험 8. 다만 `zoom_reset`·초기값은 clamp를 거치지 않는데 상수가 범위 안이다 |
| `paint`는 할당·정렬을 하지 않는다 | 거의 지킴 | `paint_scene`은 `paint_quad`만. `register_mouse`가 프레임마다 `on_mouse_event`로 클로저 두 개를 Box에 넣는다(`element.rs:286, 302`). 노트 수와 무관한 상수 2회이고 GPUI `Div`도 같은 일을 한다(6절) |
| 노트 quad는 `paint_layer` 하나 안 | 지킴 | `element.rs:188-190`: `with_content_mask` 안 `paint_layer` 하나에서 행·선·노트·커서·헤드 전부 |
| `RefCell` 이중 빌림 없음 | 없음 | `prepaint`의 `borrow_mut`는 콜백을 부르지 않는다. `paint`의 `borrow()`는 블록 끝에서 풀리고(`element.rs:185-192`) 그 뒤 `borrow_mut`로 `painted`를 쓴다. 클릭 처리기는 `viewport()` 복사본을 받은 뒤 `on_click`을 부른다 |
| 노트는 틱 순 정렬 캐시(정렬 없음) | 지킴 | `Clip::notes()`는 `Note::Ord`(시작, 음높이, …)로 정렬된 `Vec`(`note.rs:142-163`, `clip.rs:161`). `partition_point` 술어는 시작 틱에 단조 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.3 「클립 하나의 안쪽을 세로축 음높이, 가로축 틱으로」 | 충족 | `scene.rs:137-156`(행), `204-250`(노트), `mod.rs:81-96`(행·틱 ↔ px) |
| 5.3 「커서는 (틱, 음높이) 한 칸」 | 충족 | `scene.rs:119-128` 커서 칸(한 스텝 너비 × 한 행) |
| 5.3 「앞뒤 클립의 노트는 흐리게 보이고 편집은 되지 않는다」 | 충족(표시 부분) | `scene.rs:217` `dim = current != id`, `element.rs:230-244` 흐린 색. 편집 금지는 S3-T3 범위 |
| 5.7 수식키 없음 행(`h` `l` `j` `k`·화살표) | 충족 | `assets/keymaps/default.json:33-40`, `mod.rs:380-395`, 기존 시험 6 |
| 5.7 `Ctrl` 행(마디·옥타브) | 충족 | `default.json:41-44, 72-77`, `mod.rs:396-424`, 시도 14 |
| 5.7 「한 스텝」= Step 모드 현재 길이 | 부분 충족(계획이 S3-T4로 미룸) | `mod.rs:27` 4분음표 고정. 계획 「리스크」에 명시. 이 Task의 미충족으로 세지 않는다 |
| 5.8 `[` `]` 옥타브 | 충족 | `default.json:74, 77` |
| 5.8 `Cmd+=` `Cmd+-` `Cmd+0` | 충족 | `standard.json:23-25`, `pianoroll_window.rs:175-203`, 기존 시험 8, 시도 19 |
| 5.8 `z` 선택에 맞춰 줌 | 범위 밖(S3-T2 선택 뒤) | 계획 「범위 밖」 |
| 7.4 MS-5 「클릭하면 키보드 커서가 그 자리로 온다」 | 충족 | `pianoroll_window.rs:91-98`, 시도 16 |
| 9.4 피아노롤은 자체 Element | 충족 | `element.rs:100-199` `Element` 직접 구현 |
| UI-3 「키 입력 후 60 fps 안 반영, GPU 인스턴싱으로 10,000개 끊김 없이 스크롤」 | 부분 충족 | 구조는 SP-2 분기 A와 같다(`paint_layer` + `paint_quad`). 그러나 이 Element의 paint 시간은 재지 않았고(지적 3), 키보드 입력 뒤 휠 스크롤이 무시된다(지적 2) |
| UI-4 재생 헤드 모노토닉 클록 보간 | 충족 | `pianoroll_window.rs:37-43` → `transport.rs:45-51, 102-113`(`HostTime::now()`), `element.rs:194-196` 매 프레임 |
| 계획 2 「커서가 뷰포트 밖으로 가면 뷰포트가 따라간다」 | 부분 충족 | 키 이동은 따라간다(기존 시험 7). 들어갈 때 옮긴 커서는 따라가지 않는다(지적 1) |
| 계획 3 「마우스 휠은 세로·가로 스크롤(`Shift`+휠은 가로)」 | 부분 충족 | 포인터 이동 뒤에는 맞다(시도 5). 키 입력 직후는 무시(지적 2) |
| 계획 4 가로 동기 훅 `piano_roll_viewport()` | 충족 | `window.rs:280-283` |
| 단계 게이트 「criterion — 10,000 노트 화면의 paint 시간 기준값」 | 부분 충족 | `benches/pianoroll.rs`는 `scene::build`(29.7 µs)만 잰다. paint는 SP-2의 별도 프로그램 측정에 기댄다(지적 3) |
| 단계 게이트 「UI 테스트 — 커서 이동 키가 뷰포트를 따라 스크롤」 | 충족 | 기존 시험 7, 시도 3·17 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 이 Task는 스케줄러를 건드리지 않는다. `piano_roll_playhead`는 `Transport::position`(UI 스레드, 템포 맵 계산)만 읽는다.
- paint 경로의 할당: `paint_scene`(`element.rs:203-276`)은 `paint_quad`만 부른다(장면 버퍼는 `prepaint`에서 `clear` 뒤 재사용). `register_mouse`(`element.rs:279-319`)는 프레임마다 `window.on_mouse_event` 두 번으로 클로저를 Box에 넣는다. 노트 수와 무관한 상수 2회이며 GPUI의 모든 interactive `Div`가 같은 일을 하므로 지적이 아니라 7절 제안 2로 둔다.
- `prepaint`의 매 프레임 작업: `cache.refresh`는 `(revision, TrackId)`가 같으면 즉시 반환(`mod.rs:195-197`). `current` 클립 탐색은 클립 수에 선형(`element.rs:141-146`). `scene::build`는 `Vec::clear` 뒤 `push`라 워밍업 뒤 재할당이 없다.
- 제품 코드의 `unwrap`/`expect`: `mod.rs:315` `Pitch::new(60).expect("C4")` 하나. 상수 60은 늘 유효하고 문서 주석에 적혀 있다. 정당.
- 에러가 삼켜지는 곳: `mod.rs:406, 411, 421` `bar_start(...).unwrap_or(...)`(u64 넘침이면 제자리·0틱. 의도된 경계 처리), `mod.rs:125` `Pitch::new(...).ok()?`(행 128 밖은 앞에서 걸러 도달하지 않음), `mod.rs:374-377` `u8::try_from(...).unwrap_or(pitch)`(clamp 뒤라 도달하지 않음). 모두 경계 고정이라 삼켜도 된다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 피아노롤에 처음 들어갈 때(`enter`, `v`) `entered_piano_roll`이 커서를 첫 클립 시작으로 옮기지만 그때 `Inner::width/height`가 0이라 `ensure_cursor_visible`이 무시되고, 첫 `set_size`는 `center_rows`로 세로만 맞춘다. 첫 클립이 화면 너비 밖이면(예: 50마디, 192,000틱 → 19,200 px) 들어간 직후 커서가 보이지 않고 사용자는 빈 격자만 본다. 계획 「끝의 정의」 2의 「커서가 뷰포트 밖으로 가면 뷰포트가 따라간다」와 어긋난다. 재현: `entering_piano_roll_from_far_clip_scrolls_viewport_to_cursor`, `toggling_view_into_far_clip_scrolls_viewport_to_cursor`. 고치는 길 예: `set_size`의 첫 크기 분기에서 `center_rows` 대신 가로·세로 모두 `ensure_cursor_visible`(또는 `center_cursor`)을 부르거나, `entered_piano_roll`이 「다음 prepaint에서 따라가기」 플래그를 세운다 | `mod.rs:252-255, 282-290`, `pianoroll_window.rs:110-116` |
| 2 | 수정 | 휠 처리기가 `Hitbox::is_hovered`로 hover를 판정한다. GPUI 0.3.8의 `is_hovered`는 마지막 입력이 키보드면 거짓이고(`gpui-pre-0.3.8/src/window.rs:782-791`), `ScrollWheel`은 입력 모달리티를 바꾸지 않는다(`window.rs:5463-5468`). 그래서 키를 누른 뒤 포인터를 움직이지 않고 트랙패드를 굴리면(키보드 중심 편집기에서 가장 흔한 순서) 휠이 모두 무시된다. GPUI `Div`는 같은 이유로 `ScrollWheelEvent`에 `should_handle_scroll`을 쓴다(`div.rs:396, 3340`). 재현: `wheel_after_keyboard_input_without_pointer_move_scrolls`(대조 `wheel_after_pointer_move_scrolls_rows_and_shift_wheel_scrolls_ticks` 통과). 고치는 길: `wheel_hit.is_hovered(window)` → `wheel_hit.should_handle_scroll(window)` | `element.rs:303` |
| 3 | 수정 | 단계 파일 S3-T1 테스트 게이트는 「criterion — 10,000 노트 화면의 paint 시간 기준값 커밋, 60 fps 상한 안」인데 커밋된 벤치는 `scene::build`(29.7 µs)만 잰다. 이 Element의 `prepaint`+`paint`(행 128 + 선 + 노트 10,000 quad + 히트박스 + 마우스 등록)는 SP-2의 별도 스파이크 프로그램으로 대신했다. 구조가 같아 결과가 다르지 않으리라 보지만 측정이 없는 것은 게이트의 「대략」 충족이다. 받아들일 길 중 하나를 택하고 보고서에 적는다: (a) 헤드리스 창에서 10,000 노트를 release로 `render_frame`해 시간을 재는 벤치·시험을 더한다, (b) 게이트 문구를 「scene::build 기준값 + SP-2 paint 측정」으로 읽는다는 결정을 단계 파일 결정 분기나 `docs/decisions/`에 남기고 사람이 실제 창에서 10,000 노트 스크롤을 확인하는 수동 항목을 S3 출구 게이트에 올린다 | `benches/pianoroll.rs:60-91`, `plan/stages/13-S3-step-and-pianoroll.md:14` |

제안(구현자가 거절할 수 있음. 거절 이유를 보고서에 적는다):

| # | 내용 | 파일:줄 |
| --- | --- | --- |
| 제안 1 | 시험 11 `element_paints_every_visible_note_once`가 비교하는 `painted`는 `paint`가 `scene.notes.len()`을 그대로 적은 값이라(`element.rs:191`) 「그린 노트 수 = 장면의 노트 수」가 동어반복이다. 노트 루프 안에서 세면 비용 없이 실제 `paint_quad` 호출 수가 된다 | `element.rs:185-193, 229-253` |
| 제안 2 | `register_mouse`가 프레임마다 클로저 두 개를 Box에 넣는다. 계획의 불변식 「`paint`는 할당·정렬을 하지 않는다」를 글자 그대로는 어기지만 상수 2회이고 GPUI 관례다. 불변식 문구를 「노트 수에 비례하는 할당이 없다」로 고치거나 주석으로 남긴다 | `element.rs:279-319` |
| 제안 3 | `Ctrl+h`를 「마디 중간이면 그 마디 시작, 시작이면 앞 마디 시작」으로 읽었다(`mod.rs:407`). REQ 5.7의 「한 마디 뒤로」는 「한 마디만큼 뒤로(틱 − 마디 길이)」로도 읽힌다. 두 읽기가 S3-T2의 `Ctrl+Shift+h`(「한 마디 뒤까지」 선택)에도 번지므로 S3-T2 계획에서 읽기를 한 줄로 고정하거나 결정 요청에 올린다 | `mod.rs:404-417` |
| 제안 4 | 주 뷰 네 변의 4 px 띠는 다른 히트박스(패널 분할 손잡이 등 S2 레이아웃)가 가려 클릭이 피아노롤에 닿지 않는다(탐침: 가장자리 1·4 px 막힘, 5 px부터 통과). 맨 위 행 12 px 중 4 px, 왼쪽 끝 40틱이 마우스로 잡히지 않는다. S3-T1 범위 밖이지만 S3-T6(마우스 편집) 이월 줄로 남긴다 | `window.rs` 레이아웃, `element.rs:289` |
| 제안 5 | `MaxLenCache`는 `(revision, TrackId)`만 보므로 revision이 오르지 않은 채 트랙이 바뀌면 긴 노트가 빠진다(`max_len_cache_same_revision_hides_note_lengthened_later`). 지금 `ProjectDocument`는 `apply`·`undo`·`redo`에서 revision을 올리고 `project_mut`을 내지 않아 도달하지 않는다. 이 계약을 `MaxLenCache` 문서 주석에 「문서를 revision 없이 바꾸는 API를 더하면 안 된다」로 적어 두면 뒤 Task가 지킬 수 있다 | `mod.rs:177-184` |
| 제안 6 | 크기를 알기 전의 `zoom`(`mod.rs:435-447`)은 `width`가 0이어도 커서 x를 지키느라 `first_tick`을 옮긴다(시도 13). 무해하지만 `center_cursor`처럼 크기를 모르면 배율만 바꾸는 편이 일관된다 | `mod.rs:435-447` |
| 제안 7 | `piano_roll_handlers`는 뷰와 무관하게 걸려 있어 배치 뷰에서도 `cmd-=`(전역 standard 키)가 숨은 피아노롤을 줌한다. 배치 뷰 Element(S4-T1)가 자기 줌을 가질 때 뷰별로 나눈다는 메모를 S4 이월에 남긴다 | `pianoroll_window.rs:16-33`, `window.rs:644` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2는 사용자가 첫 조작에서 바로 만나는 동작 결함이고(먼 클립으로 들어가면 빈 격자, 키 입력 뒤 휠 무시), 둘 다 한두 줄로 고칠 수 있다. 지적 3은 단계 게이트의 해석을 문서로 고정하면 된다. 고친 뒤 `review_s3_t1.rs`의 실패 3개가 통과하는지 2회차에서 본다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(19개, 실패 3개)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`window.rs`는 피아노롤 연결·접근자·`entered_piano_roll`뿐, `Cargo.lock`은 `criterion` dev-dependency)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`git diff main...HEAD -- crates/keyscore-app/tests`에 삭제 줄 없음)
- [x] 새 의존성이 있으면 라이선스를 확인했다(`criterion`은 이미 워크스페이스에 있고 `cargo deny`가 통과한 dev-dependency)
- [x] 스타일 지적을 적지 않았다
