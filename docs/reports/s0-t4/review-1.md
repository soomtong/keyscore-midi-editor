# 적대적 리뷰: S0-T4 undo 엔진 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `a2563ca` (브랜치 `s0/t4-undo`, `main` 대비 5커밋)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`edit.rs`, `history.rs`, `track.rs`·`clip.rs`·`project.rs` 변경분만 읽고 적었다.

1. `Edit::UnsplitClip`은 공개 변형이고 `original`을 검증하지 않는다(겹침만 본다). 붙어 있지 않은 두 클립을 `left`·`right`로 주면 `right`가 통째로 지워지고, 역편집 `SplitClip { at: left.end() }`이 `original` 안쪽이 아니면 undo가 실패하거나 엉뚱한 상태로 돌아간다.
2. `apply_all`의 되돌리기는 「역편집은 직전 상태에서 반드시 성공한다」에 기댄다. 1번처럼 역이 정확하지 않은 편집 뒤에 실패하는 편집을 묶으면 되돌리기가 실패해 `HistoryInconsistent`를 돌려주면서 프로젝트는 바뀐 채로 남을 것이다(원자성).
3. `NoteRestore`는 밖에서 만들 수 없지만 `Edit`·`NoteRestore`가 `Clone`이다. 노트 삭제로 얻은 `RestoreNote`를 보관했다가 클립이 짧아진 뒤(혹은 `InsertClip { id: Some(..) }`로 같은 id의 짧은 새 클립을 만든 뒤) 적용하면 분할을 거치지 않은 숨김 꼬리가 생길 것이다.
4. `Clip::restore_note`는 같은 `(시작, 음높이)` 키 뒤에 넣고, `remove_note`는 같은 값의 첫 노트를 지운다. 같은 키에 길이만 다른 노트가 둘 있을 때 앞의 것을 지우고 undo하면 순서가 바뀌어 `PartialEq`가 달라질 것이다.
5. `InsertClip { id: Some(..) }`·`SplitClip { right_id: Some(..) }`에 다른 트랙의 id를 주면 프로세스 전역 유일성(`ClipId` 문서)이 깨진다. 트랙 안 중복 검사만 있다.
6. `used_bytes`의 usize 뺄셈(`history.rs` 64, 98, 126, 148줄)이 언더플로하는 순서가 있는가. 묶기·상한 버림·undo·redo가 섞일 때 항목 바이트 합과 맞는가.
7. `SetTempo`/`SetMeter`의 `None`(삭제) 경로: 틱 0, 변화가 없는 틱에서 상태를 바꾸지 않고 실패하는가. 역이 `None`일 때 redo가 맞는가.
8. 히스토리 밖에서 트랙 순서를 바꾼 뒤 undo: 같은 id를 가진 복제 트랙이 있으면 엉뚱한 트랙에 조용히 성공할 것이다.
9. 묶기 중간에 실패한 `apply`가 끼면 묶음이 이어지는가, 끊기는가.
10. 한 묶음 안에서 앞 편집의 결과를 뒤 편집이 가리킬 때(같은 id 재삽입 → 이동 → 분할 → 노트) undo/redo가 `ClipId`까지 같은가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 손으로 만든 `UnsplitClip` | 없음 | `edit_split_inverse_keeps_right_id_for_redo`는 분할의 역으로 얻은 값만 쓴다 |
| 2. 역이 부정확한 편집이 낀 묶음의 원자성 | 부분 | `history_failed_transaction_leaves_project_unchanged`는 정확한 역만 생성한다 |
| 3. `RestoreNote` 재사용으로 꼬리 생성 | 없음 | `history_remove_tail_note_then_undo_restores_it`은 정상 경로만 |
| 4. 같은 키 노트의 undo 순서 | 없음 | 속성 생성기는 같은 (시작, 음높이)에 다른 길이를 거의 만들지 않는다 |
| 5. 다른 트랙 id 재사용 | 없음 | |
| 6. `used_bytes` 회계 | 부분 | 상한 테스트 2개. undo/redo 반복·묶기 혼합은 없음 |
| 7. 템포·박자 `None` | 있음 | `edit_apply_failure_leaves_project_unchanged` |
| 8. 밖에서 트랙 순서 변경 | 부분 | `history_undo_after_outside_mutation_fails_without_change`는 실패하는 경우만 본다 |
| 9. 묶기 중 실패 | 없음 | |
| 10. 묶음 안 의존 편집 | 없음 | 생성기는 묶음당 편집 하나 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 노트(0, 길이 900)를 `RemoveNote`해 얻은 `RestoreNote`를 보관. `RemoveClip` 뒤 `InsertClip { id: Some(a), 길이 10 }`으로 같은 id의 짧은 새 클립을 만들고 `History::apply(vec![restore])` | 깨짐. `Ok(())`, 길이 10 클립에 끝 900 노트(분할 없이 생긴 꼬리) | `restore_note_reused_on_fresh_short_clip_is_rejected` |
| 2 | `UnsplitClip { left: a(0..960), right: b(1920..2880), original: Clip::new(0, 960) }`을 `History::apply` 후 undo | 깨짐. apply는 `Ok`, b와 노트가 지워지고 undo는 `Err(SplitOutsideClip { at: 960, .. })`. 되돌릴 방법이 없다(데이터 손실) | `handmade_unsplit_then_undo_restores_initial` |
| 3 | 2의 `UnsplitClip` 뒤에 `RemoveTrack { index: 99 }`를 같은 묶음으로 `History::apply` | 깨짐. `Err(HistoryInconsistent)`를 돌려주면서 프로젝트는 b가 지워진 채로 남는다(원자성 위반) | `handmade_unsplit_in_failing_batch_leaves_project_unchanged` |
| 4 | 같은 (100, 70)에 길이 10·20 노트를 넣고 길이 10을 `RemoveNote` 후 undo | 깨짐. 노트 순서가 [20, 10]이 되어 undo 1번 ≠ 직전 상태 | `remove_first_of_same_key_notes_then_undo_restores_order` |
| 5 | `SetTempo`/`SetMeter`의 `None`을 틱 0과 변화 없는 틱 5에 적용(히스토리 경유), 이어서 설정 → 삭제 → undo 2 → redo 2 | 못 깨뜨림. 실패 시 프로젝트·`undo_len` 불변, 왕복 일치 | 임시 실험(삭제) |
| 6 | 한 묶음에 `RemoveClip b` → `InsertClip { id: Some(b) }` → `MoveClip b` → `SplitClip b` → `InsertNote b` → 트랙 1에 `InsertClip { id: Some(a) }` → 트랙 1 `SplitClip { right_id: Some(b) }`. 앞의 변형(마지막이 트랙 0에서 `right_id: Some(a)`로 실패)도 시도 | 못 깨뜨림. 실패 묶음은 상태 불변, 성공 묶음은 undo = 초기, redo = 최종(`ClipId` 포함). 단 트랙 1에 a·b가 그대로 들어가 전역 유일성이 깨지는 것은 확인(지적 5) | 임시 실험(삭제) |
| 7 | `CoalesceKey(1)`로 성공 apply와 실패 apply(노트가 클립 밖)를 다섯 번 번갈아 | 못 깨뜨림. 묶음 하나, undo 한 번에 초기 상태. 실패가 묶음을 끊지 않는다(지적 7) | 임시 실험(삭제) |
| 8 | 상한 0에서 redo/undo 50회 반복, 가사 붙은 노트 30개를 묶기·undo·redo 섞어 적용 후 undo 전부 → redo 전부 → undo 전부 | 못 깨뜨림. usize 뺄셈 패닉 없음. 회계는 항목에 저장된 `bytes`만 더하고 빼서 일관된다 | 임시 실험(삭제) |
| 9 | 트랙 0을 복제해 트랙 1로 넣고, 히스토리로 트랙 0의 클립 a를 5000으로 이동한 뒤 히스토리 밖에서 트랙 0을 맨 뒤로 옮기고 undo | 문서상 계약 위반 상황이라 차단으로 세지 않음. undo가 `Ok(true)`를 돌려주지만 아무것도 바뀌지 않는다(복제 트랙의 a가 이미 원위치). 원래 트랙의 a는 5000에 남고 undo 단위는 소모된다(지적 6) | 임시 실험(삭제) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | 모든 클립 배치가 `check_free`/`overlaps`를 거친다. `unsplit_clip`도 `left`·`right`를 뺀 나머지와 겹침 검사 |
| 노트 시작은 클립 안 | 못 깨뜨림 | `restore_note`가 `start < length` 검사 |
| 숨김 꼬리는 분할로만 생김 | 깨뜨릴 수 있음 | 시도 1. `RestoreNote` 복제·보관 후 같은 id의 짧은 새 클립(`InsertClip { id: Some }`)이나 분할로 짧아진 클립에 적용 |
| 한 트랙 안 `ClipId` 중복 없음 | 못 깨뜨림 | `insert_clip_with_id`, `split_clip_with_id`가 검사. 트랙 사이 중복은 가능(지적 5) |
| u64 절대 끝 | 못 깨뜨림 | `restore_note`가 `start + note.end()`, `move_clip`이 `extent` 검사 |
| 원자성(Err면 프로젝트·히스토리 불변) | 깨뜨릴 수 있음 | 시도 3. `History::apply`가 `HistoryInconsistent`를 돌려주며 프로젝트를 바꾼다 |
| undo N번 = 초기, undo 후 redo = 원래 | 깨뜨릴 수 있음 | 시도 2(undo 불가·데이터 손실), 시도 4(노트 순서) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| EDT-1 무제한 되돌리기/다시 실행, 단위는 편집 동작 하나, 같은 키 연속만 묶음 | 부분 충족 | 구조(`History::apply` 묶음, `CoalesceKey`, 개수 무제한 + 메모리 상한)는 `history.rs:53-83`, `145-151`. 그러나 되돌리기가 정확하지 않은 경로가 있다: 같은 키 노트 순서(`clip.rs:135-138`, 시도 4), 공개 `UnsplitClip`(`edit.rs:186-201`, 시도 2) |
| 2절 Sequence 녹음 undo(테이크 전체 한 단위) | 충족(엔진 수준) | 묶음 `apply`가 한 단위(`history.rs:53-83`). 호출 쪽은 S5, 계획 「범위 밖」 |
| 2절 Step 모드 undo(노트 하나 = 한 단위, 연속 같은 조작만 묶음) | 충족(엔진 수준) | `history.rs:66` 키가 같고 `sealed`가 아닐 때만 합침. 테스트 `history_coalesced_edits_undo_at_once` |
| 4절 P6(무제한, 입력 세션 단위 묶음) | 충족(엔진 수준) | 세션 묶음은 호출자가 한 묶음으로 넘기면 된다. 2절 Step 행의 「세션 묶음 없음」과의 관계는 앱 Task에서 정할 문제다 |
| 5.7 되돌리기 묶음(`Alt+Shift+l` 다섯 번 → `Cmd+Z` 한 번) | 충족 | `history.rs:66-72`. 실패한 같은 키 조작이 묶음을 끊지 않는 것은 REQ가 정하지 않음(지적 7) |
| CTL-8 드래그 한 번 = 한 단위 | 충족(엔진 수준) | 묶음 `apply`. 호출 쪽은 S3, 계획 「범위 밖」 |
| AI-5 수락 = undo 한 단위 | 충족(엔진 수준) | 묶음 `apply` |
| MS-1 놓는 순간 한 번의 undo 단위 | 충족(엔진 수준) | 묶음 `apply`. 호출 쪽은 S3 |

