# Task 계획: S0-T8 `ksq-cli`

- 브랜치: `s0/t8-ksq-cli`
- 관련 REQ: 5.14(계이름 텍스트 가져오기·내보내기), 9.5(`.ksq` 형식), AI-18(번들 픽스처 8곡), 6.x 프롬프트 실험실(픽스처 종류)
- 선행 Task: S0-T5(텍스트), S0-T6(`.ksq`)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

- `tools/ksq-cli` 바이너리 crate가 워크스페이스에 들어간다. `keyscore-core`와 `keyscore-text`에만 의존한다.
- 명령은 세 가지다.
  - `ksq check <파일>...`: `.ksq`/`.ksqz`를 읽어 불변식(모델 타입이 보장)과 버전을 검사한다.
    - 경고는 표준 오류에 쓴다.
    - 하나라도 실패하면 종료 코드가 0이 아니다.
  - `ksq2txt <in.ksq> [-o out.txt]`: 프로젝트를 계이름 텍스트로 쓴다.
  - `txt2ksq <in.txt> [-o out.ksq] [--strict]`: 텍스트를 읽어 `.ksq`로 쓴다(느슨 모드가 기본).
- 저장소에 픽스처 8곡 `assets/fixtures/*.ksq`가 있고, 생성 명령 `ksq fixtures <dir>`로 다시 만들 수 있다. 내용은 결정적이며 손으로 만든 원곡이다.
- 게이트: 8개 모두 `ksq check`를 통과한다. `ksq2txt → txt2ksq` 왕복 뒤 프로젝트가 텍스트가 나르는 정보 기준으로 원본과 같다(아래 「불변식」).

## 공개 인터페이스 초안

