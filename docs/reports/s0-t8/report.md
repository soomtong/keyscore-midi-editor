# Task 보고: S0-T8 `ksq-cli`

- PR: 브랜치 `s0/t8-ksq-cli`
- 계획 파일: `docs/reports/s0-t8/plan.md`
- 작성: 구현자 에이전트가 보낸 보고를 계획자가 옮겼다. 게이트는 계획자가 다시 돌려 확인했다.
- 계획 대비 달라진 점:
  1. 외부 의존: core·text 외에 `serde_json`·`flate2`를 직접 쓴다. 둘 다 core가 이미 쓰는 crate라 `Cargo.lock`에 새 crate는 없다. 이유는 두 가지다.
     - `check`가 `format_version`을 구조적으로 보려면 JSON을 직접 읽어야 한다(`.ksqz`는 gzip을 풀어서). core는 새 버전을 경고 문자열로만 알린다.
     - core의 모르는 키(`extra`)가 `pub(crate)`라, 손실 보고를 위해 `ksq::to_string` 결과를 훑는다.
     - `check-deps.sh`의 ksq-cli 외부 허용 목록은 `thiserror`·`serde_json`·`flate2`·`proptest`이고, 내부 의존 규칙(core·text만)은 그대로다.
  2. 픽스처 생성: `fixtures()`가 `tools/ksq-cli/fixtures-src/*.txt`를 `include_str!`로 박아 엄격 모드로 읽는다. 공개 함수 `write_fixtures(dir)`를 더했다.
  3. 공개 API 추가: `Loss`(13가지 변형, `Display`), `CheckReport`, `CliError`, `cli::{ksq, ksq2txt, txt2ksq}`
  4. 위임된 세부 결정:
     - 클립이 없는 트랙: 텍스트 블록은 비어 있을 수 없으므로 한 마디 쉼표 클립으로 쓰고 `Loss::EmptyTrack`을 낸다.
     - `project_to_text`는 쓴 텍스트를 느슨 모드로 다시 읽어 대조한다.
       - 클립 길이 차이는 `Loss::ClipLength`, 그 밖의 차이는 `Loss::Mismatch`(text 버그 신호)다.
       - 다시 읽지 못하면 `CliError::ExportUnreadable`이다.
     - 손실 판정 기준은 「`txt2ksq`가 만들 값과 다른가」다. 그래서 손실이 없으면 다시 만든 `.ksq`가 원본과 바이트까지 같다.
     - 버전: 새 버전은 실패, `format_version`이 없으면 경고(종료 0)다.
     - 입력과 출력이 같은 파일이면 거부한다. dev/ino를 비교하므로 심볼릭·하드 링크도 잡는다.
     - `-o`가 `.ksqz`면 gzip으로 쓴다. 종료 코드는 0/1/2(사용법 오류)다.
     - 헤더는 늘 셋 다 쓴다(`@key none` 포함).
  5. 테스트 수정: `check_fails_on_invariant_violation_with_nonzero_exit`의 깨진 파일을 「노트 시작을 클립 밖으로」로 바꿨다. 끝만 넘는 노트는 core가 숨김 꼬리로 받아들이는 합법 상태라, 처음 테스트가 틀렸다(커밋 `994d45b` 본문).

## 구현 요약

| 항목 | 위치 | 내용 |
| --- | --- | --- |
| crate | `tools/ksq-cli` | 워크스페이스 멤버. 내부 의존 core·text, 외부 thiserror·serde_json·flate2(dev: proptest) |
| 변환 | `src/convert.rs` | `project_to_text`: 헤더 + 트랙별 시작 0 클립 하나, 이름 중복은 「이름 (n)」, `Loss` 13종, 되읽기 대조. 반대 방향은 `text_to_project` |
| 검사 | `src/check.rs` | core 읽기로 불변식 검사, 새 `format_version` 실패, 버전 없음 경고, `.ksqz` 지원 |
| CLI | `src/cli.rs`, `src/bin/*.rs` | `ksq check`/`ksq fixtures`, `ksq2txt [-o]`, `txt2ksq [-o] [--strict]`. std 인자 해석 |
| 픽스처 | `fixtures-src/*.txt` → `assets/fixtures/*.ksq` | 손으로 쓴 원곡 8곡을 엄격 모드로 읽어 core 결정적 쓰기로 저장 |
| 의존 검사 | `scripts/check-deps.sh`, `check-deps.test.sh` | ksq-cli 규칙(내부 core·text, 외부 허용 목록, 금지 crate)과 깨뜨리는 사례 4개 |
| 문서 | `tools/ksq-cli/README.md` | 명령, 공개 API, 불변식, 픽스처 재생성 방법 |

픽스처 8곡은 모두 8마디 원곡이고, 벨로시티와 오프셋은 없다.

