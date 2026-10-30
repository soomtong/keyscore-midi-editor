# 적대적 리뷰: S3-T5 건반 뷰 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(서브 에이전트, 구현 컨텍스트를 물려받지 않음). 1회차 리뷰어와도 다른 세션이다.
- 대상 커밋: 5c6a8ef (`s3/t5-keyboard-view`, `main...` 비교)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`keyboard_view.rs`, `keyboard_view_window.rs`, `window.rs` 하단 영역·`wrap_in_contexts`, `step_input.rs`(`step_key`, `step_set_octave`, `step_octave_by`), `step.rs::pitch_of`, `pianoroll/keys.rs`, 키맵 세 개(`default.json`, `vim.json`, `standard.json`)를 읽고 적었다. 시험 파일은 픽스처 헬퍼 부분(`keyboard_view_ui.rs` 앞 200줄)과 함수 이름 목록만 먼저 보았다.

1. 지적 1 수정(건반형 뒤 8건반 `PIANO_KEYS[i-12]` + `octave+1`)이 기본 옥타브 4만이 아니라 모든 옥타브 0–8에서 실제 입력과 맞는가. 특히 옥타브 8에서 `D9`–`G9`는 있고 `Y9`부터 없음, 옥타브 0에서 `D1`. 기존 시험 16은 옥타브 4에서만 친다.
2. 같은 검사를 계이름형에 대해 모든 옥타브에서(`c#9`, `g9` 경계, 옥타브 9의 둘째 옥타브 없음).
3. 라벨 `D5` 형식이 음이름 형식(`C5`)과 같아 그려진 글·접근성 라벨이 모호해지는가(`E5 D5`). 첫 옥타브 `A`·`E`·`D`·`F`·`G`도 음이름 글자와 겹친다.
4. 드럼 `n<번호>`가 텍스트 내보내기(`export.rs:705`)와 같은 글인가: 같은 음을 드럼 트랙에 쓰고 내보낸 글과 건반 뷰 이름을 대조한다. 라벨 표 경계(35, 56–70 사이, 81 너머).
5. 옥타브 버튼 컨텍스트: 하단 영역은 `wrap_in_contexts` 밖이다. `#content`의 `focus_document_on_press`와 `on_click`의 `window.focus(doc)`로 디스패치가 Step 경로에 닿는가. 배치 뷰 Step에서 버튼 뒤 `a`, 드럼 트랙에서 버튼, 조합 중(건반형 `d` 뒤) 버튼.
6. 버튼과 키의 등가: 조합 중 버튼은 `step_settle`로 확정하는데, 키 `]`도 같은 결과(노트·옥타브·메시지)인가. 건반형 `d` 조합 뒤 숫자 `5` 대신 `+` 버튼을 누르면 E4가 확정되고 옥타브 5(라벨이 말하는 E5와 다름은 정상인지).
7. 보이는 조건과 갱신: Step 안에서 뷰 전환(피아노롤 ↔ 배치), 라벨 입력 `D5` 뒤 건반 뷰가 `oct5`로 옮겨 가 같은 E5가 `D`로 보이는가, 접근성 라벨이 24건반 모두 모델(`name key`)과 같은가.
8. 건반형 `K`–`P`(윗줄, 반음 12–15)에 숫자를 붙였을 때의 뜻(`K` `5`). 라벨 규칙의 「뒤 숫자 = 그 음의 옥타브」와 실제 처리기가 일관한가(범위 밖이면 기록만).
9. vim 프리셋에서도 라벨 입력이 같은 음을 쓰는가(`vim.json`의 `Step`은 `{` `}` 등만 더한다).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 건반형 라벨 = 실제 입력, 옥타브 0–9 모두 | 일부. `typed_labels_write_shown_pitches_piano`는 옥타브 4만, 스냅샷은 oct0·3·4의 글만 고정 | 리뷰 시험으로 보탬 |
| 2. 계이름형 라벨 = 실제 입력, 옥타브 0–9 모두 | 일부(옥타브 4만, `labels_at_octave_nine_…`은 모델 글만) | 리뷰 시험으로 보탬 |
| 3. `D5` 꼴과 음이름 꼴의 모호함 | 없음. `octave_buttons_have_action_labels`는 버튼만 본다 | 지적 1(제안) |
| 4. 드럼 `n<번호>` = 텍스트 내보내기 | 있음(스냅샷 `drum_track_labels_snapshot`, `drum_unlabeled_notes_show_number`). 내보내기와는 코드로 대조: 같은 `format!("n{pitch}")`(`export.rs:705`, `keyboard_view.rs:75-76`), 내보내기 쪽 시험 `keyscore-text/tests/export.rs:123` | |
| 5. 배치 뷰·드럼 트랙의 옥타브 버튼 | 없음(배치 뷰는 보이는지만, 드럼은 버튼을 누르지 않음) | 리뷰 시험으로 보탬 |
| 6. 건반형 조합 중 버튼 = 키 | 일부(계이름형 `c` 조합 중 버튼만 `octave_button_settles_composing_like_key`) | 리뷰 시험으로 보탬 |
| 7. 라벨 입력 뒤 뷰 이동, 접근성 라벨 24개 = 모델, Step 안 뷰 전환 | 일부(`piano_digit_suffix_moves_view_octave`는 `oct5`만). 24건반 접근성 라벨 전수·뷰 전환은 없음 | 리뷰 시험으로 보탬 |
| 8. 윗줄 `K` + 숫자 | 없음(Task 범위 밖 S3-T4 동작이지만 라벨 규칙과의 일관성은 이 Task의 몫) | 리뷰 시험으로 보탬 |
| 9. vim 프리셋 | 없음 | 리뷰 시험으로 보탬 |

