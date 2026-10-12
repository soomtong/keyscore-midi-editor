# 적대적 리뷰: S0-T6 `.ksq` 직렬화 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(리뷰 전용 서브 에이전트, 구현 컨텍스트 없음)
- 대상 커밋: `9d62624` (브랜치 `s0/t6-ksq`). 2회차 이후 변경은 `dc1df9f..9d62624`의 4개(시드 복원, 임시 파일 권한, 히스토리 크기 추정, plan 기록)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`dc1df9f..HEAD`의 제품 코드(`ksq.rs` `write_then_rename`, `edit.rs` `map_bytes`·`clip_bytes`·`track_bytes`·`note_heap_bytes`)만 읽고 적었다.

1. 권한 설정이 내용 쓰기 앞으로 갔지만, `create_new`는 umask 기본 권한으로 파일을 만든 뒤에 `set_permissions`를 부른다. 그 사이의 빈 파일이 원본(0600)보다 넓은 권한으로 보이는가. 그 사이에 열린 핸들은 권한이 바뀐 뒤에도 유효하다.
2. 0444 원본: 이미 쓰기로 열린 핸들에 권한을 바꾸므로 `write_all`이 막히지 않는가. 원본이 없는 새 파일은 `metadata` 실패로 기본 권한을 유지하는가.
3. `map_bytes`: 빈 맵이면 0, 아니면 직렬화 길이. `to_string` 실패는 `Map<String, Value>`에서 일어날 수 없다(키가 문자열, 값은 유한). `map_or(0, …)`로 삼키지만 도달 불가.
4. 히스토리 크기 추정이 모르는 키를 담는 모든 `Op`를 덮는가: `InsertTrack`(트랙 `extra`·`port_extra`), `InsertClip*`·`UnsplitClip`(클립 `extra`), 노트 `InsertNote`·`RemoveNote`·`RestoreNote`(노트 `extra`).
5. 히스토리 상한과의 상호작용: 상한보다 큰 단위 하나가 들어올 때(최신 하나는 남는 정책), 추정이 커진 만큼 이전 단위가 더 일찍 밀려나는가.
6. `approx_bytes`가 편집마다 모르는 키를 직렬화하므로 비용이 큰가(1 MiB 모르는 키 클립이면 편집마다 1 MiB 직렬화).
7. `.proptest-regressions` 3개가 `main`과 같은가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 빈 임시 파일 권한 창 | 없음 | 리뷰 2 테스트는 「크기 0인 순간은 보지 않는다」고 명시. 같은 이유로 구현자 대응도 이 창을 못 본다 |
| 2. 0444 원본, 새 파일 | 있음 | `ksq_write_file_over_read_only_file_keeps_mode`, 새 파일 저장은 기존 테스트 |
| 3. 직렬화 실패 경로 | 없음 | 도달 불가라 필요 없음 |
| 4. 모든 `Op`의 모르는 키 | 일부 | 클립 분할만 있다(리뷰 2 테스트). 트랙 삭제(`port_extra`)·노트 삭제(노트 `extra`)는 없다 |
| 5. 상한 상호작용 | 없음 | 내부 테스트 `history_memory_limit_keeps_latest_even_if_too_large`가 정책만 본다 |
| 6. 비용 | 없음 | 아래 제안 |
| 7. 시드 복원 | 해당 없음 | `git diff main -- 'crates/keyscore-core/tests/*.proptest-regressions'`가 비어 있음을 확인 |

## 3. 깨뜨리기 시도

새 테스트는 `crates/keyscore-core/tests/review_s0_t6_r3.rs`에 있고 실행했다. 「실험」은 `/tmp` 사본에서 실행하고 지운 것이다.

### 3.1 2회차 지적의 해결 확인

| 지적 | 확인 방법 | 결과 |
| --- | --- | --- |
| 1 (권한 창) | 실험: `set_permissions`를 `write_all` 뒤로 되돌리고 `ksq_write_file_temp_never_wider_than_original_mode` 실행 | 실패로 바뀜. 해결(내용이 쓰인 뒤의 창은 닫힘). 단 빈 파일 창이 남음(지적 1) |
| 2 (히스토리 크기) | 실험: `clip_bytes`의 `map_bytes(clip.extra())` 제거 후 `history_bytes_count_unknown_keys_of_split_clip` 실행 | 실패로 바뀜. 해결 |
| 3 (시드 삭제) | `git diff main -- 'crates/keyscore-core/tests/*.proptest-regressions'` | 비어 있음. 해결 |

