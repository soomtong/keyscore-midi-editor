# Task 보고: S3-T5 건반 뷰

- PR: 로컬 브랜치 `s3/t5-keyboard-view`
- 계획 파일: docs/reports/s3-t5/plan.md
- 계획 대비 달라진 점:
  - 시험 16(`typed_labels_write_shown_pitches`)을 레이아웃마다 둘(`_piano`, `_solfege`)로 나눴다.
  - 접근자 `ProjectWindow::keyboard_view`는 새 파일 `keyboard_view_window.rs`에 두었다(피아노롤의 `pianoroll_window.rs`와 같은 나눔). 창의 `layout` 필드를 `pub(crate)`로 열었다.

## 구현 요약

- `crates/keyscore-app/src/keyboard_view.rs`(새, `pub mod`): `keys(layout, octave, key, drum) -> Vec<KeyCap>`(24건반의 음높이·검은건반·키 라벨·음 이름 또는 GM 라벨·조성 밖 여부)과 시험용 `describe`. 조성 명암은 피아노롤 `row_shade`, 음 이름은 `selection::pitch_name`(`pub(crate)`로 엶), 드럼 라벨은 `keyscore_text::drum::label_of`, 음높이는 Step의 `step::pitch_of`를 그대로 쓴다.
- `crates/keyscore-app/src/keyboard_view_window.rs`(새): `keyboard_view(cx)`(하단 영역이 펴지고 Step일 때만 `Some`), 그리기(흰건반 flex, 검은건반 절대 위치, 건반마다 `keyboard-key-<i>`와 접근성 라벨), 옥타브 버튼(`step::OctaveUp`·`OctaveDown`을 `run_action`으로 낸 뒤 문서로 포커스).
- `crates/keyscore-app/src/window.rs`: 하단 영역 자리에 `keyboard_view`가 있으면 건반 뷰, 없으면 기존 자리표시.
- 단계 파일: S5-T4(Play 건반 뷰·「연주」 버튼), S6-T4(드럼 라벨 키 매핑에 맞춰 건반 뷰 라벨 바꾸기, 레인과의 전환 규칙) 이월 줄, S3-T5 아래 「조성 표시 끄기」 미배정 남은 일.

## 테스트 결과

~~~
keyboard_view:     test result: ok. 5 passed; 0 failed
keyboard_view_ui:  test result: ok. 12 passed; 0 failed
하단 영역 기존 시험(window, review_s2_t1_r2, review_s2_t1_r3, review_s2_t5c, review_s2_t6f): 모두 ok
전체 cargo test --workspace --no-fail-fast: 아래 「자동 게이트」
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `labels_by_layout_and_octave_snapshot` | 통과(스냅샷 `keyboard_view__labels_by_layout_and_octave_snapshot.snap`) |
| 2 | `labels_at_octave_nine_mark_keys_above_127_absent` | 통과 |
| 3 | `drum_track_labels_snapshot` | 통과(스냅샷) |
| 4 | `out_of_key_follows_project_key` | 통과 |
| 5 | `piano_layout_has_sixteen_key_labels` | 통과 |
| 6 | `step_shows_keyboard_view_in_bottom_panel` | 통과 |
| 7 | `keyboard_view_follows_octave_keys` | 통과 |
| 8 | `octave_buttons_run_step_octave_actions` | 통과 |
| 9 | `octave_button_settles_composing_like_key` | 통과 |
| 10 | `octave_button_at_bound_says_range` | 통과 |
| 11 | `keyboard_view_in_arrangement_step` | 통과 |
| 12 | `closed_bottom_panel_hides_keyboard_view` | 통과 |
| 13 | `layout_setting_change_updates_labels` | 통과 |
| 14 | `drum_track_shows_gm_labels` | 통과 |
| 15 | `key_edit_updates_shading` | 통과 |
| 16 | `typed_labels_write_shown_pitches_piano`, `typed_labels_write_shown_pitches_solfege` | 통과 |

