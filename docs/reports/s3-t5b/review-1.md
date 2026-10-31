# 적대적 리뷰: S3-T5b 건반 뷰 키 라벨을 키 하나로 닿는 건반에만 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 73484d3 (`git diff main...s3/t5b-keyboard-direct-labels`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일을 열기 전에 `keyboard_view.rs`, `keyboard_view_window.rs`, 결정·계획·보고만 읽고 적었다.

1. 결정 1: 건반형은 모든 옥타브(0–9)에서 0–15번 건반에만 `key`가 있고 16–23은 `None`. 계이름형은 0–11번만. 127을 넘는 건반(옥타브 9의 8번 이후)은 라벨 없음.
2. 라벨 = 실제 입력: 모든 옥타브·두 레이아웃·드럼 트랙에서 라벨이 있는 건반의 라벨을 Step에 치면 그 건반의 음이 써진다. 특히 건반형 `K` `O` `L` `P`(다음 옥타브 C–D#)와 옥타브 9의 경계.
3. 결정 2: `keyboard-keycap-<i>`가 라벨 있는 건반에만 있고 없는 건반에는 없다. 키캡 상자가 건반 안에 들어가는가(특히 검은건반, 계이름형 `c#` 같은 두 글자 라벨).
4. 결정 3: 넓은 창에서 흰건반 너비 ≤ 40 px, 건반 높이 ≤ 120 px. 높은 하단 영역에서도 높이 120 px 이하. 검은건반이 판 안에 머무는가(판 너비가 줄었을 때 `relative` 위치가 판 기준인지).
5. 좁은 창·낮은 하단 영역: 상한 도입이 기존 동작(창에 맞춰 줄어듦)을 깨지 않는가. 키캡과 음 이름 자리(16 px 고정)가 건반 높이를 넘치는가.
6. 건반 판이 옥타브 버튼 오른쪽에 왼쪽 정렬되고, 남는 자리는 오른쪽에 빈다.
7. 바뀐 기존 시험이 결정 범위 밖의 단언(라벨 = 실제 입력, 뷰 옥타브 따라감, 확정 순서)까지 느슨하게 만들지 않았는가. 시험이 지워지거나 `#[ignore]`되지 않았는가.
8. 접근성 라벨 `<음 이름> <키>` 형식이 라벨 없는 건반에서 `<음 이름>`만 남는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 모든 옥타브에서 라벨 자리 = 레이아웃 표 자리 | 부분 | 스냅샷은 건반형 oct0·3·4, 계이름형 oct4·8, 드럼 oct3·4만. `review_s3_t5_r2` 모든 옥타브 시험은 바뀐 단언(206줄) 때문에 라벨이 하나도 없어도 통과한다(지적 1) |
| 2. 라벨 = 실제 입력(모든 옥타브·두 레이아웃·드럼) | 부분 | 멜로디 트랙만. 드럼 트랙에서 라벨을 치는 시험은 `drum_label_matches_written_note`(한 건반)뿐. 옥타브별 라벨 수는 oct4만 센다(`type_every_label`) |
| 3. 키캡이 라벨 있는 건반에만, 건반 안에 | 부분 | 있음·없음은 `keycaps_drawn_only_for_labeled_keys`. 키캡이 건반 안에 들어가는지는 없음 |
| 4. 넓은 창·높은 하단 영역의 크기 상한, 검은건반 위치 | 부분 | `keyboard_size_is_capped_in_wide_window`는 기본 하단 높이(160)에서 0·1번 건반만 본다. 높은 하단 영역, 검은건반 가운데 맞춤, 판 밖으로 나감은 없음 |
| 5. 좁은 창·낮은 하단 영역에서 넘침 | 없음 | |
| 6. 판 왼쪽 정렬, 오른쪽 빈자리 | 없음 | |
| 7. 바뀐 기존 시험이 결정 범위 밖을 느슨하게 했는가 | 해당 없음(리뷰 항목) | 시험 수는 그대로이거나 늘었다(5→6, 13→15, 13→13, 12→12). `#[ignore]` 없음. 지적 1 참고 |
| 8. 라벨 없는 건반의 접근성 라벨이 `<음 이름>`만 | 있음 | `layout_setting_change_updates_labels`(`C5`), `mml_octave_updates_labels`(`C7`), `accessible_labels_match_model_for_every_key` |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t5b.rs`에 있고 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t5b`로 실행했다(9 passed).

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 옥타브 0–9 × 두 레이아웃 × 드럼 여부마다 라벨이 정확히 「표의 자리이고 음이 있는 건반」인가 | 못 깨뜨림 | `labels_exactly_on_direct_keys_every_octave_layout_drum` |
| 2 | 드럼 트랙(ch10) 건반형·계이름형, 옥타브 0–9에서 라벨을 모두 쳐 그 음이 써지고 옥타브가 안 바뀌며 친 라벨 수 = 표 자리 중 있는 건반 수 | 못 깨뜨림 | `typed_labels_every_octave_drum_piano`, `typed_labels_every_octave_drum_solfege` |
| 3 | 멜로디 건반형 옥타브 0–9에서 같은 검사(라벨 수 포함). 옥타브 8의 `K`–`P`(C9–D#9), 옥타브 9의 0–7번 | 못 깨뜨림 | `typed_labels_every_octave_melody_piano_counted` |
| 4 | 하단 영역 높이 600, 창 1800×1200에서 24건반 모두 높이 ≤ 120, 건반 뷰 안 | 못 깨뜨림 | `tall_bottom_panel_caps_key_height` |
| 5 | 창 2400×1000에서 흰건반 14개 모두 38–40.5 px, 검은건반 가운데가 앞 흰건반 오른쪽 경계와 1 px 이내, 첫 건반이 옥타브 표시 바로 오른쪽, 오른쪽 빈자리 > 1000 px | 못 깨뜨림 | `wide_window_board_left_aligned_and_black_keys_on_boundaries` |
| 6 | 최소 창 720×480에서 모든 건반이 건반 뷰 안, 판 오른쪽 끝(23번 흰건반)을 넘지 않음 | 못 깨뜨림 | `min_window_keys_stay_inside_view` |
| 7 | 창 1600×900, 두 레이아웃에서 키캡(검은건반 `c#` 포함)이 건반 안에 들어가고, 그려진 키캡 수 = 라벨 수, 키캡 너비 ≥ 16 | 못 깨뜨림 | `keycaps_fit_inside_keys_default_window` |
| 8 | 하단 영역 최소 높이 120 + 최소 창에서 같은 검사(검은건반 높이가 가장 낮은 경우) | 못 깨뜨림 | `keycaps_fit_inside_keys_low_bottom_min_window` |

창 최소 너비가 720 px(`PROJECT_WINDOW_MIN_SIZE`)라 실제 창에서 흰건반은 늘 상한 40 px에 닿는다. 그보다 좁은 창(시험 플랫폼에서만 가능)의 키캡 넘침은 실제 결함이 아니어서 시도하지 않았다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 라벨이 있는 건반은 그 라벨을 Step에 치면 그 음이 써진다 | 아니오 | 시도 1–3. 모든 옥타브·두 레이아웃·드럼 |
| 키 하나로 닿지 않는 건반에는 라벨이 없다(결정 1) | 아니오 | 시도 1 |
| 키캡 = 라벨 있는 건반(결정 2) | 아니오 | 시도 7·8, 구현자 `keycaps_drawn_only_for_labeled_keys` |
| 흰건반 ≤ 40 px, 건반 ≤ 120 px(결정 3) | 아니오 | 시도 4·5 |
| `core` 모델 불변식 | 해당 없음 | 이 Task는 `app` 그리기·라벨 계산만 바꾼다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.4 「화면 하단에 2옥타브 건반을 그려 어느 키가 어느 음인지 보여 준다」 | 충족(결정 1의 해석대로, 키 하나로 닿는 건반에 키, 모든 건반에 음 이름은 접근성 라벨로) | `keyboard_view.rs:71-75`, `keyboard_view_window.rs:185-187` |
| 5.4 「건반 뷰의 옥타브 버튼」 | 충족(변경 없음, 판 왼쪽 정렬 뒤에도 버튼 오른쪽) | `keyboard_view_window.rs:101-148, 221-231`, 시도 5 |
| 5.6 뒤 숫자 옥타브·스티키 | 충족(입력 처리 변경 없음. 라벨 없는 건반은 뒤 숫자로 닿는다) | `review_s3_t5.rs` `piano_upper_keys_reachable_by_digit_have_no_label` |
| 5.13 드럼 트랙 건반 뷰 GM 라벨 | 충족(음 이름 자리는 드럼 라벨을 모두 그린다) | `keyboard_view.rs:76-81`, `keyboard_view_window.rs:168-171`, 시도 2 |
| 2절 「조성 표시」 | 충족(변경 없음) | `keyboard_view_window.rs:24-33`, `keyboard_view.rs:90` |
| P3 건반형 키 | 충족 | `keyboard_view.rs:39-41` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 바뀐 코드는 UI 렌더와 라벨 계산뿐이다.
- 제품 코드의 `unwrap`/`expect`: 이번 변경에서 새로 생긴 것 없음. `keyboard_view.rs:59`는 `unwrap_or(0)`(i < 24라 실패하지 않음).
- 에러가 삼켜지는 곳: `keyboard_view.rs:102,105`의 `let _ = writeln!`(String에 쓰기, 실패하지 않음, 기존 코드).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 결정 범위를 넘어 느슨해진 단언. 전에는 else 갈래가 `pitch.is_none() && key.is_none()`이어서 「있는 건반은 모두 라벨이 있다」를 함께 단언했다. 결정 1에 맞추려면 「표의 자리이고 음이 있으면 라벨이 있다」로 좁혀야 하는데, 바뀐 단언 `pitch.is_some() \|\| key.is_none()`은 「없는 건반에는 라벨이 없다」만 남는다. 그래서 `typed_labels_write_shown_pitches_every_octave_piano`·`_solfege`는 어떤 옥타브에서 라벨이 모두 사라져도 통과한다(친 라벨 수를 세지 않는다). 옥타브별 라벨 수는 이제 oct4(`type_every_label`)와 스냅샷 옥타브에서만 지켜진다. 고칠 방법: 이 단언을 「`i < 표 길이`이고 음이 있으면 `key`가 있다」로 바꾸거나 옥타브마다 친 수를 센다. 리뷰 시험 `labels_exactly_on_direct_keys_every_octave_layout_drum`, `typed_labels_every_octave_*`가 같은 빈자리를 메우며 지금 코드에서는 통과한다(제품 결함이 아니라 시험 회귀) | `crates/keyscore-app/tests/review_s3_t5_r2.rs:205-206` |
| 2 | 제안 | 시험 doc 주석이 결정으로 바뀐 옛 규칙을 근거로 든다. 「계획 규칙 2: 옥타브 숫자 없이 닿는 키가 없으면 옥타브 숫자를 붙인 입력을 보인다」와 절 제목 「라벨이 비었다」는 이제 시험 단언(라벨 없음)과 반대로 읽힌다. 결정 문서를 근거로 고쳐 쓰기를 권한다 | `crates/keyscore-app/tests/review_s3_t5.rs:198-201` |
| 3 | 제안 | `keyboard_size_is_capped_in_wide_window`는 0·1번 건반과 기본 하단 높이만 본다. 높은 하단 영역·모든 건반·검은건반 위치는 리뷰 시험 `tall_bottom_panel_caps_key_height`, `wide_window_board_left_aligned_and_black_keys_on_boundaries`가 덮는다. 따로 고칠 필요는 없다 | `crates/keyscore-app/tests/keyboard_view_ui.rs:392-413` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

제품 코드(결정 1–3의 구현)는 깨뜨리지 못했다. 지적 1(시험 회귀)만 고치면 된다. 리뷰 시험 `review_s3_t5b.rs`는 프로토콜 [7]대로 저장소에 남긴다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`crates/keyscore-app`의 건반 뷰 두 파일, 결정이 지정한 시험·스냅샷, `docs/`만)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