| 파일 | 내용 |
| --- | --- |
| `01-melody` | C장조 4/4, 100 BPM, 멜로디 한 트랙 |
| `02-melody-chords` | F장조 4/4, 90 BPM, 플루트 멜로디 + 마디마다 온음표 화음 |
| `03-drums` | 110 BPM, ch10 드럼만. 8비트, 16분 탐 필인, 셋잇단 하이햇, `@key none` |
| `04-band` | 가단조 4/4, 120 BPM, 리드·일렉피아노·베이스·드럼 4트랙 |
| `05-lyrics` | G장조 4/4, 84 BPM, 한국어 가사 멜로디, 멜리스마 `-` |
| `06-waltz` | 마단조 3/4, 144 BPM, 바이올린 점4분·마디를 넘는 타이 + 쿵-짝-짝 반주 |
| `07-triplets` | B♭장조 4/4, 96 BPM, 8분·4분 셋잇단 리드 + 셔플 베이스 |
| `08-bass-drums` | 마단조 4/4, 124 BPM, 16분 펑크 베이스(bank 8) + 드럼(GM2 리듬 뱅크) |

## 테스트 결과

~~~
cargo test --workspace: 358 passed, 0 failed (ksq-cli: 단위 10, 통합 14, proptest 2)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `fixtures_pass_check_all_eight` | 통과 |
| 2 | `fixtures_round_trip_through_text_unchanged` | 통과 |
| 3 | `fixtures_match_generator_output` | 통과 |
| 4 | `fixtures_text_reads_in_strict_mode_without_warnings` (+ `fixtures_sources_read_in_strict_mode_without_warnings`) | 통과 |
| 5 | `check_fails_on_invariant_violation_with_nonzero_exit` | 통과 |
| 6 | `check_fails_on_newer_version` | 통과 |
| 7 | `ksq2txt_reports_losses_for_multi_clip_tempo_change_velocity` | 통과 |
| 8 | `ksq2txt_renames_duplicate_track_names_and_round_trips` | 통과 |
| 9 | `txt2ksq_then_ksq2txt_round_trips_random_projects` (+ `ksq2txt_without_losses_round_trips_any_project_bytes`) | 통과 |
| 10 | `binaries_round_trip_files_via_cli` | 통과 |
| 11 | `txt2ksq_strict_rejects_offsets` | 통과 |

계획에 없던 테스트도 더했다.
- 손실이 없으면 바이트까지 왕복
- 빈 트랙
- 숨김 꼬리·오프셋·트랙 속성 손실
- 모르는 키 경로
- 클립 길이 반올림
- 헤더 기본값
- gzip과 버전 없는 파일의 `check`
- 입력과 출력이 같은 파일이면 거부
- 사용법 오류의 종료 코드 2
- 느슨 모드 경고와 손실의 stderr 출력

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (358 passed)
- [x] deny
- [x] 단계별 추가 게이트
  - `scripts/check-deps.sh` ok
  - `check-deps.test.sh` 15 passed
  - `bench-check.test.sh` 8 passed

## 알려진 한계

- SMF(`.mid`) 변환은 S1(midi crate)이다.
- 텍스트가 나르지 않는 정보는 잃고 `Loss`로 알린다: 템포·박자 변화, 클립 배치, 벨로시티, 피치 오프셋, 트랙 속성, 모르는 키.
- `06-waltz`의 마디를 넘는 타이(`e2.& | e4`)는 내보내기가 `e1` 한 음으로 쓰면서 그 자리의 마디선을 뺀다. 엄격 모드로 읽히고 왕복도 같지만, 사람이 읽기에는 마디선이 사라진다. 읽기 좋은 출력은 S3에서 다시 본다.

## 리뷰어에게

구현자가 스스로 불안하다고 한 곳:

1. 모르는 키 탐지가 ksq-cli 안의 키 목록(`ROOT/KEY/TRACK/PORT/CLIP/NOTE_KEYS`)에 기댄다. core 스키마와 이중으로 관리하는 셈이다. 목록이 빠지면 손실이 아닌 것을 손실로 보고하는 방향으로 틀린다.
2. 클립이 먼 틱(예: 10^12)에서 시작하면 합친 클립 앞 빈 곳이 쉼표 수백만 개가 된다. 출력 크기 상한이나 경고가 없다.
3. 임의 프로젝트 proptest(「손실 없음 ⇒ 바이트 같음」)는 손실 없는 사례 비율을 재지 않았다(256 사례). 같은 이름 트랙의 「A (2)」 충돌은 단위 테스트 한 건만 덮는다.

덧붙임: `TEXT_VELOCITY = 100`은 text의 비공개 시작 벨로시티를 복사한 값이다. 어긋나면 바이트 왕복 테스트가 깨져 드러난다.
