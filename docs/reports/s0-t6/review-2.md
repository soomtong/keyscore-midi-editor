# 적대적 리뷰: S0-T6 `.ksq` 직렬화 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(리뷰 전용 서브 에이전트, 구현 컨텍스트 없음)
- 대상 커밋: `5563ea9` (브랜치 `s0/t6-ksq`). 1회차 이후 변경은 `c7b1984..5563ea9`의 커밋 6개. 코드는 `e2acec2`(테스트), `5fea50c`, `a2a8f77`, `d3844b1`(테스트), 나머지 둘은 문서

## 1. 코드만 읽고 적은 「내가 검사할 것」

`c7b1984..HEAD`의 제품 코드(`ksq.rs`, `clip.rs` `restore_notes`, `tempo.rs`·`meter.rs` `from_sorted`, `project.rs`·`track.rs` 모르는 키 필드, `Cargo.toml`)만 읽고 적었다.

1. `restore_notes`가 실패하면 클립이 그대로인가(전부 검사한 뒤 붙이는가). 오류 경로의 번호가 정렬 뒤 번호가 아니라 파일 안 번호인가.
2. 한 번 정렬(`sort`)한 결과가 `insert_note`를 아무 순서로 부른 결과와 같은가. `Note`의 `Ord`가 모든 필드(모르는 키 문자열 포함)를 보는가(같지 않은 두 노트가 같은 순위면 순서가 입력에 따라 달라진다).
3. `from_sorted`는 순서를 검사하지 않고 `rebuild`의 `span = 다음 틱 - 앞 틱`이 u64 뺄셈이다. 읽기 쪽 `check_change_order`가 같은 조건(첫 항목 0, 엄격 증가)을 보장하는가. 결과 맵이 `set`을 차례로 부른 맵과 캐시까지 같은가.
4. `key_extra`·`port_extra`가 편집과 만날 때: 조성을 「없음」으로 바꿨다 undo, 포트를 가상으로 바꿨다 undo, 다른 이름의 포트·다른 조성으로 바꿨을 때 옛 모르는 키가 새 값에 붙는가.
5. 모르는 키 필드가 히스토리 메모리 상한 계산(`approx_bytes`)에 들어가는가. 분할은 원래 클립을 복제해 히스토리에 둔다.
6. `resolve_symlink`: 자기 순환·상호 순환, 다른 디렉터리를 거치는 상대 링크 사슬, 대상 없는 링크, 디렉터리를 가리키는 링크.
7. 권한 복사: 읽기 전용(0444) 원본에서 쓰기가 실패하는가. 권한을 내용을 쓴 뒤에 옮기므로, 0600 원본의 내용이 그 사이 더 넓은 권한의 임시 파일에 있다.
8. gzip 상한: `take(limit + 1)` 경계, 상한 안에서도 JSON `Value` 트리로 부풀어 메모리를 크게 쓰는가.
9. `float_roundtrip`이 작업 공간 전체에 켜졌는가(기능 통합).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. `restore_notes` 실패 시 불변·파일 번호 | 일부 | `ksq_invalid_files_report_where`가 경로를 보지만 정렬 순서와 파일 순서가 다른 사례는 없다 |
| 2. 정렬 결과 = 삽입 결과 | 없음 | 1회차 시도 12는 시간만 본다 |
| 3. `from_sorted` = `set` 반복 | 없음 | |
| 4. 모르는 키 + 편집·undo | 없음 | |
| 5. 히스토리 상한과 모르는 키 | 없음 | |
| 6. 링크 순환·사슬·대상 없음·디렉터리 | 없음 | 단순 링크 하나(1회차 테스트)만 |
| 7. 0444 원본, 임시 파일 권한 창 | 없음 | `ksq_write_file_keeps_existing_permissions`는 결과 권한만 본다 |
| 8. gzip 상한 경계 | 있음(내부 함수) | `take`를 지워도 길이 검사가 남아 테스트가 통과한다. 즉 「메모리를 상한까지만 쓴다」는 검사되지 않는다 |
| 9. `float_roundtrip` | 있음 | 1회차 테스트와 새 속성 테스트 |
| 제안 10 상대 경로 fsync | 있음, 검사 안 됨 | `ksq_write_file_relative_path_works`는 수정 전 코드에서도 통과한다(fsync 여부를 볼 수 없다). 기록만 한다 |

