# 적대적 리뷰: S2-T5b 트랜스포트 바와 트랙 목록 (리뷰 4회차, 한정 확인 리뷰)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않은 적대적 리뷰어). 1~3회차 리뷰어와도 다른 세션
- 성격: 한정 확인 리뷰. `docs/decisions/2026-10-17-s2-t5b-review-limit.md`의 선택지 A에 따라 리뷰 3의 지적 1~3과 제안 4의 대응만 본다. 그 밖의 범위는 다시 열지 않는다
- 대상 커밋: 288beac(브랜치 `s2/t5b-transport-bar`, 리뷰 3 대응은 `git show 288beac`, 기준은 cc99229)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `edit_undo`·`edit_redo`(window.rs:292-340)는 `window.context_stack()`에 `Input` 컨텍스트가 있으면 `input::Undo`·`input::Redo`를 디스패치한다. 액션 이름이 맞는지(gpui-base `actions!(input, [.., Undo, Redo, ..])`, input/base/state.rs:74-110), 컨텍스트 이름이 맞는지(`CONTEXT = "Input"`, state.rs:134)를 본다.
2. 무한 재귀: `input::Undo`가 다시 `edit::Undo`로 라우팅되는 경로가 있는가. 입력란 안의 `cmd-z` 바인딩(state.rs:273)은 `Input` 컨텍스트가 더 깊어 루트의 `edit::Undo`보다 먼저 이긴다. 메뉴 경로만 `edit::Undo`를 거친다.
3. 입력란이 없을 때 문서 undo가 살아 있는가: 키·메뉴, 두 프리셋, 팔레트를 닫은 뒤.
4. 팔레트가 열려 있고 질의가 비어 있을 때: 입력란이 포커스를 가지므로 문서는 그대로여야 하고 멈추지 않아야 한다.
5. 조성·포트 메뉴: 지금 값은 편집이 없고 다른 값은 undo 한 단위인가. 방금 고른 값을 또 고르면.
6. `sync_transport_fields`가 고친 필드의 `shown`(기준)만 새 문서 값으로 옮기는데, 고치지 않은 필드 따라가기, 고친 필드의 글자 보존, Esc·Blur 취소 시 새 문서 값 복원, 새 문서 값과 똑같은 글자 뒤 Enter(편집 없음), 옛 값과 같은 글자 뒤 Enter(확정)가 모두 맞는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 액션·컨텍스트 이름 | r3 `cmd_z_in_palette_query_does_not_undo_document` 등이 간접으로 | 이름 오타면 조용히 `Err`로 넘어가므로 입력란 글자가 실제로 되돌아가는지 본다 |
| 2. 재귀 | 없음 | 질의 비어 있음 + 연속 `cmd-z`로 확인(시도 3) |
| 3. 입력란 없음, 팔레트 닫음 | 부분(r2 `cmd_z_after_leaving_field_undoes_document`) | 프리셋·메뉴·팔레트 닫기 뒤는 없음 |
| 4. 빈 질의 | 없음 | |
| 5. 같은 값 두 번, 다른 값 | r3 C3·C4는 지금 값 한 번만 | |
| 6. 기준 이동 | r3 C5 하나뿐 | Esc·Blur 복원, 새 값 입력, 독립성은 없음 |

## 3. 깨뜨리기 시도

