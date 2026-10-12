# Task 보고: S0-T4 undo 엔진

- PR: 로컬 브랜치 `s0/t4-undo` (remote 없음)
- 계획 파일: docs/reports/s0-t4/plan.md
- 계획 대비 달라진 점
  - `Edit::RestoreNote(NoteRestore)`를 추가했다. 숨김 꼬리 노트(분할로 생긴, 끝이 클립 길이를 넘는 노트)를 지운 뒤 undo하면 `insert_note`로는 되살릴 수 없다(끝까지 클립 안이어야 함). 그렇다고 공개 편집이 꼬리를 허용하면 S0-T2의 「꼬리는 분할로만 생긴다」가 깨진다. 그래서 노트 삭제의 역편집은 필드가 crate 밖에 보이지 않는 `NoteRestore`에 담았다. 밖에서는 이 값을 만들 수 없고, 노트 삭제의 역편집으로만 얻는다.
  - `Edit::UnsplitClip`을 추가했다. 분할의 역이며, redo가 같은 오른쪽 `ClipId`로 다시 분할하도록 `SplitClip`의 `right_id`를 채운다.
  - `ModelError`에 `TrackIndexOutOfRange`, `DuplicateClipId`, `NoteNotFound`, `NoChangeAt`, `HistoryInconsistent`를 더했다.
  - 「묶음 중간 실패」 속성 테스트가 빈 묶음에서 undo를 기대하던 허점을 고쳤다. 빈 묶음은 undo 단위를 만들지 않는 것이 의도된 동작이다.

## 구현 요약

- `src/edit.rs`: `Edit`(원시 편집 16종), `TrackProp`, `NoteRestore`. `apply`가 모델 API를 거쳐 적용하고 역편집을 돌려준다. `approx_bytes`는 메모리 상한용 추정치다.
- `src/history.rs`: `History`, `CoalesceKey`.
  - `apply_all`은 묶음을 차례로 적용하고, 실패하면 이미 적용한 것을 거꾸로 되돌린다.
  - 묶기(coalesce)는 키가 같고 그 사이에 seal·undo·redo가 없을 때만 일어난다.
  - 메모리 상한을 넘으면 가장 오래된 단위부터 버리되, 가장 최근 단위 하나는 남긴다.
- `src/track.rs`(crate 내부): `insert_clip_with_id`, `split_clip_with_id`, `unsplit_clip`, `restore_note`, 겹침 검사 `check_free`. 공개 `insert_clip`·`split_clip`은 이 함수들을 거친다.
- `src/clip.rs`(crate 내부): `restore_note`(시작만 클립 안이면 받고, 절대 끝의 u64 검사).
- `src/project.rs`: `insert_track`.

## 테스트 결과

~~~
keyscore-core 전체: 90 passed (lib 53, history 5, 그 밖 기존 32)
history 속성 테스트: 각 512 사례
check-deps.test 11, bench-check.test 8 통과
~~~

테스트를 먼저 썼다(`fec79bb`). `todo!()` 뼈대에서 새 단위 12개와 통합 5개가 실패하는 것을 확인했다.

속성 테스트의 연산 분포를 임시 계측했다(확인 후 되돌림). 처음에는 트랙이 자주 모두 지워져 대상 없는 연산이 많았고, 노트 삭제 성공이 한 번 실행에 38회뿐이었다. 트랙을 하나는 남기고 노트 시작을 클립 안으로 맞추자 노트 삽입 성공 982회, 삭제 152회, 분할 390회, 클립 이동 798회로 늘었다(`590d0e5`).

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `history_random_edits_undo_all_restores_initial` | 통과 |
| 2 | `history_undo_then_redo_all_restores_final` | 통과(`ClipId` 포함 동일) |
| 3 | `history_new_edit_after_undo_clears_redo` | 통과 |
| 4 | `history_coalesced_edits_undo_at_once` | 통과 |
| 5 | `history_coalesce_breaks_on_other_key_seal_or_undo` | 통과 |
| 6 | `history_failed_transaction_leaves_project_unchanged` | 통과 |
| 7 | `history_undo_redo_preserve_invariants`(1·2·추가 속성 안에서 매 단계 검사) | 통과 |
| 8 | `history_split_redo_reuses_right_clip_id` | 통과 |
| 9 | `history_memory_limit_drops_oldest`, `history_memory_limit_keeps_latest_even_if_too_large` | 통과 |
| 10 | `history_undo_after_outside_mutation_fails_without_change` | 통과 |
| 11 | `edit_apply_returns_inverse_for_each_variant`, `edit_apply_failure_leaves_project_unchanged` | 통과 |
| 추가 | `history_mixed_undo_redo_and_new_edits_stay_consistent`(스냅샷 기준 모델과 매 단계 비교), `history_remove_tail_note_then_undo_restores_it`, `edit_split_inverse_keeps_right_id_for_redo`, `edit_approx_bytes_grows_with_notes`, `history_empty_apply_adds_nothing`, `history_errors_are_model_errors` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings. `Edit::apply`와 테스트의 `to_edits`에 `too_many_lines` 항목 단위 허용을 이유와 함께 달았다.
- [x] test
- [x] deny
- [x] `check-deps.sh`, `check-deps.test.sh` 11/11, `bench-check.test.sh` 8/8