## 3. 깨뜨리기 시도

시험 파일 `crates/keyscore-app/tests/review_s3_t5_r2.rs`, `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t5_r2`로 실행했다(12개 모두 통과, 아래 11·12는 기대값을 고친 뒤). 기존 `keyboard_view`(5), `keyboard_view_ui`(13), `review_s3_t5`(13)도 같은 대상 디렉터리에서 모두 통과했다. `cargo clippy -p keyscore-app --test review_s3_t5_r2 -- -D warnings` 경고 0.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 건반형 옥타브 0–9마다 라벨이 있는 건반을 모두 쳐서 써진 음 = 건반 음, 라벨 없는 건반은 없음 건반뿐 | 못 깨뜨림(`D1`·`D9`·`G9` 경계 포함) | `typed_labels_write_shown_pitches_every_octave_piano` |
| 2 | 같은 검사를 계이름형에서(`c#9`, `g9`, oct9 둘째 옥타브 없음) | 못 깨뜨림 | `typed_labels_write_shown_pitches_every_octave_solfege` |
| 3 | 건반형 oct8 뒤 8건반 라벨이 `D9 F9 T9 G9`에서 끊기고 G9 = 127 | 못 깨뜨림 | `piano_octave_eight_upper_labels_stop_at_127` |
| 4 | 두 레이아웃, oct0·4·8·9에서 `keyboard-key-<i>` 접근성 라벨 24개 = 모델의 `<이름> <키>`/`없음` | 못 깨뜨림. 다만 건반형 뒤 8건반은 `E5 D5`처럼 음 이름과 키 라벨이 같은 꼴로 읽힌다(지적 1) | `accessible_labels_match_model_for_every_key` |
| 5 | 라벨 `D5`를 친 뒤 E5가 써지고 뷰가 `oct5`로 옮겨 같은 E5 건반이 `D`(`E5 D`)로 보임 | 못 깨뜨림 | `typed_upper_label_moves_view_and_relabels_same_key` |
| 6 | 배치 뷰 Step에서 `+` 클릭 → `a`가 C5, `−` 두 번 → `C3 A` | 못 깨뜨림 | `arrangement_step_octave_button_then_key` |
| 7 | 드럼 트랙에서 `−` 두 번 → `bd A`, `n40 D`, `d`가 40을 씀 | 못 깨뜨림 | `drum_track_octave_button_then_label` |
| 8 | 건반형 `d` 조합 중 `+` 클릭과 키 `]`의 결과(노트·옥타브·마지막 메시지) 비교 | 못 깨뜨림(둘 다 E4 확정 뒤 옥타브 5, 이어 `a` = C5) | `octave_button_while_piano_composing_matches_key` |
| 9 | vim 프리셋에서 건반형 oct4 라벨을 모두 쳐 봄 | 못 깨뜨림 | `typed_labels_under_vim_preset` |
| 10 | Step 안에서 뷰 전환(`ToggleView`) 뒤 건반 뷰 유무가 모드와 맞는가, `bottom-panel` 자리표시와 배타인가 | 못 깨뜨림 | `view_toggle_in_step_keeps_view_consistent_with_mode` |
| 11 | 건반형 윗줄 `k` `5`. 처음엔 「뒤 숫자 = 그 음의 옥타브」로 C5(72)를 기대해 실패(84)했다. 결정 `2026-10-29-s3-t4-step-input-details.md` 2절(사람 결정)이 「뒤 숫자는 줄 기준 옥타브」(oct4 `K` 뒤 `3` = C4)라 84가 맞다. 라벨 `D5` = E5도 같은 읽기(줄 기준 옥타브 5의 `D`)라 일관하다. 기대값을 결정대로 고쳤다 | 못 깨뜨림 | `piano_upper_row_key_with_digit_follows_row_octave` |
| 12 | 설정 `input.octave_wait_ms` = 0(설정 파일로만 가능, `settings.rs:200`이 0–2000을 받음)에서 라벨 `D5`를 침 | 라벨은 E5(76)인데 E4(64)가 써지고 `5`는 길이 코드가 된다. 설정 화면 선택지는 100 ms부터(`settings_tabs.rs:31`)라 제안으로 둔다(지적 2). 시험은 지금 동작을 기대값으로 적었다 | `zero_octave_wait_upper_label_commits_before_digit` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 건반 뷰는 문서·Step 상태를 바꾸지 않는다 | 아니오 | `keyboard_view::keys`는 순수 함수, `ProjectWindow::keyboard_view`는 `&self`. 바꾸는 길은 버튼의 `run_action`뿐(시도 6·7·8) |
| 라벨과 실제 입력이 어긋나지 않는다 | 기본·설정 화면이 고를 수 있는 값에서는 아니오(시도 1·2·9, 옥타브 0–9 전수). 설정 파일로 대기 0 ms를 주면 둘째 옥타브 라벨이 어긋난다 | 시도 12, 지적 2 |
| 없음(127 초과) 건반에는 라벨이 없다 | 아니오 | `keyboard_view.rs:53-61`, 시도 1·2·3 |
| 하단 영역에 건반 뷰와 자리표시 중 하나만 | 아니오 | `window.rs:1282-1291`, 시도 10 |
| 모델 불변식(겹침·클립 안·채널) | 해당 없음 | 이 Task는 `core`를 건드리지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.1 옥타브 `[` `]`, `Z` `X`(건반 뷰 반영) | 충족 | `keyboard_view_window.rs:58-68`, 시도 1·2(`[` `]`로 0–9 순회) |
| 5.4 「2옥타브 건반을 그려 어느 키가 어느 음인지 보여 준다」(Step에 옮겨 씀) | 충족(24건반 모두 라벨, 없음 건반 제외) | `keyboard_view.rs:62-71`, 시도 1·2·3. 1회차 지적 1 해소 |
| 5.4 「건반 뷰의 옥타브 버튼」 | 충족 | `keyboard_view_window.rs:79-126`, 시도 6·7·8 |
| 5.4 「연주」 버튼, Play 모드 건반 뷰 | 범위 밖(S5 이월 줄 `plan/stages/15-S5-sequence-and-play.md`) | |
| 5.6 뒤 숫자 = 옥타브(두 레이아웃), 스티키 | 충족(결정 2026-10-29 2절의 줄 기준 옥타브와 일관) | `keyboard_view.rs:63-70`, 시도 5·11 |
| 5.13 드럼 트랙 GM 라벨, 표에 없는 노트 `n<번호>` | 충족(라벨 키 배치 `A` = `bd`, 킷 페이지, 드럼에서 `[` `]` = 킷 페이지는 S6-T4 이월 줄) | `keyboard_view.rs:72-77`, 시도 7, `plan/stages/16-S6-expression.md`. 1회차 지적 2 해소 |
| 2 「조성 표시」 밝게/어둡게 | 충족 | `keyboard_view.rs:86`, `keyboard_view_window.rs:24-33` |
| 2 「조성 표시」 「끌 수 있음」 | 미충족이나 단계 파일 S3-T5 범위(「조성 강조」)에 없고 피아노롤도 없어 배정 질의로 올라가 있다(`plan/stages/13-S3-step-and-pianoroll.md:76`). 1회차와 같은 판단 | |
| 7.1 하단 영역 「건반 뷰 (Play·Step 모드)」 | Step 충족, Play는 S5 | `window.rs:1282-1291`, 시도 6·10 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. UI 스레드의 그리기와 Action 처리기만 바뀌었다.
- 제품 코드의 `unwrap`/`expect`: 이 Task가 더한 것은 없다. `keyboard_view.rs:52` `unwrap_or(0)`(i < 24), `:65` `PIANO_KEYS[i - 12]`는 `PIANO_KEYS.get(i)`가 `None`인 i ≥ 16에서만 불려 4–11이라 범위 안, `:70` `SOLFEGE_KEYS[i - 12]`는 12 ≤ i < 24라 범위 안. `describe`의 `unwrap_or`는 시험용 표시 기본값.
- 에러가 삼켜지는 곳: `describe`의 `let _ = writeln!`(String, 실패 없음). 버튼의 `run_action` 결과는 처리기의 상태줄 메시지로 드러난다(시도 8에서 키와 같은 메시지).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 1회차 지적 1의 수정으로 건반형 뒤 8건반의 키 라벨이 `D5` `F5` `G5`처럼 과학적 음이름과 같은 꼴이 됐다. 접근성 라벨은 `E5 D5`, `G5 G5`, `A5 H5`로 읽혀 화면 낭독에서 어느 쪽이 음이고 어느 쪽이 키인지 구별되지 않는다(그려진 글도 E5 건반에 `D5`만 보인다. C 건반은 `K`와 `C5`가 함께 보여 같은 꼴 둘이 나란하다). 접근성 라벨에 「키」 같은 구분어를 넣거나(`E5, 키 D 5`), 그리는 글의 꼴을 음이름과 다르게(예 `D·5`) 하는 것을 권한다. 계획 규칙 7의 형식(`<음 이름> <키 라벨>`)대로이므로 결함은 아니다. 시험 `accessible_labels_match_model_for_every_key`(통과, 지금 형식 확인) | `crates/keyscore-app/src/keyboard_view_window.rs:44-53`, `crates/keyscore-app/src/keyboard_view.rs:63-67` |
| 2 | 제안 | 설정 `input.octave_wait_ms`가 0이면(설정 파일로만 가능, `settings.rs:200`은 0–2000을 받고 설정 화면은 100부터) 음 키가 곧바로 확정돼 뒤 숫자가 옥타브가 되지 못한다. 그때 건반형 뒤 8건반(`D5`)과 계이름형 둘째 옥타브(`c5`) 라벨은 보이는 음을 쓰지 않는다(`D5` → E4 + 길이 16분음표). 계획 「불변식」 「라벨과 실제 입력이 어긋나지 않는다」의 예외다. 대기 0이면 숫자 라벨을 감추거나, 설정 읽기의 하한을 화면 선택지와 맞추거나, 계획에 한계로 적기를 권한다. 시험 `zero_octave_wait_upper_label_commits_before_digit`(지금 동작을 기대값으로 고정, 제안을 받아들이면 기대값을 바꾼다) | `crates/keyscore-app/src/keyboard_view.rs:62-71`, `crates/keyscore-app/src/settings.rs:200` |

1회차 지적 1·2·3의 수정은 맞다: 뒤 8건반 라벨은 결정 2026-10-29 2절(줄 기준 옥타브)과 일관하고 옥타브 0–9 모두에서 실제 입력과 같다(시도 1·3·5·11). 드럼 `n<번호>`는 텍스트 내보내기와 같은 글이다. 버튼 접근성 라벨은 동작을 말한다. 수정이 만든 회귀는 찾지 못했다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단·수정 지적이 없다. 제안 1·2는 구현자가 받아들이거나 거절 이유를 보고서에 적는다. 리뷰 시험 `review_s3_t5_r2.rs`(12개, 모두 통과)를 PR에 담는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(시험 파일은 헬퍼 부분과 함수 이름만 먼저 보았다. 1절 머리에 적음)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`selection.rs` `pitch_name`, `window.rs` `layout`의 `pub(crate)`는 계획·1회차에 적힘. `plan/stages/` 세 줄은 이월·남은 일 기록)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