## 3. 깨뜨리기 시도

새 테스트는 `crates/keyscore-core/tests/review_s0_t6_r2.rs`에 있고 실행했다. 「실험」 표시는 `/tmp` 사본에서 실행하고 지운 것이다.

### 3.1 1회차 지적의 해결 확인 (`/tmp` 사본에서 수정을 되돌림)

| 지적 | 되돌린 것 | 실패로 바뀐 테스트 | 판단 |
| --- | --- | --- | --- |
| 1 (차단) | `set_key_extra`·`set_port_extra` 호출 제거 | 리뷰 1 `…key_object…`, `…port_object…`, 구현자 `ksq_round_trip_with_unknown_keys_everywhere` | 해결 |
| 2 (차단) | `float_roundtrip` 제거 | 리뷰 1 `ksq_unknown_float_write_read_write_is_identical`, `…_for_known_hard_cases`, 구현자 `…unknown_keys_everywhere` | 해결 |
| 3 | `from_sorted` 대신 항목마다 `set` | 리뷰 1 `ksq_many_tempo_changes_read_in_linear_time` | 해결 |
| 4 | `restore_notes` 대신 노트마다 `restore_note` | 리뷰 1 `ksq_reverse_ordered_notes_read_in_linear_time` | 해결 |
| 5 | `resolve_symlink` 제거 | 리뷰 1 `ksq_write_file_through_symlink_updates_target` | 해결 |
| 11 | 지적 1 또는 2를 각각 되돌림 | `ksq_round_trip_with_unknown_keys_everywhere`가 두 경우 모두 실패 | 해결 |
| 제안 7 | `set_permissions` 제거 | `ksq_write_file_keeps_existing_permissions` | 받음. 단 새 결함(지적 1) |
| 제안 8 | `take`와 길이 검사 제거 | `ksq::tests::ksq_gzip_larger_than_limit_is_rejected` | 받음. 2절 8 참고 |
| 제안 10 | (되돌려도 실패하는 테스트 없음) | 없음 | 코드로 확인(`ksq.rs` 816-820). 테스트는 검사하지 못한다 |

제안 6은 결정 요청(`2026-10-12-ksq-newer-version-save.md`)과 S2-T7 이월로, 제안 9는 「기록만」으로 처리되었다. 실험으로 gzip 뒤 쓰레기 바이트가 여전히 오류 없이 읽힘을 확인했다. 거절 이유(읽기 결과에 영향 없음, 다시 저장하면 사라짐)를 받아들인다.

