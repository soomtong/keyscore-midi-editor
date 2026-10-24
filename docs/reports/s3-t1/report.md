# Task 보고: S3-T1 피아노롤 Element

- PR: 로컬 브랜치 `s3/t1-pianoroll-element`
- 계획 파일: docs/reports/s3-t1/plan.md
- 계획 대비 달라진 점
  - `cell_at`은 `PianoRollState`가 아니라 `Viewport`의 메서드다(순수 함수라 뷰포트에 두는 쪽이 시험하기 쉽다). 계획 파일을 고쳤다.
  - `PianoRollElement::on_click`은 `Box` 대신 `Rc<dyn Fn>`을 받는다(마우스 처리기 클로저가 `'static`이라 복제가 필요하다).
  - `scene::Scene`에 `cursor: Option<Rect>`, `playhead: Option<f32>`는 계획대로이고, 줄·행 종류 이름은 `LineKind`, `Shade`다.
  - 커서 칸이 뷰포트보다 넓으면(큰 줌) 왼쪽 끝을 맞춘다. 계획의 「커서 칸이 보이게」를 이렇게 읽었다(시험 8의 상한 줌에서 드러났다).

## 구현 요약

- `crates/keyscore-app/src/pianoroll/mod.rs`: `Cursor`, `Viewport`(틱·행 ↔ 픽셀, `cell_at`), `CursorMove`, `Selection`(빈 채로, S3-T2가 채움), `MaxLenCache`(문서 `revision`·트랙마다 클립별 최대 노트 길이), `PianoRollState`(`Rc<RefCell<Inner>>`로 Element와 공유: 커서, 가로 스크롤 틱, px/틱, 세로 스크롤 행, 행 높이, 마지막 크기, 한 스텝 틱, 선택, 캐시, 장면 버퍼, 그린 노트 수). 커서 이동(스텝·반음·마디·옥타브, 경계 고정), `ensure_cursor_visible`, `zoom`(커서 왼쪽 x 유지), `zoom_reset`, `center_cursor`, `scroll_by`, `place_cursor_in_clip`, `current_clip`. 캐스트는 `tick_f`·`tick_u`·`row_u8`·`px_f32` 네 도우미에만 두고 이유를 적었다.
- `crates/keyscore-app/src/pianoroll/scene.rs`: 순수 함수 `build(input, out)`. 행 배경(`row_shade`: 장조·자연 단조 음계 안/밖, 조성 없음은 흰·검은건반), 격자 선(박자표 맵의 마디·박, 마디 중간 변화에서 박 선을 끊음), 노트(클립마다 정렬된 노트 열에서 `partition_point`로 왼쪽 경계, 오른쪽 끝에서 `break`, 행 범위 밖 제외, 이웃 클립은 `dim`), 커서 칸, 재생 헤드 x. 버퍼는 `clear()` 뒤 재사용한다.
- `crates/keyscore-app/src/pianoroll/element.rs`: `PianoRollElement`(`Element` 직접 구현). `prepaint`가 문서를 읽어 크기를 상태에 적고 캐시를 갱신하고 장면을 만든 뒤 히트박스를 넣는다. `paint`는 `with_content_mask` 안의 `paint_layer` 하나에서 행 → 선 → 노트 → 커서 → 재생 헤드 순으로 `paint_quad`만 부른다(할당 없음). 재생 중이면 `request_animation_frame`. 클릭(왼쪽)은 칸을 계산해 콜백, 휠은 `scroll_by`(Shift+휠은 가로)와 `window.refresh()`.
- `crates/keyscore-app/src/pianoroll_window.rs`: 창 쪽 처리기 `cursor::Left/Right/Up/Down/PrevBar/NextBar/OctaveUp/OctaveDown`, `view::ZoomIn/ZoomOut/ZoomReset/CenterCursor`(모드 처리기 묶음에 더함), `piano_roll_playhead`(재생 중 `Transport::position`, 일시정지 자리, 정지 없음), `render_piano_roll`(테마 색과 트랙 색으로 `Palette`, 클릭은 커서 설정 + 문서 포커스), `entered_piano_roll`.
- `crates/keyscore-app/src/window.rs`: `pianoroll` 필드, `piano_roll()`·`piano_roll_viewport()` 접근자, `View::PianoRoll`이면 자리표시자 대신 피아노롤(id·라벨은 「main-view」·「피아노롤」 그대로), `apply_mode_event`가 피아노롤로 들어갈 때 `entered_piano_roll`.
- `crates/keyscore-app/benches/pianoroll.rs`: `pianoroll/scene_10k_notes`(SP-2와 같은 배치의 노트 10,000개가 모두 보이는 뷰포트). `benches/baseline.json`에 기준값을 더했다. `keyscore-app`에 `criterion` dev-dependency와 `[[bench]]`.