### 3.2 새 변경 깨뜨리기

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 0600 원본을 저장하는 동안 관찰 스레드가 임시 파일(`.tmp-`)이 보이는 모든 순간의 권한을 기록, 빈 파일 포함 | 깨짐. 3회 실행 3회 모두 `(길이 0, 권한 0644)`를 관찰. `create_new`가 umask 기본 권한으로 만들고 그 뒤에 `set_permissions`를 부른다. 이 창에 다른 사용자가 연 핸들은 이후 쓰인 내용을 읽는다. 실험: `OpenOptionsExt::mode(원본 권한)`으로 만들면 통과(5회 안정). 2회차 지적 1의 목적(남이 못 읽게)이 아직 닫히지 않았다 | `ksq_write_file_temp_never_wider_than_original_mode_even_when_empty` |
| 2 | `port`에 1 MiB 모르는 키가 든 트랙을 `RemoveTrack` | 못 깨뜨림. 히스토리가 1 MiB 이상으로 센다 | `history_bytes_count_port_unknown_keys_of_removed_track` |
| 3 | 노트 선택 객체에 1 MiB 모르는 키가 든 노트를 `RemoveNote` | 못 깨뜨림 | `history_bytes_count_note_unknown_keys_of_removed_note` |
| 4 | 상한 64 KiB에 1 MiB 모르는 키 클립을 분할 | 못 깨뜨림. 편집은 성공하고 최신 단위 하나는 상한을 넘어도 남는다(기존 정책). undo가 가능하면 원래 프로젝트로 돌아온다. 처음에는 「상한을 넘지 않는다」고 단언했다가 기존 정책 테스트(`…keeps_latest_even_if_too_large`)와 어긋나 단언을 정책에 맞게 고쳤다 | `history_cap_smaller_than_one_unknown_key_clip_keeps_latest_undoable` |
| 5 | 0444 원본 저장, 새 파일 저장(코드 읽기와 기존 테스트) | 못 깨뜨림. 열린 핸들에 권한을 맞추므로 쓰기가 막히지 않는다 | 없음(기존 테스트) |
| 6 | `map_bytes` 0 경로, 직렬화 실패 경로 | 못 깨뜨림. 빈 맵은 0, 실패는 도달 불가(코드로 확인, 실행하지 않음) | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 모델 불변식(겹침, 노트 범위, 채널) | 아니오 | 2회차 이후 모델 코드 변경 없음 |
| 히스토리 메모리 상한 | 아니오(2회차 지적 해결) | 시도 2·3. 최신 단위 하나가 상한을 넘는 것은 기존 정책 |
| 임시 파일이 원본보다 넓은 권한으로 존재하지 않음 | 예(창이 짧음) | 시도 1 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-1 (JSON, gzip, 노트 형식, 모르는 키, 기본값, `format_version`) | 충족 | 2회차와 같음. 이후 변경은 권한·추정 크기뿐 |
| 9.5 델타 배열·하위 호환 | 충족 | 같음 |
| LYR-1(.ksq 부분) | 충족 | 같음 |
| 단계 게이트: 10만 노트 읽기 < 100 ms | 충족 | 2회차 이후 읽기 경로 변경 없음. 보고서 14.4 ms |
| 단계 게이트: 상위 버전 경고 | 충족 | 같음 |

권한 창은 REQ 항목이 아니다(저장 안전성). 지적 1로만 다룬다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 스케줄러 코드를 건드리지 않는다.
- 제품 코드의 `unwrap`/`expect`: 2회차 이후 추가 없음.
- 에러가 삼켜지는 곳: `map_bytes`의 `map_or(0, …)`(ksq.rs 아님, edit.rs). 도달 불가라 정당. 그 밖은 2회차와 같음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 임시 파일을 `create_new`로 만들 때 umask 기본 권한(0644)이고 `set_permissions`가 그 뒤다. 0600 원본을 저장할 때 그 사이에 열린 핸들이 내용을 읽는다(2회차 지적 1의 잔여). `OpenOptionsExt::mode`로 원본 권한을 처음부터 주면 닫힌다(실험으로 확인, 한 줄 수준). `set_permissions`는 umask가 깎은 권한을 보정하려고 남겨도 된다. 재현: 시도 1 | crates/keyscore-core/src/ksq.rs:801-812 |
| 2 | 제안 | `map_bytes`가 `approx_bytes`를 부를 때마다 모르는 키를 JSON으로 직렬화한다. 1 MiB 모르는 키 클립은 편집(분할 등)마다 1 MiB를 직렬화해 문자열을 만든다. 정확성에는 영향이 없고 외부 도구가 만든 큰 모르는 키가 있어야 드러난다. 길이만 세는 함수(재귀로 문자열·배열 길이 합)로 바꾸거나 S8-T3(NF-7)에서 측정하면 된다. 실행하지 않았다(코드로 확인) | crates/keyscore-core/src/edit.rs:476-482 |
| 3 | 제안 | 2회차 제안 4~6의 처리는 타당하다. 4(모르는 키와 값 묶기)는 REQ에 규칙이 없고 undo와 맞바꿈이라 결정 요청(`docs/decisions/2026-10-12-ksq-unknown-keys-on-value-change.md`)과 S2-T5 이월이 맞고, 리뷰 1 대응 문장도 정정되었다. 5(`Value` 팽창)는 NF-7 측정 성격이라 S8-T3 이월이 맞다. 6(읽기 전용 파일)은 저장 흐름 정책이라 S2-T7 이월이 맞다. 7(지적마다 커밋)은 이번 대응에서 지켜졌다(커밋 3개). 병합을 막지 않는다 | docs/reports/s0-t6/report.md |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의

2회차 지적 1~3은 실험과 diff로 해결을 확인했다. 새 변경에서 깨진 것은 하나다. 지적 1은 2회차 지적 1의 목적(남이 못 읽게)이 빈 파일 창에서 아직 닫히지 않은 것이고, 수정은 `create_new` 시점에 권한을 주는 한 줄 수준이다. 위협 모델이 좁고(같은 디렉터리를 읽는 다른 로컬 사용자, 마이크로초 창) 데이터 손실이 아니므로 「범위 재정의」나 결정 요청 전환 사유는 아니다. 프로토콜의 3회 상한에 닿으므로 다음 확인은 전체 재리뷰가 아니라 `review_s0_t6_r3.rs`의 실패 1건이 통과하고 자동 게이트가 유지되는지로 한정하기를 권한다. 그 확인이 통과하면 병합 가능으로 본다. 제안 2·3은 병합 조건이 아니다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(4개, 실패 1개)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(시드 diff 비어 있음)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트(리뷰어 실행, 리뷰 테스트 포함)

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과
- `cargo test --workspace --no-fail-fast`: 실패는 `review_s0_t6_r3.rs`의 1건(지적 1의 재현)뿐. 나머지 전부 통과
- `cargo deny check`: 통과
- `scripts/check-deps.sh`: ok. `scripts/check-deps.test.sh` 11/11. `scripts/bench-check.test.sh` 8/8
