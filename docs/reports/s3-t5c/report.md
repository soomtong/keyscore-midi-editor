# Task 보고: S3-T5c 드럼 트랙의 조성 명암 제외와 건반 뷰 라벨 넘침

- PR: 로컬 브랜치 `s3/t5c-drum-key-shading`
- 계획 파일: docs/reports/s3-t5c/plan.md
- 계획 대비 달라진 점:
  - 실제 창 확인에서 드럼 트랙의 검은건반이 조성이 있을 때 「조성 안」 회색(명도 0.36)으로, 조성 없음의 검정(0.2)과 달리 그려졌다. 그리기도 드럼이면 조성이 없는 것으로 다룬다(`shades_key`, 단위 시험 `drum_track_draws_like_no_key`). 이 시험은 구현과 같은 커밋에서 썼다(먼저 실패를 확인하지 않았다). 옛 계산(`key != None`)이면 `!shades_key(d_major, true)` 단언이 실패한다.
  - 넘침 시험 기준은 「글 너비 + 양쪽 2 px ≤ 건반 너비」. 처음 기준(글 ≤ 건반)은 글씨를 줄이지 않아도 통과해(12 px `n58` = 22 px, 검은건반 24 px) 고쳤고, 줄이기를 빼면 실패함을 확인했다.

## 구현 요약

- `pianoroll/scene.rs` `build_rows`: 현재 트랙이 드럼이면 행 명암을 조성 없음으로 계산.
- `keyboard_view.rs` `keys`: `drum`이면 `out_of_key` 거짓.
- `keyboard_view_window.rs`: `shades_key`(드럼이면 조성 명암 없음), 이름 자리 `keyboard-name-<i>`, 검은건반 이름 글 9 px.
- 단계 파일: S3-T5 남은 일 닫음, S7 이월 줄(명암만 끄는 토글).

## 테스트 결과

~~~
keyboard_view 7, keyboard_view_ui 17, pianoroll 14, review_s3_t5 13, review_s3_t5_r2 12, review_s3_t5b 9: 모두 통과
keyscore-app lib keyboard_view_window::tests 1 통과
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `scene_rows_on_drum_track_ignore_key` | 통과 |
| 2 | `drum_track_has_no_key_shading` | 통과 |
| 3 | `drum_key_edit_keeps_keyboard_unshaded` | 통과 |
| 4 | `black_key_names_fit_inside_keys` | 통과 |
| (더함) | `keyboard_view_window::tests::drum_track_draws_like_no_key` | 통과 |

실제 창 확인(에이전트, 2026-10-31): 트랙 추가 → D장조 → 채널 10 → Step → oct2. 드럼 건반에 조성 명암이 없고 검은건반은 조성 없음과 같은 검정, `n58`·`tb`·`cb`가 건반 안에 들어간다. 클릭·키는 앱이 맨 앞일 때만 보냈고, 앱은 Cmd+Q → 「저장 안 함」으로 끝냈다. 앞선 확인에서 `kill`로 끝낸 실행이 남긴 자동 저장본(빈 「제목 없음」, 에이전트 시험 데이터)은 복구 창에서 「버리기」로 지웠다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [ ] test(전체): 리뷰 뒤 돌린다
- [ ] deny, scripts: 의존 변화 없음. 병합 전 함께 돌린다

## 알려진 한계

- 명암만 끄는 토글 없음(결정 3, S7 이월).

## 리뷰어에게

- 드럼 판정은 「현재 트랙」 기준이다(피아노롤은 장면 입력의 트랙). 트랙 전환·채널 변경·undo 뒤의 갱신을 봐 달라.

## 리뷰 1 대응

판정 「수정 후 재리뷰」(`review-1.md`). 리뷰 시험 `review_s3_t5c.rs`(10개)를 PR에 담았다.

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | 받아들임. 검은건반에는 세 글자까지만 그린다. 네 글자 `n100`–`n127`(GM 퍼커션 35–81 밖)은 접근성 라벨에만 남는다(근거는 리뷰 2 시험 `review_s3_t5c_r2.rs`, 드럼 트랙. 처음 든 `accessible_labels_match_model_for_every_key`는 멜로디 트랙만 본다). 리뷰 시험 4개 통과 | 5607dbb |
| 2 | 제안 | 고치지 않음. 창 밖에서 장면을 읽는 공개 API를 시험만을 위해 열지 않는다. 장면 입력은 현재 트랙이고(`pianoroll_window.rs`) 그릴 때마다 다시 계산하며, 장면 시험이 드럼 분기를 잡는다 |  - |
| 3 | 제안 | 고치지 않음. 헤드리스 시험은 칠한 색을 읽지 못한다. `shades_key` 단위 시험이 판정을 잡고, 실제 색은 에이전트가 실제 창에서 확인했다(보고 「실제 창 확인」) | - |

## 리뷰 2 대응

판정 「병합 가능」(`review-2.md`). 리뷰 시험 `review_s3_t5c_r2.rs`를 PR에 담았다.

| # | 심각도 | 대응 |
| --- | --- | --- |
| 1 | 제안 | 받아들임. 리뷰 1 대응의 근거를 드럼 트랙 시험으로 고치고, 결정 문서에 「검은건반에는 세 글자까지만 그린다」를 적었다 |
| 2 | 제안 | 받아들임. 「세 글자 이하는 그린다」를 확인하는 리뷰 시험을 PR에 넣었다 |
