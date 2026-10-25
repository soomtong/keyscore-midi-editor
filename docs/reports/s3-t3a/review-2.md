# 적대적 리뷰: S3-T3a 삭제·클립보드·반복 (리뷰 2회차)

- 리뷰어 세션: 구현자·1회차 리뷰어와 다른 세션(구현 컨텍스트를 물려받지 않은 새 에이전트)
- 대상 커밋: `831a8d12bcf153da52c3d6b84937f1c64491d780`(브랜치 `s3/t3a-delete-clipboard-repeat`). 1회차 뒤 수정은 `89262e9` `f52da16` `e7bdb4c` `af64f0f` `831a8d1`
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t3a_r2.rs`(9개, 실패 2), `crates/keyscore-core/tests/review_s3_t3a_r2.rs`(7개 + 속성 1개 포함, 실패 1)
- 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t3a_r2`, `-p keyscore-core --test review_s3_t3a_r2`. 회귀 확인으로 `review_s3_t3a`(app 20·core 3), `note_edit`(10), `clipboard_notes`(7), `transform_insert`(6), `pianoroll_select`(14), `review_s3_t2`(13)를 다시 돌려 모두 통과를 봤다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

1회차 이후 차분(`git diff 8326a45..HEAD`)과 `transform.rs` `insert_notes`·`clip_for_notes`, `pianoroll/edit.rs`, `note_edit.rs`, `clipboard.rs`, `window.rs`, 키맵을 읽고 적었다.

