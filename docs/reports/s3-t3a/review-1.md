# 적대적 리뷰: S3-T3a 삭제·클립보드·반복 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 컨텍스트를 물려받지 않은 새 에이전트)
- 대상 커밋: `d63465d4ab9b3c5c0dab6a13bd8b83214e212035`(브랜치 `s3/t3a-delete-clipboard-repeat`, `git diff main...HEAD`)
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t3a.rs`(20개, 실패 4), `crates/keyscore-core/tests/review_s3_t3a.rs`(3개, 모두 통과)
- 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t3a`, `-p keyscore-core --test review_s3_t3a`. 구현자 시험 `note_edit`(10)·`clipboard_notes`(7)·`transform_insert`(5)도 다시 돌려 통과를 봤다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일을 열기 전에 `note_edit.rs`, `pianoroll/edit.rs`, `clipboard.rs`, `transform.rs` 차분, `window.rs`의 undo·redo, 키맵을 읽고 적었다.

1. 틈에 붙일 때 새 클립 시작을 「자리 틱이 든 마디의 시작」으로 잡는데, 앞 클립이 마디 중간에서 끝나면 그 시작이 앞 클립과 겹친다. 틈인데 거부되는가.
2. 복제·반복 간격을 한 스텝(960)의 배수로 올린다. 한 스텝보다 짧은 노트(16분음표)를 반복하면 「바로 뒤」가 아니라 틈이 생긴다(REQ 7.5 「선택의 길이만큼 뒤에」).
3. `select_appeared`는 뷰를 보지 않는다. 배치 뷰에서 undo하면 화면에 없는 노트 선택이 생기는가(S3-T2: 뷰 전환이 선택을 비운다).
4. vim `y y p`는 시퀀스다. 접두 키 `y`가 Action 없이 관찰되면 묶음을 끊어 `y y p` 연속이 여러 단위가 되는가.
5. 입력란 포커스에서 맨 글자 키(`x`, `r`)가 이번에 처리기가 생겨 「처리됨」으로 삼켜지고 입력란에 글자가 들어가지 않는가.
6. 메뉴로 온 붙여넣기가 입력란에 실제로 들어가는가(`input::Paste`를 디스패치만 하고 확인하지 않음).
7. 같은 값의 노트 둘: 삭제·undo·선택·복사가 둘을 모두 다루는가.
8. 묶인 반복이 클립을 늘린 경우 undo 한 번에 클립 길이까지 돌아오는가.
9. 틈에 붙여 새 클립을 만든 뒤 undo·redo: 새 클립 id가 바뀌어도 선택이 새 클립의 노트를 가리키는가.
10. 출처 판정: 내부 복사 뒤 다른 앱이 텍스트를 올리면 그 텍스트가 이기는가. 앱을 다시 띄운 뒤(내부 없음) 우리가 쓴 SMF로 같은 노트가 돌아오는가.
11. 선택이 다른 트랙이면 문서를 바꾸는 동작이 거부되는가.
12. 큰 입력: undo마다 현재 트랙 전체 복제, 노트마다 `InsertNote`/`RemoveNote`의 O(n) 끼우기·찾기, 큰 외부 텍스트를 메인 스레드에서 두 번 파싱.
13. core `clip_for_notes`: 박자표 변화 뒤 마디, 속성(적용 성공·겹침 없음·노트가 클립 안).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 앞 클립이 마디 중간에서 끝난 틈 | 없음 | `cut_copy_paste_in_piano_roll`의 틈은 마디 경계 클립뿐 |
| 2 짧은 노트의 복제 간격 | 부분 | `plan_duplicate_offset_is_span_rounded_up_to_step`가 「한 스텝보다 짧은 노트 → 한 스텝」을 기대값으로 고정한다(REQ와 대조하지 않음) |
| 3 배치 뷰 undo 뒤 선택 | 없음 | |
| 4 vim `y y p` 연속 묶음 | 없음 | `r r r`만 있다 |
| 5 입력란의 맨 글자 키 | 없음 | `text_field_keeps_clipboard_keys`는 `cmd-c`/`cmd-v`만 |
| 6 메뉴 붙여넣기가 입력란에 들어감 | 부분 | 클립보드·문서가 그대로인 것만 본다 |
| 7 같은 값 둘 | 부분 | 순수 `targets_take_every_copy_and_delete_removes_them`만. 계획 시험 10의 「같은 값 둘 포함」이 UI 시험에 없다 |
| 8 묶인 반복 + 연장의 undo | 없음 | `r r r`은 연장이 없는 자리 |
| 9 틈 붙여넣기 undo·redo 선택 | 없음 | 있는 클립에 붙인 경우만 |
| 10 출처 판정(내부 뒤 외부, 재시작) | 부분 | 외부만 있는 경우와 내부만 있는 경우 |
| 11 다른 트랙 선택 | 없음 | 같은 트랙 다른 클립만 |
| 12 큰 입력 비용 | 없음 | |
| 13 core 박자표·속성 | 부분 | `insert_notes`는 박자표·속성이 있고 `clip_for_notes`는 없다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 앞 클립 [0, 1마디+2박), 같은 마디의 틈(1마디+3박)에 `cmd-v` | 깨짐: 「클립 구간 [3840, 7680)이 같은 트랙의 다른 클립과 겹친다」로 거부 | `paste_in_gap_in_same_bar_as_prev_clip_end_makes_new_clip`(실패) |
| 1' | 같은 앞 클립, 다음 마디의 틈(2마디+1박) | 못 깨뜨림(대조) | `paste_in_gap_after_mid_bar_clip_end_makes_new_clip`(통과) |
| 2 | 16분음표(240) 하나에 `cmd-d` | 깨짐: 사본이 960에 온다(720틱 틈) | `duplicate_sixteenth_lands_right_after_selection`(실패) |
| 2' | 16분음표 하나에 `r r r` | 깨짐: 0, 960, 1920, 2880(이어지지 않음) | `repeat_sixteenth_makes_contiguous_run`(실패) |
| 3 | 피아노롤에서 `x` → `v`(배치 뷰) → `cmd-z` | 깨짐: 배치 뷰에 노트 선택 `[(960, 960, 62)]`, `Selection` 컨텍스트가 켜진다 | `undo_in_arrangement_view_leaves_no_note_selection`(실패) |
| 4 | vim `y y p` 셋 | 못 깨뜨림: undo 한 단위 | `vim_yyp_repeated_is_one_undo_unit`, 대조 `default_r_repeated_is_one_undo_unit` |
| 5 | 피아노롤에서 트랙 이름 입력란에 `x r` 키 입력 | 못 깨뜨림: 입력란이 `xr`, 문서 그대로 | `edit_letter_keys_in_name_field_are_typed`, 대조 `preexisting_letter_key_in_name_field_is_typed` |
| 6 | 템포 입력란에서 메뉴 `edit::Paste`, 키 `cmd-v` | 못 깨뜨림: 입력란이 `90` | `menu_paste_in_tempo_field_reaches_input`, `key_paste_in_tempo_field_reaches_input` |
| 7 | 같은 값 둘을 선택해 `x` → `cmd-z` → `cmd-c` | 못 깨뜨림: 둘 다 지워지고 둘 다 돌아오며 선택은 그 값, 내부 내용 2개 | `delete_same_value_twins_removes_both_and_undo_selects` |
| 8 | `r` 넷으로 클립을 3마디로 늘린 뒤 `cmd-z` | 못 깨뜨림: 노트·길이 모두 원래대로 | `coalesced_repeat_with_extension_undoes_length` |
| 9 | 틈 붙여넣기 → `cmd-z` → `cmd-shift-z` | 못 깨뜨림: 비었다가 새 클립의 노트로 | `gap_paste_undo_redo_selects_new_clip_notes` |
| 10 | 내부 복사 뒤 외부 텍스트 `o5 l4 c`, 재시작 흉내(내부 없이 같은 SMF·텍스트를 다시 씀) | 못 깨뜨림 | `external_text_after_internal_copy_wins`, `paste_after_restart_reads_own_smf` |
| 11 | 선택이 다른 트랙일 때 `x` `r` `cmd-d` `cmd-x` | 못 깨뜨림: 모두 거부 | `edit_with_selection_on_other_track_is_rejected` |
| 12 | 노트 10만 트랙 undo·redo, 노트 2만 클립 전체 복사·붙여넣기·undo·삭제·복제, 1 MB 외부 글 붙여넣기(디버그 빌드) | 못 깨뜨림(시간은 제안 1·3의 근거): undo·redo 각 38 ms, 2만 붙여넣기 305 ms, 그 undo 2.59 s, 삭제 288 ms, 복제 74 ms, 1 MB 글 57 ms | `undo_on_large_track_cost`, `large_selection_edit_cost`, `paste_large_plain_text_cost` |
| 13 | core: 3/4 변화 뒤 틈 클립, 1박에서 시작한 클립 연장, `clip_for_notes` 속성 300건 | 못 깨뜨림 | core `clip_for_notes_after_meter_change_uses_that_bar`, `insert_notes_into_unaligned_clip_extends_to_absolute_bar_end`, `clip_for_notes_ok_applies_without_overlap` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | `insert_notes`·`clip_for_notes`가 모든 클립과 겹침을 검사하고, 적용은 core가 다시 검사한다(core 속성 시험 2개) |
| 노트는 클립 안 | 못 깨뜨림 | 연장이 먼저 오고 `InsertNote`는 `insert_note` 검사를 거친다. 꼬리 노트 사본은 클립을 늘린다 |
| 전부 아니면 전무 | 못 깨뜨림 | 계산이 실패하면 적용하지 않고, 적용 실패는 core가 되돌린다. 거부 시험에서 `undo_len`·클립이 그대로 |
| 선택은 그 클립의 실제 값 | 깨짐(약함) | 배치 뷰 undo가 화면에 없는 노트 선택을 만든다(지적 3). 값 자체는 실제 노트다 |
| 채널 1–16, 틱 ≥ 0 | 해당 없음 | 이 Task는 채널을 만들지 않고, 틱 덧셈은 `checked_add` |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8 `x` `Delete` `Backspace`(선택 없으면 커서 노트), `Shift+x` = `Cmd+X` | 충족 | `note_edit.rs` `edit_delete`·`edit_cut`, `assets/keymaps/default.json:15-18` |
| 5.8 `r` 「선택을 바로 뒤에 반복 복제. 누를 때마다 하나 더」 | 미충족 | 한 스텝보다 짧은 선택은 바로 뒤가 아니다. `pianoroll/edit.rs:95` `duplicate_offset`, 시험 `repeat_sixteenth_makes_contiguous_run` |
| 7.5 `Cmd+D` 「복제(바로 뒤에 붙여넣기) … 선택의 길이만큼 뒤에」 | 미충족 | 같은 곳. 시험 `duplicate_sixteenth_lands_right_after_selection` |
| 7.5 `Cmd+X` `Cmd+C` `Cmd+V` | 충족 | `note_edit.rs` 처리기, `assets/keymaps/standard.json:8-11` |
| 7.5 클립보드 규칙 1(내부 구조 + SMF Type 0 + MML 텍스트) | 충족 | `clipboard.rs` `store`·`write_system`, 시험 `copy_puts_smf_and_text_on_system_clipboard`, `paste_in_other_window_uses_internal_structure` |
| 7.5 클립보드 규칙 2(커서 위치, 클립 경계를 넘으면 클립을 늘림) | 충족 | `pianoroll/edit.rs:58` `plan_paste`, core `insert_notes` |
| 7.5 클립보드 규칙 3(시스템 클립보드의 SMF·MML을 `Cmd+V`로, IO-2·IO-3과 같은 변환) | 충족 | `clipboard.rs:242` `paste_contents`, 시험 `paste_reads_external_smf_then_text`, `external_text_after_internal_copy_wins` |
| KM-15 시스템 클립보드 왕복 | 충족 | 위 둘과 `paste_after_restart_reads_own_smf` |
| 5.8.1 vim `p` `P` `X` `D` `Y` `y y`, `y y p` | 충족 | `assets/keymaps/vim.json:23-30`, 시험 `vim_p_shift_p_shift_x_shift_d_shift_y`, `vim_yyp_repeated_is_one_undo_unit` |
| 5.7 되돌리기 묶음(같은 조작의 연속) | 충족 | `selection.rs` `is_coalescing_action`, `note_edit.rs` `COALESCE_*` |
| 5.10 클립 밖 입력은 마디 단위 새 클립(계획의 넓힌 읽기) | 부분 충족 | 앞 클립이 마디 중간에서 끝나는 마디의 틈은 거부. `transform.rs:258`·`:276`, 시험 `paste_in_gap_in_same_bar_as_prev_clip_end_makes_new_clip` |
| KM-14 태도(입력란 포커스에서는 입력란의 동작) | 충족 | `note_edit.rs` `edit_allowed`, 시험 5·6 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task의 코드는 모두 메인 스레드 처리기와 순수 함수다.
- 제품 코드의 `unwrap`/`expect`: 없음(`note_edit.rs`, `clipboard.rs`, `pianoroll/edit.rs`, `transform.rs` 차분). `unsafe`도 없다.
- 에러가 삼켜지는 곳:
  - `ClipNotes::to_text`·`to_smf`가 `None`이면(가사·피치 오프셋으로 `Loss::Mismatch` 등) 시스템 클립보드에 그 형식이 조용히 빠진다. 상태줄은 「노트 n개를 복사했다」만 말한다(제안 4).
  - `ClipNotes::from_text`는 `@track` 블록으로 다시 읽은 실패를 버리고 첫 실패를 돌려준다. 맨 글을 붙이면 「@track 블록 밖에는 헤더(@tempo·@time·@key)만 쓸 수 있다」가 나온다(제안 3, `paste_large_plain_text_cost`의 출력).
  - `ClipNotes::new`의 `filter_map(… .ok())`은 시작을 줄이기만 해 실패하지 않으므로 문제없다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 복제·반복 간격을 한 스텝(지금 960)의 배수로 올려, 한 스텝보다 짧은 선택은 「바로 뒤」에 오지 않는다. 16분음표 하나의 `cmd-d` 사본이 240이 아니라 960에 오고, `r r r`은 16분음표 연타가 아니라 4분 간격 노트가 된다. REQ 7.5 「선택의 길이만큼 뒤에」, 5.8 「선택을 바로 뒤에」와 어긋난다. 계획 「리스크」에 적었지만 CLAUDE.md 규칙(REQ에 없는 결정은 `docs/decisions/`에 결정 요청)대로 결정 요청이 없다. 스타카토(900틱/960칸)를 칸에 맞추려는 의도라면 「폭을 한 스텝 배수로 올리기」가 아니라 예를 들어 「폭이 스텝보다 짧으면 폭 그대로」 같은 규칙과 결정 요청이 필요하다. 시험: `duplicate_sixteenth_lands_right_after_selection`(실패), `repeat_sixteenth_makes_contiguous_run`(실패) | `crates/keyscore-app/src/pianoroll/edit.rs:95` |
