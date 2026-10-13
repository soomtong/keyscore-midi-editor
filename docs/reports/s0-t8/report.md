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

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

구현자 에이전트가 고치고 표를 보냈으며, 계획자가 옮겼다.

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 출력 크기 상한 없음 (수정) | 텍스트를 만들기 전에 트랙마다 크기를 어림한다(합친 클립 끝까지의 온음표 수 × 쉼표 8바이트 + 노트 96바이트 + 가사·이름 두 배 + 상수). 어림이 정규 `.ksq`(`ksq::to_string`) 바이트의 256배를 넘으면 `CliError::OutputTooLarge`로 거부하고 아무것도 쓰지 않는다(종료 1). 어림이 모자랄 때를 대비해 내보낸 뒤 실제 크기도 잰다. 상한을 받는 `project_to_text_limited`를 더했다. 리뷰 테스트는 기대값 그대로 통과한다 | `ksq-cli: Bound ksq2txt output to 256x the canonical ksq size` |
| 2 | `Mismatch`에도 종료 0 (제안) | 손실 목록을 먼저 찍고 `Mismatch`가 있으면 출력하지 않고 종료 1 | `ksq-cli: Refuse to write ksq2txt output on unexplained mismatch` |
| 3 | `ksq2txt -o` 비원자 쓰기 (제안) | 같은 디렉터리 임시 파일 → fsync → rename. 심볼릭 링크는 대상을 바꾸고 권한을 유지한다 | `ksq-cli: Write ksq2txt -o output via temp file and rename` |
| 4 | BOM 거부 (제안) | 맨 앞 BOM 하나만 뗀다 | `ksq-cli: Skip one leading UTF-8 BOM in txt2ksq input` |
| 5 | proptest의 손실 없는 사례가 드묾 (제안) | 텍스트만 쓴 생성기와 섞은 생성기를 1:1로 섞어 손실 없는 사례를 약 44%로 올렸다. 40% 밑이면 실패하는 테스트를 더했다 | `ksq-cli: Raise lossless share of any_project proptest cases` |

구현자가 알린 점:
- 상한 기준은 입력 파일 바이트가 아니라 정규 `.ksq` 바이트다. 라이브러리와 CLI가 같은 기준을 쓰게 하려는 것이다. `.ksqz` 입력은 압축 전 크기 기준이다.
- 성부가 많은 클립에서는 어림이 모자랄 수 있다. 그때는 실제 크기 검사가 잡지만, 상한 크기까지는 메모리를 쓴다.

## 리뷰 대응 (review-2, 판정: 수정 후 재리뷰)

구현자 에이전트가 고치고 표를 보냈으며, 계획자가 옮겼다.

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 상한 기준을 모르는 키·긴 이름·`.ksqz`로 부풀릴 수 있음 (수정) | 기준을 「텍스트가 나르는 내용만으로 다시 만든 프로젝트」의 정규 `.ksq` 바이트로 바꿨다(시작 0으로 합친 클립, 기본 트랙 속성, 노트의 시작·길이·음높이·가사). `ksq2txt`는 여기에 입력 파일 바이트의 256배를 더 건다(둘 중 작은 쪽). `.ksqz`는 압축된 바이트가 기준이다. 리뷰 테스트는 기대값 그대로 통과한다. 구현자 단위 테스트 하나는 새 기준으로 기대값을 고쳤다 | `ksq-cli: Base the output limit on the text-carried projection` |
| 2 | 어림이 성부별 쉼표를 세지 않음 (수정) | 트랙마다 성부 수의 위쪽 한계(예산 없는 min(노트 수, 16) + 끝 채움 1, 넘는 성부는 결정 문서의 예산 식)로 쉼표와 헤더 줄을 곱해 센다. 노트 상수는 512바이트다. 「어림이 실제 크기 이상」을 확인하는 proptest를 더했다(성부 17–19개 생성기 포함, release 2만 사례) | `ksq-cli: Count per-voice rests and headers in the output estimate` |
| 3 | `-o /dev/null`·FIFO 회귀 (수정) | `-o` 대상이 이미 있고 일반 파일도 디렉터리도 아니면 그대로 열어 쓴다(심볼릭 링크는 대상 기준). `txt2ksq`도 같은 분기로 쓴다(core는 그대로). FIFO·`/dev/null` 테스트를 더했다 | `ksq-cli: Write directly to existing non-regular -o targets` |
| 4 | 쓰기 동작 문서화 (제안) | README에 「-o 쓰기」 절과 core 쓰기 코드 복제 사실을 적었다 | `ksq-cli: Document -o write semantics and duplicated atomic write code` |
| 덧붙임 | proptest 문턱 여유 | 400 → 350/1000, 고정 시드 이유와 시드별 흔들림을 주석에 적었다 | `ksq-cli: Lower lossless-share threshold to 35 percent with rationale` |

