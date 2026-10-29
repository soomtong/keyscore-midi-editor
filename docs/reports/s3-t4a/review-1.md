# 적대적 리뷰: S3-T4a Step 입력 핵심 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `521aef7`(브랜치 `s3/t4a-step-input`, `main...` 3커밋: `115ab1a` `3a7e193` `521aef7`)
- 추가한 시험(커밋하지 않음): `crates/keyscore-app/tests/review_s3_t4a.rs`(11개), `crates/keyscore-core/tests/review_s3_t4a.rs`(속성 1개)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `Backspace`가 지우는 범위는 「현재 길이 한 스텝」(`plain_step`)이다. `Shift+음`(절반)·`n`(셋잇단)으로 친 직전 스텝과 길이가 다르면 엉뚱한 범위를 지우고 뒤 노트를 잘못된 양만큼 당기지 않는가.
2. 클립 안 쉼표가 클립 끝을 넘은 뒤의 음은 `step_host`에서 「틈」이 된다. 클립을 늘리지 않고 새 클립을 만들지 않는가(REQ 5.10).
3. 화음이 열린 채 옥타브 키(`[` `]` `z` `x`)·`n`이 오면 `step_commit`만 부르고 `step_settle`을 부르지 않는다. 결정 5절 「다른 Step 동작이 오면 먼저 화음을 닫는다」와 맞는가.
4. 대기 시간 타이머의 세대 번호: 변화표가 세대를 바꾼 뒤 첫 타이머가 조합을 일찍 확정하지 않는가.
5. 음높이 경계: 계이름 변화표로 0 아래·127 위로 나갈 때, `b #`이 다음 옥타브 C가 되는가.
6. 모드 나가기·undo가 열린 화음·조합을 정리하는가. 타이머가 Step 밖에서 쓰지 않는가(모든 모드 전환이 `apply_mode_event`를 지나는가).
7. 건반형 조합 중 `z` `x` `n`이 음 키와 부딪히지 않고 「확정 후 동작」인가.
8. core `splice`: 다음 클립·지우기·바꾸기·숨김 꼬리가 섞일 때 불변식(클립 겹침 없음, 노트 시작은 클립 안)과 undo 왕복.
9. undo 뒤 커서·직전 스텝(`last`)이 낡은 채 남아 타이가 엉뚱한 노트를 바꾸지 않는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 절반·셋잇단 뒤 `Backspace` | 없음 | `backspace_deletes_step_and_pulls_later_notes`는 같은 길이만 본다 |
| 2. 쉼표로 클립 끝을 넘은 뒤 입력 | 없음 | `outside_clip_creates_bar_clip_and_extends_at_end`는 끝 = 커서만 본다 |
| 3. 화음 중 옥타브·`n` | 없음 | 화음 시험은 `tab`·음 키로 닫는 경우만 |
| 4. 낡은 타이머 | 부분 | `enter_and_next_note_commit_immediately`가 다음 음 키 경우만 본다. 변화표 경우는 없음 |
| 5. 음높이 경계 | 부분 | 건반형 상한만. 계이름 변화표 경계 없음 |
| 6. 나가기·undo 정리 | 있음 | `leaving_step_commits_composition`, `each_commit_is_one_undo_unit`. 열린 화음으로 나가기는 없음 |
| 7. 건반형 조합 중 `z` `x` `n` | 부분 | `step_contexts_have_no_note_key_conflicts`가 바인딩만 본다. 동작 결과는 없음 |
| 8. `splice` 불변식 | 부분 | 속성 시험은 다음 클립·지우기·바꾸기·undo 없이 한 클립만 |
| 9. undo 뒤 커서 | 없음 |  |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `4 c` 뒤 `Shift+d` 다음 `Backspace`: D(240틱)만 지우고 뒤 노트를 240만큼 당겨야 한다 | 깨짐. 뒤 노트가 480 당겨져 1440이 아니라 1200, 커서는 480이 아니라 240(C 한가운데) | `backspace_after_half_note_restores_later_notes` |
| 2 | `4 n a n b n c` 뒤 `Backspace`: c(320)만 지우고 커서 640 | 깨짐. 커서 480(b 한가운데), 뒤 노트도 480 당겨짐 | `backspace_after_triplets_lands_on_previous_step` |
| 3 | 1마디 클립의 3박에서 `2 space`(클립 끝을 넘는 쉼표) 뒤 `c enter` | 깨짐. 클립이 `(0, 3840)`·`(3840, 3840)` 둘로 갈린다. 한 클립 `(0, 7680)`이 기대 | `rest_past_clip_end_then_note_extends_same_clip` |
| 4 | `ctrl-c enter` 뒤 `]` | 깨짐(결정 5절). 화음이 열린 채 남고 커서도 0 | `octave_key_closes_open_chord` |
| 5 | `ctrl-c enter` 뒤 `n` | 깨짐(결정 5절). 화음이 열린 채 남는다 | `triplet_key_closes_open_chord` |
| 6 | 옥타브 0 `c -`, 옥타브 9 `g #`, `b #` | 못 깨뜨림. `c -`는 B-1(11)로 0–127 안이라 쓰고, `g #`(128)은 쓰지 않고 커서 그대로, oct4 `b #` = C5 | `solfege_accidental_out_of_range_is_refused` |
| 7 | `c`, 200 ms 뒤 `#`, 150 ms 더(첫 타이머 만료 시점 지남) | 못 깨뜨림. 낡은 타이머는 확정하지 않고, 뒤 `5`가 C#5로 확정 | `stale_timer_after_accidental_does_not_commit_early` |
| 8 | 화음이 열린 채 `Esc` 뒤 다시 Step | 못 깨뜨림. 화음이 닫히고 커서 960 | `leaving_with_open_chord_closes_it` |
| 9 | 건반형 조합 중 `x`, `n` | 못 깨뜨림. 옛 옥타브로 확정 뒤 동작 | `piano_pending_z_x_n_commit_first` |
| 10 | core `splice`에 다음 클립·임의 지우기·바꾸기(길이 늘림)·밀기·당기기·넣기를 섞고 undo | 못 깨뜨림. 거부면 문서 그대로, 적용되면 겹침 없음·시작은 클립 안·다음 클립 그대로·undo 왕복 일치 | `splice_with_next_clip_keeps_invariants_and_undoes`(core) |
| 11 | `c d` 뒤 `cmd-z`, 이어서 `e` | 깨지지 않음(관찰). 커서가 1920에 남아 E가 1920에 들어가고 960–1920이 빈다 | `undo_in_step_leaves_cursor_after_removed_note` |
| 12 | 틈에서 첫 음 `c` 뒤 `Backspace` | 깨지지 않음(관찰). 노트는 지워지고 빈 1마디 클립이 남는다 | `backspace_first_note_leaves_empty_clip` |
| 13 | undo 뒤 `~`(낡은 `last`) | 못 깨뜨림(코드 확인). `replace`의 원래 노트가 없으면 `splice`가 `NoteNotFound`로 거부해 엉뚱한 노트를 바꾸지 않는다. 클립이 사라졌으면 「바로 앞에 친 음이 없다」 | 없음(시도 11의 상태에서 코드 경로 확인) |