1. `clip_for_notes`의 새 시작 규칙(`e > bar_start && e <= at`): 자리 틱이 앞 클립 끝과 같은 틱, 앞 클립 끝이 마디선과 같은 경우, 박자표 변화 뒤 마디.
2. 새 끝 규칙(`s >= end && s < bar_stop`): 다음 클립이 노트 끝과 마디 끝 사이에서 시작할 때. 완전성(노트가 틈에 들어가면 늘 `Ok`인가)과 안전성(새 클립이 틈 밖으로 나가지 않는가).
3. 같은 「마디 중간 클립 경계」 문제가 있던 클립을 늘리는 `insert_notes`(붙여넣기·복제·반복)에는 그대로 남아 있지 않은가. 수정이 틈 쪽만 고쳐 자리 틱 한 틱 차이로 판단이 갈리는가.
4. 노트가 자리 틱 앞에 있는 경우(core API를 직접 부를 때) 오류 종류.
5. 틈 붙여넣기가 마디 중간에서 시작하는 새 클립을 만든 뒤의 선택·undo·redo와 이어지는 `r`.
6. vim `p`(커서 + 한 스텝)가 클립 마지막 칸에서 눌리면 자리 틱이 클립 끝이 되어 「클립을 늘림」이 아니라 새 클립이 되는가. 그 뒤 선택과 커서가 다른 클립이 되어 `r`이 거부되는가.
7. undo 뒤 선택(`select_appeared`)이 커서가 없는 클립의 노트를 고르면 다음 편집이 「선택이 현재 클립에 없다」로 막히는가.
8. 제안 4 대응(빠진 형식 알림): 이 경로를 실제로 일으키는 입력이 있는가, 문구.
9. 잘라내기의 상태줄 문구.
10. 제안 2 대응(`paste_contents`가 `changeCount`만 먼저 읽음): 내부 구조 판정이 그대로인가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 새 시작 규칙의 경계(같은 틱, 마디선, 박자표) | 부분 | `clip_for_notes_starts_after_prev_clip_and_stops_before_next_in_same_bar`는 자리 틱이 앞 클립 끝보다 뒤인 경우만 |
| 2 새 끝 규칙의 완전성 | 부분 | 같은 시험의 고정 예 하나. 1회차 속성 시험은 「Ok면 안전」만 본다 |
| 3 `insert_notes`의 마디 중간 다음 클립 | 없음 | `insert_notes_extends_clip_by_bars_and_rejects_overlap`은 다음 클립이 마디선에서 시작한다 |
| 4 자리 틱 앞 노트 | 없음 | |
| 5 마디 중간 새 클립의 선택·undo·redo·`r` | 없음 | 1회차 `gap_paste_undo_redo_selects_new_clip_notes`는 마디선 클립 |
| 6 vim `p` 클립 마지막 칸 | 없음 | `vim_p_shift_p_shift_x_shift_d_shift_y`는 클립 안 자리 |
| 7 커서와 다른 클립의 undo 선택 | 없음 | |
| 8 빠진 형식 알림 | 없음 | 보고서에 「일으키는 시험을 쓰지 못했다」 |
| 9 잘라내기 문구 | 없음 | |
| 10 내부 구조 판정 | 있음 | `paste_reads_external_smf_then_text`, 1회차 `external_text_after_internal_copy_wins` |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | core: 클립 A [0, 1마디), 다음 클립 B가 5000(마디 중간)에서 시작. A 안 3000에 1박 노트(끝 3960) `insert_notes` | 깨짐: 빈 자리 [3840, 5000)에 들어가는데 `ClipOverlap { start: 0, end: 7680 }` | core `insert_notes_fits_before_mid_bar_next_clip`(실패) |
| 1' | 같은 모양에서 A 끝 틱(3840)에 같은 노트 `clip_for_notes` | 못 깨뜨림(대조): [3840, 5000)의 새 클립. 한 틱 앞은 거부, 여기는 받아들임 | core `clip_for_notes_at_clip_end_fits_before_mid_bar_next_clip` |
| 2 | 앱: 같은 모양에서 C4를 복사해 3000에 `cmd-v` | 깨짐: 「붙여넣기: 클립 구간 [0, 7680)이 같은 트랙의 다른 클립과 겹친다」 | `paste_near_clip_end_fits_before_mid_bar_next_clip`(실패) |
| 2' | 앱: 같은 모양에서 E4(2880)에 `cmd-d`(사본 3840–4800) | 깨짐: 「복제: 클립 구간 [0, 7680)이 … 겹친다」 | `duplicate_near_clip_end_fits_before_mid_bar_next_clip`(실패) |
| 2'' | 앱: 같은 C4를 A 끝(3840)에 `cmd-v` | 못 깨뜨림(대조): 새 클립 [3840, 5000) | `paste_at_clip_end_before_mid_bar_next_clip_makes_new_clip` |
| 3 | core: 자리 틱 = 앞 클립의 마디 중간 끝(5760), 앞 클립 끝이 마디선(3840), 3/4 변화 뒤 마디 안의 앞 클립 끝 | 못 깨뜨림: [5760, 7680), 마디 시작 3840, [8680, 10560) | core `clip_for_notes_at_prev_mid_bar_end_tick_starts_there`, `clip_for_notes_prev_end_on_bar_line_starts_at_bar`, `clip_for_notes_mid_bar_prev_end_after_meter_change` |
| 4 | core: 노트가 자리 틱 앞(앞 클립 끝 뒤 / 앞) | 못 깨뜨림: 앞 클립 끝을 바닥으로 받아들이고, 그보다 앞이면 `NoteBeforeClipStart` | core `clip_for_notes_note_before_at_uses_prev_end_as_floor` |
| 5 | core 속성 400건: 마디 중간 끝·시작을 섞은 클립(16분음표 단위), 박자표 변화, 틈의 자리 틱, 틈 안에 드는 노트 → 늘 `Ok`, 새 클립이 [앞 끝, 다음 시작) 안에서 자리 틱을 품고 적용 성공 | 못 깨뜨림 | core `clip_for_notes_accepts_notes_that_fit_the_gap` |
| 6 | 앱: 앞 클립 [0, 1.5마디), 다음 클립 [2마디 + 3박, …), 1마디 + 3박에 붙이기 → `cmd-z` → `cmd-shift-z` → `r` | 못 깨뜨림: 새 클립 [1.5마디, 2마디), 선택·undo·redo 선택, `r` 사본이 새 클립 안 | `gap_paste_between_mid_bar_clips_selects_and_repeats` |
| 7 | 앱: 앞 클립이 마디 중간에서 끝나는 틱에 그대로 `cmd-v` | 못 깨뜨림: 그 틱에서 시작하는 새 클립 | `paste_at_mid_bar_clip_end_tick_starts_new_clip_there` |
| 8 | 앱(vim): 클립 A 마지막 칸(6720)에서 `p` → `r` | 동작 기록: A를 늘리지 않고 A 뒤에 맞닿은 새 클립 [2마디, 3마디). 이어진 `r`은 「선택이 현재 클립에 없다」로 거부(제안 1) | `vim_p_on_last_step_of_clip_makes_separate_clip`(통과) |
| 9 | 앱: A에서 `x` → 커서를 B로 → `cmd-z` → `x` | 동작 기록: 선택은 A의 C4, 다음 `x`는 거부(제안 2) | `undo_selection_away_from_cursor_blocks_next_delete`(통과) |
| 10 | 앱: `cmd-x` 뒤 상태줄 | 동작 기록: 「노트 2개를 복사했다」(제안 3) | `cut_status_line_says_copied`(통과) |
| 11 | 앱: 텍스트 왕복이 어려워 보이는 노트(부분 겹침, 같은 음 겹침, 같은 시작·다른 길이, 따옴표 가사, 화음 가사, 1틱 노트)를 복사. 트랙 이름(앞뒤 공백, 따옴표, 줄바꿈, 탭)도 따로 시도(시험에는 남기지 않음) | 못 깨뜨림: 모두 텍스트와 SMF가 올라간다. 빠진 형식 알림 경로는 여전히 일으키지 못했다 | `copy_awkward_notes_keeps_text_or_reports_missing` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | `clip_for_notes`의 새 시작·끝은 틈 안에 머문다(속성 400건 + 1회차 속성 300건). `insert_notes`는 겹치면 거부 |
| 노트는 클립 안 | 못 깨뜨림 | 새 끝은 노트 끝 이상(`s >= end`)에서만 자른다 |
| 전부 아니면 전무 | 못 깨뜨림 | 거부 시험에서 노트·`undo_len` 그대로 |
| 선택은 그 클립의 실제 값 | 못 깨뜨림 | 배치 뷰 undo는 선택을 바꾸지 않는다(1회차 지적 3 수정, `review_s3_t3a::undo_in_arrangement_view_leaves_no_note_selection` 통과) |
| 채널 1–16, 틱 ≥ 0 | 해당 없음 | |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8 `r` 「선택을 바로 뒤에 반복 복제」, 7.5 `Cmd+D` 「선택의 길이만큼 뒤에」 | 충족(간격) | `crates/keyscore-app/src/pianoroll/edit.rs` `duplicate_offset`, 1회차 시험 둘 통과. 다만 클립 끝 근처는 지적 1 |
| 7.5 클립보드 규칙 2 「노트는 클립 경계를 넘으면 클립을 늘리고」 | 부분 충족 | 다음 클립이 마디 중간에서 시작하면 빈 자리가 있어도 늘리지 않고 거부. `crates/keyscore-core/src/transform.rs:214`, 시험 `paste_near_clip_end_fits_before_mid_bar_next_clip`, `duplicate_near_clip_end_fits_before_mid_bar_next_clip` |
| 5.10 클립 밖 입력은 새 클립(계획의 넓힌 읽기, 리뷰 1 지적 2 수정) | 충족 | `transform.rs:262-292`, core 시험 3·4·5 |
| 7.5 클립보드 규칙 1·3, KM-15 | 충족 | 1회차 판정 그대로, `paste_contents` 수정 뒤 `note_edit`·`review_s3_t3a` 통과 |
| 5.8.1 vim `p` `P` `X` `D` `Y` `y y` `y y p` | 충족 | 1회차 판정 그대로. `p`의 클립 끝 동작은 제안 1 |
| 5.7 되돌리기 묶음 | 충족 | `review_s3_t3a::vim_yyp_repeated_is_one_undo_unit` 등 통과 |
| 이월(S3-T2 리뷰 1 제안 8) undo·redo 뒤 선택 | 충족 | `window.rs:458`, 시험 6. 커서와 다른 클립 경우는 제안 2 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 모두 메인 스레드 처리기와 순수 함수다.
- 제품 코드의 `unwrap`/`expect`: 1회차 뒤 차분에 없음. `unsafe` 없음.
- 에러가 삼켜지는 곳: 새로 생긴 곳 없음. `from_text`의 다시 읽기 실패는 고정 문구로 바뀌었다(의도된 대응). 빠진 형식 알림 경로(`note_edit.rs:197-212`)는 시험으로 일으키지 못했다(시도 11).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 있던 클립을 늘리는 붙여넣기·복제·반복(`insert_notes`)은 늘 마디 끝까지 늘리려 해서, 다음 클립이 마디 중간에서 시작하면 노트가 빈 자리에 들어가는데도 「겹친다」로 거부한다. 클립 A [0, 1마디), B가 5000에서 시작할 때 A 안 3000의 1박 붙여넣기와 2880 노트의 `cmd-d`(사본 3840–4800)가 거부된다. 같은 노트를 한 틱 뒤인 A 끝(3840)에 붙이면 리뷰 1 지적 2 수정의 새 규칙(「다음 클립이 노트 끝과 마디 끝 사이에서 시작하면 끝은 그 시작」)으로 받아들여진다. 같은 빈 자리를 자리 틱 한 틱 차이로 다르게 판단한다. 리뷰 1 지적 2의 근거(SMF 가져오기·줄이기가 만드는 마디 중간 경계)는 이 경우에도 그대로이고, REQ 7.5 클립보드 규칙 2 「노트는 클립 경계를 넘으면 클립을 늘리고」를 못 지킨다. 연장 끝도 `clip_for_notes`처럼 「노트 끝 이상에서 시작하는 다음 클립의 시작」으로 자르거나, 거부를 유지한다면 그 이유를 계획 4에 적고 결정 요청으로 넘긴다(REQ 5.10 「마디 단위로 늘어난다」와 7.5 「늘리고」가 부딪히는 자리다). `insert_notes`만 고치면 S3-T2 `map_notes`(`Alt` 밀기)의 연장과 규칙이 갈리므로 그 차이도 보고서에 적는다. 시험: core `insert_notes_fits_before_mid_bar_next_clip`(실패), app `paste_near_clip_end_fits_before_mid_bar_next_clip`(실패), `duplicate_near_clip_end_fits_before_mid_bar_next_clip`(실패), 대조 `paste_at_clip_end_before_mid_bar_next_clip_makes_new_clip`(통과) | `crates/keyscore-core/src/transform.rs:214-227` |