구현자가 알린 점:
- 어림이 보수적이라, 노트가 16개를 넘으면서 아주 드문 트랙(예: 노트 20개를 1만 마디에 흩음)은 실제 출력이 상한 안이어도 거부할 수 있다. 내보내기가 쉼표로 못 채우는 간격에서도 새 성부를 열기 때문에 성부 수를 겹침 수로 묶을 수 없다. 평범한 곡과 400마디 픽스처는 여유가 크다.
- 어림 상수는 keyscore-text의 비공개 규칙(16성부 면제, 예산 계수 16·64, 헤더 64·5바이트)을 결정 문서에서 옮긴 값이다. text가 규칙을 바꾸면 함께 고쳐야 하며, 새 proptest가 어림이 실제보다 작아지는 경우를 잡는다.

## 리뷰 대응 (review-3, 판정: 수정 후 재리뷰)

지적 1(평범한 `.ksqz` 거부)과 지적 2(어림 부족)가 서로 당겨서, 계획자가 방향을 정했다. 정밀한 어림 대신 「확실한 위쪽 한계 + 넉넉한 기준 + 사용자 탈출구」다. 구현자 에이전트가 고치고 표를 보냈으며, 계획자가 옮겼다.

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | `.ksqz` 평범한 곡 거부 (차단) | 입력 파일 바이트 조건을 없앴다. 상한은 「텍스트가 나르는 내용으로 다시 만든 프로젝트」의 정규 `.ksq` 바이트 × 256 하나이고, 압축과 무관하다 | `ksq-cli: Drop the input file size cap from the ksq2txt limit` |
| 2 | 어림이 몰린 사건에서 모자람 (수정) | 어림을 확실한 위쪽 한계로 바꿨다. 성부 수 ≤ max(노트 수, 1) + 1이고, 성부마다 헤더 줄 + 쉼표(온음표당 6바이트, 실측 최대 5.07)를 곱하고 노트당 320바이트를 더한다. 논증은 `check_estimate` 주석에 있다. 탈출구로 `ksq2txt --max-output <바이트>`를 더했다. README에 「4분음표 트랙이면 약 1,400마디부터 거부될 수 있다」를 적었다. 「어림 ≥ 실제」 proptest에 먼 몰린 사건 생성기를 넣었다 | `ksq-cli: Make the output estimate a sure upper bound, add --max-output` |
| 3 | `-o /dev/stdout` 회귀 (수정) | `/dev/` 아래 경로(심볼릭 링크를 따라간 결과 포함)는 그대로 열어 쓴다. 두 바이너리 모두다 | `ksq-cli: Write -o paths under /dev/ in place for both binaries` |
| 4 | FIFO 테스트 멈춤 가능성 (제안) | 20초 한도를 두고, 읽는 쪽은 `O_RDWR`로 푼다 | `ksq-cli: Unblock FIFO test reader with O_RDWR and add timeouts` |
| 5 | 성긴 곡 거부 (제안) | 1·2로 해소. 리뷰 3의 1,000마디 곡이 통과한다 | 위와 같음 |

