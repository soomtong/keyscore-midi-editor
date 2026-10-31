# Task 계획: S3-T5b 건반 뷰 키 라벨을 키 하나로 닿는 건반에만

- 브랜치: `s3/t5b-keyboard-direct-labels`
- 관련 REQ: 5.4, 5.6, P3
- 선행 Task: S3-T5(병합 5c99eed)
- 관련 결정: `docs/decisions/2026-10-31-keyboard-view-direct-labels.md`
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

결정 1–3을 구현한다.

1. `keyboard_view::keys`: 건반형은 첫 16건반, 계이름형은 첫 12건반에만 `key`가 있다. 나머지는 `None`. 127을 넘는 건반은 지금처럼 없음.
2. 그리기: 키 라벨은 테두리 있는 키캡 상자(`keyboard-keycap-<i>`), 음 이름은 상자 밖 작은 글자(멜로디는 C 건반만, 드럼은 모두). 접근성 라벨 형식(`<음 이름> <키>`)은 그대로.
3. 크기: 흰건반 최대 40 px, 건반 높이 최대 120 px. 건반 판은 옥타브 버튼 오른쪽에 왼쪽 정렬.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/keyboard_view.rs — 시그니처는 그대로
pub fn keys(layout: KeyLayout, octave: u8, key: Key, drum: bool) -> Vec<KeyCap>;
/// 흰건반 최대 너비·건반 최대 높이(논리 px).
pub const WHITE_KEY_MAX_WIDTH: f32 = 40.0;
pub const KEY_MAX_HEIGHT: f32 = 120.0;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `labels_by_layout_and_octave_snapshot`(기대값 갱신) | 스냅샷 | 건반형 뒤 8건반·계이름형 둘째 옥타브 라벨 없음 |
| 2 | `piano_layout_labels_follow_p3_keys_only`(이름·기대값 갱신) | 단위 | 건반형 16건반만 P3 순서 라벨 |
| 3 | `solfege_layout_labels_first_octave_only` | 단위 | 계이름형 12건반만 라벨 |
| 4 | `keycaps_drawn_only_for_labeled_keys` | UI | `keyboard-keycap-<i>`가 라벨 있는 건반에만 있다 |
| 5 | `keyboard_size_is_capped_in_wide_window` | UI | 넓은 창에서 흰건반 너비 ≤ 40 px, 건반 높이 ≤ 120 px |
| 6 | 기존 시험 기대값 갱신(결정 「바꾸는 시험」) | 단위·UI | 라벨 = 실제 입력, 라벨 없는 건반 |

## 불변식

- 라벨이 있는 건반은 그 라벨을 Step에 치면 그 음이 써진다(시험 `type_every_label`).
- 건반 뷰는 문서·Step 상태를 바꾸지 않는다.

## 범위 밖

- Play 모드 건반 뷰(S5-T4). 이 결정으로 그때 라벨 규칙을 바꿀 필요가 없다.

## 리스크·결정 요청

- 없음(결정 문서가 사람 승인을 받았다).