### 제안

| # | 내용 | 근거 시험(결과) | 파일:줄 |
| --- | --- | --- | --- |
| 1 | vim `p`를 클립 마지막 칸에서 누르면 자리 틱(커서 + 한 스텝)이 클립 끝과 같아 「틈」으로 판정되어, 클립을 늘리지 않고 맞닿은 새 클립을 만든다. 커서는 원래 클립에 남아 바로 이은 `r`(`y y p`)이 「선택이 현재 클립에 없다」로 거부된다. vim 사용자에게 `p`는 「커서 뒤에 이어 붙이기」이므로, 자리 틱이 클립 끝과 정확히 같으면 그 클립에 넣어 늘리는 쪽(REQ 7.5 「클립 경계를 넘으면 클립을 늘리고」)을 고려할 만하다. 계획 4 문장(「자리 틱을 품은 클립」)대로라 지적은 아니다. `Cmd+V`로 클립 끝 틱에 붙일 때도 같다 | `vim_p_on_last_step_of_clip_makes_separate_clip`(통과, 동작 기록) | `crates/keyscore-app/src/pianoroll/edit.rs:67`, `crates/keyscore-app/src/note_edit.rs:394` |
| 2 | undo가 커서가 없는 클립의 노트를 되살리면 선택이 그 클립으로 가고 커서는 그대로라, 이어지는 `x` `r` 등이 「선택이 현재 클립에 없다」로 막힌다. 선택을 바꿀 때 커서를 그 클립(예: 되살린 첫 노트)으로 옮기거나, 커서 클립이 아니면 선택을 바꾸지 않는 규칙을 계획 6에 적을 만하다 | `undo_selection_away_from_cursor_blocks_next_delete`(통과, 동작 기록) | `crates/keyscore-app/src/window.rs:458` |
| 3 | 잘라내기의 상태줄이 「노트 n개를 복사했다」로 끝난다(노트는 지워졌다). 「잘라냈다」가 맞다 | `cut_status_line_says_copied`(통과, 동작 기록) | `crates/keyscore-app/src/note_edit.rs:332` |
| 4 | 빠진 형식 알림 문구가 「다른 앱용 텍스트은」「SMF은」으로 조사가 틀린다(「텍스트는」「SMF는」). 형식 이름 뒤에 고정 조사를 붙이지 않는 문장(예: 「다른 앱용 형식을 만들지 못했다: 텍스트·SMF」)이 낫다. 이 경로는 시도 11로도 일으키지 못해 시험은 없다 | 코드 읽기, `copy_awkward_notes_keeps_text_or_reports_missing`(통과, 경로 미발생) | `crates/keyscore-app/src/note_edit.rs:207` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

