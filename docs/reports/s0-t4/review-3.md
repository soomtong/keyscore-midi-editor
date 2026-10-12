# 적대적 리뷰: S0-T4 undo 엔진 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `dab76f1` (브랜치 `s0/t4-undo`). 2회차 리뷰 커밋 `d0c3788` 이후 3커밋(`f09ea9a`, `b8ba908`, `dab76f1`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff d0c3788`의 `document.rs`, `history.rs`, `edit.rs`, `project.rs`, `lib.rs`와 변경되지 않은 `track.rs`, `clip.rs`만 읽고 적었다.

1. `Document`가 `&mut Project`를 내주는 다른 길(`DerefMut`, `AsMut`, 공개 필드, `History` 재수출)이 남았는가. `History`가 정말 crate 밖에서 이름조차 쓸 수 없는가.
2. `Edit::InsertTrack`은 `Project::insert_track`의 `TrackId` 검사만 거친다. 넣는 트랙의 `ClipId`가 다른 트랙의 `ClipId`와 겹치는지는 보지 않는다. 다른 `TrackId`인데 같은 `ClipId`를 가진 트랙을 공개 API로 만들 수 있으면 문서 안에서 `ClipId` 유일성이 깨진다. `ClipId`는 전역 카운터로만 발급되고(`clip.rs:13`) 공개 생성자가 없으므로, 같은 `ClipId`는 트랙 `clone`(같은 `TrackId`)으로만 복사될 것이다. 이 추론을 실제로 확인한다(옛 스냅숏 트랙, 복제 문서에서 분할한 트랙, `into_project`).
3. 옛 스냅숏의 트랙을 「지우고 넣기」 한 묶음으로 바꿔치기하면 같은 `TrackId` 아래 내용이 통째로 바뀐다. 이후 undo의 역편집(`InsertClipWithId`, `UnsplitClip`, `RestoreNote`)이 그 트랙에 맞는가. LIFO가 지켜지면 맞아야 한다.
4. `Document::new`의 검사는 `TrackId` 중복을 먼저 본다. `ClipId` 검사(`DuplicateClipId`)에 도달하는 입력이 있는가. 있다면 그 입력은 `TrackId`가 모두 다른데 `ClipId`가 겹치는 프로젝트이고, 2가 가능하다는 뜻이다.
5. 문서 복제본 둘은 아무것도 공유하지 않는가. 같은 묶기 키를 두 문서에 이어 쓰면 서로 다른 문서의 단위와 묶이지 않는가.
6. 숨김 꼬리가 u64 끝 근처에 있을 때 `MoveClip`·undo·redo가 꼬리의 절대 끝을 u64 밖으로 밀지 않는가.
7. 실패한 `apply`가 redo 스택, 묶기 상태, `history_bytes`를 바꾸지 않는가.
8. 메모리 상한: `undo`도 `enforce_limit`를 부른다. undo로 생긴 redo 단위가 크면 오래된 undo 단위가 버려져, undo 도중에 처음 상태로 돌아갈 수 없게 되는가.
9. `compile_fail` 예 4개가 「그 항목이 없다」는 이유 하나로만 실패하는가(대조군은 컴파일되는가).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. `&mut Project` 경로 없음, `History` 비공개 | 부분 | `compile_fail` 예가 `project_mut` 부재만 본다. `History` 비공개는 예가 없다(실험에서 E0433 확인) |
| 2. 다른 `TrackId`·같은 `ClipId` 트랙 삽입 | 없음 | 속성 테스트(`tests/history.rs`)는 새 트랙만 넣고 옛 트랙을 다시 넣지 않는다 |
| 3. 옛 판 트랙 바꿔치기 뒤 undo·redo | 없음 | |
| 4. `Document::new`의 `ClipId` 검사 도달 | 없음 | 단위 테스트는 `TrackId` 중복만 본다. 2의 결론상 공개 API로는 도달하지 않는 방어 검사다 |
| 5. 문서 복제본 간섭, 묶기 키 | 부분 | `cloned_history_undo_does_not_create_tail_without_split`(리뷰 2 개정)이 복제본 undo가 원본을 바꾸지 않음을 본다. 번갈아 편집·묶기는 없다 |
| 6. u64 끝 근처 꼬리 | 부분 | S0-T2 리뷰 테스트가 `Track` 수준에서 본다. 문서 undo·redo를 거치는 경우는 없다 |
| 7. 실패한 `apply`의 redo·바이트 보존 | 부분 | `history_failed_transaction_leaves_project_unchanged`는 프로젝트와 `undo_len`만 본다 |
| 8. undo가 오래된 undo 단위를 버림 | 있음(의도된 동작으로) | `history_undo_enforces_limit`(리뷰 1 제안 8 반영). 문서화는 `enforce_limit` 주석뿐 |
| 9. `compile_fail` 예의 실패 이유 | 있음 | 보고서가 대조군 확인을 적었다. 이번에 다시 확인했다(시도 7) |

