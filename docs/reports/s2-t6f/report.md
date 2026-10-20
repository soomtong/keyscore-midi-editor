# Task 보고: S2-T6f 이월 줄 정리

- PR: 브랜치 `s2/t6f-carry-overs`
- 계획 파일: docs/reports/s2-t6f/plan.md
- 계획 대비 달라진 점
  - 계획 7(앱 포트 이름의 트랙 창 알림)은 새 테스트를 만들지 않았다. S2-T6e 리뷰 3의 대조군 `review3_control_track_on_own_port_hides_same_name_destination`이 바로 그 경우를 돌리므로, 그 창(A)의 기대값을 「알림 없음」에서 「새로 보인다·없어졌다 한 번씩」으로 바꿨다.
  - 리뷰 부록 테스트는 원래 붙이라고 한 파일(`review_s2_t1_r3.rs`, `review_s2_t6b_r3.rs`)에 그대로 붙였다(그 파일의 도우미를 쓴다). 붙이면서 고친 것은 `size` import 하나와 clippy `doc_markdown` 백틱뿐이다.
  - 창 옵션을 보는 함수는 `project_window_options(cx)`와 `settings_window::window_options()`다.

## 구현 요약

- undo 상한 알림(결정 A)
  - core `History`가 상한 때문에 버린 단위를 센다.
  - `Document::dropped_undo_units()`는 읽기만 하는 공개 API다.
  - 앱 `ProjectDocument::with_limit`·`dropped_undo_units`, `HISTORY_LIMIT_BYTES`(256 MiB)를 공개한다.
  - `open_project_window_with_history_limit`: 시험이 작은 상한으로 창을 연다.
  - 창은 문서가 바뀔 때 버린 수가 0에서 처음 늘면 상태줄에 한 번만 알린다(`notice_history_limit`). 문구는 「되돌리기 기록이 메모리 상한(256 MiB)을 넘어 가장 오래된 단계부터 버렸다」다.
- 바탕 레이어 `alt-cmd-i`: 다음 넷에서 나오지 않음을 시험으로 고정했다. 코드는 바꾸지 않았다(키맵 레이어와 명령 표가 바탕 레이어를 담지 않는다).
  - 키맵 편집기의 실효 바인딩과 충돌 검사
  - 팔레트·메뉴 키 표시
  - 오버레이
  - 바탕 레이어에는 디버그 빌드에서 그 키가 있음을 함께 확인한다(시험이 헛돌지 않게).
- 패널 크기: 창 크기가 바뀌면(`observe_window_bounds`) 패널 크기 상태를 새로 만든다(`reset_panel_states`, 접기·펴기와 같은 길).
  - 사이드·아래 패널은 저장된 크기로 그려지고, 메인 영역이 늘고 준다.
  - 이어지는 조절이 비율로 줄어든 값을 저장하지 않는다.
- 창 최소 크기
  - 프로젝트 창은 720×480이다(`PROJECT_WINDOW_MIN_SIZE`).
  - 설정 창은 560×400이다(`SETTINGS_WINDOW_MIN_SIZE`).
- 오버레이 인자 라벨: 원문 항목을 키·이름으로 찾지 않는다. 항목의 인자로 `build_action`을 한 뒤 엔진이 그 키에 고른 Action과 `partial_eq`로 맞춘다.
- 핫플러그 알림: `windows_using`은 앱 가상 포트 이름이면 아무 창도 쓰지 않는다고 본다.
  - 앱 포트가 이겨 같은 이름 목적지는 쓰이지 않으므로, 그 목적지의 오감은 모든 창에 핫플러그 알림으로 보인다.

## 테스트 결과