실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t4a`(6 통과, 5 실패: 시도 1–5), `CARGO_TARGET_DIR=target/review cargo test -p keyscore-core --test review_s3_t4a`(1 통과).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | `splice` 연장 검사 + `Track::replace_notes`/`resize_clip` 재검사. 속성 시험 시도 10 |
| 노트 시작은 클립 안 | 못 깨뜨림 | 당기기는 `NoteBeforeClipStart`로 거부, 밀기는 마디 연장. 시도 10 |
| 숨김 꼬리는 분할로만 생김 | 못 깨뜨림 | `splice`가 넣는 노트는 꼬리가 아니고(끝까지 연장), 원래 꼬리만 꼬리로 남는다(`splice_keeps_hidden_tail_as_tail`) |
| 확정 하나 = undo 하나 | 지켜짐 | `apply_edit` 한 번. 다만 `Backspace` 지적 1은 undo 단위가 아니라 내용 문제 |
| 조합이 있으면 `Pending` 경로 | 지켜짐 | `step_context_path` |
| Step 밖에서 타이머가 쓰지 않음 | 지켜짐 | 모든 모드 전환이 `apply_mode_event`를 지나며 `step_settle`(window.rs:303–305) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.1 길이·`Shift`·`n`·옥타브·화음·타이 | 충족 | step_input.rs:307–488, 628–697 |
| 5.1 `Backspace`는 한 스텝 뒤로 가며 그 자리 노트를 지운다 | 부분 충족 | step_input.rs:557–558. 절반·셋잇단 뒤에는 「그 자리」가 아니다(지적 1) |
| 5.6 문법·입력 예 표(두 레이아웃) | 충족 | `req_5_6_rows_*` |
| 5.6 300 ms·`Enter`·다음 음·모드 나가기 확정 | 충족 | step_input.rs:345–361, window.rs:303–305 |
| 5.6 변화표(계이름) | 충족 | `b` 내림은 B27로 빠짐(keymap-conflicts 결정). default.json `Step > Solfege > Pending` |
| 5.10 클립 밖 입력은 마디 단위 새 클립 | 충족 | step_input.rs:247–250 |
| 5.10 입력이 클립 끝을 넘으면 클립이 마디 단위로 늘어난다 | 부분 충족 | 끝 = 커서일 때만 늘어나고, 쉼표가 끝을 넘으면 다음 음이 새 클립이 된다(지적 2, step_input.rs:197–207, 497–502) |
| 5.11 Step `Tab`/`Shift+Tab`, 화음 줄 | 충족 | step_input.rs:598–624 |
| PRE-1 Step 음 키 미리듣기 | 충족(경로만) | step_input.rs:291–303, 339. 소리 자체는 시험 환경에 엔진이 없어 보지 않음 |
| KM-7 | 충족 | 기호 키 그대로(`!`…`&`, `#` `+` `-`) |
| EDT-1 / 표 「Step 모드 undo: 노트 하나 = undo 한 단위」 | 충족 | `each_commit_is_one_undo_unit` |