## 테스트 결과

~~~
cargo test -p keyscore-app --test pianoroll: 12 passed
cargo bench -p keyscore-app --bench pianoroll: pianoroll/scene_10k_notes 평균 29.7 µs(기준값 29654.427 ns)
cargo test -p keyscore-app --release --test pianoroll frame_with_10k: 프레임 중앙값 1.53 ms, 최대 1.85 ms(노트 9,874개)
cargo test --workspace --no-fail-fast: 1887 passed, 1 failed, 2 ignored(1시간판 측정용). 실패 1은 기존 지터 시험
  review_s2_t5a_r2::pause_position_does_not_drift_with_many_live_edits(S2 출구 실행에서도 같은 실패, 단독 재실행 13 passed)
python3 -I scripts/bench-check.py: pianoroll/scene_10k_notes -1.8%. ksq/read_100k_notes가 +10.3%로 한 번 실패했으나
  core는 손대지 않았고 단독 재실행은 14.53 ms(기준 15.50 ms, -6%)라 전체 시험 직후의 기계 소음으로 본다
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `scene_shows_current_clip_notes_and_dims_neighbors` | 통과(뼈대에서 실패) |
| 2 | `scene_skips_offscreen_notes_with_sorted_search` | 통과(뼈대에서 실패) |
| 3 | `scene_rows_shade_in_key_pitches` | 통과(뼈대에서 실패) |
| 4 | `scene_grid_lines_follow_meter_map` | 통과(뼈대에서 실패) |
| 5 | `scene_playhead_and_cursor_positions` | 통과(뼈대에서 실패) |
| 6 | `cursor_keys_move_by_step_semitone_bar_octave` | 통과(뼈대에서 실패) |
| 7 | `cursor_moves_scroll_viewport_to_follow` | 통과(뼈대에서 실패). 단계 파일 게이트 |
| 8 | `zoom_keys_scale_and_keep_cursor_visible` | 통과(뼈대에서 실패. 첫 구현은 상한 줌에서 커서 왼쪽이 숨어 실패 → 넓은 칸은 왼쪽 끝 맞춤) |
| 9 | `entering_piano_roll_places_cursor_in_first_clip` | 통과(뼈대에서 실패) |
| 10 | `playhead_follows_transport` | 통과(뼈대에서 실패). CoreMIDI 엔진 설치, 루프백 수신기는 없음 |
| 11 | `element_paints_every_visible_note_once` | 통과(뼈대에서 실패. 기본 뷰포트에는 클립 A의 노트 3개가 보여 기대값을 「3개 이상」으로 고쳤다) |
| 12 | `cell_at_maps_click_point_to_cursor` | 통과(뼈대에서 실패) |
| 13 | `pianoroll/scene_10k_notes` | 29.7 µs, 60 fps 예산(16.7 ms)의 0.2% |
| 13b(리뷰 1 지적 3) | `frame_with_10k_notes_fits_60fps_budget_in_release` | release 중앙값 1.53 ms(예산의 9%), debug는 출력만 |

