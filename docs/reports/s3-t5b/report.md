# Task 보고: S3-T5b 건반 뷰 키 라벨을 키 하나로 닿는 건반에만

- PR: 로컬 브랜치 `s3/t5b-keyboard-direct-labels`
- 계획 파일: docs/reports/s3-t5b/plan.md
- 계획 대비 달라진 점: 음 이름 자리(높이 16 px)를 모든 건반에 비워 두었다. 실제 창에서 C 건반의 키캡만 한 줄 위에 떠 보였기 때문이다(키캡 높이 맞춤). `review_s3_t5` `mml_octave_updates_labels`의 기대값도 바꿨다(결정 문서 「바꾸는 시험」에 더함).

## 구현 요약

- `keyboard_view.rs`: 키 라벨은 레이아웃 표(건반형 16, 계이름형 12)의 자리에만. `WHITE_KEY_MAX_WIDTH`(40)·`KEY_MAX_HEIGHT`(120) 상수.
- `keyboard_view_window.rs`: 키 라벨을 키캡 상자(`keyboard-keycap-<i>`, 테두리·둥근 모서리)로, 음 이름은 상자 아래 고정 높이 자리에. 건반 판에 너비·높이 상한, 왼쪽 정렬·세로 가운데.
- 시험: 결정 문서 「바꾸는 시험」의 기대값 갱신(지우지 않음), 새 시험 `solfege_layout_labels_first_octave_only`, `keycaps_drawn_only_for_labeled_keys`, `keyboard_size_is_capped_in_wide_window`.

## 테스트 결과

~~~
keyboard_view:     6 passed; 0 failed
keyboard_view_ui: 15 passed; 0 failed
review_s3_t5:     13 passed; 0 failed
review_s3_t5_r2:  12 passed; 0 failed
~~~

실제 창 확인(에이전트, 2026-10-31): `target/debug/keyscore-app`을 띄워 앱이 맨 앞일 때만 `Ctrl+Tab` 하나를 보내 Step에 들어가 `screencapture -l`로 창만 찍었다. 건반형 oct4에서 키캡 A–P 16개가 한 줄로 서고, 뒤 8건반은 라벨이 없으며, 건반 판은 왼쪽 약 560 px에 120 px 높이로 그려졌다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(전체, `cargo test --workspace --no-fail-fast`, 7e43788): 2652 통과, 1 실패. 실패는 무관한 재생 타이밍 시험 `review_s2_t5a_r2::pause_position_does_not_drift_with_many_live_edits`(일시정지 자리 6227 < 들은 박 6240, HANDOFF의 알려진 지터)이고, 혼자 세 번 다시 돌려 모두 통과(13 passed)
- [x] deny, `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`

## 알려진 한계

- 조성 명암은 조성이 있는 프로젝트에서 사람이 본다(이번 촬영은 조성 없음).

## 리뷰어에게

- S3-T5 리뷰 시험의 기대값을 바꿨다. 결정 문서가 근거다. 바뀐 기대값이 결정 범위 밖의 동작까지 느슨하게 만들지 않았는지 봐 달라(특히 `review_s3_t5_r2`의 모든 옥타브 시험 단언).

## 리뷰 1 대응

판정 「수정 후 재리뷰」(`review-1.md`). 리뷰 시험 `review_s3_t5b.rs`(9개)를 PR에 담았다.

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | 받아들임. 모든 옥타브 시험이 건반마다 「라벨 있음 = 키 하나로 닿는 자리(건반형 16, 계이름형 12)이고 음이 있음」을 단언한다 | 5f4ac0a |
| 2 | 제안 | 받아들임. 옛 계획 규칙 2를 근거로 든 주석·절 제목을 결정에 맞게 고쳤다 | 6fa5dcc |
| 3 | 제안 | 고치지 않음. 리뷰 시험 `tall_bottom_panel_caps_key_height`, `wide_window_board_left_aligned_and_black_keys_on_boundaries`가 PR에 들어와 나머지 건반과 높은 하단 영역을 덮는다 | - |

## 리뷰 2 대응

판정 「병합 가능」(`review-2.md`). 변이 8가지를 모두 시험이 잡았다.

| # | 심각도 | 대응 |
| --- | --- | --- |
| 1 | 제안 | 받아들임. 결정 문서 「바꾸는 시험」의 시험 이름을 지금 이름으로 고치고 옛 이름을 괄호에 남겼다(뜻은 그대로인 사실 정정) |