리뷰어 테스트 개정(`review_s0_t4.rs`, `review_s0_t4_r2.rs`)에 대한 판단: 정직하다. 테스트 수는 그대로(4개, 6개)이고 `#[ignore]`나 삭제가 없다. 원래 시도가 쓰던 `History`, `&mut Project`가 공개 API에서 사라져 그대로는 컴파일되지 않으므로 바꿀 수밖에 없다. 각 시도의 목표는 다음과 같이 보존되었다.

- 2회차 시도 1(`TrackId` 중복): 전제(`Project` 수준에서는 여전히 중복을 만들 수 있다)를 숨기지 않고 `!track_ids_unique`로 명시한 뒤, 그 프로젝트를 `Document::new`가 거부하는지 본다. 깨진 경로가 실제로 막혔는지를 정확히 묻는다.
- 시도 2(원자성): 중복 상태가 문서에 들어갈 수 없으므로, 문서 안에서 중복을 만들려는 묶음이 원자적으로 실패하는지로 바꿨다. 같은 목표의 남은 공개 경로를 노린다.
- 시도 3(복제한 히스토리): `Document::clone`으로 같은 공격을 그대로 재현하고, 원본·복제본이 각자 맞게 되돌아가는지까지 본다. 원래보다 강하다.
- 시도 4·5(다른 히스토리, 히스토리 밖 변경): 그 경로 자체가 컴파일되지 않으므로(`compile_fail` 예 1, `History` 비공개) 같은 모양의 편집을 문서로 하는 정상 경로 확인으로 바뀌었다. 공격의 목표는 컴파일 오류로 지켜지고, 테스트는 회귀 방지 역할이다. 다만 시도 2·4·5의 테스트 이름(`duplicated_track_id_breaks_batch_atomicity`, `undo_split_after_note_from_other_history_keeps_note_or_fails`, `undo_after_outside_shrink_does_not_create_tail`)은 이제 내용과 맞지 않는다(제안 2).

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 옛 스냅숏의 A, 복제 문서에서 분할해 새 `ClipId`를 받은 A를 `InsertTrack`으로 넣기. A가 있을 때 넣기, 「지우고 넣기」 한 묶음으로 옛 판 바꿔치기(두 번), B를 지우고 그 자리에 A의 옛 판 넣기. 각 단계 뒤 트랙 사이 `ClipId` 유일 등 불변식, 끝에서 undo 전부 = 초기, redo 전부 = 최종 | 못 깨뜨림. 같은 `ClipId`는 같은 `TrackId`의 트랙에만 있어 `DuplicateTrackId`로 막히고, 바꿔치기 뒤 undo·redo도 정확하다 | `reinserting_stale_track_cannot_share_clip_id_and_round_trips` |
| 2 | `into_project`로 꺼낸 프로젝트에 복제 문서(분할로 새 id를 받음)의 트랙을 `track_mut` 대입·`push_track`으로 섞어 `Document::new` | 못 깨뜨림. 다른 자리에 대입하면 `DuplicateTrackId`, 같은 id 자리 교체는 정상 구성이며 불변식을 지킨다. `DuplicateClipId`에 도달하는 구성은 찾지 못했다(1절 4) | `document_new_rejects_every_mix_of_tracks_from_cloned_documents` |
| 3 | 복제 문서 둘을 같은 묶기 키로 번갈아 편집·undo·redo | 못 깨뜨림. 각자 자기 단위끼리만 묶이고 서로 영향이 없다 | `cloned_documents_interleaved_do_not_interfere` |
| 4 | `u64::MAX - 4000`의 클립을 꼬리가 남게 분할, 오른쪽 삭제, 끝으로 이동(거부 기대), 꼬리 노트 삭제 + 이동 묶음, undo·redo 전부 | 못 깨뜨림. 꼬리가 넘치는 이동은 `TickOverflow`, 모든 단계에서 절대 끝이 u64 안 | `hidden_tail_near_u64_end_stays_in_range_through_undo_redo` |
| 5 | undo 뒤 묶기 키를 단 묶음(분할 성공 → `RemoveTrack` 두 번)이 실패. 프로젝트·`undo_len`·`redo_len`·`history_bytes` 비교 후 redo | 못 깨뜨림. 모두 그대로이고 redo가 된다 | `failed_apply_keeps_redo_coalescing_and_bytes` |
| 6 | 속성 테스트 512회: 묶음(1–3편집, 묶음 안 실패 포함), 묶기 키, seal, undo, redo, 문서 복제 후 복제본 전부 undo, 옛 스냅숏(복제 문서 포함) 트랙 삽입·바꿔치기, u64 끝 근처 시작점. 상태 스택 모형과 매 단계 비교(undo는 그 단위 직전 상태, redo는 undo 직전 상태), 매 단계 불변식, 실패 묶음은 `HistoryInconsistent`가 아니고 프로젝트·길이·바이트 불변 | 못 깨뜨림. 임시 계측으로 확인한 실행량: 성공 묶음 2749, 실패 묶음 2871, 옛 트랙 재삽입·바꿔치기 성공 831, 묶기 134, undo 1683, u64 끝 근처 클립 상태 10982, 숨김 꼬리 상태 1946(계측 코드는 삭제) | `document_matches_state_stack_model` |
| 7 | `compile_fail` 예 4개를 일반 테스트 파일로 옮겨 오류 확인. 대조군 `matches!(e, Edit::SplitClip { .. })`, `doc.project()`. 추가로 `Edit::InsertClip { id: _, .. }`, `keyscore_core::History::new` | 예 4개 모두 E0599(메서드·변형 없음)로 올바른 이유. 대조군은 컴파일된다. `InsertClip { id }`는 E0026, `History`는 E0433으로 막혀 있으나 문서 예는 없다 | 임시 실험(삭제) |
| 8 | 상한 = 단위 셋(`SetKey` 둘, 노트 1000개 클립 `InsertClip`)의 바이트 + 100. 모두 상한 안(216 ≤ 316). undo 한 번 | 동작 확인. undo 한 번에 `undo_len`이 3 → 1이 된다(redo 단위가 48216바이트라 가장 오래된 undo 단위를 버림). 문서화된 상한 정책(가장 오래된 undo부터 버림, redo는 지우지 않음)대로이며 불변식·원자성은 지켜진다. 문서 `undo` 설명에는 없다(제안 3) | 임시 실험(삭제) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | 시도 1·4·6 |
| 노트 시작은 클립 안 | 못 깨뜨림 | 시도 4·6. `restore_note`는 crate 내부, 역편집은 문서 안 LIFO로만 적용 |
| 숨김 꼬리는 분할로만 생김(결정 Q1) | 못 깨뜨림 | 시도 6의 상태 스택 비교(모든 undo·redo가 전에 있던 상태로만 간다). `Document` 밖 경로는 컴파일되지 않음(시도 7) |
| 문서 안 `TrackId` 유일 | 못 깨뜨림 | `Document::new`(`document.rs:48`), `insert_track`(`project.rs:94`). 시도 1·2·6. `Project` 단독으로는 깨뜨릴 수 있으나 문서화된 구성 단계다(`project.rs:56-59`) |
| 문서 안 `ClipId` 유일(트랙 사이 포함) | 못 깨뜨림 | `ClipId`는 공개 생성자가 없고 같은 `TrackId`의 트랙 복제로만 복사된다. 시도 1·2·6 |
| u64 절대 끝 | 못 깨뜨림 | 시도 4·6 |
| 원자성(Err면 프로젝트·히스토리 불변) | 못 깨뜨림 | 시도 5·6(실패 묶음 2871회, `HistoryInconsistent` 0회) |
| undo N번 = 초기, undo 후 redo = 원래 | 못 깨뜨림(상한 무제한) | 시도 1·4·6. 상한이 있으면 정책상 오래된 단위가 버려진다(시도 8) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| EDT-1 무제한 되돌리기/다시 실행, 단위는 편집 동작 하나, 같은 키 연속만 묶음 | 충족 | `document.rs:84-108`, `history.rs:58-147`. 시도 3·5·6. 「무제한」은 개수 무제한 + 메모리 상한(단계 파일 괄호, `plan.md:15-17`) |
| 2절 Sequence 녹음 undo(테이크 전체 한 단위) | 충족(엔진 수준) | 묶음 `apply`(`document.rs:84`) |
| 2절 Step 모드 undo(노트 하나 = 한 단위, 연속 같은 조작만 묶음) | 충족(엔진 수준) | `history.rs:70`, 시도 3·6의 묶기 |
| 4절 P6(무제한, 입력 세션 단위 묶음) | 충족(엔진 수준) | 묶음 `apply`, `seal`(`document.rs:112`) |
| 5.7 되돌리기 묶음 | 충족 | `history.rs:70`. 실패한 apply가 묶기를 끊지 않음(시도 5) |
| CTL-8 드래그 한 번 = 한 단위 | 충족(엔진 수준) | 묶음 `apply` |
| AI-5 수락 = undo 한 단위 | 충족(엔진 수준) | 묶음 `apply` |
| MS-1 놓는 순간 한 번의 undo 단위 | 충족(엔진 수준) | 묶음 `apply` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 해당 없음. `core` 편집 경로만 다룬다. `TrackId::allocate`·`ClipId::allocate`는 `AtomicU64::fetch_add`(락 없음).
- 제품 코드의 `unwrap`/`expect`: 없음. `document.rs`의 `unwrap`은 모두 `#[cfg(test)]`(133줄 이후) 안이다.
- 에러가 삼켜지는 곳: `history.rs:190-195`(되돌리기 중 역연산 실패 시 `HistoryInconsistent`). 이제 역편집이 문서 밖으로 나가지 않고 히스토리를 따로 쓸 수 없어, 이 갈래는 정상 경로에서 도달하지 않는다. 시도 6에서 0회.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `compile_fail` 예가 `project_mut` 부재만 본다. `History`가 crate 밖에서 보이지 않는다는 것(`keyscore_core::History`)과 리뷰 2 지적 4가 함께 요구한 `Edit::InsertClip { id, .. }` 패턴의 예가 없다. 지금은 둘 다 컴파일되지 않음을 확인했다(E0433, E0026). 회귀 방지로 예를 더하면 좋다 | `crates/keyscore-core/src/document.rs:14-33` |
| 2 | 제안 | 리뷰 2 개정 테스트 중 세 개의 이름이 개정된 내용과 맞지 않는다: `duplicated_track_id_breaks_batch_atomicity`(이제 중복 삽입 거부·원자성 확인), `undo_split_after_note_from_other_history_keeps_note_or_fails`(다른 히스토리 없음), `undo_after_outside_shrink_does_not_create_tail`(히스토리 밖 변경 없음). 개정 자체는 정직하다(2절) | `crates/keyscore-core/tests/review_s0_t4_r2.rs` |
| 3 | 제안 | 상한이 있을 때 `Document::undo`가 성공하면서 오래된 undo 단위를 버릴 수 있다(시도 8: 상한 안의 히스토리에서 undo 한 번에 `undo_len` 3 → 1). 녹음 테이크·붙여넣기처럼 역(클립 삭제)은 작고 정방향(클립 내용)은 큰 편집에서 생긴다. 정책은 문서화되어 있고(`history.rs:149-150`) 범위(메모리 상한 설정) 안이지만, 공개 문서 `Document::undo`에는 없다. 문서에 적거나, S2에서 기본 상한을 정할 때 「redo를 먼저 버린다」 같은 정책을 결정 사항으로 다루는 편이 낫다 | `crates/keyscore-core/src/document.rs:93-100`, `crates/keyscore-core/src/history.rs:149-157` |
| 4 | 제안 | core README의 `TrackId` 항목 「프로젝트 안에서 유일하다(복제본은 같은 프로젝트에 넣을 수 없다)」는 `Project` 단독으로는 사실이 아니다(`track_mut` 대입). 같은 README 불변식 절과 `project.rs:56-58`은 정확하므로 문구만 맞추면 된다(「문서 안에서 유일하다」) | `crates/keyscore-core/README.md:13` |

