# 적대적 리뷰: S0-T4 undo 엔진 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `7d00e32` (브랜치 `s0/t4-undo`). 1회차 리뷰 커밋 `3a24d44` 이후 5커밋(`29fd5e8`, `abc7624`, `e46470b`, `936c5ea`, `7d00e32`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 3a24d44`의 `edit.rs`, `history.rs`, `project.rs`, `track.rs`, `clip.rs`, `note.rs`만 읽고 적었다.

1. `Project::track_mut(index)`와 `track_by_id_mut(id)`는 `&mut Track`을 내준다. `*t = 다른 트랙.clone()`이나 `std::mem::swap`으로 트랙을 통째로 바꾸면 `insert_track`의 `DuplicateTrackId` 검사를 거치지 않는다. 프로젝트 안에 같은 `TrackId`가 둘 생기면 `ClipId`도 두 트랙에 겹친다.
2. 1의 상태에서 `RemoveTrack`의 역 `InsertTrack`은 남은 복제본 때문에 `DuplicateTrackId`로 실패한다. 그러면 실패하는 편집을 같은 묶음에 넣었을 때 `apply_all` 되돌리기가 실패해 `HistoryInconsistent`와 함께 트랙이 지워진 채 남고, 단독 `RemoveTrack`의 undo는 영구히 실패할 것이다.
3. `History`가 `Clone`이다. 역편집은 「히스토리 안에만」 있지만 히스토리를 복제하면 같은 역편집을 두 번, 다른 상태에 적용할 수 있다. 리뷰 1 지적 1(보관한 `RestoreNote`를 다른 맥락에 적용)과 같은 경로가 모든 편집을 히스토리로 하면서도 남는다. 한 히스토리를 두 프로젝트에 쓰거나 히스토리를 둘 쓰는 경우도 같다.
4. `Clip::restore_note`는 시작만 클립 안이면 꼬리를 허용한다. 3이나 히스토리 밖 변경으로 클립이 짧아진 뒤 노트 삭제를 undo하면, 분할을 가로지른 적이 없는 노트가 꼬리로 되살아날 것이다(결정 Q1).
5. `unsplit_clip`의 검사는 두 클립의 경계와 원본의 시작·끝만 본다. 내용(노트, 이름, 색)은 비교하지 않는다. 분할 뒤 오른쪽 클립에 다른 경로로 넣은 노트는 undo가 `Ok`를 돌려주면서 지울 것이다. `History` 문서(「undo가 `Err`를 돌려줄 수 있다. 그때도 프로젝트는 바뀌지 않고」)와 다르다.
6. `compile_fail` 예 `let _ = Edit::UnsplitClip;`은 변형이 있어도 컴파일되지 않는다(구조체 변형을 값으로 쓰면 E0533). 지키려는 것을 지키지 못한다.
7. `Note`의 `Ord`가 `PartialEq`가 보는 모든 필드를 덮는가. 빠진 필드가 있으면 서로 다른 노트가 `Equal`이 되어 리뷰 1 지적 4가 되살아난다.
8. 정상 경로(히스토리 하나, 프로젝트 하나)에서 트랙 A의 클립을 꺼내 트랙 B에 넣으면 새 id를 받는가. undo 전부 = 초기, redo 전부 = 최종이 `TrackId`·`ClipId`까지 같은가.
9. 역연산 중 정확하지 않은 것이 남았는가(`SetTempo`/`SetMeter`의 `set`이 중복 변화를 합치는지, `SetClipName(Some(""))` 등). 남았다면 정상 경로에서도 원자성이 깨진다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. `&mut Track` 대입으로 `TrackId` 중복 | 없음 | `project_rejects_clone_of_existing_track`는 `push_track`/`insert_track`만 본다 |
| 2. 중복 `TrackId`에서 원자성·undo | 없음 | |
| 3. `History::clone`·히스토리 여럿 | 없음 | 속성 테스트(`tests/history.rs`)는 히스토리 하나만 쓴다 |
| 4. `restore_note` 꼬리 허용의 정상 경로 밖 사용 | 없음 | `history_remove_tail_note_then_undo_restores_it`는 정상 경로만 |
| 5. 분할 undo의 내용 검사 | 부분 | `unsplit_rejects_clips_that_were_not_split_from_original`은 맞붙지 않은 두 클립만 본다 |
| 6. `compile_fail` 예가 올바른 이유로 실패 | 부분 | 예 2(`apply` 없음)는 E0599로 올바르다. 예 1은 변형이 있어도 E0533으로 실패한다 |
| 7. `Ord`와 `Eq`의 일치 | 있음 | `note.rs`의 `Ord`가 6개 필드 전부를 비교한다. `clip_note_order_is_total_so_remove_and_restore_round_trip` |
| 8. 다른 트랙으로 옮긴 클립의 id, 왕복 | 부분 | 속성 테스트가 왕복을 보지만 트랙 사이 이동을 한 묶음으로 만드는 경우는 드물다 |
| 9. 정상 경로 역연산의 정확성 | 있음 | `edit_apply_returns_inverse_for_each_variant`, 속성 테스트. `TempoMap::set`/`MeterMap::set`은 합치지 않는다(`tempo.rs:129`, `meter.rs:107`) |

