# Task 보고: S3-T3a 삭제·클립보드·반복

- PR: 로컬 브랜치 `s3/t3a-delete-clipboard-repeat`
- 계획 파일: docs/reports/s3-t3a/plan.md
- 계획 대비 달라진 점(계획 파일을 고쳤다)
  - 다른 앱에서 복사한 MML 조각에는 `@track` 머리가 없어 그대로는 읽히지 않는다. 그대로 읽어 실패하고 `@track`이 없으면 `@track Clip ch1` 블록 하나로 다시 읽는다.
  - 복사 API는 `clipboard::store(cx, notes, text, midi)`다. 텍스트·SMF를 문서를 빌린 채 만들어 프로젝트를 복제하지 않기 위해서다. `paste_contents`, `duplicate_offset`를 공개했다.
  - vim `X`·`D`는 선택과 무관한 동작이라 선택을 비우지 않는다(지운 노트는 문서 관찰자가 뺀다).
  - 되돌리기 묶음 판정 `selection::is_note_op_action`을 `is_coalescing_action`으로 바꾸고 `edit::Duplicate`·`edit::Repeat`를 더했다.

## 구현 요약

- `crates/keyscore-core/src/transform.rs`: `insert_notes`(끝을 넘으면 `bar_aligned_length`로 `ExtendClip` 먼저, 다음 클립과 겹치면 `ClipOverlap`, 그다음 `InsertNote`들), `clip_for_notes`(`at`이 든 마디 시작부터 마지막 노트 끝을 품는 마디 끝까지 `InsertClip`, 겹침·시작 앞·빈 목록 거부).
- `crates/keyscore-app/src/clipboard.rs`: `ClipNotes`(시작을 가장 이른 노트 기준으로, 노트 값은 그대로, `span`), `to_smf`/`to_text`(원래 트랙의 이름·채널·프로그램·뱅크, 복사 시작 자리의 템포·박자표·조로 만든 임시 프로젝트. 텍스트는 `Loss::Mismatch`면 쓰지 않음), `from_smf`/`from_text`(모든 트랙·클립의 노트를 절대 틱으로 모음). 앱 전역 `ClipboardState`(내부 내용, 쓴 `changeCount`, 시험용 pasteboard 이름). `NSPasteboard`는 `objc2-app-kit`의 안전한 메서드만(`clearContents` → `setString_forType`(`public.utf8-plain-text`) → `setData_forType`(`public.midi-audio`)). `paste_contents`: `changeCount`가 같으면 내부, 다르면 SMF, 없으면 텍스트.
- `crates/keyscore-app/src/pianoroll/edit.rs`(순수): `targets`(선택 값마다 클립의 같은 노트 전부), `plan_delete`, `plan_paste`(자리 틱을 품은 클립 또는 틈의 새 클립), `duplicate_offset`(폭을 한 스텝 배수로 올림, 최소 한 스텝), `plan_duplicate`, `appeared`(트랙 앞뒤 다중집합 차가 한 클립에만 있으면 그 노트).
- `crates/keyscore-app/src/note_edit.rs`: 처리기 11개(`edit::Delete/DeleteBefore/DeleteToBarEnd/Copy/Cut/CopyBar/Paste/PasteBefore/PasteAfter/Duplicate/Repeat`). 공통 `edit_allowed`(입력란 포커스면 `input::Copy/Cut/Paste`로 보내고 끝, 배치 뷰면 알림), `edit_targets`(선택이 현재 클립 밖이면 문서를 바꾸는 동작만 거부), `apply_placed`(적용 뒤 넣은 노트를 선택으로, 새 클립은 시작 틱으로 id를 찾음). 복제·반복은 `apply_coalesced`(키 `0x5337_0100`·`0x5337_0101`).
- `crates/keyscore-app/src/window.rs`: `edit::Undo/Redo` 뒤 `select_appeared`(현재 트랙 스냅샷과 비교). 키 입력 관찰은 `is_coalescing_action`.
- 의존성: `objc2 0.6.5`, `objc2-app-kit 0.3.2`(feature `NSPasteboard`), `objc2-foundation 0.3.2`. 셋 다 gpui-kit이 이미 같은 판을 쓰고 있어 `Cargo.lock`에는 keyscore-app의 의존 줄만 더해졌다.

## 테스트 결과