모두 실제로 실행했다(`cargo test -p keyscore-app --test review_s2_t5b_r4`, r3도 함께). 시험 창은 활성이다. 키는 `simulate_keystrokes`, 메뉴 막대 경로는 포커스로의 `edit::Undo/Redo` 디스패치다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 트랙 지움 → `cmd-shift-p x` → `cmd-z` → `cmd-shift-z`, 두 프리셋 | 못 깨뜨림(질의만 `""`↔`x`, 문서 그대로) | `palette_query_cmd_z_and_redo_edit_query_only` |
| 2 | 팔레트 질의 `a b` → `edit::Undo/Redo` 메뉴 경로 | 못 깨뜨림(질의가 `""`↔`ab`, 문서 그대로) | `palette_menu_undo_and_redo_edit_query_only` |
| 3 | 질의 빈 팔레트에서 `cmd-z cmd-shift-z cmd-z` | 못 깨뜨림(멈추지 않음, 재귀 없음, 문서 그대로) | `palette_empty_query_cmd_z_keeps_document_and_terminates` |
| 4 | 팔레트를 닫은(`escape escape`, 첫 Esc는 질의를 비움) 뒤 키·메뉴 undo/redo, 두 프리셋 | 못 깨뜨림(문서 undo/redo 정상) | `document_undo_works_after_palette_closed` |
| 5 | 입력란 없이 키·메뉴 undo/redo, 두 프리셋 | 못 깨뜨림 | `document_undo_without_input_keys_and_menu` |
| 6 | 박자표 필드에서 키·메뉴 undo/redo 및 여섯 번 연속 `cmd-z` | 못 깨뜨림(문서 그대로) | `meter_field_key_and_menu_undo_edit_text_only` |
| 7 | 템포 필드 `cmd-a 9 0` → 메뉴 `edit::Undo` 두 번 → `edit::Redo` 두 번 → Enter | 못 깨뜨림(`120.0`↔`90`, 문서 그대로, Enter 확정 정상). gpui 입력란은 글자 하나씩 되돌린다(키 경로도 같음, 진단으로 확인) | `menu_undo_and_redo_in_dirty_tempo_field` |
| 8 | 조성 메뉴 지금 값 → 다른 값 → 방금 고른 값 다시 → `edit::Undo`, 포트 메뉴 지금 값 두 번 | 못 깨뜨림(단위 수 n, n+1, n+1, n 그대로) | `key_and_port_menu_different_value_still_undoable_same_value_not` |
| 9 | 템포 `cmd-a 9`(고침) → `SetTempo(100)` → 필드는 `9` 유지 → `cmd-a 1 2 0 . 0 enter` | 못 깨뜨림(120 확정. r3 C5와 같은 시나리오가 이제 통과) | `typed_old_value_after_document_change_commits` |
| 10 | 같은 상태에서 `escape`·다른 곳 클릭(Blur) | 못 깨뜨림(둘 다 새 문서 값 `100.0`으로 복원) | `dirty_field_escape_and_blur_restore_new_document_value` |
| 11 | 같은 상태에서 새 문서 값 `100.0`을 쳐 Enter | 못 깨뜨림(템포 100, undo 단위 늘지 않음) | `typing_new_document_value_then_enter_makes_no_unit` |
| 12 | 박자표 고치는 중 `SetTempo(100)` → Enter | 못 깨뜨림(템포 필드는 따라가고 박자표 `3/4`는 글자 보존·확정) | `untouched_follows_dirty_keeps_text_independently` |

시도 4의 첫 Esc 동작(질의 비움 후 두 번째 Esc에서 닫힘)은 이번 대응의 범위 밖이라 기록만 한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 텍스트 입력 중 `cmd-z`는 그 입력란의 undo(KM-14), 키·메뉴 동일 | 아니오 | 시도 1·2·6·7 |
| 입력란이 없으면 문서 undo | 아니오 | 시도 4·5 |
| 편집은 undo 한 단위, 빈 단위 없음 | 아니오 | 시도 8·11 |
| 필드 표시 = 문서 값(입력 중이 아닐 때), 고친 필드는 글자 보존 | 아니오 | 시도 9~12 |

## 5. REQ 대조 (한정 범위)

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| KM-14(텍스트 입력 중 `Cmd+Z`/`Cmd+Shift+Z`) | 충족 | window.rs:292-340, 시도 1~3·6·7 |
| UI-11(메뉴의 되돌리기·다시 하기) | 충족 | 입력란 포커스에서도 메뉴 경로가 입력란을 되돌린다. 시도 2·7 |
| TRN-4(템포·박자표·조성 필드) | 충족 | transport_bar.rs:128-139, 318-324, 367-381. 시도 8~12 |

범위 밖 요구사항(TRN-1·TRN-2, OUT-4·OUT-6 등)은 리뷰 3의 판단을 유지하며 다시 보지 않았다.

## 6. 실시간·에러 경로

- 스케줄러 경로 변경 없음. 바뀐 곳은 메인 스레드 처리기뿐이다.
- `cx.build_action(...)`의 `Err`는 조용히 넘어간다. 액션 이름이 틀리면 입력란이 안 되돌아가지만 이름은 시도 1·2·7에서 실제로 동작함을 확인했다(실사용 오류 아님, 지적 아님).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| - | - | 리뷰 3 지적 1~3, 제안 4의 대응 범위에서 실제 결함을 찾지 못했다 | - |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 3 지적 1(팔레트 질의 포함 일반 입력란 판정), 지적 2(메뉴 경로), 지적 3(조성·포트 같은 값), 제안 4(고친 필드 기준 이동)의 대응이 근본까지 닿았다. 재귀는 없고(`Input` 컨텍스트의 `cmd-z` 바인딩이 루트보다 깊어 입력란 안의 키는 `edit::Undo`를 거치지 않으며 `input::Undo`는 다시 `edit::Undo`로 가지 않는다), 입력란이 없을 때 문서 undo는 키·메뉴·두 프리셋·팔레트를 닫은 뒤 모두 살아 있다. 12개 시도 모두 통과했다. 한정 확인 리뷰이므로 이 범위 밖은 판정에 넣지 않았다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다(한정 범위)
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 C: 깨지는 테스트

없음. 모든 시도가 통과해 `crates/keyscore-app/tests/review_s2_t5b_r4.rs`에 남겼다.
