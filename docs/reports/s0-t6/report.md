# Task 보고: S0-T6 `.ksq` 직렬화

- PR: 로컬 브랜치 `s0/t6-ksq` (remote 없음)
- 계획 파일: docs/reports/s0-t6/plan.md
- 계획 대비 달라진 점
  - `to_bytes`가 `Result`를 돌려준다. 처음 구현은 gzip 인코더 오류에 `expect`를 썼는데, clippy가 `# Panics` 문서를 요구했다. 메모리 버퍼라 실제로는 실패하지 않지만, 제품 코드의 패닉 가능 지점을 없애려고 오류로 돌려주게 했다. 계획 파일도 고쳤다.
  - 읽기 성능을 계획에 없던 방식으로 다듬었다. 아래 「구현 요약」의 성능 항목 참고.

## 구현 요약

- `src/ksq.rs`
  - 쓰기: 문자열을 직접 조립한다. 키 순서가 고정되고, 2칸 들여쓰기이며, 노트·템포·박자표 항목은 한 줄에 하나다. 한 줄 값은 `, `·`: ` 구분자를 쓴다. 모르는 키는 알려진 키 뒤에 이름 순으로 붙는다.
  - 읽기: `serde_json::Value`로 파싱한 뒤 모델 API로 조립한다. 오류는 `tracks[i].clips[j].notes[k]` 꼴의 경로를 담는다. 템포·박자표는 틱 0부터 엄격히 늘어야 한다. 숨김 꼬리는 crate 내부 `Clip::restore_note`로 받는다.
  - gzip은 첫 두 바이트(1f 8b)로 판별한다.
  - 파일 쓰기: 같은 디렉터리의 `.이름.tmp-pid-n`에 `create_new`로 쓰고, `sync_all`한 뒤 rename한다. 디렉터리 fsync는 최선 노력이다. 실패하면 임시 파일을 지운다.
  - 성능: 처음 구현은 노트 10만 개 읽기에 61 ms가 걸렸다. 원인은 둘이었다. 객체마다 하위 트리를 통째로 복제했고, 노트마다 경로 문자열을 미리 다섯 번 만들었다. 값을 소유권으로 넘기고 노트 경로를 오류 때만 만들도록 바꿔 15 ms가 되었다(JSON 파싱만 11 ms).
- 모델(crate 내부): `Project`·`Track`·`Clip`에 모르는 키 보관(`serde_json::Map`)을 추가했다. 분할하면 오른쪽 클립이 이름·색처럼 모르는 키도 복사받는다. `Note`에는 선택 속성 객체의 모르는 키를 정규화한 JSON 문자열로 보관한다. 이 문자열은 노트의 전체 순서에 포함된다.
- `Note::with_offset`이 `Result`를 돌려주고 절대 피치 범위(0 이상 128반음 미만)를 검사한다(결정 B). `ModelError::AbsolutePitchOutOfRange`.
- 의존: `serde_json`, `flate2`(core), `insta`(dev). `flate2`를 `check-deps.sh`의 core 허용 목록에 더했다. 전이로 `miniz_oxide`·`zlib-rs`·`crc32fast`·`adler2`·`simd-adler32`가 들어오며, `cargo deny`가 통과한다.

## 테스트 결과

~~~
cargo test --workspace: 129 passed
ksq 속성 테스트 256사례: 노트 1,288개 중 숨김 꼬리 119, 오프셋 1,281, 가사 595 (임시 계측)
bench ksq/read_100k_notes 15.5 ms, ksq/write_100k_notes 15.4 ms
check-deps.test 11, bench-check.test 8 통과
~~~

테스트를 먼저 썼다(`6da164e`). `todo!()` 뼈대에서 단위 8개와 통합 3개가 실패하는 것을 확인했다.

스냅샷은 처음 생성된 `.snap.new`의 내용을 눈으로 확인한 뒤 받아들였다. 확인한 내용은 키 순서, 들여쓰기, Δt(0·480·480), 벨로시티 16비트(51492 = MIDI 1.0 100), 오프셋(반음의 절반 = 16777216), 문자열 이스케이프다.

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `ksq_round_trip_random_projects`(+ `ksq_write_read_write_is_identical`) | 통과 |
| 2 | `ksq_unknown_keys_survive_read_write` | 통과 |
| 3 | `ksq_missing_keys_take_defaults` | 통과 |
| 4 | `ksq_newer_format_version_opens_with_warning` | 통과 |
| 5 | `ksq_snapshot_format` | 통과 |
| 6 | `ksq_invalid_files_report_where`(10개 사례 + JSON 문법 오류 행 번호) | 통과 |
| 7 | `ksq_gzip_round_trip_and_detection` | 통과 |
| 8 | `ksq_write_file_failure_keeps_original`, `ksq_write_file_and_read_back` | 통과 |
| 9 | `note_offset_outside_midi2_range_is_rejected` | 통과 |
| 10 | 벤치 `ksq/read_100k_notes`(+ `write`) | 15.5 ms < 100 ms, 기준값 커밋 |