계획에 있었지만 쓰지 않은 테스트와 이유: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [ ] test(전체, `--no-fail-fast`): 돌리는 중
- [x] deny
- [x] `scripts/check-deps.sh`
- [ ] `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`: 돌리는 중

## 알려진 한계

- Play 모드의 건반 뷰·「연주」 버튼: S5-T4 이월.
- 드럼 트랙 라벨은 「지금 그 키가 쓰는 음」의 GM 라벨이다. REQ 5.13 표의 키 배치(`A` = `bd`)와 킷 페이지는 S6-T4 이월.
- 조성 표시 끄기(REQ 2 「끌 수 있음」): 피아노롤에도 없고 미배정. 사람에게 묻는다.
- 조성 안·밖이 명도로만 구분된다(색만 다른 두 상태). 접근성 검사는 S8.
- 건반형 뒤 8건반의 접근성 라벨은 `E5 D5`처럼 음 이름과 키 라벨이 같은 꼴이다(리뷰 2 제안 1, S8 접근성 문구).
- 설정 파일로 옥타브 대기를 0 ms로 두면 둘째 옥타브 라벨(`D5`, `c5`)의 숫자가 길이 코드로 읽힌다(리뷰 2 제안 2).
- 실제 창의 모양(건반 비율·글 크기)은 사람이 본다. 에이전트는 화면을 찍지 않는다(HANDOFF 교훈).

## 리뷰어에게

- 라벨 규칙 2(계이름형 둘째 옥타브 `c5`는 옥타브를 바꾸는 입력)와 3(드럼 라벨 해석)은 REQ 문장의 구체화다. REQ와 어긋나면 지적해 달라.
- 버튼은 모드 키 컨텍스트 밖(하단 영역)에서 `run_action`으로 Action을 낸다. 포커스·컨텍스트 경로에 따라 처리기가 닿지 않거나 다른 처리기가 받는 경우가 있는지.
- `keyboard_view`는 `self.step.layout`(그리기 전에 설정에서 읽어 둔 값)을 쓴다. Step 처리기와 같은 값이다.

## 리뷰 1 대응

판정 「수정 후 재리뷰」(`review-1.md`).

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | 받아들임. 건반형 뒤 8건반에 같은 키 + 옥타브 숫자(`D5`)를 단다. 계획 규칙 2와 시험 5·16, 스냅샷을 고쳤다. `piano_upper_keys_reachable_by_digit_but_unlabeled` 통과 | fb4d2e7 |
| 2 | 수정 | 받아들임. 드럼 트랙의 표에 없는 음은 `n<번호>`. 계획 규칙 3, 스냅샷, 시험 3·14를 고쳤다. `drum_unlabeled_notes_show_number` 통과 | 55886a4 |
| 3 | 제안 | 받아들임. 접근성 라벨 「옥타브 올리기」「옥타브 내리기」, 시험 `octave_buttons_have_action_labels` | e4b12e7 |
| 4 | 제안 | 받아들임. 리뷰 시험 `review_s3_t5.rs`(13개)를 PR에 담았다 | 0f1e895 |

## 리뷰 2 대응

판정 「병합 가능」(`review-2.md`). 리뷰 시험 `review_s3_t5_r2.rs`(12개)를 PR에 담았다.

| # | 심각도 | 대응 |
| --- | --- | --- |
| 1 | 제안 | 거절. 접근성 라벨 `E5 D5`는 계획 규칙 7의 형식(「음 이름 키 라벨」 순서)이다. 음과 키를 말로 구별하는 접근성 문구는 S8의 접근성 검사(색만 다른 상태 검사와 같은 Task)에서 화면 읽기 전체 문구와 함께 정한다. 보고서 「알려진 한계」에 적는다 |
| 2 | 제안 | 거절. 대기 0 ms는 설정 화면이 주지 않는 값(설정 파일 직접 편집)이고, 그때 음 뒤 숫자가 길이 코드가 되는 것은 Step 입력(S3-T4a)의 동작이지 건반 뷰의 결함이 아니다. 리뷰 시험이 지금 동작을 고정한다. 「알려진 한계」에 적는다 |