EDT-1이 부분 충족이므로 미충족 1건으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 해당 없음. 이 Task는 `core` 편집 경로만 다룬다. `ClipId::allocate`는 `AtomicU64::fetch_add`라 락이 없다.
- 제품 코드의 `unwrap`/`expect`: 없음. `edit.rs:209`의 `unwrap_or_default`, `track.rs:407`의 `unwrap_or_else`는 패닉하지 않는다.
- usize 뺄셈: `history.rs:64`, `98`, `126`, `148`. 모두 「`used_bytes` = 두 스택 항목 `bytes`의 합」이라는 회계에 기대며, 항목의 `bytes`를 저장해 두고 그대로 빼므로 언더플로하지 않는다(시도 8). `clip.rs:183`·`187`, `track.rs:406`의 뺄셈은 앞선 범위 검사로 보호된다.
- 에러가 삼켜지는 곳: `history.rs:184-188`. 되돌리기 중 역편집이 실패하면 원래 오류와 실패한 역편집의 오류를 모두 버리고 `HistoryInconsistent`만 돌려준다. 그때 프로젝트는 반쯤 적용된 상태로 남는다(지적 2).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 숨김 꼬리를 분할 없이 만들 수 있다. `Edit`·`NoteRestore`가 `Clone`이고 `RestoreNote`의 적용이 「그 노트가 원래 그 클립에 꼬리로 있었는가」를 확인하지 않는다. `InsertClip { id: Some(..) }`로 같은 id의 짧은 새 클립을 만들거나 분할로 클립을 줄인 뒤 보관한 `RestoreNote`를 적용하면 결정 Q1 위반. 테스트 `restore_note_reused_on_fresh_short_clip_is_rejected` | `edit.rs:21-27`, `edit.rs:98`, `edit.rs:236-239`, `clip.rs:125-140`, `edit.rs:142-149` |
| 2 | 차단 | 원자성 위반. 공개 `UnsplitClip`이 정확한 역을 갖지 않는 값을 받아들이므로, 같은 묶음의 뒤 편집이 실패하면 `apply_all` 되돌리기가 실패해 `Err(HistoryInconsistent)`를 돌려주면서 프로젝트를 바꾼다(클립 b와 노트 손실). `History` 문서 「모든 연산은 원자적이다」와 다르다. 테스트 `handmade_unsplit_in_failing_batch_leaves_project_unchanged` | `edit.rs:69-75`, `edit.rs:186-201`, `track.rs:415-432`, `history.rs:177-195` |
| 3 | 차단 | undo가 초기 상태로 돌리지 못한다(데이터 손실). 같은 `UnsplitClip`을 단독으로 적용하면 `History::apply`는 성공하지만 undo는 영구히 `Err(SplitOutsideClip)`. `original`이 두 클립을 덮는 범위면 undo는 성공하지만 b의 시작·노트를 되살리지 못한다. `unsplit_clip`이 「`left`와 `right`가 맞붙어 있고 `original`을 그 경계에서 나누면 현재 두 클립이 된다」를 검사하지 않는 것이 원인. `RestoreNote`처럼 밖에서 만들 수 없게 하거나 이 조건을 검사해야 한다. 테스트 `handmade_unsplit_then_undo_restores_initial` | `edit.rs:69-75`, `edit.rs:186-201`, `track.rs:415-432` |
| 4 | 차단 | 같은 (시작, 음높이)에 길이·벨로시티·가사가 다른 노트가 둘 이상일 때, 앞의 노트를 지우고 undo하면 `restore_note`가 같은 키 맨 뒤에 넣어 순서가 바뀐다. `Clip`의 `PartialEq`가 노트 벡터 순서를 비교하므로 undo 1번 ≠ 직전 상태(게이트 「undo N번 = 초기 상태」 위반). 삭제 위치를 역편집에 담거나 정렬 키를 전순서로 만들어야 한다. 속성 생성기가 이 경우를 거의 만들지 않아 놓쳤다. 테스트 `remove_first_of_same_key_notes_then_undo_restores_order` | `clip.rs:135-138`, `clip.rs:143-151`, `edit.rs:230-239` |
| 5 | 수정 | `InsertClip { id: Some(..) }`·`SplitClip { right_id: Some(..) }`에 다른 트랙의 id를 주면 받아들인다(시도 6). `ClipId` 문서는 「프로세스 안에서 유일하게 발급」이고 S0-T2 리뷰 1 제안 4가 전역 유일성을 택한 이유(다른 트랙 id로 무관한 클립을 지우는 사고 방지)가 무너진다. 공개 편집이 임의 id를 받을 이유는 undo·redo뿐이므로 지적 1·3과 함께 막는 것이 맞다. 테스트 없음 | `edit.rs:46-52`, `edit.rs:62-68`, `track.rs:293-300`, `track.rs:392-396`, `clip.rs:5-10` |
| 6 | 수정 | 히스토리 밖 트랙 순서 변경 뒤 undo가 `Ok(true)`를 돌려주면서 아무것도 되돌리지 않고 undo 단위를 소모한다(시도 9, 복제 트랙이 같은 id를 가질 때). `History` 문서와 보고서는 이 경우 「undo가 실패할 수 있다」고만 적어 조용한 오적용을 언급하지 않는다. 테스트 `history_undo_after_outside_mutation_fails_without_change`는 실패하는 경우만 본다. 최소한 문서·보고서의 「알려진 한계」를 정확히 적고, 가능하면 트랙을 안정 식별자로 가리키는 방안을 결정 요청으로 남긴다 | `history.rs:23-24`, `edit.rs:304-309`, `docs/reports/s0-t4/report.md` 「알려진 한계」 |
| 7 | 제안 | 같은 키 조작 사이에 실패한 `apply`가 있어도 묶기가 이어진다(시도 7). REQ 5.7 「같은 조작을 연속으로 누른 것」에서 실패한 키 입력이 연속을 끊는지는 REQ가 정하지 않는다. 의도라면 `apply` 문서에 적고, 아니면 실패 시 `sealed = true` | `history.rs:59-62` |
| 8 | 제안 | `undo`는 `enforce_limit`를 부르지 않는다. 역의 역이 더 큰 편집(`RemoveClip`의 역 `InsertClip`)을 undo하면 `used_bytes`가 상한을 넘은 채 다음 `apply`/`redo`까지 남는다. 버리는 대상이 undo 스택뿐이라 redo 스택이 상한을 넘는 것도 막지 못한다. 동작을 문서에 적거나 redo 스택도 상한에 포함 | `history.rs:91-112`, `history.rs:145-151` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 4건(숨김 꼬리 생성, 원자성 위반, `UnsplitClip`으로 undo 불가·데이터 손실, 같은 키 노트 순서). 모두 공개 API만으로 재현되며 `crates/keyscore-core/tests/review_s0_t4.rs`의 4개 테스트가 현재 실패한다. 범위 안에서 고칠 수 있으므로 범위 재정의는 아니다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(4개 실패, 임시 실험 4개 통과 후 삭제)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`core`의 edit·history·track·clip·project·error·lib, 테스트, 보고서, core README)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트 결과

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과(리뷰 테스트 포함)
- `cargo test --workspace`: 기존 테스트 전부 통과(core lib 53, history 5 등). 리뷰 테스트 `review_s0_t4` 4개 실패(의도한 깨뜨리기)
- `cargo deny check`: 통과(advisories, bans, licenses, sources ok)
- `scripts/check-deps.sh`: 통과
- `scripts/check-deps.test.sh`: 11 통과
- `scripts/bench-check.test.sh`: 8 통과
