# 적대적 리뷰: S3-T5c 드럼 트랙의 조성 명암 제외와 건반 뷰 라벨 넘침 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: efdb6d9 (`git diff main...s3/t5c-drum-key-shading`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일을 열기 전에 제품 코드(`scene.rs`, `element.rs`, `pianoroll_window.rs`, `keyboard_view.rs`, `keyboard_view_window.rs`)만 읽고 적었다.

1. 피아노롤 장면의 트랙은 `PianoRollElement::new(.., self.current_track(), ..)`에서 오고, 건반 뷰의 드럼 판정도 `current_track_is_drum`이 같은 `current_track()`을 쓴다. 두 경로가 같은 트랙을 보는지, 트랙 전환 뒤 둘이 함께 바뀌는지.
2. 채널 변경(1 → 10, 10 → 1)과 그 undo·redo 뒤 피아노롤 행 명암과 건반 뷰 `out_of_key`가 함께 갱신되는지. 장면은 `prepaint`마다 `scene::build`로 다시 계산되고 `rows`를 `clear`하므로 낡은 명암이 남지 않아야 한다.
3. 트랙이 없을 때(`track = None`)는 조성 명암이 그대로인지(드럼 판정이 거짓으로 떨어지는지).
4. 멜로디 트랙의 행 명암이 장조·단조·조성 없음에서 이전과 같은지(회귀).
5. 드럼 트랙의 건반 뷰 채우기 색이 조성 없음과 같은지(`shades_key`) — 흰건반·검은건반 모두.
6. 검은건반 이름 글 9 px가 모든 옥타브(0~8)의 드럼 라벨, 계이름형 라벨(`C#`, `D#`류), 멜로디 트랙에서 건반 안에 들어가는지. 좁은 창에서도.
7. 넘침 시험이 글씨 줄이기를 빼면 실제로 실패하는지.
8. 드럼 판정을 빼면(`build_rows`의 분기, `keys`의 `!drum`) 각 시험이 실패하는지.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 피아노롤·건반 뷰가 같은 현재 트랙을 보고 트랙 전환 뒤 함께 바뀜 | 없음 | 구현 시험은 단일 트랙만. 건반 뷰는 리뷰 시험으로 보강. 피아노롤 장면은 창 밖에서 읽을 공개 API가 없어 창 수준 시험 불가(코드 읽기로 확인: `pianoroll_window.rs:84`가 `current_track()`을 넘기고 `element.rs:202`가 `prepaint`마다 프로젝트에서 다시 찾는다) |
| 2. 채널 변경·undo·redo 뒤 갱신 | 없음 | 구현 시험은 조성 편집(`drum_key_edit_keeps_keyboard_unshaded`)만. 리뷰 시험으로 건반 뷰 보강 |
| 3. 트랙 없음이면 조성 명암 유지 | 없음 | 리뷰 시험으로 보강 |
| 4. 멜로디 행 명암 회귀 | 부분(`scene_rows_shade_in_key_pitches` F장조 두 행) | 리뷰 시험으로 채널 16 × 조성 25 × 128행 전수 |
| 5. 드럼 건반 채우기 색 = 조성 없음 | 있음(`keyboard_view_window::tests::drum_track_draws_like_no_key`, 검은건반 하나) | 흰건반은 `key_fill`이 `has_key`를 보지 않아 해당 없음 |
| 6. 검은건반 이름 글이 모든 옥타브·계이름형·멜로디에서 들어감 | 부분(`black_key_names_fit_inside_keys`는 건반형 oct2·oct3만) | 리뷰 시험에서 oct6~9 `n100`~`n127`이 넘침 |
| 7. 글씨 줄이기를 빼면 넘침 시험 실패 | 보고서에 주장 | 리뷰에서 실제로 확인(실패함) |
| 8. 드럼 분기를 빼면 시험 실패 | 있음 | 리뷰에서 변이로 확인 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 멜로디(채널 1)·드럼(채널 10) 두 트랙, D장조, Step. `cursor::LastTrack` → `FirstTrack`으로 오가며 건반 뷰 명암·라벨 | 못 깨뜨림 | `keyboard_view_follows_switch_between_melody_and_drum_tracks` |
| 2 | 채널 1 → 10 → undo → redo → 11 → undo | 못 깨뜨림 | `keyboard_view_follows_channel_change_undo_redo` |
| 3 | 채널 1~16 × 조성 25 × 128행 장면 행 명암을 `row_shade`와 대조 | 못 깨뜨림 | `scene_rows_shade_by_channel_and_key_everywhere` |
| 4 | 트랙 없음(`track: None`) + D장조 | 못 깨뜨림(명암 유지) | `scene_rows_without_track_keep_key_shading` |
| 5 | 장면 버퍼 재사용(창의 `prepaint`처럼)으로 멜로디 → 드럼 → 멜로디 | 못 깨뜨림(`rows.clear()`로 낡은 행 없음) | `scene_buffer_reuse_does_not_keep_stale_shading` |
| 6 | 드럼 트랙, 넓은 창, 건반형, 옥타브 0~9 전부에서 「글 + 4 px ≤ 건반」(구현 시험과 같은 기준) | 깨짐: oct6~9의 `n100`~`n127`(네 글자)이 9 px 글씨로 22 px, 검은건반 24 px. 22건 | `black_key_drum_names_fit_every_octave_piano` |
| 7 | 같은 것을 계이름형으로 | 깨짐(같은 22건. 이름은 레이아웃과 무관) | `black_key_drum_names_fit_every_octave_solfege` |
| 8 | 기본 시험 창 크기, 최소 창 크기(720×480) | 깨짐(같은 22건). 두 크기 모두 검은건반은 24 px라 세 글자 라벨은 들어간다. 최소 크기 아래(480 폭)에서만 세 글자도 넘치는데 실제 창은 그 크기가 될 수 없다 | `black_key_drum_names_fit_default_window`, `black_key_drum_names_fit_min_window` |
| 9 | 멜로디 트랙의 검은건반 이름 자리 | 못 깨뜨림(이름은 C 건반에만, 검은건반 이름 자리는 비어 있음) | `melody_black_keys_draw_no_name` |
| 10 | 변이 M1: `.when(cap.black, \|d\| d.text_size(px(9.)))` 제거 | 구현 시험 `black_key_names_fit_inside_keys` 실패 확인(보고서 주장 맞음) | (기존 시험) |
| 11 | 변이 M2: `build_rows`의 드럼 분기 무력화 | `scene_rows_on_drum_track_ignore_key`와 리뷰 시험 2개 실패 | (기존·리뷰 시험) |
| 12 | 변이 M3: `keys`의 `!drum` 제거 | `drum_track_has_no_key_shading`, `drum_key_edit_keeps_keyboard_unshaded`, 리뷰 시험 2개 실패 | (기존·리뷰 시험) |
| 13 | 변이 M4: `shades_key`가 `drum`을 무시 | lib 단위 시험 `drum_track_draws_like_no_key`만 실패. UI 시험 17개는 통과(창의 색은 관찰할 길이 없다) | (기존 시험) |