계획에 있었지만 쓰지 않은 테스트와 이유: 없음. 다만 시험 10의 「재생 중이면 매 프레임 다시 그림」은 프레임 수를 세지 못해(헤드리스에서 `request_animation_frame`을 관찰할 길이 없다) 재생 중 `piano_roll_playhead()`가 0보다 큰 틱을 돌려주는 것까지만 본다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings(워크스페이스, 모든 타깃)
- [x] test: 1887 passed, 1 failed(기존 지터 시험, 단독 통과), 2 ignored
- [x] deny: advisories·bans·licenses·sources ok
- [x] `scripts/check-deps.sh`: ok
- [x] 단계별 추가 게이트: criterion 기준값 커밋(`pianoroll/scene_10k_notes` 29654 ns), `scripts/bench-check.py`는 새 벤치 -1.8%(core `ksq/read`의 +10.3%는 단독 재실행 -6%, 소음). UI 시험 7(커서 이동 키가 뷰포트를 따라 스크롤) 통과

## 알려진 한계

- 기본 배율(4분음표 96 px), 행 높이 12 px, 줌 배수 1.25, 배율 상한·하한(0.0005–2 px/틱)은 REQ에 없는 값이다. 설정으로 빼는 일은 S8 테마와 함께 본다.
- 한 스텝은 4분음표 고정이다. S3-T4가 Step 길이 코드와 묶는다(`Inner::step_ticks`).
- 선택은 늘 비어 있다. S3-T2가 채운다. 선택 노트 색은 `theme.selection`으로 미리 그린다.
- 재생 중 재생 헤드는 `request_animation_frame`으로 매 프레임 다시 그리므로 그 창 전체가 다시 그려진다(GPUI는 뷰 단위 재그리기). 노트 10,000개에서도 paint 2 ms 아래(SP-2)라 예산 안이지만, 트랙 목록·인스펙터까지 매 프레임 레이아웃하는 비용은 재지 않았다. 거슬리면 S4 이후에 재생 헤드만 따로 그린다.
- 세로 스크롤은 행 범위 안(0–128)으로만 잠근다. 가로 스크롤은 0틱 왼쪽으로 가지 않고 오른쪽은 한계가 없다(곡 끝 동작은 S3-T2 이월 줄과 함께 정한다).
- 마우스 러버밴드·드래그·우클릭 메뉴(vim 마크)는 S3-T6, 노트 위 글자는 S6다.
- 실제 창에서 그림을 눈으로 확인하지 못했다(에이전트의 키 입력 자동화가 다른 창으로 들어가 중단). 사람이 `cargo run -p keyscore-app --release`로 픽스처를 열고 `enter`로 피아노롤에 들어가 노트·격자·커서·재생 헤드를 본다.

## 리뷰 1 대응(`docs/reports/s3-t1/review-1.md`, 판정 「수정 후 재리뷰」)

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 먼 클립으로 들어갈 때 뷰포트가 따라가지 않음 | `ensure_cursor_visible`이 크기를 모르면 `pending_follow`를 켜고, 처음 크기를 알 때(`set_size`) 적용한다. `entered_piano_roll`은 옮기지 않았어도 늘 `ensure_cursor_visible`을 부른다. 리뷰 시험 `entering_piano_roll_from_far_clip_scrolls_viewport_to_cursor`·`toggling_view_into_far_clip_scrolls_viewport_to_cursor` 통과 | `Follow the cursor on the first piano roll layout` |
| 2 키 입력 뒤 휠 무시 | 휠 처리기의 hover 판정을 `Hitbox::should_handle_scroll`로(GPUI `Div`와 같음). 리뷰 시험 `wheel_after_keyboard_input_without_pointer_move_scrolls` 통과 | `Handle piano roll wheel after keyboard input` |
| 3 paint 벤치가 장면 계산만 잼 | (a)+(b): 헤드리스 release 시험 `frame_with_10k_notes_fits_60fps_budget_in_release`가 노트 10,000개 프레임(레이아웃·prepaint·paint) 중앙값 16.7 ms 안을 단언한다(debug는 수치만 출력). 단계 파일 S3-T1에 게이트 읽기(criterion은 장면 계산, 프레임은 release 시험, GPU는 SP-2)를 적고 S3 출구에 실제 창 수동 항목을 더했다. release 측정값: 중앙값 1.53 ms, 최대 1.85 ms(그린 노트 9,874개, 20프레임). debug는 중앙값 18.4 ms라 단언하지 않는다. SP-2의 paint p95 약 2 ms와 맞는다 | `Measure the 10k-note piano roll frame in a release test` |