## 자동 게이트

- [x] fmt, clippy -D warnings, test, deny
- [x] `check-deps.sh`, `check-deps.test.sh` 11/11, `bench-check.test.sh` 8/8
- [x] 단계 추가 게이트: 10만 노트 읽기 < 100 ms. 기준값을 갱신했다(템포 벤치 2개도 같은 실행에서 다시 측정, 이전 대비 2% 안)

## 알려진 한계

- 파일에서 읽은 숨김 꼬리는 「꼬리는 분할로만 생긴다」의 예외다. 저장된 분할 상태의 복원으로 보지만, 손으로 고친 파일이 분할 없이 꼬리를 가질 수 있다(계획의 「리스크」).
- 클립 길이 키가 없으면 「가장 늦은 노트 끝, 최소 1」로 정한다. REQ에 없는 해석이다(계획의 「리스크」).
- 쓰기 실패 테스트는 디렉터리 권한과 「대상이 디렉터리」로 실패를 만든다. 프로세스가 죽는 경우는 rename의 원자성에 기댄다(직접 시험하지 않음).
- `.ksq` 텍스트가 `"format_version": 1`보다 새 버전이면 경고 문장 하나만 돌려준다. 버전별 마이그레이션은 버전 2가 생길 때 만든다.
- CC 레인은 모델에 없어서(S6) 쓰지 않는다. 파일에 있으면 모르는 키로 보존된다.

## 리뷰어에게

- 결정적 쓰기와 「쓰기 → 읽기 → 쓰기가 같은 바이트」가 모든 입력에서 성립하는지 봐 달라. 예: 모르는 키 이름이 알려진 키와 같거나, 노트 선택 객체의 모르는 키가 `offset`·`lyric`과 섞이는 경우, 빈 가사 `""`, 정수가 아닌 숫자(1.5, 1e3), 큰 수.
- 읽기에서 패닉이나 무한 메모리 사용으로 이어지는 입력이 있는지 봐 달라.
- `write_file`의 임시 파일 경로와 실패 처리를 봐 달라.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | `key`·`port` 객체의 모르는 키를 버림 (차단) | `Project`에 `key_extra`, `Track`에 `port_extra`(crate 내부)를 더하고, 쓸 때 알려진 키 뒤에 이름 순으로 붙인다. 조성이 「없음」이거나 포트가 가상 포트면 객체 자리가 없어 쓰지 않는다. (리뷰 2 정정: 모르는 키는 값과 묶여 있지 않아, 다른 값으로 바뀐 뒤에도 남아 다시 쓰일 수 있다. 결정 요청을 썼다) |
| 2 | 모르는 키의 실수가 저장마다 바뀜 (차단) | `serde_json`의 `float_roundtrip`을 켰다(`5fea50c`) |
| 3 | 템포·박자표 읽기 O(n²) | 항목을 모아 `TempoMap::from_sorted`·`MeterMap::from_sorted`(crate 내부)로 한 번에 만든다. 순서 검사는 읽기에서 한다 |
| 4 | 역순 노트 읽기 O(n²) | `Clip::restore_notes`(crate 내부)가 모두 검사한 뒤 붙이고 한 번 정렬한다 |
| 5 | 심볼릭 링크를 일반 파일로 덮어씀 | 링크를 따라가 대상 파일 자리에 쓴다. 링크의 링크도 따라가며, 40단계를 넘으면 오류다. 대상이 없는 링크는 링크가 가리키는 경로에 만든다 |
| 6 | 상위 `format_version`을 저장하면 1로 내려 씀 (제안) | REQ에 규칙이 없어 결정 요청 `docs/decisions/2026-10-12-ksq-newer-version-save.md`를 썼다. 저장 흐름은 S2-T7이라 그 Task로 이월했다. 이 병합은 막지 않는다 |
| 7 | 저장하면 권한이 바뀜 (제안) | 받음. 원래 파일의 권한을 임시 파일에 옮긴 뒤 rename한다. 테스트 `ksq_write_file_keeps_existing_permissions` |
| 8 | gzip 해제 상한 없음 (제안) | 받음. 512 MiB에서 멈추고 오류다. 테스트 `ksq_gzip_larger_than_limit_is_rejected`(상한을 작게 준 내부 함수로 시험) |
| 9 | gzip 뒤 쓰레기 바이트 무시 (제안) | 기록만 함. 읽기 결과에 영향이 없고, 다시 저장하면 사라진다 |
| 10 | 상대 경로 저장 시 디렉터리 fsync 생략 (제안) | 받음. 부모가 빈 경로면 `.`을 fsync한다 |
| 11 | 왕복 속성 테스트가 모르는 키·실수를 만들지 않음 | 받음. `ksq_round_trip_with_unknown_keys_everywhere`가 최상위·key·트랙·port·클립·노트 속성 모두에 모르는 키(실수·중첩 포함)를 주입한다 |