변이는 모두 `git checkout -- crates/keyscore-app/src`로 되돌렸다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 멜로디 트랙의 명암은 바뀌지 않는다(계획 「불변식」) | 아니오 | 시도 3: 채널 10 외 15채널 × 조성 25에서 `row_shade(key, pitch)`와 같음 |
| 드럼 트랙은 조성 명암 없음(결정 2) | 아니오 | 시도 1·2·3·5. 피아노롤 창 경로는 코드 읽기로만 |
| core 모델 불변식 | 해당 없음 | core 변경 없음 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| REQ 2 「조성 표시」(결정 1: 끄기 = 조성 없음, 결정 2: 드럼 제외) | 충족 | `pianoroll/scene.rs:146`, `keyboard_view.rs:91`, `keyboard_view_window.rs:45` |
| REQ 2 「조성」(전역 조성 하나) | 충족(변경 없음) | `element.rs`의 `key: project.key()` |
| REQ 5.13 (채널 10 = 드럼 트랙, 건반 뷰 라벨 공유) | 충족. 다만 라벨 `n100`~`n127`의 검은건반 글이 구현이 정한 여백 기준을 넘는다(지적 1, 표시 품질) | `keyboard_view_window.rs:199` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(그리기·장면 계산만 바뀜).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것은 `keyboard_view_window.rs:257`의 `PitchClass::new(2).unwrap()` 하나이며 `#[cfg(test)]` 안이다.
- 에러가 삼켜지는 곳: 없음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 검은건반 이름 글 9 px로도 네 글자 드럼 라벨 `n100`~`n127`(oct6~9에서 보임)은 22 px라, 구현이 보고서에서 정한 기준(글 + 양쪽 2 px ≤ 건반 24 px)을 넘는다. 구현 시험은 oct2·oct3만 봐서 놓쳤다. 넓은 창·기본 창·최소 창, 건반형·계이름형 모두 같은 22건. 글씨를 라벨 길이에 맞춰 더 줄이거나(예: 네 글자면 더 작게) 기준을 바꾸고, 시험을 모든 옥타브로 넓힌다. 재현: `black_key_drum_names_fit_every_octave_piano` 외 3개 | `crates/keyscore-app/src/keyboard_view_window.rs:199`, `crates/keyscore-app/tests/keyboard_view_ui.rs:375` |
| 2 | 제안 | 「드럼 판정이 현재 트랙 기준」이라는 핵심 주장이 피아노롤 쪽은 창 수준 시험이 없다(장면을 창 밖에서 읽을 수 없다). 코드상으로는 맞다(`pianoroll_window.rs:84` → `element.rs:202`, `prepaint`마다 다시 계산). 건반 뷰 쪽은 리뷰 시험이 트랙 전환·채널·undo·redo를 덮었다. 필요하면 `PianoRollState`에 시험용 행 명암 조회를 두는 방안 | `crates/keyscore-app/src/pianoroll/element.rs:202` |
| 3 | 제안 | 변이 M4(`shades_key`가 드럼을 무시)는 lib 단위 시험 하나만 잡고 UI 시험은 모두 통과한다. 창의 실제 색 경로(`render_keyboard_view`의 `has_key`)는 시험되지 않는다. 보고서가 밝힌 대로 이 단위 시험은 실패 먼저 확인 없이 구현과 함께 썼다 | `crates/keyscore-app/src/keyboard_view_window.rs:163` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1을 고친 뒤 재리뷰. 드럼 명암 제외(끝의 정의 1·2)와 멜로디 회귀 없음은 깨뜨리지 못했다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 실행한 시험

~~~
CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t5c
  10개 중 6 통과, 4 실패(지적 1: black_key_drum_names_fit_every_octave_piano·_solfege·_default_window·_min_window)
CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test keyboard_view_ui --test keyboard_view --test pianoroll
  17 + 7 + 14 모두 통과
CARGO_TARGET_DIR=target/review cargo clippy -p keyscore-app --test review_s3_t5c -- -D warnings: 경고 없음
~~~