「부분 충족」 2건(5.1 `Backspace`, 5.10 연장)은 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 스케줄러를 건드리지 않는다. 미리듣기는 기존 `sound_note`)
- 제품 코드의 `unwrap`/`expect`: 없음(`step.rs`의 `unwrap`은 `#[cfg(test)]` 안). `unwrap_or(0)` 두 곳(step.rs `semitone_name`, step_input.rs:573)은 도달 불가 분기로 정당
- 에러가 삼켜지는 곳: step_input.rs:349 `let _ = this.update(...)`(창이 닫힌 뒤 타이머, 정당). 쓰기 실패는 모두 상태줄 알림

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `Backspace`가 「현재 길이 한 스텝」(`plain_step`)을 지운다. `Shift+음`(절반)·`n`(셋잇단)으로 친 직전 스텝 뒤에는 방금 친 음 길이와 달라, 뒤 노트를 잘못된 양만큼 당기고(원래 자리보다 앞으로) 커서를 앞 노트 한가운데에 둔다. 셋잇단 셋을 친 뒤 `Backspace`로 고칠 수 없다(P6 「한 음 뒤로·재입력이 한 키로」). 커서가 직전 스텝 끝(`last.end`)에 있으면 그 스텝의 시작·길이를 쓰는 식이 필요하다. 결정 1절의 「한 스텝」 뜻을 함께 고친다. 시험: `backspace_after_half_note_restores_later_notes`, `backspace_after_triplets_lands_on_previous_step` | step_input.rs:557–558 |
| 2 | 차단 | 클립 안 쉼표가 클립 끝을 넘은 뒤 친 음은 그 클립을 늘리지 않고 끝에 붙은 새 클립을 만든다. 이어 친 선율이 마디선에서 두 클립으로 갈린다. REQ 5.10 「입력이 클립 끝을 넘으면 클립이 마디 단위로 늘어난다」 부분 충족. 결정 4절(커서가 든 클립 = 시작 ≤ 커서 < 끝, 또는 끝 = 커서)이 이 경우를 다루지 않는다. 코드로 고치거나(예: 쉼표가 클립 안에서 시작해 끝을 넘으면 클립을 늘리거나, 직전 Step 입력의 클립을 기억해 이어 쓰기), REQ와 다른 해석이면 결정 요청으로 사람에게 묻는다. 시험: `rest_past_clip_end_then_note_extends_same_clip` | step_input.rs:192–208, 497–502 |
| 3 | 수정 | 결정 5절 「화음이 열린 채 다른 Step 동작이 오면 먼저 화음을 닫는다」와 다르다. 옥타브 키(`[` `]` `z` `x`)와 `n`은 `step_commit`만 불러 화음이 열린 채 남는다. 지금 동작(`ctrl-c ] ctrl-c`로 옥타브 화음)이 더 쓸모 있다고 보면 결정 5절(에이전트 해석)을 고쳐 예외를 적고 시험으로 고정한다. 어느 쪽이든 코드와 결정이 같아야 한다. 시험: `octave_key_closes_open_chord`, `triplet_key_closes_open_chord` | step_input.rs:460, 470 |
| 4 | 제안 | Step에서 `Cmd+Z`는 커서를 돌리지 않는다. `c d cmd-z e`는 E를 1920에 써 960–1920이 빈다(`undo_in_step_leaves_cursor_after_removed_note`). Step 확정의 undo는 커서를 그 스텝 시작으로 돌리는 편이 「한 음 뒤로·재입력」(P6)에 맞다 | window.rs:554–563 |
| 5 | 제안 | 틈에서 첫 음을 `Backspace`로 지우면 그 음이 만든 빈 클립이 남는다(`backspace_first_note_leaves_empty_clip`). 결정 4절 「진입만 하고 나가면 빈 클립이 남지 않는다」의 취지와 어긋난다. 적어도 보고서 「알려진 한계」에 적는다 | step_input.rs:545–596 |
| 6 | 제안 | 상태줄 조합 표시가 내림표를 올림 이름으로 바꾼다(`c -`가 `b_`, 실제 음은 B3인데 `oct4`와 나란히 보여 B4로 읽힌다). 또 `n` 뒤 길이 코드(`n5c`)는 `prefix`를 지워 `5c_`로 보이지만 셋잇단은 적용된다 | step.rs `semitone_name`, step_input.rs:451–452 |
| 7 | 제안 | core 속성 시험이 다음 클립·`remove`·`replace`·undo 왕복을 다루지 않는다. 리뷰의 `splice_with_next_clip_keeps_invariants_and_undoes`(통과)를 `transform_splice.rs`로 옮기면 「core 불변식 proptest 필수」를 더 넓게 채운다 | crates/keyscore-core/tests/transform_splice.rs:265 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

판정: 「수정 후 재리뷰」. 지적 1–3을 해결(코드 수정 또는 결정 문서 갱신·결정 요청)한 뒤 다시 리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(기존 시험 4개의 수정은 상태줄 `STEP` → `STEP ♩ oct4`, 경로 `Step > Piano`에 맞춘 기대값 갱신뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