테스트를 먼저 썼다(`e2acec2`). 새 테스트 3개와 리뷰어 테스트 7개가 실패하는 것을 확인했다. 이 과정에서 제가 쓴 비교 기준의 허점을 하나 고쳤다. `content()`가 노트를 통째로 비교해서, 주입한 노트 속성의 모르는 키까지 「내용」으로 셌다. 이제 알려진 필드만 비교하고 모르는 키는 따로 검사한다(`d3844b1`).

~~~
cargo test --workspace: 149 passed
bench: ksq/read_100k_notes 14.4 ms(기준 대비 -7.2%), write 15.1 ms
check-deps.test 11, bench-check.test 8 통과
~~~

## 리뷰 대응 (review-2, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 0600 원본 저장 중 임시 파일이 0644로 내용을 담음 | 권한을 내용 쓰기 전에 맞춘다 | `core: Set temp file permissions before writing .ksq content` |
| 2 | 히스토리 크기 추정이 모르는 키를 세지 않음 | 클립·트랙·port·노트의 모르는 키를 직렬화 길이로 더한다 | `core: Count unknown .ksq keys in history size estimates` |
| 3 | `main`의 `.proptest-regressions` 3개를 지움 | 구현자 실수였다. 뼈대 실패로 생긴 새 시드만 지우려다 `rm -f tests/*.proptest-regressions`로 기존 파일까지 지웠다. `main`에서 되살렸고 사례는 모두 통과한다 | `core: Restore proptest regression files deleted by mistake` |
| 4 | `key_extra`·`port_extra`가 값과 묶여 있지 않음 (제안) | 결정 요청 `docs/decisions/2026-10-12-ksq-unknown-keys-on-value-change.md`, S2-T5로 이월. 위 리뷰 1 대응 표의 문장을 정정했다 | `plan:` |
| 5 | `Value` 트리 메모리 팽창 (제안) | S8-T3(NF-7 측정)으로 이월 | `plan:` |
| 6 | 읽기 전용 파일 저장 (제안) | S2-T7(저장 흐름)로 이월 | `plan:` |
| 7 | 지적마다 커밋 하나 (제안) | 받음. 이번 대응은 지적마다 커밋을 나눴다 | |

리뷰어가 남긴 재현 테스트 2개(`ksq_write_file_temp_never_wider_than_original_mode`, `history_bytes_count_unknown_keys_of_split_clip`)가 이제 통과한다.

~~~
cargo test --workspace: 160 passed
~~~

## 리뷰 대응 (review-3, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | 임시 파일을 기본 권한으로 만든 뒤 바꿔, 빈 파일의 짧은 창이 남음 | 유닉스에서 `OpenOptionsExt::mode`로 처음부터 원본 권한으로 만든다. umask가 더 좁혔을 수 있어 그 뒤 원본 권한을 정확히 다시 맞춘다(좁은 쪽에서 넓히는 것뿐이다). 리뷰어 테스트가 3회 연속 통과한다 |
| 2 | `map_bytes`가 편집마다 모르는 키를 직렬화 (제안) | 기록만 함. 정확성 문제가 아니고, 큰 모르는 키는 드물다. 성능 문제가 보이면 크기를 캐시한다 |
| 3 | 2회차 제안 4~6 처리 타당 (확인) | 없음 |

~~~
cargo test --workspace: 164 passed
~~~