| 2 | 수정 | 틈 붙여넣기의 새 클립 시작을 무조건 「자리 틱이 든 마디의 시작」으로 잡아, 앞 클립이 그 마디 안에서 끝나면 틈인데도 「다른 클립과 겹친다」로 거부한다. 계획 4는 「다음 클립과 겹치면 거부」만 적었다. SMF 가져오기·클립 줄이기는 마디에 맞지 않는 끝을 흔히 만든다. 시작을 `max(마디 시작, 앞 클립 끝)`으로 하거나 그 마디의 앞 클립을 늘리는 등 규칙을 계획에 적고 시험해야 한다. 시험: `paste_in_gap_in_same_bar_as_prev_clip_end_makes_new_clip`(실패) | `crates/keyscore-core/src/transform.rs:258`, `:276` |
| 3 | 수정 | 배치 뷰에서 undo·redo하면 `select_appeared`가 피아노롤 노트 선택을 만든다. S3-T2는 뷰 전환 때 FSM이 노트 선택을 비우고(배치 뷰의 선택은 S4의 클립 선택), 계획 6은 「화면에 없는」 변화를 선택하지 않는다는 태도다. 배치 뷰에 `Normal > Arrangement > Selection` 컨텍스트가 켜져 `escape`가 `select::Clear`로 가고, S4가 배치 뷰 `Selection` 키를 달면 보이지 않는 노트 선택에 걸린다. 피아노롤 뷰일 때만 선택을 바꿔야 한다. 시험: `undo_in_arrangement_view_leaves_no_note_selection`(실패) | `crates/keyscore-app/src/window.rs:455` |