1회차 지적 1–3의 수정은 맞다. 복제 간격은 폭 그대로이고 결정 요청이 있으며, 틈 새 클립의 시작·끝 규칙은 경계·박자표·속성 시험에서 깨지지 않았고, 배치 뷰 undo는 선택을 만들지 않는다. 새 회귀는 없다. 다만 지적 2의 수정이 틈 쪽에만 들어가, 있던 클립을 늘리는 같은 모양(지적 1)이 결함으로 남았다. 고치거나 거부 이유를 계획·결정 요청으로 남긴 뒤 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(app 실패 2·통과 7, core 실패 1·통과 6)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(1회차 뒤 차분은 지적·제안 대응과 `plan/stages/18-S8-release.md` 이월 줄뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`plan_duplicate_offset_is_span_rounded_up_to_step`은 지적 1 대응으로 `plan_duplicate_offset_is_exact_span`으로 이름·기대값이 바뀌었다. 보고서에 적혀 있다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(1회차 뒤 새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 리뷰 산출물

- 새 `.proptest-regressions` 파일: 없음(속성 시험은 통과).
- 리뷰 시험 두 파일은 `rustfmt`·`clippy -D warnings`(pedantic 포함)를 통과한다. 실패 3개는 지적 1의 근거로 남긴다.