계획자가 승인해 바꾼 리뷰 3 테스트 두 곳(입력과 의도는 그대로, 기본 상한 가정만 바꿈):

- `project_to_text_crammed_events_far_start_estimate_bounds_actual`
  - 바꾼 곳: 맨 앞의 `project_to_text`를 `project_to_text_limited(&p, u64::MAX)`로
  - 수치: 실제 8,316,216, 기본 상한 8,561,664, 어림 60,500,248바이트
  - 이유: 실제가 상한에 3%까지 붙어 있어, 확실한 위쪽 한계로는 기본 상한 안에 들일 수 없다.
- `ksq2txt_output_fifo_reader_closes_early_exits_1_and_keeps_fifo`
  - 바꾼 곳: 바이너리 인자에 `--max-output 100000000000`
  - 이유: 1만 마디 곡이라 기본 상한에서 거부된다. 이 인자를 주면 EPIPE 경로를 그대로 시험한다.

리뷰 1·2 테스트 파일은 바뀌지 않았다(계획자가 diff로 확인).

## 리뷰 대응 (review-4, 판정: 수정 후 재리뷰)

재리뷰 3회를 마쳤지만 남은 지적이 REQ 의미와 무관한 함수 하나의 구현 수정이라, 계획자 판단으로 결정 요청 없이 고치고 좁은 확인 리뷰를 한 번 더 받는다. 구현자 에이전트가 고치고 표를 보냈으며, 계획자가 옮겼다.

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | `/dev/` 판정이 `..`와 상대 링크를 풀지 않음 (수정) | 링크를 따라간 경로의 부모 디렉터리를 `fs::canonicalize`하고 파일 이름을 붙여 판정한다. 파일 자체는 정규화하지 않는다(`/dev/fd/N`이 표준 출력 대상 경로로 바뀌기 때문). 방어로, `/dev/` 밖 특수 파일로 판정했는데 연 핸들이 일반 파일이면 원자적 쓰기로 보낸다. 리뷰 4 실패 테스트 3개가 기대값 그대로 통과한다 | `ksq-cli: Judge /dev/ outputs by the canonical parent directory` |
| 2 | 거부 메시지 예시 값이 너무 작음 (제안) | 그 실행의 어림 값을 `--max-output` 값으로 안내한다. 안내값이면 통과, 1 작으면 거부하는 테스트를 더했다 | `ksq-cli: Suggest the estimated size as the --max-output value` |

리뷰 1–4 테스트 파일은 바뀌지 않았다(계획자가 diff로 확인).

## 리뷰 대응 (review-5, 판정: 수정 후 재리뷰)

구현자 에이전트가 고치고 표를 보냈으며, 계획자가 옮겼다.

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 끝 `/`를 버리고 씀 (수정) | 경로나 링크 대상이 `/`로 끝나면 특수 판정을 하지 않고 원래 경로를 그대로 열어 커널 오류(ENOTDIR 등)를 낸다 | `ksq-cli: Keep the trailing slash of -o paths and surface ENOTDIR` |
| 2 | u64를 넘는 어림의 틀린 안내 (수정) | 어림이 u64를 넘으면 `CliError::OutputBeyondAnyLimit`으로 「어떤 상한으로도 변환할 수 없다」고 알리고 `--max-output`을 안내하지 않는다. README에 예외를 적었다 | `ksq-cli: Report estimates beyond u64 as unconvertible at any limit` |

리뷰 1–5 테스트 파일은 바뀌지 않았다(계획자가 diff로 확인). 구현자가 알린 점: `--max-output`을 아주 크게 주면 정말 거대한 출력도 만들려 한다. 탈출구의 정의대로이며, 사용자가 명시적으로 고른 상한이다.