리뷰 1 테스트의 개정(`review_s0_t4.rs` 시도 1–3)에 대한 판단: 정당하다. 원래 테스트가 쓰던 `Edit::apply`, `Edit::UnsplitClip`, `InsertClip { id }`가 공개 API에서 사라져 컴파일되지 않으므로 그대로 둘 수 없다. 시도 2·3의 「위조한 분할 되돌리기」는 이제 밖에서 만들 수 없고, 남은 방어(맞붙지 않은 클립 거부)는 crate 안 단위 테스트가 맡는다. 다만 시도 1의 의도(「보관한 역편집을 다른 맥락에 적용」)는 개정 테스트에 보존되지 않았다. 개정 테스트는 정상 경로의 삭제·삽입·undo·redo만 보며, 같은 의도의 공개 경로(`History::clone`, 히스토리 밖 변경)는 시도하지 않는다. 이번 시도 3·5가 그 경로로 같은 위반을 재현한다(지적 2). 개정된 시도 2·3의 테스트 이름 `handmade_unsplit_*`는 이제 손으로 만든 값을 쓰지 않아 내용과 맞지 않는다(제안 6).

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 트랙 [A, B]에서 `*p.track_mut(1).unwrap() = p.tracks()[0].clone()` | 깨짐. 프로젝트에 `TrackId(7)`이 둘, `ClipId`도 두 트랙에 겹친다 | `track_mut_assignment_cannot_duplicate_track_id` |
| 2 | 1의 상태(`std::mem::swap`으로 만듦)에서 `History::apply([RemoveTrack A, RemoveTrack 없는 트랙])` | 깨짐. `Err(HistoryInconsistent)`를 돌려주며 트랙 하나가 지워진 채 남는다(원자성 위반, 데이터 손실) | `duplicated_track_id_breaks_batch_atomicity` |
| 3 | 노트 (0, 길이 900) `RemoveNote` → `History::clone` → 원본 히스토리로 `SplitClip at 10` → 복제본으로 undo. 모든 편집은 히스토리 경유 | 깨짐. `Ok(true)`, 길이 10 클립에 끝 900 노트. 분할 시점에 그 노트는 없었다(결정 Q1 위반, 리뷰 1 지적 1과 같은 결과) | `cloned_history_undo_does_not_create_tail_without_split` |
| 4 | 히스토리 h로 `SplitClip`, 다른 히스토리로 오른쪽 클립에 노트 삽입, h로 undo | 깨짐. `Ok(true)`, 다른 히스토리로 넣은 노트가 사라진다(데이터 손실) | `undo_split_after_note_from_other_history_keeps_note_or_fails` |
| 5 | `RemoveNote` 뒤 히스토리 밖에서 `Track::split_clip(a, 10)`, 오른쪽 `remove_clip`, undo | 깨짐. `Ok(true)`, 분할 없는 꼬리. `History` 문서의 「밖에서 고치면 `Err`, 프로젝트 불변」과 다르다 | `undo_after_outside_shrink_does_not_create_tail` |
| 6 | 한 묶음으로 A의 클립을 `RemoveClip` → B에 `InsertClip` → `RemoveTrack A`, undo → redo. 직접 호출(`remove_clip` → `insert_clip`)도 | 못 깨뜨림. 새 id를 받고, undo = 초기, redo = 최종(`TrackId`·`ClipId` 포함) | `clip_moved_between_tracks_gets_new_id_and_round_trips`(통과, 회귀 방지로 남김) |
| 7 | 1의 상태에서 히스토리로 A의 클립을 5000으로 옮기고, 대입으로 두 트랙 순서를 바꾼 뒤 undo. 이어서 `RemoveTrack A` 단독 apply 후 undo | 깨짐. 첫 undo는 `Ok(true)`인데 아무것도 되돌리지 않는다(클립 시작 [0, 5000] 그대로, 리뷰 1 지적 6 재현). `RemoveTrack` undo는 `Err(DuplicateTrackId)`로 영구 실패해 트랙을 되살릴 수 없다 | 임시 실험(삭제). 원인은 시도 1과 같다 |
| 8 | `compile_fail` 예를 일반 테스트로 옮겨 오류 확인. 대조군으로 존재하는 변형 `let _ = Edit::SplitClip;` | 예 1: E0599(변형 없음)로 지금은 올바르다. 그러나 대조군도 E0533으로 컴파일되지 않는다. `UnsplitClip { .. }`이 공개 변형으로 되돌아와도 이 예는 계속 통과한다. 예 2: E0599(`apply` 없음)로 올바르다 | 임시 실험(삭제) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | 모든 배치가 `check_free`/`overlaps`를 거친다 |
| 노트 시작은 클립 안 | 못 깨뜨림 | `restore_note`가 `start < length` 검사(`clip.rs:124`) |
| 숨김 꼬리는 분할로만 생김(결정 Q1) | 깨뜨릴 수 있음 | 시도 3(`History::clone`, 모든 편집이 히스토리 경유), 시도 5(히스토리 밖 변경) |
| 프로젝트 안 `TrackId` 유일 | 깨뜨릴 수 있음 | 시도 1. `track_mut`/`track_by_id_mut`가 내준 `&mut Track`에 대입·`swap` |
| 프로젝트 안 `ClipId` 유일 | 깨뜨릴 수 있음 | 시도 1의 결과. 정상 경로(시도 6)로는 못 깨뜨림 |
| 원자성(Err면 프로젝트·히스토리 불변) | 깨뜨릴 수 있음 | 시도 2 |
| undo N번 = 초기, undo 후 redo = 원래 | 히스토리 하나·프로젝트 하나에서는 못 깨뜨림 | 시도 6, 속성 테스트. 시도 7(`TrackId` 중복)에서는 깨진다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| EDT-1 무제한 되돌리기/다시 실행, 단위는 편집 동작 하나, 같은 키 연속만 묶음 | 부분 충족 | 구조는 `history.rs:59-89`, `152-158`. 정상 경로의 정확성은 속성 테스트와 시도 6으로 확인. 그러나 공개 API로 만든 `TrackId` 중복 상태에서 트랙 삭제를 되돌릴 수 없고(시도 7), 원자성이 깨진다(시도 2) |
| 2절 Sequence 녹음 undo(테이크 전체 한 단위) | 충족(엔진 수준) | 묶음 `apply`(`history.rs:59-89`) |
| 2절 Step 모드 undo(노트 하나 = 한 단위, 연속 같은 조작만 묶음) | 충족(엔진 수준) | `history.rs:72`, `history_coalesced_edits_undo_at_once` |
| 4절 P6(무제한, 입력 세션 단위 묶음) | 충족(엔진 수준) | 묶음 `apply` |
| 5.7 되돌리기 묶음 | 충족 | `history.rs:72`. 실패한 apply가 묶기를 끊지 않는 것은 문서화(`history.rs:52-54`) |
| CTL-8 드래그 한 번 = 한 단위 | 충족(엔진 수준) | 묶음 `apply` |
| AI-5 수락 = undo 한 단위 | 충족(엔진 수준) | 묶음 `apply` |
| MS-1 놓는 순간 한 번의 undo 단위 | 충족(엔진 수준) | 묶음 `apply` |