### 3.2 새 코드 깨뜨리기

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 0600 원본에 200,000노트 파일을 20번 저장하며, 관찰 스레드가 내용이 있는 임시 파일의 권한을 본다 | 깨짐. 5회 실행 5회 모두 `.secret.ksq.tmp-… 644`를 관찰. 임시 파일을 umask 기본 권한으로 만들고 내용을 다 쓴 뒤에야 권한을 옮긴다. `/tmp` 사본에서 `set_permissions`를 `write_all` 앞으로 옮기면 3회 모두 통과(크기 0인 순간은 보지 않으므로 고친 뒤 흔들리지 않는다) | `ksq_write_file_temp_never_wider_than_original_mode` |
| 2 | 1 MiB 모르는 키가 든 클립을 `Document`에서 분할 | 깨짐. 히스토리가 그 클립을 복제해 보관하는데 `history_bytes()`가 184바이트. `clip_bytes`·`track_bytes`·`note_heap_bytes`가 모르는 키(클립·트랙·port·노트)를 세지 않는다. 큰 모르는 키가 든 파일에서 분할을 반복하면 히스토리 상한이 실제 메모리를 막지 못한다. `/tmp` 사본에서 `clip_bytes`에 모르는 키 크기를 더하면 통과 | `history_bytes_count_unknown_keys_of_split_clip` |
| 3 | 임의 틱(최대 u64/64 간격)·템포·박자표 512사례(실험 8배)로 읽은 맵과 `set`으로 만든 맵 비교 | 못 깨뜨림. 캐시까지 같다 | `ksq_read_maps_equal_maps_built_with_set` |
| 4 | 같은 시작 틱 노트를 파일에 섞인 순서로 적고, `insert_note`로 만든 클립과 비교 + 바이트 왕복 512사례(실험 8배) | 못 깨뜨림. `Note`의 `Ord`가 모든 필드를 본다(note.rs:139-160) | `ksq_notes_in_any_file_order_read_as_note_order` |
| 5 | 정렬하면 앞으로 오는 잘못된 노트, 잘못된 노트 둘 | 못 깨뜨림. 파일 안 첫 번호(`notes[2]`)를 가리키고, 검사를 모두 끝낸 뒤 붙인다(clip.rs:145-162) | `ksq_bad_note_error_points_to_file_index` |
| 6 | `SetKey(None)` → 저장 텍스트 → undo | 못 깨뜨림. 모르는 키가 돌아온다(모델의 `key_extra`는 그대로 있었다) | `ksq_key_extra_returns_after_set_key_none_undo` |
| 7 | `SetTrackProp(Port(Virtual))` → undo | 못 깨뜨림. 위와 같다 | `ksq_port_extra_returns_after_set_port_virtual_undo` |
| 8 | 실험: `{"named": "IAC Bus 1", "uid": 12345}`를 읽고 포트를 `Named("Other")`로, 조성 D장조를 A단조로 바꿔 저장 | 깨짐(의미). `{"named": "Other", "uid": 12345}`, `{"tonic": 9, "mode": "minor", "accidentals": "sharps"}`로 쓴다. 가상/「없음」으로 바꿔 저장한 뒤에도 메모리에는 남아 있다가, 다시 다른 이름의 포트로 바꾸면 `{"named": "Third", "uid": 12345}`로 되살아난다. 보고서의 「그 값으로 바뀐 경우에 한해 버려진다」는 파일 기준으로만 맞다(지적 4) | 없음 |
| 9 | 자기 순환 링크, 서로 가리키는 두 링크 | 못 깨뜨림. 오류, 링크 그대로, 임시 파일 없음 | `ksq_write_file_symlink_loop_is_error` |
| 10 | `a/link → ../b/link2 → real.ksq` 사슬 | 못 깨뜨림. 실제 파일만 바뀌고 두 링크가 남는다. 상대 대상을 링크의 부모에 이어 붙이고 정규화하지 않으므로 커널 해석과 같다 | `ksq_write_file_relative_symlink_chain_updates_final_target` |
| 11 | 대상 없는 링크, 디렉터리를 가리키는 링크 | 못 깨뜨림. 앞은 대상 자리에 만들고 링크가 남는다. 뒤는 오류이고 디렉터리·링크가 그대로다 | `ksq_write_file_dangling_and_directory_symlinks` |
| 12 | 0444 원본에 두 번 저장(gzip) | 못 깨뜨림. 성공하고 0444가 남는다. 권한 복사는 이미 열린 핸들에 하므로 쓰기를 막지 않는다. 다만 읽기 전용 파일이 아무 경고 없이 바뀐다(지적 6) | `ksq_write_file_over_read_only_file_keeps_mode` |
| 13 | 실험: 31 KB gzip(`{"x": [0,0,…]}` 1,600만 원소, 풀면 32 MB) 읽기, 릴리스 | 상한 안에서 최대 상주 메모리 583 MB. 풀린 텍스트 대비 약 18배(`Value` 32바이트/원소). 상한 512 MiB까지 풀리는 약 500 KB 파일이면 약 9 GB를 쓸 수 있다(지적 5) | 없음 |
| 14 | 실험: 1회차 퍼즈 3종과 실수 왕복, 구현자 왕복 속성 테스트 2종, 이번 속성 테스트 2종을 사례 수 8배로 릴리스 실행 | 못 깨뜨림 | 없음(사례 수만 바꾼 실행) |
| 15 | 실험: 브랜치가 지운 `.proptest-regressions` 3개를 되살려 해당 테스트 실행 | 저장된 사례는 모두 통과. 지운 것 자체가 범위 밖 변경이다(지적 3) | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | 읽기가 `Track::insert_clip`을 거친다. 퍼즈에서 깨지지 않음 |
| 노트 시작은 클립 안, 절대 끝 u64 안 | 아니오 | `restore_notes`가 모든 노트를 검사한 뒤 붙인다(clip.rs:146-158) |
| 클립 안 노트는 `Note` 전체 순서 | 아니오 | 시도 4 |
| 템포·박자표 맵: 틱 0 항목, 엄격 증가, 캐시 | 아니오 | `check_change_order`(ksq.rs:416-435)가 `from_sorted`의 전제를 보장한다. `from_sorted`는 `pub(crate)`이고 호출자는 읽기 둘뿐. 시도 3 |
| 쓰기 → 읽기 → 쓰기 = 같은 바이트 | 아니오 | 실수(지적 2 해결), 퍼즈 8배 |
| 읽기 → 쓰기가 모르는 키를 보존 | 아니오 | `key`·`port` 포함(지적 1 해결). 편집 뒤 의미는 지적 4 |
| 히스토리 메모리 상한(S0-T4) | 예 | 모르는 키가 큰 파일을 읽어 분할·지우기(시도 2) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-1 JSON, 평문 기본, gzip `.ksqz` | 충족 | ksq.rs:66-103, 744-770 |
| IO-1 노트 `[Δt, 길이, 음높이, 벨로시티]` + 선택 객체 | 충족 | 결정 `2026-10-12-ksq-note-format.md` B. ksq.rs:571-628 |
| IO-1 헤더 `format_version` | 충족 | 상위 버전 저장 정책은 결정 대기, S2-T7 이월(`plan/stages/12-S2-app-shell.md`) |
| IO-1 모르는 키 보존 | 충족 | 최상위·`key`·트랙·`port`·클립·노트 속성. ksq.rs:305-321, 128-136, 437-458, 516-525. 실수 Cargo.toml:29 |
| IO-1 없는 키 기본값 | 충족 | 1회차와 같음 |
| 9.5 델타 배열, 하위 호환 규칙 | 충족 | 위와 같음 |
| LYR-1(.ksq 부분) | 충족 | 1회차와 같음 |
| 단계 게이트: 10만 노트 읽기 < 100 ms | 충족 | 노트 정렬 한 번(clip.rs:159-160). 1회차 역순 테스트 통과. 보고서 벤치 14.4 ms |
| 단계 게이트: 상위 버전 경고 | 충족 | ksq.rs:359-369 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 스케줄러 코드를 건드리지 않는다.
- 제품 코드의 `unwrap`/`expect`: 새 코드에 없음. `indices[at]`(ksq.rs:565)는 `at`이 `restore_notes`가 받은 벡터의 번호라 범위 안이다.
- 에러가 삼켜지는 곳:
  - ksq.rs:808 원래 파일의 `metadata` 실패는 「새 파일」로 보고 넘어간다. 의도(주석 있음).
  - ksq.rs:820 디렉터리 fsync 실패 무시(의도, 주석 있음).
  - gzip 상한 경계에서 잘린 위치가 UTF-8 문자 중간이면 「상한을 넘는다」 대신 「gzip을 풀 수 없다: …UTF-8…」 오류가 된다. 오류이긴 하므로 지적하지 않는다(코드로 확인, 실행하지 않음).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 권한 복사(제안 7 반영)를 내용을 모두 쓴 뒤에 한다. 0600 원본을 저장하는 동안 같은 내용이 umask 기본(보통 0644) 임시 파일에 있고, 그 사이 다른 사용자가 연 핸들은 권한이 바뀐 뒤에도 읽을 수 있다. 권한을 옮기는 목적(남이 못 읽게)과 어긋난다. `write_all` 전에 권한을 옮기거나 처음부터 그 권한으로 만들어야 한다. 재현: 시도 1 | crates/keyscore-core/src/ksq.rs:806-810 |