### 제안

| # | 내용 | 근거 시험(결과) | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 큰 붙여넣기의 undo가 노트 수의 제곱에 비례한다. 노트마다 `Edit::InsertNote`라 undo는 `RemoveNote`마다 선형 찾기(`position(|n| n == from)`)와 벡터 당기기를 한다. 노트 2만 개를 사이사이에 붙인 뒤 undo가 디버그 빌드 2.59 s(붙여넣기 자체는 305 ms). 한 번에 넣고 빼는 묶음 편집이나 정렬 위치 이진 찾기를 S3 안에서 고려할 만하다(범위 밖이면 이월 줄로) | `large_selection_edit_cost`(통과, 측정) | `crates/keyscore-core/src/clip.rs:175`, `crates/keyscore-app/src/pianoroll/edit.rs:107` |
| 2 | `paste_contents`가 `changeCount`를 보기 전에 `read_system`으로 SMF 바이트와 문자열을 모두 꺼낸다. 내부 구조를 쓸 때도 pasteboard 데이터를 복사하고, 다른 앱이 지연 제공(promise)한 큰 데이터면 그 앱에 렌더링을 시킨다. `changeCount`만 먼저 읽고 필요할 때 형식별로 읽는 편이 낫다 | 코드 읽기(시험 없음) | `crates/keyscore-app/src/clipboard.rs:216`, `:242` |
| 3 | 맨 글을 붙였을 때의 알림이 「@track 블록 밖에는 헤더만 쓸 수 있다」라 사용자가 이해하기 어렵다. 다시 읽기도 실패하면 「계이름 텍스트가 아니다」처럼 알리고 첫 오류는 덧붙이는 정도로 | `paste_large_plain_text_cost`(통과, 출력에 알림) | `crates/keyscore-app/src/clipboard.rs:139` |
| 4 | 텍스트·SMF를 만들지 못해 시스템 클립보드에 빠진 형식이 있으면 상태줄에 알린다(가사·피치 오프셋 노트를 복사해 다른 앱에 붙였는데 아무것도 안 들어가는 이유를 알 수 없다) | 코드 읽기 | `crates/keyscore-app/src/note_edit.rs:172` |
| 5 | 계획 시험 10의 「같은 값 둘 포함」과, 입력란에서 맨 글자 키·메뉴 붙여넣기가 실제로 입력란에 들어가는 확인을 구현자 시험으로 옮긴다(지금은 리뷰 시험에만 있다) | `delete_same_value_twins_removes_both_and_undo_selects`, `edit_letter_keys_in_name_field_are_typed`, `menu_paste_in_tempo_field_reaches_input`(모두 통과) | `crates/keyscore-app/tests/note_edit.rs` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(REQ 7.5·5.8 미충족, 결정 요청 없음)은 차단이다. 결정 요청을 쓰고 그 결정대로 간격 규칙을 고치거나 REQ 문장대로(선택의 폭 그대로) 고친 뒤, 지적 2·3과 함께 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(실패 4, 통과 16 + core 3)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/13-S3-step-and-pianoroll.md`의 나눔 기록, 두 기존 시험의 기대값 수정은 보고서에 적힌 이월 동작 변경이다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`objc2`·`objc2-foundation` MIT, `objc2-app-kit` Zlib OR Apache-2.0 OR MIT, gpui-kit 트리에 이미 있는 판)
- [x] 스타일 지적을 적지 않았다

## 부록: 리뷰 산출물

- 새 `.proptest-regressions` 파일: 없음(기존 7개 그대로).
- 리뷰 시험 두 파일은 `rustfmt`·`clippy -D warnings`(pedantic 포함)를 통과한다. 실패 4개는 지적 1–3의 근거로 남긴다.