차단·수정 사항 없음.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2의 차단 2건은 `Document`로 해결되었다. 프로젝트와 히스토리를 한 값이 소유하고 `&mut Project`와 `History`가 밖으로 나오지 않아, 역편집이 다른 맥락에 적용될 경로가 컴파일 단계에서 사라졌다(시도 7). `Project` 수준의 id 중복은 `Document::new`가 거부하고(시도 2), 문서 안에서는 `ClipId`가 같은 `TrackId`의 트랙 복제로만 복사되므로 `InsertTrack`의 `TrackId` 검사만으로 트랙 사이 `ClipId` 유일성이 지켜진다(시도 1). 옛 트랙 재삽입·복제 문서·묶기·실패 묶음·u64 끝을 섞은 상태 스택 모형 비교(시도 6)를 포함해 6개 테스트 모두 깨뜨리지 못했다. 남은 것은 제안 4건이다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(6개 모두 통과 = 못 깨뜨림, 임시 실험 2개와 계측은 삭제)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`core`의 document·edit·history·lib·project, core 테스트, core README, 계획·보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 테스트는 API 제거로 개정, 2절에서 판단. 테스트 수 동일)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트 결과

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과(리뷰 테스트 포함)
- `cargo test --workspace`: 118 통과, 0 실패(core lib 61, history 5, model_invariants 4, review_s0_t2 계열 13, review_s0_t3 계열 10, review_s0_t4 4, review_s0_t4_r2 6, review_s0_t4_r3 6, tempo_map 5, doc-test `compile_fail` 4)
- `cargo deny check`: 통과(advisories, bans, licenses, sources ok. 기존의 unmatched license allowance 경고만)
- `scripts/check-deps.sh`: 통과
- `scripts/check-deps.test.sh`: 11 통과
- `scripts/bench-check.test.sh`: 8 통과
