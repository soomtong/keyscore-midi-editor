# Task 계획: S3-T5c 드럼 트랙의 조성 명암 제외와 건반 뷰 라벨 넘침

- 브랜치: `s3/t5c-drum-key-shading`
- 관련 REQ: 2 「조성 표시」, 5.13
- 선행 Task: S3-T1, S3-T5, S3-T5b
- 관련 결정: `docs/decisions/2026-10-31-key-shading-off-and-drums.md`
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

1. 피아노롤 장면(`pianoroll::scene::build`)은 현재 트랙이 드럼이면 행 명암을 조성 없음으로 계산한다(흰건반 행 밝게, 검은건반 행 어둡게). `row_shade` 자체는 그대로다.
2. 건반 뷰(`keyboard_view::keys`)는 `drum`이면 모든 건반의 `out_of_key`가 거짓이다.
3. 건반 뷰의 검은건반 이름 글(드럼 라벨 `n58` 등)이 건반 너비 안에 들어간다. 검은건반 글은 더 작은 글씨로 그린다. 이름 자리에 `keyboard-name-<i>` id를 단다.
4. 단계 파일: S3-T5 「남은 일」을 결정으로 닫고, S7 단계 파일에 결정 3의 이월 줄.

## 공개 인터페이스 초안

바뀌지 않는다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `scene_rows_on_drum_track_ignore_key` | 단위(`pianoroll.rs`) | 채널 10 트랙, F장조: 행 명암이 조성 없음과 같다 |
| 2 | `drum_track_has_no_key_shading` | 단위(`keyboard_view.rs`) | `keys(.., D장조, drum=true)`의 `out_of_key`가 모두 거짓, 멜로디는 그대로 |
| 3 | `drum_key_edit_keeps_keyboard_unshaded` | UI(`keyboard_view_ui.rs`) | 드럼 트랙 Step, 조성 D장조로 편집: 건반 뷰 `out_of_key` 없음 |
| 4 | `black_key_names_fit_inside_keys` | UI | 드럼 oct2·oct3, 넓은 창: 이름 글 너비 ≤ 건반 너비 |

## 불변식

- 멜로디 트랙의 명암은 바뀌지 않는다(기존 시험 그대로).

## 범위 밖

- 명암만 끄는 토글(결정 3, S7 이월).
- 드럼 라벨 키 매핑·피아노롤 행 라벨(S6-T4).

## 리스크·결정 요청

- 없음.