~~~
cargo test -p keyscore-core --test transform_insert: 5 passed(proptest 1: Ok면 적용 성공·노트가 클립 안)
cargo test -p keyscore-app --test clipboard_notes: 7 passed
cargo test -p keyscore-app --test note_edit: 10 passed
cargo test --workspace --no-fail-fast: 2008 passed, 2 failed, 2 ignored(1시간판 측정용)
  실패 2는 이월 줄이 일부러 바꾼 동작(undo 뒤 선택이 빈다)을 단언하던 기존 시험이다. 기대값을 새 규칙(되살아난
  노트가 선택)으로 고쳤다: pianoroll_select::selection_follows_document_changes,
  review_s3_t2::brk_undo_via_menu_then_same_op_starts_new_unit. 고친 뒤 두 바이너리 재실행 14·13 passed
~~~

실패 확인: core 시험은 `todo!()` 뼈대로, 앱 순수 시험은 함수 본문을 `todo!()`로 바꾼 상태로, UI 시험은 처리기 없이 먼저 돌려 실패를 봤다(UI 9개 실패, `text_field_keeps_clipboard_keys`만 처리기 없이도 통과. 이 시험은 회귀를 막는 것이다).

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `transform_insert::insert_notes_extends_clip_by_bars_and_rejects_overlap`, `insert_notes_extends_to_bar_after_meter_change`, `insert_notes_rejects_unknown_track_and_clip` | 통과 |
| 2 | `transform_insert::clip_for_notes_spans_bars_and_rejects_overlap` | 통과 |
| 3 | `transform_insert::insert_notes_preserves_invariants` | 통과 |
| 4 | `clipboard_notes::clip_notes_normalizes_to_earliest_start_and_span` | 통과 |
| 5 | `clipboard_notes::clip_notes_smf_and_text_round_trip` | 통과 |
| 6 | `clipboard_notes::from_text_rejects_non_notation_and_empty` | 통과 |
| 7 | `clipboard_notes::plan_paste_into_clip_gap_and_extension` | 통과 |
| 8 | `clipboard_notes::plan_duplicate_offset_is_span_rounded_up_to_step` | 통과 |
| 9 | `clipboard_notes::appeared_picks_new_notes_in_one_clip`, `targets_take_every_copy_and_delete_removes_them` | 통과 |
| 10 | `note_edit::delete_x_delete_backspace_remove_targets` | 통과 |
| 11 | `note_edit::cut_copy_paste_in_piano_roll` | 통과 |
| 12 | `note_edit::copy_puts_smf_and_text_on_system_clipboard` | 통과(게이트) |
| 13 | `note_edit::paste_in_other_window_uses_internal_structure` | 통과(게이트) |
| 14 | `note_edit::paste_reads_external_smf_then_text` | 통과 |
| 15 | `note_edit::duplicate_and_repeat_append_copies_and_coalesce` | 통과 |
| 16 | `note_edit::vim_p_shift_p_shift_x_shift_d_shift_y` | 통과 |
| 17 | `note_edit::undo_redo_restore_selection_to_changed_notes` | 통과 |
| 18 | `note_edit::arrangement_view_edit_keys_do_not_change_document`, `text_field_keeps_clipboard_keys` | 통과 |
| 19 | 기존 게이트(Action 수·명령 표) | 전체 실행에 포함 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(위 2건 기대값 수정 뒤 해당 바이너리 재실행 통과)
- [x] deny(advisories·bans·licenses·sources ok)
- [x] `scripts/check-deps.sh` ok, `scripts/check-deps.test.sh` 16 passed, `scripts/bench-check.test.sh` 8 passed

## 알려진 한계

- 배치 뷰의 클립 단위 삭제·복사·붙여넣기·복제·반복은 S4다. 이 Task의 처리기는 배치 뷰에서 알리기만 한다.
- 시스템 클립보드에서 큰 SMF를 읽는 것은 메인 스레드다(파일 가져오기와 달리 백그라운드가 아니다).
- 텍스트·SMF로 나가는 내용은 피치 오프셋·7비트 밖 벨로시티 등을 잃는다(내보내기와 같은 손실). 앱 안에서는 내부 구조를 쓰므로 잃지 않는다. 시스템 클립보드를 다른 앱이 바꾼 뒤에는 그 앱의 내용을 읽는다.
- vim 프리셋의 `y y`는 `y y p` 바인딩 때문에 시퀀스 시간(1 s) 뒤에 실행된다(S2-T2의 바인딩, 이 Task는 바꾸지 않았다).
- 시험용 이름 붙은 pasteboard는 시험이 끝나도 pasteboard 서버에 남는다(로그아웃 때 사라진다).
- `edit::RepeatLast`(`.`)가 이 Task의 동작을 반복하는 것은 S3-T3b에서 만든다.