| 2 | 수정 | 히스토리 크기 추정이 이 Task가 더한 모르는 키(클립·트랙·`port`·노트)를 세지 않는다. 분할은 원래 클립을 복제해 보관하므로 1 MiB 모르는 키 클립 분할이 184바이트로 셈된다. 큰 모르는 키가 든 파일(외부 도구가 쓴 것)에서 편집을 반복하면 S0-T4의 히스토리 메모리 상한이 지켜지지 않는다. 재현: 시도 2 | crates/keyscore-core/src/edit.rs:470-488 |
| 3 | 수정 | `e2acec2`가 `main`에 있던 `.proptest-regressions` 파일 3개(`model_invariants`, `review_s0_t2`, `tempo_map`)를 지웠다. 커밋 본문에 이유가 없고 Task 범위 밖이다. 저장된 실패 사례는 「기존 테스트」라서 지우면 그 사례가 더는 먼저 실행되지 않는다(지금은 모두 통과함을 실험으로 확인). 되살려야 한다 | crates/keyscore-core/tests/*.proptest-regressions |
| 4 | 제안 | `key_extra`·`port_extra`가 알려진 값과 묶여 있지 않다. 포트를 다른 이름으로 바꾸면 옛 포트의 `uid`가 새 포트에 붙어 저장되고, 가상으로 바꿔 저장한 뒤에도 메모리에 남아 다른 포트로 바꾸면 되살아난다(조성도 같다). 새 버전 앱이 `uid`를 이름보다 우선하면 잘못된 포트로 보낼 수 있다. undo로 돌아오는 것(시도 6·7)과 맞바꿈이라 정책이 필요하다. REQ에 규칙이 없으므로 결정 요청을 권한다. 적어도 보고서의 「그 값으로 바뀐 경우에 한해 버려진다」는 「파일에 쓰지 않을 뿐 모델에는 남고, 다른 값으로 바뀌면 다시 쓰인다」로 고쳐야 한다 | crates/keyscore-core/src/track.rs:206, project.rs:67, docs/reports/s0-t6/report.md:71 |
| 5 | 제안 | gzip 상한은 풀린 텍스트만 막는다. JSON `Value` 트리가 텍스트의 약 18배로 부풀어 상한 안의 약 500 KB 파일로 약 9 GB를 쓸 수 있다(시도 13). 상한을 낮추거나(10만 노트가 3 MB 남짓) 원소 수 상한을 두는 것을 권한다 | crates/keyscore-core/src/ksq.rs:751-770 |
| 6 | 제안 | 읽기 전용(0444) 파일에 저장해도 rename으로 바꿔치기해 성공한다(시도 12). 1회차 전에도 같았다. 저장 거부·확인은 S2-T7 저장 흐름에서 정하면 된다 | crates/keyscore-core/src/ksq.rs:783-799 |
| 7 | 제안 | `a2a8f77`이 지적 1·3·4·5와 제안 7·8·10을 한 커밋에 담았다. 프로토콜 [7] 「지적 사항마다 커밋 하나」와 다르다(본문에 이유는 있음). 이력은 고치지 않아도 되고, 다음 수정부터 지킨다 | |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

1회차 차단 2건과 수정 4건은 모두 해결되었고, 되돌리면 해당 테스트가 실패함을 확인했다. 새로 깨진 것이 둘(지적 1·2)이고 범위 밖 삭제(지적 3)가 하나다. 모두 이 Task 안에서 작게 고칠 수 있다. `review_s0_t6_r2.rs`의 실패 2건이 통과하고 지운 파일이 되살아나면 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(11개, 실패 2개)
- [x] REQ ID를 하나씩 대조했다
- [ ] `git diff`에 Task 범위 밖의 변경이 없다 — `.proptest-regressions` 3개 삭제(지적 3). `plan/stages/12-S2-app-shell.md` 한 줄 추가는 결정 이월이라 범위 안
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(테스트 함수 기준. 저장된 회귀 사례 삭제는 지적 3)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음. `serde_json` 기능만 추가, `cargo deny check` 통과)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트(리뷰어 실행, 리뷰 테스트 포함)

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과
- `cargo test --workspace --no-fail-fast`: 158 통과, 2 실패(모두 `review_s0_t6_r2.rs`, 지적 1·2의 재현). 구현자 테스트와 리뷰 1 테스트는 전부 통과
- `cargo deny check`: 통과(advisories·bans·licenses·sources ok. `bzip2-1.0.6` 허용 항목 미사용 경고는 기존)
- `scripts/check-deps.sh`: ok. `scripts/check-deps.test.sh` 11/11. `scripts/bench-check.test.sh` 8/8