~~~rust
// tools/ksq-cli/src/lib.rs (바이너리들이 공유, 통합 테스트가 직접 부른다)
pub fn check(path: &Path) -> Result<CheckReport, CliError>;
pub fn project_to_text(project: &Project) -> Result<(String, Vec<Loss>), CliError>;
pub fn text_to_project(text: &str, mode: Mode) -> Result<(Project, Vec<Diagnostic>), CliError>;
pub fn fixtures() -> Vec<(&'static str, Project)>;   // (파일 이름, 프로젝트)
// 바이너리: ksq (check, fixtures), ksq2txt, txt2ksq
~~~

## 프로젝트 ↔ 텍스트 사상

텍스트는 헤더 하나(템포·박자·조)와 트랙마다 시작 0인 클립 하나를 나른다. 그래서 `.ksq`가 담는 것 일부는 텍스트로 가지 못한다. `ksq2txt`는 잃는 정보를 `Loss` 목록으로 돌려주고, 표준 오류에 한 줄씩 쓴다. 출력은 그대로 만든다.

- 트랙 하나의 여러 클립: 텍스트에서는 한 클립으로 합친다. 클립 사이 빈 곳은 쉼표가 된다. 숨김 꼬리는 잘린다.
- 첫 템포·박자표만 헤더로 간다. 뒤따르는 변화는 잃는다.
- 벨로시티와 피치 오프셋: 텍스트 문법이 `v`·오프셋을 내보내지 않으므로 잃는다.
- 텍스트가 나르지 않는 트랙 속성과 모르는 키도 잃는다.
- 같은 이름의 트랙: 이름을 다르게 해야 왕복하므로 `이름 (2)`처럼 뒤에 번호를 붙이고 `Loss`로 알린다.

`txt2ksq`는 헤더를 템포·박자 맵의 0틱 값과 조로, 각 트랙을 트랙 하나와 시작 0 클립 하나로 만든다. 텍스트에 없는 헤더는 `Project::new()` 기본값을 쓴다.

이 사상은 도구의 동작이라 REQ 의미를 바꾸지 않는다(위임된 세부, 루트 `MEMORY.md`). 제품의 가져오기·내보내기 동작은 S3·S7에서 정한다.

## 픽스처 8곡

REQ 실험실 문구(멜로디만, 멜로디+코드, 드럼만, 4트랙 등)를 따르고, 남는 자리는 텍스트 문법의 주요 표기를 고루 덮도록 고른다. 모두 8마디 안팎의 원곡이고, 텍스트가 나르는 정보만 쓴다. 그래야 왕복이 같음으로 검사된다.

| 파일 | 내용 |
| --- | --- |
| `01-melody.ksq` | 멜로디만, 4/4, C장조 |
| `02-melody-chords.ksq` | 멜로디 + 화음 트랙 |
| `03-drums.ksq` | 드럼만(ch10, 라벨) |
| `04-band.ksq` | 4트랙(멜로디·화음·베이스·드럼) |
| `05-lyrics.ksq` | 한국어 가사 붙은 멜로디 |
| `06-waltz.ksq` | 3/4, 단조, 점음표·타이 |
| `07-triplets.ksq` | 셋잇단·셔플 느낌 |
| `08-bass-drums.ksq` | 베이스 + 드럼, 프로그램·뱅크 지정 |

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `fixtures_pass_check_all_eight` | 통합 | `assets/fixtures`에 8개가 있고 모두 `check` 통과 |
| 2 | `fixtures_round_trip_through_text_unchanged` | 통합 | 각 픽스처의 ksq → txt → ksq 결과가 원본과 같음 |
| 3 | `fixtures_match_generator_output` | 통합 | 디스크 파일 = `fixtures()` 생성 결과(바이트 동일) |
| 4 | `fixtures_text_reads_in_strict_mode_without_warnings` | 통합 | 픽스처 텍스트가 엄격 모드로 경고 없이 읽힘 |
| 5 | `check_fails_on_invariant_violation_with_nonzero_exit` | 통합(바이너리) | 깨진 `.ksq`에서 종료 코드 ≠ 0, 메시지에 경로 |
| 6 | `check_fails_on_newer_version` | 통합 | 더 높은 버전 파일 거부 |
| 7 | `ksq2txt_reports_losses_for_multi_clip_tempo_change_velocity` | 단위 | `Loss` 목록 |
| 8 | `ksq2txt_renames_duplicate_track_names_and_round_trips` | 단위 | 이름 중복 처리 |
| 9 | `txt2ksq_then_ksq2txt_round_trips_random_projects` | 속성(proptest) | 텍스트가 나르는 정보만 쓴 무작위 프로젝트 왕복 |
| 10 | `binaries_round_trip_files_via_cli` | 통합(바이너리) | 실제 바이너리로 `-o` 경로 쓰기·읽기 |
| 11 | `txt2ksq_strict_rejects_offsets` | 통합 | `--strict`에서 `~` 거부, 종료 코드 ≠ 0 |

## 불변식

- 왕복 같음의 정의: 트랙 순서, 이름, 채널, 프로그램, 뱅크가 같아야 한다. 트랙마다 클립 하나(시작 0, 같은 길이)이고, 노트의 (시작, 길이, 음높이, 가사)가 같아야 한다. 헤더 템포·박자·조도 같아야 한다.
- 픽스처 쓰기는 `keyscore_core::ksq` 결정적 쓰기를 쓴다. 같은 생성기면 같은 바이트다.
- CLI는 입력 파일을 덮어쓰지 않는다. `-o`가 입력과 같은 경로면 거부한다.
- 의존 방향: `tools/ksq-cli` → core, text만. `scripts/check-deps.sh`에 이 규칙과 테스트를 더한다.

## 범위 밖

- SMF(`.mid`) 변환: `00-conventions.md`는 ksq↔mid를 적지만, S0-T8 범위에 없고 SMF는 S1(midi crate)이다.
- 트랙 매칭, 클립 위치 지정, 가져오기 UI: S3·S7.
- 픽스처의 음악적 품질 평가: 실험실(S7)에서 한다.

## 리스크·결정 요청

- 인자 파싱 의존성은 추가하지 않는다(std만). 의존 허용 목록을 늘리지 않으려는 것이다.
- `anyhow`는 바이너리에서만 허용되지만, 여기서는 `thiserror` 오류 하나로 충분해 쓰지 않는다.
- 결정 요청 없음.