EDT-1이 부분 충족이므로 미충족 1건으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 해당 없음. `core` 편집 경로만 다룬다. `TrackId::allocate`·`ClipId::allocate`는 `AtomicU64::fetch_add`(락 없음).
- 제품 코드의 `unwrap`/`expect`: 없음. `edit.rs:321` `unwrap_or_default`, `track.rs` `unwrap_or_else(ClipId::allocate)`는 패닉하지 않는다.
- usize 뺄셈(`history.rs:70`, `104`, `133`, `155`): 저장한 `bytes`만 빼므로 언더플로하지 않는다(리뷰 1 시도 8과 같은 구조).
- 에러가 삼켜지는 곳: `history.rs:192-195`. 되돌리기 중 역연산이 실패하면 원래 오류와 역연산 오류를 버리고 `HistoryInconsistent`만 돌려준다. 주석(「위조된 역연산이 끼어들 수 없다」)의 전제가 시도 1·2로 깨진다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 프로젝트 안 `TrackId` 유일성을 공개 API로 깨뜨릴 수 있다. `track_mut`/`track_by_id_mut`가 내준 `&mut Track`에 다른 트랙의 복제본을 대입하거나 `std::mem::swap`하면 `insert_track`의 검사를 우회한다. 결과로 `ClipId`가 두 트랙에 겹치고(시도 1), 실패 묶음이 `Err(HistoryInconsistent)`와 함께 트랙을 지운 채 남기며(시도 2, 원자성 위반·데이터 손실), undo가 `Ok(true)`로 아무것도 되돌리지 않거나 트랙 삭제 undo가 영구 실패한다(시도 7). 리뷰 1 지적 6의 수정 근거(「같은 `TrackId`를 거부하므로 undo는 올바른 트랙을 찾거나 `Err`」)가 성립하지 않는다. `project.rs:56`의 주석 「트랙은 자기 불변식을 스스로 지키므로 `&mut Track`을 내줘도 된다」는 트랙 id가 프로젝트 불변식이 된 지금 맞지 않다. 테스트 `track_mut_assignment_cannot_duplicate_track_id`, `duplicated_track_id_breaks_batch_atomicity` | `project.rs:56-59`, `project.rs:75-77`, `project.rs:86`, `history.rs:190-195` |
| 2 | 차단 | 역편집을 다른 맥락에 적용하는 경로(리뷰 1 지적 1)가 남아 있다. `History`가 `Clone`이고 한 `Project`에 묶여 있지 않아, 모든 편집을 히스토리로 하더라도 복제한 히스토리(또는 같은 프로젝트에 쓴 두 번째 히스토리)의 undo가 지금 상태와 맞지 않는 역연산을 `Ok`로 적용한다. 분할을 가로지른 적이 없는 노트가 숨김 꼬리로 되살아나고(시도 3, 결정 Q1 위반), 분할 undo가 다른 히스토리로 넣은 노트를 지운다(시도 4, 데이터 손실). `History` 문서 「역편집은 히스토리 안에만 있고 밖으로 나오지 않는다」의 보장이 `Clone` 때문에 실제로는 없다. 테스트 `cloned_history_undo_does_not_create_tail_without_split`, `undo_split_after_note_from_other_history_keeps_note_or_fails` | `history.rs:25-29`, `track.rs:452-462`, `clip.rs:123-137`, `edit.rs:348-351` |
| 3 | 수정 | 히스토리 밖 변경 뒤 undo가 `Ok`를 돌려주면서 모델을 어긋나게 바꾼다. 분할 undo는 경계·끝만 맞으면 `original`로 덮어써 오른쪽 클립의 새 노트를 지우고, 노트 삭제 undo는 짧아진 클립에 꼬리를 만든다(시도 5). `History` 문서(「`Err`를 돌려줄 수 있다. 그때도 프로젝트는 바뀌지 않고」)와 보고서 「알려진 한계」(「실패해도 상태는 그대로다」)는 `Ok`인 오적용을 언급하지 않는다. 분할 undo에서 지금 두 클립이 `original`을 그 경계에서 나눈 결과와 같은지(노트·이름·색 포함) 비교하고, 노트 되살리기에서 꼬리 허용 조건을 좁히거나, 최소한 문서를 사실대로 고친다. 지적 2를 고치면 대부분 함께 해결된다. 테스트 `undo_after_outside_shrink_does_not_create_tail` | `history.rs:23-27`, `track.rs:449-462`, `clip.rs:121-137`, `docs/reports/s0-t4/report.md:60` |
| 4 | 수정 | `compile_fail` 예 1(`let _ = keyscore_core::Edit::UnsplitClip;`)은 지키려는 것을 지키지 못한다. 존재하는 구조체 변형도 값으로 쓰면 E0533으로 컴파일되지 않으므로, `UnsplitClip { .. }`이 공개 변형으로 돌아와도 이 예는 통과한다(시도 8). 필드를 채워 생성하거나 `matches!(e, Edit::UnsplitClip { .. })`처럼 패턴으로 써야 한다. `RestoreNote`, `InsertClip { id }`에 대한 예도 없다 | `edit.rs:30-33` |
| 5 | 수정 | 보고서 「알려진 한계」가 개정 전 설계를 적고 있다: 「트랙은 인덱스로 가리킨다 … 히스토리 밖에서 트랙 순서를 바꾸면 이후 undo가 엉뚱한 트랙을 가리킬 수 있다」. 지금은 `TrackId`로 가리키며, 엉뚱한 트랙 적용은 지적 1의 경로로만 생긴다 | `docs/reports/s0-t4/report.md:60`, `docs/reports/s0-t4/report.md:62` |
| 6 | 제안 | 리뷰 1 시도 2·3의 개정 테스트 이름 `handmade_unsplit_then_undo_restores_initial`, `handmade_unsplit_in_failing_batch_leaves_project_unchanged`는 손으로 만든 값을 더 이상 쓰지 않으므로 내용과 맞게 바꾸는 편이 낫다. 개정 자체는 정당하다(2절) | `crates/keyscore-core/tests/review_s0_t4.rs:98`, `crates/keyscore-core/tests/review_s0_t4.rs:121` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 2건(공개 `&mut Track`으로 `TrackId`·`ClipId` 유일성과 원자성이 깨짐, `History::clone`·히스토리 여럿으로 역편집이 다른 맥락에 적용되어 분할 없는 꼬리와 데이터 손실이 생김). 모두 공개 API만으로 재현되며 `crates/keyscore-core/tests/review_s0_t4_r2.rs`의 5개 테스트가 실패한다. 리뷰 1의 차단 4건은 정상 경로에서는 해결되었다(리뷰 1 테스트 4개 통과, 시도 6 통과). 남은 문제는 같은 설계의 두 전제(「프로젝트 안 `TrackId` 유일」, 「역편집은 만든 맥락에만 적용」)를 타입이 강제하지 않는 데서 온다. 범위 안에서 고칠 수 있으므로 범위 재정의는 아니다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(5개 실패, 1개 통과, 임시 실험 2개 후 삭제)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`core`의 clip·edit·error·history·lib·note·project·track, 테스트, core README, 계획·보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 1 테스트 1–3은 API 제거로 개정, 2절에서 판단)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트 결과

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과(리뷰 테스트 포함)
- `cargo test --workspace --no-fail-fast`: 기존 테스트 전부 통과(core lib 59, history 5, model_invariants 4, review_s0_t2 계열 13, review_s0_t3 계열 10, review_s0_t4 4, doc-test `compile_fail` 2). 리뷰 테스트 `review_s0_t4_r2` 5개 실패(의도한 깨뜨리기), 1개 통과
- `cargo deny check`: 통과(advisories, bans, licenses, sources ok)
- `scripts/check-deps.sh`: 통과
- `scripts/check-deps.test.sh`: 11 통과
- `scripts/bench-check.test.sh`: 8 통과