## 알려진 한계

리뷰 2 뒤 개정했다. 처음 판에는 「undo가 실패하는 경우는 히스토리 밖에서 고친 때뿐이고, 트랙은 인덱스로 가리킨다」고 적었다. 이제는 둘 다 맞지 않는다.

- 메모리 사용량은 추정치다(구조체 크기 + 노트·문자열). 실측은 NF-7(S8)이다. 앱의 기본 상한은 S2 설정에서 정한다.
- 편집의 공개 진입점은 `Document`뿐이고, `&mut Project`를 내주지 않는다. `Project`의 `&mut` API(`track_mut` 등)는 구성 단계(S0-T6 파일 읽기, 테스트)용이다. 그 단계에서 만든 `TrackId`·`ClipId` 중복은 `Document::new`가 거부한다.
- 상위 편집(퀀타이즈, 클립 자동 연장, 붙여넣기)은 `Edit` 묶음으로 S0-T7·S3·S4에서 만든다.

## 리뷰어에게

- 원자성(`apply_all`의 되돌리기)과 「실패한 `Edit::apply`는 상태 불변」 가정을 봐 달라. 특히 `SetTempo`·`SetMeter`의 `None`(삭제) 경로, `UnsplitClip`이 엉뚱한 `original`을 받는 경우를 봐 달라.
- `NoteRestore`가 정말 밖에서 만들 수 없는지, 다른 공개 경로로 숨김 꼬리를 새로 만들 수 있는지 봐 달라. 예를 들어 `UnsplitClip`의 `original`에 꼬리가 있는 클립을 넣는 경우다.
- 묶기 규칙(`sealed`)이 REQ 5.7 「같은 조작을 연속으로 누른 것」과 맞는지 봐 달라.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | `RestoreNote`를 보관했다 같은 id의 짧은 새 클립에 적용하면 분할 없이 꼬리가 생김 (차단) | 역편집 전용 연산을 공개 `Edit`에서 빼서 crate 내부 `Op`에 두고, `History` 안에만 보관한다. `Edit::apply`도 공개하지 않는다. 공개 `InsertClip`은 id를 지정할 수 없다. 그래서 역편집을 밖에서 만들거나 보관할 수 없고, 지운 클립의 id를 다시 쓸 공개 경로도 없다. `Edit` 문서에 `compile_fail` 예 2개를 넣었다 |
| 2 | 손으로 만든 `UnsplitClip`과 실패 편집을 묶으면 원자성이 깨짐 (차단) | 1과 같은 수정으로 위조할 수 없다. `apply_all`의 되돌리기는 방금 적용한 연산의 역만 쓰므로 실패하지 않는다 |
| 3 | 맞붙지 않은 두 클립을 받는 `UnsplitClip` 뒤 undo 불가 (차단) | 1에 더해, crate 내부 `unsplit_clip`이 「두 클립이 맞붙어 있고, 원본의 시작·끝이 두 클립의 시작·끝과 같다」를 검사한다. 단위 테스트 `unsplit_rejects_clips_that_were_not_split_from_original` |
| 4 | 같은 (시작, 음높이) 노트를 지웠다 undo하면 순서가 바뀜 (차단) | `Note`에 전체 순서(시작, 음높이, 길이, 오프셋, 벨로시티, 가사)를 정의하고, 클립은 이 순서로 넣는다. 리뷰어 테스트가 이제 통과한다 |
| 5 | 다른 트랙의 `ClipId`를 받아 전역 유일성이 깨짐 | id를 지정하는 삽입·분할은 내부 연산뿐이고, 히스토리가 만든 역연산으로만 쓰인다 |
| 6 | 히스토리 밖 트랙 순서 변경 뒤 undo가 `Ok`이지만 아무것도 안 함 | 트랙을 `TrackId`로 가리킨다. `Project`는 같은 `TrackId`의 트랙(복제본)을 거부한다. 그래서 undo는 올바른 트랙을 찾거나 `Err(UnknownTrack)`를 돌려준다 |
| 7 | 실패한 `apply`가 묶기를 끊지 않음 (제안) | 의도다. 실패한 `apply`는 히스토리를 바꾸지 않으므로, 이어지는 조작을 둘로 나누지 않는다. 문서에 적고 테스트 `history_failed_apply_does_not_break_coalescing`를 더했다 |
| 8 | undo가 상한을 적용하지 않음 (제안) | 받음. undo 뒤에도 `enforce_limit`를 부른다. 테스트 `history_undo_enforces_limit` |