## 리뷰 1 대응(`docs/reports/s3-t3a/review-1.md`, 판정 「수정 후 재리뷰」)

| 지적 | 대응 | 커밋 |
| --- | --- | --- |
| 1 차단: 복제·반복 간격을 한 스텝 배수로 올려 짧은 선택이 「바로 뒤」에 오지 않음 | REQ 7.5 글자대로 폭 그대로(`duplicate_offset(notes)`). 격자 정렬은 결정 요청 `docs/decisions/2026-10-26-duplicate-offset-grid.md`(결정 대기, 추천 B, 반영은 S3-T3b). 순수 시험 이름·기대값을 바꿈(`plan_duplicate_offset_is_exact_span`). 계획 5·시험 8·리스크 고침 | `Duplicate and repeat by the exact selection span` |
| 2 앞 클립이 마디 중간에서 끝나면 같은 마디 틈에 붙지 않음 | core `clip_for_notes`: 시작은 마디 시작, 앞 클립이 그 마디 안 자리 틱 앞에서 끝나면 그 끝. 끝은 마디 끝, 다음 클립이 노트 끝과 마디 끝 사이에서 시작하면 그 시작. 노트가 다른 클립에 닿을 때만 거부. 시험 `clip_for_notes_starts_after_prev_clip_and_stops_before_next_in_same_bar`. 계획 4 고침 | `Fit gap clips between mid-bar clip ends for paste` |
| 3 배치 뷰 undo가 노트 선택을 만듦 | `select_appeared`는 피아노롤 뷰에서만 | `Restore the undo selection only in the piano roll view` |

| 제안 | 대응 |
| --- | --- |
| 1 큰 붙여넣기 undo가 제곱 | core 편집 단위 문제라 이 Task에서 고치지 않는다. S8-T3 이월 줄로 옮겼다 |
| 2 내부 구조를 쓸 때도 SMF·텍스트를 읽음 | `paste_contents`가 `changeCount`만 먼저 본다 |
| 3 맨 글의 알 수 없는 오류 문구 | 머리 없는 글이 조각으로도 안 읽히면 「클립보드의 글이 계이름 텍스트가 아니다」. `paste_reads_external_smf_then_text`가 문구를 본다 |
| 4 빠진 형식을 알리지 않음 | 복사 알림에 「다른 앱용 텍스트·SMF는 만들지 못했다」. 이 경로를 일으키는 노트(`Loss::Mismatch`)를 만드는 시험은 쓰지 못했다 |
| 5 리뷰 시험 옮기기 | `review_s3_t3a.rs` 20개를 그대로 PR에 둔다(모두 통과). 「같은 값 둘」 UI 시험은 그 파일의 `delete_same_value_twins_removes_both_and_undo_selects` |

수정 뒤: `transform_insert` 6, `review_s3_t3a`(core) 3, `clipboard_notes` 7, `note_edit` 10, `review_s3_t3a`(app) 20 통과. fmt·clippy 통과.

## 리뷰어에게

- 대상 규칙(`note_edit.rs` `edit_targets`): 복사만 선택의 클립이 현재 클립이 아니어도 된다. 같은 값의 노트가 여럿이면 모두 대상이다.
- 붙여넣기 출처 판정(`clipboard::paste_contents`): `changeCount` 하나로 내부/외부를 가른다. 다른 프로세스가 같은 값을 만드는 경우가 있는지, 앱을 다시 띄운 뒤 첫 붙여넣기(내부 없음)가 외부 내용으로 가는지 봐 달라.
- 틈에 붙일 때 새 클립을 만드는 것은 5.10의 넓힌 읽기다(계획 「리스크」).
- undo·redo 뒤 선택(`window.rs` `select_appeared`)은 현재 트랙 전체를 복제해 비교한다. 큰 트랙에서의 비용과, 다른 트랙을 바꾼 undo에서의 동작을 봐 달라.
- 입력란 포커스에서는 `input::Copy/Cut/Paste`를 디스패치한다. 이 이름은 시험이 고정한다.
