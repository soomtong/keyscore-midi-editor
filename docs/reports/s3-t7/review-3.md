# 적대적 리뷰: S3-T7 검색과 노트 정보 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트, 구현 컨텍스트를 물려받지 않음. 리뷰 1·2 리뷰어와도 다른 세션)
- 대상 커밋: `a014c2a` (`git diff main...HEAD`, 리뷰 2 뒤 변경은 `git diff 7e66b4b a014c2a`)
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t7_r3.rs`
- 실행 환경: `CARGO_TARGET_DIR=target/review`, `-p keyscore-app --test <이름>`만 돌렸다. MIDI·재생 시험과 전체 시험은 돌리지 않았다

## 0. 리뷰 2 지적의 확인

| 리뷰 2 지적 | 확인 | 근거 |
| --- | --- | --- |
| 1 수정: 보고서 게이트 칸 | 아직 `GATES_PLACEHOLDER`(`report.md:69`). 구현자가 병합 직전 마지막 HEAD에서 채운다고 했으므로 이것만으로 판정을 막지 않는다. 병합 전 확인 항목으로 남긴다 | `docs/reports/s3-t7/report.md:69` |
| 2 수정: vim `3 n` 카운트 | 고쳐짐(도달하는 결과는 맞다). 다만 고친 걷기가 새로 두 가지를 깨뜨렸다(7절 지적 1·2) | `search_bar.rs:219-243`, `search_bar.rs:278-280`. `review_s3_t7_r2::vim_count_n_moves_count_results` 통과 |
| 3 제안: 단계 파일 | 반영 | `plan/stages/13-S3-step-and-pianoroll.md` |
| 4 제안: 작은 오프셋 「+0 cent」 | 반영(0.1 cent). 0.05 cent 미만과 아주 작은 음수 오프셋은 여전히 「+0 cent」(7절 지적 3) | `note_info.rs:82-89` |
| 5 제안: 배치 뷰 `i` | 반영(결정 6절 1) | 결정 문서 |

이전 시험을 이번 HEAD에서 다시 돌렸다: `review_s3_t7_r1` 13 통과, `review_s3_t7_r2` 8 통과, `search` 18 통과, `search_pure` 10 통과, `note_info` 5 통과.

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 카운트 걷기 축약 `walks = (count - 1) % n + 1`이 `n`을 count번 누른 것과 같은 자리·같은 돎 문구를 내는가. 시작이 결과 위·결과 사이·첫 결과 앞·끝 결과 뒤, 노트 없는 트랙 결과가 섞일 때.
2. `wrapped = count > n`: count가 결과 수보다 크면 반드시 돌았는가, count ≤ n에서 돌 때도 문구가 붙는가.
3. 카운트가 새는 길: Step 모드에서 `3 n`(검색 불가), 지난 검색어 없이 `3 n`, `3 /`로 막대를 연 뒤 `Enter`하고 다음 모션(`l`)이 3칸 가는가.
4. 오퍼레이터와 카운트: `3 d n`, `d 3 n` 뒤 다음 오퍼레이터·모션에 카운트가 남는가.
5. 큰 카운트 × 많은 결과의 비용: `step`은 부를 때마다 결과 전체를 훑고 `Vec`를 새로 만든다. 걷기는 최대 min(count, n)번이라 O(min(count, n) · n). 노트 10,000개(NF-8의 규모)에서 `9999 n` 한 번이 화면을 멈추는가.
6. cent 0.1 표시: 0.05 cent 미만의 오프셋, 아주 작은 음수 오프셋의 부호, x.x5 반올림 경계, 99.95 → 100 올림, ±1200 cent 끝.
7. 기본 프리셋 `Cmd+G`와 메뉴·팔레트 경로는 카운트를 받지 않는가(`take_count`가 키 디스패치 밖에서 `None`).
8. 문서: 결정 5절의 항목 번호, 보고서·단계 파일 기록이 코드와 맞는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 카운트 = 키 반복 | 부분 | `search.rs::vim_count_beyond_hits_wraps`는 결과 둘, 노트 결과만, 결과 위 시작만. 노트 없는 결과가 섞인 경우 없음 |
| 2 count ≤ n에서의 돎 문구 | 없음 | 위 시험은 count > n만 |
| 3 카운트가 새는 길 | 없음 | 코드 읽기로는 `search_repeat`가 맨 앞에서 `take_count`, 그 밖은 `observe_keystrokes`가 비운다 |
| 4 오퍼레이터와 카운트 | 없음 | 리뷰 1 시험은 카운트 없는 `d n`만. 이번에도 따로 보지 않았다(실행하지 않은 시도는 적지 않는다) |
| 5 큰 카운트의 비용 | 없음 | |
| 6 cent 경계 | 부분 | `search_pure.rs::info_lines_show_pitch_start_length_channel`이 +25, +0.2, -12.5만 |
| 7 메뉴·팔레트·`Cmd+G` 카운트 | 해당 없음 | `take_count`가 키 디스패치 밖에서 `None`(`keymap/mod.rs:484-491`). 코드 읽기 |
| 8 문서 | 해당 없음 | 7절 지적 4 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 노트 결과만(c# 넷): 시작 넷(결과 위·사이·앞·뒤) × `k n`/`k N`(k = 1–5)을 같은 키 k번과 비교(현재 트랙·틱·음높이) | 못 깨뜨림 | `count_walk_equals_repeated_keys_on_note_hits` |
| 2 | 노트 결과와 노트 없는 트랙 결과가 섞임(「pad」 다섯): 같은 비교, k = 1–6 | 깨짐. Pad 3(노트 없음)에서 `5 n`은 (Pad 3, 0, 72), `n` 다섯 번은 (Pad 3, 0, 61). 도달한 결과는 같고 커서 음높이가 다르다. 걷기 루프는 중간 노트 결과의 음높이를 커서에 남기지 않고, 마지막이 노트 없는 결과면 걷기 전 음높이를 둔다 | `count_walk_equals_repeated_keys_with_empty_track_hits` |
| 3 | count ≤ n인데 중간에 도는 걷기(마지막 결과에서 `2 n`)의 돎 문구 | 못 깨뜨림 | `count_walk_within_hits_reports_wrap` |
| 4 | 지난 검색어 없이 `3 n` 뒤 `k`가 3칸 가는가 | 못 깨뜨림 | `count_with_no_last_search_does_not_leak` |
| 5 | `3 /` → `c#4` → `Enter` 뒤 카운트가 남는가 | 못 깨뜨림 | `count_before_slash_does_not_leak_past_bar` |
| 6 | 노트 10,000개(c#4, 한 트랙)에서 `9999 n`의 시간 | 깨짐. debug 빌드에서 `n` 한 번 34 ms, `9999 n` 16.2 s. `step`이 부를 때마다 결과 전체로 `Vec`를 만들고 훑으며, 걷기가 그것을 9999번 부른다(O(min(count, n) · n), 약 1억 비교와 1만 번의 40만 바이트 할당). release는 재지 않았으나 같은 차수라 초 단위 멈춤이 예상된다 | `large_count_walk_is_not_quadratic` |
| 7 | 오프셋 -1단위(아래로 굽음)의 표시 | 깨짐. 「C#4 (61) +0 cent」. 반올림이 0이면 부호를 `tenths`에서 가져와 「+」가 된다 | `tiny_negative_offset_is_not_shown_as_plus` |
| 8 | 오프셋 SEMITONE/4000(0.025 cent)의 표시 | 깨짐(제안 수준). 「+0 cent」. 결정 6절 2의 0.1 cent 반올림 그대로지만, 리뷰 2 제안 4의 「0처럼 읽힌다」가 0.05 cent 아래에 남는다 | `tiny_positive_offset_is_not_shown_as_zero` |
| 9 | 반올림 경계 99.96 → +100, 12.34 → +12.3, -12.36 → -12.4, ±1200 cent | 못 깨뜨림 | `offset_rounding_edges` |

리뷰 시험 9개: 5 통과, 4 실패.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 검색·정보 팝업은 문서·undo·`.`을 바꾸지 않는다 | 못 찾음 | 리뷰 2 뒤 변경은 `search_jump`의 걷기와 `pitch_text`뿐이고 둘 다 문서를 쓰지 않는다 |
| 카운트 `k`의 걷기는 키 k번과 같은 결과에 닿는다 | 결과(트랙·틱)는 못 깨뜨림. 커서 음높이는 깨짐 | 시도 1·2 |
| 카운트는 쓰거나 버리며 다음 키로 새지 않는다 | 못 깨뜨림 | 시도 4·5 |
| core 모델 불변식 | 해당 없음 | 문서를 고치지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8 `Cmd+F` 검색(`c#4`, 트랙 이름, 가사) | 충족 | `search.rs:47-90`, `search_bar.rs:75-130` |
| 5.8 `i` 커서 노트 정보 팝업(틱·길이·채널) | 충족 | `note_info.rs:18-69` |
| 5.8.1 vim `/`·`n`·`N`·`K` | 충족 | `vim.json`, `search_bar.rs:276-298` |
| 5.8.1 머리말 「반복은 숫자 카운트 접두」(`n`) | 충족 | `search_bar.rs:219-243`, `search_bar.rs:280`. 도달 결과는 키 반복과 같다(시도 1·2). 노트 없는 결과의 커서 음높이 차이는 결정 4절 1(「지금 커서 음높이」)을 글자 그대로 따른 것이라 REQ 미충족으로 세지 않는다 |
| 7.5 `Cmd+F` | 충족 | `standard.json:22` |
| P2 `Esc`는 통상의 취소 | 충족 | 리뷰 2와 같음 |
| 7절 모달 다이얼로그 없음 | 충족 | 리뷰 2와 같음 |
| NF-8 UI 프레임 60 fps(노트 10,000개) | 미충족(경계 사례) | 시도 6. `9999 n` 한 키가 debug 16 s 멈춘다. `search_bar.rs:224-243`, `search.rs:236-245` |

「부분 충족」은 미충족으로 센다: NF-8 하나.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(리뷰 2 뒤 변경에 `keyscore-midi`·엔진 없음)
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음. `search_bar.rs:225` `u32::try_from(...).unwrap_or(u32::MAX)`, `search_bar.rs:233` `track_index(...).unwrap_or(0)`(결과는 방금 지금 프로젝트에서 찾았으므로 `None`이 될 수 없다). `hits[f.index]`는 `step`이 `hits` 순번만 돌려준다
- 에러가 삼켜지는 곳: 없음

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 카운트 걷기가 O(min(count, n) · n)이다. 리뷰 2 지적 2의 수정이 새로 만든 회귀다. `step`은 부를 때마다 결과 전체로 `Vec`를 새로 만들고 훑는데 걷기가 그것을 최대 9999번 부른다. 노트 10,000개에서 `9999 n` 한 키가 debug 16.2 s(`n` 한 번 34 ms) 창을 멈춘다(NF-8). 결과는 이미 (틱, 트랙 순서, 음높이)로 정렬되어 있으므로 `step` 한 번으로 첫 결과를 찾고 나머지 `walks - 1`은 순번 산술(`(i ± (walks - 1)) mod n`, 돎은 순번이 끝을 넘었는가)로 구하면 O(n)이다. 시험 `large_count_walk_is_not_quadratic` | `crates/keyscore-app/src/search_bar.rs:224-243`, `crates/keyscore-app/src/search.rs:236-245` |
| 2 | 제안 | 노트 없는 트랙 결과에서 끝나는 카운트 걷기는 걷기 전 커서 음높이를 두고, 같은 키를 k번 누르면 마지막으로 지난 노트 결과의 음높이가 남는다(Pad 3에서 `5 n` → 72, `n` 다섯 번 → 61). 도달한 결과(트랙·틱)와 「N/M」는 같다. 결정 4절 1의 「지금 커서 음높이」를 글자 그대로 따른 것이라 버그라 하기 어렵지만 결정 5절 4의 「카운트만큼 걷는다」(vim `3n` = `n` 세 번)와 어긋난다. 걷기 루프의 `at.pitch`를 노트 결과에서만 바꾸고 그 값을 최종 커서 음높이로 쓰거나, 결정 5절 4에 「노트 없는 결과에서 끝나면 걷기 전 음높이」를 한 줄 적는다. 시험 `count_walk_equals_repeated_keys_with_empty_track_hits` | `crates/keyscore-app/src/search_bar.rs:228-243`, `search_bar.rs:255-259` |
| 3 | 제안 | 0.05 cent 미만의 오프셋은 여전히 「+0 cent」이고, 아래로 굽은 아주 작은 오프셋(-1단위)도 「+0 cent」로 부호가 뒤집힌다. 부호는 `units`에서 가져오고(「-0 cent」), 반올림이 0이면 「±0.1 cent 미만」 같은 표시를 쓰거나 결정 6절 2에 그 표시를 적기를 권한다. 시험 `tiny_negative_offset_is_not_shown_as_plus`, `tiny_positive_offset_is_not_shown_as_zero` | `crates/keyscore-app/src/note_info.rs:82-83` |
| 4 | 제안 | 결정 5절의 항목 번호가 1, 2, 4, 3 순서다(4가 2와 3 사이에 끼었다) | `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md:60-62` |
| 5 | 확인 항목 | 보고서 「자동 게이트」는 아직 `GATES_PLACEHOLDER`다. 구현자 예고대로 병합 직전 마지막 HEAD(이 리뷰 대응 뒤)에서 네 게이트 결과로 채운다. 「테스트 결과」에 `review_s3_t7_r3` 줄도 더한다 | `docs/reports/s3-t7/report.md:69` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2 지적 2(카운트)는 동작으로 고쳐졌고 도달하는 결과는 키 반복과 같다. 다만 그 수정이 큰 카운트에서 창을 초 단위로 멈추는 이차 비용을 새로 들였다(지적 1, 수정). 차단은 없다. 재리뷰는 지적 1의 수정과 `large_count_walk_is_not_quadratic`·`count_walk_equals_repeated_keys_on_note_hits` 통과, 보고서 게이트 칸의 확인만이면 된다. 제안 2–4는 받아들이거나 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`review_s3_t7_r3` 9개: 5 통과, 4 실패 `count_walk_equals_repeated_keys_with_empty_track_hits`, `large_count_walk_is_not_quadratic`, `tiny_negative_offset_is_not_shown_as_plus`, `tiny_positive_offset_is_not_shown_as_zero`)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 2 뒤 변경은 `search_bar.rs`, `note_info.rs`, 시험 둘, 결정·보고서·단계 파일)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(리뷰 2 뒤 새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
