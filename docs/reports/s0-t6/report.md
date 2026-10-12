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