~~~
cargo test --workspace: 1566 passed, 0 failed, 2 ignored(리뷰 3 뒤 최종 게이트. 무시 둘은 이전부터 있던 자식 프로세스 전용 테스트)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `document_counts_dropped_undo_units`(keyscore-core `tests/undo_limit.rs`) | 통과 |
| 2 | `undo_limit_notice_shows_once` | 통과 |
| 3 | `base_debug_key_is_hidden_from_editor_overlay_and_menus` | 통과(코드 변경 없음) |
| 4 | `window_resize_keeps_side_panels_at_stored_widths`, `inspector_resize_after_window_resize_keeps_track_list_width` | 통과 |
| 5 | `project_window_has_min_size` | 통과 |
| 6 | `argument_label_with_global_override`, `argument_label_editor_rebind_shadowed_by_deeper_context` | 통과 |
| 7 | `review3_control_track_on_own_port_hides_same_name_destination`(기대값 변경) | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`

## 알려진 한계

- 상한 알림 문구는 앱 상한(256 MiB)을 적는다. 시험용 작은 상한 창에서도 같은 문구다.
- 창을 움직여도(크기는 그대로) 경계 사건이 오면 패널 크기 상태를 새로 만든다. 그린 크기는 같다.
- 창 크기를 끄는 동안에는 프레임마다 크기 상태를 새로 만든다.

## 리뷰어에게

- 창 크기 변경에서 크기 상태를 새로 만드는 것이 끌기 중(패널 경계를 끄는 동안 창이 바뀌는 경우)이나 접힌 패널과 겹칠 때 저장 크기를 흔드는지.
- 오버레이 라벨: 같은 이름·같은 인자가 여러 키에 있을 때, 인자가 없는 Action, `build_action`이 실패하는 항목.
- 상한 알림이 undo·redo·코얼레싱(같은 키 묶음)에서 한 번만 나는지.

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 저장된 사이드 패널 합 + 메인이 창보다 크면, 창 크기 변경 뒤 한 패널만 조절해도 다른 패널 저장 너비가 바뀐다 | 그릴 때 창 너비에 맞춰 사이드 패널을 앱이 먼저 같은 비율로 줄여 요청한다(`fit_side_panels`, 메인 최소 120). 조절 사건에서는 요청한 너비와 비교해 더 많이 바뀐 패널만 저장한다. 메인이 최소 너비에 닿은 채 한쪽을 끌면 gpui-base가 반대쪽을 밀어내는데, 밀린 쪽은 저장하지 않는다. 처음 시도한 「마지막으로 그린 크기와 비교」는 끌기 중에도 그리기가 이어져 끈 패널을 놓쳤고, 「두 번 연달아 같은 크기」 기준은 창 크기 변경 뒤 그리기가 한 번뿐이라 잡히지 않아 버렸다 |
| 제안 2: 버린 수 세기 시험이 되돌리기·다시 실행 경로를 실제로 타지 않는다 | 리뷰어의 core 테스트 `tests/review_s2_t6f.rs`가 그 경로를 매 단계 확인하므로 그대로 남긴다 |
| 제안 3: 디버그 키 시험의 일부 단언은 늘 참 | 리뷰어의 `user_binding_on_debug_key_wins_over_base_layer`(사용자가 잡으면 그 바인딩이 이긴다)를 남겨 보완한다 |
| 제안 4: 최소 크기 시험이 상수끼리 비교 | 리뷰어의 최소 크기 배치 테스트를 남겨 보완한다. 상수 단언은 그대로 둔다(값을 바꿀 때 보이게) |

알려진 한계: 창이 좁아 사이드 패널이 줄어 보이는 동안 사람이 줄어 보이는 패널을 아주 조금(더 많이 바뀐 쪽이 아닐 만큼) 끌면 그 조절은 저장되지 않는다.

리뷰 테스트 `tests/review_s2_t6f.rs`(앱 11개)와 keyscore-core `tests/review_s2_t6f.rs`(1개)는 남기고 모두 통과한다.

## 리뷰 2 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 줄인 요청 너비가 패널 최소(120)보다 작으면 gpui-base가 120으로 그려 그 차이를 사람이 끈 것으로 본다 | `fit_side_panels`가 펴진 패널을 최소보다 작게 요청하지 않고, 한쪽이 최소에 걸리면 남는 자리를 다른 쪽에 준다 |
| 지적 2: 기준(`requested`)을 그릴 때마다 저장값에서 다시 계산해, 줄어 보이는 동안 한 번 저장한 뒤의 조절을 놓친다 | 기준을 「크기 상태의 지금 너비」로 바꿨다(`baseline`). 상태를 새로 만든 뒤 처음 그릴 때 요청한 너비로 채우고, 조절을 저장할 때마다 그 조절 뒤 너비로 옮긴다 |
| 제안 3: 720 창에서 트랙 목록·메인이 120보다 작다 | 지적 1 수정으로 트랙 목록은 최소 120을 지킨다. 메인은 gpui-base 최소(100)까지 줄 수 있다(창 최소 720에서 기본 패널은 줄지 않는다) |

리뷰 테스트 `tests/review_s2_t6f_r2.rs`(11개)는 남기고 모두 통과한다.

## 리뷰 3 대응

판정 「병합 가능」. 제안 하나(창 너비가 360보다 좁아 두 패널이 다 최소 120에 걸려 넘치면 gpui-base 크기 상태가 비율로 줄어 첫 기준과 어긋나, 이어지는 조절이 다른 패널 저장값을 바꿀 수 있다)는 프로젝트 창 최소 크기(720)를 macOS가 지켜 실제 앱에서는 생기지 않으므로 알려진 한계로 둔다. 리뷰 테스트 `tests/review_s2_t6f_r3.rs`(8개)는 남기고 통과한다.