| 제안 | 대응 |
| --- | --- |
| 1 `painted`가 동어반복 | 수락. `paint_scene`이 노트 루프에서 세어 돌려준다 |
| 2 불변식 문구 | 수락. 계획의 불변식을 「노트 수에 비례하는 할당·정렬 없음」으로 고쳤다 |
| 3 `Ctrl+h` 읽기 | 수락(이월). S3-T2 이월 줄에 읽기를 고정하고 다른 읽기는 결정 요청으로 |
| 4 주 뷰 가장자리 4 px 띠 | 수락(이월). S3-T6 이월 줄 |
| 5 `MaxLenCache` 계약 주석 | 수락. 문서 주석에 적었다 |
| 6 크기 모를 때 `zoom` | 수락. 배율만 바꾼다 |
| 7 배치 뷰에서도 줌 | 수락(이월). S4-T1 이월 줄 |

리뷰 시험 `crates/keyscore-app/tests/review_s3_t1.rs` 19개 모두 통과(수정 전 3개 실패).

## 리뷰 2 대응(`docs/reports/s3-t1/review-2.md`, 판정 「병합 가능」, 제안 3)

| 제안 | 대응 |
| --- | --- |
| 1 진입 때 커서가 오른쪽 끝에 붙음 | 수락. 들어갈 때 커서가 안 보이면 `center_cursor`(칸이 뷰포트보다 넓으면 왼쪽 끝 맞춤). 크기를 모르면 `Follow::Center`로 미룬다 |
| 2 돌아올 때 규칙 고정 | 수락. 「보이면 스크롤 유지, 안 보이면 가운데」를 `reveal_cursor_on_entry`로 두고 계획 「끝의 정의」 2에 적었다. 리뷰 2 시험 `returning_from_arrangement_*` 둘 다 그대로 통과 |
| 3 release 프레임 시험 운용 | 수락. 최대값도 단언한다. S3 출구 자동 게이트에 release 실행 명령을 적었다 |

리뷰 시험 `review_s3_t1_r2.rs` 10개 통과.

## 리뷰어에게

- `scene.rs`의 `build_notes`: `partition_point` 술어는 「시작 + 클립 최대 길이 ≤ 왼쪽 끝」이다. 캐시가 낡으면(새 노트가 더 길어졌는데 `revision`이 같을 때) 긴 노트가 빠질 수 있다. 문서 편집은 모두 `revision`을 올리므로 괜찮다고 보지만, 편집 경로가 아닌 곳에서 프로젝트를 바꾸는 길이 있는지 봐 달라.
- `element.rs`의 `prepaint`가 문서를 읽으면서 상태 `RefCell`을 가변으로 빌린다. 같은 프레임에 다른 요소가 상태를 빌리면 패닉인데, 지금 빌리는 곳은 창의 Action 처리기(프레임 밖)와 마우스 처리기(paint 뒤)뿐이다.
- `ensure_cursor_visible`의 넓은 칸 규칙(큰 줌에서 왼쪽 끝 맞춤)과 `center_cursor`가 첫 틱을 0 아래로 못 가게 자르는 것 때문에, 곡 첫머리에서는 커서가 가운데가 아닐 수 있다.
- 휠 처리기는 `window.refresh()`로 다시 그린다. `cx.notify()`가 아니라 창 전체를 다시 그리는 것이 맞는지(뷰 상태는 `Rc<RefCell>`이라 창 뷰를 알리지 않아도 다음 그리기에 반영된다).
- `paint`는 장면을 `borrow()`한 채 `paint_layer` 안에서만 쓴다. 콜백에서 상태를 가변으로 빌리는 일은 없다.