리뷰어 테스트 처리: 시도 1–3은 역편집을 손으로 만들거나 다시 적용하는 코드였다. 이제는 그런 값을 만들 수 없어서 컴파일되지 않는다. 그래서 각 시도를 같은 목표를 공개 API(History)로 노리는 테스트로 바꿨고, 파일 머리와 커밋 `936c5ea` 본문에 이유를 적었다. 시도 4는 `TrackId` 표기만 바꿨다.

테스트를 먼저 썼다. 리뷰어의 실패 테스트 4개를 그대로 커밋했고, `TrackId`·노트 전체 순서 테스트도 `todo!()` 뼈대(`29fd5e8`)에서 실패하는 것을 확인했다.

`tests/history.rs`에서 또 하나 고쳤다. `initial()`이 부를 때마다 새 `TrackId`를 만들어서, 「끝까지 undo하면 `initial()`과 같다」 비교가 항상 실패한다. 처음 상태를 복제해 두고 그것과 비교한다.

~~~
cargo test --workspace: 102 passed (doc-test 2개 포함, compile_fail 2개)
check-deps.test 11, bench-check.test 8 통과
~~~

## 리뷰 대응 (review-2, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | `track_mut` 대입으로 `TrackId`·`ClipId` 중복, 그 상태에서 원자성 위반 (차단) | 편집의 공개 진입점을 `Document`로 바꿨다. `Document`는 프로젝트와 히스토리를 함께 소유하고 `&mut Project`를 내주지 않는다. `Document::new`는 `TrackId`·`ClipId` 중복을 거부한다. `Project` 수준의 중복은 구성 단계에서 여전히 만들 수 있고, `Project::track_mut` 문서에 적었다 |
| 2 | 복제·두 번째 `History`로 맞지 않는 역편집이 `Ok`로 적용됨 (차단) | `History`를 crate 내부로 숨겼다. 복제는 `Document` 통째로만 되며, 프로젝트와 히스토리가 함께 복제된다 |
| 3 | 히스토리 밖 변경 뒤 undo가 `Ok`로 꼬리를 만들거나 노트를 지움 | 1·2로 히스토리 밖 변경 경로가 없다. `compile_fail` 예로 `project_mut`이 없음을 확인한다 |
| 4 | `compile_fail` 예 1이 아무것도 지키지 못함 | `matches!` 패턴과 메서드 호출로 바꿨다. 같은 모양에 실재하는 `SplitClip`과 `project()`를 넣으면 컴파일되는 것을 확인했다(확인 후 되돌림). 그러니 예가 실패하는 이유는 그 항목이 없다는 것 하나뿐이다 |
| 5 | 보고서 「알려진 한계」가 낡음 | 고쳤다(위 절, 개정 표시) |
| 6 | `handmade_*` 테스트 이름 (제안) | `split_then_undo_restores_initial`, `split_in_failing_batch_leaves_document_unchanged`로 바꿨다 |

리뷰어 테스트 처리: 2회차 시도 1–5는 `&mut Project`를 히스토리 밖에서 쓰거나 `History`를 따로 만들거나 복제했다. 이제는 그런 코드가 컴파일되지 않는다. 그래서 각 시도를 같은 목표를 `Document`로 노리는 테스트로 바꿨고, 파일 머리와 커밋 `b8ba908` 본문에 이유를 적었다.
- 시도 1: `Project` 수준 중복은 여전히 만들 수 있다는 전제를 확인하고, 그 프로젝트를 `Document::new`가 거부하는지 본다.
- 시도 2: 문서 안에서 복제 트랙을 넣는 묶음이 원자적으로 실패하는지 본다.
- 시도 3: 문서를 복제했을 때 복제본의 undo가 원본에 영향을 주지 않고 꼬리를 만들지 않는지 본다.
- 시도 4·5: 같은 모양의 편집을 문서로 하고, undo가 노트를 말없이 잃거나 꼬리를 만들지 않는지 본다.
- 시도 6: 그대로 `Document`로 옮겼다.

~~~
cargo test --workspace: 112 passed (doc-test 4개 포함, 4개 모두 compile_fail)
check-deps.test 11, bench-check.test 8 통과
~~~
